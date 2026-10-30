/*---------------------------------------------------------------------------------------------
 *  POLER Studio IDE - Sovereign AI & High-Velocity Text Search Engine
 *  Direct integration of native poler-engine v0.93.0 (Rust, Zig, ASM)
 *
 *  v3 - latency & parity hardening (post-ae6448c7 field report):
 *   - ONE engine attempt per search: after the multi-root gateway
 *     bails, the ripgrep compatibility provider runs directly — the
 *     per-folder engine retries of v2 could rescan the same tree up to
 *     three times before ripgrep started, which is what tripped the
 *     Copilot findTextInFiles tool timeout on huge (OneDrive) trees.
 *   - Governor: 30s base (+10s per extra workspace root, cap 180s);
 *     the semantic probe keeps a tight 10s budget so giant trees fail
 *     fast into the exact grep gateway instead of burning the budget.
 *   - Exclude-aware root pruning: top-level directories matched by the
 *     active exclude globs (node_modules, bower_components) are never
 *     handed to the engine — parity with the ripgrep provider's `-g`
 *     excludes.
 *   - Multi-Root Workspaces: ONE poler-engine process serves every added
 *     workspace root (`--grep-root` flags, rayon-parallel inside the engine);
 *     engine builds without `--grep-root` transparently fall back to
 *     parallel per-root spawns.
 *   - Multi-tier candidate resolution: every root is narrowed to the
 *     sub-project directory implied by `includes`, so unrelated parent
 *     folders are never scanned.
 *   - Semantic search (`--format ai-json`): Copilot receives POLER
 *     ContextAnchors with `enclosing_scope` — whole logical blocks
 *     (functions/scenes) instead of scattered lines, reducing LLM
 *     hallucinations.
 *--------------------------------------------------------------------------------------------*/

import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { CancellationToken } from '../../../../base/common/cancellation.js';
import { URI } from '../../../../base/common/uri.js';
import { Progress } from '../../../../platform/progress/common/progress.js';
import { Range, TextSearchComplete2, TextSearchContext2, TextSearchMatch2, TextSearchProviderOptions, TextSearchQuery2, TextSearchResult2 } from '../common/searchExtTypes.js';
import type { RipgrepTextSearchOptions } from '../common/searchExtTypesInternal.js';
import { IOutputChannel } from './ripgrepSearchUtils.js';
import { getPolerEnginePath } from '../../../../base/node/ripgrep.js';

/**
 * JSON contract of `poler-engine --grep-json` (engine/src/retrieval/grep.rs).
 */
interface IPolerLineMatch {
	line_no: number;
	byte_offset: number;
	text: string;
	matched: boolean;
	ranges?: [number, number][];
}

interface IPolerFileGroup {
	path: string;
	binary: boolean;
	lines: IPolerLineMatch[];
}

interface IPolerGrepResult {
	pattern: string;
	output: string;
	groups: IPolerFileGroup[];
	stats: {
		files_scanned: number;
		files_matched: number;
		lines_matched: number;
		occurrences: number;
		elapsed_ms: number;
		errors: string[];
	};
}

/**
 * JSON contract of `poler-engine -q <query> --format ai-json`
 * (engine/src/output/context_anchor.rs — the AI-facing Context Anchor
 * specification: absolute file paths, 1-based lines, enclosing scope).
 */
interface IPolerScene {
	chapter: string;
	temporal_metric?: string;
	location?: string;
	subjects: string[];
	enclosing_scope: string;
}

interface IPolerContextAnchor {
	file: string;
	rel_file?: string;
	/** 1-based line of the hit; 0 — unknown (unreadable file). */
	line: number;
	/** Preview aligned on the hit line (v0.83.0 contract). */
	preview?: string;
	token: string;
	epsilon: number;
	resonance: number;
	scene: IPolerScene;
	k_hop_relations: [string, string, string][];
	nexus_symbol?: string;
}

interface IPolerSearchResult {
	query: string;
	total_hits: number;
	soft_fallback?: boolean;
	soft_fallback_kind?: string;
	anchors: IPolerContextAnchor[];
}

/**
 * Default per-root time budget for the engine governor. The engine scans
 * at ripgrep-class throughput (~2 GiB/s warm on two cores, measured on a
 * 4.6 GiB C4 corpus), but cold OneDrive/cloud placeholder hydration and
 * first-run Defender checks on Windows can dominate the wall clock, so
 * the budget must cover one full cold pass — killing the engine at 10s
 * only to rescan the same tree with ripgrep doubles the latency for no
 * gain. Override with the POLER_ENGINE_TIMEOUT_MS environment variable.
 */
const DEFAULT_TIMEOUT_MS = 30_000;
const EXTRA_TIMEOUT_PER_ROOT_MS = 10_000;
const MAX_TIMEOUT_MS = 180_000;

/**
 * Tight budget for the semantic probe: the resonant search builds corpus
 * statistics (measured ~0.05 GiB/s, 32.5s on a 1.5 GiB tree), so on large
 * workspaces it must fail fast and let the exact grep gateway (with the
 * full governor budget) take over instead of burning the whole budget.
 */
const SEMANTIC_PROBE_TIMEOUT_MS = 10_000;

/**
 * Cap on the accumulated argv length of one engine invocation: hundreds of
 * pruned top-level scan units would otherwise exceed the 32 KiB Windows
 * CreateProcess command-line limit.
 */
const MAX_ARGV_LENGTH = 8_192;

/**
 * Upper bound of enclosing-scope lines reported as search context for one
 * semantic anchor. Keeps the Copilot context window bounded while still
 * returning whole logical blocks.
 */
const MAX_SEMANTIC_CONTEXT_LINES = 100;

interface IPolerSpawnResult {
	stdout: string;
	stderr: string;
	code: number | null;
	cancelled: boolean;
	/** The governor killed the process (slow scan, not an engine error). */
	timedOut: boolean;
}

/**
 * Multi-tier candidate resolution for one workspace root.
 *
 * Given the `includes` glob patterns of the query, narrow the scan folder to
 * the sub-project directory the user (or the Copilot agent) actually asked
 * for, without scanning unrelated parent folders:
 *
 *  - Tier 1: the include path taken literally under the root
 *    (`src/vs/base` → `<root>/src/vs/base`).
 *  - Tier 2: a relative slice of the include path
 *    (`Документы/Cassiopeia` matched anywhere below the root).
 *  - Tier 3: the last include path segment alone (`…/Cassiopeia`).
 *
 * Returns the original `scanFolder` when no include directory resolves.
 */
export function resolveMultiTierScanFolder(scanFolder: string, includes: readonly string[] | undefined): string {
	if (!includes || !includes.length) {
		return scanFolder;
	}

	for (const inc of includes) {
		const parts = inc.replace(/^\*+\/?/, '').split('/');
		const cleanParts: string[] = [];
		for (const part of parts) {
			if (part.includes('*') || part.includes('?')) {
				break;
			}
			cleanParts.push(part);
		}
		if (cleanParts.length) {
			// Tier 1: direct subpath check
			const subPath = path.join(...cleanParts);
			const candidate1 = path.join(scanFolder, subPath);
			if (isDirectory(candidate1)) {
				return candidate1;
			}

			// Tier 2: relative slice check (handles prefix overlap
			// such as "Документы/Cassiopeia" nested deeper)
			for (let i = 1; i < cleanParts.length; i++) {
				const subSlice = path.join(...cleanParts.slice(i));
				const candidateSlice = path.join(scanFolder, subSlice);
				if (isDirectory(candidateSlice)) {
					return candidateSlice;
				}
			}

			// Tier 3: direct child directory check
			const lastDir = cleanParts[cleanParts.length - 1];
			const candidateLast = path.join(scanFolder, lastDir);
			if (isDirectory(candidateLast)) {
				return candidateLast;
			}
		}
	}

	return scanFolder;
}

function isDirectory(candidate: string): boolean {
	try {
		return fs.existsSync(candidate) && fs.statSync(candidate).isDirectory();
	} catch {
		return false;
	}
}

/**
 * Extract the literal directory-name segment an exclude glob effectively
 * removes in full. A glob that removes the whole `node_modules` tree under
 * any parent — `**` around the bare name — reduces to `node_modules`, and
 * `bower_components` behaves the same. Patterns with several literal
 * segments (`Документы/Cassiopeia` deep under the root) or wildcard
 * segments (`*.code-search` anywhere) exclude only a subtree and must NOT
 * prune a whole top-level directory, so they yield nothing.
 */
export function literalPrunedDirName(glob: string): string | undefined {
	const parts = glob.replace(/\\/g, '/').split('/');
	const segs: string[] = [];
	for (const part of parts) {
		if (!part || part === '**' || part === '*') {
			continue;
		}
		if (/[*?\[{]/.test(part)) {
			return undefined; // wildcard segment: no whole-directory prune
		}
		segs.push(part);
	}
	return segs.length === 1 ? segs[0] : undefined;
}

function collectPrunedDirNames(excludes: readonly unknown[] | undefined): Set<string> {
	const names = new Set<string>();
	if (!excludes) {
		return names;
	}
	for (const exclude of excludes) {
		const pattern = typeof exclude === 'string'
			? exclude
			: ((exclude as any)?.pattern as string | undefined);
		if (typeof pattern !== 'string') {
			continue;
		}
		const dir = literalPrunedDirName(pattern);
		if (dir) {
			names.add(dir.toLowerCase());
		}
	}
	return names;
}

/**
 * Expand one resolved workspace root into engine scan units, skipping
 * top-level directories the active excludes remove entirely (e.g.
 * `node_modules`). Real directories and plain files become scan units —
 * the engine walks each with the same `.gitignore`/hidden-file semantics
 * it applies to a bare root, while symlinked directories are skipped
 * exactly like the engine's own no-follow walk would skip them. Falls
 * back to the bare root on any failure or an oversized expansion.
 */
export function expandRootExcludingPruned(root: string, prunedDirs: ReadonlySet<string>): string[] {
	if (!prunedDirs.size) {
		return [root];
	}
	let entries: fs.Dirent[];
	try {
		entries = fs.readdirSync(root, { withFileTypes: true });
	} catch {
		return [root];
	}
	const units: string[] = [];
	let argvLength = root.length;
	for (const entry of entries) {
		if (entry.name.startsWith('.')) {
			continue; // hidden entries are skipped by the engine walk anyway
		}
		const isSymlinkDir = entry.isSymbolicLink() && !entry.isFile();
		if (isSymlinkDir) {
			continue; // the engine does not follow symlinked directories
		}
		if (entry.isDirectory() && prunedDirs.has(entry.name.toLowerCase())) {
			continue;
		}
		const unit = path.join(root, entry.name);
		argvLength += unit.length + 16;
		if (argvLength > MAX_ARGV_LENGTH) {
			return [root]; // too many units for one command line
		}
		units.push(unit);
	}
	return units.length ? units : [root];
}

export class PolerTextSearchEngine {
	public static readonly ENGINE_VERSION = 'v0.93.0';

	constructor(private readonly outputChannel: IOutputChannel) {}

	public static isAvailable(): boolean {
		return !!getPolerEnginePath();
	}

	/**
	 * Per-folder entry point (single workspace root). Kept for the
	 * per-folder ripgrep fallback path which also handles non-file
	 * schemes (vscode-userdata) through URI translation.
	 */
	public async provideTextSearchResults(
		query: TextSearchQuery2,
		options: RipgrepTextSearchOptions,
		progress: Progress<TextSearchResult2>,
		token: CancellationToken
	): Promise<TextSearchComplete2 | null> {
		const scanFolder = resolveMultiTierScanFolder(options.folderOptions.folder.fsPath, options.folderOptions.includes);
		const prunedDirs = collectPrunedDirNames(options.folderOptions.excludes as readonly unknown[] | undefined);
		const scanUnits = expandRootExcludingPruned(scanFolder, prunedDirs);
		return this.provideTextSearchResultsForRoots(query, scanUnits, this.resolveExtensionsFilter(options.folderOptions.includes), options, progress, token, options.numThreads, 1);
	}

	/**
	 * Multi-root entry point (Stage A.1): a single poler-engine process
	 * scans every added workspace root in parallel through the engine's
	 * internal rayon pool. When the engine binary predates the
	 * `--grep-root` flag, we transparently retry with one parallel
	 * process per root.
	 *
	 * Returns `null` when the engine cannot serve the request so the
	 * caller can fall back to the ripgrep compatibility provider.
	 */
	public async provideTextSearchResultsMultiRoot(
		query: TextSearchQuery2,
		options: TextSearchProviderOptions,
		progress: Progress<TextSearchResult2>,
		token: CancellationToken,
		numThreads?: number
	): Promise<TextSearchComplete2 | null> {
		const polerPath = getPolerEnginePath();
		if (!polerPath) {
			return null;
		}
		if (!query.pattern) {
			return { limitHit: false };
		}

		const folderOptions = options.folderOptions ?? [];
		if (!folderOptions.length) {
			return { limitHit: false };
		}

		// The engine serves the file scheme only; translated schemes
		// (vscode-userdata) go through the per-folder provider.
		if (folderOptions.some(f => f.folder.scheme !== 'file')) {
			this.outputChannel.appendLine('[POLER ENGINE] Multi-root gateway supports file-scheme folders only; delegating to the per-folder provider.');
			return null;
		}

		// Multi-tier candidate resolution per root: isolate each
		// sub-project without scanning unrelated parent folders.
		const resolvedRoots: string[] = [];
		for (const folderOption of folderOptions) {
			const resolved = resolveMultiTierScanFolder(folderOption.folder.fsPath, folderOption.includes);
			if (!resolvedRoots.includes(resolved)) {
				resolvedRoots.push(resolved);
			}
		}

		// A single engine process filters by ONE extension list:
		// apply it only when every root agrees, otherwise the exact
		// include/exclude filtering is left to the downstream
		// TextSearchManager glob tester.
		const perFolderFilters = folderOptions.map(f => this.resolveExtensionsFilter(f.includes));
		const extensionsFilter = (perFolderFilters.length > 0 && perFolderFilters.every(f => f === perFolderFilters[0]))
			? perFolderFilters[0]
			: undefined;

		// Exclude-aware pruning (v3): never hand directories the
		// active excludes remove entirely (node_modules & friends)
		// to the engine — parity with the ripgrep `-g` excludes.
		const prunedDirs = new Set<string>();
		for (const folderOption of folderOptions) {
			for (const name of collectPrunedDirNames(folderOption.excludes as readonly unknown[] | undefined)) {
				prunedDirs.add(name);
			}
		}
		const scanUnits: string[] = [];
		for (const root of resolvedRoots) {
			for (const unit of expandRootExcludingPruned(root, prunedDirs)) {
				if (!scanUnits.includes(unit)) {
					scanUnits.push(unit);
				}
			}
		}

		const singleProcess = await this.provideTextSearchResultsForRoots(query, scanUnits, extensionsFilter, options, progress, token, numThreads, folderOptions.length);
		if (singleProcess !== null) {
			return singleProcess;
		}

		// The engine rejected the multi-root invocation (older binary
		// without --grep-root): retry with parallel per-root spawns.
		this.outputChannel.appendLine(`[POLER ENGINE] ${PolerTextSearchEngine.ENGINE_VERSION} lacks --grep-root; falling back to ${resolvedRoots.length} parallel per-root scans.`);
		const perRootResults = await Promise.all(resolvedRoots.map(root =>
			this.provideTextSearchResultsForRoots(query, [root], extensionsFilter, options, progress, token, numThreads, folderOptions.length)
		));
		if (perRootResults.some(r => r === null)) {
			// At least one root could not be served — let the
			// ripgrep compatibility provider take over.
			return null;
		}
		return { limitHit: perRootResults.some(r => !!r && r.limitHit) };
	}

	/**
	 * Semantic search entry point (Stage A.2): `poler-engine -q <pattern>
	 * --format ai-json` returns ranked ContextAnchors whose
	 * `enclosing_scope` carries the whole logical block (function or
	 * scene). Each anchor is reported as a match plus context lines, so
	 * Copilot's @workspace context sees self-sufficient blocks instead
	 * of scattered single lines.
	 *
	 * v3: the probe runs on a TIGHT budget — the resonant pass builds
	 * corpus statistics and is an order of magnitude slower than the
	 * exact grep gateway, so large workspaces fail fast (10s) and let
	 * the grep gateway (with the full governor budget) take over.
	 */
	public async provideSemanticSearchResults(
		query: TextSearchQuery2,
		options: TextSearchProviderOptions,
		progress: Progress<TextSearchResult2>,
		token: CancellationToken,
		numThreads?: number
	): Promise<TextSearchComplete2 | null> {
		const polerPath = getPolerEnginePath();
		if (!polerPath) {
			return null;
		}
		if (!query.pattern) {
			return { limitHit: false };
		}

		const folderOptions = options.folderOptions ?? [];
		if (!folderOptions.length) {
			return { limitHit: false };
		}
		if (folderOptions.some(f => f.folder.scheme !== 'file')) {
			return null;
		}

		// The resonant search answers identifier/phrase queries;
		// regex mode stays on the exact grep gateway.
		if (query.isRegExp) {
			this.outputChannel.appendLine('[POLER ENGINE] Semantic search skips regexp patterns; using the exact grep gateway.');
			return null;
		}

		const resolvedRoots: string[] = [];
		for (const folderOption of folderOptions) {
			const resolved = resolveMultiTierScanFolder(folderOption.folder.fsPath, folderOption.includes);
			if (!resolvedRoots.includes(resolved)) {
				resolvedRoots.push(resolved);
			}
		}

		const extensionsFilter = this.resolveExtensionsFilter(folderOptions[0].includes);
		const maxScope = (options.previewOptions?.charsPerLine ?? 10_000) * MAX_SEMANTIC_CONTEXT_LINES;

		// The resonant search scans one root per process; multi-root
		// workspaces run the roots in parallel. Query tokens are
		// case-folded inside the engine, so the IDE-side
		// isCaseSensitive flag needs no explicit mapping here.
		const perRoot = await Promise.all(resolvedRoots.map(async root => {
			const args: string[] = [root, '-q', query.pattern, '--format', 'ai-json', '--max-scope', `${maxScope}`];
			if (extensionsFilter) {
				args.push('--extensions', extensionsFilter);
			}
			if (numThreads && numThreads > 0) {
				args.push('--threads', `${Math.max(1, Math.floor(numThreads / resolvedRoots.length))}`);
			}

			this.outputChannel.appendLine(`[POLER ENGINE ${PolerTextSearchEngine.ENGINE_VERSION}] Semantic search (Causal Nexus / ε-ranking): ${polerPath} ${args.slice(1).join(' ')}`);

			// v3: tight probe budget — see SEMANTIC_PROBE_TIMEOUT_MS.
			const probeBudgetMs = Math.min(this.timeoutFor(resolvedRoots.length), SEMANTIC_PROBE_TIMEOUT_MS);
			const spawnResult = await this.spawnPoler(polerPath!, args, root, token, probeBudgetMs);
			if (spawnResult.cancelled) {
				return { limitHit: false } as TextSearchComplete2;
			}
			if (spawnResult.timedOut) {
				this.outputChannel.appendLine(`[POLER ENGINE] Semantic probe exceeded ${probeBudgetMs}ms on ${root}; handing over to the exact grep gateway.`);
				return null;
			}
			if (spawnResult.code !== 0 && !spawnResult.stdout.trim()) {
				if (spawnResult.stderr.trim()) {
					this.outputChannel.appendLine(`[POLER ENGINE] Semantic search notice: ${spawnResult.stderr.trim()}`);
				}
				return null;
			}

			try {
				return this.reportAnchors(spawnResult.stdout, options, progress);
			} catch (err: any) {
				this.outputChannel.appendLine(`[POLER ENGINE] ai-json parse fallback: ${err.message}`);
				return null;
			}
		}));

		if (perRoot.some(r => r === null)) {
			this.outputChannel.appendLine('[POLER ENGINE] Semantic search unavailable; falling back to the exact grep gateway.');
			return null;
		}
		return { limitHit: perRoot.some(r => !!r && r.limitHit) };
	}

	/**
	 * Shared exact-grep engine runner over one or more roots.
	 *
	 * v3: `budgetRootCount` sizes the governor by the number of REAL
	 * workspace roots — exclude-pruned scan units are not separate
	 * budgets (rayon shares one pool for the whole invocation).
	 */
	private async provideTextSearchResultsForRoots(
		query: TextSearchQuery2,
		roots: string[],
		extensionsFilter: string | undefined,
		options: TextSearchProviderOptions,
		progress: Progress<TextSearchResult2>,
		token: CancellationToken,
		numThreads?: number,
		budgetRootCount?: number
	): Promise<TextSearchComplete2 | null> {
		const polerPath = getPolerEnginePath();
		if (!polerPath) {
			return null;
		}
		if (!query.pattern) {
			return { limitHit: false };
		}

		const [primaryRoot, ...extraRoots] = roots;

		const args: string[] = [primaryRoot, '--grep', query.pattern, '--grep-json'];
		for (const extra of extraRoots) {
			args.push('--grep-root', extra);
		}
		if (extensionsFilter) {
			args.push('--extensions', extensionsFilter);
		}
		if (!query.isCaseSensitive) {
			args.push('--grep-i');
		}
		if (query.isRegExp) {
			args.push('--grep-regex');
		}
		if (options.surroundingContext) {
			args.push('--grep-before', `${options.surroundingContext}`);
			args.push('--grep-after', `${options.surroundingContext}`);
		}
		if (numThreads && numThreads > 0) {
			args.push('--threads', `${numThreads}`);
		}
		if (options.maxResults) {
			args.push('--grep-max-count', `${options.maxResults}`);
		}

		this.outputChannel.appendLine(`[POLER ENGINE ${PolerTextSearchEngine.ENGINE_VERSION}] Spawning: ${polerPath} ${args.slice(1).join(' ')}`);

		const budgetMs = this.timeoutFor(budgetRootCount ?? roots.length);
		const spawnResult = await this.spawnPoler(polerPath, args, primaryRoot, token, budgetMs);
		if (spawnResult.cancelled) {
			return { limitHit: false };
		}
		if (spawnResult.timedOut) {
			// v3: the governor killed a RUNNING scan (cold cloud
			// hydration, giant tree). Report it distinctly: the
			// ripgrep fallback still gets exactly one chance, and
			// the OS file cache the engine just warmed makes that
			// pass far cheaper than a blind rescan.
			this.outputChannel.appendLine(`[POLER ENGINE] Governor timeout (>${budgetMs}ms) while scanning ${roots.length} root(s); handing the query to the ripgrep compatibility provider.`);
			return null;
		}

		if (spawnResult.code !== 0 && !spawnResult.stdout.trim()) {
			if (spawnResult.stderr.trim()) {
				this.outputChannel.appendLine(`[POLER ENGINE] Notice/Error: ${spawnResult.stderr.trim()}`);
			}
			// Exit code 1 — no matches (grep contract); exit code 2
			// — fatal. Multi-root flag rejection also exits 2 and
			// is reported by the caller through a null result.
			if (spawnResult.code === 1) {
				return { limitHit: false };
			}
			if (spawnResult.code === 2 && /unexpected argument/i.test(spawnResult.stderr) && spawnResult.stderr.includes('--grep-root')) {
				// Older engine: let the caller retry per-root.
				return null;
			}
			return null;
		}

		try {
			return this.reportGrepGroups(spawnResult.stdout, options, progress);
		} catch (err: any) {
			this.outputChannel.appendLine(`[POLER ENGINE] JSON parse fallback: ${err.message}`);
			return null;
		}
	}

	/**
	 * Convert a `--grep-json` report into search results.
	 */
	private reportGrepGroups(stdout: string, options: TextSearchProviderOptions, progress: Progress<TextSearchResult2>): TextSearchComplete2 {
		const json: IPolerGrepResult = JSON.parse(stdout);
		let reportedCount = 0;
		const maxResults = options.maxResults ?? Number.MAX_SAFE_INTEGER;
		let limitHit = false;

		for (const group of json.groups) {
			if (limitHit) {
				break;
			}

			const uri = URI.file(group.path);

			for (const line of group.lines) {
				if (!line.matched) {
					continue;
				}

				const lineIndex = Math.max(0, line.line_no - 1);
				const rawText = line.text ?? '';
				const ranges = (line.ranges && line.ranges.length > 0)
					? line.ranges.map(([startCol, endCol]) => ({
						sourceRange: new Range(lineIndex, startCol, lineIndex, endCol),
						previewRange: new Range(0, startCol, 0, endCol),
					}))
					: [{
						sourceRange: new Range(lineIndex, 0, lineIndex, rawText.length),
						previewRange: new Range(0, 0, 0, rawText.length),
					}];

				const match = new TextSearchMatch2(uri, ranges, rawText);
				progress.report(match);
				reportedCount++;

				if (reportedCount >= maxResults) {
					limitHit = true;
					break;
				}
			}
		}

		const elapsed = json.stats ? json.stats.elapsed_ms : 0;
		const scanned = json.stats ? json.stats.files_scanned : 0;
		this.outputChannel.appendLine(`[POLER ENGINE ${PolerTextSearchEngine.ENGINE_VERSION}] Resonant search completed in ${elapsed}ms (${scanned} files scanned, ${reportedCount} matches reported)`);

		return { limitHit };
	}

	/**
	 * Convert an ai-json `SearchResult` into search results: one
	 * `TextSearchMatch2` on the anchor line plus `TextSearchContext2`
	 * lines carrying the whole `enclosing_scope` block.
	 */
	private reportAnchors(stdout: string, options: TextSearchProviderOptions, progress: Progress<TextSearchResult2>): TextSearchComplete2 {
		const json: IPolerSearchResult = JSON.parse(stdout);
		let reportedCount = 0;
		const maxResults = options.maxResults ?? Number.MAX_SAFE_INTEGER;
		let limitHit = false;
		const charsPerLine = options.previewOptions?.charsPerLine ?? 10_000;

		for (const anchor of json.anchors) {
			if (reportedCount >= maxResults) {
				limitHit = true;
				break;
			}

			const uri = URI.file(anchor.file);
			const lineIndex = Math.max(0, anchor.line - 1);
			const previewText = anchor.preview ?? firstLine(anchor.scene.enclosing_scope) ?? anchor.token;

			// Context lines: the enclosing scope IS the logical
			// block (function/scene). When the caller explicitly
			// asks for a narrow context window we honor it;
			// otherwise the whole bounded block is returned so
			// Copilot sees self-sufficient code.
			const scopeLines = anchor.scene.enclosing_scope.split(/\r?\n/);
			const hitOffset = offsetOfLine(scopeLines, previewText);
			const blockStartLine = Math.max(0, lineIndex - hitOffset);
			let from = 0;
			let to = Math.min(scopeLines.length, MAX_SEMANTIC_CONTEXT_LINES);
			if (options.surroundingContext > 0) {
				from = Math.max(0, hitOffset - options.surroundingContext);
				to = Math.min(scopeLines.length, hitOffset + options.surroundingContext + 1);
			}
			for (let i = from; i < to; i++) {
				if (i === hitOffset) {
					continue; // the hit line is reported as the match itself
				}
				const text = truncateLine(scopeLines[i], charsPerLine);
				progress.report(new TextSearchContext2(uri, text, blockStartLine + i));
			}

			const tokenIndex = previewText.indexOf(anchor.token);
			const ranges = tokenIndex >= 0
				? [{
					sourceRange: new Range(lineIndex, tokenIndex, lineIndex, tokenIndex + anchor.token.length),
					previewRange: new Range(0, tokenIndex, 0, tokenIndex + anchor.token.length),
				}]
				: [{
					sourceRange: new Range(lineIndex, 0, lineIndex, previewText.length),
					previewRange: new Range(0, 0, 0, previewText.length),
				}];

			progress.report(new TextSearchMatch2(uri, ranges, previewText));
			reportedCount++;
		}

		const totalHits = json.total_hits ?? reportedCount;
		this.outputChannel.appendLine(`[POLER ENGINE ${PolerTextSearchEngine.ENGINE_VERSION}] Semantic anchors: ${reportedCount}/${totalHits} blocks reported (ε-ranked, enclosing_scope context)`);

		if (totalHits > reportedCount) {
			limitHit = true;
		}
		return { limitHit };
	}

	/**
	 * Extract a comma-separated extension filter from the query include
	 * patterns (e.g. an include selector `*.{ts,tsx}` resolves to the
	 * extension list `ts,tsx`).
	 */
	private resolveExtensionsFilter(includes: readonly string[] | undefined): string | undefined {
		if (!includes || !includes.length) {
			return undefined;
		}
		for (const inc of includes) {
			const extMatch = inc.match(/\*\.([a-zA-Z0-9_,{}]+)$/);
			if (extMatch) {
				return extMatch[1].replace(/[{}]/g, '');
			}
		}
		return undefined;
	}

	/**
	 * Governor budget: base plus one extra unit per additional root.
	 */
	private timeoutFor(rootCount: number): number {
		const override = process.env.POLER_ENGINE_TIMEOUT_MS ? parseInt(process.env.POLER_ENGINE_TIMEOUT_MS, 10) : NaN;
		if (!Number.isNaN(override) && override > 0) {
			return override;
		}
		const base = DEFAULT_TIMEOUT_MS + Math.max(0, rootCount - 1) * EXTRA_TIMEOUT_PER_ROOT_MS;
		return Math.min(base, MAX_TIMEOUT_MS);
	}

	/**
	 * Spawn the engine and collect its output with cancellation and a
	 * hard time budget.
	 */
	private spawnPoler(polerPath: string, args: string[], cwd: string, token: CancellationToken, timeoutMs: number): Promise<IPolerSpawnResult> {
		return new Promise((resolve) => {
			let isCancelled = false;
			let timedOut = false;
			let settled = false;
			const proc = cp.spawn(polerPath, args, {
				cwd,
				maxBuffer: 64 * 1024 * 1024,
				windowsHide: true,
			});

			const finish = (result: IPolerSpawnResult) => {
				if (settled) {
					return;
				}
				settled = true;
				clearTimeout(timeoutTimer);
				resolve(result);
			};

			const timeoutTimer = setTimeout(() => {
				if (!proc.killed) {
					timedOut = true;
					this.outputChannel.appendLine(`[POLER ENGINE] Timeout (>${timeoutMs}ms) on: ${cwd}. Aborting.`);
					try { proc.kill(); } catch { /* ignore */ }
					finish({ stdout: '', stderr: `timeout after ${timeoutMs}ms`, code: null, cancelled: false, timedOut: true });
				}
			}, timeoutMs);

			token.onCancellationRequested(() => {
				isCancelled = true;
				try {
					proc.kill();
				} catch {
					// Ignore kill errors on cancelled process
				}
			});

			let stdout = '';
			let stderr = '';

			proc.stdout!.on('data', (data) => {
				stdout += data.toString('utf8');
			});
			proc.stderr!.on('data', (data) => {
				stderr += data.toString('utf8');
			});

			proc.on('error', (err) => {
				this.outputChannel.appendLine(`[POLER ENGINE] Process error: ${err.message}. Falling back to Ripgrep.`);
				finish({ stdout: '', stderr: err.message, code: null, cancelled: false, timedOut: false });
			});

			proc.on('close', (code) => {
				finish({ stdout, stderr, code, cancelled: isCancelled, timedOut });
			});
		});
	}
}

function firstLine(text: string): string {
	const idx = text.indexOf('\n');
	return idx === -1 ? text : text.substring(0, idx);
}

/**
 * Index of the hit line inside the enclosing-scope line array. Per the
 * v0.83.0 contract `preview` is aligned on the hit line, while
 * `enclosing_scope` starts at the beginning of the logical block: the hit
 * preview line is located inside the scope to recover the block offset.
 * Falls back to 0 (scope starts at the hit) when the preview line cannot
 * be located.
 */
function offsetOfLine(scopeLines: readonly string[], previewText: string): number {
	const head = firstLine(previewText).trim();
	if (head) {
		const exact = scopeLines.findIndex(l => l === head);
		if (exact >= 0) {
			return exact;
		}
		const trimmed = scopeLines.findIndex(l => l.trim() === head);
		if (trimmed >= 0) {
			return trimmed;
		}
	}
	return 0;
}

function truncateLine(line: string, charsPerLine: number): string {
	if (line.length <= charsPerLine) {
		return line;
	}
	return line.substring(0, charsPerLine);
}

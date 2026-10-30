/*---------------------------------------------------------------------------------------------
 *  POLER Studio IDE - Sovereign AI & High-Velocity Text Search Engine
 *  Direct integration of native poler-engine v0.93.0 (Rust, Zig, ASM)
 *
 *  v2 - Copilot @workspace search gateway:
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
 * Default per-root time budget for the engine governor. Multi-root runs get
 * one extra base unit per additional root (rayon shares one pool inside a
 * single process, so the budget must cover all roots). Override with the
 * POLER_ENGINE_TIMEOUT_MS environment variable.
 */
const DEFAULT_TIMEOUT_MS = 10_000;
const EXTRA_TIMEOUT_PER_ROOT_MS = 5_000;
const MAX_TIMEOUT_MS = 120_000;

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
		return this.provideTextSearchResultsForRoots(query, [scanFolder], this.resolveExtensionsFilter(options.folderOptions.includes), options, progress, token, options.numThreads);
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

		const singleProcess = await this.provideTextSearchResultsForRoots(query, resolvedRoots, extensionsFilter, options, progress, token, numThreads);
		if (singleProcess !== null) {
			return singleProcess;
		}

		// The engine rejected the multi-root invocation (older binary
		// without --grep-root): retry with parallel per-root spawns.
		this.outputChannel.appendLine(`[POLER ENGINE] ${PolerTextSearchEngine.ENGINE_VERSION} lacks --grep-root; falling back to ${resolvedRoots.length} parallel per-root scans.`);
		const perRootResults = await Promise.all(resolvedRoots.map(root =>
			this.provideTextSearchResultsForRoots(query, [root], options, progress, token, numThreads)
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

			const spawnResult = await this.spawnPoler(polerPath!, args, root, token, this.timeoutFor(1));
			if (spawnResult.cancelled) {
				return { limitHit: false } as TextSearchComplete2;
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
	 */
	private async provideTextSearchResultsForRoots(
		query: TextSearchQuery2,
		roots: string[],
		extensionsFilter: string | undefined,
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

		const spawnResult = await this.spawnPoler(polerPath, args, primaryRoot, token, this.timeoutFor(roots.length));
		if (spawnResult.cancelled) {
			return { limitHit: false };
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
					this.outputChannel.appendLine(`[POLER ENGINE] Timeout (>${timeoutMs}ms) on: ${cwd}. Aborting.`);
					try { proc.kill(); } catch { /* ignore */ }
					finish({ stdout: '', stderr: `timeout after ${timeoutMs}ms`, code: null, cancelled: false });
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
				finish({ stdout: '', stderr: err.message, code: null, cancelled: false });
			});

			proc.on('close', (code) => {
				finish({ stdout, stderr, code, cancelled: isCancelled });
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

/*---------------------------------------------------------------------------------------------
 *  POLER Studio IDE - Sovereign AI & High-Velocity Text Search Engine
 *  Direct integration of native poler-engine v0.93.0 (Rust, Zig, ASM)
 *--------------------------------------------------------------------------------------------*/

import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { CancellationToken } from '../../../../base/common/cancellation.js';
import { URI } from '../../../../base/common/uri.js';
import { Progress } from '../../../../platform/progress/common/progress.js';
import { Range, TextSearchComplete2, TextSearchMatch2, TextSearchQuery2, TextSearchResult2 } from '../common/searchExtTypes.js';
import type { RipgrepTextSearchOptions } from '../common/searchExtTypesInternal.js';
import { IOutputChannel } from './ripgrepSearchUtils.js';
import { getPolerEnginePath } from '../../../../base/node/ripgrep.js';

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

export class PolerTextSearchEngine {
	constructor(private readonly outputChannel: IOutputChannel) {}

	public static isAvailable(): boolean {
		return !!getPolerEnginePath();
	}

	public async provideTextSearchResults(
		query: TextSearchQuery2,
		options: RipgrepTextSearchOptions,
		progress: Progress<TextSearchResult2>,
		token: CancellationToken
	): Promise<TextSearchComplete2 | null> {
		const polerPath = getPolerEnginePath();
		if (!polerPath) {
			return null;
		}

		if (!query.pattern) {
			return { limitHit: false };
		}

		let scanFolder = options.folderOptions.folder.fsPath;
		let extensionsFilter: string | undefined;

		if (options.folderOptions.includes && options.folderOptions.includes.length) {
			for (const inc of options.folderOptions.includes) {
				const extMatch = inc.match(/\*\.([a-zA-Z0-9_,{}]+)$/);
				if (extMatch) {
					extensionsFilter = extMatch[1].replace(/[{}]/g, '');
				}

				const parts = inc.replace(/^\*+\/?/, '').split('/');
				const cleanParts: string[] = [];
				for (const part of parts) {
					if (part.includes('*') || part.includes('?')) {
						break;
					}
					cleanParts.push(part);
				}
				if (cleanParts.length) {
					// 1. Direct subpath check
					const subPath = path.join(...cleanParts);
					const candidate1 = path.join(scanFolder, subPath);
					if (fs.existsSync(candidate1) && fs.statSync(candidate1).isDirectory()) {
						scanFolder = candidate1;
						break;
					}

					// 2. Relative slice check (handles prefix overlap e.g. "Документы/Cassiopeia")
					let matchedSubdir = false;
					for (let i = 0; i < cleanParts.length; i++) {
						const subSlice = path.join(...cleanParts.slice(i));
						const candidateSlice = path.join(scanFolder, subSlice);
						if (fs.existsSync(candidateSlice) && fs.statSync(candidateSlice).isDirectory()) {
							scanFolder = candidateSlice;
							matchedSubdir = true;
							break;
						}
					}
					if (matchedSubdir) {
						break;
					}

					// 3. Direct child directory check
					const lastDir = cleanParts[cleanParts.length - 1];
					const candidateLast = path.join(scanFolder, lastDir);
					if (fs.existsSync(candidateLast) && fs.statSync(candidateLast).isDirectory()) {
						scanFolder = candidateLast;
						break;
					}
				}
			}
		}

		const args: string[] = [scanFolder, '--grep', query.pattern, '--grep-json'];

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

		if (options.maxResults) {
			args.push('--grep-max-count', `${options.maxResults}`);
		}

		this.outputChannel.appendLine(`[POLER ENGINE v0.93.0] Spawning: ${polerPath} ${args.slice(1).join(' ')}`);

		return new Promise((resolve) => {
			let isCancelled = false;
			const proc = cp.spawn(polerPath, args, {
				cwd: scanFolder,
				maxBuffer: 64 * 1024 * 1024,
				windowsHide: true,
			});

			const timeoutTimer = setTimeout(() => {
				if (!proc.killed) {
					this.outputChannel.appendLine(`[POLER ENGINE] Timeout (>10s) on folder: ${scanFolder}. Aborting.`);
					try { proc.kill(); } catch {}
					resolve(null);
				}
			}, 10000);

			token.onCancellationRequested(() => {
				isCancelled = true;
				clearTimeout(timeoutTimer);
				try {
					proc.kill();
				} catch {
					// Ignore kill errors on cancelled process
				}
			});

			let stdout = '';
			let stderr = '';

			proc.stdout.on('data', (data) => {
				stdout += data.toString('utf8');
			});

			proc.stderr.on('data', (data) => {
				stderr += data.toString('utf8');
			});

			proc.on('error', (err) => {
				this.outputChannel.appendLine(`[POLER ENGINE] Process error: ${err.message}. Falling back to Ripgrep.`);
				resolve(null);
			});

			proc.on('close', (code) => {
				clearTimeout(timeoutTimer);
				if (isCancelled) {
					resolve({ limitHit: false });
					return;
				}

				if (code !== 0 && !stdout.trim()) {
					if (stderr.trim()) {
						this.outputChannel.appendLine(`[POLER ENGINE] Notice/Error: ${stderr.trim()}`);
					}
					// Return empty results if nothing found (grep exit code 1) or null if crashed
					if (code === 1) {
						resolve({ limitHit: false });
						return;
					}
					resolve(null);
					return;
				}

				try {
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
					this.outputChannel.appendLine(`[POLER ENGINE v0.93.0] Resonant search completed in ${elapsed}ms (${scanned} files scanned, ${reportedCount} matches reported)`);

					resolve({ limitHit });
				} catch (err: any) {
					this.outputChannel.appendLine(`[POLER ENGINE] JSON parse fallback: ${err.message}`);
					resolve(null);
				}
			});
		});
	}
}

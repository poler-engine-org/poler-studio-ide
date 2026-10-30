import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { PolerEditCoreService } from './editCoreService';

interface IPolerEditLargeFileInput {
	filePath: string;
	query?: string;
	startLine?: number;
	lineCount?: number;
	caseSensitive?: boolean;
}

const DEFAULT_WINDOW_LINES = 128;
const MAX_WINDOW_LINES = 2048;

export function registerPolerEditLargeFileTool(service: PolerEditCoreService): vscode.Disposable {
	return vscode.lm.registerTool<IPolerEditLargeFileInput>('poler_edit_large_file', {
		invoke: async (options, token) => {
			const filePath = await resolveWorkspaceFilePath(options.input.filePath);
			const document = await service.openDocument(filePath);
			if (token.isCancellationRequested) {
				throw new vscode.CancellationError();
			}

			if (options.input.query && options.input.startLine === undefined) {
				const tokenBudget = options.tokenizationOptions?.tokenBudget ?? 2048;
				const matchLimit = Math.max(1, Math.floor(tokenBudget / 12));
				const search = await document.search(options.input.query, options.input.caseSensitive ?? false, matchLimit);
				const hits = search.hits.map(hit => `L${hit.line + 1}:${hit.col + 1}`).join('\n');
				const output = [
					`POLER exact search: ${path.basename(filePath)}`,
					`Matches returned: ${search.hits.length}; scanned ${search.scannedBytes} bytes in ${search.elapsedMs} ms${search.truncated ? '; more matches exist' : ''}.`,
					hits || 'No matches found.',
					search.truncated && search.hits.length ? `Continue in line windows starting at or before L${search.hits[search.hits.length - 1].line + 1}; pass startLine and lineCount.` : '',
				].filter(Boolean).join('\n');
				return new vscode.LanguageModelToolResult([new vscode.LanguageModelTextPart(output)]);
			}

			const firstLine = Math.max(0, Math.floor((options.input.startLine ?? 1) - 1));
			const requestedLineCount = Math.min(MAX_WINDOW_LINES, Math.max(1, Math.floor(options.input.lineCount ?? DEFAULT_WINDOW_LINES)));
			const lines = await document.viewport(firstLine, requestedLineCount);
			const query = options.input.query;
			const outputLines = lines
				.filter(line => !query || (options.input.caseSensitive
					? line.text.includes(query)
					: line.text.toLocaleLowerCase().includes(query.toLocaleLowerCase())))
				.map(line => `${line.line + 1}: ${line.text}${line.truncated ? ' [line truncated]' : ''}`);
			const stats = await document.stats();
			const header = `POLER ${path.basename(filePath)} | lines ${firstLine + 1}-${firstLine + lines.length}${stats.lines ? ` of ${stats.lines}` : ''}${query ? ` | literal matches for ${JSON.stringify(options.input.query)}` : ''}`;
			let includedLines = outputLines;
			let resultText = `${header}\n${includedLines.join('\n') || 'No matching lines in this window.'}`;
			const tokenization = options.tokenizationOptions;
			let shortened = false;
			if (tokenization) {
				while (includedLines.length > 0 && await tokenization.countTokens(resultText, token) > tokenization.tokenBudget) {
					shortened = true;
					if (includedLines.length > 1) {
						includedLines = includedLines.slice(0, Math.max(1, Math.floor(includedLines.length / 2)));
					} else {
						const shortenedLine = includedLines[0].slice(0, Math.floor(includedLines[0].length / 2));
						includedLines = shortenedLine ? [`${shortenedLine} [line shortened]`] : [];
					}
					resultText = `${header}\n${includedLines.join('\n')}`;
				}
			}
			if (shortened) {
				const lastLineNumber = includedLines.length ? Number(includedLines[includedLines.length - 1].match(/^(\d+):/)?.[1]) : firstLine + lines.length;
				resultText += `\nWindow shortened to fit the model token budget; continue at or before line ${lastLineNumber}.`;
			}
			return new vscode.LanguageModelToolResult([new vscode.LanguageModelTextPart(resultText)]);
		},
		prepareInvocation: options => ({ invocationMessage: `Reading with POLER Edit: ${path.basename(options.input.filePath)}` }),
	});
}

async function resolveWorkspaceFilePath(filePath: string): Promise<string> {
	if (!vscode.workspace.isTrusted) {
		throw new Error('POLER Edit file access requires a trusted workspace.');
	}
	if (!path.isAbsolute(filePath)) {
		throw new Error('filePath must be an absolute path inside the current workspace.');
	}

	const realPath = await fs.promises.realpath(filePath);
	const stat = await fs.promises.stat(realPath);
	if (!stat.isFile()) {
		throw new Error('filePath must point to a regular file.');
	}

	const workspaceRoots = await Promise.all((vscode.workspace.workspaceFolders ?? [])
		.filter(folder => folder.uri.scheme === 'file')
		.map(folder => fs.promises.realpath(folder.uri.fsPath)));
	const isInWorkspace = workspaceRoots.some(root => {
		const relativePath = path.relative(root, realPath);
		return relativePath === '' || (!path.isAbsolute(relativePath) && relativePath !== '..' && !relativePath.startsWith(`..${path.sep}`));
	});
	if (!isInWorkspace) {
		throw new Error('filePath must be inside an open workspace folder.');
	}
	return realPath;
}

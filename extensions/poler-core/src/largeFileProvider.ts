/*---------------------------------------------------------------------------------------------
 *  POLER Studio Core - Large File Provider (Zero-Copy Piece-Table windows)
 *
 *  Files above `poler.editCore.largeFileThresholdMB` (default 50 MB) are
 *  served through the native poler-edit-core piece table instead of being
 *  fully materialized into Monaco's line array:
 *
 *   - the `poler-edit` virtual scheme renders a bounded window of lines
 *     fetched through the `viewport` protocol command, so multi-GB dumps
 *     and logs open instantly;
 *   - navigation (go to line, next/previous window) and in-file search are
 *     executed by the native engine (Aho-Corasick over mmap, `search`
 *     command) and simply re-render the window;
 *   - the window offset is carried in the URI query so every rendered
 *     line maps transparently onto the real file line
 *     `windowStart + virtualLine - BANNER_LINES`, which is exactly the
 *     offset translation Language Server Protocol consumers need.
 *--------------------------------------------------------------------------------------------*/

import * as path from 'path';
import * as vscode from 'vscode';
import { PolerEditCoreDocument, PolerEditCoreService } from './editCoreService';

/**
 * Rendered lines above the document content: a metadata banner describing
 * the native document. Virtual line `v` maps to real line
 * `windowStart + v - BANNER_LINES`.
 */
const BANNER_LINES = 2;

/**
 * URI shape: `poler-edit://native?file=<encoded fsPath>&window=<startLine>`
 */
export function buildPolerEditUri(filePath: string, windowStart: number): vscode.Uri {
    return vscode.Uri.from({
        scheme: PolerEditCoreService.SCHEME,
        authority: 'native',
        query: `file=${encodeURIComponent(filePath)}&window=${Math.max(0, windowStart)}`
    });
}

export function decodePolerEditUri(uri: vscode.Uri): { filePath: string; windowStart: number } {
    const params = new URLSearchParams(uri.query);
    const filePath = params.get('file') ?? '';
    const windowStart = Math.max(0, parseInt(params.get('window') ?? '0', 10) || 0);
    return { filePath, windowStart };
}

export class PolerEditCoreLargeFileProvider implements vscode.TextDocumentContentProvider {
    public readonly onDidChange: vscode.Event<vscode.Uri>;

    constructor(private readonly service: PolerEditCoreService) {
        this.onDidChange = service.onDidChangeDocument;
    }

    async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
        const { filePath, windowStart: startLine } = decodePolerEditUri(uri);
        if (!filePath) {
            return 'POLER Piece-Table: malformed poler-edit URI (missing file parameter)';
        }

        let doc: PolerEditCoreDocument;
        try {
            doc = await this.service.openDocument(filePath);
        } catch (err: any) {
            return `POLER Piece-Table: unable to open ${filePath}\n${err.message ?? String(err)}`;
        }

        const windowLines = this.windowLines();
        const lines = await doc.viewport(startLine, windowLines);
        const stats = await doc.stats();

        const header = [
            `POLER Piece-Table • ${path.basename(filePath)} • ${formatBytes(stats.bytes)} bytes • lines ${stats.lines ?? '?'} (index ${stats.indexedBytes === stats.bytes ? 'complete' : 'pending'})`,
            `Window: lines ${startLine + 1}…${startLine + lines.length}${stats.lines ? ` of ${stats.lines}` : ''} • real line = window line + ${startLine - BANNER_LINES} • POLER: Piece-Table → Go To Line / Find / Next Window`
        ];

        const body = lines.map(l => l.text).join('\n');
        return header.join('\n') + '\n' + body;
    }

    /**
     * Real (0-based) file line for a virtual line inside the rendered
     * window: the transparent Piece-Table ↔ LSP position translation.
     */
    public static virtualLineToRealLine(uri: vscode.Uri, virtualLine: number): number {
        return decodePolerEditUri(uri).windowStart + virtualLine - BANNER_LINES;
    }

    /**
     * Virtual line inside the rendered window for a real (0-based) file
     * line.
     */
    public static realLineToVirtualLine(uri: vscode.Uri, realLine: number): number {
        return realLine - decodePolerEditUri(uri).windowStart + BANNER_LINES;
    }

    private windowLines(): number {
        return Math.max(256, vscode.workspace.getConfiguration('poler').get<number>('editCore.windowLines', 2048));
    }

    /**
     * Reveal a real (0-based) file line by shifting the window and
     * refreshing the virtual document.
     */
    public async revealLine(doc: PolerEditCoreDocument, realLine: number): Promise<vscode.Uri> {
        const windowLines = this.windowLines();
        const start = Math.max(0, realLine - Math.floor(windowLines / 3));
        const uri = buildPolerEditUri(doc.filePath, start);
        this.service.notifyChanged(uri);
        return uri;
    }
}

function formatBytes(bytes: number): string {
    if (bytes >= 1024 * 1024 * 1024) {
        return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    }
    if (bytes >= 1024 * 1024) {
        return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }
    return `${(bytes / 1024).toFixed(1)} KB`;
}

/*---------------------------------------------------------------------------------------------
 *  POLER Studio Core - poler-edit-core bridge (Zero-Copy Piece-Table)
 *
 *  JSON-lines RPC client for the native editor server
 *  (`poler-edit-core --edit-serve`, see engine/src/editor/protocol.rs):
 *
 *    open {path}                -> {doc, bytes, pieces, lines}
 *    close {doc}                -> {}
 *    stats {doc}                -> {bytes,pieces,lines,indexed_bytes,...}
 *    viewport {doc,line,count}  -> {lines:[{line,text,truncated}]}
 *    goto {doc,line}            -> {byte}          (LSP line -> byte offset)
 *    linecol {doc,byte}         -> {line,col}      (byte offset -> LSP line/col)
 *    insert_at / delete / undo / redo / save / save_as / index / search / cancel / quit
 *
 *  Lines are 0-based, columns are counted in characters - exactly the
 *  Language Server Protocol coordinate model, so the byte<->position
 *  translation below IS the LSP proxy required by the integration spec.
 *--------------------------------------------------------------------------------------------*/

import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

export interface IEditCoreStats {
    bytes: number;
    pieces: number;
    lines: number | null;
    indexedBytes: number;
    edited: boolean;
    undoDepth: number;
    redoDepth: number;
}

export interface IEditCoreViewportLine {
    line: number;
    text: string;
    truncated: boolean;
}

export interface IEditCoreHit {
    byte: number;
    line: number;
    col: number;
    len: number;
}

export interface IEditCoreSearchStats {
    hits: IEditCoreHit[];
    truncated: boolean;
    cancelled: boolean;
    scannedBytes: number;
    elapsedMs: number;
}

export interface IEditCorePosition {
    line: number;
    col: number;
}

/**
 * One open document inside the native piece-table server.
 */
export class PolerEditCoreDocument {
    public docId: number;

    constructor(
        docId: number,
        public readonly filePath: string,
        public readonly bytes: number,
        private readonly service: PolerEditCoreService
    ) {
        this.docId = docId;
    }

    /**
     * Re-attach the document to a freshly restarted server: the native
     * doc handle is only valid inside one server process, so a restart
     * mints a new id that existing wrappers must adopt in place.
     * @internal
     */
    public _reattach(docId: number): void {
        this.docId = docId;
    }

    stats(): Promise<IEditCoreStats> {
        return this.service.request({ cmd: 'stats', doc: this.docId }).then(r => ({
            bytes: r.bytes ?? 0,
            pieces: r.pieces ?? 0,
            lines: (r.lines ?? null) as number | null,
            indexedBytes: r.indexed_bytes ?? 0,
            edited: !!r.edited,
            undoDepth: r.undo_depth ?? 0,
            redoDepth: r.redo_depth ?? 0
        }));
    }

    viewport(line: number, count: number): Promise<IEditCoreViewportLine[]> {
        return this.service.request({ cmd: 'viewport', doc: this.docId, line, count }).then(r => {
            return (r.lines ?? []).map((l: any) => ({
                line: l.line as number,
                text: l.text as string,
                truncated: !!l.truncated
            }));
        });
    }

    /**
     * LSP bridge: (line, characterCol) -> byte offset in the native buffer.
     *
     * The `goto` protocol command answers the byte offset of the line
     * START only, so the column is converted from characters (the LSP
     * coordinate unit, also used by the server's own insert/delete paths)
     * to bytes through a one-line viewport fetch.
     */
    async positionToOffset(position: IEditCorePosition): Promise<number> {
        const lineStart = await this.service.request({ cmd: 'goto', doc: this.docId, line: position.line }).then(r => r.byte as number);
        if (position.col <= 0) {
            return lineStart;
        }
        const one = await this.viewport(position.line, 1);
        const text = one.length ? one[0].text : '';
        // Characters -> bytes: columns are character counts, file bytes are UTF-8.
        const prefix = Array.from(text).slice(0, position.col).join('');
        return lineStart + Buffer.byteLength(prefix, 'utf8');
    }

    /** LSP bridge: byte offset -> (line, characterCol) in the native buffer. */
    offsetToPosition(byte: number): Promise<IEditCorePosition> {
        return this.service.request({ cmd: 'linecol', doc: this.docId, byte }).then(r => ({
            line: r.line as number,
            col: r.col as number
        }));
    }

    search(query: string, caseSensitive: boolean, limit: number): Promise<IEditCoreSearchStats> {
        return this.service.request({ cmd: 'search', doc: this.docId, query, case_sensitive: caseSensitive, limit }).then(r => ({
            hits: (r.hits ?? []).map((h: any) => ({ byte: h.byte, line: h.line, col: h.col, len: h.len })),
            truncated: !!r.truncated,
            cancelled: !!r.cancelled,
            scannedBytes: r.scanned_bytes ?? 0,
            elapsedMs: r.elapsed_ms ?? 0
        }));
    }

    cancel(): Promise<void> {
        return this.service.request({ cmd: 'cancel' }).then(() => undefined);
    }

    indexAll(): Promise<number> {
        return this.service.request({ cmd: 'index', doc: this.docId }).then(r => r.lines as number);
    }

    insertAt(position: IEditCorePosition, text: string): Promise<number> {
        return this.service.request({ cmd: 'insert_at', doc: this.docId, line: position.line, col: position.col, text }).then(r => r.bytes as number);
    }

    deleteRange(start: IEditCorePosition, end: IEditCorePosition): Promise<number> {
        return this.service.request({
            cmd: 'delete', doc: this.docId,
            start_line: start.line, start_col: start.col,
            end_line: end.line, end_col: end.col
        }).then(r => r.bytes as number);
    }

    undo(): Promise<boolean> {
        return this.service.request({ cmd: 'undo', doc: this.docId }).then(r => !!r.applied);
    }

    redo(): Promise<boolean> {
        return this.service.request({ cmd: 'redo', doc: this.docId }).then(r => !!r.applied);
    }

    save(): Promise<number> {
        return this.service.request({ cmd: 'save', doc: this.docId }).then(r => r.bytes as number);
    }

    saveAs(targetPath: string): Promise<number> {
        return this.service.request({ cmd: 'save_as', doc: this.docId, path: targetPath }).then(r => r.bytes as number);
    }

    close(): Promise<void> {
        return this.service.request({ cmd: 'close', doc: this.docId }).then(() => undefined);
    }
}

interface IPendingRequest {
    resolve: (value: any) => void;
    reject: (err: Error) => void;
    timer: NodeJS.Timeout;
}

/**
 * Process manager + JSON-lines RPC for `poler-edit-core --edit-serve`.
 * One server process serves every open large document (multi-tab GUI
 * contract of the protocol). The client transparently restarts the server
 * once when it dies and replays `open` for the surviving documents.
 */
export class PolerEditCoreService implements vscode.Disposable {
    public static readonly SCHEME = 'poler-edit';

    private _proc: cp.ChildProcess | undefined;
    private _nextId = 1;
    private _pending = new Map<number, IPendingRequest>();
    private _documents = new Map<string, PolerEditCoreDocument>();
    private _stdoutBuffer = '';
    private _starting: Promise<void> | undefined;
    private readonly _onDidChangeDocument = new vscode.EventEmitter<vscode.Uri>();
    private readonly _disposables: vscode.Disposable[] = [this._onDidChangeDocument];

    public readonly onDidChangeDocument = this._onDidChangeDocument.event;

    /**
     * Resolve the poler-edit-core binary: explicit setting first, then the
     * packaged resources, then poler-engine (same protocol via
     * `--edit-serve`), then PATH.
     */
    public static resolveBinary(): string | undefined {
        const configured = vscode.workspace.getConfiguration('poler').get<string>('editCorePath');
        if (configured && fs.existsSync(configured)) {
            return configured;
        }

        const exeName = process.platform === 'win32' ? 'poler-edit-core.exe' : 'poler-edit-core';
        const engineName = process.platform === 'win32' ? 'poler-engine.exe' : 'poler-engine';
        const candidates = [
            ...((process.env as any).POLER_EDIT_CORE_PATH ? [(process.env as any).POLER_EDIT_CORE_PATH] : []),
            ...(process.env.POLER_ENGINE_PATH ? [process.env.POLER_ENGINE_PATH] : []),
            ...(process.env.LOCALAPPDATA ? [path.join(process.env.LOCALAPPDATA, 'Programs', 'POLER Edit', exeName)] : []),
            exeName,
            engineName
        ];

        for (const candidate of candidates) {
            if (path.isAbsolute(candidate)) {
                if (fs.existsSync(candidate)) {
                    return candidate;
                }
                continue;
            }
            // Bare name: resolve against common bin locations and PATH.
            const localBins = [
                path.join(process.env.USERPROFILE || process.env.HOME || '', 'bin', candidate)
            ];
            for (const local of localBins) {
                if (fs.existsSync(local)) {
                    return local;
                }
            }
            const which = PolerEditCoreService.which(candidate);
            if (which) {
                return which;
            }
        }
        return undefined;
    }

    private static which(binary: string): string | undefined {
        try {
            const dirs = (process.env.PATH || '').split(path.delimiter).filter(Boolean);
            for (const dir of dirs) {
                const full = path.join(dir, binary);
                try {
                    fs.accessSync(full, fs.constants.X_OK);
                    return full;
                } catch {
                    // continue
                }
            }
        } catch {
            // PATH resolution unavailable
        }
        return undefined;
    }

    public isEnabled(): boolean {
        return vscode.workspace.getConfiguration('poler').get<boolean>('editCore.enabled', true)
            && !!PolerEditCoreService.resolveBinary();
    }

    public static largeFileThresholdBytes(): number {
        const mb = vscode.workspace.getConfiguration('poler').get<number>('editCore.largeFileThresholdMB', 50);
        return Math.max(1, mb) * 1024 * 1024;
    }

    public static isLargeFile(filePath: string): boolean {
        try {
            const stat = fs.statSync(filePath);
            return stat.isFile() && stat.size >= PolerEditCoreService.largeFileThresholdBytes();
        } catch {
            return false;
        }
    }

    /**
     * Open (or reuse) a document in the native piece-table server. The
     * open is mmap-backed and returns instantly - the line index is built
     * lazily and asynchronously through `indexAll`.
     */
    public async openDocument(filePath: string): Promise<PolerEditCoreDocument> {
        const normalized = path.normalize(filePath);
        const existing = this._documents.get(normalized);
        if (existing) {
            return existing;
        }

        await this.ensureServer();
        const res = await this.request({ cmd: 'open', path: normalized });
        if (res.ok === false) {
            throw new Error(`poler-edit-core: ${res.error ?? 'open failed'}`);
        }
        const doc = new PolerEditCoreDocument(res.doc, normalized, res.bytes ?? 0, this);
        this._documents.set(normalized, doc);
        return doc;
    }

    public getDocument(filePath: string): PolerEditCoreDocument | undefined {
        return this._documents.get(path.normalize(filePath));
    }

    public async closeDocument(filePath: string): Promise<void> {
        const doc = this._documents.get(path.normalize(filePath));
        if (!doc) {
            return;
        }
        this._documents.delete(path.normalize(filePath));
        try {
            await doc.close();
        } catch {
            // server may already be gone
        }
    }

    /**
     * Fire-and-forget line indexing; progress events are ignored by the
     * RPC layer and indexing continues inside the server.
     */
    public async indexInBackground(doc: PolerEditCoreDocument): Promise<void> {
        try {
            await doc.indexAll();
            this._onDidChangeDocument.fire(vscode.Uri.parse(`${PolerEditCoreService.SCHEME}://${doc.filePath}`));
        } catch {
            // Indexing is best-effort; viewport requests still work
            // through the unindexed fallback path.
        }
    }

    public notifyChanged(uri: vscode.Uri): void {
        this._onDidChangeDocument.fire(uri);
    }

    /**
     * Diagnostic handle: PID of the live edit-core server process, if any.
     */
    public get serverPid(): number | undefined {
        return this._proc?.pid;
    }

    /**
     * Bring the edit-core server up. When a server is (re)started while
     * documents were already open, they are replayed onto the new process
     * BEFORE any request continues - the native doc handles are only valid
     * inside one server process. Concurrent callers share the same
     * restart+replay promise.
     */
    private async ensureServer(): Promise<void> {
        if (this.isServerAlive()) {
            return;
        }
        if (this._starting) {
            return this._starting;
        }
        const hadDocuments = this._documents.size > 0;
        this._starting = this.startServer().then(() => hadDocuments ? this.reopenDocuments() : undefined);
        try {
            await this._starting;
        } finally {
            this._starting = undefined;
        }
    }

    /**
     * A process killed by a signal keeps `exitCode === null` and only sets
     * `signalCode`, so both have to be inspected to decide liveness.
     */
    private isServerAlive(): boolean {
        const proc = this._proc as ({ signalCode?: string | null } & cp.ChildProcess) | undefined;
        if (!proc || proc.killed) {
            return false;
        }
        if (proc.exitCode !== null) {
            return false;
        }
        const signal = (proc as { signalCode?: string | null }).signalCode;
        if (signal !== undefined && signal !== null) {
            return false;
        }
        return true;
    }

    private async startServer(): Promise<void> {
        const binary = PolerEditCoreService.resolveBinary();
        if (!binary) {
            throw new Error('poler-edit-core binary not found (configure poler.editCorePath)');
        }

        this._proc = cp.spawn(binary, ['--edit-serve'], {
            stdio: ['pipe', 'pipe', 'pipe'],
            windowsHide: true
        });
        this._stdoutBuffer = '';

        this._proc.stdout!.setEncoding('utf8');
        this._proc.stdout!.on('data', (chunk: string) => this.handleStdout(chunk));
        this._proc.stderr!.setEncoding('utf8');
        this._proc.stderr!.on('data', (chunk: string) => {
            // The server is quiet by design; surface anything unusual.
            const text = chunk.trim();
            if (text) {
                console.warn(`[poler-edit-core] ${text}`);
            }
        });
        this._proc.on('exit', () => {
            const error = new Error('poler-edit-core server exited');
            for (const pending of this._pending.values()) {
                clearTimeout(pending.timer);
                pending.reject(error);
            }
            this._pending.clear();
        });

        const quit = () => {
            if (this._proc && this._proc.exitCode === null) {
                try { this._proc.stdin!.write('{"id":999999,"cmd":"quit"}\n'); } catch { /* already closed */ }
                try { this._proc.kill(); } catch { /* already closed */ }
            }
        };
        process.once('exit', quit);
        this._disposables.push(new vscode.Disposable(() => {
            process.removeListener('exit', quit);
            quit();
        }));
    }

    private handleStdout(chunk: string): void {
        this._stdoutBuffer += chunk;
        let newline = this._stdoutBuffer.indexOf('\n');
        while (newline !== -1) {
            const line = this._stdoutBuffer.substring(0, newline).trim();
            this._stdoutBuffer = this._stdoutBuffer.substring(newline + 1);
            if (line) {
                this.handleLine(line);
            }
            newline = this._stdoutBuffer.indexOf('\n');
        }
    }

    private handleLine(line: string): void {
        let value: any;
        try {
            value = JSON.parse(line);
        } catch {
            return; // tolerate stray output
        }
        if (value.ev === 'progress') {
            // Indexing progress: emitted as events, not tied to a request id.
            return;
        }
        const id = value.id as number | undefined;
        if (id === undefined) {
            return;
        }
        const pending = this._pending.get(id);
        if (!pending) {
            return;
        }
        this._pending.delete(id);
        clearTimeout(pending.timer);
        if (value.ok === false) {
            pending.reject(new Error(value.error ?? 'poler-edit-core request failed'));
        } else {
            pending.resolve(value);
        }
    }

    /**
     * Send one request and await its correlated response. Crashed servers
     * are restarted once; after a restart the open documents are replayed.
     */
    public async request(payload: Record<string, unknown>): Promise<any> {
        return this.requestInternal(payload, true);
    }

    /**
     * @param allowRestart retry once through a transparent server restart
     * when the process died mid-flight. Internal replay calls pass `false`
     * to avoid re-entering the restart path (a replay failure must surface
     * to `reopenDocuments`, not trigger another restart cycle).
     */
    private async requestInternal(payload: Record<string, unknown>, allowRestart: boolean): Promise<any> {
        await this.ensureServer();
        const attempt = async (): Promise<any> => {
            const id = this._nextId++;
            const message = JSON.stringify({ id, ...payload });
            return new Promise<any>((resolve, reject) => {
                const timer = setTimeout(() => {
                    this._pending.delete(id);
                    reject(new Error(`poler-edit-core timeout: ${String(payload.cmd)}`));
                }, 120_000);
                this._pending.set(id, { resolve, reject, timer });
                this._proc!.stdin!.write(message + '\n', err => {
                    if (err) {
                        this._pending.delete(id);
                        clearTimeout(timer);
                        reject(err);
                    }
                });
            });
        };

        try {
            return await attempt();
        } catch (err) {
            // A dead server (crash / signal kill) justifies exactly one
            // transparent restart - ensureServer() replays the open
            // documents before the retry runs. A live server means the
            // request itself failed (timeout, protocol error) - rethrow.
            if (allowRestart && !this.isServerAlive()) {
                this._proc = undefined;
                await this.ensureServer();
                return await attempt();
            }
            throw err;
        }
    }

    private async reopenDocuments(): Promise<void> {
        const entries = [...this._documents.entries()];
        this._documents.clear();
        for (const [filePath, existingDoc] of entries) {
            try {
                const res = await this.requestInternal({ cmd: 'open', path: filePath }, false);
                if (res.ok === false) {
                    continue;
                }
                existingDoc._reattach(res.doc);
                this._documents.set(filePath, existingDoc);
            } catch {
                // The file may no longer be available; skip silently.
            }
        }
    }

    public dispose(): void {
        for (const pending of this._pending.values()) {
            clearTimeout(pending.timer);
            pending.reject(new Error('poler-edit-core disposed'));
        }
        this._pending.clear();
        if (this._proc && this._proc.exitCode === null) {
            try { this._proc.stdin!.write('{"id":999999,"cmd":"quit"}\n'); } catch { /* already closed */ }
            try { this._proc.kill(); } catch { /* already closed */ }
        }
        this._proc = undefined;
        this._disposables.forEach(d => d.dispose());
        this._onDidChangeDocument.dispose();
    }
}

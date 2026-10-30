/*---------------------------------------------------------------------------------------------
 *  POLER Studio Core - Sovereign Copilot Gateway (Stage C)
 *
 *  1. PAT fallback authorization: when the locally stored OAuth session
 *     (state.vscdb) answers 401/403, consumers can fall back to a service
 *     Personal Access Token carried in the `Authorization: Bearer
 *     <GITHUB_TOKEN>` header. `resolveServiceToken()` reads the token from
 *     GITHUB_TOKEN / GH_TOKEN / GITHUB_PAT.
 *
 *  2. Local LLM backend: a minimal OpenAI-compatible HTTP endpoint
 *     (127.0.0.1 only) that proxies `/v1/chat/completions` to the native
 *     resonant inference of poler-engine:
 *
 *       poler-engine --llm local  --model <glm.pqw> -q "<prompt>"
 *       poler-engine --llm quantum [-q "<prompt>"]     (no .pqw needed)
 *
 *     The endpoint is designed to be registered as a Copilot BYOK
 *     "Custom Endpoint" (OpenAI-compatible, chat-completions API type) so
 *     GitHub Copilot can run against the sovereign engine with zero Python
 *     and zero GPU dependencies (.pqw/.pqc weights only).
 *--------------------------------------------------------------------------------------------*/

import * as cp from 'child_process';
import * as http from 'http';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { EngineBridge } from './engineBridge';

/**
 * Resolve the fallback Personal Access Token: environment first, then the
 * POLER secret store file (`~/.poler/github.pat`, 0600).
 */
export function resolveServiceToken(): string | undefined {
    const fromEnv = process.env.GITHUB_TOKEN || process.env.GH_TOKEN || process.env.GITHUB_PAT;
    if (fromEnv && fromEnv.trim()) {
        return fromEnv.trim();
    }
    try {
        const tokenFile = path.join(process.env.USERPROFILE || process.env.HOME || '', '.poler', 'github.pat');
        if (fs.existsSync(tokenFile)) {
            const stored = fs.readFileSync(tokenFile, 'utf8').trim();
            if (stored) {
                return stored;
            }
        }
    } catch {
        // token file unavailable
    }
    return undefined;
}

interface IChatMessage {
    role: string;
    content: string;
}

/**
 * OpenAI-compatible chat completion bridge over the native engine.
 */
export class PolerCopilotGateway implements vscode.Disposable {
    private _server: http.Server | undefined;
    private _address = '';

    constructor(private readonly bridge: EngineBridge) {}

    public get address(): string {
        return this._address;
    }

    public get port(): number {
        return vscode.workspace.getConfiguration('poler').get<number>('copilot.gatewayPort', 8770);
    }

    public get modelId(): string {
        const modelPath = vscode.workspace.getConfiguration('poler').get<string>('copilot.localModelPath', '');
        return modelPath ? 'poler-local-glm' : 'poler-resonance';
    }

    public async start(): Promise<string> {
        if (this._server && this._address) {
            return this._address;
        }

        const server = http.createServer((req, res) => this.handle(req, res));
        this._server = server;

        return new Promise<string>((resolve, reject) => {
            server.once('error', (err: any) => {
                if (err?.code === 'EADDRINUSE') {
                    // A previous gateway instance is still alive on this port —
                    // reuse it instead of failing the switch.
                    this._server = undefined;
                    this._address = `http://127.0.0.1:${this.port}`;
                    resolve(this._address);
                } else {
                    reject(err);
                }
            });
            server.listen(this.port, '127.0.0.1', () => {
                this._address = `http://127.0.0.1:${this.port}`;
                resolve(this._address);
            });
        });
    }

    public async stop(): Promise<void> {
        const server = this._server;
        this._server = undefined;
        this._address = '';
        if (!server) {
            return;
        }
        await new Promise<void>(resolve => server.close(() => resolve()));
    }

    private handle(req: http.IncomingMessage, res: http.ServerResponse): void {
        if (req.method === 'GET' && (req.url === '/v1/models' || req.url === '/v1/models/')) {
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({
                object: 'list',
                data: [{
                    id: this.modelId,
                    object: 'model',
                    created: Math.floor(Date.now() / 1000),
                    owned_by: 'poler-engine',
                    permission: []
                }]
            }));
            return;
        }

        if (req.method === 'POST' && (req.url === '/v1/chat/completions' || req.url === '/v1/chat/completions/')) {
            this.handleChat(req, res);
            return;
        }

        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: { message: `no such endpoint: ${req.url}`, type: 'invalid_request_error' } }));
    }

    private handleChat(req: http.IncomingMessage, res: http.ServerResponse): void {
        let body = '';
        req.setEncoding('utf8');
        req.on('data', (chunk: string) => { body += chunk; });
        req.on('end', async () => {
            let payload: { messages?: IChatMessage[]; stream?: boolean; model?: string };
            try {
                payload = JSON.parse(body);
            } catch {
                res.writeHead(400, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ error: { message: 'invalid JSON body', type: 'invalid_request_error' } }));
                return;
            }

            const messages = payload.messages ?? [];
            const prompt = messages.map(m => `${m.role}: ${m.content}`).join('\n\n').trim();
            if (!prompt) {
                res.writeHead(400, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ error: { message: 'messages[] required', type: 'invalid_request_error' } }));
                return;
            }

            try {
                const completion = await this.runEngine(prompt);
                const created = Math.floor(Date.now() / 1000);
                const id = `chatcmpl-poler-${Date.now()}`;
                if (payload.stream) {
                    // SSE stream: a single content chunk followed by the
                    // terminating events, per the OpenAI wire format.
                    res.writeHead(200, {
                        'Content-Type': 'text/event-stream',
                        'Cache-Control': 'no-cache',
                        'Connection': 'keep-alive'
                    });
                    const chunk = (delta: Record<string, unknown>, finish?: string) =>
                        `data: ${JSON.stringify({ id, object: 'chat.completion.chunk', created, model: this.modelId, choices: [{ index: 0, delta, finish_reason: finish ?? null }] })}\n\n`;
                    res.write(chunk({ role: 'assistant', content: '' }));
                    res.write(chunk({ content: completion }));
                    res.write(chunk({}, 'stop'));
                    res.write('data: [DONE]\n\n');
                    res.end();
                } else {
                    res.writeHead(200, { 'Content-Type': 'application/json' });
                    res.end(JSON.stringify({
                        id,
                        object: 'chat.completion',
                        created,
                        model: this.modelId,
                        choices: [{
                            index: 0,
                            message: { role: 'assistant', content: completion },
                            finish_reason: 'stop'
                        }],
                        usage: {
                            prompt_tokens: Math.ceil(prompt.length / 4),
                            completion_tokens: Math.ceil(completion.length / 4),
                            total_tokens: Math.ceil((prompt.length + completion.length) / 4)
                        }
                    }));
                }
            } catch (err: any) {
                res.writeHead(502, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ error: { message: err.message ?? String(err), type: 'engine_error' } }));
            }
        });
    }

    /**
     * Run the native inference and extract the clean completion text.
     * The engine prefixes the answer and appends a stats line; both are
     * stripped so the OpenAI wire format carries only the completion.
     */
    private async runEngine(prompt: string): Promise<string> {
        const modelPath = vscode.workspace.getConfiguration('poler').get<string>('copilot.localModelPath', '');
        const args = modelPath
            ? ['--llm', 'local', '--model', modelPath, '-q', prompt]
            : ['--llm', 'quantum', '-q', prompt];

        const result = await this.bridge.execEngine(args);
        if (result.code !== 0 && !result.stdout.trim()) {
            throw new Error(`poler-engine --llm failed (exit ${result.code}): ${result.stderr.trim() || 'no output'}`);
        }

        let text = result.stdout;
        const prefixes = ['ответ: ', 'квантовый ответ: '];
        for (const prefix of prefixes) {
            if (text.startsWith(prefix)) {
                text = text.substring(prefix.length);
                break;
            }
        }
        // Drop the trailing performance banner line.
        const lines = text.split('\n');
        while (lines.length && (lines[lines.length - 1].trim() === '' || /^glm-local ·/.test(lines[lines.length - 1].trim()) || /^quantum-L5 ·/.test(lines[lines.length - 1].trim()))) {
            lines.pop();
        }
        return lines.join('\n').trim();
    }

    /**
     * The Copilot BYOK "Custom Endpoint" JSON snippet for this gateway.
     */
    public customEndpointSnippet(): string {
        return JSON.stringify({
            providers: {
                poler: {
                    type: 'custom',
                    url: `${this._address || `http://127.0.0.1:${this.port}`}/v1`,
                    apiType: 'chat-completions',
                    models: [{
                        id: this.modelId,
                        name: `POLER Local (${this.modelId})`,
                        url: `${this._address || `http://127.0.0.1:${this.port}`}/v1`,
                        apiType: 'chat-completions',
                        maxOutputTokens: 128,
                        contextWindow: 8192,
                        toolCalling: false,
                        vision: false,
                        requestHeaders: {
                            Authorization: `Bearer ${resolveServiceToken() ?? 'poler-local'}`
                        }
                    }]
                }
            }
        }, null, 2);
    }

    public dispose(): void {
        this.stop();
    }
}

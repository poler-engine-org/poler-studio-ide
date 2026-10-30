import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

export interface ImpactResult {
    symbol: string;
    dangerLevel: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL';
    definitions: Array<{ file: string; line: number; text: string }>;
    upstreamCallers: Array<{ file: string; line: number; caller: string; text: string }>;
    downstreamCallees: Array<{ file: string; line: number; callee: string }>;
}

export interface SearchAnchor {
    file: string;
    line: number;
    col: number;
    scope: string;
    signature: string;
    energyEpsilon: number;
    resonance: number;
    snippet: string;
}

export class EngineBridge {
    private static _instance: EngineBridge;

    public static instance(): EngineBridge {
        if (!EngineBridge._instance) {
            EngineBridge._instance = new EngineBridge();
        }
        return EngineBridge._instance;
    }

    private constructor() {}

    /**
     * Автоматический поиск бинарника poler-engine в системе
     */
    public findEnginePath(): string {
        const configPath = vscode.workspace.getConfiguration('poler').get<string>('enginePath');
        if (configPath && fs.existsSync(configPath)) {
            return configPath;
        }

        const candidates = [
            'C:\\Users\\vital\\bin\\poler-engine.exe',
            path.join(process.env.USERPROFILE || '', 'bin', 'poler-engine.exe'),
            path.join(process.env.HOME || '', '.local', 'bin', 'poler-engine'),
            '/home/vitalij/.local/bin/poler-engine',
            path.join(__dirname, '..', '..', '..', 'resources', 'bin', 'poler-engine.exe'),
            path.join(__dirname, '..', '..', '..', 'resources', 'bin', 'poler-engine')
        ];

        for (const c of candidates) {
            if (fs.existsSync(c)) {
                return c;
            }
        }
        return 'poler-engine'; // fallback в PATH
    }

    /**
     * Автоматический поиск poler-edit-core для mmap кусочных таблиц
     */
    public findEditCorePath(): string {
        const configPath = vscode.workspace.getConfiguration('poler').get<string>('editCorePath');
        if (configPath && fs.existsSync(configPath)) {
            return configPath;
        }

        const candidates = [
            'C:\\Users\\vital\\bin\\poler-edit-core.exe',
            path.join(process.env.USERPROFILE || '', 'bin', 'poler-edit-core.exe'),
            path.join(process.env.HOME || '', '.local', 'bin', 'poler-edit-core'),
            path.join(__dirname, '..', '..', '..', 'resources', 'bin', 'poler-edit-core.exe')
        ];

        for (const c of candidates) {
            if (fs.existsSync(c)) {
                return c;
            }
        }
        return 'poler-edit-core';
    }

    /**
     * Вызов нативного ядра poler-engine
     */
    public async execEngine(args: string[]): Promise<{ stdout: string; stderr: string; code: number }> {
        const bin = this.findEnginePath();
        return new Promise((resolve) => {
            const child = cp.spawn(bin, args, { shell: false });
            let stdout = '';
            let stderr = '';

            child.stdout?.on('data', (d) => { stdout += d.toString(); });
            child.stderr?.on('data', (d) => { stderr += d.toString(); });

            child.on('close', (code) => {
                resolve({ stdout, stderr, code: code ?? 0 });
            });

            child.on('error', (err) => {
                resolve({ stdout: '', stderr: err.message, code: -1 });
            });
        });
    }

    /**
     * Вызов Causal Nexus & AIDDE Impact
     */
    public async getImpact(targetPath: string, symbol: string): Promise<ImpactResult | null> {
        const res = await this.execEngine(['--exec', `impact "${targetPath}" "${symbol}" --json`]);
        if (res.code !== 0 && !res.stdout) {
            return null;
        }

        try {
            const parsed = JSON.parse(res.stdout);
            return {
                symbol,
                dangerLevel: parsed.danger_level_if_modified || 'MEDIUM',
                definitions: (parsed.definition_sites || []).map((d: any) => ({
                    file: d.file || '',
                    line: d.line || 1,
                    text: d.line_text || ''
                })),
                upstreamCallers: (parsed.upstream_callers || []).map((u: any) => ({
                    file: u.file || '',
                    line: u.line || 1,
                    caller: u.caller_fn || '',
                    text: u.text || ''
                })),
                downstreamCallees: (parsed.downstream_callees || []).map((d: any) => ({
                    file: d.file || '',
                    line: d.line || 1,
                    callee: d.callee_fn || ''
                }))
            };
        } catch {
            return null;
        }
    }

    /**
     * Квантово-резонансный топографический поиск
     */
    public async searchResonance(targetPath: string, query: string, mode: string = 'psi'): Promise<SearchAnchor[]> {
        const res = await this.execEngine([targetPath, '-q', query, '--format', 'ai-json', '--resonance-mode', mode]);
        if (res.code !== 0 && !res.stdout) {
            return [];
        }

        try {
            const parsed = JSON.parse(res.stdout);
            return (parsed.anchors || []).map((a: any) => ({
                file: a.file || '',
                line: a.line || 1,
                col: a.col || 1,
                scope: a.scope || '',
                signature: a.signature || '',
                energyEpsilon: a.energy_epsilon || 1.0,
                resonance: a.resonance || 1.0,
                snippet: a.snippet || ''
            }));
        } catch {
            return [];
        }
    }

    /**
     * Контекстный шлюз feed (Understand / Edit)
     */
    public async getFeed(targetPath: string, forWhat: 'understand' | 'edit', query: string, budget: number = 4000): Promise<string> {
        const res = await this.execEngine(['--exec', `feed "${targetPath}" --for ${forWhat} -q "${query}" --budget ${budget}`]);
        return res.stdout || res.stderr;
    }

    /**
     * Вычисление формул, астрономии и физики без Python
     */
    public async calc(expr: string): Promise<string> {
        const res = await this.execEngine(['--exec', `calc ${expr}`]);
        return (res.stdout || res.stderr).trim();
    }
}

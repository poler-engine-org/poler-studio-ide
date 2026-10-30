import * as vscode from 'vscode';
import { EngineBridge } from './engineBridge';

export class ScientificStudioPanel {
    public static currentPanel: ScientificStudioPanel | undefined;
    private readonly _panel: vscode.WebviewPanel;
    private _disposables: vscode.Disposable[] = [];

    public static createOrShow(extensionUri: vscode.Uri) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;

        if (ScientificStudioPanel.currentPanel) {
            ScientificStudioPanel.currentPanel._panel.reveal(column);
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            'polerScientificStudio',
            '🔬 POLER Scientific Studio',
            column || vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [extensionUri]
            }
        );

        ScientificStudioPanel.currentPanel = new ScientificStudioPanel(panel, extensionUri);
    }

    private constructor(panel: vscode.WebviewPanel, private readonly _extensionUri: vscode.Uri) {
        this._panel = panel;
        this._update();

        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        this._panel.webview.onDidReceiveMessage(
            async (message) => {
                switch (message.command) {
                    case 'evalExpression': {
                        const result = await EngineBridge.instance().calc(message.expr);
                        this._panel.webview.postMessage({
                            command: 'evalResult',
                            id: message.id,
                            result: result
                        });
                        break;
                    }
                    case 'insertToDoc': {
                        const editor = vscode.window.activeTextEditor;
                        if (editor) {
                            editor.edit((editBuilder) => {
                                editBuilder.insert(editor.selection.active, '\n' + message.text + '\n');
                            });
                            vscode.window.showInformationMessage('POLER: Результат успішно вставлено в активний файл!');
                        } else {
                            vscode.window.showWarningMessage('POLER: Відкрийте файл у редакторі для вставки');
                        }
                        break;
                    }
                }
            },
            null,
            this._disposables
        );
    }

    public dispose() {
        ScientificStudioPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) x.dispose();
        }
    }

    private _update() {
        this._panel.webview.html = this._getHtmlForWebview();
    }

    private _getHtmlForWebview(): string {
        return `<!DOCTYPE html>
<html lang="uk">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>POLER Scientific Studio</title>
    <style>
        :root {
            --bg-primary: #12121a;
            --bg-secondary: #1a1a26;
            --border-color: #2e2e42;
            --accent-green: #2ecc71;
            --accent-cyan: #00e6b4;
            --text-color: #ecf0f1;
            --text-muted: #95a5a6;
        }
        body {
            background-color: var(--bg-primary);
            color: var(--text-color);
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
            margin: 0;
            padding: 12px;
            display: flex;
            flex-direction: column;
            gap: 12px;
            height: 100vh;
            box-sizing: border-box;
            overflow-y: auto;
        }
        .header-bar {
            display: flex;
            align-items: center;
            justify-content: space-between;
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 6px;
            padding: 8px 12px;
        }
        .badge {
            font-size: 11px;
            font-weight: bold;
            padding: 3px 8px;
            border-radius: 4px;
        }
        .badge-green {
            background: #121e16;
            color: var(--accent-green);
            border: 1px solid #1e3d28;
        }
        .badge-cyan {
            background: #0f2324;
            color: var(--accent-cyan);
            border: 1px solid #16423c;
        }
        .toolbar {
            display: flex;
            gap: 8px;
            align-items: center;
        }
        button {
            background: #27ae60;
            color: white;
            border: none;
            border-radius: 4px;
            padding: 6px 12px;
            font-size: 12px;
            cursor: pointer;
            font-weight: bold;
            transition: background 0.15s;
        }
        button:hover {
            background: #2ecc71;
        }
        button.secondary {
            background: #2c3e50;
            color: #ecf0f1;
            border: 1px solid #4a627a;
        }
        button.secondary:hover {
            background: #34495e;
        }
        .cell-card {
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 6px;
            padding: 12px;
            display: flex;
            flex-direction: column;
            gap: 8px;
        }
        .cell-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            font-size: 11px;
            color: var(--text-muted);
        }
        textarea {
            width: 100%;
            background: #0d0d14;
            color: #00e6b4;
            border: 1px solid #2e2e42;
            border-radius: 4px;
            font-family: 'Consolas', 'Courier New', monospace;
            font-size: 12px;
            padding: 8px;
            box-sizing: border-box;
            resize: vertical;
            min-height: 80px;
        }
        .output-box {
            background: #0a0a0f;
            border: 1px solid #1f2833;
            border-radius: 4px;
            padding: 8px;
            font-family: monospace;
            font-size: 12px;
            color: var(--accent-green);
            white-space: pre-wrap;
            display: none;
        }
        canvas {
            width: 100%;
            height: 220px;
            background: #0a0a12;
            border: 1px solid var(--border-color);
            border-radius: 6px;
        }
    </style>
</head>
<body>
    <div class="header-bar">
        <div style="display:flex; align-items:center; gap:8px;">
            <span style="font-size:14px; font-weight:bold;">🔬 POLER Scientific Studio</span>
            <span class="badge badge-green">POLER 🟢 O(1)</span>
            <span class="badge badge-cyan">Quantum Ψ</span>
        </div>
        <div class="toolbar">
            <button class="secondary" onclick="addCell()">➕ SciLang Комірка</button>
            <button class="secondary" onclick="runAll()">▶ Виконати все</button>
        </div>
    </div>

    <!-- 3D Холст Молекулярної Геометрії -->
    <div class="cell-card">
        <div class="cell-header">
            <span><b>🌐 3D Молекулярний Холст & Динамічна Сітка</b></span>
            <span>PyMOL CPK / Unreal Engine DynamicMesh</span>
        </div>
        <canvas id="view3d"></canvas>
        <div style="display:flex; gap:8px; align-items:center; font-size:11px;">
            <span>Модель:</span>
            <button class="secondary" style="padding:2px 8px; font-size:11px;" onclick="renderPreset('caffeine')">Кофеїн (C₈H₁₀N₄O₂)</button>
            <button class="secondary" style="padding:2px 8px; font-size:11px;" onclick="renderPreset('water')">Вода (H₂O)</button>
            <button class="secondary" style="padding:2px 8px; font-size:11px;" onclick="renderPreset('torus')">Тор (GF(3) Сітка)</button>
        </div>
    </div>

    <!-- Стартова комірка з розрахунком Всесвіту -->
    <div class="cell-card" id="cell-0">
        <div class="cell-header">
            <span><b>[1] ⚛ POLER SciLang — Розрахунок кількості атомів Всесвіту</b></span>
            <button style="padding:3px 10px;" onclick="runCell(0)">▶ Виконати</button>
        </div>
        <textarea id="code-0">let G = 6.67430e-11; let H0 = 2.184e-18; let Om_b = 0.0493;
let rho_c = (3.0 * H0^2) / (8.0 * 3.141592653589793 * G);
let rho_b = Om_b * rho_c;
let mp = 1.67262192369e-27;
let nb = rho_b / mp;
let R_U = 4.40e26;
let V_U = (4.0 / 3.0) * 3.141592653589793 * R_U^3;
let N_atoms = nb * V_U;</textarea>
        <div class="output-box" id="out-0"></div>
        <div style="display:none; justify-content:flex-end; gap:6px;" id="actions-0">
            <button class="secondary" style="font-size:10px; padding:3px 8px;" onclick="insertOutput(0)">📝 Вставити в активний файл</button>
        </div>
    </div>

    <script>
        const vscode = acquireVsCodeApi();

        function runCell(idx) {
            const code = document.getElementById('code-' + idx).value;
            const expr = code.replace(/\\n/g, ' ');
            vscode.postMessage({
                command: 'evalExpression',
                id: idx,
                expr: expr
            });
        }

        window.addEventListener('message', event => {
            const msg = event.data;
            if (msg.command === 'evalResult') {
                const out = document.getElementById('out-' + msg.id);
                out.style.display = 'block';
                out.textContent = msg.result || '✓ Виконано успішно';
                const act = document.getElementById('actions-' + msg.id);
                if (act) act.style.display = 'flex';
            }
        });

        function insertOutput(idx) {
            const out = document.getElementById('out-' + idx);
            if (out && out.textContent) {
                vscode.postMessage({
                    command: 'insertToDoc',
                    text: out.textContent
                });
            }
        }

        // Простий 3D Canvas рендерер CPK сфер
        const canvas = document.getElementById('view3d');
        const ctx = canvas.getContext('2d');
        let angle = 0;

        function drawMolecule() {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            const cx = canvas.width / 2;
            const cy = canvas.height / 2;
            angle += 0.02;

            // Рендеримо центральний атом і орбітальні зв'язки
            const x1 = cx + Math.cos(angle) * 50;
            const y1 = cy + Math.sin(angle) * 30;
            const x2 = cx - Math.cos(angle) * 50;
            const y2 = cy - Math.sin(angle) * 30;

            // Зв'язок
            ctx.strokeStyle = '#4a5568';
            ctx.lineWidth = 4;
            ctx.beginPath();
            ctx.moveTo(x1, y1);
            ctx.lineTo(cx, cy);
            ctx.lineTo(x2, y2);
            ctx.stroke();

            // Атоми
            drawAtom(cx, cy, 18, '#e74c3c', 'O');
            drawAtom(x1, y1, 12, '#ecf0f1', 'H');
            drawAtom(x2, y2, 12, '#ecf0f1', 'H');

            requestAnimationFrame(drawMolecule);
        }

        function drawAtom(x, y, r, color, sym) {
            ctx.beginPath();
            ctx.arc(x, y, r, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();
            ctx.strokeStyle = '#ffffff';
            ctx.lineWidth = 1;
            ctx.stroke();
            ctx.fillStyle = '#000000';
            ctx.font = 'bold 10px sans-serif';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(sym, x, y);
        }

        canvas.width = canvas.clientWidth;
        canvas.height = canvas.clientHeight;
        drawMolecule();
    </script>
</body>
</html>`;
    }
}

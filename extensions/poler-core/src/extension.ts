import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { EngineBridge } from './engineBridge';
import { CausalViewProvider, QuickFeedViewProvider } from './causalView';
import { ScientificStudioPanel } from './scientificStudioView';
import { PolerEditCoreService } from './editCoreService';
import { buildPolerEditUri, decodePolerEditUri, PolerEditCoreLargeFileProvider } from './largeFileProvider';
import { PolerCopilotGateway, resolveServiceToken } from './copilotGateway';
import { registerPolerEditLargeFileTool } from './copilotEditTool';

export function activate(context: vscode.ExtensionContext) {
    const bridge = EngineBridge.instance();
    const causalProvider = new CausalViewProvider();
    const quickFeedProvider = new QuickFeedViewProvider();
    const editCoreService = new PolerEditCoreService();
    const largeFileProvider = new PolerEditCoreLargeFileProvider(editCoreService);
    const copilotGateway = new PolerCopilotGateway(bridge);
    context.subscriptions.push(editCoreService, copilotGateway);
    context.subscriptions.push(registerPolerEditLargeFileTool(editCoreService));

    // 1. Регистрация представлений в Activity Bar (POLER Studio)
    context.subscriptions.push(
        vscode.window.registerTreeDataProvider('poler.causalView', causalProvider),
        vscode.window.registerTreeDataProvider('poler.quickFeedView', quickFeedProvider)
    );

    // 1b. POLER Edit Core: виртуальна схема poler-edit — миттєве відкриття
    // великих файлів (>50 MB) через нативну Piece-Table poler-edit-core.
    context.subscriptions.push(
        vscode.workspace.registerTextDocumentContentProvider(PolerEditCoreService.SCHEME, largeFileProvider)
    );

    // 1c. Підказка для великих файлів: звичайний відкритий файл понад
    // поріг пропонується перемкнути на нативний Piece-Table.
    if (editCoreService.isEnabled()) {
        const promptLargeFiles = vscode.workspace.getConfiguration('poler').get<boolean>('editCore.promptForLargeFiles', true);
        if (promptLargeFiles) {
            const opened = new Set<string>();
            context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(document => {
                if (document.uri.scheme !== 'file' || opened.has(document.uri.fsPath)) {
                    return;
                }
                opened.add(document.uri.fsPath);
                if (PolerEditCoreService.isLargeFile(document.uri.fsPath)) {
                    const sizeMb = (fs.statSync(document.uri.fsPath).size / (1024 * 1024)).toFixed(0);
                    vscode.window.showInformationMessage(
                        `POLER: файл ${path.basename(document.uri.fsPath)} (${sizeMb} MB) — відкрити через нативну Piece-Table (миттєво, без завантаження в пам'ять)?`,
                        'Відкрити нативно (O(1))'
                    ).then(choice => {
                        if (choice) {
                            vscode.commands.executeCommand('poler.editCore.openLargeFile', document.uri);
                        }
                    });
                }
            }));
        }
    }

    // 2. Статус-бар POLER O(1)
    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBar.text = '$(zap) POLER 🟢 O(1)';
    statusBar.tooltip = 'POLER[Ψ] Engine v0.93.0 Bridge Active. Натисніть для миттєвого калькулятора / фізики СІ-2019';
    statusBar.command = 'poler.calcExpression';
    statusBar.show();
    context.subscriptions.push(statusBar);

    // 3. Команда: Causal Nexus & AIDDE Impact (Alt+F12)
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.causalImpact', async () => {
            const editor = vscode.window.activeTextEditor;
            let targetSymbol = '';

            if (editor) {
                const selection = editor.selection;
                targetSymbol = editor.document.getText(selection).trim();
                if (!targetSymbol) {
                    const range = editor.document.getWordRangeAtPosition(selection.active);
                    if (range) {
                        targetSymbol = editor.document.getText(range);
                    }
                }
            }

            if (!targetSymbol) {
                targetSymbol = await vscode.window.showInputBox({
                    prompt: 'Введіть символ або функцію для побудови графу зв\'язків (Causal Nexus):',
                    placeHolder: 'наприклад: process_request або EngineBridge'
                }) || '';
            }

            if (!targetSymbol) {
                return;
            }

            const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ||
                (editor ? path.dirname(editor.document.fileName) : '.');

            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: `POLER: Побудова Causal Nexus для "${targetSymbol}"...`,
                cancellable: false
            }, async () => {
                const impact = await bridge.getImpact(workspaceFolder, targetSymbol);
                causalProvider.setImpact(impact);

                if (impact) {
                    const dangerMsg = impact.dangerLevel === 'CRITICAL' ? '🔴 КРИТИЧНИЙ' :
                        impact.dangerLevel === 'HIGH' ? '🟠 ВИСОКИЙ' :
                            impact.dangerLevel === 'MEDIUM' ? '🟡 СЕРЕДНІЙ' : '🟢 БЕЗПЕЧНИЙ';
                    vscode.window.showInformationMessage(
                        `POLER Nexus [${targetSymbol}]: Рівень ризику ${dangerMsg}. ` +
                        `Визначень: ${impact.definitions.length}, Висхідних викликів: ${impact.upstreamCallers.length}, Залежностей: ${impact.downstreamCallees.length}`
                    );
                } else {
                    vscode.window.showWarningMessage(`POLER: Символ "${targetSymbol}" не знайдено в графі зв'язків.`);
                }
            });
        })
    );

    // 4. Команда: Квантово-резонансний топографічний пошук (Ψ)
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.resonanceSearch', async () => {
            const editor = vscode.window.activeTextEditor;
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ||
                (editor ? path.dirname(editor.document.fileName) : '.');

            const query = await vscode.window.showInputBox({
                prompt: 'POLER Топографічний резонансний пошук (Ψ):',
                placeHolder: 'Введіть запит, семантичну сутність або сигнатуру...'
            });

            if (!query) {
                return;
            }

            const mode = vscode.workspace.getConfiguration('poler').get<string>('resonanceMode') || 'psi';

            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: `POLER: Квантове сканування "${query}" (режим: ${mode})...`,
                cancellable: false
            }, async () => {
                const anchors = await bridge.searchResonance(workspaceFolder, query, mode);
                if (anchors.length === 0) {
                    vscode.window.showInformationMessage(`POLER: Збігів для "${query}" не знайдено.`);
                    return;
                }

                interface AnchorPickItem extends vscode.QuickPickItem {
                    anchor: typeof anchors[0];
                }

                const items: AnchorPickItem[] = anchors.map(a => ({
                    label: `$(symbol-reference) ${path.basename(a.file)}:${a.line}`,
                    description: `ε: ${a.energyEpsilon.toFixed(2)} | Res: ${a.resonance.toFixed(2)} | Scope: ${a.scope || 'global'}`,
                    detail: a.snippet || a.signature,
                    anchor: a
                }));

                const selected = await vscode.window.showQuickPick(items, {
                    placeHolder: `Знайдено ${anchors.length} резонансних точок для "${query}"`
                });

                if (selected) {
                    const fullPath = path.isAbsolute(selected.anchor.file)
                        ? selected.anchor.file
                        : path.join(workspaceFolder, selected.anchor.file);
                    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fullPath));
                    const pos = new vscode.Position(
                        Math.max(0, selected.anchor.line - 1),
                        Math.max(0, selected.anchor.col - 1)
                    );
                    const selEditor = await vscode.window.showTextDocument(doc);
                    selEditor.selection = new vscode.Selection(pos, pos);
                    selEditor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
                }
            });
        })
    );

    // 5. Команда: Feed Understand (Генерація контексту архітектури для LLM)
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.feedUnderstand', async () => {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '.';
            const query = await vscode.window.showInputBox({
                prompt: 'POLER Feed: Understand — Архітектурна тема / скоуп для зрізу:',
                placeHolder: 'наприклад: kernel syscall dispatch or buffer manager'
            }) || '';

            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: 'POLER: Збірка архітектурного контексту (Understand Feed)...',
                cancellable: false
            }, async () => {
                const feed = await bridge.getFeed(workspaceFolder, 'understand', query);
                const doc = await vscode.workspace.openTextDocument({
                    content: feed,
                    language: 'markdown'
                });
                await vscode.window.showTextDocument(doc, { preview: false });
            });
        })
    );

    // 6. Команда: Feed Edit (Генерація правочного пакету з AIDDE impact)
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.feedEdit', async () => {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '.';
            const query = await vscode.window.showInputBox({
                prompt: 'POLER Feed: Edit — Цільова задача / символ для правочного пакету:',
                placeHolder: 'наприклад: optimize vring latency'
            }) || '';

            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: 'POLER: Збірка правочного пакету (Edit Feed)...',
                cancellable: false
            }, async () => {
                const feed = await bridge.getFeed(workspaceFolder, 'edit', query);
                const doc = await vscode.workspace.openTextDocument({
                    content: feed,
                    language: 'markdown'
                });
                await vscode.window.showTextDocument(doc, { preview: false });
            });
        })
    );

    // 7. Команда: Наукова Студія (SciLang + 3D Хімія/Фізика)
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.scientificStudio', () => {
            ScientificStudioPanel.createOrShow(context.extensionUri);
        })
    );

    // 8. Команда: Нативний калькулятор формул і констант СІ-2019
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.calcExpression', async () => {
            const editor = vscode.window.activeTextEditor;
            let expr = '';
            if (editor) {
                expr = editor.document.getText(editor.selection).trim();
            }

            if (!expr) {
                expr = await vscode.window.showInputBox({
                    prompt: 'POLER Calc: Введіть математичний або фізичний вираз (СІ-2019, астрономія, матриці):',
                    placeHolder: 'наприклад: c * 1s, planck, G, sin(pi/3), 2^64 - 1'
                }) || '';
            }

            if (!expr) {
                return;
            }

            const result = await bridge.calc(expr);
            const action = await vscode.window.showInformationMessage(
                `POLER Calc [${expr}]: ${result}`,
                'Вставити в код',
                'Скопіювати'
            );

            if (action === 'Вставити в код' && editor) {
                editor.edit(builder => {
                    builder.replace(editor.selection, result);
                });
            } else if (action === 'Скопіювати') {
                await vscode.env.clipboard.writeText(result);
            }
        })
    );

    // 10. POLER Edit Core: команди нативної Piece-Table для великих файлів
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.editCore.openLargeFile', async (uri?: vscode.Uri) => {
            let filePath: string | undefined;
            if (uri && uri.scheme === 'file') {
                filePath = uri.fsPath;
            } else {
                const picked = await vscode.window.showOpenDialog({
                    openLabel: 'Відкрити нативно (Piece-Table)',
                    canSelectMany: false
                });
                filePath = picked?.[0]?.fsPath;
            }
            if (!filePath) {
                return;
            }
            if (!PolerEditCoreService.isLargeFile(filePath)) {
                const threshold = PolerEditCoreService.largeFileThresholdBytes();
                const openAny = await vscode.window.showInformationMessage(
                    `POLER: файл менше порога (${(threshold / (1024 * 1024)).toFixed(0)} MB) — відкрити нативно все одно?`,
                    'Так, відкрити', 'Скасувати'
                );
                if (openAny !== 'Так, відкрити') {
                    return;
                }
            }
            try {
                const doc = await editCoreService.openDocument(filePath);
                await vscode.window.showTextDocument(buildPolerEditUri(filePath, 0));
                editCoreService.indexInBackground(doc);
            } catch (err: any) {
                vscode.window.showErrorMessage(`POLER Piece-Table: ${err.message ?? String(err)}`);
            }
        }),

        vscode.commands.registerCommand('poler.editCore.gotoLine', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.uri.scheme !== PolerEditCoreService.SCHEME) {
                vscode.window.showWarningMessage('POLER: команда працює у вікні poler-edit (нативний Piece-Table).');
                return;
            }
            const { filePath } = decodePolerEditUri(editor.document.uri);
            const doc = editCoreService.getDocument(filePath);
            if (!doc) {
                return;
            }
            const input = await vscode.window.showInputBox({
                prompt: 'POLER Piece-Table: перехід до рядка (1-based) або зміщення в байтах (наприклад 0x1F400 / 128000)',
                placeHolder: '12345'
            });
            if (!input) {
                return;
            }
            let realLine: number;
            if (/^0x[0-9a-fA-F]+$/.test(input) || /^\d{6,}$/.test(input)) {
                // Byte offset form: LSP-міст байт → рядок/колонка
                const byte = /^0x/.test(input) ? parseInt(input, 16) : parseInt(input, 10);
                const position = await doc.offsetToPosition(byte);
                realLine = position.line;
            } else {
                realLine = Math.max(0, (parseInt(input, 10) || 1) - 1);
            }
            const uri = await largeFileProvider.revealLine(doc, realLine);
            await vscode.window.showTextDocument(uri);
        }),

        vscode.commands.registerCommand('poler.editCore.nextWindow', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.uri.scheme !== PolerEditCoreService.SCHEME) {
                return;
            }
            const { filePath, windowStart } = decodePolerEditUri(editor.document.uri);
            const doc = editCoreService.getDocument(filePath);
            if (!doc) {
                return;
            }
            const uri = buildPolerEditUri(filePath, windowStart + 2048);
            editCoreService.notifyChanged(uri);
            await vscode.window.showTextDocument(uri);
        }),

        vscode.commands.registerCommand('poler.editCore.prevWindow', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.uri.scheme !== PolerEditCoreService.SCHEME) {
                return;
            }
            const { filePath, windowStart } = decodePolerEditUri(editor.document.uri);
            const doc = editCoreService.getDocument(filePath);
            if (!doc) {
                return;
            }
            const uri = buildPolerEditUri(filePath, Math.max(0, windowStart - 2048));
            editCoreService.notifyChanged(uri);
            await vscode.window.showTextDocument(uri);
        }),

        vscode.commands.registerCommand('poler.editCore.findInFile', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.uri.scheme !== PolerEditCoreService.SCHEME) {
                vscode.window.showWarningMessage('POLER: команда працює у вікні poler-edit (нативний Piece-Table).');
                return;
            }
            const { filePath } = decodePolerEditUri(editor.document.uri);
            const doc = editCoreService.getDocument(filePath);
            if (!doc) {
                return;
            }
            const query = await vscode.window.showInputBox({
                prompt: 'POLER Piece-Table: нативний пошук у файлі (Aho-Corasick поверх mmap, без завантаження в памʼять)',
                placeHolder: 'ERROR|WARN|токен...'
            });
            if (!query) {
                return;
            }
            const stats = await doc.search(query, true, 200);
            if (stats.hits.length === 0) {
                vscode.window.showInformationMessage(`POLER Piece-Table: збігів для "${query}" не знайдено (${stats.elapsedMs} мс, ${stats.scannedBytes} байт просканово).`);
                return;
            }
            interface HitItem extends vscode.QuickPickItem {
                hitLine: number;
            }
            const items: HitItem[] = stats.hits.map(hit => ({
                label: `$(search) рядок ${hit.line + 1}:${hit.col + 1} (байт 0x${hit.byte.toString(16)})`,
                description: `довжина ${hit.len}`,
                hitLine: hit.line
            }));
            if (stats.truncated) {
                items.push({ label: `$(warning) показано 200 із більшої кількості збігів`, hitLine: -1 });
            }
            const selected = await vscode.window.showQuickPick(items, {
                placeHolder: `POLER Piece-Table: ${stats.hits.length} збігів за ${stats.elapsedMs} мс`
            });
            if (selected && selected.hitLine >= 0) {
                const uri = await largeFileProvider.revealLine(doc, selected.hitLine);
                await vscode.window.showTextDocument(uri);
            }
        }),

        vscode.commands.registerCommand('poler.editCore.saveNative', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.uri.scheme !== PolerEditCoreService.SCHEME) {
                return;
            }
            const { filePath } = decodePolerEditUri(editor.document.uri);
            const doc = editCoreService.getDocument(filePath);
            if (!doc) {
                return;
            }
            const stats = await doc.stats();
            if (stats.edited || stats.undoDepth > 0 || stats.redoDepth > 0) {
                const bytes = await doc.save();
                vscode.window.showInformationMessage(`POLER Piece-Table: збережено ${bytes} байт у ${path.basename(filePath)}.`);
            } else {
                vscode.window.showInformationMessage('POLER Piece-Table: немає нативних правок — файл на диску вже актуальний.');
            }
        })
    );

    // 11. POLER Copilot Gateway: переключатель джерела моделі
    // (GitHub Copilot Cloud ↔ локальний резонансний інференс POLER).
    context.subscriptions.push(
        vscode.commands.registerCommand('poler.copilot.useLocalModel', async () => {
            const config = vscode.workspace.getConfiguration('poler');
            const current = config.get<'copilot' | 'local'>('copilot.modelSource', 'copilot');
            const target = current === 'local' ? 'copilot' : 'local';

            if (target === 'local') {
                const modelPath = config.get<string>('copilot.localModelPath', '');
                if (modelPath && !fs.existsSync(modelPath)) {
                    const pick = await vscode.window.showWarningMessage(
                        `POLER: модель ${modelPath} не знайдена — використати квантово-резонансний режим (--llm quantum, без .pqw)?`,
                        'Так, продовжити', 'Скасувати'
                    );
                    if (pick !== 'Так, продовжити') {
                        return;
                    }
                    await config.update('copilot.localModelPath', '', vscode.ConfigurationTarget.Global);
                }
                try {
                    const address = await copilotGateway.start();
                    await config.update('copilot.modelSource', 'local', vscode.ConfigurationTarget.Global);
                    const token = resolveServiceToken();
                    const copy = await vscode.window.showInformationMessage(
                        `POLER: локальний шлюз запущено (${address}). Джерело моделі: ${copilotGateway.modelId}${token ? ' • PAT fallback активний (Authorization: Bearer <GITHUB_TOKEN>)' : ''}. Скопіювати BYOK Custom Endpoint JSON?`,
                        'Скопіювати JSON', 'Готово'
                    );
                    if (copy === 'Скопіювати JSON') {
                        await vscode.env.clipboard.writeText(copilotGateway.customEndpointSnippet());
                        vscode.window.showInformationMessage('POLER: BYOK Custom Endpoint JSON скопійовано в буфер обміну.');
                    }
                } catch (err: any) {
                    vscode.window.showErrorMessage(`POLER: не вдалося запустити локальний шлюз: ${err.message ?? String(err)}`);
                }
            } else {
                await copilotGateway.stop();
                await config.update('copilot.modelSource', 'copilot', vscode.ConfigurationTarget.Global);
                vscode.window.showInformationMessage('POLER: джерело моделі повернуто до GitHub Copilot Cloud.');
            }
        })
    );

    // 12. Нативний Reference Provider на базі poler-engine impact
    context.subscriptions.push(
        vscode.languages.registerReferenceProvider({ scheme: 'file' }, {
            async provideReferences(document, position, _context, _token) {
                const range = document.getWordRangeAtPosition(position);
                if (!range) return [];
                const word = document.getText(range);
                const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || path.dirname(document.fileName);

                const impact = await bridge.getImpact(workspaceFolder, word);
                if (!impact) return [];

                const locations: vscode.Location[] = [];
                for (const caller of impact.upstreamCallers) {
                    const fullPath = path.isAbsolute(caller.file) ? caller.file : path.join(workspaceFolder, caller.file);
                    const uri = vscode.Uri.file(fullPath);
                    const line = Math.max(0, caller.line - 1);
                    locations.push(new vscode.Location(uri, new vscode.Position(line, 0)));
                }
                return locations;
            }
        })
    );
}

export function deactivate() { }

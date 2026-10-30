import * as vscode from 'vscode';
import * as path from 'path';
import { EngineBridge } from './engineBridge';
import { CausalViewProvider, QuickFeedViewProvider } from './causalView';
import { ScientificStudioPanel } from './scientificStudioView';

export function activate(context: vscode.ExtensionContext) {
    const bridge = EngineBridge.instance();
    const causalProvider = new CausalViewProvider();
    const quickFeedProvider = new QuickFeedViewProvider();

    // 1. Регистрация представлений в Activity Bar (POLER Studio)
    context.subscriptions.push(
        vscode.window.registerTreeDataProvider('poler.causalView', causalProvider),
        vscode.window.registerTreeDataProvider('poler.quickFeedView', quickFeedProvider)
    );

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

    // 9. Нативний Reference Provider на базі poler-engine impact
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

export function deactivate() {}

import * as vscode from 'vscode';
import * as path from 'path';
import { ImpactResult } from './engineBridge';

export class CausalItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly contextValue?: string,
        public readonly targetUri?: vscode.Uri,
        public readonly targetLine?: number,
        public readonly details?: string
    ) {
        super(label, collapsibleState);
        this.tooltip = details || label;
        if (details) {
            this.description = details;
        }

        if (targetUri && targetLine !== undefined) {
            this.command = {
                command: 'vscode.open',
                title: 'Open File',
                arguments: [
                    targetUri,
                    {
                        selection: new vscode.Range(
                            new vscode.Position(Math.max(0, targetLine - 1), 0),
                            new vscode.Position(Math.max(0, targetLine - 1), 0)
                        )
                    }
                ]
            };
        }
    }
}

export class CausalViewProvider implements vscode.TreeDataProvider<CausalItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<CausalItem | undefined | null | void> = new vscode.EventEmitter<CausalItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<CausalItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private _currentImpact: ImpactResult | null = null;

    public setImpact(impact: ImpactResult | null) {
        this._currentImpact = impact;
        this._onDidChangeTreeData.fire();
    }

    public getTreeItem(element: CausalItem): vscode.TreeItem {
        return element;
    }

    public getChildren(element?: CausalItem): vscode.ProviderResult<CausalItem[]> {
        if (!this._currentImpact) {
            return [
                new CausalItem(
                    'Натисніть Alt+F12 на символі для аналізу Causal Nexus',
                    vscode.TreeItemCollapsibleState.None,
                    'empty'
                )
            ];
        }

        if (!element) {
            // Кореневі категорії
            const dangerIcon = this._currentImpact.dangerLevel === 'CRITICAL' ? '🔴' :
                               this._currentImpact.dangerLevel === 'HIGH' ? '🟠' :
                               this._currentImpact.dangerLevel === 'MEDIUM' ? '🟡' : '🟢';

            const rootItems: CausalItem[] = [
                new CausalItem(
                    `Символ: ${this._currentImpact.symbol} [${dangerIcon} Ризик: ${this._currentImpact.dangerLevel}]`,
                    vscode.TreeItemCollapsibleState.None,
                    'summary'
                ),
                new CausalItem(
                    `Визначення (${this._currentImpact.definitions.length})`,
                    this._currentImpact.definitions.length > 0 ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None,
                    'defs_group'
                ),
                new CausalItem(
                    `Висхідні виклики / Upstream (${this._currentImpact.upstreamCallers.length})`,
                    this._currentImpact.upstreamCallers.length > 0 ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None,
                    'upstream_group'
                ),
                new CausalItem(
                    `Вхідні залежності / Downstream (${this._currentImpact.downstreamCallees.length})`,
                    this._currentImpact.downstreamCallees.length > 0 ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None,
                    'downstream_group'
                )
            ];
            return rootItems;
        }

        const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '';

        if (element.contextValue === 'defs_group') {
            return this._currentImpact.definitions.map(d => {
                const uri = vscode.Uri.file(path.isAbsolute(d.file) ? d.file : path.join(workspaceFolder, d.file));
                const shortFile = path.basename(d.file);
                const item = new CausalItem(
                    `${shortFile}:${d.line}`,
                    vscode.TreeItemCollapsibleState.None,
                    'def_item',
                    uri,
                    d.line,
                    d.text.trim()
                );
                item.iconPath = new vscode.ThemeIcon('symbol-field');
                return item;
            });
        }

        if (element.contextValue === 'upstream_group') {
            return this._currentImpact.upstreamCallers.map(u => {
                const uri = vscode.Uri.file(path.isAbsolute(u.file) ? u.file : path.join(workspaceFolder, u.file));
                const shortFile = path.basename(u.file);
                const item = new CausalItem(
                    `${u.caller || 'anonymous'} ← ${shortFile}:${u.line}`,
                    vscode.TreeItemCollapsibleState.None,
                    'upstream_item',
                    uri,
                    u.line,
                    u.text ? u.text.trim() : u.caller
                );
                item.iconPath = new vscode.ThemeIcon('arrow-up');
                return item;
            });
        }

        if (element.contextValue === 'downstream_group') {
            return this._currentImpact.downstreamCallees.map(d => {
                const uri = vscode.Uri.file(path.isAbsolute(d.file) ? d.file : path.join(workspaceFolder, d.file));
                const shortFile = path.basename(d.file);
                const item = new CausalItem(
                    `→ ${d.callee} (${shortFile}:${d.line})`,
                    vscode.TreeItemCollapsibleState.None,
                    'downstream_item',
                    uri,
                    d.line,
                    d.callee
                );
                item.iconPath = new vscode.ThemeIcon('arrow-down');
                return item;
            });
        }

        return [];
    }
}

export class QuickFeedViewProvider implements vscode.TreeDataProvider<CausalItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<CausalItem | undefined | null | void> = new vscode.EventEmitter<CausalItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<CausalItem | undefined | null | void> = this._onDidChangeTreeData.event;

    public getTreeItem(element: CausalItem): vscode.TreeItem {
        return element;
    }

    public getChildren(): vscode.ProviderResult<CausalItem[]> {
        const item1 = new CausalItem(
            '🧠 Створити Feed: Understand (Архітектурний скелет)',
            vscode.TreeItemCollapsibleState.None,
            'action_feed_understand'
        );
        item1.iconPath = new vscode.ThemeIcon('book');
        item1.command = {
            command: 'poler.feedUnderstand',
            title: 'Understand Feed'
        };

        const item2 = new CausalItem(
            '🛠️ Створити Feed: Edit (Правочний пакет + Impact)',
            vscode.TreeItemCollapsibleState.None,
            'action_feed_edit'
        );
        item2.iconPath = new vscode.ThemeIcon('edit');
        item2.command = {
            command: 'poler.feedEdit',
            title: 'Edit Feed'
        };

        const item3 = new CausalItem(
            '⚡ Резонансний пошук Ψ (Топографія коду)',
            vscode.TreeItemCollapsibleState.None,
            'action_resonance'
        );
        item3.iconPath = new vscode.ThemeIcon('search');
        item3.command = {
            command: 'poler.resonanceSearch',
            title: 'Resonance Search'
        };

        const item4 = new CausalItem(
            '🔬 Наукова Студія (SciLang + 3D Хімія/Фізика)',
            vscode.TreeItemCollapsibleState.None,
            'action_studio'
        );
        item4.iconPath = new vscode.ThemeIcon('beaker');
        item4.command = {
            command: 'poler.scientificStudio',
            title: 'Open Studio'
        };

        return [item1, item2, item3, item4];
    }
}

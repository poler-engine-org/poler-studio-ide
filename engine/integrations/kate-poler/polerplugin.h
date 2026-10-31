/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#pragma once

#include <KTextEditor/MainWindow>
#include <KTextEditor/Plugin>
#include <ktexteditor/configpage.h>

class PolerPanel;
class PolerPluginView;
class PolerCommands;

/**
 * PolerPlugin — точка входа KTextEditor-плагина «POLER Engine».
 *
 * Kate остаётся Kate: плагин только добавляет инструментальную панель
 * (MainWindow::createToolView, правая сторона) и консольные команды
 * :poler/:pcrystal/:pmotor/:pharvest. Ни один нативный аспект редактора
 * не заменяется и не перехватывается — вся мощь Kate сохранена на 100%,
 * а суверенное ядро poler-engine встраивается рядом как равноправный
 * орган: поиск, кристалл памяти, моторный мост, дисковый сборщик.
 */
class PolerPlugin : public KTextEditor::Plugin
{
public:
    explicit PolerPlugin(QObject *parent = nullptr);
    ~PolerPlugin() override;

    QObject *createView(KTextEditor::MainWindow *mainWindow) override;

    int configPages() const override;
    KTextEditor::ConfigPage *configPage(int number = 0, QWidget *parent = nullptr) override;

public:
    void viewDestroyed(QObject *view);

private:
    QList<PolerPluginView *> m_views;
};

class PolerPluginView : public QObject
{
    Q_OBJECT

public:
    PolerPluginView(KTextEditor::Plugin *plugin, KTextEditor::MainWindow *mainWindow);
    ~PolerPluginView() override;

    PolerPanel *panel() const
    {
        return m_panel;
    }

private:
    bool eventFilter(QObject *obj, QEvent *event) override;

    QWidget *m_toolView = nullptr;
    PolerPanel *m_panel = nullptr;
    KTextEditor::MainWindow *m_mainWindow = nullptr;
    PolerCommands *m_commands = nullptr;

    friend class PolerPlugin;
};

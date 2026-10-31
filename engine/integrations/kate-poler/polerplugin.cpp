/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#include "polerplugin.h"

#include "polercommands.h"
#include "polerpanel.h"

#include <KLocalizedString>
#include <KPluginFactory>

#include <QEvent>
#include <QIcon>
#include <QKeyEvent>

K_PLUGIN_FACTORY_WITH_JSON(PolerPluginFactory, "polerplugin.json", registerPlugin<PolerPlugin>();)

// ---------------------------------------------------------------------------
// PolerPlugin
// ---------------------------------------------------------------------------

PolerPlugin::PolerPlugin(QObject *parent)
    : KTextEditor::Plugin(parent)
{
}

PolerPlugin::~PolerPlugin() = default;

QObject *PolerPlugin::createView(KTextEditor::MainWindow *mainWindow)
{
    auto *view = new PolerPluginView(this, mainWindow);
    connect(view, &PolerPluginView::destroyed, this, &PolerPlugin::viewDestroyed);
    m_views.append(view);
    return view;
}

void PolerPlugin::viewDestroyed(QObject *view)
{
    // Не разыменовывать view — он уже частично разрушен.
    m_views.removeAll(view);
}

int PolerPlugin::configPages() const
{
    return 0;
}

KTextEditor::ConfigPage *PolerPlugin::configPage(int, QWidget *)
{
    return nullptr;
}

// ---------------------------------------------------------------------------
// PolerPluginView
// ---------------------------------------------------------------------------

PolerPluginView::PolerPluginView(KTextEditor::Plugin *plugin, KTextEditor::MainWindow *mainWindow)
    : QObject(mainWindow)
    , m_toolView(mainWindow->createToolView(plugin,
                                            QStringLiteral("kate_private_plugin_polerengineplugin"),
                                            KTextEditor::MainWindow::Right,
                                            QIcon::fromTheme(QStringLiteral("edit-find")),
                                            i18n("POLER Engine")))
    , m_panel(new PolerPanel(mainWindow, m_toolView))
    , m_mainWindow(mainWindow)
    , m_commands(new PolerCommands(m_panel, this))
{
    m_toolView->installEventFilter(this);
}

PolerPluginView::~PolerPluginView()
{
    // Уничтожаем toolview (вместе с панелью-потомком).
    delete m_panel->parent();
}

bool PolerPluginView::eventFilter(QObject *obj, QEvent *event)
{
    if (event->type() == QEvent::KeyPress) {
        auto *ke = static_cast<QKeyEvent *>(event);
        if ((obj == m_toolView) && (ke->key() == Qt::Key_Escape)) {
            m_mainWindow->hideToolView(m_toolView);
            event->accept();
            return true;
        }
    }
    return QObject::eventFilter(obj, event);
}

#include "polerplugin.moc"
#include "moc_polerplugin.cpp"


/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#pragma once

#include <KTextEditor/Command>

#include <QString>
#include <QStringList>

namespace KTextEditor
{
class View;
class Range;
}

class PolerPanel;

/**
 * PolerCommands — консольные команды Kate (появляются в командной строке
 * редактора, vim-режиме и любых местах, где работают команды KTextEditor):
 *
 *   :poler <запрос>     — поиск по каталогу активного документа (вкладка Поиск)
 *   :pcrystal <слово>   — инспекция кристалла памяти Trit5
 *   :pmotor <директива> — моторный мост S2→E2 (RU/UA директивы)
 *   :pharvest <термы>   — полнодисковый сбор в документ
 *
 * Регистрация выполняется автоматически конструктором базового класса
 * KTextEditor::Command(QList<QString>, parent).
 */
class PolerCommands : public KTextEditor::Command
{
    Q_OBJECT

public:
    explicit PolerCommands(PolerPanel *panel, QObject *parent = nullptr);

    bool exec(KTextEditor::View *view,
              const QString &cmd,
              QString &msg,
              const KTextEditor::Range &range = KTextEditor::Range::invalid()) override;
    bool help(KTextEditor::View *view, const QString &cmd, QString &msg) override;

private:
    PolerPanel *m_panel;
};

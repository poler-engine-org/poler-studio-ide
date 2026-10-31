/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#include "polercommands.h"

#include "polerpanel.h"

#include <KTextEditor/View>

#include <KLocalizedString>

namespace
{
QStringList polerCommands()
{
    return {QStringLiteral("poler"), QStringLiteral("pcrystal"), QStringLiteral("pmotor"), QStringLiteral("pharvest")};
}
}

PolerCommands::PolerCommands(PolerPanel *panel, QObject *parent)
    : KTextEditor::Command(polerCommands(), parent)
    , m_panel(panel)
{
}

bool PolerCommands::exec(KTextEditor::View *, const QString &cmd, QString &msg, const KTextEditor::Range &)
{
    const QString name = cmd.section(QLatin1Char(' '), 0, 0).trimmed();
    const QString arg = cmd.section(QLatin1Char(' '), 1).trimmed();

    if (arg.isEmpty()) {
        msg = i18n("Укажи аргумент: :%1 <текст>", name);
        return false;
    }
    if (!m_panel) {
        msg = i18n("Панель POLER недоступна");
        return false;
    }

    if (name == QLatin1String("poler")) {
        m_panel->searchFor(arg);
        msg = i18n("POLER: поиск «%1» запущен (вкладка Поиск)", arg);
        return true;
    }
    if (name == QLatin1String("pcrystal")) {
        m_panel->crystalFor(arg);
        msg = i18n("POLER: инспекция кристалла «%1»", arg);
        return true;
    }
    if (name == QLatin1String("pmotor")) {
        m_panel->motorDirective(arg);
        msg = i18n("POLER: моторная директива «%1» отправлена", arg);
        return true;
    }
    if (name == QLatin1String("pharvest")) {
        m_panel->harvestFor(arg);
        msg = i18n("POLER: сбор диска по термам «%1» запущен", arg);
        return true;
    }
    msg = i18n("Неизвестная команда: %1", name);
    return false;
}

bool PolerCommands::help(KTextEditor::View *, const QString &cmd, QString &msg)
{
    const QString name = cmd.section(QLatin1Char(' '), 0, 0).trimmed();
    if (name == QLatin1String("poler")) {
        msg = i18n(":poler <запрос> — топографический поиск POLER по каталогу активного документа; "
                   "многословный запрос = proximity-AND (все токены в окне ±128). Результаты на вкладке «Поиск».");
        return true;
    }
    if (name == QLatin1String("pcrystal")) {
        msg = i18n(":pcrystal <слово> — синапсы кристалла памяти Trit5 (~/.poler/permanent_memory.t5c): "
                   "возбуждающие (+1) и тормозные (−1) связи слова.");
        return true;
    }
    if (name == QLatin1String("pmotor")) {
        msg = i18n(":pmotor <директива> — моторный мост S2→E2 (RU/UA: открой/покажи/запусти/прочитай/собери). "
                   "R1 ReadOnly исполняется автоматически, M2 Mutating — по подтверждению.");
        return true;
    }
    if (name == QLatin1String("pharvest")) {
        msg = i18n(":pharvest <термы> — полнодисковый сборщик poler_disk_harvester: все совпадения терм "
                   "со всех корней в один документ (вкладка «Сбор»).");
        return true;
    }
    return false;
}

#include "moc_polercommands.cpp"

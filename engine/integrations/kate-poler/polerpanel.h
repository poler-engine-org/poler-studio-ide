/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#pragma once

#include "enginebridge.h"

#include <KTextEditor/MainWindow>

#include <QCheckBox>
#include <QComboBox>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QTabWidget>
#include <QTreeWidget>
#include <QWidget>

/**
 * PolerPanel — инструментальная панель плагина, 4 вкладки:
 *
 *  1. Search   — топографический поиск движка по пути (grep-json: точные строки
 *                с переходом; ai-json: сцены с ε/резонансом).
 *  2. Crystal  — инспекция кристалла памяти Trit5 (--triune-crystal-inspect):
 *                возбуждающие/тормозные синапсы слова.
 *  3. Motor    — моторный мост S2→E2: директивы RU/UA (открой/покажи/запусти…)
 *                с контуром R1/M2 и honest-JSON телеметрией.
 *  4. Harvest  — полнодисковый сборщик (--harvest-disk): термы → один документ
 *                (markdown/json/corpus) с живым прогрессом и открытием результата.
 *
 * Панель встраивается через MainWindow::createToolView(...) справа и не меняет
 * ни одного аспекта Kate: всё нативное остаётся нативным.
 */
class PolerPanel : public QWidget
{
    Q_OBJECT

public:
    explicit PolerPanel(KTextEditor::MainWindow *mainWindow, QWidget *parent = nullptr);
    ~PolerPanel() override;

    // Точки входа для команд :poler/:pmotor/:pharvest/:pcrystal.
public Q_SLOTS:
    void searchFor(const QString &query);
    void motorDirective(const QString &text);
    void harvestFor(const QString &terms);
    void crystalFor(const QString &word);

    void showSearchTab();
    void showCrystalTab();
    void showMotorTab();
    void showHarvestTab();

private:
    void buildSearchTab();
    void buildCrystalTab();
    void buildMotorTab();
    void buildHarvestTab();

    void runSearch();
    void runCrystal();
    void runMotor();
    void runHarvest();
    void openHarvestResult();

    void openFileAtLine(const QString &filePath, int line);

    /** Каталог активного документа (для дефолтного корня поиска/сбора). */
    QString activeDocumentDir() const;

    KTextEditor::MainWindow *const m_mainWindow;
    EngineBridge m_bridge;

    QTabWidget *m_tabs = nullptr;

    // Search
    QWidget *m_searchTab = nullptr;
    QLineEdit *m_searchQuery = nullptr;
    QLineEdit *m_searchPath = nullptr;
    QComboBox *m_searchMode = nullptr; // 0 = grep (строки), 1 = сцены (ai-json)
    QPushButton *m_searchButton = nullptr;
    QTreeWidget *m_searchResults = nullptr;
    QLabel *m_searchStatus = nullptr;

    // Crystal
    QWidget *m_crystalTab = nullptr;
    QLineEdit *m_crystalWord = nullptr;
    QPushButton *m_crystalButton = nullptr;
    QPlainTextEdit *m_crystalOut = nullptr;

    // Motor
    QWidget *m_motorTab = nullptr;
    QLineEdit *m_motorDirective = nullptr;
    QCheckBox *m_motorYes = nullptr;
    QPushButton *m_motorButton = nullptr;
    QPlainTextEdit *m_motorOut = nullptr;

    // Harvest
    QWidget *m_harvestTab = nullptr;
    QLineEdit *m_harvestRoots = nullptr;
    QLineEdit *m_harvestTerms = nullptr;
    QLineEdit *m_harvestOut = nullptr;
    QComboBox *m_harvestFormat = nullptr;
    QPushButton *m_harvestButton = nullptr;
    QPushButton *m_harvestOpenButton = nullptr;
    QPlainTextEdit *m_harvestLog = nullptr;
};

/*
    SPDX-FileCopyrightText: 2026 POLER Engine Org
    SPDX-License-Identifier: LGPL-2.0-or-later
*/
#include "polerpanel.h"

#include "enginebridge.h"

#include <KTextEditor/Cursor>
#include <KTextEditor/Document>
#include <KTextEditor/View>

#include <KLocalizedString>

#include <QDir>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
#include <QUrl>
#include <QVBoxLayout>

PolerPanel::PolerPanel(KTextEditor::MainWindow *mainWindow, QWidget *parent)
    : QWidget(parent)
    , m_mainWindow(mainWindow)
{
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);

    m_tabs = new QTabWidget(this);
    layout->addWidget(m_tabs);

    buildSearchTab();
    buildCrystalTab();
    buildMotorTab();
    buildHarvestTab();

    if (!EngineBridge::available()) {
        m_searchStatus->setText(i18n("⚠ poler-engine не найден — установи в ~/.local/bin или задай $POLER_ENGINE_BIN"));
    }
}

PolerPanel::~PolerPanel() = default;

// ---------------------------------------------------------------------------
// Построение вкладок
// ---------------------------------------------------------------------------

void PolerPanel::buildSearchTab()
{
    m_searchTab = new QWidget(this);
    auto *v = new QVBoxLayout(m_searchTab);

    auto *row1 = new QHBoxLayout();
    m_searchQuery = new QLineEdit(m_searchTab);
    m_searchQuery->setPlaceholderText(i18n("Запрос (слова / фраза; multi-word = proximity-AND)"));
    m_searchQuery->setClearButtonEnabled(true);
    m_searchMode = new QComboBox(m_searchTab);
    m_searchMode->addItem(i18n("Строки (grep)"));
    m_searchMode->addItem(i18n("Сцены (ε, резонанс)"));
    m_searchButton = new QPushButton(i18n("Искать"), m_searchTab);
    row1->addWidget(m_searchQuery, 1);
    row1->addWidget(m_searchMode);
    row1->addWidget(m_searchButton);

    auto *row2 = new QHBoxLayout();
    m_searchPath = new QLineEdit(m_searchTab);
    m_searchPath->setPlaceholderText(i18n("Корень поиска (пусто = каталог активного документа)"));
    m_searchPath->setText(activeDocumentDir());
    auto *browse = new QPushButton(i18n("…"), m_searchTab);
    browse->setFixedWidth(32);
    row2->addWidget(m_searchPath, 1);
    row2->addWidget(browse);

    m_searchResults = new QTreeWidget(m_searchTab);
    m_searchResults->setHeaderLabels({i18n("Файл"), i18n("Строка"), i18n("ε / R"), i18n("Текст / сцена")});
    m_searchResults->setRootIsDecorated(false);
    m_searchResults->setUniformRowHeights(true);
    m_searchResults->setSortingEnabled(true);
    m_searchResults->setColumnWidth(0, 220);
    m_searchResults->setColumnWidth(1, 70);
    m_searchResults->setColumnWidth(2, 90);

    m_searchStatus = new QLabel(m_searchTab);
    m_searchStatus->setWordWrap(true);

    v->addLayout(row1);
    v->addLayout(row2);
    v->addWidget(m_searchResults, 1);
    v->addWidget(m_searchStatus);

    connect(m_searchButton, &QPushButton::clicked, this, &PolerPanel::runSearch);
    connect(m_searchQuery, &QLineEdit::returnPressed, this, &PolerPanel::runSearch);
    connect(browse, &QPushButton::clicked, this, [this] {
        const QString dir = QFileDialog::getExistingDirectory(this, i18n("Корень поиска"), m_searchPath->text());
        if (!dir.isEmpty()) {
            m_searchPath->setText(dir);
        }
    });
    connect(m_searchResults, &QTreeWidget::itemDoubleClicked, this, [this](QTreeWidgetItem *item, int) {
        const QString file = item->data(0, Qt::UserRole).toString();
        const int line = item->data(1, Qt::UserRole).toInt();
        if (!file.isEmpty()) {
            openFileAtLine(file, line > 0 ? line : 1);
        }
    });

    m_tabs->addTab(m_searchTab, QIcon::fromTheme(QStringLiteral("edit-find")), i18n("Поиск"));
}

void PolerPanel::buildCrystalTab()
{
    m_crystalTab = new QWidget(this);
    auto *v = new QVBoxLayout(m_crystalTab);

    auto *row = new QHBoxLayout();
    m_crystalWord = new QLineEdit(m_crystalTab);
    m_crystalWord->setPlaceholderText(i18n("Слово (RU/UA/EN) — синапсы кристалла Trit5"));
    m_crystalWord->setClearButtonEnabled(true);
    m_crystalButton = new QPushButton(i18n("Инспекция"), m_crystalTab);
    row->addWidget(m_crystalWord, 1);
    row->addWidget(m_crystalButton);

    m_crystalOut = new QPlainTextEdit(m_crystalTab);
    m_crystalOut->setReadOnly(true);
    m_crystalOut->setLineWrapMode(QPlainTextEdit::NoWrap);
    QFont mono(QStringLiteral("monospace"));
    mono.setStyleHint(QFont::TypeWriter);
    m_crystalOut->setFont(mono);
    m_crystalOut->setPlaceholderText(i18n("Кристалл ищется в ~/.poler/permanent_memory.t5c\n"
                                          "Синапсы +1 — притяжение, −1 — торможение."));

    v->addLayout(row);
    v->addWidget(m_crystalOut, 1);

    connect(m_crystalButton, &QPushButton::clicked, this, &PolerPanel::runCrystal);
    connect(m_crystalWord, &QLineEdit::returnPressed, this, &PolerPanel::runCrystal);

    m_tabs->addTab(m_crystalTab, QIcon::fromTheme(QStringLiteral("database-index")), i18n("Кристалл"));
}

void PolerPanel::buildMotorTab()
{
    m_motorTab = new QWidget(this);
    auto *v = new QVBoxLayout(m_motorTab);

    auto *row = new QHBoxLayout();
    m_motorDirective = new QLineEdit(m_motorTab);
    m_motorDirective->setPlaceholderText(i18n("Директива RU/UA: «покажи статус git», «открой лог сборки»…"));
    m_motorDirective->setClearButtonEnabled(true);
    m_motorButton = new QPushButton(i18n("Исполнить"), m_motorTab);
    row->addWidget(m_motorDirective, 1);
    row->addWidget(m_motorButton);

    m_motorYes = new QCheckBox(i18n("M2 авто (--motor-yes, мутации без подтверждения)"), m_motorTab);

    m_motorOut = new QPlainTextEdit(m_motorTab);
    m_motorOut->setReadOnly(true);
    {
        QFont mono(QStringLiteral("monospace"));
        mono.setStyleHint(QFont::TypeWriter);
        m_motorOut->setFont(mono);
    }
    m_motorOut->setPlaceholderText(i18n("Моторный мост S2→E2.\n"
                                        "R1 ReadOnly — авто. M2 Mutating — [y/N] (без tty отказ),\n"
                                        "с флагом — авто. Телеметрия: речь + motor_exec события."));

    v->addLayout(row);
    v->addWidget(m_motorYes);
    v->addWidget(m_motorOut, 1);

    connect(m_motorButton, &QPushButton::clicked, this, &PolerPanel::runMotor);
    connect(m_motorDirective, &QLineEdit::returnPressed, this, &PolerPanel::runMotor);

    m_tabs->addTab(m_motorTab, QIcon::fromTheme(QStringLiteral("run-build")), i18n("Мотор"));
}

void PolerPanel::buildHarvestTab()
{
    m_harvestTab = new QWidget(this);
    auto *v = new QVBoxLayout(m_harvestTab);

    m_harvestRoots = new QLineEdit(m_harvestTab);
    m_harvestRoots->setPlaceholderText(i18n("Корни сбора, разделитель «;» (пусто = каталог активного документа)"));
    m_harvestRoots->setText(activeDocumentDir());

    m_harvestTerms = new QLineEdit(m_harvestTab);
    m_harvestTerms->setPlaceholderText(i18n("Термы отбора (OR): гамильтониан hamiltonian ε ∇ …"));

    auto *rowOut = new QHBoxLayout();
    m_harvestOut = new QLineEdit(m_harvestTab);
    m_harvestOut->setPlaceholderText(i18n("Выходной файл (.md / .json / .txt)"));
    m_harvestOut->setText(QDir::home().filePath(QStringLiteral("POLER_HARVEST.md")));
    auto *browse = new QPushButton(i18n("…"), m_harvestTab);
    browse->setFixedWidth(32);
    rowOut->addWidget(m_harvestOut, 1);
    rowOut->addWidget(browse);

    auto *rowCtl = new QHBoxLayout();
    m_harvestFormat = new QComboBox(m_harvestTab);
    m_harvestFormat->addItem(i18n("markdown"));
    m_harvestFormat->addItem(i18n("json"));
    m_harvestFormat->addItem(i18n("corpus (t5c)"));
    m_harvestButton = new QPushButton(i18n("Собрать"), m_harvestTab);
    m_harvestOpenButton = new QPushButton(i18n("Открыть результат"), m_harvestTab);
    m_harvestOpenButton->setEnabled(false);
    rowCtl->addWidget(m_harvestFormat);
    rowCtl->addWidget(m_harvestButton);
    rowCtl->addWidget(m_harvestOpenButton);

    m_harvestLog = new QPlainTextEdit(m_harvestTab);
    m_harvestLog->setReadOnly(true);
    {
        QFont mono(QStringLiteral("monospace"));
        mono.setStyleHint(QFont::TypeWriter);
        m_harvestLog->setFont(mono);
    }
    m_harvestLog->setPlaceholderText(i18n("poler_disk_harvester: SIMD Aho-Corasick + memmap2.\n"
                                          "DoD: 100k файлов < 2.5 c, RSS < 128 МБ. Прогресс — ниже."));

    v->addWidget(m_harvestRoots);
    v->addWidget(m_harvestTerms);
    v->addLayout(rowOut);
    v->addLayout(rowCtl);
    v->addWidget(m_harvestLog, 1);

    connect(m_harvestButton, &QPushButton::clicked, this, &PolerPanel::runHarvest);
    connect(m_harvestTerms, &QLineEdit::returnPressed, this, &PolerPanel::runHarvest);
    connect(browse, &QPushButton::clicked, this, [this] {
        const QString f = QFileDialog::getSaveFileName(this, i18n("Выходной файл"), m_harvestOut->text());
        if (!f.isEmpty()) {
            m_harvestOut->setText(f);
        }
    });
    connect(m_harvestOpenButton, &QPushButton::clicked, this, &PolerPanel::openHarvestResult);

    m_tabs->addTab(m_harvestTab, QIcon::fromTheme(QStringLiteral("folder-open-recent")), i18n("Сбор"));
}

// ---------------------------------------------------------------------------
// Слоты точек входа (команды :poler/…)
// ---------------------------------------------------------------------------

void PolerPanel::searchFor(const QString &query)
{
    showSearchTab();
    m_searchQuery->setText(query);
    runSearch();
}

void PolerPanel::motorDirective(const QString &text)
{
    showMotorTab();
    m_motorDirective->setText(text);
    runMotor();
}

void PolerPanel::harvestFor(const QString &terms)
{
    showHarvestTab();
    m_harvestTerms->setText(terms);
    runHarvest();
}

void PolerPanel::crystalFor(const QString &word)
{
    showCrystalTab();
    m_crystalWord->setText(word);
    runCrystal();
}

void PolerPanel::showSearchTab()
{
    m_tabs->setCurrentWidget(m_searchTab);
}
void PolerPanel::showCrystalTab()
{
    m_tabs->setCurrentWidget(m_crystalTab);
}
void PolerPanel::showMotorTab()
{
    m_tabs->setCurrentWidget(m_motorTab);
}
void PolerPanel::showHarvestTab()
{
    m_tabs->setCurrentWidget(m_harvestTab);
}

// ---------------------------------------------------------------------------
// Вызовы движка
// ---------------------------------------------------------------------------

void PolerPanel::runSearch()
{
    const QString query = m_searchQuery->text().trimmed();
    if (query.isEmpty() || m_bridge.busy()) {
        return;
    }
    QString root = m_searchPath->text().trimmed();
    if (root.isEmpty()) {
        root = activeDocumentDir();
    }
    if (root.isEmpty()) {
        root = QDir::homePath();
    }

    m_searchResults->clear();
    m_searchStatus->setText(i18n("Поиск…"));

    if (m_searchMode->currentIndex() == 0) {
        // grep-json: точные строки с номерами.
        QStringList args{root, QStringLiteral("--grep"), query, QStringLiteral("--grep-json"), QStringLiteral("--grep-i")};
        m_bridge.run(args, [this](int code, const QString &out, const QString &err) {
            if (code == 0 || code == 1) {
                const auto doc = QJsonDocument::fromJson(out.toUtf8());
                const auto groups = doc.object().value(QLatin1String("groups")).toArray();
                int n = 0;
                for (const auto &g : groups) {
                    const auto obj = g.toObject();
                    const QString path = obj.value(QLatin1String("path")).toString();
                    const auto lines = obj.value(QLatin1String("lines")).toArray();
                    for (const auto &l : lines) {
                        const auto lo = l.toObject();
                        if (!lo.value(QLatin1String("matched")).toBool()) {
                            continue;
                        }
                        auto *item = new QTreeWidgetItem(m_searchResults);
                        item->setText(0, QFileInfo(path).fileName());
                        item->setToolTip(0, path);
                        const int lineNo = lo.value(QLatin1String("line_no")).toInt();
                        item->setText(1, QString::number(lineNo));
                        item->setTextAlignment(1, Qt::AlignRight);
                        item->setText(3, lo.value(QLatin1String("text")).toString());
                        item->setData(0, Qt::UserRole, path);
                        item->setData(1, Qt::UserRole, lineNo);
                        ++n;
                    }
                }
                m_searchStatus->setText(code == 0 ? i18n("Найдено строк: %1", n) : i18n("Ничего не найдено"));
            } else {
                m_searchStatus->setText(i18n("Ошибка (код %1): %2", code, err.trimmed()));
            }
        });
    } else {
        // ai-json: сцены с ε и резонансом.
        QStringList args{root, QStringLiteral("-q"), query, QStringLiteral("--format"), QStringLiteral("ai-json")};
        m_bridge.run(args, [this](int code, const QString &out, const QString &err) {
            if (code == 0 || code == 1) {
                const auto doc = QJsonDocument::fromJson(out.toUtf8());
                const auto anchors = doc.object().value(QLatin1String("anchors")).toArray();
                int n = 0;
                for (const auto &a : anchors) {
                    const auto obj = a.toObject();
                    const QString path = obj.value(QLatin1String("file")).toString();
                    const auto scene = obj.value(QLatin1String("scene")).toObject();
                    const QString chapter = scene.value(QLatin1String("chapter")).toString();
                    const double eps = obj.value(QLatin1String("epsilon")).toDouble();
                    const double res = obj.value(QLatin1String("resonance")).toDouble();
                    auto *item = new QTreeWidgetItem(m_searchResults);
                    item->setText(0, QFileInfo(path).fileName());
                    item->setToolTip(0, path);
                    item->setText(1, QStringLiteral("—"));
                    item->setText(2, QStringLiteral("ε=%1 R=%2").arg(eps, 0, 'f', 0).arg(res, 0, 'f', 0));
                    item->setText(3, chapter);
                    item->setData(0, Qt::UserRole, path);
                    // Диапазон строк из главы вида "main [block] (L1857–L1860)".
                    static const QRegularExpression re(QStringLiteral("\\(L(\\d+)"));
                    const auto m = re.match(chapter);
                    item->setData(1, Qt::UserRole, m.hasMatch() ? m.captured(1).toInt() : 0);
                    ++n;
                }
                m_searchStatus->setText(n > 0 ? i18n("Сцен: %1 (двойной клик — перейти)", n) : i18n("Ничего не найдено"));
            } else {
                m_searchStatus->setText(i18n("Ошибка (код %1): %2", code, err.trimmed()));
            }
        });
    }
}

void PolerPanel::runCrystal()
{
    const QString word = m_crystalWord->text().trimmed();
    if (word.isEmpty() || m_bridge.busy()) {
        return;
    }
    m_crystalOut->clear();
    m_crystalOut->setPlainText(i18n("Инспекция «%1»…", word));
    QStringList args{QStringLiteral("--triune-crystal-inspect"), word};
    m_bridge.run(args, [this](int code, const QString &out, const QString &err) {
        if (code == 0) {
            m_crystalOut->setPlainText(out.trimmed());
        } else {
            m_crystalOut->setPlainText(i18n("Ошибка (код %1):\n%2", code, err.trimmed()));
        }
    });
}

void PolerPanel::runMotor()
{
    const QString directive = m_motorDirective->text().trimmed();
    if (directive.isEmpty() || m_bridge.busy()) {
        return;
    }
    m_motorOut->clear();
    m_motorOut->setPlainText(i18n("Директива: «%1» → моторный мост S2→E2…", directive));

    QStringList args{QStringLiteral("--triune-speak"),
                     directive,
                     QStringLiteral("--motor-act"),
                     QStringLiteral("--triune-json"),
                     QStringLiteral("--triune-tokens"),
                     QStringLiteral("24")};
    if (m_motorYes->isChecked()) {
        args << QStringLiteral("--motor-yes");
    }
    m_bridge.run(args, [this](int code, const QString &out, const QString &err) {
        QString report;
        const auto doc = QJsonDocument::fromJson(out.toUtf8());
        const auto speech = doc.object().value(QLatin1String("speech")).toArray();
        for (const auto &s : speech) {
            const auto utt = s.toObject().value(QLatin1String("utterance")).toObject();
            report += QStringLiteral("речь: ") + utt.value(QLatin1String("text")).toString() + QLatin1Char('\n');
        }
        const auto motor = doc.object().value(QLatin1String("motor_exec")).toArray();
        if (!motor.isEmpty()) {
            report += QStringLiteral("\n─ motor_exec ─\n");
            for (const auto &m : motor) {
                const auto ev = m.toObject().value(QLatin1String("event")).toObject();
                report += QStringLiteral("[%1] %2 → %3 : %4\n")
                              .arg(ev.value(QLatin1String("level")).toString(),
                                   ev.value(QLatin1String("verb")).toString(),
                                   ev.value(QLatin1String("program")).toString(),
                                   ev.value(QLatin1String("status")).toString());
            }
        }
        if (report.isEmpty() && !out.trimmed().isEmpty()) {
            report = out.trimmed(); // честный fallback: сырой JSON
        }
        if (!err.trimmed().isEmpty()) {
            report += QStringLiteral("\n─ stderr ─\n") + err.trimmed();
        }
        m_motorOut->setPlainText(report.isEmpty() ? i18n("Пустой ответ (код %1)", code) : report);
    });
}

void PolerPanel::runHarvest()
{
    const QString terms = m_harvestTerms->text().trimmed();
    if (terms.isEmpty() || m_bridge.busy()) {
        return;
    }
    QStringList roots;
    for (const QString &r : m_harvestRoots->text().split(QLatin1Char(';'))) {
        const QString t = r.trimmed();
        if (!t.isEmpty()) {
            roots << t;
        }
    }
    if (roots.isEmpty()) {
        const QString d = activeDocumentDir();
        if (d.isEmpty()) {
            m_harvestLog->setPlainText(i18n("Укажи хотя бы один корень сбора."));
            return;
        }
        roots << d;
    }
    const QString outPath = m_harvestOut->text().trimmed();
    if (outPath.isEmpty()) {
        m_harvestLog->setPlainText(i18n("Укажи выходной файл."));
        return;
    }

    m_harvestLog->clear();
    m_harvestOpenButton->setEnabled(false);
    m_harvestButton->setEnabled(false);

    QStringList args{QStringLiteral("--harvest-disk")};
    args << roots;
    args << QStringLiteral("--harvest-query") << terms << QStringLiteral("--harvest-out") << outPath;
    switch (m_harvestFormat->currentIndex()) {
    case 1:
        args << QStringLiteral("--harvest-format") << QStringLiteral("json");
        break;
    case 2:
        args << QStringLiteral("--harvest-format") << QStringLiteral("corpus");
        break;
    default:
        args << QStringLiteral("--harvest-format") << QStringLiteral("markdown");
        break;
    }

    const QString out = outPath;
    m_bridge.run(
        args,
        [this, out](int code, const QString &, const QString &err) {
            m_harvestButton->setEnabled(true);
            if (code == 0 || code == 1) {
                m_harvestLog->appendPlainText(err.trimmed());
                m_harvestLog->appendPlainText(i18n("\nГотово. Результат: %1", out));
                m_harvestOpenButton->setEnabled(true);
            } else {
                m_harvestLog->appendPlainText(i18n("Ошибка (код %1):\n%2", code, err.trimmed()));
            }
        },
        0,
        [this](const QString &chunk) {
            m_harvestLog->appendPlainText(chunk.trimmed());
        });
}

void PolerPanel::openHarvestResult()
{
    const QString out = m_harvestOut->text().trimmed();
    if (!out.isEmpty() && QFileInfo::exists(out)) {
        openFileAtLine(out, 1);
    }
}

// ---------------------------------------------------------------------------
// Навигация
// ---------------------------------------------------------------------------

void PolerPanel::openFileAtLine(const QString &filePath, int line)
{
    KTextEditor::View *view = m_mainWindow->openUrl(QUrl::fromLocalFile(filePath));
    if (view && line > 0) {
        view->setCursorPosition(KTextEditor::Cursor(line - 1, 0));
    }
}

QString PolerPanel::activeDocumentDir() const
{
    if (KTextEditor::View *view = m_mainWindow->activeView()) {
        if (KTextEditor::Document *doc = view->document()) {
            const QUrl url = doc->url();
            if (url.isLocalFile()) {
                const QString dir = QFileInfo(url.toLocalFile()).absolutePath();
                if (!dir.isEmpty()) {
                    return dir;
                }
            }
        }
    }
    return QString();
}

#include "moc_polerpanel.cpp"

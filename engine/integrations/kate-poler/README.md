# poler-kate — Kate × POLER Engine

**Нативный плагин KTextEditor (KF6/Qt6/C++)**, встраивающий суверенное ядро
`poler-engine` в Kate: топографический поиск, кристалл памяти Trit5, моторный
мост S2→E2 и полнодисковый сборщик — без единой потери нативного функционала
и внешнего вида Kate.

## Почему плагин, а не форк

Требование: «сохранить весь функционал и внешний вид» Kate 26.08. Форк означал
бы вечную гонку с апстримом KDE и риск потери нативности. Плагин — официальный
механизм расширения: Kate остаётся Kate на 100%, а движок садится рядом как
равноправный орган через публичный API `KTextEditor::Plugin` +
`MainWindow::createToolView` + `KTextEditor::Command`.

Архитектурный принцип: **плагин не дублирует алгоритмы движка** — он вызывает
бинарник `poler-engine` и парсит его готовый JSON (`grep-json`, `ai-json`,
`triune-json`). Все гарантии движка (O(1) кольцевые буферы, таймаут-каскады
SIGTERM→SIGKILL, отсутствие зомби и TTY-дедлоков — см.
`docs/ENGINE_EXECUTION_PROTOCOL.md`) наследуются автоматически.

## Что появляется в Kate

Панель **POLER Engine** (правая сторона, скрывается по Esc) с 4 вкладками:

| Вкладка | Возможности | Вызов движка |
|---|---|---|
| **Поиск** | точные строки с переходом по двойному клику; сцены с ε/резонансом; multi-word = proximity-AND | `<root> --grep Q --grep-json --grep-i` / `<root> -q Q --format ai-json` |
| **Кристалл** | синапсы слова в Trit5: возбуждающие (+1) / тормозные (−1) | `--triune-crystal-inspect WORD` |
| **Мотор** | директивы RU/UA («покажи статус git»), R1 авто / M2 с подтверждением, телеметрия речи + motor_exec | `--triune-speak T --motor-act [--motor-yes] --triune-json` |
| **Сбор** | полнодисковый харвестер: корни, термы, формат (markdown/json/corpus), живой прогресс, открытие результата | `--harvest-disk ROOTS --harvest-query T --harvest-out F` |

Консольные команды (командная строка Kate / vim-режим):

```
:poler <запрос>       — поиск по каталогу активного документа
:pcrystal <слово>     — инспекция кристалла памяти
:pmotor <директива>   — моторный мост S2→E2
:pharvest <термы>     — полнодисковый сбор
```

## Сборка и установка (Arch Linux)

```bash
# 1. Зависимости (kate уже установлен — ktexteditor подтянется):
sudo pacman -S base-devel cmake extra-cmake-modules qt6-base \
               kf6-texteditor kf6-kcoreaddons kf6-ki18n

# 2. Сборка из корня репозитория poler-engine:
cd integrations/kate-poler
cmake -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
cmake --build build -j$(nproc)

# 3. Установка (плагин ляжет рядом с плагинами Kate: kf6/ktexteditor/):
sudo cmake --install build

# 4. Движок должен быть в PATH (поиск: $POLER_ENGINE_BIN →
#    ~/.local/bin/poler-engine → /usr/local/bin → /usr/bin → PATH):
which poler-engine || ls ~/.local/bin/poler-engine
```

Включение: **Kate → Settings → Configure Kate → Plugins → «POLER Engine»**.
Панель появится справа; панель/вкладки виджетов Kate настраиваются как обычно
(перетаскивание, скрытие, профили сессий сохраняются).

## Файлы

| Файл | Роль |
|---|---|
| `polerplugin.json` | метаданные KPlugin (Id: polerengineplugin) |
| `polerplugin.h/.cpp` | `PolerPlugin` (createView) + `PolerPluginView` (toolview, Esc) |
| `polerpanel.h/.cpp` | панель: 4 вкладки, парсинг JSON, навигация к строке |
| `enginebridge.h/.cpp` | асинхронный QProcess-мост + живой stderr-прогресс |
| `polercommands.h/.cpp` | команды `:poler/:pcrystal/:pmotor/:pharvest` |

## Технические решения

- **K_PLUGIN_FACTORY_WITH_JSON** + `kcoreaddons_add_plugin(INSTALL_NAMESPACE
  "kf6/ktexteditor")` — тот же механизм, каким Kate собирает собственные
  addons (проверено по `addons/CMakeLists.txt` апстрима).
- Навигация к строке: `MainWindow::openUrl` + `View::setCursorPosition`;
  для сцен номер строки извлекается из `scene.chapter` («… (L1857–L1860)»),
  для grep — точный `line_no` из `grep-json`.
- Один активный вызов движка на мост (защита от гонок), отмена — SIGTERM→SIGKILL.
- Колбэки завершения защищены `shared_ptr<bool>` (однократный вызов, no UB).
- Кириллица: движок сам матчит RU/UA во всех регистрах (Aho-Corasick).

## Дорожная карта

- Инкрементальное дообучение кристалла из панели (`--crystal-ingest-dir`).
- Инлайн-подсветка резонанса в редакторе (KTextEditor::MovingRange).
- Автосбор перед сессией: `--harvest-format corpus` → кристалл.
- LSP-подобный режим подсказок из кристалла (KTextEditor::CodeCompletionModel).

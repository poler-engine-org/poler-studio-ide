# Native Retrieval: анализ трёх библиотек (v0.20.0)

Дата: 2026-08-30. Задача (владелец): соединить grep- и RAG-инструменты
с нашими алгоритмами — один бинарник для ИИ-агента, у которого всё под
капотом. Проанализированы GNU grep и RAG-эталон chunking-библиотеки;
POLER-Quantum-RS в анализе не участвует (кристалл — продукт, движок —
инструмент; семантический слой из него не берём).

## Источники

| Библиотека | Что взято на разбор | Почему |
|---|---|---|
| GNU grep ( зеркало GitMirroring/grep, оригинал Savannah) | `src/grep.c` (контекст, exit-коды, режимы вывода), `src/kwsearch.c` (literal-поиск kwset), `src/dfasearch.c` (DFA+regex) | эталон точного поиска: полнота, streaming, скриптовая совместимость |
| benbrandt/text-splitter 0.x (Rust, 628★) | `src/splitter.rs` (сборка чанков, binary search по capacity, overlap-курсор), `src/splitter/text.rs` (иерархия семантических уровней), `src/chunk_size.rs`, `src/trim.rs` | чистая Rust-реализация semantic chunking; та же философия «без тяжёлых зависимостей» |
| LangChain RecursiveCharacterTextSplitter (сверочно) | правила рекурсивной нарезки `\n\n → \n → " " → ""` + chunk_overlap | самый распространённый RAG-дефолт, сверка эвристик |

## Gap-таблица: слой A — точный поиск (grep-семантика)

| Функция | GNU grep | poler-engine до v0.20.0 | Решение |
|---|---|---|---|
| Literal-поиск всех совпадений | kwset (Boyer-Moore/Aho-Corasick) | крейт `aho-corasick` в deps, не экспонирован | `retrieval::grep` Literal-матчер на AC |
| Regex-поиск (`-E`) | DFA + PCRE fallback | крейт `regex` в deps, не экспонирован | Regex-матчер |
| Регистронезависимость (`-i`) | fold + locale | — | `RegexBuilder::case_insensitive` (Unicode, шире grep) |
| Рекурсивный обход + .gitignore | find + `--exclude` | `ignore::WalkBuilder` уже в `lib.rs` | переиспользовать |
| Контекст `-A/-B/-C` | `out_before`/`out_after`, pending-буфер, `lastout`-смежность | — | pending-буфер + групповой разделитель `--` |
| `-c` (count), `-l/-L` (list) | `count_matches`, `list_files` | — | режимы Count / ListMatching / ListNonMatching |
| Exit-коды для скриптов | 0 = найдено, 1 = пусто, 2 = ошибка | — | совместимо 0/1/2 |
| Бинарные файлы | «Binary file X matches» | — | NUL-детект, пропуск содержимого |
| `-m` max-count | per-file остановка | — | `max_count` |
| Структурный вывод для агента | — | `serde_json`, `context_anchor` | **наше преимущество**: JSON с byte-offsets, line ranges |
| MCP-инструмент | — | `mcp.rs` (7 инструментов) | `poler_grep` |

## Gap-таблица: слой B — passage-уровень (RAG-чанки)

| Функция | text-splitter / LangChain | poler-engine до v0.20.0 | Решение |
|---|---|---|---|
| Иерархия семантических уровней | grapheme → word → sentence → серии `\n` | `SceneLocator` (границы по заголовкам) + `web_tokenize` (слова) | своя: **heading → paragraph → sentence → word** |
| Вместимость чанка | `ChunkSizer` (chars/tokens) | токенизатор есть | размер в токенах POLER |
| Слияние соседних секций | binary search + merge | — | жадное слияние до capacity |
| Overlap | binary search курсора назад | — | курсор назад на N токенов по границам предложений |
| Trim краёв | `Trim::Start/End` | — | trim пробелов |
| Markdown-структура | markdown splitter (свой парсер) | `markdown_scenes` уже написан | переиспользовать уровни заголовков |
| Код-структура | tree-sitter (тяжёлая зависимость) | `ast_code.rs` свой, 0 зависимостей | **наше преимущество**: границы по blank-line/скоупам без tree-sitter |
| Якоря и breadcrumbs | — | `context_anchor.rs`, `SceneContext` | **наше преимущество**: chunk = {text, byte_range, line_range, breadcrumb, tokens} |
| CLI / MCP | — | — | `--chunk` + `poler_chunk` |

## Выводы

1. Алгоритмическое ядро у движка УЖЕ своё (обходчик ripgrep-класса,
   AC/regex/memchr-крейты, токенизатор, структурные парсеры) — не хватает
   только оболочки: grep-семантики вывода и чанкер-конвейера.
2. Ни одной новой зависимости не требуется.
3. Ключевое отличие от обоих эталонов — агентная ориентация: каждый
   результат несёт машинные координаты (byte ranges, breadcrumbs, якоря),
   а не только человекочитаемый текст.
4. Слой лексического ранжирования (BM25 + PageRank + title + ε) не
   трогаем: три слоя остаются независимыми и объяснимыми.

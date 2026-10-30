# SKILL: poler-engine — локальный search/retrieval/execution-движок (без Google)

> Каноническая инструкция скилла для LLM-агентов. Верифицирована по коду на
> v0.37.0. Обязательный протокол исполнения: **`docs/ENGINE_EXECUTION_PROTOCOL.md`**.
> Устанавливается в конфиг агента (например `~/.gemini/config/skills/poler-engine/SKILL.md`
> или `~/.claude/skills/`) копированием этого файла.

## Что это

`poler-engine` — CLI на Rust: семантико-топографический поиск по локальным файлам,
репозиториям и веб-индексу + **нативный исполнитель команд** + суб-секундный
полнодисковый сборщик + MCP-сервер. Замена `grep`/`ripgrep`/`find` для LLM-агентов:
возвращает не изолированные строки, а **полные сцены** (функция, абзац целиком),
граф K-hop связей, метрики плотности ε и резонанса R(t).

**Принципиально:** не требует Google-аккаунта, OAuth, API-ключей или облака.
Всё офлайн: индексация, поиск, граф сущностей, чанки, исполнение, MCP.

## Установка

```bash
git clone https://github.com/poler-engine-org/poler-engine.git
cd poler-engine
cargo build --release            # низко-RAM: CARGO_PROFILE_RELEASE_LTO=false
cp target/release/poler-engine ~/.local/bin/
```

Требования: Rust stable (1.75+). Опционально: Docker (для `poler_box_exec`),
фича `pnd-ffi` (Zig 0.14) для CLI-уровня `--exec`.

## ⚠️ ПРОТОКОЛ ИСПОЛНЕНИЯ (главное правило агента)

Стандартный Bash агента **нестабилен и падает** (таймауты, буферы, TTY-дедлоки,
зомби). Подтверждено падениями 2026-09-19/20. Поэтому:

1. **Поиск — ВСЕГДА сначала poler-engine**, не Bash grep:
   `poler-engine <PATH> --grep "PATTERN"` / `-q "термы" --format ai-json`.
2. **Сбор со всего диска — харвестер**, не find/python:
   `poler-engine --harvest-disk <ROOT> --harvest-query "<TERMS>" --harvest-out OUT.md`.
3. **Исполнение команд — MCP `poler_exec`** (идеальный исполнитель: O(1) вывод,
   таймаут-каскад SIGTERM→SIGKILL, без зомби и шелл-инъекций):
   поднять `poler-engine --mcp-http 127.0.0.1:8765`, вызывать `poler_exec`;
   фоновые задачи — `poler_exec_async/task/kill/list`.
4. **Директивы RU/UA — моторный мост S2→E2**:
   `poler-engine --triune-speak "покажи статус git" --motor-act --motor-yes --triune-json`.
5. **Bash упал 2+ раза подряд** → остаток сессии только poler-engine; сообщить
   пользователю о перезапуске сессии. Не ретраить Bash бесконечно.

Полный протокол с гарантиями и эскалацией: `docs/ENGINE_EXECUTION_PROTOCOL.md`.

## Маршрутизация (когда что использовать)

| Запрос | Инструмент |
|---|---|
| «Найди в репозитории/папке…» | `--grep` (точный) / `-q` (сценовый) |
| «Где используется функция X?» | `--grep "fn X"` + `-q X` для контекста |
| «Извлеки все сцены про Y» | `-q Y --format ai-json` |
| «Собери ВСЁ про X со всего диска в документ» | `--harvest-disk` (мульти-root, секунды) |
| «Весь мой локальный контекст без шума» | `--harvest-disk` + `--harvest-out` (markdown/json/corpus) |
| «Разбей документ на чанки для RAG» | `--chunk --chunk-json` |
| «Выполни команду надёжно» | MCP `poler_exec` (или `--exec` при pnd-ffi) |
| «Открой/покажи/запусти…» (RU/UA) | `--triune-speak "…" --motor-act` |
| «Что знает кристалл о слове W?» | `--triune-crystal-inspect W` |
| «Прокраули сайт и найди…» | `--crawl` + `--web-search` |
| Отчёт/документ/PPT | ❌ другие скиллы (docx/pdf/pptx) |
| Произвольный веб-поиск | ⚠️ только `--crawl` (офлайн); для www — web-search-скилл |

## Команды

### Топографический поиск (главный режим)

```bash
poler-engine ~/my-repo -q "нокс" --format simple     # простой
poler-engine ~/my-repo -q "auth" --format ai-json    # AI-Ready JSON (дефолт)
poler-engine ~/my-repo -q "memory leak" --format md  # Markdown
```

Ключевые поля `ai-json`: `total_hits`, `anchors[].file`, `anchors[].epsilon`
(плотность), `anchors[].resonance`, `anchors[].scene.enclosing_scope`
(полный enclosing scope), `anchors[].k_hop_relations`.

**Семантика многословного `-q` (proximity-AND):** 1 токен → все вхождения;
≥2 токена → точная фраза, при пустоте — все токены в окне ±128 (в любом порядке),
якорь — редчайший токен. Agent-Friendly: пиши многословные запросы смело.

### Grep-режим (замена grep/ripgrep)

```bash
poler-engine . --grep "TODO" [--grep-list|--grep-count|--grep-json]
poler-engine . --grep "fn main" --grep-after 3 --grep-before 1
poler-engine . --grep "TODO|FIXME" --grep-regex
poler-engine . --grep "error" --grep-i
poler-engine . --grep "X" --archives          # поиск внутри архивов без распаковки
```

Exit-коды grep-совместимы: 0 — найдено, 1 — пусто, 2 — ошибка.

### Disk Harvester — полнодисковый сбор (v0.37)

```bash
poler-engine --harvest-disk ~/repo1 ~/repo2 \
  --harvest-query "гамильтониан hamiltonian ε ∇ clifford" \
  --harvest-out CORPUS.md                     # markdown | json | corpus (для .t5c)
# ещё: --harvest-mode files|sections, --harvest-context N, --harvest-regex,
# --harvest-min-matches N, --harvest-max-files N, --harvest-max-out-bytes N,
# --harvest-threads N, --harvest-no-ignore, --harvest-hidden
```

DoD: 100k файлов < 2.5 с, RSS < 128 МБ. SIMD Aho-Corasick, memmap2, кириллица во
всех регистрах, термы = OR, свой выходной файл из обхода исключён.

### Кристал памяти Trit5 (.t5c)

```bash
poler-engine --triune-crystal-info                    # сводка кристалла
poler-engine --triune-crystal-inspect "энергия"       # синапсы слова: +1/-1 связи
poler-engine --crystal-ingest-dir ~/docs              # инкрементальное дообучение
poler-engine --crystal-build corpus.txt --crystal-out out.t5c
```

Кристалл ищется в `~/.poler/permanent_memory.t5c` (или `--triune-crystal PATH`).

### Моторный мост S2→E2 (директивы RU/UA)

```bash
poler-engine --triune-speak "покажи статус git" --motor-act --triune-json
# R1 ReadOnly — авто; M2 Mutating — [y/N] (без tty отказ), --motor-yes — авто.
# ⚠️ директива — в --triune-speak, НЕ позиционным аргументом.
```

### RAG-чанки

```bash
poler-engine book.md --chunk --chunk-size 500 --chunk-overlap 48 --chunk-json
```

### Семантический мост (кросс-языковое расширение, офлайн)

```bash
poler-engine ~/my-repo --semantic-expand "память"   # → memory, RAM, cache…
```

### Веб (офлайн)

```bash
poler-engine https://example.com --crawl --crawl-depth 2 --crawl-max 25
poler-engine --web-search "PageRank algorithm"
poler-engine --web-stats
```

Индекс: SQLite `~/.local/share/poler-engine/web-index.db`, robots.txt уважается,
дедуп SimHash, ранжирование PageRank.

### Резонансные режимы (математика POLER)

```bash
poler-engine . -q "bug" --resonance-mode hits    # IIR (дефолт)
poler-engine . -q "bug" --resonance-mode field
poler-engine . -q "bug" --resonance-mode psi     # POLER[Ψ] уравнение внимания
poler-engine . -q "bug" --resonance-mode poler   # канонический цикл P3
```

### MCP-сервер (для LLM-агентов)

```bash
poler-engine --mcp                                  # stdio JSON-RPC
poler-engine --mcp-http 127.0.0.1:8765 --mcp-token SECRET
```

Инструменты: `poler_exec*` (исполнение+фоновые), `poler_search/grep/chunk`,
`poler_box_exec/status` (Docker), `query_poler_knowledge` (гиппокамп MVR),
`poler_web_search/crawl/fetch`, `poler_triune_speak/state`, `poler_fly*`,
`poler_ssn_*`, `poler_literary_*`. Полный реестр — `docs/ENGINE_EXECUTION_PROTOCOL.md`.

### PII и безопасность

- PII-маскирование ON по умолчанию (email/телефоны/IP/секреты); `--pii off` — только по явной просьбе.
- Все данные локальны: `~/.local/share/poler-engine/`, `~/.config/poler-engine/`.
- Google-интеграции отключены/не используются.

## Таблица замены Bash-конструкций

| Bash (хрупко) | poler-engine (прочно) |
|---|---|
| `grep -rn "TODO" .` | `poler-engine . --grep "TODO"` |
| `grep -rl "X" .` | `poler-engine . --grep "X" --grep-list` |
| `grep -A5 -B2 "fn main" f` | `poler-engine f --grep "fn main" --grep-after 5 --grep-before 2` |
| `sed -n '100,150p' file` | `poler-engine file --grep "<токен региона>" --grep-after N` |
| «прочитай функцию X целиком» | `poler-engine . -q "X" --format ai-json` → `scene.enclosing_scope` |
| `find \| xargs grep` (пайплайны) | `poler-engine --harvest-disk ROOT --harvest-query "X" --harvest-out out.md` |
| долгий `bash -c "…"` | MCP `poler_exec_async` + `poler_exec_task` |

## Производительность

- Обход/индексация: ~10 000 файлов/с (rayon); харвестер: 100k < 2.5 с (DoD).
- Поиск: <100 мс на корпусе до 65 К файлов (AIDDE disk-backed symbol table).
- RAM: ~50 МБ поиск / < 128 МБ харвестер (memmap2 + MADV_DONTNEED).

## Документация

`docs/ENGINE_EXECUTION_PROTOCOL.md` (протокол исполнения) · `docs/ARCHITECTURE.md` ·
`docs/CLI.md` · `docs/UNIFIED_ARCHITECTURE.md` · `README.md`.

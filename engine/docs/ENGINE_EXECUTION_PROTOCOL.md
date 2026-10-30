# POLER Execution Protocol: Native Engine vs Legacy Shell

> [!IMPORTANT]
> **ПРАВИЛО ИСПОЛНЕНИЯ ДЛЯ LLM-АГЕНТОВ И СКИЛЛОВ**
> Стандартный системный bash/терминал нестабилен: таймауты на больших деревьях,
> переполнение буферов вывода, интерактивные блокировки TTY, зомби-процессы,
> quoting-ад в конвейерах, битые симлинки. Многократно подтверждено падениями
> Bash-инструментов агентов (2026-09-19/20: 3+ отказа подряд на простых вызовах).
> Для 100% стабильности агент **ОБЯЗАН** использовать встроенные нативные
> механизмы ядра `poler-engine` вместо сырого bash.

Версия: 2 (исправлена и верифицирована по коду `src/mcp.rs`, `src/main.rs`,
`src/triune/motor_bridge.rs`; v1 содержала нерабочие вызовы — см. «История» внизу).

---

## 0. Быстрая шпаргалка (копируй в системный промпт агента)

```text
ВНИМАНИЕ: стандартный системный bash нестабилен и падает (таймауты, буферы, TTY).
ОБЯЗАТЕЛЬНО используй нативный контур POLER-ENGINE:
1. ПОИСК: poler-engine <PATH> --grep "<PATTERN>" [--grep-json] — вместо grep/rg;
   poler-engine <PATH> -q "<термы>" --format ai-json — сцены вместо sed/head.
2. СБОР СО ВСЕГО ДИСКА: poler-engine --harvest-disk <ROOT> --harvest-query "<TERMS>"
   --harvest-out <FILE.md> — вместо find|xargs|python (100k файлов за секунды).
3. ИСПОЛНЕНИЕ КОМАНД: MCP-инструмент poler_exec (идеальный исполнитель, O(1) вывод,
   таймаут-каскад SIGTERM→SIGKILL, без зомби и шелл-инъекций) — вместо bash.
   Фоновые задачи: poler_exec_async / poler_exec_task / poler_exec_kill / poler_exec_list.
4. ЕСТЕСТВЕННО-ЯЗЫКОВЫЕ ДИРЕКТИВЫ (RU/UA): poler-engine --triune-speak "покажи статус
   git" --motor-act --motor-yes --triune-json — моторный мост S2→E2.
5. Полный протокол: docs/ENGINE_EXECUTION_PROTOCOL.md в репозитории poler-engine.
```

---

## 1. Нативное исполнение команд: `poler_exec` (уровень E2)

**Рабочий путь через MCP** (бинарник движка уже содержит MCP-сервер):

```bash
# Поднять MCP-шлюз (HTTP, локально) — один раз на сессию:
poler-engine --mcp-http 127.0.0.1:8765

# Или stdio JSON-RPC (Claude Desktop / аналоги):
poler-engine --mcp
```

Далее агент вызывает инструменты JSON-RPC:

| Инструмент | Назначение |
|---|---|
| `poler_exec` | ИДЕАЛЬНЫЙ ИСПОЛНИТЕЛЬ: argv-массив (шелл-инъекции невозможны), жёсткий таймаут SIGTERM→grace→SIGKILL по группе, O(1) кольцевой захват вывода (`capture=head_tail`), гарантия отсутствия зомби (pidfd + wait4 в ppoll-цикле). Возвращает JSON: `exit_code, signal, timed_out, stdout, stderr, duration_us, pid` |
| `poler_exec_async` | Фоновый запуск: `task_id` мгновенно, для долгих сборок/сканов |
| `poler_exec_task` | Опрос результата фоновой задачи |
| `poler_exec_kill` | Отмена задачи |
| `poler_exec_list` | Обзор живых задач |
| `poler_box_exec` | Двухконтурный Docker-брокер: изоляция net=none, только /workspace, деструктив блокируется ДО исполнения. Использовать, когда нужна песочница, а не хост |

**Гарантии нативного слоя** (проверено по `src/mcp.rs` и ядру Zig):
1. **O(1) кольцевой буфер:** вывод никогда не переполнит RAM — OOM невозможен.
2. **Каскад таймаутов SIGTERM → SIGKILL:** завершает всю группу процессов, предотвращая зависшие фоновые задачи и зомби.
3. **argv без шелл-парсинга:** инъекции конструктивно невозможны.
4. **Неблокирующий I/O:** раздельные потоки stdout/stderr, дедлоки TTY исключены; опция `pty` (настоящий терминал 200x50) для sudo/fzf/htop.

**CLI-уровень** (если сборка с фичей `pnd-ffi`; дефолтная сборка — без неё):

```bash
# ВАЖНО: все флаги ДО --exec; после --exec всё до конца строки — команда и её аргументы.
poler-engine --exec-timeout-ms 30000 --exec -- cargo build --release
# exit 124 = таймаут (конвенция GNU timeout), 127 = не найдено.
```

## 2. Нативный моторный мост S2→E2 (естественно-языковые директивы)

Моторный мост исполняет **директивы на русском/украинском** (whitelist-глаголы:
открой/відкрий, запусти/запусти, покажи/покажи, прочитай/прочитай, собери/збери):

```bash
# Правильный вызов: директива идёт в --triune-speak, НЕ позиционным аргументом!
poler-engine --triune-speak "покажи статус git" --motor-act --motor-yes --triune-json
```

- **R1 (ReadOnly)** — авто-исполнение: статус, логи, чтение файлов.
- **M2 (Mutating)** — по умолчанию подтверждение `[y/N]` (без tty — тихий отказ);
  `--motor-yes` — авто-режим (ответственность на операторе).
- Результат — чистый JSON в stdout (поле `motor_exec`), диагностика — в stderr.
- ⚠️ `--motor-act` без `--triune-speak`/`--triune-demo`/`--triune-auto-evolve`
  НЕ активирует контур — это флаг режима triune, а не самостоятельная команда.

## 3. Нативный поиск и сбор диска вместо медленных скриптов

**Точечный поиск (замена grep/rg):**

```bash
poler-engine <PATH> --grep "PATTERN" [--grep-list|--grep-count|--grep-json|--grep-i|--grep-regex]
```

**Сценовое чтение (замена sed/head — возвращает функцию/абзац целиком):**

```bash
poler-engine <PATH> -q "имя_функции" --format ai-json   # → anchors[].scene.enclosing_scope
```

**Полнодисковый сбор в один документ (poler_disk_harvester):**

```bash
# --harvest-query ОБЯЗАТЕЛЕН; корней может быть несколько:
poler-engine --harvest-disk /path/one /path/two \
  --harvest-query "термины для отбора" \
  --harvest-out RESULT.md            # форматы: markdown | json | corpus (t5c-корпус)
```

- DoD: 100 000 файлов < 2.5 с, RSS < 128 МБ (замер на живой машине:
  5 747 файлов / 1 459 каталогов / 3.27 с холодного обхода).
- SIMD Aho-Corasick, Zero-Copy memmap2 + MADV_SEQUENTIAL/DONTNEED,
  обход .gitignore/target/node_modules, NUL-сниффинг бинарников.
- Кириллица матчится во всех регистрах автоматически; термы через пробел = OR;
  `--harvest-regex` — regex-режим; exit-коды grep-совместимы (0/1/2).

## 4. MCP-инструменты для суверенных агентов (полный реестр)

`poler_search` · `poler_grep` · `poler_chunk` — локальный топографический поиск,
grep и RAG-чанкинг (в т.ч. по архивам без распаковки — `--archives`).

`poler_exec` · `poler_exec_async` · `poler_exec_task` · `poler_exec_kill` ·
`poler_exec_list` — нативное исполнение (см. раздел 1).

`poler_box_exec` · `poler_box_status` — Docker-песочница и её статус.

`query_poler_knowledge` — суверенный гиппокамп с верификацией MVR.

`poler_web_search` · `poler_crawl` · `poler_fetch` — офлайн-веб (индекс/краулер/CDP).

`poler_triune_speak` · `poler_triune_state` — триединое ядро (речь/состояние).

`poler_fly*` (node/edge/khop/path/common/centrality/rotor/motifs/propagate),
`poler_ssn_*` (step/inject/status/eject), `poler_literary_*` (field/step/generate/eject).

## 5. Правила эскалации (обязательны для агентов)

1. **Поиск по коду/файлам — ВСЕГДА сначала poler-engine** (`--grep` / `-q`), не Bash grep.
2. **Bash-инструмент упал или таймаутил 2+ раза подряд** → остаток сессии работать
   только через poler-engine (`--grep`, `--harvest-disk`, MCP `poler_exec`).
   Не ретраить Bash бесконечно — сообщить пользователю о рекомендации перезапуска.
3. **Долгие операции** (сборки, полнодисковые сканы) — `poler_exec_async` или
   `--harvest-disk`, никогда не через Bash с большим timeout.
4. **Мутации системы** — моторный мост M2 (подтверждение [y/N]) или `poler_box_exec`
   (Docker-судья), никогда не сырой `bash -c` из промпта.

---

## История версий

- **v1 (коммит 5f84a16):** черновик. Нерабочие вызовы: `--motor-act --motor-yes "команда"`
  (позиционный аргумент = PATH поиска, мотор не активируется без triune-режима);
  `poler_knowledge` (реальное имя — `query_poler_knowledge`); poler_box_exec описан
  как «прямой системный exec», хотя это Docker-песочница (прямой exec — `poler_exec`/`--exec`);
  заявка «100k файлов за ~1 с» (DoD: < 2.5 с).
- **v2 (этот документ):** все команды перепроверены по исходникам `src/mcp.rs`,
  `src/main.rs`, `src/triune/motor_bridge.rs` на коммите 5f84a16+.

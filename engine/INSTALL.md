# Установка и сборка POLER Engine v2.0

> Если что-то не собирается — раздел 8 «Частые проблемы» внизу.
> Карта документации: [docs/INDEX.md](docs/INDEX.md).

## 1. Требования

| Компонент | Минимум | Примечание |
|---|---|---|
| ОС | Linux x86_64 | AVX2 желателен ( fallback — скалярный путь в pqc/meta_compiler) |
| Rust | 1.98+ (`rustup update stable`) | edition 2021, rust-version 1.80 в манифесте — но собирайте свежим |
| RAM | 8 ГБ свободной для сборки | при < 8 ГБ — обязательно `-j1` (LTO fat + codegen-units=1 прожорливы) |
| Диск | ~4 ГБ | репо (с квантовыми крейтами внутри) + target/ |
| Python 3 | только для конвертеров моделей | stdlib (+ torch уже не нужен — конвертеры читают zip/safetensors напрямую) |

## 2. Сборка из одного репозитория (M2)

> **Вы ИИ-агент в песочнице с ограниченным диском?** Тебе сюда:
> **[docs/INSTALL-AGENT.md](docs/INSTALL-AGENT.md)** — установка конструктором
> (готовый бинарник 18 МБ из Releases / скил `--depth 1` / чтение кода из
> архива без распаковки). Раздел ниже — стандартная сборка на своей машине.

С фазы M2 (2026-09-16, docs/MERGE_PLAN.md) квантовые крейты `pqc`/`pqw`
живут прямо в репозитории (`crates/`, единый Cargo Workspace с сохраненной
историей RQ1–RQ23) — **никаких соседних клонов больше не нужно**:

```bash
git clone https://github.com/poler-engine-org/poler-engine.git
cd poler-engine
cargo build --release -j1        # -j1 при RAM < 8 ГБ; иначе можно без флага
./target/release/poler-engine --version
```

Внешних зависимостей у pqc/pqw — ноль, дерево движка не изменилось.
Старый репозиторий poler-engine-org/POLER-Quantum-RS больше не требуется
для сборки (история крейтов доступна через `git log <merge>^2`, провенанс —
`crates/README.md`).

## 3. Сборка

```bash
cd poler-engine
cargo build --release -j1        # -j1 при RAM < 8 ГБ; иначе можно без флага
./target/release/poler-engine --version
```

Первый warning-free билд — часть культуры проекта (CONTRIBUTING.md §2).
Бинарник статически несёт всё, кроме glibc; ~12–15 МБ.

### 3.1. Сборка без AVX2

Бинарник детектит AVX2 в рантайме (`is_x86_feature_detected!` в pqc и
meta_compiler) и падает на скалярный fallback — отдельной сборки не нужно.

### 3.2. Docker

С фазы M2 Docker-контекст самодостаточен (крейты внутри репо) —
трюк с копированием соседнего репозитория больше не нужен:

```bash
docker build -t poler-engine .
docker run --rm -v "$PWD:/data" poler-engine /data -q "запрос" --format ai-json
```

Dockerfile в корне репозитория. CI собирает образ на голом checkout
без секретов (см. .github/workflows/ci.yml).

## 4. Проверка установки

```bash
# 1. Автономный самотест формата весов (без моделей)
./target/release/poler-engine --pqw-selftest          # ожидание: 6/6

# 2. Полный тест-сьют (~1 845 тестов: движок + pqc + pqw, минуты)
cargo test --workspace

# 3. Живой поиск на тестовом корпусе
git clone --depth 1 https://github.com/Kotokvit/Eteryya.git ~/eteryya
./target/release/poler-engine ~/eteryya -q "Алексей" --format ai-json | head -50
```

Тесты на реальных моделях (tests/pqw_real_model.rs, tests/gliner_real_model.rs,
tests/safetensors_crystallize_bench.rs) само-скипаются без файлов моделей —
это норма (TESTING.md §1.3).

## 5. Модели (.pqw) — как получить

Формат описан в `docs/formats/PQW_FORMAT.md`. Готовые чекпойнты проекта
кладутся в `models/`:

```bash
mkdir -p models
```

### 5.1. BGE-M3 (эмбеддер, int8, ~573 МБ)

```bash
# скачать исходные веса HuggingFace (pytorch_model.bin) в cache/hf/…
python3 scripts/convert_hf_to_pqw.py --src <путь к pytorch_model.bin> \
    --dst models/bge-m3.pqw --quant int8
```

Конвертер потоковый: RAM не растёт с моделью (тот же паттерн — для 70B).

### 5.2. GLiNER (NER, int8, 297 МБ из 1.17 ГБ)

```bash
python3 scripts/convert_gliner_to_pqw.py --src <urchade_gliner_multi…> \
    --dst models/gliner.pqw
```

### 5.3. ChatGLM3-6B (декодер, int4, ~3.2 ГБ)

```bash
python3 scripts/convert_chatglm_to_pqw.py --src <chatglm3-6b/> \
    --dst models/chatglm3-6b.pqw --quant int4
```

Требует ~12–15 ГБ RAM на чтение fp16 и ~10 ГБ диска под поток; запускать
на машине с запасом (в 4-ГБ песочнице — OOM). Известный открытый вопрос:
int4-декодер воспроизводит 0 токенов (диагностируется, HISTORY.md).

### 5.4. Использование

```bash
./target/release/poler-engine ~/corpus --semantic dense --model models/bge-m3.pqw -q "запрос"
./target/release/poler-engine ~/corpus --ner gliner --model models/gliner.pqw --ner-labels "человек,место"
./target/release/poler-engine --llm local --model models/chatglm3-6b.pqw
```

## 6. Тестовый корпус

Роман «Eteryya» (65K+ файлов, 153 МБ): https://github.com/Kotokvit/Eteryya —
эталон полноты («Алексей» = 10 696 хитов, «Нокс» = 547) и одновременно
литературный канон проекта (docs/HISTORY.md, этап 0).

## 7. Интерфейсы после установки

```bash
poler-engine --shell      # REPL с Tab-completion
poler-engine --tui        # TUI: chat | notes | sources
poler-engine --gateway    # Terminal Gateway (sandbox-контур; см. docs/terminal-gateway-architecture.md)
poler-engine --mcp        # MCP-сервер stdio (для LLM-агентов)
```

## 8. Частые проблемы

| Симптом | Причина | Лечение |
|---|---|---|
| `error: failed to load manifest for pqc` | устаревший клон до M2 (path-депы на ../POLER-Quantum-RS_repo) | обновиться: `git pull` — с M2 крейты внутри репо |
| Сборка убита OOM-killer | LTO + параллельный codegen | `cargo build --release -j1` |
| `чужая магия: не PRBQ-хранилище` | файл не того формата | docs/formats/PRBQ_FORMAT.md |
| Тесты `pqw_real_model` skip | нет models/*.pqw | §5 (skip — норма) |
| Тест `glm_int4…` не запускается | помечен `#[ignore = KNOWN-BUG]` (int4-декодер, TESTING.md §4) | `cargo test -p poler-engine --lib glm_int4 -- --ignored` |
| rustc ругается на `#[inline(always)]` + `#[target_feature]` | rustc ≥ 1.87 запрещает комбинацию | не использовать их вместе (кодоген уже испускает `#[inline]`) |

## 9. Что удалено в v2.0 (чтобы не искать)

Google OAuth / NotebookLM / Gmail / Drive-импорт, `dev-stand/`, флаги
`--google-*`, `--nlm-*`, `--auth-ui`, `--license-import`. Суверенный стек:
всё локально. Старый мост — `docs/companion-bridge-design.md` (исторический
документ). EULA-статус: `--license`.

---
name: poler-engine
description: AI-Native суверенный поисково-аналитический движок POLER v0.36.0 (Rust + Zig 0.14.0). Резонансный поиск с ε-плотностью и K-hop графом, grep с гарантией полноты, скан архивов БЕЗ распаковки (zip/tar/tar.gz/tar.zst/gz/zst, пароли ZipCrypto/AES прямо в CLI), RAG-чанки с byte-якорями, крипто-память Vault .pvt (PND v8.2, CBC + KDF 100k + MAC), Суверенный Гиппокамп знаний с MVR-провенансом, нативный инференс .pqw (энкодеры XLM-R/BGE-M3, GLiNER, GLM-декодер int8/int4 — без Python/ONNX/GPU), ИДЕАЛЬНЫЙ ИСПОЛНИТЕЛЬ КОМАНД poler_exec E2 (ядро Zig raw-syscalls: таймауты TERM→grace→KILL группе, захват голова+маркер+хвост O(1), pidfd — зомби невозможны, PTY 200x50 для isatty-программ, cwd/env, PATH разрешает ребёнок, отмена cancel_flag за ≤25 мс, ФОНОВЫЕ задачи poler_exec_async/kill), CLI --exec и MCP-семейство из 5 инструментов с ПАРАЛЛЕЛЬНЫМ исполнением (пул воркеров), ЖИВАЯ МУХА C2 — коннектом FLYCSR1 (мозг мухи FlyWire v783: 138 639 нейронов, 54,5 млн синапсов) как резидентная матрица A в RAM: 10 MCP-инструментов poler_fly_* (паспорт нейрона, ребро/ротор J = A − Aᵀ, K-hop, кратчайший путь с цепочкой синапсов, общие партнёры, центральность/PageRank, глобальный топ циркуляции, мотивы u⇄v/feedforward/feedback, симуляция распространения сигнала x(t+1) = leak·x + γ·A·x) + 7 новых CLI-режимов; артефакт грузится один раз и живёт между вызовами, тяжёлые запросы параллелятся, РЕЗИДЕНТНЫЙ MCP-сервер для LLM-агентов (тёплые хэндлы в RAM: grep p50≈650 мкс, knowledge p99≈250 мкс; стриминг логов в зашифрованный Vault на лету). Математическая база — пятифазный когнитивный цикл ℘-O-L-ε-R[n]-Ψ и каноническое уравнение dp/dt = -η·Π_Λ[D·p + γJ·p + ∇F]. Использовать для: поиска по данным и архивам, RAG-подготовки, защиты памяти, базы знаний, семантического поиска, NER, НАДЁЖНОГО запуска команд с таймаутами/PTY/фоновым режимом и лимитом вывода, работы с сотнями документов (пересказ, противоречия, кластеризация, валидация), ГРАФОВЫХ ЗАПРОСОВ И СИМУЛЯЦИЙ НА БИОЛОГИЧЕСКОМ ЭТАЛОНЕ СВЯЗНОСТИ (нейроны, пути, хабы, динамика сигнала), ЛИТЕРАТУРНОГО ДВИГАТЕЛЯ POLER[Ψ] L1 (физика смысла: замысел → фазовая траектория → сверхпроводимость H^Ψ = 0 или предельный цикл, калиброванный ротором J = A − Aᵀ живого мозга; архетипы, причинный проектор, темпоральное эхо, призма No-Excuses, Trit5 No-Mul). СИНАПТИЧЕСКОГО ВИХРЯ SSN S1/v0.35.0 (живой мозг как субстрат управления: 226 уравнений и 109 алгоритмов извлечены из архивов, 67/67 проверок доказаны ДО реализации; полный стек — виртуальная топология (ноль RAM на граф, 100M синапсов < 1 ГБ), фазовые синапсы, латеральное торможение, STDP+DA, гомеостаз-интегратор, ретикулярный тон (анти-windup), E/I-контроллер, нейромодуляторы DA/5HT/NE, ритмы θ/γ; CSE-сенсорика с золотой фазой (разделимость 1.35); резидентные сессии мозга в MCP poler_ssn_*; 10 seed × 10k шагов устойчивости, ~4200 шагов/с). ТРИЕДИНАЯ АРХИТЕКТУРА S2/v0.36.0 (муха + синусоидный вихрь + троичный кристалл → ЖИВАЯ РЕЧЬ: пульс мухи — ротор касты 12×12 разводит лексику по архетипам, γ=0 зацикливает речь; вихрь дышит между токенами, медиаторы DA/5HT/NE задают температуру сэмплирования; кристалл знаний .t5c — Trit5 5 тритов/байт, No-Mul SIMD, зашит в бинарник, sha256; NMDA-гейт пропускает совпадающие смыслы; полный провенанс каждого токена sem/gate/syn/fly/tau; моторный слой S2 — только предложения poler_exec, деструктив отклоняется; ПОТОКОВОЕ КВАНТОВАНИЕ .t5q — 70B веса без сырого диска: пик RAM 64 КБ, 19× сжатие, 1.67 бита/вес, чанк-инвариантность доказана; ~96 токенов речи/с).
---

# POLER-Engine — суверенный гиппокамп, крипто-субстрат и нативный инференс для LLM-агентов

Поисково-аналитический, крипто-структурный и инференс-движок, спроектированный как
**инструмент для ИИ**: индексирует, находит, отдаёт, защищает память (.pvt Vault),
читает архивы без распаковки, режет RAG-чанки, эмбеддит и извлекает сущности нативно
на CPU. Не понимает контент, не генерирует текст, не принимает решений — понимает ИИ,
творит автор.

Репозиторий: https://github.com/poler-engine-org/poler-engine
Архитектура: `docs/UNIFIED_ARCHITECTURE.md` | Vault: `docs/formats/VAULT_FORMAT.md` |
.pqw: `docs/formats/PQW_FORMAT.md` | CLI: `docs/CLI.md` | **Математика: `MATH.md`** (рядом с этим файлом)

## Принципы (не нарушать)

1. **Инструмент, не ИИ** — индексировать/находить/отдавать/шифровать/связывать; не
   понимать, не генерировать, не решать.
2. **Суверенный стек** — никаких облачных API, нуль внешних ключей. Локальные CPU,
   mmap-структуры, чистый Rust + Zig C-ABI.
3. **Крипто-слой данных (CDL)** — PND v8.2 (256-бит, 32 раунда Feistel, GF(2⁸)),
   KDF 100k итераций, постраничный CBC (4096 Б), цепочечный MAC, внешний SHA-256.
4. **Архивы не распаковываются** — записи читаются напрямую в bounded-память
   виртуальными путями `архив::запись`; ни один байт не пишется на диск
   (zip-slip/бомбы/inode-кража исключены by design).
5. **Нативный инференс** — вес­а `.pqw` v2 (mmap zero-copy, SHA-256 при открытии),
   кернелы pqc (int8/int4/f32, LayerNorm/RoPE/SwiGLU/MoE). Не ONNX, не Python, не GPU.
6. **MVR (Mathematical Verification Record)** — каждое утверждение проверяемо:
   golden-векторы bit-for-bit, дифференциальные тесты против fp32-эталонов, Z3/SMT.
7. **Банальность — сила** — быстрый детерминированный доступ к данным, как grep/SQL/git.

## Сборка и тесты

```bash
cd poler-engine
# Крипто-мост: готовая Zig-библиотека (или zig в PATH / POLER_ZIG)
POLER_CORE_LIB=$PWD/os/core/zig-out/lib cargo build --release --features pnd-ffi
./target/release/poler-engine --version

# Тесты: 1238/1238 (pnd-ffi), default 1176/1176, zig 21/21 + 33/33, golden 54 626 bit-for-bit
cargo test --features pnd-ffi
(cd os/core && zig test poler_core.zig)
```

## Справочник режимов CLI

### 1. Точный grep (слой 0) — ВСЕ совпадения, гарантия полноты, exit-коды grep

```bash
poler-engine ~/corpus --grep "fn main" [--grep-regex] [--grep-i] [-A/-B N] [--grep-count]
poler-engine ~/corpus --grep "TODO" --grep-json      # byte offsets для агента
```

### 2. Идеальный исполнитель команд E2/v0.32.0 (фича pnd-ffi) — замена падающим Bash-инструментам

Рождён диагностикой исходников GNU bash 5.2 самим движком
(docs/EXEC_AUDIT.md: 424 unsafe-вызова в 75 .c-файлах, free() в trap-механике,
REINSTALL_SIGCHLD-гонка, неограниченный $(...), ноль таймаутов). Ядро —
Zig с raw-syscall слоем: классы bash-ошибок исключены конструктивно.

```bash
# флаги — ДО --exec; после --exec — команда целиком (включая её -флаги)
poler-engine --exec-timeout-ms 5000 --exec make -j4
poler-engine --exec-max-out 65536 --exec dd if=/dev/zero bs=1M count=64
poler-engine --exec-stdin "вопрос" --exec cat
# E2: рабочий каталог, окружение, терминал, голова+хвост
poler-engine --exec-cwd /var/log --exec ls -la
poler-engine --exec-env LC_ALL=C --exec-env DEBUG=1 --exec sort file.txt
poler-engine --exec-pty --exec sudo -n apt-get update      # isatty-программы
poler-engine --exec-capture head_tail --exec-max-out 4096 --exec make -j8  # и голова, и хвост лога
# коды: ребёнка | 124 таймаут | 127 не найдено | 126 без права | 125 плохой cwd
```

Гарантии: таймаут timerfd(MONOTONIC) + SIGTERM → grace → SIGKILL группе;
захват — O(1) памяти (tail: хвост max_out; head_tail: первые B/2 + маркер
«dropped N» + последние B/2 — стек-трейс в начале гигантского лога
не теряется); зомби невозможны (pidfd + wait4 в ppoll-цикле); шелл-инъекции
невозможны (argv массивом, без парсинга); fds-гигиена CLOEXEC; PTY —
настоящий терминал 200x50 (stdout/stderr слиты); PATH разрешает сам ребёнок
(ноль stat в родителе); отмена — атомарный флаг, TERM→KILL за ≤25 мс.

### 3. Архивы без распаковки (v0.28.1) — виртуальные файлы «архив::запись»

```bash
poler-engine ~/corpus --grep "Subquantum" --archives # zip/tar/tar.gz/tar.zst/gz/zst in-place
poler-engine --archive-list export.zip               # листинг БЕЗ пароля и без распаковки
poler-engine --archive-list export.zip --archive-json # JSON: имена/размеры/шифрование
poler-engine "export.zip::docs/paper.md" --chunk     # RAG-чанки записи архива
poler-engine ~/corpus --grep "secret" --archives --archive-password "PASS"  # пароль в CLI
env POLER_ARCHIVE_KEY="PASS" poler-engine ~/corpus --grep "secret" --archives
poler-engine ~/corpus --grep "x" --archives --archive-max-entry-mb 128     # лимит бомб
```

### 4. RAG-чанки (слой B) — секция→абзац→предложение, byte-якоря + breadcrumb

```bash
poler-engine document.md --chunk [--chunk-size 384] [--chunk-overlap N] [--chunk-json]
poler-engine "dump.zip::17-Active_inference.md" --chunk --chunk-json  # запись архива
```

### 5. Резонансный POLER-поиск — ε-плотность, IIR-резонанс R(t), сцены, K-hop граф

```bash
poler-engine ~/corpus -q "Алексей" --format ai-json    # машинный JSON с ε и R(t)
poler-engine ~/corpus -q "Алексей" --resonance-mode psi|poler|field|hits
poler-engine ~/corpus -q "тема" -k 4                   # K-hop подграф связей
# Гиперпараметры канона — прямо в CLI (см. MATH.md):
#   --psi-eta 0.05 --psi-gamma 0.5 --psi-rho 0.9 --psi-depth 8
#   --poler-eta 0.01 --poler-gamma 0.1 --poler-mix 0.1 --poler-dissipator 0.02
```

### 6. Нативный суверенный ML (.pqw, без Python/ONNX/GPU)

```bash
poler-engine --pqw-selftest                        # автономный цикл 6/6 (энкодер→GLiNER→GLM)
poler-engine doc.md -q "POLER" --semantic dense --model models/encoder.pqw
poler-engine doc.md -q "запрос" --semantic dense --model models/bge-m3.pqw \
    --semantic-corpus ~/corpus --semantic-limit 10 # живой поиск: чанки→эмбеддинги→косинус
poler-engine doc.md -q "..." --ner gliner --model models/gliner.pqw \
    --ner-labels "ORGANIZATION,PERSON,CONCEPT"      # zero-shot NER
poler-engine doc.md -q "..." --llm local --model models/glm.pqw  # GLM-декодер

# Конвертеры реальных весов (стримингово, RAM не растёт с моделью):
python3 scripts/convert_hf_to_pqw.py     # torch-zip → .pqw int8/int4 (XLM-R/BGE-M3)
python3 scripts/convert_gliner_to_pqw.py # GLiNER (mdeberta-спина) → .pqw
python3 scripts/convert_chatglm_to_pqw.py# ChatGLM3-6B → .pqw int4
python3 scripts/gen_demo_pqw.py          # демо-модели для смоук-тестов
```

### 7. Защищённая память Vault (.pvt CDL) — «насмерть», git-переносимая

```bash
export POLER_VAULT_KEY="парольная_фраза"            # или ввод с stdin/TTY
poler-engine --memory-seal <ФАЙЛ> [--memory-out OUT.pvt] [--memory-content-id ID]
poler-engine --memory-open <ФАЙЛ.pvt> [--memory-out OUT.bin]
poler-engine --memory-verify <ФАЙЛ.pvt>             # внешний SHA-256 БЕЗ ключа
poler-engine --memory-info <ФАЙЛ.pvt>               # страницы, соль, KDF-итерации
# Ключ через env: --memory-key-env ИМЯ_ПЕРЕМЕННОЙ; KDF: --memory-kdf-iters N
```

### 8. Суверенный Гиппокамп — база знаний с эпистемической градацией

```bash
poler-engine --knowledge-ingest ~/library/          # 6 слоёв, FNV-дедуп, MVR-разметка
poler-engine --knowledge-search "запрос" [--min-provenance mvr|source|narrative]
poler-engine --knowledge-stats                      # источники/чанки/токены/провенанс
# Эмбеддер: --knowledge-embedder none|hash|pqw (pqw: env POLER_KNOWLEDGE_MODEL=path.pqw)
# БД по умолчанию knowledge.db (+ .graph/.vectors спутники); --knowledge-db PATH
```

### 9. AIDDE impact-анализ — call graph + паспорт символа

```bash
poler-engine ./src --impact "run_gateway" [--impact-depth 3] [--impact-cache db]
```

### 10. Коннектом FLYCSR1 (C2/v0.33.0 «Живая муха») — мозг мухи как матрица A

```bash
poler-engine --connectome docs/flywire-connectome/flywire_v783_core.csr.zst
poler-engine --connectome ...core.csr.zst --connectome-nodes ...nodes.bin \
  --connectome-node 0                # паспорт нейрона (индекс или root_id)
poler-engine --connectome ... --connectome-edge 0:6135       # вес/медиатор/знак + J = A − Aᵀ
poler-engine --connectome ... --connectome-khop 0 -k 2      # BFS потока сигнала: [13, 443], 457
poler-engine --connectome ... --connectome-impact 0         # CSC «кто управляет нейроном»
# C2/v0.33.0 — глубокое взаимодействие:
poler-engine --connectome ... --connectome-neighbors 79529 --connectome-dir in   # партнёры по весу
poler-engine --connectome ... --connectome-path 0:116214   # кратчайший путь с цепочкой синапсов
poler-engine --connectome ... --connectome-common 0,6135 --connectome-dir down # общие мишени/источники
poler-engine --connectome ... --connectome-centrality --connectome-top 5        # хабы + PageRank
poler-engine --connectome ... --connectome-rotor-top 5 --connectome-min-abs 50 # топ циркуляции J
poler-engine --connectome ... --connectome-motifs 79529     # реципрокные/ff/fb мотивы
poler-engine --connectome ... --connectome-propagate 0 --connectome-steps 3    # симуляция сигнала
poler-engine --connectome ... --connectome-json            # JSON для агентов
```

- Знак связи: **+1** ach/glut, **−1** gaba, **0** модуляторы (oct/ser/da);
  фильтр потока: `--connectome-sign all|exc|inh`. Exit-коды grep: 0 найдено / 1 нет / 2 ошибка.
- Артефакты в git (~43 МБ): full 35 МБ / core 7 МБ / nodes 1.1 МБ; загрузка
  core ≈ 70 мс (≈ 25 МБ RAM), full ≈ 400 мс; сырьё 852 МБ feather не нужно.
- **MCP — основной путь для агентов**: резидентная муха живёт в RAM между
  вызовами (`poler_fly` грузит артефакт один раз, дальше все запросы без
  диска; CSC строится лениво один раз). Клиент каждый CLI-вызов платил
  60–400 мс загрузки + 43 мс CSC — MCP-сервер платит это ровно один раз.
- Золотые числа ядра: топ циркуляции J[74067][133436] = **+2395**; хаб
  79529 (6399 исх. / 5080 вх., 3684 реципрокных пары); PageRank-топ 66912;
  симуляция от 0: активные [14, 455, 15 457] за 3 шага, торможение доминирует.

### 11. Веб (локальный индекс, не облако)

```bash
poler-engine https://example.com --crawl --crawl-depth 2 --crawl-max 25
poler-engine --web-search "запрос"                  # Semantic Bridge ru↔en офлайн
poler-engine --semantic-expand "запрос"             # диагностика моста
```

### 12. Интерфейсы и сервисы

```bash
poler-engine --mcp                                   # MCP-сервер (stdio JSON-RPC)
poler-engine --mcp-http 127.0.0.1:8765 --mcp-token X # MCP через HTTP (Bearer)
poler-engine --shell | --tui | --gateway | --license
poler-engine --benchmark [--benchmark-json r.json]   # Exact/Lexical/Passage/latency/RAM
```

## Когнитивный цикл работы с документами (глаза для модели)

Как читать сотни документов «как человек», а не автоматом: движок даёт якоря и
извлечение, агент — понимание. Фазы ℘-O-L-ε-R[n]-Ψ (полная математика — `MATH.md`):

| Фаза | Смысл | Команда движка |
|------|-------|----------------|
| ℘ Перцепция | осмотр без искажения | `--archive-list --archive-json` (что внутри, до пароля) |
| O Образ | сущности и структура | `--chunk --chunk-json` (byte-якоря, breadcrumb); `--ner gliner` |
| L Логика | противоречия | `--grep --archives` по утверждению → кросс-док сравнение цитат |
| ε Энергия | плотность смысла | `-q --format ai-json` (ε-ранжирование сцен) |
| R[n] Резонанс | темпоральное эхо | итеративный `-q` + `-k` K-hop подграф (накопление контекста) |
| Ψ Аттрактор | верифицированный итог | `--knowledge-search --min-provenance mvr` → MVR-паспорт |

Четыре операции над корпусом (протестированы на 300-документном архиве 32 МБ):

1. **Точный пересказ** — `--chunk` режет с координатами (путь, строки, byte_start/byte_end);
   каждый тезис привязан к строке исходника, галлюцинации исключены конструктивно.
2. **Поиск противоречий** — `--grep --archives "формула/утверждение"` собирает ВСЕ контексты;
   агент сопоставляет и фиксирует расхождения с двумя точными цитатами.
3. **Классификация по темам** — `--ner gliner` извлекает [ТЕОРИЯ]/[АВТОР]/[МЕТОД]/[ИНВАРИАНТ],
   пересечение сущностей + K-hop граф дают кластеры документов.
4. **Паспорт валидации** — градус провенанса каждого тезиса: **MVR** (доказано кодом/Z3) >
   **Source** (рецензируемый источник) > **Narrative** (гипотеза/заметка).

## Архивы: правила для агента

- **Форматы:** zip (stored/deflate/zstd; ZipCrypto и AES-256), tar, tar.gz, tar.zst,
  gz (мульти-член), zst; контейнеры jar/war/epub/odt. bzip2/xz нет (суверенный минимум).
- **Пароль** — цепочка: `--archive-password` → env `POLER_ARCHIVE_KEY` → TTY-промпт.
  Неверный пароль = ошибка в stderr на запись, остальной корпус сканируется.
- **Сначала осмотр** (`--archive-list` без пароля), потом вскрытие.
- **Бомба-гард:** распакованная запись > 64 МиБ (`--archive-max-entry-mb`) пропускается.
- **Селектор `архив::запись`** работает с `--chunk`. Нужен «файл» из архива — используй `::`,
  никогда не unzip. Вложенные архивы (zip в tar) v1 не раскрываются.
- **На диск не пишется ничего** — верифицировано тестами.

## Vault: правила для агента

- Ключ: env `POLER_VAULT_KEY` → stdin (пустой = отказ). Вывод существует → отказ с подсказкой.
- `.pvt` можно хранить в git/переносить: `--memory-verify` проверяет целостность БЕЗ ключа
  (внешний SHA-256), tamper 1 бит ловится. Полный доступ — только с ключом.
- Формат: страницы 4096 Б CBC (IV из соли+номера), цепочечный MAC листьев, KDF 100k.
- Логи/диалоги агента запечатываются в `.pvt` и гоняются через любой git.

### 11. Литературный Двигатель POLER[Ψ] (L1/v0.34.0) — физика смысла, калиброванная мухой

Полная матричная форма канонического уравнения: замысел → инвариантный вектор
Ω(o) (FNV-хэш термов, ноль RNG, ‖Ω‖ = 1) → фазовая динамика

```
p_{t+1} = p_t − η·Π_Λ(∇F + D·p + γJ·p) + η_r·Π_Λ(κ·(echo − p))
```

где Π_Λ — проектор причинности (галлюцинации аннигилируются математически),
echo = Σ 0.9ᵏ·p_{t−k} — темпоральное эхо (бесконечный контекст через
интеграл состояний), **J = A − Aᵀ — ротор живого мозга мухи** (циркуляция
смыслов: каста нейронов от семян, якорь = FNV(root_id) mod 12 архетипов),
**D = L·Lᵀ — метрика Ляпунова на графе касты** (диссипация пертурбаций).
H^Ψ → 0 = Observer-Kill = «информационная сверхпроводимость».

```bash
# Анализ поля: термы-инварианты + 12 архетипов (Кэмпбелл + канон POLER)
poler-engine --literary-field "герой идёт в поход против тьмы и бездны"

# Генерация: чистая физика текста — СХОДИМОСТЬ (F → 1e-5, сверхпроводимость)
poler-engine --literary-generate "текст замысла" [--literary-steps 128]

# Генерация с мушиной калибровкой — ПРЕДЕЛЬНЫЙ ЦИКЛ (живой мозг не даёт
# нарративу замереть) + драматургические пары = роторные пары нейронов
poler-engine --literary-generate "текст" --literary-csr .../flywire_v783_core.csr.zst \
  --literary-seeds 0,116214 --literary-max-cast 32 [--literary-no-mul] [--literary-json]
```

Отчёт: три акта с архетипическими доминантами, конфликты (кто доминирует
над кем, с силой J), инженерная телеметрия F / ε / Σ(t) / H^Ψ / причинность.
Принцип «No Excuses»: при семантическом тупике (F растёт 3 шага) энергия
смысла ПРЕЛОМЛЯЕТСЯ призмой в ближайший архетип с точным сохранением нормы —
отказа от генерации не существует. Trit5 No-Mul: p_t квантуется в {−1,0,+1},
резонансные скалярные произведения — AVX2 без f32-умножений.
Руководство агента: `docs/LITERARY.md`.

### 12. Синаптический Вихрь SSN (S1/v0.35.0) — живой мозг, доказанный до реализации

Субстрат управления: текст → CSE-сенсорика → живая нейродинамика → readout.
Извлечено из архивов пользователя (226 уравнений, 109 алгоритмов),
доказано численно (67/67 проверок, 13 найденных и исправленных режимов
отказа F1–F13), затем портировано в Rust. **Ни одна строка ядра не
написана до доказательства.**

Полный стек шага: виртуальная топология `tgt = (i·39293 + f·29101 +
seed·73471) mod N` (связь = арифметика, ноль RAM на граф), фазовые
синапсы `cos(phase + 0.3·θ_mod)`, латеральное торможение («сосед
кричит — ты молчишь»), STDP сквозь дофаминовые ворота, гомеостаз по
медленному следу f_sys (интегральный контроллер, знак F2 + след F8),
ретикулярный тон b_tone (анти-windup F13 — второй интегральный канал),
E/I-контроллер с мёртвой зоной [2,6], нейромодуляторы DA/5HT/NE,
ритмы θ (5 рад/с) / γ (40 рад/с). Здоровье: активность ~5% с лавинными
флуктуациями (критичность, край хаоса), S < 0.8, E/I ≈ 4.

```bash
# живой мозг: 10k шагов, телеметрия (акт/f_sys/тон/S/C/E/I/медиаторы)
poler-engine --ssn-demo [--ssn-n 10000] [--ssn-seed 42] [--ssn-json]

# CSE-сенсорика: разделимость текстов (золотая фаза, зазор 1.35)
poler-engine --ssn-encode "герой идёт в поход" --ssn-encode-b "кофеварка сломалась"

# сенсорная инъекция: текст → мозг → динамика → readout
poler-engine --ssn-inject "открыть терминал и собрать проект" --ssn-steps 5000
```

MCP-семейство `poler_ssn_*` — резидентные живые мозги (LRU 8):
`poler_ssn_step` (создать/шагнуть: seed/n/fields/dims/steps),
`poler_ssn_inject` (text → CSE → активации), `poler_ssn_status`
(снимок: телеметрия + readout топ-K), `poler_ssn_eject`.
Производительность: ~4200 шагов/с (600 нейронов × 16 полей, release).
Доказательства: `proofs/ssn_verify3.py` (67/67), `proofs/ssn_f13_proof.py`
(10 seed × 10k). Руководство агента: `docs/SSN.md`.

### 13. Триединая Архитектура (S2/v0.36.0) — муха + вихрь + кристалл → речь

Три доказанные опоры сходятся в одном цикле порождения слова.
**Муха** (FlyPulse): ротор касты FLYCSR1 агрегируется в антисимметричную
матрицу 12×12 по якорям архетипов (Jᵀ = −J, метрика Ляпунова D гасит);
фаза p ← p + dt(γJ·p − D·p), дрейф tanh((γJ·p)[архетип токена]) разводит
лексику — при γ = 0 речь вырождается в циклы (доказано усреднением по
8 seed). Без артефакта коннектома — честная виртуальная муха из seed
(происхождение всегда сообщается агенту). **Вихрь** (SSN): между токенами
мозг дышит steps_per_token шагов; медиаторы задают температуру речи
τ = base·(1 + 0.6·DA − 0.5·5HT − 0.3·NE), зажата [0.6, 1.8]. **Кристалл**
(.t5c): словарь + биграммная топология (знаковая квантизация PMI:
трит = +1 при r ≥ 1.7, −1 при r ≤ 0.5), семантика выводится из золотой
фазы CSE и квантуется в триты — сенсорика и память делят один код.
Счёт кандидата: w_sem·sem·NMDA-гейт + w_syn·биграммный трит +
w_fly·дрейф − штраф повтора → WTA(8) → softmax(τ) → слово; эхо речи
возвращается в мозг (замкнутый контур, громкость 0.25).

```bash
# живая речь от промпта (виртуальная муха; ~96 токенов/с)
poler-engine --triune-speak "живой мозг говорит" --triune-tokens 32

# НАСТОЯЩИЙ мозг мухи как пульс речи
poler-engine --triune-speak "система слушает" --triune-connectome flycsr1.csr.zst --triune-seeds 1000,5000,9000

# витрина: 3 фразы + телеметрия + моторные интенты
poler-engine --triune-demo --triune-json

# собрать свой кристалл из корпуса (детерминизм: тот же корпус → те же байты)
poler-engine --crystal-build corpus.txt --crystal-out my.t5c

# ПОТОКОВОЕ КВАНТОВАНИЕ: 140 ГБ FP16-весов → .t5q без сырого диска,
# пик RAM 64 КБ, чанки чтения не влияют на результат
poler-engine --stream-quant weights.f32.bin --stream-quant-out w.t5q
curl -sL <url> | poler-engine --stream-quant - --stream-quant-f16 --stream-quant-keep 0.1
```

Моторный слой S2: скан речи на повелительные конструкции → интенты
`poler_exec open/run/show/read/build -- объект` — ТОЛЬКО предложения,
исполнение остаётся за агентом; деструктивная лексика (удалить/стереть/…)
отклоняется белым списком. MCP-семейство `poler_triune_speak`
(родить/продолжить разговор: text/seed/gamma/tokens → речь + трейс
провенанса + интенты) и `poler_triune_state` (снимок: последняя фраза,
токены, телеметрия мозга). Честная математика 70B: плотный Trit5 =
14.6 ГБ (гарантированный этаж), keep=0.1 рождает 90% вакуума —
подготовлено к sparse-загрузчику следующего поколения (~2 ГБ).
Форматы: .t5c (кристалл, sha256, детерминированная сборка) и .t5q
(поток, заголовок 72 Б + блоки [f32 scale][103 Б тритов] + трейлер,
sha256, без перемотки — дружит с pipe).

## MCP-инструменты (для LLM-агентов)

`poler_web_search`, `poler_crawl`, `poler_fetch`, `poler_search`,
`poler_grep` (аргументы `archives: true`, `archive_password: "…"`),
`poler_chunk`, `poler_box_exec`, `poler_box_status`,
`poler_exec` (E2: идеальный запуск команд — command/args/timeout_ms/
grace_ms/max_out_bytes/stdin/cwd/env/pty/capture → JSON exit_code/signal/
stdout/stderr/timed_out/cancelled/truncated/duration_us/pid; ядро Zig
raw-syscalls; умолчание capture=head_tail),
`poler_exec_async` (фоновый запуск → task_id мгновенно),
`poler_exec_task` (опрос/ожидание: task_id + wait_ms),
`poler_exec_kill` (отмена: TERM→KILL за ≤25 мс),
`poler_exec_list` (обзор фоновых задач),
`poler_knowledge` (query, top, min_provenance).

**C2/v0.33.0 «Живая муха» — 10 инструментов:** `poler_fly` (загрузка/сводка/
eject коннектома в RAM), `poler_fly_node` (паспорт), `poler_fly_edge`
(ребро + ротор J), `poler_fly_khop` (BFS фронтов), `poler_fly_path`
(кратчайший путь с цепочкой синапсов), `poler_fly_common` (общие мишени/
источники набора), `poler_fly_centrality` (хабы + PageRank), `poler_fly_rotor`
(глобальный топ циркуляции |J|), `poler_fly_motifs` (реципрокные пары,
feedforward, feedback), `poler_fly_propagate` (симуляция сигнала
x(t+1) = leak·x + γ·A·x на знаковых весах). Нейроны задаются индексом или
root_id; первый вызов грузит артефакт (csr + опционально nodes), дальше —
без диска. Семейство исполняется в пуле воркеров: пачка запросов к мухе
параллелится (11-запросный конвейер release-смоука — 0.49 с суммарно).

**L1/v0.34.0 POLER[Ψ] — 4 инструмента:** `poler_literary_field` (анализ
поля интенции: термы + архетипы + опциональная каста мухи), `poler_literary_step`
(резидентный полигон: text создаёт сессию, session шагает p_t → p_{t+1},
observation эволюционирует замысел; LRU 8 сессий в RAM), `poler_literary_generate`
(полный прогон: акты + драматургические пары + телеметрия + текст-разметка),
`poler_literary_eject` (выгрузка сессий). Мушиная калибровка переиспользует
тёплый коннектом WarmFly (csr один раз, каста от seeds).

**S2/v0.36.0 Триединство — 2 инструмента:** `poler_triune_speak` (без session
— рождает говорящее Триединство из seed/gamma; с session — продолжает
разговор: text/tokens → речь + трейс провенанса sem/gate/syn/fly/tau +
моторные интенты; мозг, муха и контекст живут между вызовами, LRU 4) и
`poler_triune_state` (снимок без шага: последняя фраза, токены,
телеметрия живого мозга). Кристалл — зашитый в бинарник .t5c;
моторные интенты — ТОЛЬКО предложения poler_exec, исполнение за агентом.

**E2: параллельное исполнение.** stdio-сервер исполняет exec-инструменты
в пуле воркеров (2–8 потоков): агент может отправлять N запросов подряд —
они выполняются ПАРАЛЛЕЛЬНО, ответы приходят по готовности (JSON-RPC
сопоставляет по id). Пример: два `sleep 1` параллельно = 1.0 с стеновых,
а не 2 с. C2 расширяет пул на всё семейство poler_fly_*.

### M6: резидентное состояние (real-time, без холодного старта)

Сервер держит в RAM между запросами: тёплый Гиппокамп (SQLite/RaBitQ/HNSW/
эмбеддер — открыты один раз), LRU-кэш файлов для grep (инвалидация mtime+len)
и WebIndex. Каждый dispatch замеряется и (опционально) стримится в
зашифрованный журнал.

```bash
poler-engine --mcp --vault-log session.pvt          # + стриминг логов вызовов в Vault .pvt
POLER_VAULT_PASS="фраза" poler-engine --mcp --vault-log session.pvt  # пароль через env
poler-engine --mcp --mcp-ram-budget 64               # бюджет RAM-кэша, МиБ (по умолч. 48)
poler-engine --mcp-bench 500                          # бенчмарк резидентности: cold vs warm p50/p95/p99
```

- Формат журнала в `.pvt`: JSON-строки `{"ts","method","tool","us","ok"}` —
  файл **валиден на каждом коммите**, читается штатным `--memory-open/verify`.
- `--mcp-bench` — приёмка M6: тёплые p99 < 5 мс (exit 0/1); на release-сборке
  grep p50≈650 мкс / p99≤2 мс, knowledge p99≈250 мкс.

## Верификационные числа (золотой стандарт)

- Rust: **1302/1302** тестов (pnd-ffi; S2: +34 triune-семейство), default **1240/1240** (lib 1185 + интеграция/gateway/модели 55); Zig: **21/21** exec + 33/33 крипто/ABI; golden-векторы PND: **54 626 bit-for-bit**.
- SSN S1 (доказательство до реализации): Python-пруф **67/67** проверок; F13-пруф **10/10** seed × 10k шагов (активность 4.5–5.2%, w@клип ≤ 1.9%); Rust full-fidelity **10/10** seed × 10k; CSE-паритет с Python побитовый (cos = 0.9987 / −0.3547); Кэли-дрейф 3.7e-14 за 2000 шагов.
- Триединство S2 (доказано до реализации): детерминизм речи (seed → те же слова); вихрь жив во время речи (активность 6–10%, S < 0.8); **муха ломает циклы** (γ=0.8 vs γ=0, усреднение 8 seed × 40 токенов, анти-луп отключён для изоляции вклада); температура зажата [0.6, 1.8] при любых медиаторах; NMDA-гейт пропускает совпадающие смыслы; мотор отклоняет деструктив; .t5c раунд-трип побитовый + sha256 ловит порчу; **чанк-инвариантность потокового квантования** (чанки 1 Б … 64 КБ → идентичные байты); пик RAM ≤ 64 КБ при 200 КБ входа; сжатие 19.1× (1.673 бита/вес); keep=0.1 → 89.8% вакуума; f16-конверсия против эталонных битовых паттернов.
- Муха C2 (ядро v783, артефакты из git): ротор-топ J[74067][133436] = +2395,
  J[112021][86059] = +2343, J[129880][136978] = +2330; хаб 79529 — 6399 исх. /
  5080 вх.; PageRank-топ 66912 (ранг 0.001836547); мотивы 79529 — 3684
  реципрокных / 11 999 ff / 8082 fb; симуляция от 0 — [14, 455, 15 457]
  активных, |торможение| > возбуждение на шаге 3; путь 0→116214 — w17 ach.
- pqw/pqc: дифференциалы против fp32-эталонов — int8 cos > 0.999, int4 > 0.98, fp32 > 0.9999.
- Токенизатор XLM-R: 40/40 текстов побитово = HF `tokenizers` v0.23.2.
- BGE-M3 int8 (573 МБ): cos ≥ 0.9999 на всех 24 слоях. GLiNER (urchade/gliner_multi):
  7/7 сущностей, скоры в допуске 0.03.
- GLM-декодер: greedy ~1450 ток/с, MoE int4 ~840 ток/с (2 ядра CPU).
- Vault: 33 МБ за 5.4 с (KDF 100k = 0.62 с фиксированно); tamper-детект побитовый.

## Связанные навыки

- `poler-causal-operator` — операционный манифест ℘-O-L-ε-R[n]-Ψ (как ДУМАТЬ в парадигме).
- `document-translator` — перевод техдоков с защитой формул LaTeX и Mermaid.
- Тестовый корпус: роман «Eteryya» https://github.com/Kotokvit/Eteryya

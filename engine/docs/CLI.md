# CLI-референс POLER Engine

> Источник истины — `src/main.rs` (clap derive): `--help` всегда актуален.
> Этот документ — сгруппированная аннотация всех режимов и флагов v2.0
> (~85 флагов). Карта всех документов — [INDEX.md](INDEX.md).

Общий скелет: `poler-engine [PATH] [РЕЖИМ] [ФЛАГИ]`. `PATH` — файл, каталог
или репозиторий; в веб-режимах — URL. Большинство флагов-режимов
конфликтуют друг с другом (clap сам разрулит).

---

## 1. Резонансный поиск (основной режим)

```bash
poler-engine ~/corpus -q "Алексей" --format ai-json
poler-engine ~/corpus -q "Нокс" --resonance-mode poler --kappa 0.9
```

| Флаг | Тип | Значение |
|---|---|---|
| `-q, --query` | текст | запрос (взвешенные термы) |
| `-t, --top` | usize | сколько результатов |
| `-w, --window` | usize | окно контекста в сценах |
| `--k-hop` | usize | глубина K-hop по графу сущностей |
| `--metric` | строка | метрика сортировки |
| `--format` | enum | `ai-json` \| `md` \| `simple` |
| `--pii` | enum | `off` \| `mask` — zero-copy очистка персональных данных |
| `--resonance-mode` | enum | `hits` \| `field` \| `psi` \| `poler` (THEORY.md §3.2) |
| `--phi` | f32 | показатель объёма в ε-плотности |
| `--kappa` | f32 | удержание IIR-резонанса R(t) |
| `--local-stats` | flag | считать статистики только по локальному корпусу |
| `--extensions` | список | фильтр расширений файлов |
| `--max-file-mb` | u64 | потолок размера файла |
| `--max-scope` / `--max-relations` / `--max-graph-triples` | usize | лимиты парсера/графа |
| `--hidden` | flag | включить скрытые файлы |

Параметры циклов: `--psi-eta/--psi-gamma/--psi-rho/--psi-depth` (Ψ-поле),
`--poler-eta/--poler-gamma/--poler-mix/--poler-dissipator` (POLER-цикл).

## 2. Watcher

```bash
poler-engine ~/corpus -q "деплой" --watch --interval-secs 30 --diff
```

`--watch` — инкрементальный перепрогон при изменениях (WatchState:
VocabArena + PostingsStore переиспользуются); `--interval-secs`; `--diff`
— только изменения с прошлого прогона.

## 3. Точный поиск — слой 0

```bash
poler-engine src --grep "fn main" -A 3 --grep-json
poler-engine . --grep "TODO" --grep-regex --grep-i --grep-count
```

| Флаг | Аналог grep | Смысл |
|---|---|---|
| `--grep PATTERN` | — | ВСЕ совпадения, гарантия полноты, без индекса |
| `--grep-regex` | -E | регулярное выражение |
| `--grep-i` | -i | регистронезависимость (Unicode-fold) |
| `--grep-after N` / `--grep-before N` | -A / -B | контекст |
| `--grep-count` | -c | счётчик на файл |
| `--grep-list` / `--grep-list-nonmatching` | -l / -L | пути с/без совпадений |
| `--grep-max-count N` | -m | потолок совпадений |
| `--grep-hidden` | — | включая скрытые |
| `--grep-json` | — | byte-offsets для агента |

Exit-коды как у grep: 0 — найдено, 1 — пусто, 2 — ошибка.

### 3.1. Архивы без распаковки (v0.28.1)

Записи zip / tar / tar.gz / tar.zst / gz / zst внутри PATH читаются
**напрямую из контейнера в память** — виртуальными файлами
`архив::запись`. Ни один байт не распаковывается на диск (zip-slip,
бомбы и краденые inode исключены by design; проверено `find -newer`).

```bash
poler-engine ~/corpus --grep "Subquantum Kinetics" --archives
# совпадения: ~/corpus/export.zip::docs/paper.md:12:…

# Листинг контейнера без пароля (осмотр перед вскрытием)
poler-engine --archive-list export.zip
poler-engine --archive-list export.zip --archive-json   # для агента

# RAG-чанки конкретной записи — селектор «архив::запись»
poler-engine "export.zip::docs/paper.md" --chunk --chunk-json
```

| Флаг | Смысл |
|---|---|
| `--archives` | с `--grep`: сканировать недра архивов (параллельно с плоскими файлами) |
| `--archive-password PASS` | пароль ZipCrypto/AES-архива — прямо в CLI (виден в истории shell) |
| env `POLER_ARCHIVE_KEY` | то же, но не попадает в историю shell |
| (TTY-промпт) | интерактивный терминал спросит пароль сам, если нашёл шифрованный архив |
| `--archive-max-entry-mb N` | лимит несжатой записи [64] — защита от zip-бомб |
| `--archive-list ARCHIVE` | листинг записей (имя/размеры/шифрование), пароль не нужен |
| `--archive-json` | JSON-форма листинга |

Форматы: zip (stored/deflate/zstd; шифрование ZipCrypto и AES-256),
tar, tar.gz, tar.zst, gz (мульти-член — ротация логов), zst, а также
zip-контейнеры приложений (jar/war/epub/odt). bzip2/xz — вне
суверенного минимума зависимостей. Локальное основание vision-дока:
`docs/future-streaming-archives.md`.

MCP: инструмент `poler_grep` принимает аргументы `archives: true` и
`archive_password: "…"` — агент сканирует недра архивов через MCP.

## 4. RAG-чанки — слой B

```bash
poler-engine document.md --chunk --chunk-size 384 --chunk-json
# v0.28.1: запись архива без распаковки — селектор «архив::запись»
poler-engine "export.zip::docs/paper.md" --chunk --chunk-json
```

`--chunk` (секция→абзац→предложение, целостность предложений),
`--chunk-size`, `--chunk-overlap`, `--chunk-json` (byte-range якоря).
Формат записи определяется по её расширению (не по расширению архива).

## 5. Суверенный ML (v2.0)

```bash
# самотест формата весов
poler-engine --pqw-selftest

# плотный семантический поиск: нативные эмбеддинги BGE-M3 из .pqw
poler-engine ~/corpus --semantic dense --model models/bge-m3.pqw -q "запрос"
poler-engine ~/corpus --semantic dense --model models/bge-m3.pqw \
    --semantic-corpus ~/lor --semantic-limit 100 --semantic-max-chunks 10000

# локальный LLM
poler-engine --llm local --model models/chatglm3-6b.pqw

# NER zero-shot
poler-engine ~/corpus --ner gliner --model models/gliner.pqw \
    --ner-labels "человек,организация,место"

# корпус для контекста LLM
--corpus PATH
```

Флаги: `--model` (путь .pqw), `--semantic` (режим), `--semantic-corpus`
(живой поиск: чанки → нативные эмбеддинги → косинус; rayon-параллельно),
`--semantic-limit`, `--semantic-max-chunks`, `--llm` (`local`), `--ner`
(`gliner`), `--ner-labels`, `--pqw-selftest`, `--corpus`.

Конвертация моделей — scripts/convert_*.py (см. INSTALL.md).

## 6. Impact-анализ (AIDDE)

```bash
poler-engine ./src --impact "run_gateway" --impact-depth 3 --impact-cache db
```

`--impact СИМВОЛ` — upstream/downstream паспорт (call graph),
`--impact-depth`, `--impact-cache` (кэш БД), `--impact-reuse`.

## 7. Коннектом FLYCSR1 (C2/v0.33.0 «Живая муха») — мозг мухи как матрица A

```bash
poler-engine --connectome docs/flywire-connectome/flywire_v783_core.csr.zst
poler-engine --connectome core.csr.zst --connectome-nodes nodes.bin --connectome-node 0
poler-engine --connectome core.csr.zst --connectome-edge 0:6135     # + ротор J = A − Aᵀ
poler-engine --connectome core.csr.zst --connectome-khop 0 -k 2    # фронты [13, 443], 457
poler-engine --connectome core.csr.zst --connectome-impact 0       # CSC «кто управляет»
# C2/v0.33.0 — глубокое взаимодействие:
poler-engine --connectome core.csr.zst --connectome-neighbors 79529 --connectome-dir in
poler-engine --connectome core.csr.zst --connectome-path 0:116214  # маршрут с цепочкой синапсов
poler-engine --connectome core.csr.zst --connectome-common 0,6135 --connectome-dir down
poler-engine --connectome core.csr.zst --connectome-centrality --connectome-top 5
poler-engine --connectome core.csr.zst --connectome-rotor-top 5 --connectome-min-abs 50
poler-engine --connectome core.csr.zst --connectome-motifs 79529
poler-engine --connectome core.csr.zst --connectome-propagate 0 --connectome-steps 3 \
  --connectome-gamma 0.05 --connectome-leak 0.8
```

| Флаг | Смысл |
|---|---|
| `--connectome CSR_ZST` | артефакт FLYCSR1 (zstd + CSR): full 35 МБ / core 7 МБ в git |
| `--connectome-nodes NODES_BIN` | таблица root_id: имена в выводе, поиск по root_id |
| `--connectome-node IDX` | паспорт нейрона: степени, массы, топ-рёбра |
| `--connectome-edge U:V` | ребро + обратное + ротор J = A − Aᵀ (циркуляция пары) |
| `--connectome-khop IDX` | BFS потока сигнала; глубина — общий `-k/--k-hop` (2) |
| `--connectome-sign all\|exc\|inh` | фильтр знака K-hop/путей/симуляции (возб/торм/все) |
| `--connectome-impact IDX` | входящие (CSC): «кто управляет нейроном» + топ-источники |
| `--connectome-neighbors IDX` | партнёры по весу (убывание), направление `--connectome-dir` |
| `--connectome-path FROM:TO` | кратчайший путь сигнала: прыжки с весом/медиатором/знаком |
| `--connectome-common A,B,C` | общие партнёры набора (2–32): мишени/источники |
| `--connectome-centrality` | хабы (топ степеней) + PageRank по \|A\| |
| `--connectome-rotor-top K` | глобальный топ пар по циркуляции J = A − Aᵀ |
| `--connectome-motifs IDX` | мотивы: реципрокные пары, feedforward, feedback |
| `--connectome-propagate SEEDS` | симуляция x(t+1) = leak·x + γ·A·x на знаковых весах |
| `--connectome-dir out\|in\|down\|up` | направление для neighbors/common (умолчание out) |
| `--connectome-limit N` | лимит списков neighbors/common (20) |
| `--connectome-top N` | размер топов centrality/propagate (10) |
| `--connectome-min-abs J` | порог циркуляции rotor-top (1) |
| `--connectome-steps N` | шаги симуляции propagate (4) |
| `--connectome-gamma G` / `--connectome-leak L` | динамика симуляции (0.05 / 0.8) |
| `--connectome-json` | JSON-вывод любого режима (для агентов) |

Знак связи: **+1** ach/glut, **−1** gaba, **0** модуляторы (oct/ser/da);
модуляторы не входят в J. Exit-коды grep: 0 найдено / 1 нет / 2 ошибка.

## 8. Веб

```bash
poler-engine https://example.com --web                # рендер CDP → поиск
poler-engine https://seed.com --crawl --crawl-depth 2 --crawl-max 25
poler-engine --web-search "запрос"                    # по локальному индексу
poler-engine --semantic-expand "запрос"               # диагностика моста ru↔en
poler-engine --web-stats                              # JSON-статистика индекса
poler-engine --browser-index https://page             # одна страница в индекс
poler-engine --web-lens 127.0.0.1:8765                # браузерный режим
poler-engine --web-lens-install                        # установка в ежедневный браузер
```

| Флаг | Смысл |
|---|---|
| `--web` | PATH = URL: рендер Chromium CDP перед поиском |
| `--cdp-port` (9222), `--web-wait-ms` (1200) | параметры CDP |
| `--crawl` | обход от seed: robots.txt, sitemap, SimHash-дедуп, PageRank |
| `--crawl-depth` (2), `--crawl-max` (25), `--crawl-delay-ms` (1000), `--crawl-page-timeout-ms` (45000) | политика обхода |
| `--web-db` | путь к web-index.db (по умолчанию ~/.local/share/poler-engine/) |
| `--cross-site` | межсайтовый обход |
| `--web-search` | поиск по индексу + авто-расширение Semantic Bridge |

## 9. Литературный Двигатель POLER[Ψ] (L1/v0.34.0) — физика смысла

Полная матричная форма канонического уравнения: замысел → Ω(o) → фазовая
траектория к когнитивному покою H^Ψ = 0. Без мухи — сходимость
(«сверхпроводимость смысла»); с мухой (`--literary-csr`) — предельный
цикл: живой мозг не даёт нарративу замереть. Руководство: `docs/LITERARY.md`.

```bash
poler-engine --literary-field "герой идёт в поход против тьмы и бездны"
poler-engine --literary-generate "текст замысла" --literary-steps 128
# мушиная калибровка: каста от семян, ротор J = A − Aᵀ закручивает нарратив
poler-engine --literary-generate "текст" \
  --literary-csr docs/flywire-connectome/flywire_v783_core.csr.zst \
  --literary-nodes docs/flywire-connectome/flywire_v783_nodes.bin \
  --literary-seeds 0,116214 --literary-khop 2 --literary-max-cast 32
poler-engine --literary-generate "текст" --literary-no-mul --literary-json
```

| Флаг | Смысл |
|---|---|
| `--literary-field TEXT` | анализ поля интенции: термы-инварианты + архетипы (℘→O) |
| `--literary-generate TEXT` | полный прогон до H^Ψ = 0: акты, конфликты, телеметрия |
| `--literary-csr CSR_ZST` | мушиная калибровка: каста → J = A−Aᵀ (циркуляция) + D = L·Lᵀ (Ляпунов) |
| `--literary-nodes NODES_BIN` | таблица root_id (семена по root_id) |
| `--literary-seeds A,B,…` | семена касты, индексы или root_id (умолчание 0) |
| `--literary-khop N` | BFS-глубина касты (1–4, умолч. 2) |
| `--literary-max-cast N` | максимум нейронов касты (2–64, умолч. 48) |
| `--literary-dims N` | осей фазового пространства (16–256, умолч. 64) |
| `--literary-steps N` | шагов генерации (умолч. 48) |
| `--literary-eta F` / `--literary-eta-r F` | η шаг интегратора / η_r резонансный (0.1 / 0.05) |
| `--literary-rho F` | ρ затухания темпорального эха R[n] (0.9) |
| `--literary-kappa F` | κ масштаб энергии значимости ε (1.2) |
| `--literary-gamma F` | γ баланс циркуляции J — сила мушиного ротора (1.0) |
| `--literary-lambda F` | λ вес логической регуляризации (0.01) |
| `--literary-no-mul` | Trit5-квантование p_t в {−1,0,+1} (No-Mul, SIMD) |
| `--literary-json` | JSON-вывод для агентов |

## 10. Интерфейсы

```bash
poler-engine --view-mol "CC(=O)Oc1ccccc1C(=O)O"   # v0.74.0: живой 3D/4D-визор молекул
poler-engine --view-mol кофеин --mol-mode vdw     # имена молекул + режимы ball|vdw|wire
poler-engine --shell                # REPL + Tab-completion
poler-engine --tui                  # TUI: chat | notes | sources
poler-engine --gateway              # Terminal Gateway (sandbox-контур)
poler-engine --gateway --dangerously-allow-all   # аварийный люк host-контура
poler-engine --mcp                  # MCP-сервер stdio (JSON-RPC)
poler-engine --mcp-http 127.0.0.1:8765 --mcp-token X   # MCP HTTP + Bearer
poler-engine --mcp --vault-log session.pvt             # M6: стриминг логов вызовов в Vault .pvt
poler-engine --mcp --vault-log session.pvt --vault-log-pass "фраза"
POLER_VAULT_PASS="фраза" poler-engine --mcp --vault-log session.pvt
poler-engine --mcp --mcp-ram-budget 64    # M6: бюджет RAM-кэша файлов (МиБ, умолч. 48)
poler-engine --mcp-bench 500              # M6: бенчмарк резидентности (cold vs warm p50/p95/p99)
poler-engine --license              # статус EULA
```

MCP-инструменты: `poler_web_search`, `poler_crawl`, `poler_fetch`,
`poler_search`, `poler_grep`, `poler_chunk`, `poler_box_exec`,
`poler_box_status`, `poler_scivoice` (голос учёного: режимы read/hf/
molecule/bio — научное чтение, RHF/STO-3G, паспорт SMILES, био-контур).

### 10.1. Лестница молекул (v0.74.0)

```bash
poler-engine --exec 'calc chem_smiles("кофеин")'      # паспорт: кольца, Липински, Гастайгер
poler-engine --exec 'calc chem_3d("аспирин")'          # 3D-конформер: длины, гибридизации
poler-engine --exec 'calc bio_eval("дофамин", "серотонин")'  # ΔG/Kd в лог-домене + DE-9IM
poler-engine --exec 'calc hf("H2O")'                   # RHF/STO-3G: E = −74.962928 Ha (PySCF-сверка)
poler-engine --exec 'calc logdet(A)'                   # определитель за пределами f64
```

`--view-mol` — орбитальная камера прямо в терминале (стрелки/hjkl —
вращение, `+/-` — зум, `m` — режим, `t` — 4D-кручение σ-связей,
`[ ]` — скорость, `q` — выход); в пайпе печатает одиночный кадр.
SMILES-парсер: органический субсет + bracket-атомы + SSSR; базис
STO-3G расширен до B–Ne (Basis Set Exchange, Li SP сверен — по пути
найден и устранён дефект nuclear_cartesian: V(p,p) был положительным).

### 10.2. PDB-макромолекулы и гибкий докинг (v0.75.0)

```bash
poler-engine --pdb 3ptb.pdb                        # паспорт белка: цепочки, состав, SS, NET-заряд
poler-engine --pdb 1crn.pdb --pdb-mode pocket      # карманы: лигандный + слепой grid-burial
poler-engine --dock "NC(=N)c1ccccc1" 3ptb.pdb      # redocking бензамидина в трипсин
poler-engine --dock кофеин x.pdb --pocket blind    # слепой докинг (без подсказки лиганда)
poler-engine --dock "N=C(N)c1ccccc1" a.pdb --pocket 12.3,4.5,-6.7 --dock-out pose.pdb
poler-engine --view-complex 3ptb_docked.pdb        # визор: CA-трейс + карман + лиганд CPK
poler-engine --exec 'calc pdb_info("1crn.pdb")'    # паспорт белка голосом учёного
poler-engine --exec 'calc dock("NC(=N)c1ccccc1", "3ptb.pdb")'  # докинг из calc
```

**Протокол**: SMILES → 3D-конформер → заряды Гастайгера (эффективные,
тяжёлый атом + H-облако) → торсионное дерево (σ-связи, кроме амидных);
PDB → united-заряды (NET остатков точны: Arg/Lys +1, Asp/Glu −1, концы ±1)
→ реконструкция H-доноров (амидная биссектриса, NZ-пирамида, гуанидиний,
кольца) → 27-дерево собирает поле кармана O(log₂₇ N). Скоринг: LJ 12-6
softcore + кулон ε(r)=4r + направленные H-связи (D–H···A геометрия,
угловой фактор) + липофильная рампа 3.2–4.5 Å + десольватация + энтропия
кручений. Поиск: MC/SA 12×2200, кручения квантованы трит-секторами 2π/27;
ансамбль топ-поз через logsumexp (лог-домен); Kd = e^(ΔG/RT).

**Валидация (3PTB, боевой трипсин + бензамидин, 1.7 Å)**: redocking
RMSD = 0.54 Å (нейтральная форма; ≤ 2 Å — академический стандарт) и
1.88 Å (физиологический бензамидиний); карман = канонический S1
(Asp189 ⇄ амидиний, Trp215/Val227/Tyr228 — гидрофобная коробка);
6–7 H-связей, 0 стерических конфликтов, 0.5 с на CPU, RSS 12 МБ
(лимит канона 48 МБ). Парсер: PDB fixed-width + mmCIF _atom_site
(включая кавычечные имена атомов C1-штрих), имена остатков
нормализуются (PDB выравнивает вправо: « CA» → Ca²⁺), altLoc ≠ A
и модели > 1 отбрасываются; 1CRN: 327 атомов/46 остатков,
последовательность крамбина точна, 3 дисульфида, NET 0.

`--view-complex` — три режима (`m`): CA-трейс по классам остатков
(гидрофобные жёлтые, полярные голубые, заряженные красно/синие,
ароматические циан, дисульфиды золотом), боковые цепи кармана,
ВдВ-поверхность всего белка; лиганд — CPK шаростержневой поверх.

M6 (резидентность): Гиппокамп/WebIndex/кэш файлов живут в RAM между
запросами (тёплые p99: grep ≤2 мс, knowledge ≈250 мкс на release);
`--vault-log` пишет каждый вызов инструмента зашифрованной JSON-строкой
(`{"ts","method","tool","us","ok"}`) — контейнер валиден на каждом коммите
и читается штатным `--memory-open`.

## 11. Бенчмарк

```bash
poler-engine --benchmark --benchmark-json report.json
```

6 контуров: Teddy-предфильтр · grep+parity (vs ripgrep/grep) ·
BM25-golden · чанкер · vectors · compression/RSS + latency (мс) и RAM
(VmHWM/VmRSS).

## 12. Крипто-слой данных: POLER Vault `--memory-*` (M4.5, фича pnd-ffi)

Зашифрованная постоянная память: любой поток (документы, логи
терминала, история чата, ответы ИИ) запечатывается в контейнер `.pvt`
(CBC PND v8.2, 256-битный ключ из парольной фразы, ланцюговій MAC,
внешний SHA-256). Контейнер синхронизируется через git/любой транспорт:
без ключа — нечитаем, но проверяем (`--memory-verify`). Формат:
`docs/formats/VAULT_FORMAT.md`. Память O(1) — от 500 КБ до сотен ГБ.

```bash
export POLER_VAULT_KEY="…"   # или строка из stdin, если env нет

poler-engine --memory-seal   logs.txt            # → logs.txt.pvt
poler-engine --memory-seal   logs.txt --memory-content-id   # + контент-хеш
poler-engine --memory-verify logs.txt.pvt        # без ключа: FNV + SHA-256
poler-engine --memory-info   logs.txt.pvt        # метаданные без ключа
poler-engine --memory-open   logs.txt.pvt        # → logs.txt (проверка MAC)
poler-engine --memory-open   logs.txt.pvt --memory-out restored.txt
```

Опции: `--memory-out PATH` (явный выход; по умолчанию существующий файл
не перезаписывается), `--memory-key-env VAR` (имя env с ключом),
`--memory-content-id` (публичный контент-хез в заголовок — дедупликация
 ценой возможности сверки догадок), `--memory-kdf-iters N` (default
100000, минимум 10000). Требует сборки с `--features pnd-ffi`
(Zig-ядро, см. INSTALL.md).

## 13. Удалённое в v2.0 (не возвращать)

`--google-*`, `--nlm-*`, `--auth-ui`, `--import-browser-session`,
`--license-import` — Google/NLM-интеграции вырезаны (суверенный стек,
SKILL.md). `--web`/`--crawl`/`--web-search`/`--mcp*`/`--gateway` —
сохранены: CDP-краулинг и публичные REST — не облачная зависимость.

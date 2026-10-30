# vault_drafts — черновики из сокровищницы до-движковой эпохи

> **Статус: ЧЕРНОВИКИ.** Этот каталог — консервация первоисточников, а не рабочий код.
> Ничего здесь не подключено к `Cargo.toml`, не компилируется как единое целоe и
> не тестируется CI. Это задел-сырьё для линии v0.87+ (см. [USAGE_PLAN.md](USAGE_PLAN.md)).
> Оригинальные файлы не редактируются — правки возможны только при переносе в `src/`.

## Происхождение

| Пакет | Источник | Объём | Когда |
|---|---|---|---|
| `deepseek_vault/` | АРХИВ_ДИАЛОГОВ_DEEPSEEK (Google Drive, tar.gz 108 МБ → 284 МБ, 2174 записи) — экспорт диалогов эпохи до poler-engine, из которых движок и вырос | 1029 блоков кода, 4.93 МБ | session-22, 2026-10-09 |
| `poler_quantum/` | Снапшот репозитория [Kotokvit/POLER-Quantum](https://github.com/Kotokvit/POLER-Quantum) (main, 1ea7d9d) — линия POLER[n] attention-core: Python-эталон `poler_quantum/` (128 тестов) + Rust-архив `archive/` (poler-core/FEP, poler-lens, poler-dynamis, poler-sctp, poler-bridge, poler-tcp, lean4, fpga, zig, julia) | 90 файлов, 1.08 МБ | session-23, 2026-10-10 |

Линия POLER-Quantum-RS (крейты `pqw`/`pqc`, RQ1–RQ6, 287 тестов) — отдельный
репозиторий; она уже интегрирована в движок (`src/pqc/` v1.5.0, path-зависимость
описана в [../INSTALL.md](../INSTALL.md)) и здесь не дублируется.

## Структура

```
vault_drafts/
├── README.md               — этот файл
├── USAGE_PLAN.md           — план использования: матрица «слиток → модуль движка»
├── deepseek_vault/         — 1029 блоков, плоская нумерация NNNN_NNN_lang.ext
│   ├── catalog.md|json     — каталог: источник, строка, язык, тема, sha256
│   └── VAULT_ANALYSIS.md   — полный разбор session-22 (матрица портрирования)
└── poler_quantum/          — снапшот репо POLER-Quantum (без .git)
    ├── poler_quantum/      — активный Python-эталон (core/quantum/benchmark, 128 тестов)
    ├── archive/            — Rust-крейты до перехода на POLER-Quantum-RS
    │   ├── poler-core/     — PolerCore (℘–O–L–ε–R[n]–Ψ) + FEPLoss (F и ∇F)
    │   ├── poler-lens/     — LensIndex (No-Hits барьер, 99.2% сжатия)
    │   ├── poler-dynamis/  — квант-химия: двухфазный SCF, ERI, GTO, базисы, libcint
    │   ├── poler-sctp/     — ConstraintLayer (косинусная топология)
    │   ├── poler-bridge/   — асинхронный TCP-сервер моста
    │   ├── poler-tcp/      — бинарный протокол POLR
    │   ├── poler-eri/      — сгенерированные ERI-контракции (ssss/pppp)
    │   ├── rust_engine_core/ — epsilon/psi/iir_filter (ранние версии блоков движка)
    │   ├── lean4_formal_proofs/ — теорема стационарности HΨ=0 на Lean 4
    │   ├── fpga_verilog/   — multiplier-less ядро POLER для FPGA
    │   ├── hardware_zig/   — f64-тензоры на Zig
    │   └── julia_brusselator/ — субквантовая кинетика (Брюсселятор)
    ├── docs/               — POLER_Attention_Core, dynamic-quantization, rust-core-roadmap
    ├── legacy/             — ранние прототипы (POLER_sim/Psi_v3/modeB/modeC, qiskit_ansatz)
    └── tests/, examples/   — тесты эталона (128) и демо
```

## Как искать (dogfooding)

Каталог — обычный кодовый корпус: движок индексирует его как любой другой.

```bash
poler-engine docs/vault_drafts/deepseek_vault --grep "SynapticVortex" --grep-list
poler-engine docs/vault_drafts -q "class ConsciousAI" --format simple
poler-engine docs/vault_drafts/poler_quantum -q "FEPLoss" --format simple
```

Токоно-граничный поиск не ищет префиксы идентификаторов
(`SynapticVortex` ≠ `SynapticVortexV5`) — для инвентаризации используй `--grep`.

## Почему это в репо

1. **Сохранность**: 4.93 МБ уникального кода до-движковой эпохи существовали только
   в tar.gz на диске; теперь история проекта неотделима от самого проекта.
2. **Единый корпус**: движок может искать собственную колыбель одним запросом,
   не переключаясь между хостом и GitHub.
3. **Доказанная ценность**: 5 технологий из этой сокровищницы уже ported в ядро
   (POLER-цикл → `src/poler.rs`, SCTP → `src/ssn/transport.rs`, архетипы → `p3-engine`,
   вихрь → `src/ssn/vortex.rs`, literary → `src/literary/`); 7 слитков ждут очереди —
   см. [USAGE_PLAN.md](USAGE_PLAN.md).

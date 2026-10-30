# USAGE_PLAN: как сокровищница ложится в poler-engine (линия v0.87+)

> План использования двух пакетов черновиков: `deepseek_vault/` (1029 блоков
> из до-движковой эпохи DeepSeek) и `poler_quantum/` (снапшот POLER-Quantum —
> линия POLER[n] attention-core). Составлен session-23 (2026-10-10) после
> проверки фактического состояния ядра; выбор очерёдности — за владельцем.

---

## 0. Сводка активов

| Пакет | Состав | Золотые файлы |
|---|---|---|
| `deepseek_vault/` | 1029 блоков / 4.93 МБ: python 139, bash 408, c 58, js 52, rust 39, zig 29, julia 12, verilog 8, nasm 2 | `0986/0984_md_python.py` (синапс-SSN 1.7 Б), `0601_662_c.c` (LanguageCoreV2), `0496_576_rust.rs` (SubquantumEntangler), `0377_346_verilog.txt` + `1021–1028_md_verilog` (Verilog-слой), `0589–0596_661_python.py` (RPN), memory_soliton-блоки (93 шт) |
| `poler_quantum/` | 90 файлов / 1.08 МБ: Python-эталон (128 тестов) + Rust-архив | `archive/poler-core/src/{fep_loss,poler_core}.rs` (FEP: F и ∇F, цикл ℘–O–L–ε–R[n]–Ψ), `archive/poler-dynamis/` (двухфазный SCF, ERI, базисы, libcint), `archive/poler-lens/src/lens_index.rs` (No-Hits барьер), `archive/{poler-bridge,poler-tcp}` (POLR-мост), `archive/lean4_formal_proofs/` (HΨ=0), `archive/fpga_verilog/poler_fpga_core.v`, `poler_quantum/core/*.py` (эталон: free_energy, purification, compression, AdaptiveDepth) |

Родословная трёх репозиториев (уточнена session-23):

```
DeepSeek-диалоги (эпоха до движка) ──┬──> poler-engine  (этот репо: поиск, ядро, всё)
                                     └──> POLER-Quantum (Python-эталон POLER[n])
                                              └──> POLER-Quantum-RS (pqw/pqc)
                                                       └──> src/pqc/ v1.5.0 — УЖЕ В ДВИЖКЕ
```

`docs/rust-core-roadmap.md` (в снапшоте) прямо перечисляет блоки poler-engine
как строительную базу квант-ядра: `src/psi.rs`, `src/resonance/iir_filter.rs`,
`src/resonance/epsilon.rs`, `src/web/simhash.rs`, `src/streaming.rs` — все
пять существуют и сегодня (проверено session-23). Сокровищница и движок —
не чужие друг другу миры, а две ветви одного корня, которые пора сшить.

---

## 1. Проверенная база движка (что уже в `src/`, session-23)

| Блок | Файл | Что даёт для интеграции |
|---|---|---|
| Π_Λ-проектор | `src/psi.rs` (329) | логические ограничения, пинв-устойчивость |
| ε-значимость | `src/resonance/epsilon.rs` (447) | «важность → глубина представления» |
| IIR-резонанс | `src/resonance/iir_filter.rs` (107) | R_t = ε_t + ρ·R_{t−1}, O(N)/O(1) |
| Канонический POLER-цикл | `src/poler.rs` (372) | D=L·Lᵀ, J=A−Aᵀ, CORDIC — есть ∇F-слот |
| SCTP-проектор | `src/ssn/transport.rs` (438) | Π=I−Jcᵀ(JcJcᵀ)⁻¹Jc (EQ-B69) |
| Квант-ядро pqc | `src/pqc/` v1.5.0 | pqw-контейнер, фазовый энкодер, Born |
| POLER-ERI | `src/quantum/{eri,meta_compiler}.rs` | вентильный компилятор квант-химии |
| Химия | `src/chem/periodic.rs` и др. | таблица Менделеева, формулы, стехиометрия |
| JIT-компилятор | `src/graph/graph_asm.rs` (968) | граф → x86_64, Trit-веса в регистрах |
| Пластичность | `src/triune/compiler.rs` | PlasticityCompiler: STDP + фазовый ротор |
| Буфер | `src/editor/buffer.rs` | mmap Piece-Table + SIMD line-index |

Вывод: для большинства слитков двигателю не хватает не «технологии», а
**проводки** — того же рода, что v0.86 сделал для EntityGraph (технология
жила в параллельном мире, пока не была вшита в поиск).

---

## 2. Матрица приоритетов

| # | Слиток | Откуда | Куда | Усилие | Ценность |
|---|---|---|---|---|---|
| 1 | **FEP-контур** (FEPLoss + PolerCore ℘–O–L–ε–R[n]–Ψ) | poler_quantum `archive/poler-core/` | `src/fep/` | ~1–2 дня | замыкает математический контур: ∇F для poler.rs, связка resonance↔psi |
| 2 | **LENS No-Hits** (query_constraint, вес > 0.05) | poler_quantum `archive/poler-lens/` | `src/search/` (фильтр рёбер нексуса) | ~1 день | барьер галлюцинаций поверх Causal Nexus v0.86 |
| 3 | **Синапс-атомарная SSN** (1.7 Б/синапс, func_id:uint4) | vault 0986/0984 | `src/ssn/synapse.rs` + `graph_asm` | ~неделя | инференс без трансформера; FlyWire 50M синапсов ≈ 85 МБ |
| 4 | **POLER-DYNAMIS v5** (двухфазный SCF) | poler_quantum `archive/poler-dynamis/` | `src/quantum/dynamis/` | ~неделя | квант-химия целиком: SCF+ERI+базисы+libcint |
| 5 | **LanguageCoreV2** (трёхфакторное STDP) | vault 0601+4 C-файла | `src/triune/lang_core.rs` | ~неделя | спайковое языковое ядро без токенайзера |
| 6 | **POLR TCP-мост** (async сервер + бинарный протокол) | poler_quantum `archive/{poler-bridge,poler-tcp}` | `src/gateway/` | ~2–3 дня | распределённый POLER Mesh |
| 7 | **FPGA-путь** (multiplier-less ядро) | poler_quantum `archive/fpga_verilog/` + vault верилоги | `tools/fpga/` | задел | аппаратный полигон (GTX 1060 → FPGA-плата) |
| 8 | **Lean4-пруфы** (стационарность HΨ=0) | poler_quantum `archive/lean4_formal_proofs/` | `proofs/lean/` | задел | формальная верификация инвариантов ядра |
| 9 | Бэклог: SubquantumEntangler, RPN, солитоны, Julia, qiskit | vault | `tools/verifiers/` | — | исследования с мини-верификаторами |

---

## 3. Детали по приоритетам

### 3.1. FEP-контур (рекомендация на v0.87)

Это единственный слиток из «великолепной семёрки» session-22, для которого
готовый Rust-код существовал не в движке, а в другом репо — теперь он здесь.

* **Перенос**: `FEPLoss` (fep_loss.rs, 50 строк) и `PolerCore` (poler_core.rs,
  92 строки) — суммарно ~142 строки чистого Rust без burn-зависимостей.
  `FEPLoss::compute_loss_and_grad(p, obs_tanh) -> (F, ∇F)`; `PolerCore::
  evolve_step(observation, forbidden) -> (p_next, f_energy, stabilized)`:
  ℘(tanh) → F → R[n]-эхо (ρ^k, глубина 8) → p_{t+1} = clamp(p + η·(−∇F + γ·∇ε)).
* **Сшивка с существующим**: `resonance/epsilon.rs` уже считает значимость —
  сделать `η_eff = η·exp(β·tanh(ε̂))` (формула POLER_Attention_Core) параметром
  шага; IIR-фильтр заменить ручное ρ^k-эхо; Π_Λ из `psi.rs` — в шаг эволюции
  (ConstraintLayer заявляет его в доке, но в коде не делает — см. §5).
* **Оракул**: Python-эталон `poler_quantum/core/` (128 тестов, детерминизм
  на фиксированных сидах) — паритет Rust↔Python по канону cross_qiskit_parity
  из POLER-Quantum-RS (у движка уже есть этот паттерн для pqc).
* **Критерий приёмки**: стационарность `F < 1e-7 ∧ ‖p‖ ≤ 1.05` (Lean4-
  определение IsStationary) — как Rust-тест convergence на эталонных задачах
  tracking-бенчмарка (`poler_quantum/benchmark/`).

### 3.2. LENS No-Hits — барьер галлюцинаций в поиске

`LensIndex::query_constraint(source, target)` — семантическое ребро допустимо
только при весе > 0.05. Идея: v0.86 строит нексусные рёбра K-hop — LENS-фильтр
станет second opinion: рёбра ниже порога помечаются weak и не попадают в
`NexusNode.relations`. Это дёшево (HashMap, O(1) на ребро) и не искажает
ранжирование — чисто презентационный слой, как и весь нексус.

### 3.3. Синапс-атомарная SSN — инференс без трансформера (рекомендация session-22)

`Synapse{w: fp16, b: fp16, func_id: uint4}` ≈ 1.7 байта/синапс + блоки
linear/sinusoidal/decoder/attention + ObservationCircuit. Проецирование на ядро:

* `func_id` (16 функций) → таблица примитивов `graph_asm::compile_x86_64`
  (у JIT уже есть Trit{val,scale} — веса прямо в машинном коде);
* плотность: 50M синапсов FlyWire × 1.7 Б ≈ **85 МБ — весь мозг мухи в RAM**
  целиком (для сравнения: fp32-матрица смежности 50M — ~200 ГБ);
* ObservationCircuit → `triune` (контур самопереписи .t5q уже есть);
* эталон блоков — vault 0986/0984 (Python), формат весов стыкуется с `.pqw`
  (фазовые триты + McWeeny-инвариант уже в контейнере).

### 3.4. POLER-DYNAMIS v5 — квантово-химический контур

`archive/poler-dynamis/` — полный SCF-стек: `energy_engine.rs` (двухфазный
закон dDM/dt = −η·Π_Λ[Δ_idem + γJ·DM + K]; Фаза 1 — Fock-диагонализация как
предельный случай η→∞ с dE-триггером; Фаза 2 — POLER-flow), `eri_engine.rs`
+ `generated/eri_{ssss,pppp}.rs`, `gto_utils.rs` ((2l−1)!!), `basis_parser.rs`
(6-31G/cc-pVDZ), `elements.rs`, `subquantum_bridge.rs` (libcint FFI).

Сшивка: `quantum/eri.rs` в движке уже несёт POLER-ERI v3.2.0 и мета-компилятор
вентилей — DYNAMIS становится слоем выше (SCF-драйвер поверх ERI-ядра), а
`chem/periodic.rs` закрывает elements.rs (таблица Менделеева уже своя, IUPAC
2021). dE-триггер «закон сохранения смысла» — тот же паттерн, что в literary.

### 3.5. LanguageCoreV2 — STDP-языковое ядро

Трёхфакторное правило `dw = η·reward·eligibility` (LTP 0.02·e^(−dt/10),
eligibility-след 0.95, softmax-награда), зоны input/hidden/output, задержанные
рекуррентные связи d=1..5, 25% тормозных, WTA. Ложится на PlasticityCompiler
(`triune/compiler.rs` — STDP уже есть, не хватает фактора reward и зон).

### 3.6. POLR-мост — POLER Mesh

`tcp_server.rs` (async) + `shared_protocol.py` (бинарный POLR). Формат
состояния (roadmap §5): p ∈ [-1,1]^d, триты, J=A−Aᵀ верхним треугольником —
совместим с `.pqw`, который движок уже читает. Узлы mesh исполняют состояние
без переквантования; для движка это путь к распределённому инференсу через
существующий `gateway/`.

### 3.7. FPGA и Lean4 — заделы

* `poler_fpga_core.v` — multiplier-less (shift-and-add `R_t = ε_t + ρR_{t−1}`),
  та же философия, что CORDIC в poler.rs. Плюс vault-верилоги:
  AsymmetricSynapsePrimitive, QuantumNeuronCore, WavePropagationNetwork.
  Полигон: iverilog/yosys в `tools/fpga/`, симуляция без железа.
* Lean4: текущий `poler_stationarity.lean` — скелет (см. §5), но декларирует
  правильный критерий; настоящий пруф-контур — кандидат на `proofs/lean/`
  рядом с существующими `proofs/*.py`.

---

## 4. Порядок релизов (рекомендация)

| Релиз | Содержание | Обоснование |
|---|---|---|
| **v0.87 «FEP + LENS»** | §3.1 + §3.2 | минимальное усилие при максимальном замыкании: оба контура маленькие, но сшивают уже существующие модули (poler.rs, resonance, psi, search/nexus) |
| **v0.88 «Синапс-SSN»** | §3.3 | инференс-слой — самая уникальная технология сокровищницы; func_id→graph_asm |
| **v0.89 «DYNAMIS»** | §3.4 | квант-химия; много кода уже написано в archive/ |
| **v0.90 «STDP-язык»** | §3.5 | языковой контур поверх пластичности |
| заделы | §3.6–3.7 + бэклог | по мере интереса владельца |

Выбор очерёдности — прерогатива владельца; «FEP+LENS» первым потому, что это
день-два работы и они не требуют новых зависимостей.

---

## 5. Честные оговорки (проверено чтением кода)

1. **ConstraintLayer.forward** (synaptic_ops.rs) делает только `(J−D)·p` —
   Π_Λ-проекция заявлена в комментарии, но в коде отсутствует. При переносе
   добавить проекцию через `psi.rs`, иначе слиток потеряет главное.
2. **lean4/poler_stationarity.lean** — скелет, не пруф: `Idempotent := True`
   (заглушка), теорема `wheeler_dewitt_stationarity` доказывается `dsimp +
   exact` (тривиальна — это конъюнкция гипотез). Ценность — канон критерия
   стационарности, не математика.
3. **energy_engine.rs**: `symmetric_eig` — placeholder (Jacobi 2×2, «для
   production: faer's eigendecomposition»); сам файл носит следы конвертации
   из машинного формата (сломанные переносы в шапке). При переносе требуют
   полировки; fock_eig_step уже на Cholesky-разложении S.
4. **FEPLoss** в коде проще формулы из доков: G-метрика не реализована
   (обычная L2), регуляризатор — 0.5·λ·‖p‖². Python-эталон (`core/free_energy.py`)
   ближе к доке — сверять при портировании.
5. **Vault-код** — прототипы эпохи «слепой реактор»: ценность в математике и
   структуре, не в готовности; без прогона тестов переносить только идею +
   формулы, переписывая на канон движка (как это делали 614→poler.rs).
6. **Дубликаты**: poler_quantum/archive/rust_engine_core/{epsilon,psi,
   iir_filter}.rs — ранние версии блоков, которые в движке уже есть в
   развитом виде (src/resonance/, src/psi.rs); переносить оттуда нечего,
   это генеалогия.

---

## 6. Как искать в черновиках

```bash
# инвентаризация (греп — префиксы идентификаторов не ищутся токенами)
poler-engine docs/vault_drafts/deepseek_vault --grep "SynapticVortex" --grep-list
poler-engine docs/vault_drafts/deepseek_vault --grep "STDP" --grep-list
# ранжированный поиск с нексусом
poler-engine docs/vault_drafts -q "class ConsciousAI" --format simple
poler-engine docs/vault_drafts/poler_quantum -q "FEPLoss" --format simple
# каталог блоков
less docs/vault_drafts/deepseek_vault/catalog.md
```

Матрица «портрировано/нет» по состоянию на session-22 — в
[VAULT_ANALYSIS.md](VAULT_ANALYSIS.md) (deepseek_vault/) и §1–§2 выше
(дополнено session-23: слиток №7 FEP закрыт пакетом poler_quantum).

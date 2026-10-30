# Карта документации POLER Engine

> Этот файл — точка входа во всю документацию проекта. Если вы открыли репозиторий
> впервые, начните отсюда, затем идите в `ARCHITECTURE.md`.

POLER Engine — поисково-аналитический движок полного логического скоупа,
спроектированный как **инструмент для ИИ-агентов**: индексирует, находит,
отдаёт, связывает по метаданным. Не понимает контент, не генерирует текст,
не принимает решений — понимает ИИ, творит автор.

Принципы проекта (не нарушать): суверенный стек без облачных API, Rust + CPU
only, нативный инференс через собственное ядро `pqc`, банальность как сила
(как grep/SQL/git).

---

## Сводная таблица

| Документ | Что покрывает | Для кого |
|---|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Целостная архитектура v2.0: слои, модули, потоки данных, интерфейсы | все, кто меняет код |
| [THEORY.md](THEORY.md) | Математический аппарат: POLER[Ψ], ε-плотность, IIR-резонанс R(t), RaBitQ, FSST, Teddy, R1CS, Trit5 | чтобы понимать «почему так» |
| [HISTORY.md](HISTORY.md) | Генеалогия проекта (от лора Этерии до v2.0) и хроника разработки | все, кто хочет контекста |
| [quantum-eri.md](quantum-eri.md) | Квантовый мост, POLER-ERI v3.2.0, Reverse Meta-Compiler (`src/quantum/`) | инженеры SIMD/компиляторов |
| [UNIFIED_ARCHITECTURE.md](UNIFIED_ARCHITECTURE.md) | **Канон единой архитектуры** (M3–M7): 4 слоя монорепозитория, крипто-мост M4, дорожная карта | архитекторы экосистемы |
| [POLER_MANIFEST_v0.29.md](POLER_MANIFEST_v0.29.md) | Манифест владельца: позиционирование «Суверенный Гиппокамп и Микро-Рантайм», 5-уровневый стек POLER SYSTEM | агенты, LLM, люди |
| [MERGE_PLAN.md](MERGE_PLAN.md) | Исторический план слияния (M2–M5; **M2 и M3+M4 исполнены** — см. UNIFIED_ARCHITECTURE.md) | археология консолидации |
| [MODULES.md](MODULES.md) | Справочник по 29 модулям `src/` с публичным API | навигация по кодовой базе |
| [CLI.md](CLI.md) | Референс всех режимов и флагов командной строки | пользователи и агенты |
| [SSN.md](SSN.md) | **Синаптический Вихрь SSN (S1/v0.35.0)**: живой мозг, доказанный до реализации — 67/67 проверок, 13 исправленных режимов отказа (F1–F13), полный стек динамики, CSE-сенсорика, MCP poler_ssn_*, пруфы proofs/*.py | агенты, живые сессии, субстрат управления |
| [LITERARY.md](LITERARY.md) | **Литературный Двигатель POLER[Ψ] (L1/v0.34.0)**: физика смысла — канонический интегратор, калиброванный мухой; MCP poler_literary_*, архетипы, призма No-Excuses, Trit5 No-Mul, стратегия допроса | агенты, нарратив |
| [CONNECTOME.md](CONNECTOME.md) | **«Живая муха» (C2/v0.33.0)**: руководство агента по коннектому FLYCSR1 — 10 MCP-инструментов poler_fly_*, стратегия допроса, интерпретация, золотые числа | агенты, нейронаука |
| [TESTING.md](TESTING.md) | Философия тестирования: дифференциалы, golden, детерминизм | контрибьюторы |
| [../crates/reader/README.md](../crates/reader/README.md) | **POLER Reader (v0.47.0)**: приложение живого голоса — роторный резонатор + коартикуляция, формат .poler-book (книга ×1500 меньше PCM), CLI poler-reader + команда шелла `read`, suite V1–V8 | аудиокниги, живой голос |
| [formats/PQW_FORMAT.md](formats/PQW_FORMAT.md) | Спецификация формата весов `.pqw` v2 | конвертеры, ядро pqc |
| [formats/PRBQ_FORMAT.md](formats/PRBQ_FORMAT.md) | Спецификация хранилища квантованных векторов PRBQ v1 | векторный субстрат |
| [formats/WEB_INDEX_FORMAT.md](formats/WEB_INDEX_FORMAT.md) | Схема SQLite веб-индекса (pages/terms/links/hosts) | веб-краулер, --web-search |
| [formats/VAULT_FORMAT.md](formats/VAULT_FORMAT.md) | Спецификация зашифрованного контейнера памяти `.pvt` v1 (M4.5 CDL: CBC PND v8.2, ланцюговій MAC, внешний SHA-256, CLI --memory-*) | крипто-слой данных, синхронизация памяти через git |
| [../GLOSSARY.md](../GLOSSARY.md) | Глоссарий терминологии POLER (ε, R(t), русла J, сцены, архетипы…) | все |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Как контрибьютить: процесс, инварианты, дисциплина коммитов | контрибьюторы |
| [../INSTALL.md](../INSTALL.md) | Сборка (включая path-зависимость POLER-Quantum-RS), конвертация моделей | первый запуск |

## Специализированные документы

| Документ | Тема |
|---|---|
| [vault_drafts/USAGE_PLAN.md](vault_drafts/USAGE_PLAN.md) | **Сокровищница до-движковой эпохи (session-22/23):** 1029 блоков кода из архива DeepSeek + снапшот POLER-Quantum (FEP-код Rust, DYNAMIS, LENS, FPGA, Lean4) в [vault_drafts/](vault_drafts/) — матрица «портрировано/нет», план интеграции v0.87+ | 
| [POLER_REVERSE_META_COMPILER.md](POLER_REVERSE_META_COMPILER.md) | Англоязычная техспецификация meta_compiler (параллельная сессия): Reverse/Meta-стадии, dual execution targets, 31 400 проходов/сек — дополнение к [quantum-eri.md](quantum-eri.md) |
| [GAME_ENGINE_ROADMAP_UE_ANALYSIS.md](GAME_ENGINE_ROADMAP_UE_ANALYSIS.md) | **UE-анализ и архитектура игрового ядра (цикл S):** 10 болей UE/Unity/Godot, 8 паттернов UE что берём, Rust-ответы, роадмап G-циклов до сети, файлы UE для изучения в форке владельца; UE-заголовки — в [research-archive/ue_reference.poler](research-archive/ue_reference.poler) (полер-бокс, чтение без распаковки) |
| [benchmarks/GAME_CYCLE_T_v0.55.0.md](benchmarks/GAME_CYCLE_T_v0.55.0.md) | **Цикл T «Кристаллы» (v0.55.0):** T1 акустический кристалл (ω→высота, audio_hash/crystal_hash, 15 с демо), T2 спектральный синтез (аналитические тайлы, SVD rank-4 = 55–57 dB, VLM SHIP), T0 полер-бокс |
| [benchmarks/GAME_CYCLE_U_v0.56.0.md](benchmarks/GAME_CYCLE_U_v0.56.0.md) | **Цикл U «Окно» (v0.56.0):** события с коалесцингом → ввод (рёбра кадр) → орбит-камера → окно: Offscreen replay-эталон (frames_hash бит-в-бит, 2.5 мс/кадр ≈ 400 fps) + настоящее X11-окно (dlopen, zero-dep); U0 normal maps из той же функции шума (VLM 8/8/7.5 SHIP, кадры 9/10 SHIP) |
| [MONOREPO_CONSOLIDATION_PLAN.md](MONOREPO_CONSOLIDATION_PLAN.md) | Англоязычный мастер-план монорепо (параллельная сессия): дерево crates/ — дополнение к [MERGE_PLAN.md](MERGE_PLAN.md) (фазы M0–M5 и исторический контекст — там) |
| [COGNITIVE_ARCHITECTURE_299_SOURCES_SYNTHESIS.md](COGNITIVE_ARCHITECTURE_299_SOURCES_SYNTHESIS.md) | Синтез теоретических столпов из 299 источников (FEP, резонанс фаз) — дополнение к [THEORY.md](THEORY.md) §1–2 и [HISTORY.md](HISTORY.md) (карта тем архива — там) |
| [ENCYCLOPEDIA_299_SOURCES.md](ENCYCLOPEDIA_299_SOURCES.md) | **Полная энциклопедия и реестр всех 299 первоисточников** (215 КБ): сквозной каталог с #001 по #299 с описанием тем, математики и привязки к коду |
| [mathematical-treatise/VOLUME_I_QUANTUM_DYNAMICS_AND_ROTORS.md](mathematical-treatise/VOLUME_I_QUANTUM_DYNAMICS_AND_ROTORS.md) | **Трактат Том I:** Квантовая динамика, кососимметричный ротор J=U-Uᵀ, закон сохранения энергии d/dt∥ψ∥²=0, мера Борна |
| [mathematical-treatise/VOLUME_II_R1CS_SIMD_META_COMPILER_ALGEBRA.md](mathematical-treatise/VOLUME_II_R1CS_SIMD_META_COMPILER_ALGEBRA.md) | **Трактат Том II:** Алгебра R1CS, битовые маски VectorGate8 {-1,0,+1}, AVX2 без умножений и коммутативный граф CSE |
| [mathematical-treatise/VOLUME_III_RABITQ_AND_HIGH_DIMENSIONAL_GEOMETRY.md](mathematical-treatise/VOLUME_III_RABITQ_AND_HIGH_DIMENSIONAL_GEOMETRY.md) | **Трактат Том III:** 1-битные метрические пространства RaBitQ, вращение Уолша-Адамара, несмещённая оценка arcsin-MLE |
| [mathematical-treatise/VOLUME_IV_ACTIVE_INFERENCE_AND_ENERGY_DYNAMICS.md](mathematical-treatise/VOLUME_IV_ACTIVE_INFERENCE_AND_ENERGY_DYNAMICS.md) | **Трактат Том IV:** FEP Фристона, функционал свободной энергии, Z-преобразование и передаточная функция IIR R(t) |
| [mathematical-treatise/VOLUME_V_NONLINEAR_CRYPTOGRAPHIC_DIFFUSION_PND_V8.md](mathematical-treatise/VOLUME_V_NONLINEAR_CRYPTOGRAPHIC_DIFFUSION_PND_V8.md) | **Трактат Том V (изд. 2, code-grounded):** PND v8 по реальному Zig-ядру poler-os — golden-сверка 54 626 векторов; Φ биективна (реальная 6-шаговая структура с mul/xorshift); заявка δ≤8 ОПРОВЕРГНУТА точно (8-значная Δφ-таблица на полном 2³², P[Δc=0]=0.30385, δ≈2²⁸·²); MDS ℬ=5 доказана; честная дифференциальная граница шифра ~2⁻⁷⁰ вместо 2⁻¹⁵⁰ |
| [mathematical-treatise/VOLUME_VI_CAUSAL_DYNAMICS_AND_PRISM_TOKENIZATION.md](mathematical-treatise/VOLUME_VI_CAUSAL_DYNAMICS_AND_PRISM_TOKENIZATION.md) | **Трактат Том VI:** Каузальная динамика, инвариант ħω=0, 5-фазный цикл ℘–O–L–ε–R[n], проектор МакВини Π_Λ=3P²-2P³, канонический градиентный поток dp/dt и фазовый переход SyntaxUnfolder вместо BPE |
| [mathematical-treatise/VOLUME_VII_THE_UNIFIED_DISCRETE_EQUATION.md](mathematical-treatise/VOLUME_VII_THE_UNIFIED_DISCRETE_EQUATION.md) | **Трактат Том VII:** ЕДИНОЕ ДИСКРЕТНОЕ УРАВНЕНИЕ — замкнутая рекуррентность p_{t+1}=Q_Λ(p_t−η_t·Π_Λ[Dp+γJp+∇F]+η_r·Π_Λ[W_K·p−M]) с IIR-эхом M_t=ρ(M_{t−1}+s_{t−1}), квантователем МакВини, химическим потенциалом числа частиц и аттрактором H^Ψ=0; таблица субстратов (семантический/квантовый/теоретико-числовой/крипто); MVR цикл F — 10/10 AXIOM CONFIRMED (SymPy+Z3+NumPy+SciPy+qiskit) |
| [mathematical-treatise/VOLUME_VIII_IDEAL_QUBIT_SUBSTRATE_QUANTUM_PC.md](mathematical-treatise/VOLUME_VIII_IDEAL_QUBIT_SUBSTRATE_QUANTUM_PC.md) | **Трактат Том VIII:** ИДЕАЛЬНЫЙ КУБИТНЫЙ СУБСТРАТ — POLER Quantum PC (`pqc qc/algo/substrate`): инструмент «квантовый компьютер точнее физических кубитов» (нулевая ошибка гейтов/считывания, отсутствие декогеренции); точное кольцо ℤ[1/√2, i] — бит-в-бит амплитуды Clifford+T; QCASM-lite; оракулы-привилегии владельца вектора состояния (Grover без синтеза T-гейтов); субстрат УДЕ §2.2 с γ-прецессией (Thm G.1: роторная работа ровно 0 при F=H) и SCF; MVR цикл G — 19/19 AXIOM CONFIRMED | (+ §8: цикл G-продолжение — поиск периода/SMT-цикл H, Gottesman–Knill, шум vs qiskit-цикл I, .poler-контейнер)
| [MVR_PROTOCOL.md](MVR_PROTOCOL.md) | **POLER-MVR-v3 — протокол тотальной инструментальной верификации:** фазы 0–6 (археология источника → CAS/SMT → побитовая сверка с кодом → тест/бенчмарк → фиксация аксиомы), жизненный цикл инструментов LOADED/STANDBY/EVICTED, трассировка theorem_id → verifier → commit → file#L |
| [../tools/verifiers/REGISTRY.md](../tools/verifiers/REGISTRY.md) | **Реестр верификаторов:** циклы A–F + A-финал (Z3/SymPy/NumPy/Zig-golden/qiskit против кода), Rust-тесты, рождённые верификацией, честные находки (Σθ-неинвариантность precess_step, семантика fp.eq, 8-значная Δφ-таблица Φ, паритет-асимметрия pndMix) |
| [algebra_of_sense_trit5.md](algebra_of_sense_trit5.md) | Алгебра смысла и кодек Trit5 (краткая версия, см. THEORY.md §6) |
| [terminal-gateway-architecture.md](terminal-gateway-architecture.md) | Terminal Gateway v0.22–0.28: двойной контур исполнения, sandbox, root broker |
| [native-retrieval-analysis.md](native-retrieval-analysis.md) | Разбор GNU grep / text-splitter → дизайн слоёв 0/B/S |
| [future-streaming-archives.md](future-streaming-archives.md) | Zero-Storage Streaming Archives (дизайн-нок будущего; локальное основание исполнено в M4.6 `src/archive/` — grep `--archives`, `--archive-list`, селектор «архив::запись») |
| [research/dialogue_tool_vs_ai.md](research/dialogue_tool_vs_ai.md) | Ключевой тезис «инструмент, не ИИ» (основа PLAN_POLER_V2 Part B) |
| [research/](research/) | SOTA-сурвеи 2026: semantic search, code+agentic, RAG+KG, streaming NLP |
| [sources-archive/](sources-archive/) | ZIP: 299 первоисточников «Когнитивная архитектура семантического резонанса» — полная интеллектуальная история POLER (карта тем — в HISTORY.md, приложение A) |
| [chat_dialogue.md](chat_dialogue.md) | Санитизированный транскрипт сессии-марафона 2026-09-03…15 (10 МБ; выжимка — HISTORY.md; сам файл — первоисточник, не документация) |

## Корневые документы

| Документ | Тема |
|---|---|
| [../README.md](../README.md) | Чейнджлог и витрина возможностей (новейшие релизы сверху) |
| [../PLAN_POLER_V2.md](../PLAN_POLER_V2.md) | Мастер-план v2.0: 12 разделов + Parts B–F (стратегия суверенного стека) |
| [../FUTURE_ROADMAP.md](../FUTURE_ROADMAP.md) | Дальняя дорога: физика ПК, Streaming Archives, монетизация |
| [../SKILL.md](../SKILL.md) | Манифест скилла poler-engine для ИИ-агентов (принципы, сборка, CLI) |
| [../AGENT.md](../AGENT.md) | Протокол агента репозитория (context-free resilience) |
| [../AGENT_STATE.md](../AGENT_STATE.md) | Машиночитаемый вектор движения: current_task → next_task |
| [../TERMS.md](../TERMS.md) | Юридические термины использования |

## Порядок чтения для нового агента/разработчика

1. `SKILL.md` — что это и чего делать нельзя (принципы).
2. `AGENT.md` + `AGENT_STATE.md` — протокол работы и текущее состояние.
3. `ARCHITECTURE.md` — как устроена система сейчас.
4. `MODULES.md` — куда смотреть по конкретной подсистеме.
5. `HISTORY.md` — почему система стала такой (контекст решений).
6. `THEORY.md` — математика, на которой всё держится.
7. `TESTING.md` + `CONTRIBUTING.md` — перед первым PR.

## Устаревшее (читать с оговорками)

- `companion-bridge-design.md` — описывает NotebookLM/Google-мост, **удалённый
  в v2.0** (суверенный стек). Оставлен как исторический дизайн-документ.
- Нижние секции `README.md` («Структура проекта», счётчики тестов) частично
  описывают v0.3.x — актуальную структуру смотрите в `MODULES.md`.
- `INSTALL.md` до сентября 2026 содержал инструкции Google OAuth — переписан.

## Поддержка актуальности

Документация — часть определения Done для каждого «кирпича» (см.
CONTRIBUTING.md). Правило простое: **изменил публичный API/формат/CLI — обновил
соответствующий документ в том же коммите**. Карта владения:

- новый флаг CLI → `CLI.md` (+ `--help` сам обновится из doc-комментариев);
- новый модуль или изменение pub-API → `MODULES.md` (+ `//!`-шапка модуля);
- изменение бинарного формата → `formats/*.md` с указанием новой версии магии;
- новый релиз/кирпич → `README.md` (верхняя секция) + `HISTORY.md`;
- изменение принципов → `SKILL.md` (согласовывать с владельцем).

//! RQ17: L5-генерация — первые слова и рассуждение квантово-фазового
//! разума.
//!
//! Замыкает цикл **авторегрессии**: до сих пор решётка только училась
//! (ingest → born-кристаллизация → транспорт волны по руслам `J`);
//! теперь она **говорит**. Сенсорный кодировщик односторонен
//! (`токен → fnv1a64 → координата`), поэтому генерации нужна обратная
//! карта — [лексикон](LexiconBuilder) кристалла, накопленный во время
//! обучения: доминантный токен на координату.
//!
//! ## RQ22: фокусировка волны и релевантность
//!
//! RQ17–RQ21 оставили волну свободной: ассоциации ведут речь куда
//! угодно, и ответ на вопрос мог уйти в посторонние ветки решётки.
//! RQ22 направляет волну **от дуг вопроса**:
//!
//! ```text
//!   промпт ──▶ p₀: дуги вопроса зажжены (TF-IDF + кинетика внимания)
//!        │        аттрактор = дуги свидетельства после ε-ворот
//!        ▼
//!   мышление: Π_Λ(e^{Δt·J} p) — транспорт течёт по зажжённым дугам
//!        ▼
//!   речь: лотерея × фокус-множитель BFS-дистанции до аттрактора
//!        │   dist 0 → ×4, 1 → ×3, 2 → ×2, 3..radius → ×1,
//!        │   вне радиуса → ×0 (посторонние ветки отсечены)
//!        │   русл в радиусе нет → шаг блуждания (честный фолбэк)
//!        ▼
//!   релевантность = доля шагов в фокусе (телеметрия ответа)
//! ```
//!
//! Грамматика **самоподкрепляется**: удачно вставленный коннектор
//! (мост RQ21, за которым гарантированно следует знаменательное
//! слово) получает второй направленный свидетель — русло коннектора
//! насыщается за одну реплику, а кинетика внимания делает его путём
//! волны. Мозг учится говорить связно через собственную речь
//! (метрика: `syntax_bridges_density` — мостов на 100 слов).
//!
//! ## Физика речи: Born-блуждание по руслам J
//!
//! ```text
//!   промпт ──▶ слушание (кольцо гироскопа: каналы вопроса)
//!        │         + кинетика внимания (дуги вопроса разогнаны)
//!        ▼
//!   мышление: K × p ← Π_Λ(e^{Δt·J} p)   [волна идёт по руслам]
//!        │
//!        ▼
//!   ┌─ шаг речи ──────────────────────────────────────────┐
//!   │ кольцо контекста → билеты по нисходящим руслам J    │
//!   │   вес = 1 + |крутящий момент|: устоявшаяся связь    │
//!   │   говорит (1), напряжённая — кричит (2)             │
//!   │ × синтаксис (RQ21) × фокус аттрактора (RQ22)        │
//!   │ Born-лотерея (целые билеты, ноль FPU) → координата  │
//!   │ лексикон: координата → слово                        │
//!   │ эмиссия = сенсорное событие: gyro.observe(coord,±1) │
//!   │ Born-измерение фазы → градиент → момент (инерция)   │
//!   │ шаг транспорта — волна продолжает течь              │
//!   └─────────────────────────────────────────────────────┘
//!        │
//!        ▼
//!   аттрактор: русел из кольца нет — мысль завершена
//! ```
//!
//! **Направление речи = синтаксис.** Русла `J` направлены по порядку
//! следования слов в обучающем потоке: канал «квант → фаза» означает,
//! что в свидетельстве «квант» предшествовал «фазе». Блуждание по
//! нисходящим руслам воспроизводит усвоенный порядок слов — грамматика
//! не хранится отдельно, она **топологический инвариант** русел.
//!
//! **Каскад живости.** Шаг речи пробует источники билетов по убыванию
//! осмысленности: нисходящие русла из кольца (мысль течёт вперёд) →
//! восходящие (возврат к предыдущей мысли) → кинетические дуги вне
//! кольца (внутренний голос). Пусто на всех уровнях — сходимость.
//!
//! **Анти-заикание.** Координата, эмитированная на последних
//! `repeat_veto` шагах, из лотереи исключается: волна обязана течь,
//! а не вибрировать на месте.
//!
//! ## Пример
//!
//! ```
//! use pqc::generate::{GeneratorConfig, L5Generator};
//! use pqc::gyro_lattice::QuantizedGyroCurriculum;
//!
//! // Живой мозг: обучаем движок на микрокорпусе…
//! let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
//! let corpus = "квант фаза решётка квант фаза born шаг трит решётка \
//!               квант фаза решётка момент импульс фаза";
//! for _ in 0..3 {
//!     qc.ingest(corpus, 1).unwrap();
//! }
//!
//! // …и спрашиваем его: генерация из промпта.
//! let cfg = GeneratorConfig::default();
//! let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
//! let rep = gen.generate("квант").unwrap();
//! // Всё, что произнесено, — слова из обученного лексикона.
//! for step in &rep.steps {
//!     assert!(rep.text.contains(&step.token));
//! }
//! ```

use std::collections::VecDeque;
use std::time::Instant;

use pqw::checksum::fnv1a64;
use pqw::lexicon::Lexicon;
use pqw::stream::tokenize;

use crate::archetype_lattice::DEFAULT_BRIDGE_EPS;
use crate::error::Result;
use crate::gyro_lattice::{QuantizedGyroCurriculum, SIN_LUT, TransportMode};
use crate::rng::Rng;
use crate::syntax_bridge::{is_cyrillic_token, syntax_class, SyntaxChain, SyntaxClass};
use crate::syntax_unfolder::morpheme_at;

/// Потолок длины токена для лексикона: прогоны длиннее — шум
/// (канонический предел секции — 255 байт, уходим с запасом).
pub const LEXICON_TOKEN_MAX: usize = 64;

/// Радиус фокуса по умолчанию в CLI (маршрутизация волны включена).
pub const DEFAULT_FOCUS_RADIUS: usize = 3;

/// Радиус метрики релевантности при выключенной маршрутизации:
/// телеметрия считается всегда — показывает, насколько свободная
/// речь держится темы.
const FOCUS_METRIC_RADIUS: usize = 3;

/// Максимум радиуса фокуса (BFS по руслам — радиус больше 8
/// обесценивает отсечку: почти вся решётка достигаема).
pub const FOCUS_RADIUS_MAX: usize = 8;

// ===================== Лексикон-строитель =====================

/// Накопитель лексикона во время обучения: доминантный токен на
/// координату.
///
/// Плотная раскладка без HashMap (конвенция RQ16): `freq[i]` — сколько
/// раз координата `i` получала свидетельство, `best[i]` — слово с
/// наибольшей частотой. Коллизия хеша разрешается доминантностью —
/// как полюс МакВини выбирает выровненную фазу из суперпозиции
/// вкладов. Переживает рестарт через секцию `LEXI` контейнера v4.
pub struct LexiconBuilder {
    d: u32,
    /// Счётчик наблюдений координаты (частота доминирования).
    freq: Vec<u32>,
    /// Вес, при котором текущее слово стало доминантой (ничья хранит
    /// старое — детерминизм).
    best_freq: Vec<u32>,
    best: Vec<String>,
    seen: u64,
}

impl LexiconBuilder {
    /// Строитель под размерность `d_pol ≥ 1`.
    pub fn new(d_pol: u32) -> LexiconBuilder {
        let d = d_pol as usize;
        LexiconBuilder {
            d: d_pol,
            freq: vec![0u32; d],
            best_freq: vec![0u32; d],
            best: vec![String::new(); d],
            seen: 0,
        }
    }

    /// Размерность.
    pub fn d_pol(&self) -> u32 {
        self.d
    }

    /// Токенов наблюдено всего.
    pub fn tokens_seen(&self) -> u64 {
        self.seen
    }

    /// Одно наблюдение: токен → координата (тот же хеш, что у сенсорного
    /// кодировщика). Прогоны длиннее [`LEXICON_TOKEN_MAX`] — шум, мимо.
    ///
    /// Доминанта: слово сменяется только перевесом частоты
    /// (`freq > best_freq`); ничья хранит старое — детерминизм.
    pub fn observe(&mut self, token: &str) {
        if token.is_empty() || token.len() > LEXICON_TOKEN_MAX {
            return;
        }
        let h = fnv1a64(token.as_bytes());
        let i = (h % self.d as u64) as usize;
        self.seen += 1;
        self.freq[i] += 1;
        // Доминанта: слово сменяется только перевесом частоты
        // (`freq > best_freq`; ничья хранит старое — детерминизм).
        // Пустой слот имеет best_freq = 0 — первое наблюдение
        // перевешивает автоматически.
        if self.freq[i] > self.best_freq[i] {
            self.best[i] = token.to_string();
            self.best_freq[i] = self.freq[i];
        }
    }

    /// Токены текста (тот же токенизатор, что у кодировщика).
    pub fn observe_text(&mut self, text: &str) {
        for token in tokenize(text) {
            self.observe(token);
        }
    }

    /// Координат со словом.
    pub fn len(&self) -> usize {
        self.best.iter().filter(|t| !t.is_empty()).count()
    }

    /// Есть ли хоть одно слово.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Слово координаты (доминантный токен).
    pub fn token_of(&self, coord: u32) -> Option<&str> {
        let i = coord as usize;
        if i >= self.best.len() {
            return None;
        }
        let t = &self.best[i];
        if t.is_empty() {
            None
        } else {
            Some(t.as_str())
        }
    }

    /// N-я непустая координата (порядок плотный, детерминированный) —
    /// выбор затравки свободной речи.
    pub fn coord_at(&self, n: usize) -> Option<u32> {
        let mut k = 0usize;
        for (i, t) in self.best.iter().enumerate() {
            if t.is_empty() {
                continue;
            }
            if k == n {
                return Some(i as u32);
            }
            k += 1;
        }
        None
    }

    /// Готовый лексикон для контейнера v4 (`None`, если слов нет).
    pub fn finish(&self) -> Option<Lexicon> {
        let entries: Vec<(u32, String)> = self
            .best
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_empty())
            .map(|(i, t)| (i as u32, t.clone()))
            .collect();
        Lexicon::new(entries, self.d).ok()
    }

    /// Влить поднятый из контейнера лексикон (resume): влитые слова
    /// получают доминантный вес 1 — смена только перевесом (два
    /// свежих наблюдения другого токена).
    pub fn absorb(&mut self, lex: &Lexicon) {
        for &(coord, ref token) in lex.entries() {
            let i = coord as usize;
            if i >= self.best.len() || token.is_empty() {
                continue;
            }
            if self.best[i].is_empty() {
                self.best[i] = token.clone();
                self.freq[i] = self.freq[i].max(1);
                self.best_freq[i] = 1;
            }
        }
    }
}

// ===================== Конфигурация генератора =====================

/// Конфигурация L5-генератора.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneratorConfig {
    /// Шагов «мышления» (транспорт L5) после слушания промпта.
    pub think_steps: usize,
    /// Потолок эмиссий (слов + морфем).
    pub max_tokens: usize,
    /// Размер кольца контекста речи (рабочая память).
    pub window: usize,
    /// Сид Born-лотереи.
    pub seed: u64,
    /// Снять моментные ворота: чистый оператор `Π_Λ(e^{Δt·J} p)`.
    /// Автоматически включается, если промпт не зажёг ни одной дуги.
    pub free: bool,
    /// AOT-фолбэк: координаты вне лексикона декодируются морфемами
    /// [`crate::syntax_unfolder`] (синтаксический скелет речи).
    pub morphemes: bool,
    /// Анти-заикание: координата исключается из лотереи на `N` шагов
    /// после эмиссии (`0` — выключено).
    pub repeat_veto: usize,
    /// Архетипический мост RQ18: связывать вопрос с памятью через
    /// нелинейное `a ⊗_ε lattice` (уровень концептуального прыжка
    /// в лотерее речи).
    pub bridge: bool,
    /// Порог энергетического гейта моста `ε ∈ (0, 1]`
    /// (default [`crate::archetype_lattice::DEFAULT_BRIDGE_EPS`]).
    pub bridge_eps: f64,
    /// Грамматические мосты RQ21: взвешивание синтаксических
    /// цепочек лотереи (союзы/предлоги/связки) + вставка
    /// детерминированных коннекторов в зияющие облака слов.
    /// Выключено по умолчанию — чистая ассоциативная топология RQ17
    /// (CLI включает: `--no-syntax` снимает).
    pub syntax: bool,
    /// Радиус фокуса волны RQ22: маршрутизация речи от дуг вопроса.
    /// `0` — выключено (свободное блуждание RQ17/RQ21);
    /// `1..=[`FOCUS_RADIUS_MAX`]` — кандидаты лотареи вне BFS-радиуса
    /// от аттрактора вопроса отсекаются (вес ×0), близкие — усиливаются
    /// (dist 0 → ×4, 1 → ×3, 2 → ×2). Библиотечный дефолт — `0`
    /// (совместимость RQ17); CLI включает `3` (`--no-focus` снимает).
    pub focus_radius: usize,
    /// Самоподкрепление грамматики RQ22: удачно вставленные
    /// коннекторы (мосты RQ21, за которыми следует знаменательное
    /// слово) укрепляют свои русла `J` — второй направленный свидетель
    /// (насыщение русла за одну реплику) + кинетика внимания
    /// (коннектор — путь волны). Библиотечный дефолт — `false`;
    /// CLI включает (`--no-reinforce` снимает).
    pub reinforce: bool,
    /// Автобиографическая затравка RQ23: кольцо контекста речи
    /// стартует из хвоста контекст-рефлекса `W` (последние слова
    /// предыдущей сессии диалога) — волна продолжает нить разговора
    /// вместо холодного старта. Нить занимает половину окна,
    /// вопрос — вторую половину; пустой промпт заполняет кольцо
    /// следом целиком (свободная речь как продолжение монолога).
    /// Библиотечный дефолт — `false` (RQ17-совместимость); CLI
    /// включает (`--fresh` снимает).
    pub autobiographical: bool,
}

impl Default for GeneratorConfig {
    fn default() -> GeneratorConfig {
        GeneratorConfig {
            think_steps: 4,
            max_tokens: 64,
            window: 8,
            seed: 42,
            free: false,
            morphemes: false,
            repeat_veto: 1,
            bridge: true,
            bridge_eps: DEFAULT_BRIDGE_EPS,
            syntax: false,
            focus_radius: 0,
            reinforce: false,
            autobiographical: false,
        }
    }
}

// ===================== Телеметрия =====================

/// Источник билетов шага речи.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TicketSource {
    /// Нисходящее русло из кольца контекста — мысль течёт вперёд.
    Flow,
    /// Восходящее русло — возврат к предыдущей мысли.
    Backtrack,
    /// Архетипический мост RQ18: концептуальный прыжок через
    /// `a ⊗_ε lattice` — структурный изоморфизм там, где прямых
    /// русел J нет (метафора, глубокая аналогия).
    Archetype,
    /// Кинетическая дуга вне кольца — внутренний голос.
    Kinetic,
    /// Грамматический мост RQ21: детерминированный коннектор
    /// синтаксиса (союз/предлог/связка из встроенной таблицы
    /// RU/EN) — вставлен между знаменательными словами, чтобы
    /// облако ассоциаций становилось предложением.
    Bridge,
}

/// Одна эмиссия (квант речи).
#[derive(Clone, Debug, PartialEq)]
pub struct BornStep {
    /// Координата решётки, выбранная лотереей.
    pub coord: u32,
    /// Произнесённое слово (лексикон или AOT-морфема).
    pub token: String,
    /// Источник билетов.
    pub source: TicketSource,
    /// Живых билетов на этом шаге (ширина волны).
    pub tickets: usize,
    /// Born-исход измерения фазы координаты (ложь — полюс +1).
    pub born_bit: bool,
    /// Перещёлкиваний решётки транспортом на этом шаге.
    pub moved: usize,
    /// «Плавающий» θ-сдвиг транспорта на этом шаге.
    pub theta_shift: f64,
    /// Слово пришло из AOT-морфем, а не из лексикона.
    pub morpheme: bool,
}

/// Отчёт генерации.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationReport {
    /// Токенов в промпте.
    pub prompt_tokens: usize,
    /// Дуг TF-IDF свидетельства промпта (после ε-ворот).
    pub prompt_arcs: usize,
    /// Зажиганий момента свидетельством промпта.
    pub ignited: usize,
    /// Ворота сняты автоматически (нулевая кинетика после слушания).
    pub auto_free: bool,
    /// Мост `⊗_ε`: энергия пересечения архетипа промпта с памятью
    /// (`0.0`, если мост выключен или промпт пуст).
    pub bridge_energy: f64,
    /// Мост: дуг пересечения (согласие + конфликт).
    pub bridge_co: usize,
    /// Мост: согласных полюсов (структурный резонанс).
    pub bridge_resonance: usize,
    /// Мост: встречных полюсов (аннигиляция — открытые вопросы).
    pub bridge_conflict: usize,
    /// Мост: гейт открыт `E ≥ ε` — изоморфизм найден.
    pub bridge_resonant: bool,
    /// Перещёлкиваний решётки за фазу мышления.
    pub think_moved: usize,
    /// θ-сдвиг за фазу мышления.
    pub think_theta_shift: f64,
    /// Эмиссии в порядке речи.
    pub steps: Vec<BornStep>,
    /// Собранный текст (слова через пробел).
    pub text: String,
    /// Координат вне лексикона пропущено (мимо морфемного фолбэка).
    pub skipped_unseen: usize,
    /// Сходимость: живых билетов нет, ĤΨ = 0 — мысль завершена.
    pub converged: bool,
    /// Вырожденный цикл (волна вибрирует между двумя координатами).
    pub cycled: bool,
    /// Вставлено грамматических мостов RQ21 (коннекторы синтаксиса).
    pub bridges: usize,
    /// Пик облака знаменательных слов (телеметрия связности;
    /// с синтаксисом не превышает [`crate::syntax_bridge::BRIDGE_RUN`]).
    pub syntax_run_max: usize,
    /// RQ22: плотность грамматических мостов — вставленных коннекторов
    /// на 100 сгенерированных слов (метрика связности речи; здоровая
    /// связная речь ≈ 15–25).
    pub syntax_bridges_density: f64,
    /// RQ22: коннекторов подкреплено (русла J укреплены —
    /// самоподкрепление грамматики).
    pub reinforced: usize,
    /// RQ22: дуг вопроса в аттракторе (семантическое ядро промпта).
    pub attractor_arcs: usize,
    /// RQ22: радиус фокуса, применённый при маршрутизации
    /// (`0` — маршрутизация выключена).
    pub focus_radius: usize,
    /// RQ22: маршрутизация волны была активна (аттрактор непуст).
    pub focus_active: bool,
    /// RQ22: шагов речи в фокусе (BFS-дистанция до аттрактора
    /// ≤ радиуса метрики).
    pub focused_steps: usize,
    /// RQ22: шагов речи вне фокуса (блуждание/фолбэк).
    pub wander_steps: usize,
    /// RQ22: релевантность ответа — доля шагов в фокусе `[0, 1]`
    /// (пустая речь — `1.0`: нечему блуждать).
    pub relevance: f64,
    /// Сквозная задержка.
    pub elapsed: std::time::Duration,
}

// ===================== L5-генератор =====================

/// Born-блуждатель по руслам `J`: замыкает авторегрессионный цикл
/// поверх обученного [`QuantizedGyroCurriculum`].
///
/// Заимствует движок **мутабельно**: генерация — не наблюдение, а
/// физический процесс (зажигание момента, транспорт, сенсорная
/// обратная связь эмиссий). Плотные буферы билетов и список смежности
/// русел строятся один раз на конструкторе и переиспользуются между
/// шагами — ноль аллокаций на квант речи (кроме растущего `steps`).
pub struct L5Generator<'a> {
    qc: &'a mut QuantizedGyroCurriculum,
    cfg: GeneratorConfig,
    rng: Rng,
    /// Кольцо контекста речи (последние координаты, обратный порядок
    /// не важен: все члены кольца равноправные источники русел).
    ring: VecDeque<u32>,
    /// Смежность русел: `adj[c] = [(сосед, +1 — русло c→сосед,
    /// −1 — русело сосед→c)]`. Строится один раз из каналов J.
    adj: Vec<Vec<(u32, i8)>>,
    /// Речевые билеты архетипического моста RQ18: `(дуга, вес)` —
    /// согласие 2, конфликт 3. Пуст, если гейт заперт или мост выключен.
    bridge: Vec<(u32, u32)>,
    /// Плотные билеты: нисходящие (вперёд по речи).
    down: Vec<u32>,
    /// Плотные билеты: восходящие (возврат).
    up: Vec<u32>,
    /// Материализованный список кандидатов текущего шага.
    candidates: Vec<(u32, u32)>,
    /// Анти-заикание: последние эмитированные координаты.
    veto: VecDeque<u32>,
    /// Синтаксическая цепочка RQ21: класс последней эмиссии и длина
    /// облака знаменательных слов (взвешивание лотареи + мосты).
    chain: SyntaxChain,
    /// Алфавит речи (кириллица ↔ латиница) по последнему слову —
    /// таблица мостов RU/EN.
    script_cyrillic: Option<bool>,
    /// RQ22: BFS-дистанция каждой координаты до аттрактора вопроса
    /// (по руслам J, оба направления; `u8::MAX` — вне радиуса).
    /// Пуста, когда фокус выключен или аттрактора нет.
    dist: Vec<u8>,
    /// RQ22: маршрутизация волны активна (аттрактор непуст и
    /// `focus_radius > 0`).
    focus_on: bool,
    /// RQ22: радиус метрики релевантности (радиус маршрутизации при
    /// активном фокусе, иначе `FOCUS_METRIC_RADIUS`).
    metric_radius: usize,
}

impl<'a> L5Generator<'a> {
    /// Новая сессия речи поверх движка. Строит смежность русел —
    /// O(каналы), один раз.
    ///
    /// RQ23: при `cfg.autobiographical` кольцо контекста получает
    /// затравку из хвоста контекст-рефлекса `W` движка (последние
    /// ⌈window/2⌉ слов предыдущей сессии) — волна продолжает нить
    /// разговора.
    pub fn new(qc: &'a mut QuantizedGyroCurriculum, cfg: GeneratorConfig) -> Result<Self> {
        let d = qc.d_pol() as usize;
        let mut adj: Vec<Vec<(u32, i8)>> = vec![Vec::new(); d];
        for (i, j, w) in qc.channels() {
            let dir = if w > 0.0 { 1i8 } else { -1 };
            // Русло i→j: из i — нисходящее к j, из j — восходящее к i.
            adj[i as usize].push((j, dir));
            adj[j as usize].push((i, -dir));
        }
        let window = cfg.window.max(1);
        let focus_on = cfg.focus_radius > 0;
        let mut ring = VecDeque::with_capacity(window + 1);
        // Автобиографическая затравка: нить прошлого диалога —
        // половина кольца (вторая половина достанется вопросу).
        if cfg.autobiographical {
            let take = window.div_ceil(2);
            for &(c, _) in qc.reflex().trail_tail(take) {
                if (c as usize) < d {
                    ring.push_back(c);
                }
            }
        }
        Ok(L5Generator {
            qc,
            cfg,
            rng: Rng::seed_from_u64(cfg.seed),
            ring,
            adj,
            bridge: Vec::new(),
            down: vec![0u32; d],
            up: vec![0u32; d],
            candidates: Vec::new(),
            veto: VecDeque::with_capacity(cfg.repeat_veto + 1),
            chain: SyntaxChain::new(),
            script_cyrillic: None,
            dist: Vec::new(),
            focus_on,
            metric_radius: FOCUS_METRIC_RADIUS,
        })
    }

    /// Полный цикл: слушание → зажигание → мышление → речь.
    pub fn generate(&mut self, prompt: &str) -> Result<GenerationReport> {
        let t0 = Instant::now();
        let mut report = GenerationReport {
            prompt_tokens: 0,
            prompt_arcs: 0,
            ignited: 0,
            auto_free: false,
            bridge_energy: 0.0,
            bridge_co: 0,
            bridge_resonance: 0,
            bridge_conflict: 0,
            bridge_resonant: false,
            think_moved: 0,
            think_theta_shift: 0.0,
            steps: Vec::new(),
            text: String::new(),
            skipped_unseen: 0,
            converged: false,
            cycled: false,
            bridges: 0,
            syntax_run_max: 0,
            syntax_bridges_density: 0.0,
            reinforced: 0,
            attractor_arcs: 0,
            focus_radius: 0,
            focus_active: false,
            focused_steps: 0,
            wander_steps: 0,
            relevance: 1.0,
            elapsed: std::time::Duration::ZERO,
        };

        // ---- Фаза A: слушание промпта ----
        // Токены входят в кольцо гироскопа (каналы вопроса копятся),
        // свидетельство TF-IDF зажигает момент — БЕЗ born-кристаллизации:
        // вопрос — вход, а не знание. RQ22: дуги свидетельства —
        // семантический аттрактор волны (p₀).
        let (attractor, ignited, tokens) = self.qc.listen_ignite_arcs(prompt);
        report.prompt_tokens = tokens;
        report.prompt_arcs = attractor.len();
        report.ignited = ignited;
        report.attractor_arcs = attractor.len();

        // Кольцо контекста: координаты промпта в порядке появления,
        // без повторов, с потолком окна.
        let window = self.cfg.window.max(1);
        let mut seen_ring: Vec<u32> = Vec::with_capacity(window);
        for token in tokenize(prompt) {
            // Алфавит речи (RQ21): таблица мостов следует за промптом.
            if self.script_cyrillic.is_none() {
                self.script_cyrillic = Some(is_cyrillic_token(token));
            }
            let coord =
                (fnv1a64(token.as_bytes()) % self.qc.d_pol() as u64) as u32;
            if !seen_ring.contains(&coord) {
                seen_ring.push(coord);
            }
        }
        for &c in seen_ring.iter().take(window) {
            self.ring.push_back(c);
        }
        // RQ23: нить + вопрос делят кольцо — вопрос вытесняет старейшие
        // слова нити, но не наоборот (никогда не превышаем окно).
        while self.ring.len() > window {
            self.ring.pop_front();
        }
        // Пустой промпт — свободная речь. RQ23: автобиографическая
        // затравка приоритетнее — свободная речь как продолжение
        // монолога (кольцо уже несёт хвост следа из конструктора);
        // затравка из лексикона — только холодный старт без следа.
        if self.ring.is_empty() {
            let n = self.qc.lexicon_len();
            if n > 0 {
                let pick = (self.rng.next_u64() % n as u64) as usize;
                if let Some(c) = self.qc.lexicon_coord_at(pick) {
                    self.ring.push_back(c);
                    // Алфавит затравки задаёт таблицу мостов свободной речи.
                    if self.script_cyrillic.is_none() {
                        self.script_cyrillic = self
                            .qc
                            .lexicon_token(c)
                            .map(is_cyrillic_token);
                    }
                }
            }
        }

        // ---- Фаза A1: аттрактор вопроса (RQ22) ----
        // Дуги TF-IDF свидетельства — семантическое ядро вопроса. Если
        // вопрос мозгу незнаком (дуг нет), аттрактором становятся
        // сырые координаты токенов — фокус держится хотя бы на
        // окрестности вопроса. Пустой промпт — аттрактора нет:
        // свободная речь блуждает по-честному.
        let mut attractor = attractor;
        if attractor.is_empty() && !seen_ring.is_empty() {
            attractor = seen_ring.clone();
            report.attractor_arcs = attractor.len();
        }
        if !attractor.is_empty() {
            let radius = self.cfg.focus_radius;
            self.metric_radius = if radius > 0 { radius } else { FOCUS_METRIC_RADIUS };
            if self.focus_on {
                // BFS по руслам J от аттрактора: дистанция каждой
                // достижимой координаты (обе стороны русла — волна
                // течёт в обе стороны).
                self.dist = Self::attractor_distances(&self.adj, &attractor, radius);
                report.focus_active = true;
                report.focus_radius = radius;
            } else {
                // Маршрутизация выключена — метрика релевантности
                // всё равно считается (насколько свободная речь
                // держится темы).
                self.dist = Self::attractor_distances(
                    &self.adj,
                    &attractor,
                    self.metric_radius,
                );
            }
        }

        // ---- Фаза A2: архетипический мост (RQ18) ----
        // Нелинейная интерференция `a ⊗_ε lattice` ДО мышления: момент
        // внимания на дугах пересечения разгоняет волну к структурному
        // изоморфизму — мышление потечёт к мосту. Билеты (согласие 2,
        // конфликт 3) ждут своей очереди в лотерее речи.
        if self.cfg.bridge {
            let arch = self.qc.prompt_archetype(prompt);
            let brep = self.qc.archetype_bridge(&arch, self.cfg.bridge_eps)?;
            report.bridge_energy = brep.energy;
            report.bridge_co = brep.co_support;
            report.bridge_resonance = brep.resonance;
            report.bridge_conflict = brep.conflict;
            report.bridge_resonant = brep.resonant;
            self.bridge = brep.tickets;
        }

        // ---- Фаза B: мышление ----
        // Моментных ворот нет, если промпт не зажёг кинетики: мозг,
        // не отозвавшийся на вопрос, всё равно говорит чистым потоком J.
        let mut free = self.cfg.free;
        if !free && self.qc.momentum_kinetic() == 0 {
            free = true;
            report.auto_free = true;
        }
        for _ in 0..self.cfg.think_steps {
            let s = self.qc.reasoning_step_mode(if free {
                TransportMode::Free
            } else {
                TransportMode::Gated
            })?;
            report.think_moved += s.moved;
            report.think_theta_shift += s.theta_shift;
        }

        // ---- Фаза C: речь ----
        let mut consecutive_unseen = 0usize;
        while report.steps.len() < self.cfg.max_tokens {
            // Вырожденный цикл: ABAB — волна вибрирует между двумя
            // координатами, топливо есть, смысла нет.
            if report.steps.len() >= 4 {
                let c: Vec<u32> = report.steps[report.steps.len() - 4..]
                    .iter()
                    .map(|s| s.coord)
                    .collect();
                if c[0] == c[2] && c[1] == c[3] && c[0] != c[1] {
                    report.cycled = true;
                    break;
                }
            }

            let picked = self.born_lottery();
            let Some((coord, source, tickets)) = picked else {
                report.converged = true;
                break;
            };

            // Декодирование: лексикон → AOT-морфема → пропуск.
            let (token, morpheme) = match self.qc.lexicon_token(coord) {
                Some(t) => (t.to_string(), false),
                None if self.cfg.morphemes => match morpheme_at(coord as usize % 256) {
                    Some(m) => (m, true),
                    None => {
                        report.skipped_unseen += 1;
                        consecutive_unseen += 1;
                        self.push_ring(coord);
                        if consecutive_unseen >= 3 {
                            report.converged = true;
                            break;
                        }
                        continue;
                    }
                },
                None => {
                    report.skipped_unseen += 1;
                    consecutive_unseen += 1;
                    self.push_ring(coord);
                    if consecutive_unseen >= 3 {
                        report.converged = true;
                        break;
                    }
                    continue;
                }
            };
            consecutive_unseen = 0;

            // ---- RQ21: грамматический мост ----
            // Облако знаменательных слов зияет, и голос лотереи — снова
            // знаменательный: перед ним вставляется коннектор синтаксиса
            // (союз/предлог/связка), выбранный детерминированно по цепи.
            // Мост — полноценный квант речи: Born-измерение, момент,
            // сенсорное событие (коннектор входит в кольцо гироскопа и
            // лексикон — грамматика прорастает в решётку). Мост — не
            // последнее слово: за ним всегда следует выбранное слово
            // (бюджет эмиссий проверен с запасом).
            if self.cfg.syntax
                && self.chain.bridge_due()
                && syntax_class(&token) == SyntaxClass::Content
                && report.steps.len() + 2 <= self.cfg.max_tokens
            {
                let cyr = self.script_cyrillic.unwrap_or(true);
                let (btoken, bclass) = self.chain.next_bridge(cyr);
                let bcoord =
                    (fnv1a64(btoken.as_bytes()) % self.qc.d_pol() as u64) as u32;
                let bp = self.qc.p_at(bcoord);
                let bbit = if bp > 0.0 {
                    false
                } else if bp < 0.0 {
                    true
                } else {
                    self.rng.next_f64() < 0.5
                };
                let bp_emp = if bbit { -1.0 } else { 1.0 };
                self.qc.momentum_feedback(bcoord, bp_emp - bp);
                let bsign: i8 = if bbit { -1 } else { 1 };
                self.qc.observe_event(bcoord, bsign);
                // RQ22: самоподкрепление грамматики — коннектор удачен
                // по построению (за ним в этой же итерации последует
                // выбранное знаменательное слово: бюджет проверен с
                // запасом). Второй направленный свидетель насыщает русла
                // коннектора сразу (кольцо ещё не содержит следующего
                // слова — реверсивных пар нет), а кинетика внимания
                // делает дугу коннектора путём волны.
                if self.cfg.reinforce {
                    self.qc.observe_reinforcement(bcoord, bsign);
                }
                self.qc.observe_lexicon(btoken);
                let bstats = self.qc.reasoning_step_mode(if free {
                    TransportMode::Free
                } else {
                    TransportMode::Gated
                })?;
                self.push_ring(bcoord);
                if !report.text.is_empty() {
                    report.text.push(' ');
                }
                report.text.push_str(btoken);
                report.steps.push(BornStep {
                    coord: bcoord,
                    token: btoken.to_string(),
                    source: TicketSource::Bridge,
                    tickets: 0,
                    born_bit: bbit,
                    moved: bstats.moved,
                    theta_shift: bstats.theta_shift,
                    morpheme: false,
                });
                report.bridges += 1;
                if self.cfg.reinforce {
                    report.reinforced += 1;
                }
                self.chain.push(bclass);
                self.script_cyrillic = Some(is_cyrillic_token(btoken));
                if self.cfg.repeat_veto > 0 {
                    self.veto.push_back(bcoord);
                    while self.veto.len() > self.cfg.repeat_veto {
                        self.veto.pop_front();
                    }
                }
            }

            // Born-измерение фазы координаты: полюса детерминированы,
            // суперпозиция честной монетой. Измерение — физический акт:
            // градиент p_emp − p зажигает момент (инерция речи).
            let p = self.qc.p_at(coord);
            let born_bit = if p > 0.0 {
                false
            } else if p < 0.0 {
                true
            } else {
                self.rng.next_f64() < 0.5
            };
            let p_emp = if born_bit { -1.0 } else { 1.0 };
            self.qc.momentum_feedback(coord, p_emp - p);

            // Эмиссия — сенсорное событие: слово входит в кольцо
            // гироскопа, русла от него копятся (авторегрессия замкнута).
            let sign: i8 = if born_bit { -1 } else { 1 };
            self.qc.observe_event(coord, sign);

            // Шаг транспорта: волна продолжает течь от произнесённого.
            let stats = self.qc.reasoning_step_mode(if free {
                TransportMode::Free
            } else {
                TransportMode::Gated
            })?;

            self.push_ring(coord);
            if !report.text.is_empty() {
                report.text.push(' ');
            }
            report.text.push_str(&token);
            report.steps.push(BornStep {
                coord,
                token,
                source,
                tickets,
                born_bit,
                moved: stats.moved,
                theta_shift: stats.theta_shift,
                morpheme,
            });

            // RQ22: телеметрия фокуса — держит ли шаг речь возле
            // аттрактора вопроса (BFS-дистанция по руслам J).
            if self.focus_step(coord) {
                report.focused_steps += 1;
            } else {
                report.wander_steps += 1;
            }

            // RQ21: цепочка синтаксиса поглощает эмиссию (морфемы AOT —
            // знаменательные: это кодовый скелет, не коннекторы).
            let class = if morpheme {
                SyntaxClass::Content
            } else {
                syntax_class(&report.steps.last().unwrap().token)
            };
            self.chain.push(class);
            self.script_cyrillic = Some(is_cyrillic_token(
                report.steps.last().unwrap().token.as_str(),
            ));

            // Анти-заикание.
            if self.cfg.repeat_veto > 0 {
                self.veto.push_back(coord);
                while self.veto.len() > self.cfg.repeat_veto {
                    self.veto.pop_front();
                }
            }
        }

        report.syntax_run_max = self.chain.max_run();
        // RQ22: плотность грамматических мостов — метрика связности
        // (мостов на 100 сгенерированных слов) и релевантность ответа
        // (доля шагов в фокусе аттрактора).
        let words = report.steps.len();
        report.syntax_bridges_density = if words > 0 {
            report.bridges as f64 * 100.0 / words as f64
        } else {
            0.0
        };
        let total = report.focused_steps + report.wander_steps;
        report.relevance = if total > 0 {
            report.focused_steps as f64 / total as f64
        } else {
            1.0
        };
        report.elapsed = t0.elapsed();
        Ok(report)
    }

    /// RQ22: шаг в фокусе аттрактора? BFS-дистанция координаты до дуг
    /// вопроса не превышает радиуса метрики. Без аттрактора (свободная
    /// речь) — `true`: нечему блуждать. Мосты-коннекторы не считаются:
    /// релевантность меряет, куда идёт **волна** (выборы лотареи),
    /// а не синтаксический клей.
    fn focus_step(&self, coord: u32) -> bool {
        if self.dist.is_empty() {
            return true;
        }
        let d = self.dist[coord as usize];
        d != u8::MAX && d as usize <= self.metric_radius
    }

    /// RQ22: BFS-дистанции от аттрактора по руслам `J` (обе стороны
    /// русла: волна течёт вперёд и возвращается). `u8::MAX` — координата
    /// за радиусом (недостижима). Аттрактор — дистанция 0. Радиус
    /// клампится до 250: `d + 1` не должен столкнуться с сентинелом
    /// `u8::MAX`.
    fn attractor_distances(
        adj: &[Vec<(u32, i8)>],
        attractor: &[u32],
        radius: usize,
    ) -> Vec<u8> {
        let radius = radius.min(250);
        let mut dist = vec![u8::MAX; adj.len()];
        let mut queue: VecDeque<u32> = VecDeque::new();
        for &c in attractor {
            let idx = c as usize;
            if idx < dist.len() && dist[idx] == u8::MAX {
                dist[idx] = 0;
                queue.push_back(c);
            }
        }
        while let Some(c) = queue.pop_front() {
            let d = dist[c as usize];
            // Не расширяем дальше радиуса: всё за ним — «далеко».
            if d as usize >= radius {
                continue;
            }
            for &(n, _dir) in &adj[c as usize] {
                if dist[n as usize] == u8::MAX {
                    dist[n as usize] = d + 1;
                    queue.push_back(n);
                }
            }
        }
        dist
    }

    /// RQ22: множитель фокуса координаты. В фокусированном проходе:
    /// дист. 0 → ×4, 1 → ×3, 2 → ×2, дальше в радиусе → ×1,
    /// за радиусом → ×0 (посторонняя ветка отсечена). В свободном
    /// проходе — ×1 всегда (чистая топология RQ17/RQ21).
    fn focus_gain(&self, coord: u32, focused: bool) -> u32 {
        if !focused || self.dist.is_empty() {
            return 1;
        }
        match self.dist[coord as usize] {
            0 => 4,
            1 => 3,
            2 => 2,
            d if d != u8::MAX => 1,
            _ => 0,
        }
    }

    /// Синтаксический множитель билета координаты (RQ21): класс слова
    /// из лексикона × состояние цепочки. Синтаксис выключен или
    /// координата вне лексикона — множитель 1 (чистая топология RQ17).
    fn syntax_boost(&self, coord: u32) -> u32 {
        if !self.cfg.syntax {
            return 1;
        }
        match self.qc.lexicon_token(coord) {
            Some(t) => self.chain.ticket_boost(syntax_class(t)),
            None => 1,
        }
    }

    /// Born-лотерея одного шага: каскад источников билетов +
    /// фокусировка волны RQ22.
    ///
    /// Лотерея семплирует **ассоциативную топологию**: вес билета
    /// нисходящего русла `(c → n)` — `1 + |k|`, где `k = s·sin(Δθ)` —
    /// крутящий момент транспорта. Устоявшаяся связь (полюса
    /// согласованы, `k = 0`) говорит с весом 1 — знание вспоминается
    /// и после сходимости; напряжённая связь (суперпозиция на конце,
    /// `|k| = 1`) кричит с весом 2 — мысль течёт туда, где не решено.
    /// Фазы меняются транспортом — распределение речи следует за
    /// волной: настоящая авторегрессия, а не случайный блуждатель.
    ///
    /// RQ22: при активной маршрутизации первый проход —
    /// **фокусированный**: веса русел и кинетики умножаются на
    /// фокус-множитель BFS-дистанции до аттрактора вопроса, за
    /// радиусом вес ×0 (посторонние ветки отсечены). Архетипический
    /// мост RQ18 фокусом не режется — это санкционированный прыжок
    /// (дуги пересечения `a ⊗_ε lattice` уже принадлежат вопросу).
    /// Если в фокусированном проходе не осталось ни одного билета
    /// (русл в радиусе нет) — честный шаг блуждания чистым проходом:
    /// молчание хуже отклонения.
    ///
    /// Возвращает `(координата, источник, всего билетов)` или `None`,
    /// если волна иссякла (русл из кольца нет).
    fn born_lottery(&mut self) -> Option<(u32, TicketSource, usize)> {
        if self.focus_on && !self.dist.is_empty() {
            if let Some(pick) = self.lottery_pass(true) {
                return Some(pick);
            }
            // Фокус пуст — шаг блуждания (русл в радиусе аттрактора
            // больше нет: русла исчерпаны вето и кольцом).
            return self.lottery_pass(false);
        }
        self.lottery_pass(false)
    }

    /// Один проход лотареи: `focused = true` — фокус-множители и
    /// отсечка посторонних веток; `false` — чистая топология
    /// RQ17/RQ21.
    fn lottery_pass(&mut self, focused: bool) -> Option<(u32, TicketSource, usize)> {
        // Уровень 1+2: русла из кольца контекста. Внимание = членство
        // в кольце (моментные ворота — домен транспорта, не речи).
        // RQ21: вес каждого русла умножается на синтаксический
        // множитель цепочки (`syntax_boost`) — топология остаётся
        // источником смысла, грамматика лишь перераспределяет шансы.
        // RQ22: поверх — фокус-множитель аттрактора (0 = ветка
        // отсечена).
        let (mut total_down, mut total_up) = (0u64, 0u64);
        self.candidates.clear();
        for &c in self.ring.iter() {
            for &(n, dir) in &self.adj[c as usize] {
                if self.veto.contains(&n) {
                    continue;
                }
                let fg = self.focus_gain(n, focused);
                if fg == 0 {
                    continue; // посторонняя ветка: билетов нет
                }
                let ci = self.qc.phase_code(c) as usize;
                let cj = self.qc.phase_code(n) as usize;
                let k = (dir as i32) * (SIN_LUT[ci][cj] as i32);
                // Крутящий момент русла: 0 — устоявшаяся связь, ±1 —
                // напряжённая (суперпозиция на одном из концов).
                let w = (1 + k.unsigned_abs()) * self.syntax_boost(n) * fg;
                if dir > 0 {
                    if self.down[n as usize] == 0 {
                        self.candidates.push((n, 0));
                    }
                    self.down[n as usize] += w;
                    total_down += w as u64;
                } else {
                    self.up[n as usize] += w;
                    total_up += w as u64;
                }
            }
        }

        if total_down > 0 {
            // Свёртка кандидатов с их весами (только нисходящие).
            for slot in self.candidates.iter_mut() {
                slot.1 = self.down[slot.0 as usize];
            }
            let pick = self.draw(total_down);
            let coord = self.pick_candidate(pick);
            self.clear_weights();
            return Some((coord, TicketSource::Flow, total_down as usize));
        }
        if total_up > 0 {
            // Восходящие: материализуем из плотных весов.
            self.candidates.clear();
            for (i, &w) in self.up.iter().enumerate() {
                if w > 0 {
                    self.candidates.push((i as u32, w));
                }
            }
            let pick = self.draw(total_up);
            let coord = self.pick_candidate(pick);
            self.clear_weights();
            return Some((coord, TicketSource::Backtrack, total_up as usize));
        }

        // Уровень 3: архетипический мост RQ18 — концептуальный прыжок.
        // Прямые русла иссякли: если вопрос структурно изоморфен памяти
        // (E ≥ ε), лотерея прыгает на дуги продукта a ⊗_ε lattice —
        // метафора там, где русл J нет. Конфликт кричит весом 3,
        // согласие говорит весом 2, прозрачная память (фон метафоры)
        // шепчет весом 1. RQ21: билеты моста несут синтаксический
        // множитель наравне с руслами. RQ22: фокус мост не режет —
        // прыжок уже вырос из вопроса (пересечение с его архетипом).
        if !self.bridge.is_empty() {
            self.candidates.clear();
            let mut total: u64 = 0;
            for &(c, w) in self.bridge.iter() {
                if self.ring.contains(&c) || self.veto.contains(&c) {
                    continue;
                }
                let w = w * self.syntax_boost(c);
                self.candidates.push((c, w));
                total += w as u64;
            }
            if total > 0 {
                let pick = self.draw(total);
                let coord = self.pick_candidate(pick);
                self.candidates.clear();
                return Some((coord, TicketSource::Archetype, total as usize));
            }
            self.candidates.clear();
        }

        // Уровень 4: кинетические дуги вне кольца — внутренний голос.
        // RQ21: синтаксический множитель применяется и к кинетике
        // (поля взяты явно — замыкание живёт в мире disjoint captures).
        // RQ22: кинетика внимания (дуги вопроса + подкреплённые
        // коннекторы) направляется фокусом наравне с руслами.
        self.candidates.clear();
        let ring = &self.ring;
        let veto = &self.veto;
        let syntax = self.cfg.syntax;
        let chain = &self.chain;
        let lexicon = &self.qc;
        let dist = &self.dist;
        self.qc.for_each_kinetic_arc(|arc, _m| {
            if ring.contains(&arc) || veto.contains(&arc) {
                return;
            }
            // Фокус-множитель кинетической дуги (0 — посторонняя).
            let fg = if focused && !dist.is_empty() {
                match dist[arc as usize] {
                    0 => 4,
                    1 => 3,
                    2 => 2,
                    d if d != u8::MAX => 1,
                    _ => 0,
                }
            } else {
                1
            };
            if fg == 0 {
                return;
            }
            // Одна координата — один билет за уровень (с множителем).
            let w = if syntax {
                let class = lexicon
                    .lexicon_token(arc)
                    .map(syntax_class)
                    .unwrap_or(SyntaxClass::Content);
                chain.ticket_boost(class)
            } else {
                1
            } * fg;
            if let Some(slot) = self.candidates.iter_mut().find(|s| s.0 == arc) {
                slot.1 += w;
            } else {
                self.candidates.push((arc, w));
            }
        });
        let total: u64 = self.candidates.iter().map(|s| s.1 as u64).sum();
        if total > 0 {
            let pick = self.draw(total);
            let coord = self.pick_candidate(pick);
            self.candidates.clear();
            return Some((coord, TicketSource::Kinetic, total as usize));
        }
        self.candidates.clear();
        None
    }

    /// Равномерный выбор билета из `total`.
    fn draw(&mut self, total: u64) -> u64 {
        self.rng.next_u64() % total
    }

    /// Выбор координаты по номеру билета (кумулятивные веса).
    fn pick_candidate(&mut self, mut pick: u64) -> u32 {
        for &(coord, w) in self.candidates.iter() {
            if pick < w as u64 {
                return coord;
            }
            pick -= w as u64;
        }
        // Невозможно после draw(total) — но фолбэк детерминирован.
        self.candidates
            .last()
            .map(|&(c, _)| c)
            .expect("draw требует непустых кандидатов")
    }

    /// Сброс плотных весов (подготовка к следующему шагу): чистятся
    /// только тронутые слоты — соседи кольца контекста.
    fn clear_weights(&mut self) {
        for &c in self.ring.iter() {
            for &(n, _dir) in &self.adj[c as usize] {
                self.down[n as usize] = 0;
                self.up[n as usize] = 0;
            }
        }
        self.candidates.clear();
    }

    /// Продвижение кольца контекста.
    fn push_ring(&mut self, coord: u32) {
        self.ring.push_back(coord);
        while self.ring.len() > self.cfg.window.max(1) {
            self.ring.pop_front();
        }
    }
}

// ===================== Тесты =====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gyro_lattice::QuantizedGyroCurriculum;
    use crate::syntax_bridge::BRIDGE_RUN;

    /// Микрокорпус с выраженным порядком слов: «квант» предшествует
    /// «фазе», «фаза» — «решётке». Русла дают нисходящий поток.
    const CORPUS: &str = "квант фаза решётка квант фаза решётка \
                          квант фаза born трит момент импульс";

    fn trained_engine() -> QuantizedGyroCurriculum {
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..3 {
            qc.ingest(CORPUS, 1).unwrap();
        }
        qc
    }

    // ===================== RQ23: автобиография =====================

    #[test]
    fn autobiographical_seed_prefills_ring() {
        // Рефлекс с хвостом следа: генератор с autobiographical=true
        // стартует кольцо из нити прошлого диалога (полокна), вопрос
        // занимает вторую половину; fresh-конфиг — холодный старт.
        let mut qc = trained_engine();
        qc.observe_dialog_turn("квант фаза решётка", &[(20, 1), (30, -1)]);
        assert!(!qc.reflex().trail().is_empty());

        let gcfg = GeneratorConfig {
            autobiographical: true,
            ..GeneratorConfig::default()
        };
        let gen = L5Generator::new(&mut qc, gcfg).unwrap();
        // Окно 8 по умолчанию → затравка ⌈8/2⌉ = 4 координаты следа.
        assert_eq!(gen.ring.len(), 4);

        // Без автобиографии кольцо пусто до промпта.
        let gen2 = L5Generator::new(&mut qc, GeneratorConfig::default()).unwrap();
        assert!(gen2.ring.is_empty());
    }

    #[test]
    fn autobiographical_free_speech_continues_thread() {
        // Пустой промпт + след: кольцо остаётся с нитью (затравка
        // лексикона НЕ перезаписывает её) — свободная речь продолжает
        // монолог прошлой сессии.
        let mut qc = trained_engine();
        qc.observe_dialog_turn("квант фаза решётка", &[(20, 1), (30, -1)]);
        let gcfg = GeneratorConfig {
            autobiographical: true,
            max_tokens: 6,
            ..GeneratorConfig::default()
        };
        let rep = {
            let mut gen = L5Generator::new(&mut qc, gcfg).unwrap();
            gen.generate("").unwrap()
        };
        // Речь либо состоялась из нити, либо честно молчит — но не
        // падает и не зацикливается.
        assert!(rep.steps.len() <= 6);
        let _ = rep.text;
    }

    #[test]
    fn autobiographical_turn_changes_next_answer() {
        // Нить влияет на следующую реплику: ответ после обновления
        // следа отличается от ответа холодного старта (тот же сид).
        let mut qc = trained_engine();
        let cold = {
            let gcfg = GeneratorConfig {
                seed: 11,
                max_tokens: 12,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, gcfg).unwrap();
            gen.generate("квант").unwrap().text
        };
        // Реплика уходит в след (как в chat --learn).
        qc.observe_dialog_turn("квант фаза", &[(20, 1), (30, -1), (40, 1)]);
        let warm = {
            let gcfg = GeneratorConfig {
                seed: 11,
                max_tokens: 12,
                autobiographical: true,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, gcfg).unwrap();
            gen.generate("квант").unwrap().text
        };
        // Затравка кольца меняет путь волны: тексты различны (или оба
        // пусты — вырожденный словарь; здесь лексикон живой).
        if !cold.is_empty() && !warm.is_empty() {
            assert_ne!(cold, warm, "нить обязана менять течение мысли");
        }
    }

    #[test]
    fn lexicon_builder_dominance() {
        let mut b = LexiconBuilder::new(64);
        b.observe_text("фаза фаза фаза решётка");
        let d = b.d_pol();
        let coord_f = (fnv1a64("фаза".as_bytes()) % d as u64) as u32;
        let coord_r = (fnv1a64("решётка".as_bytes()) % d as u64) as u32;
        assert_eq!(b.token_of(coord_f), Some("фаза"));
        assert_eq!(b.token_of(coord_r), Some("решётка"));
        assert_eq!(b.len(), 2);
        assert_eq!(b.tokens_seen(), 4);
        // Коллизия: тот же coord, другой токен — доминанта держится до
        // перевеса частоты.
        let mut b2 = LexiconBuilder::new(64);
        for _ in 0..3 {
            b2.observe("фаза");
        }
        b2.observe("решётка"); // та же координата в тесте не гарантирована;
        // проверяем лишь детерминизм: повторное наблюдение не ломает карту
        let _ = b2.finish().unwrap();
    }

    #[test]
    fn lexicon_builder_finish_absorb_roundtrip() {
        let mut b = LexiconBuilder::new(128);
        b.observe_text(CORPUS);
        let lex = b.finish().expect("лексикон непуст");
        // Roundtrip через строитель: absorb → те же слова.
        let mut b2 = LexiconBuilder::new(128);
        b2.absorb(&lex);
        for &(c, ref t) in lex.entries() {
            assert_eq!(b2.token_of(c), Some(t.as_str()));
        }
        assert_eq!(b2.len(), lex.len());
    }

    #[test]
    fn generated_words_come_from_corpus() {
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 12,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert!(!rep.steps.is_empty(), "мозг обязан говорить");
        assert!(!rep.text.is_empty());
        // Каждое слово — из обученного корпуса (морфем нет).
        for step in &rep.steps {
            assert!(!step.morpheme);
            assert!(
                CORPUS.contains(&step.token),
                "чужое слово: {}",
                step.token
            );
        }
        assert!(rep.steps.len() <= 12);
    }

    #[test]
    fn generation_deterministic_by_seed() {
        let run = |seed: u64| {
            let mut qc = trained_engine();
            let cfg = GeneratorConfig {
                seed,
                max_tokens: 10,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            gen.generate("фаза").unwrap()
        };
        let a = run(7);
        let b = run(7);
        assert_eq!(a.text, b.text);
        assert_eq!(a.steps.len(), b.steps.len());
        // Другой сид — другая траектория (статистически; фиксированный
        // корпус мал, поэтому проверяем лишь сам факт детерминизма сида).
        let _ = run(8);
    }

    #[test]
    fn empty_prompt_free_speech_seeded_from_lexicon() {
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("").unwrap();
        // Затравка из лексикона: либо слова, либо честная пустота
        // (микрокорпус может сойтись мгновенно).
        for step in &rep.steps {
            assert!(CORPUS.contains(&step.token));
        }
    }

    #[test]
    fn max_tokens_respected() {
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 3,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант фаза решётка").unwrap();
        assert!(rep.steps.len() <= 3);
    }

    #[test]
    fn repetition_never_immediate() {
        // Анти-заикание: соседние эмиссии не совпадают (veto = 1).
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 16,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        for w in rep.steps.windows(2) {
            assert_ne!(w[0].coord, w[1].coord, "немедленный повтор координаты");
        }
    }

    #[test]
    fn unknown_prompt_still_speaks_or_converges_honestly() {
        // Промпт из невиданных слов: каналов нет, зажиганий нет —
        // авто-free открывает чистый поток J, а если и он пуст —
        // честная сходимость без паники.
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("зукбар шлёп").unwrap();
        for step in &rep.steps {
            assert!(CORPUS.contains(&step.token) || step.morpheme);
        }
    }

    #[test]
    fn morpheme_fallback_enabled() {
        // Kinetic-путь может привести на координату без слова; с
        // включённым фолбэком она декодируется морфемой AOT.
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            morphemes: true,
            max_tokens: 10,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        // Морфема либо не понадобилась, либо валидна — паник нет.
        for step in &rep.steps {
            assert!(!step.token.is_empty());
        }
    }

    #[test]
    fn feedback_enriches_channels() {
        // Эмиссии — сенсорные события: после генерации тактов
        // гироскопа больше, чем до (речь оставляет след).
        let mut qc = trained_engine();
        let before = qc.gyro_ticks();
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант фаза").unwrap();
        let after = qc.gyro_ticks();
        assert!(after > before, "эмиссии обязаны наблюдаться гироскопом");
        assert!(!rep.steps.is_empty());
    }

    #[test]
    fn config_defaults_sane() {
        let c = GeneratorConfig::default();
        assert_eq!(c.think_steps, 4);
        assert_eq!(c.max_tokens, 64);
        assert_eq!(c.window, 8);
        assert_eq!(c.repeat_veto, 1);
        assert!(!c.free);
        assert!(!c.morphemes);
        assert!(c.bridge, "мост включён по умолчанию (ТЗ RQ18)");
        assert_eq!(c.bridge_eps, DEFAULT_BRIDGE_EPS);
    }

    // ===================== Архетипический мост (RQ18) =====================

    #[test]
    fn bridge_disabled_reports_zero_stats() {
        // --no-bridge: статистика моста нулевая, речь работает как в RQ17.
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            bridge: false,
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert_eq!(rep.bridge_energy, 0.0);
        assert_eq!(rep.bridge_co, 0);
        assert!(!rep.bridge_resonant);
        assert!(!rep.steps.is_empty());
    }

    #[test]
    fn bridge_stats_deterministic_by_seed() {
        // Одинаковый промпт + одинаковый сид — бит-в-бит одинаковая
        // статистика моста (путь внимания без ГПСЧ).
        let run = |seed: u64| {
            let mut qc = trained_engine();
            let cfg = GeneratorConfig {
                seed,
                max_tokens: 10,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            gen.generate("квант фаза решётка").unwrap()
        };
        let a = run(7);
        let b = run(7);
        assert_eq!(a.bridge_energy, b.bridge_energy);
        assert_eq!(a.bridge_co, b.bridge_co);
        assert_eq!(a.bridge_resonance, b.bridge_resonance);
        assert_eq!(a.bridge_conflict, b.bridge_conflict);
        assert_eq!(a.bridge_resonant, b.bridge_resonant);
        assert_eq!(a.text, b.text);
    }

    #[test]
    fn bridge_resonates_on_known_prompt() {
        // Знакомый вопрос структурно изоморфен памяти: гейт открыт,
        // пересечение непусто. (Промпт из слов корпуса.)
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант фаза").unwrap();
        assert!(rep.bridge_resonant, "E={}", rep.bridge_energy);
        assert!(rep.bridge_co > 0);
        assert_eq!(rep.bridge_co, rep.bridge_resonance + rep.bridge_conflict);
        // Энергия в [0, 1], порог дефолтный 0.5.
        assert!(rep.bridge_energy >= DEFAULT_BRIDGE_EPS);
        assert!(rep.bridge_energy <= 1.0);
    }

    #[test]
    fn bridge_silent_on_unknown_prompt() {
        // Чужой вопрос: архетип промпта ортогонален памяти — E = 0,
        // мост молчит (нет ложных ассоциаций). Речь при этом может
        // идти свободным потоком J.
        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("зукбар шлёп").unwrap();
        assert!(!rep.bridge_resonant);
        assert_eq!(rep.bridge_energy, 0.0);
        assert_eq!(rep.bridge_co, 0);
    }

    #[test]
    fn archetype_jump_on_channel_free_memory() {
        // Концептуальный прыжок RQ18, детерминированный. Палиндромный
        // корпус: все слова кристаллизованы (born-шаг от свидетельства),
        // но встречные потоки ЗАКРЫЛИ все русла J — прямой речи не
        // существует. Единственный путь — архетипический мост: лотерея
        // прыгает на прозрачную структуру продукта (метафора без русл).
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..2 {
            qc.ingest("alpha beta gamma delta epsilon", 0).unwrap();
            qc.ingest("epsilon delta gamma beta alpha", 0).unwrap();
        }
        assert!(qc.nnz() >= 4, "полюса кристаллизованы");
        assert_eq!(qc.channel_count(), 0, "палиндром закрыл все русла");
        let cfg = GeneratorConfig {
            max_tokens: 4,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("alpha").unwrap();
        // Мост резонирует: вопрос «alpha» изоморфен памяти (E = 1).
        assert!(rep.bridge_resonant, "E={}", rep.bridge_energy);
        assert_eq!(rep.bridge_co, 1);
        // Первое слово — прыжок моста: Flow/Backtrack мертвы (русл нет),
        // лотерея падает на уровень Archetype и говорит из продукта.
        let first = rep
            .steps
            .first()
            .expect("мост обязан говорить, когда русл нет");
        assert_eq!(first.source, TicketSource::Archetype);
        assert!(["beta", "gamma", "delta", "epsilon"].contains(&first.token.as_str()));
        // Все эмиссии — из памяти (лексикон корпуса).
        for step in &rep.steps {
            assert!(
                ["alpha", "beta", "gamma", "delta", "epsilon"]
                    .contains(&step.token.as_str()),
                "чужое слово: {}",
                step.token
            );
        }
    }

    #[test]
    fn archetype_jump_respects_veto_and_ring() {
        // Билеты моста исключают кольцо и veto: мост не заикается и
        // не повторяет контекст — только НОВЫЕ дуги продукта.
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..2 {
            qc.ingest("alpha beta gamma delta epsilon", 0).unwrap();
            qc.ingest("epsilon delta gamma beta alpha", 0).unwrap();
        }
        let cfg = GeneratorConfig {
            max_tokens: 8,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("alpha").unwrap();
        // Промпт-координата никогда не эмитируется (она в кольце).
        let coord_alpha = (fnv1a64("alpha".as_bytes()) % 128) as u32;
        for step in &rep.steps {
            assert_ne!(step.coord, coord_alpha, "мост повторил промпт");
        }
        // Анти-заикание: соседние эмиссии различны.
        for w in rep.steps.windows(2) {
            assert_ne!(w[0].coord, w[1].coord);
        }
    }

    #[test]
    fn lexicon_token_max_filter() {
        let mut b = LexiconBuilder::new(64);
        let long = "а".repeat(LEXICON_TOKEN_MAX + 1);
        b.observe(&long);
        assert_eq!(b.tokens_seen(), 0, "длинный прогон — шум, мимо");
        assert!(b.is_empty());
    }

    #[test]
    fn morpheme_table_lookup() {
        // Слот 1 — «fn » (сид build.rs): фолбэк-декодирование живо.
        let m = morpheme_at(1).expect("слот 1 засеян");
        assert!(m.starts_with("fn"));
        assert!(morpheme_at(0).is_none(), "слот 0 пуст");
        assert!(morpheme_at(256).is_none(), "вне таблицы — None");
    }

    // ===================== Грамматические мосты (RQ21) =====================

    /// Корпус-цепочка из знаменательных слов БЕЗ служебных: облака
    /// обязаны зиять, и синтаксису есть что соединять.
    const SYNTAX_CORPUS: &str = "квант фаза решётка момент импульс кристалл \
                                  память русло трит шаг волна полюс заряд спин \
                                  энергия фотон поле гармоника \
                                  решётка кристалл импульс волна русло заряд \
                                  фотон полюс момент спин фаза трит память шаг \
                                  энергия поле гармоника квант";

    /// Корпус-цепочка латиницей: таблица мостов обязана переключиться
    /// на английский синтаксис.
    const SYNTAX_CORPUS_EN: &str = "photon energy lattice momentum crystal \
                                     memory channel trit step wave pole charge \
                                     spin field harmonic \
                                     lattice crystal momentum wave channel charge \
                                     photon pole momentum spin energy field trit memory";

    /// Максимальный пробег знаменательных слов в тексте отчёта.
    /// Терминальный пробег (речь уже закончилась — бюджет/сходимость)
    /// не считается: мост обязан разрывать облако только там, где речь
    /// продолжается (хвостовой коннектор без слова запрещён).
    fn max_content_run(rep: &GenerationReport) -> usize {
        let classes: Vec<SyntaxClass> = rep
            .steps
            .iter()
            .map(|s| {
                if s.morpheme {
                    SyntaxClass::Content
                } else {
                    syntax_class(&s.token)
                }
            })
            .collect();
        let last_is_content = classes.last() == Some(&SyntaxClass::Content);
        // Отрезаем терминальный пробег до первого служебного с конца.
        let mid = if last_is_content {
            let cut = classes
                .iter()
                .rposition(|c| *c != SyntaxClass::Content)
                .map(|i| i + 1)
                .unwrap_or(0);
            &classes[..cut]
        } else {
            &classes[..]
        };
        let mut run = 0usize;
        let mut max = 0usize;
        for c in mid {
            if *c == SyntaxClass::Content {
                run += 1;
                max = max.max(run);
            } else {
                run = 0;
            }
        }
        max
    }

    #[test]
    fn syntax_off_by_default_keeps_r17_topology() {
        // Библиотечный дефолт — чистая топология RQ17: мосты выключены,
        // обратная совместимость контракта генерации.
        let c = GeneratorConfig::default();
        assert!(!c.syntax, "библиотечный дефолт RQ21: syntax = false");

        let mut qc = trained_engine();
        let cfg = GeneratorConfig {
            max_tokens: 16,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert_eq!(rep.bridges, 0, "мосты выключены — вставок нет");
        for step in &rep.steps {
            assert_ne!(step.source, TicketSource::Bridge);
            // Корпус без служебных слов — их не может быть и в речи.
            assert_ne!(syntax_class(&step.token), SyntaxClass::Conjunction);
        }
    }

    #[test]
    fn syntax_bridges_break_content_clouds() {
        // ТЗ RQ21: генерация из ассоциативного облака переходит в
        // выстроенные предложения — облако не длиннее BRIDGE_RUN,
        // между облаками — коннекторы русской таблицы.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
        for _ in 0..3 {
            qc.ingest(SYNTAX_CORPUS, 1).unwrap();
        }
        let cfg = GeneratorConfig {
            syntax: true,
            max_tokens: 40,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();

        assert!(rep.steps.len() >= 6, "речь обязана разойтись: {rep:?}");
        assert!(rep.bridges >= 1, "мосты обязаны вставляться: {rep:?}");
        assert_eq!(
            rep.bridges,
            rep.steps
                .iter()
                .filter(|s| s.source == TicketSource::Bridge)
                .count(),
            "счётчик мостов совпадает с шагами Bridge"
        );
        // Инвариант связности: облако знаменательных слов не длиннее
        // BRIDGE_RUN — предложение склеено коннекторами.
        assert!(
            max_content_run(&rep) <= BRIDGE_RUN,
            "облако прорвалось: run={}, text={}",
            max_content_run(&rep),
            rep.text
        );
        // Пик цепи ≤ BRIDGE_RUN + 1: терминальному облаку мост не нужен
        // (за ним речи уже нет — хвостовой коннектор запрещён).
        assert!(rep.syntax_run_max <= BRIDGE_RUN + 1);
        // Мосты — слова русской таблицы (кириллица), не латиница.
        for step in rep.steps.iter().filter(|s| s.source == TicketSource::Bridge) {
            assert!(is_cyrillic_token(&step.token), "мост {step:?} не русский");
            assert_ne!(syntax_class(&step.token), SyntaxClass::Content);
        }
        // Мост не может быть последним словом (за ним следует выбранное).
        if let Some(last) = rep.steps.last() {
            if last.source == TicketSource::Bridge {
                assert!(
                    rep.converged || rep.cycled,
                    "хвостовой мост без причины: {rep:?}"
                );
            }
        }
    }

    #[test]
    fn syntax_english_table_for_latin_speech() {
        // Латинский промпт и лексикон — мосты из английской таблицы.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
        for _ in 0..3 {
            qc.ingest(SYNTAX_CORPUS_EN, 1).unwrap();
        }
        let cfg = GeneratorConfig {
            syntax: true,
            max_tokens: 40,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("photon").unwrap();

        assert!(rep.steps.len() >= 6, "EN-речь обязана разойтись");
        assert!(rep.bridges >= 1, "мосты обязаны вставляться: {rep:?}");
        assert!(max_content_run(&rep) <= BRIDGE_RUN);
        const EN_BRIDGES: [&str; 8] = ["and", "but", "or", "in", "on", "is", "when", "so"];
        for step in rep.steps.iter().filter(|s| s.source == TicketSource::Bridge) {
            assert!(
                EN_BRIDGES.contains(&step.token.as_str()),
                "мост не из EN-таблицы: {step:?}"
            );
            assert!(!is_cyrillic_token(&step.token));
        }
    }

    #[test]
    fn syntax_speech_is_deterministic_by_seed() {
        let run = |seed: u64| {
            let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
            for _ in 0..3 {
                qc.ingest(SYNTAX_CORPUS, 1).unwrap();
            }
            let cfg = GeneratorConfig {
                syntax: true,
                seed,
                max_tokens: 32,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            gen.generate("квант").unwrap()
        };
        let a = run(7);
        let b = run(7);
        assert_eq!(a.text, b.text);
        assert_eq!(a.bridges, b.bridges);
        assert_eq!(a.steps.len(), b.steps.len());
    }

    #[test]
    fn syntax_grows_grammar_into_lattice() {
        // Грамматика прорастает в решётку: произнесённые коннекторы
        // становятся словами лексикона (мост → координата → лотерея).
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
        for _ in 0..3 {
            qc.ingest(SYNTAX_CORPUS, 1).unwrap();
        }
        let lex_before = qc.lexicon_len();
        let cfg = GeneratorConfig {
            syntax: true,
            max_tokens: 40,
            ..GeneratorConfig::default()
        };
        let rep = {
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            gen.generate("квант").unwrap()
        };
        assert!(rep.bridges >= 1);

        // Каждый произнесённый мост стал доминантом своей координаты.
        let d = qc.d_pol() as u64;
        for step in rep.steps.iter().filter(|s| s.source == TicketSource::Bridge) {
            let coord = (fnv1a64(step.token.as_bytes()) % d) as u32;
            assert_eq!(
                qc.lexicon_token(coord),
                Some(step.token.as_str()),
                "мост «{}» не пророс в лексикон",
                step.token
            );
        }
        assert!(qc.lexicon_len() > lex_before, "словарь вырос от речи");
    }

    #[test]
    fn syntax_respects_max_tokens_with_bridge_headroom() {
        // Мост не съедает последнее место: за мостом всегда следует
        // слово — бюджета хватает с запасом (шагов не больше потолка).
        let trained = || {
            let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
            for _ in 0..3 {
                qc.ingest(SYNTAX_CORPUS, 1).unwrap();
            }
            qc
        };
        for ceiling in [6usize, 8, 12, 24] {
            let mut qc = trained();
            let cfg = GeneratorConfig {
                syntax: true,
                max_tokens: ceiling,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            let rep = gen.generate("квант").unwrap();
            assert!(rep.steps.len() <= ceiling, "потолок {ceiling} пробит");
            let bridges = rep
                .steps
                .iter()
                .filter(|s| s.source == TicketSource::Bridge)
                .count();
            assert!(bridges <= rep.bridges);
        }
    }

    // ===================== RQ22: фокус волны =====================

    /// Цепочка из двух кластеров: квант→фаза→решётка (вопрос) и
    /// маяк→берег→туман (посторонний кластер), соединённые одним
    /// руслом решётка→маяк. Окно гироскопа 1 — пары только с
    /// непосредственным предшественником; кольцо сбрасывается между
    /// ингестами (иначе краевая пара туман→квант сшила бы кластеры
    /// напрямую). Три повтора — каждое русло насыщено.
    const TWO_CLUSTER_CORPUS: &str = "квант фаза решётка маяк берег туман";

    fn two_cluster_engine() -> QuantizedGyroCurriculum {
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 1).unwrap();
        for _ in 0..3 {
            qc.ingest(TWO_CLUSTER_CORPUS, 1).unwrap();
            qc.reset_context_ring();
        }
        qc
    }

    /// Базовая конфигурация RQ22-тестов: без архетипа (чистая
    /// проверка отсечки), без синтаксиса (без коннекторов-клея).
    fn rq22_cfg(focus_radius: usize) -> GeneratorConfig {
        GeneratorConfig {
            max_tokens: 48,
            seed: 7,
            bridge: false,
            focus_radius,
            ..GeneratorConfig::default()
        }
    }

    #[test]
    fn focus_routes_speech_into_attractor_radius() {
        // Цепочка: квант(0) — фаза(1) — решётка(2) — маяк(3) — берег(4).
        // Радиус 2: маяк и дальше — посторонние ветки, отсечены.
        let mut qc = two_cluster_engine();
        let mut gen = L5Generator::new(&mut qc, rq22_cfg(2)).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert!(rep.focus_active, "маршрутизация обязана включиться");
        assert_eq!(rep.focus_radius, 2);
        assert_eq!(rep.attractor_arcs, 1, "аттрактор — дуга «квант»");
        // Каждый выбор лотареи — в радиусе (архетип выключен:
        // посторонних прыжков нет, фолбэка быть не должно).
        assert_eq!(rep.wander_steps, 0, "волна ушла из фокуса: {rep:?}");
        assert_eq!(rep.relevance, 1.0);
        // Посторонние слова кластера «маяк» не произнесены НИ РАЗУ.
        let near: [&str; 3] = ["квант", "фаза", "решётка"];
        for s in &rep.steps {
            assert!(
                near.contains(&s.token.as_str()),
                "фокус пропустил постороннее слово «{}»",
                s.token
            );
        }
        // Речь не пуста — фокус не гасит ответ (в радиусе есть русла).
        assert!(!rep.steps.is_empty());
    }

    #[test]
    fn focus_off_measures_relevance_of_free_speech() {
        // Фокус выключен: телеметрия релевантности всё равно считается —
        // насколько свободное блуждание держится темы вопроса.
        let mut qc = two_cluster_engine();
        let mut gen = L5Generator::new(&mut qc, rq22_cfg(0)).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert!(!rep.focus_active);
        assert_eq!(rep.attractor_arcs, 1);
        assert!(rep.focus_radius == 0);
        assert!((0.0..=1.0).contains(&rep.relevance));
        assert_eq!(
            rep.focused_steps + rep.wander_steps,
            rep.steps.len()
        );
    }

    #[test]
    fn focus_tight_radius_cuts_chain_earlier() {
        // Радиус 1 — «решётка» (дист. 2) уже посторонняя: речь
        // заперта в паре квант↔фаза.
        let mut qc = two_cluster_engine();
        let mut gen = L5Generator::new(&mut qc, rq22_cfg(1)).unwrap();
        let rep = gen.generate("квант").unwrap();
        let near: [&str; 2] = ["квант", "фаза"];
        for s in &rep.steps {
            assert!(
                near.contains(&s.token.as_str()),
                "радиус 1 пропустил «{}»",
                s.token
            );
        }
    }

    #[test]
    fn focus_unknown_question_stays_honest() {
        // Незнакомый вопрос: редкий токен получает высокий IDF —
        // аттрактор существует даже для неизвестного слова (его
        // собственная координата + кинетика внимания — «внутренний
        // голос» RQ17). Фокус не гасит ответ: мозг одинаково жив
        // (говорит или честно сходится), релевантность измерена
        // корректно в обоих режимах.
        let mk = |focus: usize| {
            let mut qc = QuantizedGyroCurriculum::new(4096, 0.05, 42, 8).unwrap();
            for _ in 0..3 {
                qc.ingest(TWO_CLUSTER_CORPUS, 1).unwrap();
            }
            let mut gen = L5Generator::new(&mut qc, rq22_cfg(focus)).unwrap();
            gen.generate("зюзяля").unwrap()
        };
        let r_focused = mk(3);
        let r_free = mk(0);
        // Аттрактор — сама координата незнакомого слова (IDF высок).
        assert!(r_focused.attractor_arcs >= 1);
        assert_eq!(r_focused.attractor_arcs, r_free.attractor_arcs);
        // Одинаковая жизнеспособность: оба говорят или оба честно молчат.
        assert_eq!(
            r_focused.steps.is_empty(),
            r_free.steps.is_empty(),
            "фокус изменил жизнеспособность незнакомого вопроса"
        );
        for r in [&r_focused, &r_free] {
            assert!((0.0..=1.0).contains(&r.relevance));
            assert_eq!(r.focused_steps + r.wander_steps, r.steps.len());
        }
    }

    #[test]
    fn focus_deterministic_by_seed() {
        let mk = || {
            let mut qc = two_cluster_engine();
            let mut gen = L5Generator::new(&mut qc, rq22_cfg(2)).unwrap();
            gen.generate("квант").unwrap().text
        };
        assert_eq!(mk(), mk());
    }

    #[test]
    fn focus_gains_by_distance() {
        // Таблица множителей: 0→×4, 1→×3, 2→×2, дальше в радиусе→×1,
        // за радиусом→×0; свободный проход — всегда ×1.
        let mut qc = two_cluster_engine();
        let mut gen = L5Generator::new(&mut qc, rq22_cfg(3)).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert!(rep.focus_active);
        // Цепочка: квант(0) → фаза(1) → решётка(2) → маяк(3) → берег(4).
        let coord = |t: &str| (fnv1a64(t.as_bytes()) % 256) as u32;
        assert_eq!(gen.focus_gain(coord("квант"), true), 4);
        assert_eq!(gen.focus_gain(coord("фаза"), true), 3);
        assert_eq!(gen.focus_gain(coord("решётка"), true), 2);
        assert_eq!(gen.focus_gain(coord("маяк"), true), 1, "дист. 3 в радиусе 3");
        assert_eq!(gen.focus_gain(coord("туман"), true), 0, "за радиусом — отсечён");
        assert_eq!(gen.focus_gain(coord("туман"), false), 1, "свободный проход не режет");
    }

    #[test]
    fn attractor_bfs_respects_radius() {
        // Дистанции: BFS не заходит за радиус; недостижимые — MAX.
        let qc = two_cluster_engine();
        let adj: Vec<Vec<(u32, i8)>> = {
            let mut adj: Vec<Vec<(u32, i8)>> = vec![Vec::new(); 256];
            for (i, j, w) in qc.channels() {
                let dir = if w > 0.0 { 1i8 } else { -1 };
                adj[i as usize].push((j, dir));
                adj[j as usize].push((i, -dir));
            }
            adj
        };
        let coord = |t: &str| (fnv1a64(t.as_bytes()) % 256) as u32;
        let dist = L5Generator::attractor_distances(&adj, &[coord("квант")], 2);
        assert_eq!(dist[coord("квант") as usize], 0);
        assert_eq!(dist[coord("фаза") as usize], 1);
        assert_eq!(dist[coord("решётка") as usize], 2);
        assert_eq!(dist[coord("маяк") as usize], u8::MAX, "маяк за радиусом 2");
    }

    // ============== RQ22: самоподкрепление грамматики ==============

    #[test]
    fn observe_reinforcement_saturates_channel() {
        // Физика подкрепления: пара (слово → коннектор). Обычное
        // наблюдение — один свидетель (русло свежее, наружу невидимо);
        // подкрепление — второй согласованный свидетель (насыщенное
        // русло J, контейнер его переживёт) + кинетика внимания.
        let mut qc = QuantizedGyroCurriculum::new(1024, 0.05, 42, 8).unwrap();
        let coord = |t: &str| (fnv1a64(t.as_bytes()) % 1024) as u32;
        let (w, c) = (coord("фотон"), coord("и"));
        assert_ne!(w, c, "коллизия хеша в d=1024 — поменяйте сид");
        // Свидетель 1: пара (w → c).
        qc.observe_event(w, 1);
        qc.observe_event(c, 1);
        let touches = |qc: &QuantizedGyroCurriculum, x: u32| {
            qc.channels().iter().any(|&(i, j, _)| i == x || j == x)
        };
        assert!(!touches(&qc, c), "одно свидетельство — русло свежее");
        // Подкрепление: второй свидетель — русло насыщается.
        qc.observe_reinforcement(c, 1);
        assert!(touches(&qc, c), "подкрепление открывает русло коннектора");
        // Кинетика внимания: дуга коннектора стала путём волны.
        assert_ne!(qc.momentum_at(c), 0);
        // Насыщенное русло переживает чекпоинт + рестарт (секция GYRO).
        let bytes = qc.checkpoint().unwrap();
        let reader = pqw::PqwReader::from_bytes(&bytes).unwrap();
        let mut qc2 = QuantizedGyroCurriculum::new(1024, 0.05, 1, 8).unwrap();
        qc2.resume_from_reader(&reader).unwrap();
        assert!(touches(&qc2, c), "русло коннектора пережило рестарт");
    }

    #[test]
    fn bridges_density_metric_per_hundred_words() {
        // Плотность мостов = bridges × 100 / слов — считается честно.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 8).unwrap();
        for _ in 0..3 {
            qc.ingest(SYNTAX_CORPUS, 1).unwrap();
        }
        let cfg = GeneratorConfig {
            syntax: true,
            reinforce: true,
            max_tokens: 48,
            seed: 42,
            bridge: false,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
        let rep = gen.generate("квант").unwrap();
        assert!(rep.bridges > 0);
        // Подкрепление в речи: каждый вставленный коннектор укреплен.
        assert_eq!(rep.reinforced, rep.bridges);
        let expect = rep.bridges as f64 * 100.0 / rep.steps.len() as f64;
        assert!((rep.syntax_bridges_density - expect).abs() < 1e-12);
        assert!(rep.syntax_bridges_density > 0.0);
        assert!(rep.syntax_bridges_density < 100.0);
    }

    #[test]
    fn focus_and_syntax_compose() {
        // Фокус + мосты: коннектор не выводит речь из фокуса
        // (релевантность меряет только выборы лотареи), и вся
        // комбинация детерминирована сидом.
        let mk = || {
            let mut qc = two_cluster_engine();
            let cfg = GeneratorConfig {
                syntax: true,
                reinforce: true,
                focus_radius: 2,
                max_tokens: 40,
                seed: 9,
                bridge: false,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut qc, cfg).unwrap();
            gen.generate("квант фаза").unwrap()
        };
        let r1 = mk();
        let r2 = mk();
        assert_eq!(r1.text, r2.text, "детерминизм RQ22");
        assert_eq!(r1.wander_steps, 0);
        assert_eq!(r1.relevance, 1.0);
    }
}

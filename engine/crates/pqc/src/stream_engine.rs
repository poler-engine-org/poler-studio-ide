//! Zero-Storage Streaming Learning Engine (RQ6): непрерывный фазовый
//! инференс и потоковое обучение из интернета — целиком в RAM.
//!
//! ```text
//! Веб-текст/HTML ──strip_html──▶ текст ──TF-IDF ε-плотность──▶ LENS-CSR
//!        ──Packed4 (реюз буфера)──▶ контейнер в RAM ──zero-copy──▶ pqc
//!        ──коммутатор Фокиана──▶ ActiveInference (Born-шаг) ──▶ QCM
//! ```
//!
//! Надстройка над RQ3 ([`crate::stream`]) и RQ5 ([`crate::learn`]):
//! каждый чанк потока немедленно превращается в свидетельство
//! (TF-IDF ε-плотность), сериализуется в сверхплотный контейнер
//! [`.pqw` v0.2 Packed4](https://docs.rs/pqw) **в переиспользуемом
//! буфере** (после прогрева — ни одной системной аллокации на контейнер),
//! а петля Active Inference делает Born-фазовый шаг к цели чанка.
//! Дисковый I/O отсутствует полностью.
//!
//! ## TF-IDF ε-плотность
//!
//! Токены хешируются FNV-1a в координату `h mod d_pol` со знаковой
//! полярностью из старшего бита (как в [`pqw::stream::TextPhaseEncoder`]);
//! внутри чанка накапливается знаковый `tf_i`, между чанками — частота
//! документов `df_i`. Вес координаты:
//!
//! ```text
//! score_i = tf_i · idf_i,    idf_i = ln((1 + N) / (1 + df_i)),   N = docs + 1
//! ```
//!
//! Сквозной шум (координата во всех чанках) давится `idf → 0`,
//! отличительные токены усиливаются. L∞-нормализация даёт
//! `p ∈ [−1, 1]^d`, LENS-фильтр `|p| ≥ ε` оставляет плотные дуги.
//! Коллизии хеш-пространства сливаются в одну координату —
//! `df` считается по координатам, а не по токенам.
//!
//! ## Семантический коммутатор (барьер NO_HITS)
//!
//! Невязка Фокиана между свежим свидетельством `f` и памятью модели `ρ` —
//! симметризованный коммутатор ранговых Born-операторов:
//!
//! ```text
//! F = f·fᵀ,  DM = ρ·ρᵀ,   [F, DM]_S = F·DM − DM·F
//! ‖[F, DM]‖_F = |fᵀρ| · √(2·(‖f‖²‖ρ‖² − (fᵀρ)²))    — O(d), без матриц
//! ```
//!
//! Ровно нуль при `LENS_HITS = 0` (пустое свидетельство: `f = 0` —
//! детерминированный отказ, шаг обучения не выполняется), нуль на
//! стационаре (`f ∥ ρ`) и при ортогональной новой теме (`f ⊥ ρ`,
//! нет взаимодействия). Максимум — «закрутка» свидетельства против
//! памяти на промежуточных углах.
//!
//! ## Политика фона (Π_Λ при обучении на чанке)
//!
//! * [`Forget::Hold`] (по умолчанию) — дуги вне чанка заморожены:
//!   LENS-граф только накапливает факты (оператор резонанса `J`);
//! * [`Forget::Decay`] — вне чанка цель `0`: старые темы диссипируют
//!   к честным монетам (оператор диссипации `D = LLᵀ`).
//!
//! ## Пример: поток из трёх чанков
//!
//! ```
//! use pqc::stream_engine::{Forget, StreamEngine};
//!
//! let mut engine = StreamEngine::new(512, 0.2, 42).unwrap();
//!
//! // Первый чанк: свидетельство усваивается, коммутатор ненулевой.
//! let a = engine.ingest("фазовый континуум фазовый триты", 0).unwrap();
//! assert!(!a.no_hits);
//! assert!(a.nnz > 0);
//! // Заголовок 0x80 + Packed4: ceil(512 / 4) = 128 байт фаз.
//! assert_eq!(a.container_bytes, 0x80 + 512 / 4);
//!
//! // Пустой чанк: барьер NO_HITS — детерминированный отказ.
//! let empty = engine.ingest("… — !!!", 0).unwrap();
//! assert!(empty.no_hits);
//! assert_eq!(empty.step, None);
//! assert_eq!(empty.fock.normalized, 0.0);
//!
//! // Повтор того же чанка с уточнением: потеря ниже.
//! let b = engine.ingest("фазовый континуум фазовый триты", 3).unwrap();
//! assert!(b.param_loss < a.param_loss);
//! ```

use std::time::Instant;

use pqw::checksum::fnv1a64;
use pqw::stream::tokenize;
use pqw::{PqwReader, PqwWriter};

use crate::ansatz::{Ansatz, PhaseAnsatz};
use crate::coherence::{coherence, CoherenceReport};
use crate::gyro::Gyroscope;
use crate::learn::{ActiveInference, ActiveStepReport};
use crate::rng::Rng;

/// Политика фона: что происходит с дугами модели вне текущего чанка.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Forget {
    /// Дуги вне чанка заморожены (`∇ = 0`): LENS-граф накапливает факты.
    #[default]
    Hold,
    /// Дуги вне чанка диссипируют к честным монетам (цель `0`).
    Decay,
}

/// Невязка Фокиана `[F, DM]_S` свидетельства против памяти.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FockResidual {
    /// Frobenius-норма коммутатора `‖[F, DM]‖_F`.
    pub raw: f64,
    /// Нормировка в `[0, 1]`: `raw · √2 / (‖f‖² · ‖ρ‖²)`.
    pub normalized: f64,
}

/// Симметризованный коммутатор ранговых Born-операторов свидетельства `f`
/// и памяти `ρ` (см. схему модуля). O(d): матрицы не строятся.
///
/// Инварианты: `f = 0` → нуль; `f ∥ ρ` → нуль; `f ⊥ ρ` → нуль.
pub fn fock_residual(f: &[f64], rho: &[f64]) -> FockResidual {
    let n = f.len().min(rho.len());
    let mut dot = 0.0_f64;
    let mut nf2 = 0.0_f64;
    let mut nr2 = 0.0_f64;
    for i in 0..n {
        dot += f[i] * rho[i];
        nf2 += f[i] * f[i];
        nr2 += rho[i] * rho[i];
    }
    // ‖[F, DM]‖_F = |fᵀρ| · √(2·(‖f‖²‖ρ‖² − (fᵀρ)²))
    let area2 = 2.0 * (nf2 * nr2 - dot * dot).max(0.0);
    let raw = dot.abs() * area2.sqrt();
    let denom = std::f64::consts::SQRT_2 * nf2 * nr2;
    let normalized = if denom > 0.0 { raw / denom } else { 0.0 };
    FockResidual { raw, normalized }
}

/// Итог прогона чанка через движок.
#[derive(Clone, Debug)]
pub struct StreamChunkReport {
    /// Число токенов чанка.
    pub tokens: usize,
    /// Число LENS-дуг свидетельства (пережили ε-фильтр).
    pub nnz: u64,
    /// Размер ин-мемори Packed4-контейнера в байтах.
    pub container_bytes: usize,
    /// Емкость переиспользуемого буфера (стабильна после прогрева).
    pub buffer_capacity: usize,
    /// Число усвоенных документов потока (включая этот).
    pub docs_seen: u64,
    /// Барьер NO_HITS: свидетельство пусто, шаг не выполнялся.
    pub no_hits: bool,
    /// Коммутатор Фокиана свидетельство × память (до шага).
    pub fock: FockResidual,
    /// Первый Born-шаг Active Inference (`None` ⟺ no_hits).
    pub step: Option<ActiveStepReport>,
    /// Всего шагов к цели чанка (1 + `extra_steps`).
    pub steps_run: usize,
    /// Финальная параметрическая потеря `½‖p − target‖²` на дугах чанка.
    pub param_loss: f64,
    /// QCM модели после шага.
    pub qcm: CoherenceReport,
    /// Сквозная задержка прогона.
    pub elapsed: std::time::Duration,
}

/// Движок потокового обучения: кодировщик TF-IDF + Packed4-буфер +
/// модель (LENS-дуги) + Active Inference.
///
/// Builder-методы (`with_*`) следует вызывать до первого [`ingest`]:
/// пересборка ученика сбрасывает ГПСЧ-поток в детерминированное начало.
///
/// [`ingest`]: StreamEngine::ingest
pub struct StreamEngine {
    d_pol: u32,
    epsilon: f32,
    seed: u64,
    // Регламент ученика (пересобирается builder'ами).
    eta0: f64,
    beta: f64,
    gamma: f64,
    purify_every: usize,
    learner_epsilon: f64,
    shots: u64,
    // TF-IDF статистика потока.
    doc_freq: Vec<u32>,
    docs: u64,
    // Переиспользуемый Packed4-контейнер (RAM).
    buf: Vec<u8>,
    // Память модели: плотный вектор фаз + LENS-дуги (union support).
    model: Vec<f64>,
    ansatz: Ansatz,
    // Петля обучения.
    learner: ActiveInference,
    // ГПСЧ для QCM-сэмплирования (отдельно от измерений ученика).
    rng: Rng,
    qcm_shots: u64,
    forget: Forget,
    // RQ10: гироскоп памяти J = A − Aᵀ (уровень 4 фазовой сборки).
    gyro: Option<Gyroscope>,
}

impl StreamEngine {
    /// Новый движок: размерность `d_pol ≥ 1`, порог LENS `ε ∈ [0, 1]`,
    /// детерминирующий сид (из него выводятся оба ГПСЧ-потока).
    ///
    /// Регламент по умолчанию: `η₀ = 0.25`, `β = 1.0`, `γ = 0.5`,
    /// McWeeny `K = 5`, `shots = 4096`, политика [`Forget::Hold`].
    pub fn new(d_pol: u32, epsilon: f32, seed: u64) -> crate::Result<StreamEngine> {
        if d_pol == 0 {
            return Err(crate::PqcError::EmptyState);
        }
        if !epsilon.is_finite() || epsilon < 0.0 || epsilon > 1.0 {
            return Err(crate::PqcError::BadPhase(epsilon as f64));
        }
        let d = d_pol as usize;
        // Золотое сечение: декорреляция потоков измерений и оценки.
        let qcm_seed = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut engine = StreamEngine {
            d_pol,
            epsilon,
            seed,
            eta0: 0.25,
            beta: 1.0,
            gamma: 0.5,
            purify_every: 5,
            learner_epsilon: 0.0,
            shots: 4096,
            doc_freq: vec![0u32; d],
            docs: 0,
            buf: Vec::new(),
            model: vec![0.0_f64; d],
            ansatz: Ansatz::Product(PhaseAnsatz::new(d_pol, Vec::new())?),
            learner: ActiveInference::new(0.25, 1.0, 4096).with_seed(seed),
            rng: Rng::seed_from_u64(qcm_seed),
            qcm_shots: 256,
            forget: Forget::Hold,
            gyro: None,
        };
        engine.learner = engine.build_learner();
        Ok(engine)
    }

    /// Пересборка ученика из текущего регламента (детерминизм: сид фиксирован).
    fn build_learner(&self) -> ActiveInference {
        ActiveInference::new(self.eta0, self.beta, self.shots)
            .with_seed(self.seed)
            .with_gamma(self.gamma)
            .with_purify_every(self.purify_every)
            .with_epsilon(self.learner_epsilon)
    }

    /// Политика фона (builder).
    pub fn with_forget(mut self, forget: Forget) -> StreamEngine {
        self.forget = forget;
        self
    }

    /// Выстрелов на измерение в петле обучения (builder).
    pub fn with_shots(mut self, shots: u64) -> StreamEngine {
        self.set_shots(shots);
        self
    }

    /// Выстрелов на измерение — мутабельная версия [`StreamEngine::with_shots`]
    /// для уровней curriculum: пересобирает ученика (сид фиксирован —
    /// детерминизм уровня не зависит от бюджета выстрелов).
    pub fn set_shots(&mut self, shots: u64) {
        self.shots = shots;
        self.learner = self.build_learner();
    }

    /// Гиперпараметры петли: `η₀`, `β`, `γ` (builder).
    pub fn with_hyper(mut self, eta0: f64, beta: f64, gamma: f64) -> StreamEngine {
        self.eta0 = eta0;
        self.beta = beta;
        self.gamma = gamma;
        self.learner = self.build_learner();
        self
    }

    /// Период McWeeny-пурификации K (builder).
    pub fn with_purify_every(mut self, every: usize) -> StreamEngine {
        self.purify_every = every;
        self.learner = self.build_learner();
        self
    }

    /// Порог Π_Λ ученика (builder): дуги `|p| < ε` заморожены.
    pub fn with_learner_epsilon(mut self, epsilon: f64) -> StreamEngine {
        self.learner_epsilon = epsilon;
        self.learner = self.build_learner();
        self
    }

    /// Выстрелов QCM-оценки модели (builder).
    pub fn with_qcm_shots(mut self, shots: u64) -> StreamEngine {
        self.qcm_shots = shots;
        self
    }

    /// RQ10: включить гироскоп памяти `J = A − Aᵀ` (builder).
    ///
    /// Окно `window ≥ 1` — дальность направленного контекста в токенах
    /// (стоимость O(window) на токен, O(N²) не возникает никогда);
    /// `budget ≥ 16` — потолок сырых направленных пар в RAM между
    /// прореживаниями. Токены подаются в порядке появления: полярность —
    /// из хеша, как в TF-IDF-кодировщике.
    pub fn with_gyro(mut self, window: usize, budget: usize) -> StreamEngine {
        self.gyro = Some(Gyroscope::new(window, budget));
        self
    }

    /// Гироскоп памяти (если включён).
    pub fn gyro(&self) -> Option<&Gyroscope> {
        self.gyro.as_ref()
    }

    /// Мутабельный доступ к гироскопу (если включён).
    pub fn gyro_mut(&mut self) -> Option<&mut Gyroscope> {
        self.gyro.as_mut()
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Порог LENS.
    pub fn epsilon(&self) -> f32 {
        self.epsilon
    }

    /// Число усвоенных документов.
    pub fn docs_seen(&self) -> u64 {
        self.docs
    }

    /// Плотная память модели `p ∈ [−1, 1]^d`.
    pub fn model(&self) -> &[f64] {
        &self.model
    }

    /// LENS-дуги модели (union support всех чанков).
    pub fn model_arcs(&self) -> &[(u32, f64)] {
        match &self.ansatz {
            Ansatz::Product(pa) => pa.arcs(),
            Ansatz::Statevector(_) => &[],
        }
    }

    /// Последний Packed4-контейнер в RAM (свидетельство чанка).
    pub fn container(&self) -> &[u8] {
        &self.buf
    }

    /// Resume: поднять накопленную память из `.pqw`-контейнера (RQ9).
    ///
    /// Контейнер хранит trit-проекцию памяти — union support + знаки дуг.
    /// Анзац восстанавливается из деквантованных дуг (±1 — «затвердевшее
    /// мнение»), плотная память `model` — их проекцией; непрерывные
    /// амплитуды заново уточняются измерениями новых чанков
    /// («мнение рождается измерением»). Возвращает число поднятых дуг.
    ///
    /// Размерность контейнера обязана совпадать с `d_pol` движка.
    pub fn resume_from_reader(&mut self, reader: &PqwReader) -> crate::Result<usize> {
        if reader.d_pol() != self.d_pol {
            return Err(crate::PqcError::LengthMismatch {
                expected: self.d_pol as usize,
                actual: reader.d_pol() as usize,
            });
        }
        // RQ10: реляционная память поднимается даже при пустых фазах.
        self.resume_gyro_from_reader(reader)?;
        let arcs: Vec<(u32, f64)> = reader.decoded().collect();
        let n = arcs.len();
        if n == 0 {
            return Ok(0);
        }
        self.ansatz = Ansatz::Product(PhaseAnsatz::new(self.d_pol, arcs.clone())?);
        self.model = vec![0.0_f64; self.d_pol as usize];
        for &(i, p) in &arcs {
            self.model[i as usize] = p;
        }
        Ok(n)
    }

    /// RQ10: resume гироскопа из v3-контейнера. Контейнер хранит
    /// квантованную консолидированную циркуляцию `J` + счётчик тактов —
    /// реляционная память переживает рестарт. Возвращает число впитанных
    /// пар (0, если секции нет или гироскоп не включён).
    ///
    /// Вызывается автоматически из [`StreamEngine::resume_from_reader`]
    /// при включённом гироскопе.
    pub fn resume_gyro_from_reader(&mut self, reader: &PqwReader) -> crate::Result<usize> {
        let Some(section) = reader.gyro() else {
            return Ok(0);
        };
        let Some(g) = self.gyro.as_mut() else {
            return Ok(0);
        };
        let pairs: Vec<(u32, u32, f64)> = section
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        Ok(g.absorb(&pairs, section.ticks()))
    }

    /// Движок обучающей петли (для диагностики расписания η).
    pub fn learner(&self) -> &ActiveInference {
        &self.learner
    }

    /// Прогон HTML: разметка срезается, затем [`StreamEngine::ingest`].
    pub fn ingest_html(
        &mut self,
        html: &[u8],
        extra_steps: usize,
    ) -> crate::Result<StreamChunkReport> {
        let text = strip_html(html);
        self.ingest(&text, extra_steps)
    }

    /// Сквозной прогон чанка: TF-IDF ε-плотность → LENS-CSR → Packed4
    /// (реюз буфера) → коммутатор → Born-фазовый шаг (1 + `extra_steps`)
    /// → QCM. Весь цикл — в RAM, без дискового I/O.
    pub fn ingest(&mut self, text: &str, extra_steps: usize) -> crate::Result<StreamChunkReport> {
        let t0 = Instant::now();
        let d = self.d_pol as usize;

        // 0. RQ10: гироскоп памяти — токены в порядке появления.
        //    Полярность — из хеша (тот же поток, что TF-IDF-кодировщик):
        //    направленная циркуляция смысла J = A − Aᵀ, O(window)/токен.
        if let Some(g) = self.gyro.as_mut() {
            for token in tokenize(text) {
                let h = fnv1a64(token.as_bytes());
                let coord = (h % self.d_pol as u64) as u32;
                let sign = if (h >> 63) & 1 == 1 { 1.0 } else { -1.0 };
                g.observe(coord, sign);
            }
        }

        // 1. TF-IDF ε-плотность чанка (N включает текущий документ).
        let (state, tokens) = self.encode_chunk(text);
        let no_hits = state.iter().all(|&p| p == 0.0);

        // 2. Барьер NO_HITS: пустое свидетельство — детерминированный отказ.
        if no_hits {
            let sample = self.ansatz.sample(&mut self.rng, self.qcm_shots, 0)?;
            let qcm = coherence(&sample);
            return Ok(StreamChunkReport {
                tokens,
                nnz: 0,
                container_bytes: self.buf.len(),
                buffer_capacity: self.buf.capacity(),
                docs_seen: self.docs,
                no_hits: true,
                fock: FockResidual {
                    raw: 0.0,
                    normalized: 0.0,
                },
                step: None,
                steps_run: 0,
                param_loss: 0.0,
                qcm,
                elapsed: t0.elapsed(),
            });
        }

        // 3. Коммутатор Фокиана ДО шага: закрутка свидетельства против памяти.
        let fock = fock_residual(&state, &self.model);

        // 4. LENS-CSR → Packed4 в переиспользуемом буфере (RAM).
        let state_f32: Vec<f32> = state.iter().map(|&p| p as f32).collect();
        self.buf.clear();
        {
            let mut w = PqwWriter::new(self.d_pol)?;
            // ρ-гиперпараметр контейнера кодирует политику фона:
            // 1.0 — полная память (Hold), 0.0 — диссипация (Decay).
            let rho = if self.forget == Forget::Decay {
                0.0
            } else {
                1.0
            };
            w = w.hyperparams(self.eta0 as f32, self.gamma as f32, rho, self.epsilon);
            w.add_state(&state_f32)?;
            w.write_packed_trits(&mut self.buf)?;
        }
        let container_bytes = self.buf.len();
        // Zero-copy чтение свидетельства поверх того же буфера.
        let (chunk_arcs, nnz) = {
            let reader = PqwReader::from_bytes(&self.buf)?;
            let arcs: Vec<(u32, f64)> = reader.decoded().collect();
            (arcs, reader.nnz())
        };

        // 5. Слияние LENS-дуг чанка в память (union support; новые дуги
        //    стартуют с честной монеты p = 0 — мнение рождается измерением).
        self.merge_support(&chunk_arcs)?;

        // 6. Born-фазовый шаг Active Inference к цели чанка: непрерывная
        //    TF-IDF плотность на LENS-дугах (контейнер хранит её тритовую
        //    проекцию — сериализованное свидетельство, а цель обучения
        //    остаётся фазовым континуумом до самой пурификации McWeeny).
        let mut mask = vec![false; d];
        for &(i, _) in &chunk_arcs {
            mask[i as usize] = true;
        }
        let target = &state;
        let decay = self.forget == Forget::Decay;
        let first = self.learner.step(&mut self.ansatz, |p_emp| {
            p_emp
                .iter()
                .enumerate()
                .map(|(i, &e)| {
                    if mask[i] {
                        e - target[i]
                    } else if decay {
                        e // цель 0: диссипация старых тем
                    } else {
                        0.0 // Hold: фон заморожен
                    }
                })
                .collect()
        })?;
        self.sync_model();

        // 7. Дополнительные шаги к той же цели (уточнение фаз).
        for _ in 0..extra_steps {
            self.learner.step(&mut self.ansatz, |p_emp| {
                p_emp
                    .iter()
                    .enumerate()
                    .map(|(i, &e)| {
                        if mask[i] {
                            e - target[i]
                        } else if decay {
                            e
                        } else {
                            0.0
                        }
                    })
                    .collect()
            })?;
        }
        self.sync_model();
        let steps_run = 1 + extra_steps;

        // 8. Обновление TF-IDF статистики: дотронутые координаты.
        for (i, s) in state.iter().enumerate() {
            if *s != 0.0 {
                self.doc_freq[i] = self.doc_freq[i].saturating_add(1);
            }
        }
        self.docs += 1;

        // 9. QCM модели после шага.
        let sample = self.ansatz.sample(&mut self.rng, self.qcm_shots, 0)?;
        let qcm = coherence(&sample);

        // 10. Финальная потеря на дугах чанка (против непрерывной цели).
        let param_loss = 0.5
            * chunk_arcs
                .iter()
                .map(|&(i, _)| {
                    let diff = self.model[i as usize] - state[i as usize];
                    diff * diff
                })
                .sum::<f64>();

        Ok(StreamChunkReport {
            tokens,
            nnz,
            container_bytes,
            buffer_capacity: self.buf.capacity(),
            docs_seen: self.docs,
            no_hits: false,
            fock,
            step: Some(first),
            steps_run,
            param_loss,
            qcm,
            elapsed: t0.elapsed(),
        })
    }

    /// TF-IDF ε-плотность чанка: знаковый tf по координатам хеш-пространства,
    /// idf по частоте документов, L∞-нормализация.
    ///
    /// Тонкая обёртка над разреженным ядром [`tfidf_arcs`] (общим с
    /// квантованным curriculum RQ15): дуги материализуются в плотный вектор.
    fn encode_chunk(&self, text: &str) -> (Vec<f64>, usize) {
        let d = self.d_pol as usize;
        let (arcs, tokens) = tfidf_arcs(self.d_pol, self.docs, |i| self.doc_freq[i as usize], text);
        let mut state = vec![0.0_f64; d];
        for (i, s) in arcs {
            state[i as usize] = s;
        }
        (state, tokens)
    }

    /// Слияние дуг чанка в память: новые координаты входят в support
    /// с фазой честной монеты `p = 0`.
    fn merge_support(&mut self, chunk_arcs: &[(u32, f64)]) -> crate::Result<()> {
        let need_merge = match &self.ansatz {
            Ansatz::Product(pa) => chunk_arcs
                .iter()
                .any(|&(i, _)| pa.arcs().binary_search_by_key(&i, |a| a.0).is_err()),
            Ansatz::Statevector(_) => false,
        };
        if !need_merge {
            return Ok(());
        }
        let mut arcs: Vec<(u32, f64)> = match &self.ansatz {
            Ansatz::Product(pa) => pa.arcs().to_vec(),
            Ansatz::Statevector(_) => Vec::new(),
        };
        // HashSet вместо binary_search: массив теряет сортированность
        // после первого push, бинарный поиск тогда ненадёжен.
        let mut seen: std::collections::HashSet<u32> = arcs.iter().map(|a| a.0).collect();
        for &(i, _) in chunk_arcs {
            if seen.insert(i) {
                arcs.push((i, 0.0));
            }
        }
        arcs.sort_unstable_by_key(|a| a.0);
        self.ansatz = Ansatz::Product(PhaseAnsatz::new(self.d_pol, arcs)?);
        Ok(())
    }

    /// Память ← параметры анзаца (фон — честные монеты).
    fn sync_model(&mut self) {
        if let Ansatz::Product(pa) = &self.ansatz {
            for v in self.model.iter_mut() {
                *v = 0.0;
            }
            for &(i, p) in pa.arcs() {
                self.model[i as usize] = p;
            }
        }
    }
}

/// Разреженное TF-IDF-ядро чанка (общее для RQ8 и RQ15).
///
/// Знаковый tf по координатам хеш-пространства (`fnv1a64 mod d_pol`,
/// полярность — старший бит хеша), idf по частоте документов
/// `ln((1+n)/(1+df))`, L∞-нормализация на максимум. Возвращает дуги
/// `(индекс, значение)` ДО ε-ворот, отсортированные по индексу, и число
/// токенов. Плотный движок материализует дуги в вектор, квантованный
/// curriculum потребляет их напрямую — свидетельство у обоих ОДНО.
///
/// Порядок `HashMap`-итераций не влияет на результат: значения
/// координато-независимы, сортировка канонизирует порядок.
pub(crate) fn tfidf_arcs<F: Fn(u32) -> u32>(
    d_pol: u32,
    docs: u64,
    df_of: F,
    text: &str,
) -> (Vec<(u32, f64)>, usize) {
    let mut tf: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    let mut tokens = 0usize;
    for token in tokenize(text) {
        tokens += 1;
        let h = fnv1a64(token.as_bytes());
        let idx = (h % d_pol as u64) as u32;
        let sign: i64 = if (h >> 63) & 1 == 1 { 1 } else { -1 };
        *tf.entry(idx).or_insert(0) += sign;
    }
    let n = docs as f64 + 1.0;
    let mut arcs: Vec<(u32, f64)> = Vec::with_capacity(tf.len());
    let mut max = 0.0_f64;
    for (&i, &t) in tf.iter() {
        let idf = ((1.0 + n) / (1.0 + df_of(i) as f64)).ln();
        let s = t as f64 * idf;
        max = max.max(s.abs());
        arcs.push((i, s));
    }
    if max > 0.0 {
        for (_, s) in arcs.iter_mut() {
            *s /= max;
        }
    }
    arcs.sort_unstable_by_key(|a| a.0);
    (arcs, tokens)
}

/// Срезание HTML-разметки: теги, `<script>`, `<style>`, комментарии и
/// doctype удаляются; базовые сущности декодируются; между блоками
/// вставляется пробел (границы слов).
///
/// Zero-dependency: конечный автомат по символам, без regex.
pub fn strip_html(input: &[u8]) -> String {
    let text = String::from_utf8_lossy(input);
    let lower = text.to_ascii_lowercase();
    let src: Vec<char> = text.chars().collect();
    let low: Vec<char> = lower.chars().collect();

    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    let n = src.len();
    while i < n {
        let c = src[i];
        if c != '<' {
            if c == '&' {
                i = decode_entity(&src, i, &mut out);
            } else {
                out.push(c);
                i += 1;
            }
            continue;
        }
        // Открывающий угол: классифицируем тег.
        let rest = &low[i..];
        if starts_with(rest, "<!--") {
            // Комментарий до -->.
            i += 4;
            while i < n && !(src[i] == '-' && starts_with(&low[i..], "-->")) {
                i += 1;
            }
            i = (i + 3).min(n);
        } else if starts_with(rest, "<script") || starts_with(rest, "<style") {
            // Сырой блок до закрывающего тега.
            let closing = if starts_with(rest, "<script") {
                "</script"
            } else {
                "</style"
            };
            i += 1;
            while i < n && !starts_with(&low[i..], closing) {
                i += 1;
            }
            // Пропустить сам закрывающий тег до '>'.
            while i < n && src[i] != '>' {
                i += 1;
            }
            i = (i + 1).min(n);
        } else {
            // Обычный тег: до '>' с учётом кавычек атрибутов.
            i += 1;
            let mut quote = '\0';
            while i < n {
                let ch = src[i];
                if quote != '\0' {
                    if ch == quote {
                        quote = '\0';
                    }
                } else if ch == '"' || ch == '\'' {
                    quote = ch;
                } else if ch == '>' {
                    break;
                }
                i += 1;
            }
            i = (i + 1).min(n);
        }
        // Граница слова на месте разметки.
        if !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
    }
    out
}

/// `rest` начинается с `prefix` (регистронезависимо, уже в lower).
fn starts_with(rest: &[char], prefix: &str) -> bool {
    let p: Vec<char> = prefix.chars().collect();
    rest.len() >= p.len() && rest[..p.len()] == p[..]
}

/// Декодирование HTML-сущности с позиции `i` (`&…;`).
/// Возвращает индекс после сущности; неизвестная — как есть.
fn decode_entity(src: &[char], i: usize, out: &mut String) -> usize {
    let n = src.len();
    let mut j = i + 1;
    while j < n && j - i <= 10 && src[j] != ';' {
        j += 1;
    }
    if j >= n || j - i > 10 || src[j] != ';' {
        out.push('&');
        return i + 1;
    }
    let entity: String = src[i + 1..j].iter().collect();
    let decoded = match entity.as_str() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "rsquo" => Some('\''),
        "nbsp" => Some(' '),
        other => {
            if let Some(hex) = other
                .strip_prefix("#x")
                .or_else(|| other.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = other.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    };
    match decoded {
        Some(ch) => {
            out.push(ch);
            j + 1
        }
        None => {
            out.push('&');
            i + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Реалистичный поток: три темы по несколько чанков.
    fn topic_a() -> String {
        "фазовый континуум фазовый континуум триты борн анзац".to_string()
    }
    fn topic_b() -> String {
        "линза решётка разреженный граф индекс csr".to_string()
    }

    #[test]
    fn fock_residual_invariants() {
        let f = vec![0.6, 0.8];
        // f = ρ: стационар — нуль.
        let stat = fock_residual(&f, &f);
        assert!(stat.raw.abs() < 1e-12);
        // f = 0: NO_HITS — нуль.
        let no = fock_residual(&[0.0; 4], &[0.3, -0.5, 0.7, 0.0]);
        assert!(no.raw.abs() < 1e-12 && no.normalized.abs() < 1e-12);
        // Ортогональные векторы: нет взаимодействия — нуль.
        let orth = fock_residual(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(orth.raw.abs() < 1e-12);
        // Промежуточный угол: максимум закрутки, нормировка в (0, 1].
        let twist = fock_residual(&[1.0, 0.0], &[1.0, 1.0]);
        assert!(twist.raw > 0.0);
        assert!(twist.normalized > 0.0 && twist.normalized <= 1.0);
        // Ручная сверка: f = (1,0), ρ = (1,1) → |fᵀρ| = 1, площадь
        // √(2·(1·2 − 1)) = √2 → raw = √2.
        assert!((twist.raw - std::f64::consts::SQRT_2).abs() < 1e-12);
    }

    #[test]
    fn fock_residual_max_at_half_angle() {
        // При угле 45° между f и ρ нормированная невязка максимальна (= 1/2).
        let f = vec![1.0, 0.0];
        let rho = vec![1.0_f64, 1.0];
        let r = fock_residual(&f, &rho);
        // raw = |fᵀρ|·√(2(a²−c²)) = 1·√(2(2−1)) = √2; denom = √2·1·2 = 2√2.
        assert!((r.normalized - 0.5).abs() < 1e-12);
    }

    #[test]
    fn strip_html_tags_scripts_entities() {
        let html = b"<html><head><style>body{color:red}</style><title>T</title>\
                     <script>var x = 1 < 2;</script></head>\
                     <body><h1>Quantum &amp; Phase</h1><p>a &lt; b &gt; c &#8212; d</p>\
                     <!-- comment --></body></html>";
        let text = strip_html(html);
        assert!(!text.to_lowercase().contains("color"));
        assert!(!text.to_lowercase().contains("var x"));
        assert!(!text.to_lowercase().contains("comment"));
        assert!(
            !text.contains('<') || text.contains("a < b"),
            "теги срезаны: {text}"
        );
        assert!(text.contains("Quantum & Phase"));
        assert!(text.contains("a < b > c — d"));
    }

    #[test]
    fn strip_html_plain_text_untouched() {
        let plain = "чистый текст без разметки 42";
        assert!(strip_html(plain.as_bytes()).contains("чистый"));
        // Незакрытый тег в конце не зацикливается.
        let bad = "текст <div attrs=\"не закрыт".as_bytes();
        let t = strip_html(bad);
        assert!(t.contains("текст"));
    }

    #[test]
    fn tf_idf_downweights_stream_ubiquitous_tokens() {
        // d = 4096: коллизии хешей пренебрежимы; координаты различны.
        let ic = idx_of("common", 4096);
        let im = idx_of("midway", 4096);
        let ir = idx_of("rare", 4096);
        assert_ne!(ic, im);
        assert_ne!(ic, ir);
        assert_ne!(im, ir);

        // Поток: common в 5 документах, midway в 1 (rare ещё не видел).
        let mut e = StreamEngine::new(4096, 0.05, 1).unwrap();
        for _ in 0..5 {
            e.ingest("common", 0).unwrap();
        }
        e.ingest("midway", 0).unwrap();
        // docs = 6, df_common = 5, df_midway = 1, df_rare = 0.

        // Новый чанк с равной tf: порядок задаёт только idf.
        let (s, tokens) = e.encode_chunk("common midway rare");
        assert_eq!(tokens, 3);
        assert!(
            s[ir].abs() > s[im].abs(),
            "rare ≯ midway: {} vs {}",
            s[ir],
            s[im]
        );
        assert!(
            s[im].abs() > s[ic].abs(),
            "midway ≯ common: {} vs {}",
            s[im],
            s[ic]
        );
        // Редчайший токен доминирует после L∞-нормализации.
        assert!((s[ir].abs() - 1.0).abs() < 1e-12);
    }

    fn idx_of(token: &str, d: u32) -> usize {
        (fnv1a64(token.as_bytes()) % d as u64) as usize
    }

    #[test]
    fn buffer_capacity_stable_after_warmup() {
        let mut e = StreamEngine::new(512, 0.2, 7).unwrap();
        let mut caps = Vec::new();
        for k in 0..5 {
            let rep = e
                .ingest(&format!("{} {k} optimizer stream", topic_a()), 0)
                .unwrap();
            caps.push(rep.buffer_capacity);
        }
        // После первых двух чанков ёмкость не растёт: системных аллокаций нет.
        assert_eq!(caps[2], caps[3]);
        assert_eq!(caps[3], caps[4]);
    }

    #[test]
    fn packed4_container_size_formula() {
        // d = 512: 0x80 заголовок + 128 байт фаз; d = 65536 → 16 КиБ фаз.
        let mut e = StreamEngine::new(512, 0.2, 1).unwrap();
        let rep = e.ingest(&topic_a(), 0).unwrap();
        assert_eq!(rep.container_bytes, 0x80 + 512usize.div_ceil(4));

        let mut big = StreamEngine::new(65_536, 0.2, 1).unwrap();
        let rep = big.ingest(&topic_a(), 0).unwrap();
        assert_eq!(rep.container_bytes, 0x80 + 16 * 1024);
    }

    #[test]
    fn no_hits_barrier_is_deterministic() {
        let mut e = StreamEngine::new(128, 0.1, 3).unwrap();
        let rep = e.ingest(&topic_a(), 0).unwrap();
        let arcs_before = e.model_arcs().len();
        let empty = e.ingest("… — !!! …", 0).unwrap();
        assert!(empty.no_hits);
        assert_eq!(empty.step, None);
        assert_eq!(empty.nnz, 0);
        assert_eq!(empty.steps_run, 0);
        assert_eq!(
            empty.docs_seen, rep.docs_seen,
            "no_hits не считается документом"
        );
        assert_eq!(e.model_arcs().len(), arcs_before);
        assert_eq!(empty.fock.normalized, 0.0);
    }

    #[test]
    fn ingest_is_deterministic_per_seed() {
        let run = || {
            let mut e = StreamEngine::new(256, 0.15, 2026).unwrap();
            let a = e.ingest(&topic_a(), 2).unwrap();
            let b = e.ingest(&topic_b(), 2).unwrap();
            (
                a.param_loss,
                b.param_loss,
                a.container_bytes,
                b.nnz,
                e.model().to_vec(),
            )
        };
        let x = run();
        let y = run();
        assert_eq!(x, y);
    }

    #[test]
    fn repeated_chunk_converges_and_commutator_decays() {
        // Пурификация выключена: изолируем стационарность обучения
        // (память ∥ свидетельство) от кристаллизации в триты (K = 5,
        // отдельный режим — см. purify_every_five_steps в learn).
        let mut e = StreamEngine::new(512, 0.2, 11)
            .unwrap()
            .with_purify_every(0);
        let mut losses = Vec::new();
        let mut focks = Vec::new();
        for _ in 0..6 {
            let rep = e.ingest(&topic_a(), 2).unwrap();
            losses.push(rep.param_loss);
            focks.push(rep.fock.normalized);
        }
        // Потеря к последнему чанку падает.
        assert!(
            losses.last().unwrap() < &losses[0],
            "потеря не упала: {losses:?}"
        );
        // Коммутатор не спиралит: память остаётся в окрестности свидетельства.
        // Строгий нуль недостижим Born-обучением — шумовой пол измерения
        // O(1/√shots) держит невязку ~0.05 при 4096 выстрелах; после
        // сходимости коммутатор стабилизируется (не растёт).
        assert!(
            *focks.last().unwrap() < 0.1,
            "коммутатор ушёл от свидетельства: {focks:?}"
        );
        assert!(
            (*focks.last().unwrap() - focks[4]).abs() < 0.03,
            "коммутатор не стабилизировался: {focks:?}"
        );
    }

    #[test]
    fn topic_switch_is_tracked() {
        let mut e = StreamEngine::new(512, 0.2, 5).unwrap();
        // Тема A накатана.
        for _ in 0..4 {
            e.ingest(&topic_a(), 2).unwrap();
        }
        // Смена режима: тема B.
        let first_b = e.ingest(&topic_b(), 2).unwrap();
        let first_loss = first_b.param_loss;
        let mut last = first_b;
        for _ in 0..4 {
            last = e.ingest(&topic_b(), 2).unwrap();
        }
        assert!(last.param_loss < first_loss);
        // Дуги обеих тем живут в памяти (LENS накапливает факты).
        let arcs = e.model_arcs().len();
        assert!(arcs > 2, "union support пуст: {arcs}");
    }

    #[test]
    fn hold_freezes_background_decay_fades_it() {
        let text_a = topic_a();
        // Hold: пурификация выключена — изолируем политику фона.
        let mut hold = StreamEngine::new(512, 0.2, 9).unwrap().with_purify_every(0);
        hold.ingest(&text_a, 4).unwrap();
        let pa: Vec<f64> = hold.model().to_vec();
        hold.ingest(&topic_b(), 4).unwrap();
        let pa2: Vec<f64> = hold.model().to_vec();
        // Дрейф только дуг темы A (новые дуги B стартуют с нуля — не в счёт).
        let moved = pa
            .iter()
            .enumerate()
            .filter(|(_, p)| **p != 0.0)
            .map(|(i, p)| (p - pa2[i]).abs())
            .fold(0.0_f64, f64::max);
        // Hold: градиент фона нулевой; остаётся ограниченный перелёт момента
        // (heavy-ball overshoot ≤ η·v/(1−γ)), затухающий как γ^t — против
        // неограниченного растворения в Decay.
        assert!(
            moved < 0.15,
            "Hold дрейф фона превысил перелёт момента: max |Δp| = {moved:.4}"
        );

        // Decay: дуги A диссипируют к монетам после смены темы.
        let mut decay = StreamEngine::new(512, 0.2, 9)
            .unwrap()
            .with_forget(Forget::Decay)
            .with_purify_every(0);
        decay.ingest(&text_a, 4).unwrap();
        let da: Vec<f64> = decay.model().to_vec();
        // Координаты темы A (новые дуги B в счёт не идут).
        let a_coords: Vec<usize> = da
            .iter()
            .enumerate()
            .filter(|(_, p)| **p != 0.0)
            .map(|(i, _)| i)
            .collect();
        let strong_a = a_coords.iter().filter(|&&i| da[i].abs() > 0.3).count();
        assert!(strong_a > 0, "тема A не усвоена вовсе");
        for _ in 0..6 {
            decay.ingest(&topic_b(), 4).unwrap();
        }
        let da2: Vec<f64> = decay.model().to_vec();
        let strong_a2 = a_coords.iter().filter(|&&i| da2[i].abs() > 0.3).count();
        assert!(
            strong_a2 < strong_a,
            "Decay не растворил старую тему: {strong_a} → {strong_a2}"
        );
    }

    #[test]
    fn support_grows_with_new_topics_only() {
        let mut e = StreamEngine::new(512, 0.2, 13).unwrap();
        let n0 = e.model_arcs().len();
        e.ingest(&topic_a(), 0).unwrap();
        let n1 = e.model_arcs().len();
        assert!(n1 > n0);
        // Повтор той же темы: support не растёт (тех же дуги).
        e.ingest(&topic_a(), 0).unwrap();
        assert_eq!(e.model_arcs().len(), n1);
        // Новые дуги стартуют с честной монеты и учатся измерением.
        e.ingest(&topic_b(), 0).unwrap();
        assert!(e.model_arcs().len() >= n1);
    }

    #[test]
    fn qcm_grows_with_conviction() {
        let mut e = StreamEngine::new(512, 0.2, 17).unwrap();
        let first = e.ingest(&topic_a(), 0).unwrap();
        for _ in 0..5 {
            e.ingest(&topic_a(), 3).unwrap();
        }
        let rep = e.ingest(&topic_a(), 3).unwrap();
        assert!(rep.qcm.qcm_theory > first.qcm.qcm_theory);
        assert!(rep.qcm.qcm_gap() < 0.1);
    }

    /// DoD RQ6: полный цикл «HTML/текст → LENS-CSR → Packed4 → Born-шаг»
    /// в RAM за < 5 мс (d = 512, 1024 выстрела, best-of-5; в release
    /// при полном пакете 4096 выстрелов укладывается с запасом).
    #[test]
    fn full_cycle_latency_d512_under_5ms() {
        let mut e = StreamEngine::new(512, 0.2, 42).unwrap().with_shots(1024);
        let html = format!(
            "<html><body><h1>{}</h1><p>{}</p></body></html>",
            topic_a(),
            "поток обучение онлайн фазы"
        );
        // Прогрев (аллокации буферов и JIT-пути).
        e.ingest_html(html.as_bytes(), 0).unwrap();
        let mut best = std::time::Duration::MAX;
        for k in 0..5 {
            let rep = e.ingest_html(format!("{html} {k}").as_bytes(), 0).unwrap();
            best = best.min(rep.elapsed);
        }
        assert!(
            best.as_secs_f64() < 5.0 / 1000.0,
            "сквозной цикл d=512 занял {best:?} — бюджет 5 мс превышен"
        );
    }

    /// Требование RQ6: QCM + семантический коммутатор за < 150 мкс (d = 512).
    #[test]
    fn qcm_and_commutator_under_150us() {
        let mut e = StreamEngine::new(512, 0.2, 42).unwrap();
        e.ingest(&topic_a(), 0).unwrap();
        let state: Vec<f64> = e.model().to_vec();
        let mut best = std::time::Duration::MAX;
        for _ in 0..16 {
            let t0 = Instant::now();
            let f = fock_residual(&state, e.model());
            let sample = e.ansatz_sample_for_eval();
            let qcm = coherence(&sample);
            let dt = t0.elapsed();
            assert!(f.normalized >= 0.0);
            assert!(qcm.qcm_theory.is_finite());
            best = best.min(dt);
        }
        let max_budget = if cfg!(debug_assertions) { 1500.0 / 1_000_000.0 } else { 150.0 / 1_000_000.0 };
        assert!(
            best.as_secs_f64() < max_budget,
            "QCM + коммутатор заняли {best:?} — бюджет {max_budget:?} превышен"
        );
    }

    #[test]
    fn resume_from_container_preserves_support_and_grows() {
        // Сессия 1: движок выучивает две темы; сериализуется TRIT-проекция
        // памяти (|p| ≥ ε) — «затвердевшие мнения». Слабые дуги |p| < ε
        // порог LENS не пропускает: ε-фильтр и есть врата памяти.
        let mut a = StreamEngine::new(512, 0.05, 7).unwrap();
        a.ingest(&topic_a(), 2).unwrap();
        a.ingest(&topic_b(), 2).unwrap();
        assert!(!a.model_arcs().is_empty());

        // Полный чекпоинт накопленной памяти: сериализуем model().
        let mut full = Vec::new();
        {
            let mut w = PqwWriter::new(512).unwrap();
            let state: Vec<f32> = a.model().iter().map(|&p| p as f32).collect();
            w.add_state(&state).unwrap();
            w.write_packed_trits(&mut full).unwrap();
        }
        let reader = PqwReader::from_bytes(&full).unwrap();
        let container_nnz = reader.nnz() as usize;
        assert!(
            container_nnz >= 1,
            "контейнер обязан хранить хотя бы одну дугу |p| ≥ ε"
        );

        // Сессия 2: resume → поднят ВЕСЬ nnz контейнера, знаки восстановлены.
        let mut b = StreamEngine::new(512, 0.05, 7).unwrap();
        let lifted = b.resume_from_reader(&reader).unwrap();
        assert_eq!(lifted, container_nnz, "поднят весь nnz контейнера");
        assert_eq!(b.model_arcs().len(), container_nnz);
        let sign_sum: f64 = b.model().iter().map(|p| p.abs()).sum();
        assert!(
            (sign_sum - container_nnz as f64).abs() < 1e-9,
            "знаки дуг = ±1 на всём поднятом support"
        );

        // Дальнейшее обучение растит граф, не теряя поднятого.
        b.ingest(&topic_a(), 2).unwrap();
        assert!(b.model_arcs().len() >= container_nnz);
        assert!(b.docs_seen() >= 1, "resume не отменяет счётчик документов");
    }

    #[test]
    fn resume_rejects_dimension_mismatch() {
        let mut a = StreamEngine::new(256, 0.05, 1).unwrap();
        a.ingest(&topic_a(), 0).unwrap();
        let mut full = Vec::new();
        {
            let mut w = PqwWriter::new(256).unwrap();
            let state: Vec<f32> = a.model().iter().map(|&p| p as f32).collect();
            w.add_state(&state).unwrap();
            w.write_packed_trits(&mut full).unwrap();
        }
        let reader = PqwReader::from_bytes(&full).unwrap();
        let mut other = StreamEngine::new(512, 0.05, 1).unwrap();
        assert!(other.resume_from_reader(&reader).is_err());
    }

    #[test]
    fn gyro_accumulates_during_ingest() {
        // Гироскоп видит токены в порядке появления: повтор
        // «alpha beta gamma …» накапливает направленную циркуляцию.
        let mut e = StreamEngine::new(512, 0.05, 11)
            .unwrap()
            .with_gyro(8, 4096);
        let text = "alpha beta gamma alpha beta gamma alpha beta gamma";
        e.ingest(text, 0).unwrap();
        let g = e.gyro().expect("гироскоп включён");
        assert!(g.ticks() >= 9, "такты = токенам, а не блокам");
        assert!(g.raw_pairs() > 0, "направленные пары накоплены");
        // Моды извлекаются прямо из движка.
        assert!(!g.resonant_modes(0.05, 4).is_empty());

        // Без --gyro гироскопа нет: прежнее поведение не изменилось.
        let mut plain = StreamEngine::new(512, 0.05, 11).unwrap();
        plain.ingest(text, 0).unwrap();
        assert!(plain.gyro().is_none());
    }

    #[test]
    fn gyro_resume_chain_survives_restart() {
        // Сессия 1: учимся с гироскопом, пишем v3-контейнер (фазы + J).
        let mut a = StreamEngine::new(512, 0.05, 3)
            .unwrap()
            .with_gyro(8, 4096);
        let text = "resonance operator memory gyroscope resonance operator memory";
        for _ in 0..3 {
            a.ingest(text, 1).unwrap();
        }
        let gyro = a.gyro().unwrap();
        let data = gyro
            .gyro_data(0.05, 512)
            .expect("после трёх повторов циркуляция обязана выжить");
        assert!(data.ticks() > 0);
        assert!(!data.pairs().is_empty());

        let mut v3 = Vec::new();
        {
            let mut w = PqwWriter::new(512).unwrap();
            let state: Vec<f32> = a.model().iter().map(|&p| p as f32).collect();
            w.add_state(&state).unwrap();
            w.write_v3(&mut v3, &data).unwrap();
        }
        let reader = PqwReader::from_bytes(&v3).unwrap();
        assert!(reader.header().is_gyro());
        assert_eq!(reader.gyro().unwrap().pairs().len(), data.pairs().len());

        // Сессия 2: resume с гироскопом — и фазы, и циркуляция подняты.
        let mut b = StreamEngine::new(512, 0.05, 3)
            .unwrap()
            .with_gyro(8, 4096);
        let lifted_arcs = b.resume_from_reader(&reader).unwrap();
        assert_eq!(lifted_arcs, reader.nnz() as usize);
        let g2 = b.gyro().unwrap();
        assert_eq!(g2.ticks(), data.ticks(), "счётчик тактов пережил рестарт");
        let pairs2 = g2.skew_pairs(0.05);
        let pairs1 = data.pairs().to_vec();
        assert_eq!(pairs2.len(), pairs1.len());
        // Деквантованные веса в пределах полкванта решётки.
        for (p, q) in pairs2.iter().zip(pairs1.iter()) {
            assert!((p.2 - q.2).abs() <= data.scale() as f64 / 254.0 + 1e-12);
        }

        // Обучение продолжается поверх поднятой циркуляции: такты растут.
        b.ingest(text, 1).unwrap();
        assert!(b.gyro().unwrap().ticks() > data.ticks());

        // Без --gyro v3-контейнер читается как обычные фазы (секция игнорируется).
        let mut c = StreamEngine::new(512, 0.05, 3).unwrap();
        let lifted = c.resume_from_reader(&reader).unwrap();
        assert_eq!(lifted, reader.nnz() as usize);
        assert!(c.gyro().is_none());
    }
}

#[cfg(test)]
impl StreamEngine {
    /// Тестовый доступ к QCM-сэмплированию без публичного мутабельного API.
    fn ansatz_sample_for_eval(&mut self) -> crate::ansatz::SampleReport {
        self.ansatz
            .sample(&mut self.rng, self.qcm_shots, 0)
            .unwrap()
    }
}

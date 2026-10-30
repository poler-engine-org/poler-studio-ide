//! RQ15: квантованный curriculum — born-шаг прямо в тритовой решётке.
//!
//! Замыкает цепочку RQ8 → RQ14: плотный LENS-граф (RQ8) сжат в 2-битные
//! триты (RQ13), углы Блоха разворачиваются LUT-ом на лету (RQ14) — теперь
//! и **петля обучения** сведена к разреженному шагу
//! [`born_step_packed4_sparse`]: обучение происходит В решётке, а не рядом
//! с ней. Нет плотного f64-вектора параметров, нет анзаца, нет
//! statevector: память = Packed4-байты = фазовая секция контейнера `.pqw`
//! v2, бит-в-бит. Чекпоинт [`QuantizedCurriculum::checkpoint`] — выписка
//! решётки без переквантования; resume — копирование байтов обратно.
//! «Смысловой кристалл» из манифеста: состояние меняется от каждого
//! наблюдения и переживает любой сброс.
//!
//! ## Петля (ingest)
//!
//! ```text
//! text → TF-IDF дуги (ε-ворота)                     [свидетельство чанка]
//!      → Born-измерение решётки                      [полюса детерминированы,
//!                                                      фон — честная монета]
//!      → суррогатный градиент g = p_emp − target     [квадратичная цель]
//!      → момент v ← γ·v − g                          [Π_Λ: момент живёт только
//!                                                      на дугах свидетельства]
//!      → born_step_packed4_sparse(решётка, v, η)     [кристалл обновлён]
//! ```
//!
//! ## Физика решётки (гистерезис π/3)
//!
//! В плотном контуре RQ5/RQ6 чувствительность задаётся цепным правилом
//! `dp/dθ = −√(1−p²)`: у полюсов ±1 оно вырождается в нуль — фаза залипает.
//! В квантованном контуре ту же роль играет **геометрия решётки**:
//! перещёлкивание происходит при пересечении границы `|cos θ| = 0.5`,
//! то есть при `|η·v| ≥ π/6` от экватора (Zero → ±) и `|η·v| ≥ π/3` от
//! полюса (полюс → Zero — гистерезис вдвое шире: «затвердевшее мнение»
//! устойчиво к единичному несогласию). Смена знака проходит только через
//! экватор: полюс → Zero → противоположный полюс — телепортации фазы не
//! существует. Слабое свидетельство накапливается моментом до порога
//! кристаллизации: стационар `v* = |g|/(1−γ)` при `η = 0.6, γ = 0.5`
//! кристаллизует дуги с `|target| ≳ 0.44`; слабее — честный фон Zero
//! (мёртвая зона квантования — это цена 2 бит, и она честная).
//!
//! McWeeny-пурификация здесь — тождество: точки решётки {−1, 0, +1} —
//! неподвижные точки `p ← 2(3λ² − 2λ³) − 1`; инвариант `P² = P` выполнен
//! самим представлением.
//!
//! ## Стоимость
//!
//! Измерение дёшевеет по мере обучения: полюса детерминированы (ноль
//! ГПСЧ-выстрелов), честные монеты сэмплируются только у ещё не
//! кристаллизованных дуг. Память движка: решётка `⌈d/4⌉` байт
//! (персистентная) + разреженные `HashMap` момента и TF-IDF статистики —
//! `O(словарь)`, а не `O(d)`.
//!
//! ## Пример
//!
//! ```
//! use pqc::qcurriculum::QuantizedCurriculum;
//!
//! let mut qc = QuantizedCurriculum::new(64, 0.05, 42).unwrap();
//! // Неравные частоты: сильные дуги кристаллизуются сразу, слабые —
//! // накапливаются моментом, совсем слабые остаются честным фоном.
//! let text = "phase phase phase lattice lattice born trit crystal";
//! let first = qc.ingest(text, 0).unwrap();
//! assert!(first.moved > 0, "кристалл двинулся с первого наблюдения");
//!
//! // Повторение опыта: невязка решётки к цели падает.
//! for _ in 0..8 {
//!     qc.ingest(text, 0).unwrap();
//! }
//! let last = qc.ingest(text, 0).unwrap();
//! assert!(last.param_loss < first.param_loss);
//! assert!(qc.nnz() > 0, "в решётке есть затвердевшие дуги");
//!
//! // Память = контейнер, бит-в-бит.
//! let ckpt = qc.checkpoint().unwrap();
//! assert_eq!(&ckpt[pqw::HEADER_SIZE..], qc.lattice());
//! ```

use std::collections::HashMap;
use std::time::Instant;

use pqw::phase::Trit;
use pqw::reader::PqwReader;
use pqw::trit_bloch::{counts, p_at, trit_at, TritCounts};
use pqw::writer::PqwWriter;

use crate::bloch_stream::born_step_packed4_sparse;
use crate::error::{PqcError, Result};
use crate::rng::Rng;
use crate::stream_engine::tfidf_arcs;

/// Квантованный учебный движок: born-шаг в 2-битной решётке тритов.
///
/// Регламент по умолчанию: `η = 0.6` (калибровка RQ14), `γ = 0.5`
/// (полюсная защита RQ5), `shots = 4096`, политика Hold (фон заморожен).
/// Шаги `ingest` детерминированы сидом: одинаковый поток данных даёт
/// побитово одинаковую решётку.
pub struct QuantizedCurriculum {
    d_pol: u32,
    epsilon: f32,
    eta: f64,
    gamma: f64,
    shots: u64,
    seed: u64,
    /// Обучаемая память: Packed4-байты, `⌈d/4⌉` штук — коды v2.
    lattice: Vec<u8>,
    /// Кинетическая память (момент θ-пространства), только на support.
    velocity: HashMap<u32, f64>,
    /// TF-IDF статистика потока (дотронутые координаты).
    doc_freq: HashMap<u32, u32>,
    docs: u64,
    ingests: usize,
    rng: Rng,
}

/// Отчёт одного ingest-а: «обучение произошло» в числах.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizedReport {
    /// Токенов в чанке.
    pub tokens: usize,
    /// Дуг свидетельства чанка (после ε-ворот).
    pub nnz: usize,
    /// Документов усвоено (включая текущий).
    pub docs_seen: u64,
    /// Шагов born-петли (1 + extra_steps).
    pub steps_run: usize,
    /// Тритов, сменивших код за все шаги ingest-а.
    pub moved: usize,
    /// Доля перещёлкнувшихся тритов (от носителя шага).
    pub moved_frac: f64,
    /// Суммарный «плавающий» сдвиг Σ|θ′ − θ| до переквантования.
    pub theta_shift: f64,
    /// Ненулевых тритов решётки до ingest-а.
    pub lattice_nnz_before: usize,
    /// Ненулевых тритов решётки после ingest-а.
    pub lattice_nnz_after: usize,
    /// Невязка решётки к цели: ½‖p_lattice − target‖² на дугах чанка.
    pub param_loss: f64,
    /// Чанк не дал свидетельства (пустой поток токенов).
    pub no_hits: bool,
    /// Сквозная задержка.
    pub elapsed: std::time::Duration,
}

impl QuantizedCurriculum {
    /// Новый движок: `d_pol ≥ 1`, порог LENS `ε ∈ [0, 1]`, сид ГПСЧ.
    pub fn new(d_pol: u32, epsilon: f32, seed: u64) -> Result<QuantizedCurriculum> {
        if d_pol == 0 {
            return Err(PqcError::EmptyState);
        }
        if !epsilon.is_finite() || epsilon < 0.0 || epsilon > 1.0 {
            return Err(PqcError::BadPhase(epsilon as f64));
        }
        Ok(QuantizedCurriculum {
            d_pol,
            epsilon,
            eta: 0.6,
            gamma: 0.5,
            shots: 4096,
            seed,
            lattice: vec![0u8; (d_pol as usize).div_ceil(4)],
            velocity: HashMap::new(),
            doc_freq: HashMap::new(),
            docs: 0,
            ingests: 0,
            rng: Rng::seed_from_u64(seed),
        })
    }

    /// Выстрелов на Born-измерение фона (уровни curriculum меняют бюджет).
    ///
    /// В отличие от плотного движка ГПСЧ-поток НЕ сбрасывается: траектория
    /// детерминирована последовательностью ingest-ов, а не настройками.
    pub fn set_shots(&mut self, shots: u64) -> &mut Self {
        self.shots = shots.max(1);
        self
    }

    /// Гиперпараметры петли: шаг η и трение γ.
    pub fn set_hyper(&mut self, eta: f64, gamma: f64) -> &mut Self {
        self.eta = eta;
        self.gamma = gamma;
        self
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Порог LENS.
    pub fn epsilon(&self) -> f32 {
        self.epsilon
    }

    /// Шаг потока η.
    pub fn eta(&self) -> f64 {
        self.eta
    }

    /// Трение потока γ.
    pub fn gamma(&self) -> f64 {
        self.gamma
    }

    /// Выстрелов на измерение.
    pub fn shots(&self) -> u64 {
        self.shots
    }

    /// Сид ГПСЧ (детерминизм траектории).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Число усвоенных документов.
    pub fn docs_seen(&self) -> u64 {
        self.docs
    }

    /// Число выполненных ingest-ов.
    pub fn ingests(&self) -> usize {
        self.ingests
    }

    /// Решётка тритов — обучаемая память, Packed4-байты (`⌈d/4⌉` штук).
    ///
    /// Бит-в-бит совпадает с фазовой секцией контейнера v2
    /// (см. [`QuantizedCurriculum::checkpoint`]).
    pub fn lattice(&self) -> &[u8] {
        &self.lattice
    }

    /// Число байтов решётки.
    pub fn lattice_bytes(&self) -> usize {
        self.lattice.len()
    }

    /// Счётчики решётки (+1 / −1 / 0).
    pub fn counts(&self) -> TritCounts {
        counts(&self.lattice, self.d_pol as usize)
    }

    /// Число ненулевых тритов решётки (кристаллизованные дуги).
    pub fn nnz(&self) -> usize {
        let c = self.counts();
        c.pos + c.neg
    }

    /// Значение `p = cos θ` трита `i` (LUT: {−1, 0, +1}).
    pub fn p_at(&self, i: u32) -> f64 {
        p_at(&self.lattice, i as usize)
    }

    /// Число координат с живым моментом (диагностика Π_Λ).
    pub fn velocity_len(&self) -> usize {
        self.velocity.len()
    }

    /// Чекпоинт: контейнер v2, фазовая секция = решётка бит-в-бит.
    ///
    /// Нулевая переквантование: кристаллизованные триты выписываются как
    /// дуги `±1`, фон — честные монеты. Заголовок несёт гиперпараметры
    /// (`η`, `γ`, `ρ = 1` — Hold, `ε`).
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        let mut w = PqwWriter::new(self.d_pol)?
            .hyperparams(self.eta as f32, self.gamma as f32, 1.0, self.epsilon);
        let d = self.d_pol as usize;
        for i in 0..d {
            match trit_at(&self.lattice, i) {
                Trit::Pos => {
                    w.add_phase(i as u32, 1.0)?;
                }
                Trit::Neg => {
                    w.add_phase(i as u32, -1.0)?;
                }
                Trit::Zero => {}
            }
        }
        let mut buf = Vec::with_capacity(pqw::HEADER_SIZE + self.lattice.len());
        w.write_packed_trits(&mut buf)?;
        Ok(buf)
    }

    /// Resume: поднять решётку из `.pqw`-контейнера v2/v3 бит-в-бит.
    ///
    /// Топологическая секция v3 (гироскоп) игнорируется — квантованный
    /// контур хранит только фазы. Момент и TF-IDF статистика стартуют
    /// с нуля: «мнение пережило рестарт, инерция — нет». Возвращает
    /// число ненулевых тритов поднятой решётки.
    pub fn resume_from_reader(&mut self, reader: &PqwReader) -> Result<usize> {
        if reader.d_pol() != self.d_pol {
            return Err(PqcError::LengthMismatch {
                expected: self.d_pol as usize,
                actual: reader.d_pol() as usize,
            });
        }
        if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
            return Err(PqcError::Unsupported {
                what: "quantized curriculum requires Packed4 container (v2/v3)",
            });
        }
        let phase = reader.phase_bytes();
        let need = (self.d_pol as usize).div_ceil(4);
        if phase.len() != need {
            return Err(PqcError::LengthMismatch {
                expected: need,
                actual: phase.len(),
            });
        }
        self.lattice.copy_from_slice(phase);
        // Инерция не переживает рестарт (Π_Λ: момент только на живом
        // свидетельстве), TF-IDF статистика тоже — idf перезапускается.
        self.velocity.clear();
        self.doc_freq.clear();
        self.docs = 0;
        Ok(self.nnz())
    }

    /// Один ingest живых данных: свидетельство → измерение → born-шаг.
    ///
    /// `extra_steps` — дополнительные шаги петли к той же цели
    /// (уточнение фаз, как у плотного движка RQ8).
    pub fn ingest(&mut self, text: &str, extra_steps: usize) -> Result<QuantizedReport> {
        let t0 = Instant::now();
        let d = self.d_pol as usize;
        let lattice_nnz_before = self.nnz();

        // 1. TF-IDF свидетельство чанка (до ε-ворот — для parity с RQ8).
        let (pre_arcs, tokens) = tfidf_arcs(self.d_pol, self.docs, |i| {
            self.doc_freq.get(&i).copied().unwrap_or(0)
        }, text);

        // 2. Барьер NO_HITS: пустое свидетельство — детерминированный отказ
        //    (документ не учитывается, как в плотном движке).
        if pre_arcs.is_empty() {
            return Ok(QuantizedReport {
                tokens,
                nnz: 0,
                docs_seen: self.docs,
                steps_run: 0,
                moved: 0,
                moved_frac: 0.0,
                theta_shift: 0.0,
                lattice_nnz_before,
                lattice_nnz_after: lattice_nnz_before,
                param_loss: 0.0,
                no_hits: true,
                elapsed: t0.elapsed(),
            });
        }

        // 3. ε-ворота LENS: |s| ≥ ε (сравнение в f32 — как add_state v2).
        let evidence: Vec<(u32, f64)> = pre_arcs
            .iter()
            .copied()
            .filter(|&(_, s)| (s as f32).abs() >= self.epsilon)
            .collect();
        // L∞-нормализация гарантирует max |s| = 1 ≥ ε — свидетельство
        // непусто, если поток токенов непуст.
        debug_assert!(!evidence.is_empty());

        // 4. Петля born-шагов: измерение → момент → шаг в решётке.
        let steps_run = 1 + extra_steps;
        let mut moved_total = 0usize;
        let mut theta_shift_total = 0.0_f64;
        for _ in 0..steps_run {
            // Π_Λ строгая: момент живёт ТОЛЬКО на дугах свидетельства —
            // вне носителя обнуляется (фаза не дрейфует от инерции истории).
            let mut vel: HashMap<u32, f64> = HashMap::with_capacity(evidence.len());
            for &(i, t) in &evidence {
                let p = p_at(&self.lattice, i as usize);
                let p_emp = self.measure_coord(p);
                let g = p_emp - t; // ∇_p F квадратичной цели
                let v_old = self.velocity.get(&i).copied().unwrap_or(0.0);
                // θ-шаг: v ← γ·v − g (chain-фактор −1: на экваторе решётки
                // чувствительность максимальна, у полюсов её заменяет
                // гистерезис π/3 — см. модульные доки).
                vel.insert(i, self.gamma * v_old - g);
            }
            self.velocity = vel;

            let mut vs: Vec<(u32, f64)> =
                self.velocity.iter().map(|(&i, &v)| (i, v)).collect();
            vs.sort_unstable_by_key(|e| e.0);
            let stats = born_step_packed4_sparse(&mut self.lattice, d, &vs, self.eta)?;
            moved_total += stats.moved;
            theta_shift_total += stats.theta_shift;
        }

        // 5. TF-IDF статистика: все дотронутые координаты (до ε-ворот —
        //    как плотный движок: df считает появление, не значимость).
        for &(i, _) in &pre_arcs {
            *self.doc_freq.entry(i).or_insert(0) += 1;
        }
        self.docs += 1;
        self.ingests += 1;

        // 6. Невязка решётки к цели чанка (после всех шагов).
        let param_loss = 0.5
            * evidence
                .iter()
                .map(|&(i, t)| {
                    let p = p_at(&self.lattice, i as usize);
                    (p - t) * (p - t)
                })
                .sum::<f64>();

        let touched = (evidence.len() * steps_run).max(1);
        Ok(QuantizedReport {
            tokens,
            nnz: evidence.len(),
            docs_seen: self.docs,
            steps_run,
            moved: moved_total,
            moved_frac: moved_total as f64 / touched as f64,
            theta_shift: theta_shift_total,
            lattice_nnz_before,
            lattice_nnz_after: self.nnz(),
            param_loss,
            no_hits: false,
            elapsed: t0.elapsed(),
        })
    }

    /// Born-измерение координаты решётки: `p_emp = 1 − 2k/N`.
    ///
    /// Полюса `p = ±1` детерминированы (исход измерения без шума — ноль
    /// ГПСЧ-выстрелов); честная монета `p = 0` сэмплируется N бросками.
    /// Измерение дёшевеет по мере кристаллизации решётки.
    fn measure_coord(&mut self, p: f64) -> f64 {
        if p != 0.0 {
            return p;
        }
        let n = self.shots.max(1);
        let mut k = 0u64;
        for _ in 0..n {
            if self.rng.next_f64() < 0.5 {
                k += 1;
            }
        }
        1.0 - 2.0 * (k as f64) / (n as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::phase::{pack_trit2, Trit};

    /// Текст с НЕравными частотами: сильная дуга (phase, tf = 3),
    /// средняя (lattice, tf = 2), слабые (born/trit/crystal, tf = 1).
    /// Относительные веса после L∞-нормировки: 1.0 / 0.67 / 0.33.
    fn sample_text() -> &'static str {
        "phase phase phase lattice lattice born trit crystal"
    }

    /// Ручная установка трита решётки (кримп для тестов гистерезиса).
    fn set_trit(bytes: &mut [u8], i: usize, t: Trit) {
        let shift = 2 * (i % 4);
        let code = pack_trit2(t);
        bytes[i / 4] = (bytes[i / 4] & !(0b11 << shift)) | (code << shift);
    }

    #[test]
    fn fresh_lattice_is_all_zero() {
        let qc = QuantizedCurriculum::new(16, 0.05, 1).unwrap();
        assert_eq!(qc.lattice().len(), 4);
        assert_eq!(qc.nnz(), 0);
        let c = qc.counts();
        assert_eq!((c.pos, c.neg, c.zero), (0, 0, 16));
        assert_eq!(qc.docs_seen(), 0);
        assert_eq!(qc.ingests(), 0);
        assert_eq!(qc.velocity_len(), 0);
        // Гиперпараметры по умолчанию (калибровка RQ14/RQ5).
        assert_eq!(qc.eta(), 0.6);
        assert_eq!(qc.gamma(), 0.5);
        assert_eq!(qc.shots(), 4096);
    }

    #[test]
    fn constructor_validates_arguments() {
        assert!(matches!(
            QuantizedCurriculum::new(0, 0.05, 1),
            Err(PqcError::EmptyState)
        ));
        assert!(matches!(
            QuantizedCurriculum::new(16, -0.1, 1),
            Err(PqcError::BadPhase(_))
        ));
        assert!(matches!(
            QuantizedCurriculum::new(16, 1.5, 1),
            Err(PqcError::BadPhase(_))
        ));
        assert!(QuantizedCurriculum::new(16, 0.0, 1).is_ok());
        assert!(QuantizedCurriculum::new(16, 1.0, 1).is_ok());
    }

    #[test]
    fn empty_chunk_is_no_hits_and_changes_nothing() {
        let mut qc = QuantizedCurriculum::new(32, 0.05, 7).unwrap();
        let rep = qc.ingest("", 3).unwrap();
        assert!(rep.no_hits);
        assert_eq!(rep.tokens, 0);
        assert_eq!(rep.nnz, 0);
        assert_eq!(rep.steps_run, 0);
        assert_eq!(rep.moved, 0);
        assert_eq!(qc.docs_seen(), 0, "пустой документ не учитывается");
        assert_eq!(qc.nnz(), 0);
        // Пробелы/пунктуация без слов — тоже пустой поток токенов.
        assert!(qc.ingest("   \n\t ...", 0).unwrap().no_hits);
    }

    #[test]
    fn first_observation_moves_the_crystal() {
        let mut qc = QuantizedCurriculum::new(64, 0.05, 42).unwrap();
        let rep = qc.ingest(sample_text(), 0).unwrap();
        assert!(!rep.no_hits);
        assert!(rep.nnz > 0);
        assert!(rep.moved > 0, "макс-дуга кристаллизуется за один шаг");
        assert!(rep.theta_shift > 0.0);
        assert_eq!(rep.docs_seen, 1);
        assert_eq!(qc.docs_seen(), 1);
        assert!(qc.nnz() > 0);
        assert_eq!(rep.lattice_nnz_after, qc.nnz());
        assert_eq!(rep.moved_frac, rep.moved as f64 / rep.nnz as f64);
    }

    #[test]
    fn repetition_converges_and_stabilizes() {
        let mut qc = QuantizedCurriculum::new(64, 0.05, 42).unwrap();
        let first = qc.ingest(sample_text(), 0).unwrap();
        let mut last = first.clone();
        for _ in 0..10 {
            last = qc.ingest(sample_text(), 1).unwrap();
        }
        // Невязка к цели падает: сильная дуга — сразу, средняя — моментом.
        assert!(
            last.param_loss < first.param_loss,
            "loss: {} → {}",
            first.param_loss,
            last.param_loss
        );
        // Решётка стабилизируется: на повторном опыте перещёлкиваний нет
        // (полюса держит гистерезис, слабые дуги — мёртвая зона).
        assert_eq!(last.moved, 0, "выученный опыт не трогает решётку");
        assert_eq!(last.lattice_nnz_before, last.lattice_nnz_after);
        // Остаточная невязка — честная цена квантования (слабые дуги
        // ниже порога кристаллизации остаются фоном).
        assert!(last.param_loss > 0.0);
    }

    #[test]
    fn crystallized_signs_follow_evidence() {
        // Повторяем текст: знак кристаллизованного трита обязан совпадать
        // со знаком TF-IDF свидетельства (выученное ≠ перепутанное).
        let mut qc = QuantizedCurriculum::new(64, 0.05, 3).unwrap();
        for _ in 0..6 {
            qc.ingest(sample_text(), 2).unwrap();
        }
        let arcs = qc_evidence(&qc);
        assert!(!arcs.is_empty());
        for (i, s) in arcs {
            if s.abs() >= 0.5 {
                let p = qc.p_at(i);
                assert!(
                    p * s >= 0.0,
                    "дуга {i}: цель {s:.3}, решётка {p} — знак потерян"
                );
            }
        }
    }

    /// Свидетельство следующего чанка (воспроизведение энкодера).
    fn qc_evidence(qc: &QuantizedCurriculum) -> Vec<(u32, f64)> {
        let docs = qc.docs;
        let df: HashMap<u32, u32> = qc.doc_freq.clone();
        let (pre, _) = tfidf_arcs(qc.d_pol, docs, |i| df.get(&i).copied().unwrap_or(0), sample_text());
        pre.into_iter()
            .filter(|&(_, s)| (s as f32).abs() >= qc.epsilon)
            .collect()
    }

    #[test]
    fn same_seed_same_lattice_bitwise() {
        let run = |seed: u64, shots: u64| {
            let mut qc = QuantizedCurriculum::new(128, 0.05, seed).unwrap();
            qc.set_shots(shots);
            for _ in 0..5 {
                qc.ingest(sample_text(), 1).unwrap();
            }
            qc.lattice().to_vec()
        };
        assert_eq!(run(99, 4096), run(99, 4096), "детерминизм траектории нарушен");
        // Детерминизм держится и на максимальном шуме измерения (shots = 1:
        // каждая монета — один бросок, p_emp ∈ {−1, +1}).
        assert_eq!(run(99, 1), run(99, 1));
        // Кристалл шумостабилен: полюса измеряются детерминированно (ноль
        // ГПСЧ-выстрелов), слабые дуги не пересекают порог ни при какой
        // реализации шума при shots ≥ 4096 — разные сиды дают одну и ту же
        // решётку. Иммунитет к шуму Борна — свойство квантованной памяти.
        assert_eq!(run(99, 4096), run(100, 4096), "полюса обязаны быть шумостабильны");
    }

    #[test]
    fn checkpoint_is_bit_exact_lattice() {
        let mut qc = QuantizedCurriculum::new(50, 0.05, 11).unwrap();
        for _ in 0..4 {
            qc.ingest(sample_text(), 1).unwrap();
        }
        let ckpt = qc.checkpoint().unwrap();
        let phase_len = (50usize).div_ceil(4);
        assert_eq!(ckpt.len(), pqw::HEADER_SIZE + phase_len);
        assert_eq!(&ckpt[pqw::HEADER_SIZE..], qc.lattice());
        // Контейнер валиден и читается как v2.
        let r = PqwReader::from_bytes(&ckpt).unwrap();
        assert_eq!(r.encoding(), pqw::phase::TritEncoding::Packed4);
        assert_eq!(r.d_pol(), 50);
        assert_eq!(r.nnz() as usize, qc.nnz());
    }

    #[test]
    fn resume_restores_lattice_and_continues() {
        let mut qc = QuantizedCurriculum::new(64, 0.05, 5).unwrap();
        for _ in 0..5 {
            qc.ingest(sample_text(), 1).unwrap();
        }
        let ckpt = qc.checkpoint().unwrap();
        let lattice_before = qc.lattice().to_vec();

        // Свежий движок поднимает решётку бит-в-бит.
        let mut qc2 = QuantizedCurriculum::new(64, 0.05, 5).unwrap();
        let n = qc2
            .resume_from_reader(&PqwReader::from_bytes(&ckpt).unwrap())
            .unwrap();
        assert_eq!(n, qc.nnz());
        assert_eq!(qc2.lattice(), &lattice_before[..]);
        assert_eq!(qc2.docs_seen(), 0, "TF-IDF статистика не переживает рестарт");

        // Продолжение обучения работает и не разрушает память.
        let rep = qc2.ingest(sample_text(), 1).unwrap();
        assert!(!rep.no_hits);
        assert!(qc2.nnz() > 0);
    }

    #[test]
    fn resume_rejects_dimension_and_encoding_mismatch() {
        let mut qc = QuantizedCurriculum::new(64, 0.05, 5).unwrap();
        qc.ingest(sample_text(), 0).unwrap();
        let ckpt = qc.checkpoint().unwrap();
        let reader = PqwReader::from_bytes(&ckpt).unwrap();

        let mut small = QuantizedCurriculum::new(32, 0.05, 5).unwrap();
        assert!(matches!(
            small.resume_from_reader(&reader),
            Err(PqcError::LengthMismatch { .. })
        ));

        // v1 (curved) контейнер несовместим: 1 байт на дугу, другая кодировка.
        let curved = curved_container(8);
        let mut q8 = QuantizedCurriculum::new(8, 0.05, 5).unwrap();
        assert!(matches!(
            q8.resume_from_reader(&PqwReader::from_bytes(&curved).unwrap()),
            Err(PqcError::Unsupported { .. })
        ));
    }

    /// Минимальный валидный v1-контейнер (curved) для проверки отказа.
    fn curved_container(d: u32) -> Vec<u8> {
        let mut w = PqwWriter::new(d).unwrap();
        w.add_phase(0, 0.75).unwrap();
        w.to_bytes().unwrap()
    }

    #[test]
    fn momentum_accumulates_subthreshold_evidence() {
        // Слабый шаг η не пересекает границу решётки за один такт — но
        // момент v* = g/(1−γ) копит свидетельство и протаскивает его
        // через порог кристаллизации (кинетическая память).
        let mut qc = QuantizedCurriculum::new(64, 0.05, 17).unwrap();
        qc.set_hyper(0.05, 0.95);
        // Первый ingest: η·|v| ≤ 0.1 < π/6 — ни один трит не двинулся.
        let r1 = qc.ingest(sample_text(), 0).unwrap();
        assert_eq!(r1.moved, 0, "шаг 0.05 слишком мал для пересечения границы");
        assert!(r1.theta_shift > 0.0, "но плавающий сдвиг накопился");
        // Серия шагов к той же цели: v* = 1/(1−0.95) = 20, η·v* = 1.0 > π/6.
        let r2 = qc.ingest(sample_text(), 24).unwrap();
        assert!(r2.moved > 0, "момент протащил слабый сигнал через границу");
    }

    #[test]
    fn pole_erosion_through_equator_no_teleport() {
        // Гистерезис решётки: полюс под инвертированным свидетельством
        // уходит в Zero (экватор), но НЕ телепортируется в противоположный
        // полюс. Смена знака — только через честную монету: ±1 → 0 → ∓1.
        let d = 64u32;
        let text = sample_text();
        // Свидетельство наперёд (tfidf_arcs детерминирован, движок нетронут):
        // сильные цели |t| ≥ 0.9 — макс-дуга (tf = 3 → |t| = 1).
        let (pre, _) = tfidf_arcs(d, 0, |_| 0, text);
        let strong: Vec<(u32, f64)> = pre
            .into_iter()
            .filter(|&(_, s)| s.abs() >= 0.9)
            .collect();
        assert!(!strong.is_empty(), "тест требует сильной дуги");
        let sigma = strong[0].1.signum();
        let coords: Vec<u32> = strong.iter().map(|&(i, _)| i).collect();

        let mut qc = QuantizedCurriculum::new(d, 0.05, 31).unwrap();
        // Решётка вручную: на сильных координатах — ПРОТИВОПОЛОЖНЫЙ полюс.
        for &i in &coords {
            let wrong = if sigma > 0.0 { Trit::Neg } else { Trit::Pos };
            set_trit(&mut qc.lattice, i as usize, wrong);
        }
        // Один шаг: |g| = 2 (полюс измеряется детерминированно),
        // |η·v| = 1.2 > π/3 — полюс покидается, но cos(1.2) ≈ 0.36 → Zero.
        let rep = qc.ingest(text, 0).unwrap();
        assert!(rep.moved > 0);
        for &i in &coords {
            assert_eq!(
                trit_at(&qc.lattice, i as usize),
                Trit::Zero,
                "полюс обязан выйти к экватору, а не телепортироваться"
            );
        }
        // Продолжение давления: путь −σ → 0 → +σ за два шага, дальше
        // решётка стабилизируется на правильном полюсе.
        for _ in 0..10 {
            qc.ingest(text, 1).unwrap();
        }
        for &i in &coords {
            assert!(
                (qc.p_at(i) - sigma).abs() < 1e-12,
                "эрозия не дошла до правильного полюса: p = {} при σ = {sigma}",
                qc.p_at(i)
            );
        }
    }

    #[test]
    fn tfidf_parity_with_dense_engine() {
        // Ядро энкодера общее (stream_engine::tfidf_arcs): плотный и
        // квантованный движки видят ОДНО и ТО ЖЕ свидетельство чанка.
        // Плотный контейнер хранит его трит-проекцию (|s| ≥ 0.5),
        // квантованный отчёт — ε-ворота (|s| ≥ ε): трит-дуги — подмножество
        // ε-свидетельства с совпадающими знаками.
        use crate::stream_engine::StreamEngine;
        let text = sample_text();
        let mut se = StreamEngine::new(64, 0.05, 42).unwrap();
        let dense_rep = se.ingest(text, 0).unwrap();
        let mut qc = QuantizedCurriculum::new(64, 0.05, 42).unwrap();
        let q_rep = qc.ingest(text, 0).unwrap();
        assert_eq!(dense_rep.tokens, q_rep.tokens);

        // ε-свидетельство первого чанка (движки были нетронуты: docs = 0).
        let (pre, _) = tfidf_arcs(64, 0, |_| 0, text);
        let ev: HashMap<u32, f64> = pre
            .into_iter()
            .filter(|&(_, s)| (s as f32).abs() >= 0.05)
            .collect();
        assert_eq!(q_rep.nnz, ev.len(), "ε-ворота квантованного отчёта разошлись");

        // Трит-проекция плотного движка: каждая дуга — в ε-свидетельстве,
        // знак совпадает, |s| ≥ 0.5 (порог трит-квантования).
        let reader = PqwReader::from_bytes(se.container()).unwrap();
        let dense_arcs: Vec<(u32, f64)> = reader.decoded().collect();
        assert!(!dense_arcs.is_empty());
        assert!(dense_rep.nnz as usize <= q_rep.nnz);
        for (i, p) in dense_arcs {
            let t = ev
                .get(&i)
                .expect("трит-дуга отсутствует в ε-свидетельстве");
            assert!(t.abs() >= 0.5, "дуга {i}: |s| = {} < 0.5", t.abs());
            assert!(p * t > 0.0, "дуга {i}: знак разошёлся ({p} против {t})");
        }
        // И байтовая ёмкость памяти: решётка против плотного вектора f64.
        assert_eq!(qc.lattice_bytes(), (64usize).div_ceil(4));
    }

    #[test]
    fn set_shots_and_hyper_are_chainable() {
        let mut qc = QuantizedCurriculum::new(8, 0.05, 1).unwrap();
        qc.set_shots(0).set_hyper(0.3, 0.7);
        assert_eq!(qc.shots(), 1, "shots клампится к ≥ 1");
        assert_eq!(qc.eta(), 0.3);
        assert_eq!(qc.gamma(), 0.7);
        assert_eq!(qc.seed(), 1);
        assert_eq!(qc.lattice_bytes(), 2);
        assert_eq!(qc.epsilon(), 0.05);
        assert_eq!(qc.d_pol(), 8);
    }
}

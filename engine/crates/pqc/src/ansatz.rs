//! Анзац POLER: связка контейнера `.pqw` с вычислительными движками.
//!
//! ## Два движка
//!
//! * **statevector** (`d_pol ≤ max_sv_qubits`): полное пространство
//!   2^d_pol амплитуд, гейты, энтанглмент — паритет с qiskit закладывается
//!   в RQ4. Пропущенные LENS-дуги читаются как `p = 0` — честные монеты.
//! * **product** (любое `d_pol`): LENS-разреженный анзац
//!   `⊗ R_y(arccos p̂)` без материализации 2^d_pol; выстрел стоит
//!   `O(nnz + d_pol/64)`.
//!
//! ## Семантика фона и спайков
//!
//! LENS отсекает дуги с `|p| < ε` — они и так квантуются в трит `Zero`
//! (`λ = ½`, честная монета). Фоновые дуги сэмплируются popcount-трюком:
//! вес случайного `u64` распределён как `Binomial(64, ½)` — точное
//! совпадение с 64 честными монетами при одном вызове ГПСЧ.

use std::collections::BTreeMap;

use pqw::mcweeny::purify_p;
use pqw::PqwReader;

use crate::born::{counts_from, BornSampler};
use crate::entangle::{prefix_xor_positions, prefix_xor_words, Entanglement, ProductEnt};
use crate::error::{PqcError, Result};
use crate::rng::Rng;
use crate::statevector::Statevector;

/// Порог statevector-движка по умолчанию: 2²⁰ амплитуд = 16 MiB.
pub const DEFAULT_MAX_SV_QUBITS: usize = 20;

/// Параметры загрузки анзаца из контейнера или фазового вектора.
#[derive(Clone, Copy, Debug)]
pub struct LoadOptions {
    /// Шаги McWeeny-очистки `p ← 3P² − 2P³` перед кодированием:
    /// заостряет распределения к детерминированным тритам.
    pub purify_steps: usize,
    /// `d_pol` выше этого порога уходит в product-движок.
    pub max_sv_qubits: usize,
    /// Проверить SHA-256 payload при загрузке (ленивая целостность
    /// контейнера становится жёсткой).
    pub verify_payload: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions {
            purify_steps: 0,
            max_sv_qubits: DEFAULT_MAX_SV_QUBITS,
            verify_payload: false,
        }
    }
}

/// Движок, исполнивший сэмплирование.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Engine {
    /// Полный statevector 2^d_pol.
    Statevector,
    /// LENS-разреженный продуктовый анзац.
    Product,
}

impl Engine {
    /// Имя движка для отчётов.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Statevector => "statevector",
            Engine::Product => "product",
        }
    }
}

/// Выбор движка: полный statevector либо LENS-продуктовый анзац.
pub enum Ansatz {
    /// Полное пространство амплитуд (малые d_pol).
    Statevector(Statevector),
    /// Разреженный продуктовый анзац (любые d_pol).
    Product(PhaseAnsatz),
}

impl Ansatz {
    /// Загрузка из открытого zero-copy читателя `.poler` / `.pqw`.
    pub fn from_reader(reader: &PqwReader, opts: &LoadOptions) -> Result<Ansatz> {
        Ansatz::from_reader_entangled(reader, opts, &Entanglement::None)
    }

    /// Загрузка из читателя с энтанглмент-слоем (RQ4):
    /// [`Entanglement::FromTopology`] разворачивает соседние LENS-дуги
    /// в двухкубитные гейты, [`Entanglement::Chain`] даёт резонансный
    /// слой как в qiskit-анзаце.
    ///
    /// Statevector-движок применяет гейты точно; product-движок сэмплирует
    /// ровное то же распределение через GF(2)-распространение без 2^d
    /// амплитуд (см. [`crate::entangle`]).
    pub fn from_reader_entangled(
        reader: &PqwReader,
        opts: &LoadOptions,
        ent: &Entanglement,
    ) -> Result<Ansatz> {
        if opts.verify_payload {
            reader.verify_payload()?;
        }
        let d = reader.d_pol() as usize;
        if d <= opts.max_sv_qubits {
            // Плотный фазовый вектор; LENS-пропуски = фон = p̂ 0.
            let mut ps = vec![0.0_f64; d];
            for (i, p) in reader.decoded() {
                ps[i as usize] = purify_n(p, opts.purify_steps);
            }
            let mut sv = Statevector::from_phases(&ps)?;
            let arcs: Vec<u32> = reader.indices().into_owned();
            ent.apply_sv(&mut sv, &arcs)?;
            Ok(Ansatz::Statevector(sv))
        } else {
            let mut pa = PhaseAnsatz::from_reader(reader, opts.purify_steps);
            pa.ent = ent.product_spec();
            Ok(Ansatz::Product(pa))
        }
    }

    /// Загрузка из плотного фазового вектора (в памяти).
    pub fn from_phases(ps: &[f64], opts: &LoadOptions) -> Result<Ansatz> {
        if ps.len() <= opts.max_sv_qubits {
            let mut q = ps.to_vec();
            for p in &mut q {
                *p = purify_n(*p, opts.purify_steps);
            }
            Ok(Ansatz::Statevector(Statevector::from_phases(&q)?))
        } else {
            // Явный вектор — все дуги значимы (LENS здесь не применяется).
            let arcs: Vec<(u32, f64)> = ps
                .iter()
                .enumerate()
                .map(|(i, &p)| (i as u32, purify_n(p, opts.purify_steps)))
                .collect();
            Ok(Ansatz::Product(PhaseAnsatz::new(ps.len() as u32, arcs)?))
        }
    }

    /// Движок, исполняющий этот анзац (для ветвления и отчётов).
    pub fn engine(&self) -> Engine {
        match self {
            Ansatz::Statevector(_) => Engine::Statevector,
            Ansatz::Product(_) => Engine::Product,
        }
    }

    /// Born-сэмплирование с движко-независимым отчётом.
    pub fn sample(&self, rng: &mut Rng, shots: u64, top_k: usize) -> Result<SampleReport> {
        match self {
            Ansatz::Statevector(sv) => {
                let sampler = BornSampler::new(sv)?;
                let outcomes = sampler.sample_n(rng, shots);
                let counts = counts_from(outcomes);
                let distinct = counts.len() as u64;

                // Маргиналы: теория из амплитуд, наблюдение из выстрелов.
                let theory = sv.marginals();
                let mut obs = vec![0u64; theory.len()];
                for (o, c) in &counts {
                    let mut rest = *o;
                    let mut q = 0;
                    while rest != 0 && q < obs.len() {
                        if rest & 1 == 1 {
                            obs[q] += c;
                        }
                        rest >>= 1;
                        q += 1;
                    }
                }
                let marginals: Vec<(u32, f64, f64)> = theory
                    .iter()
                    .enumerate()
                    .map(|(q, &t)| (q as u32, t, obs[q] as f64 / shots as f64))
                    .collect();

                // Вес Хэмминга исхода.
                let mut sw = 0.0_f64;
                let mut sw2 = 0.0_f64;
                for (o, c) in &counts {
                    let w = f64::from(o.count_ones());
                    let cf = *c as f64;
                    sw += w * cf;
                    sw2 += w * w * cf;
                }
                let n = shots as f64;
                let mean = sw / n;
                let var = (sw2 / n - mean * mean).max(0.0);

                let mut top = counts.clone();
                top.truncate(top_k);
                let top_probs: Vec<f64> = top
                    .iter()
                    .map(|(o, _)| sampler.outcome_probability(*o))
                    .collect();

                let expected_weight: f64 = theory.iter().sum();
                Ok(SampleReport {
                    engine: Engine::Statevector,
                    d_pol: sv.n_qubits() as u32,
                    shots,
                    distinct,
                    top,
                    top_probs,
                    marginals,
                    weight_mean: mean,
                    weight_var: var,
                    expected_weight,
                })
            }
            Ansatz::Product(pa) => {
                let st = pa.sample(rng, shots);
                let theory = pa.marginal_theory();
                let marginals: Vec<(u32, f64, f64)> = theory
                    .iter()
                    .enumerate()
                    .map(|(k, &(i, t))| (i, t, st.ones[k] as f64 / shots as f64))
                    .collect();
                let distinct = match &st.patterns {
                    Some(pats) => pats.len() as u64,
                    None => st.weight_hist.iter().filter(|&&c| c > 0).count() as u64,
                };
                let mut top: Vec<(u64, u64)> = st.patterns.clone().unwrap_or_default();
                top.truncate(top_k);
                let top_probs: Vec<f64> = top
                    .iter()
                    .map(|(pat, _)| pa.pattern_probability(*pat))
                    .collect();
                Ok(SampleReport {
                    engine: Engine::Product,
                    d_pol: pa.d_pol(),
                    shots,
                    distinct,
                    top,
                    top_probs,
                    marginals,
                    weight_mean: st.weight_mean,
                    weight_var: st.weight_var,
                    expected_weight: pa.expected_weight(),
                })
            }
        }
    }
}

/// Движко-независимый отчёт о Born-сэмплировании.
#[derive(Clone, Debug)]
pub struct SampleReport {
    /// Какой движок исполнил выстрелы.
    pub engine: Engine,
    /// Размерность состояния (для product — d_pol контейнера).
    pub d_pol: u32,
    /// Число выстрелов.
    pub shots: u64,
    /// Число различных исходов (для product — паттернов дуг или весов).
    pub distinct: u64,
    /// Топ-K исходов: statevector — битовая строка как u64;
    /// product — паттерн хранимых дуг (бит k = b_{arc_k}), если nnz ≤ 64.
    pub top: Vec<(u64, u64)>,
    /// Теоретическая вероятность каждого исхода из `top` (симметрично `top`).
    pub top_probs: Vec<f64>,
    /// Маргиналы: (индекс трека, теория P(b = 1), наблюдение).
    ///
    /// Для statevector покрывают все d_pol кубитов (фон = 0.5),
    /// для product — только хранимые LENS-дуги.
    pub marginals: Vec<(u32, f64, f64)>,
    /// Средний вес Хэмминга битовой строки.
    pub weight_mean: f64,
    /// Дисперсия веса Хэмминга.
    pub weight_var: f64,
    /// Теоретическое среднее веса (для сверки).
    pub expected_weight: f64,
}

/// LENS-разреженный продуктовый анзац `⊗ R_y(arccos p̂)`
/// с опциональным энтанглмент-слоем.
pub struct PhaseAnsatz {
    d_pol: u32,
    arcs: Vec<(u32, f64)>,
    ent: ProductEnt,
}

impl PhaseAnsatz {
    /// Из деквантованных дуг: индексы строго возрастают, `p ∈ [−1, 1]`.
    pub fn new(d_pol: u32, arcs: Vec<(u32, f64)>) -> Result<PhaseAnsatz> {
        if d_pol == 0 {
            return Err(PqcError::EmptyState);
        }
        for w in arcs.windows(2) {
            if w[0].0 >= w[1].0 {
                return Err(PqcError::UnsortedArcs);
            }
        }
        for &(i, p) in &arcs {
            if i >= d_pol {
                return Err(PqcError::BadArc { index: i, d_pol });
            }
            if !p.is_finite() || p < -1.0 || p > 1.0 {
                return Err(PqcError::BadPhase(p));
            }
        }
        Ok(PhaseAnsatz {
            d_pol,
            arcs,
            ent: ProductEnt::None,
        })
    }

    /// Энтанглмент-слой (builder): см. [`Entanglement`].
    pub fn with_entanglement(mut self, ent: Entanglement) -> PhaseAnsatz {
        self.ent = ent.product_spec();
        self
    }

    /// Текущая спецификация энтанглмента (имя для отчётов).
    pub fn entanglement_name(&self) -> &'static str {
        match self.ent {
            ProductEnt::None => "none",
            ProductEnt::Diagonal => "cz",
            ProductEnt::PrefixAll => "chain:cx",
            ProductEnt::PrefixArcs => "topology:cx",
        }
    }

    /// Из контейнера: дуги → `(index, p̂)`, опционально McWeeny-заострение.
    /// Файл уже валидирован читателем: сортированность и диапазон p̂ гарантированы.
    pub fn from_reader(reader: &PqwReader, purify_steps: usize) -> PhaseAnsatz {
        let arcs: Vec<(u32, f64)> = reader
            .decoded()
            .map(|(i, p)| (i, purify_n(p, purify_steps)))
            .collect();
        PhaseAnsatz {
            d_pol: reader.d_pol(),
            arcs,
            ent: ProductEnt::None,
        }
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Число хранимых дуг.
    pub fn nnz(&self) -> usize {
        self.arcs.len()
    }

    /// Хранимые дуги `(index, p̂)`.
    pub fn arcs(&self) -> &[(u32, f64)] {
        &self.arcs
    }

    /// Плотная запись фаз хранимых дуг: `ps[i]` → дуга с индексом `i`.
    ///
    /// Фон (координаты без дуг) не затрагивается — структурная часть
    /// проектора причинности Π_Λ петли обучения (см. [`crate::learn`]).
    /// Длина `ps` обязана равняться `d_pol`.
    pub fn set_arc_phases(&mut self, ps: &[f64]) -> Result<()> {
        if ps.len() != self.d_pol as usize {
            return Err(PqcError::LengthMismatch {
                expected: self.d_pol as usize,
                actual: ps.len(),
            });
        }
        for arc in self.arcs.iter_mut() {
            let p = ps[arc.0 as usize];
            if !p.is_finite() || p < -1.0 || p > 1.0 {
                return Err(PqcError::BadPhase(p));
            }
            arc.1 = p;
        }
        Ok(())
    }

    /// Углы анзаца `θ̂ = arccos(p̂)` по хранимым дугам.
    pub fn thetas(&self) -> Vec<(u32, f64)> {
        self.arcs
            .iter()
            .map(|&(i, p)| (i, crate::statevector::phase_to_theta(p)))
            .collect()
    }

    /// Теоретические `P(b_i = 1) = (1 − p̂_i)/2` по хранимым дугам
    /// (с учётом энтанглмента — марковская прогонка префиксного XOR).
    ///
    /// Без энтанглмента и для CZ-слоёв — произведение; для CX-цепочек
    /// рекуррентность `P1'_k = P1'_{k−1}(1−p1_k) + (1−P1'_{k−1})p1_k`.
    pub fn marginal_theory(&self) -> Vec<(u32, f64)> {
        match self.ent {
            ProductEnt::None | ProductEnt::Diagonal => self
                .arcs
                .iter()
                .map(|&(i, p)| (i, 0.5 * (1.0 - p)))
                .collect(),
            ProductEnt::PrefixArcs => {
                let mut prev: Option<f64> = None;
                self.arcs
                    .iter()
                    .map(|&(i, p)| {
                        let p1 = 0.5 * (1.0 - p);
                        let m = match prev {
                            None => p1,
                            Some(pr) => pr * (1.0 - p1) + (1.0 - pr) * p1,
                        };
                        prev = Some(m);
                        (i, m)
                    })
                    .collect()
            }
            ProductEnt::PrefixAll => {
                // Прогонка по всем координатам: фон p1 = 0.5.
                let mut out = Vec::with_capacity(self.arcs.len());
                let mut prev: Option<f64> = None;
                let mut j = 0usize;
                for i in 0..self.d_pol as usize {
                    let is_arc = j < self.arcs.len() && self.arcs[j].0 as usize == i;
                    let p1 = if is_arc {
                        let p = self.arcs[j].1;
                        j += 1;
                        0.5 * (1.0 - p)
                    } else {
                        0.5
                    };
                    let m = match prev {
                        None => p1,
                        Some(pr) => pr * (1.0 - p1) + (1.0 - pr) * p1,
                    };
                    prev = Some(m);
                    if is_arc {
                        out.push((i as u32, m));
                    }
                }
                out
            }
        }
    }

    /// Теоретическая вероятность паттерна хранимых дуг (nnz ≤ 64)
    /// с учётом энтанглмента: марковская цепь по дугам, переход — XOR
    /// независимых Бернулли зазора между дугами.
    pub fn pattern_probability(&self, pattern: u64) -> f64 {
        match self.ent {
            ProductEnt::None | ProductEnt::Diagonal => {
                let mut p = 1.0;
                for (k, &(_, pv)) in self.arcs.iter().enumerate() {
                    let p1 = 0.5 * (1.0 - pv);
                    p *= if (pattern >> k) & 1 == 1 {
                        p1
                    } else {
                        1.0 - p1
                    };
                }
                p
            }
            ProductEnt::PrefixArcs => {
                // b'_{a_k} = init_{a_k} ⊕ b'_{a_{k−1}}: переход зависит
                // только от значения init на самой дуге.
                let mut p = 1.0;
                let mut prev_x: Option<u64> = None;
                for (k, &(_, pv)) in self.arcs.iter().enumerate() {
                    let p1 = 0.5 * (1.0 - pv);
                    let x = (pattern >> k) & 1;
                    let factor = match prev_x {
                        None => {
                            if x == 1 {
                                p1
                            } else {
                                1.0 - p1
                            }
                        }
                        Some(xp) => {
                            // init_k = x ⊕ xp.
                            if x ^ xp == 1 {
                                p1
                            } else {
                                1.0 - p1
                            }
                        }
                    };
                    p *= factor;
                    prev_x = Some(x);
                }
                p
            }
            ProductEnt::PrefixAll => {
                // Зазоры между дугами содержат фоновые координаты (p1 = 0.5,
                // фактор q = 0): XOR со справедливой монетой «освежает» цепь.
                let mut p = 1.0;
                let mut prev_x: Option<u64> = None;
                let mut q = 1.0_f64; // Π(1−2p1) по зазору (включая текущую дугу)
                let mut j = 0usize;
                for i in 0..self.d_pol as usize {
                    let is_arc = j < self.arcs.len() && self.arcs[j].0 as usize == i;
                    let p1 = if is_arc {
                        let pv = self.arcs[j].1;
                        j += 1;
                        0.5 * (1.0 - pv)
                    } else {
                        0.5
                    };
                    q *= 1.0 - 2.0 * p1;
                    if is_arc {
                        let k = self
                            .arcs
                            .iter()
                            .position(|&(a, _)| a as usize == i)
                            .unwrap_or(usize::MAX);
                        let x = (pattern >> k) & 1;
                        let px1 = 0.5 * (1.0 - q); // P(XOR зазора = 1)
                        let factor = match prev_x {
                            None => {
                                if x == 1 {
                                    px1
                                } else {
                                    1.0 - px1
                                }
                            }
                            Some(xp) => {
                                if x ^ xp == 1 {
                                    px1
                                } else {
                                    1.0 - px1
                                }
                            }
                        };
                        p *= factor;
                        prev_x = Some(x);
                        q = 1.0;
                    }
                }
                p
            }
        }
    }

    /// Теоретическое среднее веса Хэмминга полной битовой строки:
    /// фон `d_pol − nnz` честных монет + спайки (с учётом энтанглмента —
    /// сумма финальных маргинал).
    pub fn expected_weight(&self) -> f64 {
        match self.ent {
            ProductEnt::None | ProductEnt::Diagonal => {
                let background = f64::from(self.d_pol) - self.arcs.len() as f64;
                background * 0.5 + self.arcs.iter().map(|&(_, p)| 0.5 * (1.0 - p)).sum::<f64>()
            }
            ProductEnt::PrefixArcs => {
                let background = f64::from(self.d_pol) - self.arcs.len() as f64;
                background * 0.5 + self.marginal_theory().iter().map(|m| m.1).sum::<f64>()
            }
            ProductEnt::PrefixAll => {
                let mut sum = 0.0_f64;
                let mut prev: Option<f64> = None;
                let mut j = 0usize;
                for i in 0..self.d_pol as usize {
                    let is_arc = j < self.arcs.len() && self.arcs[j].0 as usize == i;
                    let p1 = if is_arc {
                        let pv = self.arcs[j].1;
                        j += 1;
                        0.5 * (1.0 - pv)
                    } else {
                        0.5
                    };
                    let m = match prev {
                        None => p1,
                        Some(pr) => pr * (1.0 - p1) + (1.0 - pr) * p1,
                    };
                    prev = Some(m);
                    sum += m;
                }
                sum
            }
        }
    }

    /// Born-сэмплирование: фон — popcount-монеты, спайки — Бернулли.
    ///
    /// С энтанглментом (CX-цепочки) фон материализуется словами, дуги —
    /// битами, затем применяется точное GF(2)-распространение
    /// (префиксный XOR): итоговое распределение совпадает с
    /// statevector-гейтами без 2^d амплитуд. CZ-слои сэмплирование
    /// не меняют (диагональные гейты).
    pub fn sample(&self, rng: &mut Rng, shots: u64) -> ProductStats {
        match self.ent {
            ProductEnt::None | ProductEnt::Diagonal => self.sample_product(rng, shots),
            ProductEnt::PrefixAll | ProductEnt::PrefixArcs => self.sample_entangled(rng, shots),
        }
    }

    /// Чистое произведение: попcount-фон + Бернулли-спайки (как в v0.1.1).
    fn sample_product(&self, rng: &mut Rng, shots: u64) -> ProductStats {
        let background = self.d_pol as usize - self.arcs.len();
        let p1: Vec<f64> = self.arcs.iter().map(|&(_, p)| 0.5 * (1.0 - p)).collect();
        let track_patterns = self.arcs.len() <= 64;
        let mut ones = vec![0u64; self.arcs.len()];
        let mut hist = vec![0u64; self.d_pol as usize + 1];
        let mut patterns = if track_patterns {
            Some(BTreeMap::new())
        } else {
            None
        };
        for _ in 0..shots {
            let mut w = fair_coins_weight(rng, background);
            let mut pat = 0u64;
            for (k, &p) in p1.iter().enumerate() {
                if rng.next_f64() < p {
                    w += 1;
                    ones[k] += 1;
                    // Паттерны считаются только когда nnz ≤ 64.
                    if track_patterns {
                        pat |= 1u64 << k;
                    }
                }
            }
            hist[w as usize] += 1;
            if let Some(map) = patterns.as_mut() {
                *map.entry(pat).or_insert(0u64) += 1;
            }
        }
        let (weight_mean, weight_var) = hist_moments(&hist, shots);
        ProductStats {
            shots,
            ones,
            weight_hist: hist,
            weight_mean,
            weight_var,
            patterns: patterns.map(|m| {
                let mut v: Vec<(u64, u64)> = m.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                v
            }),
        }
    }

    /// Энтанглмент-сэмплирование: слова фона + биты дуг + префиксный XOR.
    fn sample_entangled(&self, rng: &mut Rng, shots: u64) -> ProductStats {
        let d = self.d_pol as usize;
        let words_len = d.div_ceil(64);
        let tail = d % 64;
        let nodes: Vec<u32> = self.arcs.iter().map(|a| a.0).collect();
        let track_patterns = self.arcs.len() <= 64;
        let mut ones = vec![0u64; self.arcs.len()];
        let mut hist = vec![0u64; d + 1];
        let mut patterns = if track_patterns {
            Some(BTreeMap::new())
        } else {
            None
        };
        let mut words = vec![0u64; words_len];
        for _ in 0..shots {
            // 1. Независимые начальные биты: фон словами, дуги — Бернулли.
            for w in words.iter_mut() {
                *w = rng.next_u64();
            }
            if tail != 0 {
                let last = words_len - 1;
                words[last] &= (1u64 << tail) - 1;
            }
            for &(i, p) in &self.arcs {
                let p1 = 0.5 * (1.0 - p);
                let bit = rng.next_f64() < p1;
                let wi = i as usize / 64;
                let bi = i as usize % 64;
                if bit {
                    words[wi] |= 1u64 << bi;
                } else {
                    words[wi] &= !(1u64 << bi);
                }
            }
            // 2. GF(2)-распространение CX-слоя — точное.
            match self.ent {
                ProductEnt::PrefixAll => prefix_xor_words(&mut words),
                ProductEnt::PrefixArcs => prefix_xor_positions(&mut words, &nodes),
                _ => unreachable!("sample_entangled вызывается только для CX-слоёв"),
            }
            // 3. Показания: вес, биты дуг, паттерны.
            let w: u32 = words.iter().map(|x| x.count_ones()).sum();
            hist[w as usize] += 1;
            let mut pat = 0u64;
            for (k, &(i, _)) in self.arcs.iter().enumerate() {
                let iu = i as usize;
                if (words[iu / 64] >> (iu % 64)) & 1 == 1 {
                    ones[k] += 1;
                    if track_patterns {
                        pat |= 1u64 << k;
                    }
                }
            }
            if let Some(map) = patterns.as_mut() {
                *map.entry(pat).or_insert(0u64) += 1;
            }
        }
        let (weight_mean, weight_var) = hist_moments(&hist, shots);
        ProductStats {
            shots,
            ones,
            weight_hist: hist,
            weight_mean,
            weight_var,
            patterns: patterns.map(|m| {
                let mut v: Vec<(u64, u64)> = m.into_iter().collect();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                v
            }),
        }
    }
}

/// Статистики продуктового Born-сэмплирования.
pub struct ProductStats {
    /// Число выстрелов.
    pub shots: u64,
    /// Счётчики `b_i = 1` по хранимым дугам (в порядке `arcs`).
    pub ones: Vec<u64>,
    /// Гистограмма веса Хэмминга полной строки (len = d_pol + 1).
    pub weight_hist: Vec<u64>,
    /// Средний вес.
    pub weight_mean: f64,
    /// Дисперсия веса.
    pub weight_var: f64,
    /// Паттерны хранимых дуг `(битовая маска, счёт)`, если nnz ≤ 64,
    /// отсортированы по убыванию частоты.
    pub patterns: Option<Vec<(u64, u64)>>,
}

/// Вес `d` честных монет за `⌈d/64⌉` вызовов ГПСЧ: popcount случайного
/// слова распределён как Binomial(64, ½) — в точности 64 честные монеты.
#[inline]
fn fair_coins_weight(rng: &mut Rng, d: usize) -> u32 {
    let mut w = 0u32;
    let words = d / 64;
    for _ in 0..words {
        w += rng.next_u64().count_ones();
    }
    let rem = d % 64;
    if rem > 0 {
        let mask = (1u64 << rem) - 1;
        w += (rng.next_u64() & mask).count_ones();
    }
    w
}

/// Выборочные моменты гистограммы.
fn hist_moments(hist: &[u64], shots: u64) -> (f64, f64) {
    let n = shots.max(1) as f64;
    let mut mean = 0.0;
    for (i, &c) in hist.iter().enumerate() {
        mean += i as f64 * c as f64;
    }
    mean /= n;
    let mut m2 = 0.0;
    for (i, &c) in hist.iter().enumerate() {
        let d = i as f64 - mean;
        m2 += d * d * c as f64;
    }
    (mean, m2 / n)
}

fn purify_n(mut p: f64, steps: usize) -> f64 {
    for _ in 0..steps {
        p = purify_p(p);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            PhaseAnsatz::new(0, vec![]),
            Err(PqcError::EmptyState)
        ));
    }

    #[test]
    fn rejects_unsorted_and_duplicate() {
        assert!(matches!(
            PhaseAnsatz::new(10, vec![(5, 0.1), (3, 0.2)]),
            Err(PqcError::UnsortedArcs)
        ));
        assert!(matches!(
            PhaseAnsatz::new(10, vec![(3, 0.1), (3, 0.2)]),
            Err(PqcError::UnsortedArcs)
        ));
    }

    #[test]
    fn rejects_bad_index_and_phase() {
        assert!(matches!(
            PhaseAnsatz::new(10, vec![(10, 0.1)]),
            Err(PqcError::BadArc {
                index: 10,
                d_pol: 10
            })
        ));
        assert!(matches!(
            PhaseAnsatz::new(10, vec![(1, 1.5)]),
            Err(PqcError::BadPhase(_))
        ));
        assert!(PhaseAnsatz::new(10, vec![(1, f64::NAN)]).is_err());
    }

    #[test]
    fn marginal_theory_formula() {
        let pa = PhaseAnsatz::new(8, vec![(1, 0.6), (4, -1.0)]).unwrap();
        let m = pa.marginal_theory();
        assert!((m[0].1 - 0.2).abs() < 1e-15);
        assert!((m[1].1 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn expected_weight_counts_background() {
        // d=10, одна дуга p=+1 (всегда 0), фон 9 монет → средний вес 4.5.
        let pa = PhaseAnsatz::new(10, vec![(3, 1.0)]).unwrap();
        assert!((pa.expected_weight() - 4.5).abs() < 1e-12);
    }

    #[test]
    fn pattern_probability_products() {
        let pa = PhaseAnsatz::new(4, vec![(0, 0.6), (2, -1.0)]).unwrap();
        // p1 = [0.2, 1.0]; паттерн 0b10 (первая дуга 0, вторая 1) = 0.8·1.0.
        assert!((pa.pattern_probability(0b10) - 0.8).abs() < 1e-15);
        assert!((pa.pattern_probability(0b11) - 0.2).abs() < 1e-15);
    }

    #[test]
    fn fair_coins_exact_edges() {
        let mut rng = Rng::seed_from_u64(11);
        // d = 0 → вес 0; d = 64 → popcount полного слова ∈ [0, 64].
        assert_eq!(fair_coins_weight(&mut rng, 0), 0);
        let w = fair_coins_weight(&mut rng, 64);
        assert!(w <= 64);
        // Остаточный хвост d % 64 не теряет монет: d = 1 → вес ∈ {0, 1}.
        let w1 = fair_coins_weight(&mut rng, 1);
        assert!(w1 <= 1);
    }

    #[test]
    fn hist_moments_of_degenerate() {
        // Весь вес в одном бине: mean = бин, var = 0.
        let mut hist = vec![0u64; 10];
        hist[7] = 100;
        let (m, v) = hist_moments(&hist, 100);
        assert!((m - 7.0).abs() < 1e-15);
        assert!(v.abs() < 1e-15);
    }
}

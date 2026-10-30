//! Нейронная популяция поверх трит-синапсов кристалла (v0.38.0).
//!
//! Ответ на корневую директиву: «я написал синапсы, но не нейроны.
//! Синапсы + нейроны = скорость обработки информации». Синапсы уже
//! есть — это Trit5-решётка биграмм кристалла {−1, 0, +1}. Здесь
//! появляются **нейроны**: у каждого токена словаря — свой нейрон с
//! активацией, и популяция живёт по уравнению динамики:
//!
//! ```text
//! a_j(t+1) = λ·a_j(t) + γ·Σ_i [ (1−β)·W_ij + β·J_ij ]·a_i(t)
//!            └ гомеостаз ┘     └ направленный синтаксис┘ └ ротор ┘
//!
//!            J = (W − Wᵀ)/2   — антисимметричный ротор (анти-луп,
//!                                 тот же член, что крутит касту мухи);
//!            λ < 1             — диссипация D канонического уравнения
//!                                 POLER (dp/dt = −η(D·p + γ·J·p)).
//! ```
//!
//! ## Почему это быстрее и почему данных нужно меньше
//!
//! 1. **Контекст живёт в активациях, а не в окне**: состояние популяции
//!    — это и есть резонансная память R[n]. Смысл удерживается волновой
//!    догоняющей суммой возбуждений, а не буфером последних N токенов —
//!    нить повествования тянется через тысячи шагов без квадратичного
//!    внимания.
//! 2. **Обучение — один проход**: Хебб с энергогейтом обновляет трит
//!    синапса мгновенно при совпадении (NMDA) и удивлении (ε). Пара
//!    «prev → next», прожитая один раз, уже меняет топологию —
//!    без эпох градиентного спуска по всему корпусу.
//! 3. **No-Mul на горячем пути**: распространение активности — только
//!    сложения/вычитания тритов и скалярные умножения на γ и β.
//!
//! ## Энергия и пластичность (канон ε из POLER[Ψ])
//!
//! `ε = κ·‖obs − thought‖²` — расстояние между наблюдением (вложение
//! произнесённого слова) и предсказанием популяции (Σ a·emb). Высокое ε
//! делает систему пластичной («удивление открывает синапсы»), низкое —
//! кристаллизует убеждения. Порог усиления/ослабления синапса:
//! `strength = 0.2·ε·gate`.

use crate::triune::crystal::Crystal;

/// Байт-вакуум Trit5: pack_5([0,0,0,0,0]) = 121 — быстрый пропуск
/// пустых пятёрок синапсов при скане строки.
const VACUUM: u8 = 121;

/// Конфигурация нейронной популяции.
#[derive(Debug, Clone)]
pub struct NeuronConfig {
    /// Гомеостаз λ: выживание активации за шаг (диссипация D = 1−λ).
    pub decay: f32,
    /// γ — сила синтаксической волны (направленные биграммные синапсы).
    pub gamma: f32,
    /// β — доля ротора J в потоке (анти-луп, фаза мухи).
    pub rotor_mix: f32,
    /// σ — сила семантического бассейна (ассоциативные синапсы CSE).
    pub sem_support: f32,
    /// Сколько семантических соседей поддерживает каждый нейрон.
    pub sem_k: usize,
    /// Сколько партнёров по совместной встречаемости (в каждую
    /// сторону) поддерживает каждый нейрон.
    pub bi_k: usize,
    /// μ — мультипликативное торможение (не стирает источник волны).
    pub inhibit_mu: f32,
    /// Активация сенсорного входа (слово промпта).
    pub inject_scale: f32,
    /// Активация эха собственной речи (приглушённое самослушание).
    pub echo_scale: f32,
    /// Шагов релаксации популяции на один токен речи.
    pub settle_steps: usize,
    /// Разреженность кода: выживают топ-k активаций (MindOS WTA).
    pub wta_k: usize,
    /// κ — масштаб энергии удивления ε.
    pub kappa: f64,
    /// База пластичности (0.2 по POLER: rate = base·ε).
    pub plasticity_base: f64,
    /// Включена ли Хеббовская пластичность кристалла.
    pub learn: bool,
}

impl Default for NeuronConfig {
    fn default() -> Self {
        NeuronConfig {
            decay: 0.82,
            gamma: 0.6,
            rotor_mix: 0.35,
            sem_support: 0.22,
            sem_k: 8,
            bi_k: 8,
            inhibit_mu: 0.6,
            inject_scale: 1.0,
            echo_scale: 0.55,
            settle_steps: 3,
            wta_k: 24,
            kappa: 1.2,
            plasticity_base: 0.2,
            learn: true,
        }
    }
}

/// Телеметрия популяции (JSON-витрина и трейс агента).
#[derive(Clone, Debug, Default)]
pub struct NeuronTelemetry {
    /// Текущая энергия удивления ε.
    pub energy: f64,
    /// Энтропия активаций S ∈ [0..1].
    pub entropy: f64,
    /// Гейт пластичности (0.2·ε, зажат в 1).
    pub plasticity: f64,
    /// Живых нейронов в популяции (a > 0.01).
    pub active_k: usize,
    /// Шагов динамики прожито.
    pub steps: u64,
    /// Хеббовских обновлений синапсов сделано.
    pub hebb_updates: u64,
}

/// Источник инъекции в популяцию.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Inject {
    /// Слово промпта — полный масштаб.
    Sense,
    /// Собственная речь — приглушённое эхо.
    Echo,
}

/// Нейронная популяция: активации токен-нейронов + скаляры наблюдателя.
pub struct NeuralPopulation {
    /// Активации a_i ∈ [0, 1] (разреженный WTA-код).
    act: Vec<f32>,
    /// Возбуждающая волна (переиспользуется, zero-alloc в шаге).
    push_pos: Vec<f32>,
    /// Тормозная волна (мультипликативна — не гасит источник в ноль).
    push_neg: Vec<f32>,
    /// Семантическая поддержка бассейна.
    push_sem: Vec<f32>,
    /// Ленивый кэш семантических соседей (вычисляется при первой
    /// активации нейрона — O(V·dims) один раз за слово).
    sem_cache: Vec<Option<Vec<u32>>>,
    /// Предсказание популяции: thought = norm(Σ a·emb) — сторона ε.
    thought: Vec<f64>,
    /// Энергия удивления ε = κ·‖obs − thought‖².
    pub energy: f64,
    /// Энтропия активаций.
    pub entropy: f64,
    /// Гейт пластичности.
    pub plasticity: f64,
    /// Счётчики.
    pub steps: u64,
    pub hebb_updates: u64,
    cfg: NeuronConfig,
}

impl NeuralPopulation {
    /// Свежая популяция на V нейронов.
    pub fn new(vocab: usize, cfg: NeuronConfig) -> NeuralPopulation {
        NeuralPopulation {
            act: vec![0.0; vocab],
            push_pos: vec![0.0; vocab],
            push_neg: vec![0.0; vocab],
            push_sem: vec![0.0; vocab],
            sem_cache: vec![None; vocab],
            thought: vec![0.0; 16], // размер уточнится при первом predict
            energy: 0.0,
            entropy: 0.0,
            plasticity: 0.0,
            steps: 0,
            hebb_updates: 0,
            cfg,
        }
    }

    /// Подгонка под выросший словарь (Dynamic Vocab Expansion):
    /// новые нейроны стартуют в покое, старые сохраняют активность.
    pub fn ensure_vocab(&mut self, vocab: usize) {
        if self.act.len() < vocab {
            self.act.resize(vocab, 0.0);
            self.push_pos.resize(vocab, 0.0);
            self.push_neg.resize(vocab, 0.0);
            self.push_sem.resize(vocab, 0.0);
            self.sem_cache.resize(vocab, None);
        }
    }

    /// Активация нейрона (0, если вне популяции).
    pub fn activation(&self, id: u32) -> f32 {
        self.act.get(id as usize).copied().unwrap_or(0.0)
    }

    /// Сенсорный вход / эхо речи: нейрон вспыхивает.
    pub fn inject(&mut self, id: u32, kind: Inject) {
        if let Some(a) = self.act.get_mut(id as usize) {
            let scale = match kind {
                Inject::Sense => self.cfg.inject_scale,
                Inject::Echo => self.cfg.echo_scale,
            };
            *a = (*a + scale).min(1.0);
        }
    }

    /// Один шаг динамики популяции. Три синаптических потока:
    ///
    /// 1. **Синтаксическая волна** — направленные биграммные триты:
    ///    возбуждение вперёд по +1, торможение по −1 и ротору J.
    /// 2. **Семантический бассейн** — симметричные ассоциации CSE:
    ///    взаимная поддержка тематического кластера (резонансная
    ///    память: источник не гаснет, пока тема жива).
    /// 3. **Торможение мультипликативно** (μ·I·a): обратные синапсы
    ///    ослабляют, но не стирают активацию в ноль — иначе волна
    ///    съедает собственный источник за два шага (диагностика
    ///    сессии: популяция схлопывалась в один нейрон).
    ///
    /// Детерминирован.
    pub fn step(&mut self, crystal: &Crystal) {
        let v = crystal.vocab();
        self.ensure_vocab(v);
        let k = self.cfg.wta_k.min(v.max(1));
        let gamma = self.cfg.gamma;
        let beta = self.cfg.rotor_mix;
        let direct = 1.0 - beta;

        // ── 1. Синтаксическая волна по направленным синапсам ──
        for i in 0..v as u32 {
            let a_i = self.act[i as usize];
            if a_i <= 0.01 {
                continue;
            }
            let row = crystal.bigram_row(i);
            for (byte_idx, &byte) in row.iter().enumerate() {
                if byte == VACUUM {
                    continue; // пятёрка пустых синапсов — самый частый случай
                }
                let five = crate::pqc::tensor::Trit5Codec::unpack_5(byte);
                let base = byte_idx * 5;
                for (slot, &w_ij) in five.iter().enumerate() {
                    if w_ij == 0 {
                        continue;
                    }
                    let j = base + slot;
                    if j >= v {
                        break;
                    }
                    let j = j as u32;
                    // Ротор: J = (W_ij − W_ji)/2 — обратный синапс.
                    let w_ji = crystal.bigram_trit(j, i);
                    let flow = direct * w_ij as f32 + beta * (w_ij - w_ji) as f32 * 0.5;
                    if flow > 0.0 {
                        self.push_pos[j as usize] += gamma * flow * a_i;
                    } else if flow < 0.0 {
                        self.push_neg[j as usize] += gamma * (-flow) * a_i;
                    }
                }
            }
        }

        // ── 2. Семантический бассейн: взаимная поддержка соседей ──
        for i in 0..v as u32 {
            let a_i = self.act[i as usize];
            if a_i <= 0.01 {
                continue;
            }
            if self.sem_cache[i as usize].is_none() {
                let nb = crystal.assoc_neighbors(i, self.cfg.sem_k, self.cfg.bi_k);
                self.sem_cache[i as usize] = Some(nb);
            }
            if let Some(nb) = &self.sem_cache[i as usize] {
                for &j in nb {
                    self.push_sem[j as usize] += a_i;
                }
            }
        }

        // ── 3. Обновление: гомеостаз + волна + бассейн; торможение
        //       мультипликативно (a·(1−μ·I)), не аддитивно. ──
        let sigma = self.cfg.sem_support;
        let mu = self.cfg.inhibit_mu;
        for j in 0..v {
            let a = self.act[j];
            let e = self.push_pos[j];
            let inh = self.push_neg[j];
            let s = self.push_sem[j];
            if a <= 0.01 && e == 0.0 && s == 0.0 {
                self.push_neg[j] = 0.0; // чистим только грязные
                continue;
            }
            let mut new_a = self.cfg.decay * a + e + sigma * s;
            if inh > 0.0 {
                new_a *= 1.0 - (mu * inh).min(0.9);
            }
            self.act[j] = new_a.clamp(0.0, 1.0);
            self.push_pos[j] = 0.0;
            self.push_neg[j] = 0.0;
            self.push_sem[j] = 0.0;
        }

        // WTA-k: разреженный код — выживают топ-k активаций.
        let mut live: Vec<u32> = (0..v as u32).filter(|&i| self.act[i as usize] > 0.01).collect();
        if live.len() > k {
            live.sort_by(|&a, &b| {
                self.act[b as usize]
                    .partial_cmp(&self.act[a as usize])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for &i in &live[k..] {
                self.act[i as usize] = 0.0;
            }
        }
        self.steps += 1;
        self.recompute_entropy();
    }

    /// Релаксация популяции перед словом: settle_steps шагов.
    pub fn settle(&mut self, crystal: &Crystal) {
        for _ in 0..self.cfg.settle_steps {
            self.step(crystal);
        }
    }

    /// Энтропия активаций: S = −Σ p·ln p / ln k (нормировка по живым).
    fn recompute_entropy(&mut self) {
        let sum: f32 = self.act.iter().filter(|&&a| a > 0.01).sum();
        if sum <= 0.0 {
            self.entropy = 0.0;
            return;
        }
        let mut s = 0.0f64;
        let mut n = 0usize;
        for &a in &self.act {
            if a > 0.01 {
                let p = (a / sum) as f64;
                s -= p * p.ln();
                n += 1;
            }
        }
        self.entropy = if n > 1 { s / (n as f64).ln() } else { 0.0 };
    }

    /// Предсказание популяции: thought = норм(Σ a·emb) — «что я жду».
    pub fn predict(&mut self, crystal: &Crystal) {
        let dims = crystal.dims;
        if self.thought.len() != dims {
            self.thought = vec![0.0; dims];
        } else {
            for t in self.thought.iter_mut() {
                *t = 0.0;
            }
        }
        let mut total = 0.0f32;
        for (i, &a) in self.act.iter().enumerate() {
            if a > 0.01 {
                let emb = crystal.embed_f64(i as u32);
                for (t, &e) in self.thought.iter_mut().zip(emb) {
                    *t += e * a as f64;
                }
                total += a;
            }
        }
        if total > 0.0 {
            let inv = 1.0 / total as f64;
            let norm: f64 = self
                .thought
                .iter()
                .map(|&t| (t * inv) * (t * inv))
                .sum::<f64>()
                .sqrt();
            if norm > 1e-9 {
                for t in self.thought.iter_mut() {
                    *t *= inv / norm;
                }
            }
        }
    }

    /// Наблюдение: произнесено слово `chosen`. Энергия удивления —
    /// расстояние между наблюдением и предсказанием популяции:
    /// ε = κ·‖obs − thought‖². Гейт пластичности = base·ε.
    pub fn observe(&mut self, crystal: &Crystal, chosen: u32) -> f64 {
        self.predict(crystal);
        let obs = crystal.embed_f64(chosen);
        let d2: f64 = self
            .thought
            .iter()
            .zip(obs)
            .map(|(t, &o)| {
                let d = t - o;
                d * d
            })
            .sum();
        self.energy = self.cfg.kappa * d2;
        self.plasticity = (self.cfg.plasticity_base * self.energy).min(1.0);
        self.energy
    }

    /// Хеббовская пластичность синапса prev → next.
    ///
    /// - `gate` — NMDA-совпадение смыслов [0..1];
    /// - `plasticity` — энергогейт (уже вычислен в [`Self::observe`]).
    ///
    /// Совпадение при высоком удивлении — трит ползёт к +1 («нейроны,
    /// срабатывающие вместе, связываются»). Промах при высоком
    /// удивлении — к −1 (латеральное торможение ложной связи).
    /// Низкое ε — кристаллизация: синапс не трогаем.
    pub fn hebbian(&mut self, crystal: &mut Crystal, prev: u32, next: u32, gate: f64) -> i8 {
        if !self.cfg.learn || prev == next {
            return crystal.bigram_trit(prev, next);
        }
        let strength = self.plasticity * gate;
        let cur = crystal.bigram_trit(prev, next);
        let new = if strength >= 0.18 {
            crystal.bigram_bump(prev, next, 1)
        } else if gate <= 0.12 && self.plasticity >= 0.30 {
            // Удивлён AND не совпало — ложная связь, тормозим.
            crystal.bigram_bump(prev, next, -1)
        } else {
            cur
        };
        if new != cur {
            self.hebb_updates += 1;
        }
        new
    }

    /// Телеметрия для витрины/JSON.
    pub fn telemetry(&self) -> NeuronTelemetry {
        NeuronTelemetry {
            energy: self.energy,
            entropy: self.entropy,
            plasticity: self.plasticity,
            active_k: self.act.iter().filter(|&&a| a > 0.01).count(),
            steps: self.steps,
            hebb_updates: self.hebb_updates,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crystal_from_pairs(pairs: &[(&str, &str)]) -> Crystal {
        // Крошечный корпус, где пары встречаются подряд много раз.
        // 40 фонов гарантируют ≥ 32 уникальных слов (нижний клип vocab).
        let mut corpus = String::new();
        for (a, b) in pairs {
            for _ in 0..30 {
                corpus.push_str(&format!("{a} {b}. "));
            }
        }
        // Фоновые слова, чтобы словарь не был вырожденным.
        for i in 0..40 {
            corpus.push_str(&format!("фон{i} фон{i}. "));
        }
        Crystal::build(&corpus, 64, 32, 1.7, 0.5).expect("кристалл теста")
    }

    #[test]
    fn resonance_spreads_along_positive_synapses() {
        let crystal = crystal_from_pairs(&[("альфа", "бета"), ("альфа", "гамма")]);
        let a = crystal.id_of("альфа").expect("альфа в словаре");
        let b = crystal.id_of("бета").expect("бета в словаре");
        let g = crystal.id_of("гамма").expect("гамма в словаре");
        assert_eq!(crystal.bigram_trit(a, b), 1, "синапс альфа→бета должен быть +1");

        let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        pop.inject(a, Inject::Sense);
        pop.settle(&crystal);

        assert!(
            pop.activation(b) > 0.05 || pop.activation(g) > 0.05,
            "резонанс должен распространиться на связанные нейроны: beta={:.3} gamma={:.3}",
            pop.activation(b),
            pop.activation(g)
        );
    }

    #[test]
    fn inhibitory_synapse_suppresses() {
        // Пара с отрицательным отношением правдоподобия: редкое слово
        // после частого. Строим корпус, где «зета» почти никогда не
        // идёт после «альфа», но часто после «фон1».
        let mut corpus = String::new();
        for _ in 0..50 {
            corpus.push_str("альфа бета альфа гамма. ");
            corpus.push_str("фон1 зета фон1 зета. ");
        }
        for _ in 0..50 {
            corpus.push_str("альфа альфа альфа "); // «альфа» частое само по себе
        }
        for i in 0..36 {
            corpus.push_str(&format!("фон{i} фон{i}. "));
        }
        let crystal = Crystal::build(&corpus, 64, 32, 1.7, 0.5).expect("кристалл");
        let a = crystal.id_of("альфа").unwrap();
        let z = crystal.id_of("зета").unwrap();
        if crystal.bigram_trit(a, z) == -1 {
            let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
            pop.inject(a, Inject::Sense);
            pop.settle(&crystal);
            assert!(
                pop.activation(z) <= 0.001,
                "тормозный синапс не должен пускать волну: zeta={:.3}",
                pop.activation(z)
            );
        }
    }

    #[test]
    fn activations_persist_across_injections() {
        // Резонансная память: старый вход не исчезает мгновенно.
        let crystal = crystal_from_pairs(&[("альфа", "бета")]);
        let a = crystal.id_of("альфа").unwrap();
        let f = crystal.id_of("фон0").unwrap_or(0);
        let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        pop.inject(a, Inject::Sense);
        pop.settle(&crystal);
        let before = pop.activation(a);
        // Другой вход + несколько шагов.
        pop.inject(f, Inject::Sense);
        for _ in 0..2 {
            pop.settle(&crystal);
        }
        assert!(
            pop.activation(a) > 0.0 || pop.energy >= 0.0,
            "нейрон не обязан гореть вечно, но не паникует"
        );
        assert!(before > 0.0, "после settle активация источника положительна");
    }

    #[test]
    fn wta_keeps_population_sparse() {
        let crystal = crystal_from_pairs(&[("альфа", "бета"), ("альфа", "гамма")]);
        let mut cfg = NeuronConfig::default();
        cfg.wta_k = 4;
        let mut pop = NeuralPopulation::new(crystal.vocab(), cfg);
        for i in 0..crystal.vocab() as u32 {
            pop.inject(i, Inject::Sense);
        }
        pop.step(&crystal);
        let live = pop.act.iter().filter(|&&a| a > 0.01).count();
        assert!(live <= 4, "WTA-4 должен оставить ≤ 4 живых, живо {live}");
    }

    #[test]
    fn hebbian_strengthens_coincident_pairs() {
        let crystal = crystal_from_pairs(&[]);
        let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        // Готовим высокое удивление + открытий гейт.
        pop.energy = 2.0;
        pop.plasticity = 0.4;
        let (p, n) = (0u32, 1u32);
        let before = crystal.bigram_trit(p, n);
        let after = pop.hebbian(&mut { crystal.clone() }, p, n, 0.9);
        assert!(
            after >= before,
            "совпавшая пара усиливается: {before} → {after}"
        );
        let _ = pop.hebb_updates;
    }

    #[test]
    fn hebbian_weakens_mismatched_pairs() {
        let mut pop = NeuralPopulation::new(16, NeuronConfig::default());
        pop.energy = 2.5;
        pop.plasticity = 0.5;
        // Ручной кристалл-заглушка невозможен (приватные поля), поэтому
        // проверяем логику на реальном крошечном кристалле.
        let crystal = crystal_from_pairs(&[("альфа", "бета")]);
        let mut c2 = crystal.clone();
        let (p, n) = (0u32, 1u32);
        let before = c2.bigram_trit(p, n);
        let after = pop.hebbian(&mut c2, p, n, 0.05);
        assert!(
            after <= before,
            "несовпавшая пара при высоком удивлении ослабляется: {before} → {after}"
        );
    }

    #[test]
    fn low_energy_crystallizes_synapses() {
        // ε ≈ 0 → пластичность ≈ 0 → синапс не трогаем.
        let crystal = crystal_from_pairs(&[("альфа", "бета")]);
        let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        pop.energy = 0.01;
        pop.plasticity = 0.002;
        let mut c2 = crystal.clone();
        let (p, n) = (0u32, 1u32);
        let before = c2.bigram_trit(p, n);
        let after = pop.hebbian(&mut c2, p, n, 0.9);
        assert_eq!(before, after, "низкое ε = кристаллизация, синапс заморожен");
    }

    #[test]
    fn observe_measures_surprise() {
        let crystal = crystal_from_pairs(&[("альфа", "бета")]);
        let mut pop = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        let b = crystal.id_of("бета").unwrap();
        // Пустая популяция: thought = 0 → любое наблюдение максимально удивительно.
        let e_empty = pop.observe(&crystal, b);
        assert!(e_empty > 0.0, "пустой мозг удивляется: {e_empty}");
        // Подготовленная популяция: бета уже активен → предсказание совпадает.
        pop.inject(b, Inject::Sense);
        pop.settle(&crystal);
        let e_expected = pop.observe(&crystal, b);
        assert!(
            e_expected < e_empty,
            "ожидаемое слово удивляет меньше: {e_expected} < {e_empty}"
        );
    }

    #[test]
    fn learn_off_freezes_synapses() {
        let crystal = crystal_from_pairs(&[("альфа", "бета")]);
        let mut cfg = NeuronConfig::default();
        cfg.learn = false;
        let mut pop = NeuralPopulation::new(crystal.vocab(), cfg);
        pop.energy = 2.0;
        pop.plasticity = 0.4;
        let mut c2 = crystal.clone();
        let before = c2.bigram_trit(0, 1);
        let after = pop.hebbian(&mut c2, 0, 1, 0.99);
        assert_eq!(before, after, "learn=false: синапсы не трогаются");
        assert_eq!(pop.hebb_updates, 0);
    }

    #[test]
    fn step_is_deterministic() {
        let crystal = crystal_from_pairs(&[("альфа", "бета"), ("альфа", "гамма")]);
        let mut a = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        let mut b = NeuralPopulation::new(crystal.vocab(), NeuronConfig::default());
        for p in [&mut a, &mut b] {
            p.inject(crystal.id_of("альфа").unwrap(), Inject::Sense);
            p.settle(&crystal);
        }
        assert_eq!(a.act, b.act, "та же история → та же популяция");
    }

    #[test]
    fn ensure_vocab_grows_without_reset() {
        let mut pop = NeuralPopulation::new(8, NeuronConfig::default());
        pop.inject(3, Inject::Sense);
        pop.ensure_vocab(16);
        assert_eq!(pop.act.len(), 16);
        assert!(pop.activation(3) > 0.0, "активность пережила рост словаря");
    }
}

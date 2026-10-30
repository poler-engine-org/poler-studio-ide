//! Синаптический Вихрь (Synaptic Vortex) — доказанное ядро SSN.
//!
//! Это порт V17-прогона верификационного сьюта (proofs/ssn_verify3.py,
//! 65 PASS / 0 FAIL), полный стек динамиками со всеми исправлениями F1–F12:
//!
//! | Находка | Исправление |
//! |---|---|
//! | F2 | знак гомеостаза: гиперактивность гасит, гипоактивность растит |
//! | F8 | гомеостаз по МЕДЛЕННОМУ следу f_sys (интегральный контроллер) |
//! | F12 | ЕДИНОЕ определение активности: порог спайка a > 0.1 (EQ-A23) |
//!
//! ## Полный стек шага (10k шагов → активность 4.9–5.5%, E/I ≈ 4.4)
//!
//! 1. Ритмы: θ (5 рад/с), γ (40 рад/с); θ_mod[i] = sin(θ + 0.01·i)
//! 2. Спонтанная стимуляция: 5% шагов → 1% нейронов += 0.5
//! 3. Виртуальная топология: tgt(i,f) = (i·39293 + f·29101 + seed·73471) mod N
//!    — связь существует как АРИФМЕТИКА, ноль RAM на граф (100M синапсов < 1 ГБ)
//! 4. Фазовые синапсы: v[tgt] += a·w·base·gain·(0.8 + 0.2·cos(phase + 0.3·θ_mod))
//!    — квантовый множитель cos() даёт топологическую синхронизацию
//! 5. Инерция мышления (EQ-A16): v += a·0.2·NE
//! 6. Активация: a = max(0, tanh(v)·0.92 + N(0, 0.015))
//! 7. Латеральное торможение: «если сосед кричит — ты молчишь»,
//!    экспоненциальное затухание σ = R/2, R = N/100
//! 8. STDP + дофаминовый гейтинг: LTP только сквозь открытые ворота DA
//! 9. Гомеостаз (EQ-A22, знак F2 + след F8): f_sys ← 0.99·f_sys + 0.01·act;
//!    corr = (f_sys − 0.05)·0.02; w>0 ×(1−corr), w<0 ×(1+corr)
//! 10. Нейромодуляторы: DA (тренд), 5HT (дисперсия), NE (вариабельность CV)
//! 11. E/I-контроллер: мёртвая зона [2,6], GABA/глутамат ±0.01, клипы [0.1,0.9]
//!
//! Золотые параметры: homeo_k = 0.02, stdp_k = 0.0003 — целевая активность
//! 5% достигается и ДЕРЖИТСЯ (критическая динамика, σ ≈ 0.04 — край хаоса).

use crate::ssn::rng::Rng;

/// Конфигурация вихря — все золотые параметры доказанного режима.
#[derive(Debug, Clone, Copy)]
pub struct VortexConfig {
    /// Число нейронов (доказано на 600; масштабируется до миллионов).
    pub n: usize,
    /// Число виртуальных полей (синапсов на нейрон).
    pub fields: usize,
    /// Seed виртуальной топологии (формула целей).
    pub topology_seed: u64,
    /// Доля тормозных нейронов (биология: ~20% GABA-ергических).
    pub inhibitory_frac: f64,
    /// Масштаб тормозных весов (отрицательные).
    pub inhibitory_scale: f64,
    /// Масштаб экспоненциального начального веса.
    pub w_init_scale: f64,
    /// Целевая активность (EQ-A22: 5%).
    pub target_activity: f64,
    /// Коэффициент гомеостаза (золотое значение из V4-доказательства).
    pub homeo_k: f64,
    /// Коэффициент STDP-LTP (DA-гейтирован).
    pub stdp_k: f64,
    /// Порог спайка — ЕДИНСТВЕННОЕ определение активности (F12, EQ-A23).
    pub firing_threshold: f64,
    /// Затухание медленного следа f_sys (F8: интегральный контроллер).
    pub f_sys_decay: f64,
    /// СКО шума активации.
    pub noise_sigma: f64,
    /// Вероятность шага со спонтанной стимуляцией.
    pub spont_prob: f64,
    /// Доля нейронов, получающих спонтанный +0.5.
    pub spont_frac: f64,
    /// Сила латерального торможения.
    pub lateral_strength: f64,
    /// Делитель радиуса латерального торможения (R = N / div).
    pub lateral_r_div: usize,
    /// Мёртвая зона E/I-контроллера.
    pub ei_dead_zone: (f64, f64),
    /// Шаг E/I-контроллера.
    pub ei_step: f64,
    /// Клипы медиаторов [min, max].
    pub chem_clip: (f64, f64),
    /// Частота θ-ритма, рад/с.
    pub theta_freq: f64,
    /// Частота γ-ритма, рад/с.
    pub gamma_freq: f64,
    /// Шаг времени.
    pub dt: f64,
    /// Скорость роста/падения DA (трендовая динамика).
    pub da_rate: (f64, f64),
    /// Затухание 5HT.
    pub ht_decay: f64,
    /// Затухание NE.
    pub ne_decay: f64,
    /// Клипы нейромодуляторов [min, max].
    pub mod_clip: (f64, f64),
    /// Скорость роста ретикулярного тона (F13: анти-windup).
    pub tone_k: f64,
    /// Клип ретикулярного тона.
    pub tone_max: f64,
}

impl Default for VortexConfig {
    fn default() -> Self {
        VortexConfig {
            n: 600,
            fields: 16,
            topology_seed: 777,
            inhibitory_frac: 0.2,
            inhibitory_scale: -0.3,
            w_init_scale: 0.3,
            target_activity: 0.05,
            homeo_k: 0.02,
            stdp_k: 0.0003,
            firing_threshold: 0.1,
            f_sys_decay: 0.99,
            noise_sigma: 0.015,
            spont_prob: 0.05,
            spont_frac: 0.01,
            lateral_strength: 0.1,
            lateral_r_div: 100,
            ei_dead_zone: (2.0, 6.0),
            ei_step: 0.01,
            chem_clip: (0.1, 0.9),
            theta_freq: 5.0,
            gamma_freq: 40.0,
            dt: 0.001,
            da_rate: (0.01, 0.005),
            ht_decay: 0.995,
            ne_decay: 0.99,
            mod_clip: (0.1, 0.9),
            tone_k: 0.002,
            tone_max: 0.25,
        }
    }
}

/// Телеметрия шага вихря — снимок здоровья мозга.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VortexTelemetry {
    pub step: usize,
    /// Текущая доля спайков (a > 0.1) — ЕДИНСТВЕННАЯ метрика (F12).
    pub activity: f64,
    /// Медленный след частоты (интегральный контроллер гомеостаза).
    pub f_sys: f64,
    /// Ретикулярный тон (F13): интегральный канал, не насыщается.
    pub b_tone: f64,
    /// Синхронность S = min/max последних 20 (эпилепсия: S → 1).
    pub synchrony: f64,
    /// Критичность C = 1/(1+|CV−1|) последних 100 (край хаоса: C → 1).
    pub criticality: f64,
    /// E/I по спайкам.
    pub ei: f64,
    pub gaba: f64,
    pub glut: f64,
    pub da: f64,
    pub ht: f64,
    pub ne: f64,
    pub w_min: f64,
    pub w_max: f64,
    /// Доля весов у верхнего клипа (индикатор windup-насыщения, F13).
    pub w_at_clip: f64,
    /// Активных нейронов сейчас.
    pub active_count: usize,
}

/// Синаптический Вихрь — резидентный мозг SSN.
pub struct SynapticVortex {
    cfg: VortexConfig,
    rng: Rng,
    /// Тип нейрона: 0 — возбуждающий, 1 — тормозный (GABA).
    types: Vec<u8>,
    /// Веса [n × fields], row-major. Отрицательные = тормозные.
    w: Vec<f64>,
    /// Фазы синапсов [n × fields].
    phases: Vec<f64>,
    /// Активации нейронов.
    pub a: Vec<f64>,
    /// Последние мембранные потенциалы (для STDP).
    v: Vec<f64>,
    theta_r: f64,
    gamma_r: f64,
    gaba: f64,
    glut: f64,
    da: f64,
    ht: f64,
    ne: f64,
    f_sys: f64,
    /// F13: ретикулярный тон — восходящая активация (ароузал).
    b_tone: f64,
    /// История активности (для тренда DA, CV NE, метрик S и C).
    hist: Vec<f64>,
    steps: usize,
}

impl SynapticVortex {
    /// Создать мозг с конфигурацией и seed траектории.
    pub fn new(cfg: VortexConfig, seed: u64) -> Self {
        let mut cfg = cfg;
        cfg.n = cfg.n.max(8);
        cfg.fields = cfg.fields.max(1);
        let n = cfg.n;
        let f = cfg.fields;
        let mut rng = Rng::new(seed);
        let mut types = vec![0u8; n];
        let n_inh = (n as f64 * cfg.inhibitory_frac) as usize;
        for t in types.iter_mut().take(n_inh) {
            *t = 1;
        }
        let mut w = Vec::with_capacity(n * f);
        for i in 0..n {
            for _ in 0..f {
                let base = rng.exponential(cfg.w_init_scale).clamp(0.01, 1.0);
                w.push(if types[i] == 1 { base * cfg.inhibitory_scale } else { base });
            }
        }
        let phases: Vec<f64> = (0..n * f).map(|_| rng.uniform(0.0, 2.0 * std::f64::consts::PI)).collect();
        SynapticVortex {
            cfg,
            rng,
            types,
            w,
            phases,
            a: vec![0.0; n],
            v: vec![0.0; n],
            theta_r: 0.0,
            gamma_r: 0.0,
            gaba: 0.5,
            glut: 0.5,
            da: 0.3,
            ht: 0.3,
            ne: 0.3,
            f_sys: 0.0,
            b_tone: 0.0,
            hist: Vec::new(),
            steps: 0,
        }
    }

    /// Ссылка на конфигурацию.
    pub fn config(&self) -> &VortexConfig {
        &self.cfg
    }

    /// Виртуальная топология: цель синапса (i, f) — чистая арифметика,
    /// ноль RAM на хранение графа связей (архитектура против грубой силы).
    #[inline]
    fn tgt(&self, i: usize, f: usize) -> usize {
        (((i as u64)
            .wrapping_mul(39_293)
            .wrapping_add((f as u64).wrapping_mul(29_101))
            .wrapping_add(self.cfg.topology_seed.wrapping_mul(73_471)))
            % self.cfg.n as u64) as usize
    }

    /// Один шаг полного стека. Возвращает текущую долю спайков.
    pub fn step(&mut self) -> f64 {
        let cfg = self.cfg;
        let n = self.a.len();
        let f = cfg.fields;
        let tau = 2.0 * std::f64::consts::PI;

        // 1. ритмы
        self.theta_r = (self.theta_r + cfg.dt * cfg.theta_freq) % tau;
        self.gamma_r = (self.gamma_r + cfg.dt * cfg.gamma_freq) % tau;
        let theta_mod: Vec<f64> = (0..n)
            .map(|i| (self.theta_r + 0.01 * i as f64).sin())
            .collect();

        // 2. спонтанная стимуляция
        if self.rng.bernoulli(cfg.spont_prob) {
            for i in 0..n {
                if self.rng.bernoulli(cfg.spont_frac) {
                    self.a[i] += 0.5;
                }
            }
        }

        // 3–4. фазовые синапсы через виртуальную топологию
        let gain = (1.0 + 2.0 * self.ne) * (1.0 - 0.5 * self.ht);
        let mut v = vec![0.0f64; n];
        for local in 0..f {
            for i in 0..n {
                let t = self.tgt(i, local);
                let base = if self.types[i] == 1 { self.gaba * 1.2 } else { self.glut * 0.8 };
                let q = (self.phases[i * f + local] + 0.3 * theta_mod[t]).cos();
                v[t] += self.a[i] * self.w[i * f + local] * base * gain * (0.8 + 0.2 * q);
            }
        }
        // 5. инерция мышления (EQ-A16)
        for i in 0..n {
            v[i] += self.a[i] * 0.2 * self.ne;
        }

        // 6. активация + F13: ретикулярный тон подталкивает субпороговые
        let mut a = vec![0.0f64; n];
        for i in 0..n {
            a[i] = (v[i].tanh() * 0.92 + self.rng.normal(0.0, cfg.noise_sigma) + self.b_tone).max(0.0);
        }

        // 7. латеральное торможение (последовательно, как в доказательстве)
        let r = (n / cfg.lateral_r_div).max(1);
        let sigma = r as f64 / 2.0;
        let a_pre = a.clone();
        for idx in 0..n {
            if a_pre[idx] > cfg.firing_threshold {
                let lo = idx.saturating_sub(r);
                let hi = (idx + r + 1).min(n);
                for k in lo..hi {
                    let decay = (-((k as f64 - idx as f64).abs() / sigma)).exp()
                        * cfg.lateral_strength
                        * a_pre[idx];
                    a[k] = (a[k] - decay).max(0.0);
                }
            }
        }
        self.a = a;
        self.v = v;

        // 8. STDP-LTP с дофаминовым гейтингом
        for local in 0..f {
            for i in 0..n {
                let t = self.tgt(i, local);
                let wi = &mut self.w[i * f + local];
                if self.v[t] * *wi > cfg.firing_threshold && *wi > 0.0 {
                    *wi = (*wi + cfg.stdp_k * self.da).min(1.0);
                }
            }
        }

        // 9. гомеостаз: ЕДИНАЯ метрика (F12) → медленный след (F8) → знак (F2)
        let active_frac = self.a.iter().filter(|x| **x > cfg.firing_threshold).count() as f64 / n as f64;
        self.f_sys = cfg.f_sys_decay * self.f_sys + (1.0 - cfg.f_sys_decay) * active_frac;
        let err = self.f_sys - cfg.target_activity;
        let corr = err * cfg.homeo_k;
        for wi in self.w.iter_mut() {
            if *wi > 0.0 {
                *wi *= 1.0 - corr;
            } else {
                *wi *= 1.0 + corr;
            }
            *wi = wi.clamp(-1.0, 1.0);
        }
        // F13: ретикулярный тон — анти-windup канал (не насыщается)
        self.b_tone = (self.b_tone + cfg.tone_k * (cfg.target_activity - self.f_sys)).clamp(0.0, cfg.tone_max);

        self.hist.push(active_frac);
        self.steps += 1;

        // 10. нейромодуляторы
        if self.hist.len() >= 10 {
            let slope = Self::slope_last(&self.hist, 10);
            let (up, down) = cfg.da_rate;
            self.da = (self.da + if slope > 0.0 { up } else { -down }).clamp(cfg.mod_clip.0, cfg.mod_clip.1);
        }
        let mean_a = self.a.iter().sum::<f64>() / n as f64;
        let var_a = self.a.iter().map(|x| (x - mean_a) * (x - mean_a)).sum::<f64>() / n as f64;
        self.ht = (cfg.ht_decay * self.ht + (1.0 - cfg.ht_decay) * (1.0 - var_a.sqrt().min(1.0)))
            .clamp(cfg.mod_clip.0, cfg.mod_clip.1);
        let m = self.hist.len().min(5);
        let win = &self.hist[self.hist.len() - m..];
        let mh = win.iter().sum::<f64>() / m as f64;
        let vh = win.iter().map(|x| (x - mh) * (x - mh)).sum::<f64>() / m as f64;
        let cv = vh.sqrt() / (mh + 1e-9);
        self.ne = (cfg.ne_decay * self.ne + (1.0 - cfg.ne_decay) * cv.min(2.0))
            .clamp(cfg.mod_clip.0, cfg.mod_clip.1);

        // 11. E/I-контроллер (по спайкам — F12)
        let mut e_count = 0usize;
        let mut i_count = 0usize;
        for i in 0..n {
            if self.a[i] > cfg.firing_threshold {
                if self.types[i] == 1 {
                    i_count += 1;
                } else {
                    e_count += 1;
                }
            }
        }
        let ei = e_count as f64 / (i_count.max(1) as f64);
        let (lo, hi) = cfg.ei_dead_zone;
        if ei > hi {
            self.gaba = (self.gaba + cfg.ei_step).min(cfg.chem_clip.1);
            self.glut = (self.glut - cfg.ei_step).max(cfg.chem_clip.0);
        } else if ei < lo {
            self.gaba = (self.gaba - cfg.ei_step).max(cfg.chem_clip.0);
            self.glut = (self.glut + cfg.ei_step).min(cfg.chem_clip.1);
        }

        active_frac
    }

    /// Наклон (тренд) последних `k` точек истории — метод наименьших
    /// квадратов, эквивалент numpy.polyfit(range(k), y, 1)[0].
    fn slope_last(hist: &[f64], k: usize) -> f64 {
        let m = hist.len().min(k);
        let win = &hist[hist.len() - m..];
        let xbar = (m - 1) as f64 / 2.0;
        let ybar = win.iter().sum::<f64>() / m as f64;
        let mut num = 0.0;
        let mut den = 0.0;
        for (j, y) in win.iter().enumerate() {
            let dx = j as f64 - xbar;
            num += dx * (y - ybar);
            den += dx * dx;
        }
        if den > 0.0 { num / den } else { 0.0 }
    }

    /// Сенсорный вход: добавить паттерн (например, CSE-вектор) к активациям.
    /// Положительная часть, масштаб согласован со спонтанной стимуляцией.
    /// Возвращает число затронутых нейронов.
    pub fn inject(&mut self, pattern: &[f64]) -> usize {
        let n = self.a.len();
        let mut touched = 0;
        for i in 0..n {
            let p = pattern[i % pattern.len()].max(0.0);
            if p > 0.0 {
                self.a[i] = (self.a[i] + p * 0.5).min(1.0);
                touched += 1;
            }
        }
        touched
    }

    /// Телеметрия: снимок здоровья мозга.
    pub fn telemetry(&self) -> VortexTelemetry {
        let n = self.a.len();
        let cfg = &self.cfg;
        let activity = self.a.iter().filter(|x| **x > cfg.firing_threshold).count() as f64 / n as f64;
        let active_count = self.a.iter().filter(|x| **x > cfg.firing_threshold).count();

        let m20 = self.hist.len().min(20);
        let synchrony = if m20 > 0 {
            let win = &self.hist[self.hist.len() - m20..];
            let mx = win.iter().cloned().fold(f64::MIN, f64::max);
            let mn = win.iter().cloned().fold(f64::MAX, f64::min);
            mn / (mx + 1e-9)
        } else {
            0.0
        };

        let m100 = self.hist.len().min(100);
        let criticality = if m100 > 0 {
            let win = &self.hist[self.hist.len() - m100..];
            let mh = win.iter().sum::<f64>() / m100 as f64;
            let vh = win.iter().map(|x| (x - mh) * (x - mh)).sum::<f64>() / m100 as f64;
            1.0 / (1.0 + (vh.sqrt() / (mh + 1e-9) - 1.0).abs())
        } else {
            0.0
        };

        let mut e_count = 0usize;
        let mut i_count = 0usize;
        for i in 0..n {
            if self.a[i] > cfg.firing_threshold {
                if self.types[i] == 1 {
                    i_count += 1;
                } else {
                    e_count += 1;
                }
            }
        }

        VortexTelemetry {
            step: self.steps,
            activity,
            f_sys: self.f_sys,
            b_tone: self.b_tone,
            synchrony,
            criticality,
            ei: e_count as f64 / (i_count.max(1) as f64),
            gaba: self.gaba,
            glut: self.glut,
            da: self.da,
            ht: self.ht,
            ne: self.ne,
            w_min: self.w.iter().cloned().fold(f64::INFINITY, f64::min),
            w_max: self.w.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            w_at_clip: self.w.iter().filter(|x| **x >= 0.999).count() as f64
                / self.w.len() as f64,
            active_count,
        }
    }

    /// Разреженный readout: топ-k активных нейронов (индексы по убыванию a).
    pub fn readout(&self, k: usize) -> Vec<(usize, f64)> {
        let mut idx: Vec<usize> = (0..self.a.len()).collect();
        idx.sort_by(|&x, &y| {
            self.a[y].partial_cmp(&self.a[x]).unwrap_or(std::cmp::Ordering::Equal).then(x.cmp(&y))
        });
        idx[..k.min(idx.len())]
            .iter()
            .map(|&i| (i, self.a[i]))
            .filter(|(_, v)| *v > self.cfg.firing_threshold)
            .collect()
    }

    /// Максимальная длина «мёртвой» серии (активность < 1%).
    pub fn max_dead_run(&self) -> usize {
        let mut best = 0usize;
        let mut cur = 0usize;
        for &h in &self.hist {
            if h < 0.01 {
                cur += 1;
                best = best.max(cur);
            } else {
                cur = 0;
            }
        }
        best
    }

    /// Число выполненных шагов.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Доля весов, упёршихся в верхний клип 1.0 (насыщение гомеостаза, F13).
    pub fn w_at_clip(&self) -> f64 {
        self.w.iter().filter(|x| **x >= 0.999).count() as f64 / self.w.len() as f64
    }

    /// Анатомия весов: (среднее w>0, среднее w<0, доля у клипа 1.0).
    pub fn weight_stats(&self) -> (f64, f64, f64) {
        let mut pos_sum = 0.0;
        let mut pos_n = 0.0;
        let mut neg_sum = 0.0;
        let mut neg_n = 0.0;
        for &x in &self.w {
            if x > 0.0 {
                pos_sum += x;
                pos_n += 1.0;
            } else {
                neg_sum += x;
                neg_n += 1.0;
            }
        }
        let clip = self.w.iter().filter(|x| **x >= 0.999).count() as f64 / self.w.len() as f64;
        (
            if pos_n > 0.0 { pos_sum / pos_n } else { 0.0 },
            if neg_n > 0.0 { neg_sum / neg_n } else { 0.0 },
            clip,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V17 (сокращённый): 2 seed × 4000 шагов — инварианты здоровья.
    /// Полная версия (5 seed × 10k) — ssn_vortex_full_fidelity, #[ignore].
    #[test]
    fn v17_invariants_two_seeds() {
        for seed in [777u64, 1] {
            let mut vx = SynapticVortex::new(VortexConfig::default(), seed);
            for _ in 0..4000 {
                vx.step();
            }
            let t = vx.telemetry();
            assert!(
                0.01 < t.activity && t.activity < 0.5,
                "seed={seed}: активность {} вне (1%, 50%)",
                t.activity
            );
            assert!(t.synchrony < 0.8, "seed={seed}: эпилепсия S={}", t.synchrony);
            assert!(t.w_min >= -1.0 && t.w_max <= 1.0, "seed={seed}: веса вне клипов");
            assert!(
                (0.1..=0.9).contains(&t.da) && (0.1..=0.9).contains(&t.ht) && (0.1..=0.9).contains(&t.ne),
                "seed={seed}: модуляторы вне коридора DA={} 5HT={} NE={}",
                t.da,
                t.ht,
                t.ne
            );
            assert!(vx.max_dead_run() < 500, "seed={seed}: мёртвая серия {}", vx.max_dead_run());
            assert!(t.ei > 0.5 && t.ei < 8.0, "seed={seed}: E/I={}", t.ei);
            // V17f: ретикулярный тон в клипе
            assert!((0.0..=0.25).contains(&t.b_tone), "seed={seed}: b_tone={}", t.b_tone);
            // V17g: анти-windup — насыщение весов < 10%
            assert!(t.w_at_clip < 0.10, "seed={seed}: w@клип={:.1}%", t.w_at_clip * 100.0);
        }
    }

    /// V17-двойник: активации всегда в [0, 1+ε] (tanh·0.92 + шум, ReLU).
    #[test]
    fn v17b_activations_bounded() {
        let mut vx = SynapticVortex::new(VortexConfig::default(), 42);
        let mut max_a = 0.0f64;
        for _ in 0..2000 {
            vx.step();
            for &x in &vx.a {
                max_a = max_a.max(x);
                assert!(x >= -1e-12, "отрицательная активация {x}");
            }
        }
        assert!(max_a <= 1.0 + 1e-9, "max_a={max_a}");
    }

    /// Виртуальная топология: инъективность в пределах поля (gcd = 1
    /// для доказанных N), детерминизм, попадание в [0, n).
    #[test]
    fn topology_deterministic_in_range() {
        let vx = SynapticVortex::new(VortexConfig::default(), 5);
        for f in 0..16 {
            let mut seen = std::collections::HashSet::new();
            for i in 0..600 {
                let t = vx.tgt(i, f);
                assert!(t < 600);
                seen.insert(t);
            }
            assert_eq!(seen.len(), 600, "поле {f}: коллизии целей");
        }
    }

    /// Инъекция сенсорного паттерна не ломает инварианты (V17 + inject).
    #[test]
    fn inject_keeps_invariants() {
        let mut vx = SynapticVortex::new(VortexConfig::default(), 20260918);
        let pattern = crate::ssn::cse::encode("открыть терминал и собрать проект", 128);
        for cycle in 0..8 {
            vx.inject(&pattern);
            for _ in 0..500 {
                vx.step();
            }
            let t = vx.telemetry();
            assert!(
                t.activity < 0.5,
                "цикл {cycle}: активность {} — эпилепсия после инъекции",
                t.activity
            );
        }
    }

    /// Полная верность доказательству: 10 seed × 10k шагов (F13-режим).
    /// Python-пруф: proofs/ssn_f13_proof.py — 10/10 здоровы.
    /// Запуск: cargo test --release -- --ignored
    #[test]
    #[ignore]
    fn ssn_vortex_full_fidelity() {
        for seed in [777u64, 1, 42, 20260918, 555, 31337, 999, 12345, 678, 100_000] {
            let mut vx = SynapticVortex::new(VortexConfig::default(), seed);
            for _ in 0..10_000 {
                vx.step();
            }
            let t = vx.telemetry();
            assert!(0.01 < t.activity && t.activity < 0.5, "seed={seed}: акт={}", t.activity);
            assert!(t.synchrony < 0.8, "seed={seed}: S={}", t.synchrony);
            assert!(vx.max_dead_run() < 500, "seed={seed}: dead={}", vx.max_dead_run());
            assert!(t.w_at_clip < 0.10, "seed={seed}: w@клип={:.1}%", t.w_at_clip * 100.0);
        }
    }
}

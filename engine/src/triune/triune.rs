//! TriuneCore — Триединая Архитектура: муха + вихрь + кристалл → речь.
//!
//! Оркестратор S2/v0.36.0. Три опоры сходятся в одном цикле порождения
//! слова (всё доказано до реализации — принцип F-сьита):
//!
//! ```text
//! сенсорный вход ──CSE──▶ ┌─────────────────────────────┐
//!                         │ 1. ВИХРЬ (SSN, v0.35.0)     │ живой мозг:
//!                         │    шаги, гомеостаз,         │ активность ~5%,
//!                         │    медиаторы DA/5HT/NE      │ критичность
//!                         │    ↓ температура речи τ     │
//!                         │ 2. МУХА (FlyPulse)          │ пульс и драйв:
//!                         │    p ← p + dt(γJ·p − D·p)   │ ротор не даёт
//!                         │    ↓ дрейф по архетипам     │ мысли застыть
//!                         │ 3. КРИСТАЛЛ (.t5c, Trit5)   │ знания мира:
//!                         │    sem (SIMD No-Mul)        │ троичные веса,
//!                         │    syn (биграммный трит)    │ 5 тритов/байт
//!                         └─────────────┬───────────────┘
//!                    NMDA-гейт совпадения смыслов (MindOS)
//!                         ▼ WTA(k) → softmax(τ) → слово
//!              речь ── собственный голос возвращается в мозг ──┐
//!                └──────────────────────────────────────────────┘
//! ```
//!
//! ## Что даёт каждая опора (проверяемо в трейсе токена)
//!
//! - **Вихрь**: медиаторы задают температуру τ сэмплирования —
//!   дофамин открывает исследования, серотонин держит стабильность.
//! - **Муха**: дрейф `tanh((γJ·p)[архетип токена])` разводит лексику
//!   по архетипам — при γ = 0 речь вырождается в циклы (тест
//!   `fly_rotation_breaks_loops`).
//! - **Кристалл**: семантика через SIMD-триты (без умножения) +
//!   биграммный трит синтаксиса.
//! - **NMDA-гейт**: слово проходит только при совпадении смысла с
//!   контекстом (V9: 0.995 align / 0.000 ortho).
//!
//! Каждый токен несёт полный провенанс [`TokenTrace`] — агент видит,
//! КАКАЯ опора и насколько двигала каждое слово (прозрачность вместо
//! чёрного ящика).

use std::collections::HashMap;

use crate::ssn::mindos;
use crate::ssn::rng::Rng;
use crate::ssn::vortex::{SynapticVortex, VortexConfig, VortexTelemetry};
use crate::triune::crystal::{Crystal, ARCHETYPES};
use crate::triune::flypulse::{FlyPulse, PulseOrigin};
use crate::triune::motor::{self, MotorIntent};
use crate::triune::neuron::{Inject, NeuralPopulation, NeuronConfig, NeuronTelemetry};

/// Конфигурация Триединства.
#[derive(Debug, Clone)]
pub struct TriuneConfig {
    /// Конфиг живого мозга (вихря).
    pub vortex: VortexConfig,
    /// Размерность CSE (общая для сенсорики и кристалла).
    pub dims: usize,
    /// Шагов вихря на один токен речи (пульс).
    pub steps_per_token: usize,
    /// Вес семантического счёта кристалла.
    pub w_sem: f64,
    /// Вес биграммного трита.
    pub w_syn: f64,
    /// Вес мушиного дрейфа.
    pub w_fly: f64,
    /// Штраф повтора токена (анти-луп; 0 = выключен).
    pub lambda_rep: f64,
    /// Число лидеров WTA перед сэмплированием.
    pub wta_k: usize,
    /// Базовая температура речи.
    pub tau_base: f64,
    /// Окно контекста для CSE (в токенах).
    pub ctx_window: usize,
    /// Громкость эха собственной речи (инъекция в мозг, 0..1).
    pub echo_scale: f64,
    /// Динамическое расширение словаря: неизвестное слово промпта
    /// мгновенно получает CSE-вложение и входит в активную лексику
    /// (Dynamic Vocab Expansion, v0.37.0).
    pub dynamic_vocab: bool,
    /// Предел динамических слов за сессию (сверх — subword-фолбек).
    pub dynamic_vocab_cap: usize,
    /// Нейронная популяция поверх трит-синапсов (v0.38.0):
    /// резонансная память + энергогейтная пластичность.
    pub neurons: NeuronConfig,
    /// Вес активности нейрона в счёте кандидата.
    pub w_act: f64,
}

impl Default for TriuneConfig {
    fn default() -> Self {
        TriuneConfig {
            vortex: VortexConfig::default(),
            dims: crate::triune::crystal::DEFAULT_DIMS,
            steps_per_token: 40,
            w_sem: 2.0,
            w_syn: 1.5,
            w_fly: 1.2,
            lambda_rep: 0.6,
            wta_k: 8,
            tau_base: 0.9,
            ctx_window: 2,
            echo_scale: 0.25,
            dynamic_vocab: true,
            dynamic_vocab_cap: 2048,
            neurons: NeuronConfig::default(),
            w_act: 1.6,
        }
    }
}

/// Провенанс одного сказанного токена.
#[derive(Clone, Debug)]
pub struct TokenTrace {
    pub token: String,
    pub idx: u32,
    /// Семантика кристалла (SIMD No-Mul трит-скалярное).
    pub sem: f64,
    /// NMDA-гейт совпадения смыслов [0..1].
    pub gate: f64,
    /// Биграммный трит −1/0/+1.
    pub syn: i8,
    /// Дрейф мухи по архетипу токена [−1..1].
    pub fly: f64,
    /// Штраф повтора.
    pub rep: f64,
    /// Итоговый счёт.
    pub score: f64,
    /// Температура сэмплирования (от медиаторов).
    pub tau: f64,
    /// Активность вихря в момент слова.
    pub activity: f64,
    /// Активность нейрона токена в популяции (v0.38.0).
    pub act: f64,
}

/// Высказывание: текст + трейс + финальная телеметрия + интенты.
pub struct Utterance {
    pub text: String,
    pub trace: Vec<TokenTrace>,
    pub telemetry: VortexTelemetry,
    /// Финальный дрейф мухи.
    pub fly_drift: [f32; ARCHETYPES],
    /// Моторные предложения (S2).
    pub intents: Vec<MotorIntent>,
    /// Какая муха билась (настоящая/виртуальная).
    pub fly_origin: PulseOrigin,
    /// Словарь кристалла.
    pub crystal_vocab: usize,
    /// Телеметрия нейронной популяции (v0.38.0).
    pub neurons: NeuronTelemetry,
}

/// Триединое ядро: резидентная говорящая система.
pub struct TriuneCore {
    /// Живой мозг (вихрь SSN).
    pub vortex: SynapticVortex,
    /// Пульс мухи.
    pub fly: FlyPulse,
    /// Кристалл знаний.
    pub crystal: Crystal,
    rng: Rng,
    cfg: TriuneConfig,
    /// Счётчик употреблений токенов (анти-луп).
    usage: HashMap<u32, u32>,
    /// Последние токены (контекст).
    ctx: Vec<u32>,
    /// Сколько новых слов добавлено динамически за сессию.
    dynamic_added: usize,
    /// Нейронная популяция токен-нейронов (v0.38.0).
    pub neurons: NeuralPopulation,
    /// Сенсорных инъекций сделано.
    pub injections: usize,
    /// Всего токенов произнесено.
    pub tokens_spoken: u64,
}

impl TriuneCore {
    /// Новое Триединство. `fly` — настоящая каста или виртуальная муха.
    pub fn new(crystal: Crystal, cfg: TriuneConfig, fly: FlyPulse, seed: u64) -> TriuneCore {
        let vortex = SynapticVortex::new(cfg.vortex, seed);
        let neurons = NeuralPopulation::new(crystal.vocab(), cfg.neurons.clone());
        TriuneCore {
            vortex,
            fly,
            crystal,
            rng: Rng::new(seed ^ 0x7472_696e_6564_0001),
            cfg,
            usage: HashMap::new(),
            ctx: Vec::new(),
            dynamic_added: 0,
            neurons,
            injections: 0,
            tokens_spoken: 0,
        }
    }

    /// Резолюция слова промпта: словарь → динамическое расширение →
    /// subword-фолбек. Возвращает id последнего куска (контекст),
    /// None — слово неразложимо и лимит исчерпан.
    fn resolve_prompt_token(&mut self, word: &str) -> Option<u32> {
        if let Some(id) = self.crystal.id_of(word) {
            return Some(id);
        }
        if self.cfg.dynamic_vocab && self.dynamic_added < self.cfg.dynamic_vocab_cap {
            if let Some(id) = self.crystal.expand_vocab(word) {
                self.dynamic_added += 1;
                return Some(id);
            }
            return None;
        }
        // Лимит исчерпан (или выключен): жадное разложение на куски.
        self.crystal.subword_ids(word).last().copied()
    }

    /// Температура речи из медиаторов (вихрь дышит — τ плывёт).
    ///
    /// DA ↑ → исследования (τ ↑), 5HT ↑ → стабильность (τ ↓),
    /// NE ↑ → фокус (τ ↓). Зажата в [0.6, 1.8] — медиаторы не
    /// устраивают ни лихорадку, ни кому.
    pub fn temperature(t: &VortexTelemetry, base: f64) -> f64 {
        let da = t.da - 0.5;
        let ht = t.ht - 0.5;
        let ne = t.ne - 0.5;
        (base * (1.0 + 0.6 * da - 0.5 * ht - 0.3 * ne)).clamp(0.6, 1.8)
    }

    /// Говорить от промпта: полный цикл Триединства.
    pub fn speak(&mut self, prompt: &str, max_tokens: usize) -> Utterance {
        // ── Сенсорный вход: весь промпт — в мозг ──
        if !prompt.trim().is_empty() {
            let pattern = crate::ssn::cse::encode(prompt, self.cfg.dims);
            self.vortex.inject(&pattern);
            self.injections += 1;
        }
        // Контекст речи: слова промпта — каждое либо находится в словаре,
        // либо ДОБАВЛЯЕТСЯ на лету (Dynamic Vocab Expansion), либо
        // раскладывается на словарные куски (subword-фолбек).
        let prompt_ids: Vec<u32> = crate::triune::crystal::tokenize(prompt)
            .iter()
            .filter_map(|w| self.resolve_prompt_token(w))
            .collect();
        // Промпт — в нейронную популяцию: слова вспыхивают и держат
        // резонансную память всю фразу (контекст = активации, не окно).
        self.neurons.ensure_vocab(self.crystal.vocab());
        for &id in &prompt_ids {
            self.neurons.inject(id, Inject::Sense);
        }
        self.ctx = prompt_ids.iter().rev().take(self.cfg.ctx_window).cloned().rev().collect();
        let prompt_tail: String = prompt_ids
            .iter()
            .rev()
            .take(3)
            .cloned()
            .rev()
            .map(|i| self.crystal.tokens[i as usize].clone())
            .collect::<Vec<_>>()
            .join(" ");

        let mut trace = Vec::with_capacity(max_tokens);
        let mut words: Vec<String> = Vec::with_capacity(max_tokens);

        for _ in 0..max_tokens {
            // 1. Пульс: вихрь дышит steps_per_token шагов.
            for _ in 0..self.cfg.steps_per_token {
                self.vortex.step();
            }
            let tel = self.vortex.telemetry();
            // 2. Муха крутит фазу касты.
            let drift = self.fly.step();
            let drift_max = drift.iter().fold(0.0f32, |m, &v| m.max(v.abs())).max(1e-6);

            // 2b. Нейроны релаксируют: активация течёт по трит-синапсам,
            //     популяция догоняет контекст (v0.38.0).
            self.neurons.settle(&self.crystal);

            // 3. Контекст (CSE) для семантики и гейта.
            let ctx_str = if self.ctx.is_empty() {
                prompt_tail.clone()
            } else {
                self.ctx
                    .iter()
                    .map(|&i| self.crystal.tokens[i as usize].clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let ctx_f64 = crate::ssn::cse::encode(&ctx_str, self.cfg.dims);
            let ctx_f32: Vec<f32> = ctx_f64.iter().map(|&x| x as f32).collect();
            let prev = self.ctx.last().copied();

            // 4. Счёт всех кандидатов: кристалл + муха + анти-луп.
            let vocab = self.crystal.vocab();
            let tau = Self::temperature(&tel, self.cfg.tau_base);
            let mut scores = vec![0.0f64; vocab];
            let mut sems = vec![0.0f64; vocab];
            let mut gates = vec![0.0f64; vocab];
            let mut syns = vec![0i8; vocab];
            let mut flies = vec![0.0f64; vocab];
            for idx in 0..vocab as u32 {
                let sem = self.crystal.embed_trit(idx).dot(&ctx_f32) as f64;
                let gate = mindos::nmda_gate(self.crystal.embed_f64(idx), &ctx_f64).max(0.0);
                let syn = match prev {
                    Some(p) => self.crystal.bigram_trit(p, idx),
                    None => 0,
                };
                let fly = (drift[self.crystal.anchor(idx)] / drift_max).tanh() as f64;
                let used = *self.usage.get(&idx).unwrap_or(&0);
                let rep = -self.cfg.lambda_rep * used.min(3) as f64;
                let act = self.neurons.activation(idx) as f64;
                sems[idx as usize] = sem;
                gates[idx as usize] = gate;
                syns[idx as usize] = syn;
                flies[idx as usize] = fly;
                scores[idx as usize] = self.cfg.w_sem * sem * gate
                    + self.cfg.w_syn * syn as f64
                    + self.cfg.w_fly * fly
                    + self.cfg.w_act * act
                    + rep;
            }

            // 5. WTA(k): разреженный код лидеров (MindOS).
            let mut wta_scores = scores.clone();
            mindos::wta(&mut wta_scores, self.cfg.wta_k);

            // 6. Softmax(τ) по выжившим → сэмпл.
            let survivors: Vec<usize> = wta_scores
                .iter()
                .enumerate()
                .filter(|(_, &s)| s.abs() > 1e-12)
                .map(|(i, _)| i)
                .collect();
            let (chosen, _) = if survivors.is_empty() {
                // Вырожденный случай: все счёты нулевые — равномерный выбор.
                let i = (self.rng.f64() * vocab as f64) as usize % vocab;
                (i, 0.0f64)
            } else {
                let max_s = survivors.iter().map(|&i| scores[i]).fold(f64::MIN, f64::max);
                let probs: Vec<f64> = survivors
                    .iter()
                    .map(|&i| ((scores[i] - max_s) / tau).exp())
                    .collect();
                let sum: f64 = probs.iter().sum();
                let mut pick = self.rng.f64() * sum;
                let mut chosen_i = survivors[0];
                for (&si, &p) in survivors.iter().zip(&probs) {
                    pick -= p;
                    if pick <= 0.0 {
                        chosen_i = si;
                        break;
                    }
                }
                (chosen_i, scores[chosen_i])
            };

            // 7. Слово сказано: трейс + употребление + контекст.
            let idx = chosen as u32;
            *self.usage.entry(idx).or_insert(0) += 1;
            self.ctx.push(idx);
            if self.ctx.len() > self.cfg.ctx_window {
                self.ctx.remove(0);
            }
            self.tokens_spoken += 1;
            words.push(self.crystal.tokens[idx as usize].clone());
            let act_spoken = self.neurons.activation(idx) as f64;
            trace.push(TokenTrace {
                token: self.crystal.tokens[idx as usize].clone(),
                idx,
                sem: sems[chosen],
                gate: gates[chosen],
                syn: syns[chosen],
                fly: flies[chosen],
                rep: -self.cfg.lambda_rep * (self.usage[&idx] - 1).min(3) as f64,
                score: scores[chosen],
                tau,
                activity: tel.activity,
                act: act_spoken,
            });

            // 7b. Нейронный контур (v0.38.0): энергия удивления →
            //     Хеббовский сдвиг синапса → эхо в популяцию.
            self.neurons.observe(&self.crystal, idx);
            if let Some(p) = prev {
                self.neurons
                    .hebbian(&mut self.crystal, p, idx, gates[chosen].max(0.0));
            }
            self.neurons.inject(idx, Inject::Echo);

            // 8. Мозг слышит собственную речь (замкнутый контур, эхо
            //    приглушено: самослушание — не крик в собственное ухо).
            let echo_raw = crate::ssn::cse::encode(&self.crystal.tokens[idx as usize], self.cfg.dims);
            let echo: Vec<f64> = echo_raw.iter().map(|x| x * self.cfg.echo_scale).collect();
            self.vortex.inject(&echo);
        }

        let telemetry = self.vortex.telemetry();
        let neurons = self.neurons.telemetry();
        let text = words.join(" ");
        let intents = motor::scan(&text);
        Utterance {
            fly_drift: self.fly.drift(),
            fly_origin: self.fly.origin.clone(),
            crystal_vocab: self.crystal.vocab(),
            telemetry,
            trace,
            intents,
            text,
            neurons,
        }
    }

    /// Телеметрия без речи.
    pub fn telemetry(&self) -> VortexTelemetry {
        self.vortex.telemetry()
    }

    /// Число живых сессий-инъекций (для статусных сводок).
    pub fn state_summary(&self) -> (usize, u64, usize) {
        (self.injections, self.tokens_spoken, self.ctx.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_crystal() -> Crystal {
        Crystal::embedded().expect("зашитый кристалл")
    }

    #[test]
    fn speak_is_deterministic() {
        let crystal = test_crystal();
        let mut a = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(777, 0.8),
            42,
        );
        let mut b = TriuneCore::new(crystal, TriuneConfig::default(), FlyPulse::synthetic(777, 0.8), 42);
        let ua = a.speak("живой мозг говорит", 16);
        let ub = b.speak("живой мозг говорит", 16);
        assert_eq!(ua.text, ub.text, "тот же seed → та же речь");
        assert_eq!(ua.trace.len(), 16);
    }

    #[test]
    fn speak_produces_vocabulary_tokens() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(1, 0.8),
            7,
        );
        let u = core.speak("кристалл знаний", 20);
        let words: Vec<&str> = u.text.split(' ').collect();
        assert_eq!(words.len(), 20);
        for w in &words {
            assert!(core.crystal.id_of(w).is_some(), "«{w}» вне словаря кристалла");
            assert!(!w.is_empty());
        }
    }

    #[test]
    fn vortex_stays_alive_during_speech() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(5, 0.8),
            11,
        );
        let u = core.speak("система слушает вход", 30);
        let t = &u.telemetry;
        assert!(
            (0.005..0.25).contains(&t.activity),
            "активность {} вне коридора жизни",
            t.activity
        );
        assert!(t.synchrony < 0.95, "синхронность {} — эпилепсия", t.synchrony);
        assert!(t.active_count > 0, "мозг умер во время речи");
    }

    /// Главная проверка роли мухи: γ = 0 → лексика зацикливается,
    /// живой ротор разводит слова по архетипам. Анти-луп отключён,
    /// чтобы изолировать вклад именно мухи.
    #[test]
    fn fly_rotation_breaks_loops() {
        let mut cfg = TriuneConfig::default();
        cfg.lambda_rep = 0.0;
        let prompt = "система";
        let mut uniq_sleeping = 0.0f64;
        let mut uniq_flying = 0.0f64;
        for seed in 0..8u64 {
            let mut sleeping = TriuneCore::new(
                test_crystal(),
                cfg.clone(),
                FlyPulse::synthetic(seed, 0.0),
                seed * 100 + 1,
            );
            let mut flying = TriuneCore::new(
                test_crystal(),
                cfg.clone(),
                FlyPulse::synthetic(seed, 0.8),
                seed * 100 + 1,
            );
            let us = sleeping.speak(prompt, 40);
            let uf = flying.speak(prompt, 40);
            let ds = us.text.split(' ').collect::<std::collections::BTreeSet<_>>().len() as f64;
            let df = uf.text.split(' ').collect::<std::collections::BTreeSet<_>>().len() as f64;
            uniq_sleeping += ds / 40.0;
            uniq_flying += df / 40.0;
        }
        let mean_s = uniq_sleeping / 8.0;
        let mean_f = uniq_flying / 8.0;
        assert!(
            mean_f > mean_s,
            "муха должна разводить лексику: летающая {mean_f:.3} vs спящая {mean_s:.3}"
        );
    }

    #[test]
    fn temperature_follows_mediators_within_bounds() {
        let tel = |da: f64, ht: f64, ne: f64| VortexTelemetry {
            step: 0,
            activity: 0.05,
            f_sys: 0.05,
            b_tone: 0.5,
            synchrony: 0.3,
            criticality: 0.55,
            ei: 4.4,
            gaba: 0.3,
            glut: 0.7,
            da,
            ht,
            ne,
            w_min: 0.01,
            w_max: 2.0,
            w_at_clip: 0.0,
            active_count: 30,
        };
        let hi = TriuneCore::temperature(&tel(0.9, 0.1, 0.1), 0.9);
        let lo = TriuneCore::temperature(&tel(0.1, 0.9, 0.9), 0.9);
        assert!(hi > lo, "DA↑/5HT↓ должно расширять: {hi:.3} vs {lo:.3}");
        assert!((0.6..=1.8).contains(&hi) && (0.6..=1.8).contains(&lo));
        // Экстремальные медиаторы не пробивают клипы.
        assert!(TriuneCore::temperature(&tel(1.0, 0.0, 0.0), 100.0) <= 1.8);
        assert!(TriuneCore::temperature(&tel(0.0, 1.0, 1.0), 0.001) >= 0.6);
    }

    #[test]
    fn nmda_gate_passes_coincident_tokens() {
        // Средний гейт для токенов контекста выше, чем для случайных.
        let crystal = test_crystal();
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(3, 0.8),
            9,
        );
        let u = core.speak("вихрь крутит смысл", 12);
        assert!(!u.text.is_empty(), "речь не должна быть пустой");
        let ctx = crate::ssn::cse::encode("вихрь крутит смысл", core.cfg.dims);
        let mut gates_ctx = vec![];
        let mut gates_far = vec![];
        for id in ["вихрь", "крутит", "смысл"] {
            if let Some(i) = core.crystal.id_of(id) {
                gates_ctx.push(mindos::nmda_gate(core.crystal.embed_f64(i), &ctx).max(0.0));
            }
        }
        for id in ["терминал", "логи", "статус"] {
            if let Some(i) = core.crystal.id_of(id) {
                gates_far.push(mindos::nmda_gate(core.crystal.embed_f64(i), &ctx).max(0.0));
            }
        }
        if !gates_ctx.is_empty() && !gates_far.is_empty() {
            let mc: f64 = gates_ctx.iter().sum::<f64>() / gates_ctx.len() as f64;
            let mf: f64 = gates_far.iter().sum::<f64>() / gates_far.len() as f64;
            assert!(mc > mf, "совпадающие смыслы должны проходить гейт: {mc:.3} vs {mf:.3}");
        }
        assert_eq!(crystal.vocab(), core.crystal.vocab());
    }

    #[test]
    fn motor_intents_proposed_not_executed() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(13, 0.8),
            21,
        );
        let u = core.speak("система управляет операционной системой", 24);
        // Что бы ни говорил кристалл — интенты только предложения.
        for i in &u.intents {
            assert!(!i.proposal.contains(";"), "инъекция команд запрещена");
            assert!(!i.proposal.contains("&&"), "конкатенация команд запрещена");
        }
    }

    /// Dynamic Vocab Expansion: неизвестное слово промпта входит в
    /// активную лексику и звучит в речи.
    #[test]
    fn speak_learns_unknown_words_on_the_fly() {
        let word = "квантизаторнейронность"; // заведомо вне словаря корпуса
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(31, 0.8),
            5,
        );
        assert!(core.crystal.id_of(word).is_none(), "слово вне словаря");
        let v0 = core.crystal.vocab();
        let _ = core.speak(&format!("живой {word}"), 16);
        assert!(core.crystal.id_of(word).is_some(), "слово выучено на лету");
        assert_eq!(core.crystal.vocab(), v0 + 1);
    }

    /// Subword-фолбек при выключенном динамическом словаре: склейка
    /// словарных слов резолвится через последний кусок.
    #[test]
    fn subword_fallback_when_dynamic_disabled() {
        let mut cfg = TriuneConfig::default();
        cfg.dynamic_vocab = false;
        let mut core =
            TriuneCore::new(test_crystal(), cfg, FlyPulse::synthetic(41, 0.8), 6);
        let w1 = core.crystal.tokens[0].clone();
        let w2 = core.crystal.tokens[1].clone();
        let glued = format!("{w1}{w2}");
        let v0 = core.crystal.vocab();
        let _ = core.speak(&format!("и {glued}"), 12);
        assert_eq!(
            core.crystal.vocab(),
            v0,
            "при выключенном dynamic_vocab словарь не растёт"
        );
    }

    /// Нейронная популяция (v0.38.0): речь учится — Хеббовские сдвиги
    /// синапсов происходят прямо во время говорения.
    #[test]
    fn speech_updates_synapses_while_speaking() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(51, 0.8),
            17,
        );
        let u = core.speak("кристалл знаний живёт", 24);
        assert!(
            u.neurons.hebb_updates > 0,
            "нейронный контур должен обновлять синапсы во время речи"
        );
        assert!(u.neurons.active_k > 0, "популяция жива");
        assert!(u.neurons.energy >= 0.0);
        // Трейс несёт активацию нейрона каждого токена.
        assert!(u.trace.iter().all(|t| t.act >= 0.0));
    }

    /// Выключенная пластичность: кристалл побитово заморожен.
    #[test]
    fn no_learn_freezes_crystal_bits() {
        let mut cfg = TriuneConfig::default();
        cfg.neurons.learn = false;
        cfg.dynamic_vocab = false; // и словарь не растёт — чистый эксперимент
        let mut core =
            TriuneCore::new(test_crystal(), cfg, FlyPulse::synthetic(61, 0.8), 23);
        let before = core.crystal.to_bytes();
        let u = core.speak("замороженный кристалл молчит правду", 20);
        let after = core.crystal.to_bytes();
        assert_eq!(before, after, "learn=false: байты .t5c не меняются");
        assert_eq!(u.neurons.hebb_updates, 0);
    }

    /// Резонансная память (v0.38.0): нейроны промпта живут в
    /// популяции дольше, чем окно ctx_window = 2 токена. Контекст —
    /// в активациях, а не в буфере.
    #[test]
    fn population_remembers_prompt_beyond_ctx_window() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(3, 0.8),
            9,
        );
        let prompt_words = ["живой", "мозг", "говорит", "система"];
        let _ = core.speak("живой мозг говорит", 4); // 4 токена речи
        // ctx_window = 2 давно вытеснил бы промпт; популяция держит.
        let alive: Vec<&str> = prompt_words
            .iter()
            .copied()
            .filter(|w| {
                core.crystal
                    .id_of(w)
                    .map(|id| core.neurons.activation(id) > 0.009)
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            alive.len() >= 2,
            "нейроны промпта должны жить за пределами окна: живы {alive:?}"
        );
    }

    /// Кристалл после обучения речи сериализуется и переживает
    /// загрузку (накопленная пластичность не теряется).
    #[test]
    fn learned_crystal_survives_roundtrip() {
        let mut core = TriuneCore::new(
            test_crystal(),
            TriuneConfig::default(),
            FlyPulse::synthetic(71, 0.8),
            29,
        );
        let _ = core.speak("живой мозг учится говорить", 20);
        let bytes = core.crystal.to_bytes();
        let reloaded = Crystal::load(&bytes, core.cfg.dims).expect("перезагрузка выученного");
        assert_eq!(reloaded.vocab(), core.crystal.vocab());
        // Хеббовские сдвиги видны и после загрузки.
        let mut mismatches = 0;
        for p in 0..reloaded.vocab() as u32 {
            for n in 0..reloaded.vocab() as u32 {
                if reloaded.bigram_trit(p, n) != core.crystal.bigram_trit(p, n) {
                    mismatches += 1;
                }
            }
        }
        assert_eq!(mismatches, 0, "триты синапсов должны пережить round-trip");
    }
}

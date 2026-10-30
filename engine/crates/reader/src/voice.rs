//! LivingVoice — ядро живого голоса (порт цикла K + коартикуляция).
//!
//! На книгу создаётся ОДИН голос: семя фиксирует личность (формантный
//! джиттер ±1%, вихревые связи, маски щели), а рендер ведёт ψ непрерывно
//! сквозь все звуки. Формантные цели поступают из фонетического
//! пайплайна (phonemes) и сглаживаются экспоненциальным глайдом τ=40 мс.

use crate::linalg::{self, Vec6};
use crate::resonator::{self, Couplings, Tract, BLOCK};
use crate::rng::Xorshift64;
use crate::trit;

/// Частота дискретизации.
pub const FS: f64 = crate::FS as f64;

/// Постоянная времени глайдов формант (коартикуляция), секунды.
pub const GLIDE_TAU: f64 = 0.040;

/// Архетип гласного: цели формант + физиология.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Archetype {
    /// «а» спокойное: открытое горло.
    ACalm,
    /// «е/я» яркое: переднее поднятие языка.
    ABright,
    /// «и» тёмное: узкий передний тракт.
    IDark,
    /// «у/о» спокойное: лабиализованное округление.
    UCalm,
}

impl Archetype {
    /// (F1, F2, F3) Гц.
    pub fn formants(self) -> [f64; 3] {
        match self {
            Archetype::ACalm => [730.0, 1090.0, 2440.0],
            Archetype::ABright => [800.0, 1200.0, 2600.0],
            Archetype::IDark => [300.0, 2200.0, 2900.0],
            Archetype::UCalm => [330.0, 900.0, 2200.0],
        }
    }

    /// (BW1, BW2, BW3) Гц.
    pub fn bandwidths(self) -> [f64; 3] {
        match self {
            Archetype::ACalm => [90.0, 100.0, 130.0],
            Archetype::ABright => [80.0, 95.0, 125.0],
            Archetype::IDark => [70.0, 110.0, 140.0],
            Archetype::UCalm => [80.0, 95.0, 130.0],
        }
    }

    /// Базовый F0 (Гц).
    pub fn f0(self) -> f64 {
        match self {
            Archetype::ACalm => 120.0,
            Archetype::ABright => 135.0,
            Archetype::IDark => 105.0,
            Archetype::UCalm => 115.0,
        }
    }

    /// Частота вибрато (Гц).
    pub fn vibrato_hz(self) -> f64 {
        match self {
            Archetype::ACalm => 5.2,
            Archetype::ABright => 6.3,
            Archetype::IDark => 4.6,
            Archetype::UCalm => 5.0,
        }
    }

    /// Относительный джиттер периода.
    pub fn jitter(self) -> f64 {
        match self {
            Archetype::ACalm => 0.008,
            Archetype::ABright => 0.012,
            Archetype::IDark => 0.006,
            Archetype::UCalm => 0.007,
        }
    }

    /// Шиммер (дБ).
    pub fn shimmer_db(self) -> f64 {
        match self {
            Archetype::ACalm => 1.0,
            Archetype::ABright => 1.4,
            Archetype::IDark => 0.8,
            Archetype::UCalm => 0.9,
        }
    }

    /// Имя (для паспорта/CLI).
    pub fn name(self) -> &'static str {
        match self {
            Archetype::ACalm => "a_calm",
            Archetype::ABright => "a_bright",
            Archetype::IDark => "i_dark",
            Archetype::UCalm => "u_calm",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "a_calm" | "a" => Some(Archetype::ACalm),
            "a_bright" | "e" => Some(Archetype::ABright),
            "i_dark" | "i" => Some(Archetype::IDark),
            "u_calm" | "u" => Some(Archetype::UCalm),
            _ => None,
        }
    }

    pub const ALL: [Archetype; 4] = [
        Archetype::ACalm,
        Archetype::ABright,
        Archetype::IDark,
        Archetype::UCalm,
    ];
}

/// Живой голос: личность (семя) + непрерывный рендер.
pub struct LivingVoice {
    /// Семя личности диктора.
    pub seed: u64,
    /// Опорный архетип (калибровка громкости и F0 по нему).
    pub base: Archetype,
    /// Реальные форманты личности (±1% от архетипа, из семени).
    pub formant_jitter: [f64; 3],
    /// Вихревые связи (личность).
    pub couplings: Couplings,
    /// Состояние щели.
    slit: [i8; trit::SLIT_K],
    neg_mask: u8,
    swap_mask: u8,
    trit_pos: usize,
    last_trit: i8,
    /// Калибровочный множитель входа (gain staging).
    pub gain: f64,
    // ── динамическое состояние рендера ──
    psi: Vec6,
    /// Текущие сглаженные форманты (ради непрерывности).
    cur_formants: [f64; 3],
    cur_bandwidths: [f64; 3],
    /// Цели глайдов (задаются фонетическим пайплайном).
    glide_target_formants: [f64; 3],
    glide_target_bandwidths: [f64; 3],
    ad: linalg::Mat,
    bd: Vec6,
    block_left: usize,
    t: f64,
    in_period_t: f64,
    period: f64,
    open_quotient: f64,
    period_jitter: f64,
    period_shimmer: f64,
    f0_now: f64,
    /// Поток микротремора (период-к-периоду).
    tremor: Xorshift64,
    /// Счётчики для паспорта.
    pub n_periods: u64,
    jitter_factors: Vec<f64>,
    shimmer_factors: Vec<f64>,
    lyapunov_violations: u32,
    prev_energy_gap: Option<f64>,
}

impl LivingVoice {
    /// Создать голос из 64-битного семени.
    pub fn new(seed: u64, base: Archetype) -> Self {
        let mut rg = Xorshift64::new(seed ^ 0xA5A5_5A5A_DEAD_BEEF);
        // формантный джиттер ±1% (личность)
        let mut fj = [0.0; 3];
        for f in fj.iter_mut() {
            *f = 1.0 + 0.01 * (rg.unit_milli() - 0.5);
        }
        let couplings = resonator::couplings_from_seed(&mut rg);
        // щель из семени
        let slit = trit::balanced_trit_state(&mut rg);
        let neg_mask = (rg.next_u64() & 0xFF) as u8;
        let swap_mask = (rg.next_u64() & 0xFF) as u8;

        let mut voice = LivingVoice {
            seed,
            base,
            formant_jitter: fj,
            couplings,
            slit,
            neg_mask,
            swap_mask,
            trit_pos: 0,
            last_trit: 0,
            gain: 1.0,
            psi: [0.0; 6],
            cur_formants: [0.0; 3],
            cur_bandwidths: base.bandwidths(),
            glide_target_formants: [0.0; 3],
            glide_target_bandwidths: base.bandwidths(),
            ad: linalg::eye(),
            bd: [0.0; 6],
            block_left: 0,
            t: 0.0,
            in_period_t: 0.0,
            period: 1.0 / base.f0(),
            open_quotient: 0.5,
            period_jitter: 1.0,
            period_shimmer: 1.0,
            f0_now: base.f0(),
            tremor: Xorshift64::new(seed),
            n_periods: 0,
            jitter_factors: Vec::new(),
            shimmer_factors: Vec::new(),
            lyapunov_violations: 0,
            prev_energy_gap: None,
        };

        // Калибровка усиления входа (gain staging, как в цикле K):
        // измерить вынужденную амплитуду на F0 опорного архетипа и
        // нормировать до 0.45 — иначе стационар ~1e-3 против транзиента 1.0.
        let tract = voice.base_tract();
        let (ad, mut bd) = resonator::zoh(&tract, &voice.couplings, FS);
        let mut probe = [0.0; 6];
        let n_probe = (0.15 * FS) as usize;
        for i in 0..n_probe {
            let drive =
                (2.0 * std::f64::consts::PI * voice.base.f0() * i as f64 / FS).sin();
            let mut p = linalg::mat_vec(&ad, &probe);
            for k in 0..6 {
                p[k] += drive * bd[k];
            }
            probe = p;
        }
        let obs = resonator::observe(&probe).abs().max(1e-12);
        let k = 0.45 / obs;
        for b in bd.iter_mut() {
            *b *= k;
        }
        voice.gain = k;

        // стартовое состояние: первая формантная плоскость возбуждена
        voice.psi[0] = 1.0;
        voice.ad = ad;
        voice.bd = bd;
        voice.cur_formants = tract.formants;
        voice.cur_bandwidths = tract.bandwidths;
        voice.glide_target_formants = tract.formants;
        voice.glide_target_bandwidths = tract.bandwidths;
        voice
    }

    /// Опорный тракт личности (архетип × джиттер).
    pub fn base_tract(&self) -> Tract {
        let f = self.base.formants();
        Tract {
            formants: [
                f[0] * self.formant_jitter[0],
                f[1] * self.formant_jitter[1],
                f[2] * self.formant_jitter[2],
            ],
            bandwidths: self.base.bandwidths(),
        }
    }

    /// Задать ЦЕЛЬ формант (Гц) — фонетический пайплайн вызывает на
    /// границах звуков; фактический глайд — экспоненциальный (τ=40 мс).
    pub fn set_target(&mut self, formants: [f64; 3], bandwidths: [f64; 3]) {
        // цель сохраняется в cur_* и достигается глайдом в sample()
        // (коартикуляция: непрерывность ψ + плавность геометрии)
        self.glide_target_formants = formants;
        self.glide_target_bandwidths = bandwidths;
    }

    /// Один сэмпл голоса. `u_drive` — амплитуда фонации (0 = щель закрыта:
    /// пауза/глухой согласный), `f0_target` — контур высоты тона (Гц),
    /// `arch_params` — (vibrato_hz, jitter_rel, shimmer_db) текущего звука.
    /// Возвращает наблюдение y.
    pub fn sample(
        &mut self,
        u_drive: f64,
        f0_target: f64,
        vib_hz: f64,
        jitter_rel: f64,
        shimmer_db: f64,
    ) -> f64 {
        let dt = 1.0 / FS;

        // ── коартикуляция: экспоненциальный глайд формант ──
        let alpha = 1.0 - (-dt / GLIDE_TAU).exp();
        for k in 0..3 {
            self.cur_formants[k] +=
                alpha * (self.glide_target_formants[k] - self.cur_formants[k]);
            self.cur_bandwidths[k] +=
                alpha * (self.glide_target_bandwidths[k] - self.cur_bandwidths[k]);
        }

        // ── пересчёт ZOH блоками (внутри блока геометрия фиксирована) ──
        if self.block_left == 0 {
            let tract = Tract {
                formants: self.cur_formants,
                bandwidths: self.cur_bandwidths,
            };
            let (ad, bd) = resonator::zoh(&tract, &self.couplings, FS);
            self.ad = ad;
            self.bd = bd;
            self.block_left = BLOCK;
        }
        self.block_left -= 1;

        // ── периодный контроль (решения ПЕРИОД-до-ПЕРИОДУ) ──
        self.f0_now = f0_target;
        self.period = 1.0 / f0_target.max(20.0);
        if self.in_period_t >= self.period * self.period_jitter {
            self.in_period_t = 0.0;
            // (K1) No-Mul слой эволюционирует щель раз на период
            self.slit = trit::no_mul_trit_layer(&self.slit, self.neg_mask, self.swap_mask);
            self.trit_pos = (self.trit_pos + 1) % trit::SLIT_K;
            self.last_trit = self.slit[self.trit_pos];
            // (K2) открытая квота по триту
            self.open_quotient = trit::open_quotient(self.last_trit);
            // (K3) микротремор на один период
            self.period_jitter = 1.0 + jitter_rel * self.tremor.sym_milli();
            self.period_shimmer = 1.0 + (shimmer_db / 8.686) * self.tremor.sym_milli();
            self.n_periods += 1;
            self.jitter_factors.push(self.period_jitter);
            self.shimmer_factors.push(self.period_shimmer);
        }

        // (K4) вибрато — медленная ЧМ
        let vibrato = 0.004 * (2.0 * std::f64::consts::PI * vib_hz * self.t).sin();
        let f0_i = f0_target * (1.0 + vibrato) / self.period_jitter;

        // (K5) дифференцированный импульс Розенберга (zero-mean)
        let open_len = self.period * self.open_quotient;
        let tau = self.in_period_t / open_len.max(1e-9);
        let u = if u_drive > 0.0 && tau < 1.0 {
            let tp = 0.7;
            if tau < tp {
                self.period_shimmer
                    * u_drive
                    * (std::f64::consts::PI / tp)
                    * (std::f64::consts::PI * tau / tp).sin()
            } else {
                -self.period_shimmer
                    * u_drive
                    * (std::f64::consts::PI / (1.0 - tp))
                    * (std::f64::consts::PI * (tau - tp) / (1.0 - tp)).sin()
            }
        } else {
            0.0
        };

        // (K6) Ляпунов-контроль: между импульсами энергия не растёт
        if u == 0.0 {
            let e: f64 = self.psi.iter().map(|&v| v * v).sum();
            if let Some(pe) = self.prev_energy_gap {
                if e > pe + 1e-12 {
                    self.lyapunov_violations += 1;
                }
            }
            self.prev_energy_gap = Some(e);
        } else {
            self.prev_energy_gap = None;
        }

        // (K7) точный ZOH-шаг резонатора
        let mut p = linalg::mat_vec(&self.ad, &self.psi);
        let k = u * (f0_i / f0_target.max(20.0));
        for i in 0..6 {
            p[i] += k * self.bd[i];
        }
        self.psi = p;

        // (K8) наблюдение
        let y = resonator::observe(&self.psi);

        self.in_period_t += dt;
        self.t += dt;
        y
    }

    /// Паспорт рендера (статистика живости).
    pub fn passport(&self) -> RenderPassport {
        let jsd = stddev(&self.jitter_factors) * 100.0;
        let ssd = stddev(&self.shimmer_factors) * 100.0;
        RenderPassport {
            n_periods: self.n_periods,
            jitter_std_pct: jsd,
            shimmer_std_pct: ssd,
            lyapunov_violations: self.lyapunov_violations,
        }
    }

    /// Текущее состояние ψ (для Π_Λ-когерентности).
    pub fn psi(&self) -> &Vec6 {
        &self.psi
    }
}

/// Цели глайдов задаются методом set_target; фактический глайд —
/// экспоненциальный (τ=40 мс) внутри sample().
impl LivingVoice {}

fn stddev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|&x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Статистика живости одного рендера.
#[derive(Debug, Clone, Copy)]
pub struct RenderPassport {
    pub n_periods: u64,
    pub jitter_std_pct: f64,
    pub shimmer_std_pct: f64,
    pub lyapunov_violations: u32,
}

//! # Голосовые нейроны: поглощение живой речи (vocal tract biomimicry)
//!
//! Задание владельца (цикл Y+): «нам не текст интересен, нам ГОЛОС нужен —
//! обучение ради наращивания новых нейронов и синапсов, пока без звучания».
//! Полный отказ от текста/фонем/ASR: на входе — ТОЛЬКО сырой аудиопоток
//! (голоса, дыхание, связки, перепады давления), на выходе — граф нейронных
//! связей голосового тракта в тритном базисе GF(3).
//!
//! ```text
//!   сырой WAV (PCM16, живая речь)
//!        │  1. STFT 1024/512 (Ханн) — свой FFT движка
//!        ▼
//!   2. ПОЛОСНЫЕ НЕЙРОНЫ: посев = 24 геометрические полосы движка (Y);
//!      рост = ФОРМАНТНЫЙ СИННАПТОГЕНЕЗ: полоса с двумя доминантными
//!      пиками (prominence ≥ 3) и живой динамикой (≥ 4 дБ) расщепляется
//!      по межпиковой долине — рождаются ДВА refined-нейрона (F1/F2-трек)
//!        ▼
//!   3. СВЯЗКИ: F0 автокорреляцией (60–400 Гц) → фазовая траектория
//!      φ(t)=2π∫F0 dt, джиттер (σ ΔT0/T0), шиммер (ΔдБ), форманты F1/F2/F3
//!      пиками среднего PSD; фазовые гармоники полос — круговое среднее
//!      фаз бинов, развёрнутое во времени
//!        ▼
//!   4. СИНАПСЫ: STDP-граф лаг-1 корреляций z-нормированных лог-энергий
//!      выращенных нейронов → триты {−1,0,+1} + σ ошибки (GF(3))
//!        ▼
//!   5. SSN-ВИХРЬ (600 нейронов, золотые параметры V17): голос инъектируется
//!      как сенсорный паттерн каждый STFT-кадр (23 мс = 23 шага dt=1 мс);
//!      телеметрия: активность, КРИТИЧНОСТЬ (край хаоса C→1), синхронность
//!      (эпилепсия S→1), E/I, DA/5HT/NE. Базлайн — тот же вихрь без голоса
//!        ▼
//!   PQW-контейнер .poler: топология = дуги STDP-графа N×N, фазы = триты,
//!   гиперпараметры = fs/голос/уровень — тот же формат, что и Y «Эхо»
//! ```

use crate::game::asset::{self, WavData};
use crate::game::audio::fft_inplace;
use crate::ssn::vortex::{SynapticVortex, VortexConfig, VortexTelemetry};

/// Окно STFT — как в Y «Эхо» (движковый канон).
pub const STFT_N: usize = 1024;
/// Шаг STFT (50% перекрытие, COLA).
pub const STFT_HOP: usize = 512;

/// Конфигурация сессии роста голосовых нейронов.
#[derive(Debug, Clone)]
pub struct VocalConfig {
    /// Потолок наращивания (посев 24 + рождения).
    pub max_neurons: usize,
    /// Максимум рождений нейронов за один файл речи.
    pub births_per_file: usize,
    /// Доминантность пика (×полоса) для расщепления.
    pub peak_prominence: f64,
    /// Минимальная динамика полосы (σ лог-энергии, дБ).
    pub min_dyn_db: f64,
    /// Масштаб z-паттерна, инъектируемого в вихрь.
    pub inject_scale: f64,
    /// Seed вихря (личность слушающего мозга).
    pub vortex_seed: u64,
}

impl Default for VocalConfig {
    fn default() -> Self {
        VocalConfig {
            max_neurons: 168,
            births_per_file: 4,
            peak_prominence: 3.0,
            min_dyn_db: 4.0,
            inject_scale: 0.6,
            vortex_seed: 777,
        }
    }
}

/// Полосный нейрон голосового тракта (бин-диапазон half-спектра STFT).
#[derive(Debug, Clone, Copy)]
pub struct BandNeuron {
    pub lo: usize,
    pub hi: usize,
    /// Индекс файла, в котором нейрон родился (посев = 0-й «файл» = -1).
    pub born_file: usize,
    /// Глобальный кадр рождения.
    pub born_frame: usize,
    /// Посев движка (24 канонические полосы) — уже «зрелые».
    pub is_seed: bool,
}

/// Событие рождения нейрона (синаптогенез).
#[derive(Debug, Clone)]
pub struct BirthEvent {
    pub file: usize,
    pub frame: usize,
    pub lo_hz: f64,
    pub hi_hz: f64,
    pub reason: String,
}

/// Телеметрия вихря на кадр.
#[derive(Debug, Clone, Copy, Default)]
struct FrameTel {
    activity: f64,
    criticality: f64,
    synchrony: f64,
    ei: f64,
    da: f64,
    ne: f64,
    ht: f64,
}

impl From<VortexTelemetry> for FrameTel {
    fn from(t: VortexTelemetry) -> Self {
        FrameTel {
            activity: t.activity,
            criticality: t.criticality,
            synchrony: t.synchrony,
            ei: t.ei,
            da: t.da,
            ne: t.ne,
            ht: t.ht,
        }
    }
}

/// Нативный baseline движка (Y «Эхо», 24 полосы) по одному файлу.
#[derive(Debug, Clone)]
pub struct EngineBaseline {
    pub src_bytes: u64,
    pub pqw_bytes: usize,
    pub compression_x: f64,
    pub synapses_576: usize,
    pub strongest_hz: (f64, f64, f32),
    pub dominant_band_hz: f64,
}

/// Отчёт по одному проглоченному файлу живой речи.
#[derive(Debug, Clone)]
pub struct FileReport {
    pub file: usize,
    pub dur_s: f64,
    pub frames: usize,
    pub births: usize,
    pub neurons_after: usize,
    pub f0_mean_hz: f64,
    pub f0_min_hz: f64,
    pub f0_max_hz: f64,
    pub voiced_pct: f64,
    pub jitter_pct: f64,
    pub shimmer_db: f64,
    pub level_db: f64,
    pub engine: EngineBaseline,
}

/// Итоговый отчёт сессии.
#[derive(Debug, Clone)]
pub struct VocalReport {
    pub files: usize,
    pub frames: usize,
    pub src_bytes: u64,
    pub neurons_final: usize,
    pub births_total: usize,
    pub neurons: Vec<(f64, f64, usize, usize)>, // lo_hz, hi_hz, born_file, born_frame
    pub stdp_arcs: usize,
    pub stdp_self_loops: usize,
    pub stdp_strong: usize,
    pub stdp_strong_cross: usize,
    pub stdp_density: f64,
    pub stdp_strong_density: f64,
    pub stdp_trits: (usize, usize, usize), // (−1, 0, +1)
    pub stdp_sigma: f64,
    pub stdp_top: Vec<(f64, f64, f32)>,    // (от Гц, к Гц, вес)
    pub stdp_hubs: Vec<(f64, usize, usize)>, // (Гц, out-deg, in-deg)
    pub f0_mean_hz: f64,
    pub jitter_pct: f64,
    pub shimmer_db: f64,
    pub formants_hz: (f64, f64, f64),
    pub band_phase_vel_mean_radps: f64,
    pub vortex_steps: usize,
    pub vortex_voice: VortexStats,
    pub vortex_idle: VortexStats,
    pub vortex_verdict: String,
}

/// Агрегат телеметрии вихря.
#[derive(Debug, Clone, Copy, Default)]
pub struct VortexStats {
    pub activity: f64,
    pub criticality: f64,
    pub synchrony: f64,
    pub ei: f64,
    pub da: f64,
    pub ne: f64,
    pub ht: f64,
}

/// Сессия: нейроны растут от файла к файлу, один вихрь слушает всё.
pub struct VocalSession {
    pub cfg: VocalConfig,
    pub fs: u32,
    pub neurons: Vec<BandNeuron>,
    /// Лог-энергии (дБ) по нейронам × глобальные кадры.
    series: Vec<Vec<f32>>,
    /// Озвученность кадра (F0 валиден).
    voiced: Vec<bool>,
    f0: Vec<f32>,
    /// Накопленная фаза связок (рад, без разворота).
    phase: Vec<f32>,
    energy_db: Vec<f32>,
    file_of_frame: Vec<u32>,
    vortex: SynapticVortex,
    vortex_hist: Vec<FrameTel>,
    /// Финальная STDP-матрица N×N (лаг-1) — для внешней визуализации.
    final_graph: Vec<f32>,
    psd_sum: Vec<f64>,
    psd_frames: usize,
    pub births: Vec<BirthEvent>,
    pub files: usize,
    pub frames: usize,
    pub src_bytes: u64,
}

fn bin_hz(bin: usize, fs: u32) -> f64 {
    bin as f64 * fs as f64 / STFT_N as f64
}

/// Публичный Гц-калькулятор бина (для shell-отчётов).
pub fn bin_hz_pub(bin: usize, fs: u32) -> f64 {
    bin_hz(bin, fs)
}

/// Гармоника периодограммы: FFT степени 2 с паддингом нулями.
fn spectrum_pow2(x: &[f64]) -> Vec<f64> {
    let mut n = 16usize;
    while n < x.len() {
        n *= 2;
    }
    let mut re = vec![0.0f64; n];
    for (i, v) in x.iter().enumerate() {
        re[i] = *v;
    }
    let mut im = vec![0.0f64; n];
    fft_inplace(&mut re, &mut im);
    (0..n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

impl VocalSession {
    /// Новая сессия: посев = 24 геометрические полосы движка (Y «Эхо»).
    pub fn new(fs: u32, cfg: VocalConfig) -> Self {
        let seeds: Vec<BandNeuron> = asset::band_ranges(fs)
            .iter()
            .flatten()
            .map(|&(lo, hi)| BandNeuron { lo, hi, born_file: 0, born_frame: 0, is_seed: true })
            .collect();
        let n0 = seeds.len();
        let vortex = SynapticVortex::new(VortexConfig::default(), cfg.vortex_seed);
        VocalSession {
            cfg,
            fs,
            neurons: seeds,
            series: vec![Vec::new(); n0],
            voiced: Vec::new(),
            f0: Vec::new(),
            phase: Vec::new(),
            energy_db: Vec::new(),
            file_of_frame: Vec::new(),
            vortex,
            vortex_hist: Vec::new(),
            final_graph: Vec::new(),
            psd_sum: vec![0.0; STFT_N / 2 + 1],
            psd_frames: 0,
            births: Vec::new(),
            files: 0,
            frames: 0,
            src_bytes: 0,
        }
    }

    pub fn neurons(&self) -> &[BandNeuron] {
        &self.neurons
    }

    /// Финальная STDP-матрица N×N (лаг-1 корреляции) — топология связей.
    pub fn graph_matrix(&self) -> &[f32] {
        &self.final_graph
    }

    /// Проглотить один файл живой речи: STFT → рост → серии → вихрь.
    pub fn ingest_file(&mut self, wav: &WavData, src_bytes: u64) -> FileReport {
        let mono = wav.mono();
        if mono.len() < STFT_N {
            // дегенерат: короче одного окна STFT — нечему учиться
            self.files += 1;
            self.src_bytes += src_bytes;
            return FileReport {
                file: self.files - 1,
                dur_s: wav.duration_s(),
                frames: 0,
                births: 0,
                neurons_after: self.neurons.len(),
                f0_mean_hz: 0.0,
                f0_min_hz: 0.0,
                f0_max_hz: 0.0,
                voiced_pct: 0.0,
                jitter_pct: 0.0,
                shimmer_db: 0.0,
                level_db: -120.0,
                engine: EngineBaseline {
                    src_bytes,
                    pqw_bytes: 0,
                    compression_x: 0.0,
                    synapses_576: 0,
                    strongest_hz: (0.0, 0.0, 0.0),
                    dominant_band_hz: 0.0,
                },
            };
        }
        let frames = 1 + (mono.len() - STFT_N) / STFT_HOP;
        let n_bins = STFT_N / 2 + 1;
        let mut hann = vec![0.0f64; STFT_N];
        for (n, w) in hann.iter_mut().enumerate() {
            *w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * n as f64 / STFT_N as f64).cos());
        }
        // --- проход 1: матрица |X[k]|² и фаз по кадрам ---
        let mut pow_mat = vec![vec![0.0f32; n_bins]; frames];
        let mut frame_rms = vec![0.0f64; frames];
        for fi in 0..frames {
            let base = fi * STFT_HOP;
            let mut re = vec![0.0f64; STFT_N];
            let mut im = vec![0.0f64; STFT_N];
            let mut acc = 0.0;
            for n in 0..STFT_N {
                let s = mono[base + n] * hann[n];
                re[n] = s;
                acc += s * s;
            }
            frame_rms[fi] = (acc / STFT_N as f64).sqrt();
            fft_inplace(&mut re, &mut im);
            for k in 0..n_bins {
                let p = re[k] * re[k] + im[k] * im[k];
                pow_mat[fi][k] = p as f32;
                self.psd_sum[k] += p;
            }
        }
        self.psd_frames += frames;
        // --- F0 связок: нормированная автокорреляция кадра ---
        let lag_lo = ((self.fs as f64 / 400.0).ceil() as usize).max(2);
        let lag_hi = ((self.fs as f64 / 60.0).floor() as usize).min(STFT_N / 2 - 1);
        let mut f0_file = vec![0.0f64; frames];
        let mut voiced_file = vec![false; frames];
        for fi in 0..frames {
            if frame_rms[fi] < 0.004 {
                continue;
            }
            let base = fi * STFT_HOP;
            let seg = &mono[base..base + STFT_N];
            let e0: f64 = seg.iter().map(|s| s * s).sum();
            if e0 < 1e-12 {
                continue;
            }
            let mut best_lag = 0usize;
            let mut best_r = 0.0f64;
            for lag in lag_lo..=lag_hi {
                let r: f64 = (0..STFT_N - lag).map(|i| seg[i] * seg[i + lag]).sum::<f64>() / e0;
                if r > best_r {
                    best_r = r;
                    best_lag = lag;
                }
            }
            if best_r >= 0.30 && best_lag > 0 {
                // параболическая интерполяция пика автокорреляции
                let seg2 = &mono[base..base + STFT_N];
                let ra = if best_lag > 1 {
                    (0..STFT_N - (best_lag - 1)).map(|i| seg2[i] * seg2[i + best_lag - 1]).sum::<f64>() / e0
                } else {
                    best_r
                };
                let rb = (0..STFT_N - (best_lag + 1)).map(|i| seg2[i] * seg2[i + best_lag + 1]).sum::<f64>() / e0;
                let denom = ra - 2.0 * best_r + rb;
                let shift = if denom.abs() > 1e-12 {
                    (0.5 * (ra - rb) / denom).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                let lag_f = best_lag as f64 + shift;
                f0_file[fi] = self.fs as f64 / lag_f.max(1.0);
                voiced_file[fi] = true;
            }
        }
        // --- рост: формантный синаптогенез ---
        let births_before = self.births.len();
        let file_psd: Vec<f64> = {
            let mut p = vec![0.0f64; n_bins];
            for fi in 0..frames {
                for k in 0..n_bins {
                    p[k] += pow_mat[fi][k] as f64;
                }
            }
            for v in p.iter_mut() {
                *v /= frames.max(1) as f64;
            }
            p
        };
        let mut born = 0usize;
        while self.neurons.len() < self.cfg.max_neurons && born < self.cfg.births_per_file {
            match self.best_split(&file_psd, &pow_mat, frames) {
                Some((idx, split_bin, score, lo_hz, hi_hz)) => {
                    self.apply_split(idx, split_bin);
                    self.births.push(BirthEvent {
                        file: self.files,
                        frame: self.frames,
                        lo_hz,
                        hi_hz,
                        reason: format!("формантный пик ×{score:.1}"),
                    });
                    born += 1;
                }
                None => break,
            }
        }
        // --- серии полосных энергий (после роста) ---
        let n = self.neurons.len();
        let mut band_e = vec![vec![0.0f32; n]; frames];
        for fi in 0..frames {
            for (ni, bn) in self.neurons.iter().enumerate() {
                let mut acc = 0.0f32;
                for k in bn.lo..bn.hi {
                    acc += pow_mat[fi][k];
                }
                let m = (bn.hi - bn.lo).max(1) as f32;
                band_e[fi][ni] = (10.0 * (acc / m + 1e-20).log10()).max(-120.0);
            }
        }
        // --- вихрь: голос как сенсорный паттерн, 23 шага на кадр ---
        let steps_per_frame =
            ((STFT_HOP as f64 / self.fs as f64 / 0.001).round() as usize).max(1);
        let mut tel_file: Vec<FrameTel> = Vec::with_capacity(frames);
        // z-нормализация по файлу (стимул относительно себя)
        let mut mu = vec![0.0f64; n];
        let mut sd = vec![1e-6f64; n];
        for ni in 0..n {
            let m: f64 = (0..frames).map(|fi| band_e[fi][ni] as f64).sum::<f64>() / frames.max(1) as f64;
            let v: f64 = (0..frames).map(|fi| (band_e[fi][ni] as f64 - m).powi(2)).sum::<f64>()
                / frames.max(1) as f64;
            mu[ni] = m;
            sd[ni] = v.sqrt().max(1e-6);
        }
        for fi in 0..frames {
            let mut pattern = vec![0.0f64; self.vortex.config().n];
            for (i, p) in pattern.iter_mut().enumerate() {
                let ni = i % n;
                *p = ((band_e[fi][ni] as f64 - mu[ni]) / sd[ni]) * self.cfg.inject_scale;
            }
            self.vortex.inject(&pattern);
            for _ in 0..steps_per_frame {
                self.vortex.step();
            }
            tel_file.push(self.vortex.telemetry().into());
        }
        // --- накопление сессионных серий ---
        for fi in 0..frames {
            for ni in 0..n {
                self.series[ni].push(band_e[fi][ni]);
            }
            let e_db = (10.0 * (frame_rms[fi].powi(2) + 1e-20).log10()).max(-120.0);
            self.energy_db.push(e_db as f32);
            self.voiced.push(voiced_file[fi]);
            self.f0.push(f0_file[fi] as f32);
            self.file_of_frame.push(self.files as u32);
        }
        // фаза связок: φ += 2π·F0·dt
        let dt_frame = STFT_HOP as f64 / self.fs as f64;
        let mut last_phase = *self.phase.last().unwrap_or(&0.0) as f64;
        for fi in 0..frames {
            if voiced_file[fi] {
                last_phase += 2.0 * std::f64::consts::PI * f0_file[fi] * dt_frame;
            }
            self.phase.push(last_phase as f32);
        }
        self.vortex_hist.extend(tel_file.iter().copied());
        // --- статистики файла ---
        let vf: Vec<f64> = (0..frames).filter(|&fi| voiced_file[fi]).map(|fi| f0_file[fi]).collect();
        let f0_mean = vf.iter().sum::<f64>() / vf.len().max(1) as f64;
        let f0_min = vf.iter().cloned().fold(f64::INFINITY, f64::min);
        let f0_max = vf.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let voiced_pct = vf.len() as f64 / frames.max(1) as f64 * 100.0;
        // джиттер: среднее |ΔT0|/T0 по соседним озвученным кадрам
        let mut jit_acc = 0.0;
        let mut jit_n = 0usize;
        for fi in 1..frames {
            if voiced_file[fi] && voiced_file[fi - 1] && f0_file[fi] > 0.0 && f0_file[fi - 1] > 0.0 {
                let t0a = 1.0 / f0_file[fi - 1];
                let t0b = 1.0 / f0_file[fi];
                jit_acc += (t0b - t0a).abs() / ((t0a + t0b) / 2.0);
                jit_n += 1;
            }
        }
        let jitter_pct = if jit_n > 0 { jit_acc / jit_n as f64 * 100.0 } else { 0.0 };
        // шиммер: среднее |ΔдБ| по озвученным
        let mut shim_acc = 0.0;
        let mut shim_n = 0usize;
        for fi in 1..frames {
            if voiced_file[fi] && voiced_file[fi - 1] {
                shim_acc += (self.energy_db[self.frames + fi] - self.energy_db[self.frames + fi - 1]).abs() as f64;
                shim_n += 1;
            }
        }
        let shimmer_db = if shim_n > 0 { shim_acc / shim_n as f64 } else { 0.0 };
        let rms_all: f64 = frame_rms.iter().map(|r| r * r).sum::<f64>() / frames.max(1) as f64;
        let level_db = 20.0 * (rms_all.sqrt() + 1e-9).log10();
        // нативный baseline движка (Y «Эхо» 24 полосы)
        let ranges = asset::band_ranges(self.fs);
        let hz = |b: usize| -> Option<std::ops::Range<f64>> {
            ranges
                .get(b)
                .and_then(|x| *x)
                .map(|(lo, hi)| bin_hz(lo, self.fs)..bin_hz(hi, self.fs))
        };
        let zero = EngineBaseline {
            src_bytes,
            pqw_bytes: 0,
            compression_x: 0.0,
            synapses_576: 0,
            strongest_hz: (0.0, 0.0, 0.0),
            dominant_band_hz: 0.0,
        };
        let engine = match asset::absorb_audio(wav) {
            Ok(e) => match asset::audio_to_pqw(&e) {
                Ok(bytes) => {
                    let arcs = e.graph.iter().filter(|g| g.abs() >= 0.04).count();
                    let (si, sj, sv) = e.strongest_synapse();
                    let dom = hz(
                        e.mean_db
                            .iter()
                            .enumerate()
                            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(i, _)| i)
                            .unwrap_or(0),
                    );
                    EngineBaseline {
                        src_bytes,
                        pqw_bytes: bytes.len(),
                        compression_x: src_bytes as f64 / bytes.len().max(1) as f64,
                        synapses_576: arcs,
                        strongest_hz: (
                            hz(si).map(|r| r.start).unwrap_or(0.0),
                            hz(sj).map(|r| r.start).unwrap_or(0.0),
                            sv,
                        ),
                        dominant_band_hz: dom.map(|r| (r.start + r.end) / 2.0).unwrap_or(0.0),
                    }
                }
                Err(_) => zero,
            },
            Err(_) => zero,
        };
        let report = FileReport {
            file: self.files,
            dur_s: wav.duration_s(),
            frames,
            births: self.births.len() - births_before,
            neurons_after: self.neurons.len(),
            f0_mean_hz: if vf.is_empty() { 0.0 } else { f0_mean },
            f0_min_hz: if vf.is_empty() { 0.0 } else { f0_min },
            f0_max_hz: if vf.is_empty() { 0.0 } else { f0_max },
            voiced_pct,
            jitter_pct,
            shimmer_db,
            level_db,
            engine,
        };
        self.files += 1;
        self.frames += frames;
        self.src_bytes += src_bytes;
        report
    }

    /// Лучший кандидат на расщепление: два доминантных пика в полосе.
    fn best_split(
        &self,
        file_psd: &[f64],
        pow_mat: &[Vec<f32>],
        frames: usize,
    ) -> Option<(usize, usize, f64, f64, f64)> {
        let mut best: Option<(usize, usize, f64, f64, f64)> = None;
        for (idx, bn) in self.neurons.iter().enumerate() {
            let w = bn.hi - bn.lo;
            if w < 5 || self.neurons.len() >= self.cfg.max_neurons {
                continue;
            }
            // созревание: новорождённый нейрон зреет 2 файла, прежде чем делиться
            if !bn.is_seed && self.files < bn.born_file + 2 {
                continue;
            }
            let band_ps: Vec<f64> = file_psd[bn.lo..bn.hi].to_vec();
            let mean_p: f64 = band_ps.iter().sum::<f64>() / w as f64;
            if mean_p <= 1e-20 {
                continue;
            }
            // динамика полосы по кадрам (σ лог-энергии, дБ)
            let mut es: Vec<f64> = Vec::with_capacity(frames);
            for fi in 0..frames {
                let mut acc = 0.0f32;
                for k in bn.lo..bn.hi {
                    acc += pow_mat[fi][k];
                }
                es.push(((10.0 * (acc / w as f32 + 1e-20).log10()).max(-120.0)) as f64);
            }
            let me = es.iter().sum::<f64>() / frames.max(1) as f64;
            let dyn_db = (es.iter().map(|e| (e - me) * (e - me)).sum::<f64>() / frames.max(1) as f64).sqrt();
            if dyn_db < self.cfg.min_dyn_db {
                continue;
            }
            // два пика с доминантностью ≥ prominence, разведённые ≥ 2 бина
            let mut p1 = 0usize;
            let mut v1 = f64::NEG_INFINITY;
            for (i, &p) in band_ps.iter().enumerate() {
                if p > v1 {
                    v1 = p;
                    p1 = i;
                }
            }
            let prom1 = v1 / mean_p;
            if prom1 < self.cfg.peak_prominence {
                continue;
            }
            let mut p2_opt: Option<(usize, f64)> = None;
            for (i, &p) in band_ps.iter().enumerate() {
                if (i as i64 - p1 as i64).abs() < 2 {
                    continue;
                }
                if let Some((_, v2)) = p2_opt {
                    if p <= v2 {
                        continue;
                    }
                }
                p2_opt = Some((i, p));
            }
            let (p2, v2) = p2_opt?;
            let prom2 = v2 / mean_p;
            if prom2 < self.cfg.peak_prominence {
                continue;
            }
            // долина между пикaми — точка расщепления
            let (a, b) = if p1 < p2 { (p1, p2) } else { (p2, p1) };
            let mut valley = a;
            let mut vv = f64::INFINITY;
            for i in a..=b {
                if band_ps[i] < vv {
                    vv = band_ps[i];
                    valley = i;
                }
            }
            let split_abs = bn.lo + valley;
            if split_abs - bn.lo < 2 || bn.hi - split_abs < 3 {
                continue;
            }
            let lo_hz = bin_hz(bn.lo, self.fs);
            let hi_hz = bin_hz(bn.hi, self.fs);
            let score = prom1.min(prom2);
            if best.map(|b| b.2) < Some(score) {
                best = Some((idx, split_abs, score, lo_hz, hi_hz));
            }
        }
        best
    }

    /// Расщепить нейрон idx на двух детей (история наследуется от родителя).
    fn apply_split(&mut self, idx: usize, split_bin: usize) {
        let parent = self.neurons[idx];
        let hist = std::mem::take(&mut self.series[idx]);
        let c1 = BandNeuron { lo: parent.lo, hi: split_bin + 1, born_file: self.files, born_frame: self.frames, is_seed: false };
        let c2 = BandNeuron { lo: split_bin + 1, hi: parent.hi, born_file: self.files, born_frame: self.frames, is_seed: false };
        self.neurons.remove(idx);
        self.series.remove(idx);
        self.neurons.push(c1);
        self.neurons.push(c2);
        self.series.push(hist.clone());
        self.series.push(hist);
    }

    /// Финализация: STDP-триты, фазовые метрики, вихрь-базлайн, PQW-граф.
    pub fn finalize(&mut self) -> VocalReport {
        let n = self.neurons.len();
        let t = self.frames;
        // --- STDP: лаг-1 корреляции z-серий по всей сессии ---
        let mut mu = vec![0.0f64; n];
        let mut sd = vec![1e-6f64; n];
        for ni in 0..n {
            let m: f64 = self.series[ni].iter().map(|&v| v as f64).sum::<f64>() / t.max(1) as f64;
            let v: f64 = self.series[ni].iter().map(|&x| (x as f64 - m).powi(2)).sum::<f64>() / t.max(1) as f64;
            mu[ni] = m;
            sd[ni] = v.sqrt().max(1e-6);
        }
        let z = |ni: usize, ti: usize| -> f64 { (self.series[ni][ti] as f64 - mu[ni]) / sd[ni] };
        let mut graph = vec![0.0f32; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut acc = 0.0;
                for ti in 0..t.saturating_sub(1) {
                    acc += z(i, ti) * z(j, ti + 1);
                }
                graph[i * n + j] = (acc / t.saturating_sub(1).max(1) as f64).clamp(-1.0, 1.0) as f32;
            }
        }
        self.final_graph = graph.clone();
        let mut arcs = 0usize;
        let mut self_loops = 0usize;
        let mut strong = 0usize;
        let mut strong_cross = 0usize;
        let mut trits = (0usize, 0usize, 0usize);
        let mut sigma_acc = 0.0;
        let mut sigma_n = 0usize;
        let mut top: Vec<(f64, f64, f32)> = Vec::new();
        let mut out_deg = vec![0usize; n];
        let mut in_deg = vec![0usize; n];
        for i in 0..n {
            for j in 0..n {
                let g = graph[i * n + j];
                if g.abs() >= 0.04 {
                    arcs += 1;
                    if i == j {
                        self_loops += 1;
                    }
                }
                // сильные голосовые связи: |G| ≥ 0.3 — триты GF(3) считаются по ним
                if g.abs() >= 0.30 {
                    strong += 1;
                    let trit = if g > 0.25 { 1 } else if g < -0.25 { -1 } else { 0 };
                    match trit {
                        1 => trits.2 += 1,
                        -1 => trits.0 += 1,
                        _ => trits.1 += 1,
                    }
                    let target = trit as f64 * 0.5;
                    sigma_acc += (g as f64 - target).powi(2);
                    sigma_n += 1;
                    if i != j {
                        strong_cross += 1;
                        out_deg[i] += 1;
                        in_deg[j] += 1;
                        let hz = |k: usize| bin_hz(self.neurons[k].lo, self.fs);
                        top.push((hz(i), hz(j), g));
                    }
                }
            }
        }
        // если кросс-связей нет — показываем автопетли (честность отчёта)
        if top.is_empty() {
            for i in 0..n {
                let g = graph[i * n + i];
                if g.abs() >= 0.30 {
                    top.push((bin_hz(self.neurons[i].lo, self.fs), bin_hz(self.neurons[i].lo, self.fs), g));
                }
            }
        }
        top.sort_by(|a, b| b.2.abs().partial_cmp(&a.2.abs()).unwrap_or(std::cmp::Ordering::Equal));
        top.truncate(10);
        // хабы — топ-степени сильных кросс-дуг
        let mut hubs: Vec<(f64, usize, usize)> = (0..n)
            .map(|i| (bin_hz(self.neurons[i].lo, self.fs), out_deg[i], in_deg[i]))
            .collect();
        hubs.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
        hubs.truncate(5);
        let stdp_sigma = (sigma_acc / sigma_n.max(1) as f64).sqrt();
        let density = arcs as f64 / (n * n).max(1) as f64;
        let strong_density = strong_cross as f64 / (n * (n - 1)).max(1) as f64;
        // --- F0/фазовые метрики сессии ---
        let vf: Vec<f64> = (0..t).filter(|&ti| self.voiced[ti]).map(|ti| self.f0[ti] as f64).collect();
        let f0_mean = vf.iter().sum::<f64>() / vf.len().max(1) as f64;
        let mut jit_acc = 0.0;
        let mut jit_n = 0usize;
        for ti in 1..t {
            if self.voiced[ti] && self.voiced[ti - 1] && self.f0[ti] > 0.0 && self.f0[ti - 1] > 0.0 {
                let a = 1.0 / self.f0[ti - 1] as f64;
                let b = 1.0 / self.f0[ti] as f64;
                jit_acc += (b - a).abs() / ((a + b) / 2.0);
                jit_n += 1;
            }
        }
        let jitter_pct = if jit_n > 0 { jit_acc / jit_n as f64 * 100.0 } else { 0.0 };
        let mut shim_acc = 0.0;
        let mut shim_n = 0usize;
        for ti in 1..t {
            if self.voiced[ti] && self.voiced[ti - 1] {
                shim_acc += (self.energy_db[ti] - self.energy_db[ti - 1]).abs() as f64;
                shim_n += 1;
            }
        }
        let shimmer_db = if shim_n > 0 { shim_acc / shim_n as f64 } else { 0.0 };
        // --- форманты по среднему PSD сессии ---
        let psd_mean: Vec<f64> = self.psd_sum.iter().map(|&p| p / self.psd_frames.max(1) as f64).collect();
        let pick = |lo: f64, hi: f64| -> f64 {
            let k_lo = ((lo / (self.fs as f64 / STFT_N as f64)).ceil() as usize).max(1);
            let k_hi = ((hi / (self.fs as f64 / STFT_N as f64)).floor() as usize).min(psd_mean.len() - 1);
            if k_hi <= k_lo + 1 {
                return 0.0;
            }
            let mut best_k = k_lo;
            let mut best_v = f64::NEG_INFINITY;
            for k in k_lo..=k_hi {
                let v = 10.0 * (psd_mean[k] + 1e-20).log10();
                if v > best_v {
                    best_v = v;
                    best_k = k;
                }
            }
            bin_hz(best_k, self.fs)
        };
        let formants = (pick(180.0, 900.0), pick(700.0, 2800.0), pick(1600.0, 3800.0));
        // --- фазовая скорость полос (гармоники) — по трём верхним нейронам ---
        let mut phase_vel_acc = 0.0;
        let mut phase_vel_n = 0usize;
        let energy_rank: Vec<usize> = {
            let mut idx: Vec<usize> = (0..n).collect();
            idx.sort_by(|&a, &b| mu[b].partial_cmp(&mu[a]).unwrap_or(std::cmp::Ordering::Equal));
            idx
        };
        for &ni in energy_rank.iter().take(6) {
            let mut prev = 0.0f32;
            for ti in 0..t {
                // скорость фазы связок — только озвученные переходы
                let cur = self.phase[ti];
                if ti > 0 && self.voiced[ti] && self.voiced[ti - 1] {
                    let d = (cur - prev).abs() as f64;
                    phase_vel_acc += d / (STFT_HOP as f64 / self.fs as f64);
                    phase_vel_n += 1;
                }
                prev = cur;
            }
        }
        let band_phase_vel_mean = phase_vel_acc / phase_vel_n.max(1) as f64;
        // --- вихрь: базлайн без голоса (те же шаги, спонтанный режим) ---
        let steps_per_frame =
            ((STFT_HOP as f64 / self.fs as f64 / 0.001).round() as usize).max(1);
        let total_steps = steps_per_frame * t;
        let mut idle = SynapticVortex::new(VortexConfig::default(), self.cfg.vortex_seed);
        let mut idle_tel: Vec<FrameTel> = Vec::with_capacity(t);
        for _ in 0..t {
            for _ in 0..steps_per_frame {
                idle.step();
            }
            idle_tel.push(idle.telemetry().into());
        }
        let agg = |v: &[FrameTel]| -> VortexStats {
            let m = |f: &dyn Fn(&FrameTel) -> f64| v.iter().map(f).sum::<f64>() / v.len().max(1) as f64;
            VortexStats {
                activity: m(&|x| x.activity),
                criticality: m(&|x| x.criticality),
                synchrony: m(&|x| x.synchrony),
                ei: m(&|x| x.ei),
                da: m(&|x| x.da),
                ne: m(&|x| x.ne),
                ht: m(&|x| x.ht),
            }
        };
        let voice_stats = agg(&self.vortex_hist);
        let idle_stats = agg(&idle_tel);
        let ok_voice = voice_stats.activity > 0.01
            && voice_stats.activity < 0.5
            && voice_stats.synchrony < 0.8
            && voice_stats.criticality > 0.3;
        let verdict = if ok_voice {
            format!(
                "вихрь УСТОЙЧИВ на краю хаоса под голосом: активность {:.1}% (цель 5%), критичность C={:.2}, синхронность S={:.2} (<0.8), E/I={:.1}; DA={:.2} 5HT={:.2} NE={:.2} — мозг слушает, не умирая и не вспыхивая",
                voice_stats.activity * 100.0,
                voice_stats.criticality,
                voice_stats.synchrony,
                voice_stats.ei,
                voice_stats.da,
                voice_stats.ht,
                voice_stats.ne
            )
        } else {
            format!(
                "вихрь НА ГРАНИ: активность {:.1}%, C={:.2}, S={:.2}, E/I={:.1} — требуется настройка inject_scale",
                voice_stats.activity * 100.0,
                voice_stats.criticality,
                voice_stats.synchrony,
                voice_stats.ei
            )
        };
        VocalReport {
            files: self.files,
            frames: t,
            src_bytes: self.src_bytes,
            neurons_final: n,
            births_total: self.births.len(),
            neurons: self
                .neurons
                .iter()
                .map(|b| (bin_hz(b.lo, self.fs), bin_hz(b.hi, self.fs), b.born_file, b.born_frame))
                .collect(),
            stdp_arcs: arcs,
            stdp_self_loops: self_loops,
            stdp_strong: strong,
            stdp_strong_cross: strong_cross,
            stdp_density: density,
            stdp_strong_density: strong_density,
            stdp_trits: trits,
            stdp_sigma,
            stdp_top: top,
            stdp_hubs: hubs,
            f0_mean_hz: f0_mean,
            jitter_pct,
            shimmer_db,
            formants_hz: formants,
            band_phase_vel_mean_radps: band_phase_vel_mean,
            vortex_steps: total_steps,
            vortex_voice: voice_stats,
            vortex_idle: idle_stats,
            vortex_verdict: verdict,
        }
    }

    /// Выращенный граф → PQW-контейнер (.poler), схема как у Y «Эхо».
    pub fn graph_to_pqw(&self) -> Result<Vec<u8>, String> {
        use pqw_core::PqwWriter;
        let n = self.neurons.len();
        let d_pol = (n * n + 5 * n) as u32;
        let t = self.frames;
        // модуляционный спектр по нейронам
        let frame_rate = self.fs as f64 / STFT_HOP as f64;
        let mut mod_depth = vec![0.0f32; n];
        let mut mod_hz = vec![0.2f32; n];
        let mut mean_db = vec![-60.0f32; n];
        let mut dyn_db = vec![4.0f32; n];
        for ni in 0..n {
            let m: f64 = self.series[ni].iter().map(|&v| v as f64).sum::<f64>() / t.max(1) as f64;
            let v: f64 = self.series[ni].iter().map(|&x| (x as f64 - m).powi(2)).sum::<f64>() / t.max(1) as f64;
            mean_db[ni] = m as f32;
            dyn_db[ni] = v.sqrt() as f32;
            let stride = (t / 4096).max(1);
            let series: Vec<f64> = (0..t).step_by(stride).map(|ti| self.series[ni][ti] as f64).collect();
            let rate = frame_rate / stride as f64;
            let spec = spectrum_pow2(&series);
            let mut total = 0.0;
            let mut in_band = 0.0;
            let mut peak_i = 0usize;
            let mut peak_v = 0.0;
            for (k, &p) in spec.iter().enumerate().take(spec.len() / 2).skip(1) {
                let f = k as f64 * rate / spec.len() as f64;
                total += p;
                if (0.05..=8.0).contains(&f) {
                    in_band += p;
                    if p > peak_v {
                        peak_v = p;
                        peak_i = k;
                    }
                }
            }
            if total > 1e-12 {
                mod_depth[ni] = (in_band / total).min(1.0) as f32;
                mod_hz[ni] = (peak_i as f64 * rate / spec.len() as f64) as f32;
            }
        }
        // STDP-граф (лаг-1) на финальных сериях
        let mut mu = vec![0.0f64; n];
        let mut sd = vec![1e-6f64; n];
        for ni in 0..n {
            let m: f64 = self.series[ni].iter().map(|&v| v as f64).sum::<f64>() / t.max(1) as f64;
            let v: f64 = self.series[ni].iter().map(|&x| (x as f64 - m).powi(2)).sum::<f64>() / t.max(1) as f64;
            mu[ni] = m;
            sd[ni] = v.sqrt().max(1e-6);
        }
        let mut w = PqwWriter::new(d_pol).map_err(|e| e.to_string())?.hyperparams(
            self.fs as f32,
            (self.voiced.iter().filter(|&&v| v).count() as f64 / t.max(1) as f64 * 2.0 - 1.0) as f32,
            ((mean_db.iter().sum::<f32>() / n.max(1) as f32 + 60.0) / 80.0 * 2.0 - 1.0).clamp(-1.0, 1.0),
            0.02,
        );
        for i in 0..n {
            for j in 0..n {
                let mut acc = 0.0;
                for ti in 0..t.saturating_sub(1) {
                    let zi = (self.series[i][ti] as f64 - mu[i]) / sd[i];
                    let zj = (self.series[j][ti + 1] as f64 - mu[j]) / sd[j];
                    acc += zi * zj;
                }
                let g = (acc / t.saturating_sub(1).max(1) as f64).clamp(-1.0, 1.0);
                if g.abs() >= 0.04 {
                    w.add_phase((i * n + j) as u32, g as f32).map_err(|e| e.to_string())?;
                }
            }
        }
        for b in 0..n {
            let base = (n * n + b * 5) as u32;
            w.add_phase(base, ((mean_db[b] + 60.0) / 80.0 * 2.0 - 1.0).clamp(-1.0, 1.0))
                .map_err(|e| e.to_string())?;
            w.add_phase(base + 1, (dyn_db[b] / 36.0 * 2.0 - 1.0).clamp(-1.0, 1.0))
                .map_err(|e| e.to_string())?;
            w.add_phase(base + 2, (mod_depth[b] * 2.0 - 1.0).clamp(-1.0, 1.0))
                .map_err(|e| e.to_string())?;
            let lo = 0.05f32;
            let hi = 8.0f32;
            let p = ((mod_hz[b].max(lo) / lo).ln() / (hi / lo).ln() * 2.0 - 1.0).clamp(-1.0, 1.0);
            w.add_phase(base + 3, p).map_err(|e| e.to_string())?;
            w.add_phase(base + 4, 0.0).map_err(|e| e.to_string())?;
        }
        w.to_bytes().map_err(|e| e.to_string())
    }

    /// Кадровые треки для внешней визуализации (TSV).
    pub fn traces(&self) -> Vec<Vec<f32>> {
        // [file, energy_db, voiced, f0, phase, activity, criticality, synchrony, ei, da, ne]
        let mut rows = Vec::with_capacity(self.frames);
        for ti in 0..self.frames {
            let v = self.vortex_hist.get(ti).copied().unwrap_or(FrameTel::default());
            rows.push(vec![
                self.file_of_frame.get(ti).copied().unwrap_or(0) as f32,
                self.energy_db[ti],
                if self.voiced[ti] { 1.0 } else { 0.0 },
                self.f0[ti],
                self.phase[ti],
                v.activity as f32,
                v.criticality as f32,
                v.synchrony as f32,
                v.ei as f32,
                v.da as f32,
                v.ne as f32,
            ]);
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Синтетическая «речь»: два формантных резонанса в одной полосе + AM.
    fn two_formants(fs: u32, dur_s: f64) -> WavData {
        let n = (fs as f64 * dur_s) as usize;
        let mut samples = Vec::with_capacity(n);
        let mut t = 0.0;
        let dt = 1.0 / fs as f64;
        for _ in 0..n {
            let am = 0.3 + 0.7 * (2.0 * std::f64::consts::PI * 4.0 * t).sin();
            let s = am
                * (0.5 * (2.0 * std::f64::consts::PI * 900.0 * t).sin()
                    + 0.5 * (2.0 * std::f64::consts::PI * 1050.0 * t).sin());
            samples.push(s as f32);
            t += dt;
        }
        WavData { fs, channels: 1, samples }
    }

    #[test]
    fn grows_neurons_from_two_formants() {
        let fs = 22050u32;
        let wav = two_formants(fs, 3.0);
        let cfg = VocalConfig {
            max_neurons: 64,
            births_per_file: 4,
            peak_prominence: 2.0,
            min_dyn_db: 1.5,
            ..VocalConfig::default()
        };
        let mut s = VocalSession::new(fs, cfg);
        let r = s.ingest_file(&wav, 132_300);
        assert!(s.neurons().len() > 24, "полосы обязаны расти: {}", s.neurons().len());
        assert!(r.births > 0, "рождения обязаны случиться");
        assert!(r.frames > 100);
        let rep = s.finalize();
        assert!(rep.stdp_arcs > 0);
        assert!(rep.neurons_final > 24);
        assert!(rep.jitter_pct >= 0.0 && rep.jitter_pct < 100.0);
    }

    #[test]
    fn vortex_survives_voice_injection() {
        let fs = 22050u32;
        let wav = two_formants(fs, 2.0);
        let mut s = VocalSession::new(fs, VocalConfig::default());
        s.ingest_file(&wav, 88_200);
        let rep = s.finalize();
        assert!(
            rep.vortex_voice.activity > 0.01 && rep.vortex_voice.activity < 0.5,
            "активность под голосом: {}",
            rep.vortex_voice.activity
        );
        assert!(rep.vortex_voice.synchrony < 0.8, "эпилепсия недопустима");
    }

    #[test]
    fn graph_pqw_container_writes() {
        let fs = 22050u32;
        let wav = two_formants(fs, 1.5);
        let mut s = VocalSession::new(fs, VocalConfig::default());
        s.ingest_file(&wav, 66_150);
        let bytes = s.graph_to_pqw().expect("PQW обязан писаться");
        assert!(!bytes.is_empty());
        assert!(bytes.len() < 100_000, "граф ≤ 100 КБ на демо: {}", bytes.len());
    }
}

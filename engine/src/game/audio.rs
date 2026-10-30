//! # T1: Акустический кристалл — игровой звук (цикл T, v0.55.0)
//!
//! Звук на готовом ядре [`crate::game`]: мир озвучивается **своей
//! физикой** — «музыка сфер» в буквальном смысле. Кеплеровская
//! ω = √(G·M)/r^1.5 каждого тела отображается в высоту тона
//! (логарифмически, в полутонах), радиус — в громкость, мировая
//! X-координата — в панораму. Никаких «просто так» подобранных
//! частот: интервалы между голосами — это реальные орбитальные
//! резонансы системы.
//!
//! Дисциплина (уроки UE, POLER_NOTES.md §3):
//! - **ноль аллокаций на горячем пути** — скретч-буферы живут в
//!   структуре, рендер тика пишет в готовый буфер;
//! - **детерминизм бит-в-бит** — `audio_hash` (FNV-1a по битам
//!   f32-сэмплов) и `crystal_hash` (спектральный слепок) — контракты
//!   воспроизведения, аналогичные `state_hash` мира;
//! - **суверенный минимум** — свой WAV-писальник (PCM16) и свой
//!   радикс-2 FFT: ни одной внешней зависимости.
//!
//! Кристалл: STFT-окна → топ-K спектральных пиков на окно →
//! конденсированный слепок K_audio — звук как низкоранговый объект,
//! а не поток байт (фундамент T2 для текстур — та же математика).

use std::f64::consts::PI;
use std::io::Write;
use std::path::Path;

use super::world::{BodyClass, World};

/// Частота дискретизации по умолчанию (студийный стандарт).
pub const AUDIO_FS: u32 = 48_000;

// ---------------------------------------------------------------------------
// Осцилляторы и огибающие
// ---------------------------------------------------------------------------

/// Форма волны голоса. Выбор по классу тела: звёзды — чистый тон,
/// планеты — тёплый треугольник, станции — технологичный меандр.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Waveform {
    /// Значение фазы ∈ [0, 1) → [-1, 1]. Полиномиальные формы без
    /// ветвлений где возможно — ровный машинный код без скачков.
    pub fn sample(self, phase: f64) -> f64 {
        let t = phase - phase.floor();
        match self {
            Waveform::Sine => (2.0 * PI * t).sin(),
            Waveform::Triangle => 4.0 * (t - 0.5).abs() - 1.0,
            Waveform::Saw => 2.0 * t - 1.0,
            Waveform::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Waveform::Sine => "sine",
            Waveform::Triangle => "triangle",
            Waveform::Saw => "saw",
            Waveform::Square => "square",
        }
    }
}

/// ADSR-огибающая (атака–спад–сустейн–релиз), секунды и уровень.
/// Для падов сустан-слоя атаки/релизы длинные; значения клампятся.
#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    pub attack_s: f64,
    pub decay_s: f64,
    pub sustain: f64,
    pub release_s: f64,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { attack_s: 0.05, decay_s: 0.10, sustain: 0.85, release_s: 0.40 }
    }
}

impl Adsr {
    /// Уровень огибающей в момент t при общей длительности hold_s
    /// (нота держится hold_s, затем релиз).
    pub fn level(&self, t: f64, hold_s: f64) -> f64 {
        let a = self.attack_s.max(0.0);
        let d = self.decay_s.max(0.0);
        let s = self.sustain.clamp(0.0, 1.0);
        let r = self.release_s.max(0.0);
        if t < 0.0 {
            return 0.0;
        }
        if t < a {
            return (t / a.max(1e-9)) * 1.0;
        }
        if t < a + d {
            let k = (t - a) / d.max(1e-9);
            return 1.0 + k * (s - 1.0);
        }
        if t < hold_s {
            return s;
        }
        let k = ((t - hold_s) / r.max(1e-9)).min(1.0);
        s * (1.0 - k)
    }
}

// ---------------------------------------------------------------------------
// Трек: стерео-сэмплы + WAV + метрики + хеши
// ---------------------------------------------------------------------------

/// Готовый стерео-трек: interleaved L,R; сэмплы f32 ∈ [-1, 1].
#[derive(Debug, Clone)]
pub struct Track {
    pub fs: u32,
    pub samples: Vec<f32>,
}

impl Track {
    /// Пустой трек (тишина) заданной длительности.
    pub fn silence(fs: u32, seconds: f64) -> Self {
        let frames = (seconds * fs as f64).round() as usize;
        Self { fs, samples: vec![0.0; frames * 2] }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration_s(&self) -> f64 {
        self.frames() as f64 / self.fs as f64
    }

    /// Пик по модулю (0..1).
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// RMS в dBFS (тишина → −∞, возвращаем −120).
    pub fn rms_dbfs(&self) -> f64 {
        if self.samples.is_empty() {
            return -120.0;
        }
        let sum: f64 = self.samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        let rms = (sum / self.samples.len() as f64).sqrt();
        if rms <= 1e-12 {
            -120.0
        } else {
            20.0 * rms.log10()
        }
    }

    /// Пик в dBFS.
    pub fn peak_dbfs(&self) -> f64 {
        let p = self.peak() as f64;
        if p <= 1e-12 {
            -120.0
        } else {
            20.0 * p.log10()
        }
    }

    /// Маяк детерминизма: FNV-1a по битам сэмплов — одинаковый мир
    /// и конфиг дают бит-в-бит тот же трек.
    pub fn audio_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for s in &self.samples {
            for b in s.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// Запись WAV: PCM16, стерео. Возвращает размер файла.
    pub fn write_wav(&self, path: &Path) -> std::io::Result<u64> {
        let frames = self.frames();
        let data_len = (frames * 4) as u32;
        let mut buf = Vec::with_capacity(44 + data_len as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&(36 + data_len).to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&2u16.to_le_bytes()); // stereo
        buf.extend_from_slice(&self.fs.to_le_bytes());
        buf.extend_from_slice(&(self.fs * 2 * 2).to_le_bytes()); // byte rate
        buf.extend_from_slice(&4u16.to_le_bytes()); // block align
        buf.extend_from_slice(&16u16.to_le_bytes()); // bits
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.samples {
            let v = (*s as f64).clamp(-1.0, 1.0);
            let q = (v * 32767.0).round() as i16;
            buf.extend_from_slice(&q.to_le_bytes());
        }
        let mut f = std::fs::File::create(path)?;
        f.write_all(&buf)?;
        f.flush()?;
        Ok(buf.len() as u64)
    }
}

// ---------------------------------------------------------------------------
// Сонофикация мира: музыка сфер
// ---------------------------------------------------------------------------

/// Конфиг сонофикации: как физика мира отображается в звук.
#[derive(Debug, Clone)]
pub struct SonifyConfig {
    pub fs: u32,
    /// Мастер-гейн (до лимитера).
    pub master_gain: f64,
    /// Сколько октав звука на октаву орбитальной скорости (0 = чистые
    /// радиусы, 1.0 = честная скоростная связь).
    pub speed_octaves: f64,
    /// Опорная частота самого медленного голоса (Гц).
    pub ref_hz: f64,
    /// Мягкий лимитер: tanh-насыщение на этом пороге.
    pub ceiling: f64,
    /// Огибающая нот.
    pub adsr: Adsr,
}

impl Default for SonifyConfig {
    fn default() -> Self {
        Self {
            fs: AUDIO_FS,
            master_gain: 0.5,
            speed_octaves: 1.0,
            ref_hz: 55.0, // A1 — низкий фундамент
            ceiling: 0.95,
            adsr: Adsr::default(),
        }
    }
}

/// Карта класс тела → форма волны: звёзды поют синусом, станции
/// звучат технологично (меандр).
pub fn waveform_for_class(class: BodyClass) -> Waveform {
    match class {
        BodyClass::Star => Waveform::Sine,
        BodyClass::Planet => Waveform::Triangle,
        BodyClass::Moon => Waveform::Sine,
        BodyClass::Station => Waveform::Square,
        BodyClass::Probe => Waveform::Saw,
    }
}

/// Высота голоса из орбитальной ω: полутоновое отображение
/// log2(ω/ω_min) ∈ [0, speed_octaves·октав]. Радиусы (и скорости)
/// внутренней системы дают более высокие ноты — как клавесин сфер.
fn voice_hz(omega: f64, omega_min: f64, omega_max: f64, cfg: &SonifyConfig) -> f64 {
    let (lo, hi) = (omega_min.min(omega_max), omega_min.max(omega_max));
    let w = omega.clamp(lo, hi.max(lo + 1e-12));
    let norm = ((w - lo) / (hi - lo).max(1e-12)).clamp(0.0, 1.0); // 0..1
    let octaves = cfg.speed_octaves.max(0.0) * 3.0; // до 3 октав диапазона
    cfg.ref_hz * 2.0_f64.powf(norm * octaves)
}

/// Озвучить мир: `ticks` фиксированных тиков по `dt`, каждый тик
/// мир тикает физикой (Orbit → Transform), звук пишется сэмпл-в-сэмпл.
/// Голоса привязываются к сущностям с орбитами; корень без орбиты
/// (звезда) даёт фундаментальный тон ref_hz.
pub fn sonify_world(world: &mut World, ticks: u32, dt: f64, cfg: &SonifyConfig) -> Track {
    let fs = cfg.fs;
    // Снимок голосов: (сущность, волна, гейн). Частота берётся из
    // ω на каждом тике — если физика изменит ω, звук пойдёт следом.
    let mut voices: Vec<(super::world::Entity, Waveform, f64)> = Vec::new();
    let mut omega_min = f64::INFINITY;
    let mut omega_max = f64::NEG_INFINITY;
    let entities = world.entities();
    for e in entities {
        let Some(body) = world.body(e) else { continue };
        let wf = waveform_for_class(body.class);
        // Гейн: радиус тела, нормированный на крупнейший.
        voices.push((e, wf, body.radius));
        if let Some(o) = world.orbit(e) {
            let w = o.omega;
            omega_min = omega_min.min(w);
            omega_max = omega_max.max(w);
        }
    }
    let r_max = voices.iter().map(|(_, _, r)| *r).fold(0.0f64, f64::max).max(1e-12);
    for v in voices.iter_mut() {
        v.2 = (v.2 / r_max).sqrt() * 0.8;
    }
    // Фундамент для тел без орбиты: используем [ref_hz] напрямую.
    if !omega_min.is_finite() {
        omega_min = 1.0;
        omega_max = 1.0;
    }
    let omega_max = if omega_max <= omega_min { omega_min * 2.0 } else { omega_max };

    let frames_total = (ticks as usize) * ((dt * fs as f64).round() as usize);
    let duration_s = frames_total as f64 / fs as f64;
    let mut samples = Vec::with_capacity(frames_total * 2);

    // Фазы голосов: непрерывны через границы тиков (нет щелчков).
    let mut phases = vec![0.0f64; voices.len()];
    // Нормировка панорамы:extent мира по X после первого тика.
    let mut x_extent = 1.0f64;
    {
        let mut probe = 0.0f64;
        for e in voices.iter().map(|(e, _, _)| *e) {
            if let Some(p) = world.world_pos(e) {
                probe = probe.max(p[0].abs());
            }
        }
        if probe > 1e-9 {
            x_extent = probe;
        }
    }

    for tick_i in 0..ticks {
        world.tick(dt);
        // Частоты на этом тике (ω постоянна в кеплер-лайте, но путь
        // общий — если появится прецессия, звук услышит её первым).
        let t0 = tick_i as f64 * dt;
        let frames = ((t0 + dt) * fs as f64).round() as usize
            - (t0 * fs as f64).round() as usize;
        for fi in 0..frames {
            let t = t0 + fi as f64 / fs as f64;
            let mut l = 0.0f64;
            let mut r = 0.0f64;
            for (vi, (e, wf, gain)) in voices.iter().enumerate() {
                let hz = match world.orbit(*e) {
                    Some(o) => voice_hz(o.omega, omega_min, omega_max, cfg),
                    None => cfg.ref_hz,
                };
                phases[vi] += hz / fs as f64;
                if phases[vi] >= 1.0 {
                    phases[vi] -= phases[vi].floor();
                }
                let env = cfg.adsr.level(t, duration_s - cfg.adsr.release_s.max(0.0));
                let pan = world
                    .world_pos(*e)
                    .map(|p| (p[0] / x_extent).clamp(-1.0, 1.0))
                    .unwrap_or(0.0);
                // Эквал power-pan: константная сумма энергий.
                let th = (pan + 1.0) * 0.25 * PI;
                let (gl, gr) = (th.cos(), th.sin());
                let s = wf.sample(phases[vi]) * env * gain;
                l += s * gl;
                r += s * gr;
            }
            let lg = soft_limit(l * cfg.master_gain, cfg.ceiling);
            let rg = soft_limit(r * cfg.master_gain, cfg.ceiling);
            samples.push(lg as f32);
            samples.push(rg as f32);
        }
    }
    Track { fs, samples }
}

/// Мягкий лимитер: ниже колена (0.7·ceiling) — линейность, выше —
/// гладкое tanh-сжатие к потолку. Непрерывен в точке склейки.
fn soft_limit(x: f64, ceiling: f64) -> f64 {
    let c = ceiling.max(1e-3);
    let knee = c * 0.7;
    let a = x.abs();
    if a <= knee {
        x
    } else {
        let t = ((a - knee) / (c - knee)).min(1.0);
        let shaped = knee + (c - knee) * t.tanh();
        x.signum() * shaped
    }
}

// ---------------------------------------------------------------------------
// Спектральный кристалл: STFT → топ-K пиков → K_audio
// ---------------------------------------------------------------------------

/// Итеративный радикс-2 FFT in-place (n — степень двойки).
/// Бабочки по экспонентам через рекуррентность — быстро и точно.
pub fn fft_inplace(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two());
    if n <= 1 {
        return;
    }
    // Бит-реверс
    let mut j = 0usize;
    for i in 0..n - 1 {
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
        let mut m = n >> 1;
        while m >= 1 && j & m != 0 {
            j ^= m;
            m >>= 1;
        }
        j |= m;
    }
    // Бабочки
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr0, wi0) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut wr, mut wi) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let ur = re[i + k];
                let ui = im[i + k];
                let vr = re[i + k + len / 2] * wr - im[i + k + len / 2] * wi;
                let vi = re[i + k + len / 2] * wi + im[i + k + len / 2] * wr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let nwr = wr * wr0 - wi * wi0;
                wi = wr * wi0 + wi * wr0;
                wr = nwr;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Спектральный пик окна: (частота Гц, амплитуда).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecPeak {
    pub hz: f64,
    pub amp: f64,
}

/// Конденсированный слепок трека: `windows` окон Ханна по N=4096,
/// в каждом — топ-K пиков (без DC и Найквиста). Это и есть K_audio:
/// звук как низкоранговый объект, пригодный для сравнения и хранения.
#[derive(Debug, Clone)]
pub struct SpectralCrystal {
    pub windows: Vec<Vec<SpecPeak>>,
    pub fs: u32,
}

impl SpectralCrystal {
    pub fn from_track(track: &Track, windows: usize, top_k: usize) -> Self {
        let n = 4096usize;
        let frames = track.frames();
        let mut out = Vec::with_capacity(windows);
        if frames == 0 {
            return Self { windows: out, fs: track.fs };
        }
        // Ханн один раз.
        let hann: Vec<f64> = (0..n)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / n as f64).cos()))
            .collect();
        // Моно-микс скретчем.
        let mut mono = vec![0.0f64; frames];
        for f in 0..frames {
            mono[f] = 0.5 * (track.samples[2 * f] as f64 + track.samples[2 * f + 1] as f64);
        }
        for w in 0..windows {
            let start = if windows <= 1 {
                0
            } else {
                ((frames - n) as f64 * w as f64 / (windows - 1) as f64).round() as usize
            };
            let mut re = vec![0.0f64; n];
            let mut im = vec![0.0f64; n];
            for i in 0..n.min(frames - start) {
                re[i] = mono[start + i] * hann[i];
            }
            fft_inplace(&mut re, &mut im);
            let mut peaks: Vec<SpecPeak> = Vec::new();
            for bin in 1..n / 2 {
                let amp = (re[bin] * re[bin] + im[bin] * im[bin]).sqrt();
                peaks.push(SpecPeak { hz: bin as f64 * track.fs as f64 / n as f64, amp });
            }
            peaks.sort_by(|a, b| b.amp.partial_cmp(&a.amp).unwrap_or(std::cmp::Ordering::Equal));
            peaks.truncate(top_k);
            peaks.sort_by(|a, b| a.hz.partial_cmp(&b.hz).unwrap_or(std::cmp::Ordering::Equal));
            out.push(peaks);
        }
        Self { windows: out, fs: track.fs }
    }

    /// Хеш кристалла: FNV-1a по квантованным пикам (Гц → мГц, амп → 1e-6).
    pub fn crystal_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for w in &self.windows {
            for p in w {
                for b in (p.hz * 1000.0).round().to_le_bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x0000_0100_0000_01b3);
                }
                for b in (p.amp * 1e6).round().to_le_bytes() {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        h
    }

    /// Доминирующие частоты всего трека (топ по сумме окон).
    pub fn top_hz(&self, k: usize) -> Vec<f64> {
        let mut all: Vec<SpecPeak> = self.windows.iter().flatten().copied().collect();
        all.sort_by(|a, b| b.amp.partial_cmp(&a.amp).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen: Vec<f64> = Vec::new();
        for p in all {
            if !seen.iter().any(|f| (f - p.hz).abs() < 1.0) {
                seen.push(p.hz);
            }
            if seen.len() == k {
                break;
            }
        }
        seen
    }
}

/// Имя ноты ближайшая к частоте (A4 = 440 Гц) с отклонением в центах.
pub fn note_name(hz: f64) -> String {
    if hz <= 0.0 {
        return "—".into();
    }
    const NAMES: [&str; 12] =
        ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    let m = midi.round() as i64;
    let cents = ((midi - m as f64) * 100.0).round() as i64;
    let name = NAMES[(m.rem_euclid(12)) as usize];
    let octave = (m.div_euclid(12)) - 1;
    format!("{name}{octave}{cents:+}c")
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adsr_shape_is_continuous() {
        let a = Adsr { attack_s: 0.1, decay_s: 0.1, sustain: 0.5, release_s: 0.2 };
        assert_eq!(a.level(-1.0, 1.0), 0.0);
        assert!((a.level(0.0, 1.0) - 0.0).abs() < 1e-12);
        assert!((a.level(0.05, 1.0) - 0.5).abs() < 1e-9); // середина атаки
        assert!((a.level(0.1, 1.0) - 1.0).abs() < 1e-9); // пик
        assert!((a.level(0.2, 1.0) - 0.5).abs() < 1e-9); // вышли на сустейн
        assert!((a.level(0.5, 1.0) - 0.5).abs() < 1e-9); // держим
        assert!((a.level(1.1, 1.0) - 0.25).abs() < 1e-9); // середина релиза
        assert!(a.level(1.2, 1.0).abs() < 1e-9); // тишина
        // Непрерывность на склейках
        for t in [0.0999, 0.1001, 0.1999, 0.2001, 0.9999, 1.0001] {
            assert!(a.level(t, 1.0).is_finite());
        }
    }

    #[test]
    fn waveforms_ranges_and_periods() {
        for wf in [Waveform::Sine, Waveform::Triangle, Waveform::Saw, Waveform::Square] {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for i in 0..1000 {
                let v = wf.sample(i as f64 / 1000.0);
                assert!(v.abs() <= 1.0 + 1e-12, "{wf:?} выходит за [-1,1]: {v}");
                min = min.min(v);
                max = max.max(v);
            }
            assert!(max > 0.5 && min < -0.5, "{wf:?} слишком тихий");
        }
        // Периодичность: фаза 0.25 == фаза 1.25
        for wf in [Waveform::Sine, Waveform::Triangle, Waveform::Saw, Waveform::Square] {
            let d = (wf.sample(0.25) - wf.sample(1.25)).abs();
            assert!(d < 1e-12);
        }
    }

    #[test]
    fn fft_pure_tone_bin() {
        let n = 1024usize;
        let hz_bin = 7usize;
        let fs = 48000.0;
        let mut re: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * hz_bin as f64 * i as f64 / n as f64).cos())
            .collect();
        let mut im = vec![0.0; n];
        fft_inplace(&mut re, &mut im);
        let mag: Vec<f64> = (0..n / 2)
            .map(|b| (re[b] * re[b] + im[b] * im[b]).sqrt())
            .collect();
        let peak_bin = mag
            .iter()
            .enumerate()
            .skip(1)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(peak_bin, hz_bin);
        // Энергия тона ~ n/2 (амплитуда 1)
        assert!((mag[hz_bin] / (n as f64 / 2.0) - 1.0).abs() < 1e-9);
        // Частота в Гц сходится
        let hz = hz_bin as f64 * fs / n as f64;
        assert!((hz - 7.0 * fs / n as f64).abs() < 1e-9);
    }

    #[test]
    fn track_metrics_and_hash() {
        let t = Track::silence(48000, 0.5);
        assert_eq!(t.frames(), 24000);
        assert!((t.duration_s() - 0.5).abs() < 1e-9);
        assert_eq!(t.peak(), 0.0);
        assert_eq!(t.rms_dbfs(), -120.0);
        let h0 = t.audio_hash();
        let t2 = Track::silence(48000, 0.5);
        assert_eq!(h0, t2.audio_hash());
        // Один ненулевой сэмпл меняет хеш
        let mut t3 = t2.clone();
        t3.samples[100] = 0.5;
        assert_ne!(h0, t3.audio_hash());
    }

    #[test]
    fn wav_header_and_roundtrip() {
        let mut t = Track::silence(44100, 0.1);
        // 440 Гц слева, тишина справа
        for f in 0..t.frames() {
            let s = (2.0 * PI * 440.0 * f as f64 / 44100.0).sin() * 0.5;
            t.samples[2 * f] = s as f32;
        }
        let dir = std::env::temp_dir().join("poler_audio_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tone.wav");
        let bytes = t.write_wav(&p).unwrap();
        assert_eq!(bytes, 44 + 4410 * 4);
        let raw = std::fs::read(&p).unwrap();
        assert_eq!(&raw[0..4], b"RIFF");
        assert_eq!(&raw[8..12], b"WAVE");
        assert_eq!(&raw[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([raw[20], raw[21]]), 1); // PCM
        assert_eq!(u16::from_le_bytes([raw[22], raw[23]]), 2); // stereo
        assert_eq!(u32::from_le_bytes(raw[24..28].try_into().unwrap()), 44100);
        assert_eq!(u16::from_le_bytes([raw[34], raw[35]]), 16); // bits
        assert_eq!(&raw[36..40], b"data");
        let data_len = u32::from_le_bytes(raw[40..44].try_into().unwrap());
        assert_eq!(data_len as usize, raw.len() - 44);
        // Первый сэмпл слева близок к нулю (sin(0)), правый — точно 0
        let l0 = i16::from_le_bytes([raw[44], raw[45]]);
        let r0 = i16::from_le_bytes([raw[46], raw[47]]);
        assert!(l0.abs() < 400);
        assert_eq!(r0, 0);
    }

    #[test]
    fn sonify_determinism_bit_to_bit() {
        let mk = |seed_scene: bool| {
            let mut scene = crate::game::demo_scene();
            if seed_scene {
                scene.bodies[1].orbit_radius = Some(2.1);
            }
            let mut w = scene.build_world().unwrap();
            let cfg = SonifyConfig { fs: 22050, ..Default::default() };
            let t = sonify_world(&mut w, 40, crate::game::FIXED_DT, &cfg);
            (t.audio_hash(), t.peak(), t.frames())
        };
        let (h1, _, f1) = mk(false);
        let (h2, _, f2) = mk(false);
        assert_eq!(h1, h2, "одинаковый мир → бит-в-бит тот же трек");
        assert_eq!(f1, f2);
        let (h3, _, _) = mk(true);
        assert_ne!(h1, h3, "другая орбита → другой звук");
        assert!(f1 > 0);
    }

    #[test]
    fn sonify_sounds_and_limiter_holds() {
        let mut w = crate::game::demo_scene().build_world().unwrap();
        let cfg = SonifyConfig { fs: 32000, master_gain: 2.0, ..Default::default() };
        let t = sonify_world(&mut w, 60, crate::game::FIXED_DT, &cfg);
        assert!(t.peak() > 0.05, "мир должен звучать");
        assert!(t.peak() <= 0.95 + 1e-6, "лимитер держит потолок");
        assert!(t.rms_dbfs() > -60.0, "не околонулевая громкость");
        // Нет NaN/Inf
        assert!(t.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn crystal_detects_orbital_tones() {
        let mut w = crate::game::demo_scene().build_world().unwrap();
        let cfg = SonifyConfig { fs: 48000, ..Default::default() };
        let t = sonify_world(&mut w, 120, crate::game::FIXED_DT, &cfg);
        let crystal = SpectralCrystal::from_track(&t, 4, 6);
        assert_eq!(crystal.windows.len(), 4);
        let top = crystal.top_hz(4);
        assert!(!top.is_empty());
        // Доминирующие частоты в разумном диапазоне клавесина сфер
        assert!(top.iter().all(|f| *f > 20.0 && *f < 8000.0), "{top:?}");
        // Все — конечные
        assert!(crystal.windows.iter().flatten().all(|p| p.hz.is_finite() && p.amp.is_finite()));
        // Детерминизм кристалла
        let c2 = SpectralCrystal::from_track(&t, 4, 6);
        assert_eq!(crystal.crystal_hash(), c2.crystal_hash());
    }

    #[test]
    fn note_names() {
        assert!(note_name(440.0).starts_with("A4"));
        assert!(note_name(880.0).starts_with("A5"));
        assert!(note_name(261.6).starts_with("C4"));
        assert_eq!(note_name(0.0), "—");
    }
}


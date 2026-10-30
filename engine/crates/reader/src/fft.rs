//! radix-2 FFT для спектральной верификации (без зависимостей).

/// Комплексное число (достаточно для верификатора).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cplx {
    pub re: f64,
    pub im: f64,
}

impl Cplx {
    pub fn new(re: f64, im: f64) -> Self {
        Cplx { re, im }
    }
    pub fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
    pub fn conj(self) -> Self {
        Cplx { re: self.re, im: -self.im }
    }
}

impl std::ops::Add for Cplx {
    type Output = Cplx;
    fn add(self, o: Cplx) -> Cplx {
        Cplx::new(self.re + o.re, self.im + o.im)
    }
}
impl std::ops::Sub for Cplx {
    type Output = Cplx;
    fn sub(self, o: Cplx) -> Cplx {
        Cplx::new(self.re - o.re, self.im - o.im)
    }
}
impl std::ops::Mul for Cplx {
    type Output = Cplx;
    fn mul(self, o: Cplx) -> Cplx {
        Cplx::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

/// FFT с прореживанием по времени; длина — степень двойки.
/// Прямое преобразление без нормировки (как в numpy).
pub fn fft(x: &[Cplx]) -> Vec<Cplx> {
    let n = x.len();
    assert!(n.is_power_of_two(), "FFT: длина должна быть степенью 2");
    bit_reverse_copy(x, n)
}

fn bit_reverse_copy(input: &[Cplx], n: usize) -> Vec<Cplx> {
    let mut a: Vec<Cplx> = input.to_vec();
    // перестановка с обращением бит
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            a.swap(i, j);
        }
    }
    // бабочки
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let wl = Cplx::new(ang.cos(), ang.sin());
        let half = len / 2;
        let mut start = 0;
        while start < n {
            let mut w = Cplx::new(1.0, 0.0);
            for k in 0..half {
                let u = a[start + k];
                let v = a[start + k + half] * w;
                a[start + k] = u + v;
                a[start + k + half] = u - v;
                w = w * wl;
            }
            start += len;
        }
        len *= 2;
    }
    a
}

/// Обратное FFT (нормированное на 1/N).
pub fn ifft(x: &[Cplx]) -> Vec<Cplx> {
    let n = x.len();
    let conj: Vec<Cplx> = x.iter().map(|&c| c.conj()).collect();
    let mut y = fft(&conj);
    for v in y.iter_mut() {
        *v = v.conj();
        v.re /= n as f64;
        v.im /= n as f64;
    }
    y
}

/// Окно Ханна длиной n.
pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            0.5 * (1.0
                - (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos())
        })
        .collect()
}

/// Топ спектральных пиков (Гц, отн. амплитуда) от min_hz.
/// Возвращает отсортированные по частоте, не ближе min_sep_hz друг к другу.
pub fn spectral_peaks(
    x: &[f64],
    fs: u32,
    top: usize,
    min_hz: f64,
    min_sep_hz: f64,
) -> Vec<(f64, f64)> {
    let n = x.len().next_power_of_two();
    let mut buf = vec![Cplx::new(0.0, 0.0); n];
    let w = hann(x.len());
    for (i, &s) in x.iter().enumerate() {
        buf[i] = Cplx::new(s * w[i], 0.0);
    }
    let spec = fft(&buf);
    let half = n / 2 + 1;
    let bin_hz = fs as f64 / n as f64;
    // амплитуды от min_hz
    let mut mags: Vec<(f64, f64)> = (0..half)
        .map(|i| (i as f64 * bin_hz, spec[i].abs()))
        .filter(|&(f, _)| f >= min_hz)
        .collect();
    let maxmag = mags.iter().map(|&(_, m)| m).fold(0.0, f64::max);
    if maxmag <= 0.0 {
        return Vec::new();
    }
    // топ-пики с подавлением близких
    mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut picked: Vec<(f64, f64)> = Vec::new();
    for (f, m) in mags {
        if m / maxmag < 0.05 {
            break;
        }
        if picked.iter().any(|&(pf, _)| (f - pf).abs() < min_sep_hz) {
            continue;
        }
        picked.push((f, m / maxmag));
        if picked.len() >= top {
            break;
        }
    }
    picked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    picked
}

/// Кепстральный F0 (Гц) — устойчив к формантам (лог-спектр выравнивает).
pub fn cepstral_f0(x: &[f64], fs: u32) -> Option<f64> {
    let n = x.len().next_power_of_two();
    let mut buf = vec![Cplx::new(0.0, 0.0); n];
    let w = hann(x.len());
    for (i, &s) in x.iter().enumerate() {
        buf[i] = Cplx::new(s * w[i], 0.0);
    }
    let spec = fft(&buf);
    let logspec: Vec<Cplx> = spec
        .iter()
        .map(|&c| {
            let m = c.abs().max(1e-12);
            Cplx::new(m.ln(), 0.0)
        })
        .collect();
    let ceps = ifft(&logspec);
    // пик кепстра в диапазоне периодов [fs/400, fs/60]
    let lo = (fs as f64 / 400.0).floor() as usize;
    let hi = (fs as f64 / 60.0).ceil() as usize;
    if hi >= ceps.len() {
        return None;
    }
    let mut best = lo;
    let mut best_v = -f64::INFINITY;
    for q in lo..hi {
        let v = ceps[q].abs();
        if v > best_v {
            best_v = v;
            best = q;
        }
    }
    if best == lo || best_v <= 0.0 {
        return None;
    }
    Some(fs as f64 / best as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_of_pure_tone() {
        // 1 кГц на fs 8192, N=8192 → пик в бине 1000
        let fs = 8192u32;
        let n = 8192usize;
        let f0 = 1000.0f64;
        let x: Vec<Cplx> = (0..n)
            .map(|i| Cplx::new((2.0 * std::f64::consts::PI * f0 * i as f64 / fs as f64).sin(), 0.0))
            .collect();
        let spec = fft(&x);
        let mut best = 0usize;
        let mut bm = 0.0;
        for (i, &c) in spec.iter().enumerate().take(n / 2) {
            if c.abs() > bm {
                bm = c.abs();
                best = i;
            }
        }
        let bin_hz = fs as f64 / n as f64;
        assert!((best as f64 * bin_hz - f0).abs() < 2.0 * bin_hz);
    }

    #[test]
    fn ifft_inverts_fft() {
        let x: Vec<Cplx> = (0..64)
            .map(|i| Cplx::new((i as f64 * 0.3).sin(), (i as f64 * 0.11).cos()))
            .collect();
        let y = ifft(&fft(&x));
        for (a, b) in x.iter().zip(y.iter()) {
            assert!((a.re - b.re).abs() < 1e-12);
            assert!((a.im - b.im).abs() < 1e-12);
        }
    }

    #[test]
    fn spectral_peaks_find_two_tones() {
        let fs = 8000u32;
        let n = 8192usize;
        let mut x = vec![0.0f64; n];
        for i in 0..n {
            let t = i as f64 / fs as f64;
            x[i] = (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                + 0.5 * (2.0 * std::f64::consts::PI * 1200.0 * t).sin();
        }
        let peaks = spectral_peaks(&x, fs, 4, 100.0, 80.0);
        assert!(peaks.len() >= 2);
        assert!((peaks[0].0 - 440.0).abs() < 10.0, "{:?}", peaks);
        assert!((peaks[1].0 - 1200.0).abs() < 10.0, "{:?}", peaks);
    }

    #[test]
    fn cepstral_f0_of_vowel_like() {
        // f0=120 + формантные пики: кепстр должен найти 120
        let fs = 22_050u32;
        let n = 8192usize;
        let f0 = 120.0f64;
        let mut x = vec![0.0f64; n];
        for i in 0..n {
            let t = i as f64 / fs as f64;
            // пилообразный сигнал (гармоники f0) + резонансный акцент
            let ph = (t * f0).fract();
            let saw = 2.0 * ph - 1.0;
            x[i] = saw
                + 0.6 * (2.0 * std::f64::consts::PI * 730.0 * t).sin() * saw.abs();
        }
        let f = cepstral_f0(&x, fs).unwrap();
        assert!((f - f0).abs() < 8.0, "кепстр: {f} против {f0}");
    }
}

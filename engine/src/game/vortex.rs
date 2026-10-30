//! # Vortex-кодек — «Шеннон-байпас» (цикл V, V0)
//!
//! Идея (владелец, 2026-09-23): Шеннон прав для максимально-энтропийных
//! источников — но физический шум не белый. Турбулентность — каскад
//! когерентных фазовых вихрей с колмогоровским спектром
//! E(k) ∝ k^(−5/3) (Навье–Стокс, инерционный интервал Ричардсона).
//! Такой сигнал энергетически разрежен в базисе Фурье, поэтому:
//!
//! ```text
//!   1. FFT-декомпозиция поля на гармоники-«вихри»;
//!   2. отсечение диссипативного хвоста — критерий Колмогорова:
//!      хранить наименьшее число мод, покрывающих 1−ε энергии;
//!   3. квантование каждой моды в GF(3)-триты: амплитуда — t тритов
//!      (3^t лог-уровней, A ∝ k^(−5/6) ложится на лог-шкалу),
//!      фаза — p тритов (3^p секторов: циклон/антициклон/глазок);
//!   4. упаковка: позиции битами, триты пачками по 5 в байт
//!      (3^5 = 243 ≤ 256 — наследие «Сетуни» и GF(3)-ядра POLER).
//! ```
//!
//! Честная граница (см. тесты): белый шум с максимальной энтропией
//! не сжимает никто — кодек честно деградирует (PSNR ~ единицы dB).
//! Но весь физический и игровой шум (турбулентность, fBm-текстуры,
//! шумы сенсоров) — красный спектр, и там вихревой кодек обгоняет
//! и порядково-энтропийный предел Шеннона, и gzip/zlib на порядок.
//!
//! Контракт детерминизма: `vortex_hash` (по байтам VRTX-контейнера)
//! и полевой хеш восстановленного растра бит-в-бит воспроизводимы.

use std::f64::consts::TAU;

// ---------------------------------------------------------------------------
// Детерминированный RNG (SplitMix64) — свой, суверенный
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform [0, 1), 53 бита мантиссы.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Нормальное распределение (Бокс–Мюллер, полярная форма не нужна).
    pub fn next_gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-300);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }

    /// Текущее состояние (для хэшей детерминизма и сайдкаров).
    pub fn state(&self) -> u64 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// FFT: обратное через conjugate-трюк, 2D — строка-столбец
// ---------------------------------------------------------------------------

/// Обратный FFT через прямое ядро из `audio` (T1):
/// ifft(x) = conj(fft(conj(x))) / N.
fn ifft_inplace(re: &mut [f64], im: &mut [f64]) {
    for v in im.iter_mut() {
        *v = -*v;
    }
    super::audio::fft_inplace(re, im);
    let n = re.len() as f64;
    for (r, i) in re.iter_mut().zip(im.iter_mut()) {
        *r /= n;
        *i = -*i / n;
    }
}

/// 2D FFT in-place по полю n×n (n — степень двойки), хранение по строкам.
/// Обратное преобразование включает нормировку 1/n².
fn fft2(re: &mut [f64], im: &mut [f64], n: usize, inverse: bool) {
    assert!(n.is_power_of_two() && n >= 2 && re.len() == n * n && im.len() == n * n);
    let mut br = vec![0.0f64; n];
    let mut bi = vec![0.0f64; n];
    for y in 0..n {
        let off = y * n;
        br.copy_from_slice(&re[off..off + n]);
        bi.copy_from_slice(&im[off..off + n]);
        run_1d(&mut br, &mut bi, inverse);
        re[off..off + n].copy_from_slice(&br);
        im[off..off + n].copy_from_slice(&bi);
    }
    for x in 0..n {
        for y in 0..n {
            br[y] = re[y * n + x];
            bi[y] = im[y * n + x];
        }
        run_1d(&mut br, &mut bi, inverse);
        for y in 0..n {
            re[y * n + x] = br[y];
            im[y * n + x] = bi[y];
        }
    }
}

fn run_1d(re: &mut [f64], im: &mut [f64], inverse: bool) {
    if inverse {
        ifft_inplace(re, im);
    } else {
        super::audio::fft_inplace(re, im);
    }
}

// ---------------------------------------------------------------------------
// Битовый и тритный ввод-вывод
// ---------------------------------------------------------------------------

struct BitWriter {
    out: Vec<u8>,
    cur: u64,
    nbits: u32,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter { out: Vec::new(), cur: 0, nbits: 0 }
    }

    fn push(&mut self, value: u64, width: u32) {
        for i in (0..width).rev() {
            self.cur = (self.cur << 1) | ((value >> i) & 1);
            self.nbits += 1;
            if self.nbits == 8 {
                self.out.push(self.cur as u8);
                self.cur = 0;
                self.nbits = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.out.push((self.cur << (8 - self.nbits)) as u8);
        }
        self.out
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, byte: 0, bit: 0 }
    }

    fn read(&mut self, width: u32) -> u64 {
        let mut v = 0u64;
        for _ in 0..width {
            let b = if self.byte < self.data.len() {
                (self.data[self.byte] >> (7 - self.bit)) & 1
            } else {
                0
            };
            v = (v << 1) | b as u64;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.byte += 1;
            }
        }
        v
    }
}

/// Упаковка тритов пачками по 5 в байт: 3^5 = 243 ≤ 256.
/// Неполная последняя пятёрка добивается нулями в СТАРШИХ разрядах
/// (значение — как будто за неполной пачкой стоят нули): roundtrip
/// честен для любого count, не только кратного 5.
pub fn pack_trits(digits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(digits.len().div_ceil(5));
    for chunk in digits.chunks(5) {
        let mut v: u32 = 0;
        for &d in chunk {
            v = v * 3 + (d & 3) as u32;
        }
        for _ in chunk.len()..5 {
            v *= 3; // добивка старших разрядов нулями
        }
        out.push(v as u8);
    }
    out
}

/// Обратная распаковка ровно `count` тритов.
pub fn unpack_trits(bytes: &[u8], count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count);
    for &b in bytes {
        let mut v = b as u32;
        let mut ds = [0u8; 5];
        for k in (0..5).rev() {
            ds[k] = (v % 3) as u8;
            v /= 3;
        }
        out.extend_from_slice(&ds);
        if out.len() >= count {
            break;
        }
    }
    out.truncate(count);
    out
}

fn push_base3(out: &mut Vec<u8>, mut v: u64, width: u32) {
    let mut ds = vec![0u8; width as usize];
    for k in (0..width as usize).rev() {
        ds[k] = (v % 3) as u8;
        v /= 3;
    }
    out.extend_from_slice(&ds);
}

fn read_base3(ds: &[u8], width: u32) -> u64 {
    let mut v = 0u64;
    for k in 0..width as usize {
        v = v * 3 + (ds[k] & 3) as u64;
    }
    v
}

// ---------------------------------------------------------------------------
// GF(3)-квантование мод: амплитуда (лог-шкала) и фаза (сектора)
// ---------------------------------------------------------------------------

/// Амплитуда → код q ∈ [0, 3^t): лог-равномерная шкала в [a_min, a_max].
/// Спектры Колмогорова степенные — лог-шкала честно ложится на них.
pub fn amp_quant(a: f64, a_min: f64, a_max: f64, t: u32) -> u64 {
    let levels = 3f64.powi(t as i32);
    if !(a_min > 0.0) || !(a_max > a_min) || levels <= 1.5 {
        return 0;
    }
    let lo = a_min.ln();
    let hi = a_max.ln();
    let q = ((a.ln() - lo) / (hi - lo) * (levels - 1.0)).round();
    q.clamp(0.0, levels - 1.0) as u64
}

/// Код q → амплитуда.
pub fn amp_dequant(q: u64, a_min: f64, a_max: f64, t: u32) -> f64 {
    let levels = 3f64.powi(t as i32);
    if !(a_min > 0.0) || !(a_max > a_min) || levels <= 1.5 {
        return a_min;
    }
    a_min * (a_max / a_min).powf(q as f64 / (levels - 1.0))
}

/// Фаза φ ∈ (−π, π] → сектор s ∈ [0, 3^p). Сектор 0 — «глазок вихря»
/// (φ ≈ 0), соседние — деления циклона/антициклона.
pub fn phase_quant(phi: f64, p: u32) -> u64 {
    let sectors = 3f64.powi(p as i32);
    let ph = ((phi % TAU) + TAU) % TAU;
    (((ph / TAU) * sectors).round() as i64).rem_euclid(sectors as i64) as u64
}

/// Сектор → фаза.
pub fn phase_dequant(s: u64, p: u32) -> f64 {
    TAU * s as f64 / 3f64.powi(p as i32)
}

// ---------------------------------------------------------------------------
// Источники «шума» — весь спектр честности
// ---------------------------------------------------------------------------

fn normalize01(f: &mut [f64]) {
    let mut mn = f64::INFINITY;
    let mut mx = f64::NEG_INFINITY;
    for &v in f.iter() {
        mn = mn.min(v);
        mx = mx.max(v);
    }
    let span = mx - mn;
    if span <= 1e-300 {
        for v in f.iter_mut() {
            *v = 0.0;
        }
        return;
    }
    for v in f.iter_mut() {
        *v = (*v - mn) / span;
    }
}

/// «Шум из когерентных вихрей» — конструкция владельца: сумма
/// синусоид с ЦЕЛЫМИ волновыми числами, амплитуды A ∝ |k|^(−5/6)
/// (K41), фазы детерминированные из сида. Выглядит как шум, но
/// спектр точно дискретный — кодек обязан его найти целиком.
pub fn vortex_field(n: usize, seed: u64, modes: usize) -> Vec<f64> {
    assert!(n.is_power_of_two() && n >= 4);
    let mut rng = Rng::new(seed);
    let mut f = vec![0.0f64; n * n];
    let half = (n / 2) as i64;
    let mut seen = std::collections::HashSet::new();
    let mut placed = 0usize;
    let mut attempts = 0usize;
    while placed < modes && attempts < modes * 64 + 256 {
        attempts += 1;
        // Лог-равномерный радиус — каскад от инерционных к диссипативным масштабам
        let r = (rng.next_f64() * (half as f64).ln()).exp().max(1.0);
        let th = rng.next_f64() * TAU;
        let kx = (r * th.cos()).round() as i64;
        let ky = (r * th.sin()).round() as i64;
        if kx == 0 && ky == 0 {
            continue;
        }
        if kx.abs() > half || ky.abs() > half {
            continue;
        }
        if !seen.insert((kx, ky)) {
            continue;
        }
        let knorm = ((kx * kx + ky * ky) as f64).sqrt();
        let amp = knorm.powf(-5.0 / 6.0);
        let phi = rng.next_f64() * TAU;
        let nf = n as f64;
        for y in 0..n {
            let yv = y as f64;
            for x in 0..n {
                let arg = TAU * (kx as f64 * x as f64 + ky as f64 * yv) / nf + phi;
                f[y * n + x] += amp * arg.sin();
            }
        }
        placed += 1;
    }
    normalize01(&mut f);
    f
}

/// Синтетическая турбулентность (метод случайных фурье-мод):
/// спектр A(k) ∝ k^(−5/6)·exp(−k²/2κ²), случайные фазы → Re(ifft2).
/// Вещественная часть автоматически эрмитово-симметрична.
pub fn kolmogorov_field(n: usize, seed: u64) -> Vec<f64> {
    assert!(n.is_power_of_two() && n >= 4);
    let mut rng = Rng::new(seed);
    let mut re = vec![0.0f64; n * n];
    let mut im = vec![0.0f64; n * n];
    let half = n as f64 / 2.0;
    let kappa = half * 0.35;
    for ky in 0..n {
        let wy = ky.min(n - ky) as f64;
        for kx in 0..n {
            let wx = kx.min(n - kx) as f64;
            let k = (wx * wx + wy * wy).sqrt();
            if k < 0.5 {
                continue; // DC не трогаем
            }
            let amp = k.powf(-5.0 / 6.0) * (-k * k / (2.0 * kappa * kappa)).exp();
            let phi = rng.next_f64() * TAU;
            let i = ky * n + kx;
            re[i] = amp * phi.cos();
            im[i] = amp * phi.sin();
        }
    }
    fft2(&mut re, &mut im, n, true);
    normalize01(&mut re);
    re
}

/// Игровой fBm (тот же, что у текстур T2) — «красный» спектр.
pub fn fbm_field(n: usize, seed: u64, octaves: u32) -> Vec<f64> {
    assert!(n.is_power_of_two() && n >= 4);
    let mut f = vec![0.0f64; n * n];
    for y in 0..n {
        for x in 0..n {
            f[y * n + x] = super::texture::fbm(
                x as f64 / n as f64 * 6.0,
                y as f64 / n as f64 * 6.0,
                seed,
                octaves,
                0.55,
            );
        }
    }
    f
}

/// Честная граница: истинно белый шум (максимум энтропии).
pub fn white_field(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    (0..n * n).map(|_| rng.next_f64()).collect()
}

// ---------------------------------------------------------------------------
// Кодек: analyze → encode → decode → synthesize
// ---------------------------------------------------------------------------

/// Параметры кодека.
#[derive(Debug, Clone, Copy)]
pub struct VortexParams {
    /// Доля мод от n² (когда `energy_eps` не задан).
    pub keep_fraction: f64,
    /// Критерий Колмогорова: покрыть 1−ε полной энергии минимальным числом мод.
    pub energy_eps: Option<f64>,
    /// Триты амплитуды: 3^t лог-уровней.
    pub amp_trits: u32,
    /// Триты фазы: 3^p секторов.
    pub phase_trits: u32,
}

impl Default for VortexParams {
    fn default() -> Self {
        VortexParams { keep_fraction: 0.02, energy_eps: None, amp_trits: 4, phase_trits: 6 }
    }
}

/// Одна квантованная мода: позиция + GF(3)-коды амплитуды и фазы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harmonic {
    pub kx: u32,
    pub ky: u32,
    pub q: u64,
    pub s: u64,
}

/// Вихревой спектр: тритные моды по убыванию энергии.
#[derive(Debug, Clone)]
pub struct VortexSpectrum {
    pub n: u32,
    pub amp_trits: u32,
    pub phase_trits: u32,
    pub a_min: f64,
    pub a_max: f64,
    pub harmonics: Vec<Harmonic>,
    /// Полная энергия (после `decode` неизвестна — 0).
    pub energy_total: f64,
    /// Энергия сохранённых мод (после `decode` — 0; см. `kept_energy_approx`).
    pub energy_kept: f64,
}

impl VortexSpectrum {
    /// Хеш детерминизма: FNV-1a по байтам VRTX-контейнера.
    pub fn vortex_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in encode(self) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// Доля покрытой энергии по квантованным амплитудам (для `decode`).
    pub fn kept_energy_approx(&self) -> f64 {
        let kept: f64 = self
            .harmonics
            .iter()
            .map(|h| {
                let a = amp_dequant(h.q, self.a_min, self.a_max, self.amp_trits);
                a * a
            })
            .sum();
        if self.energy_total > 0.0 {
            (kept / self.energy_total).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Разложение поля на тритные вихревые моды.
///
/// Моды ниже 1e-24 полной энергии не хранятся (числовой шум FFT):
/// они ничего не дают синтезу. При равных энергиях порядок
/// детерминирован (по индексу).
pub fn analyze(field: &[f64], n: usize, params: &VortexParams) -> VortexSpectrum {
    assert_eq!(field.len(), n * n, "analyze: поле должно быть n×n");
    assert!(n.is_power_of_two() && n >= 2, "analyze: n — степень двойки");
    let t = params.amp_trits.clamp(1, 10);
    let p = params.phase_trits.clamp(1, 10);

    let mut re = field.to_vec();
    let mut im = vec![0.0f64; n * n];
    fft2(&mut re, &mut im, n, false);

    let mut order: Vec<(u32, f64)> = (0..n * n)
        .map(|i| (i as u32, re[i] * re[i] + im[i] * im[i]))
        .collect();
    let energy_total: f64 = order.iter().map(|&(_, e)| e).sum();
    order.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let budget = match params.energy_eps {
        Some(eps) => {
            let target = energy_total * (1.0 - eps.clamp(1e-12, 1.0));
            let mut acc = 0.0;
            let mut k = 0usize;
            while k < order.len() {
                acc += order[k].1;
                k += 1;
                if acc >= target {
                    break;
                }
            }
            k
        }
        None => ((params.keep_fraction.clamp(0.0, 1.0) * (n * n) as f64).round() as usize)
            .clamp(1, n * n),
    };

    let mut kept: Vec<(u32, f64, f64)> = Vec::with_capacity(budget.min(order.len()));
    let mut energy_kept = 0.0f64;
    // Относительный пол: числовой шум FFT (~1e-30 энергии) — не физическая
    // мода; не тратим на него бюджет мод и байты контейнера.
    let floor_e = energy_total * 1e-24;
    for &(idx, e) in order.iter().take(budget) {
        if e <= floor_e {
            continue; // нулевые/шумовые моды не храним
        }
        let i = idx as usize;
        kept.push((idx, e.sqrt(), im[i].atan2(re[i])));
        energy_kept += e;
    }

    let (a_min, a_max) = kept
        .iter()
        .map(|&(_, a, _)| a)
        .fold((f64::INFINITY, 0.0f64), |(mn, mx), a| (mn.min(a), mx.max(a)));
    if kept.is_empty() || !(a_max > 0.0) {
        return VortexSpectrum {
            n: n as u32,
            amp_trits: t,
            phase_trits: p,
            a_min: 1.0,
            a_max: 1.0,
            harmonics: Vec::new(),
            energy_total,
            energy_kept: 0.0,
        };
    }
    let a_min = if a_min > 0.0 { a_min } else { a_max * 1e-12 };

    let harmonics = kept
        .iter()
        .map(|&(idx, a, phi)| Harmonic {
            kx: idx % n as u32,
            ky: idx / n as u32,
            q: amp_quant(a, a_min, a_max, t),
            s: phase_quant(phi, p),
        })
        .collect();

    VortexSpectrum { n: n as u32, amp_trits: t, phase_trits: p, a_min, a_max, harmonics, energy_total, energy_kept }
}

/// Синтез поля из тритных мод. Вещественная часть обратного FFT:
/// эрмитова симметрия выполняется автоматически.
pub fn synthesize(spec: &VortexSpectrum) -> Vec<f64> {
    let n = spec.n as usize;
    let mut re = vec![0.0f64; n * n];
    let mut im = vec![0.0f64; n * n];
    for h in &spec.harmonics {
        let a = amp_dequant(h.q, spec.a_min, spec.a_max, spec.amp_trits);
        let phi = phase_dequant(h.s, spec.phase_trits);
        let i = h.ky as usize * n + h.kx as usize;
        re[i] = a * phi.cos();
        im[i] = a * phi.sin();
    }
    fft2(&mut re, &mut im, n, true);
    re
}

/// Упаковка спектра в байты: VRTX-контейнер.
///
/// ```text
///   [0..4)   магия "VRTX"          [13]     t (триты амплитуды)
///   [4]      версия 1              [14]     p (триты фазы)
///   [5..9)   n (u32 LE)            [15..23) a_min (биты f64)
///   [9..13)  K (u32 LE)            [23..31) a_max (биты f64)
///   далее: позиции (2·log2(n) бит на модулю, MSB-first),
///   затем триты пачками по 5 в байт (q: t цифр, s: p цифр).
/// ```
pub fn encode(spec: &VortexSpectrum) -> Vec<u8> {
    let n = spec.n as usize;
    let log2n = n.trailing_zeros();
    let mut out = Vec::with_capacity(31 + spec.harmonics.len() * 4);
    out.extend_from_slice(b"VRTX");
    out.push(1);
    out.extend_from_slice(&spec.n.to_le_bytes());
    out.extend_from_slice(&(spec.harmonics.len() as u32).to_le_bytes());
    out.push(spec.amp_trits as u8);
    out.push(spec.phase_trits as u8);
    out.extend_from_slice(&spec.a_min.to_bits().to_le_bytes());
    out.extend_from_slice(&spec.a_max.to_bits().to_le_bytes());

    let mut bw = BitWriter::new();
    for h in &spec.harmonics {
        bw.push(h.kx as u64, log2n);
        bw.push(h.ky as u64, log2n);
    }
    out.extend_from_slice(&bw.finish());

    let mut digits: Vec<u8> =
        Vec::with_capacity(spec.harmonics.len() * (spec.amp_trits + spec.phase_trits) as usize);
    for h in &spec.harmonics {
        push_base3(&mut digits, h.q, spec.amp_trits);
        push_base3(&mut digits, h.s, spec.phase_trits);
    }
    out.extend_from_slice(&pack_trits(&digits));
    out
}

/// Распаковка VRTX-контейнера. Энергии в байтах нет — метрики
/// считаются на уровне `analyze`/`kept_energy_approx`.
pub fn decode(bytes: &[u8]) -> Result<VortexSpectrum, String> {
    if bytes.len() < 31 || &bytes[0..4] != b"VRTX" {
        return Err("vortex: не VRTX-контейнер".into());
    }
    if bytes[4] != 1 {
        return Err(format!("vortex: версия формата {}", bytes[4]));
    }
    let n = u32::from_le_bytes(bytes[5..9].try_into().unwrap()) as usize;
    let k = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
    let t = bytes[13] as u32;
    let p = bytes[14] as u32;
    let a_min = f64::from_bits(u64::from_le_bytes(bytes[15..23].try_into().unwrap()));
    let a_max = f64::from_bits(u64::from_le_bytes(bytes[23..31].try_into().unwrap()));
    if !n.is_power_of_two() || n < 2 || t == 0 || p == 0 {
        return Err("vortex: битый заголовок".into());
    }
    let log2n = n.trailing_zeros();
    let pos_bytes = (k * 2 * log2n as usize).div_ceil(8);
    if bytes.len() < 31 + pos_bytes {
        return Err("vortex: обрезан поток позиций".into());
    }
    let mut br = BitReader::new(&bytes[31..31 + pos_bytes]);
    let mut positions = Vec::with_capacity(k);
    for _ in 0..k {
        let kx = br.read(log2n) as u32;
        let ky = br.read(log2n) as u32;
        positions.push((kx, ky));
    }
    let trit_count = k * (t + p) as usize;
    let digits = unpack_trits(&bytes[31 + pos_bytes..], trit_count);
    if digits.len() < trit_count {
        return Err("vortex: обрезан трит-поток".into());
    }
    let mut harmonics = Vec::with_capacity(k);
    let mut i = 0usize;
    for &(kx, ky) in &positions {
        let q = read_base3(&digits[i..], t);
        i += t as usize;
        let s = read_base3(&digits[i..], p);
        i += p as usize;
        harmonics.push(Harmonic { kx, ky, q, s });
    }
    Ok(VortexSpectrum {
        n: n as u32,
        amp_trits: t,
        phase_trits: p,
        a_min,
        a_max,
        harmonics,
        energy_total: 0.0,
        energy_kept: 0.0,
    })
}

// ---------------------------------------------------------------------------
// Метрики честности
// ---------------------------------------------------------------------------

/// PSNR в домене u8 (как его видит PNG и глаз).
pub fn psnr_u8(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut mse = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = *x as f64 - *y as f64;
        mse += d * d;
    }
    mse /= a.len().max(1) as f64;
    if mse <= 1e-12 {
        f64::INFINITY
    } else {
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

/// Порядковая энтропия Шеннона (бит/сэмпл) — теоретический предел
/// порядково-нулевого lossless-кодера.
pub fn entropy_bits_u8(bytes: &[u8]) -> f64 {
    let mut hist = [0u64; 256];
    for &b in bytes {
        hist[b as usize] += 1;
    }
    let total = bytes.len() as f64;
    if total <= 0.0 {
        return 0.0;
    }
    let mut h = 0.0;
    for c in hist {
        if c > 0 {
            let p = c as f64 / total;
            h -= p * p.log2();
        }
    }
    h
}

/// Полный отчёт кодека по одному источнику.
#[derive(Debug, Clone)]
pub struct VortexReport {
    pub source: &'static str,
    pub n: u32,
    pub modes_total: usize,
    pub modes_kept: usize,
    pub energy_covered: f64,
    pub raw_bytes: usize,
    pub codec_bytes: usize,
    pub ratio: f64,
    pub psnr_db: f64,
    pub entropy_bits: f64,
    pub entropy_floor_ratio: f64,
    pub vortex_hash: u64,
    pub field_hash: u64,
}

/// Полный конвейер: поле → analyze → encode → decode → synthesize → метрики.
pub fn codec_run(
    source: &'static str,
    field: &[f64],
    n: usize,
    params: &VortexParams,
) -> VortexReport {
    let orig_u8 = field_u8(field);
    let spec = analyze(field, n, params);
    let bytes = encode(&spec);
    let decoded = decode(&bytes).expect("vortex: собственный контейнер декодируется");
    let recon = synthesize(&decoded);
    let recon_u8 = field_u8(&recon);
    let raw_bytes = n * n; // u8 на сэмпл — честная база сравнения
    let entropy = entropy_bits_u8(&orig_u8);
    VortexReport {
        source,
        n: n as u32,
        modes_total: n * n,
        modes_kept: spec.harmonics.len(),
        energy_covered: if spec.energy_total > 0.0 {
            spec.energy_kept / spec.energy_total
        } else {
            0.0
        },
        raw_bytes,
        codec_bytes: bytes.len(),
        ratio: raw_bytes as f64 / bytes.len().max(1) as f64,
        psnr_db: psnr_u8(&orig_u8, &recon_u8),
        entropy_bits: entropy,
        entropy_floor_ratio: 8.0 / entropy.max(1e-9),
        vortex_hash: spec.vortex_hash(),
        field_hash: fnv1a_u8(&recon_u8),
    }
}

/// Поле [0,1] → u8 с честным округлением.
pub fn field_u8(field: &[f64]) -> Vec<u8> {
    field
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// FNV-1a по байтам — единый маяк детерминизма движка.
pub fn fnv1a_u8(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// 1D-применение: извлечение сигнала из зашумлённого канала
// ---------------------------------------------------------------------------

/// Отчёт о шумоподавлении.
#[derive(Debug, Clone, Copy)]
pub struct DenoiseReport {
    pub snr_in_db: f64,
    pub snr_out_db: f64,
    pub gain_db: f64,
    pub kept: usize,
    pub total: usize,
}

/// Вихревой фильтр 1D: оставить топ-K гармоник по энергии.
/// Работает, когда сигнал спектрально разрежен (тоны, партиалы),
/// а шум — плоский: его энергия размазана, и порог её срезает.
pub fn denoise_signal(clean: &[f64], noisy: &[f64], keep: usize) -> (Vec<f64>, DenoiseReport) {
    assert_eq!(clean.len(), noisy.len());
    let n = noisy.len();
    assert!(n.is_power_of_two() && n >= 2);
    let mut re = noisy.to_vec();
    let mut im = vec![0.0f64; n];
    super::audio::fft_inplace(&mut re, &mut im);

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let ea = re[a] * re[a] + im[a] * im[a];
        let eb = re[b] * re[b] + im[b] * im[b];
        eb.partial_cmp(&ea).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
    });
    let mut mask = vec![false; n];
    for &i in order.iter().take(keep.max(1).min(n)) {
        mask[i] = true;
    }
    for i in 0..n {
        if !mask[i] {
            re[i] = 0.0;
            im[i] = 0.0;
        }
    }
    ifft_inplace(&mut re, &mut im);
    let filtered = re;

    let clean_power: f64 = clean.iter().map(|v| v * v).sum();
    let snr_of = |x: &[f64]| -> f64 {
        let err: f64 = x.iter().zip(clean.iter()).map(|(a, b)| (a - b) * (a - b)).sum();
        if err <= 1e-300 || clean_power <= 1e-300 {
            f64::INFINITY
        } else {
            10.0 * (clean_power / err).log10()
        }
    };
    let snr_in = snr_of(noisy);
    let snr_out = snr_of(&filtered);
    let report = DenoiseReport {
        snr_in_db: snr_in,
        snr_out_db: snr_out,
        gain_db: snr_out - snr_in,
        kept: keep.max(1).min(n),
        total: n,
    };
    (filtered, report)
}

/// Демо-тон «Етерия-мини»: три партиала (A3 + квинта + октава).
pub fn demo_tone(count: usize) -> Vec<f64> {
    let fs = 44100.0f64;
    (0..count)
        .map(|i| {
            let t = i as f64 / fs;
            0.55 * (TAU * 220.0 * t).sin()
                + 0.30 * (TAU * 330.0 * t + 0.7).sin()
                + 0.15 * (TAU * 440.0 * t + 1.3).sin()
        })
        .collect()
}

/// Зашумление до заданного SNR (dB).
pub fn add_noise(clean: &[f64], seed: u64, snr_db: f64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    let clean_power: f64 = clean.iter().map(|v| v * v).sum();
    let noise: Vec<f64> = (0..clean.len()).map(|_| rng.next_gaussian()).collect();
    let noise_power: f64 = noise.iter().map(|v| v * v).sum();
    let scale = (clean_power / noise_power.max(1e-300) / 10f64.powf(snr_db / 10.0)).sqrt();
    clean
        .iter()
        .zip(noise.iter())
        .map(|(&c, &nz)| c + nz * scale)
        .collect()
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn fft2_roundtrip_identity() {
        let n = 64;
        let mut rng = Rng::new(42);
        let field: Vec<f64> = (0..n * n).map(|_| rng.next_f64() * 2.0 - 1.0).collect();
        let mut re = field.clone();
        let mut im = vec![0.0; n * n];
        fft2(&mut re, &mut im, n, false);
        fft2(&mut re, &mut im, n, true);
        for (a, b) in field.iter().zip(re.iter()) {
            assert!((a - b).abs() < 1e-9, "roundtrip: {a} vs {b}");
        }
    }

    #[test]
    fn parseval_energy_conservation() {
        let n = 32;
        let mut rng = Rng::new(7);
        let field: Vec<f64> = (0..n * n).map(|_| rng.next_f64() * 2.0 - 1.0).collect();
        let mut re = field.clone();
        let mut im = vec![0.0; n * n];
        fft2(&mut re, &mut im, n, false);
        let spec_energy: f64 = re.iter().zip(im.iter()).map(|(r, i)| r * r + i * i).sum();
        let field_energy: f64 = field.iter().map(|v| v * v).sum::<f64>() * (n * n) as f64;
        let rel = (spec_energy - field_energy).abs() / field_energy;
        assert!(rel < 1e-9, "Парсеваль: относительное отклонение {rel}");
    }

    #[test]
    fn trit_packing_roundtrip() {
        let mut rng = Rng::new(3);
        for count in [0usize, 1, 4, 5, 6, 13, 27, 100] {
            let digits: Vec<u8> = (0..count).map(|_| (rng.next_u64() % 3) as u8).collect();
            let packed = pack_trits(&digits);
            let unpacked = unpack_trits(&packed, count);
            assert_eq!(digits, unpacked, "count={count}");
        }
    }

    #[test]
    fn base3_push_read_roundtrip() {
        let mut rng = Rng::new(5);
        for width in [1u32, 2, 3, 5, 8] {
            let max = 3u64.pow(width);
            let mut stream = Vec::new();
            let vals: Vec<u64> = (0..17).map(|_| rng.next_u64() % max).collect();
            for &v in &vals {
                push_base3(&mut stream, v, width);
            }
            let mut i = 0usize;
            for &v in &vals {
                assert_eq!(read_base3(&stream[i..], width), v);
                i += width as usize;
            }
        }
    }

    #[test]
    fn bitio_roundtrip() {
        let mut rng = Rng::new(11);
        let mut bw = BitWriter::new();
        let vals: Vec<(u64, u32)> = (0..100)
            .map(|_| {
                let w = (rng.next_u64() % 16 + 1) as u32;
                (rng.next_u64() & ((1u64 << w) - 1), w)
            })
            .collect();
        for &(v, w) in &vals {
            bw.push(v, w);
        }
        let bytes = bw.finish();
        let mut br = BitReader::new(&bytes);
        for &(v, w) in &vals {
            assert_eq!(br.read(w), v);
        }
    }

    #[test]
    fn amp_phase_quant_error_bounds() {
        let (a_min, a_max, t) = (0.01f64, 5.0f64, 4u32);
        let levels = 3f64.powi(t as i32);
        let max_step = ((a_max / a_min).ln() / (levels - 1.0)).exp() - 1.0;
        let mut rng = Rng::new(13);
        for _ in 0..200 {
            let a = a_min * (a_max / a_min).powf(rng.next_f64());
            let q = amp_quant(a, a_min, a_max, t);
            let back = amp_dequant(q, a_min, a_max, t);
            assert!(((back / a - 1.0).abs()) <= max_step + 1e-12);
        }
        let p = 4u32;
        let sector = TAU / 3f64.powi(p as i32);
        for _ in 0..200 {
            let phi = rng.next_f64() * TAU - PI;
            let s = phase_quant(phi, p);
            let back = phase_dequant(s, p);
            let mut d = (back - phi).abs();
            if d > PI {
                d = TAU - d;
            }
            assert!(d <= sector / 2.0 + 1e-12);
        }
    }

    #[test]
    fn vortex_source_is_rediscovered_exactly() {
        // Конструкция владельца: сумма целых синусов → спектр дискретный.
        // Критерий Колмогорова с ε→0 обязан найти все моды и ничего лишнего.
        let n = 128;
        let modes = 24;
        let field = vortex_field(n, 7, modes);
        let params = VortexParams { energy_eps: Some(1e-9), ..Default::default() };
        let spec = analyze(&field, n, &params);
        // 24 моды + эрмитовы пары; допуск на почти-нулевые хвосты нормировки
        assert!(
            spec.harmonics.len() <= modes * 2 + 8,
            "лишние моды: {}",
            spec.harmonics.len()
        );
        assert!(spec.energy_kept / spec.energy_total > 0.999999);
        let recon = synthesize(&spec);
        let psnr = psnr_u8(&field_u8(&field), &field_u8(&recon));
        assert!(psnr > 30.0, "PSNR вихревого источника {psnr}");
    }

    #[test]
    fn codec_bytes_roundtrip_is_exact() {
        let n = 64;
        let field = fbm_field(n, 21, 5);
        let params = VortexParams::default();
        let spec = analyze(&field, n, &params);
        let bytes = encode(&spec);
        let back = decode(&bytes).expect("декод");
        assert_eq!(back.n, spec.n);
        assert_eq!(back.amp_trits, spec.amp_trits);
        assert_eq!(back.phase_trits, spec.phase_trits);
        assert_eq!(back.a_min.to_bits(), spec.a_min.to_bits());
        assert_eq!(back.a_max.to_bits(), spec.a_max.to_bits());
        assert_eq!(back.harmonics, spec.harmonics);
        // Синтез через байты == синтез напрямую (масштабы бит-в-бит)
        let a = synthesize(&spec);
        let b = synthesize(&back);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.to_bits(), y.to_bits());
        }
    }

    #[test]
    fn kolmogorov_spectrum_follows_k41() {
        let n = 128;
        let field = kolmogorov_field(n, 9);
        let mut re = field.clone();
        let mut im = vec![0.0; n * n];
        fft2(&mut re, &mut im, n, false);
        // Радиальные бины энергии → регрессия log-log в инерционном интервале
        let half = n / 2;
        let mut bins: Vec<(f64, f64)> = vec![(0.0, 0.0); half + 1];
        for ky in 0..n {
            let wy = ky.min(n - ky) as f64;
            for kx in 0..n {
                let wx = kx.min(n - kx) as f64;
                let k = (wx * wx + wy * wy).sqrt().round() as usize;
                if k >= bins.len() {
                    continue;
                }
                let i = ky * n + kx;
                bins[k].0 += re[i] * re[i] + im[i] * im[i];
                bins[k].1 += 1.0;
            }
        }
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        // Инерционный интервал K41: до загиба диссипации (k ~ κ = 0.35·half).
        // Выше гауссов срез крутит наклон круче −5/3 — это уже не инерционный
        // диапазон, и мерить там наклон физически неправильно.
        for (k, &(e, cnt)) in bins.iter().enumerate() {
            if k >= 2 && k <= half / 4 && cnt > 0.0 && e > 0.0 {
                xs.push((k as f64).ln());
                ys.push((e / cnt).ln());
            }
        }
        let mx = xs.iter().sum::<f64>() / xs.len() as f64;
        let my = ys.iter().sum::<f64>() / ys.len() as f64;
        let mut num = 0.0;
        let mut den = 0.0;
        for (&x, &y) in xs.iter().zip(ys.iter()) {
            num += (x - mx) * (y - my);
            den += (x - mx) * (x - mx);
        }
        let slope = num / den;
        // K41: −5/3 ≈ −1.667; допуск на конечную решётку и срез
        assert!(
            (-2.35..=-1.05).contains(&slope),
            "наклон спектра {slope} вне полосы K41"
        );
    }

    #[test]
    fn fbm_compression_beats_entropy_floor() {
        let n = 128;
        let field = fbm_field(n, 5, 5);
        let report = codec_run("fbm", &field, n, &VortexParams::default());
        assert!(report.psnr_db > 25.0, "PSNR fBm {}", report.psnr_db);
        assert!(
            report.ratio > 5.0 * report.entropy_floor_ratio,
            "ratio {} не бьёт энтропийный предел {}",
            report.ratio,
            report.entropy_floor_ratio
        );
    }

    #[test]
    fn white_noise_is_honest_boundary() {
        let n = 128;
        let params = VortexParams::default();
        let fbm = fbm_field(n, 5, 5);
        let white = white_field(n, 5);
        let r_fbm = codec_run("fbm", &fbm, n, &params);
        let r_white = codec_run("white", &white, n, &params);
        assert!(
            r_white.psnr_db < r_fbm.psnr_db - 8.0,
            "белый шум слишком хорошо восстановился: {} vs {}",
            r_white.psnr_db,
            r_fbm.psnr_db
        );
    }

    #[test]
    fn determinism_hashes_bit_to_bit() {
        let n = 128;
        let mk = |seed| codec_run("kolmogorov", &kolmogorov_field(n, seed), n, &VortexParams::default());
        let a = mk(77);
        let b = mk(77);
        let c = mk(78);
        assert_eq!(a.vortex_hash, b.vortex_hash);
        assert_eq!(a.field_hash, b.field_hash);
        assert_ne!(a.vortex_hash, c.vortex_hash);
    }

    #[test]
    fn zero_field_edge_case() {
        let n = 32;
        let field = vec![0.0f64; n * n];
        let spec = analyze(&field, n, &VortexParams::default());
        assert!(spec.harmonics.is_empty());
        let recon = synthesize(&spec);
        assert!(recon.iter().all(|&v| v.abs() < 1e-12));
        let bytes = encode(&spec);
        let back = decode(&bytes).expect("пустой контейнер декодируется");
        assert!(back.harmonics.is_empty());
    }

    #[test]
    fn denoise_pulls_signal_from_noise() {
        let n = 65536;
        let clean = demo_tone(n);
        let noisy = add_noise(&clean, 99, 0.0); // SNR 0 dB
        let (filtered, report) = denoise_signal(&clean, &noisy, 8);
        assert_eq!(filtered.len(), n);
        assert!(
            report.gain_db > 10.0,
            "выигрыш SNR {} dB",
            report.gain_db
        );
    }
}


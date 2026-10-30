//! RaBitQ-класс: 1-битное квантование векторов с рандомизированным
//! вращением Адамара (v2.0, Приоритет 4, кирпич 1).
//!
//! Чистая реализация по мотивам RaBitQ (Gao & Long, SIGMOD 2024).
//! Ключевая идея: перед знаковым квантованием пространство вращается
//! рандомизированным преобразованием Адамара `y = H·(S·x)` — все
//! координаты становятся статистически эквивалентными, и per-vector
//! поправки (mu, delta, gamma) дают ограниченную ошибку оценки
//! скалярного произведения. Наивное знаковое квантование такой
//! гарантии не имеет.
//!
//! Плотность: d-мерный вектор → `D/8` байт кода (D = d, округлённое
//! вверх до степени двойки — требование быстрого Адамара) + 12 байт
//! скаляров (mu, delta, gamma — f32). Для 768-d (BGE-M3, паддинг до
//! 1024): 128 Б + 12 Б против 3072 Б fp32 — 21.9× с поправкой на
//! плотность; для 64-d (nomic Matryoshka): 8 Б + 12 Б против 256 Б.
//!
//! Две оценки скалярного произведения (IP):
//! * **sym** — запрос тоже квантован: XOR + popcount, десятки ГБ/с;
//!   путь обхода HNSW-графа (грубая оценка).
//! * **ADC** — запрос в полной точности против 1-битных кодов
//!   (сбор `yq_i` по установленным битам); путь переранжирования.
//!
//! Точная норма восстанавливается из скаляров: `||x||² = D·mu² + delta²`
//! (центрированная часть ортогональна единичному вектору), поэтому
//! косинус оценки нормализуется точными нормами с обеих сторон.
//!
//! Принцип «инструмент, не ИИ»: слой ничего не «понимает» — он
//! хранит биты и возвращает числовую близость. Смысл векторов —
//! дело эмбеддера (инференс готовой модели) и ИИ-потребителя.

#[cfg(target_arch = "x86_64")]
use std::arch::is_x86_feature_detected;

// ---------------------------------------------------------------------------
// Детерминированный ГПСЧ SplitMix64 — без зависимости rand
// ---------------------------------------------------------------------------

/// SplitMix64: 64-битный ГПСЧ с хорошим лавинным рассеянием.
/// Детерминирован и переносим — кодирование воспроизводимо бит в бит.
#[derive(Debug, Clone)]
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// U ∈ [0, 1) — для геометрического распределения уровней HNSW.
    #[inline]
    pub(crate) fn next_u01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
}

// ---------------------------------------------------------------------------
// Рандомизированное вращение Адамара
// ---------------------------------------------------------------------------

/// Следующая степень двойки, не меньше `n` (и не меньше 64 — минимум
/// для векторного кода; D всегда кратна 64, маска валидности не нужна).
pub(crate) fn next_pow2_dim(n: usize) -> usize {
    let mut d = n.max(64);
    d -= 1;
    d |= d >> 1;
    d |= d >> 2;
    d |= d >> 4;
    d |= d >> 8;
    d |= d >> 16;
    d |= d >> 32;
    d + 1
}

/// Быстрое преобразование Уолша—Адамара по месту, нормированное
/// на `1/sqrt(n)` — ортогонально: `||Hx|| == ||x||`.
fn fwt_inplace(a: &mut [f32]) {
    let n = a.len();
    debug_assert!(n.is_power_of_two());
    let mut h = 1usize;
    while h < n {
        let mut i = 0usize;
        while i < n {
            for j in i..i + h {
                let x = a[j];
                let y = a[j + h];
                a[j] = x + y;
                a[j + h] = x - y;
            }
            i += h * 2;
        }
        h *= 2;
    }
    let inv = 1.0 / (n as f32).sqrt();
    for v in a.iter_mut() {
        *v *= inv;
    }
}

/// Рандомизированное вращение `y = H·(S·x)`:
/// диагональные знаки `S` (±1) выводятся детерминированно из сида.
#[derive(Debug, Clone)]
pub struct Rotation {
    d_pad: usize,
    /// Знак на каждую координату: +1.0 / −1.0.
    signs: Vec<f32>,
}

impl Rotation {
    pub fn new(seed: u64, d_pad: usize) -> Self {
        debug_assert!(d_pad.is_power_of_two() && d_pad >= 64);
        let mut rng = SplitMix64::new(seed ^ 0xADA_1B1D_A000_0000);
        let mut signs = Vec::with_capacity(d_pad);
        while signs.len() < d_pad {
            let w = rng.next_u64();
            for b in 0..64 {
                if signs.len() == d_pad {
                    break;
                }
                signs.push(if (w >> b) & 1 == 1 { 1.0 } else { -1.0 });
            }
        }
        Self { d_pad, signs }
    }

    /// `x` (d ≤ d_pad, хвост добивается нулями) → вращённый `y` (d_pad).
    pub fn rotate(&self, x: &[f32], out: &mut [f32]) {
        debug_assert_eq!(out.len(), self.d_pad);
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = if i < x.len() { x[i] * self.signs[i] } else { 0.0 };
        }
        fwt_inplace(out);
    }
}

// ---------------------------------------------------------------------------
// Скаляры и оценки IP
// ---------------------------------------------------------------------------

/// Per-vector скаляры квантованного вектора.
///
/// * `mu` — среднее вращённого вектора (центр);
/// * `delta` — `||y − mu·1||₂` (норма центрированной части);
/// * `gamma` — `mean(|y_i − mu|)` — LS-оптимальный масштаб для ADC.
///
/// Инвариант: `gamma ≤ delta/sqrt(D)` (Коши: сумма модулей не
/// превосходит sqrt(D)·нормы).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VecScalars {
    pub mu: f32,
    pub delta: f32,
    pub gamma: f32,
}

/// Точная норма исходного вектора из скаляров: `D·mu² + delta²`.
#[inline]
pub fn norm2_from_scalars(sc: VecScalars, d_pad: usize) -> f64 {
    d_pad as f64 * (sc.mu as f64) * (sc.mu as f64) + (sc.delta as f64) * (sc.delta as f64)
}

/// Сторона симметричной оценки: квантованное представление вектора
/// (кода достаточно — y не нужен).
#[derive(Debug, Clone, Copy)]
pub struct SymSide<'a> {
    pub codes: &'a [u64],
    pub mu: f32,
    pub delta: f32,
}

/// Симметричная оценка IP (оба квантованы): XOR + popcount + arcsin-MLE.
///
/// Разложение по центрированным частям: `⟨x,q⟩ = D·mu_x·mu_q + ⟨r_x, r_q⟩`
/// (члены `mu·Σr` обнуляются — `Σr = 0` по построению). Для jointly-Gaussian
/// после вращения `E[⟨b̄x,b̄q⟩/D] = (2/π)·arcsin(ρ)` — обращаем:
/// `ρ̂ = sin(π·⟨b̄x,b̄q⟩/(2D))`, и `⟨r_x,r_q⟩ ≈ Δ_x·Δ_q·ρ̂`.
/// Точные нормы (Δ) с обеих сторон убирают систематическое сжатие
/// наивного `s·s·⟨b̄,b̄⟩` (2/π по Коши).
pub fn sym_ip(x: SymSide<'_>, q: SymSide<'_>, d_pad: usize) -> f64 {
    let d = d_pad as f64;
    let h = hamming(x.codes, q.codes) as f64;
    let agree = (d - 2.0 * h) / d; // ⟨b̄x,b̄q⟩/D ∈ [−1, 1]
    let rho = (std::f64::consts::FRAC_PI_2 * agree).sin();
    d * (x.mu as f64) * (q.mu as f64)
        + (x.delta as f64) * (q.delta as f64) * rho
}

/// Асимметричная оценка IP (ADC): запрос в полной точности.
///
/// `⟨x,q⟩ = mu_x·S_q + ⟨r_x, q_c⟩`, где `q_c = yq − mu_q·1` (центр
/// запроса; `Σr_x = 0` — только центрированная часть и входит).
/// Для jointly-Gaussian: `E[sign(r_x,i)·q_c,i] = ρ_i·σ_qc,i·√(2/π)` и
/// `γ = √(2/π)·Δ/√D` — тогда `(π/2)·γ_x·Σb̄·q_c` несмещенно;
/// в терминах `S⁺ = Σ_{b_i=1} yq_i` и popcount: `Σb̄·q_c = 2(S⁺ − mu_q·pop)`.
/// Проверка несмещённости: на self-IP типичное отклонение ~2%
/// (разбаланс модулей сторон конкретного вектора), E — точно `||x||²`.
pub fn adc_ip(x_codes: &[u64], x: VecScalars, q: &QueryPrep) -> f64 {
    let s_plus_c = sum_set_bits_centered(x_codes, &q.yq, q.mu_q);
    x.mu as f64 * q.s_q + std::f64::consts::PI * x.gamma as f64 * s_plus_c
}

/// Косинус по ADC-оценке: нормирование ТОЧНЫМИ нормами (нюанс RaBitQ —
/// точный знаменатель убирает главный источник смещения).
pub fn adc_cos(x_codes: &[u64], x: VecScalars, q: &QueryPrep, d_pad: usize) -> f64 {
    let nx = norm2_from_scalars(x, d_pad).sqrt();
    let nq = q.q_norm2.sqrt();
    if nx <= 0.0 || nq <= 0.0 {
        return 0.0;
    }
    adc_ip(x_codes, x, q) / (nx * nq)
}

/// `Σ (yq[i] − mu_q)` по установленным битам кода = `S⁺ − mu_q·pop`
/// (путь ADC, центрирование запроса).
#[inline]
fn sum_set_bits_centered(codes: &[u64], v: &[f32], mu_q: f32) -> f64 {
    let mu = mu_q as f64;
    let mut acc = 0.0f64;
    for (w, word) in codes.iter().enumerate() {
        let base = w * 64;
        let mut bits = *word;
        while bits != 0 {
            let t = bits.trailing_zeros() as usize;
            acc += v[base + t] as f64 - mu;
            bits &= bits - 1;
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Hamming: popcount с runtime-детектом
// ---------------------------------------------------------------------------

/// Расстояние Хэмминга кодов (число различающихся бит).
pub fn hamming(a: &[u64], b: &[u64]) -> u32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("popcnt") {
            return unsafe { hamming_popcnt(a, b) };
        }
    }
    let mut acc = 0u32;
    for i in 0..a.len() {
        acc += (a[i] ^ b[i]).count_ones();
    }
    acc
}

/// Аппаратный POPCNT-путь (SSE4.2-эпоха; присутствует на всём, что
/// умеет AVX2). Одна итерация = XOR + POPCNT на 8 байт кода.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn hamming_popcnt(a: &[u64], b: &[u64]) -> u32 {
    let mut acc = 0u32;
    for i in 0..a.len() {
        acc += (a[i] ^ b[i]).count_ones();
    }
    acc
}

// ---------------------------------------------------------------------------
// Кодировщик
// ---------------------------------------------------------------------------

/// Кодировщик RaBitQ фиксированной размерности и сида вращения.
///
/// Один экземпляр кодирует и данные, и запросы; сид вращения — часть
/// формата сериализации хранилища (запросы другого хранилища невалидны).
#[derive(Debug, Clone)]
pub struct Encoder {
    /// Исходная размерность эмбеддингов.
    pub d: usize,
    /// Размерность после паддинга до степени двойки.
    pub d_pad: usize,
    /// Сид вращения (сериализуется в заголовке хранилища).
    pub rot_seed: u64,
    rot: Rotation,
}

impl Encoder {
    pub fn new(d: usize, rot_seed: u64) -> Self {
        assert!(
            (1..=1 << 20).contains(&d),
            "Encoder: подозрительная размерность {d}"
        );
        let d_pad = next_pow2_dim(d);
        let rot = Rotation::new(rot_seed, d_pad);
        Self {
            d,
            d_pad,
            rot_seed,
            rot,
        }
    }

    /// Число 64-битных слов кода на вектор.
    #[inline]
    pub fn words_per_vec(&self) -> usize {
        self.d_pad / 64
    }

    /// Квантует `x` (d значений) в коды + скаляры.
    ///
    /// Возвращает скаляры; коды пишутся в `codes` (`words_per_vec()` слов).
    /// Бит `i` = 1 ⟺ `y_i − mu ≥ 0`.
    pub fn encode_into(&self, x: &[f32], codes: &mut [u64]) -> VecScalars {
        assert_eq!(x.len(), self.d, "Encoder: вход имеет чужую размерность");
        assert_eq!(
            codes.len(),
            self.words_per_vec(),
            "Encoder: чужая длина кода"
        );
        let mut y = vec![0.0f32; self.d_pad];
        self.rot.rotate(x, &mut y);

        let n = self.d_pad as f64;
        let mu = y.iter().sum::<f32>() / n as f32;
        let mut delta2 = 0.0f64;
        let mut abs_sum = 0.0f64;
        for v in y.iter() {
            let r = *v as f64 - mu as f64;
            delta2 += r * r;
            abs_sum += r.abs();
        }
        let delta = delta2.sqrt() as f32;
        let gamma = (abs_sum / n) as f32;

        codes.fill(0);
        for (i, v) in y.iter().enumerate() {
            if (*v - mu) as f64 >= 0.0 {
                codes[i / 64] |= 1u64 << (i % 64);
            }
        }
        VecScalars { mu, delta, gamma }
    }

    /// Подготовка запроса: вращение + квантование кода запроса
    /// (для sym-обхода) + полная точность (для ADC).
    pub fn prepare_query(&self, q: &[f32]) -> QueryPrep {
        assert_eq!(q.len(), self.d, "Encoder: запрос имеет чужую размерность");
        let mut yq = vec![0.0f32; self.d_pad];
        self.rot.rotate(q, &mut yq);

        let n = self.d_pad as f64;
        let s_q = yq.iter().map(|v| *v as f64).sum::<f64>();
        let mu_q = (s_q / n) as f32;
        let mut delta2 = 0.0f64;
        for v in yq.iter() {
            let r = *v as f64 - mu_q as f64;
            delta2 += r * r;
        }
        let delta_q = delta2.sqrt() as f32;

        let wpv = self.words_per_vec();
        let mut codes_q = vec![0u64; wpv];
        for (i, v) in yq.iter().enumerate() {
            if (*v - mu_q) as f64 >= 0.0 {
                codes_q[i / 64] |= 1u64 << (i % 64);
            }
        }

        let q_norm2 = q.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();

        QueryPrep {
            yq,
            s_q,
            mu_q,
            delta_q,
            codes_q,
            q_norm2,
        }
    }
}

/// Полная подготовка запроса (один раз на запрос):
/// вращённый вектор в f32 + квантованный код + скаляры.
#[derive(Debug, Clone)]
pub struct QueryPrep {
    /// Вращённый запрос `yq` (d_pad, полная точность — путь ADC).
    pub yq: Vec<f32>,
    /// `Σ yq_i`.
    pub s_q: f64,
    pub mu_q: f32,
    pub delta_q: f32,
    /// Код запроса (для sym-обхода).
    pub codes_q: Vec<u64>,
    /// `||q||²` точно (нормирование косинуса).
    pub q_norm2: f64,
}

impl QueryPrep {
    /// Симметричная сторона запроса (заимствование, без копий).
    pub fn sym(&self) -> SymSide<'_> {
        SymSide {
            codes: &self.codes_q,
            mu: self.mu_q,
            delta: self.delta_q,
        }
    }
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Гауссовский вектор из ГПСЧ (Бокс—Мюллер).
    fn gauss(rng: &mut SplitMix64, d: usize) -> Vec<f32> {
        (0..d)
            .map(|_| {
                let u1 = rng.next_u01().max(1e-12);
                let u2 = rng.next_u01();
                ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
            })
            .collect()
    }

    fn l2_normalize(v: &mut [f32]) {
        let n = (v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>()).sqrt();
        if n > 0.0 {
            for x in v.iter_mut() {
                *x /= n as f32;
            }
        }
    }

    fn true_ip(a: &[f32], b: &[f32]) -> f64 {
        a.iter().zip(b).map(|(x, y)| (*x as f64) * (*y as f64)).sum()
    }

    #[test]
    fn fwt_is_orthonormal() {
        let mut rng = SplitMix64::new(42);
        for &d in &[64usize, 128, 1024] {
            let x = gauss(&mut rng, d);
            let mut y = x.clone();
            fwt_inplace(&mut y);
            let nx = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt();
            let ny = y.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt();
            assert!(
                (nx - ny).abs() < 1e-3 * nx.max(1.0),
                "FWT не сохраняет норму: {nx} vs {ny}"
            );
            // Парсеваль: <Hx, Hz> == <x, z>
            let z = gauss(&mut rng, d);
            let mut hz = z.clone();
            fwt_inplace(&mut hz);
            let ip_orig = true_ip(&x, &z);
            let ip_rot = true_ip(&y, &hz);
            assert!(
                (ip_orig - ip_rot).abs() < 1e-3 * (1.0 + ip_orig.abs()),
                "Парсеваль нарушен: {ip_orig} vs {ip_rot}"
            );
        }
    }

    #[test]
    fn rotation_is_deterministic_and_seed_sensitive() {
        let r1 = Rotation::new(7, 128);
        let r2 = Rotation::new(7, 128);
        let r3 = Rotation::new(8, 128);
        let x: Vec<f32> = (0..100).map(|i| (i as f32) * 0.5 - 25.0).collect();
        let mut a = vec![0.0; 128];
        let mut b = vec![0.0; 128];
        let mut c = vec![0.0; 128];
        r1.rotate(&x, &mut a);
        r2.rotate(&x, &mut b);
        r3.rotate(&x, &mut c);
        assert_eq!(a, b, "один сид — один результат");
        assert_ne!(a, c, "разные сиды — разные вращения");
    }

    #[test]
    fn padding_matches_next_pow2() {
        assert_eq!(next_pow2_dim(1), 64);
        assert_eq!(next_pow2_dim(64), 64);
        assert_eq!(next_pow2_dim(100), 128);
        assert_eq!(next_pow2_dim(768), 1024);
        assert_eq!(next_pow2_dim(1024), 1024);
    }

    #[test]
    fn encode_scalars_respect_bounds() {
        let enc = Encoder::new(768, 0x504F_4C45);
        let mut rng = SplitMix64::new(1);
        for _ in 0..50 {
            let x = gauss(&mut rng, 768);
            let mut codes = vec![0u64; enc.words_per_vec()];
            let sc = enc.encode_into(&x, &mut codes);
            assert!(sc.delta >= 0.0);
            // Коши: gamma = mean|r| <= ||r||/sqrt(D) = delta/sqrt(D)
            assert!(
                sc.gamma <= sc.delta / (1024.0f32).sqrt() + 1e-6,
                "gamma {sc:?} нарушает неравенство Коши"
            );
            // Норма точно восстанавливается
            let true_n2 = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
            let est_n2 = norm2_from_scalars(sc, enc.d_pad);
            assert!(
                (true_n2 - est_n2).abs() < 1e-3 * true_n2,
                "норма не восстановилась: {true_n2} vs {est_n2}"
            );
        }
    }

    #[test]
    fn encode_is_deterministic() {
        let enc = Encoder::new(300, 99);
        let x: Vec<f32> = (0..300).map(|i| (i % 17) as f32 * 0.01).collect();
        let mut c1 = vec![0u64; enc.words_per_vec()];
        let mut c2 = vec![0u64; enc.words_per_vec()];
        let s1 = enc.encode_into(&x, &mut c1);
        let s2 = enc.encode_into(&x, &mut c2);
        assert_eq!(c1, c2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn adc_self_ip_is_unbiased() {
        // Инвариант: оценка несмещённа — среднее по ансамблю в пределах 5%
        // от ||x||², хвост по конкретному вектору < 25% (флуктуация γ —
        // куртозис |r| конкретного вектора; для 1-бита это физика).
        let enc = Encoder::new(64, 5);
        let mut rng = SplitMix64::new(3);
        let mut ratios: Vec<f64> = Vec::new();
        for _ in 0..40 {
            let x = gauss(&mut rng, 64);
            let mut cx = vec![0u64; enc.words_per_vec()];
            let sx = enc.encode_into(&x, &mut cx);
            let prep = enc.prepare_query(&x);
            let ip = adc_ip(&cx, sx, &prep);
            let true_ipx = true_ip(&x, &x);
            ratios.push(ip / true_ipx);
        }
        let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
        let worst = ratios.iter().map(|r| (r - 1.0).abs()).fold(0.0f64, f64::max);
        assert!(
            (mean - 1.0).abs() < 0.05,
            "ADC несмещённость нарушена: среднее отношение {mean:.3}"
        );
        assert!(
            worst < 0.25,
            "хвост self-IP {worst:.3} — что-то хуже физики 1-бита"
        );
    }

    #[test]
    fn estimators_correlate_with_truth_on_clusters() {
        // Ранжирующая способность на КЛАСТЕРНЫХ данных (реалистичный
        // сценарий эмбеддингов). На изотропном шуме 1-бит фундаментально
        // шумит (ρ̂ std ~ π/2√(2D)) — это физика метода, а не баг.
        let d = 256;
        let enc = Encoder::new(d, 0xAB12);
        let mut rng = SplitMix64::new(777);
        // 40 кластеров × 3 вектора + 40 фоновых
        let centers: Vec<Vec<f64>> = (0..40).map(|_| (0..d).map(|_| gauss01(&mut rng)).collect()).collect();
        let mut xs: Vec<Vec<f32>> = Vec::new();
        for c in &centers {
            for _ in 0..3 {
                let mut v: Vec<f32> = (0..d)
                    .map(|j| (c[j] + gauss01(&mut rng) * 0.3) as f32)
                    .collect();
                l2_normalize(&mut v);
                xs.push(v);
            }
        }
        for _ in 0..40 {
            let mut v: Vec<f32> = (0..d).map(|_| gauss01(&mut rng) as f32).collect();
            l2_normalize(&mut v);
            xs.push(v);
        }
        // Запрос — возмущённый член первого кластера
        let mut q: Vec<f32> = (0..d)
            .map(|j| (centers[0][j] + gauss01(&mut rng) * 0.3) as f32)
            .collect();
        l2_normalize(&mut q);
        let prep = enc.prepare_query(&q);

        let mut truth: Vec<f64> = Vec::with_capacity(xs.len());
        let mut sym: Vec<f64> = Vec::with_capacity(xs.len());
        let mut adc: Vec<f64> = Vec::with_capacity(xs.len());
        for x in &xs {
            let mut codes = vec![0u64; enc.words_per_vec()];
            let sc = enc.encode_into(x, &mut codes);
            truth.push(true_ip(x, &q));
            sym.push(sym_ip(
                SymSide {
                    codes: &codes,
                    mu: sc.mu,
                    delta: sc.delta,
                },
                prep.sym(),
                enc.d_pad,
            ));
            adc.push(adc_ip(&codes, sc, &prep));
        }
        fn pearson(a: &[f64], b: &[f64]) -> f64 {
            let n = a.len() as f64;
            let ma = a.iter().sum::<f64>() / n;
            let mb = b.iter().sum::<f64>() / n;
            let cov = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum::<f64>();
            let va = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>().sqrt();
            let vb = b.iter().map(|y| (y - mb).powi(2)).sum::<f64>().sqrt();
            cov / (va * vb)
        }
        let cs = pearson(&truth, &sym);
        let ca = pearson(&truth, &adc);
        assert!(cs > 0.80, "sym-корреляция на кластерах: {cs}");
        assert!(ca > 0.90, "ADC-корреляция на кластерах: {ca}");
        assert!(ca >= cs, "ADC ({ca}) не лучше sym ({cs})");
    }

    /// Один гауссовский отсчёт (общий для кластерных тестов).
    fn gauss01(rng: &mut SplitMix64) -> f64 {
        let u1 = rng.next_u01().max(1e-12);
        let u2 = rng.next_u01();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    #[test]
    fn adc_cos_bounds_and_self_similarity() {
        let d = 256;
        let enc = Encoder::new(d, 31337);
        let mut rng = SplitMix64::new(5);
        let mut a = gauss(&mut rng, d);
        l2_normalize(&mut a);
        let mut b = gauss(&mut rng, d);
        l2_normalize(&mut b);
        let mut ca = vec![0u64; enc.words_per_vec()];
        let sa = enc.encode_into(&a, &mut ca);
        let pa = enc.prepare_query(&a);
        let pb = enc.prepare_query(&b);
        let self_cos = adc_cos(&ca, sa, &pa, enc.d_pad);
        let cross_cos = adc_cos(&ca, sa, &pb, enc.d_pad);
        assert!(
            self_cos > 0.95,
            "самокосинус нормализованного вектора подозрительно мал: {self_cos}"
        );
        assert!(
            self_cos > cross_cos + 0.5,
            "самокосинус ({self_cos}) не больше кросс-косинуса ({cross_cos})"
        );
        assert!(cross_cos.abs() < 0.7, "независимые векторы «слишком близки»");
    }

    #[test]
    fn hamming_parity_scalar_vs_simd() {
        let mut rng = SplitMix64::new(11);
        for _ in 0..100 {
            let a: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
            let b: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
            let simd = hamming(&a, &b);
            let scalar = a
                .iter()
                .zip(&b)
                .map(|(x, y)| (x ^ y).count_ones())
                .sum::<u32>();
            assert_eq!(simd, scalar);
        }
    }

    #[test]
    fn non_pow2_dim_pads_correctly() {
        let enc = Encoder::new(100, 1);
        assert_eq!(enc.d_pad, 128);
        assert_eq!(enc.words_per_vec(), 2);
        let x: Vec<f32> = (0..100).map(|i| i as f32 * 0.03).collect();
        let mut codes = vec![0u64; 2];
        let sc = enc.encode_into(&x, &mut codes);
        let prep = enc.prepare_query(&x);
        let ip = adc_ip(&codes, sc, &prep);
        let tip = true_ip(&x, &x);
        assert!(
            (ip - tip).abs() < 0.05 * tip,
            "паддинг сломал точность self-IP: {ip} vs {tip}"
        );
    }
}

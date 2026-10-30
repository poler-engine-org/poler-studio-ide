//! RQ18: нелинейная архетипическая алгебра `a ⊗_ε b` — мышление
//! архетипами, а не токенами.
//!
//! Трансформер мыслит плоскими корреляциями: «рыцарь» и «программист»
//! не связаны, пока он не увидит миллион примеров. POLER мыслит
//! **архетипами** — фазовыми паттернами решётки — и оператор
//! `⊗_ε` находит структурный изоморфизм там, где прямых русл `J`
//! нет: пересечение архетипов рождает новый устойчивый смысл.
//!
//! ## Физика: нелинейная интерференция фазовых решёток
//!
//! По ТЗ: `c = a ⊗_ε b = Π_Λ( cos(θ_a ⊙ θ_b) + ε·(a ∧ b) )`.
//! На тритовой решётке Packed4 (фазы `θ ∈ {0, π/2, π}`, коды
//! `0 = Zero, 1 = Pos, 2 = Neg`) это разворачивается в два слоя
//! нелинейности — **обе без раздувания в f64**:
//!
//! **Слой 1 — пер-дуговая интерференция** (таблица [`ARCHETYPE_LUT`]):
//!
//! ```text
//! raw_i = R(a_i, b_i) + ε_w·(a_i ∧ b_i),   Π_Λ = ближайший трит
//! ```
//!
//! * `R` — резонанс: насыщенная сумма тритов (суперпозиция прозрачна,
//!   согласие полюсов резонирует, встречные полюса гаснут);
//! * `a_i ∧ b_i = a_i·b_i` — клин: `±1`, если оба полюса, иначе 0;
//! * `ε_w` — коэффициент клина, полоса устойчивости `ε_w ∈ (0, ½]`.
//!
//! Таблица (инвариантна по всей полосе `ε_w`):
//!
//! ```text
//!            b: Zero   Pos   Neg
//!      a:Zero   Zero   Pos   Neg     ← суперпозиция прозрачна: структура
//!      a:Pos    Pos    Pos   Zero       проходит сквозь незнание
//!      a:Neg    Neg    Zero  Neg     ← конфликт полюсов аннигилирует
//! ```
//!
//! **Слой 2 — энергетический гейт `ε`** (главная нелинейность):
//! пересечение считается **состоявшимся**, только если энергия
//! `E = co / min(nnz a, nnz b)` ≥ ε — какая доля меньшей структуры
//! разделяется большим архетипом. `E < ε` — архетипы ортогональны:
//! произведение — **весь нуль**, ложных ассоциаций не существует.
//! Один f64-сравнение на весь вызов (гейт), интерференция — чистый
//! целочисленный SWAR.
//!
//! ## Инварианты (ТЗ п.3 — тестами ниже)
//!
//! 1. **Идемпотентность** `a ⊗_ε a = a` бит-в-бит (0 галлюцинаций):
//!    `E = 1 ≥ ε`, таблица на диагонали тождественна.
//! 2. **Ортогональность** `a ⊗_ε b = 0` при непересекающихся
//!    носителях: `co = 0 → E = 0 < ε` — гейт заперт.
//! 3. Коммутативность `a ⊗_ε b = b ⊗_ε a` (таблица и гейт симметричны).
//!
//! ## Пример
//!
//! ```
//! use pqc::archetype_lattice::archetype_product_packed4;
//! use pqw::phase::{pack_quad, pack_trit2, Trit};
//!
//! // Два архетипа над d = 8: пересечение на дугах 0 (согласие) и 1 (конфликт).
//! let a = pack_quad([Trit::Pos, Trit::Pos, Trit::Zero, Trit::Zero]);
//! let b = pack_quad([Trit::Pos, Trit::Neg, Trit::Pos, Trit::Zero]);
//! let mut out = [0u8; 2];
//! let st = archetype_product_packed4(&[a, 0], &[b, 0], 8, 0.5, &mut out).unwrap();
//!
//! assert!(st.resonant, "пересечение есть — энергия {}/1 ≥ ε", st.co_support);
//! assert_eq!(st.resonance, 1);      // Pos ⊗ Pos = Pos
//! assert_eq!(st.conflict, 1);       // Pos ⊗ Neg = Zero (аннигиляция)
//! // Продукт: согласие выжило, конфликт погас, прозрачные прошли.
//! assert_eq!(out[0], pack_quad([Trit::Pos, Trit::Zero, Trit::Pos, Trit::Zero]));
//! ```

use pqw::phase::{pack_trit2, Trit};

use crate::error::{PqcError, Result};

/// Резонансная таблица произведения архетипов на 2-битных кодах
/// (`0 = Zero, 1 = Pos, 2 = Neg`): `ARCHETYPE_LUT[a][b] → код c`.
///
/// Это Π_Λ-проекция `R + ε_w·(a ∧ b)` при любом `ε_w ∈ (0, ½]`:
/// клин усиливает резонанс (`1 + ε_w` остаётся полюсом) и удерживает
/// аннигиляцию в нуле (`|−ε_w| < ½`). Насыщенная сумма тритов.
pub const ARCHETYPE_LUT: [[u8; 3]; 3] = [
    // b = Zero  Pos  Neg
    [0, 1, 2], // a = Zero: прозрачность
    [1, 1, 0], // a = Pos: резонанс Pos, аннигиляция Neg
    [2, 0, 2], // a = Neg: прозрачность Pos, резонанс Neg
];

/// Порог энергетического гейта по умолчанию: не меньше половины
/// меньшей структуры разделяется — это изоморфизм, а не совпадение.
pub const DEFAULT_BRIDGE_EPS: f64 = 0.5;

/// Полоса допустимых порогов гейта `ε ∈ (0, 1]`.
#[inline]
fn valid_eps(eps: f64) -> bool {
    eps.is_finite() && eps > 0.0 && eps <= 1.0
}

/// Пер-дуговое произведение архетипов на тритах (эталон таблицы).
///
/// Семантика: суперпозиция прозрачна, согласие полюсов резонирует,
/// встречные полюса аннигилируют в суперпозицию (открытый вопрос).
#[inline]
pub fn archetype_trit(a: Trit, b: Trit) -> Trit {
    // Индексы — v2-коды Packed4 (0 = Zero, 1 = Pos, 2 = Neg).
    let (ca, cb) = (pack_trit2(a) as usize, pack_trit2(b) as usize);
    let code = ARCHETYPE_LUT[ca][cb];
    // Коды таблицы — валидные коды v2 (0/1/2): 0b11 не встречается.
    match code {
        1 => Trit::Pos,
        2 => Trit::Neg,
        _ => Trit::Zero,
    }
}

/// Статистика произведения архетипов.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArchetypeStats {
    /// Ненулевых тритов в `a` (носитель архетипа a).
    pub nnz_a: usize,
    /// Ненулевых тритов в `b`.
    pub nnz_b: usize,
    /// Дуг пересечения: оба полюса (носитель клина `a ∧ b`).
    pub co_support: usize,
    /// Согласных полюсов пересечения (конструктивная интерференция).
    pub resonance: usize,
    /// Встречных полюсов (деструктивная — аннигиляция в Zero).
    pub conflict: usize,
    /// Энергия пересечения `E = co / min(nnz_a, nnz_b)` (0 при пустом).
    pub energy: f64,
    /// Гейт открыт: `E ≥ ε` — архетипы структурно изоморфны.
    pub resonant: bool,
    /// Ненулевых тритов произведения (0 при запертом гейте).
    pub nnz_out: usize,
}

/// Энергия пересечения: доля меньшего носителя, разделяемая большим.
///
/// `min(nnz) = 0` (пустой архетип) даёт `E = 0` — произведение с
/// пустотой не резонирует: нечему интерферировать.
#[inline]
pub fn archetype_energy(co_support: usize, nnz_a: usize, nnz_b: usize) -> f64 {
    let m = nnz_a.min(nnz_b);
    if m == 0 {
        0.0
    } else {
        co_support as f64 / m as f64
    }
}

/// Маска ненулевых лейнов байта: бит `2k` ⟺ лейн `k` ≠ 0.
///
/// Лейн `k` занимает биты `2k..2k+1`; коды `1` (Pos) и `2` (Neg)
/// имеют хотя бы один установленный бит, `0` (Zero) — нет.
#[inline]
pub(crate) fn nonzero_lanes(x: u8) -> u8 {
    (x | (x >> 1)) & 0b0101_0101
}

/// Нелинейное произведение архетипов `c = a ⊗_ε b` прямо в байтах
/// Packed4 — SWAR-интерференция, ни одного FPU-вызова на дугу.
///
/// Проход по байтам (`d.div_ceil(4)`): конфликтные лейны (коды
/// `1 ⊕ 2 = 3`) гасятся, остальные сливаются насыщенным ИЛИ;
/// параллельно копятся счётчики носителей и пересечения (popcount
/// масок). Единственное f64 — сравнение гейта `E ≥ ε` в конце.
///
/// Гейт заперт (`E < ε` или пустой архетип): `out` заполняется
/// нулями — произведение ортогональных архетипов есть **Zero**,
/// ложных ассоциаций не существует.
///
/// Контракт: `a.len(), b.len(), out.len() ≥ ⌈d/4⌉`, `d ≥ 1`,
/// `ε ∈ (0, 1]`.
pub fn archetype_product_packed4(
    a: &[u8],
    b: &[u8],
    d: usize,
    eps: f64,
    out: &mut [u8],
) -> Result<ArchetypeStats> {
    let n = d.div_ceil(4);
    if d == 0 {
        return Err(PqcError::EmptyState);
    }
    if a.len() < n || b.len() < n || out.len() < n {
        return Err(PqcError::LengthMismatch {
            expected: n,
            actual: a.len().min(b.len()).min(out.len()),
        });
    }
    if !valid_eps(eps) {
        return Err(PqcError::BadPhase(eps));
    }

    // Маска хвостового байта: паддинг-лейны (дуги ≥ d) выключаются
    // из интерференции И из счётчиков — решётка кончается на дуге d−1.
    let tail = d % 4;
    let tail_mask: u8 = if tail == 0 {
        0b1111_1111
    } else {
        (1u16 << (2 * tail)) as u8 - 1
    };

    let mut nnz_a = 0usize;
    let mut nnz_b = 0usize;
    let mut co = 0usize;
    let mut resonance = 0usize;
    let bytes = n - 1; // полные байты до хвостового

    for k in 0..bytes {
        let (x, y) = (a[k], b[k]);
        // Конфликтные лейны: xor кодов == 3 (только Pos ⊕ Neg).
        let xr = x ^ y;
        let conf = xr & (xr >> 1) & 0b0101_0101;
        let conf_mask = conf | (conf << 1);
        // Слияние: насыщенное ИЛИ с погашением конфликтов.
        out[k] = (x | y) & !conf_mask;
        // Счётчики: popcount пер-лейновых масок.
        let (nza, nzb) = (nonzero_lanes(x), nonzero_lanes(y));
        let co_lanes = nza & nzb;
        let eq = !(xr | (xr >> 1)) & 0b0101_0101 & co_lanes;
        nnz_a += nza.count_ones() as usize;
        nnz_b += nzb.count_ones() as usize;
        co += co_lanes.count_ones() as usize;
        resonance += eq.count_ones() as usize;
    }
    // Хвостовой байт: только живые лейны.
    {
        let (x, y) = (a[bytes] & tail_mask, b[bytes] & tail_mask);
        let xr = x ^ y;
        let conf = xr & (xr >> 1) & 0b0101_0101;
        let conf_mask = conf | (conf << 1);
        out[bytes] = (x | y) & !conf_mask;
        let (nza, nzb) = (nonzero_lanes(x), nonzero_lanes(y));
        let co_lanes = nza & nzb;
        let eq = !(xr | (xr >> 1)) & 0b0101_0101 & co_lanes;
        nnz_a += nza.count_ones() as usize;
        nnz_b += nzb.count_ones() as usize;
        co += co_lanes.count_ones() as usize;
        resonance += eq.count_ones() as usize;
    }
    // За гейтом out не читали выше — байты после ⌈d/4⌉ не трогаем.

    let energy = archetype_energy(co, nnz_a, nnz_b);
    let resonant = energy >= eps;
    if !resonant {
        // Ортогональные архетипы: произведение — нуль (инвариант ТЗ).
        out[..n].fill(0);
        return Ok(ArchetypeStats {
            nnz_a,
            nnz_b,
            co_support: co,
            resonance,
            conflict: co - resonance,
            energy,
            resonant: false,
            nnz_out: 0,
        });
    }

    // nnz_out: popcount носителя продукта (прозрачность сохраняет
    // структуру, конфликты гаснут — считаем по написанным байтам).
    let nnz_out: usize = out[..n]
        .iter()
        .map(|&b| nonzero_lanes(b).count_ones() as usize)
        .sum();
    Ok(ArchetypeStats {
        nnz_a,
        nnz_b,
        co_support: co,
        resonance,
        conflict: co - resonance,
        energy,
        resonant: true,
        nnz_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::phase::{pack_quad, pack_trit2, unpack_quad, Trit};

    /// Упаковка тритов в решётку (`d ≤ 4·len(ts)`).
    fn pack(ts: &[Trit]) -> Vec<u8> {
        let mut v = vec![0u8; ts.len().div_ceil(4)];
        for (i, &t) in ts.iter().enumerate() {
            let shift = 2 * (i % 4);
            v[i / 4] |= pack_trit2(t) << shift;
        }
        v
    }

    /// Эталонный обход по дугам — сравнение с SWAR-путём.
    fn reference(a: &[Trit], b: &[Trit], eps: f64) -> (Vec<Trit>, ArchetypeStats) {
        let d = a.len();
        let mut out = vec![Trit::Zero; d];
        let (mut na, mut nb, mut co, mut res) = (0, 0, 0, 0);
        for i in 0..d {
            out[i] = archetype_trit(a[i], b[i]);
            if a[i] != Trit::Zero {
                na += 1;
            }
            if b[i] != Trit::Zero {
                nb += 1;
            }
            if a[i] != Trit::Zero && b[i] != Trit::Zero {
                co += 1;
                if a[i] == b[i] {
                    res += 1;
                }
            }
        }
        let energy = archetype_energy(co, na, nb);
        let resonant = energy >= eps;
        if !resonant {
            out.iter_mut().for_each(|t| *t = Trit::Zero);
        }
        let nnz_out = if resonant {
            out.iter().filter(|&&t| t != Trit::Zero).count()
        } else {
            0
        };
        (
            out,
            ArchetypeStats {
                nnz_a: na,
                nnz_b: nb,
                co_support: co,
                resonance: res,
                conflict: co - res,
                energy,
                resonant,
                nnz_out,
            },
        )
    }

    /// Детерминированный ГПСЧ (xorshift64) для решёток.
    struct Xor(u64);
    impl Xor {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn trit(&mut self) -> Trit {
            match self.next() % 3 {
                0 => Trit::Neg,
                1 => Trit::Zero,
                _ => Trit::Pos,
            }
        }
    }

    // ===================== Таблица =====================

    #[test]
    fn lut_is_saturated_interference() {
        // Прозрачность: суперпозиция пропускает чужую структуру.
        assert_eq!(archetype_trit(Trit::Zero, Trit::Zero), Trit::Zero);
        assert_eq!(archetype_trit(Trit::Zero, Trit::Pos), Trit::Pos);
        assert_eq!(archetype_trit(Trit::Zero, Trit::Neg), Trit::Neg);
        assert_eq!(archetype_trit(Trit::Pos, Trit::Zero), Trit::Pos);
        assert_eq!(archetype_trit(Trit::Neg, Trit::Zero), Trit::Neg);
        // Резонанс: согласие полюсов выживает.
        assert_eq!(archetype_trit(Trit::Pos, Trit::Pos), Trit::Pos);
        assert_eq!(archetype_trit(Trit::Neg, Trit::Neg), Trit::Neg);
        // Аннигиляция: встречные полюса гаснут в открытый вопрос.
        assert_eq!(archetype_trit(Trit::Pos, Trit::Neg), Trit::Zero);
        assert_eq!(archetype_trit(Trit::Neg, Trit::Pos), Trit::Zero);
    }

    #[test]
    fn lut_codes_are_v2_valid() {
        // Никаких зарезервированных кодов 0b11 в таблице.
        for row in ARCHETYPE_LUT {
            for code in row {
                assert!(code <= 2, "код {code} вне v2");
            }
        }
    }

    // ===================== Инварианты ТЗ =====================

    #[test]
    fn idempotency_bitwise_random_lattices() {
        // ТЗ п.3: a ⊗_ε a = a БИТ-В-БИТ — 0 галлюцинаций. Любая решётка,
        // любой допустимый ε: произведение архетипа с самим собой —
        // он сам (диагональ таблицы тождественна, E = 1 ≥ ε; пустой
        // архетит — тривиально: out = a = Zero, но гейт заперт).
        for seed in [1u64, 7, 42, 0xDEAD_BEEF] {
            for &eps in &[0.01, 0.25, 0.5, 0.9, 1.0] {
                let mut rng = Xor(seed);
                for &d in &[1usize, 2, 3, 4, 5, 7, 8, 16, 33] {
                    let ts: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
                    let a = pack(&ts);
                    let mut out = vec![0u8; a.len()];
                    let st =
                        archetype_product_packed4(&a, &a, d, eps, &mut out).unwrap();
                    assert_eq!(out, a, "идемпотентность нарушена (seed={seed})");
                    if st.nnz_a > 0 {
                        // Непустой архетип резонирует с собой: E = 1.
                        assert!(st.resonant);
                        assert_eq!(st.energy, 1.0);
                        assert_eq!(st.nnz_out, st.nnz_a);
                    } else {
                        // Пустой: нечему резонировать, но Zero ⊗ Zero = Zero.
                        assert!(!st.resonant);
                        assert_eq!(st.nnz_out, 0);
                    }
                }
            }
        }
    }

    #[test]
    fn orthogonal_archetypes_give_zero() {
        // ТЗ п.3: непересекающиеся носители → произведение Zero,
        // гейт заперт — ложных ассоциаций не существует.
        let a = pack(&[Trit::Pos, Trit::Pos, Trit::Zero, Trit::Zero]);
        let b = pack(&[Trit::Zero, Trit::Zero, Trit::Neg, Trit::Pos]);
        let mut out = vec![0u8; 1];
        let st = archetype_product_packed4(&a, &b, 4, 0.5, &mut out).unwrap();
        assert_eq!(out, vec![0u8; 1], "ортогональные архетипы дали смысл");
        assert!(!st.resonant);
        assert_eq!(st.energy, 0.0);
        assert_eq!(st.nnz_out, 0);
        assert_eq!(st.co_support, 0);
    }

    #[test]
    fn empty_archetype_is_absorbing_zero() {
        // Пустой архетип (все Zero): интерферировать нечем — E = 0,
        // гейт заперт, произведение нуль. Идемпотентность тривиальна.
        let a = vec![0u8; 2];
        let b = pack(&[
            Trit::Pos,
            Trit::Neg,
            Trit::Pos,
            Trit::Neg,
            Trit::Pos,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
        ]);
        let mut out = vec![0u8; 2];
        let st = archetype_product_packed4(&a, &b, 8, 0.5, &mut out).unwrap();
        assert_eq!(out, vec![0u8; 2]);
        assert!(!st.resonant);
        assert_eq!(st.energy, 0.0);
    }

    #[test]
    fn commutativity_bitwise() {
        // a ⊗ b = b ⊗ a: таблица симметрична, гейт симметричен.
        // (nnz_a/nnz_b законно меняются местами — сравниваем с перестановкой.)
        let mut rng = Xor(99);
        for _ in 0..64 {
            let ts_a: Vec<Trit> = (0..12).map(|_| rng.trit()).collect();
            let ts_b: Vec<Trit> = (0..12).map(|_| rng.trit()).collect();
            let (a, b) = (pack(&ts_a), pack(&ts_b));
            let (mut ab, mut ba) = (vec![0u8; 3], vec![0u8; 3]);
            let s1 = archetype_product_packed4(&a, &b, 12, 0.4, &mut ab).unwrap();
            let s2 = archetype_product_packed4(&b, &a, 12, 0.4, &mut ba).unwrap();
            assert_eq!(ab, ba, "некоммутативно");
            assert_eq!(s1.nnz_b, s2.nnz_a);
            assert_eq!(s1.co_support, s2.co_support);
            assert_eq!(s1.resonance, s2.resonance);
            assert_eq!(s1.conflict, s2.conflict);
            assert_eq!(s1.energy, s2.energy);
            assert_eq!(s1.resonant, s2.resonant);
            assert_eq!(s1.nnz_out, s2.nnz_out);
        }
    }

    // ===================== Гейт =====================

    #[test]
    fn gate_boundary_is_inclusive() {
        // Граница гейта: E ровно на пороге — резонанс есть (≥ ε);
        // чуть выше порога ε — заперто. Конструкция: a и b по 4 полюса,
        // ровно 2 общих → E = 2/4 = 0.5.
        let a = pack(&[
            Trit::Pos,
            Trit::Neg,
            Trit::Pos,
            Trit::Neg,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
        ]);
        let b = pack(&[
            Trit::Pos,
            Trit::Neg,
            Trit::Zero,
            Trit::Zero,
            Trit::Pos,
            Trit::Neg,
            Trit::Zero,
            Trit::Zero,
        ]);
        let mut out = vec![0u8; 2];
        // E = 2/4 = 0.5: гейт включён при ε = 0.5 (граница — резонанс).
        let st = archetype_product_packed4(&a, &b, 8, 0.5, &mut out).unwrap();
        assert_eq!(st.energy, 0.5);
        assert!(st.resonant);
        // E = 0.5 < ε = 0.6: заперто, произведение — Zero.
        let st = archetype_product_packed4(&a, &b, 8, 0.6, &mut out).unwrap();
        assert_eq!(st.energy, 0.5);
        assert!(!st.resonant);
        assert_eq!(out, vec![0u8; 2]);
    }

    #[test]
    fn conflict_annihilates_and_resonance_survives() {
        // Пересечение из трёх дуг: два согласия, один конфликт.
        // a: 3 полюса, b: 4 полюса, co = 3 → E = 3/min(3,4) = 1.0.
        let a = pack(&[
            Trit::Pos,
            Trit::Neg,
            Trit::Pos,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
        ]);
        let b = pack(&[
            Trit::Pos,
            Trit::Neg,
            Trit::Neg,
            Trit::Pos,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
            Trit::Zero,
        ]);
        let mut out = vec![0u8; 2];
        let st = archetype_product_packed4(&a, &b, 8, 0.5, &mut out).unwrap();
        assert_eq!(st.co_support, 3);
        assert_eq!(st.resonance, 2);
        assert_eq!(st.conflict, 1);
        assert_eq!(st.energy, 1.0);
        assert!(st.resonant);
        // Продукт: согласия выжили, конфликт погас, прозрачное прошло.
        let got = unpack_quad(out[0]).unwrap();
        assert_eq!(
            got,
            [Trit::Pos, Trit::Neg, Trit::Zero, Trit::Pos]
        );
    }

    // ===================== SWAR ≡ эталон =====================

    #[test]
    fn swar_matches_reference_random() {
        // Эквивалентность SWAR-пути и по-дугового эталона на случайных
        // решётках: байты продукта и вся статистика совпадают.
        let mut rng = Xor(0xC0FFEE);
        for &d in &[1usize, 2, 3, 4, 5, 8, 9, 15, 16, 31, 64] {
            for &eps in &[0.1, 0.5, 0.75] {
                let ts_a: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
                let ts_b: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
                let (a, b) = (pack(&ts_a), pack(&ts_b));
                let mut out = vec![0u8; a.len()];
                let st =
                    archetype_product_packed4(&a, &b, d, eps, &mut out).unwrap();
                let (ref_ts, ref_st) = reference(&ts_a, &ts_b, eps);
                for i in 0..d {
                    assert_eq!(
                        unpack_quad(out[i / 4]).unwrap()[i % 4],
                        ref_ts[i],
                        "дуга {i}, d={d}, eps={eps}"
                    );
                }
                assert_eq!(st, ref_st, "статистика, d={d}, eps={eps}");
            }
        }
    }

    #[test]
    fn tail_padding_lanes_ignored() {
        // d = 5: хвостовой байт несёт 1 живой лейн и 3 паддинговых;
        // мусор в паддинг-лейнах не влияет ни на продукт, ни на счётчики.
        let mut a = pack(&[Trit::Pos, Trit::Zero, Trit::Zero, Trit::Zero, Trit::Neg]);
        let mut b = pack(&[Trit::Pos, Trit::Zero, Trit::Zero, Trit::Zero, Trit::Pos]);
        a[1] |= 0b1111_1100; // мусор в лейнах 2..3 (дуги 6, 7 — за d)
        b[1] |= 0b1111_1100;
        let mut out = vec![0u8; 2];
        let st = archetype_product_packed4(&a, &b, 5, 0.5, &mut out).unwrap();
        assert_eq!(st.nnz_a, 2);
        assert_eq!(st.nnz_b, 2);
        assert_eq!(st.co_support, 2);
        // Дуга 4: Neg ⊗ Pos = Zero (конфликт), дуга 0: резонанс.
        assert_eq!(st.resonance, 1);
        assert_eq!(st.conflict, 1);
        // Живой лейн 0 хвостового байта — Zero; паддинг обнулён маской.
        assert_eq!(out[1] & 0b0000_0011, 0);
    }

    // ===================== Контракты =====================

    #[test]
    fn contract_violations_rejected() {
        let a = vec![0u8; 1];
        let mut out = vec![0u8; 1];
        // d = 0.
        assert!(matches!(
            archetype_product_packed4(&a, &a, 0, 0.5, &mut out),
            Err(PqcError::EmptyState)
        ));
        // Короткие буферы.
        assert!(matches!(
            archetype_product_packed4(&a, &a, 8, 0.5, &mut out),
            Err(PqcError::LengthMismatch { .. })
        ));
        // ε вне (0, 1]: нуль, отрицательный, NaN, бесконечность, > 1.
        for bad in [0.0, -0.5, f64::NAN, f64::INFINITY, 1.5] {
            assert!(
                archetype_product_packed4(&a, &a, 4, bad, &mut out).is_err(),
                "ε = {bad} принят"
            );
        }
    }

    #[test]
    fn out_bytes_beyond_lattice_untouched() {
        // Байты out за ⌈d/4⌉ — чужая память, оператор их не пишет.
        let a = pack(&[Trit::Pos; 4]);
        let mut out = vec![0xFFu8; 4]; // 1 байт решётки + 3 чужих
        archetype_product_packed4(&a, &a, 4, 0.5, &mut out).unwrap();
        assert_eq!(&out[1..], &[0xFFu8; 3]);
        assert_eq!(out[0], pack_quad([Trit::Pos; 4]));
    }
}

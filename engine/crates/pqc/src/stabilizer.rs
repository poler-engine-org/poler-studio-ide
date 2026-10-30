//! # Gottesman–Knill: стабилизаторный симулятор за пределами 26 кубитов
//!
//! Statevector-движок умирает на 2ⁿ·16 байтах (26 кубитов ≈ 1 ГБ);
//! стабилизаторное представление живёт в **n·(n/4) байтах** и работает
//! с любой Clifford-схемой за полином. Это инстанциация «идеального
//! субстрата» для класса Криппендорф… то есть КЛИФФОРД: гейты
//! {H, S, S†, X, Z, CNOT, CZ, SWAP} — ноль ошибок по определению,
//! Gottesman–Knill гарантирует точное классическое моделирование.
//!
//! ## Архитектура (stab-only, без дестабилизаторов)
//!
//! Состояние — n генераторов стабилизаторной группы, каждый строка
//! (x, z, r): оператор P = i^r · X^x Z^z (покубитно X до Z):
//!
//! ```text
//! (x_j, z_j) = (0,0) → I   (1,0) → X   (0,1) → Z   (1,1) → XZ = −iY
//! ```
//!
//! * **Гейты** — сопряжение U·P·U⁻¹ (эволюция состояния), правила
//!   выведены алгебраически в этой конвенции:
//!   H: swap(x,z)|_q, r += 2·x_q·z_q;   S: x_q=1 → z_q⊕=1, r += 1;
//!   X: z_q=1 → r += 2;  Z: x_q=1 → r += 2;
//!   CNOT(c,t): x_t ⊕= x_c, z_c ⊕= z_t (без фазы — доказано для Y_c/Y_t);
//!   CZ/CCZ-подобные — канонической декомпозицией h·cx·h.
//! * **Произведение строк** (rowsum): P_h·P_i = i^{r_h+r_i+2·⟨z_h,x_i⟩}·
//!   P(x_h⊕x_i, z_h⊕z_i) — односторонняя симплектическая форма
//!   (следствие покубитного «X до Z»).
//! * **Измерение Z_q**: строка с x_q = 1 ⟹ антикоммутация ⟹ исход
//!   честно случаен (Борн ½ — точно); иначе исход детерминирован и
//!   читается GF(2)-исключением [X|Z]·c = (0|e_q): фаза произведения
//!   даёт собственное значение. O(n²/64) на детерминированный исход,
//!   O(n²/64) на случайный — полином вместо экспоненты.
//!
//! ## Честные границы
//!
//! * Только Clifford: T-гейт и произвольные вращения — вне класса
//!   (для них — statevector/exact-режимы `qpc`).
//! * ⟨P⟩ для стабилизаторного состояния равно 0 или ±1 — промежуточных
//!   значений не бывает (спектральная теорема); используется как
//!   точный критерий в паритет-тестах против statevector.

use crate::error::{PqcError, Result};
use crate::rng::Rng;

/// Максимальное число кубитов стабилизаторного движка (память
/// n·n/4 байта + 2n фаз: 16384 кубита ≈ 64+ МБ).
pub const MAX_STAB_QUBITS: usize = 16_384;

/// Стабилизаторное состояние: n генераторов группы.
#[derive(Clone)]
pub struct StabilizerState {
    n: usize,
    /// Битовые строки X-части: n строк по W слов.
    x: Vec<u64>,
    /// Битовые строки Z-части.
    z: Vec<u64>,
    /// Фазы r ∈ Z₄: P = i^r·X^x Z^z.
    r: Vec<u8>,
    /// Кэш RREF-исключения [X|Z|I]: (матрица, ведущие строки, слов в строке).
    /// Инвалидируется любой мутацией (гейты, случайные измерения).
    cache: Option<(Vec<u64>, Vec<Option<usize>>, usize)>,
}

#[inline]
fn words_for(n: usize) -> usize {
    n.div_ceil(64)
}

#[inline]
fn bit_get(row: &[u64], q: usize) -> bool {
    (row[q >> 6] >> (q & 63)) & 1 == 1
}

#[inline]
fn bit_flip(row: &mut [u64], q: usize) {
    row[q >> 6] ^= 1u64 << (q & 63);
}

#[inline]
fn bit_set(row: &mut [u64], q: usize, v: bool) {
    let w = q >> 6;
    let mask = 1u64 << (q & 63);
    row[w] = (row[w] & !mask) | (u64::from(v) << (q & 63));
}

/// Маска бит < limit внутри слова k (для обрезки [X|Z]-части).
#[inline]
fn mask_upto(limit: usize, k: usize) -> u64 {
    let lo = k * 64;
    if lo >= limit {
        return 0;
    }
    let hi = (limit - lo).min(64);
    if hi >= 64 {
        u64::MAX
    } else {
        (1u64 << hi) - 1
    }
}

impl StabilizerState {
    /// |0…0⟩: стабилизаторы Z_j.
    pub fn new(n: usize) -> Result<StabilizerState> {
        if n == 0 {
            return Err(PqcError::EmptyState);
        }
        if n > MAX_STAB_QUBITS {
            return Err(PqcError::TooManyQubits {
                requested: n,
                max: MAX_STAB_QUBITS,
            });
        }
        let w = words_for(n);
        let x = vec![0u64; n * w];
        let mut z = vec![0u64; n * w];
        for j in 0..n {
            bit_set(&mut z[j * w..(j + 1) * w], j, true);
        }
        Ok(StabilizerState {
            n,
            x,
            z,
            r: vec![0u8; n],
            cache: None,
        })
    }

    /// Число кубитов.
    pub fn n_qubits(&self) -> usize {
        self.n
    }

    // ── Гейты ───────────────────────────────────────────────────────────

    /// Адамар на кубите q: X↔Z, Y → −Y.
    pub fn h(&mut self, q: usize) {
        self.cache = None;
        self.check_q(q);
        let w = words_for(self.n);
        for i in 0..self.n {
            let xr = &mut self.x[i * w..(i + 1) * w];
            let zr = &mut self.z[i * w..(i + 1) * w];
            let xb = bit_get(xr, q);
            let zb = bit_get(zr, q);
            if xb != zb {
                bit_flip(xr, q);
                bit_flip(zr, q);
            } else if xb && zb {
                // Y → −Y: фаза i^r → i^{r+2}
                self.r[i] = (self.r[i] + 2) & 3;
            }
        }
    }

    /// Фазовый гейт S = diag(1, i): X → Y, Y → −X.
    pub fn s(&mut self, q: usize) {
        self.cache = None;
        self.check_q(q);
        let w = words_for(self.n);
        for i in 0..self.n {
            if bit_get(&self.x[i * w..(i + 1) * w], q) {
                bit_flip(&mut self.z[i * w..(i + 1) * w], q);
                self.r[i] = (self.r[i] + 1) & 3;
            }
        }
    }

    /// S†: X → −Y, Y → X.
    pub fn sdg(&mut self, q: usize) {
        self.cache = None;
        self.check_q(q);
        let w = words_for(self.n);
        for i in 0..self.n {
            if bit_get(&self.x[i * w..(i + 1) * w], q) {
                bit_flip(&mut self.z[i * w..(i + 1) * w], q);
                self.r[i] = (self.r[i] + 3) & 3;
            }
        }
    }

    /// Паули X: Z → −Z.
    pub fn x(&mut self, q: usize) {
        self.cache = None;
        self.check_q(q);
        let w = words_for(self.n);
        for i in 0..self.n {
            if bit_get(&self.z[i * w..(i + 1) * w], q) {
                self.r[i] = (self.r[i] + 2) & 3;
            }
        }
    }

    /// Паули Z: X → −X.
    pub fn z(&mut self, q: usize) {
        self.cache = None;
        self.check_q(q);
        let w = words_for(self.n);
        for i in 0..self.n {
            if bit_get(&self.x[i * w..(i + 1) * w], q) {
                self.r[i] = (self.r[i] + 2) & 3;
            }
        }
    }

    /// CNOT(control, target): x_t ⊕= x_c, z_c ⊕= z_t — без фазы.
    pub fn cx(&mut self, c: usize, t: usize) {
        self.cache = None;
        self.check_q(c);
        self.check_q(t);
        if c == t {
            return;
        }
        let w = words_for(self.n);
        for i in 0..self.n {
            let xr = &mut self.x[i * w..(i + 1) * w];
            let zr = &mut self.z[i * w..(i + 1) * w];
            let xc = bit_get(xr, c);
            let zt = bit_get(zr, t);
            if xc {
                bit_flip(xr, t);
            }
            if zt {
                bit_flip(zr, c);
            }
        }
    }

    /// SWAP(a, b): обмен столбцов — без фазы.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.cache = None;
        self.check_q(a);
        self.check_q(b);
        if a == b {
            return;
        }
        let w = words_for(self.n);
        for i in 0..self.n {
            let xr = &mut self.x[i * w..(i + 1) * w];
            let zr = &mut self.z[i * w..(i + 1) * w];
            for row in [xr, zr] {
                let ba = bit_get(row, a);
                let bb = bit_get(row, b);
                if ba != bb {
                    bit_flip(row, a);
                    bit_flip(row, b);
                }
            }
        }
    }

    /// CZ(a, b) канонической декомпозицией h(b)·cx(a,b)·h(b)
    /// (фазовые правила CZ выписывать рискованно — композиция точна).
    pub fn cz(&mut self, a: usize, b: usize) {
        self.check_q(a);
        self.check_q(b);
        if a == b {
            return;
        }
        self.h(b);
        self.cx(a, b);
        self.h(b);
    }

    // ── Внутренние ──────────────────────────────────────────────────────

    #[inline]
    fn check_q(&self, q: usize) {
        if q >= self.n {
            panic!("stabilizer: qubit {q} out of range [0, {})", self.n);
        }
    }

    /// Произведение строк: row_h := row_h · row_i (операторное).
    /// Фаза: r_h + r_i + 2·⟨z_h, x_i⟩ (mod 4).
    fn rowsum(&mut self, h: usize, i: usize) {
        let w = words_for(self.n);
        let mut phase2: u64 = 0; // число множителей 2 (mod 2 достаточно)
        let (zh, xi) = {
            let zr = &self.z[h * w..(h + 1) * w];
            let xr = &self.x[i * w..(i + 1) * w];
            for k in 0..w {
                phase2 ^= (zr[k] & xr[k]).count_ones() as u64;
            }
            (false, false)
        };
        let _ = (zh, xi);
        for k in 0..w {
            self.x[h * w + k] ^= self.x[i * w + k];
            self.z[h * w + k] ^= self.z[i * w + k];
        }
        self.r[h] = (self.r[h] + self.r[i] + (2 * (phase2 & 1) as u8)) & 3;
    }

    /// Есть ли строка-стабилизатор, антикоммутирующая с Z_q
    /// (x-компонента на кубите q)?
    fn find_anticommuting(&self, q: usize) -> Option<usize> {
        let w = words_for(self.n);
        for i in 0..self.n {
            if bit_get(&self.x[i * w..(i + 1) * w], q) {
                return Some(i);
            }
        }
        None
    }

    // ── Измерение ───────────────────────────────────────────────────────

    /// GF(2)-решатель (с кэшем RREF): найти коэффициенты c ⊆ строк,
    /// дающие целевой вектор (0 | target_z). Возвращает индексы
    /// ИСХОДНЫХ строк — через identity-блок RREF (прямой индекс строки
    /// после перестановок/сложений НЕВЕРЕН). `None` ⟹ цель вне
    /// пространства строк. Кэш перестраивается только после мутаций
    /// (гейты, случайные измерения) — детерминированные запросы
    /// амортизируются: O(n²/64) на запрос против O(n³/64) на重建.
    fn solve_rows(&mut self, target_z: &[u64]) -> Option<Vec<usize>> {
        let n = self.n;
        let w = words_for(n);
        if self.cache.is_none() {
            self.build_cache();
        }
        let (ref aug, ref pivot_row_of_col, full_w) =
            self.cache.as_ref().expect("cache built");
        let bit_at = |v: &[u64], c: usize| -> bool { (v[c >> 6] >> (c & 63)) & 1 == 1 };
        // цель: столбцы [0..2n): X-часть 0, Z-часть target_z → колонка n+q
        let mut t: Vec<u64> = vec![0u64; words_for(2 * n)];
        for k in 0..w {
            let mut bits = target_z[k];
            let base = n + k * 64;
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                let c = base + b;
                t[c >> 6] ^= 1u64 << (c & 63);
                bits &= bits - 1;
            }
        }
        let mut comb: Vec<u64> = vec![0u64; words_for(n)];
        for col in 0..2 * n {
            if bit_at(&t, col) {
                {
                    let p = pivot_row_of_col[col]?;
                    // прибавляем ведущую строку p: [X|Z] к цели, I-блок к c
                    let tw = words_for(2 * n);
                    for k in 0..tw {
                        t[k] ^= aug[p * full_w + k] & mask_upto(2 * n, k);
                    }
                    let ib = &aug[p * full_w..(p + 1) * full_w];
                    for j in 0..n {
                        if bit_at(ib, 2 * n + j) {
                            comb[j >> 6] ^= 1u64 << (j & 63);
                        }
                    }
                }
            }
        }
        let tw = words_for(2 * n);
        for k in 0..tw {
            if t[k] & mask_upto(2 * n, k) != 0 {
                return None;
            }
        }
        let mut rows = Vec::new();
        for j in 0..n {
            if bit_at(&comb, j) {
                rows.push(j);
            }
        }
        Some(rows)
    }

    /// Построить кэш: Гаусс–Жордан над [X | Z | I] (побитовая упаковка
    /// колонок: X → 0..n, Z → n..2n, I → 2n..3n).
    fn build_cache(&mut self) {
        let n = self.n;
        let w = words_for(n);
        let full_w = words_for(3 * n);
        let bit_at = |v: &[u64], c: usize| -> bool { (v[c >> 6] >> (c & 63)) & 1 == 1 };
        let mut aug = vec![0u64; n * full_w];
        for i in 0..n {
            let xr = &self.x[i * w..(i + 1) * w];
            let zr = &self.z[i * w..(i + 1) * w];
            let row = &mut aug[i * full_w..(i + 1) * full_w];
            for q in 0..n {
                if bit_get(xr, q) {
                    row[q >> 6] |= 1u64 << (q & 63);
                }
                if bit_get(zr, q) {
                    let c = n + q;
                    row[c >> 6] |= 1u64 << (c & 63);
                }
            }
            let c = 2 * n + i;
            row[c >> 6] |= 1u64 << (c & 63);
        }
        let mut pivot_row_of_col: Vec<Option<usize>> = vec![None; 2 * n];
        let mut next_row = 0usize;
        for col in 0..2 * n {
            let mut pr = None;
            for i in next_row..n {
                if bit_at(&aug[i * full_w..(i + 1) * full_w], col) {
                    pr = Some(i);
                    break;
                }
            }
            if let Some(p) = pr {
                if p != next_row {
                    for k in 0..full_w {
                        aug.swap(p * full_w + k, next_row * full_w + k);
                    }
                }
                let p = next_row;
                next_row += 1;
                pivot_row_of_col[col] = Some(p);
                for i in 0..n {
                    if i != p && bit_at(&aug[i * full_w..(i + 1) * full_w], col) {
                        for k in 0..full_w {
                            aug[i * full_w + k] ^= aug[p * full_w + k];
                        }
                    }
                }
            }
        }
        self.cache = Some((aug, pivot_row_of_col, full_w));
    }

    /// Произведение выбранных строк (операторно): возвращает (x, z, r).
    /// Порядок неважен: строки — попарно коммутирующие стабилизаторы,
    /// операторное произведение однозначно.
    fn product_rows(&self, rows: &[usize]) -> (Vec<u64>, Vec<u64>, u8) {
        let w = words_for(self.n);
        let mut x = vec![0u64; w];
        let mut z = vec![0u64; w];
        let mut r: u8 = 0;
        let mut have = false;
        for &i in rows {
            if !have {
                x.copy_from_slice(&self.x[i * w..(i + 1) * w]);
                z.copy_from_slice(&self.z[i * w..(i + 1) * w]);
                r = self.r[i];
                have = true;
            } else {
                let mut phase2: u64 = 0;
                for k in 0..w {
                    phase2 ^= (z[k] & self.x[i * w + k]).count_ones() as u64;
                }
                for k in 0..w {
                    x[k] ^= self.x[i * w + k];
                    z[k] ^= self.z[i * w + k];
                }
                r = (r + self.r[i] + (2 * (phase2 & 1) as u8)) & 3;
            }
        }
        (x, z, r)
    }

    /// Измерить кубит q в Z-базисе. Возвращает (исход, был_ли_случайным).
    ///
    /// Случайный исход: Борн-вероятность ровно ½ (антикоммутирующий
    /// стабилизатор обнуляет ⟨Z_q⟩). Детерминированный: GF(2)-исключение
    /// собирает Z_q·(±1) из генераторов — фаза даёт собственное значение.
    pub fn measure(&mut self, q: usize, rng: &mut Rng) -> (u8, bool) {
        self.check_q(q);
        let w = words_for(self.n);
        match self.find_anticommuting(q) {
            Some(p) => {
                // Случайный исход: все прочие строки с x_q = 1 умножаются
                // на строку p (после этого x_q остаётся только у p),
                // затем p := (−1)^b·Z_q. Кэш инвалидируется.
                self.cache = None;
                for i in 0..self.n {
                    if i != p && bit_get(&self.x[i * w..(i + 1) * w], q) {
                        self.rowsum(i, p);
                    }
                }
                let b = (rng.next_u64() & 1) as u8;
                for k in 0..w {
                    self.x[p * w + k] = 0;
                    self.z[p * w + k] = 0;
                }
                bit_set(&mut self.z[p * w..(p + 1) * w], q, true);
                self.r[p] = 2 * b;
                (b, true)
            }
            None => {
                // Детерминированный: Z_q·(±1) = произведение строк;
                // i^r·Z_q |ψ⟩ = |ψ⟩ ⟹ исход = r/2 mod 2.
                let mut tz = vec![0u64; w];
                bit_flip(&mut tz, q);
                let rows = self
                    .solve_rows(&tz)
                    .expect("pure stabilizer state: Z_q in group up to phase");
                let (_x, _z, r) = self.product_rows(&rows);
                debug_assert_eq!(_x.iter().sum::<u64>(), 0, "X-часть должна занулиться");
                debug_assert_eq!(_z, tz, "Z-часть должна быть e_q");
                let b = (r / 2) & 1;
                (b, false)
            }
        }
    }

    /// Измерить все кубиты. Возвращает (исходы, число случайных).
    pub fn measure_all(&mut self, rng: &mut Rng) -> (Vec<u8>, usize) {
        let mut out = Vec::with_capacity(self.n);
        let mut random_count = 0usize;
        for q in 0..self.n {
            let (b, rnd) = self.measure(q, rng);
            out.push(b);
            if rnd {
                random_count += 1;
            }
        }
        (out, random_count)
    }

    /// Точное ожидание ⟨∏_{q ∈ subset} Z_q⟩: стабилизаторное состояние
    /// даёт 0 или ±1 (спектральная теорема). `None` ⟹ 0 (оператор не в
    /// группе — сидит в ненулевой коды, ожидание обнуляется).
    pub fn expect_z_product(&mut self, subset: &[usize]) -> Option<f64> {
        let w = words_for(self.n);
        let mut tz = vec![0u64; w];
        for &q in subset {
            if q >= self.n {
                return None;
            }
            bit_flip(&mut tz, q);
        }
        if subset.is_empty() {
            return Some(1.0);
        }
        let rows = self.solve_rows(&tz)?;
        let (x, z, r) = self.product_rows(&rows);
        // произведение обязано совпасть с целью с точностью до фазы
        for k in 0..w {
            if x[k] != 0 || z[k] != tz[k] {
                return None;
            }
        }
        if r & 1 == 1 {
            return None; // неэрмитово — для совместной системы не бывает
        }
        Some(if (r / 2) & 1 == 1 { -1.0 } else { 1.0 })
    }

    /// Строка-дамп для отладки: (x, z, r) i-й строки.
    pub fn row_debug(&self, i: usize) -> (Vec<bool>, Vec<bool>, u8) {
        let w = words_for(self.n);
        let xb: Vec<bool> = (0..self.n).map(|q| bit_get(&self.x[i * w..(i + 1) * w], q)).collect();
        let zb: Vec<bool> = (0..self.n).map(|q| bit_get(&self.z[i * w..(i + 1) * w], q)).collect();
        (xb, zb, self.r[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statevector::Statevector;

    const TOL: f64 = 1e-12;

    fn sv_expect_z(sv: &Statevector, subset: &[usize]) -> f64 {
        let mut acc = 0.0f64;
        for (i, a) in sv.amplitudes().iter().enumerate() {
            let mut sign = 1.0f64;
            for &q in subset {
                if (i >> q) & 1 == 1 {
                    sign = -sign;
                }
            }
            acc += sign * a.norm_sq();
        }
        acc
    }

    fn apply_sv_gate(sv: &mut Statevector, g: &str, a: usize, b: usize) {
        use crate::gates::Gate;
        let g = match g {
            "h" => Gate::H { q: a },
            "s" => Gate::S { q: a },
            "sdg" => Gate::Sdg { q: a },
            "x" => Gate::X { q: a },
            "z" => Gate::Z { q: a },
            "cx" => Gate::Cx { control: a, target: b },
            "cz" => Gate::Cz { control: a, target: b },
            "swap" => Gate::Swap { a, b },
            _ => unreachable!(),
        };
        sv.apply(g).unwrap();
    }

    /// Паритет стабилизатора и statevector на СЛУЧАЙНЫХ Clifford-схемах:
    /// точные ⟨Z_S⟩ для случайных подмножеств — ноль статистического шума.
    #[test]
    fn parity_with_statevector_random_cliffords() {
        let mut rng = Rng::seed_from_u64(20260921);
        for trial in 0..30 {
            let n = 3 + (rng.next_u64() as usize % 4); // 3..=6
            let mut st = StabilizerState::new(n).unwrap();
            let mut sv = Statevector::new(n).unwrap();
            let depth = 4 + (rng.next_u64() as usize % 5);
            for _ in 0..depth {
                let g = ["h", "s", "sdg", "x", "z", "cx", "cz", "swap"]
                    [(rng.next_u64() as usize) % 8];
                let a = rng.next_u64() as usize % n;
                let mut b = rng.next_u64() as usize % n;
                if b == a {
                    b = (a + 1) % n;
                }
                match g {
                    "h" | "s" | "sdg" | "x" | "z" => {
                        st_apply(&mut st, g, a, b);
                        apply_sv_gate(&mut sv, g, a, b);
                    }
                    two => {
                        st_apply(&mut st, two, a, b);
                        apply_sv_gate(&mut sv, two, a, b);
                    }
                }
            }
            // случайные Z-произведения: точное сравнение
            for _ in 0..40 {
                let mut subset: Vec<usize> = Vec::new();
                for q in 0..n {
                    if rng.next_u64() & 1 == 1 {
                        subset.push(q);
                    }
                }
                let stab = st.expect_z_product(&subset);
                let exact = sv_expect_z(&sv, &subset);
                let stab_val = stab.unwrap_or(0.0);
                assert!(
                    (stab_val - exact).abs() < TOL,
                    "trial {trial}: n={n} subset={subset:?}: stab {stab_val} vs sv {exact}"
                );
                // стабилизаторное ⟨P⟩ ∈ {0, ±1} — спектральная теорема
                assert!(
                    stab_val.abs() < TOL || (stab_val.abs() - 1.0).abs() < TOL,
                    "trial {trial}: |⟨P⟩| = {} ∉ {{0,1}}",
                    stab_val.abs()
                );
            }
        }
    }

    fn st_apply(st: &mut StabilizerState, g: &str, a: usize, b: usize) {
        match g {
            "h" => st.h(a),
            "s" => st.s(a),
            "sdg" => st.sdg(a),
            "x" => st.x(a),
            "z" => st.z(a),
            "cx" => st.cx(a, b),
            "cz" => st.cz(a, b),
            "swap" => st.swap(a, b),
            _ => unreachable!(),
        }
    }

    #[test]
    fn bell_and_ghz_correlations() {
        // Белл: ⟨Z0⟩ = 0, ⟨Z1⟩ = 0, ⟨Z0Z1⟩ = +1, ⟨X0X1⟩ через H-поворот.
        let mut st = StabilizerState::new(2).unwrap();
        st.h(0);
        st.cx(0, 1);
        // Белл: ⟨Z0⟩ = 0 (антикоммутация → None — спектральная теорема:
        // стабилизаторное ⟨P⟩ ∈ {0, ±1}, ноль кодируется None).
        let z0 = st.expect_z_product(&[0]);
        assert!(z0.is_none() || z0 == Some(0.0));
        assert_eq!(st.expect_z_product(&[0, 1]), Some(1.0));

        // GHZ-9: ⟨Z_i⟩ = 0; ⟨Z_i Z_j⟩ = 1 ∀i≠j; ⟨Z_0…Z_8⟩ = 1.
        let n = 9;
        let mut st = StabilizerState::new(n).unwrap();
        st.h(0);
        for q in 1..n {
            st.cx(0, q);
        }
        for i in 0..n {
            assert!(st.expect_z_product(&[i]).is_none());
        }
        for i in 0..n {
            for j in i + 1..n {
                assert_eq!(st.expect_z_product(&[i, j]), Some(1.0), "Z{i}Z{j}");
            }
        }
        // Полное произведение Z_0…Z_8: GHZ-9 НЕЧЁТНОЙ длины —
        // ⟨Z_all⟩ = (1 + (−1)^9)/2 = 0 (спектральная честность).
        let all: Vec<usize> = (0..n).collect();
        assert!(st.expect_z_product(&all).is_none(), "нечётный GHZ: ⟨Z_all⟩ = 0");
        // А для ЧЁТНОЙ длины — стабилизатор: ⟨Z_all⟩ = +1.
        let mut st8 = StabilizerState::new(8).unwrap();
        st8.h(0);
        for q in 1..8 {
            st8.cx(0, q);
        }
        let all8: Vec<usize> = (0..8).collect();
        assert_eq!(st8.expect_z_product(&all8), Some(1.0));
        // Нечётное подмножество (Z_0Z_1Z_2): +1 на |0…0⟩, −1 на |1…1⟩ → 0.
        assert!(st.expect_z_product(&[0, 1, 2]).is_none(), "нечётный срез: 0");
    }

    #[test]
    fn ghz_measurement_all_agree() {
        let n = 64;
        let mut st = StabilizerState::new(n).unwrap();
        st.h(0);
        for q in 1..n {
            st.cx(0, q);
        }
        let mut rng = Rng::seed_from_u64(7);
        let (out, random_count) = st.measure_all(&mut rng);
        assert_eq!(random_count, 1, "GHZ: ровно один случайный исход");
        assert!(out.iter().all(|&b| b == out[0]), "все биты равны");
        // Повторный прогон с тем же сидом — побитово тот же исход.
        let mut st2 = StabilizerState::new(n).unwrap();
        st2.h(0);
        for q in 1..n {
            st2.cx(0, q);
        }
        let mut rng2 = Rng::seed_from_u64(7);
        let (out2, _) = st2.measure_all(&mut rng2);
        assert_eq!(out, out2, "детерминизм сид → исходы");
        // Разные сиды дают оба значения (в среднем 50/50).
        let mut zeros = 0;
        for seed in 0..40 {
            let mut s = StabilizerState::new(n).unwrap();
            s.h(0);
            for q in 1..n {
                s.cx(0, q);
            }
            let mut r = Rng::seed_from_u64(seed);
            let (o, _) = s.measure_all(&mut r);
            if o[0] == 0 {
                zeros += 1;
            }
        }
        assert!(zeros > 10 && zeros < 30, "40 прогонов GHZ: {zeros} нулей");
    }

    #[test]
    fn deterministic_after_collapse() {
        // GHZ: после измерения кубита 0 остальные — детерминированы тем же
        // значением (состояние сколлапсировало в |b…b⟩).
        let n = 24;
        let mut st = StabilizerState::new(n).unwrap();
        st.h(0);
        for q in 1..n {
            st.cx(0, q);
        }
        let mut rng = Rng::seed_from_u64(99);
        let (b0, rnd) = st.measure(0, &mut rng);
        assert!(rnd);
        for q in 1..n {
            let (bq, rnd_q) = st.measure(q, &mut rng);
            assert!(!rnd_q, "кубит {q} должен быть детерминирован");
            assert_eq!(bq, b0, "кубит {q} равен первому");
        }
    }

    #[test]
    fn measurement_matches_statevector_outcomes() {
        // Распределения исходов: стабилизатор vs statevector — по маргиналам
        // Z-произведений (точных, без семплинга) на случайных схемах с
        // промежуточными измерениями не требуется: сравниваем пред- и
        // пост-измерительные ожидания.
        let mut rng = Rng::seed_from_u64(555);
        for _ in 0..20 {
            let n = 4usize;
            let mut st = StabilizerState::new(n).unwrap();
            let mut sv = Statevector::new(n).unwrap();
            for _ in 0..6 {
                let g = ["h", "s", "cx", "cz"][(rng.next_u64() as usize) % 4];
                let a = rng.next_u64() as usize % n;
                let b = (a + 1 + rng.next_u64() as usize % (n - 1)) % n;
                st_apply(&mut st, g, a, b);
                apply_sv_gate(&mut sv, g, a, b);
            }
            // полные распределения: перечисляем 2^n исходов по вероятностям
            // через последовательные условные измерения — тяжело; вместо этого
            // проверяем согласие точных ⟨Z_S⟩ ещё раз после одного измерения.
            let (b0, rnd) = st.measure(0, &mut rng);
            if rnd {
                // коллапс в statevector: проектор на исход b0
                collapse_sv_z(&mut sv, 0, b0);
            }
            for _ in 0..20 {
                let mut subset: Vec<usize> = Vec::new();
                for q in 0..n {
                    if rng.next_u64() & 1 == 1 {
                        subset.push(q);
                    }
                }
                let stab = st.expect_z_product(&subset).unwrap_or(0.0);
                let exact = sv_expect_z(&sv, &subset);
                assert!(
                    (stab - exact).abs() < TOL,
                    "post-measure: stab {stab} vs sv {exact}, subset {subset:?}"
                );
            }
        }
    }

    fn collapse_sv_z(sv: &mut Statevector, q: usize, bit: u8) {
        let mask = 1usize << q;
        let mut acc = 0.0f64;
        for (i, a) in sv.amplitudes_mut().iter_mut().enumerate() {
            if ((i & mask != 0) as u8) != bit {
                *a = crate::complex::Cx::ZERO;
            } else {
                acc += a.norm_sq();
            }
        }
        let k = 1.0 / acc.sqrt();
        for a in sv.amplitudes_mut() {
            *a = a.scale(k);
        }
    }

    #[test]
    fn perf_ghz_2048_and_random_1024() {
        use std::time::Instant;
        // GHZ-2048: построение + полное измерение.
        let n = 2048;
        let t0 = Instant::now();
        let mut st = StabilizerState::new(n).unwrap();
        st.h(0);
        for q in 1..n {
            st.cx(0, q);
        }
        let mut rng = Rng::seed_from_u64(1);
        let (out, random_count) = st.measure_all(&mut rng);
        let dt = t0.elapsed();
        assert_eq!(random_count, 1);
        assert!(out.iter().all(|&b| b == out[0]));
        assert!(dt.as_secs() < 60, "GHZ-2048 слишком медленно: {dt:?}");

        // Случайный Clifford: гейты на 1024, измерение на 512
        // (каждое случайное событие инвалидирует RREF-кэш — честная
        // амортизация задокументирована; худший случай ~n rebuild'ов).
        let n = 1024;
        let t0 = Instant::now();
        let mut st = StabilizerState::new(n).unwrap();
        let mut rng = Rng::seed_from_u64(2);
        for _ in 0..60 * n / 4 {
            let g = (rng.next_u64() as usize) % 4;
            let a = rng.next_u64() as usize % n;
            let b = (a + 1 + rng.next_u64() as usize % 8) % n;
            match g {
                0 => st.h(a),
                1 => st.s(a),
                2 => st.cx(a, b),
                _ => st.cz(a, b),
            }
        }
        let gate_dt = t0.elapsed();
        let mut st = StabilizerState::new(512).unwrap();
        for _ in 0..60 * 512 / 4 {
            let g = (rng.next_u64() as usize) % 4;
            let a = rng.next_u64() as usize % 512;
            let b = (a + 1 + rng.next_u64() as usize % 8) % 512;
            match g {
                0 => st.h(a),
                1 => st.s(a),
                2 => st.cx(a, b),
                _ => st.cz(a, b),
            }
        }
        let (_out, _) = st.measure_all(&mut rng);
        let dt = t0.elapsed();
        assert!(gate_dt.as_secs() < 30, "gates 1024: {gate_dt:?}");
        assert!(dt.as_secs() < 120, "random Clifford + measure: {dt:?}");
    }

    #[test]
    fn cluster_state_line() {
        // Линейный кластер: H везде, CZ по рёбрам; ⟨Z_i Z_{i+1}⟩ = 0,
        // ⟨X_i Z_{i+1} Z_{i-1}⟩ = 1 — проверяем второе через H-поворот.
        let n = 16;
        let mut st = StabilizerState::new(n).unwrap();
        for q in 0..n {
            st.h(q);
        }
        for q in 0..n - 1 {
            st.cz(q, q + 1);
        }
        // Стабилизаторы кластера: X_i·Z_{соседи}. Проверим через поворот
        // осей: измерим в X-базисе кубит i — применим H перед ожиданиями.
        let mut st2 = st.clone();
        st2.h(5);
        // ⟨X_5⟩ = ⟨H Z_5 H⟩ … после H на 5: ⟨X_5⟩_исходного = ⟨Z_5⟩_нового.
        // Для кластера ⟨X_5 Z_4 Z_6⟩ = 1:
        let mut st3 = st.clone();
        st3.h(5);
        // Z_5 нового = X_5 старого; комбинированный оператор X_5 Z_4 Z_6
        // старого = (Z_5 нового)·Z_4·Z_6 — но expect_z_product берёт Z-произведение
        // ТЕКУЩЕГО состояния, а Z_4/Z_6 не менялись от H_5:
        assert_eq!(
            st3.expect_z_product(&[4, 5, 6]),
            Some(1.0),
            "кластер: X_5·Z_4·Z_6 — стабилизатор"
        );
        // А X_5 в одиночку — нет:
        let mut st4 = st.clone();
        st4.h(5);
        assert!(st4.expect_z_product(&[5]).is_none());
    }

    #[test]
    fn bad_args() {
        assert!(StabilizerState::new(0).is_err());
        assert!(StabilizerState::new(MAX_STAB_QUBITS + 1).is_err());
    }
}

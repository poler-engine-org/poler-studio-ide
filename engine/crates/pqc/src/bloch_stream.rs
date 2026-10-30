//! RQ14: поток углов Блоха из Packed4-тритов в обучающий контур.
//!
//! Замыкает круг RQ1 → RQ13: анзац начался с `R_y(arccos p)`, хранилище
//! пришло к 2-битным тритам (Packed4, ×32 компактнее f64) — теперь обучение
//! потребляет сжатое состояние напрямую, разворачивая `θ = arccos(p)`
//! **на лету** через LUT из трёх констант (θ ∈ {0, π/2, π} — решётка
//! точная, FPU-`acos` не нужен вовсе). Это и есть «L1-регистры»: углы
//! живут в кэше как коды, материализуются только в окне обучающего шага.
//!
//! Два потребителя:
//!
//! * [`Packed4Angles`] — чанковый стриминг `(смещение, &[θ])` окна `W`:
//!   полный вектор углов никогда не материализуется (для `d_pol = 2^26`
//!   экономится 512 МиБ RAM против плотного f64);
//! * [`born_step_packed4`] — фазовый шаг Born **прямо в упакованных
//!   байтах**: `θ ← arccos(p) − η·v; p ← cos θ` с переквантованием
//!   ближайшим тритом. Обучение в 2-битных регистрах без распаковки —
//!   трит перещёлкивается, когда `|η·v|` пересекает границу решётки.
//!
//! Инварианты (тесты ниже):
//! * `v = 0` — тождество: точки решётки — неподвижные точки шага
//!   (квантование инерционно, мёртвая зона честно задана границей 0.5);
//! * шаг коммутирует с [`Ansatz::from_packed4`]: обучение «на месте»
//!   эквивалентно обучению материализованного вектора;
//! * стриминг чанками ≡ плотному проходу (по окнам любого размера).

use crate::ansatz::{Ansatz, LoadOptions};
use crate::error::{PqcError, Result};
use pqw::trit_bloch::{fill_thetas, p_at, theta_lut};

/// Чанковый стример углов Блоха из Packed4-байтов.
///
/// Отдаёт окна по `W` углов: `(смещение, срез θ)`. Память — `O(W)`,
/// независимо от `d_pol`; источник может быть mmap-срезом (`&mmap.as_slice()`).
pub struct Packed4Angles<'a> {
    data: &'a [u8],
    d: usize,
    pos: usize,
    window: Vec<f64>,
}

impl<'a> Packed4Angles<'a> {
    /// Стример по `d` тритам из `data` с окном `window` (клампится к ≥1).
    pub fn new(data: &'a [u8], d: usize, window: usize) -> Packed4Angles<'a> {
        let w = window.max(1);
        Packed4Angles {
            data,
            d,
            pos: 0,
            window: vec![0.0; w],
        }
    }

    /// Следующее окно `(смещение, θ)`; `None` — поток исчерпан.
    pub fn next_chunk(&mut self) -> Option<(usize, &[f64])> {
        if self.pos >= self.d {
            return None;
        }
        let filled = fill_thetas(self.data, self.pos, &mut self.window);
        if filled == 0 {
            return None;
        }
        let offset = self.pos;
        self.pos += filled;
        Some((offset, &self.window[..filled]))
    }

    /// Сколько тритов уже отдано.
    pub fn consumed(&self) -> usize {
        self.pos
    }

    /// Всего тритов в потоке.
    pub fn total(&self) -> usize {
        self.d
    }
}

/// Статистика квантованного Born-шага — «обучение произошло» в числах.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedBornStats {
    /// Тритов, сменивших код (реальные движения по решётке).
    pub moved: usize,
    /// Всего тритов.
    pub total: usize,
    /// Суммарный «плавающий» сдвиг `Σ|θ' − θ|` до переквантования —
    /// отличен от нуля даже когда решётка ещё держит трит (мёртвая зона).
    pub theta_shift: f64,
    /// Итоговые счётчики кодов (pos, neg, zero).
    pub pos: usize,
    pub neg: usize,
    pub zero: usize,
}

impl PackedBornStats {
    /// Доля перещёлкнувшихся тритов.
    pub fn moved_frac(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.moved as f64 / self.total as f64
        }
    }
}

/// Фазовый шаг Born прямо в Packed4-байтах (ядро RQ14).
///
/// `θ_i ← arccos(p_i) − η·v_i`, `p_i ← cos θ_i`, переквантование ближайшим
/// тритом (`|p| ≥ 0.5 → sign`, иначе Zero). Углы старта/финиша — LUT,
/// `cos` — один вызов на трит только при `v_i ≠ 0`; `v = 0` тождествен.
///
/// Контракт: `bytes.len() ≥ ceil(d/4)`, `grad.len() ≥ d`.
pub fn born_step_packed4(
    bytes: &mut [u8],
    d: usize,
    grad: &[f64],
    eta: f64,
) -> Result<PackedBornStats> {
    if bytes.len() < d.div_ceil(4) {
        return Err(PqcError::LengthMismatch {
            expected: d.div_ceil(4),
            actual: bytes.len(),
        });
    }
    if grad.len() < d {
        return Err(PqcError::LengthMismatch {
            expected: d,
            actual: grad.len(),
        });
    }
    let lut = theta_lut();
    let mut stats = PackedBornStats {
        moved: 0,
        total: d,
        theta_shift: 0.0,
        pos: 0,
        neg: 0,
        zero: 0,
    };
    for i in 0..d {
        let byte = bytes[i / 4];
        let shift = 2 * (i % 4);
        let code = ((byte >> shift) & 0b11) as usize;
        let theta = lut[code];
        let v = grad[i];
        let (new_code, delta) = if v == 0.0 {
            (code, 0.0)
        } else {
            let theta_next = theta - eta * v;
            let p_next = theta_next.cos();
            let trit = pqw::phase::nearest_trit(p_next as f32);
            let next = pqw::phase::pack_trit2(trit) as usize;
            (next, (theta_next - theta).abs())
        };
        stats.theta_shift += delta;
        if new_code != code {
            bytes[i / 4] = (byte & !(0b11 << shift)) | ((new_code as u8) << shift);
            stats.moved += 1;
        }
        match new_code {
            1 => stats.pos += 1,
            2 => stats.neg += 1,
            _ => stats.zero += 1,
        }
    }
    Ok(stats)
}

/// Разреженный фазовый шаг Born в Packed4-байтах (ядро RQ15).
///
/// Математика идентична [`born_step_packed4`], но применяется **только к
/// перечисленным координатам** `vel` (Π_Λ-носитель шага): остальная
/// решётка не читается и не пишется. Это режим квантованного curriculum:
/// свидетельство чанка разрежено, фон решётки неприкосновенен.
///
/// Инвариант: на носителе результат совпадает с плотным шагом при
/// градиенте, равном нулю вне носителя (тест ниже).
///
/// Контракт: `bytes.len() ≥ ceil(d/4)`, `i < d` для всех пар, координаты
/// в `vel` уникальны. Статистика считается по носителю: `total = vel.len()`
/// (`moved` — подмножество носителя; полные счётчики решётки —
/// [`pqw::trit_bloch::counts`]).
pub fn born_step_packed4_sparse(
    bytes: &mut [u8],
    d: usize,
    vel: &[(u32, f64)],
    eta: f64,
) -> Result<PackedBornStats> {
    if bytes.len() < d.div_ceil(4) {
        return Err(PqcError::LengthMismatch {
            expected: d.div_ceil(4),
            actual: bytes.len(),
        });
    }
    let lut = theta_lut();
    let mut stats = PackedBornStats {
        moved: 0,
        total: vel.len(),
        theta_shift: 0.0,
        pos: 0,
        neg: 0,
        zero: 0,
    };
    for &(i, v) in vel {
        let i = i as usize;
        if i >= d {
            return Err(PqcError::BadArc {
                index: i as u32,
                d_pol: d as u32,
            });
        }
        let byte = bytes[i / 4];
        let shift = 2 * (i % 4);
        let code = ((byte >> shift) & 0b11) as usize;
        let theta = lut[code];
        let (new_code, delta) = if v == 0.0 {
            (code, 0.0)
        } else {
            let theta_next = theta - eta * v;
            let p_next = theta_next.cos();
            let trit = pqw::phase::nearest_trit(p_next as f32);
            let next = pqw::phase::pack_trit2(trit) as usize;
            (next, (theta_next - theta).abs())
        };
        stats.theta_shift += delta;
        if new_code != code {
            bytes[i / 4] = (byte & !(0b11 << shift)) | ((new_code as u8) << shift);
            stats.moved += 1;
        }
        match new_code {
            1 => stats.pos += 1,
            2 => stats.neg += 1,
            _ => stats.zero += 1,
        }
    }
    Ok(stats)
}

/// Анзац прямо из Packed4-байтов (без материализации p-вектора для
/// product-пути): SV-путь заполняет окно из [`fill_thetas`]-аналога для p,
/// product-путь собирает дуги из ненулевых тритов.
///
/// Это конструктор [`Ansatz`] для mmap-хранилищ v2: `bytes` — срез
/// `phase_bytes()` читателя (или шифр-блока RQ13 — коды те же).
pub fn ansatz_from_packed4(
    bytes: &[u8],
    d_pol: u32,
    opts: &LoadOptions,
) -> Result<Ansatz> {
    let d = d_pol as usize;
    if bytes.len() < d.div_ceil(4) {
        return Err(PqcError::LengthMismatch {
            expected: d.div_ceil(4),
            actual: bytes.len(),
        });
    }
    if d <= opts.max_sv_qubits {
        // Плотный p-вектор: p = cos θ по LUT, без acos.
        let mut ps = vec![0.0_f64; d];
        for (i, slot) in ps.iter_mut().enumerate() {
            *slot = p_at(bytes, i);
        }
        Ansatz::from_phases(&ps, opts)
    } else {
        // LENS: дуги — только ненулевые триты; p дуги = cos θ = ±1 точно.
        let arcs: Vec<(u32, f64)> = pqw::trit_bloch::BlochArcs::new(bytes, d_pol)
            .map(|(i, theta)| (i, theta.cos()))
            .collect();
        Ok(Ansatz::Product(crate::ansatz::PhaseAnsatz::new(
            d_pol, arcs,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::phase::{pack_quad, Trit};
    use pqw::trit_bloch::trit_at;

    fn sample_bytes() -> [u8; 2] {
        [
            pack_quad([Trit::Pos, Trit::Neg, Trit::Zero, Trit::Pos]),
            pack_quad([Trit::Neg, Trit::Zero, Trit::Zero, Trit::Neg]),
        ]
    }

    #[test]
    fn stream_chunks_any_window() {
        let data = sample_bytes();
        for w in [1usize, 2, 3, 4, 8, 16] {
            let mut stream = Packed4Angles::new(&data, 8, w);
            let mut collected: Vec<(usize, f64)> = Vec::new();
            while let Some((off, thetas)) = stream.next_chunk() {
                for (k, &t) in thetas.iter().enumerate() {
                    collected.push((off + k, t));
                }
            }
            assert_eq!(collected.len(), 8, "window {w}");
            assert_eq!(stream.consumed(), 8);
            assert_eq!(stream.total(), 8);
            // Значения совпадают с LUT-семантикой.
            let expect = [
                0.0,
                core::f64::consts::PI,
                core::f64::consts::FRAC_PI_2,
                0.0,
                core::f64::consts::PI,
                core::f64::consts::FRAC_PI_2,
                core::f64::consts::FRAC_PI_2,
                core::f64::consts::PI,
            ];
            for (k, (i, t)) in collected.iter().enumerate() {
                assert_eq!(*i as usize, k, "window {w}, pos {k}");
                assert!((t - expect[k]).abs() < 1e-15, "window {w}, idx {i}");
            }
        }
    }

    #[test]
    fn zero_grad_is_identity_on_lattice() {
        let mut bytes = sample_bytes();
        let grad = [0.0_f64; 8];
        let stats = born_step_packed4(&mut bytes, 8, &grad, 0.1).unwrap();
        assert_eq!(stats.moved, 0);
        assert_eq!(stats.theta_shift, 0.0);
        assert_eq!((stats.pos, stats.neg, stats.zero), (2, 3, 3));
        assert_eq!(bytes, sample_bytes());
    }

    #[test]
    fn gradient_flips_trits_across_boundary() {
        // Начало: [Zero, Zero, Pos, Pos | Zero, Zero, Zero, Zero].
        let mut bytes = [pack_quad([Trit::Zero; 4]), 0u8];
        // Сильный положительный градиент при большом η понижает θ на Zero:
        // θ = π/2 → θ' = π/2 − η·v; cos θ' ≥ 0.5 при η·v ≥ π/3 ≈ 1.047.
        let grad = [2.0_f64; 8];
        let stats = born_step_packed4(&mut bytes, 8, &grad, 1.0).unwrap();
        // η·v = 2 > π/3 → все Zero перещёлкнулись в Pos.
        assert_eq!(stats.moved, 8);
        assert_eq!(stats.pos, 8);
        assert_eq!(stats.zero, 0);
        assert!(stats.theta_shift > 8.0 * 1.0);
        // Обратный знак — в Neg: θ' = π/2 + 2 → cos < −0.5.
        let mut bytes2 = [pack_quad([Trit::Zero; 4]), 0u8];
        let grad_neg = [-2.0_f64; 8];
        let s2 = born_step_packed4(&mut bytes2, 8, &grad_neg, 1.0).unwrap();
        assert_eq!(s2.neg, 8);
        assert_eq!(s2.zero, 0);
    }

    #[test]
    fn dead_zone_small_eta_keeps_lattice() {
        // Слабый градиент: η·v = 0.1 < π/3 — Zero остаётся Zero
        // (честная мёртвая зона квантования), Pos/Neg держатся ещё шире.
        let mut bytes = sample_bytes();
        let grad = [1.0_f64; 8];
        let stats = born_step_packed4(&mut bytes, 8, &grad, 0.1).unwrap();
        assert_eq!(stats.moved, 0);
        assert_eq!(bytes, sample_bytes());
        // Но плавающий сдвиг накопился — решётка «давит», не пуская.
        assert!(stats.theta_shift > 0.0);
        assert_eq!(stats.moved_frac(), 0.0);
    }

    #[test]
    fn contract_violations_are_errors() {
        let mut bytes = [0u8; 1];
        assert!(born_step_packed4(&mut bytes, 8, &[0.0; 8], 0.1).is_err());
        let mut ok = [0u8; 2];
        assert!(born_step_packed4(&mut ok, 8, &[0.0; 4], 0.1).is_err());
    }

    #[test]
    fn sparse_step_matches_dense_on_support() {
        // Носитель — подмножество координат; вне носителя градиент нулевой.
        // Разреженный шаг обязан совпадать с плотным на носителе и не
        // трогать фон.
        let data = sample_bytes();
        let support = [(0u32, 2.0_f64), (3, -2.0), (7, 0.5)];
        let mut sparse = data;
        let s = born_step_packed4_sparse(&mut sparse, 8, &support, 0.6).unwrap();
        let mut dense = data;
        let mut dense_grad = [0.0_f64; 8];
        for &(i, v) in &support {
            dense_grad[i as usize] = v;
        }
        let dstat = born_step_packed4(&mut dense, 8, &dense_grad, 0.6).unwrap();
        assert_eq!(sparse, dense, "решётки разошлись");
        // Статистика: по носителю против полной.
        assert_eq!(s.moved, dstat.moved);
        assert_eq!(s.total, support.len());
        assert!((s.theta_shift - dstat.theta_shift).abs() < 1e-12);
        // Пустой носитель — тождество.
        let mut same = data;
        let z = born_step_packed4_sparse(&mut same, 8, &[], 0.6).unwrap();
        assert_eq!(z.moved, 0);
        assert_eq!(z.total, 0);
        assert_eq!(same, data);
    }

    #[test]
    fn sparse_step_pole_hysteresis_no_teleport() {
        // Физика решётки (RQ15): полюс под инвертирующим импульсом уходит
        // в Zero (экватор), но НЕ телепортируется в противоположный полюс.
        // Смена знака — только через честную монету: Pos → Zero → Neg.
        let mut bytes = [pack_quad([Trit::Pos; 4]), 0u8];
        // p_emp = +1, target = −1 → g = 2 → v = −2, η·|v| = 1.2 > π/3:
        // полюс покидается, но cos(1.2) ≈ 0.36 > −0.5 → Zero, не Neg.
        let s1 = born_step_packed4_sparse(&mut bytes, 8, &[(0, -2.0)], 0.6).unwrap();
        assert_eq!(s1.moved, 1);
        assert_eq!(trit_at(&bytes, 0), Trit::Zero);
        // Второй импульс с экватора: θ = π/2 − 1.2 → cos < −0.5 → Neg.
        let s2 = born_step_packed4_sparse(&mut bytes, 8, &[(0, -2.0)], 0.6).unwrap();
        assert_eq!(s2.moved, 1);
        assert_eq!(trit_at(&bytes, 0), Trit::Neg);
        // Прямая телепортация недостижима одним шагом: даже |v| = 3
        // с экватора даёт cos(π/2 − 1.8) ≈ −0.97 — верно, но с ПОЛЮСА
        // θ = 0 + 1.8 → cos ≈ −0.23 → Zero (не Neg): граница 2π/3.
        let mut pole = [pack_quad([Trit::Pos; 4]), 0u8];
        let s3 = born_step_packed4_sparse(&mut pole, 8, &[(0, -3.0)], 0.6).unwrap();
        assert_eq!(trit_at(&pole, 0), Trit::Zero);
        assert_eq!(s3.moved, 1);
    }

    #[test]
    fn sparse_contract_violations_are_errors() {
        let mut bytes = [0u8; 2];
        // Индекс за пределами d_pol.
        assert!(born_step_packed4_sparse(&mut bytes, 8, &[(8, 1.0)], 0.6).is_err());
        // Байтов меньше ceil(d/4).
        let mut short = [0u8; 1];
        assert!(born_step_packed4_sparse(&mut short, 8, &[(0, 1.0)], 0.6).is_err());
    }

    #[test]
    fn ansatz_sv_path_from_packed4() {
        // d = 8 ≤ max_sv_qubits → statevector; p-вектор из LUT ≡ плотному.
        let data = sample_bytes();
        let az = ansatz_from_packed4(&data, 8, &LoadOptions::default()).unwrap();
        assert_eq!(az.engine(), crate::ansatz::Engine::Statevector);
        // Эквивалентность плотному p-вектору: [1, −1, 0, 1 | −1, 0, 0, −1].
        let dense = [1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0];
        let az2 = Ansatz::from_phases(&dense, &LoadOptions::default()).unwrap();
        // Одинаковые состояния + одинаковые seed → идентичные отчёты.
        let mut rng1 = crate::rng::Rng::seed_from_u64(42);
        let mut rng2 = crate::rng::Rng::seed_from_u64(42);
        let s1 = az.sample(&mut rng1, 4096, 4).unwrap();
        let s2 = az2.sample(&mut rng2, 4096, 4).unwrap();
        assert_eq!(s1.engine, s2.engine);
        assert_eq!(s1.d_pol, s2.d_pol);
        assert_eq!(s1.top, s2.top);
    }

    #[test]
    fn ansatz_product_path_from_packed4() {
        // d = 8 > max_sv_qubits = 4 → product; дуги = ненулевые триты.
        let data = sample_bytes();
        let opts = LoadOptions {
            purify_steps: 0,
            max_sv_qubits: 4,
            verify_payload: false,
        };
        let az = ansatz_from_packed4(&data, 8, &opts).unwrap();
        assert_eq!(az.engine(), crate::ansatz::Engine::Product);
        match &az {
            Ansatz::Product(pa) => {
                assert_eq!(pa.d_pol(), 8);
                assert_eq!(pa.nnz(), 5); // [P, N, 0, P | N, 0, 0, N]
                let thetas = pa.thetas();
                // p = ±1 → θ = 0 или π.
                assert!((thetas[0].1).abs() < 1e-12);
                assert!((thetas[1].1 - core::f64::consts::PI).abs() < 1e-12);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn stream_over_cipher_block_codes() {
        // RQ13-совместимость: GF(3)-коды {0,1,2} шифр-блока — те же биты,
        // что и v2-коды {Zero,Pos,Neg}: разворот в углы идентичен.
        let mut block = [0u8; 2];
        // код 1 (Pos) в тритах 0..4, код 2 (Neg) в тритах 4..8.
        block[0] = 0b01_01_01_01;
        block[1] = 0b10_10_10_10;
        let mut stream = Packed4Angles::new(&block, 8, 4);
        let mut angles: Vec<f64> = Vec::new();
        while let Some((_, w)) = stream.next_chunk() {
            angles.extend_from_slice(w);
        }
        assert!(angles[..4].iter().all(|&t| t == 0.0));
        assert!(angles[4..].iter().all(|&t| (t - core::f64::consts::PI).abs() < 1e-15));
    }
}

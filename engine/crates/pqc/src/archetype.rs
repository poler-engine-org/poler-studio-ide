//! Уравнение архетипа (RQ11): `a ⊗_ε a = a` и `p* = a ⊗_ε p*`.
//!
//! Алгебра смыслов `(O, ⊕, ⊗_ε)` в матричной реализации движка —
//! строго, без метафизики:
//!
//! 1. **Идемпотентность** `a ⊗_ε a = a`. Нетривиальные идемпотенты
//!    существуют конструктивно: McWeeny-поток `P ← 3P² − 2P³`
//!    (инвариант контейнера) и спектральные проекторы мод гироскопа
//!    `A_k = u_k u_kᵀ + v_k v_kᵀ` (см. [`crate::gyro`]). Ортонормальность
//!    `(u, v)` тождественна идемпотентности `A² = A`.
//! 2. **Фиксация** `p* = a ⊗_ε p*`. Неподвижные точки транспорта фаз —
//!    конфигурации с погашенными моментами: `sin(θⱼ − θᵢ) = 0` для
//!    каждой пары J (разности фаз `0` или `π` — русла заперты).
//!    Чистая прецессия унитарна (спектр J чисто мнимый): типичная
//!    траектория — орбита вокруг архетипа, а не спуск. Оседание даёт
//!    проекция McWeeny: чередование «прецессия → очистка» стягивает
//!    фазы на тритовую решётку `{0, π/2, π}` при погашенных моментах —
//!    итоговый контейнер имеет нулевой остаток идемпотентности.
//!
//! Измеримые величины: невязка фиксации (максимальный момент), параметр
//! порядка Курамото `r`, коэффициент сжатия (Банах) и сертификат тритов.

use crate::gyro::precess_step;
use pqw::mcweeny::purify_p;

/// Точка (прореженной) траектории прецессии.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TracePoint {
    /// Номер тика (1-базированный).
    pub tick: u64,
    /// `max |Δθ|` за тик по дугам русел J (включая проекцию McWeeny).
    pub delta: f64,
    /// Параметр порядка Курамото `r` по дугам русел.
    pub r: f64,
    /// Невязка фиксации `max |sin(θⱼ − θᵢ)|` по парам J.
    pub torque: f64,
}

/// Итог петли прецессии — инференс поверх чекпоинта.
#[derive(Clone, Debug)]
pub struct PrecessReport {
    /// Исполнено тиков (меньше лимита при ранней фиксации).
    pub ticks: u64,
    /// Фиксация достигнута: `max |Δθ|` последнего тика < стоп-порога.
    pub fixated: bool,
    /// Дуг в руслах J.
    pub participants: usize,
    /// `max |Δθ|` первого тика.
    pub delta_first: f64,
    /// `max |Δθ|` последнего тика.
    pub delta_last: f64,
    /// Геометрический средний фактор убывания `max|Δθ|` — измеренный
    /// коэффициент сжатия отображения (критерий Банаха).
    pub contraction: f64,
    /// Параметр порядка Курамото: до → после.
    pub r_initial: f64,
    pub r_final: f64,
    /// Невязка фиксации `max |sin(θⱼ−θᵢ)|`: до → после.
    pub torque_initial: f64,
    pub torque_final: f64,
    /// Средняя подписанная угловая скорость дуг русел (рад/тик) —
    /// скорость маховика в незатухающем режиме.
    pub omega: f64,
    /// Максимальный полный пробег дуги (рад, без приведения к 2π).
    pub travel_max: f64,
    /// Средний полный пробег дуги (рад).
    pub travel_mean: f64,
    /// Все дуги легли на тритовую решётку `θ ∈ {0, π/2, π}`
    /// (`|cos θ| ∈ {0, 1}` с допуском `1e-9`) — сертификат
    /// идемпотентности итогового состояния.
    pub trits_exact: bool,
    /// Траектория (прореженная при длинных прогонах, ≤ ~4096 точек).
    pub trace: Vec<TracePoint>,
}

/// Допуск сертификата тритов: `|cos θ|` в `{0, 1}` ± TRIT_TOL.
const TRIT_TOL: f64 = 1e-9;
/// Потолок точек траектории (прореживание при длинных прогонах).
const TRACE_CAP: usize = 4096;

/// Дуги русел J: уникальные индексы пар, по возрастанию.
fn participants(pairs: &[(u32, u32, f64)]) -> Vec<u32> {
    let mut nodes: Vec<u32> = pairs
        .iter()
        .flat_map(|&(i, j, _)| [i, j])
        .collect();
    nodes.sort_unstable();
    nodes.dedup();
    nodes
}

/// Параметр порядка Курамото `r = |Σ e^{iθ}| / N` по дугам русел.
fn order_parameter(parts: &[u32], thetas: &[f64]) -> f64 {
    if parts.is_empty() {
        return 0.0;
    }
    let (mut re, mut im) = (0.0_f64, 0.0_f64);
    for &i in parts {
        let t = thetas[i as usize];
        re += t.cos();
        im += t.sin();
    }
    (re * re + im * im).sqrt() / parts.len() as f64
}

/// Невязка фиксации: `max |sin(θⱼ − θᵢ)|` по парам J — масштабно
/// инвариантна (синус), поэтому нормировка весов не важна.
fn torque_residual(pairs: &[(u32, u32, f64)], thetas: &[f64]) -> f64 {
    pairs
        .iter()
        .map(|&(i, j, _)| ((thetas[j as usize] - thetas[i as usize]).sin()).abs())
        .fold(0.0_f64, f64::max)
}

/// Один шаг McWeeny-очистки в θ-представлении: `p = cos θ` проходит
/// поток идемпотентности `p ← 2(3λ² − 2λ³) − 1`, `λ = (1+p)/2`, затем
/// фаза восстанавливается `θ ← arccos p'` в `[0, π]`.
///
/// Проекция сбрасывает накопленное вращение (θ → [0, π]) — это и есть
/// диссипативная половина петли: прецессия исследует орбиту, очистка
/// стягивает на многообразие идемпотентности.
pub(crate) fn purify_thetas(thetas: &mut [f64]) {
    for t in thetas.iter_mut() {
        let p = purify_p(t.cos().clamp(-1.0, 1.0));
        *t = p.clamp(-1.0, 1.0).acos();
    }
}

/// Петля прецессии памяти: итерация уравнения фиксации `p* = a ⊗_ε p*`.
///
/// Тики: транспорт фаз [`precess_step`] на нормированных весах
/// (`w ← w / max|w|` — скорость вращения задаёт `eta`, а не масштаб
/// циркуляции); каждые `purify_every` тиков — проекция McWeeny
/// (`purify_every = 0` — чистая унитарная прецессия, орбита без
/// диссипации). Стоп: `max |Δθ|` за тик < `stop_delta` (фиксация)
/// либо лимит `max_ticks`.
///
/// `thetas` — углы Блоха всех `d_pol` дуг (радианы), длина обязана
/// превышать максимальный индекс пар. Петля детерминирована.
pub fn precess_to_fixpoint(
    pairs: &[(u32, u32, f64)],
    thetas: &mut [f64],
    eta: f64,
    max_ticks: u64,
    stop_delta: f64,
    purify_every: u64,
) -> PrecessReport {
    let parts = participants(pairs);
    let r_initial = order_parameter(&parts, thetas);
    let torque_initial = torque_residual(pairs, thetas);
    let theta0: Vec<f64> = parts.iter().map(|&i| thetas[i as usize]).collect();

    // Нормировка весов: сильнейшее русло задаёт масштаб 1.
    let w_max = pairs
        .iter()
        .map(|&(_, _, w)| w.abs())
        .fold(0.0_f64, f64::max);
    let scaled: Vec<(u32, u32, f64)> = if w_max > 0.0 {
        pairs.iter().map(|&(i, j, w)| (i, j, w / w_max)).collect()
    } else {
        Vec::new()
    };

    let mut trace: Vec<TracePoint> = Vec::new();
    // Двухфазное прореживание: первые TRACE_FINE тиков — все точки
    // (ранняя фиксация не должна терять разрешение), дальше — шаг,
    // заполняющий оставшийся бюджет до ~TRACE_CAP точек.
    let fine = TRACE_CAP / 16; // 256 первых тиков без прореживания
    let stride = if max_ticks as usize > TRACE_CAP {
        let coarse = TRACE_CAP - fine;
        let rest = (max_ticks as usize).saturating_sub(fine);
        1.max(rest / coarse.max(1))
    } else {
        1
    };
    let push_trace = |trace: &mut Vec<TracePoint>, tick: u64, delta: f64, r: f64, t: f64| {
        let sample = tick as usize <= fine || (tick as usize - fine).is_multiple_of(stride);
        if sample && trace.len() < TRACE_CAP {
            trace.push(TracePoint { tick, delta, r, torque: t });
        }
    };

    let mut fixated = false;
    let mut delta_last = 0.0_f64;
    let mut tick: u64 = 0;
    for t in 1..=max_ticks {
        let before: Vec<f64> = parts.iter().map(|&i| thetas[i as usize]).collect();
        precess_step(&scaled, thetas, eta);
        if purify_every > 0 && t % purify_every == 0 {
            purify_thetas(thetas);
        }
        let mut delta = 0.0_f64;
        for (&i, &b) in parts.iter().zip(before.iter()) {
            delta = delta.max((thetas[i as usize] - b).abs());
        }
        tick = t;
        delta_last = delta;
        let r = order_parameter(&parts, thetas);
        let torque = torque_residual(pairs, thetas);
        push_trace(&mut trace, t, delta, r, torque);
        if delta < stop_delta {
            fixated = true;
            break;
        }
    }
    // Гарантируем последнюю точку в трейсе (прореживание могло пропустить).
    if tick > 0 {
        if let Some(last) = trace.last() {
            if last.tick != tick {
                trace.push(TracePoint {
                    tick,
                    delta: delta_last,
                    r: order_parameter(&parts, thetas),
                    torque: torque_residual(pairs, thetas),
                });
            }
        }
    }

    let delta_first = trace.first().map(|p| p.delta).unwrap_or(0.0);
    let r_final = order_parameter(&parts, thetas);
    let torque_final = torque_residual(pairs, thetas);

    // Коэффициент сжатия: геометрический средний фактор убывания
    // max|Δθ| от первого тика до последнего ненулевого.
    let last_nonzero = trace
        .iter()
        .rposition(|p| p.delta > 0.0)
        .map(|ix| trace[ix])
        .unwrap_or(TracePoint { tick: 0, delta: 0.0, r: 0.0, torque: 0.0 });
    let contraction = if delta_first > 0.0 && last_nonzero.tick > 1 {
        (last_nonzero.delta / delta_first).powf(1.0 / (last_nonzero.tick - 1) as f64)
    } else {
        0.0
    };

    // Пробег и средняя угловая скорость (по дугам русел).
    let mut travel_max = 0.0_f64;
    let mut travel_sum = 0.0_f64;
    for (k, &i) in parts.iter().enumerate() {
        let d = (thetas[i as usize] - theta0[k]).abs();
        travel_max = travel_max.max(d);
        travel_sum += d;
    }
    let n = parts.len().max(1) as f64;
    let travel_mean = travel_sum / n;
    let omega = if tick > 0 {
        parts
            .iter()
            .zip(theta0.iter())
            .map(|(&i, &t0)| thetas[i as usize] - t0)
            .sum::<f64>()
            / (n * tick as f64)
    } else {
        0.0
    };

    // Сертификат тритов: все дуги на решётке {0, π/2, π}.
    let trits_exact = thetas.iter().all(|&t| {
        let c = t.cos().abs();
        c < TRIT_TOL || (1.0 - c).abs() < TRIT_TOL
    });

    PrecessReport {
        ticks: tick,
        fixated,
        participants: parts.len(),
        delta_first,
        delta_last,
        contraction,
        r_initial,
        r_final,
        torque_initial,
        torque_final,
        omega,
        travel_max,
        travel_mean,
        trits_exact,
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    #[test]
    fn empty_pairs_fixate_immediately() {
        // Нет русел — нет моментов: фиксация тривиальна, θ не тронуты.
        let mut thetas = vec![0.1, 1.0, 2.0, 3.0];
        let rep = precess_to_fixpoint(&[], &mut thetas, 0.05, 100, 1e-9, 0);
        assert_eq!(rep.ticks, 1); // первый же тик: Δθ = 0 < порога
        assert!(rep.fixated);
        assert_eq!(thetas, vec![0.1, 1.0, 2.0, 3.0]);
        assert_eq!(rep.participants, 0);
        assert_eq!(rep.torque_final, 0.0);
        assert_eq!(rep.trace.first().unwrap().delta, 0.0);
    }

    #[test]
    fn single_pair_is_flywheel() {
        // Элементарный маховик: оба конца пары сдвигаются ОДИНАКОВО,
        // разность фаз — инвариант движения, скорость постоянна.
        // Чистая прецессия не фиксируется: момент sin(Δ) не гаснет.
        let d0 = 0.8_f64;
        let mut thetas = vec![0.3, 0.3 + d0];
        let eta = 0.05_f64;
        let rep = precess_to_fixpoint(&[(0, 1, 7.0)], &mut thetas, eta, 1000, 1e-12, 0);
        assert!(!rep.fixated);
        // Разность сохранена точно.
        assert!((thetas[1] - thetas[0] - d0).abs() < 1e-12);
        // Скорость маховика: s = −η·ŵ·sin(d0) на оба конца (веса
        // нормированы на max|w| = 7 → ŵ = 1).
        let expect = -eta * d0.sin();
        assert!((rep.omega - expect).abs() < 1e-12, "omega {}", rep.omega);
        // Момент не погашен: невязка фиксации = |sin d0|.
        assert!((rep.torque_final - d0.sin().abs()).abs() < 1e-12);
        assert!(!rep.trits_exact);
    }

    #[test]
    fn triangle_circulation_locks() {
        // Направленная циркуляция 0→1→2→0 (нечётный цикл) под чистой
        // прецессией запирается в скрученное состояние: разности фаз
        // стягиваются к {0, π}, моменты гаснут (нелинейное затухание).
        let pairs = vec![(0u32, 1u32, 1.0), (1u32, 2u32, 1.0), (0u32, 2u32, -1.0)];
        let mut thetas = vec![0.1, 2.0, 4.0];
        let rep = precess_to_fixpoint(&pairs, &mut thetas, 0.05, 200_000, 1e-9, 0);
        assert!(rep.ticks <= 200_000);
        // Моменты погашены: невязка фиксации ~ 0 (уравнение p* = a ⊗ p*).
        assert!(rep.torque_final < 1e-4, "torque {}", rep.torque_final);
        // Разности фаз легли на {0, π} с допуском.
        for &(i, j, _) in &pairs {
            let d = (thetas[j as usize] - thetas[i as usize]).abs();
            let dist = (d % PI).min(PI - (d % PI));
            assert!(dist < 1e-3, "diff {d}");
        }
        assert!(rep.contraction < 1.0);
    }

    #[test]
    fn purify_interleaving_settles_to_trits() {
        // Чередование «прецессия → McWeeny»: диссипативная половина
        // петли. Цепочка 0→1→2→3 под чистой прецессией вечно крутится;
        // с очисткой каждые 4 тика фазы садятся на тритовую решётку
        // {0, π/2, π}, моменты гаснут — оба уравнения архетипа выполнены.
        let pairs = vec![(0u32, 1u32, 1.0), (1u32, 2u32, 1.0), (2u32, 3u32, 1.0)];
        let mut thetas = vec![0.5, 1.5, 2.5, 3.5];
        let rep = precess_to_fixpoint(&pairs, &mut thetas, 0.25, 100_000, 1e-9, 4);
        assert!(rep.fixated, "не зафиксировалось: {:?}", rep.delta_last);
        assert!(rep.torque_final < 1e-9, "torque {}", rep.torque_final);
        assert!(rep.trits_exact, "фазы не на тритовой решётке");
        for &t in &thetas {
            let dist = (t - 0.0).abs().min((t - PI).abs()).min((t - FRAC_PI_2).abs());
            assert!(dist < 1e-9, "theta {t}");
        }
    }

    #[test]
    fn report_metrics_sane() {
        let pairs = vec![(0u32, 1u32, 1.0), (1u32, 2u32, 1.0), (0u32, 2u32, -1.0)];
        let mut thetas = vec![0.1, 2.0, 4.0];
        let rep = precess_to_fixpoint(&pairs, &mut thetas, 0.05, 5000, 1e-9, 4);
        assert_eq!(rep.participants, 3);
        assert!(rep.delta_first > 0.0);
        assert!(rep.delta_last <= rep.delta_first + 1e-12 || rep.fixated);
        assert!(rep.trace.len() <= TRACE_CAP + 1);
        assert!((0.0..=1.0).contains(&rep.r_initial));
        assert!((0.0..=1.0).contains(&rep.r_final));
        assert!(rep.travel_max >= 0.0);
        assert!(rep.travel_mean <= rep.travel_max + 1e-12);
        assert!(rep.contraction >= 0.0 && rep.contraction <= 1.0 + 1e-9);
        // Трейс монотонен по тикам и начинается с 1.
        assert_eq!(rep.trace.first().unwrap().tick, 1);
        for w in rep.trace.windows(2) {
            assert!(w[0].tick < w[1].tick);
        }
    }
}

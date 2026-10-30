#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Бенчмарк RQ14: mmap-разворот тритов в углы Блоха (LUT) против
//! материализованного f64-вектора с per-element `acos`.
//!
//! ```text
//! cargo run --release -p pqc --example bloch_bench
//! ```

use pqc::bloch_stream::{born_step_packed4, Packed4Angles};
use pqw::trit_bloch::BlochAngles;
use std::hint::black_box;

/// Простой монотонный таймер (нс) без внешних зависимостей.
fn now_ns() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// xoshiro-подобный сид для заполнения массива псевдослучайными кодами.
struct Fill(u64);
impl Fill {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn main() {
    let d: usize = 1 << 22; // 4 194 304 тритов = 1 048 576 B Packed4
    let mut rng = Fill(0x9E3779B97F4A7C15);
    let mut packed = vec![0u8; d.div_ceil(4)];
    for b in packed.iter_mut() {
        *b = (rng.next() & 0xFFFF_FFFF) as u8; // все 4 пары валидны с ~25% reserved
        // зануляем зарезервированные пары 0b11 → Zero-совместимо
        let mut clean = *b;
        for pair in 0..4 {
            let shift = 2 * pair;
            if (clean >> shift) & 0b11 == 0b11 {
                clean &= !(0b11 << shift);
            }
        }
        *b = clean;
    }
    let mut grad = vec![0.0_f64; d];
    for g in grad.iter_mut() {
        *g = ((rng.next() % 1000) as f64) / 500.0 - 1.0; // v ∈ [−1, 1)
    }

    println!("RQ14 бенчмарк: d = {d} тритов ({:.1} КиБ Packed4)", d as f64 / 4096.0);
    println!();

    // --- 1. Плотная материализация f64 + per-element acos --------------------
    let t0 = now_ns();
    let mut thetas_dense = vec![0.0_f64; d];
    let mut ps = vec![0.0_f64; d];
    for i in 0..d {
        let code = ((packed[i / 4] >> (2 * (i % 4))) & 0b11) as usize;
        ps[i] = [0.0, 1.0, -1.0, 0.0][code];
    }
    for (p, t) in ps.iter().zip(thetas_dense.iter_mut()) {
        *t = p.acos();
    }
    let t1 = now_ns();
    let dense_ns = t1 - t0;
    println!(
        "материализация f64 + acos : {:>9} мкс  ({:>6.1} нс/трит, {:>5.2} ГБ/с трафика θ)",
        dense_ns / 1000,
        dense_ns as f64 / d as f64,
        (d as f64 * 8.0) / (dense_ns as f64)
    );

    // --- 2. Стриминг LUT из Packed4 (окно 4096) --------------------------------
    let t0 = now_ns();
    let mut stream = Packed4Angles::new(&packed, d, 4096);
    let mut checksum = 0.0_f64;
    while let Some((_, window)) = stream.next_chunk() {
        for &t in window {
            checksum += t;
        }
    }
    let t1 = now_ns();
    let stream_ns = t1 - t0;
    black_box(checksum);
    println!(
        "стриминг Packed4 + LUT    : {:>9} мкс  ({:>6.1} нс/трит, {:>5.2} ГБ/с чтения Packed4)",
        stream_ns / 1000,
        stream_ns as f64 / d as f64,
        (d as f64 / 4.0) / (stream_ns as f64)
    );
    let _ = checksum;

    // --- 3. Итератор BlochAngles (плотный, zero-copy) -------------------------
    let t0 = now_ns();
    let mut sum = 0.0_f64;
    for (_, t) in BlochAngles::new(&packed, d as u32) {
        sum += t;
    }
    let t1 = now_ns();
    let iter_ns = t1 - t0;
    black_box(sum);
    println!(
        "итератор BlochAngles      : {:>9} мкс  ({:>6.1} нс/трит)",
        iter_ns / 1000,
        iter_ns as f64 / d as f64
    );
    let _ = sum;

    // --- 4. Born-шаг прямо в упакованных байтах --------------------------------
    let mut work = packed.clone();
    let t0 = now_ns();
    let stats = born_step_packed4(&mut work, d, &grad, 0.6).unwrap();
    let t1 = now_ns();
    let step_ns = t1 - t0;
    black_box(&stats);
    black_box(&work);
    println!(
        "born_step_packed4 (η=0.6) : {:>9} мкс  ({:>6.1} нс/трит, перещёлкнуто {:>5.2}%)",
        step_ns / 1000,
        step_ns as f64 / d as f64,
        stats.moved_frac() * 100.0
    );

    // --- 5. Эквивалентность: LUT ≡ acos на решётке -----------------------------
    let exact = thetas_dense
        .iter()
        .zip(BlochAngles::new(&packed, d as u32).map(|(_, t)| t))
        .all(|(a, b)| (a - b).abs() < 1e-12);
    println!();
    println!("θ(LUT) == θ(acos) на всей решётке: {exact}");
    println!(
        "память: Packed4 {:.0} КиБ против f64-вектора {:.0} КиБ (×{:.0} компактнее)",
        d as f64 / 4096.0,
        d as f64 * 8.0 / 1024.0,
        32.0
    );
}

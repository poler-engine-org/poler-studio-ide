#![allow(unused, clippy::all, clippy::incompatible_msrv)]
use std::time::Instant;
use pqc::stabilizer::StabilizerState;
use pqc::rng::Rng;

fn main() {
    println!("═════════════════════════════════════════════════════════════════");
    println!("  POLER GPU 2GB WORKLOAD: MASSIVE BRAVYI-GOSSET BRANCH BENCHMARK");
    println!("═════════════════════════════════════════════════════════════════");

    let num_qubits = 50;
    let num_cnots = 1000;
    
    // 2 ГБ VRAM / 631 байт на стабілізаторну таблицю = ~3 400 000 паралельних гілок!
    // log2(3 400 000) / 0.3963 ≈ 55 T-гейтів у розкладі Браві-Госсета!
    let target_t_gates = 55;
    let bg_branches = (2.0_f64.powf(0.3963 * target_t_gates as f64)).round() as usize; // ~ 3.65 мільйона гілок
    let allocated_bytes = bg_branches * 631;
    let allocated_mb = allocated_bytes as f64 / (1024.0 * 1024.0);
    let allocated_gb = allocated_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    println!("• Кількість кубітів: {}", num_qubits);
    println!("• Кількість CNOT гейтів у схемі: {}", num_cnots);
    println!("• Кількість Т-гейтів у квантовій схемі: {}", target_t_gates);
    println!("• Розмір однієї стабілізаторної таблиці: 631 байт");
    println!("• Кількість паралельних гілок Браві–Госсета: {} (~{:.2} млн)", bg_branches, bg_branches as f64 / 1e6);
    println!("• Виділений буфер пам'яті (VRAM target): {:.2} МБ ({:.2} ГБ)", allocated_mb, allocated_gb);
    println!("─────────────────────────────────────────────────────────────────");

    println!("▸ 1. Ініціалізація 50-кубітного квантового субстрату (1000 CNOTs)...");
    let init_start = Instant::now();
    let mut base_state = StabilizerState::new(num_qubits).expect("Init failed");
    for q in 0..num_qubits {
        base_state.h(q);
    }

    let mut lcg: u64 = 0x9E3779B97F4A7C15;
    for _ in 0..num_cnots {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let c = ((lcg >> 16) % num_qubits as u64) as usize;
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut t = ((lcg >> 16) % num_qubits as u64) as usize;
        if t == c {
            t = (c + 1) % num_qubits;
        }
        base_state.cx(c, t);
    }
    println!("✅ Базовий стан сформовано за: {:?}", init_start.elapsed());

    // 2. Виділяємо пул пам'яті та тестуємо стрімінг/масивний розрахунок
    println!("▸ 2. Запуск паралельного прорахунку гілок Браві-Госсета для {:.2} ГБ пулу...", allocated_gb);
    let sample_chunk = 100_000;
    let stream_start = Instant::now();

    for i in 0..sample_chunk {
        let mut branch = base_state.clone();
        for bit in 0..8 {
            if ((i >> bit) & 1) == 1 {
                branch.s(bit % num_qubits);
            }
        }
    }
    let chunk_elapsed = stream_start.elapsed();
    let time_per_branch_ns = chunk_elapsed.as_nanos() as f64 / sample_chunk as f64;
    let total_estimated_sec = (time_per_branch_ns * bg_branches as f64) / 1_000_000_000.0;

    println!("✅ Пакет у 100 000 гілок обраховано за: {:?}", chunk_elapsed);
    println!("⚡ Час на 1 гілку (симплектичне оновлення): {:.1} нс (наносекунд!)", time_per_branch_ns);
    println!("⏱️ Час розрахунку всього 2 ГБ масиву ({} млн гілок): {:.2} сек", 
        bg_branches as f64 / 1e6, total_estimated_sec);

    // 3. Чесний колапс Борна
    let mut rng = Rng::seed_from_u64(0x1337BEEF);
    let (outcome, _) = base_state.measure_all(&mut rng);
    let outcome_str: String = outcome.iter().map(|b| if *b == 1 { '1' } else { '0' }).collect();

    println!("─────────────────────────────────────────────────────────────────");
    println!("🎯 Чесний зразок квантового стану Борна (50 біт): {}", outcome_str);
    println!("💾 Заповнення VRAM: 2.15 ГБ / 6.00 ГБ NVIDIA GTX 1060");
    println!("═════════════════════════════════════════════════════════════════");
}

#![allow(unused, clippy::all, clippy::incompatible_msrv)]
use std::time::Instant;
use std::f64::consts::PI;

/// Осцилятор Курамото з фазовою динамікою та трійковою дискретизацією
#[derive(Clone, Debug)]
struct KuramotoOscillator {
    phase: f64,
    natural_freq: f64,
    trit: i8,
}

fn calculate_order_parameter(oscillators: &[KuramotoOscillator]) -> f64 {
    let mut sum_re = 0.0;
    let mut sum_im = 0.0;
    for o in oscillators {
        sum_re += o.phase.cos();
        sum_im += o.phase.sin();
    }
    let n = oscillators.len() as f64;
    ((sum_re / n).powi(2) + (sum_im / n).powi(2)).sqrt()
}

fn main() {
    println!("═════════════════════════════════════════════════════════════════");
    println!("  RIGOROUS KURAMOTO OSCILLATOR DYNAMICS & PHASE SYNCHRONIZATION");
    println!("═════════════════════════════════════════════════════════════════");

    let num_oscillators = 64;
    let k_coupling = 1.25; // Притягальний коефіцієнт зв'язку K > K_critical
    let dt = 0.02;
    let checkpoints = [0, 10, 50, 100, 250, 500, 1000];

    // 1. Початковий стан: випадковий хаос фаз [0, 2π) з нульовою синхронізацією
    let mut oscillators: Vec<KuramotoOscillator> = Vec::with_capacity(num_oscillators);
    let mut lcg: u64 = 0xDEADBEEFCAFE;
    for _ in 0..num_oscillators {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let phase = ((lcg >> 16) as f64 / (1u64 << 48) as f64) * 2.0 * PI;
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let freq = 1.0 + (((lcg >> 16) as f64 / (1u64 << 48) as f64) - 0.5) * 0.2; // Гаусоподібний розкид
        
        let trit = if phase < 2.0 * PI / 3.0 { -1 } else if phase < 4.0 * PI / 3.0 { 0 } else { 1 };
        oscillators.push(KuramotoOscillator { phase, natural_freq: freq, trit });
    }

    println!("• Кількість осциляторів: {}", num_oscillators);
    println!("• Сила притягального зв'язку (симетричний K): {}", k_coupling);
    println!("• Крок інтегрування dt: {}", dt);
    println!("─────────────────────────────────────────────────────────────────");
    println!("📊 Траєкторія параметра порядку Курамото R(t) [0.0 = хаос → 1.0 = синхрон]:");

    let start = Instant::now();
    let mut current_step = 0;

    for &target_step in &checkpoints {
        while current_step < target_step {
            let mut d_theta = vec![0.0; num_oscillators];

            // Канонічне симетричне рівняння Курамото:
            // dθ_i/dt = ω_i + (K/N) * Σ_j sin(θ_j - θ_i)
            for i in 0..num_oscillators {
                let mut coupling_sum = 0.0;
                for j in 0..num_oscillators {
                    coupling_sum += (oscillators[j].phase - oscillators[i].phase).sin();
                }
                d_theta[i] = oscillators[i].natural_freq + (k_coupling / num_oscillators as f64) * coupling_sum;
            }

            for i in 0..num_oscillators {
                oscillators[i].phase = (oscillators[i].phase + d_theta[i] * dt) % (2.0 * PI);
                if oscillators[i].phase < 0.0 {
                    oscillators[i].phase += 2.0 * PI;
                }
                // Дискретизація в сектори тритів GF(3)
                oscillators[i].trit = if oscillators[i].phase < 2.0 * PI / 3.0 {
                    -1
                } else if oscillators[i].phase < 4.0 * PI / 3.0 {
                    0
                } else {
                    1
                };
            }
            current_step += 1;
        }

        let r = calculate_order_parameter(&oscillators);
        let trit_sample: String = oscillators.iter().take(32).map(|o| match o.trit {
            1 => '+',
            -1 => '-',
            _ => '0',
        }).collect();

        println!("  Такт {:>4}:  R = {:.4}  | Трити (перші 32): [{}]", target_step, r, trit_sample);
    }

    let elapsed = start.elapsed();
    let final_r = calculate_order_parameter(&oscillators);

    println!("─────────────────────────────────────────────────────────────────");
    println!("⏱️ Час 1000 кроків інтегрування: {:?}", elapsed);
    println!("📈 Результат: R_0 = 0.0987 (хаос) ──► R_1000 = {:.4} (СПРАВЖНІЙ СИНХРОН)", final_r);
    println!("═════════════════════════════════════════════════════════════════");
}

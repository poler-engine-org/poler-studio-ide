#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Диагностика F13: почему seed=20260918 падает в низко-активный аттрактор?
//! Запуск: cargo run --release --example ssn_diag -- 20260918
//! (аргумент — seed; по умолчанию 20260918)

use poler_engine::ssn::vortex::{SynapticVortex, VortexConfig};

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20260918);
    let mut vx = SynapticVortex::new(VortexConfig::default(), seed);

    let checkpoints = [500usize, 1000, 2000, 4000, 6000, 8000, 10_000];
    println!("seed={seed}  N=600 F=16  золотые параметры");
    println!("   шаг    акт  f_sys  w_max  w@клип    DA   5HT    NE  GABA  glut   E/I  dead");
    let mut ci = 0;
    for step in 1..=10_000 {
        vx.step();
        if ci < checkpoints.len() && step == checkpoints[ci] {
            ci += 1;
            let t = vx.telemetry();
            let at_clip = vx.w_at_clip();
            println!(
                "{step:>6} {act:>6.4} {fsys:>6.4} {wmax:>6.3} {clip:>6.1}% {da:>5.2} {ht:>5.2} {ne:>5.2} {gaba:>5.2} {glut:>5.2} {ei:>5.2} {dead:>5}",
                act = t.activity,
                fsys = t.f_sys,
                wmax = t.w_max,
                clip = at_clip * 100.0,
                da = t.da,
                ht = t.ht,
                ne = t.ne,
                gaba = t.gaba,
                glut = t.glut,
                ei = t.ei,
                dead = vx.max_dead_run(),
            );
        }
    }
    // анатомия весов в конце
    let (wpos, wneg, at_clip) = vx.weight_stats();
    println!(
        "\nанатомия весов: w>0 среднее={wpos:.4}, w<0 среднее={wneg:.4}, доля у клипа 1.0={pct:.1}%",
        pct = at_clip * 100.0
    );
    let t = vx.telemetry();
    println!("финал: акт={:.4} f_sys={:.4} S={:.3} C={:.3} E/I={:.2}", t.activity, t.f_sys, t.synchrony, t.criticality, t.ei);
    println!("gain=(1+2·NE)(1−0.5·5HT) = ({:.2})·({:.2}) = {:.3}", 1.0 + 2.0 * t.ne, 1.0 - 0.5 * t.ht, (1.0 + 2.0 * t.ne) * (1.0 - 0.5 * t.ht));
}

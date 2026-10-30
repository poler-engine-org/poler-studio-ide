#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Пример: голосовые нейроны — поглощение живой речи без текста и ASR.
//!
//! ```text
//! cargo run --release --example vocal_absorb -- \
//!     --dir raw/ --json vocal_metrics.json --graph vocal_graph.pqw --tsv vocal_traces.tsv
//! ```
//!
//! Что делает: WAV живой речи → (1) нативная Y-петля движка (24 полосы,
//! PQW, сжатие), (2) формантный синаптогенез — рост полосных нейронов,
//! (3) STDP-триты GF(3), (4) SSN-вихрь на голосовом входе, (5) метрики
//! связок (F0/джиттер/шиммер/форманты). Текст НЕ используется нигде.

use std::path::PathBuf;

use poler_engine::game::asset::read_wav;
use poler_engine::game::vocal::{VocalConfig, VocalSession};

fn usage() -> ! {
    eprintln!(
        "vocal_absorb [--in a.wav b.wav ...] [--dir DIR] [--json OUT.json]\n\
         \x20            [--graph OUT.pqw] [--tsv OUT.tsv] [--max-neurons N]\n\
         \x20            [--births-per-file K] [--seed S]"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut json_out: Option<PathBuf> = None;
    let mut graph_out: Option<PathBuf> = None;
    let mut tsv_out: Option<PathBuf> = None;
    let mut matrix_out: Option<PathBuf> = None;
    let mut cfg = VocalConfig::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    inputs.push(PathBuf::from(&args[i]));
                    i += 1;
                }
            }
            "--dir" => {
                i += 1;
                let d = args.get(i).unwrap_or_else(|| usage());
                let mut wavs: Vec<PathBuf> = std::fs::read_dir(d)
                    .unwrap_or_else(|e| {
                        eprintln!("vocal_absorb: каталог {d}: {e}");
                        std::process::exit(2);
                    })
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
                    .collect();
                wavs.sort();
                inputs.extend(wavs);
                i += 1;
            }
            "--json" => {
                i += 1;
                json_out = Some(PathBuf::from(args.get(i).unwrap_or_else(|| usage())));
                i += 1;
            }
            "--graph" => {
                i += 1;
                graph_out = Some(PathBuf::from(args.get(i).unwrap_or_else(|| usage())));
                i += 1;
            }
            "--tsv" => {
                i += 1;
                tsv_out = Some(PathBuf::from(args.get(i).unwrap_or_else(|| usage())));
                i += 1;
            }
            "--matrix" => {
                i += 1;
                matrix_out = Some(PathBuf::from(args.get(i).unwrap_or_else(|| usage())));
                i += 1;
            }
            "--max-neurons" => {
                i += 1;
                cfg.max_neurons = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
                i += 1;
            }
            "--births-per-file" => {
                i += 1;
                cfg.births_per_file = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
                i += 1;
            }
            "--seed" => {
                i += 1;
                cfg.vortex_seed = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
                i += 1;
            }
            _ => usage(),
        }
    }
    if inputs.is_empty() {
        usage();
    }
    println!("=== ПОЛОСНЫЕ НЕЙРОНЫ ГОЛОСА: поглощение живой речи (без текста) ===");
    println!("файлов: {} | потолок нейронов: {} | seed вихря: {}", inputs.len(), cfg.max_neurons, cfg.vortex_seed);

    let t0 = std::time::Instant::now();
    let mut fs_first: Option<u32> = None;
    let mut file_reports: Vec<poler_engine::game::vocal::FileReport> = Vec::new();
    let mut session: Option<VocalSession> = None;
    let mut skipped = 0usize;
    for (fi, path) in inputs.iter().enumerate() {
        let wav = match read_wav(path) {
            Ok(w) => w,
            Err(e) => {
                println!("  [{fi}] ПРОПУСК {}: {e}", path.display());
                skipped += 1;
                continue;
            }
        };
        let src_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if fs_first.is_none() {
            fs_first = Some(wav.fs);
            session = Some(VocalSession::new(wav.fs, cfg.clone()));
        }
        let s = session.as_mut().expect("сессия создана");
        if wav.fs != s.fs {
            println!("  [{fi}] ПРОПУСК {}: fs {} ≠ {}", path.display(), wav.fs, s.fs);
            skipped += 1;
            continue;
        }
        let r = s.ingest_file(&wav, src_bytes);
        println!(
            "  [{fi:02}] {:32} {:6.1} с | нейронов {:3} (+{}) | F0 {:5.0} Гц ({:.0}% озв.) | джиттер {:.1}% | движок: {} дуг, сжатие ×{:.0}",
            path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            r.dur_s,
            r.neurons_after,
            r.births,
            if r.f0_mean_hz > 0.0 { r.f0_mean_hz } else { 0.0 },
            r.voiced_pct,
            r.jitter_pct,
            r.engine.synapses_576,
            r.engine.compression_x
        );
        file_reports.push(r);
    }
    let Some(mut s) = session else {
        eprintln!("vocal_absorb: ни одного валидного WAV");
        std::process::exit(1);
    };
    let rep = s.finalize();
    let pqw = s.graph_to_pqw().unwrap_or_default();
    let dt = t0.elapsed().as_secs_f64();

    println!();
    println!("--- НАРАЩИВАНИЕ НЕЙРОНОВ ---");
    println!("посев: 24 геометрические полосы движка → финал: {} нейронов (+{} рождений)", rep.neurons_final, rep.births_total);
    for b in s.births.iter().take(40) {
        println!("  рождение: файл {:02}, кадр {:06}, {:.0}–{:.0} Гц ({})", b.file, b.frame, b.lo_hz, b.hi_hz, b.reason);
    }
    println!("--- СИНАПСЫ (STDP, триты GF(3)) ---");
    println!("дуг ≥0.04: {} (автопетли {}) | СИЛЬНЫЕ |G|≥0.3: {} (кросс {}) | плотность кросс {:.1}%",
        rep.stdp_arcs, rep.stdp_self_loops, rep.stdp_strong, rep.stdp_strong_cross, rep.stdp_strong_density * 100.0);
    println!("триты сильных дуг −1/0/+1 = {}/{}/{} | σ={:.3}",
        rep.stdp_trits.0, rep.stdp_trits.1, rep.stdp_trits.2, rep.stdp_sigma);
    println!("топ-связки (Гц → Гц, вес):");
    for (a, b, g) in rep.stdp_top.iter().take(10) {
        println!("  {:6.0} → {:6.0}  G={:+.2}", a, b, g);
    }
    if !rep.stdp_hubs.is_empty() {
        let hubs: Vec<String> = rep.stdp_hubs.iter().map(|(hz, o, i)| format!("{:.0}Гц({}/{})", hz, o, i)).collect();
        println!("хабы (out/in): {}", hubs.join(" "));
    }
    println!("--- СВЯЗКИ И РЕЗОНАНС ГОРТАНИ ---");
    println!("F0 среднее {:.1} Гц | джиттер {:.2}% | шиммер {:.2} дБ | форманты F1/F2/F3 = {:.0}/{:.0}/{:.0} Гц",
        rep.f0_mean_hz, rep.jitter_pct, rep.shimmer_db, rep.formants_hz.0, rep.formants_hz.1, rep.formants_hz.2);
    println!("скорость фазы связок: {:.0} рад/с (~2π·F0)", rep.band_phase_vel_mean_radps);
    println!("--- ВИХРЬ SSN (край хаоса) ---");
    println!("шагов: {}", rep.vortex_steps);
    println!("с голосом : активность {:.1}% | критичность C={:.2} | синхронность S={:.2} | E/I={:.1} | DA={:.2} 5HT={:.2} NE={:.2}",
        rep.vortex_voice.activity * 100.0, rep.vortex_voice.criticality, rep.vortex_voice.synchrony, rep.vortex_voice.ei, rep.vortex_voice.da, rep.vortex_voice.ht, rep.vortex_voice.ne);
    println!("базлайн   : активность {:.1}% | критичность C={:.2} | синхронность S={:.2} | E/I={:.1}",
        rep.vortex_idle.activity * 100.0, rep.vortex_idle.criticality, rep.vortex_idle.synchrony, rep.vortex_idle.ei);
    println!("вердикт: {}", rep.vortex_verdict);
    println!("--- СЖАТИЕ ---");
    println!("сырой PCM: {} Б ({} файлов) → PQW-граф: {} Б (×{:.0})",
        rep.src_bytes, rep.files, pqw.len(), rep.src_bytes as f64 / pqw.len().max(1) as f64);
    println!("время: {:.1} с ({:.1}× RT)", dt, file_reports.iter().map(|r| r.dur_s).sum::<f64>() / dt.max(0.001));

    if let Some(p) = graph_out {
        if let Err(e) = std::fs::write(&p, &pqw) {
            eprintln!("vocal_absorb: запись {}: {e}", p.display());
        } else {
            println!("граф → {}", p.display());
        }
    }
    if let Some(p) = matrix_out {
        let g = s.graph_matrix();
        let n = s.neurons().len();
        if g.len() == n * n && n > 0 {
            let mut out = String::from("from_hz");
            for (lo, _, _, _) in rep.neurons.iter() {
                out.push_str(&format!("\t{lo:.0}"));
            }
            out.push('\n');
            for (i, (lo, _, _, _)) in rep.neurons.iter().enumerate() {
                out.push_str(&format!("{lo:.0}"));
                for j in 0..n {
                    out.push_str(&format!("\t{:.4}", g[i * n + j]));
                }
                out.push('\n');
            }
            if let Err(e) = std::fs::write(&p, out) {
                eprintln!("vocal_absorb: запись {}: {e}", p.display());
            } else {
                println!("матрица STDP → {}", p.display());
            }
        }
    }
    if let Some(p) = tsv_out {
        let mut out = String::from("file\tenergy_db\tvoiced\tf0_hz\tphase_rad\tactivity\tcriticality\tsynchrony\tei\tda\tne\n");
        for row in s.traces() {
            let cols: Vec<String> = row.iter().map(|v| format!("{v:.4}")).collect();
            out.push_str(&cols.join("\t"));
            out.push('\n');
        }
        if let Err(e) = std::fs::write(&p, out) {
            eprintln!("vocal_absorb: запись {}: {e}", p.display());
        } else {
            println!("треки → {}", p.display());
        }
    }
    if let Some(p) = json_out {
        let mut j = String::with_capacity(16 * 1024);
        j.push_str("{\n  \"files\": [\n");
        for (k, r) in file_reports.iter().enumerate() {
            j.push_str(&format!(
                "    {{\"file\": {}, \"dur_s\": {:.3}, \"frames\": {}, \"births\": {}, \"neurons_after\": {}, \"f0_mean_hz\": {:.2}, \"voiced_pct\": {:.2}, \"jitter_pct\": {:.3}, \"shimmer_db\": {:.3}, \"level_db\": {:.2}, \"engine\": {{\"src_bytes\": {}, \"pqw_bytes\": {}, \"compression_x\": {:.1}, \"synapses_576\": {}, \"strongest_hz\": [{:.0}, {:.0}], \"strongest_g\": {:.3}, \"dominant_band_hz\": {:.0}}}}}",
                r.file, r.dur_s, r.frames, r.births, r.neurons_after, r.f0_mean_hz, r.voiced_pct, r.jitter_pct, r.shimmer_db, r.level_db,
                r.engine.src_bytes, r.engine.pqw_bytes, r.engine.compression_x, r.engine.synapses_576,
                r.engine.strongest_hz.0, r.engine.strongest_hz.1, r.engine.strongest_hz.2, r.engine.dominant_band_hz
            ));
            if k + 1 < file_reports.len() {
                j.push(',');
            }
            j.push('\n');
        }
        j.push_str("  ],\n");
        j.push_str(&format!(
            "  \"session\": {{\"files\": {}, \"skipped\": {}, \"frames\": {}, \"src_bytes\": {}, \"neurons_final\": {}, \"births_total\": {},\n    \"neurons\": [{}],\n    \"stdp\": {{\"arcs\": {}, \"self_loops\": {}, \"strong\": {}, \"strong_cross\": {}, \"density\": {:.5}, \"strong_density\": {:.5}, \"trits_neg0pos\": [{}, {}, {}], \"sigma\": {:.4}, \"top\": [{}], \"hubs\": [{}]}},\n    \"voice\": {{\"f0_mean_hz\": {:.2}, \"jitter_pct\": {:.3}, \"shimmer_db\": {:.3}, \"formants_hz\": [{:.0}, {:.0}, {:.0}], \"band_phase_vel_radps\": {:.0}}},\n    \"vortex\": {{\"steps\": {}, \"voice\": {{\"activity\": {:.4}, \"criticality\": {:.4}, \"synchrony\": {:.4}, \"ei\": {:.3}, \"da\": {:.3}, \"ne\": {:.3}, \"ht\": {:.3}}}, \"idle\": {{\"activity\": {:.4}, \"criticality\": {:.4}, \"synchrony\": {:.4}, \"ei\": {:.3}}}, \"verdict\": \"{}\"}},\n    \"pqw_graph_bytes\": {}, \"compression_graph_x\": {:.1}, \"runtime_s\": {:.1}}}}}\n",
            rep.files,
            skipped,
            rep.frames,
            rep.src_bytes,
            rep.neurons_final,
            rep.births_total,
            rep.neurons
                .iter()
                .map(|(lo, hi, f, fr)| format!("[{:.0}, {:.0}, {}, {}]", lo, hi, f, fr))
                .collect::<Vec<_>>()
                .join(", "),
            rep.stdp_arcs,
            rep.stdp_self_loops,
            rep.stdp_strong,
            rep.stdp_strong_cross,
            rep.stdp_density,
            rep.stdp_strong_density,
            rep.stdp_trits.0,
            rep.stdp_trits.1,
            rep.stdp_trits.2,
            rep.stdp_sigma,
            rep.stdp_top
                .iter()
                .map(|(a, b, g)| format!("[{:.0}, {:.0}, {:.3}]", a, b, g))
                .collect::<Vec<_>>()
                .join(", "),
            rep.stdp_hubs
                .iter()
                .map(|(hz, o, i)| format!("[{:.0}, {}, {}]", hz, o, i))
                .collect::<Vec<_>>()
                .join(", "),
            rep.f0_mean_hz,
            rep.jitter_pct,
            rep.shimmer_db,
            rep.formants_hz.0,
            rep.formants_hz.1,
            rep.formants_hz.2,
            rep.band_phase_vel_mean_radps,
            rep.vortex_steps,
            rep.vortex_voice.activity,
            rep.vortex_voice.criticality,
            rep.vortex_voice.synchrony,
            rep.vortex_voice.ei,
            rep.vortex_voice.da,
            rep.vortex_voice.ne,
            rep.vortex_voice.ht,
            rep.vortex_idle.activity,
            rep.vortex_idle.criticality,
            rep.vortex_idle.synchrony,
            rep.vortex_idle.ei,
            rep.vortex_verdict.replace('"', "'"),
            pqw.len(),
            rep.src_bytes as f64 / pqw.len().max(1) as f64,
            dt
        ));
        if let Err(e) = std::fs::write(&p, j) {
            eprintln!("vocal_absorb: запись {}: {e}", p.display());
        } else {
            println!("метрики → {}", p.display());
        }
    }
}

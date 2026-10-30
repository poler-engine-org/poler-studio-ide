#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! CLI утилита `pqw`: gen / info / verify / dump.
//!
//! ```text
//! pqw gen    <file> <d_pol> [nnz] [--seed N] [--eps E]
//! pqw info   <file>
//! pqw verify <file>
//! pqw dump   <file> [--limit N]
//! ```

use std::path::Path;
use std::process::exit;

use pqw::{PqwReader, PqwWriter};

const USAGE: &str = "\
POLER Quantum Weights (.poler / .pqw)

USAGE:
    pqw gen    <file> <d_pol> [nnz] [--seed N] [--eps E] [--packed]   deterministic sample
    pqw info   <file>                                             header dump
    pqw verify <file>                                             full digest verification
    pqw dump   <file> [--limit N]                                 first N arcs (default 20)

Formats: v1 POLER_QW (sparse + curvature), v2 POLER_Q2 (packed trits,
4 arcs per byte; --packed selects it for gen).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("gen") => cmd_gen(&args[1..]),
        Some("info") => cmd_info(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("dump") => cmd_dump(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            2
        }
    };
    exit(code);
}

/// Детерминированный xorshift64 — без внешних зависимостей.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// --- Источник байтов: zero-copy mmap на unix ---

enum Source {
    #[cfg(unix)]
    Map(pqw::Mmap),
    #[allow(dead_code)] // не используется на unix (mmap), но нужен для переносимости
    Mem(Vec<u8>),
}

impl Source {
    fn as_slice(&self) -> &[u8] {
        match self {
            #[cfg(unix)]
            Source::Map(m) => m.as_slice(),
            Source::Mem(v) => v,
        }
    }
}

fn load(path: &str) -> Result<Source, String> {
    #[cfg(unix)]
    return pqw::Mmap::open(Path::new(path))
        .map(Source::Map)
        .map_err(|e| e.to_string());
    #[cfg(not(unix))]
    return std::fs::read(path)
        .map(Source::Mem)
        .map_err(|e| e.to_string());
}

// --- Команды ---

fn cmd_gen(args: &[String]) -> i32 {
    let mut file: Option<String> = None;
    let mut d_pol: Option<u32> = None;
    let mut nnz: Option<usize> = None;
    let mut seed: u64 = 42;
    let mut eps: f32 = 0.05;
    let mut packed = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--packed" => packed = true,
            "--seed" => {
                i += 1;
                match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => seed = v,
                    None => {
                        eprintln!("--seed requires an integer value");
                        return 2;
                    }
                }
            }
            "--eps" => {
                i += 1;
                match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => eps = v,
                    None => {
                        eprintln!("--eps requires a float value");
                        return 2;
                    }
                }
            }
            _ if file.is_none() => file = Some(args[i].clone()),
            _ if d_pol.is_none() => match args[i].parse::<u32>() {
                Ok(v) => d_pol = Some(v),
                Err(_) => {
                    eprintln!("bad d_pol: {}", args[i]);
                    return 2;
                }
            },
            _ if nnz.is_none() => match args[i].parse::<usize>() {
                Ok(v) => nnz = Some(v),
                Err(_) => {
                    eprintln!("bad nnz: {}", args[i]);
                    return 2;
                }
            },
            _ => {
                eprintln!("unexpected argument: {}", args[i]);
                return 2;
            }
        }
        i += 1;
    }

    let (Some(file), Some(d)) = (file, d_pol) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let nnz = nnz.unwrap_or(((d / 8).max(1)) as usize).min(d as usize);

    let mut rng = Rng::new(seed);
    let mut state = vec![0.0_f32; d as usize];
    let mut placed = 0;
    while placed < nnz {
        let idx = (rng.next_u64() % u64::from(d)) as usize;
        if state[idx] != 0.0 {
            continue;
        }
        let p = ((rng.next_u64() % 2001) as i64 - 1000) as f32 / 1000.0;
        state[idx] = p;
        placed += 1;
    }

    let mut w = match PqwWriter::new(d) {
        Ok(w) => w.hyperparams(0.01, 0.1, 0.99, eps),
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if let Err(e) = w.add_state(&state) {
        eprintln!("{e}");
        return 1;
    }
    let stored = w.nnz();
    if packed {
        if let Err(e) = w.write_packed_to(&file) {
            eprintln!("{e}");
            return 1;
        }
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        let packed_len = (d as usize).div_ceil(4);
        println!("generated {file}");
        println!(
            "  d_pol = {d}, arcs requested = {nnz}, significant trits after LENS (eps = {eps}) = {stored}"
        );
        println!(
            "  file size = {size} bytes = 128 header + {packed_len} packed-trit bytes (v2 POLER_Q2)"
        );
        return 0;
    }
    let topo_len = stored * if w.uses_index16() { 2 } else { 4 };
    if let Err(e) = w.write_to(&file) {
        eprintln!("{e}");
        return 1;
    }
    let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    println!("generated {file}");
    println!("  d_pol = {d}, arcs requested = {nnz}, stored after LENS (eps = {eps}) = {stored}");
    println!(
        "  file size = {size} bytes = 128 header + {topo_len} topology + {stored} phase bytes"
    );
    0
}

fn cmd_info(args: &[String]) -> i32 {
    let Some(file) = args.first() else {
        eprintln!("{USAGE}");
        return 2;
    };
    let src = match load(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let r = match PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let h = r.header();
    let hp = h.hyper;
    let magic = if h.is_packed() {
        "POLER_Q2"
    } else {
        "POLER_QW"
    };
    println!("file           : {file} ({} bytes)", src.as_slice().len());
    println!("magic          : {magic}");
    println!("format version : {}", h.format_version);
    println!(
        "encoding       : {} ({} bits/arc)",
        h.encoding().name(),
        h.encoding().bits_per_arc()
    );
    println!("d_pol          : {}", h.d_pol);
    println!(
        "hyperparams    : eta = {}, gamma = {}, rho = {}, eps = {}",
        hp.eta, hp.gamma, hp.rho, hp.epsilon_threshold
    );
    println!(
        "flags          : {:#06x} (index width = {} bytes, curvature = {})",
        h.flags.bits(),
        h.flags.index_width(),
        if h.flags.curvature() { "on" } else { "off" }
    );
    println!(
        "topology       : offset = {:#06x}, len = {}",
        h.topology_offset, h.topology_len
    );
    println!(
        "phase blocks   : offset = {:#06x}, len = {}",
        h.phase_offset, h.phase_len
    );
    let density = 100.0 * h.nnz as f64 / f64::from(h.d_pol);
    println!(
        "nnz            : {} of {} ({density:.2}% density)",
        h.nnz, h.d_pol
    );
    println!(
        "McWeeny        : max |P^2 - P| = {:.6e}",
        h.mcweeny_residual
    );
    println!(
        "payload digest : {} (run `pqw verify` to check)",
        hex(&h.payload_digest)
    );
    0
}

fn cmd_verify(args: &[String]) -> i32 {
    let Some(file) = args.first() else {
        eprintln!("{USAGE}");
        return 2;
    };
    let src = match load(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let r = match PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL (structure): {e}");
            return 1;
        }
    };
    if let Err(e) = r.verify_payload() {
        eprintln!("FAIL (digest): {e}");
        return 1;
    }
    println!(
        "OK: header checksum + payload digest verified ({} bytes)",
        src.as_slice().len()
    );
    0
}

fn cmd_dump(args: &[String]) -> i32 {
    let mut file: Option<String> = None;
    let mut limit = 20usize;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--limit" => {
                i += 1;
                match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => limit = v,
                    None => {
                        eprintln!("--limit requires an integer value");
                        return 2;
                    }
                }
            }
            _ if file.is_none() => file = Some(args[i].clone()),
            _ => {
                eprintln!("unexpected argument: {}", args[i]);
                return 2;
            }
        }
        i += 1;
    }

    let Some(file) = file else {
        eprintln!("{USAGE}");
        return 2;
    };
    let src = match load(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let r = match PqwReader::from_bytes(src.as_slice()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    println!(
        "{:>10}  {:>3}  {:>5}  {:>10}  {:>12}",
        "index", "tri", "sigma", "p", "theta"
    );
    let mut shown = 0usize;
    for arc in r.arcs() {
        if shown >= limit {
            println!("... ({} more arcs)", r.nnz() as usize - shown);
            break;
        }
        println!(
            "{:>10}  {:>3}  {:>5}  {:>10.6}  {:>12.6}",
            arc.index,
            arc.phase.trit(),
            arc.phase.sigma(),
            arc.phase.p(),
            arc.phase.theta()
        );
        shown += 1;
    }
    0
}

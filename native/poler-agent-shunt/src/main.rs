//! POLER Studio IDE - Zero-Token Crystal Shunt & Tool-Forced Agent Gateway (Rust 1.99)
//!
//! Replaces cloud-context bloat with local mmap zero-copy crystals:
//! - Intercepts massive payloads (1M+ tokens / dumps / logs)
//! - Persists raw payload into local crystal cache (`~/.poler/crystals/*.crystal`)
//! - Emits $O(1)$ token descriptor with entrypoint metadata
//! - Forces LLM to navigate via localized tool calls (viewports, SIMD grep, causal nexus)

use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

const SHUNT_THRESHOLD_BYTES: u64 = 16 * 1024; // 16 KiB threshold

#[derive(Debug, Serialize, Deserialize)]
pub struct CrystalDescriptor {
    pub shunted: bool,
    pub crystal_id: String,
    pub crystal_path: String,
    pub bytes: u64,
    pub lines: u64,
    pub preview: Vec<String>,
    pub descriptor_prompt: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum ShuntCommand {
    #[serde(rename = "shunt")]
    Shunt { payload: Option<String>, path: Option<String> },
    #[serde(rename = "view")]
    View { crystal_id: String, start_line: u64, count: usize },
    #[serde(rename = "grep")]
    Grep { crystal_id: String, query: String, limit: Option<usize> },
    #[serde(rename = "stats")]
    Stats { crystal_id: String },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShuntResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<CrystalDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hits: Option<Vec<GrepMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GrepMatch {
    pub line: u64,
    pub byte_offset: u64,
    pub text: String,
}

fn get_crystal_dir() -> PathBuf {
    let mut dir = dirs_or_home();
    dir.push(".poler");
    dir.push("crystals");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn dirs_or_home() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else {
        std::env::temp_dir()
    }
}

fn simple_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data.iter().take(1024 * 1024) { // Fast sample hash
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Creates a crystal from raw bytes or an existing file
pub fn shunt_bytes(data: &[u8], original_name: Option<&str>) -> io::Result<CrystalDescriptor> {
    let crystal_dir = get_crystal_dir();
    let id = simple_hash(data);
    let crystal_path = crystal_dir.join(format!("{}.crystal", id));

    // Persist to crystal
    fs::write(&crystal_path, data)?;

    // Scan lines (SIMD memchr)
    let total_lines = data.iter().filter(|&&b| b == b'\n').count() as u64 + 1;

    // Grab up to 5 preview lines
    let mut preview = Vec::new();
    let mut line_count = 0;
    for line in data.split(|&b| b == b'\n') {
        if line_count >= 5 {
            break;
        }
        let line_str = String::from_utf8_lossy(line).chars().take(200).collect::<String>();
        preview.push(line_str);
        line_count += 1;
    }

    let origin = original_name.unwrap_or("inline_payload");
    let prompt = format!(
        "[POLER ZERO-TOKEN CRYSTAL SHUNT ACTIVE]\n\
        Source: {origin}\n\
        Crystal ID: {id}\n\
        Size: {:.2} MB ({} bytes, {} lines)\n\
        Notice: Raw data is cached locally and was NOT sent to cloud network.\n\
        To inspect contents, you MUST use tool calls:\n\
        - `poler_view_crystal(id: \"{id}\", start_line: N, count: M)`\n\
        - `poler_grep_crystal(id: \"{id}\", query: \"...\")`",
        data.len() as f64 / (1024.0 * 1024.0),
        data.len(),
        total_lines
    );

    Ok(CrystalDescriptor {
        shunted: true,
        crystal_id: id,
        crystal_path: crystal_path.to_string_lossy().to_string(),
        bytes: data.len() as u64,
        lines: total_lines,
        preview,
        descriptor_prompt: prompt,
    })
}

pub fn shunt_file(path: &Path) -> io::Result<CrystalDescriptor> {
    let metadata = fs::metadata(path)?;
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    shunt_bytes(&mmap, path.file_name().and_then(|s| s.to_str()))
}

pub fn view_crystal(crystal_id: &str, start_line: u64, count: usize) -> io::Result<Vec<String>> {
    let crystal_path = get_crystal_dir().join(format!("{}.crystal", crystal_id));
    let file = File::open(&crystal_path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    let mut current_line: u64 = 1;
    let mut result = Vec::new();

    for line in mmap.split(|&b| b == b'\n') {
        if current_line >= start_line && result.len() < count {
            result.push(String::from_utf8_lossy(line).to_string());
        }
        if result.len() >= count {
            break;
        }
        current_line += 1;
    }

    Ok(result)
}

pub fn grep_crystal(crystal_id: &str, query: &str, limit: usize) -> io::Result<Vec<GrepMatch>> {
    let crystal_path = get_crystal_dir().join(format!("{}.crystal", crystal_id));
    let file = File::open(&crystal_path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    let query_bytes = query.as_bytes();
    let mut hits = Vec::new();
    let mut current_line: u64 = 1;
    let mut byte_offset: u64 = 0;

    for line in mmap.split(|&b| b == b'\n') {
        if line.windows(query_bytes.len()).any(|w| w == query_bytes) {
            hits.push(GrepMatch {
                line: current_line,
                byte_offset,
                text: String::from_utf8_lossy(line).trim().to_string(),
            });
            if hits.len() >= limit {
                break;
            }
        }
        byte_offset += line.len() as u64 + 1;
        current_line += 1;
    }

    Ok(hits)
}

pub fn run_server() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line_res in stdin.lock().lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let cmd: Result<ShuntCommand, _> = serde_json::from_str(line);
        let resp = match cmd {
            Ok(ShuntCommand::Ping) => ShuntResponse {
                ok: true,
                descriptor: None,
                lines: None,
                hits: None,
                error: None,
            },
            Ok(ShuntCommand::Shunt { payload, path }) => {
                if let Some(p) = path {
                    match shunt_file(Path::new(&p)) {
                        Ok(desc) => ShuntResponse {
                            ok: true,
                            descriptor: Some(desc),
                            lines: None,
                            hits: None,
                            error: None,
                        },
                        Err(e) => ShuntResponse {
                            ok: false,
                            descriptor: None,
                            lines: None,
                            hits: None,
                            error: Some(e.to_string()),
                        },
                    }
                } else if let Some(text) = payload {
                    match shunt_bytes(text.as_bytes(), None) {
                        Ok(desc) => ShuntResponse {
                            ok: true,
                            descriptor: Some(desc),
                            lines: None,
                            hits: None,
                            error: None,
                        },
                        Err(e) => ShuntResponse {
                            ok: false,
                            descriptor: None,
                            lines: None,
                            hits: None,
                            error: Some(e.to_string()),
                        },
                    }
                } else {
                    ShuntResponse {
                        ok: false,
                        descriptor: None,
                        lines: None,
                        hits: None,
                        error: Some("Missing payload or path".to_string()),
                    }
                }
            }
            Ok(ShuntCommand::View { crystal_id, start_line, count }) => {
                match view_crystal(&crystal_id, start_line, count) {
                    Ok(lines) => ShuntResponse {
                        ok: true,
                        descriptor: None,
                        lines: Some(lines),
                        hits: None,
                        error: None,
                    },
                    Err(e) => ShuntResponse {
                        ok: false,
                        descriptor: None,
                        lines: None,
                        hits: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            Ok(ShuntCommand::Grep { crystal_id, query, limit }) => {
                match grep_crystal(&crystal_id, &query, limit.unwrap_or(50)) {
                    Ok(hits) => ShuntResponse {
                        ok: true,
                        descriptor: None,
                        lines: None,
                        hits: Some(hits),
                        error: None,
                    },
                    Err(e) => ShuntResponse {
                        ok: false,
                        descriptor: None,
                        lines: None,
                        hits: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            Ok(ShuntCommand::Stats { crystal_id }) => {
                let crystal_path = get_crystal_dir().join(format!("{}.crystal", crystal_id));
                match fs::metadata(&crystal_path) {
                    Ok(meta) => ShuntResponse {
                        ok: true,
                        descriptor: Some(CrystalDescriptor {
                            shunted: true,
                            crystal_id: crystal_id.clone(),
                            crystal_path: crystal_path.to_string_lossy().to_string(),
                            bytes: meta.len(),
                            lines: 0,
                            preview: vec![],
                            descriptor_prompt: format!("Crystal: {crystal_id}"),
                        }),
                        lines: None,
                        hits: None,
                        error: None,
                    },
                    Err(e) => ShuntResponse {
                        ok: false,
                        descriptor: None,
                        lines: None,
                        hits: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => ShuntResponse {
                ok: false,
                descriptor: None,
                lines: None,
                hits: None,
                error: Some(format!("Invalid command JSON: {e}")),
            },
        };

        let json_str = serde_json::to_string(&resp).unwrap_or_else(|_| "{\"ok\":false}".to_string());
        writeln!(out, "{}", json_str)?;
        out.flush()?;
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--serve" {
        run_server()
    } else if args.len() > 2 && args[1] == "shunt" {
        let path = Path::new(&args[2]);
        let desc = shunt_file(path)?;
        println!("{}", serde_json::to_string_pretty(&desc)?);
        Ok(())
    } else if args.len() > 4 && args[1] == "view" {
        let id = &args[2];
        let line: u64 = args[3].parse().unwrap_or(1);
        let count: usize = args[4].parse().unwrap_or(50);
        let lines = view_crystal(id, line, count)?;
        for l in lines {
            println!("{}", l);
        }
        Ok(())
    } else {
        println!("POLER Agent Shunt Gateway v0.1.0 (Zero-Token Crystal)");
        println!("Usage:");
        println!("  poler-agent-shunt --serve           (Run JSON-RPC stdio daemon)");
        println!("  poler-agent-shunt shunt <FILE>      (Shunt file into crystal)");
        println!("  poler-agent-shunt view <ID> <L> <C> (View lines from crystal)");
        Ok(())
    }
}

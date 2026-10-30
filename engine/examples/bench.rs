#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Микробенчмарк poler-engine: плотный/разреженный корпуса + пиковая память.
//!
//! Запуск: `cargo run --release --example bench`
#![allow(clippy::field_reassign_with_default)]

use poler_engine::{scan_path_with_stats, EngineConfig, ResonanceMode};
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

/// Пиковое потребление RSS процесса (VmHWM, Linux), в МБ.
fn peak_rss_mb() -> Option<f64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let hwm = status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?;
    hwm.parse::<f64>().ok().map(|kb| kb / 1024.0)
}

fn main() {
    // ---------- Плотный корпус ----------
    let files = 400usize;
    let paragraphs = 25usize;
    let dir = TempDir::new().expect("tempdir");
    let mut total_bytes = 0usize;

    for f in 0..files {
        let mut text = format!(
            "# Глава {f}\n\n**Метрика: Т-{}**\n**Локация: Сектор {}**\n**Субъекты: Оператор, Нокс**\n\n",
            f % 50,
            f % 10
        );
        for p in 0..paragraphs {
            text.push_str(&format!(
                "Абзац {p} содержит редкие минералы кварц и базальт с индексом {}. \
                 Нокс ведёт наблюдение {p}. Система не должна превышать лимит давления. \
                 Критически важно поддерживать баланс шунта.\n\n",
                f * paragraphs + p
            ));
        }
        fs::write(dir.path().join(format!("chapter_{f:04}.md")), &text).unwrap();
        total_bytes += text.len();
    }

    let cfg = EngineConfig::default();
    println!(
        "Плотный корпус: {files} файлов, {:.2} МБ",
        total_bytes as f64 / 1024.0 / 1024.0
    );

    let _ = scan_path_with_stats(dir.path(), "нокс", &cfg); // прогрев кэша страниц

    let t = Instant::now();
    let (res, stats) = scan_path_with_stats(dir.path(), "нокс", &cfg);
    let elapsed = t.elapsed();
    println!(
        "  hits-режим:  {:.0} мс | хитов {} (возвращено {}) | токенов {} | файлов/с {:.0}",
        elapsed.as_secs_f64() * 1000.0,
        stats.total_hits,
        res.anchors.len(),
        stats.total_tokens,
        files as f64 / elapsed.as_secs_f64()
    );

    let mut cfg_field = EngineConfig::default();
    cfg_field.resonance_mode = ResonanceMode::Field;
    let t = Instant::now();
    let (res2, stats2) = scan_path_with_stats(dir.path(), "нокс", &cfg_field);
    let field_elapsed = t.elapsed();
    println!(
        "  field-режим: {:.0} мс | хитов {} (возвращено {})",
        field_elapsed.as_secs_f64() * 1000.0,
        stats2.total_hits,
        res2.anchors.len()
    );

    // ---------- Большой корпус ~50 МБ (стресс памяти) ----------
    let big_files = 160usize;
    let big_paras = 120usize;
    let bdir = TempDir::new().expect("tempdir");
    let mut big_bytes = 0usize;
    for f in 0..big_files {
        let mut text = format!("# Документ {f}\n\n**Метрика: Т-{}**\n\n", f % 40);
        for p in 0..big_paras {
            // уникальные токены для реалистичного словаря корпуса
            text.push_str(&format!(
                "Раздел {p} описывает узел node_{f}_{p} и канал link_{f}_{p}. \
                 Нокс проводит регламентную проверку {p}. Система не должна превышать порог. \
                 Базальт и кварц регистрируются датчиками.\n\n"
            ));
        }
        fs::write(bdir.path().join(format!("big_{f:04}.md")), &text).unwrap();
        big_bytes += text.len();
    }
    let _ = scan_path_with_stats(bdir.path(), "нокс", &EngineConfig::default());
    let t = Instant::now();
    let (res3, stats3) = scan_path_with_stats(bdir.path(), "нокс", &EngineConfig::default());
    let big_elapsed = t.elapsed();
    println!(
        "Большой корпус: {:.1} МБ, {} файлов | {:.0} мс | хитов {} (возвращено {}) | токенов {}",
        big_bytes as f64 / 1024.0 / 1024.0,
        big_files,
        big_elapsed.as_secs_f64() * 1000.0,
        stats3.total_hits,
        res3.anchors.len(),
        stats3.total_tokens
    );

    if let Some(mb) = peak_rss_mb() {
        println!("Пиковая память процесса (VmHWM): {mb:.0} МБ");
    }
}

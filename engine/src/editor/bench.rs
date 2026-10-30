//! poler-edit :: бенчмарк ядра для питча (docs/POLER_EDIT.md).
//!
//! `poler-engine --edit-bench <FILE> [--edit-bench-query <QUERY>]`:
//! открытие (мс), полный SIMD line-index (ГБ/с), поиск (ГБ/с), peak RSS.
//! Работает на файлах любого размера — от крошечных конфигов до
//! терабайтных логов: чтение нулевое, пока его явно не попросят.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::buffer::PolerBuffer;

fn peak_rss_kb() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                return rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
    }
    0
}

fn human_bytes(n: u64) -> String {
    if n >= 1 << 30 {
        format!("{:.2} GiB", n as f64 / (1u64 << 30) as f64)
    } else if n >= 1 << 20 {
        format!("{:.1} MiB", n as f64 / (1u64 << 20) as f64)
    } else if n >= 1 << 10 {
        format!("{:.1} KiB", n as f64 / (1u64 << 10) as f64)
    } else {
        format!("{n} B")
    }
}

/// Печатает таблицу цифр; возвращает код выхода.
pub fn run_edit_bench(path: &Path, query: &str) -> i32 {
    println!("poler-edit core benchmark");
    println!("file  : {}", path.display());
    let meta = std::fs::metadata(path);
    if let Ok(m) = &meta {
        println!("size  : {} ({})", human_bytes(m.len()), m.len());
    }

    // 1. Открытие: mmap + нарезка метадаты, ноль чтения данных.
    let t0 = Instant::now();
    let mut buf = match PolerBuffer::open(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("open error: {e}");
            return 2;
        }
    };
    let open_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("open  : {open_ms:.2} ms  (mmap zero-copy, pieces: {})", buf.pieces_len());

    // 2. Полный line-index: SIMD memchr по всем кускам.
    let total = buf.total_len();
    let t1 = Instant::now();
    let mut last_gbps = 0.0;
    let res = buf.index_all(&AtomicBool::new(false), |done, tot| {
        let el = t1.elapsed().as_secs_f64();
        if el > 0.0 && tot > 0 {
            last_gbps = (done as f64 / (1024.0 * 1024.0 * 1024.0)) / el;
        }
    });
    let (lines, completed) = match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("index error: {e}");
            return 2;
        }
    };
    let index_s = t1.elapsed().as_secs_f64();
    let index_gbps = if index_s > 0.0 {
        (total as f64 / (1024.0 * 1024.0 * 1024.0)) / index_s
    } else {
        0.0
    };
    let _ = last_gbps;
    if completed {
        println!("index : {index_s:.3} s  ({index_gbps:.2} GiB/s SIMD)  lines: {lines}");
    } else {
        println!("index : прерван");
    }

    // 3. Поиск: SIMD Aho-Corasick поверх mmap.
    let t2 = Instant::now();
    let st = match buf.search(query, false, 10_000, &AtomicBool::new(false)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("search error: {e}");
            return 2;
        }
    };
    let search_s = t2.elapsed().as_secs_f64();
    // Честная скорость: сколько РЕАЛЬНО просканировано (truncated → часть).
    // scanned == 0 означает, что лимит hits исчерпан внутри первого куска.
    let scanned = st.scanned_bytes;
    let search_gbps = if search_s > 0.0 && scanned > 0 {
        (scanned as f64 / (1024.0 * 1024.0 * 1024.0)) / search_s
    } else {
        0.0
    };
    let scanned_note = if scanned == 0 && st.truncated {
        String::from("early-exit: hits в первом же куске")
    } else {
        format!("scanned {} / {}", human_bytes(scanned), human_bytes(total))
    };
    println!(
        "search: '{query}' -> {} hits, {:.3} s ({search_gbps:.2} GiB/s, {scanned_note}), truncated={}",
        st.hits.len(),
        search_s,
        st.truncated
    );

    // 4. Память процесса (включая mmap-страницы, отдаваемые ядру).
    let rss = peak_rss_kb();
    println!("peak  : {} RSS (mmap-страницы выселяемы ядром)", human_bytes(rss * 1024));

    println!();
    println!(
        "json  : {{\"open_ms\": {open_ms:.3}, \"index_s\": {index_s:.3}, \
         \"index_gbps\": {index_gbps:.2}, \"search_s\": {search_s:.3}, \
         \"search_gbps\": {search_gbps:.2}, \"searched_bytes\": {scanned}, \
         \"lines\": {lines}, \"bytes\": {total}, \"peak_rss_bytes\": {}}}",
        rss * 1024
    );
    0
}

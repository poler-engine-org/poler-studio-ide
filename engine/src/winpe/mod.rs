//! winpe — нативний Win64-субстрат poler-box: виконання PE32+ (AMD64)
//! бінарників на Linux без Wine і без віртуалізації.
//!
//! Порт ідей poler-os (zig-kernel: pe.zig, win32_crt.zig) на Rust +
//! повний x64 C++ SEH. Архітектура:
//!  * runtime.rs — відображення образу, IAT → тunki, міст SysV↔Win64, TEB/PEB через GS
//!  * pe.rs     — PE32+ парсер (секції, імпорти, .pdata)
//!  * crt.rs    — msvcrt (printf-формтер, __getmainargs, _initterm, qsort)
//!  * api.rs    — World (реентерабельний) + диспетчер + kernel32
//!  * api_ext.rs — CRT-IO, 64-бітні хелпери, user32/advapi
//!  * seh.rs    — власний walker C++ виключень (throw → деструктори → catch)

pub mod api;
pub mod api_ext;
pub mod crt;
pub mod pe;
pub mod runtime;
pub mod seh;

/// Виконати PE32+ образ. Не повертається при успіху (процес завершується
/// кодом ExitProcess/entry). Повертає Err лише при помилці завантаження.
pub fn winexec(
    image: &[u8],
    argv: Vec<String>,
    env: Vec<(String, String)>,
) -> Result<i32, String> {
    if !pe::is_pe32_plus(image) {
        return Err("не PE32+ (AMD64)".into());
    }
    let w = api::World::get();
    w.init_registry();
    let trace = std::env::var("POLER_WIN_TRACE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0u8);
    w.set_runtime(argv, env, trace);
    w.build_env_blocks();
    let mut sub = unsafe { runtime::Substrate::load(image, w)? };
    unsafe { sub.setup_teb(w)? };
    // не повертається: entry → ExitProcess → std::process::exit
    sub.enter();
}

/// Запустити PE-файл з диска (господарський режим, без коробки).
pub fn winexec_file(path: &std::path::Path, args: &[String]) -> Result<i32, String> {
    let image = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut argv = vec![path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "app.exe".into())];
    argv.extend(args.iter().cloned());
    let env: Vec<(String, String)> = std::env::vars().collect();
    winexec(&image, argv, env)
}

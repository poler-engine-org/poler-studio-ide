// build.rs — M4: сборка Zig-криптоядра (os/core) для фичи `pnd-ffi`
//          + M5: компиляция No-Mul роторов (буквы мира + химия) в libpoler_rotors.a.
//
// Без фичи pnd-ffi Zig-часть — полный no-op: обычная сборка движка
// не требует Zig-тулчейна (как и до M4).
//
// Роторы собираются всегда, когда в системе есть `as` (GNU binutils):
// два .s-монолита из корня репо → OUT_DIR/*.o → libpoler_rotors.a →
// статическая линковка + cfg(asm_rotors). Если `as`/`ar` недоступны
// (нетрадиционная платформа/кросс-компиляция) — движок деградирует
// на чисто-Rust путь с идентичной семантикой (универсальность важнее скорости).
//
// С фичей pnd-ffi порядок разрешения окружения:
//   1. POLER_CORE_LIB=... — готовая директория с libpoler_core.a
//      (например, os/core/zig-out/lib после ручного `zig build`);
//   2. POLER_ZIG=/путь/к/zig — явный путь к бинарнику Zig 0.14.0;
//   3. `zig` в PATH.
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn find_zig() -> Option<PathBuf> {
    if let Some(p) = env::var_os("POLER_ZIG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let cand = dir.join("zig");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=os/core/poler_core.zig");
    println!("cargo:rerun-if-changed=os/core/abi.zig");
    println!("cargo:rerun-if-changed=os/core/poler_exec.zig");
    println!("cargo:rerun-if-changed=os/core/build.zig");
    // Смена env-переменных должна перезапускать скрипт: иначе линкер
    // держит устаревший -L (мина, найденная при смене каталога репо
    // с включённой фичей pnd-ffi — POLER_CORE_LIB указывал в старый клон).
    println!("cargo:rerun-if-env-changed=POLER_CORE_LIB");
    println!("cargo:rerun-if-env-changed=POLER_ZIG");

    build_asm_rotors();

    if env::var_os("CARGO_FEATURE_PND_FFI").is_none() {
        return; // фича выключена — нулевые требования к окружению
    }

    // 1. Готовая библиотека от пользователя
    if let Some(libdir) = env::var_os("POLER_CORE_LIB") {
        println!("cargo:rustc-link-search=native={}", libdir.to_string_lossy());
        println!("cargo:rustc-link-lib=static=poler_core");
        return;
    }

    // 2/3. Zig из POLER_ZIG или PATH
    let zig = find_zig().unwrap_or_else(|| panic!(
        "фича pnd-ffi требует Zig 0.14.0: установите zig в PATH, задайте\n\
         POLER_ZIG=/путь/к/zig или укажите готовую библиотеку\n\
         POLER_CORE_LIB=os/core/zig-out/lib (после ручного `zig build` в os/core)"
    ));
    let status = Command::new(&zig)
        .args(["build", "-Doptimize=ReleaseSafe"])
        .current_dir("os/core")
        .status()
        .expect("не удалось запустить `zig build` в os/core");
    assert!(status.success(), "zig build (os/core) завершился с ошибкой");

    println!("cargo:rustc-link-search=native=os/core/zig-out/lib");
    println!("cargo:rustc-link-lib=static=poler_core");
}

/// M5: No-Mul роторы — две .s-страницы → libpoler_rotors.a → статическая линковка.
/// При отсутствии `as`/`ar` молча деградирует на Rust-фолбэк (без cfg).
fn build_asm_rotors() {
    let rotors = [
        "archetype_unrolled_resonance_50k.s",
        "chem_unrolled_resonance_50k.s",
    ];
    println!("cargo:rerun-if-changed=archetype_unrolled_resonance_50k.s");
    println!("cargo:rerun-if-changed=chem_unrolled_resonance_50k.s");

    let out_dir = match env::var_os("OUT_DIR") {
        Some(d) => PathBuf::from(d),
        None => return,
    };
    let manifest = match env::var_os("CARGO_MANIFEST_DIR") {
        Some(d) => PathBuf::from(d),
        None => return,
    };

    // Целевая платформа: роторы — чистый x86_64; на иных таргетах — Rust-фолбэк.
    let target = env::var("TARGET").unwrap_or_default();
    let is_x86_64 = (target.contains("x86_64") || target.starts_with("x86_64")) && target.contains("linux");
    let is_host_build = env::var("HOST").map(|h| h == target).unwrap_or(true);
    if !is_x86_64 {
        println!("cargo:warning=роторы .s — только x86_64 (TARGET={target}); використовується Rust-фолбэк");
        return;
    }
    let _ = is_host_build; // `as` не знает таргет-префикс — для нативной x86_64 сборки этого достаточно

    let mut objs: Vec<PathBuf> = Vec::with_capacity(rotors.len());
    for rel in rotors {
        let src = manifest.join(rel);
        if !src.is_file() {
            println!("cargo:warning=ротор не найден: {} — Rust-фолбэк", src.display());
            return;
        }
        let obj = out_dir.join(rel).with_extension("o");
        let status = Command::new("as")
            .arg(&src)
            .arg("-o")
            .arg(&obj)
            .status();
        match status {
            Ok(st) if st.success() => objs.push(obj),
            _ => {
                println!("cargo:warning=`as` не зібрав {rel} — Rust-фолббек");
                return;
            }
        }
    }

    let lib = out_dir.join("libpoler_rotors.a");
    let mut cmd = Command::new("ar");
    cmd.arg("crs").arg(&lib);
    for o in &objs {
        cmd.arg(o);
    }
    match cmd.status() {
        Ok(st) if st.success() => {}
        _ => {
            println!("cargo:warning=`ar` не зібрав libpoler_rotors.a — Rust-фолббек");
            return;
        }
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=poler_rotors");
    println!("cargo:rustc-cfg=asm_rotors");
}

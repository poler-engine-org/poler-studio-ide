// build.rs — Автоматическая линковка libcint и FFI-генерация через bindgen

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=cint");

    if let Ok(lib_dir) = env::var("LIBCINT_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
    }
}

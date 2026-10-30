//! dlopen-обёртка над `libp3ffi.so` (C-ABI слой P³, Zig 0.14.0).
//!
//! Никаких внешних крейтов: dlopen/dlsym объявлены напрямую через
//! `extern "C"` (glibc ≥ 2.34 держит их в libc; на старых системах
//! libdl.so остаётся стабом — линковка проходит в обоих случаях).
//!
//! Поиск библиотеки (первый найденный выигрывает):
//! 1. `P3_FFI_LIB` (точный путь, переменная окружения);
//! 2. `ffi/libp3ffi-linux-x86_64.so` рядом с бинарником
//!    (раскладка релиза: `poler-engine + ffi/`);
//! 3. `../ffi/…` от бинарника (target/release → корень репо);
//! 4. `ffi/…` от `CARGO_MANIFEST_DIR` (прогон `cargo test`);
//! 5. соседний `P3_Engine/zig-out/lib/libp3ffi.so` (dev-песочница);
//! 6. голое имя — системный поиск dlopen (LD_LIBRARY_PATH).

use std::ffi::{c_char, c_int, c_void, CStr, CString};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::native::{Hom4, Pgl4};

const RTLD_NOW: c_int = 0x2;
const RTLD_LOCAL: c_int = 0x0;

#[cfg(unix)]
extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}
#[cfg(not(unix))]
unsafe fn dlopen(_filename: *const c_char, _flag: c_int) -> *mut c_void { std::ptr::null_mut() }
#[cfg(not(unix))]
unsafe fn dlsym(_handle: *mut c_void, _symbol: *const c_char) -> *mut c_void { std::ptr::null_mut() }
#[cfg(not(unix))]
unsafe fn dlclose(_handle: *mut c_void) -> c_int { 0 }

/// Загрузить символ и привести к типу указателя на функцию.
/// `*mut c_void` и указатели на `extern "C" fn` одинаково 8 байт на
/// x86_64/aarch64 — контроль размеров защищает от тихих сюрпризов.
///
/// # Safety
/// `handle` должен быть живым dlopen-хэндлом, а `T` — корректным
/// типом указателя на функцию C-ABI.
unsafe fn load_sym<T>(handle: *mut c_void, name: &str) -> Result<T, String> {
    debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*mut c_void>());
    let c = CString::new(name).map_err(|_| "NUL в имени символа".to_string())?;
    let p = dlsym(handle, c.as_ptr());
    if p.is_null() {
        return Err(format!("символ {name} отсутствует"));
    }
    Ok(std::mem::transmute_copy(&p))
}

/// Ожидаемая версия ABI C-слоя P³ (см. `p3_ffi.zig: ABI_VERSION`).
pub const EXPECTED_ABI: u32 = 1;

pub type FnAbiVersion = unsafe extern "C" fn() -> u32;
pub type FnKernelTag = unsafe extern "C" fn() -> *const c_char;
pub type FnFsDistance = unsafe extern "C" fn(a: *const f64, b: *const f64) -> f64;
pub type FnPglIdentity = unsafe extern "C" fn(out: *mut f64);
pub type FnPglMul = unsafe extern "C" fn(out: *mut f64, a: *const f64, b: *const f64);
pub type FnPglTranspose = unsafe extern "C" fn(out: *mut f64, m: *const f64);
pub type FnPglApply = unsafe extern "C" fn(out: *mut f64, m: *const f64, v: *const f64);
pub type FnPglDet = unsafe extern "C" fn(m: *const f64) -> f64;
pub type FnGivens4 =
    unsafe extern "C" fn(out: *mut f64, i: u32, j: u32, theta: f64);
pub type FnSpectralProjector = unsafe extern "C" fn(out: *mut f64, v: *const f64);
pub type FnIdempotentRank = unsafe extern "C" fn(m: *const f64) -> f64;
pub type FnIdempotentResidual = unsafe extern "C" fn(m: *const f64) -> f64;
pub type FnOrthoResidual = unsafe extern "C" fn(m: *const f64) -> f64;
pub type FnRenderFrame = unsafe extern "C" fn(
    pts: *const f64,
    n_pts: u32,
    seg_ids: *const u8,
    pairs: *const u32,
    n_pairs: u32,
    width: u32,
    height: u32,
    focal: f64,
    cam_dist: f64,
    yaw: f64,
    pitch: f64,
    thickness: u32,
    rgb: *mut u8,
    depth: *mut f32,
    seg: *mut u8,
);

/// Загруженная библиотека P³. Держит dlopen-хэндл; символы вычитаны
/// один раз при открытии и проверены на ABI.
pub struct P3Lib {
    _handle: *mut c_void,
    pub path: PathBuf,
    pub abi_version: u32,
    pub kernel_tag: String,

    pub fs_distance: FnFsDistance,
    pub pgl_identity: FnPglIdentity,
    pub pgl_mul: FnPglMul,
    pub pgl_transpose: FnPglTranspose,
    pub pgl_apply: FnPglApply,
    pub pgl_det: FnPglDet,
    pub givens4: FnGivens4,
    pub spectral_projector: FnSpectralProjector,
    pub idempotent_rank: FnIdempotentRank,
    pub idempotent_residual: FnIdempotentResidual,
    pub ortho_residual: FnOrthoResidual,
    pub render_frame: FnRenderFrame,
}

impl Drop for P3Lib {
    fn drop(&mut self) {
        if !self._handle.is_null() {
            unsafe { dlclose(self._handle) };
        }
    }
}

/// Список путей поиска (в порядке приоритета).
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let so_names = [
        "libp3ffi-linux-x86_64.so",
        "libp3ffi.so",
    ];

    // 1. Переменная окружения
    if let Ok(p) = std::env::var("P3_FFI_LIB") {
        out.push(PathBuf::from(p));
    }

    let mut add_with = |base: Option<&Path>| {
        if let Some(base) = base {
            for n in so_names {
                out.push(base.join("ffi").join(n));
                out.push(base.join(n));
            }
        }
    };

    // 2–3. Рядом с бинарником и уровнем выше (target/release → корень)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            add_with(Some(dir));
            if let Some(parent) = dir.parent() {
                add_with(Some(parent));
                // target/release → две вверх до корня репозитория
                if let Some(grand) = parent.parent() {
                    add_with(Some(grand));
                }
            }
        }
    }

    // 4. Манифест крейта (cargo test)
    add_with(Some(Path::new(env!("CARGO_MANIFEST_DIR"))));

    // 5. Соседняя dev-копия P3_Engine (песочница POLER BOX)
    add_with(Some(Path::new(
        "/home/z/my-project/P3_Engine/zig-out/lib",
    )));

    // 6. Системный поиск по имени
    out.push(PathBuf::from("libp3ffi.so"));
    out
}

impl P3Lib {
    /// Найти и загрузить библиотеку; проверяет ABI-рукопожатие.
    pub fn open() -> Result<P3Lib, String> {
        let mut errors = Vec::new();
        for path in candidate_paths() {
            if !path.exists() && path.file_name().is_some() {
                // последний кандидат — системное имя; проверять нечего
                if path.parent() != Some(Path::new("")) {
                    continue;
                }
            }
            match unsafe { Self::dlopen_checked(&path) } {
                Ok(lib) => return Ok(lib),
                Err(e) => errors.push(format!("{}: {}", path.display(), e)),
            }
        }
        Err(format!(
            "libp3ffi не найдена/не загружена. Пробовали:\n  {}\n\
             Пересоберите: ffi/build.sh (см. ffi/build.sh --help) или \
             задайте P3_FFI_LIB=/путь/к/libp3ffi.so",
            errors.join("\n  ")
        ))
    }

    unsafe fn dlopen_checked(path: &Path) -> Result<P3Lib, String> {
        #[cfg(unix)]
        let path_bytes = path.as_os_str().as_bytes().to_vec();
        #[cfg(not(unix))]
        let path_bytes = path.to_string_lossy().as_bytes().to_vec();
        let c_path = CString::new(path_bytes)
            .map_err(|_| "путь содержит NUL".to_string())?;
        let handle = dlopen(c_path.as_ptr(), RTLD_NOW | RTLD_LOCAL);
        if handle.is_null() {
            return Err("dlopen failed".into());
        }

        // Рукопожатие ДО загрузки остальных символов
        let abi_fn: FnAbiVersion = load_sym(handle, "p3ffi_abi_version")?;
        let abi = unsafe { abi_fn() };
        if abi != EXPECTED_ABI {
            return Err(format!("ABI mismatch: lib={abi}, engine={EXPECTED_ABI}"));
        }
        let tag_fn: FnKernelTag = load_sym(handle, "p3ffi_kernel_tag")?;
        let tag_c = unsafe { tag_fn() };
        let tag = if tag_c.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(tag_c) }.to_string_lossy().into_owned()
        };

        Ok(P3Lib {
            _handle: handle,
            path: path.to_path_buf(),
            abi_version: abi,
            kernel_tag: tag,
            fs_distance: load_sym(handle, "p3ffi_fs_distance")?,
            pgl_identity: load_sym(handle, "p3ffi_pgl_identity")?,
            pgl_mul: load_sym(handle, "p3ffi_pgl_mul")?,
            pgl_transpose: load_sym(handle, "p3ffi_pgl_transpose")?,
            pgl_apply: load_sym(handle, "p3ffi_pgl_apply")?,
            pgl_det: load_sym(handle, "p3ffi_pgl_det")?,
            givens4: load_sym(handle, "p3ffi_givens4")?,
            spectral_projector: load_sym(handle, "p3ffi_spectral_projector")?,
            idempotent_rank: load_sym(handle, "p3ffi_idempotent_rank")?,
            idempotent_residual: load_sym(handle, "p3ffi_idempotent_residual")?,
            ortho_residual: load_sym(handle, "p3ffi_ortho_residual")?,
            render_frame: load_sym(handle, "p3ffi_render_frame")?,
        })
    }

    // ------------------------------------------------------------------
    // Безопасные обёртки
    // ------------------------------------------------------------------

    /// d_FS(a, b) через ядро P³ (Zig).
    pub fn fs_distance(&self, a: &Hom4, b: &Hom4) -> f64 {
        unsafe { (self.fs_distance)(a.0.as_ptr(), b.0.as_ptr()) }
    }

    pub fn pgl_identity(&self) -> Pgl4 {
        let mut out = [0.0f64; 16];
        unsafe { (self.pgl_identity)(out.as_mut_ptr()) };
        Pgl4 { data: out }
    }

    pub fn pgl_mul(&self, a: &Pgl4, b: &Pgl4) -> Pgl4 {
        let mut out = [0.0f64; 16];
        unsafe { (self.pgl_mul)(out.as_mut_ptr(), a.data.as_ptr(), b.data.as_ptr()) };
        Pgl4 { data: out }
    }

    pub fn pgl_transpose(&self, m: &Pgl4) -> Pgl4 {
        let mut out = [0.0f64; 16];
        unsafe { (self.pgl_transpose)(out.as_mut_ptr(), m.data.as_ptr()) };
        Pgl4 { data: out }
    }

    pub fn pgl_apply(&self, m: &Pgl4, v: &Hom4) -> Hom4 {
        let mut out = [0.0f64; 4];
        unsafe { (self.pgl_apply)(out.as_mut_ptr(), m.data.as_ptr(), v.0.as_ptr()) };
        Hom4(out)
    }

    pub fn pgl_det(&self, m: &Pgl4) -> f64 {
        unsafe { (self.pgl_det)(m.data.as_ptr()) }
    }

    pub fn givens4(&self, i: u32, j: u32, theta: f64) -> Pgl4 {
        let mut out = [0.0f64; 16];
        unsafe { (self.givens4)(out.as_mut_ptr(), i, j, theta) };
        Pgl4 { data: out }
    }

    pub fn spectral_projector(&self, v: &Hom4) -> Pgl4 {
        let mut out = [0.0f64; 16];
        unsafe { (self.spectral_projector)(out.as_mut_ptr(), v.0.as_ptr()) };
        Pgl4 { data: out }
    }

    pub fn idempotent_rank(&self, m: &Pgl4) -> f64 {
        unsafe { (self.idempotent_rank)(m.data.as_ptr()) }
    }

    pub fn idempotent_residual(&self, m: &Pgl4) -> f64 {
        unsafe { (self.idempotent_residual)(m.data.as_ptr()) }
    }

    pub fn ortho_residual(&self, m: &Pgl4) -> f64 {
        unsafe { (self.ortho_residual)(m.data.as_ptr()) }
    }

    /// Рендер сцены в тройной буфер (RGB + depth + seg). Все буферы
    /// выделяются вызывающей стороной; размеры: rgb 3·w·h, depth w·h,
    /// seg w·h. Точки — однородные [X:Y:Z:W] подряд; pairs — индексы
    /// рёбер (i0, j0, i1, j1, …).
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame(
        &self,
        pts: &[f64],
        seg_ids: &[u8],
        pairs: &[u32],
        width: u32,
        height: u32,
        camera: Camera,
        thickness: u32,
        rgb: &mut [u8],
        depth: &mut [f32],
        seg: &mut [u8],
    ) -> Result<(), String> {
        let n_pts = seg_ids.len();
        if pts.len() != n_pts * 4 {
            return Err(format!("pts: нужно {} f64, есть {}", n_pts * 4, pts.len()));
        }
        if n_pts == 0 {
            return Err("рендер: 0 точек".into());
        }
        let npix = (width as usize) * (height as usize);
        if rgb.len() != npix * 3 || depth.len() != npix || seg.len() != npix {
            return Err("буферы не соответствуют width×height".into());
        }
        unsafe {
            let focal = if camera.focal > 0.0 {
                camera.focal
            } else {
                // авто-фокус: масштаб под разрешение кадра (1.15·высоты)
                1.15 * height as f64
            };
            (self.render_frame)(
                pts.as_ptr(),
                n_pts as u32,
                seg_ids.as_ptr(),
                pairs.as_ptr(),
                (pairs.len() / 2) as u32,
                width,
                height,
                focal,
                camera.cam_dist,
                camera.yaw,
                camera.pitch,
                thickness,
                rgb.as_mut_ptr(),
                depth.as_mut_ptr(),
                seg.as_mut_ptr(),
            );
        }
        Ok(())
    }
}

/// Демонстрационная камера P³-рендера.
/// `focal ≤ 0` — авто-фокус (1.15·высоты кадра): камера влезает в любое
/// разрешение, поэтому дефолт разрешён и для 960×540, и для 96×72.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub focal: f64,
    pub cam_dist: f64,
    pub yaw: f64,
    pub pitch: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            focal: 0.0,
            cam_dist: 3.4,
            yaw: -0.62,
            pitch: 0.34,
        }
    }
}

// =============================================================================
// АВТО-ВЫБОР КРЕМНИЯ (цикл W): FFI-ядро Zig → Rust-близнец
// =============================================================================

/// Рендер тройного буфера с честным фолбэком: сначала C-ABI ядро P³ (Zig),
/// если библиотека не загрузилась (нет файла, чужая архитектура, битая) —
/// Rust-близнец `native::render_frame_native` (тот же алгоритм, зеркало
/// p3-engine/src/p3_ffi.zig). Возвращает `true`, если рисовал Zig.
///
/// Гарантия: `game demo` / `p3 render` / `game window` больше НЕ падают
/// из-за проблем с .so — нативный путь всегда доступен в самом бинарнике.
#[allow(clippy::too_many_arguments)]
pub fn render_frame_auto(
    pts: &[f64],
    seg_ids: &[u8],
    pairs: &[u32],
    width: u32,
    height: u32,
    camera: Camera,
    thickness: u32,
    rgb: &mut [u8],
    depth: &mut [f32],
    seg: &mut [u8],
) -> Result<bool, String> {
    let n_pts = seg_ids.len();
    if pts.len() != n_pts * 4 {
        return Err(format!("pts: нужно {} f64, есть {}", n_pts * 4, pts.len()));
    }
    if n_pts == 0 {
        return Err("рендер: 0 точек".into());
    }
    let npix = (width as usize) * (height as usize);
    if rgb.len() != npix * 3 || depth.len() != npix || seg.len() != npix {
        return Err("буферы не соответствуют width×height".into());
    }
    match P3Lib::open() {
        Ok(lib) => {
            lib.render_frame(pts, seg_ids, pairs, width, height, camera, thickness, rgb, depth, seg)?;
            Ok(true)
        }
        Err(_) => {
            let _ = super::native::render_frame_native(
                pts,
                seg_ids,
                pairs,
                width,
                height,
                camera,
                thickness,
                rgb,
                depth,
                seg,
            );
            Ok(false)
        }
    }
}

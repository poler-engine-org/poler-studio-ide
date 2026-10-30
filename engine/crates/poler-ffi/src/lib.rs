#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! # poler-ffi — C-ABI слой POLER ENGINE для внешних рендереров
//!
//! Цикл W «Симбиоз» (v0.59.0) → X «Обрушение» (v0.60.0): 2-е гармоники Стокса в осцилляторе нормалей.. Это КРЕМНИЙ ФИЗИКИ для Panda3D / Godot /
//! O3DE / любого движка, умеющего ctypes/cffi/C-линковку:
//!
//! ```text
//!   Panda3D (Python, рендер, ассеты)          POLER ENGINE (Rust, физика)
//!   ───────────────────────────────           ────────────────────────────
//!   demo_ocean.py                             libpoler_ffi.so
//!     ctypes.CDLL ─────────────────────────▶    polerf_water_new_u64
//!     GeomVertexData (сетка 128×128)            polerf_water_frame (тик+FFT)
//!     GLSL: Френель/блики/пена  ◀───────────── polerf_water_surface_at
//!     boat float                            ◀─ polerf_water_flow_at
//! ```
//!
//! Физика — спектральная гидродинамика GF(3) (цикл V1): целочисленные
//! фазы-счётчики 3^(p+m), тернарная лестница амплитуд 3^t, дисперсия
//! ω=√(gk+γk³), равновесие Пирсона–Московиц. Ноль f32-дрейфа: состояние
//! моря — 1–2 КБ, «резиновая вода» сеточных движков невозможна по
//! построению (тест `f32_drifts_trit_counter_does_not`).
//!
//! Все функции C-ABI потокобезопасны на РАЗНЫХ хэндлах; хэндл воды
//! владеет своим состоянием. Память: caller не освобождает ничего,
//! кроме хэндла через `polerf_water_free`.

use std::ffi::c_char;
use std::os::raw::c_void;

use poler_engine::game::water::{self, WaterParams, WaterSpectrum};

/// Версия C-ABI слоя (для рукопожатия).
pub const POLERF_ABI: u32 = 1;

/// Хэндл живого моря (владение — на стороне Rust).
pub struct WaterHandle {
    pub sea: WaterSpectrum,
    pub n: usize,
    pub domain: f64,
}

fn version_cstr() -> &'static std::ffi::CString {
    static V: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        std::ffi::CString::new(format!(
            "poler-ffi {} (water GF(3)/Panda3D bridge, abi {})",
            env!("CARGO_PKG_VERSION"),
            POLERF_ABI
        ))
        .expect("NUL в версии")
    })
}

// =============================================================================
// 1. РУКОПОЖАТИЕ
// =============================================================================

/// Строка версии и ABI: "poler-ffi 0.60.0 (water GF(3)/Panda3D bridge, abi 1)".
#[no_mangle]
pub extern "C" fn polerf_version() -> *const c_char {
    version_cstr().as_ptr()
}

/// Номер ABI.
#[no_mangle]
pub extern "C" fn polerf_abi() -> u32 {
    POLERF_ABI
}

// =============================================================================
// 2. МОРЕ (спектральная гидродинамика GF(3))
// =============================================================================

/// Создать море. Параметры: сетка n (степень двойки 16..=1024), число мод,
/// сид, ветер м/с, домен м, направление ветра рад. Остальное — физические
/// константы воды по умолчанию (вязкость/гравитация/капиллярность/триты 6+6+3).
/// Возвращает NULL при невалидных параметрах.
#[no_mangle]
pub extern "C" fn polerf_water_new_u64(
    n: u32,
    modes: u32,
    seed: u64,
    wind: f64,
    domain: f64,
    wind_dir: f64,
) -> *mut c_void {
    let p = WaterParams {
        n: n as usize,
        modes: modes as usize,
        seed,
        wind,
        domain,
        wind_dir,
        ..WaterParams::default()
    };
    if p.validate().is_err() {
        return std::ptr::null_mut();
    }
    let n = p.n;
    let domain = p.domain;
    let sea = water::generate(&p);
    Box::into_raw(Box::new(WaterHandle { sea, n, domain })) as *mut c_void
}

/// Освободить море.
///
/// # Safety
/// `h` — хэндл из `polerf_water_new_u64`, освобождаемый ровно один раз.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_free(h: *mut c_void) {
    if !h.is_null() {
        drop(Box::from_raw(h as *mut WaterHandle));
    }
}

unsafe fn sea<'a>(h: *mut c_void) -> Option<&'a mut WaterHandle> {
    if h.is_null() {
        return None;
    }
    Some(&mut *(h as *mut WaterHandle))
}

/// Шаг физики моря (dt секунд игрового времени).
///
/// # Safety
/// `h` — живой хэндл.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_step(h: *mut c_void, dt: f64) {
    if let Some(w) = sea(h) {
        w.sea.step(dt);
    }
}

/// Кадр одним вызовом: шаг физики (dt > 0; dt ≤ 0 — только сэмпл сетки
/// без продвижения времени) + сетка высот f32 (row-major n×n).
/// `out_len` обязан быть ≥ n·n; возвращает число записанных f32 (0 — ошибка).
///
/// # Safety
/// `out` — валидный для записи буфер `out_len` элементов.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_frame(
    h: *mut c_void,
    dt: f64,
    out: *mut f32,
    out_len: u32,
) -> u32 {
    let Some(w) = sea(h) else { return 0 };
    if dt.is_finite() && dt > 0.0 {
        w.sea.step(dt);
    }
    let field = w.sea.height_field();
    let need = w.n * w.n;
    if (out_len as usize) < need || out.is_null() {
        return 0;
    }
    let dst = std::slice::from_raw_parts_mut(out, need);
    for (d, s) in dst.iter_mut().zip(field.iter()) {
        *d = *s as f32;
    }
    need as u32
}

/// Высота поверхности в точке (x, y) домена; через `dhx`/`dhy` —
/// аналитические наклоны (нормаль волны без разностей!). Возврат — h.
///
/// # Safety
/// `h` — живой хэндл; `dhx`/`dhy` — валидные указатели или NULL.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_surface_at(
    h: *mut c_void,
    x: f64,
    y: f64,
    dhx: *mut f64,
    dhy: *mut f64,
) -> f64 {
    let Some(w) = sea(h) else { return 0.0 };
    let (hgt, sx, sy) = w.sea.surface_at(x, y);
    if !dhx.is_null() {
        *dhx = sx;
    }
    if !dhy.is_null() {
        *dhy = sy;
    }
    hgt
}

/// Течение (u, v, w) в точке (x, y): безвихревое поле скоростей из тех же
/// мод; через `vx`/`vy` — горизонтальная компонента. Возврат — вертикальная w.
///
/// # Safety
/// `h` — живой хэндл; `vx`/`vy` — валидные указатели или NULL.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_flow_at(
    h: *mut c_void,
    x: f64,
    y: f64,
    vx: *mut f64,
    vy: *mut f64,
) -> f64 {
    let Some(w) = sea(h) else { return 0.0 };
    let (u, v, w) = w.sea.flow_at(x, y);
    if !vx.is_null() {
        *vx = u;
    }
    if !vy.is_null() {
        *vy = v;
    }
    w
}

/// Значительная высота волн H_s, м (Пирсон–Московиц из спектра).
///
/// # Safety
/// `h` — живой хэндл.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_significant_height(h: *mut c_void) -> f64 {
    sea(h).map(|w| w.sea.significant_height()).unwrap_or(0.0)
}

/// Кадр + полная геометрия одним вызовом (путь рендер-цикла Panda3D):
/// шаг физики (dt > 0; dt ≤ 0 — только сэмпл) + высоты `h_out` (n·n f32,
/// row-major, каноничный FFT-путь кодека) + аналитические нормали `n_out`
/// (3·n·n f32, (nx,ny,nz), нормированные).
///
/// Скорость: нормали считаются спектральным осциллятором — один powf и
/// один sincos на моду на СТРОКУ, далее 2 FMA на точку-моду (против
/// powf+phase+sin_cos на каждую точку в наивном цикле: ×20 ускорение).
/// Дрейф рекуррентного поворота за строку ≤ n·ε ≈ 1e-13 рад — на семь
/// порядков ниже точности f32-вывода; точные градиенты для физики
/// остаются в `polerf_water_surface_at`.
///
/// # Safety
/// `h_out` — буфер n·n f32; `n_out` — буфер 3·n·n f32.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_geometries_f32(
    h: *mut c_void,
    dt: f64,
    h_out: *mut f32,
    n_out: *mut f32,
    out_len: u32,
) -> u32 {
    let Some(w) = sea(h) else { return 0 };
    if dt.is_finite() && dt > 0.0 {
        w.sea.step(dt);
    }
    let n = w.n;
    let npoints = n * n;
    if (out_len as usize) < npoints || h_out.is_null() || n_out.is_null() {
        return 0;
    }
    // --- высоты: каноничный FFT-путь (тот же, что у VRTX-кодека) ---
    let field = w.sea.height_field();
    let hs = std::slice::from_raw_parts_mut(h_out, npoints);
    for (d, s) in hs.iter_mut().zip(field.iter()) {
        *d = *s as f32;
    }

    // --- нормали: спектральный осциллятор ---
    // прекомпьют мод: (a, ph, a·kx, a·ky, ky, cd, sd) — один powf на моду;
    // цикл X: + 2-я гармоника Стокса (a₂, 2φ, двойной угол поворота) —
    // тот же вклад, что в surface_at (окно вторых гармоник движка).
    struct MO {
        ph: f64,
        ky: f64,
        akx: f64,
        aky: f64,
        cd: f64,
        sd: f64,
        // 2-я гармоника: th2 = 2·k·x + 2·ky·y + 2φ.
        has2: bool,
        ph2: f64,
        ky2: f64,
        akx2: f64,
        aky2: f64,
        cd2: f64,
        sd2: f64,
    }
    let step = w.domain / n as f64;
    let win = w.sea.second_window();
    let modes: Vec<MO> = w
        .sea
        .modes
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let a = w.sea.amplitude(m);
            // sin_cos() → (sin, cos): sd — синус шага, cd — косинус шага
            let (sd, cd) = (m.kx_phys * step).sin_cos();
            let (a2, phi2) = win
                .and_then(|win| w.sea.second_of(i, win))
                .unwrap_or((0.0, 0.0));
            let has2 = a2 != 0.0;
            let (sd2, cd2) = (2.0 * m.kx_phys * step).sin_cos();
            MO {
                ph: w.sea.phase(m),
                ky: m.ky_phys,
                akx: a * m.kx_phys,
                aky: a * m.ky_phys,
                cd,
                sd,
                has2,
                ph2: phi2,
                ky2: 2.0 * m.ky_phys,
                akx2: 2.0 * a2 * m.kx_phys,
                aky2: 2.0 * a2 * m.ky_phys,
                cd2,
                sd2,
            }
        })
        .collect();

    let ns = std::slice::from_raw_parts_mut(n_out, npoints * 3);
    let mut gx = vec![0.0f64; n];
    let mut gy = vec![0.0f64; n];
    let mut k = 0usize;
    for j in 0..n {
        let y = j as f64 * step;
        gx.iter_mut().for_each(|v| *v = 0.0);
        gy.iter_mut().for_each(|v| *v = 0.0);
        for m in &modes {
            // sin_cos() → (sin, cos): s — синус, c — косинус!
            let (mut s, mut c) = (m.ph + m.ky * y).sin_cos();
            let (mut s2, mut c2) = if m.has2 {
                (m.ph2 + m.ky2 * y).sin_cos()
            } else {
                (0.0, 1.0)
            };
            for i in 0..n {
                gx[i] -= m.akx * s;
                gy[i] -= m.aky * s;
                if m.has2 {
                    gx[i] -= m.akx2 * s2;
                    gy[i] -= m.aky2 * s2;
                }
                let c2r = c * m.cd - s * m.sd;
                s = s * m.cd + c * m.sd;
                c = c2r;
                if m.has2 {
                    let c2n = c2 * m.cd2 - s2 * m.sd2;
                    s2 = s2 * m.cd2 + c2 * m.sd2;
                    c2 = c2n;
                }
            }
        }
        for i in 0..n {
            // N = normalize(−gx, −gy, 1)
            let inv = 1.0 / (gx[i] * gx[i] + gy[i] * gy[i] + 1.0).sqrt();
            ns[k] = (-gx[i] * inv) as f32;
            ns[k + 1] = (-gy[i] * inv) as f32;
            ns[k + 2] = inv as f32;
            k += 3;
        }
    }
    npoints as u32
}

/// Детерминистский хэш состояния моря (бит-в-бит воспроизводимость).
///
/// # Safety
/// `h` — живой хэндл.
#[no_mangle]
pub unsafe extern "C" fn polerf_water_hash(h: *mut c_void) -> u64 {
    sea(h).map(|w| w.sea.water_hash()).unwrap_or(0)
}

// =============================================================================
// ТЕСТЫ (C-ABI слой через собственные символы)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_abi() {
        let v = unsafe { std::ffi::CStr::from_ptr(polerf_version()) }.to_string_lossy();
        assert!(v.contains("poler-ffi"), "{v}");
        assert_eq!(polerf_abi(), 1);
    }

    #[test]
    fn water_lifecycle_and_determinism() {
        let h1 = polerf_water_new_u64(64, 96, 42, 8.0, 100.0, 0.0);
        let h2 = polerf_water_new_u64(64, 96, 42, 8.0, 100.0, 0.0);
        assert!(!h1.is_null() && !h2.is_null());
        // невалидная сетка → NULL
        assert!(polerf_water_new_u64(100, 96, 42, 8.0, 100.0, 0.0).is_null());

        let hs = unsafe { polerf_water_significant_height(h1) };
        assert!((0.5..=4.0).contains(&hs), "H_s = {hs} (ветер 8 м/с ≈ 1.37)");

        // 300 секунд моря → детерминизм бит-в-бит
        let mut grid1 = vec![0f32; 64 * 64];
        let mut grid2 = vec![0f32; 64 * 64];
        unsafe {
            for _ in 0..300 {
                polerf_water_step(h1, 1.0 / 60.0);
                polerf_water_step(h2, 1.0 / 60.0);
            }
            let n1 = polerf_water_frame(h1, 0.0, grid1.as_mut_ptr(), grid1.len() as u32);
            let n2 = polerf_water_frame(h2, 0.0, grid2.as_mut_ptr(), grid2.len() as u32);
            assert_eq!(n1, 64 * 64);
            assert_eq!(n1, n2);
            assert_eq!(grid1, grid2, "детерминизм воды нарушен");
            assert_eq!(polerf_water_hash(h1), polerf_water_hash(h2));

            // аналитическая поверхность: наклоны ненулевые и согласованные
            let mut dhx = 0.0f64;
            let mut dhy = 0.0f64;
            let hgt = polerf_water_surface_at(h1, 12.0, 30.0, &mut dhx, &mut dhy);
            assert!(dhx.is_finite() && dhy.is_finite());
            let hgt2 = polerf_water_surface_at(h1, 12.0, 30.0, std::ptr::null_mut(), std::ptr::null_mut());
            assert_eq!(hgt, hgt2);

            // течение: горизонтальная компонента есть
            let mut vx = 0.0f64;
            let mut vy = 0.0f64;
            polerf_water_flow_at(h1, 12.0, 30.0, &mut vx, &mut vy);
            assert!(vx.abs() + vy.abs() > 1e-9, "тежение нулевое?");

            polerf_water_free(h1);
            polerf_water_free(h2);
        }
    }

    #[test]
    fn water_geometries_one_call() {
        let h = polerf_water_new_u64(64, 96, 42, 8.0, 100.0, 0.0);
        assert!(!h.is_null());
        unsafe {
            let n_pts = 64 * 64;
            let mut heights = vec![0f32; n_pts];
            let mut normals = vec![0f32; n_pts * 3];
            // 10 секунд моря + геометрия одним вызовом
            let got = polerf_water_geometries_f32(
                h,
                10.0,
                heights.as_mut_ptr(),
                normals.as_mut_ptr(),
                n_pts as u32,
            );
            assert_eq!(got, n_pts as u32);
            // высоты уже не плоские
            let (mn, mx) = heights.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| {
                (a.min(v), b.max(v))
            });
            assert!(mx > mn, "море плоское: {mx} == {mn}");
            // нормали нормированы и не все смотрят строго вверх
            let mut non_vertical = 0usize;
            for p in normals.chunks_exact(3) {
                let norm2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
                assert!((norm2 - 1.0).abs() < 1e-4, "|N|² = {norm2}");
                if p[2] < 0.999 {
                    non_vertical += 1;
                }
            }
            assert!(non_vertical > n_pts / 10, "наклонных нормалей мало: {non_vertical}");
            // точность осциллятора противsurface_at (точный путь):
            // узлы сетки ↔ физические (i·domain/n, j·domain/n)
            let n = 64usize;
            let step = 100.0 / n as f64;
            let mut worst = 0.0f32;
            for &(i, j) in &[(7usize, 11usize), (30, 40), (61, 5), (13, 58)] {
                let mut gx = 0.0f64;
                let mut gy = 0.0f64;
                polerf_water_surface_at(h, i as f64 * step, j as f64 * step, &mut gx, &mut gy);
                let base = (j * n + i) * 3;
                // осциллятор дал (−gx,−gy,1)/‖…‖ — восстановим наклон
                let nx = normals[base] as f64;
                let ny = normals[base + 1] as f64;
                let nz = normals[base + 2] as f64;
                let ox = -nx / nz; // = gx
                let oy = -ny / nz; // = gy
                worst = worst.max((ox - gx).abs() as f32);
                worst = worst.max((oy - gy).abs() as f32);
            }
            assert!(worst < 1e-5, "осциллятор разошёлся с surface_at: {worst:e}");
            polerf_water_free(h);
        }
    }
}

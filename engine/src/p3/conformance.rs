//! C-ABI конформанс Rust ↔ Zig (цикл R2).
//!
//! Методика «живых фикстур»: один детерминированный PRNG (xorshift64)
//! на Rust-стороне генерирует векторы/матрицы; ОДНИ И ТЕ ЖЕ байты
//! уходят в Zig через C-ABI и параллельно считаются Rust-близнецом
//! (`native`). Расхождение = |zig − rust| по каждой категории.
//!
//! Требования плана (docs/plans/PLAN_POLER_ENGINE_UNIFICATION.md §4):
//! - d_FS(a,b) совпадает в пределах f64-честности;
//! - U†U = I для PGL4-элементов;
//! - идемпотенты P² = P обеих сторон на одинаковых матрицах.

use super::ffi::P3Lib;
use super::native::{self, Hom4, Pgl4};

/// Результат одной категории проверок.
#[derive(Clone, Debug)]
pub struct CheckResult {
    pub name: &'static str,
    pub checked: u32,
    pub max_dev: f64,
    pub tol: f64,
}

impl CheckResult {
    pub fn passed(&self) -> bool {
        self.max_dev <= self.tol
    }
}

/// Полный отчёт конформанса.
#[derive(Clone, Debug)]
pub struct ConformanceReport {
    pub lib_path: String,
    pub kernel_tag: String,
    pub abi_version: u32,
    pub pairs: u32,
    pub checks: Vec<CheckResult>,
}

impl ConformanceReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed())
    }

    /// Человекочитаемая сводка (для `p3 conformance` и дампов).
    pub fn summary(&self) -> String {
        let mut out = format!(
            "P³ C-ABI конформанс Rust ↔ Zig\n  библиотека : {}\n  ядро       : {} (ABI v{})\n  фикстур    : {} пар\n\n  {:<38} {:>10}  {:>12}  {:<10}\n",
            self.lib_path,
            self.kernel_tag,
            self.abi_version,
            self.pairs,
            "категория",
            "проверок",
            "макс|Δ|",
            "допуск"
        );
        for c in &self.checks {
            let verdict = if c.passed() { "OK" } else { "FAIL" };
            out.push_str(&format!(
                "  {:<38} {:>10}  {:>12.3e}  {:<10} {}\n",
                c.name,
                c.checked,
                c.max_dev,
                format!("{:.0e}", c.tol),
                verdict
            ));
        }
        out.push_str(if self.passed() {
            "\n  ВЕРДИКТ: КОНФОРМАНС ПОДТВЕРЖДЁН — Rust и Zig вычисляют одну математику P³"
        } else {
            "\n  ВЕРДИКТ: РАСХОЖДЕНИЕ — см. категории FAIL"
        });
        out
    }
}

/// Детерминированный xorshift64 (функции Rust-стороны; байты едут в Zig).
pub struct FixtureRng(u64);

impl FixtureRng {
    pub fn new(seed: u64) -> Self {
        FixtureRng(seed.max(1))
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// f64 ∈ (−scale, scale) с 53-битной мантиссой.
    #[inline]
    pub fn next_f64(&mut self, scale: f64) -> f64 {
        let v = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        v * 2.0 * scale - scale
    }

    pub fn hom4(&mut self, scale: f64) -> Hom4 {
        Hom4([
            self.next_f64(scale),
            self.next_f64(scale),
            self.next_f64(scale),
            self.next_f64(scale),
        ])
    }

    /// Хорошо обусловленная матрица: диагонально доминирующая.
    pub fn pgl4_well_conditioned(&mut self) -> Pgl4 {
        let mut m = [[0.0f64; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                m[r][c] = self.next_f64(0.5);
            }
            m[r][r] += 4.0 * (self.next_f64(0.25) + 1.25);
        }
        Pgl4::from_row_major(m)
    }
}

/// Полный прогон конформанса. `pairs` — число случайных пар на категорию.
pub fn run_conformance(pairs: u32) -> Result<ConformanceReport, String> {
    let lib = P3Lib::open()?;
    let mut checks: Vec<CheckResult> = Vec::new();

    // ------------------------------------------------------------------
    // 0. Рукопожатие: ABI + тег ядра
    // ------------------------------------------------------------------
    if lib.abi_version != super::ffi::EXPECTED_ABI {
        return Err(format!("ABI mismatch: {} vs {}", lib.abi_version, super::ffi::EXPECTED_ABI));
    }

    // ------------------------------------------------------------------
    // 1. d_FS(a,b): Rust-близнец против ядра Zig
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0xB35_EED_A);
        let mut worst = 0.0f64;
        let mut checked = 0u32;
        for _ in 0..pairs {
            let a = rng.hom4(1.0);
            let b = rng.hom4(1.0);
            let dz = lib.fs_distance(&a, &b);
            let dr = native::fs_distance(&a, &b);
            worst = worst.max((dz - dr).abs());
            checked += 1;
        }
        // Специальные случаи: ортогональность = π/2, антиподы = 0
        let dz = lib.fs_distance(&Hom4::new(1.0, 0.0, 0.0, 0.0), &Hom4::new(0.0, 0.0, 0.0, 1.0));
        worst = worst.max((dz - std::f64::consts::FRAC_PI_2).abs());
        let dz = lib.fs_distance(&Hom4::new(1.0, 2.0, 3.0, 4.0), &Hom4::new(-1.0, -2.0, -3.0, -4.0));
        worst = worst.max(dz.abs());
        checked += 2;
        checks.push(CheckResult {
            name: "d_FS(a,b) Rust ↔ Zig",
            checked,
            max_dev: worst,
            tol: 1e-12,
        });
    }

    // ------------------------------------------------------------------
    // 2. Гомогенность: d_FS(λa, μb) = d_FS(a,b) (обе стороны)
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0xC0FFEE_7);
        let mut worst = 0.0f64;
        for _ in 0..pairs {
            let a = rng.hom4(1.0);
            let b = rng.hom4(1.0);
            let lam = rng.next_f64(10.0).abs() + 0.1;
            let mu = -rng.next_f64(10.0).abs() - 0.1;
            let a2 = Hom4(a.0.map(|v| v * lam));
            let b2 = Hom4(b.0.map(|v| v * mu));
            worst = worst.max((lib.fs_distance(&a2, &b2) - lib.fs_distance(&a, &b)).abs());
            worst = worst.max(
                (native::fs_distance(&a2, &b2) - native::fs_distance(&a, &b)).abs(),
            );
        }
        checks.push(CheckResult {
            name: "гомогенность d_FS(λa, μb)",
            checked: pairs * 2,
            max_dev: worst,
            tol: 1e-9,
        });
    }

    // ------------------------------------------------------------------
    // 3. U†U = I: композиции Гивенса (ортогональные PGL4)
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0x61E7A5);
        let mut worst = 0.0f64;
        let mut worst_zig_res = 0.0f64;
        for _ in 0..pairs {
            // U = G(0,1,α)·G(1,2,β)·G(2,3,γ)·G(0,3,δ)
            let angles: [f64; 4] = [
                rng.next_f64(std::f64::consts::PI),
                rng.next_f64(std::f64::consts::PI),
                rng.next_f64(std::f64::consts::PI),
                rng.next_f64(std::f64::consts::PI),
            ];
            let build = |g: &dyn Fn(u32, u32, f64) -> Pgl4| -> Pgl4 {
                let u = g(0, 1, angles[0]);
                let u = g(1, 2, angles[1]).mul(&u);
                let u = g(2, 3, angles[2]).mul(&u);
                g(0, 3, angles[3]).mul(&u)
            };
            let uz = build(&|i, j, t| lib.givens4(i, j, t));
            let ur = build(&|i, j, t| native::givens4(i, j, t));
            // Обе стороны ортогональны…
            worst_zig_res = worst_zig_res.max(lib.ortho_residual(&uz));
            worst_zig_res = worst_zig_res.max(native::ortho_residual(&ur));
            // …и согласованы поэлементно
            for k in 0..16 {
                worst = worst.max((uz.data[k] - ur.data[k]).abs());
            }
            // det = +1 у обеих
            worst = worst.max((lib.pgl_det(&uz) - native_pgl_det(&ur)).abs());
        }
        checks.push(CheckResult {
            name: "U†U = I (Гивенс-композиции)",
            checked: pairs,
            max_dev: worst,
            tol: 1e-12,
        });
        checks.push(CheckResult {
            name: "‖UᵀU − I‖ (остаток обеих сторон)",
            checked: pairs * 2,
            max_dev: worst_zig_res,
            tol: 1e-12,
        });
    }

    // ------------------------------------------------------------------
    // 4. Действие PGL: (A·B)v = A(Bv), согласование матрицы и вектора
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0xABCD_1234);
        let mut worst = 0.0f64;
        for _ in 0..pairs {
            let a = rng.pgl4_well_conditioned();
            let b = rng.pgl4_well_conditioned();
            let v = rng.hom4(1.0);
            // Zig-сторона целиком
            let ab_z = lib.pgl_mul(&a, &b);
            let lhs_z = lib.pgl_apply(&ab_z, &v);
            let rhs_z = lib.pgl_apply(&a, &lib.pgl_apply(&b, &v));
            for k in 0..4 {
                worst = worst.max((lhs_z.0[k] - rhs_z.0[k]).abs());
            }
            // Rust ↔ Zig на финальном векторе
            let lhs_r = native::Pgl4 { data: ab_z.data }.apply(&v);
            for k in 0..4 {
                worst = worst.max((lhs_z.0[k] - lhs_r.0[k]).abs());
            }
        }
        checks.push(CheckResult {
            name: "(A·B)v = A(Bv) + Rust ↔ Zig",
            checked: pairs * 2,
            max_dev: worst,
            tol: 1e-9,
        });
    }

    // ------------------------------------------------------------------
    // 5. det(PGL4) на хорошо обусловленных матрицах
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0xDE7E7_51);
        let mut worst_rel = 0.0f64;
        for _ in 0..pairs {
            let m = rng.pgl4_well_conditioned();
            let dz = lib.pgl_det(&m);
            let dr = native::Pgl4 { data: m.data }.det();
            let scale = dz.abs().max(1.0);
            worst_rel = worst_rel.max((dz - dr).abs() / scale);
        }
        checks.push(CheckResult {
            name: "det(PGL4) относительная",
            checked: pairs,
            max_dev: worst_rel,
            tol: 1e-9,
        });
    }

    // ------------------------------------------------------------------
    // 6. Идемпотенты P² = P: спектральные проекторы + дополнения
    // ------------------------------------------------------------------
    {
        let mut rng = FixtureRng::new(0x1DE_5EED);
        let mut worst = 0.0f64;
        let mut worst_res = 0.0f64;
        for _ in 0..pairs {
            let v = rng.hom4(1.0);
            let pz = lib.spectral_projector(&v);
            let pr = native::spectral_projector(&v);
            // Поэлементное согласование
            for k in 0..16 {
                worst = worst.max((pz.data[k] - pr.data[k]).abs());
            }
            // Остатки идемпотентности обеих сторон
            worst_res = worst_res.max(lib.idempotent_residual(&pz));
            worst_res = worst_res.max(native::idempotent_residual(&pr));
            // Дополнение I − P идемпотентно (Zig-сторона, арифметика Rust)
            let mut q = [0.0f64; 16];
            let id = lib.pgl_identity();
            for k in 0..16 {
                q[k] = id.data[k] - pz.data[k];
            }
            worst_res = worst_res.max(lib.idempotent_residual(&Pgl4 { data: q }));
            // Ранг = 1 (след)
            worst = worst.max((lib.idempotent_rank(&pz) - 1.0).abs());
        }
        checks.push(CheckResult {
            name: "проектор P = vvᵀ/vᵀv Rust ↔ Zig",
            checked: pairs,
            max_dev: worst,
            tol: 1e-12,
        });
        checks.push(CheckResult {
            name: "‖P² − P‖ и ‖(I−P)² − (I−P)‖",
            checked: pairs * 3,
            max_dev: worst_res,
            tol: 1e-12,
        });
    }

    Ok(ConformanceReport {
        lib_path: lib.path.display().to_string(),
        kernel_tag: lib.kernel_tag.clone(),
        abi_version: lib.abi_version,
        pairs,
        checks,
    })
}

fn native_pgl_det(m: &Pgl4) -> f64 {
    native::Pgl4 { data: m.data }.det()
}

// =============================================================================
// ТЕСТЫ КОНФОРМАНСА
// =============================================================================
// FFI-тесты честные: библиотека коммитится в репозиторий (ffi/), поэтому
// на Linux x86_64 (песочница POLER BOX и машина владельца) она обязана
// загрузиться. Пропуск допускаем только на иных платформах.

#[cfg(all(test, unix, target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn conformance_full_report() {
        let report = run_conformance(96).expect("libp3ffi должна загрузиться (см. ffi/build.sh)");
        print!("{}", report.summary());
        assert!(
            report.passed(),
            "конформанс провален:\n{}",
            report.summary()
        );
    }

    #[test]
    fn conformance_quick_smoke() {
        let report = run_conformance(8).expect("libp3ffi должна загрузиться");
        assert!(report.passed());
        assert!(report.checks.len() >= 7);
        // Тег ядра приходит из Zig-бинарника
        assert!(report.kernel_tag.contains("p3-kernel"));
    }

    #[test]
    fn zig_render_frame_smoke() {
        // Квадрат из 4 точек + замкнутое ребро — тройной буфер заполняется
        let lib = P3Lib::open().expect("libp3ffi");
        let pts: Vec<f64> = vec![
            -0.5, -0.5, 0.0, 1.0, 0.5, -0.5, 0.0, 1.0, 0.5, 0.5, 0.0, 1.0, -0.5, 0.5, 0.0, 1.0,
        ];
        let segs = [1u8, 1, 1, 1];
        let pairs = [0u32, 1, 1, 2, 2, 3, 3, 0];
        let (w, h) = (96u32, 72u32);
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        let mut depth = vec![0f32; (w * h) as usize];
        let mut seg = vec![0u8; (w * h) as usize];
        lib.render_frame(&pts, &segs, &pairs, w, h, Default::default(), 1, &mut rgb, &mut depth, &mut seg)
            .unwrap();
        let painted = seg.iter().filter(|&&s| s != 0).count();
        assert!(painted > 20, "painted = {painted}");
        let maxd = depth
            .iter()
            .copied()
            .filter(|&d| d < 1e29) // фон (1e30) не считаем
            .fold(0.0f32, f32::max);
        assert!((0.0..=std::f64::consts::FRAC_PI_2 as f32 + 1e-4).contains(&maxd));
    }

    /// Цикл W: попиксельное соглашение растеризаторов — Zig-ядро (C-ABI)
    /// против Rust-близнеца на общих живых фикстурах. Допуски честные:
    /// libm acos/cos/sin могут отличаться последним ulp, что на границах
    /// округления пикселей даёт единичные расхождения — но не 1%.
    #[test]
    fn ffi_vs_native_render_agreement() {
        let lib = P3Lib::open().expect("libp3ffi");
        let mut rng = FixtureRng::new(0x5EED_C0DE);
        for scene_i in 0..4usize {
            // Сцена: 8 точек, замкнутый контур + хорды, разные сегменты
            let n = 8usize;
            let mut pts = Vec::with_capacity(n * 4);
            let mut segs = Vec::with_capacity(n);
            for i in 0..n {
                pts.extend_from_slice(&[
                    rng.next_f64(1.0),
                    rng.next_f64(1.0),
                    rng.next_f64(0.6),
                    1.0,
                ]);
                segs.push(((i % 9) + 1) as u8);
            }
            let mut pairs = Vec::new();
            for i in 0..n {
                pairs.push(i as u32);
                pairs.push(((i + 1) % n) as u32);
            }
            pairs.extend_from_slice(&[0u32, 4, 2, 6]); // хорды
            let (w, h) = (144u32, 108u32);
            let camera = crate::p3::ffi::Camera {
                focal: 0.0,
                cam_dist: 3.4,
                yaw: -0.62 + 0.41 * scene_i as f64,
                pitch: 0.34 - 0.17 * scene_i as f64,
            };
            let mut z_rgb = vec![0u8; (w * h * 3) as usize];
            let mut z_depth = vec![0f32; (w * h) as usize];
            let mut z_seg = vec![0u8; (w * h) as usize];
            let mut n_rgb = vec![0u8; (w * h * 3) as usize];
            let mut n_depth = vec![0f32; (w * h) as usize];
            let mut n_seg = vec![0u8; (w * h) as usize];

            lib.render_frame(&pts, &segs, &pairs, w, h, camera, 2, &mut z_rgb, &mut z_depth, &mut z_seg)
                .unwrap();
            let _ = native::render_frame_native(
                &pts,
                &segs,
                &pairs,
                w,
                h,
                camera,
                2,
                &mut n_rgb,
                &mut n_depth,
                &mut n_seg,
            );

            let pz = z_seg.iter().filter(|&&s| s != 0).count();
            let pn = n_seg.iter().filter(|&&s| s != 0).count();
            let tolerance = 4 + ((pz.max(pn) as f64) * 0.02).ceil() as i64;
            assert!(
                (pz as i64 - pn as i64).abs() <= tolerance,
                "scene {scene_i}: painted zig={pz} native={pn} (допуск {tolerance})"
            );

            let mut both = 0usize;
            let mut worst_depth = 0.0f32;
            let mut worst_rgb = 0i32;
            let mut seg_mismatch = 0usize;
            for k in 0..(w * h) as usize {
                if z_seg[k] != 0 && n_seg[k] != 0 {
                    both += 1;
                    worst_depth = worst_depth.max((z_depth[k] - n_depth[k]).abs());
                    for c in 0..3 {
                        worst_rgb =
                            worst_rgb.max((z_rgb[k * 3 + c] as i32 - n_rgb[k * 3 + c] as i32).abs());
                    }
                    if z_seg[k] != n_seg[k] {
                        seg_mismatch += 1;
                    }
                }
            }
            assert!(both > 100, "scene {scene_i}: сцена не нарисована (both={both})");
            assert!(
                worst_depth <= 1e-9,
                "scene {scene_i}: |Δdepth| = {worst_depth:e}"
            );
            assert!(worst_rgb <= 2, "scene {scene_i}: |Δrgb| = {worst_rgb}");
            assert!(
                seg_mismatch * 100 <= both,
                "scene {scene_i}: seg-расхождения {seg_mismatch}/{both} > 1%"
            );
        }
    }
}

//! Rust-близнец математики P³ (Zig: `src/p3_kernel.zig`, `p3_idempotent.zig`).
//!
//! Это НЕ вызов Zig — это независимая реализация тех же формул на Rust.
//! Совпадение с Zig-стороны (см. `conformance.rs`) доказывает, что оба
//! движка вычисляют ОДНУ И ТУ ЖЕ математику проективного пространства.
//!
//! Конвенции (зеркально ядру P³):
//! - `Hom4` — однородный вектор [X:Y:Z:W], точка P³;
//! - `Pgl4` — column-major [16]f64, `data[col*4 + row]`;
//! - FS-метрика: d = arccos(|⟨a,b⟩|/(‖a‖·‖b‖)) ∈ [0, π/2].

/// Однородный 4-вектор [X:Y:Z:W] — точка проективного пространства P³.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hom4(pub [f64; 4]);

impl Hom4 {
    #[inline]
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Hom4([x, y, z, w])
    }

    /// Скалярное произведение гильбертова пространства R⁴.
    #[inline]
    pub fn dot(a: &Hom4, b: &Hom4) -> f64 {
        a.0[0] * b.0[0] + a.0[1] * b.0[1] + a.0[2] * b.0[2] + a.0[3] * b.0[3]
    }

    /// Норма ‖v‖.
    #[inline]
    pub fn norm(&self) -> f64 {
        Self::dot(self, self).sqrt()
    }
}

/// 4×4 матрица PGL(4,ℝ), column-major: `data[col*4 + row]`.
#[derive(Clone, Copy, Debug)]
pub struct Pgl4 {
    pub data: [f64; 16],
}

impl Pgl4 {
    pub fn identity() -> Self {
        Pgl4 {
            data: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[col * 4 + row]
    }

    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: f64) {
        self.data[col * 4 + row] = val;
    }

    /// Из row-major [4][4] (конвертация в column-major).
    pub fn from_row_major(m: [[f64; 4]; 4]) -> Self {
        let mut r = Pgl4::identity();
        for col in 0..4 {
            for row in 0..4 {
                r.set(row, col, m[row][col]);
            }
        }
        r
    }

    /// Матричное умножение A·B (композиция PGL(4)).
    pub fn mul(&self, b: &Pgl4) -> Pgl4 {
        let mut r = Pgl4::identity();
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.get(row, k) * b.get(k, col);
                }
                r.set(row, col, sum);
            }
        }
        r
    }

    pub fn transpose(&self) -> Pgl4 {
        let mut r = Pgl4::identity();
        for row in 0..4 {
            for col in 0..4 {
                r.set(row, col, self.get(col, row));
            }
        }
        r
    }

    /// Действие на точку P³: v → M·v.
    pub fn apply(&self, v: &Hom4) -> Hom4 {
        let mut out = [0.0f64; 4];
        for row in 0..4 {
            let mut sum = 0.0;
            for col in 0..4 {
                sum += self.get(row, col) * v.0[col];
            }
            out[row] = sum;
        }
        Hom4(out)
    }

    /// Определитель (разложение Лапласа по первой строке + Саррюс для 3×3)
    /// — зеркально `PGL4.det()` в Zig.
    pub fn det(&self) -> f64 {
        fn minor3(s: &[[f64; 3]; 3]) -> f64 {
            s[0][0] * (s[1][1] * s[2][2] - s[1][2] * s[2][1])
                - s[0][1] * (s[1][0] * s[2][2] - s[1][2] * s[2][0])
                + s[0][2] * (s[1][0] * s[2][1] - s[1][1] * s[2][0])
        }
        fn minor(m: &Pgl4, r: usize, c: usize) -> f64 {
            let mut s = [[0.0f64; 3]; 3];
            let mut si = 0;
            for i in 0..4 {
                if i == r {
                    continue;
                }
                let mut sj = 0;
                for j in 0..4 {
                    if j == c {
                        continue;
                    }
                    s[si][sj] = m.get(i, j);
                    sj += 1;
                }
                si += 1;
            }
            minor3(&s)
        }
        self.get(0, 0) * minor(self, 0, 0)
            - self.get(0, 1) * minor(self, 0, 1)
            + self.get(0, 2) * minor(self, 0, 2)
            - self.get(0, 3) * minor(self, 0, 3)
    }
}

/// Метрика Фубини–Штуди d_FS(a,b) = arccos(|⟨a,b⟩|/(‖a‖·‖b‖)) ∈ [0, π/2].
/// Зеркально `p3_kernel.fsDistance` (включая guard'ы и clamp).
pub fn fs_distance(a: &Hom4, b: &Hom4) -> f64 {
    let n1 = a.norm();
    let n2 = b.norm();
    if n1 < 1e-15 || n2 < 1e-15 {
        return 0.0;
    }
    let d = Hom4::dot(a, b).abs() / (n1 * n2);
    let cos_theta = d.min(1.0);
    cos_theta.acos()
}

/// Гивенс-поворот G(i,j,θ): ортогонален, det = +1. Зеркально `p3ffi_givens4`.
pub fn givens4(i: u32, j: u32, theta: f64) -> Pgl4 {
    let mut g = Pgl4::identity();
    if i == j || i > 3 || j > 3 {
        return g;
    }
    let (c, s) = (theta.cos(), theta.sin());
    let (ii, jj) = (i as usize, j as usize);
    g.set(ii, ii, c);
    g.set(jj, jj, c);
    g.set(ii, jj, -s);
    g.set(jj, ii, s);
    g
}

/// Спектральный проектор ранга 1: P = v·vᵀ/(vᵀv). Идемпотент P² = P.
pub fn spectral_projector(v: &Hom4) -> Pgl4 {
    let vv = Hom4::dot(v, v);
    let mut p = Pgl4::identity();
    if vv < 1e-300 {
        return p;
    }
    for row in 0..4 {
        for col in 0..4 {
            p.set(row, col, v.0[row] * v.0[col] / vv);
        }
    }
    p
}

/// Остаток идемпотентности max|P² − P| (по всем 16 элементам).
pub fn idempotent_residual(m: &Pgl4) -> f64 {
    let p2 = m.mul(m);
    let mut worst = 0.0f64;
    for i in 0..16 {
        worst = worst.max((p2.data[i] - m.data[i]).abs());
    }
    worst
}

/// Остаток ортогональности max|UᵀU − I|.
pub fn ortho_residual(m: &Pgl4) -> f64 {
    let utu = m.transpose().mul(m);
    let mut worst = 0.0f64;
    for row in 0..4 {
        for col in 0..4 {
            let want = if row == col { 1.0 } else { 0.0 };
            worst = worst.max((utu.get(row, col) - want).abs());
        }
    }
    worst
}

/// След (ранг идемпотента).
pub fn trace(m: &Pgl4) -> f64 {
    (0..4).map(|i| m.get(i, i)).sum()
}

/// Native Rust-растеризатор сцены P³ (чистый Rust без AVX2/FFI требований)./// Native Rust-растеризатор сцены P³ (чистый Rust без AVX2/FFI требований).
///
/// ПОПИКСЕЛЬНЫЙ БЛИЗНЕЦ Zig-ядра `p3-engine/src/p3_ffi.zig::p3ffi_render_frame`
/// (цикл W, конформанс — `p3::conformance::tests::ffi_vs_native_render_agreement`).
/// Семантика (зеркально доке Zig-ядра):
///   • глубина НЕ интерполируется между глубинами концов, а пересчитывается
///     из интерполированной 3D-точки (честная глубина вдоль отрезка);
///   • цвет: бленд палитр сегментов вершин ребра по t + яркость от
///     глубины (0.55..1.00);
///   • seg-буфер: сегмент ПЕРВОЙ вершины ребра;
///   • толщина: ядро Брезенхема 1px + плюс-спрайты радиуса
///     max(1, thickness−1) на концах (смещение глубины +1e-4·(|ox|+|oy|+1) —
///     ядро всегда выигрывает depth-тест у спрайта);
///   • depth-тест строгий (<) — при равенстве остаётся ранее нарисованное.
pub fn render_frame_native(
    pts: &[f64],
    segs: &[u8],
    pairs: &[u32],
    width: u32,
    height: u32,
    camera: crate::p3::ffi::Camera,
    thickness: u32,
    rgb: &mut [u8],
    depth: &mut [f32],
    seg: &mut [u8],
) -> Result<(), String> {
    let npix = width as usize * height as usize;
    if rgb.len() != npix * 3 || depth.len() != npix || seg.len() != npix {
        return Err("некорректный размер буферов рендера".into());
    }

    const FRAC_PI_2: f64 = std::f64::consts::FRAC_PI_2;

    /// Палитра 15 оттенков (сегмент 0 — фон, далее (seg−1) mod 15 + 1).
    const PALETTE: [[f64; 3]; 15] = [
        [90.0, 140.0, 230.0],  // 1 — синий
        [230.0, 90.0, 110.0],  // 2 — красный
        [90.0, 200.0, 120.0],  // 3 — зелёный
        [235.0, 190.0, 80.0],  // 4 — золотой
        [120.0, 200.0, 230.0], // 5 — циан
        [210.0, 120.0, 230.0], // 6 — пурпур
        [240.0, 150.0, 70.0],  // 7 — оранж
        [80.0, 210.0, 190.0],  // 8 — бирюза
        [200.0, 220.0, 90.0],  // 9 — лайм
        [150.0, 160.0, 255.0], // 10 — лаванда
        [255.0, 120.0, 160.0], // 11 — розовый
        [130.0, 230.0, 160.0], // 12 — мятный
        [190.0, 140.0, 100.0], // 13 — песочный
        [170.0, 180.0, 200.0], // 14 — стальной
        [250.0, 220.0, 140.0], // 15 — шампань
    ];

    #[inline]
    fn palette_of(s: u8) -> [f64; 3] {
        if s == 0 {
            return [10.0, 12.0, 22.0];
        }
        PALETTE[((s - 1) as usize) % 15]
    }

    #[inline]
    fn clamp_u8(v: f64) -> u8 {
        if v <= 0.0 {
            return 0;
        }
        if v >= 255.0 {
            return 255;
        }
        v.round() as u8
    }

    /// Глубина Фубини–Штуди от начала координат камеры.
    #[inline]
    fn fs_depth(px: f64, py: f64, pz: f64) -> f64 {
        let q = 1.0 / (px * px + py * py + pz * pz + 1.0).sqrt();
        q.min(1.0).acos()
    }

    struct Proj {
        vx: f64,
        vy: f64,
        vz: f64, // координаты в пространстве камеры (до сдвига cam_dist)
        ix: i64,
        iy: i64,
        ok: bool,
    }

    /// Проекция точки: yaw → pitch → перспектива. Точки за камерой
    /// (zc ≤ 1e-9) и бесконечные экранные координаты отсекаются.
    #[allow(clippy::too_many_arguments)]
    fn project_point(
        x: f64,
        y: f64,
        z: f64,
        cy: f64,
        sy: f64,
        cp: f64,
        sp: f64,
        f: f64,
        cam_dist: f64,
        w2: f64,
        h2: f64,
    ) -> Proj {
        let x1 = cy * x + sy * z;
        let z1 = -sy * x + cy * z;
        let y2 = cp * y - sp * z1;
        let z2 = sp * y + cp * z1;
        let zc = z2 + cam_dist;
        if zc <= 1e-9 {
            return Proj { vx: x1, vy: y2, vz: z2, ix: 0, iy: 0, ok: false };
        }
        let u = f * x1 / zc + w2;
        let v = h2 - f * y2 / zc;
        if !u.is_finite() || !v.is_finite() || u.abs() > 9.0e15 || v.abs() > 9.0e15 {
            return Proj { vx: x1, vy: y2, vz: z2, ix: 0, iy: 0, ok: false };
        }
        Proj { vx: x1, vy: y2, vz: z2, ix: u.round() as i64, iy: v.round() as i64, ok: true }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn plot(
        w: i64,
        h: i64,
        rgb: &mut [u8],
        depth: &mut [f32],
        seg: &mut [u8],
        x: i64,
        y: i64,
        d: f64,
        bias: f64,
        r: f64,
        g: f64,
        b: f64,
        s: u8,
    ) {
        if x < 0 || y < 0 || x >= w || y >= h {
            return;
        }
        let idx = (y * w + x) as usize;
        let dd = (d + bias) as f32;
        if dd < depth[idx] {
            depth[idx] = dd;
            seg[idx] = s;
            rgb[idx * 3] = clamp_u8(r);
            rgb[idx * 3 + 1] = clamp_u8(g);
            rgb[idx * 3 + 2] = clamp_u8(b);
        }
    }

    /// Плюс-спрайт радиуса radius вокруг конечной точки (ядро линии всегда
    /// выигрывает: спрайт несёт штраф глубины 1e-4·(|ox|+|oy|+1)).
    #[allow(clippy::too_many_arguments)]
    fn draw_endpoint_sprite(
        w: i64,
        h: i64,
        rgb: &mut [u8],
        depth: &mut [f32],
        seg: &mut [u8],
        p: &Proj,
        col: [f64; 3],
        s: u8,
        radius: i64,
    ) {
        let d = fs_depth(p.vx, p.vy, p.vz);
        let bright = 0.55 + 0.45 * (1.0 - d / FRAC_PI_2);
        let r = col[0] * bright;
        let g = col[1] * bright;
        let b = col[2] * bright;
        let mut oy: i64 = -radius;
        while oy <= radius {
            let mut ox: i64 = -radius;
            while ox <= radius {
                let manh = ox.abs() + oy.abs();
                if manh <= radius {
                    // форма «плюс/ромб»
                    let bias = 1e-4 * (manh + 1) as f64;
                    plot(w, h, rgb, depth, seg, p.ix + ox, p.iy + oy, d, bias, r, g, b, s);
                }
                ox += 1;
            }
            oy += 1;
        }
    }

    // --- 1. Фон: глубокий космос [10, 12, 22] ---
    for px in rgb.chunks_exact_mut(3) {
        px[0] = 10;
        px[1] = 12;
        px[2] = 22;
    }
    depth.fill(1e30f32);
    seg.fill(0);

    let n_pts = pts.len() / 4;
    let n_pairs = pairs.len() / 2;
    if n_pts == 0 || n_pairs == 0 {
        return Ok(());
    }

    let w = width as i64;
    let h = height as i64;
    let (sy, cy) = camera.yaw.sin_cos(); // Rust sin_cos → (sin, cos); Zig: cy=cos, sy=sin
    let (sp, cp) = camera.pitch.sin_cos();
    let f = if camera.focal > 0.0 {
        camera.focal
    } else {
        1.15 * height as f64
    };
    let w2 = width as f64 / 2.0;
    let h2 = height as f64 / 2.0;
    let radius: i64 = std::cmp::max(1, thickness as i64 - 1);

    // --- 2. Рёбра ---
    for e in 0..n_pairs {
        let ia = pairs[e * 2] as usize;
        let jb = pairs[e * 2 + 1] as usize;
        if ia >= n_pts || jb >= n_pts {
            continue;
        }
        let s0 = segs[ia];
        let s1 = segs[jb];

        let a = project_point(
            pts[ia * 4], pts[ia * 4 + 1], pts[ia * 4 + 2],
            cy, sy, cp, sp, f, camera.cam_dist, w2, h2,
        );
        let b = project_point(
            pts[jb * 4], pts[jb * 4 + 1], pts[jb * 4 + 2],
            cy, sy, cp, sp, f, camera.cam_dist, w2, h2,
        );
        if !a.ok || !b.ok {
            continue;
        }

        let col0 = palette_of(s0);
        let col1 = palette_of(s1);
        let cseg: u8 = s0; // сегмент ребра — первая вершина (конвенция ABI)

        // --- Брезенхем (целочисленный, классический — шаги как в Zig-ядре) ---
        let mut x = a.ix;
        let mut y = a.iy;
        let dx = b.ix - x;
        let dy = b.iy - y;
        let adx = dx.abs();
        let ady = dy.abs();
        let n: i64 = adx.max(ady);
        let sx: i64 = if dx > 0 { 1 } else { -1 };
        let sy2: i64 = if dy > 0 { 1 } else { -1 };
        let mut err: i64 = adx - ady;

        let mut k: i64 = 0;
        while k <= n {
            let t: f64 = if n > 0 {
                k as f64 / n as f64
            } else {
                1.0 // вырожденное ребро → вторая вершина (семантика зонда)
            };
            let px = a.vx + (b.vx - a.vx) * t;
            let py = a.vy + (b.vy - a.vy) * t;
            let pz = a.vz + (b.vz - a.vz) * t;
            let d = fs_depth(px, py, pz);
            let bright = 0.55 + 0.45 * (1.0 - d / FRAC_PI_2);
            let r = (col0[0] + (col1[0] - col0[0]) * t) * bright;
            let g = (col0[1] + (col1[1] - col0[1]) * t) * bright;
            let b = (col0[2] + (col1[2] - col0[2]) * t) * bright;
            plot(w, h, rgb, depth, seg, x, y, d, 0.0, r, g, b, cseg);

            // шаг Брезенхема
            if 2 * err > -ady {
                err -= ady;
                x += sx;
            }
            if 2 * err < adx {
                err += adx;
                y += sy2;
            }
            k += 1;
        }

        // --- Спрайты концов (толщина) ---
        draw_endpoint_sprite(w, h, rgb, depth, seg, &a, col0, cseg, radius);
        draw_endpoint_sprite(w, h, rgb, depth, seg, &b, col1, cseg, radius);
    }

    Ok(())
}

// =============================================================================
// ТЕСТЫ (чистая математика, без FFI — работают на любой платформе)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn fs_metric_bounds_and_special_cases() {
        // Ортогональные векторы → π/2
        let d = fs_distance(&Hom4::new(1.0, 0.0, 0.0, 0.0), &Hom4::new(0.0, 1.0, 0.0, 0.0));
        assert!((d - PI / 2.0).abs() < 1e-14, "d = {d}");

        // Антипараллельные — та же точка P³ → 0
        let d = fs_distance(&Hom4::new(1.0, 0.0, 0.0, 0.0), &Hom4::new(-2.0, 0.0, 0.0, 0.0));
        assert!(d.abs() < 1e-14);

        // Гомогенность: d_FS(λa, μb) = d_FS(a,b)
        let (a, b) = (
            Hom4::new(0.3, -1.2, 0.7, 1.0),
            Hom4::new(-0.8, 0.1, 0.5, 2.0),
        );
        let d0 = fs_distance(&a, &b);
        let a2 = Hom4([a.0[0] * 10.0, a.0[1] * 10.0, a.0[2] * 10.0, a.0[3] * 10.0]);
        let b2 = Hom4([-b.0[0] * 0.5, -b.0[1] * 0.5, -b.0[2] * 0.5, -b.0[3] * 0.5]);
        assert!((fs_distance(&a2, &b2) - d0).abs() < 1e-12);

        // Нулевой вектор → 0 (guard)
        assert_eq!(fs_distance(&Hom4::new(0.0, 0.0, 0.0, 0.0), &b), 0.0);

        // Диапазон [0, π/2] на псевдослучайных точках
        let mut s = 0x9E3779B97F4A7C15u64;
        for _ in 0..256 {
            let mut next = || {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
            };
            let a = Hom4::new(next(), next(), next(), next());
            let b = Hom4::new(next(), next(), next(), next());
            let d = fs_distance(&a, &b);
            assert!((0.0..=PI / 2.0 + 1e-12).contains(&d), "d = {d}");
            assert!(d.is_finite());
        }
    }

    #[test]
    fn givens_orthogonality_and_det() {
        for &(i, j, th) in &[
            (0u32, 1u32, 0.3f64),
            (0, 3, 0.7),
            (1, 2, -1.1),
            (2, 3, 2.05),
        ] {
            let g = givens4(i, j, th);
            assert!(ortho_residual(&g) < 1e-14);
            assert!((g.det() - 1.0).abs() < 1e-12, "det({i},{j})");
        }
        // Вырожденный случай i == j → единичная
        let g = givens4(2, 2, 1.0);
        assert!(ortho_residual(&g) < 1e-15);
    }

    #[test]
    fn projector_idempotent_rank1() {
        let v = Hom4::new(1.0, 2.0, 3.0, 4.0);
        let p = spectral_projector(&v);
        assert!(idempotent_residual(&p) < 1e-14);
        assert!((trace(&p) - 1.0).abs() < 1e-14);
        // Симметричен
        for r in 0..4 {
            for c in 0..4 {
                assert!((p.get(r, c) - p.get(c, r)).abs() < 1e-15);
            }
        }
        // Дополнение I − P тоже идемпотент
        let mut q = Pgl4::identity();
        for k in 0..16 {
            q.data[k] -= p.data[k];
        }
        assert!(idempotent_residual(&q) < 1e-14);
        assert!((trace(&q) - 3.0).abs() < 1e-14);
    }

    #[test]
    fn pgl_mul_apply_consistency() {
        // (A·B)v = A(Bv)
        let a = givens4(0, 1, 0.4);
        let b = givens4(2, 3, -0.9);
        let v = Hom4::new(0.5, -0.3, 0.8, 1.0);
        let ab = a.mul(&b);
        let lhs = ab.apply(&v);
        let rhs = a.apply(&b.apply(&v));
        for k in 0..4 {
            assert!((lhs.0[k] - rhs.0[k]).abs() < 1e-14);
        }
        // det(AB) = det(A)·det(B)
        assert!((ab.det() - a.det() * b.det()).abs() < 1e-12);
    }
}

//! Рендер игрового мира: extraction → P³ FFI → тройной буфер → PNG.
//!
//! Мир извлекается в геометрию «точки + рёбра» (каркасная парадигма P³):
//! каждое тело — три ортогональных кольца-каркас сферы (вращённые спином),
//! каждая орбита — кольцо пути в наклонённой плоскости родителя, вокруг
//! сцены — координатный бокс. Глубина пикселя — метрика Фубини–Штуди,
//! сегментация — классы тел (id объектов для пост-эффектов/пикинга —
//! прямой аналог stencil/segmentation буферов UE, только честная).

use std::path::PathBuf;
use std::time::Instant;

use super::scene::CameraSpec;
use super::world::{BodyClass, World};
use crate::p3::ffi::Camera;

/// Параметры кадра игрового мира.
#[derive(Clone, Debug)]
pub struct FrameConfig {
    pub width: u32,
    pub height: u32,
    pub camera: CameraSpec,
    /// Рисовать кольца орбит.
    pub render_orbits: bool,
    /// Рисовать координатный бокс вокруг сцены.
    pub render_box: bool,
    /// Имя сцены для аннотации.
    pub scene_name: String,
    /// Каталог для PNG.
    pub out_dir: PathBuf,
}

impl Default for FrameConfig {
    fn default() -> Self {
        FrameConfig {
            width: 960,
            height: 540,
            camera: CameraSpec::default(),
            render_orbits: true,
            render_box: true,
            scene_name: "aetheria".into(),
            out_dir: PathBuf::from("."),
        }
    }
}

/// Итог рендера кадра мира.
#[derive(Clone, Debug)]
pub struct FrameOutput {
    pub rgb_png: PathBuf,
    pub depth_png: PathBuf,
    pub seg_png: PathBuf,
    pub n_points: usize,
    pub n_edges: usize,
    pub painted_px: usize,
    pub max_fs_depth: f64,
    pub tick: u64,
    pub state_hash: u64,
    pub elapsed_ms: f64,
}

/// Геометрия извлечённого мира.
struct ExtractedScene {
    pts: Vec<f64>,   // однородные [X:Y:Z:W] подряд
    segs: Vec<u8>,   // сегмент точки
    pairs: Vec<u32>, // рёбра (i0, j0, i1, j1, …)
}

/// Целевой полугабарит нормализованной сцены: |coord| ≤ NORM_TARGET < 1
/// гарантирует доминирование W-карты при дегомогенизации (см.
/// HomVec4.pickBestCard) — евклидова перспектива без искажений карты.
const NORM_TARGET: f64 = 0.92;

/// Вращение точки вокруг оси Y на угол `s` (для визуализации спина).
#[inline]
fn rot_y(p: [f64; 3], s: f64) -> [f64; 3] {
    let (sin, cos) = s.sin_cos();
    [
        p[0] * cos + p[2] * sin,
        p[1],
        -p[0] * sin + p[2] * cos,
    ]
}

/// Точка орбитального кольца в наклонённой плоскости (та же параметризация,
/// что и `Orbit::local_pos` — кольцо пути совпадает с траекторией).
#[inline]
fn orbit_ring_point(radius: f64, angle: f64, incl: f64) -> [f64; 3] {
    let (s, c) = angle.sin_cos();
    let (ci, si) = (incl.cos(), incl.sin());
    [radius * c, radius * s * si, radius * s * ci]
}

/// Извлечь мир в геометрию P³ и нормализовать к [-NORM_TARGET, NORM_TARGET]
/// (защита афинной карты: |XYZ| < |W| — дегомогенизация всегда в W-карте).
fn extract(world: &World, render_orbits: bool) -> ExtractedScene {
    let pts: Vec<f64> = Vec::with_capacity(4096);
    let segs: Vec<u8> = Vec::with_capacity(1024);
    let pairs: Vec<u32> = Vec::with_capacity(4096);

    let push_pt = |buf: &mut ExtractedScene, p: [f64; 3], seg: u8| {
        buf.pts.extend_from_slice(&[p[0], p[1], p[2], 1.0]);
        buf.segs.push(seg);
    };

    let mut buf = ExtractedScene {
        pts,
        segs,
        pairs,
    };

    const SPHERE_SEG: usize = 12; // сегментов на кольцо каркаса
    const ORBIT_SEG: usize = 36; // сегментов на кольцо орбиты

    for e in world.entities() {
        let Some(body) = world.body(e) else { continue };
        let Some(wpos) = world.world_pos(e) else { continue };
        let spin = world.world_spin(e).unwrap_or(0.0);
        let scale = world.transform(e).map(|t| t.scale).unwrap_or(1.0);
        let r = body.radius * scale;
        let seg = body.seg_override.unwrap_or_else(|| body.class.seg_id());

        // --- центр масс ---
        let center_idx = buf.segs.len() as u32;
        push_pt(&mut buf, wpos, seg);

        // --- каркас сферы: три кольца, повёрнутые спином ---
        let mut ring_first: Vec<u32> = Vec::with_capacity(3);
        for plane in 0..3 {
            let first = buf.segs.len() as u32;
            ring_first.push(first);
            for k in 0..SPHERE_SEG {
                let a = 2.0 * std::f64::consts::PI * (k as f64) / (SPHERE_SEG as f64);
                let local = match plane {
                    0 => rot_y([r * a.cos(), r * a.sin(), 0.0], spin),
                    1 => rot_y([0.0, r * a.cos(), r * a.sin()], spin),
                    _ => rot_y([r * a.cos(), 0.0, r * a.sin()], spin),
                };
                push_pt(
                    &mut buf,
                    [
                        wpos[0] + local[0],
                        wpos[1] + local[1],
                        wpos[2] + local[2],
                    ],
                    seg,
                );
                // ребро к следующей точке кольца (циклически)
                let next = if k + 1 == SPHERE_SEG { first } else { first + k as u32 + 1 };
                buf.pairs.extend_from_slice(&[first + k as u32, next]);
            }
        }
        // кольца связаны с центром (радиальные спицы — только у звёзд/станций)
        if matches!(body.class, BodyClass::Star | BodyClass::Station) {
            for &first in &ring_first {
                // одна спица на кольцо (k=0) — антенна/луч
                buf.pairs.extend_from_slice(&[center_idx, first]);
            }
        }

        // --- кольцо орбиты вокруг родителя ---
        if render_orbits {
            if let Some(o) = world.orbit(e) {
                let Some(parent) = world.parent(e) else { continue };
                let Some(ppos) = world.world_pos(parent) else { continue };
                let first = buf.segs.len() as u32;
                for k in 0..ORBIT_SEG {
                    let a = 2.0 * std::f64::consts::PI * (k as f64) / (ORBIT_SEG as f64);
                    let p = orbit_ring_point(o.radius, a, o.inclination);
                    push_pt(
                        &mut buf,
                        [ppos[0] + p[0], ppos[1] + p[1], ppos[2] + p[2]],
                        7, // оранжевый — пути орбит
                    );
                    let next = if k + 1 == ORBIT_SEG { first } else { first + k as u32 + 1 };
                    buf.pairs.extend_from_slice(&[first + k as u32, next]);
                }
            }
        }
    }

    // --- нормализация ДАННЫХ (тела + орбиты) к [-NORM_TARGET, NORM_TARGET] ---
    // (перспектива масштаб-инвариантна при согласованном cam_dist; глубина
    // d_FS монотонна расстоянию — порядок глубины сохраняется точно;
    // бокс добавляется ПОСЛЕ — в нормализованных координатах)
    let mut extent: f64 = 0.0;
    for chunk in buf.pts.chunks_exact(4) {
        for c in &chunk[0..3] {
            extent = extent.max(c.abs());
        }
    }
    if extent < 1e-9 {
        extent = 1.0; // вырожденная сцена — защита от деления на ноль
    }
    let s = NORM_TARGET / extent;
    for chunk in buf.pts.chunks_exact_mut(4) {
        chunk[0] *= s;
        chunk[1] *= s;
        chunk[2] *= s;
    }

    // --- координатный бокс: чуть за данными (full-bleed, края уходят
    //     за кадр — кинематографичная отсылка масштаба) ---
    let b = NORM_TARGET * 1.065; // < 1: W-карта доминирует и для углов
    let corners: [[f64; 3]; 8] = [
        [-b, -b, -b], [b, -b, -b], [b, b, -b], [-b, b, -b],
        [-b, -b, b], [b, -b, b], [b, b, b], [-b, b, b],
    ];
    let box_first = buf.segs.len() as u32;
    for c in corners {
        push_pt(&mut buf, c, 9);
    }
    for (a, b2) in [
        (0u32, 1u32), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ] {
        buf.pairs.extend_from_slice(&[box_first + a, box_first + b2]);
    }

    buf
}

/// Аннотация кадра: заголовок, легенда классов, строка детерминизма.
fn annotate(
    rgb: &mut [u8],
    width: u32,
    cfg: &FrameConfig,
    out: &FrameStatsForAnnotation,
) {
    use crate::p3::render::{draw_swatch, draw_text};
    let white = [235u8, 240, 248];
    let dim = [150u8, 158, 172];
    let (m, ts) = (16usize, 2usize);

    // тёмная плашка
    let panel_h = m + 7 * ts + 10 + 7 + 14;
    for y in 0..panel_h.min(out.height_for_panel) {
        for x in 0..(width as usize).min(900) {
            let idx = y * width as usize * 3 + x * 3;
            if idx + 2 < rgb.len() {
                rgb[idx] = (rgb[idx] / 4).max(4);
                rgb[idx + 1] = (rgb[idx + 1] / 4).max(5);
                rgb[idx + 2] = (rgb[idx + 2] / 4).max(8);
            }
        }
    }

    draw_text(
        rgb,
        width,
        m,
        m,
        &format!(
            "POLER GAME CORE | SCENE: {} | TICK {} | WORLD {} BODIES",
            cfg.scene_name.to_uppercase(),
            out.tick,
            out.n_bodies
        ),
        white,
        ts,
    );

    // легенда классов
    let legend: [([u8; 3], &str); 6] = [
        ([252, 214, 96], "STAR "),
        ([96, 202, 252], "PLANET "),
        ([172, 176, 190], "MOON "),
        ([140, 180, 255], "STATION "),
        ([186, 148, 252], "PROBE "),
        ([252, 168, 96], "ORBIT "),
    ];
    let ly = m + 7 * ts + 10;
    let mut lx = m;
    for (col, label) in legend {
        draw_swatch(rgb, width, lx, ly + 1, col, 7);
        draw_text(rgb, width, lx + 11, ly, label, dim, 1);
        lx += 11 + 6 * label.chars().count() + 16;
    }

    // строка детерминизма
    let hy = ly + 12;
    draw_text(
        rgb,
        width,
        m,
        hy,
        &format!(
            "STATE-HASH 0x{:016X} | DETERMINISTIC | KEPLER OMEGA=SQRT(GM)/R^1.5 | DEPTH=D_FS",
            out.state_hash
        ),
        dim,
        1,
    );
}

/// Служебная структура для аннотации (чтобы не таскать всё).
struct FrameStatsForAnnotation {
    tick: u64,
    state_hash: u64,
    n_bodies: usize,
    height_for_panel: usize,
}

/// Сырой кадр: RGB + глубина + сегментация (без записи на диск).
/// Сырьё для `render_frame` (PNG) и для `WindowBackend::present`.
#[derive(Clone, Debug)]
pub struct RawFrame {
    /// Аннотированный RGB8 (3 байта/пиксель).
    pub rgb: Vec<u8>,
    /// Честная глубина d_FS (f32, 1e29 = пусто).
    pub depth: Vec<f32>,
    /// Сегментация (классы тел).
    pub seg: Vec<u8>,
    pub n_points: usize,
    pub n_edges: usize,
    pub painted_px: usize,
    pub max_fs_depth: f64,
}

/// Ядро рендера: извлечение → P³ → аннотация. Без диска.
pub fn render_raw(world: &World, cfg: &FrameConfig) -> Result<RawFrame, String> {
    if cfg.width < 64 || cfg.height < 64 || cfg.width > 4096 || cfg.height > 4096 {
        return Err("размер кадра: 64..=4096 по каждой стороне".into());
    }
    if world.is_empty() {
        return Err("мир пуст — нечего рендерить".into());
    }

    // --- 1. Извлечение геометрии ---
    let scene = extract(world, cfg.render_orbits);
    let n_points = scene.segs.len();

    // --- 2. Рендер геометрии (Native Rust P³-растеризатор) ---
    let npix = cfg.width as usize * cfg.height as usize;
    let mut rgb = vec![0u8; npix * 3];
    let mut depth = vec![0f32; npix];
    let mut seg = vec![0u8; npix];
    let camera = Camera {
        focal: 0.0, // авто
        // дистанция — в долях полугабарита сцены (сцена нормализована
        // к ±NORM_TARGET): dist=3.0 → камера на 3 полугабарита от центра
        cam_dist: cfg.camera.dist * NORM_TARGET,
        yaw: cfg.camera.yaw,
        pitch: cfg.camera.pitch,
    };
    crate::p3::native::render_frame_native(
        &scene.pts,
        &scene.segs,
        &scene.pairs,
        cfg.width,
        cfg.height,
        camera,
        2,
        &mut rgb,
        &mut depth,
        &mut seg,
    )?;

    let painted_px = seg.iter().filter(|&&s| s != 0).count();
    let max_fs_depth = depth
        .iter()
        .filter(|&&d| d < 1e29)
        .cloned()
        .fold(0.0f32, f32::max) as f64;
    if painted_px < 32 {
        return Err(format!(
            "рендер пуст: {painted_px} пикселей — проверьте камеру/сцену"
        ));
    }

    // --- 3. Аннотация RGB ---
    annotate(
        &mut rgb,
        cfg.width,
        cfg,
        &FrameStatsForAnnotation {
            tick: world.tick,
            state_hash: world.state_hash(),
            n_bodies: world.len(),
            height_for_panel: cfg.height as usize,
        },
    );

    let n_edges = scene.pairs.len() / 2;
    Ok(RawFrame { rgb, depth, seg, n_points, n_edges, painted_px, max_fs_depth })
}

/// Отрендерить мир (текущее состояние, после тика) в тройку PNG.
pub fn render_frame(world: &World, cfg: &FrameConfig) -> Result<FrameOutput, String> {
    let t0 = Instant::now();
    std::fs::create_dir_all(&cfg.out_dir)
        .map_err(|e| format!("out_dir {}: {e}", cfg.out_dir.display()))?;

    let raw = render_raw(world, cfg)?;
    let npix = cfg.width as usize * cfg.height as usize;

    // --- 4. PNG: RGB / depth (grayscale d_FS) / seg (палитра) ---
    let stem = format!("game_{}_t{}", cfg.scene_name, world.tick);
    let rgb_png = cfg.out_dir.join(format!("{stem}_rgb.png"));
    let depth_png = cfg.out_dir.join(format!("{stem}_depth.png"));
    let seg_png = cfg.out_dir.join(format!("{stem}_seg.png"));

    let mut gray = vec![0u8; npix];
    let scale = 255.0 / std::f64::consts::FRAC_PI_2;
    for (g, &d) in gray.iter_mut().zip(raw.depth.iter()) {
        *g = if d >= 1e29 {
            0
        } else {
            let v = 255.0 - (d as f64) * scale * 0.9;
            v.clamp(6.0, 255.0) as u8
        };
    }
    let seg_palette: [[u8; 3]; 10] = [
        [8, 8, 14],
        [96, 202, 252],
        [252, 128, 144],
        [148, 252, 128],
        [252, 214, 96],
        [186, 148, 252],
        [128, 252, 214],
        [252, 168, 96],
        [172, 176, 190],
        [140, 180, 255],
    ];
    let mut seg_rgb = vec![0u8; npix * 3];
    for (dst, &s) in seg_rgb.chunks_exact_mut(3).zip(raw.seg.iter()) {
        let c = seg_palette[(s as usize) % seg_palette.len()];
        dst.copy_from_slice(&c);
    }

    crate::p3::png::encode_rgb(&rgb_png, cfg.width, cfg.height, &raw.rgb)
        .map_err(|e| format!("PNG rgb: {e}"))?;
    crate::p3::png::encode_gray(&depth_png, cfg.width, cfg.height, &gray)
        .map_err(|e| format!("PNG depth: {e}"))?;
    crate::p3::png::encode_rgb(&seg_png, cfg.width, cfg.height, &seg_rgb)
        .map_err(|e| format!("PNG seg: {e}"))?;

    Ok(FrameOutput {
        rgb_png,
        depth_png,
        seg_png,
        n_points: raw.n_points,
        n_edges: raw.n_edges,
        painted_px: raw.painted_px,
        max_fs_depth: raw.max_fs_depth,
        tick: world.tick,
        state_hash: world.state_hash(),
        elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
    })
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::loop_::FIXED_DT;
    use crate::game::scene::demo_scene;

    fn tiny_cfg(dir: &std::path::Path) -> FrameConfig {
        FrameConfig {
            width: 160,
            height: 120,
            out_dir: dir.to_path_buf(),
            ..Default::default()
        }
    }

    #[test]
    fn render_smoke_and_valid_pngs() {
        let mut world = demo_scene().build_world().unwrap();
        for _ in 0..90 {
            world.tick(FIXED_DT);
        }
        let dir = std::env::temp_dir().join("poler_game_render_test");
        let out = render_frame(&world, &tiny_cfg(&dir)).expect("рендер мира");
        assert!(out.painted_px > 64, "painted = {}", out.painted_px);
        assert!(out.max_fs_depth <= std::f64::consts::FRAC_PI_2 + 1e-4);
        assert!(out.n_points > 300, "каркасы + орбиты: {}", out.n_points);
        assert!(out.n_edges > 300);
        for p in [&out.rgb_png, &out.depth_png, &out.seg_png] {
            let bytes = std::fs::read(p).unwrap();
            let (w, h, _ct, px) = crate::p3::png::decode_own(&bytes).unwrap();
            assert_eq!((w, h), (160, 120));
            let ch = if p.to_string_lossy().contains("_depth") { 1 } else { 3 };
            assert_eq!(px.len(), (w * h) as usize * ch);
        }
    }

    #[test]
    fn render_is_deterministic_bit_to_bit() {
        let mut a = demo_scene().build_world().unwrap();
        let mut b = demo_scene().build_world().unwrap();
        for _ in 0..30 {
            a.tick(FIXED_DT);
            b.tick(FIXED_DT);
        }
        let dir = std::env::temp_dir().join("poler_game_render_det");
        let oa = render_frame(&a, &tiny_cfg(&dir)).unwrap();
        let ob = render_frame(&b, &tiny_cfg(&dir)).unwrap();
        assert_eq!(oa.state_hash, ob.state_hash, "state-hash миров");
        let fa = std::fs::read(&oa.rgb_png).unwrap();
        let fb = std::fs::read(&ob.rgb_png).unwrap();
        assert_eq!(fa, fb, "PNG байт-в-байт");
    }

    #[test]
    fn empty_world_rejected() {
        let world = World::new();
        let dir = std::env::temp_dir().join("poler_game_render_empty");
        assert!(render_frame(&world, &tiny_cfg(&dir)).is_err());
    }

    #[test]
    fn spin_rotates_wireframe() {
        // rot_y(π/2): X → -Z (правый винт)
        let p = rot_y([1.0, 0.0, 0.0], std::f64::consts::FRAC_PI_2);
        assert!(p[0].abs() < 1e-15 && p[1].abs() < 1e-15);
        assert!((p[2] + 1.0).abs() < 1e-15, "z = -1: {:?}", p);
        // точка орбиты: радиус сохраняется
        let q = orbit_ring_point(2.0, 0.7, 0.3);
        let r2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
        assert!((r2 - 4.0).abs() < 1e-12);
    }
}

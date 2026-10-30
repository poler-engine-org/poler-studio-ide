//! Сцена: сериализация (serde JSON) + встроенная демо-сцена «Этерия».
//!
//! Формат соответствует UE-паттерну «уровень = декларативный контент»,
//! но без бинарных .uasset: сцена — читаемый JSON, который человек может
//! править руками, а `diff` — видеть изменения (ассеты-текст vs
//! ассеты-блоб; мы за текст, блобы придут с `.poler`-контейнером).
//!
//! Правило топологии: родитель обязан быть объявлен РАНЬШЕ ребёнка —
//! циклы невозможны конструктивно, проверка на границе.

use serde::{Deserialize, Serialize};

use super::orbit::Orbit;
use super::transform::Transform;
use super::world::{Body, BodyClass, Entity, World};

/// Спецификация камеры для рендера сцены.
///
/// `dist` — дистанция от центра сцены в ДОЛЯХ её полугабарита
/// (сцена нормализуется к ±0.92): 3.0 — классический ракурс, 2.0 —
/// широкоугольный крупный план, 5.0 — далёкий обзор.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct CameraSpec {
    pub dist: f64,
    pub yaw: f64,
    pub pitch: f64,
}

impl Default for CameraSpec {
    fn default() -> Self {
        CameraSpec {
            dist: 2.8,
            yaw: -0.62,
            pitch: 0.40,
        }
    }
}

/// Описание одного тела сцены.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct BodySpec {
    /// Уникальный идентификатор (на него ссылаются дети).
    pub id: String,
    /// Родитель (обязан быть объявлен выше по файлу) или null.
    pub parent: Option<String>,
    /// Класс: star | planet | moon | station | probe.
    pub class: String,
    /// Физический радиус (каркас в рендере).
    pub radius: f64,
    /// Масса (задаёт кеплеровские ω детей).
    pub mass: f64,
    /// Радиус орбиты вокруг родителя (нет — тело статично/прикреплено).
    #[serde(default)]
    pub orbit_radius: Option<f64>,
    /// Начальная фаза орбиты (рад).
    #[serde(default)]
    pub orbit_angle: Option<f64>,
    /// Наклон орбитальной плоскости (рад).
    #[serde(default)]
    pub inclination: Option<f64>,
    /// Скорость самовращения (рад/с).
    #[serde(default)]
    pub spin_rate: Option<f64>,
    /// Начальный спин (рад).
    #[serde(default)]
    pub spin: Option<f64>,
    /// Именованный цвет → сегмент рендера (см. `color_seg`).
    #[serde(default)]
    pub color: Option<String>,
    /// Фиксированное локальное смещение (для станций без орбиты).
    #[serde(default)]
    pub offset: Option<[f64; 3]>,
}

/// Файл сцены.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SceneFile {
    pub name: String,
    #[serde(default)]
    pub camera: CameraSpec,
    pub bodies: Vec<BodySpec>,
}

/// Именованный цвет → сегмент палитры P³ (зеркально Zig-рендеру).
/// None → сегмент по классу тела.
pub fn color_seg(name: &str) -> Option<u8> {
    Some(match name.to_lowercase().as_str() {
        "cyan" | "голубой" => 1,
        "red" | "красный" => 2,
        "green" | "зелёный" | "зеленый" => 3,
        "gold" | "золотой" => 4,
        "violet" | "фиолетовый" => 5,
        "teal" => 6,
        "orange" | "оранжевый" => 7,
        "grey" | "gray" | "серый" => 8,
        "steel" | "сталь" => 9,
        _ => return None,
    })
}

fn class_of(s: &str) -> Result<BodyClass, String> {
    match s.to_ascii_lowercase().as_str() {
        "star" | "звезда" => Ok(BodyClass::Star),
        "planet" | "планета" => Ok(BodyClass::Planet),
        "moon" | "луна" => Ok(BodyClass::Moon),
        "station" | "станция" => Ok(BodyClass::Station),
        "probe" | "зонд" => Ok(BodyClass::Probe),
        other => Err(format!("неизвестный класс тела `{other}` (star|planet|moon|station|probe)")),
    }
}

impl SceneFile {
    /// Собрать мир из спецификации. Родитель обязан быть объявлен раньше
    /// (иначе — ошибка на границе, мир не строится наполовину).
    pub fn build_world(&self) -> Result<World, String> {
        let mut world = World::new();
        let mut by_id: std::collections::HashMap<String, Entity> = std::collections::HashMap::new();
        let mut planet_idx = 0u8;

        for spec in &self.bodies {
            let class = class_of(&spec.class)?;
            if by_id.contains_key(&spec.id) {
                return Err(format!("дубликат id `{}`", spec.id));
            }
            if spec.mass < 0.0 || !spec.mass.is_finite() {
                return Err(format!("{}: масса должна быть ≥ 0 и конечной", spec.id));
            }
            if spec.radius <= 0.0 || !spec.radius.is_finite() {
                return Err(format!("{}: радиус должен быть > 0", spec.id));
            }

            let parent = match &spec.parent {
                Some(p) => match by_id.get(p) {
                    Some(e) => Some(*e),
                    None => {
                        return Err(format!(
                            "{}: родитель `{p}` не объявлен ранее (топологический порядок)",
                            spec.id
                        ))
                    }
                },
                None => None,
            };

            let e = world.spawn(&spec.id);

            // сегмент рендера: явный цвет > класс
            let seg = spec
                .color
                .as_deref()
                .and_then(color_seg)
                .or(match class {
                    BodyClass::Planet => {
                        // планеты раскладываются по палитре 1,2,3,5,6,7…
                        planet_idx += 1;
                        Some(match planet_idx {
                            1 => 1,
                            2 => 2,
                            3 => 3,
                            4 => 5,
                            5 => 6,
                            _ => 7,
                        })
                    }
                    _ => None,
                })
                .unwrap_or_else(|| class.seg_id());

            world
                .set_body(
                    e,
                    Body {
                        class,
                        mass: spec.mass,
                        radius: spec.radius,
                        seg_override: Some(seg),
                    },
                )
                .map_err(|err| format!("{}: {err}", spec.id))?;

            // трансформация: орбита или фиксированное смещение
            let mut tr = Transform {
                spin: spec.spin.unwrap_or(0.0),
                spin_rate: spec.spin_rate.unwrap_or(0.0),
                ..Transform::NEUTRAL
            };
            if let (Some(r), Some(p)) = (spec.orbit_radius, parent) {
                let m_parent = world.body(p).map(|b| b.mass).unwrap_or(0.0);
                let orbit = Orbit::kepler(
                    m_parent,
                    r,
                    spec.orbit_angle.unwrap_or(0.0),
                    spec.inclination.unwrap_or(0.0),
                );
                tr.pos = orbit.local_pos();
                world.set_orbit(e, orbit).map_err(|err| format!("{}: {err}", spec.id))?;
            } else if let Some(off) = spec.offset {
                tr.pos = off;
            }
            world.set_transform(e, tr).map_err(|err| format!("{}: {err}", spec.id))?;

            if let Some(p) = parent {
                world.set_parent(e, Some(p)).map_err(|err| format!("{}: {err}", spec.id))?;
            }
            by_id.insert(spec.id.clone(), e);
        }

        if world.is_empty() {
            return Err("сцена пуста: ни одного тела".into());
        }
        if !world.hierarchy_valid() {
            return Err("иерархия сцены некорректна (внутренняя ошибка)".into());
        }
        Ok(world)
    }

    /// Загрузить сцену из JSON-файла.
    pub fn load_json(path: &std::path::Path) -> Result<SceneFile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("чтение {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("JSON {}: {e}", path.display()))
    }
}

/// Демо-сцена «Этерия» — историческая система из лора проекта:
/// звезда Этерия, четыре планеты, луны, орбитальная станция и зонд.
/// Кеплеровские ω выводятся из масс (никаких подобранных скоростей).
pub fn demo_scene() -> SceneFile {
    SceneFile {
        name: "aetheria".into(),
        camera: CameraSpec::default(),
        bodies: vec![
            BodySpec {
                id: "aetheria".into(),
                parent: None,
                class: "star".into(),
                radius: 0.55,
                mass: 4.0,
                spin_rate: Some(0.25),
                color: Some("gold".into()),
                ..Default::default()
            },
            BodySpec {
                id: "crystal".into(),
                parent: Some("aetheria".into()),
                class: "planet".into(),
                radius: 0.14,
                mass: 0.30,
                orbit_radius: Some(1.6),
                orbit_angle: Some(0.3),
                inclination: Some(0.08),
                spin_rate: Some(1.1),
                color: Some("cyan".into()),
                ..Default::default()
            },
            BodySpec {
                id: "crystal-moon".into(),
                parent: Some("crystal".into()),
                class: "moon".into(),
                radius: 0.05,
                mass: 0.02,
                orbit_radius: Some(0.32),
                orbit_angle: Some(2.1),
                spin_rate: Some(2.0),
                color: Some("grey".into()),
                ..Default::default()
            },
            BodySpec {
                id: "polar".into(),
                parent: Some("aetheria".into()),
                class: "planet".into(),
                radius: 0.20,
                mass: 0.45,
                orbit_radius: Some(2.6),
                orbit_angle: Some(2.4),
                inclination: Some(-0.15),
                spin_rate: Some(0.8),
                color: Some("red".into()),
                ..Default::default()
            },
            BodySpec {
                id: "poler-station".into(),
                parent: Some("polar".into()),
                class: "station".into(),
                radius: 0.06,
                mass: 0.001,
                orbit_radius: Some(0.38),
                orbit_angle: Some(4.0),
                color: Some("steel".into()),
                ..Default::default()
            },
            BodySpec {
                id: "chor".into(),
                parent: Some("aetheria".into()),
                class: "planet".into(),
                radius: 0.17,
                mass: 0.35,
                orbit_radius: Some(3.8),
                orbit_angle: Some(4.6),
                inclination: Some(0.22),
                spin_rate: Some(0.6),
                color: Some("green".into()),
                ..Default::default()
            },
            BodySpec {
                id: "chor-moon-a".into(),
                parent: Some("chor".into()),
                class: "moon".into(),
                radius: 0.045,
                mass: 0.015,
                orbit_radius: Some(0.30),
                orbit_angle: Some(0.9),
                color: Some("grey".into()),
                ..Default::default()
            },
            BodySpec {
                id: "chor-moon-b".into(),
                parent: Some("chor".into()),
                class: "moon".into(),
                radius: 0.04,
                mass: 0.010,
                orbit_radius: Some(0.44),
                orbit_angle: Some(3.5),
                inclination: Some(0.5),
                color: Some("orange".into()),
                ..Default::default()
            },
            BodySpec {
                id: "depth".into(),
                parent: Some("aetheria".into()),
                class: "planet".into(),
                radius: 0.26,
                mass: 0.55,
                orbit_radius: Some(5.4),
                orbit_angle: Some(1.2),
                inclination: Some(-0.06),
                spin_rate: Some(0.5),
                color: Some("violet".into()),
                ..Default::default()
            },
            BodySpec {
                id: "depth-moon".into(),
                parent: Some("depth".into()),
                class: "moon".into(),
                radius: 0.07,
                mass: 0.03,
                orbit_radius: Some(0.52),
                orbit_angle: Some(5.2),
                color: Some("grey".into()),
                ..Default::default()
            },
            BodySpec {
                id: "probe".into(),
                parent: Some("aetheria".into()),
                class: "probe".into(),
                radius: 0.035,
                mass: 0.0005,
                orbit_radius: Some(6.6),
                orbit_angle: Some(5.8),
                inclination: Some(0.65),
                spin_rate: Some(3.0),
                color: Some("teal".into()),
                ..Default::default()
            },
        ],
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_scene_builds() {
        let scene = demo_scene();
        assert_eq!(scene.bodies.len(), 11);
        let world = scene.build_world().unwrap();
        assert_eq!(world.len(), 11);
        assert!(world.hierarchy_valid());
        assert_eq!(world.find("aetheria").and_then(|e| world.parent(e)), None);
        let ch = world.find("crystal").unwrap();
        assert_eq!(
            world.parent(ch).and_then(|p| world.name(p).map(str::to_string)),
            Some("aetheria".into())
        );
    }

    #[test]
    fn demo_scene_deterministic_state_hash() {
        // одна и та же сцена + тики → бит-в-бит одинаковый мир
        let mut a = demo_scene().build_world().unwrap();
        let mut b = demo_scene().build_world().unwrap();
        for _ in 0..120 {
            a.tick(super::super::loop_::FIXED_DT);
            b.tick(super::super::loop_::FIXED_DT);
        }
        assert_eq!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn json_roundtrip_preserves_simulation() {
        let scene = demo_scene();
        let json = serde_json::to_string_pretty(&scene).unwrap();
        let back: SceneFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, scene.name);
        assert_eq!(back.bodies.len(), scene.bodies.len());
        let mut a = scene.build_world().unwrap();
        let mut b = back.build_world().unwrap();
        for _ in 0..60 {
            a.tick(1.0 / 60.0);
            b.tick(1.0 / 60.0);
        }
        assert_eq!(
            a.state_hash(),
            b.state_hash(),
            "JSON round-trip обязан сохранять симуляцию"
        );
    }

    #[test]
    fn unknown_parent_rejected() {
        let scene = SceneFile {
            name: "broken".into(),
            camera: Default::default(),
            bodies: vec![BodySpec {
                id: "orphan".into(),
                parent: Some("ghost".into()),
                class: "planet".into(),
                radius: 0.1,
                mass: 0.1,
                ..Default::default()
            }],
        };
        let err = scene.build_world().err().expect("сцена с ghost-родителем обязана пасть");
        assert!(err.contains("ghost"), "ошибка должна называть родителя: {err}");
    }

    #[test]
    fn unknown_class_rejected() {
        let scene = SceneFile {
            name: "broken".into(),
            camera: Default::default(),
            bodies: vec![BodySpec {
                id: "weird".into(),
                parent: None,
                class: "black-hole".into(),
                radius: 0.1,
                mass: 0.1,
                ..Default::default()
            }],
        };
        assert!(scene.build_world().is_err());
    }

    #[test]
    fn invalid_json_reports_path() {
        let dir = std::env::temp_dir().join("poler_game_scene_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("bad.json");
        std::fs::write(&p, "{ не json").unwrap();
        let err = SceneFile::load_json(&p).unwrap_err();
        assert!(err.contains("bad.json"), "ошибка называет файл: {err}");
    }

    #[test]
    fn colors_map_to_segments() {
        assert_eq!(color_seg("gold"), Some(4));
        assert_eq!(color_seg("ЗЕЛЁНЫЙ"), Some(3));
        assert_eq!(color_seg("сталь"), Some(9));
        assert_eq!(color_seg("magenta"), None);
    }

    #[test]
    fn kepler_omega_from_parent_mass() {
        let scene = demo_scene();
        let world = scene.build_world().unwrap();
        let planet = world.find("crystal").unwrap();
        let orbit = world.orbit(planet).unwrap();
        let expect = (1.0f64 * 4.0).sqrt() / 1.6f64.powf(1.5);
        assert!((orbit.omega - expect).abs() < 1e-15);
        // луна: ω от массы планеты 0.30
        let moon = world.find("crystal-moon").unwrap();
        let mo = world.orbit(moon).unwrap();
        let m_expect = (0.30f64).sqrt() / 0.32f64.powf(1.5);
        assert!((mo.omega - m_expect).abs() < 1e-15);
    }
}

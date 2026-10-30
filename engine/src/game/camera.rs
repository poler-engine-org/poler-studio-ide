//! # U4: Орбит-камера под управлением ввода (цикл U, v0.56.0)
//!
//! Камера — единственный «зритель» мира, и её траектория обязана быть
//! детерминированной функцией потока событий: один replay-сценарий —
//! бит-в-бит одинаковая камера (контракт `camera_hash`).
//!
//! Управление (раскладка по умолчанию):
//! - **ЛКМ + движение** — орбита (yaw/pitch);
//! - **колесо** — дистанция (zoom), клампится в [min, max];
//! - **стрелки / WASD** — орбита без мыши (доступность, CI-скрипты);
//! - **Q/E** — дистанция без колеса.
//!
//! Плавность — экспоненциальное демпфирование к цели:
//! `v ← v + (v* − v) · (1 − e^(−λ·dt))` — кадронезависимо и
//! детерминированно при фиксированном `dt` (наш GameClock).

use super::events::MouseButton;
use super::input::Input;
use super::scene::CameraSpec;

/// Действия камеры (id для `ActionMap`; камера читает Input напрямую —
/// эти константы для документации и шелл-биндингов).
pub const ACTION_ORBIT_KEY: u32 = 100;

/// Орбит-камера: yaw/pitch/dist вокруг центра мира.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    /// Азимут (рад). Возрастает влево.
    pub yaw: f64,
    /// Наклон (рад). Положительный — сверху.
    pub pitch: f64,
    /// Дистанция в полугабаритах сцены.
    pub dist: f64,
    /// Цели (демпфер тянет к ним).
    target_yaw: f64,
    target_pitch: f64,
    target_dist: f64,
    /// Пределы.
    pub min_dist: f64,
    pub max_dist: f64,
    pub min_pitch: f64,
    pub max_pitch: f64,
    /// Скорость орбиты (рад на пиксель драга).
    pub rotate_speed: f64,
    /// Скорость орбиты с клавиатуры (рад/с).
    pub key_speed: f64,
    /// Шаг колеса (мультипликативный: dist /= step).
    pub wheel_step: f64,
    /// Скорость демпфирования (1/с): больше — резче.
    pub smoothing: f64,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        // Стартуем из дефолтной CameraSpec (сцена «Этерия» цикла S)
        let s = CameraSpec::default();
        OrbitCamera {
            yaw: s.yaw,
            pitch: s.pitch,
            dist: s.dist,
            target_yaw: s.yaw,
            target_pitch: s.pitch,
            target_dist: s.dist,
            min_dist: 1.2,
            max_dist: 12.0,
            min_pitch: -1.45,
            max_pitch: 1.45,
            rotate_speed: 0.008,
            key_speed: 1.4,
            wheel_step: 1.15,
            smoothing: 10.0,
        }
    }
}

impl OrbitCamera {
    /// Новый с начальной позицией `spec`.
    pub fn from_spec(spec: &CameraSpec) -> Self {
        let mut c = OrbitCamera::default();
        c.yaw = spec.yaw;
        c.pitch = spec.pitch;
        c.dist = spec.dist;
        c.target_yaw = spec.yaw;
        c.target_pitch = spec.pitch;
        c.target_dist = spec.dist;
        c
    }

    fn clamp_targets(&mut self) {
        self.target_pitch = self.target_pitch.clamp(self.min_pitch, self.max_pitch);
        self.target_dist = self.target_dist.clamp(self.min_dist, self.max_dist);
        // yaw не зажимаем (полный оборот), но держим в разумном коридоре
        if self.target_yaw > std::f64::consts::TAU {
            self.target_yaw -= std::f64::consts::TAU;
        }
        if self.target_yaw < -std::f64::consts::TAU {
            self.target_yaw += std::f64::consts::TAU;
        }
    }

    /// Один шаг камеры: свёрнутый ввод кадра → цели → демпфер.
    ///
    /// Вызывается раз в **фиксированный** тик (детерминизм: при одном
    /// сценарии событий и dt результат бит-в-бит воспроизводим).
    pub fn tick(&mut self, input: &Input, dt: f64) {
        // --- мышь: орбита при удержании ЛКМ ---
        if input.mouse_is_down(MouseButton::Left) {
            let (dx, dy) = input.mouse_delta();
            if dx != 0.0 || dy != 0.0 {
                self.target_yaw += dx * self.rotate_speed;
                self.target_pitch -= dy * self.rotate_speed;
            }
        }
        // --- колесо: мультипликативный zoom (дробные дельты трекпадов ---
        // поддерживаются естественно: 0.4 «зубца» = 0.4 шага)
        let wheel = input.wheel().clamp(-24.0, 24.0);
        if wheel != 0.0 {
            self.target_dist /= self.wheel_step.powf(wheel);
        }
        // --- клавиатура: стрелки/WASD орбита, Q/E дистанция ---
        let mut key_yaw = 0.0f64;
        let mut key_pitch = 0.0f64;
        if input.is_down(crate::game::input::KeyCode::Left)
            || input.is_down(crate::game::input::KeyCode::A)
        {
            key_yaw -= 1.0;
        }
        if input.is_down(crate::game::input::KeyCode::Right)
            || input.is_down(crate::game::input::KeyCode::D)
        {
            key_yaw += 1.0;
        }
        if input.is_down(crate::game::input::KeyCode::Up)
            || input.is_down(crate::game::input::KeyCode::W)
        {
            key_pitch += 1.0;
        }
        if input.is_down(crate::game::input::KeyCode::Down)
            || input.is_down(crate::game::input::KeyCode::S)
        {
            key_pitch -= 1.0;
        }
        self.target_yaw += key_yaw * self.key_speed * dt;
        self.target_pitch += key_pitch * self.key_speed * dt;
        if input.is_down(crate::game::input::KeyCode::Q) {
            self.target_dist *= 1.0 + 0.9 * dt;
        }
        if input.is_down(crate::game::input::KeyCode::E) {
            self.target_dist *= 1.0 - 0.9 * dt;
        }
        self.clamp_targets();

        // --- демпфер: экспоненциальное подтягивание к целям ---
        let k = 1.0 - (-self.smoothing * dt).exp();
        let k = k.clamp(0.0, 1.0);
        self.yaw += (self.target_yaw - self.yaw) * k;
        self.pitch += (self.target_pitch - self.pitch) * k;
        self.dist += (self.target_dist - self.dist) * k;
    }

    /// Спецификация для рендера (совместимость со сценами цикла S).
    pub fn spec(&self) -> CameraSpec {
        CameraSpec { dist: self.dist, yaw: self.yaw, pitch: self.pitch }
    }

    /// Хеш состояния камеры — компонента replay-вердикта.
    pub fn camera_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for v in [self.yaw, self.pitch, self.dist, self.target_yaw, self.target_pitch, self.target_dist] {
            h ^= v.to_bits();
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

#[cfg(test)]
impl OrbitCamera {
    /// Только для тестов: цель по yaw.
    fn target_yaw_for_test(&self) -> f64 {
        self.target_yaw
    }
    /// Только для тестов: цель по dist.
    fn target_dist_for_test(&self) -> f64 {
        self.target_dist
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::events::{Event, KeyPhase};
    use crate::game::input::KeyCode;

    const DT: f64 = crate::game::FIXED_DT;

    fn frame(camera: &mut OrbitCamera, input: &mut Input) {
        camera.tick(input, DT);
        input.end_frame();
    }

    fn key(code: KeyCode, phase: KeyPhase) -> Event {
        Event::Key { code, phase }
    }

    #[test]
    fn drag_orbits_yaw_pitch() {
        let mut cam = OrbitCamera::default();
        let mut inp = Input::default();
        inp.on_event(&Event::MouseButton { button: MouseButton::Left, phase: KeyPhase::Pressed });
        inp.on_event(&Event::MouseMove { dx: 100.0, dy: 0.0 });
        let yaw0 = cam.target_yaw_for_test();
        for _ in 0..120 {
            frame(&mut cam, &mut inp);
        }
        assert!(
            (cam.yaw - (yaw0 + 100.0 * 0.008)).abs() < 1e-6,
            "drag X вращает yaw: {} → {}",
            yaw0,
            cam.yaw
        );
        // Y-драг наклоняет вниз (экранная Y вниз → pitch падает)
        inp.on_event(&Event::MouseMove { dx: 0.0, dy: 50.0 });
        for _ in 0..120 {
            frame(&mut cam, &mut inp);
        }
        assert!(cam.pitch < 0.34 - 1e-6, "drag Y вниз уменьшает pitch: {}", cam.pitch);
    }

    #[test]
    fn no_drag_no_orbit() {
        let mut cam = OrbitCamera::default();
        let mut inp = Input::default();
        // Движение без удержания кнопки — камера не меняется
        inp.on_event(&Event::MouseMove { dx: 500.0, dy: 500.0 });
        let (y0, p0, d0) = (cam.yaw, cam.pitch, cam.dist);
        for _ in 0..30 {
            frame(&mut cam, &mut inp);
        }
        assert_eq!((y0, p0, d0), (cam.yaw, cam.pitch, cam.dist));
    }

    #[test]
    fn wheel_zooms_and_clamps() {
        let mut cam = OrbitCamera::default();
        let mut inp = Input::default();
        for _ in 0..60 {
            inp.on_event(&Event::MouseWheel { delta: 1.0 });
            frame(&mut cam, &mut inp);
        }
        // settle: события кончились — демпфер догоняет цель
        for _ in 0..120 {
            frame(&mut cam, &mut inp);
        }
        assert!(
            (cam.dist - cam.min_dist).abs() < 1e-9,
            "зум-ин упирается в min: {} → {}",
            cam.dist,
            cam.min_dist
        );
        for _ in 0..200 {
            inp.on_event(&Event::MouseWheel { delta: -1.0 });
            frame(&mut cam, &mut inp);
        }
        for _ in 0..120 {
            frame(&mut cam, &mut inp);
        }
        assert!((cam.dist - cam.max_dist).abs() < 1e-9, "зум-аут упирается в max");
        assert!(cam.pitch.clamp(cam.min_pitch, cam.max_pitch) == cam.pitch);
    }

    #[test]
    fn keyboard_orbits_and_qe_zooms() {
        let mut cam = OrbitCamera::default();
        let mut inp = Input::default();
        inp.on_event(&key(KeyCode::Left, KeyPhase::Pressed));
        for _ in 0..60 {
            frame(&mut cam, &mut inp);
        }
        assert!(cam.yaw < OrbitCamera::default().yaw, "стрелка влево крутит влево");
        inp.on_event(&key(KeyCode::Left, KeyPhase::Released));
        inp.on_event(&key(KeyCode::E, KeyPhase::Pressed));
        let d0 = cam.dist;
        for _ in 0..60 {
            frame(&mut cam, &mut inp);
        }
        assert!(cam.dist < d0, "E приближает");
        inp.on_event(&key(KeyCode::E, KeyPhase::Released));
        let d_after_e = cam.dist;
        inp.on_event(&key(KeyCode::Q, KeyPhase::Pressed));
        for _ in 0..60 {
            frame(&mut cam, &mut inp);
        }
        assert!(cam.dist > d_after_e, "Q отдаляет от точки после E");
    }

    #[test]
    fn smoothing_converges_to_target() {
        let mut cam = OrbitCamera::default();
        let mut inp = Input::default();
        inp.on_event(&Event::MouseWheel { delta: 3.0 });
        cam.tick(&inp, DT); // цели обновились по колесу
        let target = cam.target_dist_for_test();
        inp.end_frame();
        // 3τ сглаживания: остаток ~ e^-30 < 1e-12
        for _ in 0..180 {
            frame(&mut cam, &mut inp);
        }
        assert!((cam.dist - target).abs() < 1e-9, "демпфер сходится: {} → {}", cam.dist, target);
        assert!(cam.dist < OrbitCamera::default().dist, "колесо + реально приближает (zoom-in)");
    }

    #[test]
    fn deterministic_replay_bit_exact() {
        let run = || {
            let mut cam = OrbitCamera::default();
            let mut inp = Input::default();
            // Сценарий: drag + wheel + клавиши вперемешку
            inp.on_event(&Event::MouseButton { button: MouseButton::Left, phase: KeyPhase::Pressed });
            for i in 0..30u64 {
                if i % 5 == 0 {
                    inp.on_event(&Event::MouseWheel { delta: (i as f64 % 3.0) - 1.0 });
                }
                inp.on_event(&Event::MouseMove { dx: (i % 7) as f64 - 3.0, dy: 1.0 });
                inp.on_event(&key(KeyCode::W, KeyPhase::Pressed));
                frame(&mut cam, &mut inp);
            }
            inp.on_event(&Event::MouseButton { button: MouseButton::Left, phase: KeyPhase::Released });
            for _ in 0..90 {
                frame(&mut cam, &mut inp);
            }
            cam.camera_hash()
        };
        assert_eq!(run(), run(), "replay камеры бит-в-бит");
    }

    #[test]
    fn spec_roundtrip_and_default_matches_scene() {
        let cam = OrbitCamera::default();
        let spec = cam.spec();
        assert_eq!(spec.yaw, cam.yaw);
        assert_eq!(spec.pitch, cam.pitch);
        assert_eq!(spec.dist, cam.dist);
        let d = CameraSpec::default();
        assert!((spec.yaw - d.yaw).abs() < 1e-12 && (spec.dist - d.dist).abs() < 1e-12);
        let cam2 = OrbitCamera::from_spec(&spec);
        assert_eq!(cam.camera_hash(), cam2.camera_hash(), "from_spec восстанавливает");
    }
}

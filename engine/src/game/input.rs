//! # U2: Ввод — свёрнутое состояние клавиатуры/мыши (цикл U, v0.56.0)
//!
//! События (`events.rs`) — поток; ввод — **снимок состояния** кадра.
//! Разделение как в боевых движках, но без аллокаций на событие:
//! клавиатура — два слова `u64` (128 клавиш), мышь — биты + пара `f64`.
//!
//! Рёбра (edges) — сердце API:
//! - `is_down`     — удержание (движение, панорама);
//! - `just_pressed` — фронт нажатия **ровно один кадр** (прыжок, выстрел);
//! - `just_released` — фронт отпускания (переменный прыжок и т.п.).
//!
//! `KeyPhase::Repeat` (автоповтор ОС) не порождает новых рёбер:
//! удержание уже видно через `is_down`.
//!
//! `end_frame()` очищает рёбра, дельты и текстовый буфер — вызывается
//! ПОСЛЕ логики кадра, ПОСЛЕ того как весь ввод откачан из очереди.

use super::events::{Event, KeyPhase, MouseButton};

/// Код клавиши — платформенно-независимый контекст.
/// Числа стабильны (контракт сериализации/replay).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum KeyCode {
    Escape = 1,
    Enter = 2,
    Space = 3,
    Tab = 4,
    Backspace = 5,
    Delete = 6,
    Home = 7,
    End = 8,
    PageUp = 9,
    PageDown = 10,
    Insert = 11,
    Up = 12,
    Down = 13,
    Left = 14,
    Right = 15,
    A = 32,
    B = 33,
    C = 34,
    D = 35,
    E = 36,
    F = 37,
    G = 38,
    H = 39,
    I = 40,
    J = 41,
    K = 42,
    L = 43,
    M = 44,
    N = 45,
    O = 46,
    P = 47,
    Q = 48,
    R = 49,
    S = 50,
    T = 51,
    U = 52,
    V = 53,
    W = 54,
    X = 55,
    Y = 56,
    Z = 57,
    Num0 = 64,
    Num1 = 65,
    Num2 = 66,
    Num3 = 67,
    Num4 = 68,
    Num5 = 69,
    Num6 = 70,
    Num7 = 71,
    Num8 = 72,
    Num9 = 73,
    LShift = 80,
    RShift = 81,
    LCtrl = 82,
    RCtrl = 83,
    LAlt = 84,
    RAlt = 85,
    F1 = 96,
    F2 = 97,
    F3 = 98,
    F4 = 99,
    F5 = 100,
    F6 = 101,
    F7 = 102,
    F8 = 103,
    F9 = 104,
    F10 = 105,
    F11 = 106,
    F12 = 107,
}

/// Верхняя граница битовой карты клавиш.
const KEY_BITS: usize = 128;
const WORDS: usize = KEY_BITS / 64;

impl KeyCode {
    /// Стабильный числовой код (см. дискриминанты).
    pub fn bits(self) -> u16 {
        self as u16
    }

    /// Из стабильного числа (после сериализации/replay).
    pub fn from_bits(v: u16) -> Option<Self> {
        ALL_KEYS.iter().copied().find(|k| k.bits() == v)
    }

    /// Человекочитаемое имя (для биндингов в shell/JSON).
    pub fn as_str(self) -> &'static str {
        use KeyCode::*;
        match self {
            Escape => "esc",
            Enter => "enter",
            Space => "space",
            Tab => "tab",
            Backspace => "backspace",
            Delete => "delete",
            Home => "home",
            End => "end",
            PageUp => "pageup",
            PageDown => "pagedown",
            Insert => "insert",
            Up => "up",
            Down => "down",
            Left => "left",
            Right => "right",
            A => "a",
            B => "b",
            C => "c",
            D => "d",
            E => "e",
            F => "f",
            G => "g",
            H => "h",
            I => "i",
            J => "j",
            K => "k",
            L => "l",
            M => "m",
            N => "n",
            O => "o",
            P => "p",
            Q => "q",
            R => "r",
            S => "s",
            T => "t",
            U => "u",
            V => "v",
            W => "w",
            X => "x",
            Y => "y",
            Z => "z",
            Num0 => "0",
            Num1 => "1",
            Num2 => "2",
            Num3 => "3",
            Num4 => "4",
            Num5 => "5",
            Num6 => "6",
            Num7 => "7",
            Num8 => "8",
            Num9 => "9",
            LShift => "lshift",
            RShift => "rshift",
            LCtrl => "lctrl",
            RCtrl => "rctrl",
            LAlt => "lalt",
            RAlt => "ralt",
            F1 => "f1",
            F2 => "f2",
            F3 => "f3",
            F4 => "f4",
            F5 => "f5",
            F6 => "f6",
            F7 => "f7",
            F8 => "f8",
            F9 => "f9",
            F10 => "f10",
            F11 => "f11",
            F12 => "f12",
        }
    }

    /// Из имени (`"esc"`, `"w"`, `"f3"`…).
    pub fn parse(s: &str) -> Option<Self> {
        let ls = s.trim().to_ascii_lowercase();
        ALL_KEYS.iter().copied().find(|k| k.as_str() == ls)
    }
}

/// Полный список клавиш (для from_bits/parse без таблицы hand-roll).
const ALL_KEYS: [KeyCode; 69] = [
    KeyCode::Escape,
    KeyCode::Enter,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Insert,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::A,
    KeyCode::B,
    KeyCode::C,
    KeyCode::D,
    KeyCode::E,
    KeyCode::F,
    KeyCode::G,
    KeyCode::H,
    KeyCode::I,
    KeyCode::J,
    KeyCode::K,
    KeyCode::L,
    KeyCode::M,
    KeyCode::N,
    KeyCode::O,
    KeyCode::P,
    KeyCode::Q,
    KeyCode::R,
    KeyCode::S,
    KeyCode::T,
    KeyCode::U,
    KeyCode::V,
    KeyCode::W,
    KeyCode::X,
    KeyCode::Y,
    KeyCode::Z,
    KeyCode::Num0,
    KeyCode::Num1,
    KeyCode::Num2,
    KeyCode::Num3,
    KeyCode::Num4,
    KeyCode::Num5,
    KeyCode::Num6,
    KeyCode::Num7,
    KeyCode::Num8,
    KeyCode::Num9,
    KeyCode::LShift,
    KeyCode::RShift,
    KeyCode::LCtrl,
    KeyCode::RCtrl,
    KeyCode::LAlt,
    KeyCode::RAlt,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];

#[inline]
fn key_index(code: KeyCode) -> usize {
    let b = code.bits() as usize;
    debug_assert!(b < KEY_BITS, "код клавиши вне карты: {b}");
    b
}

#[inline]
fn get(bits: &[u64; WORDS], idx: usize) -> bool {
    bits[idx / 64] & (1u64 << (idx % 64)) != 0
}

#[inline]
fn set(bits: &mut [u64; WORDS], idx: usize, v: bool) {
    if v {
        bits[idx / 64] |= 1u64 << (idx % 64);
    } else {
        bits[idx / 64] &= !(1u64 << (idx % 64));
    }
}

/// Свёрнутое состояние ввода на кадр.
#[derive(Debug, Clone)]
pub struct Input {
    /// Удерживаемые клавиши.
    down: [u64; WORDS],
    /// Фронт нажатия (живёт один кадр — до `end_frame`).
    pressed_edge: [u64; WORDS],
    /// Фронт отпускания.
    released_edge: [u64; WORDS],
    /// Кнопки мыши (бит 0 = левая).
    mouse_down: u8,
    mouse_pressed: u8,
    mouse_released: u8,
    /// Позиция курсора в окне (px, Y вниз).
    mouse_pos: (f64, f64),
    /// Накопленная дельта за кадр (из коалесцированного MouseMove).
    mouse_delta: (f64, f64),
    /// Накопленное колесо за кадр.
    wheel: f64,
    /// Текстовый ввод кадра.
    text: Vec<char>,
    /// Пользователь просил закрыть окно.
    close_requested: bool,
    /// Последний размер клиентской области.
    size: (u32, u32),
    /// Фокус.
    focused: bool,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            down: [0; WORDS],
            pressed_edge: [0; WORDS],
            released_edge: [0; WORDS],
            mouse_down: 0,
            mouse_pressed: 0,
            mouse_released: 0,
            mouse_pos: (0.0, 0.0),
            mouse_delta: (0.0, 0.0),
            wheel: 0.0,
            text: Vec::new(),
            close_requested: false,
            size: (0, 0),
            focused: true,
        }
    }
}

impl Input {
    /// Пропустить событие через состояние (вызывается на drain очереди).
    pub fn on_event(&mut self, ev: &Event) {
        match ev {
            Event::WindowClose => self.close_requested = true,
            Event::WindowResize { w, h } => self.size = (*w, *h),
            Event::WindowFocus { gained } => self.focused = *gained,
            Event::Key { code, phase } => {
                let idx = key_index(*code);
                match phase {
                    KeyPhase::Pressed => {
                        if !get(&self.down, idx) {
                            set(&mut self.pressed_edge, idx, true);
                        }
                        set(&mut self.down, idx, true);
                    }
                    KeyPhase::Released => {
                        set(&mut self.down, idx, false);
                        set(&mut self.released_edge, idx, true);
                    }
                    // Автоповтор: состояние не меняет (уже down)
                    KeyPhase::Repeat => {}
                }
            }
            Event::Text(c) => {
                if !c.is_control() {
                    self.text.push(*c);
                }
            }
            Event::MouseMove { dx, dy } => {
                self.mouse_delta.0 += *dx;
                self.mouse_delta.1 += *dy;
                self.mouse_pos.0 += *dx;
                self.mouse_pos.1 += *dy;
            }
            Event::MouseButton { button, phase } => {
                let mask = 1u8 << button.index();
                match phase {
                    KeyPhase::Pressed => {
                        if self.mouse_down & mask == 0 {
                            self.mouse_pressed |= mask;
                        }
                        self.mouse_down |= mask;
                    }
                    KeyPhase::Released => {
                        self.mouse_down &= !mask;
                        self.mouse_released |= mask;
                    }
                    KeyPhase::Repeat => {}
                }
            }
            Event::MouseWheel { delta } => self.wheel += *delta,
            Event::User(_) => {}
        }
    }

    /// Конец кадра: рёбра, дельты, колесо и текст гаснут.
    /// Вызывать ПОСЛЕ разбора очереди и логики кадра.
    pub fn end_frame(&mut self) {
        self.pressed_edge = [0; WORDS];
        self.released_edge = [0; WORDS];
        self.mouse_pressed = 0;
        self.mouse_released = 0;
        self.mouse_delta = (0.0, 0.0);
        self.wheel = 0.0;
        self.text.clear();
    }

    /// Клавиша удерживается?
    pub fn is_down(&self, code: KeyCode) -> bool {
        get(&self.down, key_index(code))
    }

    /// Нажата ровно в этом кадре?
    pub fn just_pressed(&self, code: KeyCode) -> bool {
        get(&self.pressed_edge, key_index(code))
    }

    /// Отпущена ровно в этом кадре?
    pub fn just_released(&self, code: KeyCode) -> bool {
        get(&self.released_edge, key_index(code))
    }

    /// Кнопка мыши удерживается?
    pub fn mouse_is_down(&self, b: MouseButton) -> bool {
        self.mouse_down & (1 << b.index()) != 0
    }

    /// Кнопка мыши нажата в этом кадре?
    pub fn mouse_just_pressed(&self, b: MouseButton) -> bool {
        self.mouse_pressed & (1 << b.index()) != 0
    }

    /// Дельта мыши за кадр (px).
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    /// Позиция курсора (px, Y вниз).
    pub fn mouse_pos(&self) -> (f64, f64) {
        self.mouse_pos
    }

    /// Колесо за кадр (positive = zoom in).
    pub fn wheel(&self) -> f64 {
        self.wheel
    }

    /// Символы текстового ввода кадра.
    pub fn text_chars(&self) -> &[char] {
        &self.text
    }

    /// Владельческий забор текста (для обработчиков) — буфер очищается.
    pub fn take_text(&mut self) -> String {
        self.text.drain(..).collect()
    }

    /// Пользователь закрыл окно?
    pub fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Размер клиентской области (последнее известное).
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Фокус окна.
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Хеш состояния ввода — маяк детерминизма replay:
    /// состояние мира + хеш ввода кадра → следующий кадр определён.
    pub fn input_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for w in self.down.iter().chain(self.pressed_edge.iter()).chain(self.released_edge.iter()) {
            h ^= *w;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= (self.mouse_down as u64) << 56;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        h ^= self.mouse_delta.0.to_bits() ^ (self.mouse_delta.1.to_bits() << 1);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        h ^= self.wheel.to_bits();
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        h
    }
}

// ---------------------------------------------------------------------------
// Actions: семантические биндинги
// ---------------------------------------------------------------------------

/// Одна привязка действия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Карта действий: id → набор биндингов (ИЛИ).
/// Действие активно, если активен любой биндинг.
#[derive(Debug, Clone, Default)]
pub struct ActionMap {
    bindings: Vec<(u32, Bind)>,
}

impl ActionMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Привязать `bind` к действию `action`.
    pub fn bind(&mut self, action: u32, bind: Bind) -> &mut Self {
        self.bindings.push((action, bind));
        self
    }

    /// Действие удерживается?
    pub fn is_down(&self, input: &Input, action: u32) -> bool {
        self.bindings
            .iter()
            .any(|(a, b)| *a == action && Self::bind_down(input, *b))
    }

    /// Фронт действия?
    pub fn just_pressed(&self, input: &Input, action: u32) -> bool {
        self.bindings
            .iter()
            .any(|(a, b)| *a == action && Self::bind_pressed(input, *b))
    }

    fn bind_down(input: &Input, b: Bind) -> bool {
        match b {
            Bind::Key(k) => input.is_down(k),
            Bind::Mouse(m) => input.mouse_is_down(m),
        }
    }

    fn bind_pressed(input: &Input, b: Bind) -> bool {
        match b {
            Bind::Key(k) => input.just_pressed(k),
            Bind::Mouse(m) => input.mouse_just_pressed(m),
        }
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> Event {
        Event::Key { code, phase: KeyPhase::Pressed }
    }
    fn release(code: KeyCode) -> Event {
        Event::Key { code, phase: KeyPhase::Released }
    }

    #[test]
    fn edge_lifecycle_press_hold_release() {
        let mut inp = Input::default();
        // Кадр 1: нажали W
        inp.on_event(&press(KeyCode::W));
        assert!(inp.just_pressed(KeyCode::W));
        assert!(inp.is_down(KeyCode::W));
        assert!(!inp.just_released(KeyCode::W));
        inp.end_frame();
        // Кадр 2: держим (новых событий нет)
        assert!(!inp.just_pressed(KeyCode::W), "фронт живёт один кадр");
        assert!(inp.is_down(KeyCode::W), "удержание живёт");
        inp.end_frame();
        // Кадр 3: отпустили
        inp.on_event(&release(KeyCode::W));
        assert!(!inp.is_down(KeyCode::W));
        assert!(inp.just_released(KeyCode::W));
        inp.end_frame();
        assert!(!inp.just_released(KeyCode::W));
    }

    #[test]
    fn repeat_does_not_retrigger_edge() {
        let mut inp = Input::default();
        inp.on_event(&press(KeyCode::Space));
        inp.end_frame();
        inp.on_event(&Event::Key { code: KeyCode::Space, phase: KeyPhase::Repeat });
        inp.on_event(&Event::Key { code: KeyCode::Space, phase: KeyPhase::Repeat });
        assert!(!inp.just_pressed(KeyCode::Space), "автоповтор — не фронт");
        assert!(inp.is_down(KeyCode::Space));
    }

    #[test]
    fn double_press_without_release_is_not_two_edges() {
        let mut inp = Input::default();
        inp.on_event(&press(KeyCode::Escape));
        inp.on_event(&Event::Key { code: KeyCode::Escape, phase: KeyPhase::Pressed });
        assert!(inp.just_pressed(KeyCode::Escape));
        inp.end_frame();
        // Осталась одна — end_frame между нажатиями разделил бы фронты
        assert!(!inp.just_pressed(KeyCode::Escape));
    }

    #[test]
    fn mouse_delta_accumulates_and_resets() {
        let mut inp = Input::default();
        inp.on_event(&Event::MouseMove { dx: 3.0, dy: -2.0 });
        inp.on_event(&Event::MouseMove { dx: 1.5, dy: 0.5 });
        let (dx, dy) = inp.mouse_delta();
        assert!((dx - 4.5).abs() < 1e-9 && (dy + 1.5).abs() < 1e-9);
        let (px, py) = inp.mouse_pos();
        assert!((px - 4.5).abs() < 1e-9 && (py + 1.5).abs() < 1e-9);
        inp.end_frame();
        assert_eq!(inp.mouse_delta(), (0.0, 0.0));
        // Позиция сохраняется, дельта обнуляется
        assert_eq!(inp.mouse_pos(), (4.5, -1.5));
    }

    #[test]
    fn mouse_buttons_and_wheel() {
        let mut inp = Input::default();
        inp.on_event(&Event::MouseButton { button: MouseButton::Left, phase: KeyPhase::Pressed });
        assert!(inp.mouse_is_down(MouseButton::Left));
        assert!(inp.mouse_just_pressed(MouseButton::Left));
        assert!(!inp.mouse_is_down(MouseButton::Right));
        inp.on_event(&Event::MouseWheel { delta: 2.0 });
        inp.on_event(&Event::MouseWheel { delta: -0.5 });
        assert!((inp.wheel() - 1.5).abs() < 1e-9);
        inp.end_frame();
        assert!(inp.mouse_is_down(MouseButton::Left), "удержание живёт");
        assert!(!inp.mouse_just_pressed(MouseButton::Left));
        assert_eq!(inp.wheel(), 0.0);
    }

    #[test]
    fn text_buffer_filters_control() {
        let mut inp = Input::default();
        inp.on_event(&Event::Text('п'));
        inp.on_event(&Event::Text('r'));
        inp.on_event(&Event::Text('\u{7}')); // BEL — control
        let taken = inp.take_text();
        assert_eq!(taken, "пr");
        assert!(inp.text.is_empty());
    }

    #[test]
    fn window_state_reflected() {
        let mut inp = Input::default();
        inp.on_event(&Event::WindowResize { w: 800, h: 600 });
        assert_eq!(inp.size(), (800, 600));
        assert!(!inp.close_requested());
        inp.on_event(&Event::WindowFocus { gained: false });
        assert!(!inp.focused());
        inp.on_event(&Event::WindowClose);
        assert!(inp.close_requested());
        // close_requested — липкий: end_frame его не гасит (решение за игрой)
        inp.end_frame();
        assert!(inp.close_requested());
    }

    #[test]
    fn key_roundtrip_bits_parse_str() {
        for k in ALL_KEYS {
            let b = k.bits();
            assert_eq!(KeyCode::from_bits(b), Some(k), "bits roundtrip {k:?}");
            assert_eq!(KeyCode::parse(k.as_str()), Some(k), "str roundtrip {k:?}");
            assert!(KeyCode::parse(&format!("  {} ", k.as_str().to_uppercase())).is_some());
        }
        assert!(KeyCode::from_bits(0).is_none());
        assert!(KeyCode::from_bits(200).is_none());
        assert!(KeyCode::parse("not-a-key").is_none());
        assert_eq!(ALL_KEYS.len(), 69);
    }

    #[test]
    fn actions_or_semantics() {
        const JUMP: u32 = 1;
        let mut map = ActionMap::new();
        map.bind(JUMP, Bind::Key(KeyCode::Space))
            .bind(JUMP, Bind::Mouse(MouseButton::Right));
        let mut inp = Input::default();
        inp.on_event(&press(KeyCode::Space));
        assert!(map.is_down(&inp, JUMP));
        assert!(map.just_pressed(&inp, JUMP));
        inp.end_frame();
        inp.on_event(&Event::MouseButton { button: MouseButton::Right, phase: KeyPhase::Pressed });
        assert!(map.just_pressed(&inp, JUMP), "второй биндинг тоже стреляет");
        // Не привязанное действие молчит
        assert!(!map.is_down(&inp, 99));
    }

    #[test]
    fn input_hash_deterministic_and_sensitive() {
        let build = || {
            let mut i = Input::default();
            i.on_event(&press(KeyCode::W));
            i.on_event(&press(KeyCode::LShift));
            i.on_event(&Event::MouseMove { dx: -2.5, dy: 1.0 });
            i
        };
        assert_eq!(build().input_hash(), build().input_hash());
        let mut other = build();
        other.on_event(&Event::MouseMove { dx: 0.001, dy: 0.0 });
        assert_ne!(build().input_hash(), other.input_hash());
        assert_eq!(Input::default().input_hash(), Input::default().input_hash());
    }
}

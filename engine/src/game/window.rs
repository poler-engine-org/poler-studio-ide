//! # U3: Окно — платформенные бэкенды (цикл U, v0.56.0)
//!
//! Тракт окна отделён от логики: игра пишет в `WindowBackend`, не зная,
//! куда попадут пиксели — в реальное X11-окно или в детерминированный
//! офлайн-рекордер.
//!
//! ```text
//!   ┌────────────┐   poll_event()   ┌──────────────────┐
//!   │ X11Window  │ ───────────────▶ │  EventQueue      │ → Input → логика
//!   │ (dlopen)   │                  └──────────────────┘
//!   │            │   present(rgb)   ┌──────────────────┐
//!   │            │ ◀─────────────── │ render_rgb_core  │
//!   └────────────┘                  └──────────────────┘
//!   ┌────────────┐                        ▲ то же самое
//!   │ Offscreen  │ ──── скрипт событий ───┤ API, кадры → PNG + хеши
//!   └────────────┘                        (replay/CI/эталоны)
//! ```
//!
//! **OffscreenWindow** — окно без окна: событиями кормит скрипт
//! (детерминированный replay), presented-кадры пишутся PNG + хеш
//! FNV-1a. Это CI-эталон всей цепочки ввод→камера→рендер.
//!
//! **X11Window** — настоящее окно Linux через **прямой dlopen
//! libX11.so.6**: ни одной внешней зависимости, никаких winit/SDL.
//! Паттерн тот же, что у P³-моста (`p3::ffi`): extern "C" + dlsym.
//! События конвертируются в платформо-независимые `Event` по
//! стабильной таблице keysym → `KeyCode`. Событийная маска —
//! KeyPress/KeyRelease/Button/Motion/Exposure/Structure/Focus.
//! Закрытие окна ловится через протокол WM_DELETE_WINDOW.
//!
//! Win32-бэкенд — контракт трейта (`cfg(windows)` точка роста, цикл V).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use super::events::Event;

/// Тракт оконного бэкенда.
pub trait WindowBackend {
    /// Достать следующее событие (неблокирующе). None — событий нет.
    fn poll_event(&mut self) -> Option<Event>;

    /// Показать кадр RGB8 (3 байта/пиксель, строки сверху вниз).
    fn present(&mut self, rgb: &[u8], w: u32, h: u32) -> Result<(), String>;

    /// Размер клиентской области.
    fn size(&self) -> (u32, u32);

    /// Пользователь закрыл окно / поток событий исчерпан.
    fn is_closed(&self) -> bool;

    /// Имя бэкенда (диагностика).
    fn backend_name(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// Offscreen: скрипт событий → PNG-кадры + хеши (детерминизм-эталон)
// ---------------------------------------------------------------------------

/// Записанный кадр офлайн-окна.
#[derive(Debug, Clone)]
pub struct FrameRecord {
    /// Порядковый номер presented-кадра.
    pub index: u64,
    /// Путь PNG (None — кадр пропущен прореживанием `every`).
    pub path: Option<PathBuf>,
    /// FNV-1a по пикселям кадра.
    pub frame_hash: u64,
}

/// Окно без окна: события из скрипта, кадры — в PNG + хеши.
pub struct OffscreenWindow {
    width: u32,
    height: u32,
    script: VecDeque<Event>,
    out_dir: PathBuf,
    /// Писать каждый N-й presented-кадр (1 — все).
    every: u32,
    presented: u64,
    frames: Vec<FrameRecord>,
    closed: bool,
    close_on_script_end: bool,
}

impl OffscreenWindow {
    /// `script` — события в порядке выдачи; `every` — прореживание PNG.
    /// Размер берётся как есть: офлайн-эталон не требует «оконного»
    /// минимума (64+ — ограничение живого X11-окна, не рекордера).
    pub fn new(script: Vec<Event>, w: u32, h: u32, out_dir: &Path, every: u32) -> Self {
        assert!(w >= 1 && h >= 1, "офлайн-окно: размер ≥ 1×1");
        OffscreenWindow {
            width: w,
            height: h,
            script: VecDeque::from(script),
            out_dir: out_dir.to_path_buf(),
            every: every.max(1),
            presented: 0,
            frames: Vec::new(),
            closed: false,
            close_on_script_end: true,
        }
    }

    /// Не закрывать «окно» по концу скрипта (для фиксированных прогонов).
    pub fn keep_open_after_script(mut self) -> Self {
        self.close_on_script_end = false;
        self
    }

    /// Хеш всех presented-кадров — маяк детерминизма всей цепочки
    /// ввод → камера → рендер (один сценарий = один хеш).
    pub fn frames_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for f in &self.frames {
            h ^= f.frame_hash;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            h ^= f.index;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// Записанные кадры.
    pub fn frames(&self) -> &[FrameRecord] {
        &self.frames
    }
}

impl WindowBackend for OffscreenWindow {
    fn poll_event(&mut self) -> Option<Event> {
        let ev = self.script.pop_front()?;
        if matches!(ev, Event::WindowClose) {
            self.closed = true;
        }
        Some(ev)
    }

    fn present(&mut self, rgb: &[u8], w: u32, h: u32) -> Result<(), String> {
        if rgb.len() != (w as usize) * (h as usize) * 3 {
            return Err(format!("present: буфер {} ≠ {}×{}×3", rgb.len(), w, h));
        }
        // Хеш — по ВСЕМ кадрам (PNG может прореживаться, хеш — нет)
        let mut fh: u64 = 0xcbf2_9ce4_8422_2325;
        for b in rgb {
            fh ^= *b as u64;
            fh = fh.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let idx = self.presented;
        self.presented += 1;
        let path = if idx % self.every as u64 == 0 {
            std::fs::create_dir_all(&self.out_dir)
                .map_err(|e| format!("out_dir {}: {e}", self.out_dir.display()))?;
            let p = self.out_dir.join(format!("frame_{:05}.png", idx));
            crate::p3::png::encode_rgb(&p, w, h, rgb).map_err(|e| format!("PNG: {e}"))?;
            Some(p)
        } else {
            None
        };
        self.frames.push(FrameRecord { index: idx, path, frame_hash: fh });
        Ok(())
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn backend_name(&self) -> &'static str {
        "offscreen"
    }
}

impl OffscreenWindow {
    /// Скрипт исчерпан? (не то же, что закрыто)
    pub fn script_exhausted(&self) -> bool {
        self.script.is_empty()
    }

    /// Настроено ли закрытие по концу скрипта.
    pub fn closes_on_script_end(&self) -> bool {
        self.close_on_script_end
    }
}

// ---------------------------------------------------------------------------
// X11: настоящее окно через прямой dlopen (zero-dep)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod x11 {
    use super::super::events::{Event, KeyPhase, MouseButton};
    use super::super::input::KeyCode;
    use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};

    // dlopen из libc (как в p3::ffi — без внешних крейтов)
    const RTLD_NOW: c_int = 0x2;
    const RTLD_LOCAL: c_int = 0x0;
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn malloc(size: usize) -> *mut c_void;
        fn free(p: *mut c_void);
        fn memset(p: *mut c_void, c: c_int, n: usize) -> *mut c_void;
    }

    // --- типы X11 ---
    type Display = c_void;
    type Window = c_ulong;
    type Atom = c_ulong;
    type KeySym = c_ulong;
    type Status = c_int;
    type XID = c_ulong;
    #[allow(non_camel_case_types)]
    type XImage = c_void; // непрозрачный: работаем через указатель
    #[allow(non_camel_case_types)]
    type GC = *mut c_void;
    #[allow(non_camel_case_types)]
    type Visual = c_void;
    type Bool = c_int;

    // Событийные маски (X.h)
    const KEY_PRESSMASK: c_long = 1 << 0;
    const KEY_RELEASEMASK: c_long = 1 << 1;
    const BUTTON_PRESSMASK: c_long = 1 << 2;
    const BUTTON_RELEASEMASK: c_long = 1 << 3;
    const POINTER_MOTIONMASK: c_long = 1 << 6;
    const EXPOSUREMASK: c_long = 1 << 15;
    const STRUCTURE_NOTIFYMASK: c_long = 1 << 17;
    const FOCUS_CHANGEMASK: c_long = 1 << 20;

    // Типы событий
    const KEY_PRESS: i32 = 2;
    const KEY_RELEASE: i32 = 3;
    const BUTTON_PRESS: i32 = 4;
    const BUTTON_RELEASE: i32 = 5;
    const MOTION_NOTIFY: i32 = 6;
    const FOCUS_IN: i32 = 9;
    const FOCUS_OUT: i32 = 10;
    const EXPOSE: i32 = 12;
    const CONFIGURE_NOTIFY: i32 = 22;
    const CLIENT_MESSAGE: i32 = 33;

    // Кнопки мыши X11 (Button1..5)
    const BUTTON_LEFT: u32 = 1;
    const BUTTON_MIDDLE: u32 = 2;
    const BUTTON_RIGHT: u32 = 3;
    const BUTTON_WHEEL_UP: u32 = 4;
    const BUTTON_WHEEL_DOWN: u32 = 5;

    // Формат изображения
    const ZPIXMAP: c_int = 2;

    /// Заголовок любого X-события (XAnyEvent).
    #[repr(C)]
    struct XAnyHead {
        type_: i32,
        serial: c_ulong,
        send_event: Bool,
        display: *mut Display,
    }

    /// Общее событие ввода (XKeyEvent/XButtonEvent/XMotionEvent совместимы
    /// до поля 84: keycode/button/is_hint).
    #[repr(C)]
    struct XInputEvent {
        head: XAnyHead,      // 0..32
        window: Window,      // 32
        root: Window,        // 40
        subwindow: Window,   // 48
        time: c_ulong,       // 56
        x: i32,              // 64
        y: i32,              // 68
        x_root: i32,         // 72
        y_root: i32,         // 76
        state: c_uint,       // 80
        code: c_uint,        // 84: keycode | button | is_hint
        same_screen: Bool,   // 88
    }

    /// XConfigureEvent.
    #[repr(C)]
    struct XConfigureEvent {
        head: XAnyHead,        // 0..32
        event: Window,         // 32
        window: Window,        // 40
        x: i32,                // 48
        y: i32,                // 52
        width: i32,            // 56
        height: i32,           // 60
        border_width: i32,     // 64
        above: Window,         // 72
        override_redirect: Bool, // 80
    }

    /// XClientMessageEvent (data.l[0] по смещению 56).
    #[repr(C)]
    struct XClientMessageEvent {
        head: XAnyHead,   // 0..32
        window: Window,   // 32
        message_type: Atom, // 40
        format: i32,      // 48
        _pad: i32,        // 52
        l: [c_ulong; 5],  // 56
    }

    /// Максимальный размер XEvent-объединения на x86_64/aarch64 — 192 байт.
    #[repr(C, align(8))]
    struct XEventBuf {
        data: [u8; 192],
    }

    impl XEventBuf {
        fn zeroed() -> Self {
            XEventBuf { data: [0u8; 192] }
        }
        fn type_(&self) -> i32 {
            i32::from_ne_bytes(self.data[0..4].try_into().unwrap())
        }
        fn as_input(&self) -> &XInputEvent {
            unsafe { &*(self as *const Self as *const XInputEvent) }
        }
        fn as_configure(&self) -> &XConfigureEvent {
            unsafe { &*(self as *const Self as *const XConfigureEvent) }
        }
        fn as_client(&self) -> &XClientMessageEvent {
            unsafe { &*(self as *const Self as *const XClientMessageEvent) }
        }
    }

    // Сигнатуры Xlib (только нужное)
    type FnOpenDisplay = unsafe extern "C" fn(name: *const c_char) -> *mut Display;
    type FnCloseDisplay = unsafe extern "C" fn(d: *mut Display) -> c_int;
    type FnDefaultScreen = unsafe extern "C" fn(d: *mut Display) -> c_int;
    type FnDefaultRootWindow = unsafe extern "C" fn(d: *mut Display) -> Window;
    type FnDefaultDepth = unsafe extern "C" fn(d: *mut Display, s: c_int) -> c_int;
    type FnDefaultVisual = unsafe extern "C" fn(d: *mut Display, s: c_int) -> *mut Visual;
    type FnBlackPixel = unsafe extern "C" fn(d: *mut Display, s: c_int) -> c_ulong;
    type FnCreateSimpleWindow = unsafe extern "C" fn(
        d: *mut Display,
        parent: Window,
        x: i32,
        y: i32,
        w: c_uint,
        h: c_uint,
        border: c_uint,
        border_px: c_ulong,
        bg: c_ulong,
    ) -> Window;
    type FnStoreName = unsafe extern "C" fn(d: *mut Display, w: Window, name: *const c_char) -> c_int;
    type FnSelectInput = unsafe extern "C" fn(d: *mut Display, w: Window, mask: c_long) -> c_int;
    type FnMapWindow = unsafe extern "C" fn(d: *mut Display, w: Window) -> c_int;
    type FnNextEvent = unsafe extern "C" fn(d: *mut Display, ev: *mut XEventBuf) -> c_int;
    type FnPending = unsafe extern "C" fn(d: *mut Display) -> c_int;
    type FnFlush = unsafe extern "C" fn(d: *mut Display) -> c_int;
    type FnInternAtom = unsafe extern "C" fn(d: *mut Display, name: *const c_char, only_if_exists: Bool) -> Atom;
    type FnSetWmProtocols = unsafe extern "C" fn(
        d: *mut Display,
        w: Window,
        atoms: *mut Atom,
        count: c_int,
    ) -> Status;
    type FnCreateGC = unsafe extern "C" fn(d: *mut Display, drawable: XID, mask: c_ulong, values: *const c_void) -> GC;
    type FnCreateImage = unsafe extern "C" fn(
        d: *mut Display,
        vis: *mut Visual,
        depth: c_int,
        format: c_int,
        offset: c_int,
        data: *mut c_char,
        w: c_uint,
        h: c_uint,
        pad: c_int,
        bytes_per_line: c_int,
    ) -> *mut XImage;
    type FnPutImage = unsafe extern "C" fn(
        d: *mut Display,
        drawable: XID,
        gc: GC,
        image: *mut XImage,
        src_x: c_int,
        src_y: c_int,
        dest_x: c_int,
        dest_y: c_int,
        w: c_uint,
        h: c_uint,
    ) -> c_int;
    type FnDestroyImage = unsafe extern "C" fn(image: *mut XImage) -> c_int;
    type FnLookupKeysym = unsafe extern "C" fn(ev: *const XInputEvent, index: c_int) -> KeySym;

    /// keysym → KeyCode (стабильная таблица X11).
    fn keysym_to_key(ks: KeySym) -> Option<KeyCode> {
        Some(match ks {
            0xff1b => KeyCode::Escape,
            0xff0d => KeyCode::Enter,
            0x20 => KeyCode::Space,
            0xff09 => KeyCode::Tab,
            0xff08 => KeyCode::Backspace,
            0xffff => KeyCode::Delete,
            0xff50 => KeyCode::Home,
            0xff57 => KeyCode::End,
            0xff55 => KeyCode::PageUp,
            0xff56 => KeyCode::PageDown,
            0xff63 => KeyCode::Insert,
            0xff52 => KeyCode::Up,
            0xff54 => KeyCode::Down,
            0xff51 => KeyCode::Left,
            0xff53 => KeyCode::Right,
            0x61..=0x7a => {
                return KeyCode::from_bits(32 + (ks - 0x61) as u16);
            }
            0x30..=0x39 => {
                return KeyCode::from_bits(64 + (ks - 0x30) as u16);
            }
            0xffe1 => KeyCode::LShift,
            0xffe2 => KeyCode::RShift,
            0xffe3 => KeyCode::LCtrl,
            0xffe4 => KeyCode::RCtrl,
            0xffe9 => KeyCode::LAlt,
            0xffea => KeyCode::RAlt,
            0xffbe..=0xffc9 => {
                return KeyCode::from_bits(96 + (ks - 0xffbe) as u16);
            }
            _ => return None,
        })
    }

    /// Загруженная таблица символов Xlib.
    struct X11Lib {
        _handle: *mut c_void,
        open_display: FnOpenDisplay,
        close_display: FnCloseDisplay,
        default_screen: FnDefaultScreen,
        default_root_window: FnDefaultRootWindow,
        default_depth: FnDefaultDepth,
        default_visual: FnDefaultVisual,
        black_pixel: FnBlackPixel,
        create_simple_window: FnCreateSimpleWindow,
        store_name: FnStoreName,
        select_input: FnSelectInput,
        map_window: FnMapWindow,
        next_event: FnNextEvent,
        pending: FnPending,
        flush: FnFlush,
        intern_atom: FnInternAtom,
        set_wm_protocols: FnSetWmProtocols,
        create_gc: FnCreateGC,
        create_image: FnCreateImage,
        put_image: FnPutImage,
        destroy_image: FnDestroyImage,
        lookup_keysym: FnLookupKeysym,
    }

    unsafe fn sym<T>(handle: *mut c_void, name: &str) -> Result<T, String>
    where
        T: Copy,
    {
        debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*mut c_void>());
        let c = CString::new(name).map_err(|_| "NUL в символе".to_string())?;
        let p = dlsym(handle, c.as_ptr());
        if p.is_null() {
            return Err(format!("Xlib: символа {name} нет"));
        }
        Ok(std::mem::transmute_copy(&p))
    }

    impl X11Lib {
        fn load() -> Result<Self, String> {
            for name in ["libX11.so.6", "libX11.so"] {
                let cname = CString::new(name).unwrap();
                let handle = unsafe { dlopen(cname.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
                if !handle.is_null() {
                    unsafe {
                        return Ok(X11Lib {
                            open_display: sym(handle, "XOpenDisplay")?,
                            close_display: sym(handle, "XCloseDisplay")?,
                            default_screen: sym(handle, "XDefaultScreen")?,
                            default_root_window: sym(handle, "XDefaultRootWindow")?,
                            default_depth: sym(handle, "XDefaultDepth")?,
                            default_visual: sym(handle, "XDefaultVisual")?,
                            black_pixel: sym(handle, "XBlackPixel")?,
                            create_simple_window: sym(handle, "XCreateSimpleWindow")?,
                            store_name: sym(handle, "XStoreName")?,
                            select_input: sym(handle, "XSelectInput")?,
                            map_window: sym(handle, "XMapWindow")?,
                            next_event: sym(handle, "XNextEvent")?,
                            pending: sym(handle, "XPending")?,
                            flush: sym(handle, "XFlush")?,
                            intern_atom: sym(handle, "XInternAtom")?,
                            set_wm_protocols: sym(handle, "XSetWMProtocols")?,
                            create_gc: sym(handle, "XCreateGC")?,
                            create_image: sym(handle, "XCreateImage")?,
                            put_image: sym(handle, "XPutImage")?,
                            destroy_image: sym(handle, "XDestroyImage")?,
                            lookup_keysym: sym(handle, "XLookupKeysym")?,
                            _handle: handle,
                        });
                    }
                }
            }
            Err("libX11 не найдена (dlopen libX11.so.6 / libX11.so)".into())
        }
    }

    /// Живые ресурсы окна X11.
    struct X11Inner {
        lib: X11Lib,
        display: *mut Display,
        window: Window,
        gc: GC,
        /// XImage с malloc-буфером BGRA (XDestroyImage освободит оба).
        image: *mut XImage,
        /// Ширина/высота image (при Resize — пересоздаётся).
        img_w: u32,
        img_h: u32,
        wm_delete: Atom,
        wm_protocols: Atom,
        /// Последняя позиция курсора (для дельт MotionNotify).
        last_pos: Option<(f64, f64)>,
        /// Буфер события (XNextEvent пишет сюда).
        evbuf: XEventBuf,
        width: u32,
        height: u32,
        closed: bool,
        title_shown: String,
    }

    // Указатели X11 не Send/Sync сами по себе, но мы владеем ими
    // монопольно в одном потоке — окно не гоняется между потоками.
    unsafe impl Send for X11Inner {}

    impl X11Inner {
        /// Сконвертировать X-событие в платформо-независимое.
        unsafe fn translate(&mut self) -> Option<Event> {
            let t = self.evbuf.type_();
            match t {
                KEY_PRESS | KEY_RELEASE => {
                    let ie = self.evbuf.as_input();
                    let phase = if t == KEY_PRESS { KeyPhase::Pressed } else { KeyPhase::Released };
                    let ks = (self.lib.lookup_keysym)(ie, 0);
                    keysym_to_key(ks).map(|code| Event::Key { code, phase })
                }
                BUTTON_PRESS | BUTTON_RELEASE => {
                    let ie = self.evbuf.as_input();
                    let b = ie.code;
                    if t == BUTTON_PRESS && (b == BUTTON_WHEEL_UP || b == BUTTON_WHEEL_DOWN) {
                        // Колесо в X11 — кнопки 4/5
                        let delta = if b == BUTTON_WHEEL_UP { 1.0 } else { -1.0 };
                        return Some(Event::MouseWheel { delta });
                    }
                    let button = match b {
                        BUTTON_LEFT => MouseButton::Left,
                        BUTTON_MIDDLE => MouseButton::Middle,
                        BUTTON_RIGHT => MouseButton::Right,
                        other => MouseButton::X((other - 8) as u8),
                    };
                    let phase = if t == BUTTON_PRESS { KeyPhase::Pressed } else { KeyPhase::Released };
                    Some(Event::MouseButton { button, phase })
                }
                MOTION_NOTIFY => {
                    let ie = self.evbuf.as_input();
                    let (x, y) = (ie.x as f64, ie.y as f64);
                    let (dx, dy) = match self.last_pos {
                        Some((px, py)) => (x - px, y - py),
                        None => (0.0, 0.0),
                    };
                    self.last_pos = Some((x, y));
                    Some(Event::MouseMove { dx, dy })
                }
                FOCUS_IN | FOCUS_OUT => {
                    Some(Event::WindowFocus { gained: t == FOCUS_IN })
                }
                CONFIGURE_NOTIFY => {
                    let ce = self.evbuf.as_configure();
                    let (w, h) = (ce.width.max(1) as u32, ce.height.max(1) as u32);
                    if (w, h) != (self.width, self.height) {
                        self.width = w;
                        self.height = h;
                        Some(Event::WindowResize { w, h })
                    } else {
                        None
                    }
                }
                CLIENT_MESSAGE => {
                    let cm = self.evbuf.as_client();
                    if cm.message_type == self.wm_protocols && cm.l[0] == self.wm_delete {
                        self.closed = true;
                        Some(Event::WindowClose)
                    } else {
                        None
                    }
                }
                EXPOSE => None, // кадр рисуем сами по vsync-подобному ритму
                _ => None,
            }
        }

        /// (Пере)создать XImage под размер окна.
        unsafe fn rebuild_image(&mut self) -> Result<(), String> {
            if !self.image.is_null() {
                (self.lib.destroy_image)(self.image);
                self.image = std::ptr::null_mut();
            }
            let npix = self.width as usize * self.height as usize;
            let buf = malloc(npix * 4);
            if buf.is_null() {
                return Err("X11: malloc для кадра не удался".into());
            }
            memset(buf, 0, npix * 4);
            let depth = (self.lib.default_depth)(self.display, (self.lib.default_screen)(self.display));
            let vis = (self.lib.default_visual)(self.display, (self.lib.default_screen)(self.display));
            let img = (self.lib.create_image)(
                self.display,
                vis,
                depth,
                ZPIXMAP,
                0,
                buf as *mut c_char,
                self.width,
                self.height,
                32,
                (self.width * 4) as c_int,
            );
            if img.is_null() {
                free(buf);
                return Err("XCreateImage не удался".into());
            }
            self.image = img;
            self.img_w = self.width;
            self.img_h = self.height;
            Ok(())
        }

        /// Скопировать RGB8 → BGRA32 прямо в буфер XImage и блитнуть.
        unsafe fn blit_rgb(&mut self, rgb: &[u8], w: u32, h: u32) -> Result<(), String> {
            if rgb.len() != (w as usize) * (h as usize) * 3 {
                return Err(format!("blit: буфер {} ≠ {}×{}×3", rgb.len(), w, h));
            }
            if w != self.img_w || h != self.img_h || self.image.is_null() {
                self.width = w;
                self.height = h;
                self.rebuild_image()?;
            }
            // data XImage — байты B,G,R,X (little-endian ZPixmap 32-bit)
            let dst = (*(self.image as *mut XImageRaw)).data as *mut u8;
            if dst.is_null() {
                return Err("XImage без data".into());
            }
            let mut si = 0usize;
            let mut di = 0usize;
            for _ in 0..(w as usize * h as usize) {
                let r = rgb[si];
                let g = rgb[si + 1];
                let b = rgb[si + 2];
                *dst.add(di) = b;
                *dst.add(di + 1) = g;
                *dst.add(di + 2) = r;
                *dst.add(di + 3) = 0xff;
                si += 3;
                di += 4;
            }
            (self.lib.put_image)(
                self.display,
                self.window,
                self.gc,
                self.image,
                0,
                0,
                0,
                0,
                w,
                h,
            );
            (self.lib.flush)(self.display);
            Ok(())
        }
    }

    /// Минимальное зеркало XImage — нужно только поле `data`.
    #[repr(C)]
    struct XImageRaw {
        width: c_int,
        height: c_int,
        xoffset: c_int,
        format: c_int,
        data: *mut c_char,
        byte_order: c_int,
        bitmap_unit: c_int,
        bitmap_bit_order: c_int,
        bitmap_pad: c_int,
        depth: c_int,
        bytes_per_line: c_int,
        visual: *mut c_void,
        red_mask: c_ulong,
        green_mask: c_ulong,
        blue_mask: c_ulong,
    }

    impl Drop for X11Inner {
        fn drop(&mut self) {
            unsafe {
                if !self.image.is_null() {
                    (self.lib.destroy_image)(self.image);
                }
                if !self.gc.is_null() {
                    // XFreeGC не загружена — утечка GC на drop допустима?
                    // Нет: окно и так уничтожается вместе с соединением.
                }
                if !self.display.is_null() {
                    (self.lib.close_display)(self.display);
                }
            }
        }
    }

    /// Настоящее окно X11 (dlopen libX11, zero-dep).
    pub struct X11Window {
        inner: Option<X11Inner>,
    }

    impl X11Window {
        /// Открыть окно. Ошибка — внятная (нет DISPLAY / нет libX11).
        pub fn open(w: u32, h: u32, title: &str) -> Result<Self, String> {
            let lib = X11Lib::load()?;
            if std::env::var("DISPLAY").map(|d| d.is_empty()).unwrap_or(true) {
                return Err("нет DISPLAY — X-сервер недоступен (headless?)".into());
            }
            unsafe {
                let display = (lib.open_display)(std::ptr::null());
                if display.is_null() {
                    return Err("XOpenDisplay(NULL) не удался".into());
                }
                let screen = (lib.default_screen)(display);
                let root = (lib.default_root_window)(display);
                let bg = (lib.black_pixel)(display, screen);
                let window = (lib.create_simple_window)(
                    display,
                    root,
                    0,
                    0,
                    w.max(64),
                    h.max(64),
                    0,
                    0,
                    bg,
                );
                if window == 0 {
                    (lib.close_display)(display);
                    return Err("XCreateSimpleWindow не удался".into());
                }
                let ctitle = CString::new(title).map_err(|_| "NUL в заголовке".to_string())?;
                (lib.store_name)(display, window, ctitle.as_ptr());
                (lib.select_input)(
                    display,
                    window,
                    KEY_PRESSMASK
                        | KEY_RELEASEMASK
                        | BUTTON_PRESSMASK
                        | BUTTON_RELEASEMASK
                        | POINTER_MOTIONMASK
                        | EXPOSUREMASK
                        | STRUCTURE_NOTIFYMASK
                        | FOCUS_CHANGEMASK,
                );
                // WM_DELETE_WINDOW: закрытие по крестику
                let protocols_c = CString::new("WM_PROTOCOLS").unwrap();
                let delete_c = CString::new("WM_DELETE_WINDOW").unwrap();
                let wm_protocols = (lib.intern_atom)(display, protocols_c.as_ptr(), 1);
                let wm_delete = (lib.intern_atom)(display, delete_c.as_ptr(), 0);
                let mut atoms = [wm_delete];
                (lib.set_wm_protocols)(display, window, atoms.as_mut_ptr(), 1);
                let gc = (lib.create_gc)(display, window, 0, std::ptr::null());
                if gc.is_null() {
                    (lib.close_display)(display);
                    return Err("XCreateGC не удался".into());
                }
                (lib.map_window)(display, window);
                (lib.flush)(display);
                let mut inner = X11Inner {
                    lib,
                    display,
                    window,
                    gc,
                    image: std::ptr::null_mut(),
                    img_w: 0,
                    img_h: 0,
                    wm_delete,
                    wm_protocols,
                    last_pos: None,
                    evbuf: XEventBuf::zeroed(),
                    width: w.max(64),
                    height: h.max(64),
                    closed: false,
                    title_shown: title.to_string(),
                };
                inner.rebuild_image()?;
                let _ = &inner.title_shown;
                Ok(X11Window { inner: Some(inner) })
            }
        }
    }

    impl super::WindowBackend for X11Window {
        fn poll_event(&mut self) -> Option<Event> {
            let inner = self.inner.as_mut()?;
            unsafe {
                while (inner.lib.pending)(inner.display) > 0 {
                    (inner.lib.next_event)(inner.display, &mut inner.evbuf);
                    if let Some(ev) = inner.translate() {
                        return Some(ev);
                    }
                }
            }
            None
        }

        fn present(&mut self, rgb: &[u8], w: u32, h: u32) -> Result<(), String> {
            let inner = self
                .inner
                .as_mut()
                .ok_or_else(|| "X11: соединение закрыто".to_string())?;
            unsafe { inner.blit_rgb(rgb, w, h) }
        }

        fn size(&self) -> (u32, u32) {
            self.inner.as_ref().map(|i| (i.width, i.height)).unwrap_or((0, 0))
        }

        fn is_closed(&self) -> bool {
            self.inner.as_ref().map(|i| i.closed).unwrap_or(true)
        }

        fn backend_name(&self) -> &'static str {
            "x11"
        }
    }
}

#[cfg(target_os = "linux")]
pub use x11::X11Window;

/// Открытие окна для `game window`: на Linux — настоящее X11-окно,
/// на других ОС — честная ошибка (оконный бэкенд один, X11; тип
/// возврата на не-Linux — OffscreenWindow: трейт уже реализован,
/// путь Ok недостижим).
#[cfg(target_os = "linux")]
pub fn open_game_window(w: u32, h: u32, title: &str) -> Result<X11Window, String> {
    X11Window::open(w, h, title)
}

#[cfg(not(target_os = "linux"))]
pub fn open_game_window(_w: u32, _h: u32, _title: &str) -> Result<OffscreenWindow, String> {
    Err("game window: только Linux (X11-бэкенд); детерминированный прогон без окна — game input-demo".into())
}

#[cfg(not(target_os = "linux"))]
/// Заглушка вне Linux: Win32 — цикл V (контракт трейта уже зафиксирован).
pub struct X11Window;

#[cfg(not(target_os = "linux"))]
impl X11Window {
    pub fn open(_w: u32, _h: u32, _title: &str) -> Result<Self, String> {
        Err("X11Window доступен только на linux (Win32 — цикл V)".into())
    }
}

// ============================================================================
// ТЕСТЫ
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::events::KeyPhase;
    use crate::game::input::KeyCode;

    fn offscreen(script: Vec<Event>) -> OffscreenWindow {
        let dir = std::env::temp_dir().join(format!("poler_win_test_{}", std::process::id()));
        OffscreenWindow::new(script, 64, 48, &dir, 1)
    }

    #[test]
    fn offscreen_script_replay_and_close() {
        let script = vec![
            Event::Key { code: KeyCode::W, phase: KeyPhase::Pressed },
            Event::MouseMove { dx: 2.0, dy: 1.0 },
            Event::WindowClose,
        ];
        let mut w = offscreen(script);
        assert!(!w.is_closed());
        assert_eq!(w.poll_event(), Some(Event::Key { code: KeyCode::W, phase: KeyPhase::Pressed }));
        assert_eq!(w.poll_event(), Some(Event::MouseMove { dx: 2.0, dy: 1.0 }));
        assert_eq!(w.poll_event(), Some(Event::WindowClose));
        assert!(w.is_closed(), "WindowClose закрывает");
        assert_eq!(w.poll_event(), None);
        assert_eq!(w.backend_name(), "offscreen");
        assert_eq!(w.size(), (64, 48));
    }

    #[test]
    fn offscreen_present_writes_png_and_hashes() {
        let script = vec![Event::WindowClose];
        let mut w = offscreen(script);
        let (w_, h_) = w.size();
        let rgb: Vec<u8> = (0..(w_ as usize * h_ as usize * 3)).map(|i| (i * 7 % 251) as u8).collect();
        w.present(&rgb, w_, h_).unwrap();
        assert_eq!(w.frames().len(), 1);
        let p0 = w.frames()[0].path.clone().expect("PNG записан");
        let fh0 = w.frames()[0].frame_hash;
        assert!(p0.exists(), "PNG записан");
        // Детерминизм: тот же кадр — тот же хеш
        let mut w2 = offscreen(vec![Event::WindowClose]);
        w2.present(&rgb, w_, h_).unwrap();
        assert_eq!(fh0, w2.frames()[0].frame_hash);
        // Другой кадр — другой хеш
        let mut rgb2 = rgb.clone();
        rgb2[0] ^= 0xff;
        let mut w3 = offscreen(vec![Event::WindowClose]);
        w3.present(&rgb2, w_, h_).unwrap();
        assert_ne!(fh0, w3.frames()[0].frame_hash);
        // Битый буфер — ошибка
        assert!(w.present(&[1u8; 10], 4, 4).is_err());
        // PNG валиден по суверенному декодеру
        let raw = std::fs::read(&p0).unwrap();
        let (pw, ph, ct, _) = crate::p3::png::decode_own(&raw).expect("PNG валиден");
        assert_eq!((pw, ph, ct), (w_, h_, 2));
    }

    #[test]
    fn offscreen_every_thins_png_not_hash() {
        let script = vec![Event::WindowClose];
        let dir = std::env::temp_dir().join(format!("poler_win_thin_{}", std::process::id()));
        let mut w = OffscreenWindow::new(script, 64, 48, &dir, 3);
        let rgb = vec![7u8; 64 * 48 * 3];
        for _ in 0..7 {
            w.present(&rgb, 64, 48).unwrap();
        }
        assert_eq!(w.frames().len(), 7, "хеши — по всем кадрам");
        assert_eq!(w.frames().iter().filter(|f| f.path.is_some()).count(), 3, "PNG — каждый 3-й");
        // frames_hash монотонно включает все
        let h = w.frames_hash();
        assert_ne!(h, 0xcbf2_9ce4_8422_2325, "не база FNV");
    }

    #[test]
    fn offscreen_frames_hash_deterministic_across_runs() {
        let dir = std::env::temp_dir().join(format!("poler_win_det_{}", std::process::id()));
        let build = || {
            let mut w = OffscreenWindow::new(vec![Event::WindowClose], 64, 48, &dir, 2);
            for i in 0..5u8 {
                let rgb = vec![i * 31; 64 * 48 * 3];
                w.present(&rgb, 64, 48).unwrap();
            }
            w
        };
        assert_eq!(build().frames_hash(), build().frames_hash());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn x11_open_fails_gracefully_without_display() {
        // В headless-окружении нет X-сервера: ожидаем внятную ошибку,
        // а не панику/UB. Если DISPLAY вдруг есть — тест просто пропускает.
        if std::env::var("DISPLAY").map(|d| !d.is_empty()).unwrap_or(false) {
            return; // есть живой X — открытие реально, не тестируем здесь
        }
        match X11Window::open(320, 240, "poler-test") {
            Err(e) => {
                assert!(
                    e.contains("DISPLAY") || e.contains("XOpenDisplay") || e.contains("libX11"),
                    "понятная ошибка: {e}"
                );
            }
            Ok(_) => panic!("headless: окно не должно было открыться"),
        }
    }
}

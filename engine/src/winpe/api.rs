//! World — стан Win64-світу + диспетчер + kernel32-шими.
//!
//! Реентерабельність (вивчений урок війни по 7za): НІЯКИХ Mutex по шляху
//! dispatch → шим → SEH-розгортання → деструктор → free → dispatch.
//! Замість цього — Gate: володар=tid, глибина-лічильник (той самий потік
//! входить без блокування, інші чекають спіном на КОРОТКИХ секціях).

use super::crt::{cstr, cstr_w, read_u64, write_u16, write_u32, write_u64, write_u8};
use super::pe::{PeInfo, RuntimeFunction};
use super::runtime::{win_call, DataPage, Handler, ThunkPages, ID_UNKNOWN_BASE};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const REG_BASE: u64 = 0x100;

// ============================== Gate ==============================

pub struct Gate {
    owner: std::sync::atomic::AtomicU64,
    depth: Cell<u32>,
}

impl Gate {
    pub const fn new() -> Self {
        Self {
            owner: std::sync::atomic::AtomicU64::new(0),
            depth: Cell::new(0),
        }
    }

    pub fn enter(&self) {
        let tid = unsafe { libc::syscall(libc::SYS_gettid) } as u64;
        if self.owner.load(Ordering::Relaxed) == tid {
            self.depth.set(self.depth.get() + 1);
            return;
        }
        loop {
            if self
                .owner
                .compare_exchange(0, tid, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                self.depth.set(1);
                return;
            }
            std::hint::spin_loop();
        }
    }

    pub fn leave(&self) {
        let d = self.depth.get();
        if d <= 1 {
            self.depth.set(0);
            self.owner.store(0, Ordering::Release);
        } else {
            self.depth.set(d - 1);
        }
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================== Handles ==============================

pub enum Handle {
    File {
        fd: i32,
    },
    Find {
        dir: u64,
        dir_path: String,
        pattern: String,
        first_pending: bool,
    },
    Thread {
        done: *const ExitPacket,
    },
    Event {
        signaled: *const AtomicBool,
    },
    Invalid,
}

/// Обгортка-переносник сирих покажчиків у std::thread.
pub struct SendPtr<T>(pub T);
unsafe impl<T> Send for SendPtr<T> {}

/// Пакет виходу потоку (адреса стабільна — Box).
pub struct ExitPacket {
    pub done: AtomicBool,
    pub code: AtomicU32,
    pub join: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

// ============================== World ==============================

pub struct WorldInner {
    pub registry: Vec<(&'static str, Handler)>,
    pub name_to_id: HashMap<String, u64>,
    pub dyn_names: Vec<String>,
    pub handles: HashMap<u64, Handle>,
    pub next_handle: u64,
    pub heap_sizes: HashMap<usize, usize>,
    pub forever: Vec<u8>,
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub env_value_ptrs: HashMap<String, u64>,
    pub env_block_unicode: u64,
    pub envp_block: u64,
    pub iob_base: u64,
    pub rand_next: u64,
    pub trace: u8,
    pub tls_slots: Vec<u64>,
    pub image: Option<LoadedImage>,
    pub thunks: Option<ThunkPages>,
    pub data: Option<DataPage>,
    pub name_to_thunk: HashMap<String, u64>,
    pub peb_shared: u64,
    pub cmd_line_w: u64,
    pub cmd_line_a: u64,
    /// Точки перестрибування SEH-walker-а крізь наші Rust-кадри
    pub callback_hops: Vec<Hop>,
}

/// Записаний контекст продовження для обходу наших кадрів при розгортанні.
#[derive(Clone, Copy)]
pub struct Hop {
    pub rsp: u64,
    pub rip: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

pub struct LoadedImage {
    pub base: u64,
    pub info: PeInfo,
    pub rfs: Vec<RuntimeFunction>,
}

pub struct World {
    inner: std::cell::UnsafeCell<WorldInner>,
    pub gate: Gate,
}

unsafe impl Sync for World {}
unsafe impl Send for World {}

pub static WORLD: std::sync::OnceLock<World> = std::sync::OnceLock::new();

impl World {
    fn new() -> World {
        World {
            inner: std::cell::UnsafeCell::new(WorldInner {
        registry: Vec::new(),
        name_to_id: HashMap::new(),
        dyn_names: Vec::new(),
        handles: HashMap::new(),
        next_handle: 0x100,
        heap_sizes: HashMap::new(),
        forever: Vec::new(),
        argv: Vec::new(),
        env: Vec::new(),
        env_value_ptrs: HashMap::new(),
        env_block_unicode: 0,
        envp_block: 0,
        iob_base: 0,
        rand_next: 0x12345678,
        trace: 0,
        tls_slots: Vec::new(),
        image: None,
        thunks: None,
        data: None,
        name_to_thunk: HashMap::new(),
        peb_shared: 0,
                cmd_line_w: 0,
                cmd_line_a: 0,
                callback_hops: Vec::new(),
            }),
            gate: Gate::new(),
        }
    }
}

impl World {
    pub fn get() -> &'static World {
        WORLD.get_or_init(World::new)
    }

    /// Ініціалізація реєстру (один раз).
    pub fn init_registry(&self) {
        self.gate.enter();
        unsafe {
            let w = &mut *self.inner.get();
            if !w.registry.is_empty() {
                self.gate.leave();
                return;
            }
            let mut reg: Vec<(&'static str, Handler)> = Vec::new();
            reg.extend(super::crt::registry_full());
            reg.extend(super::api::registry_kernel32());
            reg.extend(super::api_ext::registry_ext());
            for (i, (name, _)) in reg.iter().enumerate() {
                w.name_to_id
                    .entry(name.to_string())
                    .or_insert(REG_BASE + i as u64);
            }
            w.registry = reg;
        }
        self.gate.leave();
    }

    pub fn init_thunks(&self) -> Result<(), String> {
        self.gate.enter();
        let r = unsafe {
            let w = &mut *self.inner.get();
            if w.thunks.is_none() {
                w.thunks = Some(ThunkPages::new()?);
            }
            if w.data.is_none() {
                w.data = Some(DataPage::new()?);
            }
            Ok(())
        };
        self.gate.leave();
        r
    }

    pub fn set_runtime(&self, argv: Vec<String>, env: Vec<(String, String)>, trace: u8) {
        self.gate.enter();
        unsafe {
            let w = &mut *self.inner.get();
            w.argv = argv;
            w.env = env;
            w.trace = trace;
        }
        self.gate.leave();
    }

    pub fn set_image(&self, base: u64, info: PeInfo, rfs: Vec<RuntimeFunction>) {
        self.gate.enter();
        unsafe {
            (*self.inner.get()).image = Some(LoadedImage { base, info, rfs });
        }
        self.gate.leave();
    }

    pub fn set_peb_shared(&self, peb: u64) {
        self.gate.enter();
        unsafe { (*self.inner.get()).peb_shared = peb };
        self.gate.leave();
    }

    pub fn set_cmd_lines(&self, w_ptr: u64, a_ptr: u64) {
        self.gate.enter();
        unsafe {
            let w = &mut *self.inner.get();
            w.cmd_line_w = w_ptr;
            w.cmd_line_a = a_ptr;
        }
        self.gate.leave();
    }

    /// id для символу (невідомий → реєструється як unknown).
    pub fn ensure_symbol(&self, _dll: &str, name: &str) -> u64 {
        self.gate.enter();
        let id = unsafe {
            let w = &mut *self.inner.get();
            if let Some(&id) = w.name_to_id.get(name) {
                id
            } else {
                let id = ID_UNKNOWN_BASE + w.dyn_names.len() as u64;
                w.dyn_names.push(name.to_string());
                w.name_to_id.insert(name.to_string(), id);
                id
            }
        };
        self.gate.leave();
        id
    }

    /// Тunk для GetProcAddress: мемоізований, пізнє виділення.
    pub fn proc_address_thunk(&self, name: &str) -> Result<u64, String> {
        self.gate.enter();
        let r = (|| {
            unsafe {
                let w = &mut *self.inner.get();
                if let Some(&t) = w.name_to_thunk.get(name) {
                    return Ok(t);
                }
                let id = w.name_to_id.get(name).copied().unwrap_or_else(|| {
                    let id = ID_UNKNOWN_BASE + w.dyn_names.len() as u64;
                    w.dyn_names.push(name.to_string());
                    w.name_to_id.insert(name.to_string(), id);
                    id
                });
                let bridge = super::runtime::bridge_addr();
                let t = w
                    .thunks
                    .as_mut()
                    .ok_or("thunks не ініціалізовані")?
                    .emit(id, bridge)?;
                w.name_to_thunk.insert(name.to_string(), t);
                Ok(t)
            }
        })();
        self.gate.leave();
        r
    }

    /// Емісія тunk-а для id (IAT-патчинг).
    pub fn thunk_emit(&self, id: u64, bridge: u64) -> Result<u64, String> {
        self.gate.enter();
        let r = unsafe {
            let w = &mut *self.inner.get();
            w.thunks
                .as_mut()
                .ok_or("thunks не ініціалізовані")?
                .emit(id, bridge)
        };
        self.gate.leave();
        r
    }

    /// Комірка для імпортованого ДАНОГО символу (з фішками CRT).
    pub fn data_cell_for(&self, name: &str) -> Result<u64, String> {
        // от rmânia значення поза gate, потім комірка
        let (envp, envu) = self.with_inner(|w| (w.envp_block, w.env_block_unicode));
        let init: Vec<u8> = match name {
            "_commode" => 0u64.to_le_bytes().to_vec(),
            "_fmode" => 0x4000u32.to_le_bytes().to_vec(),
            "__mb_cur_max" => 1u32.to_le_bytes().to_vec(),
            "_timezone" => 0i64.to_le_bytes().to_vec(),
            "_daylight" => 1i32.to_le_bytes().to_vec(),
            "_dstbias" => 0i32.to_le_bytes().to_vec(),
            "environ" | "_environ" | "__initenv" => envp.to_le_bytes().to_vec(),
            "_wenviron" => envu.to_le_bytes().to_vec(),
            _ => Vec::new(),
        };
        let size = match name {
            // CRT: stdin/stdout/stderr = (&_iob)[k] = base + k*48 → 3 слоти
            "_iob" => 3 * 80,
            "_timezone" => 8,
            "environ" | "_environ" | "__initenv" | "_wenviron" => 16,
            _ => 16,
        };
        self.gate.enter();
        let r = unsafe {
            let w = &mut *self.inner.get();
            let cell = w
                .data
                .as_mut()
                .ok_or("data page не ініціалізована")?
                .cell(size, &init)?;
            if name == "_iob" {
                w.iob_base = cell;
            }
            Ok(cell)
        };
        self.gate.leave();
        r
    }

    pub fn alloc_forever(&self, n: u32) -> u64 {
        self.gate.enter();
        let p = unsafe {
            let w = &mut *self.inner.get();
            let off = w.forever.len();
            w.forever.resize(off + n as usize + 16, 0);
            w.forever.as_mut_ptr().add(off) as u64
        };
        self.gate.leave();
        p
    }

    pub fn heap_alloc(&self, n: u32) -> u64 {
        let size = (n as usize).max(1);
        let layout = std::alloc::Layout::from_size_align(size, 16).unwrap();
        let p = unsafe { std::alloc::alloc(layout) } as usize;
        if p == 0 {
            return 0;
        }
        self.gate.enter();
        unsafe {
            (*self.inner.get()).heap_sizes.insert(p, size);
        }
        self.gate.leave();
        p as u64
    }

    pub fn heap_free(&self, p: u64) {
        if p == 0 {
            return;
        }
        let p = p as usize;
        self.gate.enter();
        let size = unsafe { (*self.inner.get()).heap_sizes.remove(&p) };
        self.gate.leave();
        if let Some(size) = size {
            let layout = std::alloc::Layout::from_size_align(size, 16).unwrap();
            unsafe { std::alloc::dealloc(p as *mut u8, layout) };
        }
    }

    pub fn heap_realloc(&self, old: u64, n: u32) -> u64 {
        if old == 0 {
            return self.heap_alloc(n);
        }
        let size = {
            self.gate.enter();
            let s = unsafe { (*self.inner.get()).heap_sizes.get(&(old as usize)).copied() };
            self.gate.leave();
            s
        };
        let old_size = size.unwrap_or(0);
        if n == 0 {
            self.heap_free(old);
            return 0;
        }
        let new = self.heap_alloc(n);
        if new != 0 && old_size > 0 {
            let copy = old_size.min(n as usize);
            unsafe { std::ptr::copy(old as *const u8, new as *mut u8, copy) };
            self.heap_free(old);
        }
        new
    }

    pub fn insert_handle(&self, h: Handle) -> u64 {
        self.gate.enter();
        let id = unsafe {
            let w = &mut *self.inner.get();
            let id = w.next_handle;
            w.next_handle += 1;
            w.handles.insert(id, h);
            id
        };
        self.gate.leave();
        id
    }

    pub fn with_handle<R>(&self, id: u64, f: impl FnOnce(&mut Handle) -> R) -> Option<R> {
        self.gate.enter();
        let r = unsafe { (*self.inner.get()).handles.get_mut(&id).map(f) };
        self.gate.leave();
        r
    }

    pub fn remove_handle(&self, id: u64) -> Option<Handle> {
        self.gate.enter();
        let r = unsafe { (*self.inner.get()).handles.remove(&id) };
        self.gate.leave();
        r
    }

    pub fn with_inner<R>(&self, f: impl FnOnce(&WorldInner) -> R) -> R {
        self.gate.enter();
        let r = unsafe { f(&*self.inner.get()) };
        self.gate.leave();
        r
    }

    pub fn with_inner_mut<R>(&self, f: impl FnOnce(&mut WorldInner) -> R) -> R {
        self.gate.enter();
        let r = unsafe { f(&mut *self.inner.get()) };
        self.gate.leave();
        r
    }

    pub fn trace(&self) -> u8 {
        self.with_inner(|w| w.trace)
    }

    /// Реєструє hop: SEH-walker перестрибує крізь наші кадри до (rip,rsp).
    pub fn push_hop(&self, hop: Hop) {
        self.with_inner_mut(|w| w.callback_hops.push(hop));
    }

    /// Забирає найновіший hop (walker перетнув наш шар; кадри покинуті).
    pub fn pop_hop(&self) -> Option<Hop> {
        self.with_inner_mut(|w| w.callback_hops.pop())
    }

    /// Юнікодний/ANSI блоки середовища + getenv-покажчики.
    pub fn build_env_blocks(&self) {
        self.gate.enter();
        unsafe {
            let w = &mut *self.inner.get();
            let mut u: Vec<u16> = Vec::new();
            let mut a: Vec<u8> = Vec::new();
            for (k, v) in &w.env {
                let kv = format!("{k}={v}");
                u.extend(kv.encode_utf16());
                u.push(0);
                a.extend_from_slice(kv.as_bytes());
                a.push(0);
            }
            u.push(0);
            a.push(0);
            let ansi = w.forever.len();
            w.forever.extend_from_slice(&a);
            let ansi_base = w.forever.as_mut_ptr() as u64 + ansi as u64;
            let mut starts: Vec<u64> = Vec::new();
            let mut i = 0usize;
            while i < a.len() {
                let start = i;
                while i < a.len() && a[i] != 0 {
                    i += 1;
                }
                starts.push(ansi_base + start as u64);
                i += 1;
            }
            let arr_off = w.forever.len();
            for p in &starts {
                w.forever.extend_from_slice(&p.to_le_bytes());
            }
            w.forever.extend_from_slice(&0u64.to_le_bytes());
            w.envp_block = w.forever.as_mut_ptr() as u64 + arr_off as u64;
            let u_off = w.forever.len();
            let mut ub = Vec::new();
            for ch in &u {
                ub.extend_from_slice(&ch.to_le_bytes());
            }
            w.forever.extend_from_slice(&ub);
            w.env_block_unicode = w.forever.as_mut_ptr() as u64 + u_off as u64;
            for (k, v) in &w.env {
                let v_off = w.forever.len();
                w.forever.extend_from_slice(v.as_bytes());
                w.forever.push(0);
                w.env_value_ptrs
                    .insert(k.clone(), w.forever.as_mut_ptr() as u64 + v_off as u64);
            }
        }
        self.gate.leave();
    }
}

// ============================== Диспетчер ==============================

#[no_mangle]
pub extern "C" fn winpe_dispatch_c(
    id: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    entry_rsp: u64,
    caller_rip: u64,
) -> u64 {
    let w = World::get();
    let trace = w.trace();
    let name: Option<String> = w.with_inner(|inner| {
        if let Some(i) = id.checked_sub(REG_BASE) {
            inner
                .registry
                .get(i as usize)
                .map(|(n, _)| n.to_string())
        } else {
            None
        }
    });
    if trace >= 1 {
        if let Some(n) = &name {
            eprintln!("[winpe] {n}({a0:#x},{a1:#x},{a2:#x},{a3:#x})");
        }
    }
    let r = if let Some(i) = id.checked_sub(REG_BASE) {
        let h = w.with_inner(|inner| inner.registry.get(i as usize).map(|(_, h)| *h));
        match h {
            Some(h) => unsafe { h(a0, a1, a2, a3, a4, a5, entry_rsp, caller_rip) },
            None => 0,
        }
    } else if let Some(i) = id.checked_sub(ID_UNKNOWN_BASE) {
        let n = w.with_inner(|inner| {
            inner
                .dyn_names
                .get(i as usize)
                .cloned()
                .unwrap_or_else(|| "?".into())
        });
        if trace >= 1 {
            eprintln!("[winpe:UNIMPL] {n}({a0:#x},{a1:#x},{a2:#x},{a3:#x})");
        }
        0
    } else {
        0
    };
    if trace >= 2 {
        if let Some(n) = &name {
            eprintln!("[winpe] {n} → {r:#x}");
        }
    }
    r
}

#[no_mangle]
pub extern "C" fn winpe_exit_impl(code: u64) -> ! {
    std::process::exit((code as u32 & 0xFF) as i32)
}

// ============================== kernel32 ==============================

const INVALID_HANDLE: u64 = 0xFFFF_FFFF_FFFF_FFFF;
const GENERIC_READ: u64 = 0x8000_0000;
const GENERIC_WRITE: u64 = 0x4000_0000;

fn utf16_to_path(p: u64) -> String {
    unsafe { cstr_w(p) }
}

/// Windows-шлях → хостовий: C:\x → /x, зворотні слеші → прямі.
fn win_to_host(p: &str) -> String {
    let mut s = p.replace('\\', "/");
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        s = s[2..].to_string();
    }
    if s.is_empty() {
        s = ".".into();
    }
    s
}

fn host_to_win(p: &str) -> String {
    format!("C:\\{}", p.trim_start_matches('/'))
}

unsafe fn h_GetStdHandle(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    match a0 as i64 {
        -10 => 0xF001,
        -11 => 0xF002,
        -12 => 0xF003,
        _ => 0xF002,
    }
}

unsafe fn h_CreateFileW(a0: u64, a1: u64, _2: u64, _3: u64, a4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    h_create_file(utf16_to_path(a0), a1, a4, w)
}

unsafe fn h_CreateFileA(a0: u64, a1: u64, _2: u64, _3: u64, a4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    h_create_file(unsafe { cstr(a0) }, a1, a4, w)
}

fn h_create_file(path: String, access: u64, disp: u64, _w: &World) -> u64 {
    
    let host = win_to_host(&path);
    let mut opts = std::fs::OpenOptions::new();
    let rd = access & GENERIC_READ != 0;
    let wr = access & GENERIC_WRITE != 0;
    match (rd, wr) {
        (true, true) => {
            opts.read(true).write(true);
        }
        (true, false) => {
            opts.read(true);
        }
        (false, true) => {
            opts.write(true).create(true);
        }
        (false, false) => {
            opts.read(true);
        }
    }
    match disp {
        1 => {
            opts.create_new(true);
        }
        2 => {
            opts.create(true).truncate(true);
        }
        4 => {
            opts.create(true);
        }
        5 => {
            opts.truncate(true);
        }
        _ => {}
    }
    match opts.open(&host) {
        Ok(f) => {
            use std::os::unix::io::IntoRawFd;
            let fd = f.into_raw_fd();
            _w.insert_handle(Handle::File { fd })
        }
        Err(e) => {
            eprintln!("[winpe] CreateFile({host}) → {e}");
            INVALID_HANDLE
        }
    }
}

unsafe fn h_ReadFile(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    let n = unsafe { libc::read(fd, a1 as *mut _, a2 as usize) };
    if n < 0 {
        return 0;
    }
    if a3 != 0 {
        unsafe { write_u64(a3, n as u64) };
    }
    1
}

unsafe fn h_WriteFile(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    let n = unsafe { libc::write(fd, a1 as *const _, a2 as usize) };
    if n < 0 {
        return 0;
    }
    if a3 != 0 {
        unsafe { write_u64(a3, n as u64) };
    }
    1
}

fn file_fd(w: &World, h: u64) -> i32 {
    match h {
        0xF001 => 0,
        0xF002 => 1,
        0xF003 => 2,
        _ => w
            .with_handle(h, |hd| match hd {
                Handle::File { fd } => *fd,
                _ => -1,
            })
            .unwrap_or(-1),
    }
}

unsafe fn h_CloseHandle(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    match w.remove_handle(a0) {
        Some(Handle::File { fd }) => {
            unsafe { libc::close(fd) };
            1
        }
        Some(Handle::Find { dir, .. }) => {
            unsafe { libc::closedir(dir as *mut _) };
            1
        }
        _ => 1, // Thread/Event — просто забуваємо
    }
}

unsafe fn h_SetFilePointer(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    let mut off = a1 as i64;
    if a2 != 0 {
        off += (unsafe { read_u64(a2) } as i64) << 32;
    }
    let whence = match a3 {
        0 => libc::SEEK_SET,
        1 => libc::SEEK_CUR,
        _ => libc::SEEK_END,
    };
    let r = unsafe { libc::lseek(fd, off, whence) };
    if r < 0 {
        INVALID_HANDLE
    } else {
        r as u64
    }
}

unsafe fn h_GetFileSize(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return INVALID_HANDLE;
    }
    let sz = st.st_size as u64;
    if a1 != 0 {
        unsafe { write_u64(a1, sz >> 32) };
    }
    sz & 0xFFFFFFFF
}

unsafe fn h_GetFileSizeEx(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return 0;
    }
    unsafe { write_u64(a1, st.st_size as u64) };
    1
}

unsafe fn h_GetFileType(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    match a0 {
        0xF001..=0xF003 => 2,
        _ => {
            let fd = file_fd(w, a0);
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd, &mut st) } == 0
                && (st.st_mode & libc::S_IFMT) == libc::S_IFCHR
            {
                2
            } else {
                1
            }
        }
    }
}

unsafe fn h_FlushFileBuffers(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = file_fd(w, a0);
    (unsafe { libc::fsync(fd) == 0 }) as u64
}

// ---- директорії / пошук ----

/// Заповнює WIN32_FIND_DATAW (cFileName на зсуві 44 — ВИВЧЕНО!).
unsafe fn fill_find_data(data: u64, name: &str, is_dir: bool, size: u64) {
    unsafe {
        write_u32(data, if is_dir { 0x10 } else { 0x80 });
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let ft = ((t + 11644473600) * 10_000_000) as u64;
        write_u64(data + 0x04, ft);
        write_u64(data + 0x0C, ft);
        write_u64(data + 0x14, ft);
        write_u32(data + 0x1C, (size >> 32) as u32);
        write_u32(data + 0x20, (size & 0xFFFFFFFF) as u32);
        write_u32(data + 0x24, 0);
        write_u32(data + 0x28, 0);
        let mut off = 0u64;
        for ch in name.encode_utf16().take(259) {
            write_u16(data + 0x2C + off * 2, ch);
            off += 1;
        }
        write_u16(data + 0x2C + off * 2, 0);
        write_u16(data + 0x232, 0);
    }
}

fn glob_match(pat: &str, s: &str) -> bool {
    fn inner(p: &[u8], s: &[u8]) -> bool {
        if p.is_empty() {
            return s.is_empty();
        }
        match p[0] {
            b'*' => (0..=s.len()).any(|i| inner(&p[1..], &s[i..])),
            b'?' => !s.is_empty() && inner(&p[1..], &s[1..]),
            c => !s.is_empty() && s[0] == c && inner(&p[1..], &s[1..]),
        }
    }
    inner(pat.as_bytes(), s.as_bytes())
}

/// A/W-сніффінг (ВИВЧЕНО): буфер може бути ANSI, навіть для *W-імпорту.
fn sniff_wide(p: u64) -> bool {
    if p == 0 {
        return false;
    }
    let b0 = unsafe { super::crt::read_u8(p) };
    let b1 = unsafe { super::crt::read_u8(p + 1) };
    b1 == 0 && b0 != 0
}

unsafe fn h_FindFirstFileW(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let pattern = if sniff_wide(a0) {
        utf16_to_path(a0)
    } else {
        unsafe { cstr(a0) }
    };
    let r = find_first(w, pattern.clone(), a1);
    if std::env::var("POLER_WIN_TRACE").is_ok() {
        eprintln!("[winpe:find] патерн {pattern:?} → {r:#x}");
    }
    r
}

unsafe fn h_FindFirstFileA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    find_first(w, unsafe { cstr(a0) }, a1)
}

fn find_first(w: &World, pattern: String, data: u64) -> u64 {
    let host_pat = win_to_host(&pattern);
    let dir = match host_pat.rfind('/') {
        Some(i) => host_pat[..i].to_string(),
        None => ".".to_string(),
    };
    let mask = match host_pat.rfind('/') {
        Some(i) => host_pat[i + 1..].to_string(),
        None => host_pat.clone(),
    };
    let d = unsafe { libc::opendir(std::ffi::CString::new(dir.clone()).unwrap().as_ptr()) };
    if d.is_null() {
        return INVALID_HANDLE;
    }
    let concrete = !mask.contains('*') && !mask.contains('?');
    let h = w.insert_handle(Handle::Find {
        dir: d as u64,
        dir_path: dir,
        pattern: mask.clone(),
        first_pending: !concrete, // ".." лише для шаблонів!
    });
    // Конкретне ім'я (без *?) → "." і ".." НЕ повертаємо (Windows-семантика);
    // тобто одразу шукаємо перший РЕАЛЬНИЙ збіг.
    if concrete {
        let r = unsafe { h_FindNextFileW(h, data, 0, 0, 0, 0, 0, 0, w) };
        if r == 0 {
            let _ = w.remove_handle(h);
            return INVALID_HANDLE;
        }
    } else {
        // шаблон: "." першим, ".." віддасть FindNext
        unsafe { fill_find_data(data, ".", true, 0) };
    }
    h
}

unsafe fn h_FindNextFileW(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let mut pending_dotdot = false;
    w.with_handle(a0, |hd| {
        if let Handle::Find {
            first_pending, ..
        } = hd
        {
            if *first_pending {
                *first_pending = false;
                pending_dotdot = true;
            }
        }
    });
    if pending_dotdot {
        unsafe { fill_find_data(a1, "..", true, 0) };
        return 1;
    }
    let snap = w
        .with_handle(a0, |hd| {
            if let Handle::Find {
                dir,
                dir_path,
                pattern,
                ..
            } = hd
            {
                Some((*dir, dir_path.clone(), pattern.clone()))
            } else {
                None
            }
        })
        .flatten();
    let Some((dir, dir_path, pattern)) = snap else { return 0 };
    unsafe {
        loop {
            let ent = libc::readdir(dir as *mut _);
            if ent.is_null() {
                return 0;
            }
            let name = std::ffi::CStr::from_ptr((*ent).d_name.as_ptr())
                .to_string_lossy()
                .into_owned();
            if name == "." || name == ".." {
                continue;
            }
            if glob_match(&pattern, &name) {
                let is_dir = (*ent).d_type == libc::DT_DIR;
                let size = if is_dir {
                    0
                } else {
                    std::fs::metadata(format!("{dir_path}/{name}"))
                        .map(|m| m.len())
                        .unwrap_or(0)
                };
                if std::env::var("POLER_WIN_TRACE").is_ok() {
                    eprintln!("[winpe:find] збіг: {name:?} is_dir={is_dir}");
                }
                fill_find_data(a1, &name, is_dir, size);
                return 1;
            }
        }
    }
}

unsafe fn h_DeleteFileW(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = win_to_host(&if sniff_wide(a0) {
        utf16_to_path(a0)
    } else {
        unsafe { cstr(a0) }
    });
    std::fs::remove_file(p).is_ok() as u64
}

unsafe fn h_MoveFileW(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::fs::rename(
        win_to_host(&utf16_to_path(a0)),
        win_to_host(&utf16_to_path(a1)),
    )
    .is_ok() as u64
}

unsafe fn h_CreateDirectoryW(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = if sniff_wide(a0) {
        utf16_to_path(a0)
    } else {
        unsafe { cstr(a0) }
    };
    std::fs::create_dir_all(win_to_host(&p)).is_ok() as u64
}

unsafe fn h_RemoveDirectoryW(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::fs::remove_dir(win_to_host(&utf16_to_path(a0))).is_ok() as u64
}

unsafe fn h_GetCurrentDirectoryA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    // ФІКС: (len, buffer) — саме так, не навпаки!
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".into());
    let win = host_to_win(&cwd);
    let bytes = win.as_bytes();
    if a0 as usize > bytes.len() && a1 != 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), a1 as *mut u8, bytes.len());
            write_u8(a1 + bytes.len() as u64, 0);
        }
        return bytes.len() as u64;
    }
    (bytes.len() + 1) as u64
}

unsafe fn h_GetCurrentDirectoryW(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".into());
    let win = host_to_win(&cwd);
    let u: Vec<u16> = win.encode_utf16().collect();
    if a0 as usize > u.len() && a1 != 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(u.as_ptr(), a1 as *mut u16, u.len());
            write_u16(a1 + u.len() as u64 * 2, 0);
        }
        return u.len() as u64;
    }
    (u.len() + 1) as u64
}

unsafe fn h_SetCurrentDirectoryW(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = if sniff_wide(a0) {
        utf16_to_path(a0)
    } else {
        unsafe { cstr(a0) }
    };
    std::env::set_current_dir(win_to_host(&p)).is_ok() as u64
}

unsafe fn h_GetFileAttributesW(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = if sniff_wide(a0) {
        utf16_to_path(a0)
    } else {
        unsafe { cstr(a0) }
    };
    attr_of(&p)
}

unsafe fn h_GetFileAttributesA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    attr_of(&unsafe { cstr(a0) })
}

fn attr_of(p: &str) -> u64 {
    match std::fs::metadata(win_to_host(p)) {
        Ok(m) => {
            if m.is_dir() {
                0x10
            } else {
                0x80
            }
        }
        Err(_) => INVALID_HANDLE,
    }
}

unsafe fn h_GetCommandLineW(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner(|inner| inner.cmd_line_w)
}

unsafe fn h_GetCommandLineA(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner(|inner| inner.cmd_line_a)
}

unsafe fn h_GetModuleFileNameW(_a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let u: Vec<u16> = "C:\\7za.exe".encode_utf16().collect();
    let n = u.len().min(a2 as usize).saturating_sub(0);
    unsafe {
        std::ptr::copy_nonoverlapping(u.as_ptr(), a1 as *mut u16, n);
        if n > 0 {
            write_u16(a1 + (n as u64 - 1) * 2, 0);
        }
    }
    n as u64
}

unsafe fn h_GetModuleHandleA(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0x0040_0000
}

unsafe fn h_GetProcAddress(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // Ординументи GetProcAddress: (HMODULE, LPCSTR) або (HMODULE, MAKEINTRESOURCE)
    if a1 & 0xFFFF_0000_0000_0000 == 0 && a1 < 0x10000 {
        return 0; // ординал — не підтримуємо
    }
    let name = unsafe { cstr(a1) };
    match w.proc_address_thunk(&name) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[winpe] GetProcAddress({name}): {e}");
            0
        }
    }
}

unsafe fn h_LoadLibraryA(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0x0040_0000
}

unsafe fn h_FreeLibrary(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    1
}

// ---- купка ----

unsafe fn h_HeapAlloc(_a0: u64, _1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_alloc(a2 as u32)
}

unsafe fn h_HeapReAlloc(_a0: u64, _1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_realloc(a2, a3 as u32)
}

unsafe fn h_HeapFree(_a0: u64, _1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_free(a2);
    1
}

unsafe fn h_GetProcessHeap(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0xAA00
}

unsafe fn h_LocalAlloc(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_alloc(a1 as u32)
}

unsafe fn h_LocalFree(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_free(a0);
    0
}

unsafe fn h_GlobalAlloc(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_alloc(a1 as u32)
}

unsafe fn h_GlobalFree(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_free(a0);
    0
}

unsafe fn h_VirtualAlloc(_a0: u64, a1: u64, _a2: u64, _a3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let size = a1.div_ceil(0x1000) * 0x1000;
    let p = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED {
        0
    } else {
        p as u64
    }
}

unsafe fn h_VirtualFree(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a2 & 0x8000 != 0 {
        unsafe { libc::munmap(a0 as *mut _, a1 as usize) };
    }
    1
}

unsafe fn h_VirtualProtect(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let prot = match a2 {
        0x10 | 0x20 | 0x40 | 0x80 => libc::PROT_READ | libc::PROT_EXEC,
        0x04 => libc::PROT_READ | libc::PROT_WRITE,
        0x02 => libc::PROT_READ,
        _ => libc::PROT_READ | libc::PROT_WRITE,
    };
    let r = unsafe { libc::mprotect(a0 as *mut _, a1 as usize, prot) };
    if a3 != 0 {
        unsafe { write_u64(a3, 0x04) };
    }
    (r == 0) as u64
}

// ---- час ----

unsafe fn h_QueryPerformanceCounter(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    let ns = ts.tv_sec as i128 * 1_000_000_000 + ts.tv_nsec as i128;
    unsafe { write_u64(a0, (ns & 0x7FFF_FFFF_FFFF_FFFF) as u64) };
    1
}

unsafe fn h_QueryPerformanceFrequency(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { write_u64(a0, 1_000_000_000) };
    1
}

unsafe fn h_GetSystemTimeAsFileTime(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    if a0 != 0 {
        unsafe { write_u64(a0, ((t + 11644473600) * 10_000_000) as u64) };
    }
    0
}

unsafe fn h_GetTickCount(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ((ts.tv_sec as u64) * 1000 + (ts.tv_nsec as u64) / 1_000_000) & 0xFFFFFFFF
}

unsafe fn h_Sleep(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ts = libc::timespec {
        tv_sec: (a0 / 1000) as _,
        tv_nsec: ((a0 % 1000) * 1_000_000) as _,
    };
    unsafe { libc::nanosleep(&ts, std::ptr::null_mut()) };
    0
}

unsafe fn h_GetSystemInfo(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ncpu = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2) as u64;
    unsafe {
        write_u32(a0, 0);
        write_u64(a0 + 0x08, 0x10000);
        write_u64(a0 + 0x10, 0x7FFF_FFFF_FFFF);
        write_u64(a0 + 0x18, (1u64 << ncpu) - 1);
        write_u32(a0 + 0x20, ncpu as u32);
        write_u32(a0 + 0x24, 8664);
        write_u32(a0 + 0x28, 0x10000);
        write_u16(a0 + 0x2C, 6);
        write_u16(a0 + 0x2E, 0x0F00);
    }
    0
}

unsafe fn h_GlobalMemoryStatusEx(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
    let total = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) } as u64 * page;
    unsafe {
        write_u32(a0, 0x40);
        write_u32(a0 + 4, 50);
        write_u64(a0 + 8, total);
        write_u64(a0 + 16, total / 2);
        write_u64(a0 + 24, total);
        write_u64(a0 + 32, total / 2);
        write_u64(a0 + 40, 1 << 62);
        write_u64(a0 + 48, 1 << 61);
        write_u64(a0 + 56, 0);
    }
    1
}

// ---- конвертери (ФІКС: усі 6 параметрів у правильному порядку!) ----

unsafe fn h_MultiByteToWideChar(_cp: u64, _flags: u64, a2: u64, a3: u64, a4: u64, a5: u64, _e: u64, _c: u64) -> u64 {
    let src: Vec<u8> = if a3 == 0xFFFF_FFFF_FFFF_FFFF || a3 == (-1i64) as u64 {
        unsafe { cstr(a2).into_bytes() }
    } else {
        unsafe { std::slice::from_raw_parts(a2 as *const u8, a3 as usize).to_vec() }
    };
    let s = String::from_utf8_lossy(&src);
    let u: Vec<u16> = s.encode_utf16().collect();
    if a4 != 0 && a5 != 0 {
        let n = u.len().min(a5 as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(u.as_ptr(), a4 as *mut u16, n);
        }
        return n as u64;
    }
    (u.len() + 1) as u64
}

unsafe fn h_WideCharToMultiByte(_cp: u64, _flags: u64, a2: u64, a3: u64, a4: u64, a5: u64, _e: u64, _c: u64) -> u64 {
    let ws = if a3 == 0xFFFF_FFFF_FFFF_FFFF || a3 == (-1i64) as u64 {
        unsafe { cstr_w(a2) }
    } else {
        let mut v = Vec::new();
        for i in 0..a3 {
            v.push(unsafe { super::crt::read_u16(a2 + i * 2) });
        }
        String::from_utf16_lossy(&v)
    };
    let b = ws.as_bytes();
    if a4 != 0 && a5 != 0 {
        let n = b.len().min(a5 as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(b.as_ptr(), a4 as *mut u8, n);
        }
        return n as u64;
    }
    (b.len() + 1) as u64
}

// ---- потоки/синхронізація ----

thread_local! {
    static THREAD_EXIT_CODE: Cell<u32> = const { Cell::new(0) };
}

/// Запуск PE-потоку: власний TEB + GS + скретч-стек + обгортка виходу.
pub fn spawn_pe_thread(start: u64, param: u64) -> u64 {
    let teb = match super::runtime::thread_teb_new() {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let packet = Box::into_raw(Box::new(ExitPacket {
        done: AtomicBool::new(false),
        code: AtomicU32::new(0),
        join: std::sync::Mutex::new(None),
    }));
    let _pkt = packet;
    let pkt = packet as usize;
    let jh = std::thread::spawn(move || {
        let pkt = pkt as *mut ExitPacket;
        unsafe {
            libc::syscall(libc::SYS_arch_prctl, 0x1001, teb);
        }
        let r = win_call(start, param, 0, 0, 0, 0).unwrap_or(0);
        let code = THREAD_EXIT_CODE.with(|c| c.get());
        unsafe {
            (*pkt).code.store(code.max(r as u32), Ordering::Relaxed);
            (*pkt).done.store(true, Ordering::Release);
            let addr = &(*pkt).done as *const _ as usize;
            libc::syscall(libc::SYS_futex, addr as *const i32, 1, i32::MAX);
        }
    });
    let handle = World::get().insert_handle(Handle::Thread { done: packet });
    if let Some(h) = World::get()
        .with_handle(handle, |hd| {
            if let Handle::Thread { done, .. } = hd {
                Some(*done as *const ExitPacket)
            } else {
                None
            }
        })
        .flatten()
    {
        let mut g = unsafe { (*h).join.lock() }.unwrap();
        let _ = g.insert(jh);
    }
    handle
}

unsafe fn h_CreateThread(_a0: u64, _a1: u64, a2: u64, a3: u64, _a4: u64, _a5: u64, _e: u64, _c: u64) -> u64 {
    spawn_pe_thread(a2, a3)
}

unsafe fn h_ExitThread(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    THREAD_EXIT_CODE.with(|c| c.set(a0 as u32));
    // повертаємось: обгортка потоку збереже код і завершиться нормально
    0
}

unsafe fn h_WaitForSingleObject(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // ФІКС borrow: JoinHandle ВИЙМАЄТЬСЯ до очікування (без довгого borrow World)
    let pkt = w
        .with_handle(a0, |hd| {
            if let Handle::Thread { done, .. } = hd {
                Some(*done as *const ExitPacket)
            } else {
                None
            }
        })
        .flatten();
    if let Some(pkt) = pkt {
        let p = unsafe { &*pkt };
        let jh = p.join.lock().unwrap().take();
        if let Some(jh) = jh {
            let _ = jh.join();
        } else {
            while !p.done.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_micros(200));
            }
        }
        return 0;
    }
    let ev = w
        .with_handle(a0, |hd| {
            if let Handle::Event { signaled, .. } = hd {
                Some(*signaled as *const AtomicBool)
            } else {
                None
            }
        })
        .flatten();
    if let Some(sig) = ev {
        let deadline = if a1 == 0xFFFF_FFFF {
            None
        } else {
            Some(std::time::Instant::now() + std::time::Duration::from_millis(a1))
        };
        while !unsafe { &*sig }.load(Ordering::Acquire) {
            if let Some(d) = deadline {
                if std::time::Instant::now() >= d {
                    return 0x102;
                }
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        return 0;
    }
    0
}

unsafe fn h_WaitForMultipleObjects(a0: u64, a1: u64, _a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    for i in 0..a0.min(64) {
        let h = unsafe { read_u64(a1 + i * 8) };
        let _ = h_WaitForSingleObject(h, a3, 0, 0, 0, 0, 0, 0, w);
    }
    0
}

unsafe fn h_CreateEventW(_a0: u64, _a1: u64, a2: u64, _a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let state = Box::into_raw(Box::new(AtomicBool::new(a2 != 0)));
    let sig = unsafe { &*state } as *const AtomicBool;
    w.insert_handle(Handle::Event { signaled: sig })
}

unsafe fn h_SetEvent(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_handle(a0, |hd| {
        if let Handle::Event { signaled, .. } = hd {
            unsafe { &**signaled }.store(true, Ordering::Release);
        }
    });
    1
}

unsafe fn h_ResetEvent(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_handle(a0, |hd| {
        if let Handle::Event { signaled, .. } = hd {
            unsafe { &**signaled }.store(false, Ordering::Release);
        }
    });
    1
}

// ---- критичні секції: u64 @+0 = (tid<<32)|depth, фьютекс на цьому слові ----

unsafe fn h_InitializeCriticalSection(cs: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { write_u64(cs, 0) };
    0
}

unsafe fn h_InitializeCriticalSectionAndSpinCount(cs: u64, _a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { write_u64(cs, 0) };
    1
}

unsafe fn cas_u64(at: u64, expect: u64, new: u64) -> bool {
    let a = unsafe { &*(at as *const std::sync::atomic::AtomicU64) };
    a.compare_exchange(expect, new, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
}

unsafe fn h_EnterCriticalSection(cs: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let tid = (unsafe { libc::syscall(libc::SYS_gettid) } as u64) & 0xFFFFFFFF;
    let word = (cs + 4) as *const i32;
    loop {
        let cur = unsafe { read_u64(cs) };
        let owner = cur >> 32;
        if owner == tid {
            unsafe { write_u64(cs, cur + 1) };
            return 0;
        }
        if owner == 0 {
            if unsafe { cas_u64(cs, cur, (tid << 32) | 1) } {
                return 0;
            }
            continue;
        }
        // Чекаємо, поки owner зміниться
        unsafe {
            let val = owner as i32;
            let ts = libc::timespec { tv_sec: 0, tv_nsec: 50_000_000 /* 50ms */ };
            libc::syscall(libc::SYS_futex, word, 0 /*FUTEX_WAIT*/, val, &ts);
        }
    }
}

unsafe fn h_LeaveCriticalSection(cs: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let tid = unsafe { libc::syscall(libc::SYS_gettid) } as u64;
    let cur = unsafe { read_u64(cs) };
    if cur >> 32 == tid {
        let depth = cur & 0xFFFFFFFF;
        if depth <= 1 {
            unsafe { write_u64(cs, 0) };
            let word = (cs + 4) as *const u64 as *const i32;
            unsafe {
                libc::syscall(libc::SYS_futex, word, 1 /*WAKE*/, i32::MAX);
            }
        } else {
            unsafe { write_u64(cs, cur - 1) };
        }
    }
    0
}

unsafe fn h_TryEnterCriticalSection(cs: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let tid = unsafe { libc::syscall(libc::SYS_gettid) } as u64;
    let cur = unsafe { read_u64(cs) };
    if cur >> 32 == tid {
        unsafe { write_u64(cs, cur + 1) };
        return 1;
    }
    if cur == 0 && unsafe { cas_u64(cs, 0, (tid << 32) | 1) } {
        return 1;
    }
    0
}

unsafe fn h_DeleteCriticalSection(_cs: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

// ---- дрібне ----

thread_local! {
    static LAST_ERROR: Cell<u32> = const { Cell::new(0) };
}

unsafe fn h_SetLastError(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    LAST_ERROR.with(|e| e.set(a0 as u32));
    0
}

unsafe fn h_GetLastError(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    LAST_ERROR.with(|e| e.get()) as u64
}

unsafe fn h_GetCurrentProcessId(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::process::id() as u64
}

unsafe fn h_GetCurrentThreadId(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::syscall(libc::SYS_gettid) }) as u64
}

unsafe fn h_ExitProcess(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::process::exit((a0 as u32 & 0xFF) as i32)
}

unsafe fn h_IsProcessorFeaturePresent(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    1
}

unsafe fn h_GetVersion(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0x0000_000A
}

unsafe fn h_GetVersionExA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        write_u32(a0, 0x9C);
        write_u32(a0 + 4, 10);
        write_u32(a0 + 8, 0);
        write_u32(a0 + 12, 19045);
        write_u32(a0 + 16, 2);
        write_u8(a0 + 0x14, 0);
    }
    1
}

unsafe fn h_GetACP(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    65001
}

unsafe fn h_IsDBCSLeadByte(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h_InterlockedIncrement(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = a0 as *const std::sync::atomic::AtomicU32;
    (unsafe { (*p).fetch_add(1, Ordering::AcqRel) } + 1) as u64
}

unsafe fn h_InterlockedDecrement(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = a0 as *const std::sync::atomic::AtomicU32;
    (unsafe { (*p).fetch_sub(1, Ordering::AcqRel) } - 1) as u64
}

unsafe fn h_InterlockedExchange(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = a0 as *const std::sync::atomic::AtomicU32;
    (unsafe { (*p).swap(a1 as u32, Ordering::AcqRel) }) as u64
}

unsafe fn h_InterlockedCompareExchange(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = a0 as *const std::sync::atomic::AtomicU32;
    unsafe {
        (*p).compare_exchange(a2 as u32, a1 as u32, Ordering::AcqRel, Ordering::Relaxed)
    }
    .map(|v| v as u64)
    .unwrap_or(a2)
}

unsafe fn h_OutputDebugStringA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    eprintln!("[ODB] {}", unsafe { cstr(a0) });
    0
}

unsafe fn h_EncodePointer(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    a0
}

unsafe fn h_DecodePointer(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    a0
}

unsafe fn h_GetTempPathW(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let u: Vec<u16> = "C:\\Temp\\".encode_utf16().collect();
    if a1 != 0 && a0 as usize > u.len() {
        unsafe {
            std::ptr::copy_nonoverlapping(u.as_ptr(), a1 as *mut u16, u.len());
            write_u16(a1 + u.len() as u64 * 2, 0);
        }
    }
    u.len() as u64
}

unsafe fn h_GetDiskFreeSpaceExW(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        write_u64(a1, 10 << 30);
        write_u64(a1 + 8, 50 << 30);
        write_u64(a1 + 16, 10 << 30);
    }
    1
}

unsafe fn h_CharToOemBuffA(_a0: u64, _a1: u64, _a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    1
}

unsafe fn h_lstrlenA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a0 == 0 {
        return 0;
    }
    let mut n = 0u64;
    while read_u8_pub(a0 + n) != 0 && n < (1 << 20) {
        n += 1;
    }
    n
}

fn read_u8_pub(p: u64) -> u8 {
    unsafe { super::crt::read_u8(p) }
}

unsafe fn h_lstrcpyA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a1) };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a0 as *mut u8, s.len());
        write_u8(a0 + s.len() as u64, 0);
    }
    a0
}

unsafe fn h_lstrcmpiA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let a = unsafe { cstr(a0) }.to_lowercase();
    let b = unsafe { cstr(a1) }.to_lowercase();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => u64::MAX,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

unsafe fn h_GetEnvironmentStrings(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner(|inner| inner.envp_block)
}

unsafe fn h_GetEnvironmentStringsW(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner(|inner| inner.env_block_unicode)
}

unsafe fn h_FreeEnvironmentStringsA(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    1
}

unsafe fn h_GetEnvironmentVariableA(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let name = unsafe { cstr(a0) };
    let v = w.with_inner(|inner| {
        inner
            .env
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&name))
            .map(|(_, v)| v.clone())
    });
    let Some(v) = v else { return 0 };
    let b = v.as_bytes();
    if a1 != 0 && a2 != 0 {
        let n = b.len().min(a2.saturating_sub(1) as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(b.as_ptr(), a1 as *mut u8, n);
            write_u8(a1 + n as u64, 0);
        }
    }
    (b.len() + 1) as u64
}

unsafe fn h_TlsAlloc(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner_mut(|inner| {
        inner.tls_slots.push(0);
        inner.tls_slots.len() as u64 - 1
    })
}

thread_local! {
    static TLS_VALUES: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
}

unsafe fn h_TlsGetValue(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    TLS_VALUES.with(|t| t.borrow().get(a0 as usize).copied().unwrap_or(0))
}

unsafe fn h_TlsSetValue(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    TLS_VALUES.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() <= a0 as usize {
            t.resize(a0 as usize + 1, 0);
        }
        t[a0 as usize] = a1;
    });
    1
}

unsafe fn h_DuplicateHandle(_a0: u64, a1: u64, _a2: u64, a3: u64, _a4: u64, _a5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { write_u64(a3, a1) };
    1
}

unsafe fn h_SetHandleCount(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    255
}

unsafe fn h_GetUserDefaultLangID(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0x0409
}

unsafe fn h_GetStringTypeW(_a0: u64, _a1: u64, _a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let n = _a2 as usize;
    for i in 0..n {
        unsafe { write_u16(a3 + i as u64 * 2, 0x0100) };
    }
    1
}

unsafe fn h_LCMapStringW(_a0: u64, _a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, _e: u64, _c: u64) -> u64 {
    if a4 == 0 {
        return a3;
    }
    let n = a3.min(a5);
    unsafe {
        std::ptr::copy(a2 as *const u16, a4 as *mut u16, n as usize);
    }
    n
}

unsafe fn h_GetLocaleInfoW(_a0: u64, _a1: u64, _a2: u64, a3: u64, _a4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a3 != 0 {
        unsafe { write_u16(a3, 0) };
        1
    } else {
        2
    }
}

unsafe fn h_RaiseException(a0: u64, _a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, entry_rsp: u64, caller_rip: u64, w: &World) -> u64 {
    if a0 == 0xE06D7363 && a2 >= 3 {
        let info = unsafe { read_u64(a3 + 8) };
        let obj = unsafe { read_u64(a3 + 16) };
        let saved_rbp = unsafe { read_u64(entry_rsp - 8) };
        let saved_rdi = unsafe { read_u64(entry_rsp - 16) };
        let saved_rsi = unsafe { read_u64(entry_rsp - 24) };
        return super::seh::cxx_throw(obj, info, w, caller_rip, entry_rsp, saved_rbp, saved_rdi, saved_rsi);
    }
    eprintln!("[winpe] RaiseException({a0:#x}) — не C++, пропускаємо");
    0
}

unsafe fn h_TerminateProcess(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::process::exit((a1 as u32 & 0xFF) as i32)
}

unsafe fn h_GetProcessAffinityMask(_a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        write_u64(a1, 0xF);
        write_u64(a2, 0xF);
    }
    1
}

unsafe fn h_IsWow64Process(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { write_u64(a1, 0) };
    1
}

// ============================== Реєстр kernel32 ==============================

macro_rules! wh {
    ($f:ident) => {{
        unsafe fn wrap(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, e: u64, c: u64) -> u64 {
            unsafe { $f(a0, a1, a2, a3, a4, a5, e, c, World::get()) }
        }
        wrap as Handler
    }};
}
macro_rules! nh {
    ($f:ident) => {
        $f as Handler
    };
}

pub fn registry_kernel32() -> Vec<(&'static str, Handler)> {
    vec![
        ("GetStdHandle", nh!(h_GetStdHandle)),
        ("CreateFileW", wh!(h_CreateFileW)),
        ("CreateFileA", wh!(h_CreateFileA)),
        ("ReadFile", wh!(h_ReadFile)),
        ("WriteFile", wh!(h_WriteFile)),
        ("CloseHandle", wh!(h_CloseHandle)),
        ("SetFilePointer", wh!(h_SetFilePointer)),
        ("GetFileSize", wh!(h_GetFileSize)),
        ("GetFileSizeEx", wh!(h_GetFileSizeEx)),
        ("GetFileType", wh!(h_GetFileType)),
        ("FlushFileBuffers", wh!(h_FlushFileBuffers)),
        ("FindFirstFileW", wh!(h_FindFirstFileW)),
        ("FindFirstFileA", wh!(h_FindFirstFileA)),
        ("FindNextFileW", wh!(h_FindNextFileW)),
        ("FindNextFileA", wh!(h_FindNextFileW)),
        ("FindClose", wh!(h_CloseHandle)),
        ("DeleteFileW", nh!(h_DeleteFileW)),
        ("DeleteFileA", nh!(h_DeleteFileW)),
        ("MoveFileW", nh!(h_MoveFileW)),
        ("MoveFileA", nh!(h_MoveFileW)),
        ("CreateDirectoryW", nh!(h_CreateDirectoryW)),
        ("CreateDirectoryA", nh!(h_CreateDirectoryW)),
        ("RemoveDirectoryW", nh!(h_RemoveDirectoryW)),
        ("GetCurrentDirectoryA", nh!(h_GetCurrentDirectoryA)),
        ("GetCurrentDirectoryW", nh!(h_GetCurrentDirectoryW)),
        ("SetCurrentDirectoryW", nh!(h_SetCurrentDirectoryW)),
        ("SetCurrentDirectoryA", nh!(h_SetCurrentDirectoryW)),
        ("GetFileAttributesW", nh!(h_GetFileAttributesW)),
        ("GetFileAttributesA", nh!(h_GetFileAttributesA)),
        ("GetCommandLineW", wh!(h_GetCommandLineW)),
        ("GetCommandLineA", wh!(h_GetCommandLineA)),
        ("GetModuleFileNameW", nh!(h_GetModuleFileNameW)),
        ("GetModuleFileNameA", nh!(h_GetModuleFileNameW)),
        ("GetModuleHandleA", nh!(h_GetModuleHandleA)),
        ("GetModuleHandleW", nh!(h_GetModuleHandleA)),
        ("GetProcAddress", wh!(h_GetProcAddress)),
        ("LoadLibraryA", nh!(h_LoadLibraryA)),
        ("LoadLibraryW", nh!(h_LoadLibraryA)),
        ("FreeLibrary", nh!(h_FreeLibrary)),
        ("HeapAlloc", wh!(h_HeapAlloc)),
        ("HeapReAlloc", wh!(h_HeapReAlloc)),
        ("HeapFree", wh!(h_HeapFree)),
        ("HeapCreate", nh!(h_GetProcessHeap)),
        ("HeapDestroy", nh!(h_GetProcessHeap)),
        ("GetProcessHeap", nh!(h_GetProcessHeap)),
        ("LocalAlloc", wh!(h_LocalAlloc)),
        ("LocalFree", wh!(h_LocalFree)),
        ("LocalReAlloc", wh!(h_HeapReAlloc)),
        ("GlobalAlloc", wh!(h_GlobalAlloc)),
        ("GlobalFree", wh!(h_GlobalFree)),
        ("GlobalLock", nh!(h_identity_p)),
        ("GlobalUnlock", nh!(h_one)),
        ("VirtualAlloc", nh!(h_VirtualAlloc)),
        ("VirtualFree", nh!(h_VirtualFree)),
        ("VirtualProtect", nh!(h_VirtualProtect)),
        ("VirtualQuery", nh!(h_zero)),
        ("QueryPerformanceCounter", nh!(h_QueryPerformanceCounter)),
        ("QueryPerformanceFrequency", nh!(h_QueryPerformanceFrequency)),
        ("GetSystemTimeAsFileTime", nh!(h_GetSystemTimeAsFileTime)),
        ("GetTickCount", nh!(h_GetTickCount)),
        ("Sleep", nh!(h_Sleep)),
        ("GetSystemInfo", nh!(h_GetSystemInfo)),
        ("GlobalMemoryStatusEx", nh!(h_GlobalMemoryStatusEx)),
        ("MultiByteToWideChar", nh!(h_MultiByteToWideChar)),
        ("WideCharToMultiByte", nh!(h_WideCharToMultiByte)),
        ("CreateThread", nh!(h_CreateThread)),
        ("ExitThread", nh!(h_ExitThread)),
        ("WaitForSingleObject", wh!(h_WaitForSingleObject)),
        ("WaitForMultipleObjects", wh!(h_WaitForMultipleObjects)),
        ("CreateEventW", wh!(h_CreateEventW)),
        ("CreateEventA", wh!(h_CreateEventW)),
        ("SetEvent", wh!(h_SetEvent)),
        ("ResetEvent", wh!(h_ResetEvent)),
        ("InitializeCriticalSection", nh!(h_InitializeCriticalSection)),
        (
            "InitializeCriticalSectionAndSpinCount",
            nh!(h_InitializeCriticalSectionAndSpinCount),
        ),
        ("EnterCriticalSection", nh!(h_EnterCriticalSection)),
        ("LeaveCriticalSection", nh!(h_LeaveCriticalSection)),
        ("TryEnterCriticalSection", nh!(h_TryEnterCriticalSection)),
        ("DeleteCriticalSection", nh!(h_DeleteCriticalSection)),
        ("SetLastError", nh!(h_SetLastError)),
        ("GetLastError", nh!(h_GetLastError)),
        ("GetCurrentProcessId", nh!(h_GetCurrentProcessId)),
        ("GetCurrentThreadId", nh!(h_GetCurrentThreadId)),
        ("ExitProcess", nh!(h_ExitProcess)),
        ("TerminateProcess", nh!(h_TerminateProcess)),
        ("IsProcessorFeaturePresent", nh!(h_IsProcessorFeaturePresent)),
        ("GetVersion", nh!(h_GetVersion)),
        ("GetVersionExA", nh!(h_GetVersionExA)),
        ("GetVersionExW", nh!(h_GetVersionExA)),
        ("GetACP", nh!(h_GetACP)),
        ("GetOEMCP", nh!(h_GetACP)),
        ("GetConsoleOutputCP", nh!(h_GetACP)),
        ("GetConsoleCP", nh!(h_GetACP)),
        ("IsDBCSLeadByte", nh!(h_IsDBCSLeadByte)),
        ("InterlockedIncrement", nh!(h_InterlockedIncrement)),
        ("InterlockedDecrement", nh!(h_InterlockedDecrement)),
        ("InterlockedExchange", nh!(h_InterlockedExchange)),
        ("InterlockedCompareExchange", nh!(h_InterlockedCompareExchange)),
        ("OutputDebugStringA", nh!(h_OutputDebugStringA)),
        ("OutputDebugStringW", nh!(h_OutputDebugStringA)),
        ("EncodePointer", nh!(h_EncodePointer)),
        ("DecodePointer", nh!(h_DecodePointer)),
        ("GetTempPathW", nh!(h_GetTempPathW)),
        ("GetTempPathA", nh!(h_GetTempPathW)),
        ("GetDiskFreeSpaceExW", nh!(h_GetDiskFreeSpaceExW)),
        ("CharToOemBuffA", nh!(h_CharToOemBuffA)),
        ("CharToOemA", nh!(h_CharToOemBuffA)),
        ("OemToCharA", nh!(h_CharToOemBuffA)),
        ("lstrlenA", nh!(h_lstrlenA)),
        ("lstrlenW", nh!(h_lstrlenA)),
        ("lstrcpyA", nh!(h_lstrcpyA)),
        ("lstrcpynA", nh!(h_lstrcpyA)),
        ("lstrcmpiA", nh!(h_lstrcmpiA)),
        ("lstrcmpA", nh!(h_lstrcmpiA)),
        ("GetEnvironmentStrings", wh!(h_GetEnvironmentStrings)),
        ("GetEnvironmentStringsA", wh!(h_GetEnvironmentStrings)),
        ("GetEnvironmentStringsW", wh!(h_GetEnvironmentStringsW)),
        ("GetEnvironmentVariableA", wh!(h_GetEnvironmentVariableA)),
        ("GetEnvironmentVariableW", wh!(h_GetEnvironmentVariableA)),
        ("FreeEnvironmentStringsA", nh!(h_FreeEnvironmentStringsA)),
        ("FreeEnvironmentStringsW", nh!(h_FreeEnvironmentStringsA)),
        ("TlsAlloc", wh!(h_TlsAlloc)),
        ("TlsGetValue", nh!(h_TlsGetValue)),
        ("TlsSetValue", nh!(h_TlsSetValue)),
        ("TlsFree", nh!(h_one)),
        ("FlsAlloc", wh!(h_TlsAlloc)),
        ("FlsGetValue", nh!(h_TlsGetValue)),
        ("FlsSetValue", nh!(h_TlsSetValue)),
        ("FlsFree", nh!(h_one)),
        ("DuplicateHandle", nh!(h_DuplicateHandle)),
        ("SetHandleCount", nh!(h_SetHandleCount)),
        ("GetUserDefaultLangID", nh!(h_GetUserDefaultLangID)),
        ("GetUserDefaultUILanguage", nh!(h_GetUserDefaultLangID)),
        ("GetSystemDefaultLangID", nh!(h_GetUserDefaultLangID)),
        ("GetStringTypeW", nh!(h_GetStringTypeW)),
        ("LCMapStringW", nh!(h_LCMapStringW)),
        ("GetLocaleInfoW", nh!(h_GetLocaleInfoW)),
        ("RaiseException", wh!(h_RaiseException)),
        ("GetProcessAffinityMask", nh!(h_GetProcessAffinityMask)),
        ("SetThreadAffinityMask", nh!(h_identity_p)),
        ("SetThreadPriority", nh!(h_identity_p)),
        ("SetPriorityClass", nh!(h_identity_p)),
        ("IsWow64Process", nh!(h_IsWow64Process)),
        ("GetExitCodeProcess", nh!(h_zero)),
        ("GetExitCodeThread", nh!(h_zero)),
        ("CreateFileMappingW", nh!(h_VirtualAlloc)),
        ("CreateFileMappingA", nh!(h_VirtualAlloc)),
        ("MapViewOfFile", nh!(h_VirtualAlloc)),
        ("UnmapViewOfFile", nh!(h_VirtualFree)),
        ("CreateMutexW", nh!(h_one)),
        ("CreateMutexA", nh!(h_one)),
        ("ReleaseMutex", nh!(h_one)),
        ("CreateSemaphoreW", nh!(h_one)),
        ("ReleaseSemaphore", nh!(h_one)),
        ("SwitchToThread", nh!(h_zero)),
        ("SetConsoleCtrlHandler", nh!(h_one)),
        ("GetConsoleMode", nh!(h_zero)),
        ("SetConsoleMode", nh!(h_one)),
        ("GetNumberOfConsoleInputEvents", nh!(h_zero)),
        ("ReadConsoleInputA", nh!(h_zero)),
        ("PeekConsoleInputA", nh!(h_zero)),
        ("GetDriveTypeW", nh!(h_three)),
        ("GetDriveTypeA", nh!(h_three)),
    ]
}

unsafe fn h_identity_p(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    a0
}
unsafe fn h_one(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    1
}
unsafe fn h_zero(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}
unsafe fn h_three(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    3 // DRIVE_FIXED
}

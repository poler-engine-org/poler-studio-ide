//! msvcrt-шар: printf-формтер (повний), __getmainargs, _initterm,
//! рядки/пам'ять/математика, qsort з компаратором через зворотний виклик.
//!
//! Вивчені інваріанти:
//!  * __getmainargs: argc пишеться ЗНАЧЕННЯМ (не покажчиком!), argv — масивом покажчиків
//!  * _initterm ПРОПУСКАЄ NULL-и (таблиця .CRT$XCU має діри)
//!  * stdout/stderr = (&_iob)[1/2] — CRT обчислює як base + k*sizeof(FILE), sizeof(FILE)=48
//!  * float-varargs: Win64-клієнт проливає XMM0-3 у shadow [rsp+8..0x28]

use super::api::World;
use super::runtime::Handler;

pub unsafe fn cstr(p: u64) -> String {
    if p == 0 {
        return String::new();
    }
    let mut v = Vec::new();
    let mut at = p as *const u8;
    unsafe {
        while *at != 0 && v.len() < 1 << 20 {
            v.push(*at);
            at = at.add(1);
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

pub unsafe fn cstr_w(p: u64) -> String {
    if p == 0 {
        return String::new();
    }
    let mut v = Vec::new();
    let mut at = p as *const u16;
    unsafe {
        loop {
            let ch = std::ptr::read_unaligned(at);
            if ch == 0 || v.len() > (1 << 20) {
                break;
            }
            v.push(ch);
            at = at.add(1);
        }
    }
    String::from_utf16_lossy(&v)
}

pub unsafe fn read_u64(at: u64) -> u64 {
    unsafe { std::ptr::read_unaligned(at as *const u64) }
}
pub unsafe fn read_u32(at: u64) -> u32 {
    unsafe { std::ptr::read_unaligned(at as *const u32) }
}
pub unsafe fn read_u16(at: u64) -> u16 {
    unsafe { std::ptr::read_unaligned(at as *const u16) }
}
pub unsafe fn read_u8(at: u64) -> u8 {
    unsafe { std::ptr::read_unaligned(at as *const u8) }
}
pub unsafe fn write_u64(at: u64, v: u64) {
    unsafe { std::ptr::write_unaligned(at as *mut u64, v) }
}
pub unsafe fn write_u32(at: u64, v: u32) {
    unsafe { std::ptr::write_unaligned(at as *mut u32, v) }
}
pub unsafe fn write_u16(at: u64, v: u16) {
    unsafe { std::ptr::write_unaligned(at as *mut u16, v) }
}
pub unsafe fn write_u8(at: u64, v: u8) {
    unsafe { std::ptr::write_unaligned(at as *mut u8, v) }
}

// ============================== VarArgs ==============================

/// Послідовний читач Win64-varargs. Два режими:
///  * Regs: перші 6 цілочисельних у регістрах (a0..a5), далі стек [entry_rsp+0x28+8k];
///    float-и arg0..3 — у XMM-spill клієнта [entry_rsp+8+16k]
///  * Linear (va_list): аргументи лежать лінійно на [ap + 8*i] — так їх
///    розкладає varargs-пролог MSVC у shadow-області викликача
pub struct VarArgs {
    regs: [u64; 6],
    entry_rsp: u64,
    linear: Option<u64>,
    idx: usize,
}

impl VarArgs {
    pub fn new(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, entry_rsp: u64) -> Self {
        Self {
            regs: [a0, a1, a2, a3, a4, a5],
            entry_rsp,
            linear: None,
            idx: 0,
        }
    }

    /// va_list-режим: аргументи лінійно від покажчика ap.
    pub fn from_valist(ap: u64) -> Self {
        Self {
            regs: [0; 6],
            entry_rsp: 0,
            linear: Some(ap),
            idx: 0,
        }
    }

    fn slot_addr(&self, i: usize) -> u64 {
        // arg4 @ [E+0x28], arg5 @ [E+0x30], arg6 @ [E+0x38] …
        self.entry_rsp + 0x28 + 8 * (i as u64 - 4)
    }

    pub fn next_u64(&mut self) -> u64 {
        let i = self.idx;
        self.idx += 1;
        if let Some(ap) = self.linear {
            return unsafe { read_u64(ap + 8 * i as u64) };
        }
        if i < 6 {
            self.regs[i]
        } else {
            unsafe { read_u64(self.slot_addr(i)) }
        }
    }

    pub fn next_i32(&mut self) -> i32 {
        self.next_u64() as i32
    }

    pub fn next_f64(&mut self) -> f64 {
        let i = self.idx;
        self.idx += 1;
        let bits = if let Some(ap) = self.linear {
            unsafe { read_u64(ap + 8 * i as u64) }
        } else if i < 4 {
            // XMM-spill клієнта: [E+8+16i] (8 байт значення double)
            unsafe { read_u64(self.entry_rsp + 8 + 16 * i as u64) }
        } else if i < 6 {
            self.regs[i]
        } else {
            unsafe { read_u64(self.slot_addr(i)) }
        };
        f64::from_bits(bits)
    }

    /// Пропустити n уже знаданих аргументів (fmt, буфери тощо).
    pub fn skip(&mut self, n: usize) {
        self.idx += n;
    }
}

// ============================== Формтер printf ==============================

#[derive(Default, Clone, Copy)]
struct Flags {
    minus: bool,
    plus: bool,
    space: bool,
    zero: bool,
    alt: bool,
}

/// Повний формтер у рядок. `w` — куди писати.
pub unsafe fn format(fmt: &str, va: &mut VarArgs, w: &mut String) {
    let b: Vec<char> = fmt.chars().collect();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c != '%' {
            w.push(c);
            i += 1;
            continue;
        }
        i += 1;
        if i >= b.len() {
            break;
        }
        if b[i] == '%' {
            w.push('%');
            i += 1;
            continue;
        }
        let mut fl = Flags::default();
        loop {
            match b.get(i) {
                Some('-') => fl.minus = true,
                Some('+') => fl.plus = true,
                Some(' ') => fl.space = true,
                Some('0') => fl.zero = true,
                Some('#') => fl.alt = true,
                _ => break,
            }
            i += 1;
        }
        let mut width: Option<usize> = None;
        if b.get(i) == Some(&'*') {
            let n = va.next_i32();
            width = Some(if n < 0 {
                fl.minus = true;
                (-n) as usize
            } else {
                n as usize
            });
            i += 1;
        } else {
            let mut n = 0usize;
            while let Some(d) = b.get(i).and_then(|c| c.to_digit(10)) {
                n = n * 10 + d as usize;
                i += 1;
            }
            if n > 0 {
                width = Some(n);
            }
        }
        let mut prec: Option<usize> = None;
        if b.get(i) == Some(&'.') {
            i += 1;
            if b.get(i) == Some(&'*') {
                let n = va.next_i32();
                prec = Some(n.max(0) as usize);
                i += 1;
            } else {
                let mut n = 0usize;
                while let Some(d) = b.get(i).and_then(|c| c.to_digit(10)) {
                    n = n * 10 + d as usize;
                    i += 1;
                }
                prec = Some(n);
            }
        }
        // розмір
        let mut long_ = 0u8; // 1=l, 2=ll/I64, 3=w, 4=h, 5=z
        loop {
            match b.get(i) {
                Some('l') => {
                    long_ = if long_ == 1 { 2 } else { 1 };
                    i += 1;
                }
                Some('I') => {
                    // I64 / I32
                    if b.get(i + 1) == Some(&'6') {
                        long_ = 2;
                        i += 3;
                    } else if b.get(i + 1) == Some(&'3') {
                        long_ = 0;
                        i += 3;
                    } else {
                        long_ = 2;
                        i += 1;
                    }
                }
                Some('h') => {
                    long_ = 4;
                    i += 1;
                }
                Some('w') => {
                    long_ = 3;
                    i += 1;
                }
                Some('z') | Some('t') => {
                    long_ = 5;
                    i += 1;
                }
                Some('F') | Some('N') => {
                    i += 1; // далекі/близькі — рудимент
                }
                _ => break,
            }
        }
        let Some(conv) = b.get(i).copied() else { break };
        i += 1;

        let mut out = String::new();
        match conv {
            'd' | 'i' => {
                let v: i64 = match long_ {
                    2 | 5 => va.next_u64() as i64,
                    1 => va.next_u64() as i32 as i64,
                    4 => va.next_u64() as i16 as i64,
                    _ => va.next_u64() as i32 as i64,
                };
                out = format_args_num(v, fl, width, prec, 10, false, false);
            }
            'u' => {
                let v: u64 = match long_ {
                    2 | 5 => va.next_u64(),
                    1 => va.next_u64() as u32 as u64,
                    4 => va.next_u64() as u16 as u64,
                    _ => va.next_u64() as u32 as u64,
                };
                out = format_args_num(v as i64, fl, width, prec, 10, false, false);
            }
            'o' => {
                let v = va.next_u64();
                out = format_args_num(v as i64, fl, width, prec, 8, false, false);
            }
            'x' | 'X' => {
                let v = va.next_u64();
                out = format_args_num(v as i64, fl, width, prec, 16, conv == 'X', false);
            }
            'c' => {
                if long_ == 1 || long_ == 3 {
                    let ch = va.next_u64() as u16;
                    let s = String::from_utf16_lossy(&[ch]);
                    out.push_str(&s);
                } else {
                    let ch = va.next_u64() as u8 as char;
                    out.push(ch);
                }
            }
            's' | 'S' => {
                let wide = conv == 'S' || long_ == 1 || long_ == 3;
                let p = va.next_u64();
                let s = if wide {
                    unsafe { cstr_w(p) }
                } else {
                    unsafe { cstr(p) }
                };
                out.push_str(&s);
                if let Some(pr) = prec {
                    out.truncate(out.chars().take(pr).map(char::len_utf8).sum());
                }
            }
            'p' => {
                let v = va.next_u64();
                out = format!("0x{:012x}", v);
            }
            'f' | 'F' => {
                let v = va.next_f64();
                out = fmt_fixed(v, fl, width, prec);
            }
            'e' | 'E' => {
                let v = va.next_f64();
                out = fmt_exp(v, fl, width, prec, conv == 'E');
            }
            'g' | 'G' => {
                let v = va.next_f64();
                out = fmt_general(v, fl, width, prec, conv == 'G');
            }
            'n' => {
                let p = va.next_u64();
                unsafe { write_u64(p, va.idx as u64 - 1) };
                continue;
            }
            _ => {
                out.push(conv);
            }
        }
        // ширини
        if let Some(wd) = width {
            let len = out.chars().count();
            if len < wd {
                let pad = wd - len;
                if fl.minus {
                    out.push_str(&" ".repeat(pad));
                } else if fl.zero
                    && matches!(conv, 'd' | 'i' | 'u' | 'o' | 'x' | 'X' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G')
                    && !out.starts_with('-')
                {
                    // нулі після знаку/префікса
                    let (pre, rest) = if out.starts_with('-') || out.starts_with('+') {
                        (out.chars().take(1).collect::<String>(), out[1..].to_string())
                    } else if out.starts_with("0x") || out.starts_with("0X") {
                        (out.chars().take(2).collect::<String>(), out[2..].to_string())
                    } else {
                        (String::new(), out.clone())
                    };
                    out = format!("{pre}{}{rest}", "0".repeat(pad));
                } else {
                    out = format!("{}{out}", " ".repeat(pad));
                }
            }
        }
        w.push_str(&out);
    }
}

fn format_args_num(
    v: i64,
    fl: Flags,
    _width: Option<usize>,
    prec: Option<usize>,
    radix: u32,
    upper: bool,
    _neg_hint: bool,
) -> String {
    let neg = v < 0;
    let mag = (v as i128).unsigned_abs();
    let mut digits = match radix {
        8 => format!("{mag:o}"),
        16 => {
            if upper {
                format!("{mag:X}")
            } else {
                format!("{mag:x}")
            }
        }
        _ => format!("{mag}"),
    };
    if let Some(p) = prec {
        while digits.len() < p {
            digits.insert(0, '0');
        }
    }
    let alt = fl.alt
        && match radix {
            16 => mag != 0,
            8 => !digits.starts_with('0'),
            _ => false,
        };
    let prefix = if neg {
        "-"
    } else if fl.plus {
        "+"
    } else if fl.space {
        " "
    } else {
        ""
    };
    let tag = if alt && radix == 16 {
        if upper { "0X" } else { "0x" }
    } else {
        ""
    };
    format!("{prefix}{tag}{digits}")
}

fn fmt_fixed(v: f64, fl: Flags, _width: Option<usize>, prec: Option<usize>) -> String {
    let p = prec.unwrap_or(6);
    let mut s = format!("{v:.p$}");
    if fl.alt && p == 0 && !s.contains('.') {
        s.push('.');
    }
    if fl.plus && !s.starts_with('-') {
        s.insert(0, '+');
    } else if fl.space && !s.starts_with('-') {
        s.insert(0, ' ');
    }
    s
}

fn fmt_exp(v: f64, fl: Flags, _width: Option<usize>, prec: Option<usize>, upper: bool) -> String {
    let p = prec.unwrap_or(6);
    let mut s = format!("{v:.p$e}");
    if upper {
        s = s.to_uppercase();
    }
    // msvcrt: три цифри експоненти? Ні — мінімум дві (як C)
    if fl.plus && !s.starts_with('-') {
        s.insert(0, '+');
    }
    s
}

fn fmt_general(v: f64, fl: Flags, _width: Option<usize>, prec: Option<usize>, upper: bool) -> String {
    let p = prec.unwrap_or(6);
    let mut s = format!("{v:.p$e}");
    // спрощено: як %e — для демо-виводу достатньо
    if upper {
        s = s.to_uppercase();
    }
    if fl.plus && !s.starts_with('-') {
        s.insert(0, '+');
    }
    s
}

// ============================== Консоль/_iob ==============================

/// Номер fd за покажчиком FILE*: CRT передає (&_iob)[k], sizeof(FILE)=48.
pub fn iob_fd(world: &World, file_ptr: u64) -> i32 {
    let base = world.with_inner(|w| w.iob_base);
    if file_ptr == 0 || base == 0 {
        return 1;
    }
    let diff = file_ptr.wrapping_sub(base);
    if diff > 0x1000 {
        return 1;
    }
    match diff / 48 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 1,
    }
}

pub fn raw_write(fd: i32, s: &str) {
    let b = s.as_bytes();
    let mut off = 0usize;
    unsafe {
        while off < b.len() {
            let n = libc::write(fd, b.as_ptr().add(off) as *const _, b.len() - off);
            if n <= 0 {
                break;
            }
            off += n as usize;
        }
    }
}

// ============================== msvcrt-шими ==============================

unsafe fn h_printf(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, entry_rsp: u64, _cr: u64) -> u64 {
    let fmt = unsafe { cstr(a0) };
    let mut va = VarArgs::new(a0, a1, a2, a3, a4, a5, entry_rsp);
    va.skip(1); // fmt
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    raw_write(1, &out);
    out.len() as u64
}

unsafe fn h_fprintf(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, entry_rsp: u64, _cr: u64, w: &World) -> u64 {
    let fmt = unsafe { cstr(a1) };
    let mut va = VarArgs::new(a0, a1, a2, a3, a4, a5, entry_rsp);
    va.skip(2); // FILE*, fmt
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    raw_write(iob_fd(w, a0), &out);
    out.len() as u64
}

unsafe fn h_sprintf(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, entry_rsp: u64, _cr: u64) -> u64 {
    let fmt = unsafe { cstr(a1) };
    let mut va = VarArgs::new(a0, a1, a2, a3, a4, a5, entry_rsp);
    va.skip(2); // buf, fmt
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    let bytes = out.as_bytes();
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), a0 as *mut u8, bytes.len()) };
    unsafe { write_u8(a0 + bytes.len() as u64, 0) };
    bytes.len() as u64
}

unsafe fn h_snprintf(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, entry_rsp: u64, _cr: u64) -> u64 {
    // (buf, size, fmt, ...)
    let size = a1;
    let fmt = unsafe { cstr(a2) };
    let mut va = VarArgs::new(a0, a1, a2, a3, a4, a5, entry_rsp);
    va.skip(3); // buf, size, fmt
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    let bytes = out.as_bytes();
    let n = bytes.len();
    if size > 0 {
        let cap = (size - 1).min(n as u64) as usize;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), a0 as *mut u8, cap) };
        unsafe { write_u8(a0 + cap as u64, 0) };
    }
    n as u64
}

unsafe fn h_vprintf(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _entry_rsp: u64, _cr: u64) -> u64 {
    // (fmt, va_list) — va_list = лінійна область прольоту викликача
    let fmt = unsafe { cstr(a0) };
    let mut va = VarArgs::from_valist(a1);
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    raw_write(1, &out);
    out.len() as u64
}

unsafe fn h__vsnprintf(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _entry_rsp: u64, _cr: u64) -> u64 {
    // (buf, size, fmt, va_list)
    let size = a1;
    let fmt = unsafe { cstr(a2) };
    let mut va = VarArgs::from_valist(a3);
    let mut out = String::new();
    unsafe { format(&fmt, &mut va, &mut out) };
    let bytes = out.as_bytes();
    if size > 0 {
        let cap = (size - 1).min(bytes.len() as u64) as usize;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), a0 as *mut u8, cap) };
        unsafe { write_u8(a0 + cap as u64, 0) };
    }
    bytes.len() as u64
}

unsafe fn h_puts(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a0) };
    raw_write(1, &format!("{s}\n"));
    1
}

unsafe fn h_fputs(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let s = unsafe { cstr(a0) };
    raw_write(iob_fd(w, a1), &s);
    1
}

unsafe fn h_fwrite(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // (buf, size, count, FILE*)
    let n = (a1 * a2) as usize;
    let fd = iob_fd(w, a3);
    unsafe {
        let mut off = 0usize;
        while off < n {
            let k = libc::write(fd, (a0 as *const u8).add(off) as *const _, n - off);
            if k <= 0 {
                break;
            }
            off += k as usize;
        }
    }
    a2
}

unsafe fn h_putchar(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ch = a0 as u8;
    let s = String::from_utf8_lossy(&[ch]).into_owned();
    raw_write(1, &s);
    a0
}

unsafe fn h_fflush(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h___getmainargs(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // (__getmainargs)(&argc, &argv, &envp, doWildCard, start_info)
    // ФІКС: argc — ЗНАЧЕННЯ (не покажчик!)
    let argv_copy = w.with_inner(|i| i.argv.clone());
    let argc = argv_copy.len() as u64;
    unsafe { write_u64(a0, argc) };
    // argv: масив покажчиків + рядки (живуть вічно)
    let n = argv_copy.len();
    let arr = w.alloc_forever(((n + 1) * 8) as u32) as usize;
    let mut strs = Vec::new();
    for a in argv_copy.iter() {
        let p = w.alloc_forever(a.len() as u32 + 1) as usize;
        unsafe {
            std::ptr::copy_nonoverlapping(a.as_ptr(), p as *mut u8, a.len());
            write_u8(p as u64 + a.len() as u64, 0);
        }
        strs.push(p);
    }
    for (i, p) in strs.iter().enumerate() {
        unsafe { write_u64((arr + i * 8) as u64, *p as u64) };
    }
    unsafe { write_u64((arr + n * 8) as u64, 0) };
    unsafe { write_u64(a1, arr as u64) };
    // envp: ANSI-блок
    let envp = w.with_inner(|i| i.envp_block);
    unsafe { write_u64(a2, envp) };
    0
}

unsafe fn h__initterm(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, entry_rsp: u64, caller_rip: u64) -> u64 {
    // ФІКС: пропускаємо NULL-и (у таблиці .CRT$XCU діри)
    // ФІКС-2: конструктори — НА ПОТОЧНОМУ стеці + hop для SEH-walker-а
    let live = super::runtime::capture_regs();
    let hop = super::api::Hop {
        rsp: entry_rsp + 8,
        rip: caller_rip,
        rbx: live[0],
        rbp: unsafe { read_u64(entry_rsp - 8) },
        rdi: unsafe { read_u64(entry_rsp - 16) },
        rsi: unsafe { read_u64(entry_rsp - 24) },
        r12: live[1],
        r13: live[2],
        r14: live[3],
        r15: live[4],
    };
    let w = World::get();
    w.push_hop(hop);
    let mut p = a0;
    let end = a1;
    let mut n = 0u64;
    while p < end {
        let f = unsafe { read_u64(p) };
        if f != 0 {
            let _ = super::runtime::win_call_here(f, 0, 0, 0, 0);
            n += 1;
        }
        p += 8;
    }
    let _ = w.pop_hop();
    n
}

unsafe fn h_initterm_legacy(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    h__initterm(a0, a1, 0, 0, 0, 0, 0, 0)
}

unsafe fn h_qsort(a0: u64, a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, entry_rsp: u64, caller_rip: u64) -> u64 {
    // (base, num, width, compar)
    let num = a1 as usize;
    let width = a2 as usize;
    let compar = a3;
    if num < 2 || width == 0 || compar == 0 {
        return 0;
    }
    let live = super::runtime::capture_regs();
    let hop = super::api::Hop {
        rsp: entry_rsp + 8,
        rip: caller_rip,
        rbx: live[0],
        rbp: unsafe { read_u64(entry_rsp - 8) },
        rdi: unsafe { read_u64(entry_rsp - 16) },
        rsi: unsafe { read_u64(entry_rsp - 24) },
        r12: live[1],
        r13: live[2],
        r14: live[3],
        r15: live[4],
    };
    World::get().push_hop(hop);
    // злиття з тимчасовим буфером; компаратор — зворотній виклик
    let mut idx: Vec<usize> = (0..num).collect();
    let mut tmp: Vec<u8> = vec![0; width];
    let mut src: Vec<u8> = unsafe { std::slice::from_raw_parts(a0 as *const u8, num * width) }.to_vec();
    unsafe { merge_sort(&mut src, &mut tmp, width, &mut idx, compar, &hop) };
    let _ = World::get().pop_hop();
    // переписуємо за індексами
    let mut dst: Vec<u8> = vec![0; num * width];
    for (i, &si) in idx.iter().enumerate() {
        dst[i * width..(i + 1) * width].copy_from_slice(&src[si * width..(si + 1) * width]);
    }
    unsafe { std::ptr::copy_nonoverlapping(dst.as_ptr(), a0 as *mut u8, num * width) };
    0
}

unsafe fn merge_sort(
    data: &mut [u8],
    tmp: &mut [u8],
    width: usize,
    idx: &mut [usize],
    compar: u64,
    _hop: &super::api::Hop,
) {
    let n = idx.len();
    if n < 2 {
        return;
    }
    let mid = n / 2;
    let mut left = idx[..mid].to_vec();
    let mut right = idx[mid..].to_vec();
    unsafe { merge_sort(data, tmp, width, &mut left, compar, _hop) };
    unsafe { merge_sort(data, tmp, width, &mut right, compar, _hop) };
    let (mut i, mut j, mut k) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        let li = left[i];
        let rj = right[j];
        let c = unsafe {
            super::runtime::win_call_here(
                compar,
                data.as_ptr().add(li * width) as u64,
                data.as_ptr().add(rj * width) as u64,
                0,
                0,
            )
        };
        if c <= 1 {
            idx[k] = li;
            i += 1;
        } else {
            idx[k] = rj;
            j += 1;
        }
        k += 1;
    }
    while i < left.len() {
        idx[k] = left[i];
        i += 1;
        k += 1;
    }
    while j < right.len() {
        idx[k] = right[j];
        j += 1;
        k += 1;
    }
}

unsafe fn h_bsearch(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let key = a0;
    let base = a1;
    let num = a2 as usize;
    let width = a3 as usize;
    let compar = a4;
    let (mut lo, mut hi) = (0i64, num as i64 - 1);
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let el = base + mid as u64 * width as u64;
        let c = super::runtime::win_call_here(compar, key, el, 0, 0) as i64;
        if c == 0 {
            return el;
        }
        if c < 0 {
            hi = mid - 1;
        } else {
            lo = mid + 1;
        }
    }
    0
}

// ---- рядки/пам'ять ----

unsafe fn h_strlen(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let mut n = 0u64;
    unsafe {
        let mut p = a0 as *const u8;
        while *p != 0 && n < (1 << 30) {
            p = p.add(1);
            n += 1;
        }
    }
    n
}

unsafe fn h_strcpy(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a1) };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a0 as *mut u8, s.len());
        write_u8(a0 + s.len() as u64, 0);
    }
    a0
}

unsafe fn h_strncpy(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let n = a2 as usize;
    let s = unsafe { cstr(a1) };
    let m = s.len().min(n);
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a0 as *mut u8, m);
        if m < n {
            std::ptr::write_bytes((a0 + m as u64) as *mut u8, 0, n - m);
        }
    }
    a0
}

unsafe fn h_strcmp(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let a = unsafe { cstr(a0) };
    let b = unsafe { cstr(a1) };
    match a.cmp(&b) {
        std::cmp::Ordering::Less => u64::MAX, // -1
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

unsafe fn h_strncmp(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let n = a2 as usize;
    unsafe {
        for k in 0..n {
            let x = read_u8(a0 + k as u64);
            let y = read_u8(a1 + k as u64);
            if x != y || x == 0 {
                return (x as i64 - y as i64) as u64;
            }
        }
    }
    0
}

unsafe fn h_strcat(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a1) };
    let mut n = 0u64;
    unsafe {
        while read_u8(a0 + n) != 0 {
            n += 1;
        }
        std::ptr::copy_nonoverlapping(s.as_ptr(), (a0 + n) as *mut u8, s.len());
        write_u8(a0 + n + s.len() as u64, 0);
    }
    a0
}

unsafe fn h_strchr(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ch = a1 as u8;
    unsafe {
        let mut p = a0;
        loop {
            let c = read_u8(p);
            if c == ch {
                return p;
            }
            if c == 0 {
                return 0;
            }
            p += 1;
        }
    }
}

unsafe fn h_strrchr(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ch = a1 as u8;
    let mut last = 0u64;
    unsafe {
        let mut p = a0;
        loop {
            let c = read_u8(p);
            if c == ch {
                last = p;
            }
            if c == 0 {
                break;
            }
            p += 1;
        }
    }
    last
}

unsafe fn h_strstr(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let hay = unsafe { cstr(a0) };
    let needle = unsafe { cstr(a1) };
    if needle.is_empty() {
        return a0;
    }
    match hay.find(&needle) {
        Some(off) => a0 + off as u64,
        None => 0,
    }
}

unsafe fn h_strupr(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        let mut p = a0;
        loop {
            let c = read_u8(p);
            if c == 0 {
                break;
            }
            write_u8(p, c.to_ascii_uppercase());
            p += 1;
        }
    }
    a0
}

unsafe fn h_strdup(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let s = unsafe { cstr(a0) };
    let p = w.heap_alloc(s.len() as u32 + 1);
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), p as *mut u8, s.len());
        write_u8(p + s.len() as u64, 0);
    }
    p
}

unsafe fn h_memcpy(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a2 > 0 {
        unsafe { std::ptr::copy(a1 as *const u8, a0 as *mut u8, a2 as usize) };
    }
    a0
}

unsafe fn h_memmove(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a2 > 0 {
        unsafe { std::ptr::copy(a1 as *const u8, a0 as *mut u8, a2 as usize) };
    }
    a0
}

unsafe fn h_memset(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a2 > 0 {
        unsafe { std::ptr::write_bytes(a0 as *mut u8, a1 as u8, a2 as usize) };
    }
    a0
}

unsafe fn h_memcmp(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let n = a2 as usize;
    unsafe {
        let x = std::slice::from_raw_parts(a0 as *const u8, n);
        let y = std::slice::from_raw_parts(a1 as *const u8, n);
        for k in 0..n {
            if x[k] != y[k] {
                return (x[k] as i64 - y[k] as i64) as u64;
            }
        }
    }
    0
}

// ---- математика ----

macro_rules! math1 {
    ($name:ident, $f:expr) => {
        unsafe fn $name(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
            let x = f64::from_bits(a0);
            let f: fn(f64) -> f64 = $f;
            f(x).to_bits()
        }
    };
}
macro_rules! math2 {
    ($name:ident, $f:expr) => {
        unsafe fn $name(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
            let x = f64::from_bits(a0);
            let y = f64::from_bits(a1);
            let f: fn(f64, f64) -> f64 = $f;
            f(x, y).to_bits()
        }
    };
}
math1!(h_sin, |x| x.sin());
math1!(h_cos, |x| x.cos());
math1!(h_tan, |x| x.tan());
math1!(h_asin, |x| x.asin());
math1!(h_acos, |x| x.acos());
math1!(h_atan, |x| x.atan());
math1!(h_exp, |x| x.exp());
math1!(h_log, |x| x.ln());
math1!(h_log10, |x| x.log10());
math1!(h_sqrt, |x| x.sqrt());
math1!(h_fabs, |x| x.abs());
math1!(h_floor, |x| x.floor());
math1!(h_ceil, |x| x.ceil());
math2!(h_pow, |x, y| x.powf(y));
math2!(h_atan2, |x, y| x.atan2(y));
math2!(h_fmod, |x, y| x % y);

// ---- купка (malloc-сімейство) ----

unsafe fn h_malloc(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_alloc(a0 as u32)
}

unsafe fn h_calloc(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let n = a0.saturating_mul(a1);
    let p = w.heap_alloc(n as u32);
    if p != 0 && n > 0 {
        unsafe { std::ptr::write_bytes(p as *mut u8, 0, n as usize) };
    }
    p
}

unsafe fn h_realloc(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_realloc(a0, a1 as u32)
}

unsafe fn h_free(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.heap_free(a0);
    0
}

unsafe fn h__msize(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner(|i| i.heap_sizes.get(&(a0 as usize)).copied().unwrap_or(0) as u64)
}

// ---- широкосимвольні ----

unsafe fn h_wcslen(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let mut n = 0u64;
    unsafe {
        while read_u16(a0 + n * 2) != 0 {
            n += 1;
        }
    }
    n
}

unsafe fn h_wcscpy(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        let mut p = 0u64;
        loop {
            let ch = read_u16(a1 + p * 2);
            write_u16(a0 + p * 2, ch);
            if ch == 0 {
                break;
            }
            p += 1;
        }
    }
    a0
}

unsafe fn h_wcscat(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        let mut n = 0u64;
        while read_u16(a0 + n * 2) != 0 {
            n += 1;
        }
        let mut p = 0u64;
        loop {
            let ch = read_u16(a1 + p * 2);
            write_u16(a0 + (n + p) * 2, ch);
            if ch == 0 {
                break;
            }
            p += 1;
        }
    }
    a0
}

unsafe fn h_wcscmp(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        let mut p = 0u64;
        loop {
            let x = read_u16(a0 + p * 2);
            let y = read_u16(a1 + p * 2);
            if x != y || x == 0 {
                return (x as i64 - y as i64) as u64;
            }
            p += 1;
        }
    }
}

unsafe fn h_wcschr(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let ch = a1 as u16;
    unsafe {
        let mut p = a0;
        loop {
            let c = read_u16(p);
            if c == ch {
                return p;
            }
            if c == 0 {
                return 0;
            }
            p += 2;
        }
    }
}

unsafe fn h_wcsstr(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let hay = unsafe { cstr_w(a0) };
    let needle = unsafe { cstr_w(a1) };
    if needle.is_empty() {
        return a0;
    }
    match hay.find(&needle) {
        Some(off) => a0 + off as u64 * 2,
        None => 0,
    }
}

unsafe fn h_wcstombs(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr_w(a1) };
    let bytes = s.as_bytes();
    let n = bytes.len().min(a2 as usize).min((a2 as usize).saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), a0 as *mut u8, n);
        if n < a2 as usize && a2 > 0 {
            write_u8(a0 + n as u64, 0);
        }
    }
    n as u64
}

unsafe fn h_mbtowc(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a0 == 0 || a1 == 0 {
        return 0;
    }
    let b = unsafe { read_u8(a1) };
    unsafe { write_u16(a0, b as u16) };
    1
}

unsafe fn h_wctomb(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a0 == 0 {
        return 0;
    }
    let ch = a1 as u16;
    if ch < 0x100 {
        unsafe { write_u8(a0, ch as u8) };
        1
    } else {
        let s = String::from_utf16_lossy(&[ch]);
        let b = s.as_bytes();
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), a0 as *mut u8, b.len()) };
        b.len() as u64
    }
}

// ---- конвертація чисел ----

unsafe fn h_atoi(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { cstr(a0).trim().parse::<i32>().unwrap_or(0) as u32 as u64 }
}

unsafe fn h_atol(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { cstr(a0).trim().parse::<i64>().unwrap_or(0) as u64 }
}

unsafe fn h_atof(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe { cstr(a0).trim().parse::<f64>().unwrap_or(0.0).to_bits() }
}

unsafe fn h_strtoul(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a0) };
    let t = s.trim_start();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == 'x' || *c == 'X' || c.is_ascii_hexdigit()).collect();
    let v = if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        digits.parse::<u64>().unwrap_or(0)
    };
    if a1 != 0 {
        let consumed = digits.len();
        unsafe { write_u64(a1, a0 + consumed as u64) };
    }
    v
}

unsafe fn h_strtod(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = unsafe { cstr(a0) };
    let t = s.trim_start();
    let digits: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        .collect();
    let v: f64 = digits.parse().unwrap_or(0.0);
    if a1 != 0 {
        unsafe { write_u64(a1, a0 + digits.len() as u64) };
    }
    v.to_bits()
}

// ---- інше ----

unsafe fn h_getenv(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let name = unsafe { cstr(a0) };
    w.with_inner(|i| {
        for (k, _v) in &i.env {
            if k.eq_ignore_ascii_case(&name) {
                return i.env_value_ptrs.get(k).copied().unwrap_or(0);
            }
        }
        0
    })
}

unsafe fn h_abort(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    eprintln!("winpe: msvcrt abort()");
    std::process::exit(3);
}

unsafe fn h_exit(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    std::process::exit((a0 as u32 & 0xFF) as i32);
}

thread_local! {
    static ERRNO: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

unsafe fn h_errno(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    ERRNO.with(|e| e.as_ptr() as u64)
}

unsafe fn h_signal(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h_rand(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner_mut(|i| {
        i.rand_next = i
            .rand_next
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        i.rand_next >> 33
    })
}

unsafe fn h_srand(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    w.with_inner_mut(|i| i.rand_next = a0 | 1);
    0
}

unsafe fn h_clock(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    ((ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1000) & 0xFFFFFFFF
}

unsafe fn h_time(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if a0 != 0 {
        unsafe { write_u64(a0, t) };
    }
    t
}

unsafe fn h_tzset(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

/// Макрообгортка: перетворює (…) з &World на чисту Handler-сигнатуру.
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

pub fn registry_full() -> Vec<(&'static str, Handler)> {
    use super::api::WORLD;
    let _ = &WORLD;
    vec![
        ("malloc", wh!(h_malloc)),
        ("calloc", wh!(h_calloc)),
        ("realloc", wh!(h_realloc)),
        ("free", wh!(h_free)),
        ("_malloc_crt", wh!(h_malloc)),
        ("_calloc_crt", wh!(h_calloc)),
        ("_free_dbg", wh!(h_free)),
        ("_msize", wh!(h__msize)),
        ("_recalloc", wh!(h_calloc)),
        ("_expand", wh!(h_malloc)),
        ("printf", nh!(h_printf)),
        ("vprintf", nh!(h_vprintf)),
        ("_vsnprintf", nh!(h__vsnprintf)),
        ("vsnprintf", nh!(h__vsnprintf)),
        ("sprintf", nh!(h_sprintf)),
        ("_snprintf", nh!(h_snprintf)),
        ("snprintf", nh!(h_snprintf)),
        ("_snwprintf", nh!(h_snprintf)),
        ("puts", nh!(h_puts)),
        ("fputs", wh!(h_fputs)),
        ("fwrite", wh!(h_fwrite)),
        ("putchar", nh!(h_putchar)),
        ("fputc", nh!(h_putchar)),
        ("fflush", nh!(h_fflush)),
        ("fprintf", wh!(h_fprintf)),
        ("vfprintf", wh!(h_fprintf)),
        ("__getmainargs", wh!(h___getmainargs)),
        ("_getmainargs", wh!(h___getmainargs)),
        ("__wgetmainargs", wh!(h___getmainargs)),
        ("_initterm", nh!(h__initterm)),
        ("_initterm_e", nh!(h_initterm_legacy)),
        ("__initterm", nh!(h__initterm)),
        ("qsort", nh!(h_qsort)),
        ("bsearch", nh!(h_bsearch)),
        ("strlen", nh!(h_strlen)),
        ("strcpy", nh!(h_strcpy)),
        ("strncpy", nh!(h_strncpy)),
        ("strcmp", nh!(h_strcmp)),
        ("strncmp", nh!(h_strncmp)),
        ("strcat", nh!(h_strcat)),
        ("strncat", nh!(h_strcat)),
        ("strchr", nh!(h_strchr)),
        ("strrchr", nh!(h_strrchr)),
        ("strstr", nh!(h_strstr)),
        ("_strupr", nh!(h_strupr)),
        ("_strdup", wh!(h_strdup)),
        ("strdup", wh!(h_strdup)),
        ("memcpy", nh!(h_memcpy)),
        ("memmove", nh!(h_memmove)),
        ("memset", nh!(h_memset)),
        ("memcmp", nh!(h_memcmp)),
        ("_memicmp", nh!(h_memcmp)),
        ("sin", nh!(h_sin)),
        ("cos", nh!(h_cos)),
        ("tan", nh!(h_tan)),
        ("asin", nh!(h_asin)),
        ("acos", nh!(h_acos)),
        ("atan", nh!(h_atan)),
        ("atan2", nh!(h_atan2)),
        ("exp", nh!(h_exp)),
        ("log", nh!(h_log)),
        ("log10", nh!(h_log10)),
        ("sqrt", nh!(h_sqrt)),
        ("fabs", nh!(h_fabs)),
        ("floor", nh!(h_floor)),
        ("ceil", nh!(h_ceil)),
        ("pow", nh!(h_pow)),
        ("fmod", nh!(h_fmod)),
        ("wcslen", nh!(h_wcslen)),
        ("wcscpy", nh!(h_wcscpy)),
        ("wcscat", nh!(h_wcscat)),
        ("wcscmp", nh!(h_wcscmp)),
        ("wcschr", nh!(h_wcschr)),
        ("wcsstr", nh!(h_wcsstr)),
        ("wcstombs", nh!(h_wcstombs)),
        ("mbtowc", nh!(h_mbtowc)),
        ("wctomb", nh!(h_wctomb)),
        ("atoi", nh!(h_atoi)),
        ("atol", nh!(h_atol)),
        ("atof", nh!(h_atof)),
        ("strtoul", nh!(h_strtoul)),
        ("strtod", nh!(h_strtod)),
        ("getenv", wh!(h_getenv)),
        ("abort", nh!(h_abort)),
        ("exit", nh!(h_exit)),
        ("_exit", nh!(h_exit)),
        ("_errno", nh!(h_errno)),
        ("signal", nh!(h_signal)),
        ("rand", wh!(h_rand)),
        ("srand", wh!(h_srand)),
        ("clock", nh!(h_clock)),
        ("time", nh!(h_time)),
        ("_tzset", nh!(h_tzset)),
        ("tzset", nh!(h_tzset)),
    ]
}

//! Розширена поверхня: CRT-IO (_open/_read/_write), 64-бітні мат-хелпери
//! msvcrt (_aulldiv...), user32/advapi стаби, _beginthreadex,
//! і вхід у SEH — _CxxThrowException.

use super::crt::{cstr, cstr_w, read_u64, write_u8, write_u64, VarArgs};
use super::api::{World, Handle};
use super::runtime::{win_call, Handler};

// ============================== CRT-IO (int-fd) ==============================

unsafe fn host_open(path: &str, oflag: u64) -> i32 {
    let host = path.replace('\\', "/");
    let host = if host.len() >= 2 && host.as_bytes()[1] == b':' {
        host[2..].to_string()
    } else {
        host
    };
    let mut flags = 0;
    if oflag & 0x0001 != 0 {
        flags |= libc::O_WRONLY;
    }
    if oflag & 0x0002 != 0 {
        flags |= libc::O_RDWR;
    }
    if oflag & 0x0100 != 0 {
        flags |= libc::O_CREAT;
    }
    if oflag & 0x0200 != 0 {
        flags |= libc::O_TRUNC;
    }
    if oflag & 0x0800 != 0 {
        flags |= libc::O_APPEND;
    }
    if flags == 0 {
        flags = libc::O_RDONLY;
    }
    let c = std::ffi::CString::new(host).unwrap_or_default();
    unsafe { libc::open(c.as_ptr(), flags | libc::O_CLOEXEC, 0o644) }
}

unsafe fn h__open(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr(a0) };
    let fd = host_open(&p, a1);
    (fd as i64) as u64
}

unsafe fn h__wopen(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr_w(a0) };
    let fd = host_open(&p, a1);
    (fd as i64) as u64
}

unsafe fn h__close(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::close(a0 as i32) }) as u64
}

unsafe fn h__read(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::read(a0 as i32, a1 as *mut _, a2 as usize) }) as u64
}

unsafe fn h__write(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::write(a0 as i32, a1 as *const _, a2 as usize) }) as u64
}

unsafe fn h__lseek(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::lseek(a0 as i32, a1 as i64, a2 as i32) }) as u64
}

unsafe fn h__tell(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::lseek(a0 as i32, 0, libc::SEEK_CUR) }) as u64
}

unsafe fn h__unlink(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr(a0) }.replace('\\', "/");
    std::fs::remove_file(p).is_ok() as u64
}

unsafe fn h__mkdir(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr(a0) }.replace('\\', "/");
    std::fs::create_dir_all(p).is_ok() as u64
}

unsafe fn h__rmdir(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr(a0) }.replace('\\', "/");
    std::fs::remove_dir(p).is_ok() as u64
}

unsafe fn h__access(a0: u64, _a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let p = unsafe { cstr(a0) }.replace('\\', "/");
    let p = p.trim_start_matches('/');
    std::fs::metadata(if p.is_empty() { "." } else { p }).is_ok() as u64
}

unsafe fn h__getcwd(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".into());
    let b = cwd.as_bytes();
    if a0 != 0 && a1 as usize > b.len() {
        unsafe {
            std::ptr::copy_nonoverlapping(b.as_ptr(), a0 as *mut u8, b.len());
            write_u8(a0 + b.len() as u64, 0);
        }
        a0
    } else {
        0
    }
}

unsafe fn h__setmode(_a0: u64, _a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0 // O_TEXT
}

unsafe fn h__commit(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { libc::fsync(a0 as i32) == 0 }) as u64
}

unsafe fn h__fileno(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    let fd = super::crt::iob_fd(w, a0);
    fd as u64
}

// ============================== 64-бітні мат-хелпери ==============================

unsafe fn h__aulldiv(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a1 == 0 {
        return 0;
    }
    a0 / a1
}

unsafe fn h__aullrem(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a1 == 0 {
        return 0;
    }
    a0 % a1
}

unsafe fn h__alldiv(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let x = a0 as i64;
    let y = a1 as i64;
    if y == 0 {
        return 0;
    }
    (x / y) as u64
}

unsafe fn h__allrem(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let x = a0 as i64;
    let y = a1 as i64;
    if y == 0 {
        return 0;
    }
    (x % y) as u64
}

unsafe fn h__allmul(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    ((a0 as i128) * (a1 as i128)) as u64
}

unsafe fn h__chkstk(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    // probing не потрібен: стек 8 МБ уже відображений
    a0
}

// ============================== строки/конверти CRT ==============================

unsafe fn h__stricmp(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let a = unsafe { cstr(a0) }.to_lowercase();
    let b = unsafe { cstr(a1) }.to_lowercase();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => u64::MAX,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

unsafe fn h__strnicmp(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let n = a2 as usize;
    let a = unsafe { cstr(a0) }.to_lowercase();
    let b = unsafe { cstr(a1) }.to_lowercase();
    for k in 0..n {
        let x = a.as_bytes().get(k).copied().unwrap_or(0);
        let y = b.as_bytes().get(k).copied().unwrap_or(0);
        if x != y || x == 0 {
            return (x as i64 - y as i64) as u64;
        }
    }
    0
}

unsafe fn h__wcsicmp(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let a = unsafe { cstr_w(a0) }.to_lowercase();
    let b = unsafe { cstr_w(a1) }.to_lowercase();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => u64::MAX,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

unsafe fn h__itoa(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let v = a0 as i64;
    let s = match a2 {
        16 => format!("{v:x}"),
        8 => format!("{v:o}"),
        2 => format!("{v:b}"),
        _ => format!("{v}"),
    };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a1 as *mut u8, s.len());
        write_u8(a1 + s.len() as u64, 0);
    }
    a1
}

unsafe fn h__ui64toa(a0: u64, a1: u64, a2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = match a2 {
        16 => format!("{a0:x}"),
        8 => format!("{a0:o}"),
        2 => format!("{a0:b}"),
        _ => format!("{a0}"),
    };
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a1 as *mut u8, s.len());
        write_u8(a1 + s.len() as u64, 0);
    }
    a1
}

unsafe fn h__atoi64(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    (unsafe { cstr(a0).trim().parse::<i64>().unwrap_or(0) }) as u64
}

unsafe fn h_setlocale(_a0: u64, _a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    // "C" — живе вічно
    static LOCALE: &[u8] = b"C\0";
    LOCALE.as_ptr() as u64
}

unsafe fn h_strerror(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    static ERR: &[u8] = b"error\0";
    ERR.as_ptr() as u64
}

unsafe fn h__lock(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h__unlock(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h__amsg_exit(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    eprintln!("winpe: CRT runtime error R{a0}");
    std::process::exit(255);
}

unsafe fn h__purecall(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    eprintln!("winpe: pure virtual call");
    std::process::exit(255);
}

unsafe fn h__XcptFilter(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0 // EXCEPTION_CONTINUE_SEARCH
}

unsafe fn h__controlfp(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0x0008_003F // _MCW_EM (усі винятки вимкнені — як за замовчуванням)
}

unsafe fn h__fpreset(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h__setmbcs(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h__ismbblead(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

// ============================== потоки CRT ==============================

unsafe fn h__beginthread(_a0: u64, _a1: u64, a2: u64, a3: u64, _4: u64, _5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // (start, stack, arg)
    begin_thread(w, a2, a3, 0)
}

unsafe fn h__beginthreadex(_a0: u64, _a1: u64, a2: u64, a3: u64, _a4: u64, _a5: u64, _e: u64, _c: u64, w: &World) -> u64 {
    // (sa, stack, start, arg, initflag, *tid)
    begin_thread(w, a2, a3, 0)
}

fn begin_thread(w: &World, start: u64, param: u64, _flags: u64) -> u64 {
    let image_tebs = w.with_inner(|inner| {
        inner
            .image
            .as_ref()
            .map(|_im| super::runtime::thread_teb_new())
    });
    let Some(Ok(teb)) = image_tebs else { return 0 };
    let packet = Box::into_raw(Box::new(super::api::ExitPacket {
        done: std::sync::atomic::AtomicBool::new(false),
        code: std::sync::atomic::AtomicU32::new(0),
        join: std::sync::Mutex::new(None),
    }));
    let _pkt = packet;
    {
        let pkt = packet as usize;
        let jh = std::thread::spawn(move || {
            let pkt = pkt as *mut super::api::ExitPacket;
            unsafe {
                libc::syscall(libc::SYS_arch_prctl, 0x1001, teb);
            }
            let r = win_call(start, param, 0, 0, 0, 0).unwrap_or(0);
            unsafe {
                (*pkt).code.store(r as u32, std::sync::atomic::Ordering::Relaxed);
                (*pkt).done.store(true, std::sync::atomic::Ordering::Release);
                let addr = &(*pkt).done as *const _ as usize;
                libc::syscall(libc::SYS_futex, addr as *const i32, 1, i32::MAX);
            }
        });
        let handle = w.insert_handle(Handle::Thread { done: packet });
        if let Some(h) = w
            .with_handle(handle, |hd| {
                if let Handle::Thread { done, .. } = hd {
                    Some(*done as *const super::api::ExitPacket)
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
}

unsafe fn h__endthreadex(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    thread_local! {
        static CODE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    CODE.with(|c| c.set(a0 as u32));
    0
}

// ============================== SEH-входи ==============================

unsafe fn h__CxxThrowException(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, entry_rsp: u64, caller_rip: u64, w: &World) -> u64 {
    // (pExceptionObject, ThrowInfo) — при успіху не повертається
    // міст збережав rbp/rdi/rsi кидача на [E−8],[E−16],[E−24]
    let saved_rbp = unsafe { read_u64(entry_rsp - 8) };
    let saved_rdi = unsafe { read_u64(entry_rsp - 16) };
    let saved_rsi = unsafe { read_u64(entry_rsp - 24) };
    super::seh::cxx_throw(a0, a1, w, caller_rip, entry_rsp, saved_rbp, saved_rdi, saved_rsi)
}

unsafe fn h___CxxFrameHandler3(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    // викликається тільки реальною ОС; у нас свій walker
    0
}

// ============================== user32/advapi ==============================

unsafe fn h_MessageBoxW(_a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    eprintln!("[winpe MessageBox] {}", unsafe { cstr_w(a1) });
    1 // IDOK
}

unsafe fn h_wsprintfA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, entry_rsp: u64, _c: u64) -> u64 {
    let fmt = unsafe { cstr(a1) };
    let mut va = VarArgs::new(0, 0, 0, 0, 0, 0, entry_rsp);
    let mut out = String::new();
    unsafe { super::crt::format(&fmt, &mut va, &mut out) };
    let b = out.as_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), a0 as *mut u8, b.len());
        write_u8(a0 + b.len() as u64, 0);
    }
    b.len() as u64
}

unsafe fn h_CharNextA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if unsafe { super::crt::read_u8(a0) } == 0 {
        a0
    } else {
        a0 + 1
    }
}

unsafe fn h_CharUpperA(a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    unsafe {
        let mut p = a0;
        loop {
            let c = super::crt::read_u8(p);
            if c == 0 {
                break;
            }
            super::crt::write_u8(p, c.to_ascii_uppercase());
            p += 1;
        }
    }
    a0
}

unsafe fn h_RegOpenKeyExA(_a0: u64, _a1: u64, _a2: u64, _a3: u64, a4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    if a4 != 0 {
        unsafe { write_u64(a4, 0x8000_0001) }; // фейковий HKEY
    }
    0 // ERROR_SUCCESS
}

unsafe fn h_RegQueryValueExA(_a0: u64, _a1: u64, _a2: u64, a3: u64, a4: u64, a5: u64, _e: u64, _c: u64) -> u64 {
    // (*type, data, *len) — порожнє значення
    if a3 != 0 {
        unsafe { write_u64(a3, 1) }; // REG_SZ
    }
    if a5 != 0 {
        unsafe { write_u64(a5, 0) };
        let _ = a4;
    }
    2 // ERROR_FILE_NOT_FOUND
}

unsafe fn h_RegCloseKey(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

unsafe fn h_GetUserNameA(a0: u64, a1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let s = b"root\0";
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), a0 as *mut u8, s.len());
        write_u64(a1, 5);
    }
    1
}

unsafe fn h_SHGetFolderPathW(_a0: u64, _a1: u64, _a2: u64, _a3: u64, a4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    let u: Vec<u16> = "C:\\Users\\root".encode_utf16().collect();
    unsafe {
        std::ptr::copy_nonoverlapping(u.as_ptr(), a4 as *mut u16, u.len());
        write_u8(a4 + u.len() as u64 * 2, 0);
    }
    0 // S_OK
}

unsafe fn h_GetKeyState(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

// ============================== Реєстр ==============================

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

pub fn registry_ext() -> Vec<(&'static str, Handler)> {
    vec![
        // CRT-IO
        ("_open", nh!(h__open)),
        ("_wopen", nh!(h__wopen)),
        ("_close", nh!(h__close)),
        ("_read", nh!(h__read)),
        ("_write", nh!(h__write)),
        ("_lseek", nh!(h__lseek)),
        ("_lseeki64", nh!(h__lseek)),
        ("_tell", nh!(h__tell)),
        ("_telli64", nh!(h__tell)),
        ("_unlink", nh!(h__unlink)),
        ("_mkdir", nh!(h__mkdir)),
        ("_rmdir", nh!(h__rmdir)),
        ("_access", nh!(h__access)),
        ("_waccess", nh!(h__access)),
        ("_getcwd", nh!(h__getcwd)),
        ("_wgetcwd", nh!(h__getcwd)),
        ("_setmode", nh!(h__setmode)),
        ("_commit", nh!(h__commit)),
        ("_fileno", wh!(h__fileno)),
        // 64-бітні хелпери
        ("_aulldiv", nh!(h__aulldiv)),
        ("_aullrem", nh!(h__aullrem)),
        ("_alldiv", nh!(h__alldiv)),
        ("_allrem", nh!(h__allrem)),
        ("_allmul", nh!(h__allmul)),
        ("_chkstk", nh!(h__chkstk)),
        ("__chkstk", nh!(h__chkstk)),
        // рядки
        ("_stricmp", nh!(h__stricmp)),
        ("stricmp", nh!(h__stricmp)),
        ("_strnicmp", nh!(h__strnicmp)),
        ("_wcsicmp", nh!(h__wcsicmp)),
        ("_itoa", nh!(h__itoa)),
        ("_i64toa", nh!(h__itoa)),
        ("_ui64toa", nh!(h__ui64toa)),
        ("_atoi64", nh!(h__atoi64)),
        ("strncpy_s", nh!(h__lseek_dummy_zero)),
        ("strcpy_s", nh!(h__lseek_dummy_zero)),
        ("setlocale", nh!(h_setlocale)),
        ("strerror", nh!(h_strerror)),
        ("_lock", nh!(h__lock)),
        ("_unlock", nh!(h__unlock)),
        ("_amsg_exit", nh!(h__amsg_exit)),
        ("_purecall", nh!(h__purecall)),
        ("_XcptFilter", nh!(h__XcptFilter)),
        ("__XcptFilter", nh!(h__XcptFilter)),
        ("_controlfp", nh!(h__controlfp)),
        ("__control87_2", nh!(h__controlfp)),
        ("_fpreset", nh!(h__fpreset)),
        ("_setmbcs", nh!(h__setmbcs)),
        ("_ismbblead", nh!(h__ismbblead)),
        // потоки
        ("_beginthread", wh!(h__beginthread)),
        ("_beginthreadex", wh!(h__beginthreadex)),
        ("_endthread", nh!(h__endthreadex)),
        ("_endthreadex", nh!(h__endthreadex)),
        // SEH
        ("_CxxThrowException", wh!(h__CxxThrowException)),
        ("__CxxThrowException", wh!(h__CxxThrowException)),
        ("__CxxFrameHandler", nh!(h___CxxFrameHandler3)),
        ("__CxxFrameHandler2", nh!(h___CxxFrameHandler3)),
        ("__CxxFrameHandler3", nh!(h___CxxFrameHandler3)),
        ("__CxxFrameHandler4", nh!(h___CxxFrameHandler3)),
        // user32/advapi/shell
        ("MessageBoxA", nh!(h_MessageBoxW)),
        ("MessageBoxW", nh!(h_MessageBoxW)),
        ("wsprintfA", nh!(h_wsprintfA)),
        ("wsprintfW", nh!(h_wsprintfA)),
        ("CharNextA", nh!(h_CharNextA)),
        ("CharUpperA", nh!(h_CharUpperA)),
        ("CharUpperBuffA", nh!(h_CharUpperA)),
        ("RegOpenKeyExA", nh!(h_RegOpenKeyExA)),
        ("RegOpenKeyExW", nh!(h_RegOpenKeyExA)),
        ("RegQueryValueExA", nh!(h_RegQueryValueExA)),
        ("RegQueryValueExW", nh!(h_RegQueryValueExA)),
        ("RegCloseKey", nh!(h_RegCloseKey)),
        ("GetUserNameA", nh!(h_GetUserNameA)),
        ("GetUserNameW", nh!(h_GetUserNameA)),
        ("SHGetFolderPathW", nh!(h_SHGetFolderPathW)),
        ("SHGetFolderPathA", nh!(h_SHGetFolderPathW)),
        ("GetKeyState", nh!(h_GetKeyState)),
    ]
}

unsafe fn h__lseek_dummy_zero(_a0: u64, _1: u64, _2: u64, _3: u64, _4: u64, _5: u64, _e: u64, _c: u64) -> u64 {
    0
}

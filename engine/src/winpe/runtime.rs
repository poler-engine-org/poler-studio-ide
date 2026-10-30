//! Сердце Win64-субстрата: відображення образу, IAT-патчинг з тunk-сторінками,
//! TEB/PEB через GS-базу, asm-міст SysV↔Win64, стеки з slack-зонами.
//!
//! Вивчені інваріанти (війна по 7za):
//!  * id передається через R10 — volatile в обох ABI (RDI/RSI non-volatile у Win64!)
//!  * тunk → міст: абсолютний `mov rax, imm64; jmp rax` (rel32 не долітає через >2ГБ)
//!  * над вершиною КОЖНОГО стеку, який ми віддаємо, має бути відображений slack
//!    (entry читає [rsp+0x10], тunki пишуть shadow-простір [rsp+8..0x28])
//!  * дані-імпорти (_commode, _fmode, _iob...) → IAT містить адресу ЗПИСУВАНОЇ комірки
//!  * усі записи в пам'ять payload — write_unaligned (unaligned легальний на Windows)

use super::pe::{self, PeInfo};
use std::cell::Cell;

/// Повна сигнатура шима: win-аргументи + entry_rsp (varargs: warg6+ на [rsp+0x38..])
/// + caller_rip (SEH: пошук кадру кидка).
pub type Handler = unsafe fn(
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    entry_rsp: u64,
    caller_rip: u64,
) -> u64;

/// Ідентифікатор тunk-виклику для поки невідомих символів: лог + нуль.
pub const ID_UNKNOWN_BASE: u64 = 0x7000_0000;

unsafe fn mmapping(len: usize, prot: i32) -> Result<*mut u8, String> {
    let p = libc::mmap(
        std::ptr::null_mut(),
        len,
        prot,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        -1,
        0,
    );
    if p == libc::MAP_FAILED {
        return Err(format!(
            "mmap({len:#x}): {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(p as *mut u8)
}

pub struct ThunkPages {
    pages: Vec<*mut u8>,
    page_size: usize,
    used: usize,
}

impl ThunkPages {
    pub fn new() -> Result<Self, String> {
        let ps = 0x10000;
        let p = unsafe { mmapping(ps, libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC)? };
        Ok(Self {
            pages: vec![p],
            page_size: ps,
            used: 0,
        })
    }

    /// Емітує тunk: mov r10, imm64(id); mov rax, imm64(bridge); jmp rax — 22 байти.
    pub unsafe fn emit(&mut self, id: u64, bridge: u64) -> Result<u64, String> {
        const SZ: usize = 24; // 22 байти + вирівнювання
        if self.used + SZ > self.page_size {
            let p = mmapping(
                self.page_size,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            )?;
            self.pages.push(p);
            self.used = 0;
        }
        let page = *self.pages.last().unwrap();
        let at = unsafe { page.add(self.used) };
        let mut code = [0u8; 22];
        code[0] = 0x49;
        code[1] = 0xBA; // mov r10, imm64
        code[2..10].copy_from_slice(&id.to_le_bytes());
        code[10] = 0x48;
        code[11] = 0xB8; // mov rax, imm64
        code[12..20].copy_from_slice(&bridge.to_le_bytes());
        code[20] = 0xFF;
        code[21] = 0xE0; // jmp rax
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), at, 22) };
        let addr = at as u64;
        self.used += SZ;
        Ok(addr)
    }
}

/// Сторінка RW-комірок для імпортованих ДАНИХ-символів (_commode, _fmode, _iob, environ...).
pub struct DataPage {
    base: *mut u8,
    size: usize,
    used: usize,
}

impl DataPage {
    pub fn new() -> Result<Self, String> {
        let size = 0x10000;
        let base = unsafe { mmapping(size, libc::PROT_READ | libc::PROT_WRITE)? };
        Ok(Self { base, size, used: 0 })
    }

    /// Комірка заданого розміру (вирівнювання 8).
    pub unsafe fn cell(&mut self, size: usize, init: &[u8]) -> Result<u64, String> {
        let need = size.div_ceil(8) * 8;
        if self.used + need > self.size {
            return Err("data page переповнено".into());
        }
        let at = unsafe { self.base.add(self.used) };
        unsafe { std::ptr::write_bytes(at, 0, need) };
        if !init.is_empty() {
            unsafe { std::ptr::copy_nonoverlapping(init.as_ptr(), at, init.len().min(need)) };
        }
        self.used += need;
        Ok(at as u64)
    }
}

thread_local! {
    /// Скретч-стек цього потоку для зворотних викликів у PE-код
    /// (qsort-компаратори, деструктори SEH, catch-хендлери, thread-proc).
    static SCRATCH_TOP: Cell<u64> = const { Cell::new(0) };
}

/// Виділяє скретч-стек: 128 КБ, top = end − 512 (slack зверху — ОБОВ'ЯЗКОВО).
pub fn scratch_top() -> Result<u64, String> {
    let cur = SCRATCH_TOP.get();
    if cur != 0 {
        return Ok(cur);
    }
    let size = 128 * 1024;
    let base = unsafe { mmapping(size, libc::PROT_READ | libc::PROT_WRITE)? } as u64;
    let top = base + size as u64 - 512; // slack над вершиною
    SCRATCH_TOP.set(top);
    Ok(top)
}

/// Новий TEB для потоку (CreateThread): власний стек-діапазон, той самий PEB.
pub fn thread_teb_new() -> Result<u64, String> {
    unsafe {
        let teb = mmapping(0x1000, libc::PROT_READ | libc::PROT_WRITE)? as u64;
        let stack = mmapping(1024 * 1024, libc::PROT_READ | libc::PROT_WRITE)? as u64;
        // наслідуємо PEB з головного TEB (GS:[0x30]->Self->[0x60]) — але ми ще
        // не в тому потоці; читаємо зі світу
        let peb = super::api::World::get().with_inner(|w| w.peb_shared);
        std::ptr::write_unaligned((teb + 0x08) as *mut u64, stack + 1024 * 1024);
        std::ptr::write_unaligned((teb + 0x10) as *mut u64, stack);
        std::ptr::write_unaligned((teb + 0x30) as *mut u64, teb); // Self
        std::ptr::write_unaligned((teb + 0x60) as *mut u64, peb);
        std::ptr::write_unaligned(
            (teb + 0x40) as *mut u64,
            libc::syscall(libc::SYS_gettid) as u64,
        );
        Ok(teb)
    }
}

// ============================== ASM-шар ==============================

std::arch::global_asm! {
    // ГРАБЛЯ №48 (v0.92.0): без явного `.text` код шара проваливается в
    // секцию, оставшуюся активной в объекте на момент ассемблирования
    // (диагностировано: символы жили в `.note.GNU-stack`) — линк падал
    // при любой перетасовке CGU/DCE, годами сходило с рук лишь потому,
    // что ссылающийся код выпиливался. `.text` первой строкой гарантирует
    // секцию кода ВСЕГДА.
    ".text",
    // ---- МІСТ Win64 → SysV ----
    // Вхід: тunk стрибнув сюди з R10 = id; RCX,RDX,R8,R9 = win-аргументи,
    // [rsp] = адресу повернення в PE (E), [E+0x28+] = win стек-аргументи.
    // RDI/RSI — non-volatile у Win64 (volatile у SysV) → зберігаємо на [E−16],[E−24].
    ".globl winpe_bridge",
    ".type winpe_bridge, @function",
    "winpe_bridge:",
    "  push rbp",                   // [E−8]  = rbp кидача (SEH читає)
    "  mov  rbp, rsp",
    "  push rdi",                   // [E−16] = rdi кидача
    "  push rsi",                   // [E−24] = rsi кидача;  rsp = E−24
    "  push rax",                   // [E−32] ВИРІВНЮВАЛЬНИЙ ПАД (найвищий — сміття)
    "  mov  rax, [rsp+0x20]",       // caller_rip = [E]            (E−32+0x20=E)
    "  push rax",                   // [E−40] arg9 caller_rip
    "  lea  rax, [rsp+0x28]",       // entry_rsp = E               (E−40+0x28=E)
    "  push rax",                   // [E−48] arg8 entry_rsp
    "  mov  rax, [rsp+0x60]",       // warg5 = [E+0x30]            (E−48+0x60=E+0x30)
    "  push rax",                   // [E−56] arg7 warg5 → rsp = E−56 ≡ 0 (mod 16)
    "  mov  rdi, r10",              // SysV arg0 = id
    "  mov  rsi, rcx",              // arg1 = a0
    "  mov  rcx, r8",               // arg3 = a2
    "  mov  r8,  r9",               // arg4 = a3
    "  mov  r9,  [rsp+0x60]",       // arg5 = warg4 = [E+0x28]     (E−56+0x60=E+0x28)
    "  call winpe_dispatch_c",      // (id, a0..a5, warg5, entry_rsp, caller_rip)
    "  add  rsp, 32",               // пад + arg9..arg7
    "  pop  rsi",
    "  pop  rdi",
    "  pop  rbp",
    "  ret",

    // ---- ЗВОРОТНИЙ ВИКЛИК SysV → Win64 ----
    // win_call64(target, scratch_top, a0, a1, a2, a3, a4) -> u64
    ".globl win_call64",
    ".type win_call64, @function",
    "win_call64:",
    "  push rbp",
    "  mov  rbp, rsp",
    "  mov  rax, rdi",              // target
    "  mov  r10, rsi",              // scratch_top
    "  mov  r11, [rbp+16]",         // a4 (5-й win-арг, стековий)
    "  xchg rdx, rcx",              // rdx=a1, rcx=a0 (r8/r9 уже на місці)
    "  mov  rsp, r10",              // скретч-стек (slack зверху є)
    "  sub  rsp, 0x30",             // shadow(32)+arg5-слот: PE-entry+0x28 = S+0x20
    "  mov  [rsp+0x20], r11",
    "  call rax",
    "  mov  rsp, rbp",
    "  pop  rbp",
    "  ret",

    // ---- ВИКЛИК Win64 НА ПОТОЧНОМУ СТЕЦІ (ланцюг кадрів PE не обривається!) ----
    // win_call_here(target, a0, a1, a2, a3) -> u64 — для _initterm/qsort:
    // повернення constructor-а ляже на той самий стек.
    ".globl win_call_here",
    ".type win_call_here, @function",
    "win_call_here:",
    "  push rbp",
    "  mov  rbp, rsp",
    "  sub  rsp, 0x20",             // shadow-простір калі-функції
    "  mov  rax, rdi",              // target
    "  mov  r9,  r8",               // a3
    "  mov  r8,  rcx",              // a2
    "  mov  rcx, rsi",              // a0
    "  call rax",
    "  mov  rsp, rbp",
    "  pop  rbp",
    "  ret",

    // ---- ВХІД ПРОЦЕСУ ----
    ".globl win_entry64",
    ".type win_entry64, @function",
    "win_entry64:",
    "  mov  rsp, rsi",              // [rsp]=exit_stub, [rsp+8]=PEB
    "  jmp  rdi",

    // ---- STUB виходу: PE-entry зробив ret ----
    ".globl winpe_exit_stub",
    ".type winpe_exit_stub, @function",
    "winpe_exit_stub:",
    "  mov  rdi, rax",              // код виходу
    "  call winpe_exit_impl",
    "  ud2",

    // ---- Захоплення живих регістрів (SEH: RBX/R12-15 кидача) ----
    // capture_regs(out: *mut [u64;5])
    ".globl winpe_capture_regs",
    ".type winpe_capture_regs, @function",
    "winpe_capture_regs:",
    "  mov [rdi+0],  rbx",
    "  mov [rdi+8],  r12",
    "  mov [rdi+16], r13",
    "  mov [rdi+24], r14",
    "  mov [rdi+32], r15",
    "  ret",

    // ---- Фінальний перехід SEH: regs[15] = {rbx,rbp,rdi,rsi,r12..r15@12} ----
    // win_continue(rip, rsp, regs)
    ".globl win_continue",
    ".type win_continue, @function",
    "win_continue:",
    "  mov  rbx, [rdx+0]",
    "  mov  rbp, [rdx+8]",
    "  mov  rdi, [rdx+16]",
    "  mov  rsi, [rdx+24]",
    "  mov  r12, [rdx+48]",
    "  mov  r13, [rdx+56]",
    "  mov  r14, [rdx+64]",
    "  mov  r15, [rdx+72]",
    "  mov  rsp, rsi",
    "  jmp  rdi",

    // ---- Адресні аксесори ----
    ".globl winpe_bridge_addr",
    ".type winpe_bridge_addr, @function",
    "winpe_bridge_addr:",
    "  lea rax, [rip + winpe_bridge]",
    "  ret",
    ".globl winpe_exit_stub_addr",
    ".type winpe_exit_stub_addr, @function",
    "winpe_exit_stub_addr:",
    "  lea rax, [rip + winpe_exit_stub]",
    "  ret",
}

extern "C" {
    fn winpe_bridge_addr() -> u64;
    #[link_name = "win_call_here"]
    fn win_call_here_asm(target: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64;
    fn winpe_exit_stub_addr() -> u64;
    #[link_name = "winpe_capture_regs"]
    fn capture_regs_asm(out: *mut u64);
    #[link_name = "win_call64"]
    fn win_call64_asm(
        target: u64,
        scratch_top: u64,
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        a4: u64,
    ) -> u64;
    #[link_name = "win_entry64"]
    fn win_entry64_asm(entry: u64, entry_rsp: u64) -> u64;
    #[link_name = "win_continue"]
    fn win_continue_asm(rip: u64, rsp: u64, regs: *const u64) -> u64;
}

pub fn bridge_addr() -> u64 {
    unsafe { winpe_bridge_addr() }
}

/// Захоплення RBX/R12-R15 (живі = значення PE-кода, що викликав шим).
pub fn capture_regs() -> [u64; 5] {
    let mut out = [0u64; 5];
    unsafe { capture_regs_asm(out.as_mut_ptr()) };
    out
}

/// Виклик Win64-функції образу з Rust (на скретч-стеку цього потоку).
pub fn win_call(target: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> Result<u64, String> {
    let top = scratch_top()?;
    Ok(unsafe { win_call64_asm(target, top, a0, a1, a2, a3, a4) })
}

/// Виклик Win64-функції НА ПОТОЧНОМУ стеці (зберігає ланцюг кадрів PE).
pub fn win_call_here(target: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    unsafe { win_call_here_asm(target, a0, a1, a2, a3) }
}

/// Виклик Win64-функції на ВКАЗАНОМУ скретч-стеку (SEH-розгортання передає свій).
pub fn win_call_on(target: u64, rsp: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> u64 {
    unsafe { win_call64_asm(target, rsp, a0, a1, a2, a3, a4) }
}

/// Фінальний стрибок SEH у catch-продовження з живими регістрами кадру.
pub fn win_continue(rip: u64, rsp: u64, regs: &[u64; 16]) -> u64 {
    unsafe { win_continue_asm(rip, rsp, regs.as_ptr()) }
}

// ============================== Substrate ==============================

pub struct Substrate {
    pub info: PeInfo,
    pub image: *mut u8,
    pub teb: u64,
    pub peb: u64,
    pub params: u64,
    pub main_stack_top: u64,
    pub cmd_line_ptr: u64,
}

impl Substrate {
    /// Відображає образ, патчить IAT (тunki через World), будує TEB/PEB/стек.
    /// Тunk/дані-сторінки оселяються у World (для пізнього GetProcAddress).
    pub unsafe fn load(image_bytes: &[u8], world: &super::api::World) -> Result<Self, String> {
        let info = pe::parse(image_bytes)?;
        let image =
            unsafe { mmapping(info.size_of_image as usize, libc::PROT_READ | libc::PROT_WRITE)? };
        unsafe { std::ptr::write_bytes(image, 0, info.size_of_image as usize) };
        let hdr = (info.size_of_headers as usize).min(image_bytes.len());
        unsafe { std::ptr::copy_nonoverlapping(image_bytes.as_ptr(), image, hdr) };
        for s in &info.sections {
            let n = (s.raw_size as usize).min(
                image_bytes
                    .len()
                    .saturating_sub(s.raw_pointer as usize),
            );
            if n > 0 {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        image_bytes.as_ptr().add(s.raw_pointer as usize),
                        image.add(s.virtual_address as usize),
                        n,
                    )
                };
            }
        }

        // --- релокації: абсолютні покажчики → фактична база ---
        let delta = (image as i64).wrapping_sub(info.image_base as i64);
        if delta != 0 {
            let n = pe::apply_relocs(image_bytes, &info, image, delta)?;
            if n > 0 {
                eprintln!("winpe: релокацій застосовано: {n} (Δ={delta:#x})");
            }
        }

        world.init_thunks()?;

        // --- IAT ---
        let bridge = bridge_addr();
        let imports = info.imports(image_bytes)?;
        let mut n_code = 0u64;
        let mut n_data = 0u64;
        for imp in imports {
            let slot = unsafe { image.add(imp.iat_rva as usize) } as *mut u64;
            if imp.is_data {
                // Дані: IAT = адреса ЗПИСУВАНОЇ комірки (не тunk!)
                let cell = world.data_cell_for(&imp.name)?;
                unsafe { std::ptr::write_unaligned(slot, cell) };
                n_data += 1;
            } else {
                let id = world.ensure_symbol(&imp.dll, &imp.name);
                let thunk = world.thunk_emit(id, bridge)?;
                unsafe { std::ptr::write_unaligned(slot, thunk) };
                n_code += 1;
            }
        }

        // ДІАГНОСТИКА: перші слоти + адреса моста
        if std::env::var("POLER_WIN_DEBUG").is_ok() {
            let bridge = bridge_addr();
            eprintln!("winpe-debug: bridge={bridge:#x}");
            for rva in [0xdf438u32, 0xdf450, 0xdf468] {
                let v = unsafe {
                    std::ptr::read_unaligned(image.add(rva as usize) as *const u64)
                };
                eprintln!("winpe-debug: слот {rva:#x} = {v:#x}");
                if v != 0 {
                    let code = unsafe { std::slice::from_raw_parts(v as *const u8, 22) };
                    eprintln!("winpe-debug:   тunk-байти: {:02x?}", code);
                }
            }
        }

        // --- права секцій: текст RX, решта RW ---
        for s in &info.sections {
            if s.virtual_address == 0 {
                continue;
            }
            let prot = if s.is_exec() {
                libc::PROT_READ | libc::PROT_EXEC
            } else {
                libc::PROT_READ | libc::PROT_WRITE
            };
            let sz = s.virtual_size.max(s.raw_size).div_ceil(0x1000) * 0x1000;
            unsafe {
                libc::mprotect(image.add(s.virtual_address as usize) as *mut _, sz as usize, prot)
            };
        }

        // --- образ у World (SEH-walker шукає .pdata звідти) ---
        let rfs = info.runtime_functions(image_bytes);
        let n_rfs = rfs.len();
        world.set_image(image as u64, info.clone(), rfs);

        // --- стек процесу: 8 МБ + 4 КБ slack зверху ---
        let stack_size = 8 * 1024 * 1024;
        let stack_base =
            unsafe { mmapping(stack_size, libc::PROT_READ | libc::PROT_WRITE)? } as u64;
        let stack_top = stack_base + stack_size as u64 - 0x1000;

        let teb = unsafe { mmapping(0x1000, libc::PROT_READ | libc::PROT_WRITE)? } as u64;
        let peb = unsafe { mmapping(0x1000, libc::PROT_READ | libc::PROT_WRITE)? } as u64;
        let params = unsafe { mmapping(0x2000, libc::PROT_READ | libc::PROT_WRITE)? } as u64;

        eprintln!(
            "winpe: образ {:#x} ({:#x} байт), секцій {}, імпортів: {n_code} код + {n_data} даних, .pdata {} RF",
            image as u64, info.size_of_image, info.sections.len(), n_rfs
        );

        Ok(Self {
            info,
            image,
            teb,
            peb,
            params,
            main_stack_top: stack_top,
            cmd_line_ptr: params + 0x1000,
        })
    }

    /// Заповнює TEB/PEB/ProcessParameters і стек входу.
    pub unsafe fn setup_teb(&mut self, world: &super::api::World) -> Result<(), String> {
        unsafe fn w64(at: u64, off: u64, v: u64) {
            std::ptr::write_unaligned((at + off) as *mut u64, v);
        }
        unsafe fn w32(at: u64, off: u64, v: u32) {
            std::ptr::write_unaligned((at + off) as *mut u32, v);
        }
        unsafe fn w16(at: u64, off: u64, v: u16) {
            std::ptr::write_unaligned((at + off) as *mut u16, v);
        }

        // ---- командний рядок: Unicode + ANSI (ОКРЕМІ буфери!) ----
        let argv = world.with_inner(|w| w.argv.clone());
        let mut cmd_utf16: Vec<u16> = Vec::new();
        let mut cmd_ansi: Vec<u8> = Vec::new();
        for (i, a) in argv.iter().enumerate() {
            if i > 0 {
                cmd_utf16.push(b' ' as u16);
                cmd_ansi.push(b' ');
            }
            cmd_utf16.extend(a.encode_utf16());
            cmd_ansi.extend_from_slice(a.as_bytes());
        }
        cmd_utf16.push(0);
        cmd_ansi.push(0);
        let cmd_ptr = self.cmd_line_ptr; // UTF-16
        let ansi_ptr = self.cmd_line_ptr + 0x800; // ANSI
        unsafe {
            std::ptr::copy_nonoverlapping(cmd_utf16.as_ptr(), cmd_ptr as *mut u16, cmd_utf16.len());
            std::ptr::copy_nonoverlapping(cmd_ansi.as_ptr(), ansi_ptr as *mut u8, cmd_ansi.len());
        };

        // TEB (x64)
        let teb = self.teb;
        unsafe {
            w64(teb, 0x00, 0); // ExceptionList
            w64(teb, 0x08, self.main_stack_top); // StackBase
            w64(teb, 0x10, self.main_stack_top - 8 * 1024 * 1024); // StackLimit
            w64(teb, 0x30, teb); // Self ← NtCurrentTeb
            w64(teb, 0x38, std::process::id() as u64); // ClientId.ProcessId
            w64(teb, 0x40, libc::syscall(libc::SYS_gettid) as u64); // ThreadId
            w64(teb, 0x60, self.peb); // ProcessEnvironmentBlock
            w64(teb, 0x68, 0); // LastErrorValue
        }

        // PEB
        let peb = self.peb;
        unsafe {
            w64(peb, 0x10, self.image as u64); // ImageBaseAddress
            w64(peb, 0x20, self.params); // ProcessParameters
            let ncpu = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(2) as u64;
            w64(peb, 0xB8, ncpu); // NumberOfProcessors
            w32(peb, 0xBC, 0); // NtGlobalFlag
            w32(peb, 0x118, 10); // OSMajorVersion
            w32(peb, 0x11C, 0); // OSMinorVersion
        }
        world.set_peb_shared(self.peb);

        // RTL_USER_PROCESS_PARAMETERS
        let p = self.params;
        unsafe {
            w32(p, 0x00, 0x2000); // MaximumLength
            w32(p, 0x04, 0x2000); // Length
            w32(p, 0x08, 1); // Flags
            w64(p, 0x20, 0); // StandardInput
            w64(p, 0x28, 1); // StandardOutput
            w64(p, 0x30, 2); // StandardError
            w16(p, 0x38, 4); // CurrentDirectory.DosPath len
            w16(p, 0x38 + 2, 8); // maxlen
            w64(p, 0x38 + 8, cmd_ptr); // буфер "C:\\"
            w16(p, 0x60, (cmd_utf16.len() * 2) as u16); // ImagePathName.len
            w16(p, 0x60 + 2, (cmd_utf16.len() * 2 + 2) as u16);
            w64(p, 0x60 + 8, cmd_ptr);
            w16(p, 0x70, (cmd_utf16.len() * 2) as u16); // CommandLine.len
            w16(p, 0x70 + 2, (cmd_utf16.len() * 2 + 2) as u16);
            w64(p, 0x70 + 8, cmd_ptr);
            let envb = world.with_inner(|w| w.env_block_unicode);
            w64(p, 0x80, envb); // Environment
        }

        // --- стек входу: [rsp]=exit_stub, [rsp+8]=PEB, [rsp+0x10]=0, [rsp+0x18]=0 ---
        let entry_rsp = self.main_stack_top - 0x100; // slack зверху для параметрів
        let entry_rsp = (entry_rsp & !0xF) + 8; // ≡ 8 (mod 16) — конвенція входу
        let stub = unsafe { winpe_exit_stub_addr() };
        unsafe {
            w64(entry_rsp, 0x00, stub);
            w64(entry_rsp, 0x08, self.peb);
            w64(entry_rsp, 0x10, 0);
            w64(entry_rsp, 0x18, 0);
        }
        self.main_stack_top = entry_rsp;

        // покажчики: GetCommandLineW → UTF-16, GetCommandLineA → ANSI
        world.set_cmd_lines(cmd_ptr, ansi_ptr);
        Ok(())
    }

    /// GS-база на ЦЬОМУ потоці + перехід у entry. Не повертається.
    pub fn enter(&self) -> ! {
        unsafe {
            libc::syscall(libc::SYS_arch_prctl, 0x1001 /* ARCH_SET_GS */, self.teb);
            let entry = (self.image as u64) + self.info.entry_rva as u64;
            win_entry64_asm(entry, self.main_stack_top);
        }
        unreachable!("win_entry не повертається")
    }
}

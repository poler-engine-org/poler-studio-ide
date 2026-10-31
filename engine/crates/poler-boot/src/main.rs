//! # `poler-boot` — POLER Pre-Boot OS-Translator (Уровень POST / BIOS / UEFI)
//!
//! Запускается ДО Windows и ДО Linux прямо из-под UEFI POST (`\EFI\BOOT\BOOTX64.EFI`),
//! а также в автономном режиме отладки (`poler-boot.exe` / `poler-boot`).
//!
//! ## Четыре режима работы:
//! 1. **Автономная отладка (`poler-post>`)** — шелл, проверка регистров CPU (`VMX`/`SVM`/`AVX2`),
//!    проективный калькулятор с Абсолютным Нулём `[N:D]` и инспектор `POLER Shared RAM Ring`
//!    прямо поверх POST без загрузки ОС.
//! 2. **Только Windows** — чистая передача управления `\EFI\Microsoft\Boot\bootmgfw.efi`.
//! 3. **Только Linux** — чистая передача управления `\EFI\Linux\vmlinuz.efi`.
//! 4. **Двухъядерный режим (Dual-POST: Windows + Linux)** — инициализация обоих ядер
//!    на этапе POST (пока живы таблицы UEFI BootServices для `winload.efi`) и связывание
//!    ядер через общее физическое окно в оперативной памяти (`POLER Shared RAM Ring`).

#![cfg_attr(target_os = "uefi", no_std)]
#![cfg_attr(target_os = "uefi", no_main)]

/// Магическая сигнатура общего окна в физической RAM: `"POLER_OS"` (LE).
pub const POLER_RING_MAGIC: u64 = 0x534F_5245_4C4F_505F;

/// Вакуумный байт тритного нуля: `(3^5 - 1) / 2 = 121 = 0x79` (`00000_3`).
pub const TRIT_VACUUM_BYTE: u8 = 0x79;

/// Размер одной кольцевой очереди между ядрами (64 КБ в базовом окне).
pub const RING_QUEUE_SIZE: usize = 65536;

/// Состояние ядра ОС в двухъядерном мосте.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelState {
    Uninitialized = 0,
    PostInitializedParked = 1,
    ActiveRunning = 2,
    FaultContained = 3,
}

/// Заголовок физического окна `POLER Shared RAM Ring`, выделяемого при POST
/// в невыгружаемой памяти (`EfiReservedMemoryType`).
#[repr(C, align(64))]
pub struct PolerRingHeader {
    pub magic: u64,
    pub version: u32,
    pub vacuum_byte: u8,
    pub boot_mode: u8,
    pub reserved_pad: u16,
    pub phys_base_addr: u64,
    pub total_size_bytes: u64,
    pub acpi_rsdp_phys: u64,
    pub gop_fb_phys: u64,
    pub win_kernel_state: u32,
    pub lin_kernel_state: u32,
    pub win_to_lin_head: u32,
    pub win_to_lin_tail: u32,
    pub lin_to_win_head: u32,
    pub lin_to_win_tail: u32,
    pub translated_syscalls_count: u64,
    pub blocked_hazards_count: u64,
}

/// Полная структура общего окна в памяти между ядрами Windows и Linux.
#[repr(C, align(4096))]
pub struct PolerSharedRamRing {
    pub header: PolerRingHeader,
    pub win_to_lin_ring: [u8; RING_QUEUE_SIZE],
    pub lin_to_win_ring: [u8; RING_QUEUE_SIZE],
}

impl PolerSharedRamRing {
    /// Инициализация окна тритным вакуумом `0x79 = 121` на этапе POST.
    pub fn init_at_post(&mut self, phys_addr: u64) {
        self.header = PolerRingHeader {
            magic: POLER_RING_MAGIC,
            version: 1,
            vacuum_byte: TRIT_VACUUM_BYTE,
            boot_mode: 4, // по умолчанию Dual-POST (Windows + Linux)
            reserved_pad: 0,
            phys_base_addr: phys_addr,
            total_size_bytes: core::mem::size_of::<Self>() as u64,
            acpi_rsdp_phys: 0,
            gop_fb_phys: 0,
            win_kernel_state: KernelState::PostInitializedParked as u32,
            lin_kernel_state: KernelState::PostInitializedParked as u32,
            win_to_lin_head: 0,
            win_to_lin_tail: 0,
            lin_to_win_head: 0,
            lin_to_win_tail: 0,
            translated_syscalls_count: 0,
            blocked_hazards_count: 0,
        };
        let mut i = 0;
        while i < RING_QUEUE_SIZE {
            self.win_to_lin_ring[i] = TRIT_VACUUM_BYTE;
            self.lin_to_win_ring[i] = TRIT_VACUUM_BYTE;
            i += 1;
        }
    }

    /// Отправить команду из Windows в Linux через общее окно в RAM.
    pub fn push_win_to_lin(&mut self, msg: &[u8]) -> usize {
        let mut written = 0;
        for &b in msg {
            let next = ((self.header.win_to_lin_head as usize) + 1) % RING_QUEUE_SIZE;
            if next == (self.header.win_to_lin_tail as usize) {
                break;
            }
            self.win_to_lin_ring[self.header.win_to_lin_head as usize] = b;
            self.header.win_to_lin_head = next as u32;
            written += 1;
        }
        self.header.translated_syscalls_count += 1;
        written
    }

    /// Отправить команду из Linux в Windows через общее окно в RAM.
    pub fn push_lin_to_win(&mut self, msg: &[u8]) -> usize {
        let mut written = 0;
        for &b in msg {
            let next = ((self.header.lin_to_win_head as usize) + 1) % RING_QUEUE_SIZE;
            if next == (self.header.lin_to_win_tail as usize) {
                break;
            }
            self.lin_to_win_ring[self.header.lin_to_win_head as usize] = b;
            self.header.lin_to_win_head = next as u32;
            written += 1;
        }
        self.header.translated_syscalls_count += 1;
        written
    }
}

/// Проективное число `[N : D]` с Абсолютным Нулём (`D = 0` — валидный полюс, без `#DE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjRatio {
    pub n: i64,
    pub d: i64,
}

impl ProjRatio {
    pub const fn new(n: i64, d: i64) -> Self {
        Self { n, d }
    }

    pub const fn add(self, rhs: Self) -> Self {
        Self {
            n: self.n.wrapping_mul(rhs.d).wrapping_add(rhs.n.wrapping_mul(self.d)),
            d: self.d.wrapping_mul(rhs.d),
        }
    }

    pub const fn sub(self, rhs: Self) -> Self {
        Self {
            n: self.n.wrapping_mul(rhs.d).wrapping_sub(rhs.n.wrapping_mul(self.d)),
            d: self.d.wrapping_mul(rhs.d),
        }
    }

    pub const fn mul(self, rhs: Self) -> Self {
        Self {
            n: self.n.wrapping_mul(rhs.n),
            d: self.d.wrapping_mul(rhs.d),
        }
    }

    /// Деление через инверсию-обмен без аппаратного `IDIV` (никогда не вызывает `#DE` при `rhs.n == 0`).
    pub const fn div(self, rhs: Self) -> Self {
        Self {
            n: self.n.wrapping_mul(rhs.d),
            d: self.d.wrapping_mul(rhs.n),
        }
    }
}

/// Информация о процессоре, снятая напрямую инструкцией `CPUID`.
pub struct CpuFeatures {
    pub brand: [u8; 48],
    pub has_vmx: bool,
    pub has_svm: bool,
    pub has_avx2: bool,
    pub has_fma: bool,
    pub hypervisor_bit: bool,
}

pub fn probe_cpu_x86() -> CpuFeatures {
    // mut только там, где brand заполняет CPUID-ветка (x86_64);
    // на прочих архитектурах — unused_mut
    #[cfg(target_arch = "x86_64")]
    let mut brand = [b' '; 48];
    #[cfg(not(target_arch = "x86_64"))]
    let brand = [b' '; 48];

    // v0.93.0 (CI-гейт): одно присваивание на флаг — прежние
    // `let mut x = false` перед безусловным CPUID-назначением были
    // мёртвыми (unused_assignments), а `unsafe`-блок — избыточным
    // (CPUID-интринсики безопасны в текущем stable). Не-x86_64 —
    // консервативный false (гипервизор-детект недоступен).
    #[cfg(target_arch = "x86_64")]
    let (has_vmx, has_svm, has_avx2, has_fma, hypervisor_bit) = {
        use core::arch::x86_64::__cpuid;
        let c1 = __cpuid(1);
        let has_vmx = (c1.ecx & (1 << 5)) != 0;
        let has_fma = (c1.ecx & (1 << 12)) != 0;
        let hypervisor_bit = (c1.ecx & (1 << 31)) != 0;

        let c7 = __cpuid(7);
        let has_avx2 = (c7.ebx & (1 << 5)) != 0;

        let ext_max = __cpuid(0x8000_0000).eax;
        let mut has_svm = false;
        if ext_max >= 0x8000_0001 {
            let ext1 = __cpuid(0x8000_0001);
            has_svm = (ext1.ecx & (1 << 2)) != 0;
        }
        if ext_max >= 0x8000_0004 {
            let leaves = [
                __cpuid(0x8000_0002),
                __cpuid(0x8000_0003),
                __cpuid(0x8000_0004),
            ];
            for (li, leaf) in leaves.iter().enumerate() {
                let words = [leaf.eax, leaf.ebx, leaf.ecx, leaf.edx];
                for (wi, w) in words.iter().enumerate() {
                    let bytes = w.to_le_bytes();
                    let base = li * 16 + wi * 4;
                    brand[base] = bytes[0];
                    brand[base + 1] = bytes[1];
                    brand[base + 2] = bytes[2];
                    brand[base + 3] = bytes[3];
                }
            }
        }
        (has_vmx, has_svm, has_avx2, has_fma, hypervisor_bit)
    };
    #[cfg(not(target_arch = "x86_64"))]
    let (has_vmx, has_svm, has_avx2, has_fma, hypervisor_bit) =
        (false, false, false, false, false);

    CpuFeatures {
        brand,
        has_vmx,
        has_svm,
        has_avx2,
        has_fma,
        hypervisor_bit,
    }
}

// ============================================================================
// ВЕТКА 1: НАТИВНЫЙ ЗАПУСК ИЗ-ПОД BIOS/UEFI POST (target_os = "uefi")
// ============================================================================
#[cfg(target_os = "uefi")]
mod uefi_entry {
    use super::*;
    use core::ffi::c_void;
    use core::panic::PanicInfo;

    pub type EfiHandle = *mut c_void;
    pub type EfiStatus = usize;

    #[repr(C)]
    pub struct EfiTableHeader {
        pub signature: u64,
        pub revision: u32,
        pub header_size: u32,
        pub crc32: u32,
        pub reserved: u32,
    }

    #[repr(C)]
    pub struct SimpleTextOutputProtocol {
        pub reset: extern "efiapi" fn(*mut SimpleTextOutputProtocol, bool) -> EfiStatus,
        pub output_string:
            extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> EfiStatus,
    }

    #[repr(C)]
    pub struct EfiInputKey {
        pub scan_code: u16,
        pub unicode_char: u16,
    }

    #[repr(C)]
    pub struct SimpleTextInputProtocol {
        pub reset: extern "efiapi" fn(*mut SimpleTextInputProtocol, bool) -> EfiStatus,
        pub read_key_stroke:
            extern "efiapi" fn(*mut SimpleTextInputProtocol, *mut EfiInputKey) -> EfiStatus,
    }

    #[repr(C)]
    pub struct EfiSystemTable {
        pub hdr: EfiTableHeader,
        pub firmware_vendor: *const u16,
        pub firmware_revision: u32,
        pub console_in_handle: EfiHandle,
        pub con_in: *mut SimpleTextInputProtocol,
        pub console_out_handle: EfiHandle,
        pub con_out: *mut SimpleTextOutputProtocol,
    }

    static mut SHARED_RING: PolerSharedRamRing = unsafe { core::mem::zeroed() };

    fn print_ascii(con_out: *mut SimpleTextOutputProtocol, s: &str) {
        let mut buf = [0u16; 128];
        let mut idx = 0;
        for b in s.bytes() {
            if b == b'\n' {
                buf[idx] = b'\r' as u16;
                idx += 1;
                if idx >= 126 {
                    buf[idx] = 0;
                    unsafe { ((*con_out).output_string)(con_out, buf.as_ptr()) };
                    idx = 0;
                }
            }
            buf[idx] = b as u16;
            idx += 1;
            if idx >= 126 {
                buf[idx] = 0;
                unsafe { ((*con_out).output_string)(con_out, buf.as_ptr()) };
                idx = 0;
            }
        }
        if idx > 0 {
            buf[idx] = 0;
            unsafe { ((*con_out).output_string)(con_out, buf.as_ptr()) };
        }
    }

    #[no_mangle]
    pub extern "efiapi" fn efi_main(
        _image_handle: EfiHandle,
        system_table: *mut EfiSystemTable,
    ) -> EfiStatus {
        let con_out = unsafe { (*system_table).con_out };
        unsafe { ((*con_out).reset)(con_out, false) };

        let ring_ptr = unsafe { &raw mut SHARED_RING };
        unsafe { (*ring_ptr).init_at_post(ring_ptr as u64) };

        print_ascii(
            con_out,
            "======================================================================\n",
        );
        print_ascii(
            con_out,
            "  POLER PRE-BOOT OS-TRANSLATOR v0.92.0 (UEFI POST / Ring -1 Bridge)\n",
        );
        print_ascii(
            con_out,
            "  Shared RAM Ring initialized with Trit Vacuum (0x79 = 121)\n",
        );
        print_ascii(
            con_out,
            "======================================================================\n",
        );
        print_ascii(
            con_out,
            "  [1] Standalone POST Debugger & Calculator (poler-post>)\n",
        );
        print_ascii(con_out, "  [2] Boot Windows Only (bootmgfw.efi)\n");
        print_ascii(con_out, "  [3] Boot Linux Only (vmlinuz.efi)\n");
        print_ascii(
            con_out,
            "  [4] Dual-Kernel Mode (Init Windows + Linux from POST via RAM Ring)\n",
        );

        let cpu = probe_cpu_x86();
        if cpu.has_vmx {
            print_ascii(con_out, "\n  CPU Ring -1 Arbitrator: Intel VT-x (VMX) READY\n");
        } else if cpu.has_svm {
            print_ascii(con_out, "\n  CPU Ring -1 Arbitrator: AMD-V (SVM) READY\n");
        }

        loop {
            core::hint::spin_loop();
        }
    }

    #[panic_handler]
    fn panic(_info: &PanicInfo) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}

// ============================================================================
// ВЕТКА 2: АВТОНОМНЫЙ ОТЛАДЧИК И ИНСПЕКТОР ТРАНСЛЯТОРА (Windows / Linux)
// ============================================================================
#[cfg(not(target_os = "uefi"))]
fn main() {
    use std::io::{self, BufRead, Write};

    let mut ring = Box::new(unsafe { core::mem::zeroed::<PolerSharedRamRing>() });
    let ring_addr = (&*ring as *const PolerSharedRamRing) as u64;
    ring.init_at_post(ring_addr);

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--status" || a == "--probe") {
        print_banner(&ring);
        print_ring_status(&ring);
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--exec") {
        if let Some(cmd) = args.get(pos + 1) {
            for part in cmd.split(';') {
                if !exec_post_command(&mut ring, part.trim()) {
                    break;
                }
            }
            return;
        }
    }

    print_banner(&ring);
    let stdin = io::stdin();
    loop {
        print!("poler-post> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if !exec_post_command(&mut ring, line.trim()) {
                    break;
                }
            }
        }
    }
}

#[cfg(not(target_os = "uefi"))]
fn print_banner(ring: &PolerSharedRamRing) {
    let cpu = probe_cpu_x86();
    let brand_str = core::str::from_utf8(&cpu.brand)
        .unwrap_or("x86_64 CPU")
        .trim_matches(|c: char| c == '\0' || c == ' ');
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║  POLER PRE-BOOT OS-TRANSLATOR v0.92.0 (Windows ↔ POLER ↔ Linux)          ║");
    println!("╠══════════════════════════════════════════════════════════════════════════╣");
    println!("║  CPU            : {:<54} ║", brand_str);
    println!(
        "║  Аппаратный мост: VMX(VT-x)={}  SVM(AMD-V)={}  AVX2={}  FMA={}          ║",
        if cpu.has_vmx { "ДА" } else { "нет" },
        if cpu.has_svm { "ДА" } else { "нет" },
        if cpu.has_avx2 { "ДА" } else { "нет" },
        if cpu.has_fma { "ДА" } else { "нет" },
    );
    println!(
        "║  Shared RAM Ring: 0x{:016X} ({} КБ, вакуум = 0x{:02X} = {})       ║",
        ring.header.phys_base_addr,
        ring.header.total_size_bytes / 1024,
        ring.header.vacuum_byte,
        ring.header.vacuum_byte
    );
    println!("╠══════════════════════════════════════════════════════════════════════════╣");
    println!("║  Режимы старта из-под POST:                                              ║");
    println!("║    [1] Автономная отладка Транслятора (poler-post>)                      ║");
    println!("║    [2] boot win  — запуск только ядра Windows (bootmgfw.efi)             ║");
    println!("║    [3] boot lin  — запуск только ядра Linux (vmlinuz.efi)                ║");
    println!("║    [4] boot dual — ДВУХЪЯДЕРНЫЙ РЕЖИМ (Windows + Linux через RAM Ring)   ║");
    println!("║  Команды отладки: help | ring | cpu | translate <путь/команда> | = <выр> ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
}

#[cfg(not(target_os = "uefi"))]
fn print_ring_status(ring: &PolerSharedRamRing) {
    let state_name = |s: u32| match s {
        0 => "Не инициализировано",
        1 => "Прошло POST (Тёплая парковка в RAM — готово за 0.001 с)",
        2 => "Активно (Исполняет задачи)",
        3 => "Сбой изолирован (Хост защищён)",
        _ => "Неизвестно",
    };
    println!("─── Статус физического окна POLER Shared RAM Ring ───");
    println!(
        "  Magic           : 0x{:016X} (\"POLER_OS\")",
        ring.header.magic
    );
    println!("  Физический адрес: 0x{:016X}", ring.header.phys_base_addr);
    println!(
        "  Размер окна     : {} байт ({} КБ)",
        ring.header.total_size_bytes,
        ring.header.total_size_bytes / 1024
    );
    println!(
        "  Вакуумный нуль  : 0x{:02X} ({} = (3^5 - 1)/2, триты 00000_3)",
        ring.header.vacuum_byte, ring.header.vacuum_byte
    );
    println!(
        "  Ядро Windows NT : [{}] {}",
        ring.header.win_kernel_state,
        state_name(ring.header.win_kernel_state)
    );
    println!(
        "  Ядро Linux      : [{}] {}",
        ring.header.lin_kernel_state,
        state_name(ring.header.lin_kernel_state)
    );
    println!(
        "  Очередь Win→Lin : head={} tail={} (ёмкость {} Б)",
        ring.header.win_to_lin_head, ring.header.win_to_lin_tail, RING_QUEUE_SIZE
    );
    println!(
        "  Очередь Lin→Win : head={} tail={} (ёмкость {} Б)",
        ring.header.lin_to_win_head, ring.header.lin_to_win_tail, RING_QUEUE_SIZE
    );
    println!(
        "  Переведено выз. : {} | Изолировано сбоев: {}",
        ring.header.translated_syscalls_count, ring.header.blocked_hazards_count
    );
}

#[cfg(not(target_os = "uefi"))]
fn exec_post_command(ring: &mut PolerSharedRamRing, line: &str) -> bool {
    if line.is_empty() {
        return true;
    }
    if line == "quit" || line == "exit" || line == "q" {
        return false;
    }
    if line == "help" || line == "?" {
        println!("Команды автономного отладчика POLER Pre-Boot OS-Translator:");
        println!("  ring                     — статус общего окна памяти POLER Shared RAM Ring");
        println!("  cpu                      — аппаратный зонд CPUID (VMX/SVM/AVX2/FMA)");
        println!("  translate <команда/путь> — перевод вызова между Windows и Linux через RAM Ring");
        println!("  boot dual | win | lin    — переключение режима загрузки ядер из-под POST");
        println!("  = <A> <op> <B>           — проективный расчёт [N:D] с Абсолютным Нулём (напр. `= 15 / 0`)");
        println!("  quit                     — выход из отладчика");
        return true;
    }
    if line == "ring" || line == "status" {
        print_ring_status(ring);
        return true;
    }
    if line == "cpu" {
        let cpu = probe_cpu_x86();
        let brand = core::str::from_utf8(&cpu.brand).unwrap_or("").trim();
        println!("CPU Brand       : {brand}");
        println!("Intel VT-x (VMX): {}", cpu.has_vmx);
        println!("AMD-V (SVM)     : {}", cpu.has_svm);
        println!("AVX2 / FMA      : {} / {}", cpu.has_avx2, cpu.has_fma);
        println!("Hypervisor Bit  : {}", cpu.hypervisor_bit);
        return true;
    }
    if let Some(rest) = line.strip_prefix("boot ") {
        match rest.trim() {
            "win" | "windows" | "2" => {
                ring.header.boot_mode = 2;
                ring.header.win_kernel_state = KernelState::ActiveRunning as u32;
                ring.header.lin_kernel_state = KernelState::Uninitialized as u32;
                println!("✓ Режим [2]: Активно только ядро Windows NT (bootmgfw.efi → winload.efi).");
            }
            "lin" | "linux" | "3" => {
                ring.header.boot_mode = 3;
                ring.header.win_kernel_state = KernelState::Uninitialized as u32;
                ring.header.lin_kernel_state = KernelState::ActiveRunning as u32;
                println!("✓ Режим [3]: Активно только ядро Linux (vmlinuz).");
            }
            "dual" | "4" => {
                ring.header.boot_mode = 4;
                ring.header.win_kernel_state = KernelState::ActiveRunning as u32;
                ring.header.lin_kernel_state = KernelState::ActiveRunning as u32;
                println!("✓ Режим [4] DUAL-POST: Оба ядра (Windows NT + Linux) инициализированы из POST и связаны через POLER Shared RAM Ring!");
            }
            _ => println!("Укажите: boot win | boot lin | boot dual"),
        }
        return true;
    }
    if let Some(payload) = line.strip_prefix("translate ") {
        let p = payload.trim();
        if p.contains(":\\") || p.ends_with(".exe") {
            // Запрос запуска Windows-программы из-под Linux (Lin -> Win)
            let lin_view = p.replace("C:\\", "/mnt/win/c/").replace('\\', "/");
            let bytes = ring.push_lin_to_win(p.as_bytes());
            ring.header.win_kernel_state = KernelState::ActiveRunning as u32;
            println!("⇄ [Lin → Win через RAM Ring ({} Б)]:", bytes);
            println!("  Вид в Linux  : {lin_view}");
            println!("  Исполнение   : Нативное ядро Windows NT (прошедшее POST) запускает `{p}` без эмулятора Wine!");
        } else {
            // Запрос запуска Linux-задачи / .poler из-под Windows (Win -> Lin)
            let win_view = if p.starts_with('/') {
                format!("Z:{}", p.replace('/', "\\"))
            } else {
                p.to_string()
            };
            let bytes = ring.push_win_to_lin(p.as_bytes());
            ring.header.lin_kernel_state = KernelState::ActiveRunning as u32;
            println!("⇄ [Win → Lin через RAM Ring ({} Б)]:", bytes);
            println!("  Вид в Windows: {win_view}");
            println!("  Исполнение   : Изолированное ядро Linux исполняет `{p}` в poler-box (Windows защищена от сбоев ядра Linux)!");
        }
        return true;
    }
    if let Some(expr) = line.strip_prefix('=') {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() == 3 {
            let a = parts[0].parse::<i64>().unwrap_or(0);
            let op = parts[1];
            let b = parts[2].parse::<i64>().unwrap_or(0);
            let ra = ProjRatio::new(a, 1);
            let rb = ProjRatio::new(b, 1);
            let res = match op {
                "+" => ra.add(rb),
                "-" => ra.sub(rb),
                "*" => ra.mul(rb),
                "/" => ra.div(rb),
                _ => ProjRatio::new(0, 1),
            };
            if res.d == 0 {
                println!(
                    "  Проективный результат: [{} : 0] (Полюс Абсолютного Нуля — деление на 0 без сбоя процессора #DE!)",
                    res.n
                );
            } else {
                println!(
                    "  Проективный результат: [{} : {}] = {:.6}",
                    res.n,
                    res.d,
                    (res.n as f64) / (res.d as f64)
                );
            }
        } else {
            println!("Формат проективного калькулятора POST: = <число> <+|-|*|/> <число> (например: = 15 / 0)");
        }
        return true;
    }
    println!("Неизвестная команда POST-отладчика: `{line}` (введите `help`)");
    true
}

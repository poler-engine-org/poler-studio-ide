# POLER-OS Phase 1: Boot + HAL — Zig Rewrite Plan

> Версия: 0.5.0-alpha  
> Цель: Загрузка x86_64 через Multiboot2, переход в long mode, базовый HAL  
> Язык: Zig 0.16.0  
> Черновик: Linux v6.6 LTS (67 файлов, 46,612 строк)  
> Ожидаемый результат: ~2,750 строк Zig

---

## 0. Стратегия пакетных менеджеров (влияет на ELF Adapter)

### Принцип: НЕ пишем пакетные менеджеры — запускаем бинарники через ELF Adapter

```
┌───────────────────────────────────────────────────┐
│           Package Manager Binaries                 │
│  pacman │ pip │ conda │ cargo │ npm │ flatpak     │
├───────────────────────────────────────────────────┤
│           ELF Adapter (3-layer)                    │
│  ┌──────────────┬──────────────┬───────────────┐  │
│  │ 80% direct   │ 15% semantic │ 5% emulate    │  │
│  │ Linux syscall│ Intent map   │ full emulate   │  │
│  │ → POLER equiv│ open→Intent  │ ioctl→state   │  │
│  └──────────────┴──────────────┴───────────────┘  │
├───────────────────────────────────────────────────┤
│           POLER Intent Layer                       │
├───────────────────────────────────────────────────┤
│           POLER-OS Kernel (Zig)                    │
└───────────────────────────────────────────────────┘
```

### Приоритеты поддержки (по сложности для ELF Adapter):

| P | Менеджер | Syscall surface | Сложность адаптации | Примечание |
|---|----------|-----------------|---------------------|------------|
| 0 | **conda/miniconda** | Минимальный | 🟢 Легко | Self-contained, минимум Linux-специфики |
| 0 | **pip** | Средний | 🟡 Средне | Нужен Python runtime |
| 1 | **pacman/AUR** | Большой | 🔴 Сложно | fork/exec, chroot, /proc |
| 1 | **cargo** | Малый | 🟢 Легко | Rust runtime = тонкий слой |
| 2 | **npm** | Средний | 🟡 Средне | Node.js + V8 |
| 2 | **flatpak** | Большой | 🔴 Сложно | namespace/cgroup |
| 3 | **brew** | Малый | 🟢 Легко | Git-based |
| 3 | **nix/guix** | Большой | 🔴 Сложно | Функциональная модель |

### Почему conda/miniconda — P0:
1. **Self-contained** — тащит свой Python, свои библиотеки
2. **Минимум экзотических syscalls** — mmap, open, read, write, fstat
3. **Изолированные среды** — `conda create -n ai-env` = sandbox
4. **Критично для AI** — AI-агенты работают в conda environments
5. **conda → mamba** (C++ реализация) ещё проще для ELF Adapter

### Что это значит для Phase 1:
- ELF Adapter design влияет на **типы Intent** которые мы определяем
- Intent{FS, Open/Read/Write/Close} — минимум для conda
- Intent{Proc, Fork/Exec} — нужен для pacman
- Intent{Net, Connect/Listen} — нужен для всех (скачивание пакетов)
- Intent{Mem, Mmap/Munmap} — нужен для всех
- **Phase 1 определяет базовые Intent типы**, ELF Adapter — Phase 4

---

## 1. Структура файлов Phase 1

```
poler-os/zig-kernel/src/
├── boot/
│   ├── multiboot2.zig       # Multiboot2 header + info parsing
│   ├── entry64.S            # 32→64 bit transition (naked asm)
│   └── init.c               # C shim if needed for early boot
├── hal/
│   ├── gdt.zig              # GDT setup (64-bit)
│   ├── idt.zig              # IDT setup + exception handlers
│   ├── idt_handlers.S       # Assembly stubs for IDT entries
│   ├── pic.zig              # 8259A PIC driver
│   ├── pit.zig              # Programmable Interval Timer
│   ├── serial.zig           # COM1 serial port (debug)
│   ├── cpu.zig              # CPU feature detection (CPUID)
│   ├── msr.zig              # MSR read/write
│   └── ports.zig            # I/O port in/out wrappers
├── mm/
│   ├── pmm.zig              # Physical memory manager (buddy?)
│   ├── vmm.zig              # Virtual memory manager (page tables)
│   ├── e820.zig             # E820 memory map parsing
│   └── heap.zig             # Kernel heap (slab?)
├── acpi/
│   ├── rsdp.zig             # RSDP scanning
│   ├── tables.zig           # RSDT/XSDT parsing
│   └── madt.zig             # MADT (LAPIC/IOAPIC) parsing
├── drivers/
│   ├── vga.zig              # VGA text mode (existing)
│   ├── fb.zig               # Framebuffer (existing)
│   ├── keyboard.zig         # PS/2 keyboard (existing)
│   └── virtio.zig           # VirtIO (existing)
├── poler/
│   ├── intent.zig           # Intent struct + dispatcher
│   ├── object.zig           # Object Table
│   └── poler_core.zig       # POLER Firewall (existing)
├── debug/
│   ├── log.zig              # Kernel logging (serial + FB)
│   └── panic.zig            # Kernel panic handler
├── main.zig                 # Kernel main (replaces main32.zig)
└── linker64.ld              # 64-bit linker script
```

---

## 2. Последовательность загрузки POLER-OS v0.5.0

```
GRUB/QEMU (-kernel)
    │
    ▼
entry64.S: startup_32 (Multiboot2 загружает в 32-bit protected mode)
    │  1. Проверить Multiboot2 magic (0x36d76289 в EAX)
    │  2. Сохранить multiboot2 info pointer (EBX)
    │  3. Запретить прерывания (CLI)
    │  4. Настроить временный GDT (null + code + data)
    │  5. Включить PAE (CR4.PAE)
    │  6. Настроить identity-mapped PML4 (4 страницы)
    │  7. Записать CR3
    │  8. Включить long mode (EFER.LME via MSR 0xC0000080)
    │  9. Включить paging (CR0.PG)
    │  10. Far jump → startup_64 (64-bit!)
    ▼
startup_64:
    │  11. Настроить 64-bit segment registers
    │  12. Настроить stack (из linker symbol)
    │  13. Вызвать poler_kernel_main(multiboot2_info)
    ▼
poler_kernel_main() [Zig]:
    │  14. Инициализация serial (debug output)
    │  15. Парсинг Multiboot2 info → E820 map
    │  16. Настройка final GDT (code32, code64, data, tss)
    │  17. Настройка IDT (exceptions + IRQ stubs)
    │  18. Настройка PIC (remap IRQ0→32, IRQ8→40)
    │  19. STI (разрешить прерывания!)
    │  20. Инициализация PMM (physical memory manager)
    │  21. Сканирование ACPI (RSDP → RSDT → MADT)
    │  22. Инициализация PIT (system timer)
    │  23. Инициализация framebuffer (если Multiboot2 дал)
    │  24. Инициализация POLER Intent Layer
    │  25. Запуск shell (interactive)
    ▼
POLER-OS Shell> _
```

---

## 3. Ключевые Zig структуры

### 3.1 Intent (базовый тип — влияет на ELF Adapter дизайн)

```zig
pub const IntentCategory = enum(u8) {
    FS = 0x01,
    NET = 0x02,
    PROC = 0x03,
    MEM = 0x04,
    HW = 0x05,
    IPC = 0x06,
    SYS = 0x07,
};

pub const IntentAction = enum(u16) {
    // FS
    Open = 0x0101,
    Read = 0x0102,
    Write = 0x0103,
    Close = 0x0104,
    Stat = 0x0105,
    Mmap = 0x0106,
    Munmap = 0x0107,
    // NET
    Connect = 0x0201,
    Listen = 0x0202,
    Accept = 0x0203,
    Send = 0x0204,
    Recv = 0x0205,
    // PROC
    Fork = 0x0301,
    Exec = 0x0302,
    Wait = 0x0303,
    Exit = 0x0304,
    Clone = 0x0305,
    // MEM
    Alloc = 0x0401,
    Free = 0x0402,
    Protect = 0x0403,
    // HW
    IrqRegister = 0x0501,
    IrqUnregister = 0x0502,
    IoPort = 0x0503,
    // SYS
    Log = 0x0701,
    Panic = 0x0702,
    Reboot = 0x0703,
};

pub const Intent = extern struct {
    category: IntentCategory,
    action: IntentAction,
    params: [6]u64,        // Универсальные параметры
    caller_id: u32,        // Object Table ID
    timestamp: u64,        // TSC
    nonce: u64,            // POLER Firewall nonce
    // Total: 72 bytes
};
```

### 3.2 E820 Entry

```zig
pub const E820Type = enum(u32) {
    RAM = 1,
    Reserved = 2,
    ACPI = 3,
    NVS = 4,
    Unusable = 5,
    ACPIReclaim = 6,
};

pub const E820Entry = extern struct {
    addr: u64,
    size: u64,
    entry_type: E820Type,
};

pub const E820Table = struct {
    entries: [128]E820Entry,
    count: usize,
};
```

### 3.3 GDT Entry (64-bit)

```zig
pub const GDTEntry = packed struct {
    limit_low: u16,
    base_low: u24,
    access: u8,
    limit_high: u4,
    flags: u4,
    base_high: u8,
};

pub const GDT64 = packed struct {
    null: u64 = 0,
    code32: u64,    // For 32-bit compatibility
    code64: u64,    // 64-bit code segment
    data: u64,      // Data segment
    tss_low: u64,   // TSS low 8 bytes
    tss_high: u64,  // TSS high 8 bytes (64-bit TSS)
};
```

### 3.4 IDT Entry (64-bit)

```zig
pub const IDTEntry = packed struct {
    offset_low: u16,       // Bits 0-15
    selector: u16,         // Code segment selector
    ist: u3,               // IST offset (0 = don't use)
    reserved: u5 = 0,
    type: u4,              // Gate type
    dpl: u2,               // Descriptor privilege level
    present: u1,
    offset_mid: u16,       // Bits 16-31
    offset_high: u32,      // Bits 32-63
    reserved2: u32 = 0,
};

pub const IDTPointer = packed struct {
    limit: u16,
    base: u64,
};
```

---

## 4. План реализации по шагам

### Шаг 1: entry64.S — 32→64 Transition (самый важный!)
**Оценка: ~200 строк ассемблера**
- Изучить: `boot/compressed/head_64.S` строки 56-300
- Реализовать:
  - Multiboot2 entry point
  - Temporary GDT
  - Identity-mapped page tables (2MB pages = только PML4 + 1 PDPT + 1 PD)
  - Enable PAE → Long Mode → Paging
  - Far jump to 64-bit code

### Шаг 2: multiboot2.zig — Info Parsing
**Оценка: ~150 строк Zig**
- Изучить: `boot/header.S`, Multiboot2 spec
- Реализовать:
  - Parse multiboot2 info structure
  - Extract memory map (→ E820 equivalent)
  - Extract framebuffer info
  - Extract command line

### Шаг 3: hal/gdt.zig + hal/idt.zig — Descriptor Tables
**Оценка: ~250 строк Zig**
- Изучить: `kernel/idt.c`, `include_asm/desc.h`, `include_asm/desc_defs.h`
- Реализовать:
  - Full 64-bit GDT (null, code32, code64, data, TSS)
  - IDT with 256 entries
  - Exception handlers (0-31)
  - IRQ stubs (32-47)
  - IDT load routine

### Шаг 4: hal/pic.zig + hal/pit.zig — Interrupt Controllers
**Оценка: ~200 строк Zig**
- Реализовать:
  - 8259A PIC remap (IRQ0-15 → INT 32-47)
  - PIC mask/unmask
  - PIT setup (channel 0, 100 Hz)
  - Timer interrupt handler

### Шаг 5: mm/e820.zig + mm/pmm.zig — Memory Management
**Оценка: ~400 строк Zig**
- Изучить: `kernel/e820.c`, `mm/init_64.c`
- Реализовать:
  - E820 parsing from Multiboot2 mmap
  - Physical memory manager (bitmap-based for Phase 1)
  - Page allocation (4KB + 2MB)
  - Kernel direct mapping setup

### Шаг 6: acpi/rsdp.zig + acpi/tables.zig + acpi/madt.zig
**Оценка: ~300 строк Zig**
- Изучить: `acpi/boot.c`, `drivers/acpi_tables.c`
- Реализовать:
  - RSDP scanning (EBDA + 0xE0000-0xFFFFF)
  - RSDT/XSDT header validation
  - MADT parsing (LAPIC + IOAPIC addresses)
  - Table checksum verification

### Шаг 7: poler/intent.zig + poler/object.zig — POLER Core Integration
**Оценка: ~250 строк Zig**
- Реализовать:
  - Intent struct definition (для ELF Adapter совместимости)
  - Intent dispatcher (упрощённый для Phase 1)
  - Object Table (базовый)
  - Firewall integration (из poler_core.zig)

### Шаг 8: main.zig — Kernel Main + Shell
**Оценка: ~200 строк Zig**
- Собрать всё вместе
- Boot sequence orchestration
- Shell с командами: help, memmap, acpi, intents, reboot, lspci

---

## 5. Баги для исправления (из v0.4.0)

При переписывании на 64-bit эти баги автоматически исчезнут:
- ~~fb_font OOB for ch≥128~~ → перепишем font mapper
- ~~shell_history usize underflow at pos=0~~ → перепишем shell
- ~~Version banner v0.3.0~~ → v0.5.0
- ~~fb_puts_hex64 skips last nibble~~ → перепишем hex64
- ~~Serial port wrong baud divisor~~ → исправим при переписывании
- ~~No STI instruction~~ → добавим в правильное место
- ~~No GDT/IDT setup~~ → ЦЕЛЬ Phase 1!

Останутся для ручного фикса:
- POLER Core key[4..7] unused → нужно расширить до 256-bit

---

## 6. Тестирование

```bash
# Сборка
zig build-x86_64

# Запуск в QEMU
/tmp/my-project/tools/qemu-sys/usr/bin/qemu-system-x86_64 \
  -kernel zig-out/bin/poler-os \
  -m 256M \
  -serial stdio \
  -display none

# С отладкой
qemu-system-x86_64 -kernel ... -d int,cpu_reset -serial stdio
```

### Milestone проверки:
- [ ] Кernel печатает "POLER-OS v0.5.0" через serial
- [ ] E820 map распечатан корректно
- [ ] GDT загружен без #GP
- [ ] IDT загружен, exception handlers работают (#PF, #GP, #DB)
- [ ] Timer interrupt считает секунды
- [ ] Keyboard input работает
- [ ] ACPI tables найдены (RSDP + MADT)
- [ ] Shell работает: help, memmap, acpi, reboot

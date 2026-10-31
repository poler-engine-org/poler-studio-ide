# POLER-OS Phase 1: Linux v6.6 LTS Drafts Analysis
## Что берём, что выкидываем, и почему

> Источник: Linux kernel v6.6 LTS (torvalds/linux)  
> Цель: Phase 1 — Boot + HAL для POLER-OS x86_64  
> Принцип: Linux как черновик — учимся ЧТО писать и КАК НЕ писать

---

## 1. BOOT SEQUENCE (arch/x86/boot/)

### Что делает Linux (744 строки в head_64.S + 645 строк в compressed/head_64.S)

Linux boot — это **трёхступенчатая** ракета:

```
BIOS/UEFI → boot/compressed/head_64.S → kernel/head_64.S → start_kernel()
   (real-mode)  (32→64 switch, decompress)  (page tables, GDT/IDT)  (C code)
```

#### Этап 1: compressed/head_64.S (645 строк) — самый важный для нас
- startup_32: начинается в 32-битном protected mode
- Настраивает GDT (временно, 4 дескриптора)
- Включает PAE (CR4.PAE)
- Настраивает identity-mapped page tables (4-level)
- Включает long mode (EFER.LME)
- Включает paging (CR0.PG)
- far jump → startup_64 (64-bit!)
- startup_64: настраивает 5-level paging если нужно
- Вызывает kernel_decompress()
- Переходит на распакованный kernel/head_64.S

#### Этап 2: kernel/head_64.S (744 строки)
- startup_64: уже в 64-bit режиме
- Настраивает GSBASE (per-cpu data)
- Вызывает verify_cpu()
- Настраивает CR3 (final page tables)
- Включает CR4 фичи (PAE, PGE, LA57)
- Flush TLB
- Настраивает stack
- Вызывает x86_64_start_kernel() → start_kernel()

#### Этап 3: boot/main.c (185 строк) — real-mode часть
- Detect memory via INT 0x15/E820
- Set video mode
- Go to protected mode
- Jump to kernel

### 🟢 ЧТО БЕРЁМ

| Компонент | Из Linux | Зачем POLER-OS |
|-----------|----------|----------------|
| 32→64 transition | compressed/head_64.S | Единственный способ войти в long mode |
| Identity page tables | ident_map_64.c | Нужны для перехода физ→вирт адреса |
| GDT layout | head_64.S | Архитектурное требование x86_64 |
| CR0/CR4/EFER sequence | head_64.S | Железный порядок включения |
| E820 memory map | boot/main.c | Единственный способ узнать память от BIOS |
| boot_params struct | setup.c | Формат передачи данных от bootloader |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Строк | Почему выкидываем |
|-----------|-------|-------------------|
| KASLR (kernel address randomization) | 889 | Phase 1 не нуждается, добавим позже через Intent |
| SEV/SME (AMD encryption) | 628+ | Cloud-фича, не нужна для desktop OS |
| EFI stub | 234 | POLER-OS начинает с BIOS/Multiboot2 |
| Real-mode code (main.c) | 185 | Multiboot2 уже в protected mode |
| Decompressor | 502 | Multiboot2 может загружать напрямую |
| A20 gate enable | 163 | Multiboot2 уже включил A20 |
| APM support | ~50 | Dead legacy |
| BIOS console I/O | ~300 | У нас VGA framebuffer |
| 32-bit boot path | 547 | POLER-OS = 64-bit only |

**Итого:** из ~3800 строк Linux boot → POLER-OS нужно ~800 строк Zig

---

## 2. IDT SETUP (arch/x86/kernel/idt.c — 343 строки)

### Что делает Linux

Linux создаёт IDT в **4 этапа**:
1. `idt_setup_early_handler()` — минимальные обработчики для early boot
2. `idt_setup_early_traps()` — #DB + #BP (debug + int3)
3. `idt_setup_traps()` — все CPU exceptions
4. `idt_setup_apic_and_irq_gates()` — APIC + IRQ gates

### 🟢 ЧТО БЕРЁМ

| Компонент | Зачем |
|-----------|-------|
| IDT entry format (gate_desc) | Архитектурное требование x86_64 |
| Exception vectors (0→31) | Обязательные CPU exceptions |
| INTG/SYSG/ISTG macro concept | Чистая абстракция для разных типов gates |
| IST (Interrupt Stack Table) | Критично для #DF, #NMI, #MC |
| early → full IDT transition pattern | Правильный порядок инициализации |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Почему |
|-----------|--------|
| F00F bug workaround | Pentium bug 1997 года! |
| CONFIG_X86_32 ветки | POLER-OS = 64-bit only |
| CPU Entry Area mapping | Linux-specific security hardening |
| /proc/interrupts integration | У нас Intent-based debug |
| system_vectors bitmap | Заменяем на Intent{IRQ, Register, ...} |
| IA32_EMULATION int80 | Будет в ELF Adapter, не в IDT |
| idt_invalidate() для kexec | Не нужно в Phase 1 |

**Итого:** 343 строки → ~150 строк Zig

---

## 3. E820 MEMORY MAP (arch/x86/kernel/e820.c — 1350 строк)

### Что делает Linux

Linux держит **ТРИ** копии E820:
1. `e820_table_firmware` — оригинал от BIOS (для hibernation)
2. `e820_table_kexec` — для kexec (второе ядро)
3. `e820_table` — основная, модифицированная ядром

И делает: sanitize, sort, merge, trim, update, print.

### 🟢 ЧТО БЕРЁМ

| Компонент | Зачем |
|-----------|-------|
| e820_entry struct (addr, size, type) | Базовый формат данных |
| E820 types (RAM, RESERVED, ACPI, NVS, UNUSABLE) | Классификация памяти |
| e820__mapped_any() — проверка региона | Валидация физических адресов |
| Sanitize/merge алгоритм | Обработка перекрытий от кривого BIOS |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Строк | Почему |
|-----------|-------|--------|
| 3 таблицы e820 | ~100 | Нужна ОДНА — POLER-OS не делает kexec/hyper |
| /sys/firmware/memmap export | ~80 | Intent-based debug вместо /sys |
| firmware_map_entry | ~60 | Только для /sys |
| e820__range_update/fine | ~200 | Заменяем на Intent{Mem, Reserve, ...} |
| PCI_mem_start hacks | ~40 | POLER-OS не наследует PCI костыли |
| KASLR region reserve | ~50 | Не нужно в Phase 1 |
| crashkernel reservation | ~80 | Не нужно в Phase 1 |
| e820__memory_setup() print | ~100 | У нас serial + framebuffer |
| platform-specific quirks | ~200 | Каждое ядро — свои quirks, нам не нужны |

**Итого:** 1350 строк → ~300 строк Zig

---

## 4. SETUP / INIT (setup.c — 1347 строк, init_main.c — 1573 строки)

### Что делает Linux

`setup_arch()` — гигантская функция-свалка:
-_reserve BIOS regions
- Parse kernel command line
- Init memory management
- Init ACPI
- Init NUMA
- Reserve initrd
- Find SMP config
- Setup PCI
- Setup LAPIC
- Setup IOAPIC
- ...и ещё 30+ вещей

### 🟢 ЧТО БЕРЁМ

| Компонент | Зачем |
|-----------|-------|
| boot_params parsing | Получить данные от bootloader |
| Memory region reservation | Не затереть BIOS/ACPI данные |
- ACPI table RSDP scanning | Найти ACPI таблицы |
- LAPIC/IOAPIC base detection | Для прерываний нужен |
- Command line parsing | Параметры загрузки |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Строк | Почему |
|-----------|-------|--------|
| 99% #include зависимостей | ~60 | POLER-OS не тянет legacy подсистемы |
| Xen guest detection | ~30 | Не виртуализируемся в Phase 1 |
| KASLR, KASAN, KMSAN | ~100 | Security hardening — позже |
| Crash dump / kdump | ~80 | Phase 1 не нуждается |
| VSYSSCALL mapping | ~40 | Dead legacy (int 0x80 compat) |
| olpc_ofw, tboot, ist | ~60 | Специфичное железо |
| DMI/SMBIOS early parse | ~40 | Не критично для boot |
| e820__memblock_setup() | ~40 | Заменяем на POLER Object Table |
| setup_arch() монолит | ~800 | Разбиваем на Intent pipeline |
| start_kernel() 80+ вызовов | ~1573 | POLER-OS: Intent-driven init |

**Итого:** 2920 строк → ~400 строк Zig

---

## 5. MEMORY MANAGEMENT INIT (mm/init_64.c — 1636 строк)

### Что делает Linux

- init_mem_mapping(): создаёт direct mapping всех физ. памяти
- Сложная логика с PMD/PUD уровнями
- NX bit настройка
- Разные path для <4GB и >4GB
- Kernel text mapping
- vmemmap initialization

### 🟢 ЧТО БЕРЁМ

| Компонент | Зачем |
|-----------|-------|
| Page table structure (PML4→PDP→PD→PT) | Архитектура x86_64 |
- Direct mapping concept | Kernel должен маппить всю память |
- NX bit для non-executable pages | Безопасность |
- 2MB/1GB huge pages | Производительность |
- __initdata/__inittext sections | Освобождение boot-кода |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Строк | Почему |
|-----------|-------|--------|
- CONFIG_DEBUG_PAGEALLOC | ~60 | Debug фича |
- CONFIG_SPARSEMEM_VMEMMAP | ~100 | Сложная NUMA оптимизация |
- KASAN shadow memory init | ~80 | Позже |
- randomize_memory() | ~60 | KASLR — позже |
- AMD SEV encryption mask | ~40 | Cloud фича |
- overlap detection hacks | ~50 | POLER-OS = чистый старт |
- PAGE_TABLE_ISOLATION | ~80 | Meltdown mitigation — потом |

**Итого:** 1636 строк → ~500 строк Zig

---

## 6. ACPI (acpi/boot.c — 1903 строки, drivers/acpi/ — ~4151 строка)

### Что делает Linux

- RSDP → RSDT/XSDT → MADT/DMAR/DSDT/SSDT table parsing
- LAPIC/IOAPIC detection via MADT
- ACPI OS Services Layer (osl.c) — 1770 строк!
- ACPI bus driver (bus.c) — 1428 строк

### 🟢 ЧТО БЕРЁМ

| Компонент | Зачем |
|-----------|-------|
| RSDP scanning (EBDA + 0xE0000-0xFFFFF) | Найти ACPI корень |
| RSDT/XSDT header validation (signature, checksum) | Валидация таблиц |
| MADT parsing (LAPIC/IOAPIC entries) | Настройка прерываний |
- Table signature constants | DSDT, FADT, MADT, etc. |
- acpi_os_map_memory() concept | Маппинг ACPI таблиц в виртуальную память |

### 🔴 ЧТО ВЫКИДЫВАЕМ

| Компонент | Строк | Почему |
|-----------|-------|--------|
| AML interpreter (не скачан, ~20K строк) | Оставляем на Phase 3+ |
| acpi_osl.c (1770 строк) | Linux-specific glue layer |
| /proc/acpi/ /sys/firmware/acpi/ | Intent-based debug вместо |
| ACPI bus driver (1428 строк) | Device model — Phase 3 |
- Sleep state support (S1-S5) | Phase 1 не спит |
- CPPC, CSTATE | Power management — позже |
- thermal zones | Позже |

**Итого:** ~6000 строк Linux ACPI → ~400 строк Zig (Phase 1: только table detection + MADT)

---

## 7. SMP BOOT (smpboot.c — 1620 строк)

### 🟢 ЧТО БЕРЁМ
- AP (Application Processor) startup protocol via SIPI
- CPU bringup sequence
- Per-CPU data initialization concept

### 🔴 ЧТО ВЫКИДЫВАЕМ
- Весь Linux CPU hotplug framework
- CPU masks (cpu_online_mask, etc.) → POLER Object Table
- /sys/devices/system/cpu/ → Intent-based
- stop_machine() infrastructure
- idle thread classes

**Итого:** 1620 строк → ~200 строк Zig

---

## ИТОГО: ФАЗА 1 — BOOT + HAL

| Подсистема | Linux строк | POLER-OS Zig строк | Компрессия |
|------------|------------|-------------------|------------|
| Boot (32→64 + paging) | ~3800 | ~800 | 4.7× |
| IDT setup | ~343 | ~150 | 2.3× |
| E820 memory map | ~1350 | ~300 | 4.5× |
| Setup/init | ~2920 | ~400 | 7.3× |
| MM init | ~1636 | ~500 | 3.3× |
| ACPI tables | ~6000 | ~400 | 15× |
| SMP boot | ~1620 | ~200 | 8.1× |
| **TOTAL** | **~17,669** | **~2,750** | **6.4×** |

### Ключевые решения для POLER-OS:

1. **Multiboot2** вместо Linux real-mode boot → экономим ~700 строк
2. **64-bit only** → выкидываем весь 32-bit path
3. **Одна E820 таблица** вместо трёх
4. **Intent-driven init** вместо start_kernel() с 80+ вызовами
5. **Минимальный ACPI** (только table detection + MADT) → AML interpreter в Phase 3
6. **Нет KASLR/SEV/KASAN** в Phase 1
7. **Нет kexec/kdump** в Phase 1
8. **Serial + FB debug** вместо /proc + /sys

---

## Приоритет чтения черновиков (в каком порядке изучать):

1. 🔥 **boot/compressed/head_64.S** — 32→64 transition (самое важное!)
2. 🔥 **boot/compressed/ident_map_64.c** — identity page tables
3. 🔥 **kernel/head_64.S** — final kernel entry
4. ⭐ **kernel/idt.c** — IDT setup pattern
5. ⭐ **kernel/e820.c** — memory map handling
6. ⭐ **include_asm/desc.h + desc_defs.h** — GDT/IDT structures
7. 📖 **kernel/setup.c** — setup_arch() flow
8. 📖 **mm/init_64.c** — page table init
9. 📖 **acpi/boot.c** — ACPI table scanning
10. 📖 **kernel/traps.c** — exception handlers

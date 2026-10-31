# POLER-OS — Карта проекта v0.7.0

> Последнее обновление: 2026-07-16

---

## 1. Архитектурная философия

**POLER-OS** — универсальная операционная система нового поколения, построенная на принципе **«программа — гость, а не хозяин»**. Ядро реализует минимальный собственный ABI и никогда не содержит кода, эмулирующего чужие API (Win32, POSIX). Совместимость с Windows и Linux программами достигается через **shim-библиотеки** в пользовательском пространстве, которые перехватывают вызовы и транслируют их в нативный ABI POLER-OS.

### Ключевые принципы

| Принцип | Реализация |
|---------|-----------|
| Нативное исполнение | Прямой машинный код на CPU, без интерпретации или JIT |
| Изоляция по умолчанию | Каждая программа в своей капсуле (память, ФС, syscall table) |
| Чистое микроядро | Только memory, scheduling, IPC, drivers — никакого Win32/POSIX |
| Shim-трансляция | Перехват syscall через IAT/GOT/PLT, не через kernel code |
| Криптографическая блокировка | После загрузки ядро становится неизменным (immutable) |

---

## 2. Текущий статус компонентов

### 2.1. Базовая инфраструктура (завершено)

| Компонент | Файл | Статус | Описание |
|-----------|------|--------|----------|
| Загрузка в 64-bit long mode | `boot64.S` | ✅ Готов | GRUB/Multiboot2 → long mode, GDT64, IDT, paging |
| Управление памятью | `pmm64.zig`, `vmm64.zig` | ✅ Готов | PMM (bitmap), VMM (4-level paging), kernel heap |
| HAL | `hal.zig` | ✅ Готов | GDT (9 entries), IDT, PIC, APIC, IO-APIC, TSS, syscall |
| Прерывания | `hal.zig` → IDT | ✅ Готов | 256 векторов, ISR/IRQ, double fault handler (IST1) |
| Framebuffer | `fb64.zig` | ✅ Готов | 1024×768×32bpp, putc/puts/putHex/scroll |
| Клавиатура | `keyboard.zig` | ✅ Готов | PS/2 scancode set 1, ASCII decode |
| Serial console | `serial.zig` | ✅ Готов | COM1 115200 baud, puts/putHex |
| Криптография | `pnd_v8.zig`, `rsa_oaep.zig`, `poler_ctr.zig` | ✅ Готов | PND v8 hash, RSA-OAEP, POLER-CTR AEAD |

### 2.2. Многозадачность (завершено)

| Компонент | Файл | Статус | Описание |
|-----------|------|--------|----------|
| Планировщик | `scheduler.zig` | ✅ Готов | Round-robin, preemptive, timer interrupt |
| Ring 3 user mode | `scheduler.zig` + `hal.zig` | ✅ Готов | User/kernel transition, syscall entry |
| ELF64 loader | `elf_loader.zig` | ✅ Готов | Load segments, create user PML4, jump to entry |
| User tasks | `scheduler.zig` | ✅ Готов | kernel_task_a/b (Ring 0), user_task_a/b (Ring 3) |

### 2.3. Драйверы блочных устройств и файловая система (завершено)

| Компонент | Файл | Статус | Описание |
|-----------|------|--------|----------|
| PCI драйвер | `pci.zig` | ✅ Готов | Конфигурационное пространство, BAR0 I/O + MMIO, device enumeration |
| VirtIO-BLK драйвер | `virtio_blk.zig` | ✅ Готов | DMA read/write, virtqueue, page-aligned buffers, PFN guard |
| FAT32 файловая система | `fat32.zig` | ✅ Готов | Read/write/create/delete файлов и каталогов, вложенные пути |
| Shell-команды | `main64.zig` | ✅ Готов | ls, cat, mkdir, touch, write, rm, disk |

**Исправленные баги (2026-07-16):**
- PFN @truncate guard: если desc_phys >= 4GB, abort с ошибкой DmaAddressTooHigh
- PCI MMIO BAR: теперь возвращается корректный адрес вместо 0
- resolveDirCluster: возвращает null вместо молчаливого fallback в root
- Версия: унифицирована на v0.7.0 во всех строках

### 2.4. SMP — Многоядерность (в процессе, приоритет 1)

| Компонент | Файл | Статус | Описание |
|-----------|------|--------|----------|
| ACPI MADT парсинг | `acpi.zig` | ✅ Готов | Обнаружение CPU через MADT, CpuInfo структура |
| Per-CPU данные | `smp.zig` | ✅ Готов | PerCpu struct, GSBASE, cpu_data array (MAX 8) |
| AP trampoline | `boot_smp.S` | ⚠️ Исправлено | 16→32→64 transition, lgdt из data area (0x8100) |
| SIPI/IPI | `hal.zig` + `smp.zig` | ✅ Готов | INIT IPI, SIPI, sendIpi() |
| Spinlocks | `spinlock.zig` | ✅ Готов | Spinlock с PAUSE, tryAcquire, SpinlockGuard RAII |
| AP инициализация | `smp.zig` | ⚠️ Требует теста | ap_entry_zig(), Local APIC init, signal READY |
| **Per-CPU планировщик** | `scheduler.zig` | ❌ Не начат | Per-CPU очереди, load balancing, affinity |
| **Atomic операции** | `atomic.zig` | ❌ Не начат | AtomicCounter, CAS |

**Известная проблема (исправлена в коде, требует теста в QEMU):** AP trampoline GDT loading — lgdt теперь читает из фиксированного адреса 0x8100 вместо вычисленного при ассемблировании. Требуется `qemu-system-x86_64 -cdrom poler-os64.iso -m 256M -smp 2 -serial stdio -no-reboot` для верификации.

### 2.4. Shim-архитектура (спроектирована, не реализована)

| Компонент | Статус | Описание |
|-----------|--------|----------|
| Loader (PE + ELF) | ❌ Не начат | `elf_loader.zig` → `loader.zig`, новый `pe_loader.zig` |
| Shim-библиотеки | ❌ Не начат | `poler-kernel32.dll`, `poler-libc.so` |
| Per-process syscall table | ❌ Не начат | capsule_type в Task, dispatch в syscall_entry |
| VDSO page | ❌ Не начат | pid/tid/timestamp без ring switch |
| Upcall механизм | ❌ Не начат | Для статических бинарников (2 ring switches) |
| Capsule isolation | ❌ Не начат | Per-process namespace, registry, FS root |

---

## 3. Приоритеты разработки

### 🔴 Приоритет 1: SMP — Многоядерность (Текущий)

**Почему критично:** Без SMP ядро использует только одно ядро. Все последующие компоненты (VFS, сеть, песочницы) требуют параллельного выполнения.

**Что осталось сделать:**

| Задача | Сложность | Время | Зависимости |
|--------|-----------|-------|-------------|
| Тест AP init в QEMU (-smp 2) | Низкая | 30 мин | QEMU установка |
| Per-CPU планировщик | Высокая | 5-7 дней | smp.zig, scheduler.zig |
| Atomic операции | Низкая | 1 день | — |
| Load balancing | Средняя | 3-5 дней | Per-CPU scheduler |
| Тест race conditions | Средняя | 2-3 дня | Spinlocks, atomic |

**Критерии приёмки:**
- [ ] Все CPU обнаружены через MADT
- [ ] Все CPU запущены и выполняют задачи
- [ ] Нет race conditions при стресс-тесте (1 час+)
- [ ] Производительность масштабируется с количеством CPU (>80% эффективности)

### 🟠 Приоритет 2: VFS — Виртуальная файловая система

**Почему критично:** Без VFS невозможно работать с файлами, загружать программы с диска, реализовать per-process namespace для песочниц.

**Задачи:**

| Задача | Описание |
|--------|----------|
| VFS абстрактный интерфейс | mount/open/read/write/close, inode abstraction |
| Inode + metadata | permissions, timestamps, size, operations table |
| Path resolution | абсолютные/относительные пути, `.`, `..`, кэш (dcache) |
| Per-process namespace | root_fs_inode в Task, chroot-подобная изоляция |
| Initrd/CPIO интеграция | Использовать существующий CPIO парсер из main64.zig |

**Связь с shim-архитектурой:** VFS с per-process namespace — фундамент для трансляции путей. Shim не может перевести `C:\Windows\System32\` → `/capsules/{pid}/C/Windows/System32/` без изолированной ФС.

### 🟠 Приоритет 3: Файловая система (ext2 → POLER-FS)

**Задачи:**

| Задача | Описание |
|--------|----------|
| ext2 read/write | Базовая поддержка ext2 для быстрого результата |
| ext2 tools | Создание образов, монтирование с Linux для отладки |
| POLER-FS design | Криптографическая integrity, FIM, secure deletion |
| POLER-FS implementation | Нативная ФС с встроенной защитой |

**Рекомендация:** Начать с ext2, параллельно проектировать POLER-FS.

**Зависимости:** VFS, AHCI/SATA драйвер.

### 🟡 Приоритет 4: Сетевой стек

**Задачи:**

| Задача | Описание |
|--------|----------|
| virtio-net драйвер | Приоритет для QEMU, virtqueue management |
| e1000 драйвер | Для реального железа, descriptor rings |
| Ethernet + IPv4/IPv6 | Базовый сетевой стек |
| TCP/UDP | Транспортный слой |
| DNS resolver | Разрешение имён |

**Зависимости:** SMP (желательно), interrupt handling.

### 🟡 Приоритет 5: AHCI/SATA драйвер

**Задачи:**

| Задача | Описание |
|--------|----------|
| PCI enumeration | Сканирование шины PCI |
| AHCI controller init | ABAR mapping, port detection |
| Command submission | Command list, FIS, DMA setup |
| Read/write блоки | Блочный интерфейс для VFS |

**Зависимости:** PCI, DMA.

---

## 4. Долгосрочные цели (после базовой инфраструктуры)

### 4.1. Безопасность

| Компонент | Описание |
|-----------|----------|
| Криптоблокировка ядра | После загрузки ядро становится immutable (PXE, TPM) |
| Verifiable boot chain | Проверка целостности на каждом этапе загрузки |
| FIM | File Integrity Monitoring на уровне ФС |
| Malware detection | Сигнатурный сканирование + поведенческий анализ |
| Rootkit detection | Проверка целостности IDT, syscall table, GDT |

### 4.2. Совместимость (Shim-архитектура)

| Компонент | Описание |
|-----------|----------|
| Linux syscall shim | Перехват POSIX syscall → POLER-OS ABI |
| PE/COFF loader | Загрузка Windows .exe + IAT подмена |
| Win32 shim | kernel32.dll, user32.dll, ntdll.dll адаптеры |
| Virtual registry | Изолированный реестр для каждой Windows-капсулы |

### 4.3. Графическая среда

| Компонент | Описание |
|-----------|----------|
| GPU drivers | Базовый framebuffer → virtio-gpu → intel/amdgpu |
| Display server | Wayland-подобный или собственный compositor |
| Desktop environment | KDE Plasma или собственный DE |

---

## 5. Shim-архитектура: техническая спецификация

### 5.1. Основной механизм: Shim как обязательная библиотека (Wine-подход)

```
┌──────────────────────────────────────────────────┐
│  calc.exe (PE binary)                            │
│  IAT: CreateFileA → kernel32.CreateFileA         │
├──────────────────────────────────────────────────┤
│  poler-kernel32.dll (shim)                       │
│  CreateFileA(path, ...):                         │
│    virt_path = translate(path)                   │
│      // C:\... → /capsules/{pid}/C/...           │
│    return poler_map_file(virt_path, flags)       │
├──────────────────────────────────────────────────┤
│  Микроядро POLER-OS (syscall ABI)               │
│  poler_map_file / poler_write / poler_alloc ...  │
└──────────────────────────────────────────────────┘
```

**Загрузчик (loader.zig):**
1. Определяет тип бинарника (PE/ELF) по заголовку
2. Парсит зависимости (DT_NEEDED / IAT)
3. Подменяет системные библиотеки на `poler-*` shim'ы
4. Мапит shim в адресное пространство процесса
5. Связывает IAT/GOT/PLT с точками входа шима
6. Регистрирует капсулу (`capsule_type = windows/linux`)
7. Запускает процесс

### 5.2. Страховочная сеть: Per-process syscall table

Для статических бинарников (Go, Rust no_std), которые вызывают `syscall` напрямую:

```zig
// В Task (scheduler.zig)
capsule_type: enum { native, windows, linux, unknown },
shim_context: ?*ShimContext,

// В syscall_entry (hal.zig)
fn handleSyscall(nr: u64, args: ...) u64 {
    const task = scheduler.currentTask();
    switch (task.capsule_type) {
        .native   => return nativeSyscall(nr, args),
        .windows  => return upcallToShim(task.shim_context, nr, args),
        .linux    => return upcallToShim(task.shim_context, nr, args),
        .unknown  => return nativeSyscall(nr, args),
    }
}
```

Upcall: ядро настраивает user stack → возвращается в shim handler → shim делает native syscall → возвращает результат. 2 ring switches — приемлемо для редких статических бинарников.

### 5.3. Оптимизация: VDSO page

Вызовы, не требующие входа в ядро:

```zig
pub const VdsoPage = struct {
    pid: u32,
    tid: u32,
    timestamp: u64,
    capsule_id: u32,
};
```

Ядро обновляет при context switch и timer interrupt. Shim читает напрямую — **0 ring switches**.

### 5.4. Сравнение подходов

| Критерий | Shim + translation (POLER-OS) | API в ядре (классика) |
|----------|-------------------------------|----------------------|
| Безопасность | Изолированные песочницы | Всё ядро — одна цель |
| Гибкость | Обновление шима без ядра | Пересборка ядра |
| Простота ядра | Маленькое, проверяемое | Раздуто чужим кодом |
| Совместимость | Несколько окружений одновременно | Только одно |
| Производительность | Только перехват syscall | Встроенные издержки API |

---

## 6. Структура репозитория

```
poler-os/
├── zig-kernel/                    # Основной код ядра
│   ├── build.zig                  # Сборка (zig build)
│   ├── build-iso.sh              # Сборка ISO (grub-mkrescue)
│   ├── .gitignore                # Исключения (ISO, GRUB modules, cache)
│   ├── docs/                     # Документация
│   │   ├── ROADMAP.md            # ← Этот файл
│   │   └── SMP_SPECIFICATION.md  # Спецификация SMP (детальная)
│   ├── iso/
│   │   └── boot/grub/
│   │       └── grub.cfg          # GRUB конфигурация
│   └── src64/
│       ├── main64.zig            # Точка входа ядра
│       ├── hal.zig               # Hardware Abstraction Layer
│       ├── acpi.zig              # ACPI (RSDP, RSDT, MADT)
│       ├── smp.zig               # SMP (per-CPU, AP init, SIPI)
│       ├── boot_smp.S            # AP trampoline (16→32→64)
│       ├── boot64.S              # BSP boot (GDT64, long mode)
│       ├── scheduler.zig         # Планировщик (round-robin)
│       ├── elf_loader.zig        # ELF64 загрузчик
│       ├── spinlock.zig          # Спинлоки с PAUSE
│       ├── pmm64.zig             # Physical Memory Manager
│       ├── vmm64.zig             # Virtual Memory Manager
│       ├── fb64.zig              # Framebuffer console
│       ├── serial.zig            # Serial COM1
│       ├── keyboard.zig          # PS/2 keyboard
│       ├── pnd_v8.zig            # PND v8 hash
│       ├── rsa_oaep.zig          # RSA-OAEP
│       └── poler_ctr.zig         # POLER-CTR AEAD
```

---

## 7. Системные требования (тестовая среда)

| Параметр | Значение |
|----------|----------|
| Architecture | x86_64 (long mode) |
| CPU | 2× Intel Xeon @ 2.50GHz (KVM) |
| RAM | 1 GB |
| Kernel (host) | Linux 4.19.91 |
| QEMU | Требуется установка (не доступна в текущем окружении) |
| Boot | GRUB/Multiboot2 |
| Video | 1024×768×32bpp framebuffer |

---

## 8. Рекомендуемый порядок работы

```
Недели 1-4:   SMP (многоядерность)              ← ТЕКУЩИЙ ЭТАП
                ├── AP trampoline fix (исправлено, нужен тест)
                ├── Per-CPU scheduler
                ├── Load balancing
                └── Stress testing
                    ↓
Недели 5-6:   VFS
                ├── Абстрактный интерфейс
                ├── Inode + path resolution
                └── Per-process namespace (root_fs_inode)
                    ↓
Недели 7-9:   ФС (ext2) + AHCI драйвер (параллельно)
                ├── ext2 read/write
                ├── PCI enumeration
                ├── AHCI controller init
                └── POLER-FS design (параллельно)
                    ↓
Недели 10+:   Сетевой стек
                ├── virtio-net (QEMU)
                ├── e1000 (реальное железо)
                └── TCP/UDP/DNS
                    ↓
После инфраструктуры:
                ├── Shim-библиотеки (Linux + Windows)
                ├── PE-загрузчик
                ├── Криптоблокировка ядра
                └── Графическая среда
```

---

## 9. Версионирование

| Версия | Веха | Статус |
|--------|------|--------|
| v0.7.0 | Базовое ядро + документация | ✅ Завершено |
| v0.8.0-dev | SMP реализация | ⚠️ В процессе |
| v0.8.0 | SMP + per-CPU scheduler | 🔲 Планируется |
| v0.9.0 | VFS + ext2 | 🔲 Планируется |
| v1.0.0 | Базовая инфраструктура (SMP + VFS + FS + Network) | 🔲 Планируется |
| v2.0.0 | Shim-архитектура + совместимость | 🔲 Долгосрочная цель |

---

*Microsoft не диктует правила. Правила диктует архитектура.*

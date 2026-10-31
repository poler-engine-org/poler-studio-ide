# POLER-OS: Детальная дорожная карта и критический аудит

> Дата: 2026-03-05
> Версия документа: 1.0
> Автор: Senior OS Architect & Project Manager
> Основание: анализ кодовой базы v0.7.0 (Zig 0.13.0, x86_64)

---

# ЧАСТЬ 1: ДОРОЖНАЯ КАРТА

## Текущее состояние проекта (v0.7.0)

```
┌─────────────────────────────────────────────────────────────────────┐
│  ЗАВЕРШЕНО                                                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │ Boot64   │ │ HAL      │ │ PMM/VMM  │ │ Heap     │ │ Crypto   │  │
│  │ GDT/IDT  │ │ APIC     │ │ 4-level  │ │ SipHash  │ │ PND v8   │  │
│  │ Paging   │ │ IO-APIC  │ │ paging   │ │ RAII     │ │ RSA-OAEP │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │Scheduler │ │ ELF64    │ │ FAT32    │ │ PCI      │ │VirtIO-BLK│  │
│  │Round-rob │ │ loader   │ │ R/W      │ │ enumerat │ │ DMA R/W  │  │
│  │ Ring 3   │ │ CR3 swap │ │ LFN      │ │ BAR MMIO │ │ virtq    │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
│                                                                      │
│  В ПРОЦЕССЕ                                                         │
│  ┌──────────┐ ┌──────────┐                                          │
│  │ SMP      │ │ Atomic   │   AP trampoline исправлен,               │
│  │ Per-CPU  │ │ ops      │   но НЕ ТЕСТИРОВАН в QEMU               │
│  └──────────┘ └──────────┘                                          │
└─────────────────────────────────────────────────────────────────────┘
```

## Сводная таблица фаз

| # | Фаза | Приор. | Сложн. | Недели (1 чел.) | Зависимости | Риск |
|---|------|--------|--------|-----------------|-------------|------|
| 1 | Минимальное ядро | P0 | 5 | ✅ ГОТОВО | — | — |
| 2 | Управление памятью | P0 | 7 | ✅ ГОТОВО | Ф1 | — |
| 3 | SMP | P0 | 8 | ⚠️ 6 нед. (доделать) | Ф2 | ВЫСОКИЙ |
| 4 | Планировщик v2 | P0 | 6 | 8 | Ф3 | СРЕДНИЙ |
| 5 | Синхронизация | P0 | 5 | 4 | Ф3 | НИЗКИЙ |
| 6 | Файловые системы | P1 | 8 | 14 | Ф5, Ф7 | ВЫСОКИЙ |
| 7 | Драйверная модель | P1 | 7 | 10 | Ф3, Ф5 | ВЫСОКИЙ |
| 8 | Сетевой стек | P1 | 8 | 16 | Ф7, Ф5 | ВЫСОКИЙ |
| 9 | Графика | P1 | 6 | 8 | Ф4 | СРЕДНИЙ |
| 10 | Vulkan GPU | P2 | 10 | 40+ | Ф9 | КРИТ. |
| 11 | User space | P1 | 7 | 12 | Ф4, Ф6 | СРЕДНИЙ |
| 12 | VFS complete | P1 | 6 | 8 | Ф6, Ф11 | СРЕДНИЙ |
| 13 | Win32 subsystem | P2 | 9 | 30 | Ф11, Ф12 | КРИТ. |
| 14 | NT API | P2 | 10 | 40+ | Ф13 | КРИТ. |
| 15 | App compatibility | P2 | 8 | 20 | Ф14 | КРИТ. |
| 16 | Developer tools | P2 | 6 | 16 | Ф11 | СРЕДНИЙ |
| 17 | Testing infra | P1 | 4 | 6 | Все | СРЕДНИЙ |
| 18 | Release stages | P3 | 3 | 8 | Все | НИЗКИЙ |

**Итого: ~252 недели (≈4.8 года для одного разработчика)**
**С командой 3-4 чел.: ~2-3 года до beta**

---

## Фаза 1: Минимальное ядро (boot, GDT, IDT, paging)

**Статус: ✅ ЗАВЕРШЕНО**

| Параметр | Значение |
|----------|----------|
| Зависимости | Нет |
| Приоритет | P0 |
| Сложность | 5/10 |
| Затраты | ✅ ~8 нед. (уже вложено) |
| Риск | — |

**Реализовано:**
- `boot64.S`: Multiboot2 → 32→64 переход, identity paging (4GB, 2MB pages)
- `hal.zig`: GDT64 (9 записей), IDT (256 векторов), PIC remap, APIC, IO-APIC, TSS IST1
- `linker64.ld`: линкер-скрипт для ядра

**Критерии завершения:**
- [x] Ядро загружается в long mode через GRUB
- [x] GDT с kernel/user сегментами
- [x] IDT обрабатывает исключения и IRQ
- [x] Paging работает (2MB pages, identity map)

---

## Фаза 2: Управление памятью (PMM, VMM, heap, slab)

**Статус: ✅ ЗАВЕРШЕНО (требует доработки)**

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф1 |
| Приоритет | P0 |
| Сложность | 7/10 |
| Затраты | ✅ ~10 нед. (уже вложено) |
| Риск | СРЕДНИЙ — нет slab allocator |

**Реализовано:**
- `pmm64.zig`: bitmap-based, allocPage/freePage
- `vmm64.zig`: 4-level paging, mapPage/unmapPage, createUserPML4, mapPageInPML4
- `heap64.zig`: free-list + SipHash-2-4 integrity tags, kmalloc/kfree

**Чего НЕ хватает:**
- [ ] Slab allocator для частых выделений малого размера
- [ ] VMA (Virtual Memory Area) tracking — нет структуры для отслеживания регионов виртуальной памяти
- [ ] mmap/munmap syscall — нет пользовательского интерфейса к VMM
- [ ] Copy-on-write (COW) для fork()
- [ ] Page fault handler полноценный (сейчас есть только для Ring 3)
- [ ] Demand paging — загрузка страниц по требованию
- [ ] OOM killer / reclaim

**Критерии завершения (полные):**
- [ ] PMM: allocPage/freePage со строгим учётом (битовый сертификат)
- [ ] VMM: map/unmap + VMA tracking + demand paging
- [ ] Heap: kmalloc/kfree + slab для объектов ≤256 байт
- [ ] Page fault handler: CoW, demand paging, stack guard, OOM
- [ ] Нагрузочный тест: 10K выделений/освобождений без утечек

---

## Фаза 3: SMP (многопроцессорность, APIC, IPI)

**Статус: ⚠️ В ПРОЦЕССЕ**

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф2 |
| Приоритет | P0 |
| Сложность | 8/10 |
| Затраты | 6 нед. (доделать + протестировать) |
| Риск | **ВЫСОКИЙ** |

**Реализовано:**
- [x] ACPI MADT парсинг (обнаружение CPU)
- [x] Per-CPU данные (PerCpu, GSBASE)
- [x] AP trampoline (boot_smp.S) — исправлен, НЕ ТЕСТИРОВАН
- [x] IPI: INIT, SIPI, generic
- [x] Spinlocks + SpinlockGuard RAII
- [x] Atomic операции (AtomicCounter, AtomicFlag, CAS)
- [x] Per-CPU планировщик (cpu_run_queues)
- [x] Serial output SMP-safe

**КРИТИЧЕСКИЕ ПРОБЛЕМЫ:**
1. **НЕ ТЕСТИРОВАНО В QEMU** — код компилируется, но не запускался с `-smp 2`
2. TLB shootdown отложен — это ОШИБКА: без TLB shootdown при смене CR3 на одном CPU другие CPU используют устаревшие TLB-записи → corruption
3. TSC не синхронизирован между CPU
4. Нет CPU hotplug / hot-unplug
5. Нет proper CPU halt/wake для power management

**Оставшиеся задачи:**

| Задача | Сложность | Время | Критичность |
|--------|-----------|-------|-------------|
| QEMU тест -smp 2 | 2 | 2 дня | P0 |
| TLB shootdown (IPI + INVLPG) | 6 | 2 нед. | P0 |
| TSC синхронизация | 4 | 1 нед. | P1 |
| Стресс-тест race conditions | 5 | 1 нед. | P0 |
| CPU hotplug infrastructure | 3 | 3 дня | P2 |

**Критерии завершения:**
- [ ] QEMU `-smp 4` — все 4 CPU обнаружены и работают
- [ ] Стресс-тест: 1 час без panic/corruption
- [ ] TLB shootdown: page table update на одном CPU виден на всех
- [ ] AtomicCounter: 2 CPU × 1M increment = 2M (не меньше)
- [ ] Per-CPU scheduler: load imbalance < 20%

---

## Фаза 4: Планировщик v2 (threads, processes, priorities)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф3 (SMP) |
| Приоритет | P0 |
| Сложность | 6/10 |
| Затраты | 8 нед. |
| Риск | СРЕДНИЙ |

**Текущее состояние:**
- Round-robin с APIC timer preemption
- MAX_TASKS = 8 (жёсткий лимит!)
- Нет приоритетов, нет процессов (только задачи)
- Нет wait/wakeup механизма
- Нет fork/exec

**Что нужно реализовать:**

```
┌─────────────────────────────────────────────────────┐
│  Process (адресное пространство + ресурсы)          │
│  ┌───────────────────────────────────────┐          │
│  │  Thread 1   Thread 2   Thread 3      │          │
│  │  (Ring 3)   (Ring 3)   (Ring 0)      │          │
│  │  priority   priority   priority      │          │
│  │  state      state      state         │          │
│  └───────────────────────────────────────┘          │
│  CR3: unique PML4                                    │
│  FD table, signal handlers, cwd                       │
└─────────────────────────────────────────────────────┘
```

| Компонент | Описание | Время |
|-----------|----------|-------|
| Process abstraction | PID, CR3, FD table, cwd, signal mask | 2 нед. |
| Thread abstraction | TID, stack, register state, affinity | 1 нед. |
| Priority scheduler | O(1) multi-level feedback queue | 2 нед. |
| fork() + exec() | COW pages, duplicate FD table | 2 нед. |
| wait/wakeup | Wait queue, event-based wakeup | 1 нед. |
| Dynamic task allocation | Убрать MAX_TASKS=8, использовать heap | 3 дня |
| Signals | POSIX-подобные сигналы | 1 нед. |

**Критерии завершения:**
- [ ] fork(): дочерний процесс продолжает выполнение после fork
- [ ] exec(): замена образа процесса новым ELF
- [ ] 1000+ процессов/потоков без деградации
- [ ] Приоритеты: реальный процесс не голодает при 100 low-priority задач
- [ ] wait()/waitpid() корректно собирает zombie

---

## Фаза 5: Синхронизация (spinlocks, mutexes, semaphores, RCU)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф3 (SMP) |
| Приоритет | P0 |
| Сложность | 5/10 |
| Затраты | 4 нед. |
| Риск | НИЗКИЙ |

**Реализовано:**
- [x] Spinlock (с PAUSE, tryAcquire, SpinlockGuard RAII)
- [x] Atomic операции (CAS, fetchAdd, xchg, barriers)

**Что нужно:**

| Примитив | Назначение | Сложность | Время |
|----------|-----------|-----------|-------|
| Mutex (sleep lock) | Долгие операции (I/O, FS) | 3 | 1 нед. |
| Semaphore | Подсчёт ресурсов | 2 | 3 дня |
| RW-lock | Чтение-запись (VFS, реестр) | 3 | 1 нед. |
| Completion | Одноразовое событие | 2 | 2 дня |
| Wait queue | Общий механизм ожидания | 3 | 1 нед. |
| RCU (read-copy-update) | Lock-free чтение (для VFS dentry) | 7 | 2 нед. |

**Критерии завершения:**
- [ ] Mutex: конкурентный доступ 4 CPU × 10K lock/unlock — no deadlock
- [ ] RW-lock: N читателей параллельно, писатель эксклюзивно
- [ ] RCU: grace period корректно ожидает всех читателей
- [ ] Приоритетное наследование для mutex (prevent priority inversion)

---

## Фаза 6: Файловые системы (VFS, ext2, POLER-FS)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф5, Ф7 (драйвер блочного устройства) |
| Приоритет | P1 |
| Сложность | 8/10 |
| Затраты | 14 нед. |
| Риск | **ВЫСОКИЙ** |

**Текущее состояние:**
- FAT32 R/W работает, но это прямое обращение к устройству (не через VFS)
- Нет VFS абстракции
- Нет ext2
- Нет POLER-FS

**Архитектура VFS:**

```
┌──────────────────────────────────────────────────────────────────┐
│  VFS Layer                                                       │
│  ┌──────────────────────────────────────────────────────┐        │
│  │  inode_ops: open/read/write/close/mkdir/unlink/...  │        │
│  │  dentry cache: path → inode mapping                  │        │
│  │  mount table: mountpoint → filesystem instance       │        │
│  │  per-process namespace: root + cwd per process       │        │
│  └──────────────────────────────────────────────────────┘        │
│     │          │            │            │                         │
│  ┌──────┐  ┌──────┐  ┌──────────┐  ┌──────────┐                │
│  │ ext2 │  │FAT32 │  │ POLER-FS │  │ initrd   │                │
│  └──────┘  └──────┘  └──────────┘  └──────────┘                │
│     │          │            │            │                         │
│  ┌─────────────────────────────────────────────────────┐         │
│  │  Block device layer (AHCI, VirtIO-BLK, NVMe)       │         │
│  └─────────────────────────────────────────────────────┘         │
└──────────────────────────────────────────────────────────────────┘
```

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| VFS inode/dentry | Абстрактные интерфейсы, operations table | 3 нед. |
| Path resolution | Абсолютные/относительные, `.`, `..`, symlink | 2 нед. |
| Dentry cache | Хешированный кэш path→inode | 1 нед. |
| Mount framework | mount/umount, mount table, root FS | 1 нед. |
| Per-process namespace | root/cwd в Process, chroot-изоляция | 1 нед. |
| ext2 read/write | Базовая ext2 для совместимости | 3 нед. |
| FAT32 через VFS | Обёртка текущего FAT32 под VFS | 1 нед. |
| POLER-FS design | Спецификация формата диска | 2 нед. |
| POLER-FS implement | B+ tree extents, AES-256-XTS, BLAKE3, NT ACL | 8 нед. |

> **POLER-FS — самая амбициозная ФС в истории indie-OS.**
> Оценка в 8 недель — это только ядро ФС. Инструменты (mkfs.poler, fsck.poler) — ещё +4 недели.

**Критерии завершения:**
- [ ] VFS: mount ext2 + FAT32 одновременно
- [ ] Per-process namespace: chroot изолирует процесс
- [ ] ext2: чтение/запись файлов, создание каталогов — совместимо с Linux e2fsprogs
- [ ] POLER-FS: mkfs, mount, ls, cp — работают в QEMU
- [ ] POLER-FS: AES-256-XTS шифрование прозрачно для приложений
- [ ] POLER-FS: BLAKE3 integrity check при каждом чтении

---

## Фаза 7: Драйверная модель (bus enumeration, PCI, USB)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф3, Ф5 |
| Приоритет | P1 |
| Сложность | 7/10 |
| Затраты | 10 нед. |
| Риск | **ВЫСОКИЙ** |

**Текущее состояние:**
- PCI enumeration работает (config space, BAR, MMIO)
- VirtIO-BLK драйвер работает
- Нет единой драйверной модели
- Нет USB
- Нет AHCI/NVMe для реального железа

**Архитектура драйверной модели:**

```
┌─────────────────────────────────────────────────┐
│  Driver Manager                                  │
│  ├── Bus drivers: PCI, USB, ACPI, ISA           │
│  ├── Device drivers: block, net, gpu, input     │
│  ├── Driver API: probe/remove/suspend/resume    │
│  └── Module loader (если ядро не immutable)      │
├─────────────────────────────────────────────────┤
│  Bus Enumeration                                 │
│  ┌──────┐  ┌──────┐  ┌──────┐                  │
│  │ PCI  │  │ USB  │  │ ACPI │                  │
│  │ ✅   │  │  ❌  │  │ ✅   │                  │
│  └──────┘  └──────┘  └──────┘                  │
├─────────────────────────────────────────────────┤
│  Device Classes                                  │
│  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐       │
│  │Block │  │ Net  │  │ GPU  │  │Input │       │
│  │VirtIO│  │ ❌   │  │ ❌   │  │ PS/2 │       │
│  └──────┘  └──────┘  └──────┘  └──────┘       │
└─────────────────────────────────────────────────┘
```

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| Driver framework | probe/remove, device tree, match table | 2 нед. |
| Block device API | Единый интерфейс для VirtIO/AHCI/NVMe | 1 нед. |
| AHCI/SATA | Реальное железо: command list, FIS, DMA | 3 нед. |
| NVMe | Submission/completion queues, PRP/SGL | 3 нед. |
| USB stack | UHCI/EHCI/xHCI, device enumeration | 6 нед. |
| virtio-net | Для QEMU тестирования | 1 нед. |
| e1000 / e1000e | Для реального железа | 2 нед. |

> **USB — чёрная дыра.** Спецификация xHCI — 600+ страниц. Реализация USB stack с нуля для одного разработчика — это 2-3 месяца минимально.

**Критерии завершения:**
- [ ] PCI: все устройства на шине обнаружены и настроены
- [ ] AHCI: чтение/запись SATA диска
- [ ] NVMe: чтение/запись NVMe диска
- [ ] USB: keyboard + mass storage работают
- [ ] Единый block device API для всех драйверов

---

## Фаза 8: Сетевой стек (TCP/IP, sockets)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф7 (сетевой драйвер), Ф5 |
| Приоритет | P1 |
| Сложность | 8/10 |
| Затраты | 16 нед. |
| Риск | **ВЫСОКИЙ** |

**Архитектура:**

```
┌──────────────────────────────────────────────┐
│  Socket API (user-space interface)           │
│  socket / bind / listen / accept / connect   │
│  send / recv / select / poll / epoll         │
├──────────────────────────────────────────────┤
│  TCP / UDP                                   │
│  Connection state machine, retransmission,   │
│  congestion control (Reno → CUBIC)           │
├──────────────────────────────────────────────┤
│  IPv4 / IPv6                                 │
│  Fragmentation, ICMP, NDP                    │
├──────────────────────────────────────────────┤
│  ARP / NDP                                   │
├──────────────────────────────────────────────┤
│  Ethernet                                    │
├──────────────────────────────────────────────┤
│  Driver: virtio-net / e1000 / e1000e         │
└──────────────────────────────────────────────┘
```

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| Ethernet frame | Tx/Rx, MAC addressing | 1 нед. |
| ARP | IPv4 → MAC resolution | 1 нед. |
| IPv4 | Header, fragmentation, forwarding | 2 нед. |
| ICMP | Echo request/reply, destination unreachable | 1 нед. |
| UDP | Best-effort datagram | 1 нед. |
| TCP | Full state machine, retransmit, congestion | 6 нед. |
| Socket API | POSIX-подобный интерфейс | 2 нед. |
| DNS resolver | Stub resolver в userspace | 1 нед. |
| IPv6 | Базовая поддержка | 2 нед. |

> **TCP — это 60% всего сетевого стека.** State machine с 11 состояниями, exponential backoff, sliding window, congestion control, Nagle, delayed ACK. У Linux — 100K+ строк только TCP. Даже минимальная реализация — это 3-4 месяца.

**Критерии завершения:**
- [ ] ping: ICMP echo через QEMU virtio-net
- [ ] TCP: соединение с удалённым HTTP-сервером
- [ ] TCP: wget-подобная утилита скачивает файл
- [ ] Socket API: порт простого сетевого приложения (nc, httpd)
- [ ] Нагрузка: 1000 одновременных TCP соединений без утечек

---

## Фаза 9: Графика (framebuffer, compositor)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф4 (планировщик) |
| Приоритет | P1 |
| Сложность | 6/10 |
| Затраты | 8 нед. |
| Риск | СРЕДНИЙ |

**Текущее состояние:**
- Framebuffer: 1024×768×32bpp, putc/puts/putHex/scroll
- Это текстовая консоль, не графический сервер

**Архитектура:**

```
┌──────────────────────────────────────────────────────────┐
│  Applications                                            │
│  ├── Terminal    ├── File Manager   ├── Settings         │
│  └──────────────────────────────────────────────────────│
│  Client-side Wayland-подобный protocol                   │
│  ┌──────────────────────────────────────────────────────┐│
│  │  Compositor (compositor.pol)                         ││
│  │  ├── Window management                               ││
│  │  ├── Surface composition (GPU-accelerated later)     ││
│  │  ├── Input routing (keyboard, mouse)                 ││
│  │  └── Damage tracking                                 ││
│  └──────────────────────────────────────────────────────┘│
│  ┌──────────────────────────────────────────────────────┐│
│  │  Display Driver                                      ││
│  │  ├── KMS (kernel mode setting)                       ││
│  │  ├── Framebuffer allocation (dumb buffers)           ││
│  │  └── Page flip (vblank sync)                         ││
│  └──────────────────────────────────────────────────────┘│
│  ┌──────────────────────────────────────────────────────┐│
│  │  Input                                               ││
│  │  ├── Keyboard (PS/2 ✅ + USB)                       ││
│  │  ├── Mouse (PS/2 + USB)                              ││
│  │  └── Evdev-подобный интерфейс                        ││
│  └──────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────┘
```

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| KMS abstraction | Mode setting, framebuffer allocation | 2 нед. |
| Input subsystem | Keyboard/mouse unified, evdev-подобный | 2 нед. |
| Compositor | CPU-рендеринг, window management, damage | 3 нед. |
| IPC protocol | Wayland-подобный протокол для клиентов | 1 нед. |

**Критерии завершения:**
- [ ] Compositor запускается и отображает 2+ окна
- [ ] Клавиатура и мышь маршрутизируются в активное окно
- [ ] Переключение виртуальных терминалов (Ctrl+Alt+F1-F6)
- [ ] 60 FPS при 1920×1080 с <5% CPU (software rendering)

---

## Фаза 10: Vulkan GPU драйвер

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф9 |
| Приоритет | P2 |
| Сложность | **10/10** |
| Затраты | **40+ нед.** |
| Риск | **КРИТИЧЕСКИЙ** |

> **Это самая опасная фаза во всём проекте.**

**Почему Vulkan — это безумие для indie-OS:**

1. **Vulkan спецификация**: 2600+ страниц. Это не документ — это энциклопедия.
2. **GPU-specific**: Каждый GPU (Intel, AMD, NVIDIA) требует СВОЙ драйвер. Нет универсального Vulkan драйвера.
3. **Mesa3D**: Единственная open-source реализация — это 5M+ строк кода, 1500+ контрибьюторов за 20 лет.
4. **Firmware**: Современные GPU требуют прошивки, которая загружается при инициализации.
5. **DRM/KMS**: Vulkan работает поверх DRM (Direct Rendering Manager), который сам по себе сложен.

**Реалистичный путь:**

```
Шаг 1 (8 нед.):   VirtIO-GPU — программный GPU в QEMU
Шаг 2 (12 нед.):  Intel i915 — базовый KMS + dumb buffer
Шаг 3 (20+ нед.): Intel Vulkan (ANV-подобный) — минимальный
Шаг 4 (∞ нед.):   AMD/NVIDIA — требует firmware + reverse engineering
```

**DirectX через Vulkan:**
- DXVK (DirectX → Vulkan translation) — это 50K+ строк C++
- VKD3D (DX12 → Vulkan) — ещё 30K+ строк
- Оба требуют рабочий Vulkan 1.2+

**Критерии завершения (минимальные):**
- [ ] VirtIO-GPU: compositor использует GPU acceleration
- [ ] Intel: KMS mode setting на реальном железе
- [ ] Vulkan: triangle rendering (vkcube-подобный)
- [ ] DXVK: запуск простейшего DirectX 9 приложения

> **Вердикт:** Полноценный Vulkan на уровне запуска AAA-игр — это 10+ лет для команды 5+. Для одного разработчика — нереально. Нужно drastically сузить scope.

---

## Фаза 11: User space (ELF loader, libc, shell)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф4, Ф6 |
| Приоритет | P1 |
| Сложность | 7/10 |
| Затраты | 12 нед. |
| Риск | СРЕДНИЙ |

**Текущее состояние:**
- ELF64 loader: загрузка сегментов, создание user PML4, jump to entry
- Shell: ls, cat, mkdir, touch, write, rm, disk (встроенные команды ядра)
- Нет libc, нет динамического линковщика, нет настоящих userspace-программ

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| Syscall interface | Стабильный syscall ABI (номера, конвенция) | 2 нед. |
| libc (минимальный) | stdio, stdlib, string, unistd, fcntl, errno | 6 нед. |
| Dynamic linker (ld.so) | DT_NEEDED resolution, GOT/PLT relocation | 3 нед. |
| Shell (userspace) | Порт dash/mksh или написание с нуля | 2 нед. |
| Core utilities | ls, cat, cp, mv, rm, mkdir, echo, grep | 2 нед. |
| init system | PID 1, запуск сервисов | 1 нед. |

> **libc — это неожиданно сложно.** Даже musl (минимальная libc) — это 30K+ строк. Написание с нуля — 2-3 месяца. Но можно форкнуть musl и адаптировать.

**Стратегия libc:**

```
Вариант A (рекомендуется): Порт musl libc
  ├── Адаптация syscall обёрток под POLER-OS ABI
  ├── Удаление Linux-specific кода (sys/epoll, inotify, etc.)
  ├── Добавление POLER-OS-specific расширений
  └── Время: 4-6 нед.

Вариант B: Написание с нуля
  ├── Полный контроль, нет чужого кода
  ├── Но: 30K+ строк для полной POSIX libc
  └── Время: 12+ нед.
```

**Критерии завершения:**
- [ ] Программа на C (hello world) компилируется, линкуется и запускается
- [ ] libc: fopen/fread/fwrite, malloc/free, printf работают
- [ ] Shell: pipe, redirect, переменные окружения
- [ ] Dynamic linker: загрузка shared library (.so)
- [ ] init: корректный запуск как PID 1

---

## Фаза 12: VFS complete

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф6, Ф11 |
| Приоритет | P1 |
| Сложность | 6/10 |
| Затраты | 8 нед. |
| Риск | СРЕДНИЙ |

**Дополнительные возможности VFS (поверх Фазы 6):**

| Компонент | Описание | Время |
|-----------|----------|-------|
| Symlinks | Symbolic link creation + following | 1 нед. |
| Hardlinks | Hard link creation + reference counting | 3 дня |
| Pipes/FIFOs | Named pipes, pipe() syscall | 1 нед. |
| Device nodes | /dev filesystem, mknod | 1 нед. |
| Procfs | /proc — информация о процессах | 1 нед. |
| Sysfs-подобный | /sys — информация о драйверах | 1 нед. |
| File locking | POSIX advisory locks, flock | 1 нед. |
| Async I/O | io_uring-подобный или POSIX AIO | 2 нед. |
| Inotify-подобный | File change notification | 1 нед. |

**Критерии завершения:**
- [ ] mount: ext2 + FAT32 + POLER-FS одновременно
- [ ] /dev/null, /dev/zero, /dev/random работают
- [ ] /proc/self/maps показывает корректные VMA
- [ ] pipe(): `ls | grep` работает в shell
- [ ] Symlink: создание и переход по symbolic link

---

## Фаза 13: Win32 подсистема (PE loader, DLL loader, basic Win32)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф11, Ф12 |
| Приоритет | P2 |
| Сложность | 9/10 |
| Затраты | 30 нед. |
| Риск | **КРИТИЧЕСКИЙ** |

> **Это проект внутри проекта. Wine потратил 30 лет на Win32 совместимость.**

**Архитектурное решение: Shim vs. Kernel-native**

В текущих документах POLER-OS есть противоречие:
- `ROADMAP.md` описывает **shim-архитектуру** (userspace-переводчик)
- `README.md` заявляет **kernel-native** обработку Win32 syscall

**Рекомендация:** Shim-архитектура — единственный реалистичный путь.

```
Причина: Win32 API — это ~10 000 функций.
Если реализовывать их в ядре — ядро раздуется до Windows NT масштабов.
Shim позволяет:
  1. Держать ядро маленьким и проверяемым
  2. Обновлять совместимость без пересборки ядра
  3. Изолировать баги совместимости (crash shim ≠ crash kernel)
```

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| PE/COFF loader | Парсинг PE32+/PE32, секции, relocations | 3 нед. |
| DLL loader | Import resolution, IAT patching | 2 нед. |
| Win32 kernel32 shim | CreateFile, ReadFile, WriteFile, CloseHandle, etc. | 6 нед. |
| Win32 user32 shim | MessageBox, CreateWindowEx, DefWindowProc, etc. | 4 нед. |
| Win32 gdi32 shim | Базовый GDI: BitBlt, TextOut, SelectObject | 4 нед. |
| Virtual registry | HKEY_LOCAL_MACHINE и др., per-capsule | 2 нед. |
| Path translation | C:\ → /capsules/{pid}/C/, NT paths | 1 нед. |
| CRT compatibility | MSVCRT-подобная libc для PE binaries | 4 нед. |
| SEH | Structured Exception Handling (Windows) | 2 нед. |
| TLS | Thread Local Storage (Windows) | 1 нед. |

**Критерии завершения:**
- [ ] PE loader: загрузка и запуск консольного .exe (hello.exe)
- [ ] kernel32 shim: CreateFile/ReadFile/WriteFile работают
- [ ] user32 shim: MessageBoxW отображает окно
- [ ] Registry: RegOpenKey/RegSetValue работают
- [ ] SEH: try/except в C++ обрабатывает исключение

---

## Фаза 14: NT API (ntdll, Object Manager, Registry, SRM, ALPC)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф13 |
| Приоритет | P2 |
| Сложность | **10/10** |
| Затраты | **40+ нед.** |
| Риск | **КРИТИЧЕСКИЙ** |

> **NT API — это undocumented territory.** Большинство вызовов ntdll не документировано Microsoft. ReactOS потратил 25+ лет и до сих пор не имеет полной реализации.

**Компоненты NT Executive:**

| Компонент | Описание | Сложность | Время |
|-----------|----------|-----------|-------|
| ntdll.dll shim | NtCreateFile, NtReadFile, NtWriteFile, NtClose... | 7 | 6 нед. |
| Object Manager | HANDLE → object mapping, object namespace | 8 | 4 нед. |
| Registry (реальный) | Бинарный hive формат, per-hive mapping | 6 | 3 нед. |
| SRM (Security Ref. Monitor) | Access tokens, SID, ACE, ACL, privilege check | 9 | 6 нед. |
| ALPC | Advanced Local Procedure Call (LPC на стероидах) | 7 | 4 нед. |
| Process/Thread API | NtCreateProcess, NtCreateThread, etc. | 6 | 3 нед. |
| Section API | NtCreateSection, NtMapViewOfSection | 5 | 2 нед. |
| I/O Manager | IRP, device stack, driver loading | 8 | 6 нед. |
| Token/Impersonation | NtDuplicateToken, NtSetInformationProcess | 6 | 3 нед. |

**Критерии завершения:**
- [ ] NtCreateFile/NtReadFile/NtWriteFile: базовый файловый I/O
- [ ] Object Manager: handle allocation, lookup, reference counting
- [ ] SRM: access check по SD (Security Descriptor)
- [ ] ALPC: client-server communication между процессами
- [ ] Registry: загрузка и чтение Windows hive-формата

---

## Фаза 15: Совместимость приложений (первые Windows-приложения)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф14 |
| Приоритет | P2 |
| Сложность | 8/10 |
| Затраты | 20 нед. |
| Риск | **КРИТИЧЕСКИЙ** |

**Целевые приложения (по возрастанию сложности):**

| Уровень | Приложение | API-требования | Время |
|---------|-----------|----------------|-------|
| 1 | cmd.exe | kernel32 basic, console | 2 нед. |
| 2 | notepad.exe | kernel32 + basic user32 + GDI | 3 нед. |
| 3 | 7-zip console | kernel32 full, file I/O, console | 2 нед. |
| 4 | PuTTY | kernel32 + ws2_32 (network) | 3 нед. |
| 5 | Notepad++ | kernel32 + user32 + comctl32 | 4 нед. |
| 6 | Firefox/Chrome | COM, OLE, Shell API, DirectX, ... | 💀 |

**Anti-cheat совместимость:**

> Заявленная цель: «anti-cheat compatibility through correct API implementation, not bypasses»

**Реальность:** Античиты используют:
1. **Kernel drivers** (EAC, BattlEye) — load kernel module → IMPOSSIBLE в POLER-OS (immutable kernel)
2. **Direct hardware access** — /dev/mem, MSR reads → нужны stub'ы
3. **Hypervisor detection** — CPUID checks → нужны правдивые ответы
4. **Integrity checks** — hash own code, verify sections → shim должен быть transparent
5. **NMI hooks** — Non-Maskable Interrupt → в POLER-OS это патч ядра = невозможно

**Вердикт:** Античиты уровня kernel-mode (EAC, BattlEye, Vanguard) **не будут работать** на POLER-OS по архитектурным причинам. Это не баг — это следствие immutable kernel. User-mode античиты (VAC, некоторые конфигурации EAC) — возможны при достаточной совместимости.

**Критерии завершения:**
- [ ] cmd.exe: запуск, выполнение команд dir, copy
- [ ] notepad.exe: открытие и редактирование текстового файла
- [ ] 7-zip: создание и распаковка архива
- [ ] Консольное приложение с сетью: подключение к серверу

---

## Фаза 16: Инструменты разработчика (compiler, debugger, build system)

| Параметр | Значение |
|----------|----------|
| Зависимости | Ф11 |
| Приоритет | P2 |
| Сложность | 6/10 |
| Затраты | 16 нед. |
| Риск | СРЕДНИЙ |

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| GCC/Zig порт | Кросс-компилятор → нативный компилятор | 4 нед. |
| GDB stub | Удалённая отладка через serial/network | 2 нед. |
| Make/CMake | Минимальный build tool | 2 нед. |
| Пакетный менеджер | Установка/удаление пакетов, подписи | 4 нед. |
| strace-подобный | Трассировка syscall'ов | 1 нед. |
| Performance profiler | Sampling profiler | 2 нед. |
| Kernel debugger | KDB — interactive kernel debugger | 4 нед. |

**Критерии завершения:**
- [ ] Нативная компиляция: `zig build` работает внутри POLER-OS
- [ ] GDB: подключение к POLER-OS VM, точки останова, step
- [ ] Пакетный менеджер: install/remove/search пакетов

---

## Фаза 17: Тестовая инфраструктура

| Параметр | Значение |
|----------|----------|
| Зависимости | Все предыдущие |
| Приоритет | P1 |
| Сложность | 4/10 |
| Затраты | 6 нед. |
| Риск | СРЕДНИЙ |

**Задачи:**

| Компонент | Описание | Время |
|-----------|----------|-------|
| Unit test framework | Zig native tests + kernel unit tests | 1 нед. |
| QEMU automation | CI: boot → run tests → collect results | 1 нед. |
| Kernel fuzzing | Syscall fuzzing, filesystem fuzzing | 2 нед. |
| Regression suite | Автоматические тесты для каждого PR | 1 нед. |
| Stress testing | SMP, memory, scheduler stress | 1 нед. |

**Критерии завершения:**
- [ ] CI: каждый коммит проходит через boot test в QEMU
- [ ] 100+ unit тестов ядра
- [ ] Fuzzer находит 0 crash за 24 часа работы
- [ ] Stress test: 1 час без panic

---

## Фаза 18: Стадии релиза

| Параметр | Значение |
|----------|----------|
| Зависимости | Все |
| Приоритет | P3 |
| Сложность | 3/10 |
| Затраты | 8 нед. |
| Риск | НИЗКИЙ |

| Стадия | Критерии | Время |
|--------|----------|-------|
| **Alpha** | Boot + SMP + VFS + ext2 + shell + libc | ~30 нед. от сейчас |
| **Beta** | + Network + AHCI + basic graphics + PE loader | ~60 нед. от сейчас |
| **RC** | + Win32 basic apps + POLER-FS + installer | ~100 нед. от сейчас |
| **1.0** | + Anti-cheat user-mode + NT API subset + stable | ~150+ нед. от сейчас |

> **1.0 = ~3 года для одного разработчика.** Это оптимистичная оценка.

---

# ЧАСТЬ 2: КРИТИЧЕСКИЙ АУДИТ

> **Предупреждение:** Этот раздел — не поощрение, а холодный анализ реальности.
> Если что-то звучит жестоко — это потому, что правда жестока.

---

## 1. Слабые места — Что сломается первым

### 1.1. TLB Shootdown (P0 КРИТИЧЕСКИЙ)

**Проблема:** SMP реализован без TLB shootdown. Когда CPU 0 меняет CR3
(например, mapPage/unmapPage), CPU 1 использует устаревший TLB.

**Последствия:**
- Page table corruption
- Use-after-free через stale TLB entry
- Неуловимые баги, проявляющиеся только под нагрузкой

**Текущий статус:** Отложен до v2.0 — **это ошибка**.

```
Фикс: При каждом mapPage/unmapPage:
  1. Запретить прерывания
  2. Отправить IPI_TLB_SHOOTDOWN всем другим CPU
  3. Каждый CPU выполняет INVLPG или reload CR3
  4. Ждать ACK от всех CPU
  5. Разрешить прерывания
```

### 1.2. Scheduler: MAX_TASKS = 8

**Проблема:** Жёсткий лимит в 8 задач. Реальная ОС требует 100-1000+ процессов.

**Последствия:** Невозможно запустить даже минимальный user space.

**Фикс:** Dynamic allocation из heap — тривиально, но требует переписывания scheduler.

### 1.3. FAT32 без VFS

**Проблема:** FAT32 работает, но это единственная ФС, и она не через VFS.
Когда появится ext2 и POLER-FS — всё нужно переписывать.

**Последствия:** Каждый новый FS драйвер = дублирование кода.

**Фикс:** Сначала VFS, потом всё остальное.

### 1.4. Нет page fault handler для demand paging

**Проблема:** Все страницы маппируются заранее. Нет copy-on-write для fork().
Нет lazy allocation.

**Последствия:** fork() невозможен. Каждый процесс занимает максимум памяти.

### 1.5. Нет AHCI/NVMe драйвера

**Проблема:** Единственный блочный драйвер — VirtIO-BLK (работает только в QEMU).
На реальном железе — некуда монтировать ФС.

**Последствия:** POLER-OS работает только в VM.

### 1.6. Heap integrity ≠ Heap security

**Проблема:** SipHash tags в heap защищают от cookie forgery, но НЕ от:
- Use-after-free (ядро не обнуляет freed блоки)
- Heap overflow (нет guard pages)
- Double-free (нет quarantine)

**Фикс:** Добавить freed-page poisoning (0xDE pattern) и quarantine.

---

## 2. Архитектурные ошибки

### 2.1. Противоречие: Shim vs. Kernel-native

**Проблема:** Документация противоречит сама себе.

- `ROADMAP.md`: «Shim-библиотеки в пользовательском пространстве»
- `README.md`: «Ядро нативно понимает PE/COFF и обрабатывает Win32/64 системные вызовы напрямую»

Это два **фундаментально разных** архитектурных решения:

| Аспект | Kernel-native | Shim |
|--------|---------------|------|
| Сложность ядра | Огромная (NT-like) | Маленькая |
| Безопасность | Bug в Win32 = kernel exploit | Bug в shim = userspace crash |
| Производительность | Быстрее (1 ring switch) | Медленнее (shim overhead) |
| Совместимость | Неизбежные пробелы | Пошаговое наращивание |
| Риск | КРИТИЧЕСКИЙ | УПРАВЛЯЕМЫЙ |

**Рекомендация:** Shim — единственный реалистичный путь. Kernel-native Win32 — это
по сути написание Windows NT с нуля. Microsoft потратила на это 30+ лет и
тысячи инженеров.

### 2.2. Immutable kernel = нет loadable modules

**Проблема:** Заявлено: «После загрузки ядро криптографически блокирует возможность
модификации самого себя.»

**Следствия:**
1. Нет loadable kernel modules (LKM) — все драйверы должны быть в ядре
2. Нет hotplug драйверов — USB device? Нужен драйвер в ядре заранее
3. Нет обновления ядра без перезагрузки
4. Anti-cheat kernel drivers НЕВОЗМОЖНЫ — а вы хотите совместимость

**Это противоречие:** Immutable kernel + anti-cheat compatibility = несовместимые цели.

```
Anti-cheat → kernel driver → load module → modify kernel → VIOLATES immutability

Решения:
  A) Отказаться от immutable kernel для совместимости
  B) Сохранить immutable kernel и принять, что kernel anti-cheat не работает
  C) Гибрид: immutable code + data sections, но разрешить подписанные модули
```

**Рекомендация:** Вариант C — подписанные модули с верификацией.
Ядро immutable по коду, но разрешает загрузку подписанных модулей в
изолированное адресное пространство (подобно gVisor user-space kernel).

### 2.3. Криптография в ядре — over-engineering

**Проблема:** PND v8 — собственная криптографическая хеш-функция.
RSA-OAEP + POLER-CTR AEAD — собственный AEAD режим.

**Риски:**
1. **NIST-approved?** — Нет. Ни один стандарт не признаёт PND.
2. **Peer review?** — Нулевой. Криптография без peer review = insecure by default.
3. **Side-channel resistant?** — Непроверено.
4. **POLER-CTR** — собственный AEAD. Почему не AES-256-GCM или ChaCha20-Poly1305?

```
Правило криптографии: НЕ ПИШИ СОБСТВЕННУЮ.

Шифр    → AES-256-GCM (hardware AES-NI) или ChaCha20-Poly1305
Хеш     → SHA-256/SHA-3 или BLAKE2b/BLAKE3
Подпись → Ed25519 или RSA-PSS
PRF     → HMAC-SHA256

PND v8 может быть интересной академической работой,
но не должна использоваться для безопасности продакшн-системы.
```

**Рекомендация:** Реализовать AES-256-GCM (с AES-NI) и ChaCha20-Poly1305.
PND v8 — оставить как экспериментальную опцию, не по умолчанию.

### 2.4. Нет чёткого syscall ABI

**Проблема:** Syscall конвенция не специфицирована как стабильный интерфейс.

Текущие syscall'ы: print, read_key, clear_screen (всего 3!).
Нет документации на syscall numbers, register convention, error handling.

**Фикс:** Определить и задокументировать стабильный syscall ABI ПЕРЕД написанием libc.

```zig
// Предлагаемый syscall ABI для x86_64
// Номер: RAX
// Аргументы: RDI, RSI, RDX, R10, R8, R9 (как Linux)
// Возврат: RAX = результат или -errno
// Error: RAX = -errno (отрицательное число)

pub const Syscall = enum(u64) {
    read = 0,
    write = 1,
    open = 2,
    close = 3,
    mmap = 4,
    munmap = 5,
    // ... до ~200
};
```

---

## 3. Альтернативные решения

### 3.1. Zircon-подобное микроядро вместо монолита

**Текущий подход:** Монолитное ядро (как Linux).

**Альтернатива:** Микроядро с capability-based security (как Zircon/Fuchsia).

| Аспект | Монолит | Микроядро (Zircon-like) |
|--------|---------|------------------------|
| Безопасность | Драйвер в kernel = полный доступ | Драйвер в userspace = изоляция |
| Immutable kernel | Трудно (драйверы в ядре) | Естественно (ядро маленькое) |
| Производительность | Быстрее (нет IPC overhead) | Медленнее на syscalls |
| Сложность | Проще начать | Сложнее спроектировать |
| Отладка | Kernel panic = всё | Crash драйвера ≠ crash системы |

**Рекомендация:** Для заявленных целей (безопасность, immutable kernel,
совместимость через shim) микроядро — более естественный выбор.
Но переход на микроядро = переписывание с нуля. Оставить монолит,
но с чётким разделением: ядро — только memory/sched/IPC, драйверы —
в userspace через RPC.

### 3.2. Wine-подход вместо kernel-native

Уже обсуждалось выше. Кратко: **Shim = единственный реалистичный путь.**

### 3.3. Mesa3D fork вместо написания GPU драйвера с нуля

**Текущий план:** Написать Vulkan драйвер с нуля.

**Альтернатива:** Fork Mesa3D и адаптировать под POLER-OS.

| Аспект | С нуля | Fork Mesa3D |
|--------|--------|-------------|
| Время | 40+ недель | 10-15 недель (адаптация) |
| Качество | Низкое (buggy) | Высокое (20 лет разработки) |
| Поддержка GPU | 1-2 | 10+ (Intel, AMD, Qualcomm) |
| Риск | КРИТИЧЕСКИЙ | УПРАВЛЯЕМЫЙ |
| Зависимость | Нет | LGPL 2.1 лицензия |

**Рекомендация:** Fork Mesa3D. Это не «подглядывание» — это open-source.
Linux, FreeBSD, ChromeOS — все используют Mesa. POLER-OS тоже должен.

### 3.4. musl libc вместо написания с нуля

**Текущий план:** Написать libc с нуля.

**Альтернатива:** Адаптировать musl (MIT лицензия).

**Рекомендация:** musl. Это MIT-лицензированная, чистая, компактная libc.
Адаптация syscall обёрток — 2 недели vs. 12+ недель написания с нуля.

---

## 4. Масштаб проекта — Сравнение

### 4.1. Размер кодовой базы

| Проект | LOC | Разработчики | Годы | Результат |
|--------|-----|-------------|------|-----------|
| **POLER-OS (текущий)** | ~5K | 1 | ~0.5 | v0.7.0 boot+SMP |
| **Linux 1.0 (1994)** | 170K | 100+ | 3 | Базовый Unix |
| **Linux 6.x (2024)** | 35M | 2000+ | 33 | Полный Unix |
| **FreeBSD 14** | 12M | 500+ | 30 | Полный Unix |
| **ReactOS 0.4.14** | 10M | 50+ | 25 | Partial Win32 |
| **Haiku R1** | 8M | 100+ | 22 | Полный BeOS compat |
| **Redox OS** | 500K | 10+ | 9 | Микроядро, basic |
| **SerenityOS** | 2M | 50+ | 6 | Полный Unix desktop |
| **Windows NT 3.1** | ~5M | 500+ | 4 | Полный Win32 |

### 4.2. Что POLER-OS хочет реализовать

POLER-OS хочет = (Linux-подобный Unix) + (ReactOS Win32 compat) + (Vulkan GPU) + (POLER-FS crypto FS) + (immutable kernel security)

**Это эквивалент: Linux + ReactOS + Mesa3D + LUKS2 + SELinux**

**Суммарная LOC: ~50M+**
**Суммарные человеко-годы: 500+**

### 4.3. Реалистичная оценка

| Scope | LOC | Человеко-годы (команда 3) |
|-------|-----|--------------------------|
| Минимальный (boot + SMP + VFS + shell) | 50K | 1.5 |
| Базовый (+ net + AHCI + libc) | 200K | 3 |
| Продвинутый (+ graphics + PE loader) | 500K | 5 |
| Полный (+ NT API + Vulkan + anti-cheat) | 2M+ | 15+ |

> **Вывод:** Полный scope POLER-OS — это 15+ человеко-лет.
> Команда 3 человека — это 5+ лет.
> Один разработчик — это 15+ лет (и это без учёта выгорания).

---

## 5. Реалистичные компоненты — Что МОЖНО построить малой командой

### ✅ Реалистично (малая команда, 1-3 года)

| Компонент | LOC | Время | Сложность |
|-----------|-----|-------|-----------|
| Boot + HAL + memory | 10K | ✅ Готово | 5/10 |
| SMP + spinlocks | 5K | ⚠️ Почти | 7/10 |
| VFS + ext2 | 15K | 3 мес. | 7/10 |
| Scheduler v2 | 5K | 2 мес. | 6/10 |
| AHCI/SATA driver | 3K | 1.5 мес. | 6/10 |
| libc (musl port) | 20K (адаптация) | 2 мес. | 5/10 |
| Shell + core utils | 5K | 1 мес. | 4/10 |
| Network stack (basic TCP) | 15K | 4 мес. | 8/10 |
| Compositor (CPU) | 8K | 2 мес. | 6/10 |
| PE loader + basic kernel32 shim | 10K | 3 мес. | 7/10 |

### ⚠️ Трудно, но возможно (3-5 лет)

| Компонент | LOC | Время | Сложность |
|-----------|-----|-------|-----------|
| POLER-FS | 20K | 6 мес. | 9/10 |
| USB stack | 15K | 4 мес. | 8/10 |
| NT Object Manager + SRM | 15K | 4 мес. | 9/10 |
| Win32 user32/gdi32 shim | 30K | 6 мес. | 9/10 |
| Vulkan (VirtIO-GPU + Intel basic) | 40K | 12 мес. | 10/10 |
| DXVK порт | 50K | 8 мес. (адаптация) | 9/10 |

### ❌ Нереально для малой команды

| Компонент | Причина |
|-----------|---------|
| AMD/NVIDIA Vulkan driver | Закрытые спецификации + firmware |
| DirectX 12 native | 100K+ LOC, спецификация не публична |
| Kernel anti-cheat compatibility | Противоречит immutable kernel |
| Full NT I/O Manager (IRP stack) | Сложность 10/10, недокументировано |
| Browser engine (для Chrome/Firefox) | 10M+ LOC |
| Full Win32 compat (Level 6 apps) | Wine = 30 лет, 4M+ LOC |

---

## 6. Многолетние компоненты — 5+ лет независимо от команды

| Компонент | Почему | Минимум |
|-----------|--------|---------|
| **Vulkan 1.2+ conformant** | Спецификация 2600 страниц, GPU-specific, нужен Mesa-подобный инфраструктурный слой | 5 лет (команда 5+) |
| **Full Win32 compat** | Wine: 30 лет, 4M+ LOC, и до сих пор не 100% | 10+ лет (команда 10+) |
| **NT API full** | ~2000 недокументированных системных вызовов | 7+ лет |
| **POLER-FS production-ready** | AES-256-XTS + B+tree + BLAKE3 + NT ACL + NVMe optimization — каждый компонент сам по себе сложен | 3-4 года (1 чел.) |
| **Anti-cheat compat (kernel)** | Каждый античит уникален, обновляется постоянно, использует undocumented API | Бесконечно |
| **Browser** | Самый сложный тип приложения в мире | 20+ лет (команда 100+) |

---

## 7. Упрощения — Что можно радикально упростить

### 7.1. Vulkan → VirtIO-GPU + Mesa software rendering

**Вместо:** Полноценный Vulkan драйвер для каждого GPU
**Сделать:**
1. VirtIO-GPU для QEMU (2 мес.)
2. Mesa3D llvmpipe (software rendering) для совместимости (1 мес. адаптации)
3. Intel i915 KMS для display output на реальном железе (2 мес.)
4. Позже: Mesa Vulkan driver port (6 мес. адаптации)

**Экономия:** 20+ недель.

### 7.2. NT API → Win32 API shim (NT как деталь реализации)

**Вместо:** Полная реализация NT Executive (Object Manager, SRM, ALPC, I/O Manager)
**Сделать:**
1. Реализовать Win32 API (kernel32, user32, gdi32) через shim
2. Shim транслирует Win32 в POLER-OS native ABI
3. NT API (ntdll) — только те вызовы, которые реально нужны приложениям
4. Object Manager → POLER-OS handle table (проще)
5. SRM → POSIX-like capability system (проще)

**Экономия:** 30+ недель.

### 7.3. POLER-FS → ext4 + encryption layer

**Вместо:** POLER-FS с нуля (B+tree, AES-256-XTS, BLAKE3, NT ACL)
**Сделать:**
1. ext4 как основная ФС (проверенная, совместимая с Linux для разработки)
2. Encryption layer поверх VFS (как eCryptfs / fscrypt)
3. BLAKE3 integrity как optional mount option
4. NT ACL как VFS xattr (расширенные атрибуты)

**Экономия:** 10+ недель. И при этом — совместимость с Linux для отладки.

### 7.4. USB → только xHCI + HID + mass storage

**Вместо:** Полный USB stack (UHCI, OHCI, EHCI, xHCI + все классы)
**Сделать:**
1. Только xHCI (все современные системы)
2. Только HID (keyboard, mouse) и mass storage
3. Позже: audio, printer, etc.

**Экономия:** 8+ недель.

### 7.5. Сеть → lwIP вместо написания с нуля

**Вместо:** Свой TCP/IP стек (16 недель)
**Сделать:**
1. Порт lwIP (lightweight IP) — 40K LOC, BSD license
2. Адаптация драйвера network interface
3. Socket API shim поверх lwIP

**Экономия:** 10+ недель.

### 7.6. Отказ от собственной криптографии

**Вместо:** PND v8 + POLER-CTR
**Сделать:**
1. AES-256-GCM (с AES-NI hardware acceleration — в 100 раз быстрее)
2. ChaCha20-Poly1305 (для систем без AES-NI)
3. SHA-256/BLAKE3 для хеширования
4. Ed25519 для подписей

**Экономия:** 4+ недели. И главное — **реальная безопасность** вместо иллюзии.

---

## СВОДНАЯ ТАБЛИЦА УПРОЩЕНИЙ

| Компонент | Было | Стало | Экономия | Риск упрощения |
|-----------|------|-------|----------|----------------|
| GPU | Vulkan с нуля | VirtIO-GPU + Mesa3D | 20 нед. | Низкий |
| Win32 | Kernel-native | Shim (userspace) | 30 нед. | Средний |
| NT API | Full Executive | Win32 shim + basic ntdll | 20 нед. | Средний |
| ФС | POLER-FS с нуля | ext4 + encryption layer | 10 нед. | Низкий |
| USB | Full stack | xHCI + HID + mass | 8 нед. | Низкий |
| Сеть | Свой TCP/IP | lwIP port | 10 нед. | Низкий |
| Крипто | PND + POLER-CTR | AES-GCM + ChaCha20 | 4 нед. | Нулевой |
| libc | С нуля | musl port | 6 нед. | Низкий |
| **Итого** | | | **~108 нед.** | |

> **108 недель экономии = 2 года.**
> Это разница между «проект умрёт через 2 года от выгорания»
> и «проект доживёт до usable beta».

---

## РЕКОМЕНДУЕМАЯ ДОРОЖНАЯ КАРТА (после упрощений)

```
Год 1: Фундамент (0-52 нед.)
├── Q1: SMP тест + TLB shootdown + scheduler v2 + fork/exec
├── Q2: VFS + ext2 + AHCI driver
├── Q3: musl libc port + shell + core utils + init
├── Q4: lwIP network stack + TCP + socket API

Год 2: User Space + Совместимость (53-104 нед.)
├── Q1: Compositor (CPU) + input subsystem
├── Q2: PE loader + kernel32 shim + basic console apps
├── Q3: user32/gdi32 shim + GUI apps (notepad)
├── Q4: POLER-FS design + implementation (encryption layer)

Год 3: Производительность + Стабильность (105-156 нед.)
├── Q1: VirtIO-GPU + Mesa3D software rendering
├── Q2: NVMe driver + POLER-FS optimization
├── Q3: xHCI USB + HID + mass storage
├── Q4: Alpha release + community testing

Год 4: Совместимость + Релиз (157-208 нед.)
├── Q1: ntdll shim + NT Object Manager basics
├── Q2: DirectX translation (DXVK port)
├── Q3: Beta release + anti-cheat user-mode analysis
├── Q4: Stabilization + documentation + 1.0 preparation
```

---

## ФИНАЛЬНЫЙ ВЕРДИКТ

### Что хорошо:
1. ✅ Ядро реально загружается и работает (это больше, чем у 90% OS-проектов)
2. ✅ Ring 3 с per-process CR3 — правильная архитектура
3. ✅ SMP код написан (нужна отладка)
4. ✅ FAT32 R/W — работает
5. ✅ Криптография есть (хотя и сомнительная)
6. ✅ Zig — отличный выбор для OS (comptime, no hidden control flow)

### Что критично:
1. 🔴 TLB shootdown отложен — это не «v2.0», это P0
2. 🔴 Нет AHCI/NVMe — OS работает только в QEMU
3. 🔴 Противоречие в архитектуре (shim vs. kernel-native)
4. 🔴 Immutable kernel + anti-cheat = несовместимые цели
5. 🔴 Собственная криптография без peer review = insecure

### Что смертельно (если не исправить):
1. 💀 Vulkan с нуля — убьёт проект (40+ недель, 10/10 сложность)
2. 💀 Full NT API — убьёт проект (40+ недель, недокументировано)
3. 💀 Оценка в 252 недели (5 лет) для одного разработчика — без упрощений
    проект не доживёт до beta

### Ключевая рекомендация:

> **СУЩЕСТВУЙТЕ, А НЕ СОВЕРШЕНСТВУЙТЕСЬ.**
>
> Работающая ОС с 80% совместимостью — это в 100 раз ценнее,
> чем идеальная ОС, которая существует только в roadmap.
>
> Используйте существующие open-source компоненты (musl, lwIP, Mesa3D).
> Не пишите с нуля то, что другие уже написали за 20 лет.
> Сфокусируйтесь на том, что уникально: immutable kernel + shim архитектура.
>
> Первая цель: **запустить notepad.exe**.
> Не Vulkan. Не AAA-игры. Notepad.
> Когда notepad работает — всё остальное — вопрос времени.
> Когда не работает даже hello.exe — Vulkan не нужен.

---

*Архитектура диктует правила. Но правила должны быть реалистичными.*

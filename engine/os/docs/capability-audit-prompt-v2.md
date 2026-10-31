# POLER-OS Capability-Based Security: Критический архитектурный аудит

## Роль

Ты — ведущий архитектор операционных систем, специалист по микроядрам, capability-based security, безопасной виртуализации, языку Zig и проектированию ОС для интеграции с AI. Выполни критический архитектурный аудит предложенной модели и разработай практический план её реализации без необоснованных предположений.

## Контекст проекта

Разрабатывается **POLER-OS** — собственная ОС на Zig 0.13.0 с dual-personality ядром (NT+POSIX). Кодовая база уже содержит ~20 000 строк рабочего кода. Проект не начинается с нуля — необходимо эволюционировать существующую архитектуру, а не перепроектировать с чистого листа.

### Существующая архитектура (реализовано)

**Рабочие компоненты:**

| Компонент | Файл | Статус |
|-----------|------|--------|
| HAL (GDT/IDT/APIC/PIC/Serial/PS2) | `hal.zig` | 100% — GDT Ring 0+3, IDT 256 записей, APIC timer, COM1 serial, PS/2 keyboard |
| Scheduler (preemptive Round-Robin) | `scheduler.zig` | 100% — APIC timer tick, MAX_TASKS=8, Ring 0+3, CR3 switching, FS_BASE MSR |
| Physical Memory Manager | `pmm64.zig` | 100% — bitmap + per-page refcount (atomic), contiguous alloc |
| Virtual Memory Manager | `vmm64.zig` | 95% — 4-level paging, per-process PML4, COW fork, TLB shootdown IPI |
| Kernel Heap | `heap64.zig` | 100% — SipHash-2-4 integrity tags на каждом блоке |
| ELF64 Loader | `elf_loader.zig` | 90% — ET_EXEC, PIE/ET_DYN, per-process PML4 loading |
| Dynamic Linker (ld-poler) | `dynlinker.zig` | 85% — PLT lock, TLS, GOT, DT_NEEDED, weak symbols, symbol versioning |
| VirtIO-BLK Driver | `virtio_blk.zig` | 90% — split virtqueue, polling I/O, IRQ vector 49 |
| FAT32 Filesystem | `fat32.zig` | 90% — read/write/create/delete, LFN, cluster allocation |
| Framebuffer | `framebuffer.zig` | 80% — VBE, 8x16 bitmap font |
| PCI Bus | `pci.zig` | 90% — enumeration, device discovery |
| ACPI | `acpi.zig` | 85% — RSDP/RSDT/MADT/MCFG |
| SMP | `smp.zig` | 80% — AP trampoline, per-CPU GSBASE (APs idle) |
| Shell | `main64.zig` | 70% — ls/cat/mkdir/touch/write/rm/disk/help/about/clear/poler |
| Syscall Dispatcher | `syscall_integration.zig` | 95% — SYSCALL/SYSRET, legacy + subsystem dispatch |
| Subsystem Dispatcher | `subsystem.zig` | 95% — NT(0x1000+)/POSIX(0x0000+)/POLER(0x2000+) routing |
| NT API | `nt_api.zig` | 40% — file I/O, process, memory, sync work; registry/LPC/token stubs |
| POSIX API | `posix_api.zig` | 50% — core I/O, process, memory; no networking |
| Object Manager | `object_manager.zig` | 85% — 29 object types, handle table (4096), unified namespace |
| Process Manager + COW fork | `kernel_integrate.zig` | 90% — create/fork/exec/terminate, per-process PML4 |
| **ACL/Capability System** | `kernel_integrate.zig` | **85% — СУЩЕСТВУЕТ, требует аудита** |
| POLER Crypto Core | `poler_core.zig` | 100% — PND mixing, 20-round Feistel, S-box |
| RSA-OAEP | `rsa_oaep.zig` | 100% — RSA-2048, SHA-256, MGF1, OAEP padding |
| CPIO Parser | `cpio.zig` | 100% — initrd parsing |

**НЕРЕАЛИЗОВАННЫЕ компоненты:**
- IPC/LPC (только номера NtXxx, реализация 5%)
- Network stack (socket/connect возвращают -ENOSYS)
- Registry (только номера NtXxx)
- Token/Security Descriptors (только номера NtXxx)
- POLER token verification (MAC вычисляется, но НЕ проверяется по сертификату)

### Существующая модель безопасности (ЧТО УЖЕ ЕСТЬ — АУДИРУЕТСЯ)

#### 1. ACL Capability Mask (64-bit)

```zig
// kernel_integrate.zig, строки 790-805
pub const CAP_FILE_READ: u64      = 1 << 0;
pub const CAP_FILE_WRITE: u64     = 1 << 1;
pub const CAP_FILE_EXECUTE: u64   = 1 << 2;
pub const CAP_PROCESS_CREATE: u64 = 1 << 3;
pub const CAP_PROCESS_KILL: u64   = 1 << 4;
pub const CAP_MEMORY_MMAP: u64    = 1 << 5;
pub const CAP_NETWORK: u64        = 1 << 6;
pub const CAP_DEVICE: u64         = 1 << 7;
pub const CAP_REGISTRY: u64       = 1 << 8;
pub const CAP_SIGNAL: u64         = 1 << 9;
pub const CAP_PRIVILEGE: u64      = 1 << 10;
pub const CAP_RAW_IO: u64         = 1 << 11;
pub const CAP_ADMIN: u64          = 1 << 12;
// Bits 13-15: Reserved
pub const CAP_NT_API: u64         = 1 << 16;
pub const CAP_POSIX_API: u64      = 1 << 17;
pub const CAP_POLER_AUTH: u64     = 1 << 18;
// Bits 19-63: Reserved
```

- Default user capabilities: `FILE_READ|FILE_WRITE|FILE_EXECUTE|PROCESS_CREATE|MEMORY_MMAP|SIGNAL|NT_API|POSIX_API|POLER_AUTH`
- Kernel capabilities: `0xFFFFFFFFFFFFFFFF` (все)
- Fork: наследует capabilities родителя
- **Нет эскалации**: `processMgrSetCaps()` не может выдать caps сверх capabilities вызывающего

#### 2. Per-syscall capability mapping

```zig
// POSIX syscall → required caps (строки 816-839)
fn posixSyscallToCapabilities(syscall_num: u64) u64
// NT syscall → required caps (строки 843-867)
fn ntSyscallToCapabilities(nt_num: u64) u64
// POLER syscall → required caps (строки 870-879)
fn polerSyscallToCapabilities(poler_num: u64) u64
```

Каждый syscall проверяется: `(process_caps & required_caps) == required_caps`

#### 3. POLER Authentication Token

```zig
// ProcessControlBlock, строки ~265-280
poler_token: [32]u8,        // Per-process crypto token (32 bytes)
acl_capabilities: u64,      // 64-bit capability mask
acl_version: u32,           // Version counter for policy changes
```

Генерация: POLER PRF (PND mix + rotation) от PID + subsystem + parent token.
Аутентификация: `polerAuthenticate()` вычисляет action MAC = `POLER_PRF(token | syscall_num | subsystem | arg_hash)`.

#### 4. Audit Trail

```zig
const AclAuditEntry = struct {
    pid: u32,
    syscall_num: u64,
    required_caps: u64,
    process_caps: u64,
    result: AuthResult,   // Allowed | Denied | Unauthenticated | Audited
    timestamp: u64,        // TSC
};
// Circular buffer: 256 entries
```

Чувствительные операции (KILL, PRIVILEGE, ADMIN, RAW_IO, DEVICE) → Audited (разрешены, но залогированы).

#### 5. Heap Integrity

```zig
// heap64.zig — SipHash-2-4 tags на каждом блоке
// 128-bit key НЕ хранится в heap memory — нельзя подделать
// Проверка при kfree() — обнаруживает corruption
```

#### 6. Process Control Block

```zig
pub const ProcessControlBlock = struct {
    pid: u32,
    ppid: u32,
    state: ProcessState,         // Unused|Creating|Running|Zombie|Killed
    subsystem: SubsystemId,      // Native|NT|POSIX|Hybrid
    exit_code: i32,
    task_id: usize,
    cr3: u64,                    // Per-process PML4 physical
    entry_point: u64,
    posix: SignalState,          // 31 POSIX signals, handlers, pending/blocked
    fd_table: FdTable,           // 1024 entries → Object Manager handles
    cwd: [256]u8,
    cwd_len: usize,
    nt_process_handle: u64,
    poler_token: [32]u8,         // POLER crypto token
    acl_capabilities: u64,       // 64-bit capability bitmask
    acl_version: u32,            // Policy version counter
};
```

MAX_PROCESSES = 64.

#### 7. Scheduler Task (TCB)

```zig
pub const Task = struct {
    id: usize,
    state: TaskState,            // Ready|Running|Killed
    privilege: TaskPrivilege,    // Kernel(0)|User(3)
    rsp: u64,                    // Saved stack pointer → InterruptFrame
    kernel_stack: [8192]u8,      // 8KB kernel stack
    cr3: u64,                    // Per-process PML4 physical
    user_stack_top: u64,
    tcb_vaddr: u64 = 0,          // TLS Thread Control Block vaddr
    fs_base: u64 = 0,            // MSR_FS_BASE for %fs:0 TLS access
};
```

MAX_TASKS = 8. tcbAllocCallback зарегистрирован через dynlinker.

#### 8. Object Manager Handle Table

```zig
pub const HandleEntry = struct {
    in_use: bool,
    obj_type: ObjectType,        // 29 types: File, Directory, Event, Mutant, etc.
    access_mask: u32,            // NT ACCESS_MASK or POSIX mode
    ref_count: u32,
    object_data: u64,            // Generic data
    // Event/Mutant/Semaphore specific fields...
};
// MAX_HANDLES = 4096, spinlock-protected
// NT handles start at 4, POSIX fds start at 0 → same table
```

#### 9. Syscall Convention

```
SYSCALL instruction → syscall_entry (asm)
  RAX=syscall#, RDI=arg1, RSI=arg2, RDX=arg3, R10=arg4, R8=arg5, R9=arg6
  → zig_syscall_handler(arg1, arg2, arg3, arg4, syscall_num)
  → subsys.dispatch() → { POSIX handler | NT handler | POLER handler }
  → Внутри каждого handler: ki.polerAuthenticate() → ACL check
```

#### 10. Memory Layout

```
0x000000 - 0x0FFFFF   : Identity-mapped (kernel image, boot, multiboot)
0x100000 -             : Kernel code (linked at 0x100000)
0x100000 (PIE base)   : User code (loadElfIntoPML4_v2)
0x100080000           : User stack (4 pages = 16KB)
0x100100000           : User heap (brk, 16MB max)
0x200080000           : Thread stack
0x200000000-0x400000000 : mmap region
0x400000_000+         : Shared libraries (libc.so, libm.so, ...)
```

### Предложенная новая модель (ЧТО ПРЕДЛАГАЕТСЯ — ОЦЕНИТЬ)

Предлагается переход от 64-bit bitmask capabilities к полноценной capability-based security модели:

1. **Capability Token** — криптографически подписанная структура с:
   - action type, scope, max_priority, ttl, issuer, signature[32] (HMAC-SHA256)

2. **Доверенный Шлюз Семантики (Semantic Gateway)** — три слоя:
   - Слой 1: Capability Kernel (Zig) — каждый syscall требует capability-token
   - Слой 2: Policy Engine (привилегированный сервис) — выдаёт/отзывает capabilities
   - Слой 3: AI-капсула (Python Interpreter + poler_os module) — изолирована

3. **AI-капсула** с Python-интерпретатором, динамически обновляемым (пересоздание гостя)

4. **IPC через capability-passing** между слоями

### Критические архитектурные ограничения (из кодовой базы)

При анализе учитывай:

1. **Все shell-команды работают в Ring 0** — нет userspace для проверки ACL
2. **IPC не реализован** — LPC порты только номера, pipe возвращает -ENOSYS
3. **Нет сети** — socket/connect возвращают -ENOSYS
4. **POLER token MAC вычисляется, но НЕ проверяется** — цепочка доверия незавершена
5. **SMP APs idle** — глобальная очередь, не per-CPU run queue
6. **MAX_TASKS=8, MAX_PROCESSES=64** — очень ограничено
7. **Обработчики прерываний и syscall работают с interrupts enabled** (sti() в syscall handler)
8. **VirtIO-BLK polling I/O** — нет асинхронного I/O, нет DMA с прерыванием
9. **FAT32 без журналирования** — crash consistency не гарантирована
10. **Нет demand paging** — все страницы маппируются eagerly
11. **64-bit ACL mask** — грубая гранулярность (19 бит из 64), нет per-object ACL
12. **Object Manager access_mask** — u32, но НЕ проверяется при операциях
13. **Нет sandboxing механизма** — user процесс может обратиться к любому syscall, если у него есть capability
14. **Shell читает ввод через serial** — нет stdin/stdout в userspace

---

## ЗАДАНИЕ АУДИТА

Не принимай утверждения из описания за истину. Для каждого тезиса оцени его корректность с точки зрения современной архитектуры ОС. Учитывай, что кодовая база уже существует и содержит работающие компоненты — предложения должны быть эволюционными, а не революционными (если только революция не оправдана).

### 1. Архитектурная оценка

Оцени предложенную модель "Capability Token + Semantic Gateway + AI-капсула" с учётом:

- Насколько она соответствует capability-based security (сравни с seL4, Capsicum, Zircon, Barrelfish, CHERI, Genode)
- Какие идеи совпадают с подходами перечисленных систем
- Какие элементы корректны, а какие упрощены или потенциально ошибочны
- Какие важные механизмы отсутствуют
- **КРИТИЧЕСКИ**: Насколько предложенная модель совместима с существующей 64-bit bitmask ACL и ProcessControlBlock? Можно ли эволюционировать, или нужен разрыв?
- **КРИТИЧЕСКИ**: Предложенный HMAC-SHA256 signature в capability token — оправдан ли, или kernel object reference (как в seL4 CNode) безопаснее и быстрее?
- Как предложенная 3-слойная архитектура соотносится с существующей архитектурой `subsystem.zig` → `kernel_integrate.zig` → `hal.zig`?

### 2. Анализ модели угроз

Рассмотри каждую угрозу в контексте СУЩЕСТВУЮЩЕГО кода POLER-OS. Для каждой угрозы укажи:

- **Вероятность** (High/Medium/Low с обоснованием)
- **Последствия** (что конкретно ломается в POLER-OS)
- **Существующая защита** (что уже есть в коде)
- **Необходимая дополнительная защита**

Угрозы:

| # | Угроза | Специфика POLER-OS |
|---|--------|--------------------|
| 2.1 | Компрометация AI-агента | AI работает в userspace, но имеет capabilities. Что если AI-код скомпрометирован? |
| 2.2 | Выполнение произвольного кода в AI-капсуле | Python interpreter + poler_os module — поверхность атаки |
| 2.3 | Эскалация привилегий | Существующий `processMgrSetCaps()` не может превысить capabilities вызывающего — достаточно ли этого? |
| 2.4 | IPC-атаки | IPC НЕ реализован. Когда будет реализован — какие атаки возможны? |
| 2.5 | TOCTOU | Syscall handler проверяет ACL, затем выполняет. Между проверкой и выполнением context switch? |
| 2.6 | Confused Deputy | `polerAuthenticate()` проверяет capabilities процесса, но что если процесс передаёт свой handle другому? |
| 2.7 | Replay атак | Capability token с TTL — можно ли перепроиграть? |
| 2.8 | Подделка capability | Существующий poler_token — 32-byte PRF. HMAC-SHA256 в новой модели — нужно ли? |
| 2.9 | Утечки памяти | PMM refcounting + COW. Что при fork bomb с 64 процессами? |
| 2.10 | Side-channel (cache timing) | SipHash-2-4 в heap — constant-time? POLER S-box — constant-time. Достаточно ли? |
| 2.11 | DMA атаки | VirtIO-BLK DMA buffers identity-mapped (phys==virt). Вектор атаки? |
| 2.12 | Загрузчик | GRUB multiboot — можно ли подменить kernel image? |
| 2.13 | Драйверы | VirtIO-BLK работает в Ring 0. Компрометация драйвера = компрометация ядра |
| 2.14 | Обновление ядра | Нет механизма kexec/ksplice. Как обновить ядро без перезагрузки? |
| 2.15 | Обновление AI | Предложено через пересоздание капсулы. А если AI удерживает capabilities? |
| 2.16 | Физический доступ | Нет disk encryption, нет secure boot. |
| 2.17 | Цепочка доверия | POLER token MAC вычисляется, но НЕ проверяется. Как завершить chain of trust? |

### 3. Capability-модель

Проектируй capability-модель как **эволюцию** существующей 64-bit bitmask, а не как полную замену. Ответь на вопросы:

**3.1 Формат объекта**

Существующая модель: `u64` bitmask (19 бит определено, 45 зарезервировано).
Предложенная модель: struct с action, scope, ttl, signature[32].

- Какие поля действительно необходимы для POLER-OS?
- Какие лучше заменить другой архитектурой?
- Как мигрировать с u64 bitmask на новую модель без поломки 20K строк кода?
- Должна ли capability быть immutable или mutable?

**3.2 Делегирование и передача**

- Существующая модель: fork наследует capabilities. Нет явного делегирования.
- Нужна ли явная передача capabilities через IPC?
- Как реализовать делегирование: subset (только уменьшение прав) или расширение?
- CNode (как в seL4) или inline token?

**3.3 Отзыв (revocation)**

- Существующая модель: `processMgrSetCaps()` — только уменьшение.
- Нужен ли немедленный отзыв capability у работающего процесса?
- Как реализовать: indirect revocation (через CNode) или token invalidation?
- Влияние на производительность при 64 процессах

**3.4 Ограничение области действия (scope)**

- Существующая модель: CAP_FILE_READ — глобальный, без разделения файлов.
- Нужен ли per-object ACL (capability на конкретный файл/устройство)?
- Как это соотносится с Object Manager access_mask (u32, НЕ проверяется)?

**3.5 Срок действия (TTL)**

- Существующая модель: capabilities бессрочные.
- Нужен ли TTL? В каких тиках (scheduler_ticks? APIC ticks? boot time?)?
- Что происходит при истечении: silent deny? exception? audit?

**3.6 Поколения (generation)**

- Существующая модель: `acl_version: u32` — инкрементируется при изменении.
- Как синхронизировать version между процессом и ядром?
- Нужен ли generation per-capability или per-process?

**3.7 Сериализация и хранение**

- Capabilities хранятся в ProcessControlBlock (в kernel memory).
- Нужно ли сериализовать capabilities для IPC?
- Как сохранить capabilities при exec() (они сохраняются или сбрасываются)?

### 4. Криптографические подписи в capabilities

Отдельно оцени предложенный HMAC-SHA256 signature в capability token:

| Подход | Плюсы | Минусы | Применимость для POLER-OS |
|--------|-------|--------|--------------------------|
| HMAC-SHA256 (предложен) | | | |
| Цифровые подписи (RSA-OAEP, уже есть в poler_core) | | | |
| Kernel object references (seL4 CNode) | | | |
| Capability table (Zircon) | | | |
| Защищённые указатели (CHERI) | | | |
| Индексные дескрипторы (i386 segments) | | | |

Для каждого подхода:
- Когда криптография оправдана, а когда ядро может безопаснее работать без неё?
- Какой подход лучше сочетается с уже реализованным POLER crypto core (Feistel cipher, RSA-OAEP)?
- Какой подход минимально инвазивен к существующему коду?

### 5. Ядро

Предложи архитектуру КАК ЭВОЛЮЦИЮ существующих компонентов. Для каждого компонента укажи: что изменить, что добавить, что оставить как есть.

**5.1 Планировщик** (`scheduler.zig`, 393 строки)

Существующий: Preemptive Round-Robin, MAX_TASKS=8, APIC timer tick.
- Нужен ли per-CPU run queue (для SMP)?
- Как интегрировать capability check в schedule()?
- Достаточно ли 8 задач для AI-капсулы + user processes?

**5.2 Диспетчер syscall** (`syscall_integration.zig`, 139 строк)

Существующий: legacy (1-6) → subsys.dispatch() → NT/POSIX/POLER handlers.
- Где вставить capability check: в subsys.dispatch() или в каждом handler?
- Как передавать capability token через SYSCALL convention (RAX/RDI/RSI/RDX/R10/R8/R9)?

**5.3 Диспетчер capabilities** (НОВЫЙ компонент)

- Где разместить: отдельный файл `capability.zig` или расширить `kernel_integrate.zig`?
- Взаимодействие с существующим `polerAuthenticate()`
- Взаимодействие с Object Manager access_mask

**5.4 Менеджер памяти** (`pmm64.zig` + `vmm64.zig`)

Существующий: bitmap + refcount, 4-level paging, COW.
- Нужна ли per-process memory capability (quota)?
- Как защитить COW при fork от capability misuse?

**5.5 IPC** (НЕ РЕАЛИЗОВАН)

- Какой IPC最适合 capability-based model: synchronous (seL4) или asynchronous (Zircon)?
- Совместимость с NT LPC (номера уже определены в nt_api.zig)
- Совместимость с POSIX pipe/msgqueue

**5.6 Процессы и потоки** (`kernel_integrate.zig`)

Существующий: ProcessControlBlock + Task (scheduler).
- Нужно ли расширение TCB для capability storage?
- Как AI-капсула будет создаваться как процесс? (subsystem = .Native? Новый SubsystemId?)
- MAX_PROCESSES=64 — достаточно ли?

**5.7 Пространства имён**

Существующий: Object Manager с 29 типами, unified namespace.
- Как интегрировать capabilities в Object Manager?
- Должны ли handles БЫТЬ capabilities (Zircon model) или остаться отдельными?

**5.8 Драйверы**

Существующий: VirtIO-BLK в Ring 0, polling I/O.
- Нужен ли user-space driver model для AI-безопасности?
- Как изолировать драйвер при capability-based model?

**5.9 VFS** (`kernel_integrate.zig`, VFS↔FAT32)

Существующий: VfsFile с path, read/write/offset, FAT32 underneath.
- Как добавить per-file capabilities (CAP_FILE_READ для конкретного файла)?
- Взаимодействие с Object Manager access_mask

**5.10 Служба политик** (НОВЫЙ компонент)

- Где разместить: kernel module или user-space service?
- Как взаимодействует с существующим `acl_global_version`?
- Кто является источником политик: пользователь? AI? Оба?

### 6. AI-подсистема

Разработай безопасную архитектуру AI-капсулы КАК ПРОЦЕСС POLER-OS.

**6.1 Жизненный цикл**

- Создание: `processMgrCreateProcess()` с subsystem=??? (Нужен ли SubsystemId.AI?)
- Запуск: ELF loading через `elf_loader.loadElfIntoPML4_v2()`
- Остановка: `processMgrTerminate()` — что с capabilities?
- Обновление: пересоздание капсулы — как мигрировать capabilities?
- Откат: восстановление предыдущей версии — как откатить capabilities?

**6.2 Sandbox**

Существующий: per-process PML4, CR3 switching, Ring 3 IRETQ.
- Достаточно ли аппаратной изоляции (Ring 3 + separate PML4)?
- Нужно ли IOMMU для DMA isolation (VirtIO-BLK DMA)?
- Как ограничить syscall surface для AI (subset из POSIX+NT+POLER)?

**6.3 Управление ресурсами**

- CPU: CFS-like scheduling для AI? Quota-based?
- RAM: memory capability с лимитом (сейчас brk=16MB, mmap=8GB — слишком много для AI?)
- FS: chroot-like namespace restriction через VFS?
- Сеть: нет стека — нужно ли для AI?

**6.4 IPC для AI**

- AI ↔ Kernel: через SYSCALL (как сейчас) или через IPC channel?
- AI ↔ Other processes: нужен ли?
- Формат сообщений: JSON? protobuf? custom binary?

**6.5 Журналирование**

Существующий: circular buffer 256 AclAuditEntry.
- Достаточно ли для AI audit?
- Нужен ли persistent log (на диск через FAT32)?
- Формат: текущий struct или расширить?

### 7. API

Предложи API для AI, совместимый с существующими subsystem dispatch.

**7.1 Общие принципы**

- AI использует какой SubsystemId? `.Native`? Новый `.AI`?
- Syscall numbers: 0x2000+ (POLER native) или новый диапазон 0x3000+?
- Как передавать capability token: неявно (в ProcessControlBlock) или явно (в аргументах syscall)?

**7.2 Конкретные API**

Для каждого вызова опиши: назначение, необходимые capabilities, ожидаемые ошибки, ограничения безопасности.

| Категория | Существующий API | Что добавить для AI |
|-----------|------------------|---------------------|
| Файловая система | VFS open/read/write/close | |
| Процессы | processMgr Create/Fork/Exec/Terminate | |
| Память | mmap/brk/NtAllocateVirtualMemory | |
| IPC | НЕТ (stub) | |
| Сеть | НЕТ (stub) | |
| Устройства | VirtIO-BLK polling | |
| События | Object Manager Event/Mutant/Semaphore | |
| Журналирование | AclAuditEntry circular buffer | |
| Capabilities | SET_CAPS/GET_CAPS (POLER native) | |

### 8. Zig — структура проекта

Предложи изменения к существующей структуре:

```
src64/
├── main64.zig           # Kernel entry + shell
├── hal.zig              # HAL
├── scheduler.zig        # Scheduler
├── poler_core.zig       # Crypto core
├── pmm64.zig            # Physical MM
├── vmm64.zig            # Virtual MM
├── heap64.zig           # Kernel heap
├── spinlock.zig         # Spinlock
├── elf_loader.zig       # ELF loader
├── dynlinker.zig        # Dynamic linker
├── kernel_integrate.zig # Integration layer
├── syscall_integration.zig # Syscall dispatcher
├── acpi.zig             # ACPI
├── pci.zig              # PCI
├── virtio_blk.zig       # VirtIO-BLK
├── fat32.zig            # FAT32
├── framebuffer.zig      # VBE
├── rsa_oaep.zig         # RSA
├── cpio.zig             # CPIO
├── multiboot1.zig       # Multiboot
├── multiboot2.zig       # Multiboot
├── smp.zig              # SMP
├── subsystem/
│   ├── subsystem.zig    # Dispatcher
│   ├── nt/nt_api.zig    # NT API
│   ├── posix/posix_api.zig # POSIX API
│   └── common/object_manager.zig # Object Manager
├── boot64.S             # Boot assembly
├── isr64.S              # ISR assembly
├── boot_smp.S           # SMP AP trampoline
└── linker64.ld          # Linker script
```

- Какие НОВЫЕ файлы нужны?
- Какие СУЩЕСТВУЮЩИЕ файлы нужно модифицировать?
- Какие зависимости между новыми и существующими модулями?
- Как минимизировать circular dependencies (сейчас решаются через function pointer callbacks)?

### 9. Roadmap реализации

Разбей внедрение на этапы, совместимые с существующим процессом разработки. Каждый этап должен быть компилируемым и тестируемым через `zig build run64-blk-headless`.

| Этап | Цель | Зависимости | Ожидаемый результат | Критерий готовности |
|------|------|-------------|---------------------|---------------------|
| 9.1 | | | | |
| 9.2 | | | | |
| ... | | | | |

Учитывай:
- Текущее состояние: ядро компилируется с 0 ошибок, ~1.7MB binary
- Shell работает в Ring 0 через serial
- Тестирование через QEMU: `zig build run64-blk-headless`
- Тестирование с диском: `zig build run64-blk` (VirtIO-BLK + FAT32)

### 10. Риски

| Категория | Риск | Вероятность | Влияние | Митигация |
|-----------|------|-------------|---------|-----------|
| Архитектурные | | | | |
| Производительность | | | | |
| Сопровождение | | | | |
| Безопасность | | | | |
| Тупиковые решения | | | | |

Особое внимание:
- Что лучше изменить сейчас, пока проект на ранней стадии (но уже 20K строк)?
- Какие решения看似 простые, но приведут к тупику через 6 месяцев?
- Какие существующие решения (SipHash heap, POLER token, 64-bit ACL) стоит сохранить?

### 11. Практические рекомендации

Дай итоговый список:

**Обязательно реализовать (P0):**
- ...

**Желательно реализовать (P1):**
- ...

**Можно отложить (P2):**
- ...

**Можно исключить (P3):**
- ...

**Заменить более надёжными:**
- ...

### 12. Итоговая оценка

- Оцени предложенную архитектуру по 10-балльной шкале
- Оцени существующую архитектуру POLER-OS по 10-балльной шкале
- Оцени реалистичность эволюционного перехода (от существующей к предложенной) по 10-балльной шкале
- Сильные стороны существующей архитектуры
- Слабые стороны существующей архитектуры
- Сильные стороны предложенной модели
- Слабые стороны предложенной модели
- Наиболее реалистичная эволюция проекта до полноценной безопасной AI-ориентированной ОС

---

## Формат ответа

Ответ должен быть технически глубоким, содержать:
- Схемы (ASCII при необходимости)
- Таблицы сравнения
- Аргументацию каждого решения
- Примеры структур данных и псевдокод ТОЛЬКО там, где это действительно помогает понять архитектуру
- Конкретные ссылки на существующие файлы и функции POLER-OS

Чётко отделяй:
- Общепринятые практики (industry standard)
- Авторские рекомендации (должны быть явно отмечены)
- Спорные места (должны быть явно отмечены с альтернативами)

# POLER-OS: Архитектура Semantic Runtime

> **Версия документа:** 2.0  
> **Дата:** 2026-07-06  
> **Статус:** Концептуальная архитектура (эволюция от v0.4.0 к v1.0)

---

## 0. Эволюция подхода

| Этап | Идея | Почему отказались |
|------|------|-------------------|
| **v1: Linux Guest VM** | Запуск Linux как VM-гостя через VT-x/AMD-V, драйверы через VirtIO | Накладные расходы виртуализации, зависимость от Linux-ядра, сложность VMX |
| **v2: ABI совместимость** | Прямой запуск ELF-бинарников, реализация POSIX syscalls | Привязка к Linux-интерфейсам, бесконечная гонка за совместимостью |
| **v3: Тонкий Wine-слой** | Трансляция Win32 → нативные вызовы (VirtualAlloc→mmap, CreateWindow→Wayland) | Слишком много Win32 реализовывать, привязка к чужому API |
| **v4: POLER Semantic Runtime** ← **ТЕКУЩИЙ** | Программа выражает намерение (Intent), а не вызывает конкретный API. Адаптеры для ELF/PE — лишь входные слои | — |

**Ключевой сдвиг:** Не «какую функцию вызвала программа», а «что она хочет сделать».

---

## 1. Высокоуровневая архитектура

```
┌─────────────────────────────────────────────────────────────────┐
│                     ПРИКЛАДНОЙ СЛОЙ                              │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  ┌──────────┐  │
│  │  ELF     │  │  PE/EXE  │  │  POLER Native │  │  WASM    │  │
│  │ (Linux)  │  │ (Windows)│  │  (.pnx)       │  │          │  │
│  └────┬─────┘  └────┬─────┘  └──────┬────────┘  └────┬─────┘  │
│       │              │               │                │         │
└───────┼──────────────┼───────────────┼────────────────┼─────────┘
        │              │               │                │
        ▼              ▼               ▼                ▼
┌─────────────────────────────────────────────────────────────────┐
│              АДАПТЕРНЫЙ СЛОЙ (Intent Extractors)                │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────┐ │
│  │  ELF Adapter │  │  PE Adapter  │  │  POLER Native Loader  │ │
│  │              │  │              │  │                       │ │
│  │ ld.so →      │  │ PE imports → │  │ Direct intent         │ │
│  │ glibc/musl → │  │ Intent map   │  │ binding               │ │
│  │ Intent map   │  │              │  │                       │ │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬────────────┘ │
│         │                 │                      │              │
│         └─────────┬───────┘──────────────────────┘              │
│                   │                                             │
└───────────────────┼─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│              POLER INTENT LAYER (Ядро абстракции)               │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  INTENT DISPATCHER                       │   │
│  │                                                         │   │
│  │  Intent{OpenFile}   → File Service                      │   │
│  │  Intent{AllocMem}   → Memory Service                    │   │
│  │  Intent{CreateSurf} → GPU Service                       │   │
│  │  Intent{PlayAudio}  → Audio Service                     │   │
│  │  Intent{SendPacket} → Network Service                   │   │
│  │  Intent{WaitEvent}  → Event Service                     │   │
│  │  Intent{SpawnProc}  → Process Service                   │   │
│  │  Intent{Sync}       → Sync Service                      │   │
│  │  ...                                                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌────────────────────┐  ┌────────────────────────────────┐   │
│  │  POLER Firewall    │  │  Rust Safety Core             │   │
│  │  ℘→O→L→ε→R→Ψ      │  │  (capability gate)            │   │
│  │  Когнитивный цикл  │  │  borrow checker guarantees    │   │
│  │  SipHash + Resonance│  │  все Intent проходят проверку│   │
│  └────────────────────┘  └────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│              СЕРВИСНЫЙ СЛОЙ (Kernel Services)                    │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│  │  File    │ │  Memory  │ │  GPU     │ │  Audio   │          │
│  │  Service │ │  Service │ │  Service │ │  Service │          │
│  │          │ │          │ │          │ │          │          │
│  │ ext4/   │ │ pmm/vmm  │ │ Vulkan/  │ │ PipeWire │          │
│  │ FAT32/  │ │ slub/    │ │ DRM/KMS  │ │ ALSA-    │          │
│  │ tmpfs   │ │ mmap     │ │ Wayland  │ │ compat   │          │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘          │
│       │             │            │             │                │
│  ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐          │
│  │  Network │ │  Process │ │  Event   │ │  Sync    │          │
│  │  Service │ │  Service │ │  Service │ │  Service │          │
│  │          │ │          │ │          │ │          │          │
│  │ TCP/IP   │ │ sched/   │ │ epoll/   │ │ futex/   │          │
│  │ socket   │ │ signal/  │ │ inotify  │ │ mutex/   │          │
│  │ netfilter│ │ cgroup   │ │ poll     │ │ condvar  │          │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘          │
│       │             │            │             │                │
└───────┼─────────────┼────────────┼─────────────┼────────────────┘
        │             │            │             │
        ▼             ▼            ▼             ▼
┌─────────────────────────────────────────────────────────────────┐
│              HAL (Hardware Abstraction Layer)                    │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│  │  ACPI    │ │  PCI     │ │  IRQ     │ │  Timer   │          │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐          │
│  │  Serial  │ │  Display │ │  Input   │ │  Storage │          │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ЖЕЛЕЗО (Hardware)                             │
│          x86_64 / RISC-V / ARM64                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. POLER Intent Layer — Детальная архитектура

### 2.1 Концепция Intent

Intent — это **семантическое описание того, что программа хочет сделать**, а не вызов конкретного API.

```
┌─────────────────────────────────────────────────┐
│                 INTENT STRUCT                    │
│                                                 │
│  struct Intent {                                │
│      category:  IntentCategory,  // FS|MEM|GPU  │
│      action:    IntentAction,    // Open|Alloc   │
│      params:    IntentParams,    // tagged union │
│      caller:    ProcessID,                       │
│      timestamp: u64,                             │
│      nonce:     u64,                             │
│  }                                              │
│                                                 │
│  enum IntentCategory {                          │
│      FileSystem,                                │
│      Memory,                                    │
│      GPU,                                       │
│      Audio,                                     │
│      Network,                                   │
│      Process,                                   │
│      Event,                                     │
│      Sync,                                      │
│      Input,                                     │
│      Clipboard,                                 │
│      Configuration,                             │
│  }                                              │
│                                                 │
│  enum IntentAction {                            │
│      Open,    Read,    Write,   Close,          │
│      Alloc,   Free,    Map,     Unmap,          │
│      Create,  Destroy, Connect, Listen,         │
│      Wait,    Signal,  Lock,    Unlock,         │
│      Spawn,   Kill,    Exec,    Query,          │
│  }                                              │
└─────────────────────────────────────────────────┘
```

### 2.2 Поток обработки Intent

```
Программа вызывает syscall / API
            │
            ▼
    ┌───────────────┐
    │    Адаптер     │   ELF: libc syscall → Intent
    │   (Extractor)  │   PE:  Win32 API  → Intent
    │                │   Native: прямое Intent API
    └───────┬───────┘
            │
            ▼
    ┌───────────────┐
    │  Intent       │   Валидация категории, action, params
    │  Dispatcher   │   Маршрутизация к нужному сервису
    └───────┬───────┘
            │
            ├──────────────────────┐
            │                      │
            ▼                      ▼
    ┌───────────────┐    ┌───────────────┐
    │  POLER        │    │  Rust Safety  │
    │  Firewall     │    │  Core         │
    │               │    │               │
    │  Когнитивный  │    │  Capability   │
    │  цикл:        │    │  check:       │
    │  ℘→O→L→ε→R→Ψ │    │  - memory     │
    │               │    │  - permission │
    │  Вердикт:     │    │  - ownership  │
    │  ALLOW /      │    │  - bounds     │
    │  DENY /       │    │               │
    │  SUSPICIOUS   │    │  Pass/Deny    │
    └───────┬───────┘    └───────┬───────┘
            │                    │
            │  (оба должны       │
            │   разрешить)       │
            ▼                    ▼
    ┌─────────────────────────────────┐
    │          Intent Router          │
    │  (только если оба разрешили)    │
    │                                 │
    │  FileSystem Intent → File Svc   │
    │  Memory Intent     → Mem Svc    │
    │  GPU Intent        → GPU Svc    │
    │  Audio Intent      → Audio Svc  │
    │  Network Intent    → Net Svc    │
    │  ...                             │
    └──────────────┬──────────────────┘
                   │
                   ▼
            ┌─────────────┐
            │   Сервис     │
            │   выполняет  │
            │   Intent     │
            └─────────────┘
```

### 2.3 Примеры трансляции API → Intent

| Вызов программы | Адаптер извлекает Intent | Сервис обрабатывает |
|---|---|---|
| `open("/foo", O_RDONLY)` | `Intent{FileSystem, Open, path="/foo", flags=READ}` | File Service: VFS lookup → ext4 read |
| `mmap(NULL, 4096, PROT_READ\|WRITE, MAP_PRIVATE, fd, 0)` | `Intent{Memory, Map, size=4096, perms=RW, fd=N}` | Memory Service: VMM → page alloc |
| `CreateWindowEx(...)` | `Intent{GPU, Create, type=Surface, title=..., size=...}` | GPU Service: Wayland surface + Vulkan swapchain |
| `D3D11CreateDevice(...)` | `Intent{GPU, Create, type=VulkanDevice, features=...}` | GPU Service: Vulkan ICD init |
| `send(sock, buf, len, 0)` | `Intent{Network, Write, socket=N, data=...}` | Network Service: TCP/IP stack send |
| `pthread_create(...)` | `Intent{Process, Spawn, type=Thread, entry=..., stack=...}` | Process Service: clone thread |
| `epoll_wait(...)` | `Intent{Event, Wait, fds=[...], timeout=...}` | Event Service: epoll subsystem |
| `VirtualAlloc(NULL, size, MEM_COMMIT, PAGE_READWRITE)` | `Intent{Memory, Alloc, size=N, perms=RW}` | Memory Service: commit pages |
| `PlaySound(...)` | `Intent{Audio, Play, data=..., format=...}` | Audio Service: PipeWire/ALSA |

---

## 3. Объектная модель POLER

### 3.1 Почему не прямой маппинг HANDLE → указатель

```
❌ ПЛОХО (как в идее.md изначально):

  HWND 0x1001 → struct wl_surface*    // Привязка к Wayland ABI
  HANDLE 0x42 → int fd                // Привязка к POSIX fd


✅ ПРАВИЛЬНО:

  HWND 0x1001
        │
        ▼
  ┌─────────────────────────────────────────┐
  │  POLER Object (универсальный дескриптор) │
  │                                         │
  │  id:          ObjectId (u64)            │
  │  type:        ObjectClass               │
  │  owner:       ProcessId                 │
  │  capabilities: CapSet                   │
  │  state:       enum { Active, ... }      │
  │                                         │
  │  ── Backend-привязки (внутренние) ──    │
  │                                         │
  │  wayland_surface: ?*wl_surface          │
  │  vulkan_swapchain: ?VkSwapchainKHR      │
  │  metadata:       WindowMeta             │
  │  fd:             ?i32                   │
  │  gpu_resource:   ?GpuHandle            │
  └─────────────────────────────────────────┘
```

### 3.2 Классы объектов

| ObjectClass | Примеры объектов | Адаптерные имена |
|---|---|---|
| `File` | Файлы, директории, pipes | fd (POSIX), HANDLE (Win32) |
| `Memory` | mmap regions, heaps | void* (POSIX), LPVOID (Win32) |
| `Surface` | Окна, offscreen buffers | wl_surface, HWND |
| `Socket` | TCP/UDP соединения | fd (POSIX), SOCKET (Win32) |
| `Process` | Процессы, потоки | pid_t, HANDLE |
| `Sync` | Мьютексы, семафоры, futex | pthread_mutex*, HANDLE |
| `GPU` | Vulkan device, swapchain | VkDevice, ID3D11Device |
| `Audio` | Потоки, устройства | snd_pcm_t*, HWAVEOUT |
| `Timer` | Таймеры, clock | timerfd, CreateTimerQueueTimer |

### 3.3 Capability-based доступ

```zig
const CapSet = packed struct {
    read:       bool,  // Чтение
    write:      bool,  // Запись
    execute:    bool,  // Выполнение
    share:      bool,  // Право передать другому процессу
    destroy:    bool,  // Право закрыть/уничтожить
    admin:      bool,  // Управление правами
    _reserved:  u2,
};
```

Каждый объект хранит Capability Set для владельца. При передаче объекта между процессами можно только **сузить** права (не расширить).

---

## 4. POLER Firewall — Когнитивный цикл безопасности

### 4.1 Цикл ℘→O→L→ε→R→Ψ

```
                ┌─────────────────────────────────┐
                │                                 │
    ℘(Perception)──→ O(Image)──→ L(Logic)──→ ε(Energy)
        │              │             │             │
        │  Наблюдение  │  Формиро-   │  Анализ     │  Оценка
        │  syscall     │  вание      │  правил     │  ресурсов
        │  параметров  │  контекста  │  и шаблонов │  и рисков
        │              │             │             │
        ▼              ▼             ▼             ▼
    R(Resonance)─────────────────── Ψ(Intention)
        │                                 │
        │  Семантическая                  │  Финальное
        │  аномалия?                      │  решение
        │  (SipHash PRF +                 │  ALLOW /
        │   Deformed Tensor)              │  DENY /
        │                                 │  SUSPICIOUS
        └─────────────────────────────────┘
```

### 4.2 Математическая модель

**Деформированное тензорное произведение:**
```
a ⊗_ε b = ε · (rotl(a, 5) ⊕ rotl(b, 7) ⊕ Φ(a ⊕ b))

где Φ(x) = rotl(x³, 13) ⊕ rotl(x, 7) ⊕ 1
```

**Когнитивный шаг (полная версия из poler_core.zig):**
```
polerStep(x, key, ε):
    attr = rotl(key, 17) ⊕ phi(key)    // Динамический аттрактор
    x'   = diffusionOp(x, key, ε)       // Диффузионный оператор
    x''  = feistel(x', key)             // Сеть Фейстеля
    res  = x'' ⊕ attr                   // XOR с аттрактором
    resonance = measure(res, expected)   // Измерение резонанса
    verdict   = classify(resonance)      // ALLOW / DENY / SUSPICIOUS
```

**SipHash PRF для детерминистической валидации syscall:**
```
firewall_key = [8]u32 { секретный ключ ядра }

Для каждого Intent:
    hash = siphash_2_4(intent_bytes, firewall_key)
    expected = siphash_2_4(history_bytes, firewall_key)
    resonance = Φ(hash ⊕ expected)
    
    if resonance < threshold_low  → ALLOW
    if resonance > threshold_high → DENY
    else                          → SUSPICIOUS (log + sandbox)
```

### 4.3 Ring buffer для детекции аномалий

```zig
const ANOMALY_RING_SIZE = 8;

var anomaly_ring: [ANOMALY_RING_SIZE]f32 = undefined;
var anomaly_idx: usize = 0;

fn record_resonance(val: f32) void {
    anomaly_ring[anomaly_idx % ANOMALY_RING_SIZE] = val;
    anomaly_idx += 1;
}

fn detect_anomaly() bool {
    // Сравниваем последние 8 резонансов
    // Если резкий скачок — семантическая аномалия
    const recent = anomaly_ring[(anomaly_idx - 1) % ANOMALY_RING_SIZE];
    var sum: f32 = 0;
    for (anomaly_ring) |v| sum += v;
    const avg = sum / ANOMALY_RING_SIZE;
    return @abs(recent - avg) > ANOMALY_THRESHOLD;
}
```

---

## 5. Rust Safety Core — Топологический барьер безопасности

### 5.1 Роль в архитектуре

```
User Process (Ring 3)
        │
        │ syscall / Intent
        ▼
┌─────────────────────────────────────────┐
│         RUST SAFETY CORE (Ring 0)       │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  SyscallGate                    │   │
│  │                                 │   │
│  │  1. Проверка caller PID        │   │
│  │  2. Валидация буферов          │   │
│  │     - verify_access(addr,size)  │   │
│  │     - ownership check          │   │
│  │     - bounds check             │   │
│  │  3. Capability verification    │   │
│  │  4. Передача в Zig Kernel      │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  MemoryGuard                    │   │
│  │                                 │   │
│  │  - Region tracking (256 slots) │   │
│  │  - Overlap detection           │   │
│  │  - Permission enforcement      │   │
│  │  - Owner isolation             │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  ViolationReporter             │   │
│  │                                 │   │
│  │  NullPointer    → DENY+kill    │   │
│  │  BufferOverflow → DENY+kill    │   │
│  │  UseAfterFree   → DENY+kill    │   │
│  │  DoubleFree     → DENY+kill    │   │
│  │  InvalidSyscall → DENY+log     │   │
│  │  PrivilegeEscal → DENY+kill    │   │
│  │  StackOverflow  → DENY+kill    │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  IPC: Zig ↔ Rust               │   │
│  │                                 │   │
│  │  Spinlock (RAII guard)         │   │
│  │  MpscQueue<T, N> (lock-free)   │   │
│  │    - push() from Zig ISR       │   │
│  │    - pop()  from Rust handler  │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
        │
        │ Verified Intent
        ▼
   Zig Kernel Services
```

### 5.2 Syscall routing через Rust

```rust
// Текущие syscalls (реализовано в syscalls.rs)
SYS_READ    = 0   // → Intent{FileSystem, Read, ...}
SYS_WRITE   = 1   // → Intent{FileSystem, Write, ...}
SYS_OPEN    = 2   // → Intent{FileSystem, Open, ...}
SYS_CLOSE   = 3   // → Intent{FileSystem, Close, ...}
SYS_MMAP    = 4   // → Intent{Memory, Map, ...}
SYS_MUNMAP  = 5   // → Intent{Memory, Unmap, ...}
SYS_EXIT    = 6   // → Intent{Process, Kill, ...}
SYS_FORK    = 7   // → Intent{Process, Spawn, ...}
SYS_EXEC    = 8   // → Intent{Process, Exec, ...}

// Планируемые Intent-syscalls (POLER Semantic API)
SYS_INTENT_DISPATCH = 256  // Универсальный Intent-вызов
SYS_INTENT_QUERY    = 257  // Запрос возможностей
SYS_INTENT_CAPS     = 258  // Управление capabilities
```

---

## 6. Адаптерный слой — Детализация

### 6.1 ELF Adapter (Linux совместимость)

```
Linux ELF Binary
       │
       ▼
┌──────────────────────────────────────────────┐
│              ELF LOADER                       │
│                                              │
│  1. Parse ELF headers                        │
│  2. Map segments (PT_LOAD → Intent{Map})     │
│  3. Resolve dynamic symbols                  │
│  4. Load ld.so (dynamic linker)              │
│  5. Load .so dependencies                    │
│  6. Jump to entry point                      │
└──────────────┬───────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────┐
│           LIBC ADAPTER (glibc/musl compat)    │
│                                              │
│  Трёхслойная стратегия:                      │
│                                              │
│  Слой 1: Прямой маппинг (80% syscalls)      │
│  ────────────────────────────────────        │
│  read()       → Intent{FS, Read}             │
│  write()      → Intent{FS, Write}            │
│  open()       → Intent{FS, Open}             │
│  close()      → Intent{FS, Close}            │
│  mmap()       → Intent{Mem, Map}             │
│  munmap()     → Intent{Mem, Unmap}           │
│  nanosleep()  → Intent{Sync, Wait}           │
│  rt_sigaction → Intent{Event, Register}      │
│  futex()      → Intent{Sync, Wait/Signal}    │
│  epoll_*()    → Intent{Event, ...}           │
│                                              │
│  Слой 2: Семантическая трансляция (15%)      │
│  ────────────────────────────────────        │
│  clone()      → Intent{Process, Spawn,       │
│                         type=Thread/Process}  │
│  ioctl()      → Intent{*, ...} по коду       │
│  prctl()      → Intent{Process, Configure}   │
│  signalfd()   → Intent{Event, Register,      │
│                          type=Signal}         │
│  eventfd()    → Intent{Event, Create}        │
│  timerfd_*()  → Intent{Event, Timer}         │
│                                              │
│  Слой 3: Эмуляция поведения (5%)            │
│  ────────────────────────────────────        │
│  Некоторые ioctl → мок-реализации            │
│  /proc, /sys   → виртуальная FS              │
│  ptrace()      → Intent{Process, Debug}      │
│  personality() → no-op (заглушка)            │
└──────────────────────────────────────────────┘
```

### 6.2 PE Adapter (Windows совместимость — без Win32 реализации)

```
Windows PE/EXE
       │
       ▼
┌──────────────────────────────────────────────┐
│              PE LOADER                        │
│                                              │
│  1. Parse PE headers                         │
│  2. Map sections → Intent{Map}               │
│  3. Resolve imports → PE Adapter DLL         │
│  4. TLS callbacks                            │
│  5. Jump to entry point                      │
└──────────────┬───────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────┐
│          PE ADAPTER (Intent Extractor)        │
│                                              │
│  НЕ реализуем Win32.                         │
│  Реализуем трансляцию намерений:             │
│                                              │
│  ── Прямой маппинг ──                        │
│  VirtualAlloc   → Intent{Memory, Alloc}      │
│  VirtualFree    → Intent{Memory, Free}       │
│  CreateFile     → Intent{FS, Open}           │
│  ReadFile       → Intent{FS, Read}           │
│  WriteFile      → Intent{FS, Write}          │
│  CloseHandle    → Intent{*, Close}           │
│  Sleep          → Intent{Sync, Wait}         │
│  GetTickCount   → Intent{Event, Query}       │
│                                              │
│  ── Семантическая трансляция ──              │
│  CreateWindowEx → Intent{GPU, Create,        │
│                           type=Surface,      │
│                           title=...,         │
│                           rect=...}          │
│  D3D11Create    → Intent{GPU, Create,        │
│  Device            type=VulkanDevice}        │
│  SendMessage    → Intent{Event, Signal,      │
│                           target=...,        │
│                           msg=...}           │
│  CreateThread   → Intent{Process, Spawn,     │
│                           type=Thread}       │
│  WSASocket      → Intent{Network, Create}    │
│                                              │
│  ── Комплексные (нужна внутренняя логика) ── │
│  HWND lifecycle → Object Table + Wayland     │
│  GDI drawing   → Intent{GPU, Draw, ...}      │
│  Registry      → Intent{FS, Open,            │
│                          path="/registry/"}  │
└──────────────────────────────────────────────┘
```

### 6.3 POLER Native (.pnx формат)

```
POLER Native Application
       │
       ▼
┌──────────────────────────────────────────────┐
│          PNX LOADER                          │
│                                              │
│  Формат: POLER eXecutable                    │
│  - Прямой Intent API (без адаптера)          │
│  - Статически слинкован с libpoler            │
│  - Capability declarations в заголовке       │
│  - ZIG / Rust / C компиляция                 │
└──────────────────────────────────────────────┘

libpoler API (нативная):

  // Прямой Intent — без syscall overhead
  poler_intent_dispatch(intent: *Intent) IntentResult
  poler_intent_query(category: IntentCategory) CapSet
  poler_object_create(class: ObjectClass, caps: CapSet) ObjectId
  poler_object_invoke(id: ObjectId, method: Method, ...) Result
  poler_object_share(id: ObjectId, target: ProcessId, caps: CapSet) bool
  poler_object_destroy(id: ObjectId) void

  // Высокоуровневые удобства (поверх Intent)
  poler_file_open(path, flags) ObjectId
  poler_mem_alloc(size, perms) ObjectId
  poler_gpu_surface(title, w, h) ObjectId
  poler_net_connect(addr, port) ObjectId
  ...
```

---

## 7. Сервисный слой — Детализация реализации

### 7.1 GPU Service (самый важный для UX)

```
┌─────────────────────────────────────────────────────┐
│                   GPU SERVICE                        │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  Wayland Compositor (POLER compositor)    │      │
│  │                                           │      │
│  │  - wl_compositor                          │      │
│  │  - wl_shell / xdg_shell                   │      │
│  │  - wl_seat (input)                        │      │
│  │  - wl_output (display)                    │      │
│  │  - POLER-specific protocols               │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  Vulkan Runtime                           │      │
│  │                                           │      │
│  │  - Vulkan Loader (libvulkan)              │      │
│  │  - ICD (Installable Client Driver)        │      │
│  │  - D3D11→Vulkan translation layer         │      │
│  │  - DXGI→Vulkan swapchain mapping          │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  DRM/KMS (Direct Rendering Manager)       │      │
│  │                                           │      │
│  │  - Mode setting                           │      │
│  │  - GBM (Generic Buffer Manager)           │      │
│  │  - Prime (buffer sharing)                 │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  Стек драйверов:                                    │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│  │ AMDGPU  │ │ i915    │ │ Nouveau │ │ VirtGPU │  │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
└─────────────────────────────────────────────────────┘
```

### 7.2 File System Service

```
┌─────────────────────────────────────────────────────┐
│                 FILE SYSTEM SERVICE                   │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  VFS (Virtual File System)                │      │
│  │                                           │      │
│  │  - inode / dentry cache                   │      │
│  │  - mount namespace per process            │      │
│  │  - POLER Object → fd mapping             │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │  ext4    │ │  FAT32   │ │  tmpfs   │            │
│  └──────────┘ └──────────┘ └──────────┘            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │  procfs  │ │  sysfs   │ │  devfs   │            │
│  │(virtual) │ │(virtual) │ │(virtual) │            │
│  └──────────┘ └──────────┘ └──────────┘            │
│                                                     │
│  Block layer:                                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │  NVMe    │ │  AHCI    │ │  VirtIO  │            │
│  │  driver  │ │  (SATA)  │ │  Block   │            │
│  └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────┘
```

### 7.3 Network Service

```
┌─────────────────────────────────────────────────────┐
│                 NETWORK SERVICE                       │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  Socket Layer                             │      │
│  │  - AF_INET, AF_INET6, AF_UNIX            │      │
│  │  - SOCK_STREAM, SOCK_DGRAM, SOCK_RAW     │      │
│  │  - POLER Object → socket mapping         │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  ┌───────────────────────────────────────────┐      │
│  │  TCP/IP Stack                             │      │
│  │  - lwIP или собственный стек             │      │
│  │  - IPv4/IPv6, TCP, UDP, ICMP             │      │
│  │  - netfilter (POLER Firewall интеграция)  │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  Драйверы:                                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │ VirtIO   │ │ e1000    │ │ r8169    │            │
│  │ Net      │ │          │ │          │            │
│  └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────┘
```

---

## 8. Сравнение подходов к совместимости

### 8.1 Чего НЕ делает POLER

| Чего мы НЕ делаем | Почему |
|---|---|
| Не реализуем Win32 API целиком | Бесконечный объём работы, всегда отстаём |
| Не запускаем Linux-ядро как VM guest | Накладные расходы, зависимость |
| Не реализуем полный POSIX | Гонка за совместимостью с glibc |
| Не используем Wine как слой | Привязка к чужой архитектуре |
| Не маппим HANDLE → указатель напрямую | ABI-зависимость, небезопасно |

### 8.2 Что ДЕЛАЕТ POLER

| Принцип | Реализация |
|---|---|
| **Программа = клиент ядра** | Intent — универсальный язык общения |
| **Совместимость через поведение** | Трансляция намерений, не API |
| **Собственная объектная модель** | POLER Object Table, capability-based |
| **Безопасность на уровне типов** | Rust borrow checker + POLER Firewall |
| **Нативные протоколы** | Wayland, Vulkan, DRM, PipeWire напрямую |
| **Комфорт для всех** | Windows-пользователи: работает ПО. Linux-пользователи: привычные инструменты |

### 8.3 Сравнение с другими подходами

```
Wine:       App → Win32 → POSIX → Linux Kernel
POLER:      App → Intent → POLER Kernel → Hardware

ReactOS:    App → Win32 → ReactOS Kernel (полная ОС)
POLER:      App → Intent → POLER Kernel (своя модель)

WSL1:      App → ELF → LXSS → NT Kernel (трансляция syscalls)
POLER:      App → Intent → POLER Kernel (трансляция намерений)

WSL2:      App → ELF → Linux Kernel (VM)
POLER:     App → Intent → POLER Kernel (без VM)
```

---

## 9. Текущий статус реализации

### 9.1 Что уже есть в коде

| Компонент | Файл | Статус | Описание |
|---|---|---|---|
| Zig Kernel | `src/main32.zig` | ✅ Работает | VGA, serial, PS/2, shell, PCI scan |
| POLER Core v5 | `src/poler_core.zig` | ✅ 9/9 тестов | Feistel + S-Box + LHCA + Firewall |
| VirtIO Layer | `drivers/virtio.zig` | ⚠️ Структуры | Детектит 0x1AF4, не инициализирует |
| Boot 32-bit | `src/boot32.S` | ⚠️ Базовый | Multiboot entry, нет GDT/IDT/A20 |
| Linker | `src/linker32.ld` | ⚠️ Нужно fix | Нет ALIGN(8)+KEEP |
| Rust Safety Core | `rust-core/src/` | ⚠️ Скелет | syscalls, memory, safety, sync — без интеграции с Zig |
| ISO build | `build.zig` | ✅ Работает | GRUB+SYSLINUX ISO |

### 9.2 Известные баги (нужно исправить)

| Баг | Файл | Описание |
|---|---|---|
| `fb_font` OOB | `main32.zig` | ch≥128 выходит за массив |
| `shell_history` underflow | `main32.zig` | usize underflow при pos=0 |
| POLER Core key | `poler_core.zig` | key[4..7] не используется (128-bit, не 256) |
| Version banner | `main32.zig` | Показывает v0.3.0 вместо v0.4.0 |
| `fb_puts_hex64` | `main32.zig` | Пропускает последний nibble |
| Serial baud | `main32.zig` | Неправильный divisor |
| Нет STI | `boot32.S` | Прерывания не включены |
| Нет GDT/IDT | `boot32.S` | Не настроены таблицы |

### 9.3 Roadmap к v1.0

```
v0.4.0  ─── ТЕКУЩАЯ ───
  │  32-bit kernel, POLER Core, shell, PCI scan
  │
v0.5.0  ─── BUGFIX + 64-bit ───
  │  - Исправить все баги из таблицы выше
  │  - GDT/IDT/ISRs в boot32.S
  │  - Переход на x86_64 (long mode)
  │  - PML4 page tables
  │  - Рабочие прерывания (STI + IDT)
  │
v0.6.0  ─── MEMORY + PROCESS ───
  │  - PMM (Physical Memory Manager)
  │  - VMM (Virtual Memory Manager)
  │  - mmap/munmap Intent handling
  │  - User mode (Ring 3)
  │  - Syscall interface (syscall/sysret)
  │  - Базовый Process Service
  │
v0.7.0  ─── INTENT LAYER ───
  │  - Intent struct + Dispatcher
  │  - POLER Object Table
  │  - Capability system
  │  - Rust Safety Core интеграция
  │  - POLER Firewall + Intent
  │
v0.8.0  ─── ELF ADAPTER ───
  │  - ELF loader
  │  - Libc adapter (слой 1: прямой маппинг)
  │  - Запуск простых Linux ELF бинарников
  │  - File System Service (ext4/VFS)
  │
v0.9.0  ─── GPU + WAYLAND ───
  │  - DRM/KMS driver
  │  - Wayland compositor
  │  - Vulkan runtime
  │  - GPU Intent handling
  │  - Графический терминал
  │
v0.10.0 ─── NETWORK + PE ADAPTER ───
  │  - TCP/IP stack
  │  - Network Service
  │  - PE Loader (базовый)
  │  - PE Adapter (прямой маппинг)
  │
v1.0.0  ─── POLER SEMANTIC RUNTIME ───
     - Полный Intent Layer
     - PNX native format
     - Libc adapter (все 3 слоя)
     - PE adapter (семантическая трансляция)
     - Audio Service (PipeWire)
     - Стабильный API
```

---

## 10. Ключевые инженерные вызовы

### 10.1 Наибольшая сложность — поведение, не интерфейс

Библиотеки не проверяют имя ядра. Они ожидают **определённое поведение** интерфейсов:

- `futex()` — порядок пробуждения, spurious wakeups
- `epoll()` — edge vs level triggering, semantics of EPOLLONESHOT
- `mmap()` — CoW semantics, MAP_NORESERVE, alignment
- `clone()` — TLS setup, PID allocation, signal disposition
- `ioctl()` — сотни device-specific sub-commands
- Сигналы — delivery order, SA_RESTART, real-time vs standard

### 10.2 Стратегия решения

1. **Начать с прямого маппинга** (80% syscalls — почти 1:1)
2. **Тестировать на реальных программах** (VS Code, Blender, bash)
3. **Итеративно добавлять семантическую трансляцию** для сложных случаев
4. **Использовать Linux-тесты** (LTP) для валидации поведения
5. **Где поведение слишком сложное** — виртуальный слой (/proc, /sys)

### 10.3 D3D → Vulkan — отдельный вызов

DXVK уже доказал, что D3D11/D3D12 → Vulkan работает. Но интеграция в POLER требует:

- Собственный DXGI implementation (swapchain, adapter enumeration)
- D3D11 → Vulkan command buffer translation
- Shader compilation (HLSL → SPIR-V)
- Integration с POLER Object Table (HWND → wl_surface через Object)

**Стратегия:** Использовать DXVK как компонент, адаптировать под POLER Intent API.

---

## 11. Дизайн-философия UX

### 11.1 Цель: Комфорт для всех

```
Пользователь с Windows:
  - Приложения работают (Photoshop, Steam, Office)
  - Привычные клавиши (Ctrl+C/V/Z)
  - Графический интерфейс похож на Windows
  - Файловый менеджер, панель задач, системный трей

Пользователь с Linux:
  - Привычные инструменты (bash, gcc, vim, git)
  - Терминал first-class citizen
  - Wayland compositor, tiling WM опции
  - /usr/bin, /etc, /home — привычная структура
  - pacman/apt-like пакетный менеджер

POLER Native разработчик:
  - Intent API — прямое обращение к возможностям ядра
  - libpoler — удобные обёртки
  - Capability-based безопасность по умолчанию
  - Zig/Rust/C компиляция
```

### 11.2 IDE стратегия

НЕ делать собственную IDE на первом этапе. Сделать так, чтобы работали:

- VS Code (ELF через adapter)
- Zed (ELF через adapter)
- Neovim (ELF через adapter)
- CLion (ELF через adapter)

Собственная IDE — только когда POLER Native станет достаточно зрелым.

---

## 12. Сводная диаграмма архитектуры

```
╔══════════════════════════════════════════════════════════════════╗
║                        POLER-OS v1.0                            ║
║                  Semantic Runtime Architecture                  ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                ║
║   ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌─────────┐        ║
║   │  ELF    │  │  PE     │  │  Native  │  │  WASM   │        ║
║   │  Apps   │  │  Apps   │  │  (.pnx)  │  │         │        ║
║   └────┬────┘  └────┬────┘  └────┬─────┘  └────┬────┘        ║
║        │            │            │              │              ║
║   ┌────┴────┐  ┌────┴────┐  ┌────┴─────┐  ┌────┴────┐        ║
║   │  ELF    │  │  PE     │  │  PNX     │  │  WASM   │        ║
║   │  Adapt  │  │  Adapt  │  │  Loader  │  │  Runtime│        ║
║   └────┬────┘  └────┬────┘  └────┬─────┘  └────┬────┘        ║
║        └──────┬─────┘            │              │              ║
║               │                  └──────┬───────┘              ║
║               ▼                         ▼                      ║
║   ╔═════════════════════════════════════════════╗              ║
║   ║         POLER INTENT LAYER                  ║              ║
║   ║                                            ║              ║
║   ║   Intent Dispatcher ──────────────────┐    ║              ║
║   ║                                     │    ║              ║
║   ║   ┌────────────┐  ┌──────────────┐ │    ║              ║
║   ║   │   POLER    │  │    Rust      │ │    ║              ║
║   ║   │  Firewall  │  │  Safety Core │ │    ║              ║
║   ║   │  ℘→O→L→ε→R→Ψ│  │  Cap Gate   │ │    ║              ║
║   ║   └────────────┘  └──────────────┘ │    ║              ║
║   ║                                    │    ║              ║
║   ║   Intent Router ◄──────────────────┘    ║              ║
║   ╚═══════════════╤══════════════════════════╝              ║
║                   │                                            ║
║   ┌───────────────┼────────────────────────────────────┐     ║
║   │          KERNEL SERVICES                         │     ║
║   │                                                  │     ║
║   │  ┌──────┐┌──────┐┌──────┐┌──────┐┌──────┐      │     ║
║   │  │ File ││ Mem  ││ GPU  ││Audio ││ Net  │      │     ║
║   │  │ Svc  ││ Svc  ││ Svc  ││ Svc  ││ Svc  │      │     ║
║   │  └──┬───┘└──┬───┘└──┬───┘└──┬───┘└──┬───┘      │     ║
║   │  ┌──┴───┐┌──┴───┐┌──┴───┐┌──┴───┐              │     ║
║   │  │ Proc ││ Event││ Sync ││Input │              │     ║
║   │  │ Svc  ││ Svc  ││ Svc  ││ Svc  │              │     ║
║   │  └──┬───┘└──┬───┘└──┬───┘└──┬───┘              │     ║
║   │     └───────┴────────┴───────┘                  │     ║
║   └─────────────────────┬──────────────────────────┘     ║
║                         │                                  ║
║   ┌─────────────────────┴──────────────────────────┐     ║
║   │               HAL (Zig Kernel)                 │     ║
║   │  ACPI │ PCI │ IRQ │ Timer │ Serial │ Display  │     ║
║   │  Input│ Storage│ SMP │ MMIO │ DMA │ Power     │     ║
║   └─────────────────────┬──────────────────────────┘     ║
║                         │                                  ║
║                    Hardware                               ║
╚════════════════════════════════════════════════════════════╝
```

---

## 13. Формальные спецификации для передачи GPT

При передаче этой схемы GPT для генерации кода, укажи следующие ключевые моменты:

1. **POLER-OS НЕ клон Linux и НЕ Wine.** Это ОС с собственной семантической моделью (Intent Layer).

2. **Язык реализации:** Zig (ядро + HAL), Rust (Safety Core), C (драйверы GPU/Vulkan).

3. **Архитектура:** x86_64, Multiboot2, long mode, PML4, Ring 0/3.

4. **Ключевой принцип:** Каждый syscall/API-вызов от программы трансформируется в Intent, проходит через Rust Safety Core + POLER Firewall, и только потом выполняется сервисом.

5. **POLER Object Table** — все ресурсы (файлы, окна, сокеты, память) — это объекты с capabilities. Не маппить HANDLE/fd напрямую на внутренние структуры.

6. **VirtIO** — сохранён для QEMU-тестирования (GPU, сеть, блок), но основная архитектура не зависит от виртуализации.

7. **Полная математическая модель POLER Core** уже реализована в `poler_core.zig` (9/9 тестов проходят). Это Feistel + S-Box + LHCA с деформированным тензорным произведением.

8. **Rust Safety Core** скелет уже есть в `rust-core/src/`. Нужна интеграция с Zig kernel через `extern "C"` FFI.

9. **Приоритеты реализации:** Сначала исправить баги → 64-bit → Memory/Process → Intent Layer → ELF Adapter → GPU/Wayland → Network → PE Adapter.

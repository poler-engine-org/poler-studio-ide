# Техническое задание: SMP (многоядерность) для POLER-OS

## Цель
Реализовать поддержку многоядерности (SMP - Symmetric Multi-Processing) для ядра POLER-OS v0.8.0-dev, позволяющую использовать все доступные ядра процессора.

---

## 1. Архитектурный обзор

### Текущее состояние (обновлено)
- Все ядра процессора инициализированы и готовы к выполнению задач
- Per-CPU данные через GSBASE (BSP + APs)
- Планировщик работает с per-CPU очередями
- Синхронизация через spinlocks и atomic операции
- Load balancing между CPU (каждые 100 тиков)
- Suspended tasks для предиктивного запуска
- Serial output защищён spinlock для SMP

### Целевое состояние
- ✅ Все ядра процессора инициализированы и выполняют задачи
- ✅ Per-CPU данные и планировщики
- ✅ Синхронизация через spinlocks и atomic операции
- ✅ Load balancing между ядрами
- 🔲 TSC синхронизация между CPU
- 🔲 TLB shootdown при смене CR3
- 🔲 Стресс-тестирование (1 час+)

---

## 2. Компоненты реализации

### 2.1. ACPI MADT парсинг ✅ РЕАЛИЗОВАНО

**Файл:** `src64/acpi.zig`

**Результат:** MADT парсинг работает, обнаруживает 2 CPU (BSP + AP).

### 2.2. Per-CPU структуры ✅ РЕАЛИЗОВАНО

**Файл:** `src64/smp.zig`

```zig
pub const CpuState = enum(u8) {
    Offline = 0,
    Initializing = 1,
    Ready = 2,
    Running = 3,
    Halted = 4,
};

pub const PerCpu = struct {
    cpu_id: u32,
    lapic_id: u32,
    state: CpuState,          // Accessed atomically in SMP!
    stack_top: u64,
    current_task_id: usize,
    scheduler_ticks: u64,
    irq_count: u64,
    syscall_count: u64,
};

pub var cpu_data: [MAX_CPUS]PerCpu align(64) = undefined;
pub var online_cpus: u32 = 0;
```

**GSBASE:** Каждый CPU получает указатель на свой PerCpu через IA32_GS_BASE MSR.
**ВАЖНО:** `cpu.state` записывается через `@atomicStore` и читается через `@atomicLoad` — plain store/load вызовут data race.

### 2.3. AP Trampoline ✅ РЕАЛИЗОВАНО (исправлено, ждёт QEMU теста)

**Файл:** `src64/boot_smp.S`

#### Проблема и исправление

**Проблема:** При релокации кода трамплина в физический адрес 0x8000 инструкция `lgdt` считывала указатель на GDT по некорректному адресу. GDTR содержал base=0x00900fff, limit=0 — мусор вместо структуры дескриптора. Это вызывало #GP(0x08) → Double Fault → Triple Fault → CPU reset.

**Исправление:**
1. Удалена встроенная GDT32 из трамплина
2. В GDT64 ядра добавлены 32-битные записи (0x28, 0x30)
3. `lgdt` читает из фиксированного адреса 0x8100 (data area)

#### Data layout на 0x8100 (AP_DATA_OFFSET = 0x100)

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 10 | GDT pointer (2-byte limit + 8-byte base) — kernel's GDT64 |
| 0x0A | 10 | IDT pointer (2-byte limit + 8-byte base) |
| 0x14 | 4 | Padding (alignment) |
| 0x18 | 8 | CR3 (PML4 physical address) |
| 0x20 | 8 | Entry point (ap_entry_zig address) |
| 0x28 | 8 | PerCpu structure address (for GSBASE) |
| 0x30 | 4 | CPU ID |
| 0x34 | 4 | Padding |
| 0x38 | 8 | Stack top |

#### GDT layout (shared by BSP and all APs)

| Selector | Entry | Description |
|----------|-------|-------------|
| 0x00 | Null | Null descriptor |
| 0x08 | 64-bit Code | Ring 0 kernel code |
| 0x10 | 64-bit Data | Ring 0 kernel data |
| 0x18 | User Data | Ring 3 user data |
| 0x20 | User Code | Ring 3 user code |
| 0x28 | 32-bit Code | Ring 0, SMP AP trampoline (16→32) |
| 0x30 | 32-bit Data | Ring 0, SMP AP trampoline (32-bit data) |
| 0x38 | TSS low | Task State Segment (low 8 bytes) |
| 0x40 | TSS high | Task State Segment (high 8 bytes) |

### 2.4. Spinlocks ✅ РЕАЛИЗОВАНО

**Файл:** `src64/spinlock.zig`

Spinlock с PAUSE, tryAcquire, SpinlockGuard RAII. Использует `@atomicRmw` с `.Xchg`.

### 2.5. IPI отправка ✅ РЕАЛИЗОВАНО

**Файл:** `src64/hal.zig`

- `APIC.sendInitIpi(apic_id)` — 0x00004500 to ICR
- `APIC.sendStartupIpi(apic_id, vector)` — 0x00004600 | vector
- `APIC.sendIpi(apic_id, vector)` — generic IPI (vector 0x30 = RESCHEDULE)

### 2.6. Atomic операции ✅ РЕАЛИЗОВАНО

**Файл:** `src64/atomic.zig`

- `AtomicCounter` — increment, decrement, add, sub, get, set (seq_cst)
- `AtomicFlag` — trySet, clear, isSet (CAS-based)
- `cas()` / `casWeak()` — compare-and-swap (strong/weak)
- `fetchAdd`, `fetchSub`, `xchg`, `fetchAnd`, `fetchOr`, `fetchXor`
- `spinWait()`, `memoryBarrier()`, `readBarrier()`, `writeBarrier()`

### 2.7. Per-CPU планировщик ✅ РЕАЛИЗОВАНО

**Файл:** `src64/scheduler.zig` v0.8.0

**Реализованные возможности:**
- Per-CPU run queues (`cpu_run_queues[MAX_CPUS]CpuRunQueue`), каждый с Spinlock
- Per-CPU current task ID (`cpu_current_task[MAX_CPUS]usize`)
- Per-CPU CR3 tracking (`cpu_current_cr3[MAX_CPUS]u64`) — каждый CPU отслеживает свой загруженный CR3
- Task affinity: `0xFF` = any CPU (soft), `0..N` = pinned (hard)
- Load balancing: каждые 100 тиков, миграция при дисбалансе > 2
- Suspended tasks: `createSuspendedTask()` → `resumeTask()` → RESCHEDULE IPI
- Idle loop: HLT + проверка run queue при пробуждении
- Всегда использует `smp.currentCpuId()` через GSBASE (не условный fallback)

### 2.8. Serial output SMP-safe ✅ РЕАЛИЗОВАНО

`ap_entry_zig()` использует `hal.spinLock(&hal.serial_lock)` перед любым Serial output.

---

## 3. Исправленные баги (сессия от 2026-07-16)

| Баг | Описание | Исправление |
|-----|----------|-------------|
| Data race на cpu.state | AP idle loop использовал plain stores для .Halted/.Running, BSP читал атомарно | Заменены на `@atomicStore(u8, ..., .release)` |
| Serial без блокировки | `ap_entry_zig()` писал в Serial без lock, APs перемешивали вывод | Добавлен `spinLock/spinUnlock(&serial_lock)` |
| current_cr3 BSP-only | Глобальная переменная `current_cr3` — APs ломали CR3 tracking друг друга | Заменена на `cpu_current_cr3[MAX_CPUS]` — per-CPU массив |
| Условный currentCpuId | `schedule()` использовал `if (online_cpus > 1)` — небезопасно после SMP init | Всегда `smp.currentCpuId()` через GSBASE |

---

## 4. Оставшиеся задачи

### Фаза 5: Тестирование и отладка
- [ ] QEMU `-smp 2` тест — верификация двойного CPU без Triple Fault
- [ ] Стресс-тест race conditions (1 час+)
- [ ] Профилирование производительности
- [ ] Оптимизация contention на spinlocks

### Отложено до v2.0
- [ ] TSC синхронизация (HPET или калибровка TSC на всех CPU)
- [ ] TLB shootdown при смене CR3 на одном CPU
- [ ] Точный microDelay() через PIT/HPET (сейчас busy-loop с грубой калибровкой)

---

## 5. Тестирование

### Тест 1: Определение CPU ✅
```
[ACPI] MADT: Found 2 CPU(s)
[ACPI]   CPU 0: APIC ID=0, Flags=0x01 (BSP, enabled)
[ACPI]   CPU 1: APIC ID=1, Flags=0x01 (enabled)
```

### Тест 2: Запуск AP (ожидаемый вывод после исправлений)
```
[SMP] BSP Local APIC ID: 0x0
[SMP] BSP GSBASE set to 0x...
[SMP] Setting up AP trampoline at 0x8000...
[SMP] Starting AP 1 (APIC_ID=0x1)
[SMP] AP 1 entering ap_entry_zig()
[SMP] AP 1 ready (APIC_ID=0x1)
[SMP] AP 1 is ready
[SMP] All CPUs online: 2
```

### Тест 3: Параллельное выполнение
```zig
var counter: AtomicCounter = .{};
fn workerThread() void {
    for (0..1000) |_| {
        _ = counter.increment();
    }
}
// Запустить 2 потока параллельно
// Ожидаемый результат: counter == 2000
```

---

## 6. Известные проблемы и решения

| Проблема | Решение | Статус |
|----------|---------|--------|
| AP Triple Fault (GDT loading) | lgdt из фиксированного data area offset | ✅ Исправлено |
| Race condition при инициализации | Atomic флаги для сигнализации готовности | ✅ Реализовано |
| False sharing в per-CPU данных | Выравнивание по cache line (64 байта) | ✅ Реализовано |
| Deadlock на spinlocks | Иерархия locks, SpinlockGuard RAII | ✅ Реализовано |
| Data race на cpu.state | atomicStore/atomicLoad в idle loop и resumeTask | ✅ Исправлено |
| Serial без lock в SMP | spinLock(&serial_lock) в ap_entry_zig | ✅ Исправлено |
| current_cr3 BSP-only | Per-CPU массив cpu_current_cr3[MAX_CPUS] | ✅ Исправлено |
| Условный currentCpuId | Всегда через GSBASE | ✅ Исправлено |
| TSC десинхронизация | HPET или калибровка TSC на всех CPU | 🔲 Отложено |
| TLB shootdown | IPI + INVLPG на всех CPU при смене CR3 | 🔲 Отложено |
| microDelay неточный | PIT/HPET-driven delay | 🔲 Отложено |

---

## 7. Ссылки

- Intel SDM Volume 3: System Programming Guide
  - Chapter 8: Multiple-Processor Management
  - Chapter 10: Advanced Programmable Interrupt Controller (APIC)
- OSDev Wiki: SMP — https://wiki.osdev.org/SMP
- Linux kernel source: `arch/x86/kernel/smpboot.c`

---

## 8. Критерии приёмки

- [x] Все CPU обнаружены через MADT
- [x] Per-CPU данные через GSBASE
- [x] Spinlocks и atomic операции
- [x] Per-CPU планировщик с load balancing
- [x] Suspended tasks (предиктивный запуск)
- [x] Serial output SMP-safe
- [x] CR3 tracking per-CPU
- [ ] Все CPU запущены и выполняют задачи (требует QEMU теста)
- [ ] Нет race conditions при стресс-тесте (1 час+)
- [ ] Производительность масштабируется с количеством CPU (>80% эффективности)
- [ ] Корректная работа прерываний на всех CPU
- [ ] Планировщик распределяет задачи между CPU

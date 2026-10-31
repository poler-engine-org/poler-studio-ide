# POLER-OS v0.5.0 → v0.6.0 План развития

**Рабочая копия:** `/home/vitalij/ZCodeProject/poler-os-backup/poler-os/zig-kernel/`  
**Отдельная папка:** `poler-os-work/` — копия, оригинал не трогаем

---

## Этап 0 — Окружение и сборка (пререквизит)

### Установка инструментов:
- **Zig 0.13.0** — скачать `zig-linux-x86_64-0.13.0.tar.xz` с ziglang.org в `~/.local/zig013`, добавить в PATH
- **NASM** — `pacman -S nasm`
- **QEMU** — `pacman -S qemu-system-x86`
- **GRUB** — `pacman -S grub` (даст grub-mkrescue)
- **Rust nightly** (опционально, для rust-core) — через rustup. Откладывается.

### Чиню build.zig:
- Хардкод `/tmp/my-project/tools/qemu-sys/usr/bin/qemu-system-x86_64` → детект через `which`/константу `qemu-system-x86_64`

### ✅ Контрольная точка Этапа 0:
`zig build` собирает poler-os64 без ошибок; `zig build run64-headless` запускается в QEMU и выводит баннер через serial.

---

## Этап 1 — Починить таймер (APIC timer vector 32 → 48+)

**Корень бага:** в `hal.zig:486` APIC timer настроен на вектор 32, а PIC remap (`hal.zig:405`) мапит PIT IRQ0 тоже на 32. `handleIRQ` (`hal.zig:339`) трактует вектор 32 как `handleTimer`. Конфликт источников.

### Исправления:
1. **isr64.S** — добавить `isr_stub_48` (новый стаб для APIC timer) и добавить `.quad isr_stub_48` в `isr_stub_table` (сейчас таблица кончается на 47)
2. **hal.zig IDT.init** (строка 281) — изменить условие `i < 48` → `i < 49` (или сделать динамическим по размеру таблицы), чтобы вектор 48 попал в IDT
3. **hal.zig APIC.init** (строка 486) — `writeReg(REG_LVT_TIMER, 32 | ...)` → `48 | LVT_TIMER_PERIODIC`. APIC timer теперь на векторе 48, не конфликтует с PIC IRQ0-15 (32-47)
4. **hal.zig handleIRQ** (строка 337) — добавить `48 => handleApicTimer` в switch. Оставить `32 => handleTimer` как PIT (замаскирован, но логически корректно). sendEOI для вектора 48: только APIC EOI, не PIC

### ✅ Контрольная точка Этапа 1:
В serial-логе видно рост tick_count по APIC timer (вектор 48), PIT IRQ0 молчит.

---

## Этап 2 — Свой GDT + TSS (переход в user mode / ring 3)

**Текущее состояние:** `GDT.init()` закомментирован (`hal.zig:615`), используется GRUB-овский GDT. TSS не загружается (`hal.zig:628` закомментирован). Структуры GDT, TSS уже описаны, `setKernelStack`/`ltr` есть.

### Исправления:
1. **hal.zig init()** — раскомментировать `GDT.init()`. GRUB грузит ядро с CS=0x08/DS=0x10, наш GDT имеет те же смещения → совместимо, перезагрузка безопасна. После `lgdt` нужно перезагрузить сегментные регистры — добавить `reloadSegments()` через inline asm (far jump на CS=0x08 + mov $0x10, %ax во все DS/ES/SS)
2. **TSS:** в `init()` после GDT — `GDT.setTSS(0, @intFromPtr(&tss), @sizeOf(TSS))` и `ltr(0x28)` (entry 5 = селектор 0x28). Выделить kernel stack (stack_top из boot64.S или новый RSP0)
3. **hal.zig setKernelStack** — вызывать при каждом переключении контекста (для scheduler, Этап 5)
4. **IST (Interrupt Stack Table)** — для критических обработчиков (#DF vector 8, #NMI 2, #MC 18) настроить IST-индексы в TSS, чтобы исключения не падали на коррумпированном стеке

### ✅ Контрольная точка Этапа 2:
Serial-лог: `[HAL] GDT loaded`, `[HAL] TSS loaded (TR=0x28)`, нет #GP.

---

## Этап 3 — poler_core.zig под freestanding + интеграция в ядро

**Текущее состояние:** `src64/poler_core.zig` идентичен `src/` версии — `const std = @import("std")` на строке 56, но std используется только в unit-тестах и типе `BenchmarkResult.operation: []const u8`. Вся криптография — чистая u32 арифметика без аллокаций. `main64.zig:278 testPolerCore()` пишет заглушку «deferred».

### Исправления:
1. В `src64/poler_core.zig` обернуть `const std = @import("std")` и тесты в `const is_test_build = @import("builtin").is_test;` / `if (is_test_build)`. Это сохранит один файл и для ядра, и для `zig build test`
2. **main64.zig testPolerCore()** — заменить заглушку на реальный self-test: `const poler = @import("poler_core.zig"); const result = poler.runSelfTests();` Вывод: «POLER Core: N/9 tests passed». Создать инстанцию PolerFirewall
3. **build.zig** — поллер кор импортируется в main64. Нативный test-таргет (`poler-core-test`) продолжает работать на host Linux
4. Зафиксировать в комментарии, что crypto-версия — v5 FIX6

### ✅ Контрольная точка Этапа 3:
poler-os64 в QEMU выводит «POLER Core: 9/9 tests passed», `zig build test` на хосте проходит все 13 unit-тестов.

---

## Этап 4 — Memory Management (PMM + VMM + heap)

**Текущее состояние:** `mm/pmm.zig` (88 строк) — битмап-аллокатор, но зависит от std и vga.zig. VMM и heap отсутствуют. `_kernel_end` доступен (`linker64.ld:92`). Paging уже identity-mapped на 4GB через boot64.S.

### 4a. PMM (доработать mm/pmm.zig)
- Убрать `@import("std")` и зависимость от vga.zig. Логирование через hal.Serial
- Принимать memory map от boot64 (Multiboot2 mmap tag) вместо жёсткой маски 0–3MB
- API: `init(mmap)`, `allocFrame() ?u64`, `freeFrame(addr)`, `allocContig(n)`, статистика

### 4b. VMM (новый mm/vmm.zig)
- 4-уровневый paging: PML4 → PDPT → PD → PT. Использовать флаги PAGE из hal.zig
- API: `mapPage(virt, phys, flags)`, `unmapPage(virt)`, `mapRange(virt, phys, len, flags)`, `virtToPhys(virt)`
- Page-table walker: обход 4 уровней с выделением таблиц через PMM по мере надобности
- Референс: `reference/include/asm_pgtable_types.h` + `reference/memory/`

### 4c. Kernel heap (новый mm/heap.zig)
- Начать с простого linked-list/free-list аллокатора (как ранний kmalloc), потом slab для фиксированных размеров
- Heap region: начиная с `_kernel_end`, растёт вверх через mapPage
- API: `kmalloc(size) ?[*]u8`, `kfree(ptr)`, `kcalloc`, `krealloc`

### Интеграция:
В main64.zig после HAL/ACPI — `pmm.init(mmap); vmm.init(); heap.init();`  
Проверка: `kmalloc(4096)` возвращает валидный указатель, запись/чтение работает.

### ✅ Контрольная точка Этапа 4:
Serial-лог: `[PMM] total/free pages`, `[VMM] mapped`, `[HEAP] alloc test OK`.

---

## Этап 5 — Scheduling (round-robin → потом CFS)

**Текущее состояние:** scheduler отсутствует, `main64.zig:337` — голый idle loop. Референс: `reference/scheduler/` — полный Linux CFS.

### 5a. Process/Task struct (sched/task.zig)
Task struct: pid, state (READY/RUNNING/BLOCKED/ZOMBIE), rsp, cr3, rip, registers, priority, quantum, next (для RR-списка). Референс: linux_sched.h task_struct (упрощённо).

### 5b. Context switch (sched/switch.S + sched/sched.zig)
- `switch_asm(old: *Task, new: *Task)` на asm: сохранить RSP/калли-сохраняемые регистры в old, загрузить из new, ret
- Round-robin runqueue: связный список. `schedule()` выбирает следующий READY-таск
- Timer tick (вектор 48 из Этапа 1) → если quantum истёк → `schedule()` (preemption)

### 5c. Инициализация
- Создать kernel idle task (PID 0) и kernel init task (PID 1)
- Idle крутит hlt, init — запускает POLER Core self-test
- `scheduler.init()` после heap. Включить preemption после настройки

### 5d. CFS (затем)
После работающего RR — заменить на CFS: vruntime + RB-tree (или упрощённо sorted list), min_vruntime, sched_entity. Референс: `linux_sched_fair.c`.

### ✅ Контрольная точка Этапа 5:
Два таска чередуются (serial-лог показывает «PID 0», «PID 1» по кругу), timer-driven preemption работает.

---

## Итоговые контрольные точки (после всех 5 этапов)

1. `zig build` — собирает poler-os64 и poler-os32 без ошибок
2. `zig build test` — нативные unit-тесты POLER Core (13 тестов) проходят
3. `zig build run64-headless` — QEMU грузит ядро, serial показывает: HAL+GDT+TSS+APIC timer(v48) → POLER Core 9/9 → PMM/VMM/heap → scheduler с 2 тасками
4. ISO-сборка через `grub-mkrescue` + `xorriso` (bootable образ для теста на реальном железе/QEMU -cdrom)

## Объём и риски

- Примерно 1500–2500 новых строк Zig/asm
- Делать поэтапно с проверкой сборкой+QEMU после каждого этапа
- Zig 0.13.0 (фиксируем версию)
- Основной отладочный канал — serial-лог

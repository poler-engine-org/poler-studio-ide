# TempleOS → VitalijOS: Что спёрто, что хрень

## Terry Davis (1969–2018) — гений. Хрен сыщешь.

Его код — это **чистая архитектурная мысль**. Один человек написал ОС с нуля:
компилятор, графический стек, scheduler, filesystem — всё сам.
Но VGA planar graphics и BIOS calls — это музей. Мы берём идеи, не реализацию.

---

## ✅ ЧТО ВЗЯЛИ (и адаптировали под UEFI GOP)

### 1. 8x8 Font (FontStd.HC) → `font_std[256]`
- **Оригинал**: 256 символов, каждый = `U64` (8 байт = 8 строк по 8 пикселей)
- **Адаптация**: тот же формат, но рендерим в 32-bit RGBA вместо VGA planar
- **Почему**: 8x8 = 64 байта/символ vs наш 8x16 = 16 байт/символ.
  FontStd компактнее (8 байт vs 16 байт на символ), но менее читаемый.
  Оставили оба варианта.

### 2. RawPutChar алгоритм (Display.HC) → `rawPutChar8x8()`
- **Оригинал**: писал прямо в VGA planar memory через `text.vga_alias`
  с использованием `rev_bits_table` для инверсии бит
- **Адаптация**: пишем в линейный 32-bit GOP framebuffer через `putPixel()`
  Бит 7 = левый пиксель — реверс НЕ нужен для GOP
- **Ключевая идея Терри**: простая сетка текста
  `row = col / text.cols`, `col = col % text.cols`
  Позиция пикселей: `x = col * FONT_WIDTH`, `y = row * FONT_HEIGHT`

### 3. PCI Configuration (PCIBIOS.HC) → `pciReadU32/pciWriteU32`
- **Оригинал**: через BIOS32 PCI services (FarCall32 — переключение в 32-bit!)
- **Адаптация**: прямое чтение/запись через порты 0xCF8/0xCFC
  В UEFI не нужен FarCall32 — у нас полный Ring 0 доступ
- **Идея**: `PCIClassFind()` — поиск устройства по class code

### 4. Identity Mapping (PageTables.HC)
- **Оригинал**: вся память identity-mapped, 2MB или 1GB страницы
- **Адаптация**: UEFI уже оставляет identity mapping, но для
  Linux Driver Server (Ring -1 через VT-x) нужно своё
- **Идея Терри**: `dev.uncached_alias` — uncached зеркальное отображение
  первого 4GB для device access. Это гениально — просто и эффективно.

### 5. Reboot (KMain.HC) → `reboot()`
- **Оригинал**: порт 0x92 (fast reset) + keyboard controller
- **Адаптация**: тот же метод для bare metal,
  но также `RuntimeServices->ResetSystem()` из UEFI

---

## ❌ ЧТО НЕ ВЗЯЛИ (хрень для наших целей)

### 1. VGA Planar Graphics (0xA0000, 4 bit-planes)
- **Почему хрень**: не работает через HDMI на GTX 1060
  Нужен GOP linear framebuffer — и это РЕШИЛО проблему чёрного экрана
- **Что вместо**: наш `framebuffer.zig` с 32-bit RGBA рендерингом

### 2. VGA Text Mode (0xB8000)
- **Почему хрень**: вообще не работает через HDMI
  Это и была причина чёрного экрана Vitalij
- **Что вместо**: попиксельный текст через bitmap font + GOP

### 3. 16-color VGA Palette
- **Почему хрень**: 4 бита на пиксель — 1990 год
  У нас 32 бита (RGBA) через GOP — 16.7 миллионов цветов
- **Что вместо**: наш `COLORS` enum с 16.7M вариантов

### 4. BIOS Calls (INT 0x10, INT 0x13, FarCall32)
- **Почему хрень**: UEFI не имеет BIOS.
  Terry использовал FarCall32 для PCI BIOS — переключался
  из 64-bit в 32-bit реальный режим. Это безумие (но гениальное).
- **Что вместо**: прямой доступ к портам и MMIO

### 5. HolyC JIT Compiler
- **Красиво, но не для нас**: Terry написал JIT компилятор
  прямо в ядре — код компилируется на лету.
  Мы используем Zig (AOT) для Ring 0 и Rust для Ring 1

### 6. Cooperative Scheduler (Yield-based)
- **Интересно, но недостаточно**: Terry использовал кооперативную
  многозадачность — задача сама вызывает Yield().
  Для нашей архитектуры нужен preemptive scheduler
  (timer interrupt → context switch), потому что
  Linux Driver Server в Ring -1 не будет добровольно Yield'ить

---

## 🧠 ИДЕИ ТЕРРИ ДЛЯ БУДУЩЕГО

### 1. CTask структура — context switching
Terry хранит ВСЕ регистры в CTask (rip, rsp, rflags, rax..r15, FPU).
Это позволяет переключать задачи за ~0.5 микросекунды.
Нам это нужно для Zig scheduler.

### 2. CHeapCtrl / BlkPool — memory management
Terry делает свою кучу: BlkPool → pages → MAlloc/Free.
Это проще чем slab allocator Linux'а.
Нам нужна своя куча для Ring 0 (Zig microkernel).

### 3. HPET Timer — high precision
Terry инициализировал HPET через ACPI:
находит Intel LPC controller → включает HPET → читает frequency.
Это основа для preemptive scheduler.

### 4. SerialDev / KeyDev — device abstraction
Terry абстрагирует ввод/вывод через device драйверы.
Нам это нужно для клавиатуры (USB HID через xHCI).

---

## СВОДКА

| Компонент | TempleOS | VitalijOS | Статус |
|-----------|----------|-----------|--------|
| Видео | VGA 640x480 16-color | UEFI GOP 1920x1080 32-bit | ✅ Заменено |
| Font | 8x8 mono | 8x16 + 8x8 | ✅ Взят 8x8 |
| PCI | BIOS32 FarCall | Port I/O 0xCF8 | ✅ Адаптировано |
| Memory | Identity-mapped 2MB/1GB | UEFI identity + custom | ✅ Идея взята |
| Scheduler | Cooperative Yield | Preemptive (TODO) | 🔄 Будет |
| FS | RedSea (custom) | Linux ext4 via driver server | ❌ Другой путь |
| Compiler | HolyC JIT | Zig AOT + Rust | ❌ Другой путь |

**Итого**: взяли ~30% идей, ~10% кода. Остальное — музей,
но Terry Davis — гений, хрен сыщешь.

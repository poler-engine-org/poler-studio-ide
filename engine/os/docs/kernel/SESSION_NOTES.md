# POLER-OS — Заметки сессии (для восстановления контекста)

> Последнее обновление: 2026-07-16
> Этот файл создан чтобы не потерять контекст при сбросе сессии.

---

## 1. Текущий статус проекта

### Что работает
- **SMP**: Протестировано на QEMU `-smp 2` и `-smp 4` → `[SMP] All CPUs online: N`
- **Per-CPU планировщик**: Round-robin, load balancing, affinity, suspended tasks
- **Spinlocks + Atomic**: spinlock.zig, atomic.zig — всё реализовано
- **AP trampoline**: boot_smp.S — 16→32→64 переход, исправлен баг с lgdt
- **ACPI MADT**: Обнаружение CPU через MADT парсинг
- **HAL**: GDT (9 entries), IDT, PIC/APIC, IO-APIC, TSS, syscall entry
- **Memory**: PMM (bitmap), VMM (4-level paging), kernel heap
- **Framebuffer**: 1024×768×32bpp
- **Crypto**: PND v8, RSA-OAEP, POLER-CTR AEAD

### Ключевые файлы
- Исходники: `src64/` — все .zig и .S файлы
- Build: `zig build` (через tools/zig)
- ISO: `grub-mkrescue-wrapper` (скрипты в репо)
- QEMU: `tools/usr/local/bin/qemu-system-x86_64`

### Коммиты SMP (уже в origin/main)
`811eaee` → `6957139` → `d9a511a` → `a4141b8`

---

## 2. Инструменты (установлены вручную)

| Инструмент | Путь |
|-----------|------|
| Zig 0.13.0 | `/home/z/my-project/poler-os-work/tools/zig` |
| QEMU 7.0 | `/home/z/my-project/poler-os-work/tools/usr/local/bin/qemu-system-x86_64` |
| GRUB + xorriso | Извлечены из .deb пакетов (библиотеки в /tmp/) |

### QEMU команда теста
```bash
qemu-system-x86_64 -L /home/z/my-project/poler-os-work/tools/usr/local/share/qemu \
  -cdrom poler-os64.iso -smp 2 -m 512M -serial stdio -display none -no-reboot -nic none
```

---

## 3. Приоритеты разработки (по ROADMAP.md)

1. ✅ **SMP** — реализовано и протестировано
2. 🔲 **VFS** — виртуальная файловая система (следующий приоритет)
3. 🔲 **ФС** (ext2 → POLER-FS) + AHCI драйвер
4. 🔲 **Сетевой стек** (virtio-net, e1000)
5. 🔲 **Shim-архитектура** (Linux + Windows совместимость)

---

## 4. Изучение x16-PRos (репозиторий PRoX2011)

**Репо**: https://github.com/PRoX2011/x16-PRos  
**Локальный клон**: `/home/z/my-project/poler-os-work/x16-PRos`  
**Архитектура**: 16-bit real-mode ОС на NASM, FAT12, VGA 640x480

### Полезные концепции (подсмотрено, НЕ копировать код)

#### 4.1. Файловая система FAT12 — структурированный подход
- Чёткое разделение: `fs.asm` (низкоуровневые операции) + `api_fs.asm` (INT 0x22 API слой)
- Поддержка поддиректорий через `current_dir_cluster` (0 = root, иначе кластер поддиректории)
- `fs_get_file_list` возвращает структурированные 18-байт записи (имя + размер + атрибуты)
- Save/Restore directory state (функции 0x0E, 0x0F) — полезно для программ которые меняют cwd
- Поддержка нескольких дисков (A:, B:, C:)

**Адаптация для POLER-OS**: При проектировании VFS использовать двухуровневый API:
- Нижний уровень: блочные операции (read/write sectors через AHCI драйвер)
- Верхний уровень: VFS абстракция (inode, mount, path resolution)
- FAT12→ext2 переход проще если API уже структурирован

#### 4.2. Загрузчики программ (COM/EXE/PLE) — три формата
- **COM**: Простейший — загрузка по адресу 0x100, PSP segment, INT 20h для выхода
- **MZ EXE**: Полная поддержка — парсинг заголовка, relocation table, PSP
- **PLE**: Собственный формат (PRos Large Executable) — заголовок с таблицей load instructions, поддержка нескольких сегментов

**Адаптация для POLER-OS**: Наш ELF64 loader уже работает. При добавлении PE/COFF loader (для Windows shim) стоит посмотреть на структуру MZ header parsing из exe.asm как reference — но реализовывать на Zig для 64-bit. PLE-подход (таблица load instructions) интересен для собственного POLER-OS формата если понадобится.

#### 4.3. MS-DOS Compatibility Layer (com.asm)
- INT 0x21 эмуляция: 37 из 87 syscall реализованы
- Перехват IVT: при запуске COM программы — сохранение IVT, установка своих обработчиков
- При завершении (INT 20h) — восстановление IVT обратно
- PSP (Program Segment Prefix) для передачи параметров командной строки

**Адаптация для POLER-OS**: Это по сути то что мы планируем как shim-архитектуру, но на 16-bit уровне. Концепция сохранения/восстановления interrupt table аналогична нашему per-process syscall table подходу. Ключевое отличие: мы делаем это через IAT/GOT/PLT подмену на 64-bit, а не через IVT.

#### 4.4. Конфигурационная система (CFG files)
- Отдельные .CFG файлы: SYSTEM.CFG, USER.CFG, PASSWORD.CFG, THEME.CFG, TIMEZONE.CFG, FONT.CFG, PROMPT.CFG
- Директория CONF.DIR для конфигов
- XOR-шифрование для паролей (примитивное, но для 16-bit ОС достаточно)
- First-boot setup: SETUP.BIN запускается при первом запуске

**Адаптация для POLER-OS**: Конфигурация через файлы — простой и проверенный подход. Стоит рассмотреть для POLER-OS:
- `/etc/system.cfg` — системные настройки
- `/etc/user.cfg` — пользовательские настройки
- `/etc/theme.cfg` — тема оформления
- Вместо XOR использовать наш RSA-OAEP / POLER-CTR для защиты конфигов

#### 4.5. Тематическая система (themes.asm)
- THEME.CFG: 16 строк формата "index, r, g, b" — по одной на каждый VGA цвет
- Загрузка и применение при запуске и при `cls` с темой (API 0x0C)
- Несколько предустановленных тем: DEFAULT, DRACULA, GRUVBOX, NORD, TOKYO, MATRIX, SYNTHWAV и т.д.
- Темы хранятся как .THM файлы в assets/themes/

**Адаптация для POLER-OS**: Для framebuffer 32bpp понадобится другая система (не 16 VGA цветов). Но концепция внешних .thm файлов и runtime загрузки — хорошая идея для графической среды.

#### 4.6. Память — фиксированные сегменты
- Строгая memory map (MEM_MAP.TXT) — каждый сегмент имеет фиксированное назначение
- Kernel < 42KB в сегменте 0x2000
- Диск-буфер, dirlist, command history — фиксированные offset'ы

**Адаптация для POLER-OS**: У нас PMM/VMM — гораздо гибче. Не нужно копировать фиксированный подход, но стоит задокументировать memory map ядра.

#### 4.7. Автодополнение команд
- В shell есть autocomplete_enabled флаг
- dirlist загружается перед вводом команды
- Используется для tab-completion

**Адаптация для POLER-OS**: Для будущего shell — полезная фича.

#### 4.8. Command history
- 16 записей × 256 байт, ring buffer
- Навигация стрелками вверх/вниз

**Адаптация для POLER-OS**: Для будущего shell.

---

## 5. Что НЕ стоит переносить

- **16-bit real mode**: POLER-OS работает в 64-bit long mode — архитектурно несовместимо
- **BIOS interrupts**: У нас свой HAL, нет доступа к BIOS
- **FAT12**: Слишком ограниченная для 64-bit ОС, начинаем с ext2
- **XOR encryption**: Криптографически слабая, у нас есть RSA-OAEP + POLER-CTR
- **Фиксированные memory segments**: У нас PMM/VMM с динамическим выделением
- **Cooperative multitasking (в TODO)**: У нас preemptive с per-CPU scheduling

---

## 6. Следующие шаги (при новой сессии)

1. Начать проектирование VFS (следующий приоритет после SMP)
2. Рассмотреть структуру API слоя по аналогии с x16-PRos (низкий + высокий уровень)
3. Продолжить развитие shim-архитектуры — концепция перехвата syscall похожа на подход x16-PRos с INT 0x21
4. Запушить эти заметки в git чтобы они всегда были в репо

---

## 7. Известные проблемы/TODO

### SMP (отложено до v2.0)
- [ ] TSC синхронизация между CPU
- [ ] TLB shootdown при смене CR3
- [ ] Стресс-тест 1 час+
- [ ] Точный microDelay() через PIT/HPET

### VFS (следующий приоритет)
- [ ] Абстрактный интерфейс (mount/open/read/write/close)
- [ ] Inode + metadata
- [ ] Path resolution (абсолютные/относительные, `.`, `..`, dcache)
- [ ] Per-process namespace (root_fs_inode)
- [ ] Initrd/CPIO интеграция

---

*Файл создан для сохранения контекста между сессиями. Обновлять при каждом значимом изменении.*

# POLER Bare-Metal Win32 GUI Subsystem Research & Forensic Report
**Дата исследования:** 22 сентября 2026 г.  
**Целевая подсистема:** `src/winpe/` (нативное исполнение Windows PE32+ без Wine/VM)  
**Директива для GLM 5.3 / Архитектора:** Доведение прямого Win32 GUI до рабочего состояния внутри `poler-box`.

---

## 1. Контекст и цели
Цель проекта — исполнение нативных 64-битных бинарников Windows (`PE32+ AMD64`) и графической подсистемы (Win32 GUI / Explorer / Notepad2) **напрямую на регистрах процессора x86-64** через микро-рантайм `poler-engine` без эмуляции и без Wine.

### Достигнутые результаты:
1. **Консольный рантайм (`v0.42.0 winpe`):**
   - Полная поддержка `7za.exe` (7-Zip x64), CRT (msvcrt), IAT-патчинг через 22-байтные тюнки на ассемблере, C++ SEH unwinding, релокации `DIR64`.
   - Время старта: **15 мс**, потребление памяти: **<5 МБ RAM**.
2. **Инициализация GUI-приложений (`Notepad2.exe` x64):**
   - Загрузка образа (6 секций), разрешение 469 импортов Win32 API.
   - Поднятие TEB/PEB через регистр `GS` (`arch_prctl(ARCH_SET_GS)`).
   - Успешная инициализация CRT, критических секций и кучи `HeapAlloc`.

---

## 2. Анализ инцидента сбоя сессии и краша (#GP)

### Логи ядра Linux:
```text
kernel: traps: poler-engine[6815] general protection fault ip:7f5b59992af9 sp:7f5b597fed20 error:0
systemd-coredump: Process 6815 (poler-engine) of user 1000 terminated abnormally with signal 11/SEGV, dumped core.
```

### Причины:
1. **Нереализованные функции графики (User32 / GDI32 / DWM):**
   - При переходе программы от инициализации к созданию окна `CreateWindowExW` или циклу выборки сообщений `GetMessageW`, вызовы падали в заглушки-нули или невалидные структуры хэндлов `HWND/HDC`.
2. **Сброс дампов памяти (`coredump`):**
   - Приложение попыталось обратиться по невалидному указателю, что привело к сбросу нескольких гигабайт памяти в `systemd-coredump`, вызвав кратковременное зависание графического композитора рабочего стола.

---

## 3. Исправления, уже внесённые в код `src/winpe/api.rs`:

1. **Little-Endian 32-bit Futex Bugfix:**
   - В `h_EnterCriticalSection` и `h_LeaveCriticalSection` указатель на фьютекс исправлен с `cs` (младшие 4 байта глубины) на `(cs + 4) as *const i32` (старшие 4 байта, содержащие TID владельца).
   - Добавлен таймаут ожидания `50ms` и корректное сравнение `owner as i32`, предотвращающее вечный спинлок.

---

## 4. Архитектурный план для GLM 5.3 (Завершение GUI-стека):

### Задача 1: Изоляция GUI в `poler-box` (Защита хоста)
- Никаких запусков графических `.exe` на живом дисплее хоста.
- Запуск только внутри `poler-box` с виртуальным скрытым X/Wayland фреймбуфером (или memfd DIBSection).

### Задача 2: Реализация минимального набора Win32 GUI API:
1. `user32.dll`:
   - `RegisterClassExW` / `RegisterClassExA` $\rightarrow$ регистрация оконного класса.
   - `CreateWindowExW` $\rightarrow$ выделение виртуального `HWND` с внутренним DIB-буфером.
   - `ShowWindow`, `UpdateWindow`, `SetWindowPos`.
   - `GetMessageW`, `PeekMessageW`, `TranslateMessage`, `DispatchMessageW` $\rightarrow$ очередь оконных событий.
   - `DefWindowProcW` $\rightarrow$ базовая обработка сообщений (`WM_CREATE`, `WM_PAINT`, `WM_DESTROY`, `WM_CLOSE`).
2. `gdi32.dll`:
   - `CreateCompatibleDC`, `CreateDIBSection` (выделение линейной RGBA32 видеопамяти).
   - `SelectObject`, `DeleteObject`, `BitBlt`, `StretchBlt`.
3. `uxtheme.dll` / `comctl32.dll`:
   - Заглушки тем оформления и стандартных контролов.

---

## 5. Расположение артефактов в архиве:
- `ubuntu24_rootfs.poler` — сжатый контейнер Ubuntu 24.04 (28 МБ).
- `windows_tools.poler` — 7-Zip PE64 (572 КБ).
- `win_live_iso.poler` — упакованный Live ISO с полным набором DLL и приложений (Notepad, Calc, Explorer).
- `Notepad2.exe` — целевой тестовый 64-битный GUI бинарник.

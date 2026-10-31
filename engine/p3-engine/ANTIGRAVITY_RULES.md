# P³ Engine — Правила для автоматического рефакторинга (антигравити)

## ЗАПРЕЩЕНО менять (CRITICAL — ломает сборку):

### 1. build.zig.zon — НЕ ТРОГАТЬ
- `.name = .P3_Engine` — ОБЯЗАТЕЛЬНО enum literal (не string!), Zig 0.14.0 требует
- `.fingerprint` — ОБЯЗАТЕЛЬНО, Zig 0.14.0 требует
- `.dependencies` — НЕ УДАЛЯТЬ! Используются в `p3-gpu` target:
  - `zglfw` — window/input для GPU binary
  - `zgpu` — WebGPU/Dawn rendering
  - `dawn_x86_64_linux_gnu` — Dawn prebuilt (lazy=true)
- Если деп看起来 "неиспользуемый" — он используется в p3-gpu target!

### 2. build.zig — GPU target НЕ УПРОЩАТЬ
- `b.dependency("zgpu", ...)` и `b.dependency("zglfw", ...)` — НЕ менять на lazyDependency
- `@import("zgpu").addLibraryPathsTo(exe_gpu)` — КРИТИЧНО, должно быть ДО linkLibrary
- `exe_gpu.linkLibrary(zgpu_dep.artifact("zdawn"))` — НЕ УДАЛЯТЬ, без этого нет Dawn
- `exe_gpu.linkLibrary(zglfw_dep.artifact("glfw"))` — НЕ УДАЛЯТЬ, без этого нет GLFW
- Порядок: addLibraryPathsTo → linkLibrary(zdawn) → linkLibrary(glfw)

### 3. p3_serial.zig — @typeInfo tags = lowercase
- Zig 0.14.0 использует lowercase: `.@"struct"`, `.@"enum"`, `.int`, `.float`, `.bool`, `.array`, `.optional`
- НЕ менять на PascalCase (.Struct, .Enum, .Int) — это Zig 0.13, не 0.14!
- Также НЕ менять `std.ArrayList` → `std.arrayList`

### 4. zgpu_stub.zig — callconv = lowercase
- `callconv(.c)` — правильно для Zig 0.14.0
- `callconv(.C)` — это Zig 0.13, НЕ менять!

## РАЗРЕШЕНО менять (safe to refactor):

- Комментарии в build.zig (добавлять/улучшать)
- Имена локальных переменных (кроме zglfw_dep, zgpu_dep)
- Порядок system library linking для raylib demo
- Добавлять новые build steps/targets
- Рефакторить helper функции (addP3Imports и т.д.)
- Улучшать stubs (добавлять методы), но НЕ менять сигнатуры существующих

## Версия Zig: 0.14.0
- Все изменения должны быть совместимы с Zig 0.14.0
- Проверять: `zig build test && zig build p3 && zig build p3-gpu`
- Если любой из трёх не компилируется — откатить изменение

#!/usr/bin/env bash
# =============================================================================
# build.sh — пересборка libp3ffi.so (C-ABI мост P³ → POLER ENGINE)
# =============================================================================
# Цикл W (v0.59.0). Исходники P³ Engine ВЕНДОРЕНЫ в репозиторий:
#   p3-engine/src/p3_ffi.zig  — C-ABI слой (цикл W, восстановлен)
#   p3-engine/src/*.zig       — ядро P³ (59 модулей, апстрим Kotokvit/P3_Engine)
#
# СБОРКА ЦЕЛИ БАЗОВЫЙ x86-64 (SSE2) — никакой AVX2/SIGILL на Ivy Bridge.
#
# Использование:
#   ./ffi/build.sh                          # vendored p3-engine/ (по умолчанию)
#   ZIG=/путь/zig ./ffi/build.sh            # свой тулчейн Zig 0.14.0
#
# Результат проверяется живым рукопожатием:
#   poler-engine --exec "p3 info"
#   poler-engine --exec "p3 conformance --pairs 256"
# =============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
P3_SRC="${1:-${P3_FFI_SRC:-$HERE/../p3-engine}}"
ZIG="${ZIG:-zig}"

if ! command -v "$ZIG" >/dev/null 2>&1; then
    echo "❌ Zig не найден. Установите 0.14.0:" >&2
    echo "   curl -sL https://ziglang.org/download/0.14.0/zig-linux-x86_64-0.14.0.tar.xz | tar xJ" >&2
    exit 1
fi

ZIG_VER="$("$ZIG" version)"
if [ "$ZIG_VER" != "0.14.0" ]; then
    echo "⚠️  Внимание: Zig $ZIG_VER, канал сборки P³ — 0.14.0 (продолжаем)" >&2
fi

if [ ! -f "$P3_SRC/src/p3_ffi.zig" ]; then
    echo "❌ $P3_SRC/src/p3_ffi.zig не найден. Передайте путь:" >&2
    echo "   ./ffi/build.sh /путь/к/P3_Engine" >&2
    exit 1
fi

echo "▸ Сборка libp3ffi.so (ReleaseFast, x86-64 baseline SSE2) из $P3_SRC"
( cd "$P3_SRC" && "$ZIG" build p3-ffi -Doptimize=ReleaseFast )

echo "▸ Самопроверка FFI (zig build test-ffi)"
( cd "$P3_SRC" && "$ZIG" build test-ffi )

echo "▸ Копирование в $HERE/libp3ffi-linux-x86_64.so"
cp "$P3_SRC/zig-out/lib/libp3ffi.so" "$HERE/libp3ffi-linux-x86_64.so"

echo "▸ Проверка: ни одной AVX2-инструкции (ymm)"
if objdump -d "$HERE/libp3ffi-linux-x86_64.so" 2>/dev/null | grep -q 'ymm'; then
    echo "❌ НАЙДЕНЫ ymm-инструкции — .so упадёт SIGILL на старых CPU!" >&2
    exit 1
fi
echo "✅ Чисто: базовый x86-64. Проверка из poler-engine:"
echo "   poler-engine --exec 'p3 info'"

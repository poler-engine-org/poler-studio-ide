#!/usr/bin/env bash
# =============================================================================
# P3 Engine Launcher — запуск через zig build
# =============================================================================
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Проверяем наличие zig
if ! command -v zig &> /dev/null; then
    # Пробуем распакованный Zig из download/
    ZIG_BIN="$HOME/my-project/download/zig-linux-x86_64-0.14.0/zig"
    if [ -f "$ZIG_BIN" ]; then
        echo "Используем локальный Zig: $ZIG_BIN"
        "$ZIG_BIN" build run-launcher -- "$@" "$REPO_DIR"
    else
        echo "Ошибка: Zig не найден. Установите zig 0.14.0 или распакуйте архив."
        exit 1
    fi
else
    echo "=== Запуск P3 Engine Launcher ==="
    echo "Репозиторий: $REPO_DIR"
    cd "$REPO_DIR"
    zig build run-launcher -- "$@"
fi

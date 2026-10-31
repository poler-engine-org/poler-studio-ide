#!/usr/bin/env bash
# ==============================================================================
# POLER RUNNER: SHANNON BYPASS & FLYWIRE BENCHMARK SUITE (v2, аудит 2026-09-21)
#
# Изменения по итогам аудита:
#   - портативное обнаружение zig: PATH → pip-пакет ziglang → ~/.local/bin/zig
#   - запускаются ВСЕ 4 компонента (раньше 02_profile пропускался)
#   - коды выхода пробрасываются; итог честный (не декларативный)
# ==============================================================================

set -u

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

FAILED=0

# ---------- портативный запуск zig ----------
run_zig() {
    local script="$1"
    if command -v zig >/dev/null 2>&1; then
        zig run "$script" -O ReleaseFast
    elif python3 -c "import ziglang" >/dev/null 2>&1; then
        # pip install ziglang==0.14.1 (0.16 ломает синтаксис asm)
        python3 -m ziglang run "$script" -O ReleaseFast
    elif [ -x "$HOME/.local/bin/zig" ]; then
        "$HOME/.local/bin/zig" run "$script" -O ReleaseFast
    else
        echo "  [SKIP] $script: zig не найден (поставьте: pip install ziglang==0.14.1)"
        return 2
    fi
}

echo "================================================================="
echo "   POLER: ПОВНИЙ КОМПЛЕКС ВЕРИФІКАЦІЇ ТА ОБХОДУ ОБМЕЖЕНЬ ШЕННОНА"
echo "================================================================="
echo ""

echo ">>> [1/5] Математична верифікація (Архетип/сід, Мак-Віні, Ландауер)..."
python3 04_shannon_bypass_math_verifier.py || FAILED=1

echo ""
echo ">>> [2/5] Побітний профайлер тактів ALU No-Mul (RDTSC)..."
run_zig 01_cpu_cycle_verifier_rdtsc.zig || FAILED=1

echo ""
echo ">>> [3/5] Кеш-ієрархія L2/L3 vs справжня DRAM (48МБ working set)..."
run_zig 02_flywire_cycle_ddr3_profiler.zig || FAILED=1

echo ""
echo ">>> [4/5] Подійна симуляція мозку масштабу FlyWire (event-driven)..."
run_zig 03_flywire_event_driven_1khz.zig || FAILED=1

echo ""
echo ">>> [5/5] Побітова верифікація РЕАЛЬНОГО CSR коннектома FlyWire v783..."
python3 native_pc_scripts/flywire/flywire_verify.py || FAILED=1

echo ""
echo "================================================================="
if [ "$FAILED" -eq 0 ]; then
    echo "   ВСІ КОМПОНЕНТИ ПРОЙДЕНО БЕЗ ПОМИЛОК (коди виходу 0)"
else
    echo "   Є ЗБОЇ — див. лог вище (FAILED=$FAILED)"
fi
echo "================================================================="
exit "$FAILED"

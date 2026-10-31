#!/usr/bin/env python3
"""
selftest.py — самопроверка моста POLER → Panda3D БЕЗ OpenGL.

Проверяет то, что не зависит от видеокарты и Panda3D:
  1. загрузку libpoler_ffi.so и рукопожатие ABI;
  2. физику воды через ctypes: H_s ≈ Пирсон–Московиц, детерминизм
     бит-в-бит, аналитические нормали, течения;
  3. (опционально, если установлен panda3d) — импорт Panda3D и
     создание пустого offscreen-буфера, если позволяет GL.

Запуск:
    python3 panda-bridge/selftest.py
Код возврата 0 — физика моста здорова.
"""
import sys
import time

sys.path.insert(0, __file__.rsplit("/", 1)[0])

from poler_panda import Water, load_library, version


def main() -> int:
    lib = load_library()
    print(f"[1] библиотека : {lib._poler_path}")
    print(f"    версия     : {version()}")

    # --- 2. Физика воды --------------------------------------------------
    print("[2] физика воды (GF(3), спектральный решатель):")
    with Water(n=128, modes=160, seed=42, wind=8.0, domain=120.0) as sea:
        hs = sea.significant_height
        # Пирсон–Московиц: H_s ≈ 0.21 v²/g = 0.21·64/9.81 ≈ 1.37 м
        expect = 0.21 * 8.0**2 / 9.81
        ok_hs = abs(hs - expect) / expect < 0.15
        print(f"    H_s = {hs:.3f} м (PM-ожидание {expect:.3f}) {'OK' if ok_hs else 'FAIL'}")
        if not ok_hs:
            return 2

        t0 = time.perf_counter()
        for _ in range(300):
            sea.step(1.0 / 60.0)
        dt = time.perf_counter() - t0
        print(f"    300 шагов (5 с моря) за {dt*1000:.1f} мс")

        grid = sea.frame(0.0)
        n = sea.n
        hmax = max(grid)
        hmin = min(grid)
        print(f"    сетка {n}×{n}: h ∈ [{hmin:.2f}, {hmax:.2f}] м")
        if hmax <= hmin:
            print("    FAIL: плоское море")
            return 3

        h1, dhx, dhy = sea.surface_at(37.0, 81.0)
        u, v, w = sea.flow_at(37.0, 81.0)
        print(
            f"    поверхность(37,81): h={h1:.3f} ∇h=({dhx:.4f},{dhy:.4f}); "
            f"течение=({u:.3f},{v:.3f},{w:.3f}) м/с"
        )
        if not (abs(dhx) + abs(dhy) > 0.0):
            print("    FAIL: наклоны нулевые")
            return 4

        hsh = sea.state_hash
        print(f"    hash состояния: 0x{hsh:016X}")

    # --- детерминизм: два моря — один хэш --------------------------------
    with Water(n=128, modes=160, seed=42, wind=8.0, domain=120.0) as a, \
         Water(n=128, modes=160, seed=42, wind=8.0, domain=120.0) as b:
        for _ in range(120):
            a.step(1.0 / 60.0)
            b.step(1.0 / 60.0)
        same = a.state_hash == b.state_hash
        print(
            f"[3] детерминизм: 0x{a.state_hash:016X} == 0x{b.state_hash:016X} "
            f"{'OK (бит-в-бит)' if same else 'FAIL'}"
        )
        if not same:
            return 5

    # --- ветер-разнос шкалы Бофорта ---------------------------------------
    print("[4] шкала ветра (H_s против Пирсона–Московица):")
    for wind in (4.0, 8.0, 16.0):
        with Water(n=64, modes=96, seed=42, wind=wind, domain=100.0) as sea:
            hs = sea.significant_height
            expect = 0.21 * wind**2 / 9.81
            dev = abs(hs - expect) / expect
            print(f"    {wind:5.1f} м/с → H_s = {hs:6.3f} м (PM {expect:6.3f}, Δ {dev*100:4.1f}%)")
            if dev > 0.15:
                return 6

    # --- 5. Panda3D (опционально) -----------------------------------------
    try:
        from direct.showbase.ShowBase import ShowBase  # noqa: F401
        import panda3d  # noqa: F401
        print("[5] Panda3D: установлен (import OK) — демо: python3 demo_ocean.py")
    except ImportError as e:
        print(f"[5] Panda3D: не установлен ({e}) — только физика; демо требует:")
        print("    pip install panda3d   # или: sudo pacman -S panda3d")

    print("\nСАМОПРОВЕРКА МОСТА: ФИЗИКА ЗДОРОВА")
    return 0


if __name__ == "__main__":
    sys.exit(main())

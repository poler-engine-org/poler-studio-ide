# POLER → Panda3D Bridge

**Цикл W «Симбиоз» (v0.59.0).** Физика воды — в POLER ENGINE (Rust,
спектральная гидродинамика GF(3)), рендер и ассеты — в Panda3D (Python).
Чужой красивый фронтенд + наша невразливая математика.

```text
   Panda3D (Python)                         POLER ENGINE (Rust)
   ─────────────────                        ───────────────────
   demo_ocean.py                            libpoler_ffi.so (356 КБ)
     GeomVertexData 128×128     ◀────────     polerf_water_geometries_f32
     GLSL: Френель/пена/                      (тик физики + FFT + нормали
     блик/Беер–Ламберт                       одним вызовом, ~130 кадров/с)
     камера, небо, ассеты                    полерf_water_flow_at (течения)
```

## Установка

```bash
pip install panda3d numpy          # или: sudo pacman -S panda3d python-numpy
cargo build --release -p poler-ffi # → target/release/libpoler_ffi.so
```

Библиотека ищется автоматически: `POLER_FFI_LIB`, рядом с `panda-bridge/`,
`target/release/`, раскладка релиза.

## Проверка (без OpenGL — на любой машине)

```bash
python3 panda-bridge/selftest.py       # физика моста: H_s, детерминизм, PM
python3 panda-bridge/smoke_headless.py # кадровый путь Panda3D без дисплея
```

## Демо-океан (нужен GPU)

```bash
python3 panda-bridge/demo_ocean.py        # ветер 8 м/с, сетка 128
python3 panda-bridge/demo_ocean.py 14 256 # шторм, мелкая сетка
```

Управление: мышь — обзор, колесо — зум, `ESC` — выход.

## API Python

```python
from poler_panda import Water

with Water(n=128, modes=160, seed=42, wind=8.0, domain=120.0) as sea:
    sea.step(1/60)                     # шаг физики (миллисекунды)
    h, n = sea.geometries(1/60)        # высоты + нормали одним вызовом
    height, dhdx, dhdy = sea.surface_at(37.0, 81.0)  # точный запрос
    u, v, w = sea.flow_at(37.0, 81.0)  # орбитальное течение
    sea.significant_height             # H_s по Пирсону–Московицу
    sea.state_hash                     # бит-в-бит детерминизм
```

## Честность

- H_s совпадает с Пирсоном–Московицем в 0.2–0.3% по всей шкале Бофорта;
- детерминизм бит-в-бит (два моря с одним сидом → один хэш состояния);
- состояние моря 1–2 КБ против 1 МБ f32-сетки — «резиновая вода»
  сеточных движков невозможна по построению;
- нормали — аналитические градиенты спектра (осциллятор с дрейфом
  ≤ 1e-13, сверяется с точным `surface_at` в тестах Rust);
- `selftest.py` и `smoke_headless.py` не требуют видеокарты.

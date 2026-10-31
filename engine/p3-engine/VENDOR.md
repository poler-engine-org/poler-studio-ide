# P³ Engine — vendored в POLER ENGINE

**Цикл W «Симбиоз» (v0.59.0).** Проективное 3D-ядро Kotokvit вендорено
в репозиторий POLER ENGINE — «+50 тысяч строк отсюда» (задание владельца):
единое дерево, единая сборка, единые тесты.

## Откуда

| | |
|---|---|
| Апстрим | https://github.com/Kotokvit/P3_Engine |
| Коммит | `641962af343dca73fbcc93ce082b16914973edda` |
| Язык | Zig 0.14.0 (59 модулей ядра + расчёты) |
| Лицензия | как у апстрима (см. README.md) |

## Что вендорено (3.1 МБ)

- `src/*.zig` — **ядро P³**: p3_kernel (HomVec4, PGL4, FS-метрика),
  p3_idempotent, p3_geodesic, p3_algebra (Грассман/Плюккер), p3_pga,
  p3_physics (кривизна, Christoffel, Ricci), p3_ecs, p3_scene, p3_renderer,
  p3_rhi, p3_character_physics, p3_vehicle_physics, p3_skeletal,
  p3_null_fluid, p3_vision, p3_cordic, p3_crossratio, p3_dual_quat, …
- `src/p3_ffi.zig` — **C-ABI мост к POLER ENGINE** (восстановлен в цикле W
  после потери песочницы; семантика снята зондом со старой libp3ffi.so
  и зеркалирует `poler-engine/src/p3/native.rs`)
- `build.zig` — минимальный сборщик интеграции (апстримовский лончер-O3DE
  остался в апстриме)
- `calculations/` — Z3/Pacejka/PGA-Clifford верификации математики
- `docs/`, `examples/`, `tools/`, `include/p3_bridge.h`, README/ROADMAP

## Что НЕ вендорено (намеренно)

- `launcher/`, `launcher-src/` — 35 МБ исходников O3DE Editor/AssetProcessor
  (справочный материал анализа слабостей O3DE, не движок; живёт в апстриме)

## Сборка C-ABI библиотеки

```bash
./ffi/build.sh          # → ffi/libp3ffi-linux-x86_64.so
```

**Цель сборки — БАЗОВЫЙ x86-64 (SSE2), `cpu_model = .baseline`.**
Никакого AVX2: старая .so (собранная под native-CPU песочницы с AVX2)
падала SIGILL на Ivy Bridge (i7-3770). `ffi/build.sh` проверяет
отсутствие ymm-инструкций objdump'ом и падает, если они появились.

## Конформанс

```
poler-engine --exec "p3 conformance --pairs 256"
cargo test --lib p3::
```

Rust-близнец (`src/p3/native.rs`) реализует ту же математику; тест
`ffi_vs_native_render_agreement` сверяет растеризаторы попиксельно.
Если .so не загрузилась — `render_frame_auto` честно откатывается
на native-путь: движок рендерит ВСЕГДА.

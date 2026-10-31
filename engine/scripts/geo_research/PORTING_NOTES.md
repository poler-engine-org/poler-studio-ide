# Порт-карта Geo/GIS → тритное ядро poler-engine (Сессия-11)

Исходники четырёх движков лежат рядом в .poler-архивах (регенерация:
`bash scripts/geo_research/fetch_geo_sources.sh`, ~30 с, диск: только сжатые
.poler, распаковки НЕТ). Все якоря ниже найдены grep'ом ДВИЖКА по архивам
без распаковки — «ансамбль» v0.65.0 в действии:

    poler-engine --archives --grep "…" --grep-list scripts/geo_research/

## Что где лежит (файл::запись)

| Крейт | .poler | файлов | сырой объём | ключ для тритного ядра |
|---|---|---|---|---|
| georust/geo | geo.poler | 563 | 22 МиБ | DE-9IM, предикаты, буферы |
| georust/rstar | rstar.poler | 41 | 0.5 МиБ | R-tree, bulk-load STR |
| stoeoef/spade | spade.poler | 119 | 50 МиБ | Делоне, TIN высот |
| uber/h3 (C) | h3.poler | 499 | 49 МиБ | гекс-сетка DGGS, кольца |

## Якоря для移植 (найдены движком)

### 1. DE-9IM — топологическая матрица 3×3 → ОДИН 6-тритный блок
- `geo.poler::geo-main/geo/src/algorithm/relate/mod.rs:70` —
  `fn relate(&self, other) -> IntersectionMatrix` =
  `RelateOperation::new(self, other).compute_intersection_matrix()`;
- Interior/Boundary/Exterior ∈ {-1, 0, +1} — РОВНО сбалансированные триты;
  9 ячеек матрицы = 9 тритов (6-тритный блок + резерв 3 трита под флаги).
- Подготовленная геометрия (индекс): `algorithm/indexed/prepared_geometry.rs`.

### 2. R-tree (rstar) — пространственный индекс O(log N)
- `rstar.poler::rstar-master/rstar/src/algorithm/bulk_load/bulk_load_sequential.rs`
  — STR-упаковка (Sort-Tile-Recursive): сортировка по X, нарезка плиток по Y;
- деления узла: `algorithm/mod.rs` ( Greene/Quadtree/R-стар split-эвристики);
- В ТРИТНОМ ЯДРЕ: bbox = трит-диапазоны {-1,0,+1}×масштаб 3⁻ᵏ; знаковый
  компаратор вместо f64-сравнений; 27-Tree (3×3×3) для XYZ.

### 3. H3 — равновеликая гекс-сетка (икосаэдрический DGGS)
- `h3.poler::h3-master/src/lib/h3Index.c` — битовая упаковка индекса ячейки;
- кольца/диски: `website/docs/library/migration-3.x/functions.md` (hexRing,
  kRing, polygonToCells);
- В ТРИТНОМ ЯДРЕ: икосаэдр 20 граней → база 3; гекс-кольца = квантованные
  трит-спирали; один 6-тритный блок держит 729 состояний (байт+473 резерва).

### 4. Spade — триангуляция Делоне для рельефа (TIN)
- `spade.poler::spade-master/src/delaunay_core.rs` — ухо/окружность (in_circle);
- высотная карта Этерии = TIN → изолинии через viz_field (marching squares
  Сессии-9 уже в ядре!), объёмные изоповерхности = marching cubes (кандидат).

## Порядок портирования (предложение Сессии-11)

1. **geo-types ядро**: Point/Line/Polygon на трит-координатах (tₓ,t_y,t_z);
2. **DE-9IM в тритах**: IntersectionMatrix → 6-тритный блок (прямой мост
   «трит = кутрит» — спин-1 проекции {-1,0,+1});
3. **27-Tree индекс**: вместо октодерева — трит-деление XYZ;
4. **H3-гексагоны**: икосаэдр + трит-кольца → карта биомов Этерии;
5. **Рендер**: viz_field/viz_surf (Сессии-8/9/10) уже рисуют поля и 3D —
   рельеф ложится прямо в существующие сцены.

## Конвейер (уже работает, эта сессия)

    поток:  codeload.tar.gz ──(stream-download: gzip-декодер НА ЛЕТУ)──▶ .poler
                                                                        │
    сборка: poler_build.sh ──(cargo build -j1 → strip → --pack)──────▶▶ builds/*.poler
                                                                        │
    коробка: poler-box box_demo.poler --box-entry poler-engine          ▼
             движок ИЗ АРХИВА в изолированной коробке грепает geo.poler
             (userns+pivot_root+seccomp; хост невидим; сеть отрезана)

Диск: 123 МиБ исходников + 58 МиБ сборки живут как 55+17 МиБ .poler (−72%),
сырых tar.gz и стадингов на диске НЕТ.

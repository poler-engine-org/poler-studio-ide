#!/usr/bin/env bash
# СЕССИЯ 9 — СУВЕРЕННЫЙ РЕНДЕР: чистая математика рендеринга,
# вырезанная из стандартных библиотек, живёт в самом движке.
#
# «От перестановки слагаемых сумма не меняется»: зачем тащить
# NetworkX/Graphviz/Matplotlib с миллионом зависимостей, если их
# математика — это обычные f64-операции? Перенесено в монолит:
#   • viz_graph — Fruchterman–Reingold (силовая укладка графов);
#   • viz_field — marching squares (изолинии контуров);
#   • viz_surf  — поворот + ортоскопия + painter's-алгоритм (3D).
#
# Суверенитет вычислений: awk ниже НЕ считает поле — он только
# печатает ЛИТЕРАЛЫ ФОРМУЛ (cos(...)·exp(...)); все значения,
# изолинии, проекции и SVG-код порождает сам poler-engine.
#
# ГРАБЛЯ 22 (закрыта здесь): -3^2 = -9, а не 9 — унарный минус
# слабее степени. Все отрицательные координаты в генерируемых
# выражениях обязаны быть в скобках: (-3.0000)^2.
#
# Проверяется сценами (маркеры обязаны найтись в SVG):
#   A) fly_k4_graph    — коннектом мухи (веса сессии-4) → укладка FR
#   B) crystal_graph   — семантический кристалл памяти с метками
#   C) sombrero_field  — мексиканская шляпа (1−r²)e^(−r²/2): кольца
#   D) sombrero_surf   — она же в 3D: painter's-алгоритм
#   E) slit_field      — двущелевая интерференция Born
#   F) slit_surf       — она же в 3D
set -u
ENG="${ENG:-./target/debug/poler-engine}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ART="$ROOT/scripts/viz_artifacts"
OUT="${OUT:-/home/z/my-project/download/experiments/viz_session9_v063.txt}"
mkdir -p "$ART" "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ СЕССИЯ 9: СУВЕРЕННЫЙ РЕНДЕР — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
echo "Движок: $("$ENG" --exec 'version' < /dev/null 2>&1 | head -1)"
echo "Артефакты SVG: $ART"
echo ""

# Сцена: имя → файл, выражение → Калькулятор. Кавычки Display
# (Value::Str печатается как "…") срезаются по первому/последнему байту.
scene() {
  local name="$1" expr="$2"
  "$ENG" --exec "calc $expr" < /dev/null 2>&1 \
    | sed -e '1s/^"//' -e '$s/"$//' > "$ART/$name.svg"
  echo "── $name.svg  ←  calc ${expr:0:64}…"
}

# Проверка маркеров: все обязаны найтись (rg -F — только литералы,
# грабля 17: без -F маркер «+48%» роняет проверку).
check() {
  local name="$1"; shift
  local ok=1
  for m in "$@"; do
    if ! rg -q -F "$m" "$ART/$name.svg"; then
      echo "   ❌ $name: нет маркера «$m»"
      ok=0
    fi
  done
  if [ "$ok" = 1 ]; then
    echo "   ✅ $name: маркеры OK ($*)"
  fi
  [ "$ok" = 1 ]
}

# ── A) Коннектом мухи: K4-ядро, живые веса сессии-4 ──────────────────
scene fly_k4_graph 'viz_graph([0,735,24,0,1496,1222; 735,0,251,231,206,194; 24,251,0,0,40,28; 0,231,0,0,0,0; 1496,206,40,0,0,8; 1222,194,28,0,8,0])'

# ── B) Семантический кристалл памяти: co-occurrence токенов ─────────
scene crystal_graph 'viz_graph([0,5,4,2,1,3; 5,0,3,1,0,2; 4,3,0,4,2,1; 2,1,4,0,5,2; 1,0,2,5,0,4; 3,2,1,2,4,0], "and,to,in,system,head,memory")'

# ── C/D) Сомбреро (мексиканская шляпа): z = (1−r²)·e^(−r²/2) ────────
# awk печатает только ЛИТЕРАЛЫ ФОРМУЛ — считает движок.
SOMBRERO=$(awk 'BEGIN {
  n = 13
  for (i = 0; i < n; i++) {
    y = -3 + 6*i/(n-1)
    row = ""
    for (j = 0; j < n; j++) {
      x = -3 + 6*j/(n-1)
      e = sprintf("((1-((%.4f)^2+(%.4f)^2))*exp(-((%.4f)^2+(%.4f)^2)/2))", x, y, x, y)
      row = row (j ? "," : "") e
    }
    printf "%s%s", (i ? ";" : ""), row
  }
}')
scene sombrero_field "viz_field([$SOMBRERO])"
scene sombrero_surf  "viz_surf([$SOMBRERO])"

# ── E/F) Двущелевая интерференция: z = cos²(πx/2)·e^(−3y²) ──────────
SLIT=$(awk 'BEGIN {
  rows = 9; cols = 13
  for (i = 0; i < rows; i++) {
    y = -1 + 2*i/(rows-1)
    row = ""
    for (j = 0; j < cols; j++) {
      x = -3 + 6*j/(cols-1)
      e = sprintf("cos(%.6f*pi/2)^2*exp(-((%.6f)^2)*3)", x, y)
      row = row (j ? "," : "") e
    }
    printf "%s%s", (i ? ";" : ""), row
  }
}')
scene slit_field "viz_field([$SLIT])"
scene slit_surf  "viz_surf([$SLIT])"

echo ""
echo "═══ ВЕРИФИКАЦИЯ МАРКЕРОВ ═══"
fail=0
check fly_k4_graph    'N = 6' 'рёбер = 11' 'компонент = 1' 'Fruchterman' 'плотность = 73.3' || fail=1
check crystal_graph   'and' 'system' 'memory' 'компонент = 1' 'рёбер = 14' || fail=1
check sombrero_field  'изолинии: 7 уровней' 'сетка 13×13' 'polyline' || fail=1
check sombrero_surf   'квадов 144' 'азимут -60' 'painter' || fail=1
check slit_field      'сетка 9×13' 'изолинии: 7 уровней' 'polyline' || fail=1
check slit_surf       'квадов 96' 'painter' 'ортоскопия' || fail=1

# Детерминизм: дважды посчитанная сцена бит-в-бит одинакова
# (золотая спираль вместо RNG — грабля 19).
echo ""
echo "═══ ДЕТЕРМИНИЗМ УКЛАДКИ ═══"
"$ENG" --exec "calc viz_surf([$SOMBRERO])" < /dev/null 2>&1 \
  | sed -e '1s/^"//' -e '$s/"$//' > /tmp/surf_repeat.svg
if cmp -s "$ART/sombrero_surf.svg" /tmp/surf_repeat.svg; then
  echo "   ✅ sombrero_surf: повтор бит-в-бит идентичен (md5 $(md5sum < "$ART/sombrero_surf.svg" | cut -d' ' -f1))"
else
  echo "   ❌ sombrero_surf: повтор отличается — недетерминизм!"
  fail=1
fi

# SVG-гигиена: каждый файл начинается с <svg и кончается </svg>.
echo ""
echo "═══ SVG-ГИГИЕНА ═══"
for f in "$ART"/*.svg; do
  if head -1 "$f" | rg -q '<svg' && tail -1 "$f" | rg -q '</svg>'; then
    echo "   ✅ $(basename "$f"): каркас SVG корректен ($(wc -c < "$f") байт)"
  else
    echo "   ❌ $(basename "$f"): каркас SVG сломан"
    fail=1
  fi
done

echo ""
if [ "$fail" = 0 ]; then
  echo "═══ СЕССИЯ 9: СУВЕРЕННЫЙ РЕНДЕР — ВСЕ ПУТИ ЗЕЛЁНЫЕ ✅ ═══"
  echo "Graphviz/NetworkX/Matplotlib больше не нужны: укладка графов,"
  echo "изолинии и 3D-проекции считает и рисует сам бинарник движка."
  exit 0
else
  echo "═══ СЕССИЯ 9: ЕСТЬ КРАСНЫЕ ПУТИ ❌ ═══"
  exit 1
fi

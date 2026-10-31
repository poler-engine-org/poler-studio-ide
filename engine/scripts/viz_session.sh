#!/usr/bin/env bash
# СЕССИЯ 8 — SEMANTIC VISUALIZER: мост «машина → человек» на Калькуляторе.
#
# Идея из полевого диалога (Текстовий файл (2).txt): Калькулятор выдаёт
# точные значения (1.616255e-35, I₃ = 2.914854), но для человека это «ни
# о чём». Нужен автоматический слой: тип значения → правило → сцена SVG
# с человеческими якорями. Реализован как семейство viz-функций
# (src/calc/viz.rs, цикл U): viz / viz_bell / viz_scale / viz_prob /
# viz_bars / viz_matrix — чистый SVG 1.1, ноль внешних зависимостей.
#
# ГРАБЛЯ 16 (закрыта здесь): литерал [a, b, c] в языке Калькулятора —
# матрица-строка, а не Value::List; viz-слой принимает 1×N/N×1 как
# список чисел: viz([0.8, 0.2]) → Born-бары, viz([[1,2],[3,4]]) → карта.
#
# Проверяется сценами (все маркеры обязаны найтись в SVG):
#   A) viz_bell(2.914854216)   — радар Ацина d=3: Цирельсон, η* = 0.6861
#   B) viz_bell(2.972698267102243) — квкварт d=4: ранг «квкварты»
#   C) viz_scale(1.616255e-35) — Планковская длина, «меньше человека»
#   D) viz_prob([0.8083, 0.0796, 0.1121]) — Born: Σ = 1, 80.83%
#   E) viz_matrix(kron(pauli_x(), pauli_z())) — теплокарта 4×4
#   F) viz(1.616255e-35)       — авто-маршрут: скаляр → шкала Вселенной
set -u
ENG="${ENG:-./target/debug/poler-engine}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ART="$ROOT/scripts/viz_artifacts"
OUT="${OUT:-/home/z/my-project/download/experiments/viz_session_v063.txt}"
mkdir -p "$ART" "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ СЕССИЯ 8: SEMANTIC VISUALIZER — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
echo "Движок: $("$ENG" --exec 'version' < /dev/null 2>&1 | head -1)"
echo "Артефакты SVG: $ART"
echo ""

# Сцена: имя → файл, выражение → Калькулятор. Кавычки Display
# (Value::Str печатается как "…") срезаются по первому/последнему байту.
scene() {
  local name="$1" expr="$2"
  "$ENG" --exec "calc $expr" < /dev/null 2>&1 \
    | sed -e '1s/^"//' -e '$s/"$//' > "$ART/$name.svg"
  echo "── $name.svg  ←  calc $expr"
}

# Проверка маркеров: все обязаны найтись.
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

scene acin_bell        'viz_bell(2.914854216)'
scene ququart_bell     'viz_bell(2.972698267102243)'
scene planck_scale     'viz_scale(1.616255e-35)'
scene born_probs       'viz_prob([0.8083, 0.0796, 0.1121])'
scene bell_heat        'viz_matrix(kron(pauli_x(), pauli_z()))'
scene auto_scene       'viz(1.616255e-35)'

echo ""
echo "═══ ВЕРИФИКАЦИЯ МАРКЕРОВ ═══"
fail=0
check acin_bell    'CGLMP' 'Цирельсон' 'Ацин' '0.6861'       || fail=1
check ququart_bell 'квкварт' '+48.63%' '0.6728'       || fail=1
check planck_scale 'Планковская длина' 'меньше человека' '1.052e35' || fail=1
check born_probs   'Born' '80.83%' 'Σ = 1'             || fail=1
check bell_heat    'Матрица 4×4' 'Re(след) = 0'        || fail=1
check auto_scene   'Шкала Вселенной' 'Планков'         || fail=1

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
  echo "═══ СЕССИЯ 8: SEMANTIC VISUALIZER — ВСЕ ПУТИ ЗЕЛЁНЫЕ ✅ ═══"
  echo "Машина сказала 1.616255e-35 — человек увидел «Планковская длина,"
  echo "в 1.052e35 раз меньше человека». Мост работает."
  exit 0
else
  echo "═══ СЕССИЯ 8: ЕСТЬ КРАСНЫЕ ПУТИ ❌ ═══"
  exit 1
fi

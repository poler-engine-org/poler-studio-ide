#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Сессия-11 «Ансамбль»: загрузка исходников Geo/GIS-движков ПРЯМО В .poler
# (v0.65.0 stream-download с распаковкой на лету: tar.gz → пофайловая
#  таблица + zstd — сырой tar.gz на диск НЕ пишется).
#
# Репозитории-доноры (чистые алгоритмы для移植 в тритное ядро poler-engine):
#   geo    — геометрия: DE-9IM предикаты, буферы, выпуклые оболочки (georust)
#   rstar  — R-tree: пространственный индекс O(log N) (georust, master)
#   spade  — триангуляция Делоне, TIN-высоты (stoeoef, master)
#   h3     — гексагональная дискретная глобальная сетка DGGS (uber, C)
#
# Повторный запуск безопасен: существующие .poler не трогаются
# (перезагрузка только с FORCE=1).
#
# Проверка после загрузки (grep БЕЗ распаковки):
#   poler-engine --archives --grep "trait Relate" --grep-list scripts/geo_research/
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail
cd "$(dirname "$0")"
ENGINE="${POLER_ENGINE:-$HOME/.local/bin/poler-engine}"
[ -x "$ENGINE" ] || ENGINE="./target/debug/poler-engine"
[ -x "$ENGINE" ] || { echo "poler-engine не найден (POLER_ENGINE=?)"; exit 2; }
FORCE="${FORCE:-0}"

fetch() { # fetch <имя> <url>
    local name="$1" url="$2" out="$1.poler"
    if [ -s "$out" ] && [ "$FORCE" != "1" ]; then
        echo "OK (есть): $out ($(stat -c%s "$out") Б) — FORCE=1 для перезагрузки"
        return 0
    fi
    echo "→ $name: $url"
    "$ENGINE" --stream-download "$url" --output-archive "$out" 2>&1 \
        | rg '"total_raw"|"total_stored"|"ratio"|"files"|"tar_mode"|"elapsed_ms"' || true
    echo "  $out: $(stat -c%s "$out") Б на диске"
}

fetch geo   "https://codeload.github.com/georust/geo/tar.gz/refs/heads/main"
fetch rstar "https://codeload.github.com/georust/rstar/tar.gz/refs/heads/master"
fetch spade "https://codeload.github.com/stoeoef/spade/tar.gz/refs/heads/master"
fetch h3    "https://codeload.github.com/uber/h3/tar.gz/refs/heads/master"

echo
echo "── Сводка (экономия диска: только .poler, никакой распаковки) ──"
ls -la ./*.poler
echo
echo "── grep внутри .poler без распаковки (анкерный smoke) ──"
"$ENGINE" --archives --grep "trait Relate" --grep-list . 2>/dev/null | head -3 || true
"$ENGINE" --archives --grep "insert" --grep-list ./rstar.poler 2>/dev/null | head -3 || true
"$ENGINE" --archives --grep "hexRing\|h3Index" --grep-regex --grep-list ./h3.poler 2>/dev/null | head -3 || true
echo "ГОТОВО: исходники в .poler, готовы к исследованиям Сессии-11 (Geo/GIS на тритах)"

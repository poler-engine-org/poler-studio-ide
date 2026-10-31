#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Сессия-11 «Ансамбль»: ЦИКЛИЧЕСКОЕ ДЕМО poler-box
#
#   poler-engine упаковывает СЕБЯ + geo-исходники в box_demo.poler
#   → запускается ИЗНУТРИ изолированной коробки (userns/mntns/pidns/netns,
#     pivot_root, seccomp, губернатор RSS/CPU)
#   → ВНУТРИ КОРОБКИ движок грепает geo.poler (--archives) —
#     хост невидим, сеть отрезана, исходники не распаковываются.
#
# Это и есть «всё соединить в ансамбль»:
#   движок → .poler → коробка → движок → .poler → grep
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail
cd "$(dirname "$0")/../.."
ENGINE="${POLER_ENGINE:-$HOME/.local/bin/poler-engine}"
[ -x "$ENGINE" ] || ENGINE="./target/debug/poler-engine"
[ -x "$ENGINE" ] || { echo "poler-engine не найден"; exit 2; }

STAGE="$(mktemp -d /tmp/poler-box-demo.XXXXXX)"
OUT=scripts/geo_research/box_demo.poler
rm -f "$OUT"

# 1. Стажнём движок (stripped) + свежескачанные geo-исходники
#    (вне репо: WYSIWYG-pack без .gitignore-фильтров, см. грабля №24)
cp "$ENGINE" "$STAGE/poler-engine"
chmod 755 "$STAGE/poler-engine"
cp scripts/geo_research/geo.poler "$STAGE/geo.poler"

#    + рантайм glibc (payload динамический: rootfs-коробка — tmpfs без ОС,
#    PT_INTERP=/lib64/ld-linux-x86-64.so.2 ищет либы по мультиарх-путям)
mkdir -p "$STAGE/lib64" "$STAGE/usr/lib" "$STAGE/lib/x86_64-linux-gnu" "$STAGE/lib"
cp -L /lib64/ld-linux-x86-64.so.2 "$STAGE/lib64/" 2>/dev/null || true
for lib in libgcc_s.so.1 libm.so.6 libc.so.6; do
    for dir in /usr/lib /lib64 /lib /lib/x86_64-linux-gnu; do
        if [ -f "$dir/$lib" ]; then
            cp -L "$dir/$lib" "$STAGE/usr/lib/" 2>/dev/null || true
            cp -L "$dir/$lib" "$STAGE/lib64/" 2>/dev/null || true
            cp -L "$dir/$lib" "$STAGE/lib/" 2>/dev/null || true
            break
        fi
    done
done

# 2. ПОСТОЯННЫЙ АРХИВАТОР: каталог → .poler (пофайловая таблица, права 755)
echo "── pack: движок + geo.poler → $OUT ──"
"$ENGINE" --pack "$STAGE" --output-archive "$OUT" 2>&1 \
    | rg '"total_raw"|"total_stored"|"ratio"|"files"|"tar_mode"' || true
rm -rf "$STAGE"

# 3. Циклический запуск: движок из .poler В КОРОБКЕ грепает geo.poler
#    (значения --box-arg с ведущими дефисами — только через =синтаксис)
echo
echo "── poler-box: движок из $OUT внутри коробки ──"
"$ENGINE" --poler-box "$OUT" \
    --box-entry poler-engine \
    --box-map ":/" \
    --box-rss-mb 256 --box-cpu-s 60 \
    --box-arg=--archives \
    --box-arg=--grep \
    --box-arg="trait Relate" \
    --box-arg=--grep-list \
    --box-arg=/geo.poler
echo "EXIT_BOX=$?"

# 4. Внутри коробки — версия движка (доказательство цикла)
echo
echo "── poler-box: --version изнутри (движок = запись архива) ──"
"$ENGINE" --poler-box "$OUT" \
    --box-entry poler-engine \
    --box-map ":/" \
    --box-rss-mb 256 --box-cpu-s 30 \
    --box-arg=--version

echo
echo "box_demo.poler: $(stat -c%s "$OUT") Б (движок + 22 МиБ geo-исходников, сжато)"

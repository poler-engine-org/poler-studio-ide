#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# ПОСТОЯННЫЙ АРХИВАТОР СБОРКИ poler-engine (v0.65.0, Сессия-11 «Ансамбль»)
#
# Правило «на постоянной основе»: ВСЁ, что выходит из cargo build, немедленно
# упаковывается движком в builds/poler-engine-<ver>.poler
# (FastCDC + BLAKE3-дедуп + zstd-15, пофайловая таблица, права 0755).
# Сырой стадинг удаляется — на диске остаётся ТОЛЬКО .poler-архив.
#
# Бонус экономии: зачистка устаревших артефактов крейта в target/debug/deps
# (тест-бинарники по 330 МиБ от прошлых прогонов — главная течь диска).
#
# Использование:
#   scripts/poler_build.sh              # сборка + упаковка + верификация
#   scripts/poler_build.sh --no-build   # упаковка уже собранного
#   scripts/poler_build.sh --deep-clean # + вычистка устаревшего musора deps
#
# Проверка архива без распаковки:
#   poler-engine --archive-list builds/poler-engine-<ver>.poler
#   poler-engine --archives --grep "POLERARC" --grep-list builds/
#   poler-engine --poler-box builds/poler-engine-<ver>.poler --box-entry poler-engine \
#       --box-map ":/" --box-arg=--version   (нужны lib/ в архиве: --with-libs)
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

NO_BUILD=0; DEEP_CLEAN=0; WITH_LIBS=0
for a in "$@"; do
    case "$a" in
        --no-build)   NO_BUILD=1 ;;
        --deep-clean) DEEP_CLEAN=1 ;;
        --with-libs)  WITH_LIBS=1 ;;
        *) echo "неизвестный флаг: $a"; exit 2 ;;
    esac
done

VER="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[""]/,""); print $3; exit}' Cargo.toml)"
[ -n "$VER" ] || VER="$(./target/debug/poler-engine --version 2>/dev/null | awk '{print $2}')"
[ -n "$VER" ] || { echo "не удалось определить версию"; exit 2; }
OUT="builds/poler-engine-${VER}.poler"
mkdir -p builds
STAGE="$(mktemp -d /tmp/poler-build-stage.XXXXXX)"
trap 'rm -rf "$STAGE"' EXIT

echo "═══ ПОСТОЯННЫЙ АРХИВАТОР СБОРКИ · poler-engine v${VER} ═══"

# 1. Сборка (дисциплина: -j1, без инкрементала — диск дорог)
if [ "$NO_BUILD" != "1" ]; then
    echo "── cargo build -j1 ──"
    CARGO_INCREMENTAL=0 cargo build -j1 2>&1 | tail -1
fi
[ -f target/debug/poler-engine ] || { echo "нет target/debug/poler-engine"; exit 2; }

# 2. Стадинг: бинарник (stripped, 0755) + viz-артефакты + ключевые доки
echo "── стадинг артефактов ──"
mkdir -p "$STAGE/bin" "$STAGE/viz" "$STAGE/docs"
cp target/debug/poler-engine "$STAGE/bin/poler-engine"
strip "$STAGE/bin/poler-engine"
chmod 755 "$STAGE/bin/poler-engine"
cp scripts/viz_artifacts/*.svg "$STAGE/viz/" 2>/dev/null || echo "  (viz-артефактов нет)"
for d in docs/UNDOCUMENTED.md README.md; do
    [ -f "$d" ] && cp "$d" "$STAGE/docs/"
done

# Рантайм для poler-box (опция --with-libs: glibc в rootfs коробки)
if [ "$WITH_LIBS" = "1" ]; then
    mkdir -p "$STAGE/lib64" "$STAGE/lib/x86_64-linux-gnu"
    cp -L /lib64/ld-linux-x86-64.so.2 "$STAGE/lib64/" 2>/dev/null || true
    for lib in libgcc_s.so.1 libm.so.6 libc.so.6; do
        cp -L "/lib/x86_64-linux-gnu/$lib" "$STAGE/lib/x86_64-linux-gnu/" 2>/dev/null || true
    done
fi

# 3. УПАКОВКА В АРХИВАТОР (сам движок пакует собственную сборку — цикл)
echo "── pack → $OUT ──"
RAW_BEFORE="$(du -sb "$STAGE" | cut -f1)"
./target/debug/poler-engine --pack "$STAGE" --output-archive "$OUT" 2>&1 \
    | rg '"total_raw"|"total_stored"|"ratio"|"files"|"tar_mode"|"elapsed_ms"|"peak_rss_kb"' || true

# 4. Верификация БЕЗ распаковки (листинг — через файл: SIGPIPE-дисциплина)
echo "── верификация (grep внутри архива) ──"
LIST_FILE="$(mktemp)"
./target/debug/poler-engine --archive-list "$OUT" > "$LIST_FILE" 2>/dev/null || true
head -6 "$LIST_FILE"; rm -f "$LIST_FILE"
COUNT_FILE="$(mktemp)"
./target/debug/poler-engine --archives --grep "POLERARC" --grep-count "$OUT" > "$COUNT_FILE" 2>/dev/null || true
MATCHES="$(rg -c ':\d+' "$COUNT_FILE" || true)"; rm -f "$COUNT_FILE"
echo "  grep-верификация: ${MATCHES:-0} записей отвечают"

# 5. SHA-256 паспорт сборки
sha256sum "$OUT" | tee "$OUT.sha256"

# 6. Экономия диска: сырое стадинг-дерево удаляем (trap), плюс опционально
#    вычищаем устаревшие артефакты крейта (тест-бинарники ~330 МиБ/шт)
if [ "$DEEP_CLEAN" = "1" ]; then
    echo "── deep-clean устаревших артефактов крейта ──"
    STALE_DIRS="$(ls -d target/debug/deps/poler_engine-* 2>/dev/null | rg -v "$(basename target/debug/poler-engine)" || true)"
    if [ -n "$STALE_DIRS" ]; then
        echo "$STALE_DIRS" | while read -r d; do echo "  rm: $d"; rm -rf "$d"; done
    fi
    STALE_RLIBS="$(ls target/debug/deps/libpoler_engine-*.rlib 2>/dev/null | head -n -1 || true)"
    if [ -n "$STALE_RLIBS" ]; then
        echo "$STALE_RLIBS" | while read -r f; do rm -f "$f" "${f%.rlib}.rmeta"; done
    fi
fi

STORED="$(stat -c%s "$OUT")"
echo
echo "═══ ИТОГ ═══"
echo "  сборка:     ${VER} (cargo build -j1)"
echo "  архив:      $OUT"
echo "  сырое:      $(numfmt --to=iec "$RAW_BEFORE")  →  сжато: $(numfmt --to=iec "$STORED")"
echo "  экономия:   $(( (100 - STORED * 100 / RAW_BEFORE) ))% диска"
echo "  на диске остаётся ТОЛЬКО .poler — стадинг удалён"

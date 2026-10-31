#!/usr/bin/env bash
# build_quantum_poler.sh — упаковка POLER Quantum PC в .poler-контейнер
# («библиотеки прямо в архиватор», цикл G строка 1).
#
# Результат: quantum.poler — самоизоляционный контейнер (userns/mnt/pid/net/ipc/uts
# + pivot_root + seccomp) с pqc и минимальным glibc-рантаймом внутри.
#
# Запуск:  bash tools/boxdemo/build_quantum_poler.sh [out.poler]
# Пример:  poler-engine --poler-box quantum.poler --box-entry=rootfs/pqc \
#              --box-arg=algo --box-arg=period --box-arg=--n --box-arg=6 \
#              --box-arg=--period --box-arg=6 --box-arg=--shots --box-arg=1024
set -euo pipefail
OUT="${1:-quantum.poler}"
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PQC="$REPO_ROOT/target/release/pqc"
ENGINE="$REPO_ROOT/target/release/poler-engine"
[ -x "$PQC" ] || { echo "нет $PQC — соберите: cargo build --release -p pqc" >&2; exit 1; }
[ -x "$ENGINE" ] || { echo "нет $ENGINE — соберите: cargo build --release" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/rootfs/lib/x86_64-linux-gnu" "$WORK/rootfs/lib64"
install -m 0755 "$PQC" "$WORK/rootfs/pqc"
cp -L /lib/x86_64-linux-gnu/libgcc_s.so.1 /lib/x86_64-linux-gnu/libm.so.6 \
      /lib/x86_64-linux-gnu/libc.so.6 "$WORK/rootfs/lib/x86_64-linux-gnu/" 2>/dev/null ||
cp -L /usr/lib/x86_64-linux-gnu/libgcc_s.so.1 /usr/lib/x86_64-linux-gnu/libm.so.6 \
      /usr/lib/x86_64-linux-gnu/libc.so.6 "$WORK/rootfs/lib/x86_64-linux-gnu/"
cp -L /lib64/ld-linux-x86-64.so.2 "$WORK/rootfs/lib64/" 2>/dev/null ||
cp -L /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 "$WORK/rootfs/lib64/"

tar -C "$WORK" -cf "$WORK/rootfs.tar" rootfs
"$ENGINE" --stream-file "$WORK/rootfs.tar" --output-archive "$OUT"
echo "готово: $OUT ($(du -h "$OUT" | cut -f1))"

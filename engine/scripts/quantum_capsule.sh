#!/usr/bin/env bash
# QUANTUM CAPSULE v2 — суверенная квантовая лаборатория в одном .poler:
# движок + коннектом мухи (CSR 6.8 МБ) + QAOA-солвер + SUM-гейт + QuantumMind.
# Модульная комбинация: архиватор (.poler, FastCDC+Zstd+BLAKE3) × poler-box
# (6 namespaces + seccomp, rootfs в RAM) × quantum × connectome × calc.
# Журнал: download/experiments/quantum_capsule_v063.txt
set -u
ENG="$HOME/.local/bin/poler-engine"
REPO="/home/z/my-project/poler-engine-src"
WORK="/tmp/quantum_capsule"
OUT="/home/z/my-project/download/experiments/quantum_capsule_v063.txt"
SESSION="/home/z/my-project/scripts/quantum_capsule.session"
CAPSULE="/home/z/my-project/download/experiments/quantum_capsule.poler"

mkdir -p "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ QUANTUM CAPSULE v2 — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
rm -rf "$WORK"; mkdir -p "$WORK/rootfs/bin" "$WORK/rootfs/lib/x86_64-linux-gnu" \
                 "$WORK/rootfs/lib64" "$WORK/rootfs/lab" "$WORK/rootfs/data"
cd "$WORK"

echo "── 1. rootfs: движок + libs + сессия + МОЗГ МУХИ (CSR) + квантовые заметки"
cp "$ENG" rootfs/bin/poler-engine
cp /lib/x86_64-linux-gnu/libgcc_s.so.1  rootfs/lib/x86_64-linux-gnu/
cp /lib/x86_64-linux-gnu/libm.so.6      rootfs/lib/x86_64-linux-gnu/
cp /lib/x86_64-linux-gnu/libc.so.6      rootfs/lib/x86_64-linux-gnu/
cp /lib64/ld-linux-x86-64.so.2          rootfs/lib64/
cp /bin/dash rootfs/bin/sh 2>/dev/null || cp /bin/bash rootfs/bin/sh
cp "$SESSION" rootfs/lab/quantum_capsule.session
# данные: коннектом FlyWire v783 core (6.8 МБ, 138 639 нейронов) + полевой журнал
cp "$REPO/docs/flywire-connectome/flywire_v783_core.csr.zst" rootfs/data/
cp "$REPO/docs/flywire-connectome/flywire_v783_nodes.bin"     rootfs/data/
cp "$REPO/docs/UNDOCUMENTED.md"                               rootfs/data/
cat > rootfs/lab/MANIFEST.txt <<'EOF'
QUANTUM CAPSULE v2 — квантовая лаборатория + мозг мухи в изоляции.
Состав: poler-engine v0.62 + FlyWire v783 core (2.7M синапсов) +
QAOA MaxCut на живом мотиве + SUM-гейт кутритов + QuantumMind Born-сэмплер.
Запуск: poler-engine --poler-box < capsule.poler --box-entry rootfs/bin/capsule_launch
EOF
# лончер: СТАТИЧЕСКИЙ бинарник (memfd-entry не умеет shebang без /proc)
cat > capsule_launch.c <<'EOF'
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
int main(void) {
    int fd = open("/lab/quantum_capsule.session", O_RDONLY);
    if (fd < 0) return 3;
    if (dup2(fd, 0) < 0) return 5;
    close(fd);
    putenv("PATH=/bin");
    char *argv[] = {(char*)"/bin/poler-engine", (char*)"--shell", 0};
    execv("/bin/poler-engine", argv);
    return 4;
}
EOF
gcc -static -O2 -o rootfs/bin/capsule_launch capsule_launch.c || { echo "FAIL: gcc -static"; exit 1; }
# /dev/null пустым файлом: Command::output() открывает его для stdin дочерних
mkdir -p rootfs/dev
: > rootfs/dev/null
du -sh rootfs | sed 's/^/  rootfs: /'

echo "── 2. упаковка: tar.gz (префикс rootfs/) → .poler (FastCDC+Zstd+BLAKE3)"
tar czf capsule.tar.gz -C "$WORK" rootfs
ls -la capsule.tar.gz | awk '{print "  tar.gz:", $5, "байт"}'
"$ENG" --stream-file ./capsule.tar.gz --output-archive "$CAPSULE" 2>&1 | tail -4
ls -la "$CAPSULE" | awk '{print "  .poler:", $5, "байт"}'

echo "── 3. верификация капсулы"
"$ENG" --poler-verify "$CAPSULE" 2>&1 | head -5
"$ENG" --poler-list "$CAPSULE" 2>&1 | rg '"name"' | head -10

echo "── 4. ЗАПУСК В КОРОБКЕ: мозг × квант × триты × Born-сэмплер (namespaces+seccomp)"
"$ENG" --poler-box "$CAPSULE" --box-entry rootfs/bin/capsule_launch \
     --box-rss-mb 512 --box-cpu-s 600 --box-tmpfs-mb 128
BOX_EXIT=$?
echo "box_exit=$BOX_EXIT"
echo "═══ КОНЕЦ: капсула $CAPSULE ═══"
rm -rf "$WORK"
echo "[очищено] $WORK — диск не замусорен"

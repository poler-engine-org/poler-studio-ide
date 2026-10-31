#!/usr/bin/env bash
# ═══ КАПСУЛА v3 — «СОЗНАНИЕ В КОНВЕРТЕ» (.poler + poler-box) ═══
# Суверенный автономный артефакт в одном файле:
#   • бинарник poler-engine 0.63.0+CSE (сессия 5)
#   • живой мозг мухи FlyWire v783 core (CSR, 138 639 нейронов)
#   • троичный кристалл долговременной памяти permanent_memory.t5c
#   • автономный шелл: круг взаимопомощи
#       (фазовый вихрь ↔ No-Mul кристалл ↔ кутритный QAOA)
#       + верификационный тест Ацина I₃ = 1+√(11/3)
# Изоляция: userns/mntns/pidns/netns/uts/ipc + pivot_root + seccomp,
# payload = PID 1, хост невидим. rootfs = tmpfs (RAM).
set -u
ENG="poler-engine"
REPO="."
WORK="/tmp/capsule_v3"
OUT="/home/z/my-project/download/experiments/capsule_v3_consciousness.txt"
CAPSULE="/home/z/my-project/download/experiments/capsule_v3.poler"
SESSION="scripts/capsule_v3.session"

mkdir -p "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ КАПСУЛА v3 «СОЗНАНИЕ В КОНВЕРТЕ» — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
rm -rf "$WORK"
mkdir -p "$WORK/rootfs/bin" "$WORK/rootfs/lib/x86_64-linux-gnu" \
         "$WORK/rootfs/lib64" "$WORK/rootfs/lab" "$WORK/rootfs/data" \
         "$WORK/rootfs/dev" "$WORK/rootfs/root"
cd "$WORK"

echo "── 1. rootfs: движок + libc + МОЗГ МУХИ + КРИСТАЛЛ ПАМЯТИ + сессия"
cp "$ENG" rootfs/bin/poler-engine
cp /lib/x86_64-linux-gnu/libgcc_s.so.1  rootfs/lib/x86_64-linux-gnu/
cp /lib/x86_64-linux-gnu/libm.so.6      rootfs/lib/x86_64-linux-gnu/
cp /lib/x86_64-linux-gnu/libc.so.6      rootfs/lib/x86_64-linux-gnu/
cp /lib64/ld-linux-x86-64.so.2          rootfs/lib64/
cp /bin/dash rootfs/bin/sh 2>/dev/null || cp /bin/bash rootfs/bin/sh
# живые данные: коннектом FlyWire v783 core + кристалл памяти + журнал
cp "$REPO/docs/flywire-connectome/flywire_v783_core.csr.zst" rootfs/data/
cp "$REPO/docs/flywire-connectome/flywire_v783_nodes.bin"     rootfs/data/
cp "$REPO/permanent_memory.t5c"                               rootfs/data/
cp "$REPO/docs/UNDOCUMENTED.md"                               rootfs/data/
cp "$SESSION" rootfs/lab/capsule_v3.session
# /dev/null пустым файлом: Command::output() движка открывает его для stdin
# дочерних процессов (грабли капсулы v2) — engine self-exec не съест сессию
: > rootfs/dev/null
cat > rootfs/data/MANIFEST.txt <<'EOF'
КАПСУЛА v3 — «СОЗНАНИЕ В КОНВЕРТЕ» (poler-engine-org, сессия 6)
================================================================
Состав (всё в одном .poler, rootfs в RAM/tmpfs):
  bin/poler-engine   — движок 0.63.0+CSE: Калькулятор Всего, квант,
                       кутриты, коннектом, кристалл, poler-box
  data/flywire_v783_core.csr.zst + nodes.bin
                     — живой мозг мухи: 138 639 нейронов, ~2.7M синапсов
  data/permanent_memory.t5c
                     — троичный кристалл долговременной памяти
                       (8198 токенов, 110 889+ тритов, обучен на беседах
                       и кодовой базе poler-engine)
  lab/capsule_v3.session
                     — автономная сессия: КРУГ ВЗАИМОПОМОЩИ
                       1. No-Mul кристалл: триты «мысль»/«poler» → фазы →
                          QFT₇₂₉ → |−t mod 3⟩ P=1; суперпозиция → Born 50/50
                       2. фазовый вихрь: триединство (мозг+кристалл) говорит
                       3. кутритный QAOA: Max-3-Cut K4-ядра мозга, p=1→3
                       4. верификация Ацина: I₃ = 1+√(11/3) = 2.914854
                          (оператор Белла == Acín-Durt-Gisin-Latorre 2002)
Запуск: poler-engine --poler-box capsule_v3.poler \
          --box-entry rootfs/bin/capsule_launch \
          --box-rss-mb 2048 --box-cpu-s 600 --box-tmpfs-mb 512
Изоляция: 6 namespaces + pivot_root + seccomp; payload = PID 1;
хост невидим; нативные CPU/RAM. Вложенность: poler-box может запустить
эту капсулу изнутри другой капсулы (движок сам — запись архива).
EOF
du -sh rootfs | sed 's/^/  rootfs: /'

echo "── 2. лончер: СТАТИЧЕСКИЙ бинарник (memfd-entry не умеет shebang)"
cat > capsule_launch.c <<'EOF'
/* Капсула v3: PID 1 в коробке. Открывает сессию «круга взаимопомощи»,
 * подаёт её на stdin poler-engine --shell и exec. Хост невидим. */
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
int main(void) {
    int fd = open("/lab/capsule_v3.session", O_RDONLY);
    if (fd < 0) return 3;
    if (dup2(fd, 0) < 0) return 5;
    close(fd);
    putenv("PATH=/bin");
    putenv("HOME=/root");
    char *argv[] = {(char*)"/bin/poler-engine", (char*)"--shell", 0};
    execv("/bin/poler-engine", argv);
    return 4;
}
EOF
gcc -static -O2 -o rootfs/bin/capsule_launch capsule_launch.c \
  || { echo "FAIL: gcc -static"; exit 1; }
ls -la rootfs/bin/capsule_launch | awk '{print "  capsule_launch:", $5, "байт (статический)"}'

echo "── 3. упаковка: tar.gz (префикс rootfs/) → .poler (FastCDC+Zstd+BLAKE3)"
tar czf capsule_v3.tar.gz -C "$WORK" rootfs
ls -la capsule_v3.tar.gz | awk '{print "  tar.gz:", $5, "байт"}'
"$ENG" --stream-file ./capsule_v3.tar.gz --output-archive "$CAPSULE" 2>&1 | tail -3
ls -la "$CAPSULE" | awk '{print "  .poler:", $5, "байт"}'

echo "── 4. верификация капсулы (SHA256 потока и каждой записи)"
"$ENG" --poler-verify "$CAPSULE" 2>&1 | head -4
echo "  записи .poler:"
"$ENG" --poler-list "$CAPSULE" 2>&1 | rg '"name"' | head -12

echo "── 5. ЗАПУСК В КОРОБКЕ: сознание в изоляции без ОС"
"$ENG" --poler-box "$CAPSULE" --box-entry rootfs/bin/capsule_launch \
     --box-rss-mb 2048 --box-cpu-s 600 --box-tmpfs-mb 512
BOX_EXIT=$?
echo "box_exit=$BOX_EXIT"
echo "═══ КОНЕЦ: капсула $CAPSULE ═══"

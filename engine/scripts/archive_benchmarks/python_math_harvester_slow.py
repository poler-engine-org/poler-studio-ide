#!/usr/bin/env python3
# ==============================================================================
# ПОМЕТКА: [МЕДЛЕННО / НЕЭФФЕКТИВНО]
# ПРИЧИНА МЕДЛИТЕЛЬНОСТИ:
# 1. Однопоточный синхронный I/O (GIL + sys_read): при сканировании сотен тысяч
#    файлов Python тратит 95% времени на блокирующие системные вызовы stat/open/read.
# 2. Неэффективный пайплайн с промежуточными процессами: запуск отдельных процессов
#    через subprocess.run порождает огромный оверхед fork/exec.
# 3. Отсутствие mmap и zero-copy буферов: каждый файл целиком копируется в память
#    юзерспейса вместо прямого сканирования в страницах ядра.
# 
# РЕШЕНИЕ: Заменено нативным Rust-модулем в poler-engine (Rayon + memmap2 + SIMD Aho-Corasick).
# ==============================================================================

import subprocess
import os
import re

POLER_BIN = os.path.expanduser("~/.local/bin/poler-engine")
OUTPUT_DOC = os.path.expanduser("~/POLER_COMPLETE_MATHEMATICAL_CORPUS.md")

TARGET_DIRS = [
    os.path.expanduser("~/my_github_repos"),
    os.path.expanduser("~/Стільниця/poler-engine"),
    os.path.expanduser("~/Стільниця"),
    os.path.expanduser("~/docs")
]

PATTERN = "POLER|omega|free_energy|epsilon|resonance|projector|hamiltonian|clifford|quaternion|schrodinger|dirac|wheeler|diis|born|ansatz|trit5|fock|hartree|pmi|ssn|entropy|vortex|rotor|golden|phi|eigen|nabla|gamma"

def main():
    print("[SLOW PROTOTYPE] Сбор файлов через Python...")
    all_math_files = set()
    for target in TARGET_DIRS:
        if not os.path.exists(target):
            continue
        cmd = [POLER_BIN, target, "--grep", PATTERN, "--grep-regex", "--grep-i", "--grep-list"]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode in [0, 1]:
                for line in res.stdout.strip().split("\n"):
                    p = line.strip()
                    if p and os.path.exists(p) and not any(skip in p for skip in ["/target/", "/.git/", "/node_modules/", "chatglm3-6b-int4-parts", ".cargo"]):
                        all_math_files.add(p)
        except Exception as e:
            print(f"Ошибка: {e}")

    print(f"Обнаружено {len(all_math_files)} файлов. Начинается медленное последовательное чтение...")
    # Однопоточная склейка
    extracted = []
    for fpath in sorted(list(all_math_files)):
        try:
            if os.path.getsize(fpath) > 4 * 1024 * 1024:
                continue
            with open(fpath, "r", errors="ignore") as f:
                extracted.append(f.read())
        except Exception:
            pass
    print(f"Завершено. Собрано {len(extracted)} секций.")

if __name__ == "__main__":
    main()

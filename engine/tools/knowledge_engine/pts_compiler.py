"""
POLER Knowledge Engine — High-Speed In-Memory Compiler & PTS Generator
Part of POLER[Ψ] Toolsuite.

Transforms raw uncurated dumps (NotebookLM, web text, notes) into strict,
machine-readable Technical Specifications (PTS-XXX) with LaTeX normalization
and 5-phase POLER alignment.
"""

import os
import sys
import glob
import re
import argparse

def clean_latex(text: str) -> str:
    """Cleans broken backslashes, escape sequences and HTML entities in LaTeX."""
    text = re.sub(r'\\\\([a-zA-Z_]+)', r'\\\1', text)
    text = re.sub(r'\\_', r'_', text)
    text = re.sub(r'&gt;', r'>', text)
    text = re.sub(r'&lt;', r'<', text)
    text = re.sub(r'&amp;', r'&', text)
    return text

def extract_metadata(fname: str, text: str):
    m = re.match(r"^(\d+)-(.*)$", fname)
    if m:
        num = int(m.group(1))
        title = m.group(2)
        if title.endswith(".md"): title = title[:-3]
        if title.endswith(".md"): title = title[:-3]
    else:
        num = 999
        title = fname
    return num, title

def categorize_source(title: str, text: str):
    t_low = (title + " " + text[:2000]).lower()
    if any(k in t_low for k in ["poler-dynamis", "poler-eri", "poler core", "poler v0.", "poler-unified", "octonion", "trit5"]):
        return "POLER Core & Dynamics Algebra", "H^\\Psi = 0 \\iff \\hbar\\omega = 0, \\quad a \\otimes_\\varepsilon a = a"
    elif any(k in t_low for k in ["дипсик", "deepseek", "контр аргументы", "тест_критики", "экперимент", "теорема_распада", "доказательсво этой"]):
        return "Debates, Proofs & Counter-Analysis", "\\frac{dp}{dt} = -\\eta \\Pi_\\Lambda [D p + \\gamma J p + \\nabla F]"
    elif any(k in t_low for k in ["hartree-fock", "kohn-sham", "quantum chem", "dft", "orca", "q-chem", "chemistry", "molecular orbital", "basis set"]):
        return "Quantum Chemistry & Orbital Dynamics", "DM^2 = 2\\,DM \\implies \\Delta_{\\text{idem}} \\to 0"
    elif any(k in t_low for k in ["cellular automata", "chaotic", "strange attractor", "cryptography with dynamical", "wireworld", "pnd_linearity"]):
        return "Cellular Automata & Nonlinear Crypto", "\\text{LHCA}(x \\oplus y) = \\text{LHCA}(x) \\oplus \\text{LHCA}(y)"
    elif any(k in t_low for k in ["free energy principle", "active inference", "friston", "fep"]):
        return "Free Energy Principle & Active Inference", "F = \\|g(p) - \\Omega(o)\\|_G^2 + \\lambda R_L(p) \\to 0"
    elif any(k in t_low for k in ["subquantum kinetics", "laviolette", "ether", "эфир", "cosmic ether"]):
        return "Subquantum Kinetics & Open Media", "\\frac{\\partial \\psi}{\\partial t} = D \\nabla^2 \\psi + f(\\psi)"
    elif any(k in t_low for k in ["processing near hbm", "sparse matrix", "tensor network", "quantum annealing", "in-memory"]):
        return "Tensor Networks, PIM & High-Performance Hardware", "I_{\\text{out}} = \\sum_j V_j \\cdot G_{ji}"
    elif any(k in t_low for k in ["rust", "zig", "burn", "dfdx", "cachyos", "simd", "gpu matmul"]):
        return "High-Performance Systems & SIMD Execution", "\\text{Constant-Time}(\\mathcal{O}(1)), \\quad \\text{Zero-Copy SIMD}"
    else:
        return "Causal Information Theory & Semantic Resonance", "T = \\frac{\\Delta I(F_n)}{\\Delta \\Sigma}"

def compile_corpus(src_dir: str, output_dir: str):
    os.makedirs(output_dir, exist_ok=True)
    files = sorted(glob.glob(os.path.join(src_dir, "*.md")))
    print(f"[*] Processing {len(files)} files from '{src_dir}'...")

    master_index_entries = []

    for fpath in files:
        fname = os.path.basename(fpath)
        with open(fpath, "r", encoding="utf-8", errors="ignore") as f:
            raw_text = f.read()

        num, title = extract_metadata(fname, raw_text)
        cleaned = clean_latex(raw_text)
        category, inv = categorize_source(title, cleaned)

        spec_lines = [
            f"# PTS-{num:03d}: ТЕХНИЧЕСКАЯ СПЕЦИФИКАЦИЯ — {title.upper()}",
            "",
            f"> **Стандарт:** POLER Technical Specification (`PTS-{num:03d}`)",
            f"> **Классификация:** `{category}`",
            f"> **Базовый математический инвариант:** ${inv}$",
            f"> **Исходный первоисточник:** `{fname}`",
            "",
            "---",
            "",
            "## 1. НАЗНАЧЕНИЕ И АРХИТЕКТУРНЫЙ КОНТЕКСТ",
            f"Данный документ формализует инженерные принципы, математические соотношения и алгоритмические структуры первоисточника #{num:03d} в составе каузального ядра **POLER[Ψ]**.",
            "Спецификация исключает стохастический шум, транслируя феноменологические наблюдения в строгие алгебраические операторы.",
            "",
            "---",
            "",
            "## 2. МАТЕМАТИЧЕСКИЙ АППАРАТ И ФОРМУЛЬНЫЕ ИНВАРИАНТЫ",
            f"$$\\boxed{{{inv}}}$$",
            "",
            "### Ключевые аналитические операторы:",
            "* **Инвариант фазового пространства:** Непрерывная эволюция в метрике Римана $G(p)$.",
            "* **Проекция каузальности:** Фильтрация допустимых траекторий через проектор МакВини $\\Pi_\\Lambda = 3P^2 - 2P^3$.",
            "* **Диссипация шума:** Оператор $D = L L^T \\ge 0$, обеспечивающий сходимость к стационарному аттрактору $H^\\Psi = 0$.",
            "",
            "---",
            "",
            "## 3. СОПРЯЖЕНИЕ С 5-ФАЗНЫМ ЦИКЛОМ POLER[Ψ]",
            "Спецификация интегрируется в пятифазную шкалу причинности $\\wp \\to O \\to L \\to \\varepsilon \\to R[n]$:",
            "* **Фаза $\\wp$ (Перцепция):** Сенсорный перехват входных тензоров без информационной деградации.",
            "* **Фаза $O$ (Образ):** Топологическое сопоставление с базисными архетипами `SubquantumDictionary`.",
            "* **Фаза $L$ (Логика):** Жесткая проекция $\\Pi_\\Lambda(p)$ — обнуление нефизических состояний.",
            "* **Фаза $\\varepsilon$ (Энергия значения):** Скалярная кривизна $\\varepsilon = \\kappa \\Delta x^T G(p) \\Delta x$.",
            "* **Фаза $R[n]$ (Резонанс):** Интегрирование темпорального эха через IIR-фильтр памяти ($P[n] = \\int A(t) e^{-\\lambda t} dt$).",
            "",
            "---",
            "",
            "## 4. ПОЛНЫЙ ТЕХНИЧЕСКИЙ ТЕКСТ И СИНТЕЗ ПЕРВОИСТОЧНИКА",
            "",
            cleaned,
            "",
            "---",
            "",
            "## 5. ВЕРИФИКАЦИОННЫЕ ТРЕБОВАНИЯ (MVR)",
            "1. **Детерминизм:** Результат вычисления инварианта должен быть строго воспроизводим ($0$ стохастических отклонений).",
            "2. **Алгебраическая замкнутость:** Все операции должны сохранять норму $\\|p_t\\|_G = \\text{const}$ при унитарных вращениях $J(p)$.",
            "3. **Constant-Time Execution:** Отсутствие ветвлений, зависящих от секретных ключей или скрытых фазовых переменных.",
            ""
        ]

        out_fname = f"PTS-{num:03d}.md"
        with open(os.path.join(output_dir, out_fname), "w", encoding="utf-8") as out_f:
            out_f.write("\n".join(spec_lines))

        master_index_entries.append((num, title, category, inv, out_fname))

    # Generate master index
    index_path = os.path.join(output_dir, "PTS_MASTER_INDEX.md")
    with open(index_path, "w", encoding="utf-8") as idx_f:
        idx_f.write("# 🏛️ СВОДНЫЙ РЕЕСТР ТЕХНИЧЕСКИХ СПЕЦИФИКАЦИЙ POLER\n\n")
        idx_f.write("| # | Спецификация | Классификация | Базовый инвариант | Ссылка |\n")
        idx_f.write("|---|---|---|---|---|\n")
        for num, title, category, inv, out_fname in sorted(master_index_entries, key=lambda x: x[0]):
            idx_f.write(f"| **PTS-{num:03d}** | {title[:50]} | `{category}` | `${inv}$` | [`Открыть`]({out_fname}) |\n")

    print(f"[+] Successfully compiled {len(files)} Technical Specifications into '{output_dir}'.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="POLER Knowledge Engine Compiler")
    parser.add_argument("--src", required=True, help="Path to raw markdown source folder")
    parser.add_argument("--out", required=True, help="Destination directory for PTS specs")
    args = parser.parse_args()
    compile_corpus(args.src, args.out)

#!/usr/bin/env python3
"""
Benchmark: POLER ε v7.0-LEM (з лематизацією) vs v7.0 canonical (без лематизації).

Порівнює:
  1. v7.0-sw canonical — словоформи З фільтром стоп-слів (несумісно з
     benchmark_poler_epsilon.py, де фільтра НЕМАЄ — виправлення аудиту)
  2. v7.0-LEM canonical_lemmatized — зводить словоформи до лем через dict_uk
     перед обчисленням рідкості

Очікуваний результат (з sympy_lemmatization_impact.py):
  - ε_lem / ε_word ≈ √(δ + |U_word|) / √(δ + α·|U_word|)
  - При α=0.7, δ=15, |U_word|=20: ratio ≈ 1.099 → ε зростає на ~9.9%
  - S/N розділення покращується: σ_noise зменшується

Використання:
  cd /home/z/my-project/litgraph-desktop
  python3 scripts/benchmark_poler_v7_lem.py
"""

import math
import re
import time
import os
import gzip
import json
from collections import defaultdict
import numpy as np

# ============================================================================
# Лексикони (з benchmark_poler_epsilon.py)
# ============================================================================

CANON_ANCHORS = set([
    "етерія", "буфер", "сектор", "хмара", "геліос", "теневра", "фосфор",
    "кассіопея", "яр", "ущелина", "аніма", "руна", "вузол", "код", "матриця",
    "інквесторат", "триада", "рада", "пропуск", "чип", "пластик", "стійбище",
    "архів", "проект", "алгоритм", "система", "редакція", "сигнал", "ток",
    "χ-оружие", "хи-оружие", "док", "причал", "буферу", "етерії", "геліоса",
])

ACTION_VERBS = set([
    "вбити", "убити", "умерти", "померти", "загинути", "застрелити", "отруїти",
    "підірвати", "зрадити", "врятувати", "визволити", "схопити", "ув'язнити",
    "поранити", "ударити", "знівечити", "підпалити", "воскреснути",
    "наказати", "примусити", "пообіцяти", "присягти", "проникнути", "зламати",
    "убить", "умереть", "погибнуть", "застрелить", "отравить", "казнить",
    "взорвать", "предать", "спасти", "освободить", "схватить", "пленить",
    "ранить", "ударить", "воскреснуть", "приказать", "заставить", "пообещать",
])

EMOTIONAL_MARKERS = set([
    "крик", "кричати", "страх", "боятися", "жах", "біль", "боліти", "плач", "плакати",
    "сльози", "лють", "гнів", "паніка", "ненависть", "любов", "кохати", "кохання",
    "розчарування", "розруха", "агонія", "кривавий", "кров", "смерть", "відчай",
    "крикнуть", "ужас", "боль", "слезы", "ярость", "гнев", "паника", "ненависть",
    "любовь", "любила", "любил", "крови", "кровь", "агония", "отчаяние", "безумие",
    "хаос", "сила", "свідомість", "реальність", "істина", "тінь", "світло", "темрява",
    "безодня", "вічність", "тиша", "пам'ять", "надія", "зрада", "прощення", "самотність",
    "доля", "свобода", "вибір", "правда", "війна", "життя", "вогонь", "гнів", "час", "мить",
])

STOP_WORDS = set([
    "і","та","й","в","у","на","з","до","за","від","по","при","про","для","із",
    "це","той","ця","те","він","вона","воно","вони","його","її","їх",
    "я","ти","ми","ви","мене","тебе","себе","мені","тобі","собі",
    "але","або","що","як","де","куди","коли","чому","тому","тож",
    "був","була","було","були","є","бути","ніхто","нічого","все","всі",
    "сьогодні","вчора","завтра","тепер","тоді","потім","раптом",
    "швидко","знову","ще","вже","тільки","навіть","можливо","так","ні",
    "и","в","на","с","к","за","от","по","при","про","для","из","не","ни",
    "это","тот","эта","эти","он","она","оно","они","его","её","их",
    "я","ты","мы","вы","меня","тебя","себя","мне","тебе",
    "но","или","что","как","где","куда","когда","почему","поэтому",
    "был","была","было","были","есть","быть",
    "сегодня","вчера","завтра","теперь","тогда","потом","внезапно",
    "быстро","снова","ещё","уже","только","даже","возможно","да","нет",
    "the","a","an","and","or","but","in","on","at","to","for","of","with",
    "this","that","these","those","he","she","it","they","his","her","its",
    "is","was","were","been","have","has","had","not","no",
    "i","you","we","me","my","your","our",
])

# ============================================================================
# Канонічні константи (з POLER_EPSILON_CANONICAL_SPECIFICATION.md §4.1)
# ============================================================================

DELTA_BIAS = 15.0
THETA_BASE = 3.5
CLIMAX_THRESHOLD = 7.5
RARITY_MIN = 0.1
RARITY_MAX = 4.5

# ============================================================================
# Лематизатор (завантаження lemma_index.json.gz)
# ============================================================================

LEMMA_INDEX = None  # lazy-loaded dict: word_form_lower -> list of {lemma, pos, paradigm_class}

def load_lemma_index(path=None):
    """Load lemma index from gzipped JSON. Returns dict or None if not found."""
    global LEMMA_INDEX
    if LEMMA_INDEX is not None:
        return LEMMA_INDEX
    # Resolve path relative to repo root (parent of scripts/)
    if path is None:
        repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        path = os.path.join(repo_root, "resources/ua-linguistic/derivatives/lemma_index.json.gz")
    if not os.path.exists(path):
        print(f"WARNING: lemma index not found at {path}")
        print("         Run `cargo run --release -- build-lemmatizer` to build it.")
        return None
    print(f"Loading lemma index from {path}...")
    start = time.time()
    with gzip.open(path, 'rt', encoding='utf-8') as f:
        LEMMA_INDEX = json.load(f)
    elapsed = (time.time() - start) * 1000
    print(f"  Loaded {len(LEMMA_INDEX):,} word forms in {elapsed:.1f} ms")
    return LEMMA_INDEX

def lemmatize_first(word):
    """Return the first lemma for a word form, or the original word if unknown."""
    idx = LEMMA_INDEX
    if idx is None:
        return word
    entries = idx.get(word.lower())
    if entries and len(entries) > 0:
        return entries[0]["lemma"].lower()
    return word.lower()

# ============================================================================
# Рідкість слів
# ============================================================================

def calculate_word_rarity(word):
    """rarity(w) = -log10(p_w), clamped to [0.1, 4.5]."""
    clean = word.strip().lower()
    if len(clean) <= 2:
        return 0.0
    if clean in CANON_ANCHORS:
        p_w = 0.0001
    elif clean in ACTION_VERBS:
        p_w = 0.0003
    elif clean in EMOTIONAL_MARKERS:
        p_w = 0.0002
    else:
        l = len(clean)
        if 3 <= l <= 4:
            p_w = 0.05
        elif 5 <= l <= 7:
            p_w = 0.01
        elif 8 <= l <= 10:
            p_w = 0.002
        else:
            p_w = 0.0005
    rarity = -math.log10(p_w)
    return max(RARITY_MIN, min(RARITY_MAX, rarity))

# ============================================================================
# Канонічна ε (v7.0) — без лематизації
# ============================================================================

def compute_epsilon_canonical(fragment, keyword=None, kappa=1.0, delta_bias=DELTA_BIAS):
    """v7.0 canonical ε: uses word forms directly."""
    tokens = [w for w in re.findall(r'\w+', fragment, re.UNICODE) if len(w) > 2]
    unique_tokens = set(w.lower() for w in tokens if w.lower() not in STOP_WORDS)
    u_len = len(unique_tokens)
    if u_len == 0:
        return 0.0, 0, 0, 0, 0, True, False

    kw_lower = keyword.lower() if keyword else None
    kw_count = 0
    emotion_count = 0
    canon_count = 0
    action_count = 0
    d_sum = 0.0

    for w in unique_tokens:
        rarity = calculate_word_rarity(w)
        d_sum += rarity
        if kw_lower and w == kw_lower:
            kw_count += 1
        if w in EMOTIONAL_MARKERS:
            emotion_count += 1
        if w in CANON_ANCHORS:
            canon_count += 1
        if w in ACTION_VERBS:
            action_count += 1

    i_kw = 1.0 + math.log(1 + kw_count)
    e_val = 1.5 * emotion_count
    c_canon = 3.0 * canon_count
    a_svo = 2.0 * action_count

    len_norm = math.sqrt(u_len + delta_bias)
    eps = (kappa * i_kw * d_sum + e_val + c_canon + a_svo) / len_norm

    theta_rel = THETA_BASE / kappa
    is_noise = eps < theta_rel
    is_climax = eps >= CLIMAX_THRESHOLD

    return eps, u_len, kw_count, emotion_count, action_count, is_noise, is_climax

# ============================================================================
# Канонічна ε (v7.0-LEM) — з лематизацією
# ============================================================================

def compute_epsilon_lemmatized(fragment, keyword=None, kappa=1.0, delta_bias=DELTA_BIAS):
    """v7.0-LEM canonical_lemmatized ε: word forms → lemmas before computing rarity."""
    tokens = [w for w in re.findall(r'\w+', fragment, re.UNICODE) if len(w) > 2]
    # Lemmatize each token, then deduplicate
    lemmatized_tokens = [lemmatize_first(w) for w in tokens if w.lower() not in STOP_WORDS]
    unique_tokens = set(lemmatized_tokens)
    u_len = len(unique_tokens)
    if u_len == 0:
        return 0.0, 0, 0, 0, 0, True, False

    kw_lower = keyword.lower() if keyword else None
    kw_count = 0
    emotion_count = 0
    canon_count = 0
    action_count = 0
    d_sum = 0.0

    for w in unique_tokens:
        rarity = calculate_word_rarity(w)
        d_sum += rarity
        if kw_lower and w == kw_lower:
            kw_count += 1
        if w in EMOTIONAL_MARKERS:
            emotion_count += 1
        if w in CANON_ANCHORS:
            canon_count += 1
        if w in ACTION_VERBS:
            action_count += 1

    i_kw = 1.0 + math.log(1 + kw_count)
    e_val = 1.5 * emotion_count
    c_canon = 3.0 * canon_count
    a_svo = 2.0 * action_count

    len_norm = math.sqrt(u_len + delta_bias)
    eps = (kappa * i_kw * d_sum + e_val + c_canon + a_svo) / len_norm

    theta_rel = THETA_BASE / kappa
    is_noise = eps < theta_rel
    is_climax = eps >= CLIMAX_THRESHOLD

    return eps, u_len, kw_count, emotion_count, action_count, is_noise, is_climax

# ============================================================================
# Аналіз манускрипту
# ============================================================================

def analyze_manuscript(filepath, name, kappa=1.0):
    print(f"\n{'='*70}")
    print(f"  POLER EPSILON v7.0 vs v7.0-LEM BENCHMARK: {name}")
    print(f"  File: {filepath}")
    print(f"  Kappa: {kappa}")
    print(f"{'='*70}")

    if not os.path.exists(filepath):
        print(f"ERROR: File {filepath} not found!")
        return

    with open(filepath, 'r', encoding='utf-8') as f:
        text = f.read()

    raw_fragments = [s.strip() for s in re.split(r'[.!?…\n]+', text) if len(s.strip()) > 10]
    total_fragments = len(raw_fragments)

    # v7.0 (word forms)
    t0 = time.time()
    scores_v7 = []
    u_lens_v7 = []
    noise_v7 = 0
    climax_v7 = 0
    for frag in raw_fragments:
        eps, u_len, kw, emo, act, is_noise, is_climax = compute_epsilon_canonical(frag, kappa=kappa)
        scores_v7.append(eps)
        u_lens_v7.append(u_len)
        if is_noise:
            noise_v7 += 1
        if is_climax:
            climax_v7 += 1
    t_v7 = time.time() - t0

    # v7.0-LEM (lemmatized)
    t0 = time.time()
    scores_lem = []
    u_lens_lem = []
    noise_lem = 0
    climax_lem = 0
    for frag in raw_fragments:
        eps, u_len, kw, emo, act, is_noise, is_climax = compute_epsilon_lemmatized(frag, kappa=kappa)
        scores_lem.append(eps)
        u_lens_lem.append(u_len)
        if is_noise:
            noise_lem += 1
        if is_climax:
            climax_lem += 1
    t_lem = time.time() - t0

    scores_v7 = np.array(scores_v7)
    scores_lem = np.array(scores_lem)
    u_lens_v7 = np.array(u_lens_v7)
    u_lens_lem = np.array(u_lens_lem)

    print(f"\nEXECUTION METRICS:")
    print(f"  Total Fragments:                {total_fragments:,}")
    print(f"  v7.0 canonical elapsed:         {t_v7*1000:.2f} ms ({total_fragments/t_v7:.1f} frag/s)")
    print(f"  v7.0-LEM lemmatized elapsed:    {t_lem*1000:.2f} ms ({total_fragments/t_lem:.1f} frag/s)")
    print(f"  Overhead:                       {(t_lem/t_v7 - 1)*100:.1f}%")

    print(f"\nEPSILON STATISTICS (v7.0 canonical vs v7.0-LEM):")
    print(f"  {'Metric':<25} {'v7.0':>12} {'v7.0-LEM':>12} {'Δ':>12}")
    print(f"  {'-'*61}")
    for label, arr_v7, arr_lem in [
        ("Mean Epsilon (μ)", scores_v7, scores_lem),
        ("Std Deviation (σ)", None, None),
        ("Min Epsilon", None, None),
        ("Median (P50)", None, None),
        ("P95", None, None),
        ("Max Epsilon", None, None),
    ]:
        if arr_v7 is None:
            if label == "Std Deviation (σ)":
                v7 = np.std(scores_v7); lem = np.std(scores_lem)
            elif label == "Min Epsilon":
                v7 = np.min(scores_v7); lem = np.min(scores_lem)
            elif label == "Median (P50)":
                v7 = np.percentile(scores_v7, 50); lem = np.percentile(scores_lem, 50)
            elif label == "P95":
                v7 = np.percentile(scores_v7, 95); lem = np.percentile(scores_lem, 95)
            elif label == "Max Epsilon":
                v7 = np.max(scores_v7); lem = np.max(scores_lem)
        else:
            v7 = np.mean(arr_v7); lem = np.mean(arr_lem)
        delta = lem - v7
        delta_pct = (delta / max(v7, 1e-10)) * 100
        print(f"  {label:<25} {v7:>12.4f} {lem:>12.4f} {delta:>+10.4f} ({delta_pct:+.2f}%)")

    print(f"\nUNIQUE WORDS (|U|) COMPARISON:")
    print(f"  Mean |U| v7.0:    {np.mean(u_lens_v7):.2f}")
    print(f"  Mean |U| v7.0-LEM: {np.mean(u_lens_lem):.2f}")
    alpha = np.mean(u_lens_lem) / max(np.mean(u_lens_v7), 1e-10)
    print(f"  Reduction factor α: {alpha:.4f}  (expected ~0.7)")
    print(f"  Theoretical ε ratio: √(δ+|U|) / √(δ+α·|U|) = "
          f"{math.sqrt(np.mean(u_lens_v7) + DELTA_BIAS) / math.sqrt(alpha * np.mean(u_lens_v7) + DELTA_BIAS):.4f}")

    print(f"\nCLASSIFICATION & NOISE FILTERING:")
    theta_rel = THETA_BASE / kappa
    print(f"  Threshold θ_rel = {theta_rel:.2f}")
    print(f"  v7.0    noise:     {noise_v7:>6,} ({noise_v7/total_fragments*100:.2f}%)")
    print(f"  v7.0-LEM noise:    {noise_lem:>6,} ({noise_lem/total_fragments*100:.2f}%)")
    print(f"  v7.0    climax:    {climax_v7:>6,} ({climax_v7/total_fragments*100:.2f}%)")
    print(f"  v7.0-LEM climax:   {climax_lem:>6,} ({climax_lem/total_fragments*100:.2f}%)")

    # Top-5 comparison
    print(f"\nTOP 5 CLIMAX FRAGMENTS COMPARISON:")
    top_v7 = np.argsort(scores_v7)[::-1][:5]
    top_lem = np.argsort(scores_lem)[::-1][:5]
    print(f"  v7.0 top-5 ε:    {[f'{scores_v7[i]:.4f}' for i in top_v7]}")
    print(f"  v7.0-LEM top-5 ε: {[f'{scores_lem[i]:.4f}' for i in top_lem]}")
    overlap = len(set(top_v7.tolist()) & set(top_lem.tolist()))
    print(f"  Overlap: {overlap}/5 fragments in both top-5 lists")

    print(f"\n{'='*70}\n")


if __name__ == "__main__":
    # Аудит 2026-09-21: (1) lemma_index і манускрипти litgraph-desktop тут відсутні —
    # тепер graceful skip замість exit(1); (2) НОТА: v7.0-каноніка ЦЬОГО файлу
    # фільтрує стоп-слова (STOP_WORDS), тому НЕ збігається з benchmark_poler_epsilon.py
    # (там фільтра немає) — виправлено хибне твердження оригінального докстрингу.
    import argparse
    import tempfile

    ap = argparse.ArgumentParser(description="POLER ε v7.0 (word forms) vs v7.0-LEM (lemmatized)")
    ap.add_argument("files", nargs="*", help="манускрипти .txt/.md (за замовчуванням — демо)")
    ap.add_argument("--kappa", type=float, default=1.0)
    args = ap.parse_args()

    have_lemma = load_lemma_index() is not None
    if not have_lemma:
        print("WARNING: lemma index відсутній (resources/ua-linguistic/derivatives/) — "
              "працює лише v7.0 (word forms); LEM-гілка автоматично ідентична їй.")

    if args.files:
        for fp in args.files:
            analyze_manuscript(fp, os.path.basename(fp), kappa=args.kappa)
    else:
        demo = (
            "Сектор Гамма-3 хмара этерии снова мигнула, и алгоритм буфера выдал ошибку.\n"
            "Крик. Страх сковал её, кровь ударила в виски, паника, отчаяние, безумие.\n"
            "Ну вот, опять пошёл этот дождь, да и ветер поднялся, совсем не погода.\n"
            "Він прокручував план: спочатку карта, потім проникнути у сектор.\n"
        ) * 12
        tmp = tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8")
        tmp.write(demo)
        tmp.close()
        print("Файли не вказані — вбудований демо-текст.")
        analyze_manuscript(tmp.name, "DEMO (built-in)", kappa=args.kappa)
        os.unlink(tmp.name)

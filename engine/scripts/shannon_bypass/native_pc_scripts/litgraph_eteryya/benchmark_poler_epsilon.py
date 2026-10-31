import math
import re
import time
import os
import numpy as np

# Лексиконы
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
])

def calculate_word_rarity(word):
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
    return max(0.1, min(4.5, rarity))

def compute_epsilon_canonical(fragment, keyword=None, kappa=1.0, delta_bias=15.0):
    tokens = [w for w in re.findall(r'\w+', fragment, re.UNICODE) if len(w) > 2]
    unique_tokens = set(w.lower() for w in tokens)
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

    theta_rel = 3.50 / kappa
    is_noise = eps < theta_rel
    is_climax = eps >= 7.50

    return eps, u_len, kw_count, emotion_count, action_count, is_noise, is_climax

def analyze_manuscript(filepath, name, kappa=1.0):
    print(f"\n=======================================================")
    print(f"   POLER EPSILON EMPIRICAL BENCHMARK: {name}")
    print(f"   File: {filepath}")
    print(f"   Sector Scaling Kappa: {kappa}")
    print(f"=======================================================")
    
    if not os.path.exists(filepath):
        print(f"ERROR: File {filepath} not found!")
        return

    start_time = time.time()
    with open(filepath, 'r', encoding='utf-8') as f:
        text = f.read()

    # Разбираем на фрагменты (предложения / абзацы)
    raw_fragments = [s.strip() for s in re.split(r'[.!?…\n]+', text) if len(s.strip()) > 10]
    total_fragments = len(raw_fragments)
    
    scores = []
    noise_count = 0
    climax_count = 0
    token_counts = []
    
    for frag in raw_fragments:
        eps, u_len, kw_c, emo_c, act_c, is_noise, is_climax = compute_epsilon_canonical(frag, kappa=kappa)
        scores.append(eps)
        token_counts.append(u_len)
        if is_noise:
            noise_count += 1
        if is_climax:
            climax_count += 1

    elapsed = time.time() - start_time
    scores = np.array(scores)
    
    print(f"EXECUTION METRICS:")
    print(f"Total Text Length:     {len(text):,} chars ({len(text.split()):,} words)")
    print(f"Total Fragments:       {total_fragments:,}")
    print(f"Compute Elapsed Time:  {elapsed*1000:.2f} ms")
    print(f"Throughput Speed:      {total_fragments / elapsed:.1f} fragments/sec")
    print(f"-------------------------------------------------------")
    print(f"STATISTICAL EPSILON DISTRIBUTION:")
    print(f"Mean Epsilon (μ):      {np.mean(scores):.4f}")
    print(f"Std Deviation (σ):     {np.std(scores):.4f}")
    print(f"Min Epsilon:           {np.min(scores):.4f}")
    print(f"25th Percentile (Q1):  {np.percentile(scores, 25):.4f}")
    print(f"50th Percentile (Med): {np.percentile(scores, 50):.4f}")
    print(f"75th Percentile (Q3):  {np.percentile(scores, 75):.4f}")
    print(f"90th Percentile:       {np.percentile(scores, 90):.4f}")
    print(f"95th Percentile:       {np.percentile(scores, 95):.4f}")
    print(f"Max Epsilon:           {np.max(scores):.4f}")
    print(f"-------------------------------------------------------")
    print(f"CLASSIFICATION & NOISE FILTERING:")
    print(f"Noise Fragments (<{3.50/kappa:.2f}):  {noise_count:,} ({noise_count/total_fragments*100:.2f}%)")
    print(f"Standard Moments:      {total_fragments - noise_count - climax_count:,} ({(total_fragments - noise_count - climax_count)/total_fragments*100:.2f}%)")
    print(f"Climax Moments (>=7.5): {climax_count:,} ({climax_count/total_fragments*100:.2f}%)")
    print(f"=======================================================\n")

    # Топ-5 кульминационных фрагментов
    top_indices = np.argsort(scores)[::-1][:5]
    print(f"TOP 5 CLIMAX FRAGMENTS (Highest Epsilon):")
    for i, idx in enumerate(top_indices):
        print(f"#{i+1}: Epsilon={scores[idx]:.4f}")
        print(f"    Text: \"{raw_fragments[idx][:120]}...\"\n")

if __name__ == "__main__":
    # Аудит 2026-09-21: оригинал имел ЗАХАРДКОЖЕННЫЕ пути litgraph-core/tests/*.md
    # (другой проект) — здесь файлов нет, скрипт всегда падал. Теперь: CLI-аргументы
    # + встроенный демо-текст, если файлов не передали.
    import argparse
    import tempfile

    parser = argparse.ArgumentParser(
        description="POLER ε-metric benchmark (порт из litgraph-desktop). "
                    "Формула: ε = (κ·I_kw·Σrarity + E + C_canon + A_SVO)/√(|U|+δ).")
    parser.add_argument("files", nargs="*", help="манускрипты .txt/.md (по умолчанию — демо-текст)")
    parser.add_argument("--kappa", type=float, default=1.0)
    args = parser.parse_args()

    if args.files:
        for fp in args.files:
            analyze_manuscript(fp, f"{os.path.basename(fp)}", kappa=args.kappa)
    else:
        # Демо-фрагменты (синтетика с якорями/действиями/эмоциями и «шумом»)
        demo = (
            "Сектор Гамма-3 хмара этерии снова мигнула, и алгоритм буфера выдал ошибку.\n"
            "Он взял карту, приказал группе двигаться к причалу и не оглядываться.\n"
            "Ну вот, опять пошёл этот дождь, да и ветер поднялся, совсем не погода.\n"
            "Крик. Страх сковал её, кровь ударила в виски, паника, отчаяние, безумие.\n"
            "Кассіопея горіла у безодні, і ніхто не міг врятувати її від долі.\n"
            "Система матрица узел код проект архив сигнал ток чип пластик.\n"
            "Він прокручував план: спочатку карта, потім проникнути у сектор.\n"
            "Она убить не смогла — предать значит умереть, спасти значит воскреснуть.\n"
        ) * 12
        tmp = tempfile.NamedTemporaryFile("w", suffix=".md", delete=False, encoding="utf-8")
        tmp.write(demo)
        tmp.close()
        print("Файлы не указаны — запускаю на встроенном демо-тексте "
              "(8 фрагментов × 12 повторов с вариативным «шумом»).")
        analyze_manuscript(tmp.name, "DEMO (built-in)", kappa=args.kappa)
        os.unlink(tmp.name)

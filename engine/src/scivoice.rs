//! =======================================================================
//! ГОЛОС УЧЁНОГО: сквозной контур научной речи движка
//! =======================================================================
//!
//! Соединяет четыре контура в одну фразировку уровня исследователя:
//! 1. Токенизатор кристалла (`triune::crystal::tokenize`) — слова из входа.
//! 2. Языковой ротор букв мира (104k No-Mul ASM / Rust-фолбэк) — 12-мерная
//!    сигнатура Φ-фаз текста: какие архетипы речи текст возбуждает и гасит.
//! 3. Химический ротор + `universal_chem` — если в тексте распознана формула,
//!    даёт стехиометрию, триты связей и фазовые суммы ротора.
//! 4. Триединое ядро (`TriuneCore` на встроенном кристалле) — нейросетевая
//!    фразировка вердикта: не скрипт, а темперамент вихря.
//!
//! Точка входа для калькулятора: `calc speak("C6H12O6")` / `calc speak("текст")`.

use std::collections::BTreeMap;

use crate::asm_rotors::{archetype_weight, asm_rotors_linked, chem_rotor_resonance, ChemRotorResonance};
use crate::universal_archetype_asm::{ARCHETYPE_NAMES, NUM_ARCHETYPES};

/// Химическое прочтение текста (если распознана формула).
#[derive(Debug, Clone)]
pub struct ChemVoice {
    pub formula: String,
    pub counts: BTreeMap<&'static str, usize>,
    pub molar_mass: f64,
    pub total_electrons: usize,
    pub total_atoms: usize,
    /// Пары «элемент-элемент» → трит связи {-1,0,+1} и Δχ
    pub bonds: Vec<(&'static str, &'static str, i8, f64)>,
    /// Доминирующий квантово-химический архетип формулы
    pub dominant_archetype: &'static str,
    /// Резонанс No-Mul химротора (фаза + Δχ-баланс)
    pub rotor: ChemRotorResonance,
}

/// Полный отчёт «голоса учёного» по тексту.
#[derive(Debug, Clone)]
pub struct VoiceReport {
    pub chars_total: usize,
    pub letters: usize,
    pub words: usize,
    /// Σ тритов языкового ротора по 12 архетипам (сигнатура текста)
    pub arch_signature: [i64; NUM_ARCHETYPES],
    /// Наиболее возбуждённый архетип
    pub dominant_arch: usize,
    /// Наиболее подавленный архетип
    pub suppressed_arch: usize,
    /// |Σ| сигнатуры / длина — когерентность Φ-фазы (0..1)
    pub coherence: f64,
    /// Химическое прочтение, если формула распознана
    pub chem: Option<ChemVoice>,
    /// Итоговый трит вердикта {-1, 0, +1}
    pub trit_verdict: i8,
    /// Нейросетевая фраза триединого ядра
    pub triune_phrase: Option<String>,
    /// Телеметрия вихря в момент речи (синхронность, активность)
    pub triune_synchrony: Option<f64>,
    /// Каким путём идут роторы в этой сборке
    pub rotors_path: &'static str,
}

/// Токенизация кристалла — канонический токенизатор движка.
fn engine_tokens(text: &str) -> Vec<String> {
    crate::triune::crystal::tokenize(text)
}

/// Попытка распознать химическую формулу в тексте.
/// Токены — с сохранением регистра (токенизатор кристалла нижнит регистр
/// для нейросети, а парсер формул требует заглавных: C6H12O6 ≠ c6h12o6).
fn detect_formula(text: &str) -> Option<(String, crate::universal_chem::ParsedMolecule)> {
    let mut raw_tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || matches!(ch, '(' | ')' | '[' | ']' | '{' | '}') {
            cur.push(ch);
        } else if !cur.is_empty() {
            raw_tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        raw_tokens.push(cur);
    }
    for tok in raw_tokens {
        // Отсекаем явный шум: слишком длинные слова без цифр — почти всегда проза
        if tok.len() > 24 {
            continue;
        }
        if let Ok(mol) = crate::universal_chem::parse_chemical_formula(&tok) {
            let has_digit = tok.chars().any(|c| c.is_ascii_digit());
            let distinct = mol.counts.len();
            // Формула обязана быть похожей на формулу: либо с индексами,
            // либо минимум с двумя разными элементами короткой записи.
            if has_digit || distinct >= 2 {
                return Some((tok, mol));
            }
        }
    }
    None
}

/// Лёгкий экземпляр триединого ядра на встроенном кристалле: одна фраза.
fn triune_voice(prompt: &str) -> Option<(String, f64)> {
    let crystal = crate::triune::Crystal::embedded().ok()?;
    let cfg = crate::triune::TriuneConfig::default();
    let fly = crate::triune::FlyPulse::synthetic(777, 0.8);
    let mut core = crate::triune::TriuneCore::new(crystal, cfg, fly, 777);
    let u = core.speak(prompt, 10);
    let text = u.text.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    Some((text, u.telemetry.synchrony))
}

/// Полный анализ текста контуром «учёный».
pub fn analyze(text: &str) -> VoiceReport {
    let tokens = engine_tokens(text);
    let mut letters = 0usize;
    let mut arch_signature = [0i64; NUM_ARCHETYPES];

    // Языковой ротор: сигнатура каждого уникального символа — через .s-монолит
    let mut cache: BTreeMap<char, [i64; NUM_ARCHETYPES]> = BTreeMap::new();
    for c in text.chars() {
        if c.is_whitespace() || c.is_control() {
            continue;
        }
        letters += 1;
        let row = cache.entry(c).or_insert_with(|| {
            let mut row = [0i64; NUM_ARCHETYPES];
            let cp = c as u32;
            for j in 0..NUM_ARCHETYPES {
                row[j] = archetype_weight(cp, j) as i64;
            }
            row
        });
        for j in 0..NUM_ARCHETYPES {
            arch_signature[j] += row[j];
        }
    }

    // Доминанта и подавление
    let mut dominant_arch = 0usize;
    let mut suppressed_arch = 0usize;
    for j in 1..NUM_ARCHETYPES {
        if arch_signature[j] > arch_signature[dominant_arch] {
            dominant_arch = j;
        }
        if arch_signature[j] < arch_signature[suppressed_arch] {
            suppressed_arch = j;
        }
    }
    let sig_sum: i64 = arch_signature.iter().sum();
    let coherence = if letters == 0 {
        0.0
    } else {
        (sig_sum.unsigned_abs() as f64 / letters as f64).min(1.0)
    };

    // Химический контур
    let chem = detect_formula(text).map(|(formula, mol)| {
        let mut elements: Vec<&'static crate::universal_chem::Element> = mol
            .counts
            .keys()
            .filter_map(|sym| crate::universal_chem::get_element_by_symbol(sym))
            .collect();
        elements.sort_by_key(|e| e.z); // пары в порядке Периодической таблицы: Na-Cl, а не Cl-Na

        // Пары связей (до 6 элементов — C,H,O для глюкозы и т.п.)
        let mut bonds = Vec::new();
        for i in 0..elements.len() {
            for j in (i + 1)..elements.len().min(6).max(i + 1) {
                let b = crate::universal_chem::calculate_bond(elements[i], elements[j]);
                bonds.push((elements[i].symbol, elements[j].symbol, b.trit, b.delta_chi));
            }
        }

        // Доминирующий архетип по атомам
        let mut arch_count = BTreeMap::new();
        for (sym, cnt) in &mol.counts {
            if let Some(e) = crate::universal_chem::get_element_by_symbol(sym) {
                *arch_count.entry(e.archetype.name()).or_insert(0usize) += cnt;
            }
        }
        let dominant_archetype = arch_count
            .iter()
            .max_by_key(|(_, &v)| v)
            .map(|(k, _)| *k)
            .unwrap_or("Mixed");

        // Активация ротора: элементы формулы
        let mut active = [0u8; 118];
        for e in &elements {
            active[(e.z as usize) - 1] = 1;
        }
        let rotor = chem_rotor_resonance(&active);

        ChemVoice {
            formula,
            total_atoms: mol.counts.values().sum(),
            counts: mol.counts,
            molar_mass: mol.molar_mass,
            total_electrons: mol.total_electrons,
            bonds,
            dominant_archetype,
            rotor,
        }
    });

    // Трит-вердикт: сигнатура речи + средний трит связей
    let bond_mean = chem
        .as_ref()
        .filter(|c| !c.bonds.is_empty())
        .map(|c| c.bonds.iter().map(|(_, _, t, _)| *t as f64).sum::<f64>() / c.bonds.len() as f64)
        .unwrap_or(0.0);
    let verdict_score = bond_mean * 2.0 + sig_sum as f64 / (letters.max(1) as f64);
    let trit_verdict = if verdict_score > 0.25 {
        1
    } else if verdict_score < -0.25 {
        -1
    } else {
        0
    };

    // Триединое ядро: фраза от нейросети (промпт — химия или сам текст)
    let triune_prompt = chem
        .as_ref()
        .map(|c| format!("{} резонанс фазы", c.formula))
        .unwrap_or_else(|| {
            tokens
                .first()
                .cloned()
                .unwrap_or_else(|| "фаза ротора".into())
        });
    let (triune_phrase, triune_synchrony) = triune_voice(&triune_prompt)
        .map(|(t, s)| (Some(t), Some(s)))
        .unwrap_or((None, None));

    VoiceReport {
        chars_total: text.chars().count(),
        letters,
        words: tokens.len(),
        arch_signature,
        dominant_arch,
        suppressed_arch,
        coherence,
        chem,
        trit_verdict,
        triune_phrase,
        triune_synchrony,
        rotors_path: if asm_rotors_linked() {
            "ASM No-Mul x86_64 (libpoler_rotors.a)"
        } else {
            "Rust-фолбэк (роторы не слинкованы)"
        },
    }
}

/// Готовая научная фраза: движок говорит как учёный, а не как скрипт.
pub fn scientist_read(text: &str) -> String {
    let r = analyze(text);
    let mut parts: Vec<String> = Vec::with_capacity(6);

    parts.push(format!(
        "Научное чтение ({} знаков, {} слов; роторы: {}):",
        r.chars_total,
        r.words,
        r.rotors_path
    ));

    if let Some(c) = &r.chem {
        parts.push(format!(
            "[1] Стехиометрия {}: M = {:.3} г/моль, {} электронов, {} атомов; доминирующий квантовый архетип {}.",
            c.formula, c.molar_mass, c.total_electrons, c.total_atoms, c.dominant_archetype
        ));
        if !c.bonds.is_empty() {
            let bonds_str = c
                .bonds
                .iter()
                .map(|(a, b, t, d)| format!("{a}-{b} {}", trit_word(*t, *d)))
                .collect::<Vec<_>>()
                .join("; ");
            parts.push(format!("[2] Связи: {bonds_str}."));
        }
        parts.push(format!(
            "[3] Химротор: фазовая сумма Sigma+-sin(Z*Phi+j/2) = {:.3}; Delta-chi-баланс пар = {:.3}.",
            c.rotor.archetype_phase_sum, c.rotor.delta_chi_balance
        ));
    }

    let sig_view: Vec<String> = (0..NUM_ARCHETYPES)
        .filter(|&j| r.arch_signature[j] != 0)
        .map(|j| format!("{}:{:+}", ARCHETYPE_NAMES[j], r.arch_signature[j]))
        .take(6)
        .collect();
    // [4] Ab initio (v0.73.0): если распознанная молекула входит в STO-3G-библиотеку —
    // говорим настоящим Хартри–Фоком (POLER-ERI), не оценкой, а расчётом.
    let mut has_hf = false;
    if let Some(c) = &r.chem {
        let hf_spec = match c.formula.as_str() {
            "H2" => Some("H2"),
            "LiH" => Some("LiH"),
            "He" => Some("He"),
            "H2O" => Some("H2O"),
            "NH3" => Some("NH3"),
            "CH4" => Some("CH4"),
            "HF" => Some("HF"),
            "CO" => Some("CO"),
            _ => None,
        };
        if let Some(spec) = hf_spec {
            if let Ok(res) = crate::quantum::eri::hartree_fock(spec) {
                has_hf = true;
                let homo = res
                    .occupied_orbital_energies
                    .last()
                    .copied()
                    .unwrap_or(0.0);
                parts.push(format!(
                    "[4] Ab initio RHF/STO-3G {}: полная энергия E = {:.6} Хартри ({:.4} эВ); {} базисных функций, {} электронов; SCF {} за {} итераций; HOMO ε = {:.4} Ха.",
                    res.molecule,
                    res.energy_hartree,
                    res.energy_hartree * 27.211386245988,
                    res.n_basis,
                    res.n_electrons,
                    if res.converged { "сошёлся" } else { "не сошёлся" },
                    res.iterations,
                    homo
                ));
            }
        }
    }

    let lang_idx = if r.chem.is_some() {
        if has_hf { 5 } else { 4 }
    } else {
        1
    };
    let lang_part = if sig_view.is_empty() {
        format!(
            "[{}] Языковой контур: {} знаков, сигнатура нейтральна (все триты 0).",
            lang_idx,
            r.letters
        )
    } else {
        format!(
            "[{}] Языковой контур: {} знаков; доминирует {} ({:+}), подавлен {} ({:+}); когерентность Phi = {:.2}.",
            lang_idx,
            r.letters,
            ARCHETYPE_NAMES[r.dominant_arch],
            r.arch_signature[r.dominant_arch],
            ARCHETYPE_NAMES[r.suppressed_arch],
            r.arch_signature[r.suppressed_arch],
            r.coherence
        )
    };
    parts.push(lang_part);
    if !sig_view.is_empty() {
        parts.push(format!("    Архетипная сигнатура: {}.", sig_view.join(", ")));
    }

    if let Some(phrase) = &r.triune_phrase {
        let sync = r.triune_synchrony.map(|s| format!(", синхронность вихря {:.2}", s)).unwrap_or_default();
        parts.push(format!(
            "[{}] Триединое ядро: «{}»{}.",
            if r.chem.is_some() {
                if has_hf { 6 } else { 5 }
            } else {
                2
            },
            phrase,
            sync
        ));
    }

    parts.push(format!(
        "Трит-вердикт: {} — {}.",
        trit_sign(r.trit_verdict),
        match r.trit_verdict {
            1 => "структура когерентна, вычисления сошлись",
            -1 => "противоречие фаз: требуется проверка входа",
            _ => "нейтральный баланс: данные insufficient для сильного вывода",
        }
    ));

    parts.join("\n")
}

/// Словесная форма трита связи.
fn trit_word(t: i8, dchi: f64) -> String {
    match t {
        1 => format!("полярная ковалентная (трит +1, Delta-chi = {dchi:.2})"),
        -1 => format!("ионная (трит -1, Delta-chi = {dchi:.2})"),
        _ => format!("неполярная ковалентная (трит 0, Delta-chi = {dchi:.2})"),
    }
}

fn trit_sign(t: i8) -> &'static str {
    match t {
        1 => "+1",
        -1 => "-1",
        _ => "0",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_glucose_full_contour() {
        let r = analyze("C6H12O6");
        let c = r.chem.as_ref().expect("глюкоза обязана распознаваться");
        assert!((c.molar_mass - 180.156).abs() < 0.01, "M = {}", c.molar_mass);
        assert_eq!(c.total_electrons, 96);
        assert_eq!(c.total_atoms, 24);
        assert_eq!(c.dominant_archetype, "OrganicLifeNonmetal");
        assert_eq!(c.bonds.len(), 3, "C-H, C-O, H-O");
        // Связи воды-подобные: O-H полярная
        let oh = c.bonds.iter().find(|(a, b, _, _)| (a == &"H" && b == &"O") || (a == &"O" && b == &"H"));
        assert_eq!(oh.map(|(_, _, t, _)| *t), Some(1));
        // Ротор обязан дать фазу (H и O активны)
        assert!(c.rotor.archetype_phase_sum != 0.0);
    }

    #[test]
    fn analyze_plain_text_no_false_chem() {
        let r = analyze("простой текст без формул");
        assert!(r.chem.is_none(), "проза не должна распознаваться как формула");
        assert!(r.letters > 0);
        assert!(r.words >= 3);
    }

    #[test]
    fn signature_is_balanced_antisymmetric() {
        use crate::universal_archetype_asm::ArchetypeMatrix;
        // Матрица антисимметрична: Σ всех тритов по тексту из полного алфавита ~ 0
        let matrix = ArchetypeMatrix::build();
        for i in 0..NUM_ARCHETYPES {
            for j in 0..NUM_ARCHETYPES {
                assert_eq!(matrix.weights[i][j], -matrix.weights[j][i]);
            }
        }
    }

    #[test]
    fn scientist_read_speaks_like_scientist() {
        let s = scientist_read("C6H12O6");
        assert!(s.contains("180"), "должен назвать молярную массу: {s}");
        assert!(s.contains("C6H12O6"), "формула обязана присутствовать: {s}");
        assert!(s.contains("Трит-вердикт"));
        assert!(s.contains("ротор"));
        // Многочастная структура: минимум 4 секции
        assert!(s.lines().count() >= 4);
    }

    #[test]
    fn verdict_is_trit() {
        for text in ["H2O", "NaCl", "текст", "Fe2(SO4)3"] {
            let v = analyze(text).trit_verdict;
            assert!(v == -1 || v == 0 || v == 1, "трит-вердикт обязан быть тритом");
        }
    }
}

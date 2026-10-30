//! Фонетический пайплайн: текст (ru/uk/латиница) → сегменты звука.
//!
//! Живой голос не «читает буквы» — он артикулирует: гласные задают
//! формантные цели (архетипы), согласные — переходы и паузы щели,
//! пунктуация — фразировку и интонационный контур F0. Всё детермини-
//! ровано: сегменты — чистая функция (текст, семя).

use crate::voice::Archetype;

/// Звуковой сегмент — атомарная команда голосу.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Целевой архетип (для гласных; согласные наследуют предыдущий).
    pub arch: Archetype,
    /// Длительность (с).
    pub dur_s: f64,
    /// Амплитуда фонации (0.0 — пауза/глухой, 0.35 — звонкий согласный,
    /// 1.0 — гласный).
    pub drive: f64,
}

/// Интонация фразы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tune {
    /// Повествование: лёгкое понижение к концу.
    Statement,
    /// Вопрос: подъём к концу.
    Question,
    /// Восклицание: энергичное начало, быстрый спад.
    Exclam,
    /// Многоточие/задумчивость: медленно, приглушённо.
    Musing,
}

impl Tune {
    /// Контур F0 (множитель к базовой высоте) по прогрессу фразы [0..1].
    pub fn f0_scale(self, progress: f64) -> f64 {
        let p = progress.clamp(0.0, 1.0);
        match self {
            Tune::Statement => 1.0 + 0.10 * (1.0 - 2.0 * p),
            Tune::Question => 0.95 + 0.20 * p,
            Tune::Exclam => 1.0 + 0.15 * (1.0 - p) * (1.0 - p) - 0.08 * p,
            Tune::Musing => 0.94 - 0.04 * p,
        }
    }

    /// Множитель темпа (1.0 — обычный).
    pub fn tempo(self) -> f64 {
        match self {
            Tune::Statement => 1.0,
            Tune::Question => 1.02,
            Tune::Exclam => 1.12,
            Tune::Musing => 0.86,
        }
    }
}

/// Классифицировать гласную букву (ru/uk) в архетип.
pub fn vowel_arch(ch: char) -> Option<Archetype> {
    match ch.to_lowercase().next()? {
        'а' | 'я' => Some(Archetype::ACalm),
        'е' | 'є' | 'э' | 'ё' => Some(Archetype::ABright),
        'и' | 'і' | 'ї' | 'ы' => Some(Archetype::IDark),
        'о' => Some(Archetype::UCalm),
        'у' | 'ю' => Some(Archetype::UCalm),
        // латиница (для англ. текстов — грубая передача тембра)
        'a' => Some(Archetype::ACalm),
        'e' => Some(Archetype::ABright),
        'i' | 'y' => Some(Archetype::IDark),
        'o' | 'u' => Some(Archetype::UCalm),
        _ => None,
    }
}

/// Звонкий согласный (фонация продолжается, тракт сужен).
fn is_voiced_consonant(ch: char) -> bool {
    let c = ch.to_lowercase().next().unwrap_or(' ');
    matches!(
        c,
        'б' | 'в' | 'г'
            | 'ґ'
            | 'д'
            | 'ж'
            | 'з'
            | 'л'
            | 'м'
            | 'н'
            | 'р'
            | 'й'
            | 'b' | 'd' | 'g' | 'v' | 'z' | 'l' | 'm' | 'n' | 'r' | 'w' | 'j'
    )
}

/// Сегментировать слово (без пунктуации) в звуки.
///
/// Правила v1 (честно задокументированные упрощения):
/// - гласная: 150 мс × темп + джиттер личности;
/// - звонкий согласный: 55 мс, drive 0.35, наследует прошлый архетип
///   (или базовый в начале);
/// - глухой согласный: 45 мс тишины (щель открыта в ноль — ψ затухает);
/// - «ь/ъ» продлевают предыдущий звук на 30 мс.
pub fn segments_for_word(
    word: &str,
    prev_arch: &mut Archetype,
    tempo: f64,
    rng: &mut crate::rng::Xorshift64,
) -> Vec<Segment> {
    let mut out = Vec::new();
    let chars = word.chars().peekable();
    for ch in chars {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        if let Some(arch) = vowel_arch(lower) {
            *prev_arch = arch;
            // длительность гласной: базовая 150 мс ± 15% (от насіння слова)
            let jitter = 1.0 + 0.15 * rng.sym_milli();
            out.push(Segment {
                arch,
                dur_s: 0.150 * jitter / tempo,
                drive: 1.0,
            });
        } else if is_voiced_consonant(lower) {
            out.push(Segment {
                arch: *prev_arch,
                dur_s: 0.055 / tempo,
                drive: 0.35,
            });
        } else if lower == 'ь' || lower == 'ъ' {
            if let Some(last) = out.last_mut() {
                last.dur_s += 0.030 / tempo;
            }
        } else if lower.is_alphabetic() {
            // глухой согласный: микропауза (щель закрыта)
            out.push(Segment {
                arch: *prev_arch,
                dur_s: 0.045 / tempo,
                drive: 0.0,
            });
        }
        // не-буквы игнорируются (пунктуация обрабатывается уровнем выше)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Xorshift64;

    #[test]
    fn vowels_map_to_archetypes() {
        assert_eq!(vowel_arch('а'), Some(Archetype::ACalm));
        assert_eq!(vowel_arch('Я'), Some(Archetype::ACalm));
        assert_eq!(vowel_arch('и'), Some(Archetype::IDark));
        assert_eq!(vowel_arch('ї'), Some(Archetype::IDark));
        assert_eq!(vowel_arch('у'), Some(Archetype::UCalm));
        assert_eq!(vowel_arch('е'), Some(Archetype::ABright));
        assert_eq!(vowel_arch('к'), None);
    }

    #[test]
    fn word_segments_voiced_unvoiced() {
        let mut rng = Xorshift64::new(1);
        let mut prev = Archetype::ACalm;
        let segs = segments_for_word("мама", &mut prev, 1.0, &mut rng);
        // м(зв) а(гл) м(зв) а(гл)
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].drive, 0.35);
        assert_eq!(segs[1].drive, 1.0);
        assert_eq!(segs[3].arch, Archetype::ACalm);

        let segs2 = segments_for_word("так", &mut prev, 1.0, &mut rng);
        // т(глух) а(гл) к(глух)
        assert_eq!(segs2.len(), 3);
        assert_eq!(segs2[0].drive, 0.0);
        assert_eq!(segs2[2].drive, 0.0);
    }

    #[test]
    fn prev_arch_updates_on_vowels() {
        let mut rng = Xorshift64::new(2);
        let mut prev = Archetype::ACalm;
        let segs = segments_for_word("ми", &mut prev, 1.0, &mut rng);
        assert_eq!(segs[1].arch, Archetype::IDark);
        assert_eq!(prev, Archetype::IDark);
    }

    #[test]
    fn soft_sign_extends_previous() {
        let mut rng = Xorshift64::new(3);
        let mut prev = Archetype::ACalm;
        let segs = segments_for_word("мать", &mut prev, 1.0, &mut rng);
        // м а т ь→т продлевается
        assert_eq!(segs.len(), 3);
        assert!(segs[2].dur_s > 0.045);
    }

    #[test]
    fn tune_contours() {
        // понижение у statement
        assert!(Tune::Statement.f0_scale(1.0) < Tune::Statement.f0_scale(0.0));
        // подъём у question
        assert!(Tune::Question.f0_scale(1.0) > Tune::Question.f0_scale(0.0));
        // exclam быстрее, musing медленнее
        assert!(Tune::Exclam.tempo() > 1.0);
        assert!(Tune::Musing.tempo() < 1.0);
    }

    #[test]
    fn durations_scale_with_tempo() {
        let mut rng = Xorshift64::new(4);
        let mut prev = Archetype::ACalm;
        let s1 = segments_for_word("да", &mut prev, 1.0, &mut rng);
        let mut prev2 = Archetype::ACalm;
        let s2 = segments_for_word("да", &mut prev2, 2.0, &mut rng);
        // ускорение вдвое — глухие/звонкие короче (гласные имеют джиттер)
        assert!(s2[0].dur_s < s1[0].dur_s);
    }
}

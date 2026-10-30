//! Стемминг для веб-индекса: кириллица (укр + рос) + латиница (Snowball).
//!
//! История бага: запрос «ініціац» давал 0 хитов — в тексте «ініціація»,
//! «ініціації», «ініціацію»… Склонения славянских языков меняют окончания,
//! и точный токен-матч слеп к словоформам.
//!
//! v2.0 Foundation (2.3), принцип «100% доработать заимствованное»:
//! * кириллица — курируемый suffix-stripping с защитой минимальной длины
//!   основы (uk+ru в одном корпусе; Snowball Russian не знает укр);
//! * латиница — Snowball English из rust-stemmers (порт классического
//!   Porter2): «running»→«run», «stories»→«stori» — латинские
//!   словоформы больше не размазываются по индексу. Стеммер создаётся
//!   один раз (OnceLock) — tokenize_stem зовётся на каждый документ.
//!
//! Инварианты кириллического стеммера:
//! * основа после отрезания обязана иметь ≥ MIN_STEM символов
//!   («мати» не превращается в «ма»);
//! * до двух проходов (комбинации окончаний: «книгами» → «книг»).
//!
//! Стем — всегда ПРЕФИКС словоформы, поэтому сниппет-подстрока
//! (snippet_for ищет term внутри текста) продолжает работать:
//! «доросл» содержится в «доросла» и «дорослий».

/// Минимальная длина СЛОВА для стемминга: короткие (≤4 симв.) не трогаем —
/// там основа слишком двусмысленна («мати»≠«мат», «вік»≠«віку» терпимо).
const MIN_WORD: usize = 5;

/// Минимальная длина основы после отрезания окончания (в символах).
const MIN_STEM: usize = 3;

/// Флективные окончания укр/рос (порядок не важен — ищем самый длинный).
const CYR_SUFFIXES: &[&str] = &[
    // дієслівні (найдовші)
    "уватися", "уються", "уватиму", "увалися", "уватиме", "иться", "еться", "утися", "атся",
    "ували", "увала", "увало", "уємо",
    // іменникові відмінкові
    "еннями", "еннях", "аннях", "ового", "енями",
    "ення", "ання", "ень", "нями",
    // прикметникові
    "ими", "ыми", "ейший", "ого", "ому", "ові", "еві",
    // короткі закінчення
    "ами", "ями", "ої", "ою", "ею", "ію", "ія", "ії", "ією", "ів", "ов", "ей", "ой", "ом",
    "ем", "ам", "ям", "ах", "ях", "ую", "ий", "ый", "ая", "яя", "ое",
    "ти", "ся", "сь", "ші", "ше",
    "и", "і", "а", "я", "у", "ю", "е", "о", "ь", "є",
];
// Прошедшее время «ла/ло/ли» сознательно НЕ в списке: срезает основы
// существительных/прилагательных женского рода («доросла»→«дорос» ≠ «доросл»);
// прошедшее время унифицируется финальными гласными: «читала»/«читали»→«читал».

fn is_cyr(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

/// Snowball English (Porter2), создаётся лениво один раз на процесс.
fn snowball_en() -> &'static rust_stemmers::Stemmer {
    static STEMMER: std::sync::OnceLock<rust_stemmers::Stemmer> = std::sync::OnceLock::new();
    STEMMER.get_or_init(|| rust_stemmers::Stemmer::create(rust_stemmers::Algorithm::English))
}

/// Стемминговая форма токена: кириллица → кастомный uk+ru стеммер,
/// латиница → Snowball English (v2.0 Foundation 2.3).
pub fn stem_cyr(token: &str) -> String {
    if !token.chars().any(is_cyr) {
        // латиница: Snowball English; некириллические не-латинские
        // скрипты (CJK и пр.) Snowball не трогает — как есть
        if token.chars().all(|c| c.is_ascii_alphabetic()) {
            return snowball_en().stem(token).into_owned();
        }
        return token.to_string();
    }
    let mut s = token.to_string();
    for _ in 0..2 {
        let n = s.chars().count();
        if n < MIN_WORD {
            break;
        }
        // самый длинной суффикс, после отрезания которого основа ≥ MIN_STEM
        let mut best: &str = "";
        let mut best_len = 0usize;
        for suf in CYR_SUFFIXES {
            let sl = suf.chars().count();
            if sl <= best_len || sl >= n {
                continue;
            }
            if n - sl < MIN_STEM {
                continue;
            }
            if s.ends_with(suf) {
                best = suf;
                best_len = sl;
            }
        }
        if best.is_empty() {
            break;
        }
        s.truncate(s.len() - best.len());
    }
    s
}

/// Токенизация + стемминг: единый путь для индексации И запроса
/// (урок бага «слайд-шоу»: разные токенизаторы → ноль хитов).
/// v2.0: латиница стеммингуется Snowball English, кириллица — как раньше.
pub fn tokenize_stem(s: &str) -> Vec<String> {
    crate::web::extract::web_tokenize(s)
        .into_iter()
        .map(|t| stem_cyr(&t))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::extract::web_tokenize;

    #[test]
    fn ukrainian_inflections_unify() {
        // семейство «ініціація» — баг из полевого анализа романа
        let a = stem_cyr("ініціація");
        let b = stem_cyr("ініціації");
        let c = stem_cyr("ініціацію");
        let d = stem_cyr("ініціац");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, d);
        assert_eq!(a, "ініціац");
    }

    #[test]
    fn adjectives_and_nouns_unify() {
        assert_eq!(stem_cyr("доросла"), stem_cyr("дорослий"));
        assert_eq!(stem_cyr("доросла"), stem_cyr("дорослі"));
        assert_eq!(stem_cyr("книги"), stem_cyr("книгу"));
        assert_eq!(stem_cyr("зміни"), stem_cyr("зміною"));
        assert_eq!(stem_cyr("машина"), stem_cyr("машину"));
    }

    #[test]
    fn latin_snowball_stems() {
        // v2.0 Foundation (2.3): латиница — Snowball English (Porter2)
        assert_eq!(stem_cyr("ownership"), "ownership");
        assert_eq!(stem_cyr("running"), "run");
        assert_eq!(stem_cyr("stories"), "stori");
        assert_eq!(stem_cyr("generations"), "generat");
        // словоформы унифицируются — раньше «running»/«runs» не совпадали
        assert_eq!(stem_cyr("running"), stem_cyr("runs"));
        assert_eq!(stem_cyr("stories"), stem_cyr("story"));
        // CJK и прочие скрипты — как есть (Snowball их не покрывает)
        assert_eq!(stem_cyr("日本語"), "日本語");
    }

    #[test]
    fn short_words_protected() {
        // слова ≤ 4 символов не стеммингуются вовсе
        assert_eq!(stem_cyr("мати"), "мати");
        assert_eq!(stem_cyr("йти"), "йти");
        assert_eq!(stem_cyr("мама"), "мама");
        assert_eq!(stem_cyr("маму"), "маму");
    }

    #[test]
    fn past_tense_not_overstemmed() {
        // «ла» не режется: женские прилагательные/существительные сохраняют основу
        assert_eq!(stem_cyr("доросла"), "доросл");
        // прошедшее время унифицируется финальными гласными
        assert_eq!(stem_cyr("читала"), stem_cyr("читали"));
    }

    #[test]
    fn tokenize_stem_consistent_query_and_doc() {
        let q = tokenize_stem("доросла ініціація");
        let doc = tokenize_stem("дорослі ініціації відбулися");
        // запросные термы обязаны быть в термах документа
        for t in &q {
            assert!(doc.contains(t), "«{t}» нет в {doc:?}");
        }
        assert_eq!(tokenize_stem("Ownership Model"), {
            let raw = web_tokenize("Ownership Model");
            raw.iter().map(|t| stem_cyr(t)).collect::<Vec<_>>()
        });
    }
}

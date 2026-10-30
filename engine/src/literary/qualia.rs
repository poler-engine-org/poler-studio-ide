//! ℘ Перцепция + O Образ: текст → инвариантный вектор Ω(o) → архетипы.
//!
//! Детерминированная экстракция Qualia без RNG и без моделей: каждый
//! токен хэшируется FNV-1a64 в ось фазового пространства (модуль dims),
//! вклад токена — его «редкость» внутри текста (частые слова почти
//! невидимы, редкие — вспыхивают). Итог L2-нормируется: Ω(o) живёт на
//! единичной сфере, ‖Ω‖ = 1 — энергия замысла фиксирована входом,
//! дальнейшая динамика её только перераспределяет (Закон Сохранения
//! Смысла, см. [`super::prism`]).
//!
//! Косинусная топология (фаза O): 12 архетипов — путь героя Кэмпбелла +
//! канон POLER (Наблюдатель, Призма, Сверхпроводимость). Якорь архетипа
//! строится тем же хэшем из его лексики — наблюдение и якоря живут в
//! одном детерминированном пространстве, косинус между ними измерим
//! без эмбеддера и без сети.



/// Число архетипов канона.
pub const ARCHETYPE_COUNT: usize = 12;

/// Архетип: имя + лексика якоря (RU/EN перемешаны — полевая топология
/// языконезависима, хэш одинаково чувствителен к обеим графемам).
pub struct Archetype {
    pub name: &'static str,
    pub keywords: &'static [&'static str],
}

/// 12 архетипов: 6 пути героя (Кэмпбелл) + 3 канона POLER + 3 вечных.
pub const ARCHETYPES: [Archetype; ARCHETYPE_COUNT] = [
    Archetype {
        name: "Герой",
        keywords: &[
            "герой", "воин", "избранный", "protagonist", "hero", "путь", "поход", "quest",
            "миссия", "подвиг",
        ],
    },
    Archetype {
        name: "Тень",
        keywords: &[
            "тень", "антагонист", "враг", "тьма", "хаос", "shadow", "villain", "зло",
            "противник", "искушение",
        ],
    },
    Archetype {
        name: "Наставник",
        keywords: &[
            "наставник", "учитель", "мудрец", "mentor", "sage", "знание", "совет",
            "мастер", "школа", "урок",
        ],
    },
    Archetype {
        name: "Порог",
        keywords: &[
            "порог", "переход", "дверь", "грань", "threshold", "предел", "врата",
            "шаг", "решение", "выбор",
        ],
    },
    Archetype {
        name: "Бездна",
        keywords: &[
            "бездна", "страх", "падение", "смерть", "abyss", "провал", "боль",
            "кризис", "утрата", "ночь",
        ],
    },
    Archetype {
        name: "Возвращение",
        keywords: &[
            "возвращение", "дом", "искупление", "воскресение", "return", "прощение",
            "плоды", "дар", "победа", "рассвет",
        ],
    },
    Archetype {
        name: "Наблюдатель",
        keywords: &[
            "наблюдатель", "свидетель", "созерцатель", "observer", "взгляд", "глаз",
            "сцена", "театр", "зеркало", "отражение",
        ],
    },
    Archetype {
        name: "Призма",
        keywords: &[
            "призма", "преломление", "спектр", "prism", "поворот", "обход", "инверсия",
            "маска", "иносказание", "метафора",
        ],
    },
    Archetype {
        name: "Сверхпроводимость",
        keywords: &[
            "сверхпроводимость", "поток", "резонанс", "проводимость", "superconductivity",
            "звучание", "эхо", "волна", "гул", "синхрон",
        ],
    },
    Archetype {
        name: "Любовь",
        keywords: &[
            "любовь", "сердце", "верность", "нежность", "love", "страсть", "объятие",
            "имя", "клятва", "тоска",
        ],
    },
    Archetype {
        name: "Бунт",
        keywords: &[
            "бунт", "свобода", "революция", "шторм", "rebellion", "гнев", "протест",
            "огонь", "разлом", "ветер",
        ],
    },
    Archetype {
        name: "Тишина",
        keywords: &[
            "тишина", "покой", "стоянка", "равновесие", "silence", "молчание", "сон",
            "зима", "пепел", "пустота",
        ],
    },
];

/// FNV-1a64 (тот же канон, что в PND/Vault) — единственный источник
/// нелинейности в перцепции; больше никакой случайности нет.
#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Хэш токена → ось фазового пространства.
#[inline]
pub fn axis_of(term: &str, dims: usize) -> usize {
    (fnv1a64(term.as_bytes()) % dims as u64) as usize
}

/// Поле Qualia: наблюдение Ω(o) + паспорт перцепции.
#[derive(Clone, Debug)]
pub struct QualiaField {
    /// Число осей фазового пространства.
    pub dims: usize,
    /// Наблюдение Ω(o): L2-нормированный вектор редкости токенов.
    pub obs: Vec<f32>,
    /// Топ термов по вкладу (для объяснимости): (терм, вклад).
    pub term_mass: Vec<(String, f32)>,
    /// Всего токенов (до дедупликации).
    pub n_tokens: usize,
    /// Уникальных термов.
    pub n_terms: usize,
}

/// Минимальная длина терма (меньше — шум графем).
const MIN_TERM_LEN: usize = 2;

/// Токенизация: буквы/цифры/`_` (Unicode), нижний регистр — тот же
/// контракт, что у инвертированного индекса движка (см.
/// `tokenizer::inverted_index`), без внешних зависимостей.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(text.len() / 6 + 8);
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            for lc in ch.to_lowercase() {
                cur.push(lc);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// ℘-фаза: текст → инвариантная истина Ω(o).
///
/// Вклад терма = `1 + 1/(1 + count)` — одиночное упоминание весит 1.5,
/// каждое повторение приглушает (редкость = значимость). Поле
/// нормируется: энергия замысла ‖Ω‖ = 1.
pub fn perceive(text: &str, dims: usize) -> QualiaField {
    let dims = dims.clamp(16, 256);
    let tokens = tokenize(text);
    let n_tokens = tokens.len();
    // Частоты термов.
    let mut freq: std::collections::HashMap<&str, (usize, usize)> =
        std::collections::HashMap::new(); // (count, axis)
    for t in &tokens {
        if t.len() < MIN_TERM_LEN {
            continue;
        }
        let e = freq.entry(t.as_str()).or_insert((0, axis_of(t, dims)));
        e.0 += 1;
    }
    let n_terms = freq.len();
    let mut obs = vec![0.0f32; dims];
    let mut mass: Vec<(String, f32)> = Vec::with_capacity(n_terms);
    for (term, (count, axis)) in &freq {
        let w = 1.0 + 1.0 / (1.0 + *count as f32);
        obs[*axis] += w;
        mass.push((term.to_string(), w));
    }
    // L2-норма: энергия фиксирована.
    let n = super::linalg::norm2(&obs);
    if n > 1e-12 {
        for v in obs.iter_mut() {
            *v /= n;
        }
    }
    // Топ-термы по вкладу (детерминированный порядок: вес ↓, терм ↑).
    mass.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    mass.truncate(12);
    QualiaField {
        dims,
        obs,
        term_mass: mass,
        n_tokens,
        n_terms,
    }
}

/// Якоря архетипов для данного dims. Стоимость построения — 12 архетипов
/// × ~10 ключевых слов FNV-хэшей (~микросекунды), кэш не нужен:
/// детерминированность важнее экономии на пустяке.
fn archetype_anchors(dims: usize) -> Vec<Vec<f32>> {
    build_anchors(dims)
}

/// Якоря для произвольного dims (без кэша — для вызова ниже).
fn build_anchors(dims: usize) -> Vec<Vec<f32>> {
    ARCHETYPES
        .iter()
        .map(|a| {
            let mut v = vec![0.0f32; dims];
            for kw in a.keywords {
                v[axis_of(kw, dims)] += 1.0;
            }
            super::linalg::normalize(&v)
        })
        .collect()
}

/// O-фаза: косинусная топология архетипов.
///
/// Возвращает пары (индекс архетипа, косинус), отсортированные по
/// убыванию близости (детерминированно: вес ↓, индекс ↑).
pub fn cosine_topology(field: &QualiaField) -> Vec<(usize, f32)> {
    let anchors = archetype_anchors(field.dims);
    let mut out: Vec<(usize, f32)> = ARCHETYPES
        .iter()
        .enumerate()
        .map(|(i, _)| (i, super::linalg::cosine(&field.obs, &anchors[i])))
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    out
}

/// Имя архетипа по индексу (паника вне диапазона — программная ошибка).
pub fn archetype_name(idx: usize) -> &'static str {
    ARCHETYPES[idx].name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perception_deterministic_and_normalized() {
        let a = perceive("Муза пришла вечером и муза ушла к утру", 64);
        let b = perceive("Муза пришла вечером и муза ушла к утру", 64);
        assert_eq!(a.obs, b.obs, "перцепция детерминирована");
        assert!((super::super::linalg::norm2(&a.obs) - 1.0).abs() < 1e-5);
        assert!(a.n_tokens >= 7);
        assert!(a.n_terms >= 5);
        // Повтор «муза» приглушён относительно одиночных слов.
        let muza = a
            .term_mass
            .iter()
            .find(|(t, _)| t == "муза")
            .map(|(_, w)| *w)
            .unwrap_or(0.0);
        let vecher = a
            .term_mass
            .iter()
            .find(|(t, _)| t == "вечером")
            .map(|(_, w)| *w)
            .unwrap_or(0.0);
        assert!(muza > 0.0 && vecher > muza, "редкость весит: {muza} vs {vecher}");
    }

    #[test]
    fn empty_and_tiny_text_safe() {
        let e = perceive("", 64);
        assert!(e.obs.iter().all(|&v| v == 0.0));
        assert_eq!(e.n_tokens, 0);
        let t = perceive("a a", 64);
        assert!(t.obs.iter().all(|&v| v == 0.0), "термы < 2 символов невидимы");
        // диапазон dims клампится
        let _ = perceive("тест", 4);
        let _ = perceive("тест", 1000);
    }

    #[test]
    fn archetype_topology_ranks_semantically() {
        // Текст о герое и наставнике против бездны.
        let f = perceive(
            "герой идёт в поход против тьмы и бездны, наставник даёт совет и знание",
            64,
        );
        let top = cosine_topology(&f);
        let names: Vec<&str> = top.iter().map(|&(i, _)| archetype_name(i)).collect();
        assert!(
            names[..4].contains(&"Герой") || names[..4].contains(&"Наставник"),
            "топ-4 архетипов: {names:?} (веса {:?})",
            top[..4].iter().map(|&(_, w)| w).collect::<Vec<_>>()
        );
        assert!(top[0].1 >= top[1].1 && top[1].1 >= top[2].1, "сортировка ↓");
        // Полная тишина: текст о покое
        let q = perceive("тишина покой молчание сон зима пепел пустота", 64);
        let tq = cosine_topology(&q);
        assert_eq!(archetype_name(tq[0].0), "Тишина", "топ: {:?}", &tq[..3]);
    }

    #[test]
    fn fnv_stable() {
        // Зафиксированные значения FNV-1a64 (канон тестовых векторов).
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(axis_of("герой", 64), axis_of("герой", 64));
    }
}

//! RQ21: Грамматические мосты — синтаксическая связность речи L5.
//!
//! RQ17 научил мозг **говорить** (Born-блуждание по руслам `J`), но
//! его речь — ассоциативное облако: слова связаны смыслом, а не
//! синтаксисом. RQ21 добавляет генератору **взвешивание
//! синтаксических цепочек**: каждый токен лексикона получает класс
//! (союз, предлог, глагольная связка, знаменательное), и лотерея
//! речи повышает билеты тех классов, которых цепочка требует —
//! а зияющим облакам знаменательных слов вставляет детерминированный
//! **грамматический мост** из встроенной таблицы русского и
//! английского синтаксиса.
//!
//! ## Физика: три этажа связности
//!
//! ```text
//!   1. Взвешивание билетов: цепочка знает, что дальше уместно —
//!      союз после облака (×4), знаменательное после предлога (×3);
//!      русла J остаются единственным источником смысла, синтаксис
//!      лишь перераспределяет вероятности лотереи.
//!
//!   2. Мост из лексикона: если корпус обучал связки/союзы, они
//!      выигрывают лотерею сами — мозг говорит своей грамматикой.
//!
//!   3. Мост из таблицы: облако ≥ BRIDGE_RUN знаменательных слов
//!      подряд — генератор вставляет коннектор (и/а/но/в …,
//!      and/but/or/in …), выбранный детерминированно по состоянию
//!      цепи. Вставка — сенсорное событие: коннектор входит в кольцо
//!      гироскопа и лексикон, и повторная речь находит его уже
//!      в лотерее — грамматика прорастает в решётку.
//! ```
//!
//! Синтаксис не порождает смысл: мосты не несут знаменательного
//! содержания, они **соединяют** то, что принесли русла. Направление
//! речи по-прежнему задаётся порядком слов обучающего потока —
//! синтаксический слой лишь делает поток предложений гладким.
//!
//! ## Детерминизм
//!
//! Ни одного ГПСЧ: класс токена — чистая таблица, вес билета —
//! целочисленный множитель, выбор моста — индекс по состоянию цепи
//! (`эмиссии + длина облака` по модулю таблицы). Одинаковый вход —
//! одинаковая речь.
//!
//! ## Пример
//!
//! ```
//! use pqc::syntax_bridge::{is_cyrillic_token, syntax_class, SyntaxChain, SyntaxClass};
//!
//! // Классы: служебные слова распознаются обоими алфавитами.
//! assert_eq!(syntax_class("и"), SyntaxClass::Conjunction);
//! assert_eq!(syntax_class("через"), SyntaxClass::Preposition);
//! assert_eq!(syntax_class("является"), SyntaxClass::Copula);
//! assert_eq!(syntax_class("and"), SyntaxClass::Conjunction);
//! assert_eq!(syntax_class("фотон"), SyntaxClass::Content);
//!
//! // Цепочка: облако знаменательных слов требует моста.
//! let mut chain = SyntaxChain::new();
//! for _ in 0..4 {
//!     chain.push(SyntaxClass::Content);
//! }
//! assert!(chain.bridge_due(), "облако из 4 слов — пора моста");
//! let (bridge, class) = chain.next_bridge(is_cyrillic_token("фотон"));
//! assert_eq!(class, SyntaxClass::Conjunction, "первый мост — союз");
//! assert!(is_cyrillic_token(bridge), "кириллический контекст — русский мост");
//! ```

/// Синтаксический класс токена лексикона.
///
/// Служебные классы (`Conjunction`, `Preposition`, `Copula`) — то,
/// что ТЗ RQ21 называет грамматическими мостами: союзы, предлоги и
/// глагольные связки. `Content` — знаменательные слова: существительные,
/// прилагательные, глаголы действия — смысловая начинка речи.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxClass {
    /// Союз: `и`, `а`, `но`, `или`, `and`, `because` … — соединяет
    /// равноправные части предложения.
    Conjunction,
    /// Предлог: `в`, `на`, `через`, `in`, `of` … — вводит зависимую
    /// группу (после предлога уместно знаменательное слово).
    Preposition,
    /// Глагольная связка: `является`, `означает`, `is`, `means` … —
    /// связывает подлежащее со сказуемым.
    Copula,
    /// Знаменательное слово — весь смысл речи; всё, что не попало
    /// в служебные таблицы.
    Content,
}

/// Порог облака: столько знаменательных слов подряд без служебного —
/// генератор обязан вставить грамматический мост.
pub const BRIDGE_RUN: usize = 4;

/// Число коннекторов в таблице мостов каждого языка (RU и EN).
const BRIDGE_TABLE: usize = 8;

// ===================== Таблицы служебных слов =====================
//
// Таблицы статичны и полны для базовой связности; `matches!` по
// литералам компилируется в ведёрный сравнитель по длине строки —
// ни хешей, ни аллокаций. Регистр: токенизатор приводит к нижнему
// (слова корпуса), мосты тоже в нижнем — сравнение прямое.

/// Союз ли токен (RU + EN).
pub fn is_conjunction(t: &str) -> bool {
    matches!(
        t,
        // Русские союзы.
        "и" | "а" | "но" | "или" | "либо" | "что" | "чтобы" | "если"
            | "когда" | "пока" | "хотя" | "зато" | "однако" | "тоже"
            | "также" | "значит" | "ибо" | "поэтому" | "потому"
            | "следовательно" | "впрочем" | "причём" | "причем"
            | "тогда" | "притом"
            // Английские союзы.
            | "and" | "but" | "or" | "yet" | "so" | "because" | "if"
            | "when" | "while" | "although" | "though" | "however"
            | "therefore" | "thus" | "than" | "that" | "both" | "either"
            | "neither" | "nor" | "unless" | "whereas" | "moreover"
            | "besides" | "since"
    )
}

/// Предлог ли токен (RU + EN).
pub fn is_preposition(t: &str) -> bool {
    matches!(
        t,
        // Русские предлоги.
        "в" | "во" | "на" | "с" | "со" | "к" | "ко" | "от" | "до" | "по"
            | "за" | "из" | "у" | "о" | "об" | "обо" | "при" | "для"
            | "между" | "через" | "без" | "под" | "над" | "перед"
            | "после" | "против" | "относительно" | "вокруг" | "около"
            | "возле" | "благодаря" | "посредством" | "внутри" | "вне"
            | "вместо" | "кроме" | "среди"
            // Английские предлоги.
            | "in" | "on" | "at" | "to" | "from" | "by" | "with" | "of"
            | "for" | "into" | "onto" | "over" | "under" | "between"
            | "among" | "through" | "without" | "within" | "upon"
            | "about" | "above" | "below" | "near" | "across" | "after"
            | "before" | "during" | "until" | "against" | "along"
            | "around" | "behind" | "beyond" | "down" | "inside"
            | "outside" | "past" | "toward" | "towards" | "up" | "off"
            | "per" | "via"
    )
}

/// Глагольная связка ли токен (RU + EN).
pub fn is_copula(t: &str) -> bool {
    matches!(
        t,
        // Русские связки и связочные глаголы.
        "есть" | "является" | "являются" | "быть" | "был" | "была"
            | "было" | "были" | "будет" | "будут" | "становится"
            | "стал" | "стала" | "стало" | "стали" | "означает"
            | "составляет" | "представляет" | "состоит" | "заключается"
            | "называется" | "считается" | "приводит" | "даёт" | "дает"
            | "делает" | "имеет" | "может" | "описывает" | "определяет"
            | "образует" | "связывает" | "порождает" | "превращает"
        // Английские связки и связочные глаголы.
            | "is" | "are" | "was" | "were" | "be" | "been" | "being"
            | "am" | "becomes" | "became" | "seems" | "appears"
            | "means" | "represents" | "consists" | "remains" | "makes"
            | "made" | "has" | "have" | "had" | "gives" | "given"
            | "called" | "describes" | "defines" | "connects"
            | "creates" | "carries" | "holds"
    )
}

/// Синтаксический класс токена: союз → предлог → связка → знаменательное
/// (порядок разрешения неоднозначностей: `since`/`that` — союз).
pub fn syntax_class(t: &str) -> SyntaxClass {
    if is_conjunction(t) {
        SyntaxClass::Conjunction
    } else if is_preposition(t) {
        SyntaxClass::Preposition
    } else if is_copula(t) {
        SyntaxClass::Copula
    } else {
        SyntaxClass::Content
    }
}

/// Кириллический ли токен — выбор таблицы мостов (RU ↔ EN).
///
/// Смешанные слова считаются кириллическими: любой кириллический
/// символ отправляет мост в русскую таблицу. Латиница и цифры —
/// английская таблица (морфемы AOT — латиница).
pub fn is_cyrillic_token(t: &str) -> bool {
    t.chars()
        .any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'))
}

// ===================== Синтаксическая цепочка =====================

/// Состояние синтаксической цепочки речи: класс последней эмиссии и
/// длина текущего облака знаменательных слов.
///
/// Цепочка — рабочая память грамматики генератора: по ней лотерея
/// решает, каким классам поднять билеты, а генератор — пора ли
/// вставить мост. Обновляется каждой эмиссией (`push`), переживает
/// весь сеанс речи.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxChain {
    /// Знаменательных слов подряд с последнего служебного.
    run: usize,
    /// Класс последней эмиссии.
    last: SyntaxClass,
    /// Всего эмиссий в цепи (детерминированный индекс моста).
    emissions: usize,
    /// Пик облака за сеанс (телеметрия связности).
    max_run: usize,
}

impl Default for SyntaxChain {
    fn default() -> Self {
        SyntaxChain::new()
    }
}

impl SyntaxChain {
    /// Свежая цепочка: пустая речь, облако нулевой длины.
    pub fn new() -> SyntaxChain {
        SyntaxChain {
            run: 0,
            last: SyntaxClass::Content,
            emissions: 0,
            max_run: 0,
        }
    }

    /// Знаменательных слов подряд (текущее облако).
    pub fn run(&self) -> usize {
        self.run
    }

    /// Пик облака за сеанс.
    pub fn max_run(&self) -> usize {
        self.max_run
    }

    /// Всего эмиссий в цепи.
    pub fn emissions(&self) -> usize {
        self.emissions
    }

    /// Класс последней эмиссии.
    pub fn last_class(&self) -> SyntaxClass {
        self.last
    }

    /// Новая эмиссия: служебное слово разрывает облако, знаменательное
    /// растит его.
    pub fn push(&mut self, class: SyntaxClass) {
        self.emissions += 1;
        self.last = class;
        if class == SyntaxClass::Content {
            self.run += 1;
            if self.run > self.max_run {
                self.max_run = self.run;
            }
        } else {
            self.run = 0;
        }
    }

    /// Пора ли вставить грамматический мост: облако достигло
    /// [`BRIDGE_RUN`] знаменательных слов подряд.
    pub fn bridge_due(&self) -> bool {
        self.run >= BRIDGE_RUN
    }

    /// Целочисленный множитель билетов лотереи для кандидата класса
    /// `c` (взвешивание синтаксических цепочек — сердце RQ21).
    ///
    /// Правила естественного порядка слов:
    ///
    /// | Последняя эмиссия | Облако | Content | Conjunction | Preposition | Copula |
    /// |---|---|---|---|---|---|
    /// | content | `< BRIDGE_RUN` | ×1 | ×1 | ×2 | ×2 |
    /// | content | `≥ BRIDGE_RUN` | ×1 | **×4** | ×3 | ×3 |
    /// | союз | — | **×3** | ×1 | ×2 | ×1 |
    /// | предлог | — | **×3** | ×1 | ×1 | ×1 |
    /// | связка | — | ×2 | ×1 | ×2 | ×1 |
    ///
    /// После союза и предлога уместно знаменательное (иначе «и в и
    /// на»); зреющее облако кричит о союзе; сразу после моста
    /// знаменательное слово открывает новую группу.
    pub fn ticket_boost(&self, c: SyntaxClass) -> u32 {
        use SyntaxClass::*;
        match self.last {
            Content => {
                if self.run >= BRIDGE_RUN {
                    match c {
                        Conjunction => 4,
                        Preposition => 3,
                        Copula => 3,
                        Content => 1,
                    }
                } else {
                    match c {
                        Preposition | Copula => 2,
                        _ => 1,
                    }
                }
            }
            Conjunction => match c {
                Content => 3,
                Preposition => 2,
                Conjunction | Copula => 1,
            },
            Preposition => match c {
                Content => 3,
                Conjunction | Preposition | Copula => 1,
            },
            Copula => match c {
                Content => 2,
                Preposition => 2,
                Conjunction | Copula => 1,
            },
        }
    }

    /// Детерминированный выбор коннектора моста: `(слово, класс)`.
    ///
    /// Индекс — `(эмиссии + длина облака) mod BRIDGE_TABLE`: цепочка
    /// прогрессирует, и последовательные мосты циклируют таблицу —
    /// без ГПСЧ, без повторов подряд. Кириллический контекст берёт
    /// русскую таблицу, латиница — английскую.
    pub fn next_bridge(&self, cyrillic: bool) -> (&'static str, SyntaxClass) {
        use SyntaxClass::*;
        const RU: [(&str, SyntaxClass); BRIDGE_TABLE] = [
            ("и", Conjunction),
            ("а", Conjunction),
            ("но", Conjunction),
            ("в", Preposition),
            ("на", Preposition),
            ("или", Conjunction),
            ("является", Copula),
            ("когда", Conjunction),
        ];
        const EN: [(&str, SyntaxClass); BRIDGE_TABLE] = [
            ("and", Conjunction),
            ("but", Conjunction),
            ("or", Conjunction),
            ("in", Preposition),
            ("on", Preposition),
            ("is", Copula),
            ("when", Conjunction),
            ("so", Conjunction),
        ];
        let table: &[(&str, SyntaxClass); BRIDGE_TABLE] = if cyrillic { &RU } else { &EN };
        let idx = self.emissions.wrapping_add(self.run) % BRIDGE_TABLE;
        table[idx]
    }
}

// ===================== Тесты =====================

#[cfg(test)]
mod tests {
    use super::*;

    // ===================== Классификация =====================

    #[test]
    fn classes_russian() {
        for t in ["и", "а", "но", "или", "что", "если", "когда", "однако", "поэтому"] {
            assert_eq!(syntax_class(t), SyntaxClass::Conjunction, "{t}");
        }
        for t in ["в", "на", "с", "к", "от", "по", "из", "для", "между", "через", "при"] {
            assert_eq!(syntax_class(t), SyntaxClass::Preposition, "{t}");
        }
        for t in ["есть", "является", "означает", "состоит", "представляет", "называется"] {
            assert_eq!(syntax_class(t), SyntaxClass::Copula, "{t}");
        }
        for t in ["фотон", "решётка", "квант", "маяк", "марго", "энергия", "запутанность"] {
            assert_eq!(syntax_class(t), SyntaxClass::Content, "{t}");
        }
    }

    #[test]
    fn classes_english() {
        for t in ["and", "but", "or", "because", "if", "when", "while", "however", "that"] {
            assert_eq!(syntax_class(t), SyntaxClass::Conjunction, "{t}");
        }
        for t in ["in", "on", "at", "of", "with", "from", "through", "between", "upon"] {
            assert_eq!(syntax_class(t), SyntaxClass::Preposition, "{t}");
        }
        for t in ["is", "are", "was", "be", "means", "consists", "represents", "remains"] {
            assert_eq!(syntax_class(t), SyntaxClass::Copula, "{t}");
        }
        for t in ["photon", "lattice", "quantum", "entanglement", "energy", "momentum"] {
            assert_eq!(syntax_class(t), SyntaxClass::Content, "{t}");
        }
    }

    #[test]
    fn class_resolution_order() {
        // «since» и «that» — в таблице союзов: союз приоритетнее
        // предлога/связки при неоднозначности.
        assert_eq!(syntax_class("since"), SyntaxClass::Conjunction);
        assert_eq!(syntax_class("that"), SyntaxClass::Conjunction);
        // Неизвестное слово — знаменательное, даже похоже на служебное.
        assert_eq!(syntax_class("ина"), SyntaxClass::Content);
        assert_eq!(syntax_class("анd"), SyntaxClass::Content);
        assert_eq!(syntax_class(""), SyntaxClass::Content);
    }

    #[test]
    fn cyrillic_detection() {
        assert!(is_cyrillic_token("фотон"));
        assert!(is_cyrillic_token("Марго"));
        assert!(is_cyrillic_token("ёль"));
        assert!(!is_cyrillic_token("photon"));
        assert!(!is_cyrillic_token("and"));
        assert!(!is_cyrillic_token("fn"));
        assert!(!is_cyrillic_token("42"));
        assert!(!is_cyrillic_token(""));
    }

    // ===================== Цепочка =====================

    #[test]
    fn chain_run_and_bridge_due() {
        let mut ch = SyntaxChain::new();
        assert!(!ch.bridge_due());
        assert_eq!(ch.run(), 0);
        for k in 1..=BRIDGE_RUN {
            ch.push(SyntaxClass::Content);
            assert_eq!(ch.run(), k, "облако растёт");
            assert_eq!(ch.bridge_due(), k >= BRIDGE_RUN, "порог {k}");
        }
        // Служебное слово разрывает облако.
        ch.push(SyntaxClass::Conjunction);
        assert_eq!(ch.run(), 0);
        assert!(!ch.bridge_due());
        assert_eq!(ch.max_run(), BRIDGE_RUN, "пик облака помнит максимум");
        assert_eq!(ch.emissions(), BRIDGE_RUN + 1);
        assert_eq!(ch.last_class(), SyntaxClass::Conjunction);
    }

    #[test]
    fn chain_boost_table() {
        use SyntaxClass::*;
        let mut ch = SyntaxChain::new();
        // Свежая цепочка: облако 0 — базовые веса без крика.
        assert_eq!(ch.ticket_boost(Content), 1);
        assert_eq!(ch.ticket_boost(Preposition), 2);
        // Облако зреет: союзной связке всё нужнее.
        ch.push(Content);
        ch.push(Content);
        assert_eq!(ch.ticket_boost(Conjunction), 1);
        assert_eq!(ch.ticket_boost(Preposition), 2);
        // Облако достигло порога: союз кричит ×4.
        ch.push(Content);
        ch.push(Content);
        assert!(ch.bridge_due());
        assert_eq!(ch.ticket_boost(Conjunction), 4);
        assert_eq!(ch.ticket_boost(Preposition), 3);
        assert_eq!(ch.ticket_boost(Copula), 3);
        assert_eq!(ch.ticket_boost(Content), 1);
        // После союза — знаменательное ×3, второй союз ×1.
        ch.push(Conjunction);
        assert_eq!(ch.ticket_boost(Content), 3);
        assert_eq!(ch.ticket_boost(Conjunction), 1);
        // После предлога — знаменательное ×3.
        ch.push(Preposition);
        assert_eq!(ch.ticket_boost(Content), 3);
        assert_eq!(ch.ticket_boost(Preposition), 1);
        // После связки — знаменательное и предлог ×2.
        ch.push(Copula);
        assert_eq!(ch.ticket_boost(Content), 2);
        assert_eq!(ch.ticket_boost(Preposition), 2);
    }

    #[test]
    fn bridge_deterministic_cycle() {
        // Одинаковая цепочка — одинаковый мост (никакого ГПСЧ):
        // повторный вызов на том же состоянии даёт то же слово.
        for run in 0..BRIDGE_RUN {
            let mut c = SyntaxChain::new();
            for _ in 0..run {
                c.push(SyntaxClass::Content);
            }
            assert_eq!(c.next_bridge(true), c.next_bridge(true), "run={run}");
            assert_eq!(c.next_bridge(false), c.next_bridge(false), "run={run}");
        }
        // Кириллица — русская таблица, латиница — английская.
        let (ru, ru_class) = SyntaxChain::new().next_bridge(true);
        let (en, en_class) = SyntaxChain::new().next_bridge(false);
        assert_eq!(ru, "и");
        assert_eq!(ru_class, SyntaxClass::Conjunction);
        assert_eq!(en, "and");
        assert_eq!(en_class, SyntaxClass::Conjunction);
        assert!(is_cyrillic_token(ru));
        assert!(!is_cyrillic_token(en));
        // Цикл таблицы: рост цепи смещает индекс моста.
        let mut c = SyntaxChain::new();
        let first = c.next_bridge(true).0;
        let mut seen = vec![first];
        for _ in 0..BRIDGE_TABLE {
            c.push(SyntaxClass::Content);
            let w = c.next_bridge(true).0;
            assert!(is_cyrillic_token(w), "мост русской таблицы: {w}");
            if !seen.contains(&w) {
                seen.push(w);
            }
        }
        // За полный цикл встречаются разные мосты (таблица дышит).
        assert!(seen.len() >= 3, "цикл скукожился: {seen:?}");
        // Каждый мост таблицы — легальный служебный класс.
        let mut c2 = SyntaxChain::new();
        for _ in 0..2 * BRIDGE_TABLE {
            let (w, cl) = c2.next_bridge(true);
            assert_ne!(cl, SyntaxClass::Content, "{w} — служебное слово");
            assert_eq!(syntax_class(w), cl, "{w} согласован с классификатором");
            c2.push(SyntaxClass::Content);
        }
    }
}

//! RQ19: `pqc learn` — целенаправленный интернет-ингест.
//!
//! Соединяет говорящий мозг (RQ17) с источником знаний ([`TextSource`]):
//!
//! ```text
//! тема ──▶ раунд 1: поиск по теме ──▶ страницы ──▶ ingest
//!           (каждая страница: TF-IDF → LENS-дуги → born_step_packed4;
//!            русла J + лексикон LEXI растут автоматически)
//!              │
//!              ▼
//!         самоуправляемый раунд: топ-новое слово корпуса
//!         (модель сама уточняет, что ей искать дальше)
//!              │
//!              ▼
//!         checkpoint v4 → pqc ask отвечает выученным
//! ```
//!
//! **Модель сама формирует запросы**: после первого раунда ингеста
//! берётся самое частотное *новое* (не из темы, не стоп-слово) слово
//! корпуса — любопытство кристалла. Так раунд 2 ищет `{тема} {слово}`,
//! раунд 3 — следующее новое слово.
//!
//! `⊗_ε`-архетипы расширяются автоматически: мост RQ18 вычисляется по
//! решётке на лету — кристаллизация нового знания расширяет и
//! архетипические связи без отдельного хранилища.

use std::collections::HashMap;

use crate::generate::{GenerationReport, GeneratorConfig, L5Generator};
use crate::gyro_lattice::QuantizedGyroCurriculum;
use pqw::PqwReader;

/// Источник знаний: поиск + извлечение чистого текста.
///
/// Реализация — [`crate::wikisrc::WikiSource`]; тесты подставляют
/// синтетический источник без сети.
pub trait TextSource {
    /// Поиск: `(заголовок, сниппет)`.
    fn search(&mut self, query: &str, limit: usize) -> Result<Vec<(String, String)>, String>;
    /// Извлечение текста страниц: `(заголовок, чистый текст)`.
    /// `intro = true` — вводные секции (концентрат определений).
    fn extracts(&mut self, titles: &[String], intro: bool) -> Result<Vec<(String, String)>, String>;
}

/// Конфигурация обучения.
#[derive(Debug, Clone)]
pub struct LearnConfig {
    /// Тема обучения («Rust tokio», «квантовая механика»).
    pub topic: String,
    /// Страниц на раунд (поиск `srlimit`).
    pub pages: usize,
    /// Раундов самоуправляемого поиска (1 = только тема).
    pub rounds: usize,
    /// Полные статьи вместо вводных секций.
    pub full: bool,
    /// Сид движка (новый мозг).
    pub seed: u64,
    /// d_pol нового мозга (существующий не меняется).
    pub dim: u32,
    /// ε-порог LENS.
    pub epsilon: f32,
    /// Кольцо контекста речи.
    pub window: usize,
    /// Проверка инференса после обучения: вопрос к мозгу.
    pub ask: Option<String>,
}

impl Default for LearnConfig {
    fn default() -> Self {
        LearnConfig {
            topic: String::new(),
            pages: 5,
            rounds: 2,
            full: false,
            seed: 42,
            dim: 4096,
            epsilon: 0.05,
            window: 8,
            ask: None,
        }
    }
}

/// Итог раунда.
#[derive(Debug, Clone)]
pub struct RoundReport {
    /// Запрос раунда (тема или самоуправляемое уточнение).
    pub query: String,
    /// Найдено страниц (заголовки).
    pub titles: Vec<String>,
    /// Ингестов (страниц, ставших документами).
    pub ingested: usize,
    /// Символов чистого текста.
    pub chars: usize,
    /// Перещёлкнутых тритов born-шагом за раунд.
    pub moved: u64,
}

/// Полный отчёт `pqc learn`.
#[derive(Debug, Clone)]
pub struct LearnReport {
    pub rounds: Vec<RoundReport>,
    /// Всего страниц ингестировано.
    pub pages: usize,
    /// Всего символов.
    pub chars: usize,
    /// Лексикон до → после.
    pub lexicon_before: usize,
    pub lexicon_after: usize,
    /// Русла J до → после.
    pub channels_before: usize,
    pub channels_after: usize,
    /// Ненулевых тритов решётки после.
    pub lattice_nnz: usize,
    /// Размер контейнера v4 (Б).
    pub brain_bytes: usize,
    /// Ответ на `ask` (если запрошено).
    pub answer: Option<GenerationReport>,
}

impl LearnReport {
    /// Всего born-перещёлкиваний за все раунды.
    pub fn moved_total(&self) -> u64 {
        self.rounds.iter().map(|r| r.moved).sum()
    }
}

/// Частотный анализ корпуса раунда: топ-новые токены.
///
/// «Новое» = не входящее в тему, не стоп-слово, длина ≥ 5. Стоп-лист —
/// служебные ru/en слова, которые не несут темы.
pub fn novel_tokens(topic: &str, texts: &[String], top: usize) -> Vec<String> {
    use pqw::stream::tokenize;
    let topic_lower = topic.to_lowercase();
    let mut freq: HashMap<String, usize> = HashMap::new();
    for text in texts {
        for tok in tokenize(text) {
            let t = tok.to_lowercase();
            if t.chars().count() < 5 || is_stopword(&t) || topic_lower.contains(&t) {
                continue;
            }
            *freq.entry(t).or_insert(0) += 1;
        }
    }
    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    // Убывающая частота, лексикографический порядок при равенстве —
    // детерминизм для тестов.
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    pairs.into_iter().take(top).map(|(t, _)| t).collect()
}

/// Стоп-слова ru/en (служебные, не тематические).
fn is_stopword(t: &str) -> bool {
    const RU: &[&str] = &[
        "который", "которая", "которое", "которые", "такого", "такими", "всего", "всех",
        "может", "могут", "должен", "должны", "также", "таким", "вместе", "более", "менее",
        "очень", "когда", "где", "чтобы", "этого", "этой", "этим", "этими", "этому", "ими",
        "время", "времени", "любой", "разных", "некоторых", "например", "является", "являются",
        "называется", "называются", "области", "своей", "своих", "свои", "свою", "такое",
        "такими", "именно", "поэтому", "втором", "третьего", "между", "через", "перед",
        "после", "первый", "второй", "третий", "первые", "вторые", "будет", "будут", "было",
        "были", "быть", "имеет", "имеют", "имел", "одной", "одного", "одним", "несколько",
    ];
    const EN: &[&str] = &[
        "which", "where", "while", "other", "others", "these", "those", "there", "their",
        "about", "after", "before", "under", "between", "through", "during", "without",
        "within", "should", "would", "could", "might", "shall", "those", "first", "second",
        "third", "three", "being", "because", "however", "whether", "example", "examples",
        "called", "using", "usually", "typically", "often", "also", "such", "than", "then",
        "them", "they", "that", "this", "with", "from", "have", "has", "had", "were",
        "will", "your", "into", "some", "more", "most", "many", "much", "very", "when",
        "what", "whom", "whose", "does", "each", "both", "used", "uses", "user", "value",
    ];
    RU.contains(&t) || EN.contains(&t)
}

/// Загрузка существующего мозга (v2/v3/v4 Packed4) или создание нового.
///
/// Существующий мозг расширяется: d_pol/ε/окно берутся из контейнера,
/// лексикон и русла продолжают жить (мнение переживает рестарт).
pub fn load_or_create_brain(
    brain_bytes: Option<&[u8]>,
    cfg: &LearnConfig,
) -> Result<(QuantizedGyroCurriculum, bool), String> {
    if let Some(bytes) = brain_bytes {
        let reader = PqwReader::from_bytes(bytes)
            .map_err(|e| format!("мозг: {e}"))?;
        if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
            return Err("мозг: нужен контейнер Packed4 (v2/v3/v4)".into());
        }
        let window = reader.gyro().map(|g| g.window().max(1) as usize).unwrap_or(cfg.window);
        let eps = reader.hyperparams().epsilon_threshold;
        let mut engine = QuantizedGyroCurriculum::new(reader.d_pol(), eps, cfg.seed, window)
            .map_err(|e| e.to_string())?;
        engine.resume_from_reader(&reader).map_err(|e| e.to_string())?;
        Ok((engine, true))
    } else {
        let engine = QuantizedGyroCurriculum::new(cfg.dim, cfg.epsilon, cfg.seed, cfg.window)
            .map_err(|e| e.to_string())?;
        Ok((engine, false))
    }
}

/// Полный пайплайн обучения: раунды поиска → ингест → отчёт.
///
/// `brain_bytes` — существующий контейнер (`None` — свежий мозг).
/// Возвращает движок (для чекпоинта и инференса) и отчёт.
pub fn learn(
    cfg: &LearnConfig,
    source: &mut dyn TextSource,
    brain_bytes: Option<&[u8]>,
) -> Result<(QuantizedGyroCurriculum, LearnReport), String> {
    if cfg.topic.trim().is_empty() {
        return Err("тема пуста".into());
    }
    if cfg.pages == 0 || cfg.rounds == 0 {
        return Err("--pages и --rounds должны быть ≥ 1".into());
    }
    let (mut engine, existed) = load_or_create_brain(brain_bytes, cfg)?;
    let lexicon_before = engine.lexicon_len();
    let channels_before = engine.channel_count();
    let mut known_titles: Vec<String> = Vec::new();

    let mut rounds: Vec<RoundReport> = Vec::new();
    let mut all_pages = 0usize;
    let mut all_chars = 0usize;
    let mut corpus: Vec<String> = Vec::new();
    let mut query = cfg.topic.trim().to_string();

    for round_idx in 0..cfg.rounds {
        // Поиск (страницы, которых ещё не видели).
        let hits = source
            .search(&query, cfg.pages * 2)?
            .into_iter()
            .filter(|(t, _)| !known_titles.iter().any(|k| k == t))
            .take(cfg.pages)
            .collect::<Vec<_>>();
        let titles: Vec<String> = hits.iter().map(|(t, _)| t.clone()).collect();
        let mut report = RoundReport {
            query: query.clone(),
            titles: titles.clone(),
            ingested: 0,
            chars: 0,
            moved: 0,
        };
        if titles.is_empty() {
            rounds.push(report);
            break;
        }

        // Извлечение и ингест каждой страницы как документа.
        let pages = source.extracts(&titles, !cfg.full)?;
        for (title, text) in &pages {
            known_titles.push(title.clone());
            corpus.push(text.clone());
            let rep = engine.ingest(text, 0).map_err(|e| e.to_string())?;
            report.ingested += 1;
            report.chars += text.len();
            report.moved += rep.moved as u64;
        }
        all_pages += report.ingested;
        all_chars += report.chars;
        rounds.push(report);

        // Самоуправляемый запрос следующего раунда: топ-новое слово.
        if round_idx + 1 < cfg.rounds {
            let novel = novel_tokens(&cfg.topic, &corpus, 1);
            match novel.first() {
                Some(w) => query = format!("{} {}", cfg.topic.trim(), w),
                None => break, // нового не нашлось — дальше искать нечего
            }
        }
    }

    // Чекпоинт v4 (лексикон + русла + фазы).
    let brain_bytes = engine.checkpoint().map_err(|e| e.to_string())?;
    let brain_size = brain_bytes.len();

    // Проверка инференса: вопрос к свежеобученному мозгу.
    let answer = match cfg.ask.as_deref() {
        Some(q) if !q.trim().is_empty() => {
            let gcfg = GeneratorConfig {
                think_steps: 4,
                max_tokens: 24,
                window: engine.gyro_window(),
                seed: cfg.seed,
                syntax: true,
                focus_radius: crate::generate::DEFAULT_FOCUS_RADIUS,
                reinforce: true,
                ..GeneratorConfig::default()
            };
            let mut gen = L5Generator::new(&mut engine, gcfg).map_err(|e| e.to_string())?;
            Some(gen.generate(q).map_err(|e| e.to_string())?)
        }
        _ => None,
    };

    let _ = existed; // отчёт не различает — важен рост
    let lexicon_after = engine.lexicon_len();
    let channels_after = engine.channel_count();
    let lattice_nnz = engine.nnz();
    Ok((
        engine,
        LearnReport {
            rounds,
            pages: all_pages,
            chars: all_chars,
            lexicon_before,
            lexicon_after,
            channels_before,
            channels_after,
            lattice_nnz,
            brain_bytes: brain_size,
            answer,
        },
    ))
}

// ============================================================================
// Тесты: синтетический источник (без сети)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Синтетический источник: фиксированные страницы по запросам.
    struct Synth {
        /// запрос → страницы (заголовок, текст)
        db: Vec<(String, Vec<(String, String)>)>,
        seen_queries: Vec<String>,
    }

    impl Synth {
        fn new() -> Synth {
            let qm = "квантовая механика".to_string();
            let qm2 = "квантовая механика суперпозиция".to_string();
            // Внимание: токенизатор точен по формам — «суперпозиция» (им. падеж)
            // встречается 5 раз, «состояний» — 3: новое слово раунда 2 —
            // «суперпозиция».
            Synth {
                db: vec![
                    (
                        qm.clone(),
                        vec![
                            (
                                "Квантовая механика".into(),
                                "квантовая механика описывает микромир волновыми функциями \
                                 суперпозиция состояний суперпозиция состояний суперпозиция \
                                 энергия квантована фотон несёт квант света".into(),
                            ),
                            (
                                "Суперпозиция".into(),
                                "суперпозиция состояний означает что квантовая система \
                                 находится во всех состояниях одновременно до измерения \
                                 принцип суперпозиция основа квантовых вычислений".into(),
                            ),
                        ],
                    ),
                    (
                        qm2.clone(),
                        vec![(
                            "Квантовый компьютер".into(),
                            "квантовый компьютер использует кубиты и суперпозицию \
                             кубит может быть нулём и единицей одновременно \
                             квантовые вычисления решают задачи быстрее классики".into(),
                        )],
                    ),
                ],
                seen_queries: Vec::new(),
            }
        }
    }

    impl TextSource for Synth {
        fn search(&mut self, query: &str, _limit: usize) -> Result<Vec<(String, String)>, String> {
            self.seen_queries.push(query.to_string());
            for (q, pages) in &self.db {
                if q == query {
                    return Ok(pages
                        .iter()
                        .map(|(t, s)| (t.clone(), s.chars().take(60).collect()))
                        .collect());
                }
            }
            Ok(Vec::new())
        }

        fn extracts(&mut self, titles: &[String], _intro: bool) -> Result<Vec<(String, String)>, String> {
            Ok(self
                .db
                .iter()
                .flat_map(|(_, pages)| pages.iter())
                .filter(|(t, _)| titles.iter().any(|want| want == t))
                .cloned()
                .collect())
        }
    }

    #[test]
    fn learn_creates_brain_from_scratch() {
        let cfg = LearnConfig {
            topic: "квантовая механика".into(),
            rounds: 1,
            ask: Some("что такое суперпозиция".into()),
            ..LearnConfig::default()
        };
        let mut src = Synth::new();
        let (engine, report) = learn(&cfg, &mut src, None).unwrap();
        // Мозг вырос
        assert!(report.pages >= 2, "страницы: {}", report.pages);
        assert!(report.chars > 200);
        assert!(report.lexicon_after > 0);
        assert!(report.channels_after > 0);
        assert!(report.lattice_nnz > 0);
        assert!(report.brain_bytes > 100);
        // Чекпоинт поднимается обратно (v4 с лексиконом)
        let bytes = engine.checkpoint().unwrap();
        assert_eq!(&bytes[..8], b"POLER_Q4");
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        let mut engine2 = QuantizedGyroCurriculum::new(
            reader.d_pol(),
            0.05,
            1,
            reader.gyro().map(|g| g.window().max(1) as usize).unwrap_or(8),
        )
        .unwrap();
        engine2.resume_from_reader(&reader).unwrap();
        assert_eq!(engine2.lexicon_len(), engine.lexicon_len());
        assert_eq!(engine2.channel_count(), engine.channel_count());
        // Инференс: ответ не пуст (лексикон жив, русла есть)
        let answer = report.answer.expect("ask");
        assert!(!answer.text.is_empty(), "модель промолчала после learn");
    }

    #[test]
    fn self_directed_round_forms_query() {
        // Раунд 2 ищет «тема + топ-новое слово» — модель сама уточняет.
        let cfg = LearnConfig {
            topic: "квантовая механика".into(),
            rounds: 2,
            ..LearnConfig::default()
        };
        let mut src = Synth::new();
        let (_engine, report) = learn(&cfg, &mut src, None).unwrap();
        assert_eq!(report.rounds.len(), 2, "должно быть 2 раунда");
        assert_eq!(report.rounds[0].query, "квантовая механика");
        // Топ-новое слово корпуса — «суперпозиция» (частотное, ≥ 5 букв)
        assert_eq!(report.rounds[1].query, "квантовая механика суперпозиция");
        assert_eq!(src.seen_queries.len(), 2);
        // Второй раунд нашёл новую страницу (без дублей)
        assert_eq!(report.rounds[1].ingested, 1);
        assert_eq!(report.pages, 3);
    }

    #[test]
    fn learn_extends_existing_brain() {
        // Существующий мозг расширяется: лексикон только растёт.
        let cfg1 = LearnConfig { topic: "квантовая механика".into(), rounds: 1, ..LearnConfig::default() };
        let mut src = Synth::new();
        let (engine1, _r1) = learn(&cfg1, &mut src, None).unwrap();
        let bytes = engine1.checkpoint().unwrap();
        let lex1 = engine1.lexicon_len();

        let cfg2 = LearnConfig {
            topic: "квантовая механика".into(),
            rounds: 2,
            pages: 1,
            ask: Some("кубит".into()),
            ..LearnConfig::default()
        };
        let mut src2 = Synth::new();
        let (engine2, report) = learn(&cfg2, &mut src2, Some(&bytes)).unwrap();
        assert_eq!(report.lexicon_before, lex1);
        assert!(report.lexicon_after >= lex1, "лексикон не уменьшается");
        assert!(report.channels_after >= report.channels_before);
        // Ответ на новый вопрос по выученной теме
        let answer = report.answer.unwrap();
        assert!(!answer.text.is_empty());
        let _ = engine2;
    }

    #[test]
    fn novel_tokens_filtering() {
        // Суперпозиция ×4 > кубит ×3; «который»/«example» — стоп-слова;
        // слова темы отфильтрованы; короткие (<5) не проходят.
        let texts = vec![
            "суперпозиция суперпозиция суперпозиция суперпозиция состояний кубит".to_string(),
            "кубит кубит фотон энергия квантована".to_string(),
        ];
        let top = novel_tokens("квантовая механика", &texts, 3);
        assert_eq!(top[0], "суперпозиция");
        assert!(top.contains(&"кубит".to_string()));
        assert!(!top.iter().any(|t| t == "квантовая"));
        assert!(!top.iter().any(|t| t == "который" || t == "example"));
        assert!(!top.iter().any(|t| t.chars().count() < 5));
        // Ничья частот решается лексикографически (детерминизм)
        let tie = vec!["яberry черешня".to_string(), "яблоко яberry черешня".to_string()];
        let t = novel_tokens("тема", &tie, 1);
        assert_eq!(t.len(), 1); // яberry ×2 == черешня ×2 → меньшее лексикографически
    }

    #[test]
    fn empty_topic_rejected() {
        let cfg = LearnConfig { topic: "  ".into(), ..LearnConfig::default() };
        let mut src = Synth::new();
        assert!(learn(&cfg, &mut src, None).is_err());
    }

    #[test]
    fn no_pages_is_not_error() {
        // Пустой поиск — честный отчёт без раундов, мозг жив.
        struct Empty;
        impl TextSource for Empty {
            fn search(&mut self, _: &str, _: usize) -> Result<Vec<(String, String)>, String> {
                Ok(Vec::new())
            }
            fn extracts(&mut self, _: &[String], _: bool) -> Result<Vec<(String, String)>, String> {
                Ok(Vec::new())
            }
        }
        let cfg = LearnConfig { topic: "несуществующая тема".into(), ..LearnConfig::default() };
        let (_e, report) = learn(&cfg, &mut Empty, None).unwrap();
        assert_eq!(report.pages, 0);
        assert_eq!(report.lexicon_after, 0);
    }
}

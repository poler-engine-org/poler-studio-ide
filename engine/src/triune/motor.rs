//! S2 моторный слой: речь → операторы MindOS → интенты poler_exec.
//!
//! Последняя миля Триединства: сгенерированный кристаллом текст
//! сканируется на повелительные конструкции («открой терминал»,
//! «запусти сборку»). Найденное отображается в **предложение**
//! [`MotorIntent`] — готовую команду для агента. Слой НИКОГДА не
//! исполняет сам: контроль остаётся за ИИ-агентом (директива
//! «удобство для агентов и только»), деструктивные глаголы
//! отклоняются белым списком.
//!
//! ## Безопасность
//!
//! - Глаголы вне белого списка → `allowed = false`, команда не
//!   рендерится.
//! - Деструктивная лексика (удалить/стереть/форматировать/…) —
//!   приоритетнее белого списка: `refused`.
//! - Все интенты — строки-предложения: агент решает, звать ли
//!   `poler_exec` (MCP-семейство E2/v0.32.0).

/// Оператор моторного слоя.
#[derive(Clone, Debug, PartialEq)]
pub enum MotorOp {
    /// Открыть приложение/путь.
    Open,
    /// Запустить процесс/сборку/тесты.
    Run,
    /// Показать статус/логи/информацию.
    Show,
    /// Прочитать файл.
    Read,
    /// Собрать релиз.
    Build,
    /// Неисполнимое предложение (текст без оператора).
    None,
}

impl MotorOp {
    /// Строка оператора для рендера.
    pub fn as_str(&self) -> &'static str {
        match self {
            MotorOp::Open => "open",
            MotorOp::Run => "run",
            MotorOp::Show => "show",
            MotorOp::Read => "read",
            MotorOp::Build => "build",
            MotorOp::None => "none",
        }
    }
}

/// Моторный интент: предложение агенту, НЕ команда к исполнению.
#[derive(Clone, Debug)]
pub struct MotorIntent {
    /// Исходный глагол (как прозвучал в речи).
    pub verb: String,
    /// Оператор.
    pub op: MotorOp,
    /// Объект действия (до 3 слов после глагола).
    pub object: Vec<String>,
    /// Отрендеренное предложение для poler_exec (пусто, если запрещено).
    pub proposal: String,
    /// Разрешено ли к предложению агенту.
    pub allowed: bool,
    /// Причина решения (прозрачность для агента).
    pub reason: String,
}

/// Белый список глаголов → оператор (RU + UA: кристалл говорит на смеси).
const WHITELIST: &[(&[&str], MotorOp)] = &[
    (&["открой", "открыть", "открывает", "відкрий", "відкрити", "відкриває"], MotorOp::Open),
    (&["запусти", "запустить", "запускает", "запустити", "запускає"], MotorOp::Run),
    (&["покажи", "показать", "показывает", "показати", "показує"], MotorOp::Show),
    (&["прочитай", "прочитать", "читает", "прочитати", "читає"], MotorOp::Read),
    (&["собери", "собрать", "собирает", "збери", "зібрати", "збирає"], MotorOp::Build),
];

/// Деструктивная лексика (проверяется ПЕРЕД белым списком; RU + UA).
const DESTRUCTIVE: &[&str] = &[
    "удали", "удалить", "удалит", "сотри", "стереть", "сотрёт",
    "уничтожь", "уничтожить", "форматировать", "отформатируй",
    "видали", "видалити", "видалить", "зітри", "зтерти",
    "rm", "sudo", "wipe", "drop",
];

/// Слова, обрывающие объект действия (союзы/предлоги/следующий глагол; RU + UA).
const OBJECT_STOP: &[&str] = &[
    "и", "а", "но", "или", "потом", "затем", "что",
    "в", "на", "с", "по", "из", "от", "до", "за", "для", "о", "к", "при",
    "і", "й", "та", "або", "потім", "що", "у", "з", "із", "від",
    "але",
];

/// Все распознаваемые глаголы белого списка (для обрыва объекта).
fn whitelist_verbs() -> Vec<&'static str> {
    WHITELIST.iter().flat_map(|(forms, _)| forms.iter().copied()).collect()
}

/// Скан речи на моторные интенты. Детерминирован: тот же текст →
/// те же интенты в порядке появления.
pub fn scan(text: &str) -> Vec<MotorIntent> {
    let words = crate::triune::crystal::tokenize(text);
    let verbs = whitelist_verbs();
    let mut out = Vec::new();
    for (i, w) in words.iter().enumerate() {
        // 1) Деструктивный глагол → отказ (даже если объект белый).
        if DESTRUCTIVE.contains(&w.as_str()) {
            out.push(MotorIntent {
                verb: w.clone(),
                op: MotorOp::None,
                object: Vec::new(),
                proposal: String::new(),
                allowed: false,
                reason: "деструктивный глагол вне белого списка".into(),
            });
            continue;
        }
        // 2) Белый глагол → предложение.
        for (forms, op) in WHITELIST {
            if forms.contains(&w.as_str()) {
                let mut object: Vec<String> = Vec::new();
                for next in words[i + 1..].iter().take(3) {
                    if OBJECT_STOP.contains(&next.as_str()) || verbs.contains(&next.as_str()) {
                        break; // объект кончился — дальше союз или новый глагол
                    }
                    object.push(next.clone());
                }
                let proposal = if object.is_empty() {
                    format!("poler_exec {}: объект не указан", op.as_str())
                } else {
                    format!("poler_exec {} -- {}", op.as_str(), object.join(" "))
                };
                out.push(MotorIntent {
                    verb: w.clone(),
                    op: op.clone(),
                    object,
                    proposal,
                    allowed: true,
                    reason: "белый список моторного слоя".into(),
                });
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_renders_proposal() {
        let intents = scan("открой терминал и покажи статус системы");
        assert_eq!(intents.len(), 2);
        assert_eq!(intents[0].op, MotorOp::Open);
        assert_eq!(intents[0].object, vec!["терминал"], "союз «и» обрывает объект");
        assert_eq!(intents[0].proposal, "poler_exec open -- терминал");
        assert_eq!(intents[1].op, MotorOp::Show);
        assert_eq!(intents[1].object, vec!["статус", "системы"]);
        assert_eq!(intents[1].proposal, "poler_exec show -- статус системы");
        assert!(intents.iter().all(|i| i.allowed));
    }

    #[test]
    fn destructive_refused() {
        let intents = scan("удалить всё и открыть терминал");
        assert_eq!(intents.len(), 2);
        assert!(!intents[0].allowed, "деструктив должен отклоняться");
        assert!(intents[0].proposal.is_empty());
        assert_eq!(intents[0].op, MotorOp::None);
        assert!(intents[1].allowed, "белый глагол в том же тексте жив");
    }

    #[test]
    fn unknown_verb_is_not_an_intent() {
        assert!(scan("система думает о смысле").is_empty());
        assert!(scan("").is_empty());
    }

    #[test]
    fn scan_is_deterministic() {
        let a = scan("запусти сборку проекта покажи логи");
        let b = scan("запусти сборку проекта покажи логи");
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.proposal, y.proposal);
            assert_eq!(x.op, y.op);
        }
    }

    #[test]
    fn infinitive_forms_match() {
        for verb in ["открой", "открыть", "открывает"] {
            let intents = scan(&format!("{verb} редактор"));
            assert_eq!(intents.len(), 1, "{verb} должен распознаваться");
            assert_eq!(intents[0].op, MotorOp::Open);
        }
    }

    #[test]
    fn ukrainian_verbs_match() {
        // UA-формы: кристалл говорит на смеси RU/UA — белый список двуязычный (v0.38.1)
        let intents = scan("відкрий термінал і покажи статус");
        assert_eq!(intents.len(), 2, "UA-глаголы должны распознаваться");
        assert_eq!(intents[0].op, MotorOp::Open);
        assert_eq!(intents[0].object, vec!["термінал"]);
        assert_eq!(intents[1].op, MotorOp::Show);
        assert_eq!(intents[1].object, vec!["статус"], "UA-союз «і» обрывает объект");

        let build = scan("збери проєкт");
        assert_eq!(build.len(), 1);
        assert_eq!(build[0].op, MotorOp::Build);

        let run = scan("запусти збірку");
        assert_eq!(run.len(), 1);
        assert_eq!(run[0].op, MotorOp::Run);
        assert_eq!(run[0].object, vec!["збірку"]);
    }

    #[test]
    fn ukrainian_destructive_refused() {
        let intents = scan("видали файл і відкрий термінал");
        assert_eq!(intents.len(), 2);
        assert!(!intents[0].allowed, "UA-деструктив должен отклоняться");
        assert!(intents[0].proposal.is_empty());
        assert!(intents[1].allowed, "белый UA-глагол в том же тексте жив");
    }
}

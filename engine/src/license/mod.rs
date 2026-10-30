//! License / EULA-модуль POLER Engine (v2.0, sovereign stack).
//!
//! v2.0: Ed25519 License Gate УДАЛЁН вместе с Google/NotebookLM-интеграциями,
//! которые он гейтил (Gmail / Drive / NLM). Локальный поиск, резонанс, AIDDE,
//! граф, TUI/shell, MCP, краулинг — работают всегда и без ключей: гейтить
//! больше нечего, «кирпич» невозможен по построению.
//!
//! Осталась единственная функция модуля — юридическая информация о модели
//! лицензирования (Source-Available EULA, см. LICENSE.md / TERMS.md) и
//! обязательство раскрытия модификаций (Notification Clause §4). Это
//! метаданные для человека, а не механизм блокировки.
//!
//! Принцип «инструмент, не ИИ» соблюдён: модуль ничего не решает,
//! ничего не блокирует — только отдаёт текст.

use std::path::PathBuf;

/// Название лицензионной модели (LICENSE.md — юридический инструмент).
pub const EULA_NAME: &str =
    "POLER Custom Source-Available & Modification Disclosure License v1.0";
/// Обязательный адрес раскрытия модификаций (Notification Clause, §4).
pub const MODIFICATION_NOTICE_EMAIL: &str = "dev@poler-engine.org";
/// Официальный репозиторий (для ссылок в баннере/CLI).
pub const EULA_REPO_URL: &str = "https://github.com/poler-engine-org/poler-engine";

/// Каталог конфигурации движка (`~/.config/poler-engine/`).
/// v2.0: локальная копия бывшей `google::config_dir()` — путь общий,
/// к облачным сервисам отношения не имеет.
pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("poler-engine")
}

/// ЕДИНЫЙ блок EULA-статуса: печатается в `--license`, в gateway-баннере
/// и в команде `license` REPL (один источник правды — no drift).
///
/// Модель: Source-Available (Unreal Engine EULA precedent) — исходники
/// открыты для изучения/сборки/модификации, но модификации при
/// дистрибуции/деплое продукта подлежат обязательному уведомлению
/// авторов (14 дней), редистрибуция ядра запрещена.
pub fn eula_notice() -> String {
    let mut s = String::new();
    s.push_str(&format!("  Модель:       {EULA_NAME}\n"));
    s.push_str(&format!(
        "  Раскрытие:    модификации при дистрибуции/деплое — уведомить\n                {} в течение 14 дней (Notification Clause)\n",
        MODIFICATION_NOTICE_EMAIL
    ));
    s.push_str("  Условия:      LICENSE.md · TERMS.md (в корне репозитория)\n");
    s.push_str(&format!("  Репозиторий:  {EULA_REPO_URL}\n"));
    s.push_str("  Запрещено:    редистрибуция ядра\n");
    s
}

/// Короткая строка для баннера запуска (одна строка, без блоков).
pub fn eula_banner_line() -> String {
    format!(
        "Лицензия: {EULA_NAME} — модификации подлежат раскрытию → {MODIFICATION_NOTICE_EMAIL} (TERMS.md)"
    )
}

/// Полный текст статуса лицензии как String: один источник для CLI
/// `--license` и команды `license` в Terminal Gateway.
///
/// v2.0: тиров и ключей больше нет — статус описывает модель лицензии
/// и фиксирует, что все функции движка локальны и свободны.
pub fn status_text() -> String {
    let mut s = String::from("POLER Engine — лицензия\n\n");
    s.push_str("  Тир:         Sovereign (v2.0 — без ключей и гейтов)\n");
    s.push_str("  Функции:     локальный поиск, grep, чанки, краулинг, MCP,\n");
    s.push_str("               AIDDE, граф, TUI/shell — без ограничений\n");
    s.push_str("  Облако:      НЕТ (Google/NotebookLM/Gmail/Drive удалены\n");
    s.push_str("               в v2.0 — суверенный стек)\n\n");
    s.push_str(&eula_notice());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_line_mentions_eula_and_email() {
        let line = eula_banner_line();
        assert!(line.contains(EULA_NAME));
        assert!(line.contains(MODIFICATION_NOTICE_EMAIL));
    }

    #[test]
    fn status_text_is_sovereign_and_lists_eula() {
        let s = status_text();
        assert!(s.contains("Sovereign"), "v2.0: тиров больше нет");
        assert!(s.contains(EULA_NAME));
        assert!(s.contains(EULA_REPO_URL));
        assert!(!s.contains("Community-лимит"), "гейт удалён");
    }

    #[test]
    fn config_dir_is_poler_scoped() {
        let d = config_dir();
        let s = d.to_string_lossy().to_string();
        assert!(s.contains("poler-engine"));
        assert!(!s.contains("google"), "v2.0: путь не должен упоминать google");
    }
}

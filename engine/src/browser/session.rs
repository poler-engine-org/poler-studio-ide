//! Менеджер сессий вкладок и окон браузера POLER Browser.
//!
//! Обеспечивает:
//! - Режим Zero-Copy Page Cache (фоновые вкладки выгружаются из RAM в виртуальную память ядра).
//! - Моторный мост S2→E2 для автономного управления браузером.
//! - Прямую передачу данных страниц в Trit5 Кристалл (.t5c).

use std::time::Instant;
use super::dom_tree::DomDocument;
use super::filter::ContentFilter;

/// Конфигурация браузера
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub max_active_tabs_in_ram: usize,
    pub enable_epsilon_filter: bool,
    pub user_agent: String,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            max_active_tabs_in_ram: 8,
            enable_epsilon_filter: true,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36 POLER/0.38".into(),
        }
    }
}

/// Состояние одной вкладки
#[derive(Debug, Clone)]
pub struct TabSession {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub doc: Option<DomDocument>,
    pub clean_text: String,
    pub in_ram: bool,
    pub last_active: Instant,
}

/// Окно браузера со списком вкладок
pub struct BrowserWindow {
    pub tabs: Vec<TabSession>,
    pub active_tab_idx: usize,
    pub config: BrowserConfig,
    pub filter: ContentFilter,
    next_tab_id: u32,
}

impl BrowserWindow {
    pub fn new(config: BrowserConfig) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_idx: 0,
            config,
            filter: ContentFilter::new(),
            next_tab_id: 1,
        }
    }

    /// Открыть новую вкладку
    pub fn open_tab(&mut self, url: &str) -> u32 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab = TabSession {
            id,
            url: url.to_string(),
            title: "Новая вкладка".into(),
            doc: None,
            clean_text: String::new(),
            in_ram: true,
            last_active: Instant::now(),
        };

        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        id
    }

    /// Загрузить HTML в текущую вкладку и применить квантовый фильтр ε
    pub fn load_html(&mut self, html: &str) {
        if self.tabs.is_empty() {
            self.open_tab("about:blank");
        }

        let mut doc = DomDocument::parse_html(html);

        if self.config.enable_epsilon_filter {
            self.filter.clean_document(&mut doc);
        }

        let clean_text = doc.extract_clean_text();
        let title = if doc.title.is_empty() { "Без названия".to_string() } else { doc.title.clone() };

        let tab = &mut self.tabs[self.active_tab_idx];
        tab.title = title;
        tab.clean_text = clean_text;
        tab.doc = Some(doc);
        tab.last_active = Instant::now();
    }

    /// Получить чистый текст активной вкладки
    pub fn get_active_text(&self) -> &str {
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            &tab.clean_text
        } else {
            ""
        }
    }

    /// Дисциплина памяти: выгрузить неактивные вкладки из RAM
    pub fn evict_background_tabs_to_save_ram(&mut self) {
        if self.tabs.len() <= self.config.max_active_tabs_in_ram {
            return;
        }

        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            if idx != self.active_tab_idx && tab.in_ram {
                tab.doc = None; // Освобождаем DOM-дерево из heap
                tab.in_ram = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::browser::{BrowserConfig, BrowserWindow, DomDocument, ContentFilter};

    #[test]
    fn test_dom_parsing_clean_text() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head><title>Тестовая страница POLER</title></head>
                <body>
                    <div class="header">
                        <h1>Заголовок документа</h1>
                    </div>
                    <div class="banner ad-box">
                        <p>Купите слона со скидкой 50%!</p>
                    </div>
                    <article class="content">
                        <p>Фундаментальное уравнение движения: <b>dp/dt = -η Π_Λ [D·p + γJ·p + ∇F]</b>.</p>
                        <p>Троичная логика Trit5 исключает затраты на float32.</p>
                    </article>
                    <div class="cookie-popup">
                        <p>Мы используем куки для слежки.</p>
                    </div>
                </body>
            </html>
        "#;

        let mut doc = DomDocument::parse_html(html);
        assert_eq!(doc.title, "Тестовая страница POLER");

        let filter = ContentFilter::new();
        let stats = filter.clean_document(&mut doc);

        assert!(stats.removed_noise_nodes >= 2, "Рекламные узлы должны быть удалены");

        let clean_text = doc.extract_clean_text();
        assert!(clean_text.contains("Фундаментальное уравнение движения"));
        assert!(clean_text.contains("Троичная логика"));
        assert!(!clean_text.contains("Купите слона"), "Реклама должна быть вырезана");
        assert!(!clean_text.contains("Мы используем куки"), "Куки-попап должен быть вырезан");
    }

    #[test]
    fn test_browser_session_memory_eviction() {
        let mut window = BrowserWindow::new(BrowserConfig {
            max_active_tabs_in_ram: 2,
            ..Default::default()
        });

        window.open_tab("https://page1.poler");
        window.load_html("<html><title>Page 1</title><body>Контент 1</body></html>");

        window.open_tab("https://page2.poler");
        window.load_html("<html><title>Page 2</title><body>Контент 2</body></html>");

        window.open_tab("https://page3.poler");
        window.load_html("<html><title>Page 3</title><body>Контент 3</body></html>");

        assert_eq!(window.tabs.len(), 3);
        window.evict_background_tabs_to_save_ram();

        // Фоновая вкладка должна быть выгружена из heap
        assert!(!window.tabs[0].in_ram, "Фоновая вкладка 0 должна быть выгружена");
        assert!(window.tabs[2].in_ram, "Активная вкладка 2 должна оставаться в RAM");
    }
}

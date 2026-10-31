// Z.ai Ultimate Suite: Content Script
(function() {
    console.log("[Z.ai Suite] Initializing POLER extension features...");

    // 1. Снятие лимитов на вставку длинного текста
    document.addEventListener('paste', function(e) {
        e.stopImmediatePropagation();
    }, true);

    // 2. Умное обновление заголовка вкладки по теме чата
    function updateTabTitle() {
        const titleElem = document.querySelector('h1, .chat-title, [class*="title"], [class*="header"]');
        if (titleElem && titleElem.innerText.trim().length > 0) {
            const topic = titleElem.innerText.trim();
            if (!document.title.startsWith(topic)) {
                document.title = `${topic} — Z.ai`;
            }
        }
    }
    setInterval(updateTabTitle, 3000);

    // 3. Плавающая кнопка "Экспорт в Markdown"
    function injectExportButton() {
        if (document.getElementById('zai-export-btn')) return;

        const btn = document.createElement('button');
        btn.id = 'zai-export-btn';
        btn.innerHTML = '📥 Экспорт в Markdown';
        btn.style.position = 'fixed';
        btn.style.bottom = '20px';
        btn.style.right = '20px';
        btn.style.zIndex = '999999';
        btn.style.padding = '10px 16px';
        btn.style.background = '#2563eb';
        btn.style.color = '#ffffff';
        btn.style.border = 'none';
        btn.style.borderRadius = '8px';
        btn.style.cursor = 'pointer';
        btn.style.fontWeight = 'bold';
        btn.style.boxShadow = '0 4px 12px rgba(0,0,0,0.3)';

        btn.onclick = function() {
            let md = `# Диалог Z.ai (${window.location.href})\nДата: ${new Date().toLocaleString()}\n\n`;
            const messages = document.querySelectorAll('[class*="message"], [class*="chat-item"], .prose');
            messages.forEach((msg, idx) => {
                const text = msg.innerText.trim();
                if (text) {
                    md += `### Сообщение ${idx + 1}\n\n${text}\n\n---\n\n`;
                }
            });
            const blob = new Blob([md], { type: 'text/markdown;charset=utf-8;' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `zai_chat_${Date.now()}.md`;
            a.click();
            URL.revokeObjectURL(url);
        };

        document.body.appendChild(btn);
    }

    setTimeout(injectExportButton, 2000);
})();

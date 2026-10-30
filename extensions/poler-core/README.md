# POLER Studio Core & Engine Bridge (poler-core)

Суверенное управляющее ядро **POLER Studio. IDE**, реализующее глубокую интеграцию нативного движка **POLER[Ψ]** (`poler-engine` v0.93.0) и **POLER Edit** (`poler-edit-core` v0.4.1 piece-table) непосредственно в интерфейс редактора.

---

## ⚡ Архитектура и Возможности

### 1. Causal Nexus & AIDDE Impact (`Alt+F12`)
- Мгновенный граф зависимостей любого символа, функции или структуры с оценкой риска изменения (`CRITICAL`, `HIGH`, `MEDIUM`, `LOW`).
- Определение корневых дефиниций, восходящих вызовов (Upstream Callers) и нисходящих зависимостей (Downstream Callees).
- Интегрированное дерево зависимостей во вкладке **POLER Studio** на панели Activity Bar.

### 2. Квантово-резонансный топографический поиск (Ψ) (`Alt+S`)
- Семантический поиск по кодовой базе с вычислением свободной энергии $\varepsilon$ и резонанса $\text{Res}$.
- Мгновенная навигация по ключевым семантическим якорям (Anchors).

### 3. AI Context Generator (Feed: Understand / Edit)
- Сверхсжатый архитектурный контекст для LLM-агентов без засорения контекстного окна токенами.
- Формирование правочных пакетов с точечным срезом затронутых зон.

### 4. Научная Студия (SciLang + 3D Хранилище Формул) (`Ctrl+Alt+C`)
- Встроенная интерактивная лаборатория для вычислений и моделирования.
- Вычисление физических формул с единицами измерения СИ-2019, астрономических констант, матриц и сложных полиномов на нативном движке без Python.
- Вставка вычисленных формул и результатов в один клик в активный документ.

### 5. Нативный Reference Provider
- Заменяет медленный глобальный ripgrep прямым запросом к `poler-engine impact`, предоставляя мгновенные ссылки на символы по всей кодовой базе в $O(1)$.

### 6. Нативная Piece-Table для больших файлов (`poler-edit` > 50 МБ)
- Файлы крупнее `poler.editCore.largeFileThresholdMB` (по умолчанию 50 МБ) открываются через сервер `poler-edit-core --edit-serve` (Zero-Copy Piece-Table поверх mmap) вместо полной материализации в построчный массив Monaco.
- Виртуальная схема `poler-edit://` рендерит ограниченное окно строк (`viewport`), поэтому многогигабайтные дампы и логи открываются мгновенно.
- Команды навигации: **Go To Line / байтовое смещение** (`Ctrl+Alt+G`), **нативный поиск в файле** Aho-Corasick (`Ctrl+Alt+F`), следующее/предыдущее окно строк, сохранение нативных правок.
- Прозрачная LSP-трансляция координат: смещение в байтах ↔ строка/колонка (команды протокола `goto`/`linecol`, колонки — в символах, как в Language Server Protocol).
- Автоматическое восстановление: при падении сервера клиент прозрачно перезапускает процесс и переоткрывает документы.

### 7. Суверенный шлюз Copilot (Stage C)
- **PAT-fallback авторизация:** при отклонении локальной OAuth-сессии (401/403 из `state.vscdb`) провайдер GitHub-аутентификации автоматически переключается на сервисный Personal Access Token (`Authorization: Bearer <GITHUB_TOKEN>`, переменные окружения `GITHUB_TOKEN` / `GH_TOKEN` / `GITHUB_PAT`).
- **Локальный LLM-бэкенд:** переключатель `poler.copilot.modelSource` (`copilot` ↔ `local`) поднимает OpenAI-совместимый шлюз на `127.0.0.1:8770`, транслирующий `/v1/chat/completions` в нативный резонансный инференс `poler-engine --llm local --model <glm.pqw>` (или `--llm quantum` без `.pqw`). Скопируйте BYOK Custom Endpoint JSON одной кнопкой для подключения GitHub Copilot к локальной модели.

---

## ⚙️ Бинарные Мосты

Расширение автоматически обнаруживает и подключает нативные бинарники:
- `poler-engine.exe` (`C:\Users\vital\bin\poler-engine.exe`, `~/.local/bin/poler-engine`)
- `poler-edit-core.exe` (`C:\Users\vital\bin\poler-edit-core.exe`)
- Либо локальные копии из директории `resources/bin/`.

При необходимости путь к бинарникам можно настроить в `settings.json`:
```json
{
  "poler.enginePath": "C:\\Users\\vital\\bin\\poler-engine.exe",
  "poler.editCorePath": "C:\\Users\\vital\\bin\\poler-edit-core.exe",
  "poler.resonanceMode": "psi",
  "poler.editCore.largeFileThresholdMB": 50,
  "poler.editCore.windowLines": 2048,
  "poler.copilot.modelSource": "copilot",
  "poler.copilot.localModelPath": "C:\\models\\glm.pqw"
}
```

# AGENT.md — протокол агента этого репозитория

Канонический протокол **Context-Free Resilience** живёт в
POLER-Quantum-RS v0.3.8: [`../poler-quantum-rs/AGENT.md`](https://github.com/poler-engine-org/POLER-Quantum-RS/blob/main/AGENT.md)
(иерархия истины, персистентный tmux-слой, git-first, гигиена токенов,
холодный старт фаз 0–4, bootstrap-скрипт). Этот файл — обязательный минимум
для агентов, работающих с poler-engine:

1. **Первым делом** прочитать `AGENT_STATE.md` в корне этого репозитория —
   вектор движения (current_task → next_task, blocked_on).
2. **Fetch-before-work**: `git fetch origin --tags` до начала любой работы;
   расхождения local/remote разрешать по §5 канона (слепой merge запрещён,
   force-push — только с разрешения пользователя).
3. Длительные задачи (>60 с: `cargo test`, `cargo build --release`, PTY-смоуки)
   — только в именованной tmux-сессии `poler-engine-<задача>`; опрос без
   блокировки: `tmux capture-pane -p -t <имя> -S -100`.
4. Значимые изменения — **немедленный коммит**; после зелёных тестов — push;
   в конце сессии `git log origin/main..HEAD` пуст.
5. Токены — только через credential-helper по stdin; никогда в remote-URL,
   коммитах, логах и ответах (маска `ghp_****`). Опубликованный в чате токен
   считается скомпрометированным — предупредить об отзыве.
6. Контекстное окно чата — не источник фактов о состоянии; истина — git,
   файловая система, процессы, tmux-сессии.

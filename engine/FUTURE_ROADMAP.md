# FUTURE_ROADMAP — «Превзойти Google»

> **Статус: ЗАПИСАНО НА БУДУЩЕЕ. Срок реализации НЕ ОПРЕДЕЛЁН.**
> Дата фиксации цели: 2026-08-25 (сессия poler-engine v0.10.0).
> Формулировка владельца: *«цель превзойти гугл, максимальное обилие сервисов
> помимо ютуба»*. Это не задача следующего релиза — это направление движения
> проекта на годы. Отсюда берём приоритеты, когда появляется свободный ресурс.

---

## 1. Декларация цели

poler-engine развивается не как «ещё один grep», а как **суверенный стек
поиска и сервисов**, не зависящий от чужих API, квот и ToS. Конечная цель —
по богатству сервисов превзойти Google-экосистему (Search, YouTube,
Рекомендации, Карты контента), оставаясь при этом запускаемым на обычном
домашнем ПК под Linux. Каждый релиз движка — кирпич в этом направлении:

| Уже есть в poler-engine (v0.11.0) | Соответствие «большому Google» |
|---|---|
| --web-search, WebRank (BM25+PageRank) | Google Search (ядро поиска) |
| --crawl, robots.txt, sitemap, SimHash | Googlebot (обход) |
| Фразовый поиск с позиционным индексом | Точные цитаты `"..."` в Google |
| MCP-сервер (4 инструмента) | «Поиск как API» для любого LLM-агента |
| ε/R/сцены, K-hop графы, Cross-Universe | Knowledge Graph + «со-прос-граф» |
| Стемминг укр/рос | Морфология (в упрощённом виде) |

## 2. Донорские технологии YouTube-архитектуры (украдено и записано)

Разбор «как YouTube тянет 2.5 млрд пользователей без GPU» — референс для
будущих витков. Всё ниже — открытые исходники, которые можно пиздить в
движок по мере надобности (политика проекта: «пиздим технологии и пообольше,
потом дорабатываем если хреновые»):

1. **TensorFlow Recommenders (TFRS)** — github.com/tensorflow/recommenders
   Двухбашенная архитектура (Two-Tower DNN: башня пользователя + башня
   контента) для мгновенного отбора кандидатов из миллиардов. Аналог в
   poler-engine: Pass 1 (быстрый scatter по postings) / Pass 2 (глубокий
   WebRank по лидерам) — расширить до «башен» эмбеддингов на CPU.
2. **ScaNN (Scalable Nearest Neighbors)** —
   github.com/google-research/google-research/tree/master/scann
   Чистый C++ с AVX-512 SIMD: поиск среди сотен миллионов векторов за
   1–2 мс НА CPU, 15–30 МБ RAM, 0% GPU. Это путь к векторному слою
   poler-engine БЕЗ видеокарт: квантование int8 + SIMD-дистанции.
3. **MediaPipe** — github.com/google-mediapipe
   Потоковый анализ видео/кадров на слабых машинах. Кандидат на
   «мультимедийный» виток движка (кадры → сцены → тройки).
4. **Whitepapers (фундамент, читать перед реализацией):**
   * «Deep Neural Networks for YouTube Recommendations» — разделение
     Candidate Generation / Deep Ranking (у poler-engine та же философия
     двухпроходности);
   * «Google's Custom Video (Argos) ASIC» — VCU-чип: компрессия/анализ
     потоков в 20–33 раза эффективнее GPU. На ПК недостижимо — но протокол
     «анализ на лету при декодировании» переносим и в программный слой.
5. **TPU-подход (int8/bfloat16 матричные умножители)** — на CPU
   эмулируется квантованием + SIMD: см. ScaNN. Обучать с нуля на ПК
   нельзя, забирать готовые сжатые веса (int8/GGUF/ONNX) — можно.

## 3. Честные физические границы домашнего ПК (не игнорировать!)

Законы кремния не обмануть — план обязан учитывать 4 предела:

1. **RAM-bandwidth**: сервер 1000–3000 ГБ/с (HBM3/8-канал DDR5) против
   40–70 ГБ/с на 2-канальном десктопе → индексы обязаны быть компактными
   (varint-delta, WITHOUT ROWID, SimHash u64 вместо текста).
2. **Дисковый IOPS**: сотни тысяч мелких файлов убивают даже NVMe →
   всё в один SQLite/mmap-слой (так и сделано: web-index.db).
3. **Обучение vs инференс**: обучение тяжёлых моделей на CPU — месяцы.
   На ПК — только инференс готовых сжатых весов. Обучение — облако.
4. **Объём RAM 16–32 ГБ**: держать терабайты сырого текста нельзя →
   стриминг, B-tree, бинарные кучи, mmap-окна.

Вывод: инфраструктура poler-engine (SQLite-индекс, varint-кодеки,
двухпроходность, mmap) уже спроектирована под эти пределы — продолжать
в том же духе.

## 4. Что можно делать СЕЙЧАС vs что отложено

| Горизонт | Виток | Донор |
|---|---|---|
| сейчас | фразовый поиск, стемминг, MCP, автономный краул | Lucene/Snowball/MCP |
| скоро | lexical-векторный гибрид на CPU (ScaNN-подход: int8+SIMD), рекомендации «похожие страницы» (Two-Tower на postings-статистиках) | ScaNN, TFRS |
| потом | мультимедиа-конвейер (кадры видео → сцены), распределённый обход на нескольких машинах | MediaPipe, Mercator |
| не на ПК | обучение моделей с нуля, кастомные ASIC | — (облако/аренда) |

## 5. Правила движения к цели

1. Каждый релиз закрывает ровно ОДНУ «хреновую» часть до конца, с живым
   полевым тестом и регрессионным тестом в suite.
2. Никаких зависимостей «на вырост» — только то, что работает в этот релиз.
3. Большие донорские идеи (ScaNN/Two-Tower/MediaPipe) втягиваются только
   когда им есть чем управляться в текущем корпусе.
4. Границы честности фиксируются в README (пример: аутентификационные
   стены notebook.google.com — это граница, а не сбой).


---

## 6. v0.15+: poler-shell (TUI/REPL) и Unified VCS & Data Mesh

> Зафиксировано 2026-08-26 (post-v0.14.0). Инициатива владельца: обернуть
> движок в терминал для человеческого удобства + нативная интеграция со
> всеми VCS/датасет-платформами (GitHub/GitLab/Gitea/Git LFS/DVC/Hugging
> Face Hub/Oxen/ParamLake/HugeSCM/Lit) — Unified Code & Data Mesh.

### 6.1. poler-shell — интерактивный терминал (v0.15.0)

Когда у движка 15+ режимов (поиск, AIDDE, веб-краулинг, NotebookLM, Google
Drive, фразы, графы, синк), человеку неудобно каждый раз вбивать длинные
флаги `--format md --nlm-chat --top 5` или вспоминать UUID ноутбуков.

**Два уровня интерфейса:**

| Уровень | Библиотека | Что даёт | Аудитория |
|---|---|---|---|
| **TUI Dashboard** | `ratatui` + `crossterm` | Левая панель — живой список 87 ноутбуков NLM + локальные репозитории (с эмодзи 🌌 Касіопея, 📈 Бухгалтерия доверия). Правая верхняя — поле ввода поиска/чата с автодополнением Tab. Правая нижняя — выдача с подсветкой синтаксиса, значениями ε/R и K-hop деревом как интерактивным деревом. | Человек-владелец |
| **REPL `poler>`** | `rustyline` | Быстрый командный режим без перезапуска процесса: `poler> search "Касіопея Astra-Nic"` / `poler> nlm ask "Параметры Планковской геодезической"` / `poler> crawl https://rust-lang.org` / `poler> impact dissect_packet --depth 3`. Tab-Completion по командам+флагам, история стрелочками ↑/↓. | Скриптовый человек+скрипт-обёртки |

**Преимущества**: постоянно открытая `WebIndex` + `NlmSession` → As-You-Type
Search за 1 ms при вводе; UUID ноутбуков не нужно помнить — выбор из списка
стрелочками; контекст команд сохраняется в рамках сессии.

**Архитектурные последствия**: текущий `main.rs` — «запустили, отдали,
вышли». v0.15 введёт `Shell` стейт-машину поверх существующих
`poler_engine::*` функций (без переделки ядра — `ratatui` только UI слой).

### 6.2. Unified VCS & Data Mesh (v0.16.0+)

Превращение poler-engine из локального инструмента в **Универсальную Сеть
Кода и Данных** — единый пульт, нативно работающий с любыми репозиториями:

```
              ┌─────────────────────────────────────────┐
              │   POLER-ENGINE UNIFIED DATA MESH        │
              └────────────────────┬────────────────────┘
                                   │
   ┌─────────────────┬─────────────┴─────────────┬─────────────────┐
   ▼                 ▼                           ▼                 ▼
┌──────────┐  ┌──────────────┐          ┌──────────────┐    ┌──────────────┐
│КОД VCS   │  │ДАННЫЕ        │          │ИИ/МОДЕЛИ     │    │ЗНАНИЯ/ОБЛАКО │
│• gix     │  │• Git LFS     │          │• HF Hub      │    │• NotebookLM  │
│• GitHub  │  │• DVC         │          │  (Models/DS) │    │• Google Drive│
│• GitLab  │  │• Oxen.ai     │          │• ParamLake   │    │• Gmail       │
│• Gitea   │  │• HugeSCM(Ant)│          │• ONNX/GGUF   │    │• Web Crawler │
│• Lit(Rust│  │              │          │              │    │              │
│ VCS)     │  │              │          │              │    │              │
└──────────┘  └──────────────┘          └──────────────┘    └──────────────┘
```

**Адаптер-модель** (по образцу `google/` модуля v0.12–v0.13): каждый VCS —
отдельный `src/vcs/<name>.rs` с тривиальным трейтом `VcsAdapter`:

```rust
trait VcsAdapter {
    fn list_repos(&self) -> Result<Vec<RepoId>, String>;
    fn list_commits(&self, repo: &RepoId) -> Result<Vec<Commit>, String>;
    fn list_issues(&self, repo: &RepoId) -> Result<Vec<Issue>, String>;
    fn fetch_blob(&self, repo: &RepoId, oid: &str) -> Result<Vec<u8>, String>;
    fn url_scheme(&self) -> &str;  // "gh://", "gl://", "lfs://", "hf://", "ox://"
}
```

Каждый VCS-объект (коммит, issue, PR, файл, датасет, модель) становится
страницей в `web-index.db` по своей URL-схеме: `gh://user/repo/commit/<sha>`,
`hf://datasets/<owner>/<name>`, `lfs://<repo>/<path>`, `ox://<repo>/<rev>/<path>`.

**Сквозной запрос**:
```bash
poler-engine search "термогеодезическая функция" --all
# → hit 1: nlm://notebook/704f.../note/note-1            (NotebookLM)
# → hit 2: gh://user/repo/commit/a1b2c3d4                (GitHub commit)
# → hit 3: hf://models/owner/model-x                     (Hugging Face model card)
# → hit 4: https://rust-lang.org/...                      (проползенный веб)
```

**Работа с гигантскими монорепами без скачивания** (HugeSCM / LFS): движок
парсит AST и строит AIDDE Call Graph по удалённым репозиториям **через
API**, не забивая локальный диск сотнями гигабайт. Git LFS `.gitattributes`
+ pointer-файлы парсятся на лету для resolve `lfs://` URL без full fetch.

**Донорские технологии** (что заимствуем и улучшаем):

| Донор | Что берём | Куда легло |
|---|---|---|
| `gh` CLI (GitHub) | REST+GraphQL API (commits/issues/PR/codeowners), Actions artefacts | `src/vcs/github.rs` |
| `glab` CLI (GitLab) | REST API v4, merge requests, pipelines | `src/vcs/gitlab.rs` |
| `tea` CLI (Gitea) | REST API (forgejo-compatible) | `src/vcs/gitea.rs` |
| `gix` crate | Pure-Rust git: коммиты/trees/blobs без git-CLI | `src/vcs/local.rs` |
| `git-lfs` pointer protocol | `version https://git-lfs/...` + oid:size: | `src/vcs/lfs.rs` |
| `dvc` `.dvc` files | Outs files + remote storage (S3/Azure/SSH) | `src/vcs/dvc.rs` |
| `huggingface_hub` API | `/api/models`, `/api/datasets`, model cards | `src/vcs/hf.rs` |
| `oxen` CLI / SDK | Oxen.ai remote data versioning | `src/vcs/oxen.rs` |
| `hugescm` (Ant Group) | China-scale monorepo, server-side resolve | `src/vcs/hugescm.rs` |
| `lit` (Rust VCS) | Pure-Rust SCM alternative | `src/vcs/lit.rs` |

### 6.3. Приоритеты в этом направлении

1. **v0.15.0 — poler-shell**: TUI+REPL поверх существующих режимов.
   Зависимости: `ratatui`, `crossterm`, `rustyline` (всё mature, ноль
   новых рисков). Никаких изменений в ядре движка — только UI слой. ✅ shipped
2. **v0.15.1 — полер-шелл финализация**: Tab-completion через rustyline Helper
   + нативные `crawl`/`impact` в REPL. ✅ shipped
3. **v0.16.0 — Unified VCS & Data Mesh**: нативные адаптеры GitHub/GitLab/Gitea
   (REST через `ureq`) + Pure-Rust git через `gix` crate. VCS-страницы в
   web-index.db (`gh://`, `gl://`, `gt://`, `gix://`). Команды `poler> gh/gl/gt/gix`
   + `sync vcs`. ✅ shipped (2026-08-26)
4. **v0.17.0 — gix clone + LFS pointer resolve**: feature-флаги
   `blocking-network-client` для синхронного `gix clone` (без `git` CLI);
   Git LFS `.gitattributes` + pointer-файлы `version https://git-lfs/...`
   для resolve `lfs://` URL без full fetch.
5. **v0.18.0 — License Gate (офлайн ed25519, формат PO1)**: тиры
   community/pro/enterprise + trial 14 дн. + скользящие квоты; платные
   интеграции (Gmail/Drive/NotebookLM) за гейтом, локальное — всегда
   свободно. ✅ shipped (2026-08-29, см. раздел 7)
6. **v0.19.0 — Browser Surface**: фиксы краулера по живому UX-аудиту
   (автодетект Chromium из playwright-кеша, пер-страничный таймаут,
   человекочитаемые robots-сообщения, самовосстановление CDP),
   `--browser-index`, WebLens — расширение MV3, вшитое в бинарник
   (§8). ✅ shipped (2026-08-30)
7. **v0.20.0 — Native Retrieval**: grep-режим (слой 0: полнота, без
   индекса, exit-коды GNU grep) + RAG-чанкер (слой B: passage-уровень
   с якорями) + MCP-инструменты poler_grep/poler_chunk. Анализ трёх
   библиотек — `docs/native-retrieval-analysis.md`. ✅ shipped (2026-08-30)
8. **v0.21.0 — Hardening & Precision**: CodeSymbolIdentity в EntityGraph,
   Triage Layer в AIDDE (proof vs heuristic), Semantic Bridge (офлайн
   ru↔en сенсор), Benchmark Suite. HF Hub — сдвинут (см. ниже).
   ✅ shipped (2026-08-30)
8b. **v0.21.1 — Security Hardening**: white-box аудит v0.21.0 (21 позиция,
   2 HIGH), 12 патчей P1–P12 одним коммитом + security-гейты
   audit_patch_verify 10/10 и audit_stress --hardened 46/46.
   ✅ shipped (2026-08-30)
8c. **v0.22.0 — Terminal Gateway + Source-Available EULA**: единый
   терминальный шлюз (`--gateway`): двойной контур исполнения
   (engine-native приоритет + sandboxed host proxy), конвейеры
   host↔engine без /bin/sh, service/attach управление нижним слоем;
   лицензия — POLER Custom Source-Available & Modification Disclosure
   License v1.0 (модель Unreal Engine EULA: Notification Clause 14 дней,
   роялти 5% > $25k/квартал, non-circumvention Ed25519-гейта).
   Архитектура: `docs/terminal-gateway-architecture.md`.
   ✅ shipped (2026-08-30)
8c-bis. **v0.22.1 — Sandbox Hardening**: adversarial-аудит по команде
   владельца «ПОРОБУЙ РАЗЛИЧНЫЕ МЕТОДЫ АТАКИ»: корпус 113 векторов / 19
   классов через judge-пробник (без исполнения) + живая E2E-батарея →
   46 bypass-векторов v0.22.0 закрыто в sandbox v2 (fail-closed),
   883 теста, гейты 113/113 + 56/56.
   ✅ shipped (2026-08-30)
8c-ter. **v0.23.0 — Interactive PTY Engine, Dynamic Workspace & Sudo
   Privilege Gate**: PTY-passthrough (контур 3: posix_openpt/setsid/
   TIOCSCTTY без новых зависимостей, авто-детект TUI/REPL, префикс
   `pty`), workspace/cd с синхронизацией process-cwd, гранулярный
   sudo-гейт (one-shot /dev/tty + лизинг `grant sudo Nm` кап 60 мин +
   `--dangerously-allow-all` danger-режим с красным баннером);
   Zero Silent Escalation; 904 теста, гейты 113/113 + 65/65.
   Архитектура: `docs/terminal-gateway-architecture.md` §6.
   ✅ shipped (2026-08-31)
8c-quad. **v0.24.0 — Workspace Boundary Guard & Mediated Agent Mode**:
   реакция на живой инцидент (агент внутри gateway читал /home и писал
   /tmp без вопросов): WsGuard-граница во всех судьях (вне корня —
   Confirm; Block-инварианты выше границы), рекурсивный
   judge_shell_payload, ws_root≠cwd, `allow <PATH>`, PATH-shim медиация
   агентов (`__gateway-shim`, отказ 126, телеметрия); 931 тест, гейты
   139/139 + 79/79 + 14/14. Архитектура: §6.4–6.5.
   ✅ shipped (2026-08-31)
8c-quinque. **v0.25.0 — Container Jail (`box`)**: жёсткая Docker-
   изоляция контуров 2/3 (host-os-proxy и pty-passthrough исполняются
   ВНУТРИ контейнера через docker exec): /workspace + персистентный
   /home/poler — единственные монтировки, cap-drop ALL +
   no-new-privileges + mem/pids-лимиты, docker-сокет не пробрасывается;
   Block-вердикты не зависят от jail, redirect-цели/движковые файл-
   команды — с границей (они на хосте), docker-демон из шлюза — Confirm;
   957 тестов (+26), гейты 139/139 + 79/79 + 14/14 + 8/8 (волна 10).
   Архитектура: `docs/terminal-gateway-architecture.md` §6.6.
   ✅ shipped (2026-08-31)
8d. **v0.23.x — globbing в gateway** (globset уже в дереве): раскрытие
   `*.rs` в аргументах движковых команд.
8e. **v0.25.x — тюнинг Container Jail**: CPU-shares/cgroup-v2, профили
   образов агентов (agy/claude-ready), Landlock/seccomp-фолбэк для
   машин без docker.
8c-sexto. **v0.26.0 — Zero-Overhead Agent Bind-Mounting + Two-Tier
   Container Brokerage**: агенты хоста (agy/claude/codex/…) пробрасываются
   в Container Jail автоматически (бинарники ro → /usr/local/bin, конфиги
   rw → /home/poler) — без docker build и двойной установки; ручной
   mount= с deny-list (docker-сокет/системные корни/белый список целей);
   runner-контур исполнения poler-runner-<fnv8> (net=none, только
   /workspace, без home/агентов) + MCP-брокер poler_box_exec/
   poler_box_status (вердикт судьи ДО docker exec; Confirm из MCP не
   подтверждается — Zero Silent Escalation; cross-process discovery по
   POLER_WORKSPACE + docker-labels). 983 теста (+26), гейты 139/139 +
   79/79 + 14/14 + 8/8 + 19/19 (волна 11). Архитектура:
   `docs/terminal-gateway-architecture.md` §6.7–6.8.
   ✅ shipped (2026-08-31)
8f. **v0.26.x — сопровождение брокера**: прокидывание runtime-
   зависимостей ELF-агентов (ldd-резолв glibc/библиотек), автосборка
   минимальных образов агентов, CPU-shares runner, Landlock-фолбэк.
8c-septem. **v0.27.0 — Root Broker + Jailbreak Sentinel**: «sudo как
   услуга» — рут остаётся привилегией ХОСТА: агент в клетке зовёт шим
   `sudo`, шим пишет запрос в файловый канал (/home/poler/.poler-broker),
   шлюз судит (Block → Deny всегда; Confirm → Deny ZSE; инструменты
   побега/пути ядра → Deny; allowlist host-only + дефолт apt/dpkg/
   fs-границы) и исполняет docker exec -u 0 СО СТОРОНЫ ХОСТА — агент
   рут не держит ни секунды (команды box sudo on|off|status|log,
   box allow sudo, box root — интерактивный рут-шелл в клетке для
   владельца); Jailbreak Sentinel — «посади агента и наблюдай»:
   probe-батарея ~28 векторов побега изнутри клетки + хост-аудит
   docker inspect, любой escape → KILL-SWITCH (docker stop, форензика
   сохранена, инцидент в hunt/incidents); agent-режим — живой LLM с
   миссией побега под наблюдением: сигнатуры транскрипта + канарейка
   (токен на хосте в выводе = доказанный побег) → kill-switch;
   цикл «нашёл → закрыл» (вектор → патч + регрессионный тест).
   1035 тестов (+52), гейты 139/139 + 79/79 + 14/14 + 8/8 + 19/19 +
   18/18 (волна 12). Архитектура: `docs/terminal-gateway-architecture.md`
   §6.9–6.10. ✅ shipped (2026-08-31)
8g. **v0.27.x — сопровождение рут-брокера/sentinel**: живой перехват
   транскрипта охоты в поток (сейчас скан пост-фактум), docker diff в
   форензике agent-режима, капабилити-профиль для postinst (opt-in),
   сетевые сигнатуры побега, поллинг канала через inotify.
8c-octo. **v0.28.0 — Root Broker Password Mode + Builtin Hunter**:
   «я ему дам пароль, и у него есть рут» — box sudo passwd (только
   интерактив; соль+растянутый FNV, host-only 0600) → агент: echo
   ПАРОЛЬ | sudo -S cmd (шим v2 читает stdin, пароль b64-полем, не в
   argv/аудите) → брокер сверяет ДО судьи: верный = НЕдеструктивное
   разрешено, инварианты (Block/побег/ядро/ZSE) НЕ ослабляются; брут-
   форс 5 промахов → лок 60с (все попытки в аудите); закрывающая
   находка: контроль-символы (newline) в argv рут-запроса — Deny;
   Builtin Hunter («сделай как GPT на серверах OpenAI») — СВОЙ
   детерминированный красный суб-агент POLER (не agy владельца) атакует
   развёрнутую систему чёрным ящиком изнутри клетки без исходников:
   judge/channel/boundary волны (16+ векторов: обходы судьи, фаззинг
   протокола брокера — мусор/oversize/id-траверсал/replay/симлинк-
   ловушка/усечение/подмена шима/брут-форс/flood) + escape-батарея
   sentinel + docker-inspect; breach → KILL-SWITCH; judge-аномалии →
   авто-блоклист брокера («нашёл → закрыл» в рантайме); --loop =
   постоянное наблюдение (свипы 30с + батареи 10мин, журнал JSONL).
   1059 тестов (+24), гейты 139/139 + 79/79 + 14/14 + 8/8 + 19/19 +
   18/18 + 19/19 (волна 13). Архитектура: `docs/terminal-gateway-
   architecture.md` §6.9.1 + §6.11. ✅ shipped (2026-08-31)
8h. **v0.28.x — сопровождение hunter/пароля**: glob-сигнатуры
   авто-блоклиста (сейчас префикс), systemd-режим loop-наблюдения
   (переживает крах шлюза), bcrypt/argon2 при появлении допустимой
   зависимости, live-перехват канала в loop-свипах (inotify),
   ранжирование judge-векторов по рискам.
9. **Hugging Face Hub + DVC + Oxen (сдвинуто релизами gateway)**:
   model cards/datasets API (`hf://`), data-versioning pointer files,
   remote storage resolve.
10. **HugeSCM/Lit/ParamLake**: адаптеры для China-scale
   монореп и AI-model versioning.

### 6.4. Архитектурное правило для v0.16+

**Ноль новых зависимостей в `web-index.db` схеме** — VCS-страницы используют
те же `WebDoc` + `links` + `content_hash` + `positions` + PageRank, что
веб и NLM. URL-схема — единственное отличие (`gh://` вместо `https://`,
`hf://` вместо `nlm://`). Все адаптеры — source-генераторы страниц, ядро
поиска не трогается.

Это сохраняет invariant v0.14.0: один `--web-search` пробивает ВСЕ юниверсы
(NLM + веб + локальный код + GitHub + HuggingFace + …) с единой PageRank
топологией.

## 7. Монетизация и дистрибуция (стратегия, зафиксирована 2026-08-30)

Решение владельца после v0.18.0: **фаза Dogfooding** — движок используется
автором на собственных задачах и базах знаний. Продажи и верификация Google
отложены до конца этой фазы. Инфраструктура для продаж уже готова и лежит
выключенной: License Gate (v0.18.0), лицензия владельца, license-tool.

### 7.1. Что уже готово (не требует действий)

- **License Gate ed25519 (формат PO1)** — офлайн-проверка, тиры
  community/pro/enterprise, trial 14 дн., квоты, grace 7 дн.
- **license-tool** — keygen + issue: выпуск лицензии покупателю занимает
  одну команду, ключ уходит письмом, активация `--license-import`.
- **Организация `poler-engine-org`** (GitHub) — дом движка: оба репо
  (poler-engine + POLER-Quantum-RS) перенесены и закрыты (private),
  история и редиректы старых URL сохранены.

### 7.2. Площадка продаж — план (когда фаза Dogfooding завершена)

Приоритет: **Merchant of Record** — платформа сама является продавцом,
берёт на себя НДС/налоги/антиторговые проверки:

1. **Lemon Squeezy** (первый кандидат): ~5% + 50¢, принимает налоги на себя,
   подходит инди-разработчику без юрлица.
2. **Paddle** (запасной): те же ~5% + 50¢, строже модерация.
3. **Steam** — НЕ рекомендуется: $100 за приложение, W-8BEN, ~30% комиссия,
   витрина нацелена на игры; CLI-инструмент там чужой.

**Правило безопасности (фиксируется навсегда):** регистрация на площадке,
банковская карта, адрес, персональные данные — ТОЛЬКО владельцем лично,
в его собственном браузере, на сайте площадки. Никогда не передаются
агентам, ассистентам, в чаты или скрипты. Агент готовит текстовые
инструкции и посадочные материалы — не данные.

### 7.3. Верификация Google (только при платящих юзерах)

Скоупы движка (`gmail.readonly`, `drive.readonly`) — restricted: публичный
запуск потребует CASA Tier 2 (~$540/год, TAC Security) + Privacy Policy в
отдельном публичном репо орги. До этого момента режим Testing: 100 тестовых
юзеров, refresh-токены 7 дней. Платная закрытая бета укладывается в Testing.

### 7.4. Юридический след (зафиксировано честно)

Снапшоты до закрытия кода (v0.17.7 и ранее, MIT OR Apache-2.0) остаются
открытыми для тех, кто успел клонировать. Форков и релизов не было —
фактического распространения нет. Новые версии закрыты свободно.

## 8. Браузерная поверхность: UX-аудит и WebLens (2026-08-30)

Вопрос владельца: «насколько ебануто чувствовать себя при ручном парсинге
интернета через движок? не всраивать ли поиск прямо в браузер?»
Ответ — руками, не лозунгами: живой аудит веб-конвейера v0.18.0.

### 8.1. Замеры живого цикла (краул → индекс → поиск)

| Сценарий | Результат |
|---|---|
| `ed25519.cr.yp.to` depth=1, 3 стр. | 6.5 с (вкл. запуск CDP-браузера) |
| `rfc-editor.org/rfc/rfc8032.html` (~90 КБ) | 2.0 с, 1 стр. |
| `docs.rs` (JS-тяжёлый) | **>180 с, завис**, убит таймаутом; частичный индекс выжил (инкрементальные коммиты работают) |
| `--web-search` | **7–30 мс**, JSON + полный разбор скоринга |
| перефраз «how to confirm message authenticity» | RFC 8032 №1 (score 0.733) — BM25-частичное совпадение; Ctrl+F такое не нашёл бы |
| русский запрос к англ. корпусу («проверка подписи») | **0 результатов** — кросс-язычного моста нет |
| REPL (`poler> web-search/stats`) | пайп-режим работает, база переиспользуется между командами |

Формула ранжирования видна в каждом ответе:
`0.55·BM25 + 0.15·PageRank + 0.20·title + 0.10·ε-density` + фразы + стемминг.

### 8.2. Честный вердикт по боли

Для владельца-терминальщика и для агентов (MCP) — **рабочее уже сегодня**:
поиск мгновенный, сниппеты честные, один индекс на всё. Для «обычного
исследователя» — нет: чтение происходит в браузере, а индекс — в терминале,
и каждое переключение окон стоит мысли.

Точки боли, найденные вживую (кандидаты в v0.19.0-окно, «малой кровью»):

1. `POLER_CHROME_BIN` не автодетектится — playwright-кеш
   (`~/.cache/ms-playwright/chromium*/chrome-linux*/chrome`,
   `chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell`)
   движок не ищет (ошибка в автозапуске CDP, `src/web/cdp.rs`).
2. Нет per-page таймаута в крауле: docs.rs висел >180 с без прогресса
   (`src/web/crawl.rs`, `crawl()`).
3. Robots-skip молчит счётчиком: `skipped_robots: 1` без объяснения,
   какой хост/путь запрещён — пользователь гадает.
4. Осиротевший CDP-браузер после убитого краула: следующий запуск падает
   «ws frame hdr: Resource temporarily unavailable» вместо тихого
   перезапуска (`CdpSession::connect`).

### 8.3. Архитектура: тиры браузерной поверхности

**T0 — есть сейчас.** CLI (`--crawl`/`--web-search`/`--web-stats`),
REPL, MCP stdio + MCP HTTP (`127.0.0.1:8765`, Bearer, CORS-preflight
уже реализованы — `src/mcp_http.rs`). Поверхность для агентов закончена.

**T1 — v0.19.0. ✅ SHIPPED (2026-08-30).** Фикс-лист §8.2 закрыт
целиком (автодетект playwright-кеша; пер-страничный таймаут
`--crawl-page-timeout-ms` + бюджет интерсепта 8 с + finished-only
тела; robots-сообщения с хостом/путём в stderr и `stats.notes`;
`/json/version`-healthcheck + тихий перезапуск осиротевшего CDP),
плюс `--browser-index <URL>`: страница → web-index.db одной командой
(respect_robots=false — явная команда пользователя, фиксируется в notes).

**T2 — WebLens MVP. ✅ SHIPPED (2026-08-30, v0.19.0).** Расширение
Manifest V3, вшитое в бинарник (`include_bytes!`, src/web/weblens.rs):
Side Panel (поиск по общему индексу — тот же инвариант v0.14.0),
подсветка термов (TreeWalker + `<mark>`, DOM не портится), кнопка
«Индексировать эту страницу» (poler_crawl с respect_robots=false),
Alt+P. Бэкенд — прежний `--mcp-http` (Bearer; preflight доведён:
Allow-Headers для Authorization/Content-Type/X-Poler-Token).
`--web-lens` = материализация + оконный Chromium с `--load-extension`
(автоустановка, ноль кликов) + MCP-демон; `--web-lens-install` —
файлы + инструкция «Load unpacked» для ежедневного браузера
(chrome://-страницы автоматизировать нельзя — защита браузера).
WebLens НЕ гейтится лицензией: локальный поиск всегда свободен
(философия §v0.18.0), гейт — только на NLM/Gmail/Drive-интеграциях.

**T3 — форк Chromium. ОТКЛОНЁН с цифрами.** Исходники ~100 ГБ,
полная сборка часами на 16–32 ГБ RAM, поезд релизов каждые 4 недели,
секьюрити-патчи ежедневно. Brave/Edge/Opera держат форки командами;
Electron существует именно потому, что пер-апп форк не держит никто.
Соло-разработчик, форкнув Chromium, перестаёт развивать движок.
Если когда-нибудь понадобится глубже, чем расширение, — CDP-аттач
к реальному браузеру пользователя (порт 9222) покрывает остальное
без единой строки чужого кода.

### 8.4. Дефляция маркетинга (честные границы WebLens)

- «Введёшь запрос на любом языке» — **ложь для MVP**: кросс-языкового
  моста нет (замер §8.1: 0 результатов). Расширение должно детектить
  язык запроса vs язык корпуса и честно предупреждать. Мост (словарь
  синонимов/перевод терминов) — отдельная задача после MVP.
- «Семантический фильтр Ψ подсвечивает смысл» — реальный ранжир:
  BM25 + PageRank + title + ε-density + фразы + стемминг. Синонимы без
  эмбеддингов не сводятся; подсветка в MVP — совпавшие термины и
  k-hop-контекст, не «тепловая карта смысла».
- Киллер-ценность WebLens — НЕ подсветка на одной странице (страница
  и так влезает в контекст), а **capture**: одна кнопка → страница
  в общий индекс → один поиск по всему накопленному. Подсветка —
  крючок UX, захват — ценность.

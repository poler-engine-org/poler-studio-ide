# CompanionBridge: Companion-мост к оф. Pre-GA NotebookLM Enterprise API

> Статус: Design + Skeleton v0.18.0-alpha
> Дата: 2026-08-26
> Автор: research session

## 1. Мотивация и стратегия

poler-engine v0.17.0 уже имеет рабочий канал к NotebookLM через
`src/google/nlm.rs` — это **потребительский batchexecute-протокол**,
извлечённый из расширения NLMTools (RPC `wXbhsf`/`rLM1Ne`/`hizoJc`/`cFji9`/`gArtLc`).
Этот канал — «котыль»: Google меняет RPC-идентификаторы без предупреждения,
но зато он **покрывает операции, которых нет в оф. API** — chat/query/get_notes/get_artifacts.

**Ключевая находка (research session 2026-08-26):** оф. Pre-GA Gemini Notebook
Enterprise API **не покрывает** чат и чтение контента источника. Подтверждение:
[discuss.google.dev — Missing Query/Chat Endpoint](https://discuss.google.dev/t/notebooklm-enterprise-api-missing-query-chat-endpoint-for-rag-orchestration/366875).
Зато он **силён** там, где наш batchexecute слаб: пакетная заливка источников
(Google Drive/Slides/raw text/web/YouTube) и upload файлов (PDF/xlsx/pptx).

Стратегия пользователя подтверждена: **не заменять ядро, а дополнять**.
CompanionBridge — это сменный коннектор ввода-вывода, который для конкретных
операций вызывает оф. Pre-GA API, а всё остальное прозрачно отдаёт на
существующий CdpBatchexecuteProvider.

## 2. Карта покрытия операций

| Операция                          | Оф. Pre-GA API                                  | Наш `batchexecute` | Кто в bridge? |
|-----------------------------------|-------------------------------------------------|--------------------|---------------|
| Create notebook                   | `notebooks.create`                              | ✗                  | оф.           |
| Get notebook                      | `notebooks.get`                                 | `rLM1Ne` ✓          | любой         |
| List notebooks                    | (не описано list, только list-recent)           | `wXbhsf` ✓          | **наш**       |
| **Batch upload** (Docs/Slides/text/URL/YouTube) | `sources:batchCreate`              | DOM drag-drop ✗    | **оф.**       |
| **Upload file** (PDF/xlsx/pptx)   | `sources:uploadFile` (X-Goog-Upload-Protocol: raw) | DOM drag-drop ✗  | **оф.**       |
| Source metadata                   | `sources.get` (wordCount, tokenCount, status)   | `hizoJc` ✓          | любой         |
| **Source content** (полный текст/слайды) | ✗ NOT IN OFFICIAL                          | `hizoJc` ✓          | **наш**       |
| **Notes**                         | ✗ NOT IN OFFICIAL                               | `cFji9` ✓           | **наш**       |
| **Artifacts** (audio/report/quiz/mindmap) | только audio overview (`audioOverviews.create`) | `gArtLc` ✓ (все) | **наш**       |
| **Chat / ask question**           | ✗ NOT IN OFFICIAL                               | `NlmSession::chat` ✓ | **наш**       |
| **Audio Overview create/delete**  | `audioOverviews.create/delete` ✓                | ✗                   | **оф.**       |
| Delete sources                    | `sources:batchDelete` ✓                         | ✗ (только через UI) | **оф.**       |

Где «любой» — оба провайдера равноценны, гибрид выбирает по доступности.

## 3. Архитектура

### 3.1 Слои

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 4: shell/TUI/CLI (shell/commands.rs, shell/tui.rs)    │
│         ↑ вызывает только HybridProvider                    │
├─────────────────────────────────────────────────────────────┤
│ Layer 3: HybridProvider (src/google/companion.rs)          │
│   • routing policy: какую операцию какому провайдеру дать    │
│   • fallback: если оф. API 401/timeout → CDP берёт на себя   │
│   • cache: source_id ↔ content (FNV-1a, уже в nlm_ingest)   │
├─────────────────────────────────────────────────────────────┤
│ Layer 2: SourceContentProvider trait                        │
│   impls:                                                    │
│   • GcpEnterpriseProvider (оф. Pre-GA, новый)               │
│   • CdpBatchexecuteProvider (обёртка над nlm.rs, существ.)  │
├─────────────────────────────────────────────────────────────┤
│ Layer 1: Transport                                           │
│   • GoogleHttp (существующий — CDP fetch() через Chromium)   │
│   • OAuth loopback (существующий — oauth.rs)                │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Trait контракт

```rust
pub trait SourceContentProvider {
    fn name(&self) -> &'static str;

    /// Покрыт ли эта операция у этого провайдера?
    fn supports(&self, op: Op) -> bool;

    fn list_notebooks(&mut self) -> Result<Vec<NotebookBrief>, BridgeError>;
    fn get_notebook(&mut self, nb_id: &str) -> Result<Notebook, BridgeError>;

    fn batch_create_sources(
        &mut self, nb_id: &str, items: &[SourceUpload],
    ) -> Result<Vec<String>, BridgeError>;

    fn upload_file(
        &mut self, nb_id: &str, local_path: &Path,
        display_name: &str, mime: &str,
    ) -> Result<String, BridgeError>;

    fn get_source(&mut self, nb_id: &str, src_id: &str) -> Result<Source, BridgeError>;
    fn get_notes(&mut self, nb_id: &str) -> Result<Vec<ParsedNote>, BridgeError>;
    fn list_artifacts(&mut self, nb_id: &str) -> Result<Vec<Artifact>, BridgeError>;
    fn chat(&mut self, nb_id: &str, question: &str) -> Result<String, BridgeError>;
    fn create_audio_overview(
        &mut self, nb_id: &str, source_ids: &[&str],
        focus: Option<&str>, lang: &str,
    ) -> Result<String, BridgeError>;
    fn delete_audio_overview(&mut self, nb_id: &str) -> Result<(), BridgeError>;
}
```

### 3.3 Routing policy в HybridProvider

```rust
impl SourceContentProvider for HybridProvider {
    fn batch_create_sources(...) -> Result<...> {
        // Только оф. API умеет это. Если недоступен — ошибка.
        if self.gcp.supports(Op::BatchCreate) {
            return self.gcp.batch_create_sources(...);
        }
        Err(BridgeError::NotSupported { op: Op::BatchCreate, provider: "hybrid" })
    }

    fn get_source(...) -> Result<Source> {
        // Гибрид: метаданные из оф. API (быстро, стабильно),
        // контент из CdpBatchexecuteProvider (не покрывается оф. API).
        if self.gcp.ready() {
            let meta = self.gcp.get_source_meta(nb, src)?;
            let content = if let Some(text) = meta.content_preview {
                text
            } else {
                self.cdp.get_source_content(nb, src)?.content
            };
            Ok(Source { meta, content })
        } else {
            self.cdp.get_source(nb, src)
        }
    }

    fn chat(...) -> Result<String> {
        // Только CDP. Оф. API не покрывает.
        self.cdp.chat(nb, question)
    }

    fn list_notebooks(...) -> Result<Vec<NotebookBrief>> {
        // Сначала пробуем оф. (если есть GCP-аккаунт), иначе CDP.
        if self.gcp.ready() {
            self.gcp.list_notebooks().or_else(|_| self.cdp.list_notebooks())
        } else {
            self.cdp.list_notebooks()
        }
    }
}
```

### 3.4 TUI интеграция: Enter на источнике → окно редактора

Workflow для Enter на выделенном источнике в Sources панели:

```
Enter на source[SourceMeta]
  ↓
HybridProvider.get_source(nb_id, src_id)
  ↓
match source.kind:
  GoogleDriveSource { drive_file_id } → xdg-open https://docs.google.com/.../d/{drive_file_id}/
  YouTubeSource { youtube_id }         → xdg-open https://youtube.com/watch?v={youtube_id}
  WebSource { url }                    → xdg-open {url}
  TextSource { content }               → write to /tmp/poler-src-{src_id}.md → $EDITOR
  FileUploadSource { local_path }      → $EDITOR {local_path}   ← ключевой кейс!
  Unknown                              → fallback на CdpBatchexecuteProvider.hizoJc
```

Ключевой кейс: при `uploadFile` оф. API мы **уже имеем локальный путь**
загруженного файла. Enter открывает его напрямую в `$EDITOR` —
мгновенно, без фетча. Это и есть «легче через оф. API», о котором говорил
пользователь.

## 4. Реализация в poler-engine

### 4.1 Файлы

- `src/google/companion.rs` (новый) — trait, типы, URL builders, GcpEnterpriseProvider skeleton, HybridProvider skeleton, тесты URL builders
- `src/google/mod.rs` — добавить `pub mod companion;`
- `src/google/oauth.rs` — добавить скоуп `https://www.googleapis.com/auth/cloud-platform`
- `src/shell/commands.rs::cmd_nlm()` — для подкоманды `nlm upload`/`nlm aoview` делегировать в `HybridProvider`
- `src/shell/tui.rs::on_enter_source()` — новый handler для Enter на источнике

### 4.2 URL builders (детерминированные, тестируются)

```rust
pub fn notebooks_base(host_region: &str) -> String {
    format!("https://{host_region}-discoveryengine.googleapis.com/v1alpha")
}

pub fn notebook_path(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    format!("{}/projects/{}/locations/{}/notebooks/{}",
        notebooks_base(region), project, location, nb_id)
}

pub fn sources_batch_create_url(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    format!("{}/sources:batchCreate", notebook_path(region, project, location, nb_id))
}

pub fn sources_upload_file_url(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    // /upload/v1alpha/... (vs base /v1alpha/...) для media-uploads
    let base = notebooks_base(region);
    let path = notebook_path(region, project, location, nb_id);
    let path_no_base = path.strip_prefix(&format!("{}/", base)).unwrap_or(&path);
    format!("{}/upload/{}:uploadFile", base, path_no_base)
}

pub fn source_get_url(region: &str, project: &str, location: &str, nb_id: &str, src_id: &str) -> String {
    format!("{}/sources/{}", notebook_path(region, project, location, nb_id), src_id)
}

pub fn sources_batch_delete_url(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    format!("{}/sources:batchDelete", notebook_path(region, project, location, nb_id))
}

pub fn audio_overview_create_url(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    format!("{}/audioOverviews", notebook_path(region, project, location, nb_id))
}

pub fn audio_overview_delete_url(region: &str, project: &str, location: &str, nb_id: &str) -> String {
    format!("{}/audioOverviews/default", notebook_path(region, project, location, nb_id))
}
```

### 4.3 Конфигурация (env vars)

- `POLER_GCP_PROJECT_NUMBER` — Google Cloud project number
- `POLER_GCP_LOCATION` — `global` (default), `us`, `eu`
- `POLER_GCP_REGION` — multi-region для endpoint: `global` (default), `us`, `eu`
- `POLER_GCP_TOKENS` — путь к google_tokens.json (reuse `tokens_path()`)
- `POLER_COMPANION_MODE` — `auto` (default, гибрид с fallback), `gcp_only`, `cdp_only`

### 4.4 Fallback правила

1. `gcp.ready()` = `tokens_path().exists() && access_token не истёк && POLER_GCP_PROJECT_NUMBER задан`
2. Если `gcp.ready() == false` → HybridProvider всегда роутит на CDP
3. Если операция не поддержана оф. API (chat, get_notes, get_artifacts, get_source_content) → всегда CDP
4. Если операция не поддержана CDP (batch_create, upload_file, audio_overview) → всегда GCP или ошибка
5. Если GCP-запрос вернул 401/403/5xx → fallback на CDP где возможно, иначе ошибка

### 4.5 Что НЕ трогаем

- **Не трогаем** `web-index.db` (BM25/PageRank/IIR/SimHash) — это ядро поиска
- **Не трогаем** `nlm_ingest.rs` (FNV-1a, URL схема) — это индексер
- **Не трогаем** `oauth.rs` flow — только добавляем скоуп `cloud-platform`
- **Не трогаем** `GoogleHttp` — он уже умеет всё нужное (CDP fetch + Bearer)
- **Не добавляем** TLS-зависимостей в Rust — TLS остаётся в Chromium
- **Не добавляем** Python-обёрток (пользователь явно сказал: «пайтон обертка нам не нужна»)

## 5. План внедрения (по milestone)

### M1 (этот PR): Skeleton
- [x] Design doc (этот файл)
- [x] `src/google/companion.rs` skeleton: trait, типы, URL builders, тесты
- [x] `src/google/mod.rs` — `pub mod companion;`
- [x] `cargo build --release` — компилируется, не ломает v0.17.0
- [x] `cargo test --lib` — тесты URL builders зелёные

### M2: GcpEnterpriseProvider — real endpoints
- [ ] Реализовать `GcpEnterpriseProvider::list_notebooks` через `GoogleHttp.get`
- [ ] Реализовать `batch_create_sources` — POST с JSON body
- [ ] Реализовать `upload_file` — POST с X-Goog-Upload-* headers
- [ ] Реализовать `get_source` (metadata only)
- [ ] Live тесты (`#[ignore]`) против реального GCP-аккаунта владельца

### M3: HybridProvider routing
- [ ] `HybridProvider::new(gcp, cdp)` — конструктор
- [ ] Реализовать routing для каждой операции
- [ ] Fallback логика с retry-once

### M4: TUI Enter-handler
- [ ] В `shell/tui.rs` — обработать Enter на source в SourcesList
- [ ] Match по source.kind → open_editor/xdg-open/write-to-tmp
- [ ] Тесты для match-arm логики (без реального exec)

### M5: CLI subcommands
- [ ] `poler nlm upload <nb_id> <file>` → `HybridProvider.upload_file`
- [ ] `poler nlm upload-batch <nb_id> <manifest.json>` → `batch_create_sources`
- [ ] `poler nlm aoview <nb_id> [--focus "..." --lang en]` → `create_audio_overview`
- [ ] `poler nlm aodel <nb_id>` → `delete_audio_overview`

### M6 (опционально): постепенный демонтаж batchexecute
- Когда Google добавит chat/query/get_notes в оф. GA API — переключить `CdpBatchexecuteProvider` на stub и удалить `nlm.rs` RPC-константы.

## 6. Риски и митигация

| Риск                                              | Митигация                                               |
|---------------------------------------------------|---------------------------------------------------------|
| Pre-GA → Google уберёт/переименует эндпоинты       | Все URL builders в одном файле, trait изолирует impls  |
| Pre-GA ToS conflict с consumer batchexecute        | HybridProvider соблюдает separation — оф. операции только через GCP-аккаунт, не через profile cookies |
| CDP-браузер не поднят → GCP-запросы валятся       | `GoogleHttp::connect` уже имеет retry + явные ошибки   |
| `gcloud auth print-access-token` недоступен        | Используем `oauth.rs` с добавленным скоупом `cloud-platform` (тот же Bearer, что уже работает для Gmail/Drive) |
| Content integrity при двойном фетче (meta+content) | `nlm_ingest::content_hash` уже проверяет FNV-1a → повторный синк идемпотентен |

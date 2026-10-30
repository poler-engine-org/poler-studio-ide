# Формат web-index.db — SQLite-схема веб-индекса

> Реализация: `src/web/index.rs` (`WebIndex`). Путь по умолчанию:
> `~/.local/share/poler-engine/web-index.db` (CLI `--web-db`).

## 1. Назначение

Локальный поисковый индекс краулера: страницы, термы, ссылки, хосты.
Заполняется `--crawl` / `--browser-index`; опрашивается `--web-search`
(BM25 + WebRank/PageRank) и `--web-stats`. Никаких серверов — один файл
SQLite, WAL-режим, `synchronous = NORMAL`.

## 2. Схема

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;

CREATE TABLE IF NOT EXISTS pages(
  id           INTEGER PRIMARY KEY,
  url          TEXT UNIQUE NOT NULL,
  title        TEXT NOT NULL DEFAULT '',
  lang         TEXT NOT NULL DEFAULT '',   -- whatlang
  meta_desc    TEXT NOT NULL DEFAULT '',
  text         TEXT NOT NULL DEFAULT '',   -- извлечённый контент
  content_hash TEXT NOT NULL,              -- точная дедупликация
  simhash      INTEGER NOT NULL,           -- нечёткая дедупликация
  doclen       INTEGER NOT NULL,           -- токенов
  rank         REAL NOT NULL DEFAULT 1.0,  -- PageRank (итерации по links)
  dup_of       TEXT NOT NULL DEFAULT '',   -- url канона, если дубликат
  fetched_at   INTEGER NOT NULL            -- unix-time
);

CREATE TABLE IF NOT EXISTS terms(
  term     TEXT NOT NULL,          -- лемма (Snowball) / стоп-фильтр
  page_id  INTEGER NOT NULL,
  tf       INTEGER NOT NULL,       -- частота в теле
  title_tf INTEGER NOT NULL,      -- частота в заголовке (буст)
  positions BLOB,                 -- упакованные позиции (фразовый поиск)
  PRIMARY KEY(term, page_id)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS links(
  src INTEGER NOT NULL,            -- pages.id
  dst TEXT NOT NULL,               -- целевой URL (может быть не краулен)
  PRIMARY KEY(src, dst)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS hosts(
  host      TEXT PRIMARY KEY,
  robots    TEXT NOT NULL DEFAULT '',  -- закэшированный robots.txt
  robots_at INTEGER NOT NULL DEFAULT 0 -- время проверки
);

CREATE TABLE IF NOT EXISTS meta(
  k TEXT PRIMARY KEY,
  v TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_terms_page ON terms(page_id);
CREATE INDEX IF NOT EXISTS idx_links_dst  ON links(dst);
```

## 3. Инварианты и поведение

1. **URL — ключ идемпотентности**: повторный краул той же страницы —
   `INSERT OR REPLACE` по url; id стабилен, термы переписываются.
2. **Двухуровневая дедупликация**: `content_hash` (точная) и `simhash`
   (нечёткая, порог Хэмминга). Дубликат помечается `dup_of` и исключается
   из ранжирования, но остаётся в индексе (аудитор след).
3. **robots.txt — не совет, а закон**: `hosts.robots` кэшируется с временем
   проверки (`robots_at`); краулер не нарушает закэшированные правила.
   Исключение — `--browser-index`: явная команда пользователя индексировать
   конкретную страницу (нарушение фиксируется в notes, а не происходит
   молча).
4. **PageRank**: `rank` пересчитывается итерациями по `links`; WebRank
   ранжирования = BM25-скор × rank (детали — web/index.rs).
5. **Фразовый поиск**: `positions` BLOB упаковывает позиции терма в тексте;
   фраза «точная цитата» проверяется по совместимым позициям кандидатов
   (без загрузки text).
6. **meta**: служебные k/v (счётчики обходов, версии схемы). Не для
   пользовательских данных — заметки живут в `poler_notes` (notes/mod.rs).

## 4. Смежные SQLite-базы (не входят в web-index.db)

| База | Модуль | Таблицы |
|---|---|---|
| poler_notes | notes/ | `poler_notes` (+индексы tags/source/notebook_id) |
| poler_sources | sources/ | `poler_sources` (+индексы kind/value) |
| impact-кэш | aidde/ | таблицы символов/call graph (см. `--impact-cache`) |

## 5. Версионирование

Схема создаётся `CREATE TABLE IF NOT EXISTS` — эволюция через добавление
колонок с DEFAULT (старые файлы открываются). Версия схемы, если появится, —
в `meta(k='schema_version')`.

# ⚡ POLER Knowledge Engine: Высокоскоростной Движок Обработки Знаний и GPU-Инференса

> **Суверенный тулсет POLER[Ψ]** для пакетной компиляции сырых дампов в строгие технические спецификации, аппаратной CUDA-транскрибации аудио/видео на GPU и сборки баз знаний FTS5.

---

## 🛠️ Входящие Модули:

1. **`pts_compiler.py`** — Пакетный компилятор сырых markdown-дампов (NotebookLM, web text, notes):
   * Очищает битые LaTeX-экранирования (`\\` $\to$ `\`).
   * Классифицирует материал и привязывает к 5-фазному циклу $\wp \to O \to L \to \varepsilon \to R[n]$.
   * Генерирует спецификации `PTS-001` ... `PTS-XXX` со сводным реестром `PTS_MASTER_INDEX.md`.

2. **`gpu_transcriber.py`** — Аппаратный GPU Whisper int8 транскрибатор:
   * Динамически подключает рантайм `cuBLAS` и `cuDNN` на видеокартах NVIDIA (GTX 1060 / Pascal+).
   * Выкачивает аудиопотоки через `yt-dlp` и транскрибирует ролики за 3–6 секунд с генерацией таймкодов.

3. **`corpus_builder.py`** — Сборщик базы данных SQLite и книг EPUB:
   * Создает базу данных SQLite со встроенным полнотекстовым индексом `FTS5`.
   * Собирает 300+ спецификаций в единый электронный фолиант `.epub` с оглавлением.

---

## 🚀 Примеры Использования:

### 1. Компиляция сырой папки в спецификации PTS:
```bash
python3 tools/knowledge_engine/pts_compiler.py \
  --src "/path/to/raw_dump" \
  --out "/path/to/output_specs"
```

### 2. Пакетная GPU-транскрибация YouTube-каналов / ссылок:
```bash
python3 tools/knowledge_engine/gpu_transcriber.py \
  --urls "urls_list.jsonl" \
  --out "transcripts_dir" \
  --model "base"
```

### 3. Сборка базы данных SQLite и книги EPUB:
```bash
python3 tools/knowledge_engine/corpus_builder.py \
  --specs "/path/to/output_specs" \
  --db "knowledge.db" \
  --epub "Knowledge_Book.epub"
```

# Zero-Storage Streaming Archives — Design Note (Future Milestone)

> **Статус:** future milestone. Не часть v0.17.3. Запланирован после v0.18.0 (Companion Bridge M1–M5). Зафиксирован дословно из research-сессии с последующей **доработкой архитектуры** под существующие модули poler-engine.

## 1. Мотивация

Большая часть знаний в интернете лежит в архивах: Common Crawl (`wet.tar.gz`, `wat.tar.gz`, `warc.tar.zst`), GitHub dumps (`*.tar.gz`), Hugging Face datasets (`.zst` shards), arXiv bulk (`*.tar`), StackExchange data dumps (`7z → xml → tar`), Wikipedia dumps (`bz2 → xml`). Суммарный объём — петабайты.

Классический пайплайн обучения/дообучения локальных LLM и NLP-моделей выглядит так:

1. Скачать 500 ГБ – 2 ТБ архивов.
2. Распаковать на локальный SSD/NVMe (требуется x2–x3 места).
3. Диск забит, SSD изнашивается триллионами циклов перезаписи мелких файлов, ~90% времени — скачивание и распаковка, а не обучение.

Этот документ описывает альтернативу: **Zero-Storage Streaming & Virtualized Datasets** — чтение архивов в оперативной памяти на лету, с интеграцией в существующий конвейер poler-engine и использованием уже существующих модулей (`streaming.rs`, `resonance/`, `web/simhash.rs`, `tokenizer/pii.rs`, `aidde/`, `psi.rs`).

## 2. Существующая инфраструктура poler-engine (на что опираемся)

| Модуль | Файл | Что переиспользуем |
|---|---|---|
| Потоковый конвейер | `src/streaming.rs` (1236 LOC) | Литеральный prefilter (aho-corasick), zero-copy токены `FileTokens<'a>`, multi-pass (1 = статистика, 2 = токенизация hit-файлов, 3 = материализация top-N якорей). **Ключевое: уже спроектирован для ограничения RAM.** Заменяем `mmap` на `HTTP Range stream` — и получаем сетевой zero-storage режим. |
| IIR-резонанс | `src/resonance/iir_filter.rs` | `R_t = ε_t + φ·R_{t−1}`, O(N), O(1) памяти. `IirFilter::push(eps) -> f64` — потоковый аккумулятор. Используется для фильтрации «осмысленных» чанков до попадания в LLM. |
| POLER[Ψ] формализм | `src/psi.rs` | Полный порт POLER_Psi_v3: `Ω(o)=tanh(o)`, свободная энергия `F=‖g(p;θ)−Ω(o)‖²`, проектор логики `Π_Λ`, ψ-поток `p_{t+1}=p_t+η·Π_Λ(−∇F+γ∇ε)`. Используется для attention-режима обучения: каждый чанк получает ψ-вес → loss-weight. |
| Локальная плотность ε | `src/resonance/epsilon.rs` | `ε(W) = κ·(1+ln(1+count(kw)))·Σ_w (ln N_total − ln freq(w))² + Σ_w Bonus_semantic(w)`. Семантические маркеры (отрицания, обязанность, угроза) — Аho-Корасик. Используется как «энергия значимости» чанка. |
| SimHash near-dup | `src/web/simhash.rs` | 64-bit отпечаток, шингл 4 слова, адаптивный порог Хэмминга (3 на длинных, до 10 на коротких). Используется для потоковой дедупликации. |
| PII-маскирование | `src/tokenizer/pii.rs` | Zero-copy `PiiCleaner::clean -> Cow<str>` — email/телефон/IP/секреты/карты. Маркеры `[EMAIL]/[PHONE]/...`. Используется в потоке до токенизации. |
| Инвертированный индекс | `src/tokenizer/inverted_index.rs` + `src/tokenizer/mod.rs` | Unicode-токенизатор (вкл. кириллица), индексируются ВСЕ токены (вкл. стоп-слова — для Negation Blindness fix). |
| AIDDE | `src/aidde/{impact,sqlite_store,symbols}.rs` | Symbol table + call graph (disk-backed через sqlite). Не directly part of streaming, но пригодится для инжекции файлов-источников в проект (например, подгрузка `.rs` файлов из удалённого тарболла как «как будто локальных» через virtual mount). |
| Companion Bridge | `src/google/companion.rs` | HybridProvider + GcpEnterpriseProvider + CdpBatchexecuteProvider. **Не directly related, но архитектурно близко: remote-bridge с fallback, точно так же как StreamingArchiveProvider будет иметь primary (HTTP Range) + fallback (CDP browser fetch).** |

## 3. Доработанная архитектура

Оригинальная идея из research-сессии была такой:

```
[ Удалённый архив в сети: 100 ТБ ]
                │ (HTTP Range / Stream)
                ▼
[ Буфер RAM: чанк на лету ] ──► [ Дедупликация (SimHash) ]
                │
                ▼
[ IIR-резонанс / фильтр шума ]
                │
                ▼
[ Токенизатор: BPE / WordPiece ]
                │
                ▼
[ VRAM GPU / NPU: батч ] ──► (RAM испаряется: диск = 0)
```

Доработка под четыре потребителя и параллельный поиск:

### 3.1 Четыре целевые аудитории

1. **Обучение локальных LLM/NLP (fine-tune, LoRA, continued pretraining).** Чанки проходят через ε/IIR/SimHash-фильтр и инжектируются в trainer с ψ-весом как `loss_weight` (importance sampling). Память — десятки МБ при тера-датасетах.
2. **Готовые LLM (RAG-context injection).** Чанки фильтруются по запросу пользователя (BM25 + PageRank из `web-index.db`), top-K аннотируются `ContextAnchor` из `src/output/`, подаются в контекстное окно готовой модели. Никакого дообучения.
3. **Помощь человеку (interactive discovery).** В TUI `poler-shell` — навигация по «виртуальной папке» удалённого архива (как `mc` над S3). Человек выбирает конкретный файл/директорию, poler-engine селективно выкачивает только его и запускает IIR/ε/SimHash на нём.
4. **Параллельный поиск по конкретным и смежным темам.** Запрос пользователя (например, «асимметричная криптография на эллиптических кривых») → список ядерных терминов + expansion через `aidde::symbols` (если запущен над проектом) → параллельные streaming-запросы к N архивам (rayon `par_iter`) → каждый поток фильтрует чанки по ε(kw) ≥ threshold → top-K чанков объединяются в один рейтинг по ψ-полю, дедуп через SimHash.

### 3.2 Доработанный конвейер

```text
                  ┌───────────────────────────────────────────────────────┐
                  │ User query (TUI / CLI / API)                          │
                  │   • ядерные термины: K = {kw1, kw2, ...}              │
                  │   • expansion: synonyms + co-occurrence из web-index │
                  │   • forbidden epochs: [t_min, t_max] (temporal Π_Λ)  │
                  └───────────────────────────────────────────────────────┘
                                       │
                                       ▼
            ┌──────────────────────────────────────────────┐
            │ Discovery: список архивов + URL              │
            │   • из web-index.db (уже проиндексированные) │
            │   • из HuggingFace API / arXiv OAI           │
            │   • Companion Bridge fallback (CDP browser)  │
            └──────────────────────────────────────────────┘
                                       │
            ┌──────────────────────────┼──────────────────────────┐
            │                          │                          │
            ▼                          ▼                          ▼  (rayon par_iter)
       [Archive A]                [Archive B]                [Archive C]
       (zip via HTTP Range)       (tar.gz stream)            (tar.zst stream)
            │                          │                          │
            ▼                          ▼                          ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 1: Topological Index I_A (offset, size, name)            │
   │   • zip: GET bytes=-65536 (хвост) → Central Directory           │
   │   • tar.gz: потоковый header-passthrough (noseek)              │
   │   • Сохраняется в web-index.db: poler_remote_archives (url,    │
   │     size, cd_offset, cd_size, sha256_partial)                  │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 2: Selective chunk fetch C_i (size_i bytes → RAM)        │
   │   • zip: GET bytes={offset}-{offset+size}                      │
   │   • tar.gz: позиционирование потока до нужного entry           │
   │   • Буфер Ring<MappedSlice> (zero-copy, reusable alloc)       │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 3: PII preprocessor (zero-copy Cow<str>)                │
   │   src/tokenizer/pii.rs: [EMAIL]/[PHONE]/[SECRET] подмена      │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 4: Tokenizer + ε(W_k)                                    │
   │   src/tokenizer/inverted_index.rs: Unicode tokenize           │
   │   src/resonance/epsilon.rs: ε = κ·(1+ln(1+count(kw)))·        │
   │     Σ (ln N - ln freq(w))² + Σ Bonus_semantic(w)             │
   │   Streaming window W_k (например 256 токенов, hop 128)        │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 5: IIR-resonance field R[n] = ε_n + φ·R[n-1]             │
   │   src/resonance/iir_filter.rs::IirFilter::push(ε_n)           │
   │   φ ∈ [0.75, 0.90] (clamp в коде)                             │
   │   if R[n] < θ_R → DROP chunk из RAM (anti-spam filter)        │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 6: Streaming SimHash F(d_i) = sign(V) (64-bit)          │
   │   src/web/simhash.rs::simhash(tokens)                         │
   │   Bloom filter last-N seen (m=2^20, k=7) в RAM                 │
   │   if hamming(F, last_seen) ≤ τ → DROP (near-dup)              │
   │   τ = hamming_threshold(n_tokens) (адаптивный 3..10)           │
   └────────────────────────────────────────────────────────────────┘
            │
            ▼
   ┌────────────────────────────────────────────────────────────────┐
   │ Layer 7: POLER[Ψ] attention field                             │
   │   src/psi.rs::PsiField::evolve(o_t, proj)                     │
   │   o_t = Ω(ε_n) = tanh(ε_n)                                     │
   │   Π_Λ = Forbid if (epoch_out_of_range || off_topic_expansion)│
   │   ψ-weight p_t ∈ (-1, 1) — приоритет чанка                    │
   └────────────────────────────────────────────────────────────────┘
            │
            ├───────────────┬───────────────┬───────────────┐
            ▼               ▼               ▼               ▼
   [Training batch]   [RAG context]   [TUI discovery]  [SimHash update]
   weight = σ(p_t)     top-K by ψ      show in Sources   persist F(d_i)
   loss · weight       panel           panel             to web-index.db
                       context_anchor  + open in $EDITOR
```

### 3.3 Параллельный поиск по конкретным и смежным темам

Это **главное архитектурное расширение** над оригинальной идеей. Запрос пользователя — не «выкачать весь архив», а «найди в этих архивах информацию по теме X и её смежным».

```text
Input: query = "асимметричная криптография на эллиптических кривых"
        archives = [HF/datasets/…/crypto-docs.zst,
                    arxiv.org/.../cs.CR.2024.tar,
                    github.com/.../crypto-impls.tar.gz]

Step 0: Term expansion (BM25 over web-index.db)
  K_core = {elliptic curve, ECDSA, ECDH, Ed25519, secp256k1}
  K_expansion = {digital signature, public key, threshold cryptography, ...}
  K_forbidden = []  # no temporal constraints

Step 1: Parallel discovery (rayon par_iter over archives)
  for archive in archives.par_iter():
    index = build_or_load_remote_index(archive)
    candidates = index.iter_files()  # не качая содержимое!
      .filter(|entry| entry.name_hint matches K_core ∪ K_expansion)
      .collect::<Vec<_>>()

Step 2: Parallel selective fetch + filter
  for (archive, entry) in candidates.par_iter():
    chunk = fetch_chunk(archive, entry.offset, entry.size)  # kilobytes
    tokens = tokenize(PiiCleaner::clean(chunk))
    eps = calculate_epsilon(tokens, K_core, K_expansion)
    if eps < eps_min: continue  # не релевантен

    # IIR resonance across this single chunk
    R_i = IirFilter::new(phi=0.85).batch(tokens, eps_seq)

    # SimHash dedup across all chunks seen so far (Bloom in shared state)
    F_i = simhash(tokens)
    if bloom.contains(F_i, hamming_threshold(n)): continue

    # ψ-field evolution (single chunk contribution)
    psi_p = psi_field.evolve(omega(eps), Allow)

    # Save top candidates with score
    results.push((archive, entry, psi_p, F_i))

Step 3: Aggregate top-K by ψ-weight, dedup by SimHash clustering
  final = results
    .sort_by(|a, b| b.psi_p.partial_cmp(&a.psi_p))
    .dedup_by_simhash_cluster(hamming_threshold)
    .take(K)

Step 4: Dispatch to consumer (trainer / RAG / TUI / persistence)
```

## 4. Математическая модель

### 4.1 Топологическая адресация сетевого архива

Для ZIP:
- Формат: Central Directory в хвосте архива (последние `eocdr_size + cd_size` байт).
- Сложность индексации: `O(δ)`, где `δ ≈ 64 КБ` (EOCDR + CD для типичного архива).
- Доля от общего объёма: `δ / L ≈ 10⁻⁶` для 50 ГБ архива.

Для tar.gz/tar.zst:
- Формат: последовательные 512-байтные headers + данные, потом padding.
- Индексация: только потоковый passthrough, noseek — затратно для произвольного доступа, но `tar` entries обычно идут по порядку, поэтому для **параллельного поиска** находим все entry в одном проходе, потом рандомный доступ не нужен (каждый поток обрабатывает свой чанк последовательно).

Обозначения:
- `L` — общий объём архива в байтах.
- `δ_zip` — размер Central Directory (хвост архива).
- `N_A` — число файлов в архиве.
- `I_A = {(name_i, offset_i, size_i, sha256_partial_i)}_{i=1..N_A}` — топологический индекс, занимает `O(N_A)` памяти (каждый entry ~100 байт, для 1M файлов = 100 МБ индекс).

### 4.2 Локальная информационная плотность окна ε (W_k)

В poler-engine уже реализовано в `src/resonance/epsilon.rs::calculate_epsilon`. Используется как есть:

```
ε(W_k) = κ · (1 + ln(1 + count(K_core ∩ W_k))) ·
         Σ_{w ∈ Unique(W_k) \ K_core} (ln N_total − ln freq(w))² +
         Σ_{w ∈ W_k} Bonus_semantic(w)
```

Где:
- `K_core` — множество ядерных терминов запроса (из term expansion step 0).
- `W_k` — токен-окно (256 токенов, hop 128).
- `N_total` — объём токенов **обсуждаемого корпуса** (web-index.db; не всего интернета, а того что уже проиндексировано poler-engine).
- `freq(w)` — глобальная частота токена в web-index.db.
- `Bonus_semantic` — маркеры отрицаний/обязанности/критичности (`не должна`, `must`, `critical`, …) — таблица из ~100 фраз.

Физический смысл: ε(W_k) — это «квадратичная форма редкости относительно корпуса + возбуждение от ядерных терминов». Чем более редкие и релевантные термины в окне — тем выше ε.

### 4.3 IIR-поле семантического резонанса R[n]

Уже реализовано в `src/resonance/iir_filter.rs`:

```
R[n] = ε_n + φ · R[n-1]
```

Размыкание: `R[n] = Σ_{k=0..n} φ^k · ε_{n-k}`.

В streaming-режиме: `R[n]` — это **текущая смысловая плотность чанка**. Окна с `R < θ_R` (например `θ_R = 0.1 · max_R_observed`) сбрасываются из RAM до попадания в нейросеть — анти-spam фильтр.

Сложность: O(1) памяти (один `f64` аккумулятор), O(N) времени.

### 4.4 Потоковый 64-битный SimHash F(d)

Уже реализовано в `src/web/simhash.rs`:

```
F(d) = sign( Σ_{shingles ∈ SHINGLE_4(d)} hash(shingle) · e_i )
```

Где `e_i` — голосование по i-тому биту. Дедупликация: два документа считаются near-дубликатами если `hamming(F(a), F(b)) ≤ τ`, где `τ = hamming_threshold(n_tokens)` — адаптивный порог (3 на длинных ≥ 2000 токенов, до 10 на коротких).

Для streaming-режима поверх **миллиардов** чанков из интернета — Bloom filter по `F(d)`:
- Параметры: `m = 2^20` бит = 128 КБ, `k = 7` хешей.
- Ложные срабатывания: `(1 - e^(-kn/m))^k ≈ 1%` при `n = 10M` seen чанков.
- При near-dup через Bloom (для каждого seen F(d_i) сохранить в Bloom, при новом чанке проверить proximity-bit) — O(1) память на чанк.

### 4.5 Энтропия Шеннона H(W_k) — фильтр шума

**Дополнение над существующим кодом** (этого нет, надо добавить):

```
H(W_k) = - Σ_{w ∈ W_k} (freq_local(w) / |W_k|) · log2(freq_local(w) / |W_k|)
```

Где `freq_local(w)` — частота в окне W_k. H ∈ [0, log2(|Vocabulary|)]. Окна с очень низкой H (повторяющийся бред, SEO-spam) или очень высокой (случайные байты, base64 мусор) — отбрасываются. Полезный диапазон: `H ∈ [3.0, 9.0]`.

### 4.6 POLER[Ψ] ψ-поле внимания (importance sampling)

Полная формула уже в `src/psi.rs`. Развёрнутое ψ-уравнение:

```
p_{t+1} = p_t + η · Π_Λ · (-∇F + γ · ∇ε)
```

Где:
- `∇F = 2 · (p_t - Ω(o_t))`, `Ω(o_t) = tanh(ε_t)` — свободная энергия.
- `∇ε = Σ_{k=1..K} ρ^k · (p_t - s_{t-k})` — резонанс памяти.
- `Π_Λ ∈ {Allow=I, Forbid=0}` — проектор логики (запрет эпохи/темы).
- `η = 0.05`, `γ = 0.5`, `ρ = 0.9`, `K = 8` — гиперпараметры из POLER_Psi_v3.

**Importance sampling для обучения** (главное расширение над оригиналом):

```
P(d_i → batch) ∝ exp(λ₁ · ψ_p(d_i) + λ₂ · H(d_i) - λ₃ · Redundancy(d_i))
```

Где:
- `ψ_p(d_i)` — ψ-вес чанка (значение поля p после processing чанка). Бинаризованный через σ(·).
- `H(d_i)` — нормализованная энтропия Шеннона окна.
- `Redundancy(d_i) = max_seen(hamming(F(d_i), F(d_j))) / 64` — мера близости к уже видимым данным.
- `λ₁, λ₂, λ₃` — веса; рекомендуются `λ₁ = 1.0, λ₂ = 0.3, λ₃ = 0.5` (с калибровкой).

При попадании чанка в training batch:
```
loss_batch = - Σ_i w_i · log p_model(y_i | x_i, θ)
       w_i = normalize(P(d_i → batch))
```

### 4.7 Сводная таблица сложности

| Этап | Время | RAM |
|---|---|---|
| Topological index (zip) | `O(δ)` (~64 КБ download) | `O(N_A)` (~100 байт/entry) |
| Topological index (tar) | `O(L)` (полный stream) | `O(N_A)` (~100 байт/entry) |
| Selective fetch C_i | `O(size_i)` (kilobytes) | `O(size_i)` |
| PII | `O(|C_i|)` | `O(1)` (zero-copy Cow) |
| Tokenize + ε | `O(|tokens_i|)` | `O(|tokens_i|)` (zero-copy `FileTokens<'a>`) |
| IIR | `O(|tokens_i|)` | `O(1)` (f64 аккумулятор) |
| SimHash | `O(|tokens_i|)` + Bloom `O(k)` | `O(1)` (u64 fingerprint + Bloom) |
| ψ-field | `O(K)` | `O(K)` (ring buffer depth K=8) |

Итоговый RAM per archive-per-worker: ~|C_i| · 1.5 (временные данные) + O(N_A) индекс + O(Bloom) = **десятки МБ на петабайты интернета**.

## 5. CLI / API дизайн (предварительный)

```bash
# Discovery: построить топологический индекс удалённого zip без скачивания тела
poler-engine stream-archive index https://example.com/dataset.zip

# Поиск конкретных файлов по имени/паттерну в удалённом архиве
poler-engine stream-archive list https://example.com/dataset.zip "*.py"

# Извлечь один файл в stdout (zero-storage, выполняется на лету)
poler-engine stream-archive extract https://example.com/dataset.zip README.md

# Параллельный поиск по теме в нескольких архивах (главное)
poler-engine stream-search "elliptic curve cryptography" \
    --archives https://hf.co/.../crypto-docs.zst \
               https://arxiv.org/.../cs.CR.tar \
               https://github.com/.../crypto-impls.tar.gz \
    --top-k 100 \
    --output rag-context.jsonl

# Топологический индекс сохраняется в web-index.db (новая таблица
# poler_remote_archives) — повторные запросы к тому же архиву
# не требуют re-fetch Central Directory.
```

## 6. Что НЕ трогает

- `web-index.db` BM25/PageRank/IIR/SimHash ядро поиска — **расширяется**, не заменяется (новая таблица `poler_remote_archives`).
- `streaming.rs` — **расширяется** generic-трейтом `ByteStreamSource` с двумя реализациями: `MmapSource` (существующая локальная) и `HttpRangeSource` (новая для zip).
- `oauth.rs` flow — не трогается (cloud-platform scope только для NotebookLM Enterprise API).
- `GoogleHttp` (CDP) — не трогается (только для Google-профиля). Streaming Archives используют прямой `ureq` (как `GcpEnterpriseProvider`).
- `aidde/`, `parser/`, `graph/` — не трогаются (но переиспользуются когда чанки попадают в проект).

## 7. Этапы имплементации (roadmap)

| Milestone | Что делаем | Сложность |
|---|---|---|
| **SA1** | `ByteStreamSource` trait + `MmapSource` (refactor существующего `streaming.rs`) + `HttpRangeSource` (ureq, GET Range) | Средняя |
| **SA2** | Topological index builder: zip Central Directory parser (локальный + удалённый), tar streamer | Высокая |
| **SA3** | `stream-archive` CLI subcommand (index/list/extract) | Низкая |
| **SA4** | PII + ε + IIR + SimHash integration в streaming-pipeline для HTTP-source | Средняя |
| **SA5** | `stream-search` — параллельный поиск (rayon `par_iter`) + term expansion из web-index.db | Высокая |
| **SA6** | POLER[Ψ] importance sampling → JSONL output для trainer / RAG context | Средняя |
| **SA7** | TUI Sources panel — виртуальная папка удалённого архива (использует M4 EnterAction) | Средняя |

После M5 (CLI subcommands для Companion Bridge) — SA1–SA3 войдут в v0.19.0, SA4–SA5 в v0.20.0, SA6–SA7 в v0.21.0.

## 8. Риски и митигация

| Риск | Митигация |
|---|---|
| HTTP-сервер не поддерживает Range Requests | Чекаем `Accept-Ranges: bytes` в HEAD-ответе. Если нет — fallback на полный download во временный файл (`$TMP/poler-stream-<sha>.tmp`) с последующим mmap. Это уже не zero-storage, но graceful degradation. |
| rate-limit / 403 от хоста | Companion-bridge fallback на CDP-браузер (Google-профиль, anti-bot). |
| Архив повреждён (битый tail) | SHA256_partial на первых 1 МБ + сравнение с сохранённым в `poler_remote_archives`. Mismatch → re-fetch всего хвоста. |
| Bloom filter overflow (миллиарды seen hashes) | Ротация Bloom каждый N=1M seen с persisted dump в `poler_remote_archives.seen_bloom` (LZ4-compressed). |
| ψ-поле расходимость в long-running поиске | Уже зафиксировано в `src/psi.rs::evolve` через `clamp(-1, 1)` перцептивного пространства. Не нужно доп. митигации. |

## 9. Источник

Дословная фиксация из research-сессии пользователя + доработка архитектуры под существующий код poler-engine. Дата: 2026-08-26. Версия-маркер: v0.17.3 (где этот документ впервые включён в репозиторий). Не блокирует v0.18.0 (Companion Bridge) — следующая работа после M5.

//! Ядро движка: полный и инкрементальный (watcher) прогоны.
//!
//! ## Потоковая архитектура памяти (исправление bug#1 из v0.2.1)
//!
//! Прежняя схема (удержание `Vec<FileScan>` с индексами всех файлов
//! одновременно) на корпусе 340 файлов / 12.4 млн токенов раздувала RSS
//! до гигабайт. Новая схема удерживает между фазами только:
//!
//! * глобальные частоты токенов (словарь корпуса — один раз);
//! * лёгкие `HitRecord` (без текста сцен);
//! * тройки уникальных сцен (`SceneInfo`).
//!
//! Всё тяжёлое (массивы токенов, тексты сцен) — временные структуры
//! одного файла, освобождаемые сразу после его обработки.

use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

use crate::aidde::symbols::{CallSite, Definition, SymbolTable};
use crate::compression::{GlobalStats, PostingsStore, StatsRef, VocabArena};
use crate::graph::EntityGraph;
use crate::output::{ContextAnchor, NexusNode, NexusSite, SearchResult};
use crate::parser::markdown_scenes::truncate_char_safe;
use crate::parser::SceneContext;
use crate::search::intent::{hit_tier, IntentMode, QueryIntent, SignatureQuery, TypeDeclQuery};
use crate::streaming::{self, HitRecord, Pass2Result, SceneInfo};

use crate::{collect_files, with_text, EngineConfig, PiiCleaner, PiiMode, ScanStats};

/// Событие инкрементального прогона (watcher).
#[derive(Debug, Clone, Default, Serialize)]
pub struct WatchEvent {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

impl WatchEvent {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

/// Кэшированное состояние одного файла для watcher-режима.
///
/// v0.6 (bug: память на многофайловых корпусах с частым словом):
/// `records` хранит только top_n лучших по резонансу якорей файла
/// (Early Top-K Pruning), а полный список хитов — компактные
/// `hit_keys` для diff-режима и честного total_hits. Сцены, на которые
/// не ссылается ни один оставшийся якорь, удаляются.
///
/// v2.0 (Приоритет 3 — Compression): parked-состояние уплотнено:
/// * `counts` — пары `(term_id, count)` против FSST-арены словаря
///   корпуса: 8 байт на запись вместо ~64–96 у HashMap<String, usize>;
/// * `hits`/`hit_keys` — lz4-парковка постингов ([`PostingsStore`]),
///   разжатие по требованию (diff/повторный pass 2 изменённых файлов
///   получает свежие hits — сжатые копии не читаются вовсе).
#[derive(Debug, Clone)]
struct FileEntry {
    mtime: SystemTime,
    size: u64,
    /// Словарь файла парами (term_id, count) — ID из WatchState::vocab.
    counts: Vec<(u32, u32)>,
    total: u64,
    /// Индексы токенов хитов файла (lz4-парковка).
    hits: PostingsStore,
    /// Все байтовые позиции хитов файла (lz4-парковка, для diff/статистики).
    hit_keys: PostingsStore,
    /// Число хитов, прошедших temporal-фильтр (честный total_hits).
    hits_temporal: usize,
    /// v0.85.0: точный хит-определение типа (def-сигнал) — гасит
    /// гэп-fallback в инкрементальном прогоне.
    has_type_def: bool,
    /// Только top_n лучших якорей файла (по резонансу).
    records: Vec<HitRecord>,
    scenes: HashMap<(usize, usize), SceneInfo>,
}

/// Паркуемое watcher-состояние прогона: FSST-словарь корпуса (термы
/// сжаты, ID appending-only — стабильны между ресканами) + состояния
/// файлов. Именно эта структура определяет RAM индекса между прогонами:
/// v2.0 держит её в ~14 Б/терм + 8 Б/(терм,файл) против ~64–96 Б/запись
/// ранее (цель приоритета 3: 5–10×).
struct WatchState {
    vocab: VocabArena,
    files: HashMap<PathBuf, FileEntry>,
}

/// Поисковый движок с опциональным инкрементальным состоянием.
///
/// `watching = true` включает кэш по mtime/size: повторные прогоны
/// (`rescan`) не перечитывают и не ретокенизируют неизменённые файлы.
/// `diff_mode = true` (только в watcher): `rescan` возвращает якоря,
/// которых не было в предыдущем прогоне (новые file+byte_pos).
pub struct Engine {
    config: EngineConfig,
    watching: bool,
    diff_mode: bool,
    state: Option<WatchState>,
    /// Байтовые позиции хитов предыдущего прогона по файлам
    /// (lz4-парковка) — для diff-режима.
    last_hit_keys: HashMap<PathBuf, PostingsStore>,
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

impl Engine {
    pub fn new(config: EngineConfig, watching: bool) -> Self {
        Self {
            config,
            watching,
            diff_mode: false,
            state: None,
            last_hit_keys: HashMap::new(),
        }
    }

    /// Включает дифф-режим (только для watcher-прогонов).
    pub fn with_diff(mut self, on: bool) -> Self {
        self.diff_mode = on;
        self
    }

    /// Полный прогон: все файлы обрабатываются с нуля.
    pub fn scan(&mut self, target: &Path, query: &str) -> (SearchResult, ScanStats) {
        let (_ev, res, stats) = self.run(target, query, false);
        (res, stats)
    }

    /// Инкрементальный прогон: обрабатываются только добавленные,
    /// изменённые (mtime/size) файлы; удалённые исключаются из статистики.
    pub fn rescan(&mut self, target: &Path, query: &str) -> (WatchEvent, SearchResult, ScanStats) {
        self.run(target, query, true)
    }

    fn run(
        &mut self,
        target: &Path,
        query: &str,
        incremental: bool,
    ) -> (WatchEvent, SearchResult, ScanStats) {
        let started = Instant::now();
        let mut stats = ScanStats::default();
        let query_tokens: Vec<String> = query
            .split_whitespace()
            .flat_map(|w| w.split(|c: char| !c.is_alphanumeric() && c != '_'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect();
        if query_tokens.is_empty() || !target.exists() {
            return (
                WatchEvent::default(),
                SearchResult {
                    query: query.to_string(),
                    total_hits: 0,
                    soft_fallback: false,
                    soft_fallback_kind: None,
                    nexus: Vec::new(),
                    anchors: Vec::new(),
                },
                stats,
            );
        }

        let config = self.config.clone();
        let watching = self.watching;
        let cleaner = PiiCleaner::new();
        let files = collect_files(target, &config);
        stats.files_scanned = files.len();
        let pre = streaming::literal_prefilter(&query_tokens);
        // v0.82.0: намерение запроса (код ↔ проза) и сигнатурный паттерн
        // (прикреплённая скобка). Ранжирование — двухуровневым ключом
        // (тир, −R): метрики ε/R остаются честными, порядок — политикой.
        // Код-интент дополнительно включает строгий phrase-режим (без
        // proximity-AND): грамматика кода упорядочена, struct-теги
        // (`form:",default=1"`) в окне ±128 токенов — шум, не сигнал.
        let intent: QueryIntent = config.intent_mode.resolve(query);
        let strict_phrase = intent == QueryIntent::Code;
        let signature = SignatureQuery::parse(query);
        // v0.83.0: якорь идентификатора (нижний регистр — токены хранятся
        // фолдом) для ленивого мягкого fallback: pass 1 собирает
        // префиксные хиты ТОЛЬКО в файлах без точных хитов.
        let sig_prefix: Option<String> =
            signature.as_ref().map(|s| s.anchor_word.to_lowercase());
        // v0.85.0 (ГРАБЛЯ 43, LLVM): запрос-объявление типа
        // `class Name` / `struct Name` (+ хвост `:`/`{`/`;`) — мягкий
        // fallback на макро-атрибуты между keyword и именем
        // (`class LLVM_ABI Function` при запросе `class Function :`).
        // Только строгая фраза (код-интент) и без сигнатурной скобки —
        // слоя взаимно исключны по форме запроса.
        let type_decl: Option<TypeDeclQuery> = if strict_phrase && signature.is_none() {
            TypeDeclQuery::parse(query)
        } else {
            None
        };

        // ---------- классификация файлов (incremental) ----------
        // v2.0: словарь корпуса (FSST-арена) живёт в состоянии и переходит
        // из прогона в прогон; ID термов appending-only — TermIdCounts
        // сохранённых файлов остаются валидными без перекодирования.
        let (vocab, kept_from_prev): (VocabArena, HashMap<PathBuf, FileEntry>) =
            match std::mem::take(&mut self.state) {
                Some(ws) => (ws.vocab, ws.files),
                None => (VocabArena::new(), HashMap::new()),
            };
        let prev = kept_from_prev;
        let mut event = WatchEvent::default();
        let mut kept: HashMap<PathBuf, FileEntry> = HashMap::new();
        let mut to_process: Vec<PathBuf> = Vec::new();

        if incremental {
            let current: HashSet<&PathBuf> = files.iter().collect();
            for (p, e) in prev {
                if current.contains(&p) {
                    let same = fs::metadata(&p)
                        .map(|m| m.len() == e.size && m.modified().ok() == Some(e.mtime))
                        .unwrap_or(false);
                    if same {
                        kept.insert(p, e);
                    } else {
                        to_process.push(p.clone());
                        event.changed.push(p.to_string_lossy().to_string());
                    }
                } else {
                    event.removed.push(p.to_string_lossy().to_string());
                }
            }
            let mut scheduled: HashSet<PathBuf> = kept.keys().cloned().collect();
            scheduled.extend(to_process.iter().cloned());
            for p in &files {
                if !scheduled.contains(p) {
                    to_process.push(p.clone());
                    event.added.push(p.to_string_lossy().to_string());
                }
            }
        } else {
            to_process = files.clone();
        }

        // ---------- Проход 1: потоковая статистика ----------
        // v2.0 (Приоритет 3): глобальный словарь корпуса — FSST-арена
        // (интернирование + compress-probe лукапы), частоты — Vec<u32>
        // по ID. Строки не клонируются: pass 1 отдаёт Box<str>-словари,
        // арена сжимает термы на лету (после ~32 КБ staging-фазы).
        struct PassSink {
            vocab: VocabArena,
            counts: Vec<u32>,
            n_total: u64,
            hit_files: HashMap<PathBuf, Vec<u32>>,
            /// v0.83.0: кандидаты мягкого fallback — файлы без точных,
            /// но с префиксными хитами (отдельно, чтобы не смешивать семантики).
            prefix_hit_files: HashMap<PathBuf, Vec<u32>>,
            /// v0.85.0: файлы с гэп-хитами `class МАКРО Name` (без точных) —
            /// кандидаты fallback'а объявлений типов (ГРАБЛЯ 43).
            type_gap_hit_files: HashMap<PathBuf, Vec<u32>>,
            /// v0.85.0: файлы, где точный хит — определение типа (def-сигнал).
            type_def_files: HashSet<PathBuf>,
            entries: HashMap<PathBuf, FileEntry>,
        }

        impl PassSink {
            fn absorb(&mut self, path: &Path, r: streaming::Pass1File, watching: bool) {
                let meta = fs::metadata(path).ok();
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let mut entry_counts: Vec<(u32, u32)> =
                    if watching { Vec::with_capacity(r.counts.len()) } else { Vec::new() };
                for (k, v) in &r.counts {
                    let id = self.vocab.intern(k) as usize;
                    if id >= self.counts.len() {
                        self.counts.resize(id + 1, 0);
                    }
                    self.counts[id] += *v;
                    if watching {
                        entry_counts.push((id as u32, *v));
                    }
                }
                self.n_total += r.total;
                // Entry создаётся ВСЕГДА: в него pass 2 кладёт records/scenes.
                // Словарь counts (пары ID) удерживается только в watcher-режиме
                // (нужен для вычитания при инкрементальном rescan).
                // hits паркуются lz4; сырой список — только для pass 2
                // hit-файлов этого прогона.
                let hits_store = PostingsStore::from_u32(&r.hits);
                self.entries.insert(
                    path.to_path_buf(),
                    FileEntry {
                        mtime,
                        size,
                        counts: entry_counts,
                        total: r.total,
                        hits: hits_store,
                        hit_keys: PostingsStore::default(),
                        hits_temporal: 0,
                        has_type_def: r.type_def_exact,
                        records: Vec::new(),
                        scenes: HashMap::new(),
                    },
                );
                if !r.hits.is_empty() {
                    self.hit_files.insert(path.to_path_buf(), r.hits);
                    if r.type_def_exact {
                        self.type_def_files.insert(path.to_path_buf());
                    }
                } else if !r.prefix_hits.is_empty() {
                    self.prefix_hit_files.insert(path.to_path_buf(), r.prefix_hits);
                } else if !r.type_gap_hits.is_empty() {
                    // v0.85.0: ленивые гэп-кандидаты — только файлы без
                    // точных и без префиксных (слои исключны по форме запроса).
                    self.type_gap_hit_files
                        .insert(path.to_path_buf(), r.type_gap_hits);
                }
            }

            fn add_kept(&mut self, e: &FileEntry) {
                // Слияние ID-пар без строк и хеширования.
                for &(id, v) in &e.counts {
                    let id = id as usize;
                    if id >= self.counts.len() {
                        self.counts.resize(id + 1, 0);
                    }
                    self.counts[id] += v;
                }
                self.n_total += e.total;
            }
        }

        let mut sink = PassSink {
            vocab,
            counts: Vec::new(),
            n_total: 0,
            hit_files: HashMap::new(),
            prefix_hit_files: HashMap::new(),
            type_gap_hit_files: HashMap::new(),
            type_def_files: HashSet::new(),
            entries: HashMap::new(),
        };
        for e in kept.values() {
            sink.add_kept(e);
        }

        let (giants, normals) = streaming::split_giants(&to_process);
        let sink_mu = Mutex::new(sink);
        normals.par_iter().for_each(|p| {
            if let Some(r) = streaming::pass1_file(
                p,
                &query_tokens,
                strict_phrase,
                &config,
                &cleaner,
                &pre,
                sig_prefix.as_deref(),
                type_decl.as_ref(),
            ) {
                sink_mu.lock().unwrap().absorb(p, r, watching);
            }
        });
        // Гигантские файлы — строго последовательно: пик памяти ограничен
        // одним большим временным индексом.
        for p in &giants {
            if let Some(r) = streaming::pass1_file(
                p,
                &query_tokens,
                strict_phrase,
                &config,
                &cleaner,
                &pre,
                sig_prefix.as_deref(),
                type_decl.as_ref(),
            ) {
                sink_mu.lock().unwrap().absorb(p, r, watching);
            }
        }
        let mut sink = sink_mu.into_inner().unwrap();
        stats.total_tokens = sink.n_total;

        // ---------- v0.83.0: мягкий fallback сигнатурного запроса ----------
        // Протокол владельца (wireshark): точный хит со скобкой дал 0
        // результатов (`proto_register_field(` при наличии только
        // `proto_register_field_array(`) — автоматический переход на поиск
        // по префиксу идентификатора. Сигнатурный слой выключается (тиры
        // возвращает intent: код выше документации), ε считается по
        // фактическому префикс-семейству файла.
        let mut soft_prefix: Option<String> = None;
        if signature.is_some() {
            let exact_any =
                !sink.hit_files.is_empty() || kept.values().any(|e| !e.hits.is_empty());
            if !exact_any && !sink.prefix_hit_files.is_empty() {
                soft_prefix = sig_prefix.clone();
                sink.hit_files = std::mem::take(&mut sink.prefix_hit_files);
            }
        }
        // ---------- v0.85.0: мягкий fallback макро-атрибутов ----------
        // объявления типа (ГРАБЛЯ 43, LLVM/Qt/Chromium)
        //
        // Кейс владельца: `class Function :` выдал 10 forward-деклараций —
        // определение спрятано за `class LLVM_ABI Function : public …`.
        // Триггер: НИ ОДИН точный хит не является определением
        // (def-сигнал `{`/`:` после имени отсутствует глобально) — либо
        // точных нет вовсе — и есть гэп-хиты. Гэп-хиты подменяют
        // hit_files (зеркально протоколу v0.83.0): определение
        // поднимается на [1], forward-декларации не вытесняют его.
        // Точное определение без макроса гасит fallback (Blender-кейс
        // `struct BMVert {` идёт точным путём v0.84.0).
        let mut soft_type_gap = false;
        if type_decl.is_some() {
            let has_exact_def = !sink.type_def_files.is_empty()
                || kept.values().any(|e| e.has_type_def);
            if !has_exact_def && !sink.type_gap_hit_files.is_empty() {
                soft_type_gap = true;
                sink.hit_files = std::mem::take(&mut sink.type_gap_hit_files);
            }
        }
        // Эффективный сигнатурный слой для pass 2 и финальной сортировки:
        // в fallback — None (тир по intent, не по промаху сигнатуры).
        let signature_pass2: Option<&SignatureQuery> = if soft_prefix.is_some() {
            None
        } else {
            signature.as_ref()
        };

        // Между проходами: staging-фаза арены закрывается — таблица FSST
        // обучается, термы сжимаются. Проход 2 ищет частоты compress-probe
        // по сжатым байтам (декомпрессии нет на горячем пути).
        sink.vocab.ensure_compact();

        // ---------- Проход 2: только hit-файлы ----------
        stats.files_with_hits = sink
            .hit_files
            .len()
            + kept.values().filter(|e| !e.hits.is_empty()).count();
        let n_total = sink.n_total as usize;
        let gstats = StatsRef::Global(GlobalStats {
            vocab: &sink.vocab,
            counts: &sink.counts,
        });
        let mut fresh_entries = sink.entries;
        let hit_paths: Vec<PathBuf> = sink.hit_files.keys().cloned().collect();
        let (hg, hn) = streaming::split_giants(&hit_paths);

        let pass2 = |p: &Path| -> Option<Pass2Result> {
            let hits = sink.hit_files.get(p)?;
            streaming::pass2_file(
                p,
                &query_tokens,
                &config,
                &cleaner,
                &gstats,
                n_total,
                hits,
                signature_pass2,
                soft_prefix.as_deref(),
            )
        };

        let results_mu: Mutex<Vec<(PathBuf, Pass2Result)>> = Mutex::new(Vec::new());
        hn.par_iter().for_each(|p| {
            if let Some((records, scenes, hit_keys)) = pass2(p) {
                results_mu
                    .lock()
                    .unwrap()
                    .push((p.clone(), (records, scenes, hit_keys)));
            }
        });
        // Гиганты: параллельная чанковая обработка (границы сцен,
        // глобальные байтовые позиции, память ограничена чанками).
        for p in &hg {
            let Some(hits) = sink.hit_files.get(p) else {
                continue;
            };
            if let Some((records, scenes, hit_keys)) = streaming::pass2_giant_parallel(
                p,
                &query_tokens,
                strict_phrase,
                &config,
                &cleaner,
                &gstats,
                n_total,
                hits,
                signature_pass2,
                soft_prefix.as_deref(),
                if soft_type_gap { type_decl.as_ref() } else { None },
            ) {
                results_mu
                    .lock()
                    .unwrap()
                    .push((p.clone(), (records, scenes, hit_keys)));
            }
        }
        let results = results_mu.into_inner().unwrap();
        for (p, (records, scenes, hit_keys)) in results {
            // pass2 уже вернул top-N записей и ПОЛНЫЙ hit_keys; честный
            // total_hits (без temporal) = hit_keys.len(). temporal-счёт
            // приближён по top-N записям (документировано).
            let hits_temporal = match &config.temporal_filter {
                // Приближение по top-N записям: полный temporal-счёт по
                // всем хитам требует light_meta на каждую сцену (дорого
                // на суперчастотных запросах) — задокументировано в README.
                Some(filter) => records
                    .iter()
                    .filter(|r| r.metric_tag.as_deref().map_or(true, |m| m == filter))
                    .count(),
                None => hit_keys.len(),
            };
            // v2.0: lz4-парковка постингов watcher-состояния.
            let parked_hit_keys = PostingsStore::from_usize(&hit_keys);
            if let Some(e) = fresh_entries.get_mut(&p) {
                e.hit_keys = parked_hit_keys;
                e.hits_temporal = hits_temporal;
                e.records = records;
                e.scenes = scenes;
            }
        }

        // ---------- граф сущностей (бюджет рёбер) ----------
        let mut graph = EntityGraph::new();
        let mut seen: HashSet<(String, String, String)> = HashSet::new();
        let mut added_triples = 0usize;
        'graph: for e in kept.values().chain(fresh_entries.values()) {
            for info in e.scenes.values() {
                for t in &info.triples {
                    if added_triples >= config.max_graph_triples {
                        break 'graph;
                    }
                    let key = (t.subject.clone(), t.predicate.clone(), t.object.clone());
                    if seen.insert(key) {
                        graph.add_triple(
                            &t.subject,
                            &t.predicate,
                            &t.object,
                            info.metric_tag.as_deref(),
                            1.0,
                        );
                        added_triples += 1;
                    }
                }
            }
        }
        stats.graph_nodes = graph.node_count();
        stats.graph_edges = graph.edge_count();

        if let Some(sql_path) = &config.graph_export {
            let mut sql = String::new();
            graph.export_sql(&mut sql);
            let _ = fs::write(sql_path, sql);
        }

        // ---------- temporal-фильтр, сортировка, top-N ----------
        let mut records: Vec<HitRecord> = kept
            .values()
            .chain(fresh_entries.values())
            .flat_map(|e| e.records.iter().cloned())
            .collect();

        // Честный total_hits — по полному числу temporal-прошедших хитов,
        // а не по обрезанным top-N записям.
        let full_hits: usize = kept
            .values()
            .chain(fresh_entries.values())
            .map(|e| e.hits_temporal)
            .sum();
        if let Some(filter) = &config.temporal_filter {
            records.retain(|r| r.metric_tag.as_deref().map_or(true, |m| m == filter));
        }
        stats.total_hits = full_hits;

        // ---------- diff-режим: только новые якоря ----------
        // v2.0: ключи предыдущего прогона хранятся lz4-парковкой;
        // разжатие — однократное и только в diff-прогоне.
        if incremental && self.diff_mode {
            let prev: HashSet<(PathBuf, usize)> = self
                .last_hit_keys
                .iter()
                .flat_map(|(p, pk)| {
                    pk.to_usize()
                        .into_iter()
                        .map(move |b| (p.clone(), b))
                        .collect::<Vec<_>>()
                })
                .collect();
            records.retain(|r| !prev.contains(&(r.path.clone(), r.byte_pos)));
        }

        // v0.82.0: финальное ранжирование — двухуровневый ключ (тир, −R).
        // Тир — детерминированная политика: сигнатура доминирует (матч → 0,
        // промах → 2), иначе intent (класс файла конфликтует с намерением → 1).
        // ε и R не искажаются: документация с R=55000 не вытеснит код с R=4240
        // не потому что её метрика урезана, а потому что тир выше.
        let sig_active = signature_pass2.is_some();
        records.sort_by(|a, b| {
            hit_tier(intent, sig_active, a.signature_matched, &a.path)
                .cmp(&hit_tier(intent, sig_active, b.signature_matched, &b.path))
                .then_with(|| {
                    b.resonance
                        .partial_cmp(&a.resonance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.byte_pos.cmp(&b.byte_pos))
        });
        records.truncate(config.top_n);

        // ---------- K-hop: одна корневая сущность на все якоря ----------
        let root = query_tokens[0].clone();
        let k_hop = graph.extract_k_hop(
            &root,
            config.k_hop_depth,
            config.temporal_filter.as_deref(),
            config.max_relations,
        );

        // ---------- Проход 3: материализация только top-N ----------
        // v0.82.0: якорь несёт номер строки хита (1-based, 0 = неизвестно)
        // и относительный путь от корня сканирования (терминал не захламляется).
        // v0.83.0: + превью от строки хита (строка совпадения — первая).
        let mut anchors: Vec<ContextAnchor> = records
            .iter()
            .map(|r| {
                let (scene, line, preview) = materialize_scene(r, &config, &cleaner);
                let rel_file = if target.is_file() {
                    r.path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                } else {
                    r.path
                        .strip_prefix(target)
                        .ok()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(|p| p.to_string_lossy().to_string())
                };
                ContextAnchor {
                    file: r.path.to_string_lossy().to_string(),
                    rel_file,
                    line,
                    preview,
                    token: query.to_string(),
                    epsilon: round2(r.epsilon),
                    resonance: round2(r.resonance),
                    scene,
                    k_hop_relations: k_hop.clone(),
                    nexus_symbol: None,
                }
            })
            .collect();

        // ---------- v0.86.0 (ГРАБЛЯ 41, SQLite): Causal Nexus ----------
        // Кросс-языковая группировка top-N код-хитов по символу:
        // SymbolTable на hit-файлах + EntityGraph::from_symbol_table
        // (CodeSymbol-идентичность, рёбра caller —calls→ callee, K-hop).
        // Директива владельца: ИИ и человек видят ОДНОВРЕМЕННО C-ядро,
        // JNI-прослойку, заголовок и вызовы — «граф, а не костыль»
        // def-бонуса. Ранжирование не искажается: якорь получает только
        // метку nexus_symbol, порядок остаётся (тир, −R).
        let root_symbol: Option<String> = signature
            .as_ref()
            .map(|s| s.anchor_word.clone())
            .or_else(|| type_decl.as_ref().map(|td| td.name_word.clone()))
            .or_else(|| {
                // одно-токенный запрос: токен может быть именем символа
                // (`sqlite3_step`); resolve регистрозависим — несовпадение
                // деградирует в группировку по enclosing-определениям
                if query_tokens.len() == 1 {
                    Some(query_tokens[0].clone())
                } else {
                    None
                }
            });
        let (nexus, nexus_symbols) =
            build_nexus(&records, &anchors, root_symbol.as_deref(), &config);
        for (a, s) in anchors.iter_mut().zip(nexus_symbols.into_iter()) {
            a.nexus_symbol = s;
        }

        stats.elapsed_ms = started.elapsed().as_millis();

        // Парковка watcher-состояния: FSST-арена (словарь корпуса,
        // компактная фаза) + файлы с ID-парами и lz4-постингами.
        // Именно это удерживается между прогонами — целевая RAM
        // приоритета 3 (5–10× меньше HashMap-представления).
        self.state = if watching {
            let mut m = kept;
            m.extend(fresh_entries);
            Some(WatchState {
                vocab: sink.vocab,
                files: m,
            })
        } else {
            None
        };

        // Полный набор ключей хитов текущего прогона (для diff): в
        // watcher-режиме собирается из состояния всех файлов —
        // lz4-парковкой, БЕЗ материализации (PathBuf, usize)-множества.
        if watching {
            self.last_hit_keys = self
                .state
                .as_ref()
                .map(|ws| {
                    ws.files
                        .iter()
                        .filter(|(_, e)| !e.hit_keys.is_empty())
                        .map(|(p, e)| (p.clone(), e.hit_keys.clone()))
                        .collect()
                })
                .unwrap_or_default();
        }

        (
            event,
            SearchResult {
                query: query.to_string(),
                total_hits: full_hits,
                soft_fallback: soft_prefix.is_some() || soft_type_gap,
                soft_fallback_kind: if soft_prefix.is_some() {
                    Some("prefix".to_string())
                } else if soft_type_gap {
                    // v0.86.0: template-гэп отличаем от макро-гэпа —
                    // агенты различают Boost-кейс от LLVM-кейса
                    if type_decl.as_ref().is_some_and(|td| td.template) {
                        Some("template_decl_gap".to_string())
                    } else {
                        Some("type_decl_gap".to_string())
                    }
                } else {
                    None
                },
                nexus,
                anchors,
            },
            stats,
        )
    }
}

/// Полная материализация сцены одного якоря (проход 3): перечитывает
/// файл через mmap и строит SceneContext только для top-N записей.
/// Возвращает (сцена, номер строки хита 1-based, превью от строки хита).
///
/// PII-маскирование (v0.4) применяется ТОЛЬКО здесь — на тексте,
/// получаемом AI-потребителем: enclosing_scope, превью и метаданных
/// сцены. Байтовые позиции внутри конвейера остаются raw-точными;
/// маскирование не смещает индексы, потому что выполняется над готовой
/// строкой.
fn materialize_scene(
    r: &HitRecord,
    config: &EngineConfig,
    cleaner: &PiiCleaner,
) -> (SceneContext, usize, Option<String>) {
    let res = with_text(&r.path, config.max_file_bytes, |raw| {
        let text: &str = raw;
        // v0.82.0: номер строки хита — по raw-байтам ДО маскирования.
        // Граница символа: byte_pos — старт токена (всегда граница), но
        // при выходе за конец текста откатываемся к ближайшей валидной.
        let mut cut = r.byte_pos.min(text.len());
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        let line = 1 + text[..cut].bytes().filter(|&b| b == b'\n').count();
        // v0.83.0: превью, выровненное по строке хита — окно больше не
        // «уезжает вниз» за байт-сдвигом назад (wireshark-кейс с
        // закрывающим `extern "C"` вместо заголовка целевой функции).
        let line_start = text[..cut].rfind('\n').map_or(0, |p| p + 1);
        let mut preview = hit_line_preview(&text[line_start..]);
        let mut sc = if r.is_code {
            let scope =
                crate::parser::ast_code::materialize_scope(text, r.scene_key.0, r.scene_key.1);
            SceneContext::from_code_scope(&scope, &r.path, config.max_scope_bytes)
        } else {
            let bounds = SceneContext::locate(text, r.byte_pos, &r.path);
            SceneContext::build(text, &bounds, &r.path)
        };
        if config.pii_mode == PiiMode::Mask {
            sc.enclosing_scope = cleaner.clean(&sc.enclosing_scope).into_owned();
            preview = cleaner.clean(&preview).into_owned();
            for s in sc.subjects.iter_mut() {
                *s = cleaner.clean(s).into_owned();
            }
        }
        sc.enclosing_scope = truncate_char_safe(&sc.enclosing_scope, config.max_scope_bytes);
        (sc, line, Some(preview))
    });
    res.unwrap_or_else(|| {
        (
            SceneContext {
                chapter: r.path.to_string_lossy().to_string(),
                temporal_metric: None,
                location: None,
                subjects: Vec::new(),
                enclosing_scope: String::new(),
                metric_tag: None,
                subject_names: Vec::new(),
                subject_pairs: Vec::new(),
            },
            0,
            None,
        )
    })
}

/// Превью от начала строки хита (v0.83.0).
///
/// Протокол владельца: «строка хита должна быть первой или второй в
/// сниппете, а не уезжать вниз за пределы превью». Строка совпадения —
/// первая строка; до трёх строк тела следом; до [`PREVIEW_MAX_BYTES`]
/// байт, обрезка char-safe с многоточием.
fn hit_line_preview(from_line_start: &str) -> String {
    /// Потолок байт превью (char-safe обрезка + «…»).
    const PREVIEW_MAX_BYTES: usize = 512;
    /// Строк в превью: строка хита + 3 строки тела.
    const PREVIEW_MAX_LINES: usize = 4;
    let cut = truncate_char_safe(from_line_start, PREVIEW_MAX_BYTES);
    let mut lines: Vec<&str> = cut.split('\n').collect();
    // пустой хвост (последняя строка обрезана посреди файла) не показываем
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.len() > PREVIEW_MAX_LINES {
        lines.truncate(PREVIEW_MAX_LINES);
    }
    lines.join("\n")
}

/// v0.86.0 (ГРАБЛЯ 41, SQLite): построение Causal Nexus — кросс-языковой
/// группировки top-N код-хитов по символу.
///
/// Технологии ядра (директива «граф, а не костыль», session-19/20):
/// * `SymbolTable` — O(1) resolve, лексический скоуп, call-graph;
/// * `EntityGraph::from_symbol_table` — CodeSymbol-идентичность
///   (case-sensitive, module::name), рёбра caller —calls→ callee;
/// * `extract_k_hop` — BFS в обе стороны с cap(max_relations).
///
/// Группировка: хит внутри определения искомого символа → узел этого
/// символа (C-ядро и JNI-прослойка дают ОДИН узел — идентичность по
/// bare-имени); хит на строке вызова → узел с ролью call; хит в чужой
/// функции без отношения к корню — собственный узел enclosing-символа;
/// упоминание в заголовке/прототипе → роль ref. Возвращает узлы и метку
/// символа для каждого якоря (None — без группы).
///
/// Стоимость: один линейный проход по hit-файлам top-N (≤ top_n файлов,
/// line-адресация O(N) — фикс ГРАБЛИ 44) — на фоне трёх проходов
/// конвейра незаметно; проза нексусом не группируется.
fn build_nexus(
    records: &[HitRecord],
    anchors: &[ContextAnchor],
    root_symbol: Option<&str>,
    config: &EngineConfig,
) -> (Vec<NexusNode>, Vec<Option<String>>) {
    let mut symbols: Vec<Option<String>> = vec![None; records.len()];
    // только код-хиты; проза нексусом не группируется
    let code_idx: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_code)
        .map(|(i, _)| i)
        .collect();
    if code_idx.is_empty() {
        return (Vec::new(), symbols);
    }

    // hit-файлы (уникальные) — компактный набор top-N якорей
    let hit_files: Vec<PathBuf> = {
        let mut seen: HashSet<&Path> = HashSet::new();
        code_idx
            .iter()
            .map(|&i| records[i].path.as_path())
            .filter(|p| seen.insert(*p))
            .map(Path::to_path_buf)
            .collect()
    };

    // символьная таблица + граф сущностей на hit-файлах —
    // ПЕРВОЕ подключение код-символ-графа к поисковому контуру
    // (прежде — только --impact; поиск жил на нарратив-тройках сцен)
    let table = SymbolTable::build(&hit_files, config.max_file_bytes);
    let graph = EntityGraph::from_symbol_table(&table);
    let root = match root_symbol {
        Some(r) if !table.resolve(r).is_empty() => Some(r),
        _ => None,
    };

    // определения по файлам (внутри файла — в порядке байтов)
    let mut defs_by_file: HashMap<&str, Vec<&Definition>> = HashMap::new();
    for d in &table.defs {
        defs_by_file.entry(d.file.as_str()).or_default().push(d);
    }
    // вызовы по файлам — для распознавания call-сайтов на строке хита
    let mut calls_by_file: HashMap<&str, Vec<&CallSite>> = HashMap::new();
    for c in &table.calls {
        calls_by_file.entry(c.file.as_str()).or_default().push(c);
    }

    // группировка: порядок узлов = порядок первого вхождения (старший
    // якорь первым — согласовано с ранжированием (тир, −R))
    let mut node_order: Vec<String> = Vec::new();
    let mut node_sites: HashMap<String, Vec<NexusSite>> = HashMap::new();
    for &i in &code_idx {
        let r = &records[i];
        let a = &anchors[i];
        let file_s = r.path.to_string_lossy().to_string();
        let defs = defs_by_file
            .get(file_s.as_str())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        // enclosing-определение: последний def с байтом <= хита
        // (partition_point: defs отсортированы по byte внутри файла)
        let enclosing = defs
            .partition_point(|d| d.byte <= r.byte_pos)
            .checked_sub(1)
            .and_then(|k| defs.get(k).copied());

        let (sym, role, kind) = match (root, enclosing) {
            (Some(root_name), Some(d)) if d.symbol == root_name => {
                // хит внутри определения искомого символа — C-ядро
                // ИЛИ JNI-прослойка: кросс-языковая идентичность по имени
                (Some(d.symbol.clone()), "def", d.kind.clone())
            }
            (Some(root_name), _) => {
                // хит на строке вызова искомого символа? (role: call)
                let call_here = calls_by_file
                    .get(file_s.as_str())
                    .map_or(false, |v| {
                        v.iter().any(|c| {
                            c.line == a.line
                                && table
                                    .resolve(&c.callee)
                                    .iter()
                                    .any(|d| d.symbol == root_name)
                        })
                    });
                if call_here {
                    (
                        Some(root_name.to_string()),
                        "call",
                        enclosing.map_or_else(String::new, |d| d.kind.clone()),
                    )
                } else if let Some(d) = enclosing {
                    // хит в чужой функции без отношения к корню —
                    // собственный узел enclosing-символа
                    (Some(d.symbol.clone()), "def", d.kind.clone())
                } else if a
                    .preview
                    .as_deref()
                    .and_then(|p| p.lines().next())
                    .is_some_and(|l| l.contains(root_name))
                {
                    // упоминание без enclosing (заголовок/прототип)
                    (Some(root_name.to_string()), "ref", "decl".to_string())
                } else {
                    (None, "", String::new())
                }
            }
            (None, Some(d)) => (Some(d.symbol.clone()), "def", d.kind.clone()),
            (None, None) => (None, "", String::new()),
        };

        if let Some(s) = sym {
            symbols[i] = Some(s.clone());
            node_sites.entry(s.clone()).or_default().push(NexusSite {
                file: a
                    .rel_file
                    .clone()
                    .unwrap_or_else(|| a.file.clone()),
                line: a.line,
                role: role.to_string(),
                kind,
            });
            if !node_order.contains(&s) {
                node_order.push(s);
            }
        }
    }

    // K-hop рёбра каждого узла: CodeSymbol-идентичность квалифицирована
    // (module::name) — bare-символ живёт в нескольких модулях/языках;
    // нексус объединяет соседства всех его узлов в графе.
    //
    // v0.87.0 VAULT-ASM: LENS No-Hits барьер сокровищницы
    // (archive/poler-lens/src/lens_index.rs) — рёбра K-hop несут вес
    // edge.weight · 0.5^depth; связи с весом ≤ 0.05 отсекаются
    // микроядром poler_lens_filter (SSE2, работает на любом x86_64).
    // Для дистанций ≤ 4 хопа (вес ≥ 0.0625) барьер прозрачен —
    // поведение неглубоких K-hop обходов v0.86 сохранено в точности.
    let mut nexus: Vec<NexusNode> = Vec::with_capacity(node_order.len());
    for sym in node_order {
        let suffix = format!("::{sym}");
        let mut keys: Vec<String> = graph
            .node_keys()
            .into_iter()
            .filter(|k| *k == sym || k.ends_with(&suffix))
            .map(str::to_string)
            .collect();
        keys.sort(); // детерминизм обхода
        // v0.90.0 SYNAPSE-TRIT: ТОЧНЫЕ проективные веса [N:D] вместо
        // f64-кастов — ранжирование и барьер LENS без погрешности (АЗУ).
        let mut weighted: Vec<(String, String, String, crate::asm::zero_asm::Proj)> =
            Vec::new();
        let mut seen_rel: HashSet<(String, String, String)> = HashSet::new();
        for key in &keys {
            for quad in graph.extract_k_hop_proj(
                key,
                config.k_hop_depth,
                config.temporal_filter.as_deref(),
                config.max_relations,
            ) {
                let triple = (quad.0.clone(), quad.1.clone(), quad.2.clone());
                if seen_rel.insert(triple) {
                    weighted.push(quad);
                }
            }
        }
        // ТОЧНОЕ ранжирование рёбер: cmp_canonical (128-битные перекрёстные
        // произведения — ДЕЛЕНИЯ НЕТ), по убыванию lens-веса; затем top-K.
        // Прежде порядок был порядком обхода (f64 truncate без сортировки).
        weighted.sort_by(|a, b| {
            crate::asm::zero_asm::cmp_canonical(&b.3, &a.3)
        });
        weighted.truncate(config.max_relations);
        // --- LENS: точный барьер [N:D] + SIMD-компакция микроядром ---
        let n = weighted.len();
        let (relations, lens) = if n > 0 {
            // точный барьер: lens > LENS_MIN_W как сравнение канонических
            // дробей (LENS_MIN_W f32 = 13421773·2^-28 — разлагается той же
            // схемой, сравнение БЕЗ деления и без f64→f32 округления)
            let barrier = crate::graph::entity_graph::lens_weight_proj(
                crate::asm::lens_asm::LENS_MIN_W as f64,
                0,
            );
            let survivors: Vec<usize> = (0..n)
                .filter(|&i| {
                    crate::asm::zero_asm::cmp_canonical(&weighted[i].3, &barrier)
                        == std::cmp::Ordering::Greater
                })
                .collect();
            // SIMD-компакция выживших микроядром (барьер 0 — все w > 0
            // прошли точный фильтр): LENS остаётся в конвейере, но решение
            // теперь ТОЧНОЕ
            let lw: Vec<f32> = survivors
                .iter()
                .map(|&i| weighted[i].3.value_f64() as f32)
                .collect();
            let flags = vec![0u64; survivors.len()];
            let mut keep_idx = vec![0u32; survivors.len()];
            let kept = if lw.is_empty() {
                0
            } else {
                crate::asm::lens_asm::filter(&lw, &flags, &mut keep_idx, 0.0, 0, 0)
            };
            let relations: Vec<(String, String, String)> = keep_idx[..kept]
                .iter()
                .map(|&i| {
                    let (s, p, o, _) = &weighted[survivors[i as usize]];
                    (s.clone(), p.clone(), o.clone())
                })
                .collect();
            let lens = if kept < n {
                Some((kept, n - kept))
            } else {
                None
            };
            (relations, lens)
        } else {
            (Vec::new(), None)
        };
        nexus.push(NexusNode {
            symbol: sym.clone(),
            sites: node_sites.remove(&sym).unwrap_or_default(),
            relations,
            lens,
        });
    }

    (nexus, symbols)
}

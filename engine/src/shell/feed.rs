//! # feed — контекст-шлюз под действие (манифест «Работа с текстом», п.11)
//!
//! «Не подавай весь текст. Подавай релевантный контекст по энергии:
//! для понимания — высокоэнергетичные узлы и их связи; для правки —
//! низкоэнергетичные участки и то, что на них влияет. Это даёт скорость
//! и уменьшает ошибки.»
//!
//! `feed` — последний слой удобства для ИИ: он НЕ ищет (это делает
//! `Engine::scan` с ε/R/сценами/K-hop), а **собирает пакет контекста
//! под конкретное действие** и укладывает его в токен-бюджет:
//!
//! * `--for understand` — СКЕЛЕТ: якоря, ранжированные по ε (высокая
//!   ε = плотное ядро темы), + K-hop связи. Карта «что это и на
//!   чём держится».
//! * `--for edit` — ПРАВОЧНЫЙ ПАКЕТ: якоря цели, ранжированные по ε
//!   ВОЗРАСТАНИЕМ (разреженные зоны — хирургические цели — первыми),
//!   с вердиктом «плотная зона / середина / разреженная зона» по
//!   перцентилю ε, + точка определения символа + AIDDE impact-паспорт
//!   (доказанный call graph: кто сломается, если править) + связи.
//!
//! Философия вердиктов (честность метрик): ε меряет ПЛОТНОСТЬ токенов
//! вокруг совпадения — не «важность» и не нагрузку на проект. Нагрузку
//! доказывает только символьный граф: центральная функция может быть
//! определена один раз (разреженная точка) и при этом держать
//! пол-проекта.
//!
//! Оценка токенов — письменность-осознанная (через
//! [`crate::universal_letters::Script`]): CJK/Хангыль/Кана ≈ 1 токен на
//! символ, кириллица ≈ 3 символа на токен, прочее ≈ 4 символа на токен.
//! Без словарей — чистая аналитика, в духе движка.
//!
//! Мега-скоуп, не влезающий в бюджет, НЕ подаётся целиком и НЕ режется
//! по живому: пакет получает сигнатуру + фокальные окна вокруг хитов,
//! разрывы помечены честными маркерами «N строк укрыто». Сцена хранит
//! первые `max_scope_bytes` (16 КБ) скоупа — движок обязан держать память
//! O(1), — но если хит запроса лежит ЗА границей обрезки, скоуп
//! перечитывается из файла целиком: фокальные окна обязаны строиться
//! вокруг реальных хитов (JSON-элемент помечается `rehydrated: true`).
//!
//! Синтаксис (poler-shell / `--exec`):
//! ```text
//! feed <PATH> --for understand --query "<тема|сущность|символ>" [--budget N] [--relations N] [--json]
//! feed <PATH> --for edit     --query "<символ|фрагмент>"       [--budget N] [--relations N] [--json]
//! ```
//!
//! Пример для агента:
//! ```text
//! poler-engine --exec 'feed ./src --for edit --query cmd_search --budget 4000 --json'
//! ```

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::json;

use crate::universal_letters::Script;
use crate::{Engine, EngineConfig};

use super::state::ShellState;
use super::CmdResult;

/// Режим подачи: под какое действие собирается пакет.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedMode {
    Understand,
    Edit,
}

impl FeedMode {
    fn as_str(self) -> &'static str {
        match self {
            FeedMode::Understand => "understand",
            FeedMode::Edit => "edit",
        }
    }
}

/// Пул якорей, который движок даёт на один запрос (шире top-N поиска:
/// feed сам решает, кого взять в бюджет).
const POOL_TOP_N: usize = 200;
/// Глубина K-hop для пакета (2 = соседи + соседи соседей).
const FEED_K_HOP: usize = 2;
/// Токен-стоимость метаданных одного якоря в пакете (файл/глава/ε/R/субъекты).
const ITEM_META_TOKENS: usize = 24;
/// Токен-стоимость конверта пакета (заголовок, режим, бюджет, статистика).
const ENVELOPE_TOKENS: usize = 48;
/// Надбавка на одну K-hop тройку сверх её текста.
const RELATION_EXTRA_TOKENS: usize = 4;
/// Дефолтный токен-бюджет пакета.
const DEFAULT_BUDGET: usize = 8000;
/// Дефолтный потолок K-hop троек в пакете.
const DEFAULT_RELATIONS: usize = 40;
/// Максимум upstream/downstream строк в человекочитаемой карте.
const IMPACT_DISPLAY_ROWS: usize = 12;
/// Радиусы фокального окна вокруг хита (строк до/после) — от щедрого
/// к минимальному: сжатие сначала пробует широкие окна.
const FOCUS_RADII: &[usize] = &[6, 4, 3, 2, 1];
/// Токен-надбавка на JSON-поля сжатого элемента (объект `compressed`).
const COMPRESS_NOTE_TOKENS: usize = 16;
/// Конверт impact-паспорта (target/danger/счётчики) + точка определения.
const IMPACT_BASE_TOKENS: usize = 34;
/// Резерв на один элемент пакета при обрезке impact-списков (мин. сжатый вид).
const MIN_ITEM_RESERVE: usize = 160;
/// Потолок файла для перечитывания мега-скоупа (манифест O(1): гиганты
/// остаются на честном 16-КБ виде сцены с хвостовым «…»).
const REHYDRATE_FILE_MAX: usize = 16 * 1024 * 1024;
/// Потолок перечитанного скоупа (функция больше 2 МБ — патология,
/// сжатие и тогда справится видом сцены).
const REHYDRATE_SCOPE_MAX: usize = 2 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Оценка токенов (письменность-осознанная, без словарей)
// ---------------------------------------------------------------------------

/// Оценка числа LLM-токенов в тексте по письменности символов.
///
/// CJK/Хангыль/Кана ≈ 1 токен на символ; кириллица ≈ 1 токен на 3 символа
/// (BPE режет кириллицу на ~2–3-символьные куски); прочее (латиница,
/// цифры, пунктуация) ≈ 1 токен на 4 символа. Пробелы не считаются —
/// BPE обычно склеивает их со следующим куском.
pub fn estimate_tokens(text: &str) -> usize {
    let mut cjk = 0usize;
    let mut cyr = 0usize;
    let mut other = 0usize;
    for c in text.chars() {
        if c.is_whitespace() {
            continue;
        }
        match Script::from_char(c) {
            Script::Cjk | Script::Hangul | Script::Hiragana | Script::Katakana => cjk += 1,
            Script::Cyrillic => cyr += 1,
            _ => other += 1,
        }
    }
    cjk + (cyr + 2) / 3 + (other + 3) / 4
}

/// Токен-стоимость одной строки impact-списка (символ + файл + пунктуация).
fn impact_entry_tokens(name: &str, file: &str) -> usize {
    estimate_tokens(&format!("{name} {file}")) + 4
}

// ---------------------------------------------------------------------------
// Сжатие мега-скоупа: сигнатура + локальный фокус
// ---------------------------------------------------------------------------

/// Сжатый вид мега-скоупа (болезнь №2 стресс-теста libgit2: 5269/3000 ток).
///
/// Движок не режет цель правки по живому и не подаёт мега-функцию целиком:
/// пакет получает сигнатуру (контракт) и фокальные окна вокруг хитов
/// (цель правки всегда внутри окон), разрывы — честные маркеры.
struct ScopeCompression {
    /// Сигнатура + фокальные окна + маркеры пропуска.
    text: String,
    /// Число фокальных окон вокруг хитов (0 — хиты внутри сигнатуры).
    windows: usize,
    /// Строк полного скоупа, укрытых маркерами.
    skipped_lines: usize,
    /// Оценка токенов полного скоупа (до сжатия).
    full_tokens: usize,
    /// Хиты не найдены буквально (psi-резонанс и т.п.) — сигнатура + финал.
    fallback: bool,
}

/// Конец сигнатуры прозы: первые 2 непустые строки (заголовок + зачин).
fn prose_sig_end(lines: &[&str]) -> usize {
    let mut end = 0usize;
    let mut nonempty = 0usize;
    for (i, l) in lines.iter().enumerate() {
        if !l.trim().is_empty() {
            nonempty += 1;
            end = i + 1;
            if nonempty == 2 {
                break;
            }
        }
    }
    end
}

/// Склеивает регионы в текст: между ними — маркеры «N строк укрыто»
/// (`/* … */` для кода, `[…]` для прозы). Возвращает (текст, укрыто строк).
fn assemble_regions(lines: &[&str], regions: &[(usize, usize)], is_code: bool) -> (String, usize) {
    let mut text = String::new();
    let mut skipped = 0usize;
    let mut prev_end: Option<usize> = None;
    for &(s, e) in regions {
        if let Some(pe) = prev_end {
            let gap = s.saturating_sub(pe + 1);
            if gap > 0 {
                skipped += gap;
                if is_code {
                    text.push_str(&format!("    /* … {gap} строк укрыто … */\n"));
                } else {
                    text.push_str(&format!("[… {gap} строк укрыто …]\n"));
                }
            }
        }
        for l in &lines[s..=e] {
            text.push_str(l);
            text.push('\n');
        }
        prev_end = Some(e);
    }
    if let Some(pe) = prev_end {
        let tail = lines.len().saturating_sub(pe + 1);
        if tail > 0 {
            skipped += tail;
            if is_code {
                text.push_str(&format!(
                    "    /* … {tail} строк укрыто (до конца скоупа) … */\n"
                ));
            } else {
                text.push_str(&format!("[… {tail} строк укрыто (до конца скоупа) …]\n"));
            }
        }
    }
    (text, skipped)
}

/// Сжимает мега-скоуп до «сигнатура + локальный фокус».
///
/// Вместо функции на тысячи строк пакет получает: (1) сигнатуру — для
/// кода строки до первой `{` включительно (контракт функции), (2) окна
/// вокруг каждого хита запроса — цель правки всегда внутри окон,
/// (3) закрывающую строку кода; разрывы помечены честными маркерами.
/// Если бюджет меньше минимального окна — подаётся минимальный вид
/// (радиус 1, одно окно) и пакет честно флагает `over_budget`.
fn compress_scope(scope: &str, terms: &[String], max_tokens: usize) -> ScopeCompression {
    let full_tokens = estimate_tokens(scope);
    let lines: Vec<&str> = scope.lines().collect();
    if lines.is_empty() {
        return ScopeCompression {
            text: scope.to_string(),
            windows: 0,
            skipped_lines: 0,
            full_tokens,
            fallback: true,
        };
    }
    let n_lines = lines.len();
    // Стиль маркеров: код → /* … */, проза → […]
    let is_code = scope.matches(';').count() + scope.matches('{').count() >= 4;
    // Сигнатура: для кода — до первой `{` (≤15 строк), для прозы — 2 непустые.
    let sig_end = if is_code {
        lines
            .iter()
            .enumerate()
            .take(15)
            .find(|(_, l)| l.contains('{'))
            .map(|(i, _)| i + 1)
            .unwrap_or_else(|| prose_sig_end(&lines))
    } else {
        prose_sig_end(&lines)
    };
    // Хиты: строки с любым термом запроса (регистронезависимо), вне сигнатуры.
    let lower_terms: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            let low = l.to_lowercase();
            *i >= sig_end
                && lower_terms
                    .iter()
                    .any(|t| t.len() >= 2 && low.contains(t.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    // Закрывающая строка кода (видно и тело, и закрытие — валидность).
    let close_line = if is_code {
        (sig_end..n_lines).rev().find(|&i| lines[i].contains('}'))
    } else {
        None
    };

    // Радиусы 6→1; если и радиус 1 не лезет — роняем хвостовые окна
    // (цель правки важнее полноты охвата, маркеры честны).
    let regions_for = |radius: usize, cap: usize| -> (Vec<(usize, usize)>, usize) {
        let mut regions: Vec<(usize, usize)> = Vec::new();
        if sig_end > 0 {
            regions.push((0, sig_end - 1));
        }
        let mut windows = 0usize;
        for &h in &hits {
            if windows >= cap {
                break;
            }
            let start = h.saturating_sub(radius).max(sig_end);
            let end = (h + radius).min(n_lines - 1);
            if let Some(last) = regions.last_mut() {
                if start <= last.1 + 2 {
                    last.1 = last.1.max(end);
                    continue;
                }
            }
            regions.push((start, end));
            windows += 1;
        }
        if let Some(c) = close_line {
            if c >= sig_end {
                match regions.last_mut() {
                    Some(last) if c <= last.1 + 2 => last.1 = last.1.max(c),
                    _ => regions.push((c, c)),
                }
            }
        }
        (regions, windows)
    };

    let mut candidates: Vec<(usize, usize)> =
        FOCUS_RADII.iter().map(|&r| (r, usize::MAX)).collect();
    candidates.push((1, 8));
    candidates.push((1, 4));
    candidates.push((1, 2));
    candidates.push((1, 1));
    for &(radius, cap) in &candidates {
        let (regions, windows) = regions_for(radius, cap);
        let (text, skipped) = assemble_regions(&lines, &regions, is_code);
        let fits = estimate_tokens(&text) + COMPRESS_NOTE_TOKENS <= max_tokens;
        let minimal = radius == 1 && cap == 1;
        if fits || minimal {
            return ScopeCompression {
                text,
                windows,
                skipped_lines: skipped,
                full_tokens,
                fallback: hits.is_empty(),
            };
        }
    }
    unreachable!("кандидат (радиус 1, одно окно) всегда терминирует цикл")
}

/// Мега-скоуп перечитан из файла целиком (срез за 16 КБ обрезки сцены).
///
/// Сцена держит первые `max_scope_bytes` (16 КБ) скоупа — обрезка честна
/// и помечена хвостовым «…». Но хит запроса может лежать за границей
/// обрезки (мега-функция 40 КБ, хит на строке 600) — фокальные окна,
/// построенные по префиксу, промахиваются мимо цели правки. Обрезанный
/// текст — префикс скоупа: находим его в файле (со второй строки — первая
/// обрезана `.trim()` скоупа) и извлекаем скоуп целиком brace-matching'ом
/// от точки внутри префикса. `feed` — контекст-шлюз под действие: один
/// дополнительный read_file оправдан точностью пакета; проза не
/// перечитывается (её сцены — не функции), гигантские файлы — тоже.
fn rehydrate_scope(anchor: &crate::ContextAnchor) -> Option<String> {
    if !anchor.scene.enclosing_scope.ends_with('…') {
        return None; // не обрезан — сцена уже держит скоуп целиком
    }
    let file = std::path::Path::new(&anchor.file);
    let lang = crate::detect_lang(file);
    if !file.is_file() || lang == crate::CodeLang::Plain {
        return None;
    }
    let text = std::fs::read_to_string(file).ok()?;
    if text.len() > REHYDRATE_FILE_MAX {
        return None;
    }
    let prefix = anchor.scene.enclosing_scope.strip_suffix('…')?;
    // Первая строка префикса обрезана `.trim()` скоупа — ищем со второй.
    let second_line = prefix.find('\n')? + 1;
    let needle = &prefix[second_line..];
    if needle.len() < 256 {
        return None; // слишком короткая зацепка — поиск небезопасен
    }
    let probe = text.find(needle)? + needle.len() / 2;
    let scope = crate::extract_enclosing_scope(&text, probe, lang);
    // Файловый уровень / не вырос против обрезанного вида / патологический
    // размер — честно остаёмся на виде сцены.
    if scope.scope_type == "file"
        || scope.text.len() <= anchor.scene.enclosing_scope.len()
        || scope.text.len() > REHYDRATE_SCOPE_MAX
    {
        return None;
    }
    Some(scope.text)
}

// ---------------------------------------------------------------------------
// Статистика энергии пула
// ---------------------------------------------------------------------------

/// Перцентиль значения в выборке (линейная интерполяция, детерминированно).
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = p * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

/// Вердикт ПЛОТНОСТИ якоря по его перцентилю ε в пуле.
///
/// Важно (философия движка): ε измеряет плотность токенов вокруг
/// совпадения — НЕ «важность» и НЕ нагрузку на проект. Единственное
/// упоминание центрального символа — разреженная точка (игла в стоге
/// сена), а его нагрузка определяется ТОЛЬКО графом вызовов (AIDDE
/// impact), не энергией. Поэтому вердикты здесь — про плотность
/// контекста: плотная зона = сложное окружение (много редких токенов
/// рядом), разреженная = хирургическая цель для точечной правки.
fn energy_verdict(p: f64) -> &'static str {
    if p >= 0.75 {
        "плотная зона"
    } else if p <= 0.25 {
        "разреженная зона"
    } else {
        "середина"
    }
}

// ---------------------------------------------------------------------------
// Ядро: сборка пакета
// ---------------------------------------------------------------------------

/// Один элемент пакета (якорь + вердикт + стоимость).
struct FeedItem<'a> {
    anchor: &'a crate::ContextAnchor,
    tokens: usize,
    /// Только для edit-режима: вердикт по перцентилю энергии.
    verdict: Option<&'static str>,
    /// Сжатие мега-скоупа (сигнатура + фокальные окна), если применялось.
    compression: Option<ScopeCompression>,
    /// Полный текст скоупа, если сцена была обрезана по `max_scope_bytes`
    /// и файл перечитан: без сжатия подаётся именно он.
    full_scope: Option<String>,
}

impl FeedItem<'_> {
    /// Текст скоупа элемента пакета: сжатый → перечитанный → вид сцены.
    fn scope_text(&self) -> &str {
        match (&self.compression, &self.full_scope) {
            (Some(c), _) => c.text.as_str(),
            (None, Some(full)) => full.as_str(),
            (None, None) => self.anchor.scene.enclosing_scope.as_str(),
        }
    }
}

/// Внутренний прогон движка: пул якорей под запрос.
fn scan_pool(target: &PathBuf, query: &str) -> Option<crate::SearchResult> {
    let config = EngineConfig {
        top_n: POOL_TOP_N,
        k_hop_depth: FEED_K_HOP,
        ..EngineConfig::default()
    };
    let mut engine = Engine::new(config, false);
    let (res, _stats) = engine.scan(target, query);
    if res.anchors.is_empty() {
        return None;
    }
    Some(res)
}

/// AIDDE impact-паспорт символа по кодовым файлам пути (если они есть).
/// Второй элемент — сколько точек определения нашёл символьный граф
/// (0 = extern/макро: определения нет, паспорт собран по вызовам).
fn impact_for(path: &PathBuf, symbol: &str) -> Option<(crate::aidde::ImpactReport, usize)> {
    let config = EngineConfig::default();
    let files: Vec<PathBuf> = crate::collect_files(path, &config)
        .into_iter()
        .filter(|p| crate::detect_lang(p) != crate::CodeLang::Plain)
        .collect();
    if files.is_empty() {
        return None;
    }
    let table = crate::aidde::SymbolTable::build(&files, config.max_file_bytes);
    let def_count = table.resolve(symbol).len();
    crate::aidde::impact_analysis(&table, symbol, 3, 200).map(|r| (r, def_count))
}

/// Честная формулировка точки определения: сколько мест объявляет символ.
/// Точное число — в числовом поле `definition_sites`; текст грамматически
/// устойчив для любого количества.
fn definition_note(def_count: usize) -> String {
    match def_count {
        0 => "внешний/макро-символ: точки определения нет — паспорт по вызовам".into(),
        1 => "единственная точка определения (символьный граф)".into(),
        _ => "несколько точек определения — показана первая (символьный граф)".into(),
    }
}

/// Связи K-hop: движок кладёт их в каждый якорь (одни и те же для корня
/// запроса) — берём из первого и обрезаем до потолка.
fn relations_of(res: &crate::SearchResult, limit: usize) -> Vec<(String, String, String)> {
    res.anchors
        .first()
        .map(|a| a.k_hop_relations.iter().take(limit).cloned().collect())
        .unwrap_or_default()
}

fn relations_tokens(rels: &[(String, String, String)]) -> usize {
    rels.iter()
        .map(|(s, p, o)| estimate_tokens(&format!("{s} {p} {o}")) + RELATION_EXTRA_TOKENS)
        .sum()
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Сборка и исполнение feed-пакета. Возвращает текст карты (человек/LLM)
/// или JSON-пакет при `json = true`.
pub fn run_feed(
    path: &PathBuf,
    mode: FeedMode,
    query: &str,
    budget: usize,
    relations_limit: usize,
    json: bool,
) -> String {
    let Some(res) = scan_pool(path, query) else {
        return format!(
            "feed: 0 якорей по «{query}» в {}\n\
             Подсказки:\n  - проверь путь/запрос (кавычки для фраз)\n  \
             - скрытые/сюжетные связи: poler-engine {} -q \"{query}\" --resonance-mode psi\n  \
             - точный подсчёт: --grep \"{query}\" {}",
            path.display(),
            path.display(),
            path.display()
        );
    };

    let mut pool: Vec<crate::ContextAnchor> = res.anchors.clone();
    // Дедупликация сцен: разные хиты внутри одной сцены дают одинаковый
    // materialized scope — в пакет достаточно одного представителя
    // (в understand — с наибольшей ε, т.к. сортировка уже по убыванию).
    let mut seen_scenes: HashSet<String> = HashSet::new();
    pool.retain(|a| {
        let key = format!(
            "{}\x00{}",
            a.file,
            a.scene.enclosing_scope.chars().take(120).collect::<String>()
        );
        seen_scenes.insert(key)
    });
    // Детерминированный полный tiebreaker: ε → R → файл → глава.
    pool.sort_by(|a, b| {
        b.epsilon
            .partial_cmp(&a.epsilon)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.resonance
                    .partial_cmp(&a.resonance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.scene.chapter.cmp(&b.scene.chapter))
    });
    let eps_sorted: Vec<f64> = {
        let mut v: Vec<f64> = pool.iter().map(|a| a.epsilon).collect();
        v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        v
    };
    let n = pool.len();
    let eps_median = percentile(&eps_sorted, 0.5);
    let eps_q3 = percentile(&eps_sorted, 0.75);
    let eps_max = eps_sorted[n - 1];

    // -------- edit-режим: разреженные зоны первыми (ε по возрастанию) --------
    let (items_order, verdicts): (Vec<usize>, Option<Vec<&'static str>>) = match mode {
        FeedMode::Understand => ((0..n).collect(), None),
        FeedMode::Edit => {
            let mut idx: Vec<usize> = (0..n).collect();
            idx.sort_by(|&i, &j| {
                pool[i]
                    .epsilon
                    .partial_cmp(&pool[j].epsilon)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| pool[i].file.cmp(&pool[j].file))
            });
            // Перцентиль каждого якоря в пуле: rank = числу значений ε
            // строго меньших (partition_point — стабильная нижняя граница;
            // равные энергии получают одинаковый ранг).
            let all_equal = eps_max == eps_sorted[0];
            let v: Vec<&'static str> = idx
                .iter()
                .map(|&i| {
                    let p = if n == 1 || all_equal {
                        0.5
                    } else {
                        let rank = eps_sorted.partition_point(|x| x < &pool[i].epsilon);
                        rank as f64 / (n - 1) as f64
                    };
                    energy_verdict(p)
                })
                .collect();
            (idx, Some(v))
        }
    };

    // -------- связи и impact: резервируются до заполнения --------
    let rels = relations_of(&res, relations_limit);
    let rel_cost = relations_tokens(&rels);

    let impact_pack = if mode == FeedMode::Edit {
        impact_for(path, query.trim())
    } else {
        None
    };
    let impact = impact_pack.as_ref().map(|(r, _)| r);
    let def_count = impact_pack.as_ref().map(|(_, c)| *c).unwrap_or(0);
    // -------- impact: обрезка списков под бюджет --------
    // Паспорт с 200+ потребителями дороже самого кода: счётчики полные
    // (граф знает всех — это дёшево и честно), списки режутся под кап
    // бюджета, upstream первыми («кто сломается» важнее всего).
    let (up_shown, down_shown, impact_cost) = match impact {
        None => (usize::MAX, usize::MAX, 0usize),
        Some(r) => {
            let full = IMPACT_BASE_TOKENS
                + r
                    .structural_relations
                    .upstream_dependents
                    .iter()
                    .map(|d| impact_entry_tokens(&d.caller, &d.file))
                    .sum::<usize>()
                + r
                    .structural_relations
                    .downstream_dependencies
                    .iter()
                    .map(|d| impact_entry_tokens(&d.callee, &d.file))
                    .sum::<usize>();
            let cap = budget
                .saturating_sub(ENVELOPE_TOKENS + rel_cost + MIN_ITEM_RESERVE)
                .min(full);
            if full <= cap {
                (usize::MAX, usize::MAX, full)
            } else {
                let mut t = IMPACT_BASE_TOKENS;
                // upstream — 70% списочного резерва («кто сломается»
                // важнее), downstream — остаток, чтобы зависимости
                // не обнулялись при тесном бюджете.
                let up_budget = if r.structural_relations.downstream_dependencies.is_empty() {
                    cap
                } else {
                    IMPACT_BASE_TOKENS + (cap - IMPACT_BASE_TOKENS) * 7 / 10
                };
                let mut up = 0usize;
                for d in r.structural_relations.upstream_dependents.iter() {
                    let c = impact_entry_tokens(&d.caller, &d.file);
                    if t + c > up_budget {
                        break;
                    }
                    t += c;
                    up += 1;
                }
                let mut down = 0usize;
                for d in r.structural_relations.downstream_dependencies.iter() {
                    let c = impact_entry_tokens(&d.callee, &d.file);
                    if t + c > cap {
                        break;
                    }
                    t += c;
                    down += 1;
                }
                (up, down, t)
            }
        }
    };

    // -------- жадное заполнение бюджета --------
    let mut spent = ENVELOPE_TOKENS + rel_cost + impact_cost;
    let mut selected: Vec<FeedItem> = Vec::new();
    for (pos, &i) in items_order.iter().enumerate() {
        let a = &pool[i];
        // Мега-скоуп: сцена держит первые 16 КБ (max_scope_bytes, O(1)-память) —
        // перечитываем файл и извлекаем функцию целиком: фокальные окна
        // обязаны строиться вокруг реальных хитов, даже за границей обрезки.
        let full_scope = rehydrate_scope(a);
        let scope_text: &str = full_scope
            .as_deref()
            .unwrap_or(a.scene.enclosing_scope.as_str());
        let full_cost = estimate_tokens(scope_text) + ITEM_META_TOKENS;
        // Мега-скоуп первого якоря: сигнатура + локальный фокус, а не весь
        // мега-файл целиком (стресс-тест libgit2: 5269/3000 ток). Цель
        // правки остаётся в пакете — внутри фокальных окон вокруг хитов.
        let (cost, compression) = if selected.is_empty() && spent + full_cost > budget {
            let max_item = budget.saturating_sub(spent + ITEM_META_TOKENS + COMPRESS_NOTE_TOKENS);
            let mut terms: Vec<String> =
                query.split_whitespace().map(str::to_string).collect();
            if !terms.iter().any(|t| t.eq_ignore_ascii_case(&a.token)) {
                terms.push(a.token.clone());
            }
            let c = compress_scope(scope_text, &terms, max_item);
            let cost = estimate_tokens(&c.text) + ITEM_META_TOKENS + COMPRESS_NOTE_TOKENS;
            (cost, Some(c))
        } else {
            (full_cost, None)
        };
        if !selected.is_empty() && spent + cost > budget {
            continue; // не влезает — пробуем следующие (они могут быть короче)
        }
        spent += cost;
        selected.push(FeedItem {
            anchor: a,
            tokens: cost,
            verdict: verdicts.as_ref().map(|v| v[pos]),
            compression,
            full_scope,
        });
    }

    let coverage = if res.total_hits > 0 {
        100.0 * selected.len().min(res.total_hits) as f64 / res.total_hits as f64
    } else {
        0.0
    };
    // Честная пометка: мега-скоуп первого якоря сжимается до сигнатуры +
    // фокальных окон (цель правки — внутри окон); превышение означает,
    // что даже сжатый вид не влез (бюджет меньше минимального окна).
    let over_budget = spent > budget;

    if json {
        let items_json: Vec<serde_json::Value> = selected
            .iter()
            .map(|it| {
                let a = it.anchor;
                let mut v = json!({
                    "file": a.file,
                    "chapter": a.scene.chapter,
                    "epsilon": round2(a.epsilon),
                    "resonance": round2(a.resonance),
                    "subjects": a.scene.subjects,
                    "tokens": it.tokens,
                    "scope": it.scope_text(),
                });
                if let Some(l) = &a.scene.location {
                    v["location"] = json!(l);
                }
                if let Some(m) = &a.scene.temporal_metric {
                    v["temporal_metric"] = json!(m);
                }
                if let Some(verdict) = it.verdict {
                    v["density_verdict"] = json!(verdict);
                }
                if let Some(c) = &it.compression {
                    v["compressed"] = json!({
                        "applied": true,
                        "windows": c.windows,
                        "skipped_lines": c.skipped_lines,
                        "full_tokens": c.full_tokens,
                        "fallback": c.fallback,
                        "note": "мега-скоуп подан сжато: сигнатура + фокальные окна вокруг хитов",
                    });
                }
                if it.full_scope.is_some() {
                    v["rehydrated"] = json!(true);
                }
                v
            })
            .collect();
        let rels_json: Vec<serde_json::Value> = rels
            .iter()
            .map(|(s, p, o)| json!([s, p, o]))
            .collect();
        let mut packet = json!({
            "feed": mode.as_str(),
            "query": query,
            "path": path.display().to_string(),
            "budget_tokens": budget,
            "spent_tokens": spent,
            "pool_anchors": n,
            "total_hits": res.total_hits,
            "selected": selected.len(),
            "coverage_pct": round2(coverage),
            "over_budget": over_budget,
            "energy": {
                "median": round2(eps_median),
                "q3": round2(eps_q3),
                "max": round2(eps_max),
            },
            "items": items_json,
            "relations": rels_json,
        });
        if let Some(r) = impact {
            // Точка определения — «игла в стоге сена»: символьный граф доказал,
            // ГДЕ и СКОЛЬКО раз символ объявлен. Это точка истины правки —
            // она НЕ выводится из ε (ε меряет плотность, не нагрузку).
            packet["definition"] = json!({
                "symbol": r.target_function,
                "file": r.file,
                "lines": r.lines,
                "definition_sites": def_count,
                "note": definition_note(def_count),
            });
            let up_total = r.structural_relations.upstream_dependents.len();
            let down_total = r.structural_relations.downstream_dependencies.len();
            let up_n = up_shown.min(up_total);
            let down_n = down_shown.min(down_total);
            let mut impact_json = json!({
                "target": r.target_function,
                "file": r.file,
                "lines": r.lines,
                "danger_level_if_modified": r.danger_level_if_modified,
                "upstream_total": up_total,
                "upstream_shown": up_n,
                "upstream_dependents": r.structural_relations.upstream_dependents
                    .iter()
                    .take(up_n)
                    .map(|d| json!({"caller": d.caller, "file": d.file}))
                    .collect::<Vec<_>>(),
                "downstream_total": down_total,
                "downstream_shown": down_n,
                "downstream_dependencies": r.structural_relations.downstream_dependencies
                    .iter()
                    .take(down_n)
                    .map(|d| json!({"callee": d.callee, "file": d.file}))
                    .collect::<Vec<_>>(),
                "triage_alerts": r.heuristic_triage_alerts
                    .iter()
                    .map(|a| json!({"marker": a.marker, "description": a.description}))
                    .collect::<Vec<_>>(),
            });
            if up_n < up_total || down_n < down_total {
                impact_json["truncated"] = json!(true);
                impact_json["note"] =
                    json!("списки обрезаны под токен-бюджет; счётчики полные — граф знает всех");
            }
            packet["impact"] = impact_json;
        }
        packet["advice"] = json!(advice_text(
            mode,
            query,
            &selected,
            impact,
            def_count,
            n,
            eps_median
        ));
        serde_json::to_string_pretty(&packet).unwrap_or_else(|_| "{}".into())
    } else {
        render_card(
            mode,
            query,
            path,
            budget,
            spent,
            over_budget,
            n,
            res.total_hits,
            coverage,
            eps_median,
            eps_q3,
            eps_max,
            &selected,
            &rels,
            impact,
            def_count,
        )
    }
}

/// Текстовый совет: как агенту использовать пакет.
fn advice_text(
    mode: FeedMode,
    query: &str,
    selected: &[FeedItem],
    impact: Option<&crate::aidde::ImpactReport>,
    def_count: usize,
    pool: usize,
    eps_median: f64,
) -> String {
    match mode {
        FeedMode::Understand => {
            let top = selected
                .first()
                .map(|it| round2(it.anchor.epsilon))
                .unwrap_or(0.0);
            format!(
                "Подай модели этот пакет как ядро темы «{query}»: {} якорей из пула {} \
                 (медиана плотности пула ε={:.1}, топ ε={:.1}). Плотные узлы — \
                 тематическое ядро: держи их в рабочей памяти, остальное догружай \
                 точечными запросами.",
                selected.len(),
                pool,
                eps_median,
                top
            )
        }
        FeedMode::Edit => {
            let mut s = String::new();
            let verdicts: Vec<&str> = selected
                .iter()
                .filter_map(|it| it.verdict)
                .collect();
            let sparse = verdicts
                .iter()
                .filter(|v| **v == "разреженная зона")
                .count();
            let dense = verdicts.iter().filter(|v| **v == "плотная зона").count();
            // Точка истины — честно о числе определений: символьный граф ЗНАЕТ,
            // где символ объявлен; ε этого не знает (плотность ≠ нагрузка).
            match (impact, def_count) {
                (Some(r), 1) => s.push_str(&format!(
                    "Точка истины: «{query}» определён ровно один раз: {} строки {} \
                     (найдено символьным графом, не угадано). ",
                    r.file, r.lines
                )),
                (Some(r), n) if n > 1 => s.push_str(&format!(
                    "«{query}» объявлен в {n} местах (первая: {} строки {}): \
                     проверь, та ли точка попала в пакет. ",
                    r.file, r.lines
                )),
                (Some(_), _) => s.push_str(
                    "Точка определения не найдена (внешний/макро-символ): \
                     паспорт собран по вызовам. ",
                ),
                (None, _) => s.push_str(&format!(
                    "Символ «{query}» не найден в кодовом call graph (или корпус текстовый): \
                     опирайся на связи сущности и плотность зон. ",
                )),
            }
            if let Some(r) = impact {
                s.push_str(&format!(
                    "Правка заденет доказанный call graph: {} upstream-потребителей, \
                     {} downstream-зависимостей, danger: {}. ",
                    r.structural_relations.upstream_dependents.len(),
                    r.structural_relations.downstream_dependencies.len(),
                    r.danger_level_if_modified
                ));
            }
            // Нагрузка на проект — только от графа; плотность — только от ε.
            // Эти вещи НЕ синонимы: центральная функция может быть определена
            // один раз (разреженная точка) и одновременно держать пол-проекта.
            let graph_load = impact
                .map(|r| r.structural_relations.upstream_dependents.len())
                .unwrap_or(0);
            if graph_load >= 10 {
                s.push_str(&format!(
                    "Нагрузка на граф ВЫСОКАЯ ({} потребителей): меняй контракт \
                     только осознанно. ",
                    graph_load
                ));
            }
            s.push_str(&format!(
                "Сами правки делай в разреженных зонах (низкая ε — локальные, \
                 изолированные контексты): их {} из {}; плотных зон (сложное \
                 окружение) — {}. Низкая ε — не шум, а точечная цель.",
                sparse,
                verdicts.len(),
                dense
            ));
            s
        }
    }
}

/// Человекочитаемая карта пакета (LLM читает её одинаково хорошо).
#[allow(clippy::too_many_arguments)]
fn render_card(
    mode: FeedMode,
    query: &str,
    path: &PathBuf,
    budget: usize,
    spent: usize,
    over_budget: bool,
    pool: usize,
    total_hits: usize,
    coverage: f64,
    eps_median: f64,
    eps_q3: f64,
    eps_max: f64,
    selected: &[FeedItem],
    rels: &[(String, String, String)],
    impact: Option<&crate::aidde::ImpactReport>,
    def_count: usize,
) -> String {
    let mut out = String::new();
    let icon = if mode == FeedMode::Understand {
        "🧠"
    } else {
        "🛠"
    };
    out.push_str(&format!(
        "{icon} FEED · {} · «{query}»\n",
        mode.as_str()
    ));
    out.push_str(&format!(
        "Путь: {} | Пул: {} якорей (всего хитов: {}) | Выбрано: {} | Бюджет: {}/{} ток (охват ≈{:.1}%)\n",
        path.display(),
        pool,
        total_hits,
        selected.len(),
        spent,
        budget,
        coverage
    ));
    if over_budget {
        out.push_str(
            "⚠ Первый якорь даже в сжатом виде (сигнатура + фокальные окна) превышает бюджет:\n\
             подними --budget либо уточни запрос для меньших скоупов.\n",
        );
    } else if let Some(c) = selected.first().and_then(|it| it.compression.as_ref()) {
        out.push_str(&format!(
            "ℹ Мега-скоуп первого якоря подан сжато: сигнатура + {} фокальных окон вокруг хитов\n\
             ({} строк укрыто маркерами, полный размер {} ток). Цель правки — внутри окон.\n",
            c.windows, c.skipped_lines, c.full_tokens
        ));
    }
    out.push_str(&format!(
        "Энергия пула: медиана ε={:.1} · Q3 ε={:.1} · max ε={:.1}\n",
        eps_median, eps_q3, eps_max
    ));

    out.push('\n');
    match mode {
        FeedMode::Understand => {
            out.push_str("── СКЕЛЕТ (высокоэнергетичные узлы темы) ──\n");
            for (i, it) in selected.iter().enumerate() {
                let a = it.anchor;
                out.push_str(&format!(
                    "\n{}. ε={:.1} R={:.1} | {} | {}\n",
                    i + 1,
                    a.epsilon,
                    a.resonance,
                    a.file,
                    a.scene.chapter
                ));
                if !a.scene.subjects.is_empty() {
                    out.push_str(&format!(
                        "   субъекты: {}\n",
                        a.scene.subjects.join("; ")
                    ));
                }
                if let Some(l) = &a.scene.location {
                    out.push_str(&format!("   {l}\n"));
                }
                out.push_str(&format!(
                    "\n```text\n{}\n```\n",
                    it.scope_text()
                ));
            }
        }
        FeedMode::Edit => {
            out.push_str("── ЦЕЛЬ ПРАВКИ (разреженные зоны первыми) ──\n");
            for (i, it) in selected.iter().enumerate() {
                let a = it.anchor;
                let verdict = it.verdict.unwrap_or("середина");
                out.push_str(&format!(
                    "\n{}. ε={:.1} [{verdict}] | {} | {}\n",
                    i + 1,
                    a.epsilon,
                    a.file,
                    a.scene.chapter
                ));
                if let Some(l) = &a.scene.location {
                    out.push_str(&format!("   {l}\n"));
                }
                out.push_str(&format!(
                    "\n```text\n{}\n```\n",
                    it.scope_text()
                ));
            }
        }
    }

    if let Some(r) = impact {
        out.push_str("\n── ВЛИЯНИЕ (AIDDE, доказано call graph) ──\n");
        out.push_str(&format!(
            "target: {} | {} | строки {} | danger: {}\n",
            r.target_function, r.file, r.lines, r.danger_level_if_modified
        ));
        out.push_str(&format!(
            "точка определения: {}\n",
            match def_count {
                0 => "нет — внешний/макро-символ (паспорт по вызовам)".to_string(),
                1 => "единственная (символьный граф)".to_string(),
                n => format!("в {n} местах — показана первая (символьный граф)"),
            }
        ));
        out.push_str(&format!(
            "⬆ upstream ({}):\n",
            r.structural_relations.upstream_dependents.len()
        ));
        for d in r.structural_relations.upstream_dependents.iter().take(IMPACT_DISPLAY_ROWS) {
            out.push_str(&format!("   • {} ({})\n", d.caller, d.file));
        }
        if r.structural_relations.upstream_dependents.len() > IMPACT_DISPLAY_ROWS {
            out.push_str(&format!(
                "   … и ещё {} (полный список знает граф: impact <PATH> <SYM>)\n",
                r.structural_relations.upstream_dependents.len() - IMPACT_DISPLAY_ROWS
            ));
        }
        out.push_str(&format!(
            "⬇ downstream ({}):\n",
            r.structural_relations.downstream_dependencies.len()
        ));
        for d in r
            .structural_relations
            .downstream_dependencies
            .iter()
            .take(IMPACT_DISPLAY_ROWS)
        {
            out.push_str(&format!("   • {} ({})\n", d.callee, d.file));
        }
    }

    if !rels.is_empty() {
        out.push_str("\n── СВЯЗИ (K-hop графа сущностей) ──\n");
        for (s, p, o) in rels {
            out.push_str(&format!("• {s} —{p}→ {o}\n"));
        }
    }

    out.push_str("\n── СОВЕТ ──\n");
    out.push_str(&advice_text(
        mode,
        query,
        selected,
        impact,
        def_count,
        pool,
        eps_median,
    ));
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Команда шелла
// ---------------------------------------------------------------------------

/// Справка по команде.
pub fn feed_usage() -> String {
    "feed <PATH> --for <understand|edit> --query \"<тема|символ>\" [опции]\n\
     Контекст-шлюз под действие (манифест п.11): пакет контекста по энергии,\n\
     уложенный в токен-бюджет — то, что подавать модели вместо всего текста.\n\n\
     Режимы:\n\
       --for understand  СКЕЛЕТ: плотное ядро темы по ε + K-hop связи\n\
                         (для понимания: «что это и на чём держится»)\n\
       --for edit        ПРАВОЧНЫЙ ПАКЕТ: цели по ε ВОЗРАСТАНИЮ (разреженные\n\
                         зоны первыми), вердикты «плотная/середина/разреженная\n\
                         зона», точка определения + AIDDE impact (кто\n\
                         сломается) + связи сущности\n\n\
     Опции:\n\
       --query, -q \"...\"   запрос: сущность/фраза/символ (обязателен)\n\
       --budget, -b N       токен-бюджет пакета (дефолт 8000)\n\
       --relations, -r N    потолок K-hop троек (дефолт 40)\n\
       --json               машиночитаемый пакет вместо карты\n\n\
     Примеры:\n\
       feed ./src --for edit --query cmd_search --budget 4000 --json\n\
       feed ./book --for understand --query \"Нокс\" --budget 6000\n\
       poler-engine --exec 'feed . --for understand -q \"архитектура\" --json'"
        .into()
}

/// Точка входа команды шелла: `feed <PATH> ...`.
pub fn cmd_feed(state: &mut ShellState, args: &[String]) -> CmdResult {
    let mut path: Option<PathBuf> = None;
    let mut mode = FeedMode::Understand;
    let mut query = String::new();
    let mut budget = DEFAULT_BUDGET;
    let mut relations = DEFAULT_RELATIONS;
    let mut json = false;

    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--for" | "--mode" | "-f" => {
                if let Some(v) = args.get(i + 1) {
                    match v.as_str() {
                        "understand" | "u" | "понять" => mode = FeedMode::Understand,
                        "edit" | "e" | "правка" => mode = FeedMode::Edit,
                        other => {
                            return CmdResult::Done(format!(
                                "feed: неизвестный режим {other} (understand | edit)"
                            ))
                        }
                    }
                    i += 2;
                    continue;
                }
            }
            "--query" | "-q" => {
                if let Some(v) = args.get(i + 1) {
                    query = v.clone();
                    i += 2;
                    continue;
                }
            }
            "--budget" | "-b" => {
                if let Some(v) = args.get(i + 1) {
                    if let Ok(n) = v.parse::<usize>() {
                        budget = n.max(64);
                        i += 2;
                        continue;
                    }
                }
                return CmdResult::Done("feed --budget: ожидается число токенов (≥64)".into());
            }
            "--relations" | "-r" => {
                if let Some(v) = args.get(i + 1) {
                    if let Ok(n) = v.parse::<usize>() {
                        relations = n;
                        i += 2;
                        continue;
                    }
                }
                return CmdResult::Done("feed --relations: ожидается число".into());
            }
            "--json" => {
                json = true;
                i += 1;
                continue;
            }
            "--help" | "-h" => return CmdResult::Done(feed_usage()),
            other if other.starts_with("--") => {
                return CmdResult::Done(format!("feed: неизвестный флаг {other}"));
            }
            other if path.is_none() && !other.is_empty() => {
                path = Some(PathBuf::from(other));
            }
            _ => {}
        }
        i += 1;
    }

    let Some(path) = path else {
        return CmdResult::Done(feed_usage());
    };
    if !path.exists() {
        return CmdResult::Done(format!("feed: путь не найден: {}", path.display()));
    }
    if query.trim().is_empty() {
        return CmdResult::Done(
            "feed: укажите --query \"<тема|символ>\" (пример: feed ./src --for edit -q cmd_search)"
                .into(),
        );
    }

    let out = run_feed(&path, mode, &query, budget, relations, json);
    state.set_output(out.clone());
    CmdResult::Done(out)
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Временный каталог с уникальным именем (best-effort очистка вручную).
    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("poler-feed-test-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("temp dir");
        d
    }

    #[test]
    fn estimate_tokens_scripts() {
        // CJK: 1 токен на символ.
        assert_eq!(estimate_tokens("你好世界"), 4);
        // Кириллица: 3 символа на токен (5 символов → 2).
        assert_eq!(estimate_tokens("этапа"), 2);
        // Латиница: 4 символа на токен (7 символов → 2).
        assert_eq!(estimate_tokens("network"), 2);
        // Пробелы не считаются.
        assert_eq!(estimate_tokens("   "), 0);
        // Смешанный текст: 4 CJK + 7 латиницы = 4 + 2 = 6.
        assert_eq!(estimate_tokens("dragons 你好世界"), 6);
    }

    #[test]
    fn feed_understand_respects_budget_and_energy_order() {
        let d = temp_dir("und");
        std::fs::write(
            d.join("a.md"),
            "# Глава 1\n\nНокс вошла в Разлом Каньона. Кристалл буфера гудел, сектор\n\
             захлопнулся за спиной. Этерия слушала море, и море отвечало ей\n\
             тишиной. Каждый мускул держал напряжение буферного поля.\n",
        )
        .unwrap();
        std::fs::write(
            d.join("b.md"),
            "# Глава 2\n\nНокс. Просто упоминание.\n",
        )
        .unwrap();
        let out = run_feed(&d, FeedMode::Understand, "Нокс", 10_000, 40, false);
        assert!(out.contains("FEED · understand"), "карта режима: {out}");
        assert!(out.contains("СКЕЛЕТ"), "секция скелета: {out}");
        assert!(out.contains("Бюджет:"), "строка бюджета: {out}");
        // Первый выбранный якорь — из плотного файла (ε выше, чем у b.md).
        let first_scope_pos = out.find("```text").expect("скоуп в карте");
        assert!(
            out[..first_scope_pos].contains("a.md"),
            "первым должен идти плотный a.md: {}",
            &out[..first_scope_pos]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_understand_json_packet_shape() {
        let d = temp_dir("json");
        std::fs::write(
            d.join("a.md"),
            "# Глава 1\n\nСлепой Пастырь Экхарт шёл по Архисфере. Сеть Юниций\n\
             молчала. Протока светилась в темноте, и Одесса спала.\n",
        )
        .unwrap();
        let out = run_feed(&d, FeedMode::Understand, "Экхарт", 8000, 40, true);
        let v: serde_json::Value = serde_json::from_str(&out).expect("валидный JSON");
        assert_eq!(v["feed"], "understand");
        assert_eq!(v["query"], "Экхарт");
        assert!(v["spent_tokens"].as_u64().unwrap() > 0);
        assert!(v["items"].as_array().unwrap().len() >= 1);
        assert!(v["energy"]["median"].is_f64());
        assert!(v["advice"].as_str().unwrap().contains("тематическое ядро"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_edit_low_energy_first_with_verdicts() {
        let d = temp_dir("edit");
        std::fs::write(
            d.join("dense.md"),
            "# Глава 1\n\nНокс вонзила когти в буферное поле. Разлом Каньона\n\
             держал удар: кристалл, сектор, Архисфера, Сеть Юниций — всё\n\
             гудело от напряжения. Этерия и Протока, Одесса и Пастырь.\n",
        )
        .unwrap();
        std::fs::write(d.join("sparse.md"), "# Глава 2\n\nНокс мимоходом.\n").unwrap();
        let out = run_feed(&d, FeedMode::Edit, "Нокс", 10_000, 40, false);
        assert!(out.contains("FEED · edit"), "карта edit: {out}");
        assert!(out.contains("ЦЕЛЬ ПРАВКИ"), "секция цели: {out}");
        assert!(
            out.contains("разреженная зона")
                || out.contains("плотная зона")
                || out.contains("середина"),
            "вердикты плотности: {out}"
        );
        assert!(out.contains("СОВЕТ"), "совет: {out}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_edit_code_impact_block() {
        // Вендор-устойчивость: собственный исходник движка всегда рядом.
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let src = project.join("src/shell");
        if !src.exists() {
            return;
        }
        let out = run_feed(&src, FeedMode::Edit, "cmd_search", 60_000, 40, false);
        assert!(out.contains("FEED · edit"), "режим: {out}");
        assert!(out.contains("ВЛИЯНИЕ"), "AIDDE-блок: {out}");
        assert!(
            out.contains("точка определения"),
            "точка определения в карте: {out}"
        );
        assert!(
            out.contains("upstream") || out.contains("downstream"),
            "call graph в карте: {out}"
        );
    }

    #[test]
    fn feed_no_hits_honest_message() {
        let d = temp_dir("empty");
        std::fs::write(d.join("x.md"), "# Глава 1\n\nСовершенно посторонний текст.\n").unwrap();
        let out = run_feed(&d, FeedMode::Understand, "квадроберинг", 8000, 40, false);
        assert!(out.contains("0 якорей"), "честный ноль: {out}");
        assert!(out.contains("psi"), "подсказка про psi: {out}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_dedups_same_scene() {
        let d = temp_dir("dedup");
        // Два упоминания запроса внутри ОДНОЙ сцены → один элемент пакета.
        std::fs::write(
            d.join("a.md"),
            "# Глава 1\n\nНокс прошла по коридору. Нокс остановилась у окна.\n\n# Глава 2\n\nНокс снова вышла ночью, и фонари дрожали в тумане,\n\nа город спал под низким небом, и снег шёл медленно,\n\nпока не закончилась страница и глава.\n",
        )
        .unwrap();
        let out = run_feed(&d, FeedMode::Understand, "Нокс", 8000, 40, true);
        let v: serde_json::Value = serde_json::from_str(&out).expect("JSON");
        let items = v["items"].as_array().unwrap();
        // Сцена одна (маркдаун-сцена = секция) — повторение внутри неё
        // не должно раздувать пакет.
        let mut keys = HashSet::new();
        for it in items {
            let scope = it["scope"].as_str().unwrap().to_string();
            keys.insert(scope.chars().take(40).collect::<String>());
        }
        assert_eq!(keys.len(), items.len(), "дубликаты сцен в пакете: {out}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_over_budget_marker_is_honest() {
        let d = temp_dir("over");
        std::fs::write(
            d.join("big.md"),
            "# Глава 1\n\nНокс шла долго. И текст был длинным, гораздо длиннее\n\
             крошечного бюджета, который задаст тест: предложение за\n\
             предложением, строка за строкой, чтобы скоуп гарантированно\n\
             не влез в бюджет и движок честно об этом сказал.\n",
        )
        .unwrap();
        // Карта: пометка о превышении.
        let card = run_feed(&d, FeedMode::Edit, "Нокс", 64, 40, false);
        assert!(
            card.contains("превышает бюджет"),
            "пометка о превышении: {card}"
        );
        // JSON: булев флаг.
        let js = run_feed(&d, FeedMode::Edit, "Нокс", 64, 40, true);
        let v: serde_json::Value = serde_json::from_str(&js).expect("JSON");
        assert_eq!(v["over_budget"], true, "флаг over_budget: {js}");
        assert!(
            v["spent_tokens"].as_u64().unwrap() > v["budget_tokens"].as_u64().unwrap(),
            "потрачено больше бюджета: {js}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cmd_feed_requires_query_and_path() {
        let mut s = ShellState::new(std::path::PathBuf::from("/tmp/x.db"));
        // без аргументов → справка
        match super::super::commands::dispatch(&mut s, "feed") {
            CmdResult::Done(out) => assert!(out.contains("--for"), "справка: {out}"),
            _ => panic!("expected Done"),
        }
        // путь есть, запроса нет → подсказка
        match super::super::commands::dispatch(&mut s, "feed /tmp") {
            CmdResult::Done(out) => assert!(out.contains("--query"), "подсказка query: {out}"),
            _ => panic!("expected Done"),
        }
        // несуществующий путь → ошибка
        match super::super::commands::dispatch(
            &mut s,
            "feed /nonexistent-dir-xyz --for edit -q foo",
        ) {
            CmdResult::Done(out) => assert!(out.contains("не найден"), "ошибка пути: {out}"),
            _ => panic!("expected Done"),
        }
    }

    #[test]
    fn percentile_and_verdict_boundaries() {
        let sorted = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&sorted, 0.5), 2.5);
        assert_eq!(percentile(&sorted, 0.0), 1.0);
        assert_eq!(percentile(&sorted, 1.0), 4.0);
        assert_eq!(energy_verdict(0.75), "плотная зона");
        assert_eq!(energy_verdict(0.25), "разреженная зона");
        assert_eq!(energy_verdict(0.5), "середина");
    }

    #[test]
    fn definition_note_is_honest_about_count() {
        assert!(definition_note(0).contains("нет"), "extern: честный ноль");
        assert!(
            definition_note(1).contains("единственная"),
            "одна точка: {:?}",
            definition_note(1)
        );
        assert!(
            definition_note(5).contains("несколько"),
            "много точек: {:?}",
            definition_note(5)
        );
    }

    #[test]
    fn feed_mega_function_compressed_to_signature_and_focus() {
        // Болезнь №2 стресс-теста libgit2: функция на тысячи строк НЕ подаётся
        // целиком сквозь бюджет — пакет получает сигнатуру + фокальное окно
        // вокруг хита, разрывы помечены честными маркерами.
        let d = temp_dir("mega");
        let mut src = String::new();
        src.push_str(
            "int git_repository_open(git_repository **out, const char *path, int flags)\n{\n",
        );
        for i in 0..120 {
            src.push_str(&format!(
                "    int filler_{i} = compute_step_{i}(flags) + {i}; /* шум */\n"
            ));
        }
        src.push_str("    return validate_and_open(out, path, flags);\n");
        src.push_str("}\n");
        std::fs::write(d.join("repo.c"), src).unwrap();
        // Полный скоуп ≈ 124 строки не влезает в 500 ток; сжатый вид — влезает.
        let js = run_feed(&d, FeedMode::Edit, "validate_and_open", 500, 40, true);
        let v: serde_json::Value = serde_json::from_str(&js).expect("JSON");
        assert_eq!(v["over_budget"], false, "сжатый вид влез в бюджет: {js}");
        assert!(
            v["spent_tokens"].as_u64().unwrap() <= 500,
            "бюджет уважен: {js}"
        );
        let it = &v["items"][0];
        assert_eq!(
            it["compressed"]["applied"],
            true,
            "сжатие применено: {js}"
        );
        let scope = it["scope"].as_str().unwrap();
        assert!(
            scope.contains("int git_repository_open"),
            "сигнатура в скоупе: {scope}"
        );
        assert!(
            scope.contains("validate_and_open"),
            "фокус-хит внутри окна: {scope}"
        );
        assert!(scope.contains("укрыто"), "маркеры пропуска: {scope}");
        assert!(
            scope.trim_end().ends_with('}'),
            "закрывающая скобка видна: {scope}"
        );
        assert!(
            it["compressed"]["skipped_lines"].as_u64().unwrap() > 50,
            "мега-объём честно укрыт: {js}"
        );
        // Карта: пометка о сжатии вместо паники «превышает бюджет».
        let card = run_feed(&d, FeedMode::Edit, "validate_and_open", 500, 40, false);
        assert!(card.contains("фокальных окон"), "пометка в карте: {card}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_mega_scope_rehydrated_beyond_16k_cut() {
        // Смоук-кейс v0.81.0: мега-функция > 16 КБ, хит — ЗА границей
        // обрезки сцены. Без перечитывания фокальные окна строились по
        // префиксу и промахивались мимо цели правки (fallback, 0 окон).
        let d = temp_dir("rehydrate");
        let mut src = String::new();
        // ANSI-сигнатура: скобка в конце последней строки параметров
        src.push_str(
            "int git_repository_init_ext(git_repository **repo_out, const char *path,\n        unsigned flags, const git_repository_init_options *opts) {\n",
        );
        // ~700 строк × ~55 байт ≈ 38 КБ > max_scope_bytes (16 КБ)
        for i in 0..700 {
            src.push_str(&format!(
                "    int stage_buffer_{i} = transform_stage_{i}(path, flags);\n"
            ));
        }
        // хит — в самом хвосте, ЗА границей 16-КБ обрезки
        src.push_str("    git_object *handle = git_object_acquire(repo_out, opts);\n");
        src.push_str("    return finish_init(handle, flags);\n");
        src.push_str("}\n");
        std::fs::write(d.join("repository_ext.c"), src).unwrap();
        let js = run_feed(&d, FeedMode::Edit, "git_object_acquire", 500, 40, true);
        let v: serde_json::Value = serde_json::from_str(&js).expect("JSON");
        assert_eq!(v["over_budget"], false, "сжатый вид влез: {js}");
        assert!(
            v["spent_tokens"].as_u64().unwrap() <= 500,
            "бюджет уважен: {js}"
        );
        let it = &v["items"][0];
        assert_eq!(it["rehydrated"], true, "скоуп перечитан из файла: {js}");
        assert_eq!(it["compressed"]["applied"], true, "сжатие применено: {js}");
        assert_eq!(
            it["compressed"]["fallback"], false,
            "хиты найдены в полном тексте: {js}"
        );
        assert!(
            it["compressed"]["windows"].as_u64().unwrap() >= 1,
            "фокальное окно построено: {js}"
        );
        let scope = it["scope"].as_str().unwrap();
        assert!(
            scope.contains("int git_repository_init_ext("),
            "ANSI-сигнатура поднята: {scope}"
        );
        assert!(
            scope.contains("git_object_acquire"),
            "хит за границей обрезки — в окне: {scope}"
        );
        assert!(scope.contains("укрыто"), "маркеры пропуска: {scope}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_mega_chapter_compressed_for_understand() {
        // Проза: глава длиннее бюджета подаётся заголовком + фокальным окном
        // вокруг смыслообразующего упоминания (Darcy-кейс).
        let d = temp_dir("mega-prose");
        let mut txt = String::from("# Глава 1\n\n");
        for i in 0..80 {
            txt.push_str(&format!(
                "Абзац номер {i} повествует о погоде и окрестностях Лонгборна.\n\n"
            ));
        }
        txt.push_str("Дарси принёс чашку кофе и молча смотрел в окно.\n\n");
        for i in 0..40 {
            txt.push_str(&format!(
                "Дальнейший абзац {i} — о миссис Беннет и её планах на неделю.\n\n"
            ));
        }
        std::fs::write(d.join("novel.md"), txt).unwrap();
        let js = run_feed(&d, FeedMode::Understand, "Дарси", 400, 40, true);
        let v: serde_json::Value = serde_json::from_str(&js).expect("JSON");
        assert_eq!(v["over_budget"], false, "сжатая глава влезла: {js}");
        assert!(
            v["spent_tokens"].as_u64().unwrap() <= 400,
            "бюджет уважен: {js}"
        );
        let it = &v["items"][0];
        assert_eq!(
            it["compressed"]["applied"],
            true,
            "сжатие применено: {js}"
        );
        let scope = it["scope"].as_str().unwrap();
        assert!(
            scope.contains("Дарси принёс"),
            "фокус-окно вокруг хита: {scope}"
        );
        assert!(scope.contains("укрыто"), "маркеры пропуска: {scope}");
        let card = run_feed(&d, FeedMode::Understand, "Дарси", 400, 40, false);
        assert!(card.contains("фокальных окон"), "пометка в карте: {card}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn feed_impact_lists_trimmed_to_budget() {
        // Паспорт с 60 потребителями не съедает пакет (остаток болезни №2
        // стресс-теста: 200 upstream × длинные пути = 6710 ток при бюджете
        // 3000): счётчики полные, списки обрезаны под кап бюджета.
        let d = temp_dir("impact-trim");
        let mut src = String::from("int target_fn(void)\n{\n    return 1;\n}\n\n");
        for i in 0..60 {
            src.push_str(&format!(
                "int caller_{i:02}(void)\n{{\n    return target_fn() + {i};\n}}\n\n"
            ));
        }
        std::fs::write(d.join("big.c"), src).unwrap();
        let js = run_feed(&d, FeedMode::Edit, "target_fn", 400, 5, true);
        let v: serde_json::Value = serde_json::from_str(&js).expect("JSON");
        let im = &v["impact"];
        let total = im["upstream_total"].as_u64().unwrap();
        assert!(total >= 60, "полный счётчик честен: {js}");
        let shown = im["upstream_shown"].as_u64().unwrap();
        assert!(
            (1..total).contains(&shown),
            "список обрезан, но не пуст: {js}"
        );
        assert_eq!(
            im["upstream_dependents"].as_array().unwrap().len() as u64,
            shown,
            "длина списка = shown: {js}"
        );
        assert_eq!(im["truncated"], true, "пометка обрезки: {js}");
        assert!(
            v["spent_tokens"].as_u64().unwrap() <= 400,
            "бюджет уважен: {js}"
        );
        assert_eq!(v["over_budget"], false, "нет переполнения: {js}");
        let _ = std::fs::remove_dir_all(&d);
    }
}

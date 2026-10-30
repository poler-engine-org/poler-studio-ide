//! v0.92.0 STREAM-CORPUS: манифест-пакер — список юнитов (URL и локальные
//! пути) → ОДИН `.poler` нативным конвейером движка.
//!
//! Директива владельца: «150 ГБ текстов (код + литература) прямо в один
//! .poler, ноль сырья на диске — все механизмы уже в движке». Модуль —
//! оркестр над [`StreamWriter`]:
//!
//! * **Слот-машина**: N фетч-воркеров (каждый держит ≤1 юнит, RAM
//!   O(fetch_jobs × unit_buf_max)), писатель потребляет юниты СТРОГО в
//!   порядке манифеста — байты архива не зависят от `--stream-jobs`;
//! * **Атомарные юниты** ≤ `unit_buf_max`: полностью буферизуются,
//!   контейнеры (tar.gz/tar.zst/gz/zst/zip) раскрываются в воркере
//!   (параллельно!), tar-фрагмент строится `tar::Builder`'ом — длинные
//!   имена автоматически GNU 'L'-записями (грабля №49). Сбой фетча =
//!   чистый скип: ни одного байта в архиве;
//! * **Большие raw-юниты** > `unit_buf_max`: заголовок с декларированным
//!   размером (Content-Length/metadata), тело стримится; обрыв =
//!   zero-pad до размера — запись структурно цела, сбой виден в
//!   статистике и верификации;
//! * **Резюм**: `<out>.units.done` — ключи завершённых юнитов; повторный
//!   запуск того же манифеста докатывает только недостающее;
//! * **Чекпоинты** (каждые `checkpoint_units` юнитов / `checkpoint_secs`
//!   секунд): архив финализируется (валидный .poler + state-sidecar),
//!   `.units.done` дополняется, писатель переоткрывается
//!   [`StreamWriter::open_append`]. Падение в любой точке откатывается к
//!   последнему чекпоинту ремонтом по sidecar — «CLEANUP сносит
//!   скачанное» архитектурно невозможно;
//! * **Диск**: пик = размер архива (доклейка НА МЕСТЕ, не 2× архива).

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
trait WinFileExt {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()>;
}
#[cfg(windows)]
impl WinFileExt for std::fs::File {
    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !buf.is_empty() {
            match self.seek_read(buf, offset) {
                Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "failed to fill whole buffer")),
                Ok(n) => { buf = &mut buf[n..]; offset += n as u64; }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::ingest::{sniff_magic, IngestKind};
use super::reader::PolerReader;
use super::stream_writer::{
    peak_rss_kb, StreamWriter, StreamWriteConfig, StreamWriteStats, MAGIC_END, TRAILER_SIZE,
};
use crate::pqc::sha256::hex;

/// Настройки манифест-пакера.
#[derive(Debug, Clone)]
pub struct ManifestOptions {
    /// Параллельные фетч-воркеры слот-машины (каждый держит ≤1 юнит).
    pub fetch_jobs: usize,
    /// Юниты ≤ этого размера буферизуются целиком (атомарный tar-фрагмент).
    pub unit_buf_max: usize,
    /// Чекпоинт каждые N завершённых юнитов.
    pub checkpoint_units: u64,
    /// Чекпоинт не реже, чем раз в T секунд.
    pub checkpoint_secs: u64,
    /// Попыток на сетевой юнит (бэкофф 300 мс × 2^попытка).
    pub retries: u32,
    /// Жёсткий кап ответов без Content-Length (chunked).
    pub chunked_cap: usize,
    /// Прогресс в stderr.
    pub progress: bool,
}

impl Default for ManifestOptions {
    fn default() -> Self {
        ManifestOptions {
            fetch_jobs: 4,
            unit_buf_max: 8 * 1024 * 1024,
            checkpoint_units: 2000,
            checkpoint_secs: 210,
            retries: 3,
            chunked_cap: 64 * 1024 * 1024,
            progress: true,
        }
    }
}

/// Итог манифест-прогона (serde: JSON в stdout CLI).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManifestStats {
    pub output: String,
    pub units_total: u64,
    pub units_done_prior: u64,
    pub units_packed: u64,
    pub units_skipped_done: u64,
    pub units_failed: u64,
    pub files: u64,
    pub total_raw: u64,
    pub total_stored: u64,
    pub ratio: f64,
    pub logical_chunks: u64,
    pub dedup_chunks: u64,
    pub dedup_bytes: u64,
    pub checkpoints: u64,
    pub resumed_archive: bool,
    pub stream_sha256_hex: String,
    pub elapsed_ms: u64,
    pub throughput_mbs: f64,
    pub peak_rss_kb: u64,
}

/// Источник юнита.
#[derive(Debug, Clone)]
enum UnitSrc {
    Local(PathBuf),
    Remote(String),
}

/// Судьба юнита после прогона через писателя.
enum PushOutcome {
    /// Полностью записан (в .units.done).
    Complete,
    /// Тело оборвалось: zero-pad до декларированного размера — запись
    /// структурно цела, содержимое битое. Помечается сделанным (повтор
    /// дублировал бы запись) и проваленным в статистике.
    BrokenWritten,
}

/// Упаковать манифест юнитов в один .poler (см. шапку модуля).
pub fn manifest_pack(
    manifest: &Path,
    out: &Path,
    cfg: StreamWriteConfig,
    opts: ManifestOptions,
) -> io::Result<ManifestStats> {
    let t0 = Instant::now();
    let all_units = parse_manifest(manifest)?;
    let units_total = all_units.len() as u64;
    let done = load_units_done(out);
    let units_done_prior = done.len() as u64;
    let pending: Vec<(String, UnitSrc)> = all_units
        .into_iter()
        .filter(|(k, _)| !done.contains(k))
        .collect();
    let units_skipped_done = units_total - pending.len() as u64;
    let resumed_archive = out.exists();

    // Ленивая активация сегмента: докатывать нечего — архив не трогаем ВООБЩЕ
    // (валидный файл до первых байтов новой доклейки).
    if pending.is_empty() {
        if !resumed_archive {
            let w = StreamWriter::open(out, cfg)?;
            let s = w.finish("manifest")?;
            return Ok(ManifestStats {
                output: s.output,
                units_total,
                units_done_prior,
                units_packed: 0,
                units_skipped_done,
                units_failed: 0,
                files: s.files,
                total_raw: s.total_raw,
                total_stored: s.total_stored,
                ratio: s.ratio,
                logical_chunks: s.logical_chunks,
                dedup_chunks: s.dedup_chunks,
                dedup_bytes: s.dedup_bytes,
                checkpoints: 0,
                resumed_archive: false,
                stream_sha256_hex: s.stream_sha256_hex,
                elapsed_ms: s.elapsed_ms,
                throughput_mbs: s.throughput_mbs,
                peak_rss_kb: s.peak_rss_kb,
            });
        }
        return lazy_manifest_stats(out, units_total, units_done_prior, units_skipped_done);
    }

    // ───────────────── слот-машина: N воркеров, порядковое потребление ─────────────────
    let n_workers = opts.fetch_jobs.clamp(1, pending.len()).min(16);
    let pending = Arc::new(pending);
    let (req_tx, req_rx) = mpsc::channel::<usize>();
    let req_rx = Arc::new(Mutex::new(req_rx));
    let (rep_tx, rep_rx) = mpsc::channel::<(usize, io::Result<FetchedUnit>)>();

    let mut handles = Vec::new();
    for _ in 0..n_workers {
        let req_rx = req_rx.clone();
        let rep_tx = rep_tx.clone();
        let units = pending.clone();
        let opts = opts.clone();
        handles.push(thread::spawn(move || {
            loop {
                let i = {
                    let guard = req_rx.lock().unwrap_or_else(|e| e.into_inner());
                    match guard.recv() {
                        Ok(i) => i,
                        Err(_) => return, // писатель ушёл — стоп
                    }
                };
                // лок освобождён ДО фетча (феч может длиться секунды)
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    fetch_unit(&units[i].0, &units[i].1, &opts)
                }))
                .unwrap_or_else(|_| Err(io::Error::other("фетч-воркер: паника")));
                if rep_tx.send((i, res)).is_err() {
                    return;
                }
            }
        }));
    }
    drop(rep_tx); // свой клон: закрытие канала ответов детектирует смерть воркеров

    // разогрев: раздать первые N индексов
    let mut assigned = 0usize;
    for _ in 0..n_workers {
        if assigned < pending.len() {
            let _ = req_tx.send(assigned);
            assigned += 1;
        }
    }

    let mut writer = if resumed_archive {
        StreamWriter::open_append(out, cfg.clone())?
    } else {
        StreamWriter::open(out, cfg.clone())?
    };

    let hint = manifest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut units_packed = 0u64;
    let mut units_failed = 0u64;
    let mut newly_done: Vec<String> = Vec::new();
    let mut checkpoints = 0u64;
    let mut since_ckpt = 0u64;
    let mut ckpt_t = Instant::now();
    let mut stashed: std::collections::HashMap<usize, io::Result<FetchedUnit>> =
        std::collections::HashMap::new();

    for i in 0..pending.len() {
        let fetched = match stashed.remove(&i) {
            Some(f) => f,
            None => loop {
                match rep_rx.recv() {
                    Ok((j, f)) => {
                        if j == i {
                            break f;
                        }
                        stashed.insert(j, f);
                    }
                    Err(_) => {
                        break Err(io::Error::other(
                            "фетч-воркеры завершились преждевременно",
                        ))
                    }
                }
            },
        };
        // слот освободился — сразу раздать следующий юнит
        if assigned < pending.len() {
            let _ = req_tx.send(assigned);
            assigned += 1;
        }
        match fetched {
            Ok(unit) => match push_unit(&mut writer, &pending[i].0, unit) {
                Ok(PushOutcome::Complete) => {
                    units_packed += 1;
                    newly_done.push(pending[i].0.clone());
                }
                Ok(PushOutcome::BrokenWritten) => {
                    units_failed += 1;
                    // битое, но записанное: не дублируем при резюме
                    newly_done.push(pending[i].0.clone());
                    eprintln!("stream-manifest: ЮНИТ БИТ (zero-pad): {}", pending[i].0);
                }
                Err(e) => {
                    // дисковая ошибка конвейера — фатально для прогона;
                    // архив остаётся на последнем чекпоинте, резюм честен
                    return Err(io::Error::other(format!(
                        "юнит {}: {e}",
                        pending[i].0
                    )));
                }
            },
            Err(e) => {
                units_failed += 1;
                eprintln!("stream-manifest: провал юнита {}: {e}", pending[i].0);
            }
        }
        since_ckpt += 1;
        if opts.progress && (i + 1) % 500 == 0 {
            let dt = t0.elapsed().as_secs_f64().max(0.001);
            eprintln!(
                "stream-manifest: {}/{} юнитов, {:.1} юнит/с, провалов {}",
                i + 1,
                pending.len(),
                (i + 1) as f64 / dt,
                units_failed
            );
        }
        if since_ckpt >= opts.checkpoint_units
            || ckpt_t.elapsed().as_secs() >= opts.checkpoint_secs
        {
            // чекпоинт: архив валиден + sidecar, .units.done синхронизирован
            // с ним, писатель переоткрывается доклейкой НА МЕСТЕ
            writer.finish(&hint)?;
            append_units_done(out, &newly_done)?;
            newly_done.clear();
            writer = StreamWriter::open_append(out, cfg.clone())?;
            checkpoints += 1;
            since_ckpt = 0;
            ckpt_t = Instant::now();
        }
    }

    let final_stats: StreamWriteStats = writer.finish(&hint)?;
    append_units_done(out, &newly_done)?;

    // воркеры закончили (все индексы потреблены) — закрываем канал заданий
    drop(req_tx);
    for h in handles {
        let _ = h.join();
    }

    let elapsed = t0.elapsed().as_millis() as u64;
    Ok(ManifestStats {
        output: final_stats.output,
        units_total,
        units_done_prior,
        units_packed,
        units_skipped_done,
        units_failed,
        files: final_stats.files,
        total_raw: final_stats.total_raw,
        total_stored: final_stats.total_stored,
        ratio: final_stats.ratio,
        logical_chunks: final_stats.logical_chunks,
        dedup_chunks: final_stats.dedup_chunks,
        dedup_bytes: final_stats.dedup_bytes,
        checkpoints,
        resumed_archive,
        stream_sha256_hex: final_stats.stream_sha256_hex,
        elapsed_ms: elapsed,
        throughput_mbs: if elapsed > 0 {
            final_stats.total_raw as f64 / 1024.0 / 1024.0 / (elapsed as f64 / 1000.0)
        } else {
            0.0
        },
        peak_rss_kb: peak_rss_kb(),
    })
}

// ────────────────────────── манифест и sidecar-ключи ──────────────────────────

/// Разбор манифеста: строка = юнит (URL http(s) или локальный путь);
/// пустые строки и '#' — комментарии; дубликаты ключей пропускаются.
fn parse_manifest(manifest: &Path) -> io::Result<Vec<(String, UnitSrc)>> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| io::Error::other(format!("манифест {}: {e}", manifest.display())))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !seen.insert(line.to_string()) {
            continue;
        }
        let src = if line.starts_with("http://") || line.starts_with("https://") {
            UnitSrc::Remote(line.to_string())
        } else {
            UnitSrc::Local(PathBuf::from(line))
        };
        out.push((line.to_string(), src));
    }
    Ok(out)
}

fn units_done_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_os_string();
    s.push(".units.done");
    PathBuf::from(s)
}

/// Ключи завершённых юнитов; обрыв последней строки (падение в момент
/// записи) терпим — недописанный ключ просто перезагрузится.
fn load_units_done(out: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(text) = std::fs::read_to_string(units_done_path(out)) {
        for line in text.lines() {
            let l = line.trim();
            if !l.is_empty() {
                set.insert(l.to_string());
            }
        }
    }
    set
}

fn append_units_done(out: &Path, keys: &[String]) -> io::Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(units_done_path(out))?;
    for k in keys {
        writeln!(f, "{k}")?;
    }
    f.sync_all()?;
    Ok(())
}

/// Имя записи в таблице файлов: для URL — базовое имя пути; для локального
/// юнита — путь как написан в манифесте (обычно относительный).
/// Абсолютные пути УКОРЕНЯЮТСЯ (tar-крейт отвергает записи с ведущим '/'
/// — «paths in archives must be relative»), обратные слэши нормализуются.
fn unit_entry_name(key: &str) -> String {
    let name = if key.starts_with("http://") || key.starts_with("https://") {
        let no_frag = key.split('#').next().unwrap_or(key);
        let path = no_frag.split('?').next().unwrap_or(no_frag);
        path.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("unit")
            .to_string()
    } else {
        key.replace('\\', "/").trim_start_matches('/').to_string()
    };
    if name.is_empty() {
        "unit".to_string()
    } else {
        name
    }
}

// ────────────────────────── фетч юнитов ──────────────────────────

/// Результат фетча юнита.
enum FetchedUnit {
    /// Атомарный юнит: содержимое полностью в памяти, контейнеры уже
    /// декодированы в воркере.
    Atomic(AtomicPayload),
    /// Большой raw-юнит: тело записи стримится, размер декларирован
    /// заранее (Content-Length / metadata).
    StreamBody {
        declared_len: u64,
        body: Box<dyn Read + Send>,
    },
}

/// Декодированное содержимое атомарного юнита.
enum AtomicPayload {
    /// Готовый tar-поток: льётся как есть — TarObserver разводит пофайловую
    /// таблицу, завершающие нулевые блоки проглатываются (грабля №47).
    TarStream(Vec<u8>),
    /// Одиночный файл → один tar-entry (длинные имена — GNU 'L', грабля №49).
    Single { name: String, data: Vec<u8> },
    /// zip-архив: записи (требует seek — но байты уже в памяти).
    ZipEntries(Vec<(String, Vec<u8>)>),
}

fn fetch_unit(key: &str, src: &UnitSrc, opts: &ManifestOptions) -> io::Result<FetchedUnit> {
    match src {
        UnitSrc::Local(path) => fetch_local(path, opts),
        UnitSrc::Remote(url) => {
            let mut attempt = 0u32;
            loop {
                match fetch_remote(url, key, opts) {
                    Ok(u) => return Ok(u),
                    Err(e) => {
                        if attempt >= opts.retries {
                            return Err(e);
                        }
                        let backoff = 300u64 << attempt.min(6);
                        thread::sleep(Duration::from_millis(backoff));
                        attempt += 1;
                    }
                }
            }
        }
    }
}

fn fetch_local(path: &Path, opts: &ManifestOptions) -> io::Result<FetchedUnit> {
    let meta = std::fs::metadata(path)
        .map_err(|e| io::Error::other(format!("{}: {e}", path.display())))?;
    if !meta.is_file() {
        return Err(io::Error::other("не файл (каталоги манифестом не пакуются)"));
    }
    if meta.len() > opts.unit_buf_max as u64 {
        let body = File::open(path)
            .map_err(|e| io::Error::other(format!("{}: {e}", path.display())))?;
        return Ok(FetchedUnit::StreamBody { declared_len: meta.len(), body: Box::new(body) });
    }
    let mut raw = Vec::with_capacity(meta.len() as usize);
    File::open(path)?.read_to_end(&mut raw)?;
    let key = path.to_string_lossy().to_string();
    Ok(FetchedUnit::Atomic(decode_atomic(&key, raw)?))
}

fn fetch_remote(url: &str, key: &str, opts: &ManifestOptions) -> io::Result<FetchedUnit> {
    let agent = ureq::AgentBuilder::new()
        .user_agent("poler-engine/0.92 (stream-manifest)")
        .build();
    let resp = agent.get(url).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => io::Error::other(format!("HTTP {code}")),
        other => io::Error::other(format!("сеть: {other}")),
    })?;
    let declared = resp
        .header("Content-Length")
        .and_then(|v| v.trim().parse::<u64>().ok());
    match declared {
        Some(len) if len > opts.unit_buf_max as u64 => Ok(FetchedUnit::StreamBody {
            declared_len: len,
            body: Box::new(resp.into_reader()),
        }),
        Some(len) => {
            // атомарно + защита от «сервер прислал больше/меньше»
            let mut raw = Vec::with_capacity(len as usize);
            let mut r = resp.into_reader().take(len + 1);
            r.read_to_end(&mut raw)?;
            if raw.len() as u64 != len {
                return Err(io::Error::other(format!(
                    "тело {} байт ≠ Content-Length {len}",
                    raw.len()
                )));
            }
            Ok(FetchedUnit::Atomic(decode_atomic(key, raw)?))
        }
        None => {
            // chunked: жёсткий кап (без него RAM не ограничен)
            let mut raw = Vec::new();
            let mut r = resp.into_reader().take(opts.chunked_cap as u64 + 1);
            r.read_to_end(&mut raw)?;
            if raw.len() > opts.chunked_cap {
                return Err(io::Error::other(
                    "chunked-ответ превышает кап юнита (увеличь --stream-unit-max)",
                ));
            }
            Ok(FetchedUnit::Atomic(decode_atomic(key, raw)?))
        }
    }
}

/// Декодирование атомарного юнита: сниффинг магии, gzip/zstd НА ЛЕТУ
/// (в памяти), tar распознаётся после декодирования, zip — пофайлово.
fn decode_atomic(key: &str, raw: Vec<u8>) -> io::Result<AtomicPayload> {
    let name = unit_entry_name(key);
    match sniff_magic(&raw) {
        IngestKind::TarRaw => Ok(AtomicPayload::TarStream(raw)),
        IngestKind::Zip => {
            let mut zip = zip::ZipArchive::new(Cursor::new(raw))
                .map_err(|e| io::Error::other(format!("zip: {e}")))?;
            let mut entries = Vec::new();
            for i in 0..zip.len() {
                let mut f = zip
                    .by_index(i)
                    .map_err(|e| io::Error::other(format!("zip entry: {e}")))?;
                if f.is_dir() {
                    continue;
                }
                let mut data = Vec::with_capacity(f.size() as usize);
                f.read_to_end(&mut data)?;
                entries.push((f.name().to_string(), data));
            }
            Ok(AtomicPayload::ZipEntries(entries))
        }
        IngestKind::TarGz | IngestKind::GzSingle => {
            let mut decoded = Vec::new();
            flate2::read::MultiGzDecoder::new(Cursor::new(raw)).read_to_end(&mut decoded)?;
            classify_decoded(name, decoded)
        }
        IngestKind::TarZst | IngestKind::ZstSingle => {
            let mut decoded = Vec::new();
            zstd::stream::read::Decoder::new(Cursor::new(raw))?.read_to_end(&mut decoded)?;
            classify_decoded(name, decoded)
        }
        IngestKind::RawStream => Ok(AtomicPayload::Single { name, data: raw }),
    }
}

fn classify_decoded(name: String, decoded: Vec<u8>) -> io::Result<AtomicPayload> {
    if sniff_magic(&decoded) == IngestKind::TarRaw {
        Ok(AtomicPayload::TarStream(decoded))
    } else {
        Ok(AtomicPayload::Single { name, data: decoded })
    }
}

// ────────────────────────── прогон юнита в писателя ──────────────────────────

/// Мост tar::Builder → StreamWriter (тот же паттерн, что у пакеров ingest.rs).
struct PushShim<'a> {
    writer: &'a mut StreamWriter,
}

impl io::Write for PushShim<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.push_bytes(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Тело большой записи с zero-pad: читается ровно declared_len байт;
/// обрыв ИЛИ ошибка чтения добиваются нулями — tar-запись остаётся
/// структурно целой (размер в заголовке соблюдён), сбой фиксируется
/// флагом и попадает в статистику/верификацию.
struct PaddedBody<R: Read> {
    inner: R,
    remaining: u64,
    broken: Arc<Mutex<Option<String>>>,
}

impl<R: Read> Read for PaddedBody<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        fn fill_zero(remaining: &mut u64, buf: &mut [u8]) -> usize {
            let n = (*remaining as usize).min(buf.len());
            buf[..n].fill(0);
            *remaining -= n as u64;
            n
        }
        if self.remaining == 0 {
            return Ok(0);
        }
        if self.broken.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            return Ok(fill_zero(&mut self.remaining, buf));
        }
        match self.inner.read(buf) {
            Ok(0) => {
                *self.broken.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some("обрыв потока".to_string());
                Ok(fill_zero(&mut self.remaining, buf))
            }
            Ok(n) => {
                let n = (n as u64).min(self.remaining) as usize;
                self.remaining -= n as u64;
                Ok(n)
            }
            Err(e) => {
                *self.broken.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("чтение: {e}"));
                Ok(fill_zero(&mut self.remaining, buf))
            }
        }
    }
}

fn tar_header_for(size: u64) -> tar::Header {
    let mut h = tar::Header::new_gnu();
    h.set_size(size);
    h.set_mode(0o644);
    h.set_cksum();
    h
}

/// Прогнать юнит в писателя. Порядок байтов = порядок манифеста;
/// tar-фрагменты строятся tar::Builder'ом (GNU 'L' для длинных имён),
/// внутренние tar-потоки льются как есть (хвосты глотаются — №47).
fn push_unit(writer: &mut StreamWriter, key: &str, fetched: FetchedUnit) -> io::Result<PushOutcome> {
    match fetched {
        FetchedUnit::Atomic(AtomicPayload::TarStream(bytes)) => {
            writer.push_bytes(&bytes)?;
            Ok(PushOutcome::Complete)
        }
        FetchedUnit::Atomic(AtomicPayload::Single { name, data }) => {
            let mut b = tar::Builder::new(PushShim { writer });
            b.append_data(&mut tar_header_for(data.len() as u64), name.as_str(), Cursor::new(&data[..]))?;
            b.finish()?;
            Ok(PushOutcome::Complete)
        }
        FetchedUnit::Atomic(AtomicPayload::ZipEntries(entries)) => {
            let mut b = tar::Builder::new(PushShim { writer });
            for (name, data) in entries {
                b.append_data(
                    &mut tar_header_for(data.len() as u64),
                    name.as_str(),
                    Cursor::new(&data[..]),
                )?;
            }
            b.finish()?;
            Ok(PushOutcome::Complete)
        }
        FetchedUnit::StreamBody { declared_len, body } => {
            let name = unit_entry_name(key);
            let broken = Arc::new(Mutex::new(None::<String>));
            {
                let mut b = tar::Builder::new(PushShim { writer });
                b.append_data(
                    &mut tar_header_for(declared_len),
                    name.as_str(),
                    PaddedBody {
                        inner: body,
                        remaining: declared_len,
                        broken: broken.clone(),
                    },
                )?;
                b.finish()?;
            }
            let failed = broken.lock().unwrap_or_else(|e| e.into_inner()).is_some();
            Ok(if failed { PushOutcome::BrokenWritten } else { PushOutcome::Complete })
        }
    }
}

// ────────────────────────── ленивая статистика ──────────────────────────

/// Статистика без-touch прогона: всё уже сделано, архив не открывался
/// на запись. Метаданные — читателем (без декомпрессии), дайджест —
/// прямо из трейлера.
fn lazy_manifest_stats(
    out: &Path,
    units_total: u64,
    units_done_prior: u64,
    units_skipped_done: u64,
) -> io::Result<ManifestStats> {
    let reader = PolerReader::open(out)
        .map_err(|e| io::Error::other(format!("ленивый прогон: {e}")))?;
    let info = reader.info().clone();
    let mut digest = [0u8; 32];
    {
        let len = std::fs::metadata(out)?.len();
        if len < TRAILER_SIZE as u64 {
            return Err(io::Error::other("ленивый прогон: трейлер не найден"));
        }
        let mut f = File::open(out)?;
        let mut t = vec![0u8; TRAILER_SIZE];
        f.read_exact_at(&mut t, len - TRAILER_SIZE as u64)?;
        if &t[0..8] != MAGIC_END {
            return Err(io::Error::other("ленивый прогон: трейлер не POLEREND"));
        }
        digest.copy_from_slice(&t[68..100]);
    }
    Ok(ManifestStats {
        output: out.display().to_string(),
        units_total,
        units_done_prior,
        units_packed: 0,
        units_skipped_done,
        units_failed: 0,
        files: info.files,
        total_raw: info.total_raw,
        total_stored: info.total_stored,
        ratio: info.ratio,
        logical_chunks: info.logical_chunks,
        dedup_chunks: info.logical_chunks.saturating_sub(info.physical_chunks),
        dedup_bytes: 0,
        checkpoints: 0,
        resumed_archive: true,
        stream_sha256_hex: hex(&digest),
        elapsed_ms: 0,
        throughput_mbs: 0.0,
        peak_rss_kb: peak_rss_kb(),
    })
}

// ────────────────────────── тесты (v0.92.0) ──────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::reader::PolerReader;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("poler-mani-{}", std::process::id()))
            .join(name);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn cfg() -> StreamWriteConfig {
        StreamWriteConfig { progress_bytes: 0, ..Default::default() }
    }

    fn opts(fetch_jobs: usize) -> ManifestOptions {
        ManifestOptions {
            fetch_jobs,
            checkpoint_units: u64::MAX,
            checkpoint_secs: u64::MAX,
            progress: false,
            ..Default::default()
        }
    }

    fn opts_ckpt(fetch_jobs: usize, checkpoint_units: u64) -> ManifestOptions {
        ManifestOptions {
            fetch_jobs,
            checkpoint_units,
            checkpoint_secs: u64::MAX,
            progress: false,
            ..Default::default()
        }
    }

    fn write_file(path: &Path, data: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, data).unwrap();
    }

    fn entropy(len: usize, seed: u64) -> Vec<u8> {
        let mut st = seed | 1;
        let mut v = Vec::with_capacity(len);
        while v.len() < len {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.extend_from_slice(&st.to_le_bytes());
        }
        v.truncate(len);
        v
    }

    fn manifest_of(lines: &[String]) -> String {
        lines.join("\n") + "\n"
    }

    /// Базовый прогон: локальные юниты (в т.ч. имя >100 символов — грабля
    /// №49: GNU 'L'), верификация сквозного дайджеста и per-file sha,
    /// round-trip содержимого.
    #[test]
    fn manifest_pack_local_files_with_long_name() {
        let d = tmp("basic");
        let f1 = d.join("corpus/a.txt");
        write_file(&f1, b"alpha line\nbeta line\n");
        let long_rel = format!("corpus/{}.txt", "L".repeat(140));
        let f2 = d.join(&long_rel);
        write_file(&f2, &entropy(200_000, 7));
        let f3 = d.join("corpus/sub/c.bin");
        write_file(&f3, &entropy(150_000, 9));
        let manifest = d.join("m.txt");
        let lines = [f1.display().to_string(), f2.display().to_string(), f3.display().to_string()];
        std::fs::write(&manifest, manifest_of(&lines)).unwrap();
        let out = d.join("basic.poler");
        let stats = manifest_pack(&manifest, &out, cfg(), opts(2)).unwrap();
        assert_eq!(stats.units_packed, 3);
        assert_eq!(stats.units_failed, 0);
        let reader = PolerReader::open(&out).unwrap();
        let files = reader.files();
        assert_eq!(files.len(), 3, "{:?}: files", files.iter().map(|f| f.name.clone()).collect::<Vec<_>>());
        let rep = reader.verify().unwrap();
        assert!(rep.stream_ok, "сквозной sha256");
        assert!(rep.all_ok, "per-file sha: {:?}", rep.files_bad);
        // длинное имя сохранилось ЦЕЛИКОМ (GNU 'L')
        assert!(
            files.iter().any(|f| f.name.contains(&"L".repeat(140))),
            "длинное имя потеряно: {:?}",
            files.iter().map(|f| f.name.clone()).collect::<Vec<_>>()
        );
        // round-trip содержимого
        let ent = files.iter().find(|f| f.name.ends_with("a.txt")).unwrap();
        let mut buf = Vec::new();
        reader.read_range(ent.raw_off, ent.raw_len as usize, &mut buf).unwrap();
        assert_eq!(buf, b"alpha line\nbeta line\n");
    }

    /// Слот-машина: байты архива зависят ТОЛЬКО от порядка манифеста —
    /// fetch_jobs=1 байтово равен fetch_jobs=4.
    #[test]
    fn manifest_determinism_regardless_of_fetch_jobs() {
        let d = tmp("det");
        let mut lines = Vec::new();
        for i in 0..12 {
            let p = d.join(format!("u{i:02}.bin"));
            write_file(&p, &entropy(300_000, 100 + i as u64));
            lines.push(p.display().to_string());
        }
        // дословный дубликат — дедуп BLAKE3 должен его схлопнуть
        let dup = d.join("dup.bin");
        write_file(&dup, &entropy(300_000, 100));
        lines.push(dup.display().to_string());
        let manifest = d.join("m.txt");
        std::fs::write(&manifest, manifest_of(&lines)).unwrap();
        let o1 = d.join("j1.poler");
        let o2 = d.join("j4.poler");
        let s1 = manifest_pack(&manifest, &o1, cfg(), opts(1)).unwrap();
        let s2 = manifest_pack(&manifest, &o2, cfg(), opts(4)).unwrap();
        assert_eq!(s1.units_packed, 13);
        assert!(s2.dedup_chunks >= 1, "дубликат не схлопнут: {}", s2.dedup_chunks);
        assert_eq!(
            std::fs::read(&o1).unwrap(),
            std::fs::read(&o2).unwrap(),
            "байты архива обязаны зависеть только от порядка манифеста"
        );
    }

    /// Резюм: прогон 1 (половина манифеста) → .units.done; прогон 2 (весь
    /// манифест) докатывает только недостающее; сквозной дайджест и все
    /// записи целы ПОСЛЕ доклейки.
    #[test]
    fn manifest_resume_appends_and_keeps_digest() {
        let d = tmp("resume");
        let mut lines = Vec::new();
        for i in 0..6 {
            let p = d.join(format!("r{i}.bin"));
            write_file(&p, &entropy(250_000, 500 + i as u64));
            lines.push(p.display().to_string());
        }
        let full = d.join("full.txt");
        std::fs::write(&full, manifest_of(&lines)).unwrap();
        let half = d.join("half.txt");
        std::fs::write(&half, manifest_of(&lines[..3])).unwrap();
        let out = d.join("res.poler");
        // прогон 1: 3 юнита, чекпоинт после каждого
        let s1 = manifest_pack(&half, &out, cfg(), opts_ckpt(2, 1)).unwrap();
        assert_eq!(s1.units_packed, 3);
        let done_text = std::fs::read_to_string(units_done_path(&out)).unwrap();
        assert_eq!(done_text.lines().count(), 3, ".units.done пережил прогон");
        // прогон 2: полный манифест — 3 пропущены, 3 докатаны ДОКЛЕЙКОЙ
        let s2 = manifest_pack(&full, &out, cfg(), opts_ckpt(2, 1)).unwrap();
        assert_eq!(s2.units_skipped_done, 3);
        assert_eq!(s2.units_packed, 3);
        assert!(s2.resumed_archive);
        let reader = PolerReader::open(&out).unwrap();
        assert_eq!(reader.files().len(), 6);
        let rep = reader.verify().unwrap();
        assert!(rep.stream_ok, "сквозной дайджест после доклейки: {:?}", rep);
        assert!(rep.all_ok, "все 6 записей целы: {:?}", rep.files_bad);
    }

    /// Краш посреди доклейки: файл усечён на index_off и загажен мусором —
    /// open_append ремонтирует по state-sidecar, докатка продолжается.
    #[test]
    fn manifest_crash_repair_from_sidecar() {
        let d = tmp("crash");
        let mut lines = Vec::new();
        for i in 0..4 {
            let p = d.join(format!("c{i}.bin"));
            write_file(&p, &entropy(200_000, 900 + i as u64));
            lines.push(p.display().to_string());
        }
        let full = d.join("m.txt");
        std::fs::write(&full, manifest_of(&lines)).unwrap();
        let half = d.join("half.txt");
        std::fs::write(&half, manifest_of(&lines[..2])).unwrap();
        let out = d.join("crash.poler");
        // прогон 1: 2 юнита, чекпоинт после каждого
        manifest_pack(&half, &out, cfg(), opts_ckpt(2, 1)).unwrap();
        // КРАШ: файл усечён до index_off последнего чекпоинта и добит
        // мусором (реальное состояние упавшего open_append)
        {
            let reader = PolerReader::open(&out).unwrap();
            let layout = reader.layout();
            drop(reader);
            let mut f = OpenOptions::new().read(true).write(true).open(&out).unwrap();
            f.set_len(layout.index_off + 8192).unwrap();
            use std::io::{Seek, SeekFrom};
            f.seek(SeekFrom::Start(layout.index_off)).unwrap();
            f.write_all(&vec![0xCDu8; 8192]).unwrap();
        }
        // резюм: ремонт по sidecar + докатка юнитов 3-4
        let s = manifest_pack(&full, &out, cfg(), opts_ckpt(2, 1)).unwrap();
        assert_eq!(s.units_skipped_done, 2);
        assert_eq!(s.units_packed, 2);
        assert!(s.resumed_archive);
        let reader = PolerReader::open(&out).unwrap();
        assert_eq!(reader.files().len(), 4);
        let rep = reader.verify().unwrap();
        assert!(
            rep.stream_ok && rep.all_ok,
            "ремонт вернул архив к чекпоинту, доклейка цела: {rep:?}"
        );
    }

    /// Большой юнит (порог атомарности занижен): стримится телом записи
    /// с декларированным размером; содержимое round-trip без потерь.
    #[test]
    fn manifest_big_unit_streams() {
        let d = tmp("big");
        let p = d.join("big.bin");
        let body = entropy(300_000, 77);
        write_file(&p, &body);
        let manifest = d.join("m.txt");
        let lines = [p.display().to_string()];
        std::fs::write(&manifest, manifest_of(&lines)).unwrap();
        let out = d.join("big.poler");
        let o = ManifestOptions {
            unit_buf_max: 1024,
            fetch_jobs: 1,
            checkpoint_units: u64::MAX,
            checkpoint_secs: u64::MAX,
            progress: false,
            ..Default::default()
        };
        let stats = manifest_pack(&manifest, &out, cfg(), o).unwrap();
        assert_eq!(stats.units_packed, 1);
        let reader = PolerReader::open(&out).unwrap();
        assert_eq!(reader.files().len(), 1);
        let rep = reader.verify().unwrap();
        assert!(rep.stream_ok && rep.all_ok);
        let f = &reader.files()[0];
        assert_eq!(f.raw_len, 300_000);
        let mut buf = Vec::new();
        reader.read_range(f.raw_off, f.raw_len as usize, &mut buf).unwrap();
        assert_eq!(buf, body);
    }

    /// Живой HTTP: мини-сервер на std::net отдаёт файл и tar.gz —
    /// удалённые юниты укладываются тем же конвейером (атомарный путь,
    /// распаковка НА ЛЕТУ в воркере).
    #[test]
    fn manifest_remote_unit_over_http() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served_txt = entropy(120_000, 42);
        let served_tgz: Vec<u8> = {
            let mut tar_bytes = Vec::new();
            {
                let mut b = tar::Builder::new(&mut tar_bytes);
                let h = |size: u64| {
                    let mut h = tar::Header::new_gnu();
                    h.set_size(size);
                    h.set_mode(0o644);
                    h.set_cksum();
                    h
                };
                b.append_data(&mut h(5), "net/one.txt", b"first".as_slice()).unwrap();
                b.append_data(&mut h(6), "net/two.txt", b"second".as_slice()).unwrap();
                b.finish().unwrap();
            }
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            gz.write_all(&tar_bytes).unwrap();
            gz.finish().unwrap()
        };
        let tgz = served_tgz.clone();
        let txt = served_txt.clone();
        let handle = thread::spawn(move || {
            // неблокирующий accept + дедлайн: сервер сам завершается,
            // join не подвешивает тест (прошлый вариант с блокирующим
            // incoming() ждал соединений вечно)
            listener.set_nonblocking(true).unwrap();
            let deadline = Instant::now() + Duration::from_secs(60);
            let mut served = 0;
            while Instant::now() < deadline && served < 16 {
                let mut s = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                served += 1;
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf); // заголовок запроса
                let req = String::from_utf8_lossy(&buf).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (code, body): (&str, Vec<u8>) = if path == "/plain.txt" {
                    ("200 OK", txt.clone())
                } else if path == "/pair.tar.gz" {
                    ("200 OK", tgz.clone())
                } else {
                    ("404 Not Found", Vec::new())
                };
                let head = format!(
                    "HTTP/1.1 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
                let _ = s.flush();
            }
        });

        let d = tmp("http");
        let manifest = d.join("m.txt");
        let lines = [
            format!("http://127.0.0.1:{port}/plain.txt"),
            format!("http://127.0.0.1:{port}/pair.tar.gz"),
        ];
        std::fs::write(&manifest, manifest_of(&lines)).unwrap();
        let out = d.join("net.poler");
        let stats = manifest_pack(&manifest, &out, cfg(), opts(2)).unwrap();
        assert_eq!(stats.units_packed, 2);
        assert_eq!(stats.units_failed, 0);
        let reader = PolerReader::open(&out).unwrap();
        let names: Vec<&str> = reader.files().iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"plain.txt"),
            "plain-файл не в таблице: {names:?}"
        );
        assert!(names.contains(&"net/one.txt"), "tar.gz не распакован: {names:?}");
        assert!(names.contains(&"net/two.txt"), "tar.gz не распакован: {names:?}");
        let rep = reader.verify().unwrap();
        assert!(rep.stream_ok && rep.all_ok, "{rep:?}");
        // round-trip удалённого файла
        let ent = reader.files().iter().find(|f| f.name == "plain.txt").unwrap();
        let mut buf = Vec::new();
        reader.read_range(ent.raw_off, ent.raw_len as usize, &mut buf).unwrap();
        assert_eq!(buf, served_txt);
        // сервер сам завершится по дедлайну/лимиту — не ждём его
        drop(handle);
    }

    /// Ленивая активация: нечего докатывать — архив НЕ трогается
    /// (mtime не меняется), статистика из метаданных.
    #[test]
    fn manifest_lazy_when_nothing_to_append() {
        let d = tmp("lazy");
        let p = d.join("one.txt");
        write_file(&p, b"single unit\n");
        let manifest = d.join("m.txt");
        let lines = [p.display().to_string()];
        std::fs::write(&manifest, manifest_of(&lines)).unwrap();
        let out = d.join("lazy.poler");
        manifest_pack(&manifest, &out, cfg(), opts(1)).unwrap();
        let before = std::fs::metadata(&out).unwrap().modified().unwrap();
        // повторный запуск того же манифеста
        thread::sleep(Duration::from_millis(20));
        let s = manifest_pack(&manifest, &out, cfg(), opts(1)).unwrap();
        let after = std::fs::metadata(&out).unwrap().modified().unwrap();
        assert_eq!(s.units_packed, 0);
        assert_eq!(s.units_skipped_done, 1);
        assert_eq!(before, after, "архив обязан остаться нетронутым");
        assert_eq!(s.files, 1);
    }
}

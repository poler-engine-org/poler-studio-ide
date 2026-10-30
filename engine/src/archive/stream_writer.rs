//! Streaming-компрессор `.poler`: сетевой поток → чанки FastCDC →
//! BLAKE3-дедупликация → многоярусный zstd — БЕЗ сырой выгрузки на диск
//! (директива docs/DIRECTIVE_STREAMING_INGESTION_PIPELINE.md, §2B).
//!
//! RAM-дисциплина: буфер нарезки ≤ max+64 KiB (~1.1 МБ), реестр
//! дедупа ~24 Б/чанк, логический индекс 24 Б/чанк, таблица файлов
//! льётся в sidecar-файл метаданных (имена/смещения/SHA256 — никогда
//! сырые данные), zstd-контексты создаются один раз. Пик RSS
//! сообщается в статистике (VmHWM из /proc/self/status).
//!
//! Crash-safety: запись идёт в `<out>.poler.part`, атомарный rename
//! в конце; брошенный без finish() писатель удаляет свои temporaries.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
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

use rayon::prelude::*;

use super::dedup::{chunk_hash, cut_point, ChunkDedup, CdcParams};
use crate::pqc::sha256::{hex, Sha256};

// ────────────────────────── Формат .poler v1 ──────────────────────────
// Все числа little-endian. Раскладка файла:
//   [0..8)                  magic "POLERARC"
//   [8..)                   физические чанки подряд:
//                             blake3[32] raw_len u32 stored_len u32
//                             method u8 pad[3]  payload[stored_len]
//   ...                     логический индекс (24 Б/чанк):
//                             raw_off u64 stored_off u64 raw_len u32 pad u32
//   ...                     файловая таблица:
//                             name_len u16 name[..] raw_off u64
//                             raw_len u64 sha256[32]
//   [len-104..len)          трейлер "POLEREND"
pub const MAGIC_START: &[u8; 8] = b"POLERARC";
pub const MAGIC_END: &[u8; 8] = b"POLEREND";
/// v0.92.0 STREAM-CORPUS: state-sidecar `<out>.state` — запись ремонта
/// для [`StreamWriter::open_append`] (последний валидный чекпоинт).
pub const STATE_MAGIC: &[u8; 8] = b"POLERST1";
pub const POLER_VERSION: u16 = 1;
pub const TRAILER_SIZE: usize = 104;
pub const CHUNK_HEADER_SIZE: usize = 44;
pub const INDEX_ENTRY_SIZE: usize = 24;
pub const FILE_ENTRY_FIXED: usize = 2 + 8 + 8 + 32; // + имя переменной длины

pub const METHOD_STORE: u8 = 0;
pub const METHOD_ZFAST: u8 = 1; // zstd ~3
pub const METHOD_ZDEEP: u8 = 2; // zstd ~15

/// Ярус сжатия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum CompressTier {
    /// Только быстрый уровень (zstd-3): максимум скорости.
    Fast,
    /// Только глубокий (zstd-15): максимум плотности.
    Deep,
    /// Авто: zstd-3; если чанк сжимается лучше 2:1 — пробуем zstd-15
    /// и берём меньший результат. Дефолт директивы.
    #[default]
    Auto,
}


/// Конфигурация потоковой записи.
#[derive(Debug, Clone)]
pub struct StreamWriteConfig {
    pub cdc: CdcParams,
    /// BLAKE3-дедупликация чанков (true по умолчанию).
    pub dedup: bool,
    pub tier: CompressTier,
    /// Размер чтения из источника (сеть/файл), байт.
    pub read_chunk: usize,
    /// Прогресс в stderr каждые N сырых байт (0 = молчать).
    pub progress_bytes: u64,
    /// v0.91.0 CORPUS-TRIT: число потоков сжатия. 0/1 — прежний
    /// последовательный путь; ≥2 — буфер нарезки растёт до батча
    /// (макс × min(jobs·2, 16)) и мисс-чанки сжимаются на rayon.
    /// Выходные байты идентичны любому jobs: границы CDC зависят только
    /// от содержимого, zstd-кадры сжимаются побайтово одинаково, запись
    /// идёт строго в порядке потока.
    pub jobs: usize,
    /// v0.91.0: уровень глубокого яруса (дефолт 15 — прежнее поведение;
    /// корпусной профиль v0.91 ставит 19). Уровень НЕ кодируется в
    /// формате: кадр zstd самодокументирован, читатель безразличен к
    /// уровню — .poler v1 остаётся .poler v1.
    pub deep_level: i32,
}

impl Default for StreamWriteConfig {
    fn default() -> Self {
        StreamWriteConfig {
            cdc: CdcParams::default(),
            dedup: true,
            tier: CompressTier::Auto,
            read_chunk: 64 * 1024,
            progress_bytes: 256 * 1024 * 1024,
            jobs: 0,
            deep_level: 15,
        }
    }
}

/// Итог потоковой записи (serde: JSON в stdout CLI).
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamWriteStats {
    pub output: String,
    pub total_raw: u64,
    pub total_stored: u64,
    pub ratio: f64,
    pub logical_chunks: u64,
    pub physical_chunks: u64,
    pub dedup_chunks: u64,
    pub dedup_bytes: u64,
    pub stored_chunks: u64,
    pub fast_chunks: u64,
    pub deep_chunks: u64,
    pub files: u64,
    pub tar_mode: bool,
    pub stream_sha256_hex: String,
    pub elapsed_ms: u64,
    pub throughput_mbs: f64,
    /// Пик RSS процесса (VmHWM), КБ — критерий ≤ 48 МБ.
    pub peak_rss_kb: u64,
}

/// Пик RSS процесса (VmHWM /proc/self/status; вне Linux — 0).
pub fn peak_rss_kb() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/self/status") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                return rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
    }
    0
}

// ────────────────────────── Логический индекс ─────────────────────────

#[derive(Debug, Clone, Copy)]
struct LogicalEntry {
    raw_off: u64,
    stored_off: u64,
    raw_len: u32,
}

impl LogicalEntry {
    fn to_bytes(self) -> [u8; INDEX_ENTRY_SIZE] {
        let mut b = [0u8; INDEX_ENTRY_SIZE];
        b[0..8].copy_from_slice(&self.raw_off.to_le_bytes());
        b[8..16].copy_from_slice(&self.stored_off.to_le_bytes());
        b[16..20].copy_from_slice(&self.raw_len.to_le_bytes());
        b[20..24].copy_from_slice(&0u32.to_le_bytes());
        b
    }

    /// v0.92.0: разбор записи логического индекса (open_append).
    fn from_bytes(b: &[u8; INDEX_ENTRY_SIZE]) -> LogicalEntry {
        LogicalEntry {
            raw_off: u64::from_le_bytes(b[0..8].try_into().unwrap()),
            stored_off: u64::from_le_bytes(b[8..16].try_into().unwrap()),
            raw_len: u32::from_le_bytes(b[16..20].try_into().unwrap()),
        }
    }
}

// ────────────────────────── tar-наблюдатель ───────────────────────────

/// Завершённый файл tar-потока.
pub struct TarFileDone {
    pub name: String,
    pub raw_off: u64,
    pub raw_len: u64,
    pub sha256: [u8; 32],
}

/// Наблюдатель tar-потока: байты не меняются, распознаются только
/// границы файлов (ustar + GNU longname 'L'/'K' + base-256 размеры)
/// и их SHA256. Сломавшийся заголовок выключает трекинг; целостность
/// потока не страдает.
struct TarObserver {
    /// Абсолютная позиция в сыром потоке.
    pos: u64,
    /// Абсолютное начало текущего 512-блока (заголовка):
    /// pos отстаёт на незасчитанный кусок среза.
    block_start: u64,
    /// Накопитель текущего 512-байтного блока заголовка.
    block: [u8; 512],
    filled: usize,
    state: ObsState,
    /// Текущий файл (имя, raw_off) — hash живёт в state.
    current: Option<(String, u64)>,
    /// Имя, отложенное записью GNU 'L'.
    long_name: Option<String>,
    /// Паддинг данных текущей записи (до кратности 512).
    pending_pad: u64,
    tar_detected: bool,
    files: u64,
}

enum ObsState {
    /// Первый блок ещё не проверен.
    Probing,
    /// Копим 512-байтный блок заголовка.
    Header,
    /// Идут данные файла: осталось байт, sha256.
    FileData(u64, Sha256),
    /// Данные нерегулярной записи — пропустить байт.
    Skip(u64),
    /// Данные записи 'L'/'K': осталось байт, буфер имени.
    LongName(u64, Vec<u8>),
    /// tar кончился (или не он): поток больше не разбирается.
    Done,
}

impl TarObserver {
    fn new() -> Self {
        TarObserver {
            pos: 0,
            block_start: 0,
            block: [0u8; 512],
            filled: 0,
            state: ObsState::Probing,
            current: None,
            long_name: None,
            pending_pad: 0,
            tar_detected: false,
            files: 0,
        }
    }

    /// v0.92.0 STREAM-CORPUS: возобновление наблюдателя после доклейки.
    /// Позиция потока продолжается с `pos`; чекпоинты проходят только
    /// НА ГРАНИЦАХ юнитов — незавершённых tar-записей в этот момент не
    /// бывает, состояние Header. Если tar ещё не распознан (пустой или
    /// сырой архив), остаёмся в Probing — первый заголовок следующего
    /// юнита пере-детектит tar-режим.
    fn resume(pos: u64, tar_detected: bool) -> Self {
        TarObserver {
            pos,
            block_start: pos,
            block: [0u8; 512],
            filled: 0,
            state: if tar_detected { ObsState::Header } else { ObsState::Probing },
            current: None,
            long_name: None,
            pending_pad: 0,
            tar_detected,
            files: 0,
        }
    }

    fn tracking(&self) -> bool {
        !matches!(self.state, ObsState::Done)
    }

    fn is_tar(&self) -> bool {
        self.tar_detected
    }

    /// Скармливаем очередные сырые байты (в порядке потока);
    /// завершённые файлы возвращаются вектор (не callback —
    /// чтобы не воевать с borrow-чекером за &mut self).
    fn feed(&mut self, bytes: &[u8], out: &mut Vec<TarFileDone>) {
        let mut rest = bytes;
        while !rest.is_empty() && self.tracking() {
            let state = std::mem::replace(&mut self.state, ObsState::Done);
            let take;
            match state {
                ObsState::Done => return,
                ObsState::FileData(mut left, mut hash) => {
                    take = (left as usize).min(rest.len());
                    hash.update(&rest[..take]);
                    left -= take as u64;
                    if left == 0 {
                        if let Some((name, raw_off)) = self.current.take() {
                            out.push(TarFileDone {
                                name,
                                raw_off,
                                raw_len: self.pos + take as u64 - raw_off,
                                sha256: hash.finalize(),
                            });
                            self.files += 1;
                        }
                        let pad = self.pending_pad;
                        self.state = if pad > 0 { ObsState::Skip(pad) } else { ObsState::Header };
                    } else {
                        self.state = ObsState::FileData(left, hash);
                    }
                }
                ObsState::Skip(mut left) => {
                    take = (left as usize).min(rest.len());
                    left -= take as u64;
                    self.state = if left == 0 { ObsState::Header } else { ObsState::Skip(left) };
                }
                ObsState::LongName(mut left, mut buf) => {
                    take = (left as usize).min(rest.len());
                    buf.extend_from_slice(&rest[..take]);
                    left -= take as u64;
                    if left == 0 {
                        let mut name = String::from_utf8_lossy(&buf).to_string();
                        while name.ends_with('\0') {
                            name.pop();
                        }
                        self.long_name = Some(name);
                        let pad = self.pending_pad;
                        self.state = if pad > 0 { ObsState::Skip(pad) } else { ObsState::Header };
                    } else {
                        self.state = ObsState::LongName(left, buf);
                    }
                }
                ObsState::Probing | ObsState::Header => {
                    // копим блок заголовка; state восстанавливаем ДО
                    // on_header — он различает Probing/Tracking
                    let need = 512 - self.filled;
                    if self.filled == 0 {
                        self.block_start = self.pos;
                    }
                    take = need.min(rest.len());
                    self.block[self.filled..self.filled + take].copy_from_slice(&rest[..take]);
                    self.filled += take;
                    self.state = state;
                    if self.filled == 512 {
                        let block = self.block;
                        self.filled = 0;
                        self.on_header(block, out);
                    }
                }
            }
            self.pos += take as u64;
            rest = &rest[take..];
        }
    }

    /// Разбор заголовка.
    fn on_header(&mut self, b: [u8; 512], out: &mut Vec<TarFileDone>) {
        let all_zero = b.iter().all(|&x| x == 0);
        let probing = matches!(self.state, ObsState::Probing);
        self.state = ObsState::Header;

        if all_zero {
            if probing {
                self.state = ObsState::Done; // пустой поток/не tar
                return;
            }
            // ГРАБЛЯ №47 (v0.92.0 STREAM-CORPUS): в манифестном конвейере
            // за tar-юнитом сразу идёт следующий, а каждый tar несёт
            // собственные завершающие нулевые блоки (tar::Builder::finish()
            // пишет 2×512). Прежний необратимый Done съедал ВСЕ юниты
            // после первого же хвоста — разбор таблицы файлов умирал.
            // Нулевые блоки проглатываются, разбор продолжается со
            // следующего реального заголовка; одиночный tar не меняется
            // (хвост просто дочитывается вхолостую до EOF).
            self.state = ObsState::Header;
            return;
        }
        if !Self::valid_magic(&b) {
            if probing {
                self.state = ObsState::Done; // не tar — обычный поток
                return;
            }
            self.state = ObsState::Done; // сломанный заголовок
            return;
        }
        if probing {
            self.tar_detected = true;
        }

        let size = Self::size_field(&b[124..136]).unwrap_or(0);
        let typeflag = b[156];
        let pad = (512 - (size % 512)) % 512;
        self.pending_pad = pad;

        match typeflag {
            b'L' | b'K' => {
                self.state = ObsState::LongName(size, Vec::with_capacity(size as usize));
            }
            b'0' | 0 | b' ' | b'7' => {
                let name = match self.long_name.take() {
                    Some(long) => long,
                    None => {
                        let n = b[..100].iter().position(|&c| c == 0).unwrap_or(100);
                        let mut name = String::from_utf8_lossy(&b[..n]).to_string();
                        let pn = b[345..500].iter().position(|&c| c == 0).unwrap_or(155);
                        if pn > 0 {
                            let prefix = String::from_utf8_lossy(&b[345..345 + pn]).to_string();
                            name = format!("{prefix}/{name}");
                        }
                        name
                    }
                };
                if size == 0 {
                    out.push(TarFileDone {
                        name,
                        raw_off: self.block_start + 512,
                        raw_len: 0,
                        sha256: Sha256::new().finalize(),
                    });
                    self.files += 1;
                    self.state = ObsState::Header;
                } else {
                    self.current = Some((name, self.block_start + 512));
                    self.state = ObsState::FileData(size, Sha256::new());
                }
            }
            _ => {
                // каталоги/симлинки/прочее: данные (если есть) пропускаем
                self.long_name = None;
                self.state = if size + pad > 0 {
                    ObsState::Skip(size + pad)
                } else {
                    ObsState::Header
                };
            }
        }
    }

    /// EOF: закрыть незавершённый файл (усечённый tar — честно
    /// отдаём то, что видели; верификация вскроет обрыв).
    fn finish(&mut self, out: &mut Vec<TarFileDone>) {
        if let ObsState::FileData(_, hash) = std::mem::replace(&mut self.state, ObsState::Done) {
            if let Some((name, raw_off)) = self.current.take() {
                out.push(TarFileDone {
                    name,
                    raw_off,
                    raw_len: self.pos - raw_off,
                    sha256: hash.finalize(),
                });
                self.files += 1;
            }
        }
        self.state = ObsState::Done;
    }

    fn valid_magic(b: &[u8; 512]) -> bool {
        &b[257..262] == b"ustar"
    }

    /// Размер записи: восьмеричное поле ИЛИ GNU base-256
    /// (старший бит первого байта) — для файлов > 8 GiB.
    fn size_field(field: &[u8]) -> Option<u64> {
        if field[0] & 0x80 != 0 {
            let mut v: u128 = (field[0] & 0x7f) as u128;
            for &b in &field[1..] {
                v = (v << 8) | b as u128;
            }
            return Some(v.min(u64::MAX as u128) as u64);
        }
        let s: String = field
            .iter()
            .take_while(|&&c| c != 0 && c != b' ')
            .map(|&c| c as char)
            .collect();
        u64::from_str_radix(s.trim(), 8).ok()
    }
}

// ────────────────────────── Сжатие чанков ──────────────────────────

/// План чанка пачки (v0.91.0): результат фазы нарезки/дедупа ДО сжатия.
struct ChunkPlan {
    raw_off: u64,
    raw_len: u32,
    hash: [u8; 32],
    outcome: ChunkOutcome,
}

/// Судьба чанка пачки.
enum ChunkOutcome {
    /// Дедуп-хит на чанке ИЗ ПРЕДЫДУЩИХ пачек (уже записан —
    /// stored_off известен из реестра).
    HitStored { stored_off: u64 },
    /// Дедуп-хит на чанке ЭТОЙ ЖЕ пачки (грабля v0.91.0: фаза планов
    /// выполняется до записи, реестр ещё не видел пачку — полный
    /// BLAKE3-ключ точен, stored_off мисса-предшественника будет
    /// известен к моменту записи: мисс всегда пишется раньше).
    HitInBatch { ordinal: usize },
    /// Мисс: порядковый номер в векторе сжатых результатов.
    Miss { ordinal: usize },
}

/// Пара переиспользуемых zstd-контекстов писателя: создание контекста —
/// самая дорогая часть разового сжатия чанка, поэтому контексты живут
/// столько же, сколько писатель (последовательный путь) или rayon-поток
/// (батч-путь v0.91.0).
struct ChunkCompressors {
    fast: Option<zstd::bulk::Compressor<'static>>,
    deep: Option<zstd::bulk::Compressor<'static>>,
    deep_level: i32,
}

impl ChunkCompressors {
    fn new(deep_level: i32) -> io::Result<ChunkCompressors> {
        Ok(ChunkCompressors { fast: None, deep: None, deep_level })
    }

    fn zfast(&mut self, raw: &[u8]) -> io::Result<Vec<u8>> {
        if self.fast.is_none() {
            self.fast = Some(zstd_compressor(3)?);
        }
        self.fast.as_mut().unwrap().compress(raw)
    }

    fn zdeep(&mut self, raw: &[u8]) -> io::Result<Vec<u8>> {
        if self.deep.is_none() {
            self.deep = Some(zstd_compressor(self.deep_level)?);
        }
        self.deep.as_mut().unwrap().compress(raw)
    }

    /// Многоярусное сжатие одного чанка (логика прежняя, до v0.91.0).
    fn compress(&mut self, raw: &[u8], tier: CompressTier) -> io::Result<(u8, Vec<u8>)> {
        if raw.len() < 512 {
            return Ok((METHOD_STORE, raw.to_vec()));
        }
        match tier {
            CompressTier::Fast => {
                let c = self.zfast(raw)?;
                if c.len() < raw.len() * 15 / 16 {
                    Ok((METHOD_ZFAST, c))
                } else {
                    Ok((METHOD_STORE, raw.to_vec()))
                }
            }
            CompressTier::Deep => {
                let c = self.zdeep(raw)?;
                if c.len() < raw.len() * 15 / 16 {
                    Ok((METHOD_ZDEEP, c))
                } else {
                    Ok((METHOD_STORE, raw.to_vec()))
                }
            }
            CompressTier::Auto => {
                let c3 = self.zfast(raw)?;
                if c3.len() >= raw.len() * 15 / 16 {
                    return Ok((METHOD_STORE, raw.to_vec()));
                }
                if c3.len() <= raw.len() / 2 {
                    // хорошо сжимается — глубокий ярус может дать кратно
                    let cd = self.zdeep(raw)?;
                    if cd.len() < c3.len() {
                        return Ok((METHOD_ZDEEP, cd));
                    }
                }
                Ok((METHOD_ZFAST, c3))
            }
        }
    }
}

thread_local! {
    /// v0.91.0: контексты rayon-потоков батч-сжатия. По паре на поток,
    /// создание amortized по всему корпусу; смена deep_level
    /// (профили/тесты) пересобирает контекст на первом чанке потока.
    static TLS_COMPRESSORS: RefCell<Option<ChunkCompressors>> =
        const { RefCell::new(None) };
}

/// Сжатие чанка на rayon-потоке: TLS-контексты, байтово та же логика,
/// что у последовательного пути ([`ChunkCompressors::compress`]).
fn tls_compress(raw: &[u8], tier: CompressTier, deep_level: i32) -> io::Result<(u8, Vec<u8>)> {
    TLS_COMPRESSORS.with(|slot| {
        let mut guard = slot.borrow_mut();
        let rebuild = match guard.as_ref() {
            Some(c) => c.deep_level != deep_level,
            None => true,
        };
        if rebuild {
            *guard = Some(ChunkCompressors::new(deep_level)?);
        }
        guard.as_mut().unwrap().compress(raw, tier)
    })
}

// ────────────────────────── StreamWriter ──────────────────────────────

/// Потоковый писатель .poler-контейнера.
pub struct StreamWriter {
    cfg: StreamWriteConfig,
    file: File,
    part_path: PathBuf,
    final_path: PathBuf,
    /// Sidecar файлой таблицы (только метаданные, не сырые данные).
    files_spill: File,
    files_spill_path: PathBuf,
    files_count: u64,
    logical: Vec<LogicalEntry>,
    dedup: ChunkDedup,
    /// Позиции.
    raw_pos: u64,
    stored_pos: u64,
    /// Буфер нарезки (ёмкость ≤ max + slack, при jobs≥2 — батч).
    buf: Vec<u8>,
    observer: TarObserver,
    stream_hash: Sha256,
    /// Пара zstd-контекстов (fast/deep) — v0.91.0 вынесена из полей
    /// fast/deep для переиспользования логики батч-путём.
    compressors: ChunkCompressors,
    /// Статистика.
    dedup_chunks: u64,
    dedup_bytes: u64,
    stored_chunks: u64,
    fast_chunks: u64,
    deep_chunks: u64,
    started: std::time::Instant,
    last_progress: u64,
    finished: bool,
    /// v0.92.0 STREAM-CORPUS: режим доклейки — часть файла, а не `.part`
    /// (Drop не сносит архив: усечённый файл ремонтируется по sidecar).
    in_append: bool,
}

impl StreamWriter {
    /// Открыть запись: создаётся `<out>.poler.part`, атомарное
    /// переименование в `<out>` — в [`StreamWriter::finish`].
    pub fn open(out: &Path, cfg: StreamWriteConfig) -> io::Result<StreamWriter> {
        let part_path = sidecar_path(out, "part");
        let files_spill_path = sidecar_path(out, "part.files");
        // read+write: дедуп-верификация читает заголовки только что
        // записанных чанков через read_exact_at из page cache
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&part_path)?;
        file.write_all(MAGIC_START)?;
        let files_spill = File::create(&files_spill_path)?;
        let cap = cfg.cdc.max + cfg.read_chunk;
        let deep_level = cfg.deep_level;
        Ok(StreamWriter {
            cfg,
            file,
            part_path,
            final_path: out.to_path_buf(),
            files_spill,
            files_spill_path,
            files_count: 0,
            logical: Vec::new(),
            dedup: ChunkDedup::new(),
            raw_pos: 0,
            stored_pos: MAGIC_START.len() as u64,
            buf: Vec::with_capacity(cap),
            observer: TarObserver::new(),
            stream_hash: Sha256::new(),
            compressors: ChunkCompressors::new(deep_level)?,
            dedup_chunks: 0,
            dedup_bytes: 0,
            stored_chunks: 0,
            fast_chunks: 0,
            deep_chunks: 0,
            started: std::time::Instant::now(),
            last_progress: 0,
            finished: false,
            in_append: false,
        })
    }

    /// v0.92.0 STREAM-CORPUS: открыть СУЩЕСТВУЮЩИЙ .poler для доклейки
    /// НА МЕСТЕ. Архив обязан быть завершённым (валидный трейлер) —
    /// либо чинится по state-sidecar (`.state`, пишется каждым finish()
    /// и каждым open_append ДО усечения).
    ///
    /// Полное восстановление состояния писателя:
    /// * логический индекс — из региона индекса архива;
    /// * файловая таблица — регион таблицы пересыпается в свежий spill;
    /// * реестр дедупа — физическим обходом чанков (44-байт заголовки);
    /// * сквозной SHA256 потока — декомпрессией логических чанков ПО
    ///   ПОРЯДКУ: verify() сверяет дайджест ВСЕГО потока, без этого
    ///   доклейка молча ломала бы верификацию всего архива;
    /// * TarObserver — resume(raw_pos, tar_mode): чекпоинты проходят
    ///   только на границах юнитов, незавершённых записей не бывает.
    ///
    /// Диск: пик = размер архива (усечение на index_off + дозапись в
    /// тот же файл), НЕ 2× архива. Порядок: sidecar ДО усечения —
    /// падение на любом шаге откатывается к последнему чекпоинту.
    pub fn open_append(out: &Path, cfg: StreamWriteConfig) -> io::Result<StreamWriter> {
        let meta = std::fs::metadata(out)
            .map_err(|e| io::Error::other(format!("open_append: {}: {e}", out.display())))?;
        if !meta.is_file() {
            return Err(io::Error::other("open_append: выход не является файлом"));
        }
        let layout = match TrailerLayout::read(out, meta.len()) {
            Some(l) => l,
            None => {
                // битый/отсутствующий трейлер — типичное следствие падения
                // посреди прошлой доклейки (файл усечен и дописан чанками)
                repair_from_sidecar(out)?;
                let len = std::fs::metadata(out)?.len();
                TrailerLayout::read(out, len).ok_or_else(|| {
                    io::Error::other("open_append: трейлер не восстановился по state-sidecar")
                })?
            }
        };
        // sidecar ДО усечения: зафиксировать валидное состояние архива
        write_state_sidecar(out, layout.final_len, layout.index_off)?;

        // логический индекс
        let mut index_bytes = vec![0u8; layout.index_len as usize];
        {
            let mut rf = File::open(out)?;
            rf.read_exact_at(&mut index_bytes, layout.index_off)?;
        }
        if index_bytes.len() % INDEX_ENTRY_SIZE != 0 {
            return Err(io::Error::other("open_append: индекс не кратен 24 Б"));
        }
        let logical: Vec<LogicalEntry> = index_bytes
            .chunks_exact(INDEX_ENTRY_SIZE)
            .map(|c| LogicalEntry::from_bytes(c.try_into().unwrap()))
            .collect();
        if logical.len() as u64 != layout.logical_count {
            return Err(io::Error::other(
                "open_append: счётчик индекса расходится с трейлером",
            ));
        }

        // файловая таблица → свежий spill (предзаполненный регионом таблицы)
        let files_spill_path = sidecar_path(out, "part.files");
        let (files_spill, files_count) = {
            let mut region = vec![0u8; layout.files_len as usize];
            let mut rf = File::open(out)?;
            rf.read_exact_at(&mut region, layout.files_off)?;
            let mut spill = File::create(&files_spill_path)?;
            spill.write_all(&region)?;
            let mut pos = 0usize;
            let mut count = 0u64;
            while pos + 2 <= region.len() {
                let nlen = u16::from_le_bytes([region[pos], region[pos + 1]]) as usize;
                pos += 2 + nlen + 48;
                if pos > region.len() {
                    return Err(io::Error::other("open_append: битая файловая таблица"));
                }
                count += 1;
            }
            (spill, count)
        };

        // усечение на месте: чанки остаются, индекс/таблица/трейлер
        // пересоберутся при следующем finish()
        let mut file = OpenOptions::new().read(true).write(true).open(out)?;
        file.set_len(layout.index_off)?;
        file.seek(SeekFrom::Start(layout.index_off))?;

        // реестр дедупа + счётчики методов: физический обход чанков
        let mut dedup = ChunkDedup::new();
        let mut stored_chunks = 0u64;
        let mut fast_chunks = 0u64;
        let mut deep_chunks = 0u64;
        let mut off = MAGIC_START.len() as u64;
        while off < layout.index_off {
            let mut hdr = [0u8; CHUNK_HEADER_SIZE];
            file.read_exact_at(&mut hdr, off)?;
            let stored_len = u32::from_le_bytes(hdr[36..40].try_into().unwrap()) as u64;
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hdr[0..32]);
            dedup.insert(&hash, off);
            match hdr[40] {
                METHOD_STORE => stored_chunks += 1,
                METHOD_ZFAST => fast_chunks += 1,
                _ => deep_chunks += 1,
            }
            off += CHUNK_HEADER_SIZE as u64 + stored_len;
        }
        if off != layout.index_off {
            return Err(io::Error::other(
                "open_append: регион чанков расходится с индексом (архив бит)",
            ));
        }
        if dedup.misses != layout.misses {
            return Err(io::Error::other("open_append: счётчик чанков расходится"));
        }

        // сквозной SHA256: декомпрессия логических чанков по порядку
        let mut stream_hash = Sha256::new();
        let mut raw_sum = 0u64;
        for e in &logical {
            let raw = decode_chunk_payload(&file, e.stored_off, e.raw_len)?;
            if raw.len() != e.raw_len as usize {
                return Err(io::Error::other("open_append: чанк декодировался не в raw_len"));
            }
            stream_hash.update(&raw);
            raw_sum += e.raw_len as u64;
        }
        if raw_sum != layout.raw_pos {
            return Err(io::Error::other("open_append: raw_pos расходится с индексом"));
        }

        // статистика дедупа: первое упоминание stored_off — мисс
        let mut seen: HashSet<u64> = HashSet::with_capacity(logical.len());
        let mut dedup_chunks = 0u64;
        let mut dedup_bytes = 0u64;
        for e in &logical {
            if !seen.insert(e.stored_off) {
                dedup_chunks += 1;
                dedup_bytes += e.raw_len as u64;
            }
        }

        let cap = cfg.cdc.max + cfg.read_chunk;
        let deep_level = cfg.deep_level;
        Ok(StreamWriter {
            cfg,
            file,
            part_path: out.to_path_buf(), // rename(p, p) — no-op на Linux
            final_path: out.to_path_buf(),
            files_spill,
            files_spill_path,
            files_count,
            logical,
            dedup,
            raw_pos: layout.raw_pos,
            stored_pos: layout.index_off,
            buf: Vec::with_capacity(cap),
            observer: TarObserver::resume(layout.raw_pos, layout.tar_mode),
            stream_hash,
            compressors: ChunkCompressors::new(deep_level)?,
            dedup_chunks,
            dedup_bytes,
            stored_chunks,
            fast_chunks,
            deep_chunks,
            started: std::time::Instant::now(),
            last_progress: 0,
            finished: false,
            in_append: true,
        })
    }

    /// Скармливать сырой поток порциями любого размера (сеть, сокеты,
    /// страницы краула). Границы чанков зависят только от содержимого.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut rest = bytes;
        let mut done_files = Vec::new();
        while !rest.is_empty() {
            // v0.91.0: при jobs≥2 буфер нарезки копится до батча
            // (макс × min(jobs·2, 16)) — и только потом режется: cut_ready
            // ДО этого вызывался на каждом 8-КиБ куске tar-стрима и батч
            // деградировал до 1–2 чанков (rayon простаивал). Инвариант
            // детерминизма сохранён: окно cut_point ограничено cdc.max,
            // накопление буфера границ не меняет (у jobs=0 поведение
            // байтово прежнее: режем как только есть ≥ max).
            let want = if self.cfg.jobs > 1 {
                self.cfg
                    .cdc
                    .max
                    .saturating_mul(self.cfg.jobs.min(8) * 2)
                    .max(self.cfg.cdc.max)
            } else {
                self.cfg.cdc.max
            };
            while self.buf.len() < want && !rest.is_empty() {
                let take = (want - self.buf.len()).min(rest.len());
                let chunk = &rest[..take];
                self.observer.feed(chunk, &mut done_files);
                self.stream_hash.update(chunk);
                self.buf.extend_from_slice(chunk);
                rest = &rest[take..];
            }
            for d in done_files.drain(..) {
                self.append_tar_file(d);
            }
            // батч готов (или источник исчерпан — хвост ждёт finish)
            if self.buf.len() >= want {
                self.cut_ready(false)?;
            }
        }
        Ok(())
    }

    /// Завершить поток: финализировать таблицу файлов, индекс и
    /// трейлер, атомарно переименовать `.part` → выход.
    pub fn finish(mut self, name_hint: &str) -> io::Result<StreamWriteStats> {
        self.cut_ready(true)?; // хвостовые чанки
        let mut done_files = Vec::new();
        self.observer.finish(&mut done_files);
        for d in done_files {
            self.append_tar_file(d);
        }

        let tar_mode = self.observer.is_tar();
        // Финализируем sha256 потока РОВНО ОДИН раз — после него
        // новых сырых байт не будет.
        let stream_digest = std::mem::take(&mut self.stream_hash).finalize();
        if !tar_mode {
            // одиночная запись: весь поток — один «файл»
            self.append_file_record(
                &sanitize_hint(name_hint),
                0,
                self.raw_pos,
                stream_digest,
            );
        }

        // логический индекс
        let index_off = self.stored_pos;
        let mut index_buf = Vec::with_capacity(self.logical.len() * INDEX_ENTRY_SIZE);
        for e in &self.logical {
            index_buf.extend_from_slice(&e.to_bytes());
        }
        self.file.write_all(&index_buf)?;
        self.stored_pos += index_buf.len() as u64;

        // файловая таблица: длина sidecar известна ДО вливания
        let files_off = self.stored_pos;
        let files_len = std::fs::metadata(&self.files_spill_path)?.len();
        let mut spill_read = File::open(&self.files_spill_path)?;
        let mut copy_buf = vec![0u8; 64 * 1024];
        loop {
            let n = spill_read.read(&mut copy_buf)?;
            if n == 0 {
                break;
            }
            self.file.write_all(&copy_buf[..n])?;
        }
        drop(spill_read);
        let _ = std::fs::remove_file(&self.files_spill_path);
        let final_len = files_off + files_len + TRAILER_SIZE as u64;

        // трейлер
        let mut trailer = [0u8; TRAILER_SIZE];
        trailer[0..8].copy_from_slice(MAGIC_END);
        trailer[8..10].copy_from_slice(&POLER_VERSION.to_le_bytes());
        let mut flags: u16 = 0;
        if self.cfg.dedup {
            flags |= 1;
        }
        if tar_mode {
            flags |= 2;
        }
        trailer[10..12].copy_from_slice(&flags.to_le_bytes());
        trailer[12..16].copy_from_slice(&(self.logical.len() as u32).to_le_bytes());
        trailer[16..20].copy_from_slice(&(self.dedup.misses as u32).to_le_bytes());
        trailer[20..28].copy_from_slice(&index_off.to_le_bytes());
        trailer[28..36]
            .copy_from_slice(&((self.logical.len() as u64) * (INDEX_ENTRY_SIZE as u64)).to_le_bytes());
        trailer[36..44].copy_from_slice(&files_off.to_le_bytes());
        trailer[44..52].copy_from_slice(&files_len.to_le_bytes());
        trailer[52..60].copy_from_slice(&self.raw_pos.to_le_bytes());
        trailer[60..68].copy_from_slice(&final_len.to_le_bytes());
        trailer[68..100].copy_from_slice(&stream_digest);
        self.file.write_all(&trailer)?;
        self.file.sync_all()?;
        // дескриптор закроется в Drop: rename с открытым хендлом
        // валиден на Linux

        std::fs::rename(&self.part_path, &self.final_path)?;
        self.finished = true;
        // v0.92.0 STREAM-CORPUS: state-sidecar — запись восстановления
        // (непрерывный регион [index_off, final_len)) для open_append и
        // ремонта по битому трейлеру после падения посреди доклейки.
        write_state_sidecar(&self.final_path, final_len, index_off)?;
        let total_stored = std::fs::metadata(&self.final_path)?.len();
        let elapsed = self.started.elapsed().as_millis() as u64;
        Ok(StreamWriteStats {
            output: self.final_path.display().to_string(),
            total_raw: self.raw_pos,
            total_stored,
            ratio: if self.raw_pos > 0 {
                total_stored as f64 / self.raw_pos as f64
            } else {
                0.0
            },
            logical_chunks: self.logical.len() as u64,
            physical_chunks: self.dedup.misses,
            dedup_chunks: self.dedup_chunks,
            dedup_bytes: self.dedup_bytes,
            stored_chunks: self.stored_chunks,
            fast_chunks: self.fast_chunks,
            deep_chunks: self.deep_chunks,
            files: if tar_mode { self.files_count } else { 1 },
            tar_mode,
            stream_sha256_hex: hex(&stream_digest),
            elapsed_ms: elapsed,
            throughput_mbs: if elapsed > 0 {
                self.raw_pos as f64 / 1024.0 / 1024.0 / (elapsed as f64 / 1000.0)
            } else {
                0.0
            },
            peak_rss_kb: peak_rss_kb(),
        })
    }

    /// Вырезать все готовые чанки из буфера и прогнать их пачкой через
    /// дедуп+сжатие+запись. `eof` — источник исчерпан: тогда остаток
    /// режется как хвост (граница может быть < max).
    ///
    /// v0.91.0: чанки собираются в пачку (raws) и обрабатываются
    /// [`process_batch`]; байты на выходе идентичны прежнему
    /// по-чанковому пути при том же cfg.
    fn cut_ready(&mut self, eof: bool) -> io::Result<()> {
        // buf вынимается: process_batch(&mut self) не может заимствовать
        // срез самого себя
        let mut buf = std::mem::take(&mut self.buf);
        let mut raws: Vec<Vec<u8>> = Vec::new();
        while !buf.is_empty() {
            if !eof && buf.len() < self.cfg.cdc.max {
                break; // ждём данных для детерминированной границы
            }
            let cut = cut_point(&buf, &self.cfg.cdc);
            // drain переносит байты чанка без копии, ёмкость остаётся
            raws.push(buf.drain(..cut).collect());
        }
        self.buf = buf;
        self.process_batch(raws)?;
        Ok(())
    }

    /// v0.91.0: обработка пачки чанков. Фаза 1 (главный поток): BLAKE3 +
    /// дедуп-план (реестр прошлых пачек + локальная карта пачки — дубли
    /// ВНУТРИ пачки тоже схлопываются); фаза 2: сжатие мисс-чанков
    /// (rayon при jobs≥2, иначе последовательно — байты одинаковы);
    /// фаза 3: запись строго по порядку потока. Инварианты прежнего
    /// process_chunk: raw_pos растёт по потоку, физические чанки лежат
    /// в порядке нарезки, дедуп-реестр засеивается в порядке записи,
    /// логический индекс — по raw_off.
    fn process_batch(&mut self, raws: Vec<Vec<u8>>) -> io::Result<()> {
        if raws.is_empty() {
            return Ok(());
        }
        let mut off = self.raw_pos;
        let mut plans: Vec<ChunkPlan> = Vec::with_capacity(raws.len());
        let mut miss_raws: Vec<Vec<u8>> = Vec::new();
        // hash → порядковый номер первого вхождения В ЭТОЙ пачке;
        // полный BLAKE3-ключ — коллизий нет (реестр хранит префиксы
        // и верифицирует pread'ом, локальная карта точна по построению)
        let mut batch_seen: HashMap<[u8; 32], usize> = HashMap::with_capacity(raws.len());
        for raw in raws {
            let raw_len = raw.len() as u32;
            let hash = chunk_hash(&raw);
            let raw_off = off;
            off += raw_len as u64;
            if self.cfg.dedup {
                if let Some(stored_off) = self.dedup.lookup(&hash, &self.file) {
                    self.dedup.record_hit();
                    self.dedup_chunks += 1;
                    self.dedup_bytes += raw_len as u64;
                    plans.push(ChunkPlan {
                        raw_off,
                        raw_len,
                        hash,
                        outcome: ChunkOutcome::HitStored { stored_off },
                    });
                    continue;
                }
                if let Some(&ordinal) = batch_seen.get(&hash) {
                    self.dedup.record_hit();
                    self.dedup_chunks += 1;
                    self.dedup_bytes += raw_len as u64;
                    plans.push(ChunkPlan {
                        raw_off,
                        raw_len,
                        hash,
                        outcome: ChunkOutcome::HitInBatch { ordinal },
                    });
                    continue;
                }
            }
            let ordinal = miss_raws.len();
            if self.cfg.dedup {
                batch_seen.insert(hash, ordinal);
            }
            plans.push(ChunkPlan {
                raw_off,
                raw_len,
                hash,
                outcome: ChunkOutcome::Miss { ordinal },
            });
            miss_raws.push(raw);
        }
        self.raw_pos = off;

        let tier = self.cfg.tier;
        let compressed: Vec<(u8, Vec<u8>)> = if self.cfg.jobs > 1 && miss_raws.len() > 1 {
            let deep_level = self.cfg.deep_level;
            miss_raws
                .par_iter()
                .map(|raw| tls_compress(raw, tier, deep_level))
                .collect::<io::Result<Vec<_>>>()?
        } else {
            let mut out = Vec::with_capacity(miss_raws.len());
            for raw in &miss_raws {
                out.push(self.compressors.compress(raw, tier)?);
            }
            out
        };

        let mut miss_stored: Vec<u64> = vec![0u64; miss_raws.len()];
        for plan in plans {
            let stored_off = match plan.outcome {
                ChunkOutcome::HitStored { stored_off } => stored_off,
                ChunkOutcome::HitInBatch { ordinal } => miss_stored[ordinal],
                ChunkOutcome::Miss { ordinal } => {
                    let (method, payload) = &compressed[ordinal];
                    let stored_off = self.stored_pos;
                    let mut hdr = [0u8; CHUNK_HEADER_SIZE];
                    hdr[0..32].copy_from_slice(&plan.hash);
                    hdr[32..36].copy_from_slice(&plan.raw_len.to_le_bytes());
                    hdr[36..40].copy_from_slice(&(payload.len() as u32).to_le_bytes());
                    hdr[40] = *method;
                    self.file.write_all(&hdr)?;
                    self.file.write_all(payload)?;
                    self.stored_pos += (CHUNK_HEADER_SIZE + payload.len()) as u64;
                    self.dedup.insert(&plan.hash, stored_off);
                    miss_stored[ordinal] = stored_off;
                    match *method {
                        METHOD_STORE => self.stored_chunks += 1,
                        METHOD_ZFAST => self.fast_chunks += 1,
                        _ => self.deep_chunks += 1,
                    }
                    stored_off
                }
            };
            self.logical.push(LogicalEntry {
                raw_off: plan.raw_off,
                stored_off,
                raw_len: plan.raw_len,
            });
        }
        self.maybe_progress();
        Ok(())
    }

    /// Завершённый tar-файл → sidecar-таблица.
    fn append_tar_file(&mut self, done: TarFileDone) {
        self.append_file_record(&done.name, done.raw_off, done.raw_len, done.sha256);
    }

    fn append_file_record(
        &mut self,
        name: &str,
        raw_off: u64,
        raw_len: u64,
        sha256: [u8; 32],
    ) {
        let mut b = Vec::with_capacity(FILE_ENTRY_FIXED + name.len());
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(name.as_bytes());
        b.extend_from_slice(&raw_off.to_le_bytes());
        b.extend_from_slice(&raw_len.to_le_bytes());
        b.extend_from_slice(&sha256);
        if self.files_spill.write_all(&b).is_ok() {
            self.files_count += 1;
        }
    }

    fn maybe_progress(&mut self) {
        if self.cfg.progress_bytes > 0
            && self.raw_pos - self.last_progress >= self.cfg.progress_bytes
        {
            self.last_progress = self.raw_pos;
            eprintln!(
                "stream: {} raw → {} stored, {} чанков (дедуп {})",
                fmt_bytes(self.raw_pos),
                fmt_bytes(self.stored_pos),
                self.logical.len(),
                self.dedup_chunks
            );
        }
    }
}

impl Drop for StreamWriter {
    fn drop(&mut self) {
        if !self.finished {
            if self.in_append {
                // Доклейка: архив остаётся усечённым (трейлер срезан) — это
                // ремонтируется по state-sidecar при следующем open_append.
                // Сам выходной файл НЕ трогаем. Spill пересоздаётся из
                // региона таблицы при ремонте.
                let _ = std::fs::remove_file(&self.files_spill_path);
            } else {
                let _ = std::fs::remove_file(&self.part_path);
                let _ = std::fs::remove_file(&self.files_spill_path);
            }
        }
    }
}

fn sidecar_path(out: &Path, suffix: &str) -> PathBuf {
    let mut s = out.as_os_str().to_os_string();
    s.push(format!(".{suffix}"));
    PathBuf::from(s)
}

// ────────────── v0.92.0 STREAM-CORPUS: доклейка/ремонт ──────────────

/// Геометрия валидного архива из 104-байт трейлера.
struct TrailerLayout {
    final_len: u64,
    index_off: u64,
    index_len: u64,
    files_off: u64,
    files_len: u64,
    raw_pos: u64,
    logical_count: u64,
    misses: u64,
    tar_mode: bool,
}

impl TrailerLayout {
    /// None = трейлер бит/отсутствует (магии нет, длины не сходятся).
    fn read(path: &Path, file_len: u64) -> Option<TrailerLayout> {
        if file_len < MAGIC_START.len() as u64 + TRAILER_SIZE as u64 {
            return None;
        }
        let mut f = File::open(path).ok()?;
        let mut start = [0u8; 8];
        f.read_exact_at(&mut start, 0).ok()?;
        if &start != MAGIC_START {
            return None;
        }
        let mut t = [0u8; TRAILER_SIZE];
        f.read_exact_at(&mut t, file_len - TRAILER_SIZE as u64).ok()?;
        if &t[0..8] != MAGIC_END {
            return None;
        }
        let final_len = u64::from_le_bytes(t[60..68].try_into().unwrap());
        if final_len != file_len {
            return None;
        }
        let flags = u16::from_le_bytes(t[10..12].try_into().unwrap());
        let index_off = u64::from_le_bytes(t[20..28].try_into().unwrap());
        let index_len = u64::from_le_bytes(t[28..36].try_into().unwrap());
        let files_off = u64::from_le_bytes(t[36..44].try_into().unwrap());
        let files_len = u64::from_le_bytes(t[44..52].try_into().unwrap());
        let raw_pos = u64::from_le_bytes(t[52..60].try_into().unwrap());
        // геометрия обязана быть непрерывной и вложенной
        if index_off < MAGIC_START.len() as u64
            || files_off < index_off + index_len
            || final_len < files_off + files_len + TRAILER_SIZE as u64
        {
            return None;
        }
        Some(TrailerLayout {
            final_len,
            index_off,
            index_len,
            files_off,
            files_len,
            raw_pos,
            logical_count: u32::from_le_bytes(t[12..16].try_into().unwrap()) as u64,
            misses: u32::from_le_bytes(t[16..20].try_into().unwrap()) as u64,
            tar_mode: flags & 2 != 0,
        })
    }
}

/// Записать state-sidecar `<out>.state`: непрерывный регион
/// [index_off, final_len) (индекс + файловая таблица + трейлер) валидного
/// архива. Пишется каждым finish() и каждым open_append ДО усечения.
fn write_state_sidecar(out: &Path, final_len: u64, index_off: u64) -> io::Result<()> {
    if final_len < index_off || index_off < MAGIC_START.len() as u64 {
        return Err(io::Error::other("state-sidecar: кривая геометрия"));
    }
    let blob_len = (final_len - index_off) as usize;
    let mut blob = vec![0u8; blob_len];
    {
        let mut rf = File::open(out)?;
        rf.read_exact_at(&mut blob, index_off)?;
    }
    let mut sc = File::create(sidecar_path(out, "state"))?;
    sc.write_all(STATE_MAGIC)?;
    sc.write_all(&final_len.to_le_bytes())?;
    sc.write_all(&index_off.to_le_bytes())?;
    sc.write_all(&(blob_len as u64).to_le_bytes())?;
    sc.write_all(&blob)?;
    sc.sync_all()?;
    Ok(())
}

/// Ремонт архива по state-sidecar: восстановить последний чекпоинт.
/// Порядок write-then-set_len покрывает оба состояния падения: файл
/// КОРОЧЕ (ранняя доклейка — blob дозапишет хвост) и файл ДЛИННЕЕ
/// (частичные новые чанки поверх региона — перезапишутся и усекутся).
fn repair_from_sidecar(out: &Path) -> io::Result<()> {
    let path = sidecar_path(out, "state");
    let mut sc = File::open(&path).map_err(|e| {
        io::Error::other(format!(
            "ремонт невозможен: нет {} ({e})",
            path.display()
        ))
    })?;
    let mut head = [0u8; 32];
    sc.read_exact(&mut head)?;
    if &head[0..8] != STATE_MAGIC {
        return Err(io::Error::other("ремонт: state-sidecar не POLERST1"));
    }
    let final_len = u64::from_le_bytes(head[8..16].try_into().unwrap());
    let index_off = u64::from_le_bytes(head[16..24].try_into().unwrap());
    let blob_len = u64::from_le_bytes(head[24..32].try_into().unwrap()) as usize;
    if final_len < index_off
        || index_off < MAGIC_START.len() as u64
        || index_off + blob_len as u64 != final_len
    {
        return Err(io::Error::other("ремонт: кривая геометрия sidecar"));
    }
    let mut blob = vec![0u8; blob_len];
    sc.read_exact(&mut blob)?;
    let mut file = OpenOptions::new().write(true).open(out)?;
    file.seek(SeekFrom::Start(index_off))?;
    file.write_all(&blob)?;
    file.set_len(final_len)?;
    file.sync_all()?;
    Ok(())
}

/// Декодировать физический чанк в сырые байты (восстановление
/// сквозного SHA256 при open_append; паттерн читателя).
fn decode_chunk_payload(file: &File, stored_off: u64, raw_len: u32) -> io::Result<Vec<u8>> {
    let mut hdr = [0u8; CHUNK_HEADER_SIZE];
    file.read_exact_at(&mut hdr, stored_off)?;
    let stored_len = u32::from_le_bytes(hdr[36..40].try_into().unwrap()) as usize;
    let method = hdr[40];
    let mut payload = vec![0u8; stored_len];
    file.read_exact_at(&mut payload, stored_off + CHUNK_HEADER_SIZE as u64)?;
    match method {
        METHOD_STORE => Ok(payload),
        METHOD_ZFAST | METHOD_ZDEEP => zstd::bulk::decompress(&payload, raw_len as usize)
            .map_err(|e| io::Error::other(format!("open_append: zstd: {e}"))),
        _ => Err(io::Error::other("open_append: неизвестный метод чанка")),
    }
}

fn sanitize_hint(hint: &str) -> String {
    let base = hint.rsplit('/').next().unwrap_or("stream");
    let base = base.trim();
    if base.is_empty() {
        "stream".to_string()
    } else {
        base.chars().take(180).collect()
    }
}

fn zstd_compressor(level: i32) -> io::Result<zstd::bulk::Compressor<'static>> {
    zstd::bulk::Compressor::new(level)
        .map_err(|e| io::Error::other(format!("zstd: {e}")))
}

pub fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{b} B")
    } else {
        format!("{v:.2} {}", UNITS[u])
    }
}

/// Целостный поток: источник → .poler одной командой.
pub fn write_stream<R: Read>(
    src: R,
    out: &Path,
    cfg: StreamWriteConfig,
    name_hint: &str,
) -> io::Result<StreamWriteStats> {
    let mut w = StreamWriter::open(out, cfg)?;
    let mut reader = src;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        w.push_bytes(&buf[..n])?;
    }
    w.finish(name_hint)
}

// ─────────────────────── Синтетика для бенчей ─────────────────────────

/// Детерминированный синтетический поток для бенчей и тестов RAM:
/// текстоподобные блоки (сжимаются, фразы повторяются → дедуп CDC)
/// и каждый 8-й блок псевдослучайный (несжимаем → ярус STORE).
pub struct SyntheticStream {
    total: u64,
    produced: u64,
    state: u64,
    block: Vec<u8>,
    block_pos: usize,
    block_no: u64,
}

impl SyntheticStream {
    pub fn new(total: u64, seed: u64) -> Self {
        SyntheticStream {
            total,
            produced: 0,
            state: seed | 1,
            block: Vec::new(),
            block_pos: 0,
            block_no: 0,
        }
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn refill(&mut self) {
        self.block.clear();
        self.block_pos = 0;
        const PHRASES: [&str; 12] = [
            "the kernel scheduler keeps fairness across run queues",
            "мозг мухи держит ритм мысли в живом вихре",
            "memory ordering guarantees require acquire-release semantics",
            "квантовая решётка тритов живёт без умножения",
            "zero-copy parsing avoids intermediate buffer allocations",
            "энтропия фильтра отделяет смысл от шума страницы",
            "the piece tree balances insertions in logarithmic time",
            "суверенный архиватор льёт поток прямо в кристалл",
            "content defined chunking resists insertion shifts",
            "резонанс смысла растёт при совпадении контекста",
            "mmap backed readers achieve constant time open latency",
            "дедупликация blake3 находит повторяющиеся блоки кода",
        ];
        if self.block_no % 8 == 7 {
            // несжимаемый блок
            self.block.reserve(64 * 1024);
            for _ in 0..(64 * 1024 / 8) {
                let w = self.next_u64();
                self.block.extend_from_slice(&w.to_le_bytes());
            }
        } else {
            let mut seed = self.next_u64();
            self.block.reserve(64 * 1024);
            while self.block.len() < 64 * 1024 {
                let p = PHRASES[(seed as usize) % PHRASES.len()];
                self.block.extend_from_slice(p.as_bytes());
                self.block.push(b'\n');
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
            }
        }
        self.block_no += 1;
    }
}

impl Read for SyntheticStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.produced >= self.total {
            return Ok(0);
        }
        let want = out.len().min((self.total - self.produced) as usize);
        let mut filled = 0;
        while filled < want {
            if self.block_pos >= self.block.len() {
                self.refill();
            }
            let take = (self.block.len() - self.block_pos).min(want - filled);
            out[filled..filled + take]
                .copy_from_slice(&self.block[self.block_pos..self.block_pos + take]);
            self.block_pos += take;
            filled += take;
        }
        self.produced += filled as u64;
        Ok(filled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("poler-sw-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn roundtrip_small_stream() {
        let path = tmp("small.poler");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
        let stats =
            write_stream(&data[..], &path, StreamWriteConfig::default(), "probe.bin").unwrap();
        assert_eq!(stats.total_raw, 300_000);
        assert_eq!(stats.files, 1);
        assert!(!stats.tar_mode);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn incremental_push_equals_write_stream() {
        // Один и тот же поток через write_stream и push_bytes нечётными
        // порциями: байты .poler обязаны совпасть (детерминизм границ).
        let mut data = Vec::new();
        let mut st = 12345u64;
        for _ in 0..(2 * 1024 * 1024 / 8) {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1);
            data.extend_from_slice(&st.to_le_bytes());
        }
        let p1 = tmp("inc1.poler");
        let p2 = tmp("inc2.poler");
        write_stream(&data[..], &p1, StreamWriteConfig::default(), "x.bin").unwrap();
        let mut w = StreamWriter::open(&p2, StreamWriteConfig::default()).unwrap();
        let mut off = 0;
        for step in [1usize, 3, 7, 61, 4096, 65537, 13] {
            let end = (off + step).min(data.len());
            w.push_bytes(&data[off..end]).unwrap();
            off = end;
            if off >= data.len() {
                break;
            }
        }
        while off < data.len() {
            let end = (off + 100_000).min(data.len());
            w.push_bytes(&data[off..end]).unwrap();
            off = end;
        }
        w.finish("x.bin").unwrap();
        let b1 = std::fs::read(&p1).unwrap();
        let b2 = std::fs::read(&p2).unwrap();
        assert_eq!(b1, b2, "push_bytes порциями == write_stream");
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[test]
    fn dedup_drops_repeated_blocks() {
        // 12 ДОСЛОВНЫХ блоков по 256 КиБ подряд (3 МиБ периодического
        // содержимого) + уникальный хвост: CDC-сеть кандидатов
        // сходится к периодической решётке → внутренние чанки повторяются.
        let mut block: Vec<u8> = Vec::with_capacity(256 * 1024);
        let mut st = 777u64;
        while block.len() < 256 * 1024 {
            st = st.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
            for b in (st as u32).to_le_bytes() {
                block.push(b);
                if block.len() == 256 * 1024 {
                    break;
                }
            }
        }
        let mut stream = Vec::new();
        for _ in 0..12 {
            stream.extend_from_slice(&block);
        }
        // уникальный хвост (несжимаемый, чтобы заодно проверить STORE)
        let mut tail = Vec::with_capacity(512 * 1024);
        let mut t = 4242u64;
        while tail.len() < 512 * 1024 {
            t = t.wrapping_mul(6364136223846793005).wrapping_add(1);
            tail.extend_from_slice(&t.to_le_bytes());
        }
        stream.extend_from_slice(&tail);
        let p = tmp("dedup.poler");
        let cfg = StreamWriteConfig { progress_bytes: 0, ..Default::default() };
        let stats = write_stream(&stream[..], &p, cfg, "rep.bin").unwrap();
        assert!(
            stats.dedup_chunks >= 3,
            "дедуп не сработал: {} (логических {})",
            stats.dedup_chunks,
            stats.logical_chunks
        );
        assert!(stats.total_stored < stream.len() as u64 / 2, "архив не ужался");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn synthetic_stream_is_deterministic() {
        let mut a = SyntheticStream::new(1_000_000, 5);
        let mut b = SyntheticStream::new(1_000_000, 5);
        let mut ra = Vec::new();
        let mut rb = Vec::new();
        a.read_to_end(&mut ra).unwrap();
        b.read_to_end(&mut rb).unwrap();
        assert_eq!(ra, rb);
        assert_eq!(ra.len(), 1_000_000);
    }

    #[test]
    fn peak_rss_reports_something() {
        let v = peak_rss_kb();
        assert!(v == 0 || v > 1000, "VmHWM = {v} КБ выглядит неразумно");
    }

    /// v0.92.0 STREAM-CORPUS: open_append восстанавливает сквозной SHA256
    /// потока (декомпрессией логических чанков по порядку) — verify()
    /// после доклейки обязан сойтись, иначе резюм молча ломал бы
    /// верификацию ВСЕГО архива. Заодно: таблица файлов накопляется,
    /// координаты записей продолжаются, а не начинаются с нуля.
    #[test]
    fn open_append_extends_stream_digest() {
        use crate::archive::reader::PolerReader;

        fn tar_frag(name: &str, data: &[u8]) -> Vec<u8> {
            let mut b = tar::Builder::new(Vec::new());
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, name, data).unwrap();
            b.finish().unwrap();
            b.into_inner().unwrap()
        }
        fn pseudo(len: usize, seed: u64) -> Vec<u8> {
            let mut st = seed | 1;
            let mut v = Vec::with_capacity(len);
            while v.len() < len {
                st = st.wrapping_mul(6364136223846793005).wrapping_add(1);
                v.extend_from_slice(&st.to_le_bytes());
            }
            v.truncate(len);
            v
        }

        let p = tmp("append-digest.poler");
        let cfg = StreamWriteConfig { progress_bytes: 0, ..Default::default() };
        {
            let mut w = StreamWriter::open(&p, cfg.clone()).unwrap();
            w.push_bytes(&tar_frag("one.bin", &pseudo(400_000, 11))).unwrap();
            w.push_bytes(&tar_frag("two.bin", &pseudo(300_000, 22))).unwrap();
            w.finish("part1").unwrap();
        }
        {
            let mut w = StreamWriter::open_append(&p, cfg).unwrap();
            w.push_bytes(&tar_frag("three.bin", &pseudo(350_000, 33))).unwrap();
            w.finish("part2").unwrap();
        }
        let reader = PolerReader::open(&p).unwrap();
        let names: Vec<&str> = reader.files().iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["one.bin", "two.bin", "three.bin"]);
        let rep = reader.verify().unwrap();
        assert!(rep.stream_ok, "сквозной дайджест после доклейки: {:?}", rep);
        assert!(rep.all_ok);
        // sidecar на месте и валиден
        assert!(std::fs::metadata(p.with_extension("poler.state")).is_ok());
        let _ = std::fs::remove_file(&p);
    }

    /// v0.91.0 CORPUS-TRIT: батч-путь (jobs=4) обязан давать БАЙТ-В-БАЙТ
    /// тот же .poler, что последовательный (jobs=0): границы CDC зависят
    /// только от содержимого, zstd-кадры чанка не зависят от потока,
    /// запись идёт в порядке потока. Плюс deep_level=19 не хуже 15.
    #[test]
    fn parallel_jobs_byte_identical() {
        // текстоподобные ~4 МиБ (повторяющиеся фразы) + дословные
        // дубли-блоки (дедуп-хиты) + несжимаемый хвост (STORE)
        let mut data = Vec::with_capacity(5 * 1024 * 1024);
        let phrases: [&str; 8] = [
            "синапс трёх состояний обходит умножение целиком",
            "content defined chunking resists boundary shifts",
            "квантовая решётка тритов живёт без умножения",
            "the corpus packs itself into a single sovereign file",
            "энтропия фильтра отделяет смысл от шума страницы",
            "mmap backed readers open archives in constant time",
            "троичный кристалл помнит весь корпус без диска",
            "zero disk ingestion streams gigabytes into one poler",
        ];
        let mut st = 0xABCD_1234u64;
        let mut next = || {
            st ^= st << 13;
            st ^= st >> 7;
            st ^= st << 17;
            st
        };
        while data.len() < 4 * 1024 * 1024 {
            let p = phrases[(next() as usize) % phrases.len()];
            data.extend_from_slice(p.as_bytes());
            data.push(b'\n');
        }
        let block: Vec<u8> = data[..256 * 1024].to_vec();
        for _ in 0..6 {
            data.extend_from_slice(&block); // дубли → дедуп-хиты
        }
        for _ in 0..(512 * 1024 / 8) {
            data.extend_from_slice(&next().to_le_bytes()); // несжимаемо
        }

        let base = StreamWriteConfig {
            cdc: CdcParams::new(4096, 16 * 1024, 64 * 1024),
            tier: CompressTier::Deep,
            deep_level: 15,
            jobs: 0,
            progress_bytes: 0,
            ..Default::default()
        };
        let p1 = tmp("par_seq.poler");
        let p2 = tmp("par_jobs.poler");
        let p3 = tmp("par_deep19.poler");
        let s1 = write_stream(&data[..], &p1, base.clone(), "par.bin").unwrap();
        let s2 = write_stream(
            &data[..],
            &p2,
            StreamWriteConfig { jobs: 4, ..base.clone() },
            "par.bin",
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&p1).unwrap(),
            std::fs::read(&p2).unwrap(),
            "jobs=4 обязан быть байтово идентичен jobs=0"
        );
        assert_eq!(s1.logical_chunks, s2.logical_chunks);
        assert_eq!(s1.dedup_chunks, s2.dedup_chunks);
        assert!(s2.dedup_chunks >= 3, "дубли схлопнуты: {}", s2.dedup_chunks);

        // глубокий уровень 19 плотнее уровня 15 на том же потоке
        let s3 = write_stream(
            &data[..],
            &p3,
            StreamWriteConfig { jobs: 2, deep_level: 19, ..base },
            "par.bin",
        )
        .unwrap();
        assert!(
            s3.total_stored <= s1.total_stored,
            "zstd-19 ({}) не хуже zstd-15 ({})",
            s3.total_stored,
            s1.total_stored
        );
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
        let _ = std::fs::remove_file(&p3);
    }
}


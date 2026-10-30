//! Инспектор бинарных состояний: полная «логика данных» произвольного файла.
//!
//! Замыкает потребность «прочитать бинарник целиком»: один проход по байтам
//! отвечает на три вопроса — **что это** (детект формата), **можно ли это
//! читать** (крипто-разведка: entropy/χ²/детект контейнеров) и **что внутри**
//! (побайтовая карта заголовка `.pqw`, hex-дамп, декод дуг, LENS-граф).
//!
//! ## Детекция формата
//!
//! ```text
//! POLER_QW → контейнер v1 (кривизна, 1 байт/дуга)
//! POLER_Q2 → контейнер v2 (Packed4, 4 трита/байт)
//! без magic, все 2-битные пары ≠ 0b11, непечатаемый фон → RawPacked4
//! остальное → Opaque (универсальная разведка)
//! ```
//!
//! RawPacked4 — кандидат: любой файл без запрещённых пар формально декодируется
//! как триты (контракт v2 не нарушается), поэтому детектор дополнительно
//! отсекает печатаемый ASCII-текст и помечает результат как `candidate`.
//!
//! ## Крипто-разведка
//!
//! Шенноновская энтропия + χ² против равномерного распределения + словарь
//! магических сигнатур шифро-контейнеров (OpenSSL `Salted__`, GPG, ZIP,
//! GZIP, 7z, xz, age, PEM-броня). Вердикт: `structured` (читаемо, шифра нет),
//! `compressed` или `encrypted-like` (похоже на шифр/случайность — структуры
//! не видно, нужен ключ/алгоритм от владельца).
//!
//! ## Граф LENS
//!
//! Рёбра — соседние хранимые дуги `(u_k → u_{k+1})`, та же семантика, что в
//! [`crate::entangle::Entanglement::FromTopology`]. Вывод: CSR-текст,
//! Graphviz DOT, JSON и ASCII-матрица смежности (при `d_pol ≤ 64`).
//!
//! ## Пример: сырой Packed4-отпечаток
//!
//! ```
//! use pqc::inspect::{detect_kind, raw_packed4_arcs, FileKind};
//!
//! // Одна дуга −1 по индексу 0: пара 0b10 в младших битах.
//! let data = [0b0000_0010u8];
//! assert!(matches!(detect_kind(&data), FileKind::RawPacked4 { .. }));
//! assert_eq!(raw_packed4_arcs(&data), vec![(0, -1)]);
//! ```

use crate::coherence::binary_entropy;
use crate::json::Json;
use pqw::mcweeny::idempotency_residual;
use pqw::phase::{unpack_quad, Trit};
use pqw::sha256::sha256;
use pqw::{PqwReader, HEADER_SIZE, MAGIC, MAGIC_V2, MAGIC_V3, MAGIC_V4, MAGIC_V5};

/// Порог χ² против равномерного для df = 255 (p ≈ 0.001): ~352.
const CHI2_UNIFORM_BAND: f64 = 352.0;
/// Порог «похоже на шифр/случайность» по энтропии (бит/байт).
const ENTROPY_ENCRYPTED: f64 = 7.5;
/// Порог «похоже на сжатые данные» по энтропии.
const ENTROPY_COMPRESSED: f64 = 6.0;
/// Максимум ASCII-строк в отчёте.
const STRINGS_CAP: usize = 64;
/// Максимум строк в CSR-дампе дуг (полный дамп — через `--decode all`).
pub const ARCS_PREVIEW: usize = 64;
/// Максимум узлов для ASCII-матрицы смежности.
pub const MATRIX_D_MAX: u32 = 64;

/// Чем является файл.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// Контейнер v1: magic `POLER_QW`, кривизна σ на дугу.
    PqwV1,
    /// Контейнер v2: magic `POLER_Q2`, Packed4-триты.
    PqwV2,
    /// Контейнер v3: magic `POLER_Q3`, Packed4 + гироскоп `J = A − Aᵀ`.
    PqwV3,
    /// Контейнер v4: magic `POLER_Q4`, v3 + лексикон `LEXI`
    /// (координата → доминантный токен, RQ17).
    PqwV4,
    /// Контейнер v5: magic `POLER_Q5`, v4 + контекст-рефлекс `REFL`
    /// (динамический след диалога, RQ23).
    PqwV5,
    /// Кандидат в сырые Packed4-пакеты (без заголовка); `d_pol = 4 × len`.
    RawPacked4 { d_pol: u32 },
    /// Неизвестный формат — универсальная разведка.
    Opaque,
}

impl FileKind {
    /// Человекочитаемое имя.
    pub fn name(self) -> &'static str {
        match self {
            FileKind::PqwV1 => "pqw-v1 (POLER_QW, curved)",
            FileKind::PqwV2 => "pqw-v2 (POLER_Q2, Packed4)",
            FileKind::PqwV3 => "pqw-v3 (POLER_Q3, Packed4 + gyro J = A − Aᵀ)",
            FileKind::PqwV4 => "pqw-v4 (POLER_Q4, Packed4 + gyro J + lexicon LEXI)",
            FileKind::PqwV5 => {
                "pqw-v5 (POLER_Q5, Packed4 + gyro J + lexicon + reflex REFL)"
            }
            FileKind::RawPacked4 { .. } => "raw-packed4 (candidate)",
            FileKind::Opaque => "opaque",
        }
    }

    /// Это один из контейнеров POLER?
    pub fn is_pqw(self) -> bool {
        matches!(
            self,
            FileKind::PqwV1
                | FileKind::PqwV2
                | FileKind::PqwV3
                | FileKind::PqwV4
                | FileKind::PqwV5
        )
    }
}

/// Детекция формата по магическим байтам и структуре пар Packed4.
pub fn detect_kind(data: &[u8]) -> FileKind {
    if data.len() >= 8 {
        if data[..8] == MAGIC {
            return FileKind::PqwV1;
        }
        if data[..8] == MAGIC_V2 {
            return FileKind::PqwV2;
        }
        if data[..8] == MAGIC_V3 {
            return FileKind::PqwV3;
        }
        if data[..8] == MAGIC_V4 {
            return FileKind::PqwV4;
        }
        if data[..8] == MAGIC_V5 {
            return FileKind::PqwV5;
        }
    }
    if !data.is_empty() && packed4_valid(data) && printable_ratio(data) < 0.85 {
        return FileKind::RawPacked4 {
            d_pol: (data.len() * 4) as u32,
        };
    }
    FileKind::Opaque
}

/// Все 2-битные пары файла ≠ `0b11` (необходимое условие Packed4).
pub fn packed4_valid(data: &[u8]) -> bool {
    data.iter().all(|&b| {
        (b & 0b11 != 0b11)
            && ((b >> 2) & 0b11 != 0b11)
            && ((b >> 4) & 0b11 != 0b11)
            && ((b >> 6) & 0b11 != 0b11)
    })
}

/// Доля печатаемых ASCII-байтов (0x20..=0x7E, таб, CR/LF).
pub fn printable_ratio(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let printable = data
        .iter()
        .filter(|&&b| (0x20..=0x7E).contains(&b) || b == b'\t' || b == b'\n' || b == b'\r')
        .count();
    printable as f64 / data.len() as f64
}

/// Декод сырого Packed4-пакета: ненулевые дуги `(index, sign)`.
///
/// Запрещённая пара `0b11` игнорируется как фон (кандидат мог ей оказаться
/// лишь при коллизии детектора — читатель v2 отверг бы её жёстко).
pub fn raw_packed4_arcs(data: &[u8]) -> Vec<(u32, i8)> {
    let mut arcs = Vec::new();
    for (bi, &b) in data.iter().enumerate() {
        let Ok(ts) = unpack_quad(b) else {
            continue;
        };
        for (j, t) in ts.iter().enumerate() {
            let sign = match t {
                Trit::Pos => 1,
                Trit::Neg => -1,
                Trit::Zero => continue,
            };
            arcs.push(((bi * 4 + j) as u32, sign));
        }
    }
    arcs
}

// ─────────────────────────────────────────────────────────────────────────────
// Крипто-разведка
// ─────────────────────────────────────────────────────────────────────────────

/// Итог крипто-разведки файла.
#[derive(Clone, Debug)]
pub struct CryptoRecon {
    /// Шенноновская энтропия, бит/байт (0..=8).
    pub entropy: f64,
    /// χ² против равномерного распределения по 256 значениям (df = 255).
    pub chi2: f64,
    /// Число различных значений байтов (0..=256).
    pub distinct: usize,
    /// Пер-блочная энтропия (первые `blocks_max` блоков по 64 Б).
    pub blocks: Vec<f64>,
    /// Совпавшие сигнатуры известных контейнеров.
    pub container_hits: Vec<&'static str>,
    /// Вердикт одним словом.
    pub verdict: &'static str,
}

impl CryptoRecon {
    /// Похоже на шифротекст/случайность (структуры не видно)?
    pub fn encrypted_like(&self) -> bool {
        self.verdict == "encrypted-like"
    }

    /// JSON-вид (для CLI `--json`).
    pub fn to_json(&self) -> Json {
        Json::Obj(vec![
            ("entropy".into(), Json::num(self.entropy)),
            ("chi2_uniform".into(), Json::num(self.chi2)),
            ("distinct_bytes".into(), Json::Num(self.distinct as f64)),
            (
                "block_entropies".into(),
                Json::Arr(
                    self.blocks
                        .iter()
                        .map(|&h| Json::num(h))
                        .collect::<Vec<_>>(),
                ),
            ),
            (
                "container_hits".into(),
                Json::Arr(
                    self.container_hits
                        .iter()
                        .map(|s| Json::str(*s))
                        .collect::<Vec<_>>(),
                ),
            ),
            ("verdict".into(), Json::str(self.verdict)),
        ])
    }
}

/// Шенноновская энтропия данных, бит/байт.
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let n = data.len() as f64;
    let mut h = 0.0;
    for c in freq {
        if c > 0 {
            let p = c as f64 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// χ² статистика против равномерного распределения (df = 255).
pub fn chi2_uniform(data: &[u8]) -> f64 {
    if data.len() < 256 {
        return f64::NAN;
    }
    let mut freq = [0u64; 256];
    for &b in data {
        freq[b as usize] += 1;
    }
    let exp = data.len() as f64 / 256.0;
    freq.iter()
        .map(|&c| {
            let d = c as f64 - exp;
            d * d / exp
        })
        .sum()
}

/// Сигнатуры известных шифро-/архиво-контейнеров.
pub fn detect_containers(data: &[u8]) -> Vec<&'static str> {
    const SIGS: [(&[u8], &str); 17] = [
        (b"Salted__", "openssl-enc (Salted__, AES/ChaCha via enc)"),
        (b"\x8c\x0d\x04\x07", "gpg-symmetric (OpenPGP)"),
        (b"-----BEGIN", "pem-armor (PEM/OpenSSH/GPG armored)"),
        (b"age-encryption.org", "age"),
        (b"PK\x03\x04", "zip (возможно, encrypted entries)"),
        (b"\x1f\x8b", "gzip"),
        (b"BZh", "bzip2"),
        (b"\xfd7zXZ\x00", "xz"),
        (b"7z\xbc\xaf\x27\x1c", "7z"),
        (b"\x04\x22\x4d\x18", "lz4"),
        (b"\x28\xb5\x2f\xfd", "zstd"),
        (b"\x89PNG\r\n\x1a\n", "png"),
        (b"\xff\xd8\xff", "jpeg"),
        (b"GIF8", "gif"),
        (b"%PDF", "pdf"),
        (b"\x7fELF", "elf"),
        (b"SQLite format 3\x00", "sqlite"),
    ];
    SIGS.iter()
        .filter(|(sig, _)| data.len() >= sig.len() && &data[..sig.len()] == *sig)
        .map(|(_, name)| *name)
        .collect()
}

/// Полная крипто-разведка: энтропия, χ², контейнеры, вердикт.
pub fn crypto_recon(data: &[u8]) -> CryptoRecon {
    let blocks = block_entropies(data, 64, 16);
    let hits = detect_containers(data);
    let entropy = shannon_entropy(data);
    let chi2 = chi2_uniform(data);
    let mut seen = [false; 256];
    for &b in data {
        seen[b as usize] = true;
    }
    let distinct = seen.iter().filter(|&&x| x).count();

    let uniformish = chi2.is_finite() && (chi2 - 255.0).abs() <= CHI2_UNIFORM_BAND;
    let verdict = if entropy >= ENTROPY_ENCRYPTED && distinct >= 200 && uniformish {
        "encrypted-like"
    } else if entropy >= ENTROPY_COMPRESSED {
        "compressed"
    } else {
        "structured"
    };

    CryptoRecon {
        entropy,
        chi2,
        distinct,
        blocks,
        container_hits: hits,
        verdict,
    }
}

/// Пер-блочная энтропия: первые `max_blocks` блоков по `block` байтов.
pub fn block_entropies(data: &[u8], block: usize, max_blocks: usize) -> Vec<f64> {
    data.chunks(block)
        .take(max_blocks)
        .map(shannon_entropy)
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Hex-дамп и строки
// ─────────────────────────────────────────────────────────────────────────────

/// Классический hex-дамп: 16 байт/строка, смещение, ASCII-колонка.
///
/// `limit = 0` — пустой вывод; вывод не длиннее `limit` байтов.
pub fn hex_dump(data: &[u8], limit: usize) -> String {
    let end = data.len().min(limit);
    let mut out = String::new();
    for row in (0..end).step_by(16) {
        let hi = (row + 16).min(end);
        let slice = &data[row..hi];
        let mut hexs = String::with_capacity(49);
        for (i, b) in slice.iter().enumerate() {
            if i == 8 {
                hexs.push(' ');
            }
            hexs.push_str(&format!("{b:02x} "));
        }
        for _ in slice.len()..16 {
            if slice.len() == 8 {
                hexs.push(' ');
            }
            hexs.push_str("   ");
        }
        let asc: String = slice
            .iter()
            .map(|&b| {
                if (0x20..=0x7E).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        out.push_str(&format!("{row:08x}  {hexs} |{asc}|\n"));
    }
    out
}

/// Печатаемые ASCII-подстроки длиной ≥ `min_len` (максимум [`STRINGS_CAP`]).
pub fn ascii_strings(data: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for &b in data {
        if (0x20..=0x7E).contains(&b) {
            cur.push(b as char);
        } else {
            if cur.len() >= min_len && out.len() < STRINGS_CAP {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
    }
    if cur.len() >= min_len && out.len() < STRINGS_CAP {
        out.push(cur);
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Заголовок .pqw: побайтовая карта
// ─────────────────────────────────────────────────────────────────────────────

/// Одна строка побайтовой карты: смещение, размер, поле, значение, смысл.
#[derive(Clone, Debug)]
pub struct FieldRow {
    /// Смещение от начала файла.
    pub offset: usize,
    /// Размер поля в байтах.
    pub size: usize,
    /// Имя поля.
    pub name: &'static str,
    /// Значение (человекочитаемое).
    pub value: String,
    /// Комментарий-смысл.
    pub note: &'static str,
}

/// Побайтовая карта заголовка контейнера (0x00..0x80).
///
/// `data` — исходные байты файла (checksum берётся прямо из них).
/// Ошибка невозможна после успешного `PqwReader::from_bytes`.
pub fn header_rows(data: &[u8], reader: &PqwReader) -> Vec<FieldRow> {
    let h = reader.header();
    let digest_hex: String = h
        .payload_digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    vec![
        FieldRow {
            offset: 0x00,
            size: 8,
            name: "magic",
            value: match h.format_version {
                2 => "POLER_Q2".into(),
                3 => "POLER_Q3".into(),
                _ => "POLER_QW".into(),
            },
            note: "сигнатура контейнера",
        },
        FieldRow {
            offset: 0x08,
            size: 4,
            name: "format_version",
            value: format!("{}", h.format_version),
            note: "1 = curved, 2 = Packed4, 3 = Packed4 + гироскоп",
        },
        FieldRow {
            offset: 0x0C,
            size: 4,
            name: "d_pol",
            value: format!("{}", h.d_pol),
            note: "размерность фазового пространства",
        },
        FieldRow {
            offset: 0x10,
            size: 4,
            name: "hyper.eta",
            value: format!("{}", h.hyper.eta),
            note: "шаг фазового потока η",
        },
        FieldRow {
            offset: 0x14,
            size: 4,
            name: "hyper.gamma",
            value: format!("{}", h.hyper.gamma),
            note: "трение γ",
        },
        FieldRow {
            offset: 0x18,
            size: 4,
            name: "hyper.rho",
            value: format!("{}", h.hyper.rho),
            note: "затухание IIR-резонатора ρ",
        },
        FieldRow {
            offset: 0x1C,
            size: 4,
            name: "hyper.epsilon_threshold",
            value: format!("{}", h.hyper.epsilon_threshold),
            note: "порог LENS ε-значимости",
        },
        FieldRow {
            offset: 0x20,
            size: 8,
            name: "mcweeny_residual",
            value: format!("{:.3e}", h.mcweeny_residual),
            note: "max |λ² − λ| на момент записи",
        },
        FieldRow {
            offset: 0x28,
            size: 24,
            name: "payload_digest",
            value: digest_hex,
            note: "SHA-256(топология ‖ фазы), усечён до 24 Б",
        },
        FieldRow {
            offset: 0x40,
            size: 8,
            name: "topology_offset",
            value: format!("0x{:x}", h.topology_offset),
            note: if h.is_gyro() {
                "начало гироскопной секции J = A − Aᵀ"
            } else {
                "начало CSR-топологии"
            },
        },
        FieldRow {
            offset: 0x48,
            size: 8,
            name: "topology_len",
            value: format!("{}", h.topology_len),
            note: if h.is_gyro() {
                "байтов гироскопной секции"
            } else {
                "байтов топологии (v2: 0)"
            },
        },
        FieldRow {
            offset: 0x50,
            size: 8,
            name: "phase_offset",
            value: format!("0x{:x}", h.phase_offset),
            note: "начало фазовых блоков",
        },
        FieldRow {
            offset: 0x58,
            size: 8,
            name: "phase_len",
            value: format!("{}", h.phase_len),
            note: "байтов фаз (v1: nnz, v2: ceil(d/4))",
        },
        FieldRow {
            offset: 0x60,
            size: 8,
            name: "nnz",
            value: format!("{}", h.nnz),
            note: "число хранимых дуг",
        },
        FieldRow {
            offset: 0x68,
            size: 8,
            name: "flags",
            value: format!("0x{:x}", h.flags.bits()),
            note: if h.is_gyro() {
                "бит0 INDEX16 пар, бит2 GYRO"
            } else if h.is_packed() {
                "v2: обязан быть 0"
            } else {
                "бит0 INDEX16, бит1 CURVATURE"
            },
        },
        FieldRow {
            offset: 0x70,
            size: 8,
            name: "reserved",
            value: if h.is_gyro() {
                // v3: счётчик тактов гироскопа (зеркалит секцию).
                format!("{}", {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&data[0x70..0x78]);
                    u64::from_le_bytes(b)
                })
            } else {
                "0".into()
            },
            note: if h.is_gyro() {
                "v3: счётчик тактов t гироскопа"
            } else {
                "обязан быть нулём"
            },
        },
        FieldRow {
            offset: 0x78,
            size: 8,
            name: "header_checksum",
            value: {
                let mut hex = String::with_capacity(16);
                for b in &data[0x78..0x80] {
                    hex.push_str(&format!("{b:02x}"));
                }
                hex
            },
            note: "FNV-1a64 по 0x00..0x78 (валидирована ридером)",
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Декод дуг и граф LENS
// ─────────────────────────────────────────────────────────────────────────────

/// Декодированная дуга: индекс, трит, p̂, θ̂.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodedArc {
    /// Индекс дуги в `[0, d_pol)`.
    pub index: u32,
    /// Трит: −1 / +1 (v2), либо знак σ-кривизны (v1).
    pub trit: i8,
    /// Деквантованное значение `p̂ ∈ [−1, 1]`.
    pub p_hat: f64,
    /// Фазовый угол `θ̂ = arccos(p̂)`.
    pub theta: f64,
}

/// Дуги контейнера `.pqw` (v1 и v2) в едином виде.
pub fn reader_arcs(reader: &PqwReader) -> Vec<DecodedArc> {
    reader
        .arcs()
        .map(|arc| DecodedArc {
            index: arc.index,
            trit: match arc.phase.trit() {
                Trit::Pos => 1,
                Trit::Neg => -1,
                Trit::Zero => 0,
            },
            p_hat: arc.p(),
            theta: arc.theta(),
        })
        .collect()
}

/// Дуги сырого Packed4-пакета (p̂ = ±1, θ̂ = 0/π).
pub fn raw_arcs(data: &[u8]) -> Vec<DecodedArc> {
    raw_packed4_arcs(data)
        .into_iter()
        .map(|(index, trit)| DecodedArc {
            index,
            trit,
            p_hat: f64::from(trit),
            theta: if trit > 0 { 0.0 } else { std::f64::consts::PI },
        })
        .collect()
}

/// CSR-текст дуг: построчный дамп `idx → trit, p̂, θ̂`.
pub fn arcs_csr_text(arcs: &[DecodedArc]) -> String {
    let mut out = String::new();
    for a in arcs {
        out.push_str(&format!(
            "  {:>6}  0x{:04x}  {:+}   p̂ = {:+.4}  θ̂ = {:.4}\n",
            a.index, a.index, a.trit, a.p_hat, a.theta
        ));
    }
    out
}

/// Born-энтропия дуг: `Σ h₂((1 − p̂)/2)` (бит).
pub fn born_entropy(arcs: &[DecodedArc]) -> f64 {
    arcs.iter()
        .map(|a| binary_entropy((1.0 - a.p_hat) / 2.0))
        .sum()
}

/// Теоретический QCM: `1 − H_born / d_pol` (см. RQ6).
pub fn qcm_theory(arcs: &[DecodedArc], d_pol: u32) -> f64 {
    if d_pol == 0 {
        return f64::NAN;
    }
    1.0 - born_entropy(arcs) / f64::from(d_pol)
}

/// Статистика LENS-графа: рёбра = соседние дуги `(u_k → u_{k+1})`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphStats {
    /// Узлы — хранимые дуги.
    pub nodes: usize,
    /// Рёбра-цепочки соседних дуг.
    pub edges: usize,
    /// Компоненты связности (для цепочки соседних дуг — одна).
    pub components: usize,
    /// Плотность: nodes / d_pol.
    pub density: f64,
}

/// Рёбра LENS: пары соседних хранимых дуг (семантика `FromTopology`).
pub fn lens_edges(indices: &[u32]) -> Vec<(u32, u32)> {
    indices
        .windows(2)
        .filter_map(|w| Some((w[0], w[1])))
        .collect()
}

/// Статистика графа по индексам дуг.
pub fn graph_stats(indices: &[u32], d_pol: u32) -> GraphStats {
    let edges = lens_edges(indices);
    GraphStats {
        nodes: indices.len(),
        edges: edges.len(),
        // Рёбра связывают каждую пару соседних дуг — LENS-цепочка
        // связна по построению (семантика FromTopology).
        components: usize::from(!indices.is_empty()),
        density: if d_pol == 0 {
            0.0
        } else {
            indices.len() as f64 / f64::from(d_pol)
        },
    }
}

/// Graphviz DOT-представление LENS-графа (узлы — дуги, p̂ в метке).
pub fn arcs_dot(arcs: &[DecodedArc], d_pol: u32) -> String {
    let mut out =
        String::from("digraph LENS {\n  rankdir=LR;\n  node [shape=circle, fontsize=10];\n");
    for a in arcs {
        out.push_str(&format!(
            "  N{} [label=\"{}\\np={:+.0}\"];\n",
            a.index, a.index, a.p_hat
        ));
    }
    let indices: Vec<u32> = arcs.iter().map(|a| a.index).collect();
    for (u, v) in lens_edges(&indices) {
        out.push_str(&format!("  N{u} -> N{v};\n"));
    }
    out.push_str(&format!("  // d_pol={d_pol}, nnz={}\n", arcs.len()));
    out.push_str("}\n");
    out
}

/// ASCII-матрица смежности (только при `d_pol ≤ MATRIX_D_MAX`).
///
/// `X` — ребро-цепочка соседних дуг, `#` — диагональ (активная дуга).
pub fn ascii_matrix(arcs: &[DecodedArc], d_pol: u32) -> Option<String> {
    if d_pol == 0 || d_pol > MATRIX_D_MAX {
        return None;
    }
    let active: Vec<bool> = (0..d_pol)
        .map(|i| arcs.iter().any(|a| a.index == i))
        .collect();
    let indices: Vec<u32> = arcs.iter().map(|a| a.index).collect();
    let edges = lens_edges(&indices);
    let width = d_pol as usize;
    let mut out = String::from("     ");
    for j in 0..width {
        out.push_str(&format!("{:x}", j % 16));
    }
    out.push('\n');
    for i in 0..width {
        out.push_str(&format!("{i:04x} "));
        for j in 0..width {
            let ch = if i == j {
                if active[i] {
                    '#'
                } else {
                    '·'
                }
            } else if edges.contains(&(i as u32, j as u32)) {
                'X'
            } else {
                '·'
            };
            out.push(ch);
        }
        out.push('\n');
    }
    Some(out)
}

/// McWeeny-невязка декодированных дуг: `max |λ² − λ|`, λ = (1 + p̂)/2.
pub fn mcweeny_of_arcs(arcs: &[DecodedArc]) -> f64 {
    idempotency_residual(arcs.iter().map(|a| a.p_hat))
}

/// Полный SHA-256 файла в hex (32 байта).
pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Полный отчёт инспекции в JSON (машиночитаемый вид для CLI `--json`).
pub fn report_json(
    path: &str,
    data: &[u8],
    kind: FileKind,
    arcs: &[DecodedArc],
    d_pol: u32,
    recon: &CryptoRecon,
    gyro: Option<Json>,
) -> Json {
    let indices: Vec<u32> = arcs.iter().map(|a| a.index).collect();
    let stats = graph_stats(&indices, d_pol);
    let mut pairs: Vec<(String, Json)> = vec![
        ("file".into(), Json::str(path)),
        ("size".into(), Json::Num(data.len() as f64)),
        ("sha256".into(), Json::str(sha256_hex(data))),
        ("kind".into(), Json::str(kind.name())),
        ("d_pol".into(), Json::Num(f64::from(d_pol))),
        ("nnz".into(), Json::Num(arcs.len() as f64)),
        (
            "arcs".into(),
            Json::Arr(
                arcs.iter()
                    .map(|a| {
                        Json::Obj(vec![
                            ("idx".into(), Json::Num(f64::from(a.index))),
                            ("trit".into(), Json::Num(f64::from(a.trit))),
                            ("p".into(), Json::num(a.p_hat)),
                        ])
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "graph".into(),
            Json::Obj(vec![
                ("nodes".into(), Json::Num(stats.nodes as f64)),
                ("edges".into(), Json::Num(stats.edges as f64)),
                ("density".into(), Json::num(stats.density)),
            ]),
        ),
        ("born_entropy_bits".into(), Json::num(born_entropy(arcs))),
        ("qcm_theory".into(), Json::num(qcm_theory(arcs, d_pol))),
        ("crypto".into(), recon.to_json()),
    ];
    if let Some(g) = gyro {
        pairs.push(("gyro".into(), g));
    }
    Json::Obj(pairs)
}

/// Верхняя граница данных без заголовка (для raw-пакетов).
pub fn raw_d_pol(data: &[u8]) -> u32 {
    (data.len() * 4) as u32
}

/// Читатель по данным файла; `Ok(None)` — файл слишком мал для заголовка.
pub fn try_pqw_reader(data: &[u8]) -> Result<Option<PqwReader<'_>>, String> {
    if data.len() < HEADER_SIZE {
        return Ok(None);
    }
    match PqwReader::from_bytes(data) {
        Ok(r) => Ok(Some(r)),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::PqwWriter;

    #[test]
    fn detect_pqw_magics() {
        let mut buf = PqwWriter::new(8).unwrap();
        let v1 = buf.add_phase(1, 0.5).unwrap().to_bytes().unwrap();
        assert_eq!(detect_kind(&v1), FileKind::PqwV1);

        let v2 = PqwWriter::new(6)
            .unwrap()
            .add_phase(2, 1.0)
            .unwrap()
            .to_bytes_packed()
            .unwrap();
        assert_eq!(detect_kind(&v2), FileKind::PqwV2);
    }

    #[test]
    fn detect_raw_packed4_and_text() {
        // Одна дуга −1: 0b10 в младшей паре, остальное — фон.
        let raw = [0b0000_0010u8, 0, 0, 0];
        assert_eq!(detect_kind(&raw), FileKind::RawPacked4 { d_pol: 16 });

        // Печатаемый ASCII не детектируется как Packed4-кандидат.
        let text = b"hello world, plain text!";
        assert_eq!(detect_kind(text), FileKind::Opaque);
    }

    #[test]
    fn raw_arcs_decode_exact_positions() {
        // byte0 = 0b10 → дуга 0 = −1; byte1 = 0b01_00_00_00 → дуга 4 = +1;
        // byte2 = 0b1000 → дуга 9 = −1 (пара 1 байта 2).
        let data = [0b0000_0010u8, 0b0000_0001, 0b0000_1000];
        let arcs = raw_arcs(&data);
        let summary: Vec<(u32, i8)> = arcs.iter().map(|a| (a.index, a.trit)).collect();
        assert_eq!(summary, vec![(0, -1), (4, 1), (9, -1)]);
        assert_eq!(arcs[0].theta, std::f64::consts::PI);
        assert_eq!(arcs[1].theta, 0.0);
    }

    #[test]
    fn entropy_extremes() {
        assert_eq!(shannon_entropy(&[]), 0.0);
        assert_eq!(shannon_entropy(&[0u8; 1024]), 0.0);
        // 256 различных байтов ровно по одному разу → 8 бит/байт.
        let uniform: Vec<u8> = (0..=255u8).collect();
        assert!((shannon_entropy(&uniform) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn chi2_detects_degenerate() {
        let uniform: Vec<u8> = (0..=255u8).cycle().take(256 * 8).collect();
        let chi = chi2_uniform(&uniform);
        assert!(chi < 1e-9, "uniform → chi2 ≈ 0, got {chi}");
        let degenerate = [7u8; 4096];
        let chi = chi2_uniform(&degenerate);
        assert!(chi > 10_000.0, "degenerate → huge chi2, got {chi}");
    }

    #[test]
    fn container_signatures() {
        assert_eq!(
            detect_containers(b"Salted__abcdef"),
            vec!["openssl-enc (Salted__, AES/ChaCha via enc)"]
        );
        assert!(detect_containers(b"PK\x03\x04rest").contains(&"zip (возможно, encrypted entries)"));
        assert!(detect_containers(b"plain data").is_empty());
    }

    #[test]
    fn recon_verdicts() {
        // Структурированные данные (нули) — не шифр.
        let zeros = crypto_recon(&[0u8; 512]);
        assert_eq!(zeros.verdict, "structured");
        assert!(!zeros.encrypted_like());

        // Псевдослучайный поток (xorshift, детерминированный) — шифроподобный.
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut rnd = vec![0u8; 4096];
        for b in rnd.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = (x >> 33) as u8;
        }
        let enc = crypto_recon(&rnd);
        assert_eq!(enc.verdict, "encrypted-like");
        assert!(enc.encrypted_like());
    }

    #[test]
    fn hex_dump_shape() {
        let data = b"POLER_QUANTUM!!".to_vec();
        let dump = hex_dump(&data, 16);
        assert!(dump.contains("00000000"));
        assert!(dump.contains("|POLER_QUANTUM!!|"));
        assert!(dump.contains("50 4f 4c"));
        assert!(hex_dump(&data, 0).is_empty());
    }

    #[test]
    fn strings_extraction() {
        let mut data = vec![0u8, 1, 2];
        data.extend_from_slice(b"POLER_QW secret");
        data.extend_from_slice(&[0xff, 0xfe]);
        let strings = ascii_strings(&data, 6);
        assert_eq!(strings, vec!["POLER_QW secret".to_string()]);
    }

    #[test]
    fn header_map_rows() {
        let bytes = PqwWriter::new(8)
            .unwrap()
            .add_phase(3, -1.0)
            .unwrap()
            .to_bytes()
            .unwrap();
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        let rows = header_rows(&bytes, &reader);
        assert_eq!(rows.len(), 17);
        assert_eq!(rows[0].value, "POLER_QW");
        assert!(rows.iter().any(|r| r.name == "d_pol" && r.value == "8"));
        assert!(rows.iter().any(|r| r.name == "nnz" && r.value == "1"));
    }

    #[test]
    fn graph_chain_semantics() {
        let indices = [10u32, 20, 30];
        assert_eq!(lens_edges(&indices), vec![(10, 20), (20, 30)]);
        let stats = graph_stats(&indices, 1000);
        assert_eq!(stats.nodes, 3);
        assert_eq!(stats.edges, 2);
        assert!((stats.density - 0.003).abs() < 1e-12);
    }

    #[test]
    fn dot_contains_nodes_and_edges() {
        let arcs = raw_arcs(&[0b0000_0010u8, 0b0000_0100]);
        let dot = arcs_dot(&arcs, 8);
        assert!(dot.contains("digraph LENS"));
        assert!(dot.contains("N0"));
        assert!(dot.contains("N0 -> N5"));
    }

    #[test]
    fn matrix_small_only() {
        let arcs = vec![
            DecodedArc {
                index: 1,
                trit: -1,
                p_hat: -1.0,
                theta: std::f64::consts::PI,
            },
            DecodedArc {
                index: 3,
                trit: 1,
                p_hat: 1.0,
                theta: 0.0,
            },
        ];
        let m = ascii_matrix(&arcs, 8).unwrap();
        assert!(m.contains('X'));
        assert!(m.contains('#'));
        assert!(ascii_matrix(&arcs, 1000).is_none());
    }

    #[test]
    fn born_entropy_and_qcm() {
        // p̂ = ±1 → чистые проекторы, нулевая энтропия; QCM = 1.
        let arcs = raw_arcs(&[0b0000_0010u8, 0b0000_0001]);
        assert_eq!(born_entropy(&arcs), 0.0);
        assert!((qcm_theory(&arcs, 8) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn mcweeny_of_pure_arcs_is_zero() {
        let arcs = raw_arcs(&[0b0000_0010u8]);
        assert!(mcweeny_of_arcs(&arcs).abs() < 1e-15);
    }
}

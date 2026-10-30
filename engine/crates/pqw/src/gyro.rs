//! Гироскопная топология формата v3 (`POLER_Q3`): кососимметричный
//! резонансный оператор `J = A − Aᵀ` в топологической секции контейнера.
//!
//! ## Физика секции
//!
//! Направленный поток смысла между координатами `i → j` накапливается
//! знаковыми вкладами наблюдений `A[i][j] += s_i·s_j`; антисимметричная
//! часть `J = A − Aᵀ` отделяет **циркуляцию** (направленный перенос
//! смысла) от симметричной ассоциации. Кососимметричная `J` порождает
//! унитарную прецессию `ψ ← e^(−iηJ)ψ`: норма памяти сохраняется —
//! гироскоп не забывает, а вращает фазы (Im(P) — фазовые векторы смысла).
//!
//! Хранится только верхний треугольник `i < j`: `J[j][i] = −J[i][j]`
//! восстанавливается по кососимметрии. LENS-философия сохранена: пары с
//! |J| ниже квантовой решётки (|q| < 1) не хранятся вовсе.
//!
//! ## Два кодека секции (RQ23)
//!
//! * **v1 (разреженный)**: записи фиксированной ширины —
//!   `(u16|u32 i, u16|u32 j, i8 q)` = 5/9 Б на пару;
//! * **v2 (gap-RLE)**: нулевые зоны верхнего треугольника упаковываются
//!   длинами прогонов — пары хранятся как `varint(Δ слота) + i8 q`, где
//!   слот = `j(j−1)/2 + i` — позиция пары в плотном треугольнике.
//!   Соседние пары (кластеры) дают Δ = 1–3 → запись 2–3 Б против 9 Б у
//!   разреженного кодека при `d_pol ≥ 65536` (u32-индексы) — сжатие
//!   3–4.5×. Писатель измеряет оба кодека и выбирает меньший
//!   ([`GyroData::encode_best`]); читатель прозрачно декодирует оба
//!   по полю версии секции.
//!
//! ## Раскладка секции (little-endian)
//!
//! ```text
//! 0x00  4   magic "GYRO"
//! 0x04  4   версия секции (u32: 1 = разреженный, 2 = gap-RLE)
//! 0x08  4   окно W — дальность направленного контекста (u32)
//! 0x0C  4   число пар N (u32)
//! 0x10  8   счётчик тактов t — всего наблюдений (u64)
//! 0x18  4   масштаб квантования scale (f32): max|J| ↔ 127
//! 0x1C  4   v1: reserved (u32, = 0); v2: код 1 = gap-RLE varint
//! 0x20  ·   v1: N записей по 5 B (u16-индексы) или 9 B (u32):
//!           arc_i (LE), arc_j (LE), weight q (i8, ±1..±127)
//!       ·   v2: N записей varuint64(Δ слота) + i8 q; первый Δ —
//!           абсолютный слот, последующие ≥ 1 (строгий рост слотов)
//! ```
//!
//! Вес восстанавливается как `J = q · scale / 127`. Пары строго
//! возрастают по `(i, j)` — детерминизм сериализации. Счётчик тактов
//! дублируется в reserved-слове заголовка v3 (0x70) и сверяется при
//! чтении.

use crate::error::{PqwError, Result};

/// Магические байты гироскопной секции.
pub const GYRO_MAGIC: [u8; 4] = *b"GYRO";
/// Версия раскладки секции — разреженные записи фикс-ширины.
pub const GYRO_SECTION_VERSION: u32 = 1;
/// Версия раскладки секции — gap-RLE varint (нулевые зоны треугольника
/// упакованы длинами прогонов, RQ23).
pub const GYRO_SECTION_VERSION_RLE: u32 = 2;
/// Код кодировки в поле 0x1C секции v2: gap-RLE varint.
pub const GYRO_CODEC_GAP_RLE: u32 = 1;
/// Размер служебного заголовка секции.
pub const GYRO_HEADER_SIZE: usize = 0x20;
/// Целевая амплитуда квантования: max |J| ↔ 127.
pub const GYRO_QUANT_MAX: f64 = 127.0;

/// Слот пары в плотном верхнем треугольнике: `j(j−1)/2 + i`, `i < j`.
///
/// Чистая арифметика — адрес пары вычисляется на лету (тот же слот,
/// что у плотной решётки русел RQ16, но без зависимости от pqc).
#[inline]
pub fn gyro_slot(i: u32, j: u32) -> u64 {
    debug_assert!(i < j, "gyro_slot: требуется i < j");
    let (i, j) = (i as u64, j as u64);
    j * (j - 1) / 2 + i
}

/// Обратное преобразование слота в пару `(i, j)` — целочисленный
/// квадратный корень, без плавучки (детерминизм абсолютный):
/// `j = ⌊(1 + √(1+8s))/2⌋` — единственное `j` с
/// `j(j−1)/2 ≤ s < j(j+1)/2`.
#[inline]
pub fn slot_to_pair(slot: u64) -> (u32, u32) {
    let disc = 1u128 + 8 * slot as u128;
    // Целочисленный √ по Ньютону: старт с оценки, доводка до точности.
    let mut x = (disc as f64).sqrt() as u128;
    while x * x > disc {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= disc {
        x += 1;
    }
    let j = x.div_ceil(2) as u64;
    let i = slot - j * (j - 1) / 2;
    (i as u32, j as u32)
}

/// LEB128 varint: кодирование u64 в 1–9 байт (канонично).
#[inline]
fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            return;
        }
    }
}

/// LEB128 varint: чтение u64 из среза с курсором. Отказ при выходе
/// за границу или переполнении 64 бит.
fn get_varint(bytes: &[u8], pos: &mut usize) -> Result<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    while *pos < bytes.len() {
        let b = bytes[*pos];
        *pos += 1;
        value |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 63 {
            return Err(PqwError::Layout("gyro section: varint overflow"));
        }
    }
    Err(PqwError::Layout("gyro section: truncated varint"))
}

/// Одна хранимая пара гироскопа: `i < j`, деквантованный вес `J[i][j]`.
///
/// Симметричный элемент `J[j][i] = −weight`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GyroPair {
    pub i: u32,
    pub j: u32,
    /// Деквантованный вес `J[i][j] = q · scale / 127`.
    pub weight: f64,
}

/// Полные данные гироскопа для записи в контейнер v3.
///
/// Пары обязаны быть отсортированы по `(i, j)` и уникальны —
/// [`GyroData::new`] сортирует и отвергает дубликаты.
#[derive(Clone, Debug, PartialEq)]
pub struct GyroData {
    window: u32,
    ticks: u64,
    pairs: Vec<(u32, u32, f64)>,
}

impl GyroData {
    /// Конструкция из сырых пар: сортировка по `(i, j)`, валидация
    /// диапазонов (`i < j < d_pol`), конечности и ненулевых весов.
    ///
    /// Нулевые веса и пары легче полкванта решётки (`|w| < scale/254`,
    /// где scale = max|w|) выбрасываются: их квант q = 0 не кодируется
    /// декодером (LENS: циркуляция ниже квантовой решётки не хранится).
    pub fn new(
        window: u32,
        ticks: u64,
        mut pairs: Vec<(u32, u32, f64)>,
        d_pol: u32,
    ) -> Result<GyroData> {
        pairs.retain(|&(_, _, w)| w.is_finite() && w != 0.0);
        pairs.sort_unstable_by_key(|a| (a.0, a.1));
        for w in pairs.windows(2) {
            if (w[0].0, w[0].1) == (w[1].0, w[1].1) {
                return Err(PqwError::DuplicateIndex(w[1].0));
            }
        }
        for &(i, j, _) in &pairs {
            if i >= j {
                return Err(PqwError::BadIndex { index: i, d_pol });
            }
            if j >= d_pol {
                return Err(PqwError::BadIndex { index: j, d_pol });
            }
        }
        // Пары ниже полкванта дали бы q = 0 — отбрасываем до проверки пустоты.
        let max = pairs
            .iter()
            .map(|&(_, _, w)| w.abs())
            .fold(0.0_f64, f64::max);
        if max > 0.0 {
            let half_quant = max / (2.0 * GYRO_QUANT_MAX);
            pairs.retain(|&(_, _, w)| w.abs() >= half_quant);
        }
        if pairs.is_empty() {
            return Err(PqwError::Layout(
                "gyro section requires at least one pair (use v2 container for empty gyro)",
            ));
        }
        Ok(GyroData {
            window,
            ticks,
            pairs,
        })
    }

    /// Окно направленного контекста W.
    pub fn window(&self) -> u32 {
        self.window
    }

    /// Счётчик тактов (всего наблюдений потока).
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Пары верхнего треугольника `(i, j, J[i][j])`, отсортированы.
    pub fn pairs(&self) -> &[(u32, u32, f64)] {
        &self.pairs
    }

    /// Масштаб квантования: max |J| по хранимым парам.
    pub fn scale(&self) -> f32 {
        let max = self
            .pairs
            .iter()
            .map(|&(_, _, w)| w.abs())
            .fold(0.0_f64, f64::max);
        if max > 0.0 {
            max as f32
        } else {
            1.0
        }
    }

    /// Кодирование секции (v1, разреженный кодек): 32 B служебных +
    /// N × (2w+1) B записей, где w = 2 при `index16` (u16-индексы), иначе 4.
    pub fn encode(&self, index16: bool) -> Result<Vec<u8>> {
        let scale = self.scale();
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PqwError::BadValue(scale));
        }
        let record = if index16 { 5usize } else { 9 };
        let mut out = Vec::with_capacity(GYRO_HEADER_SIZE + self.pairs.len() * record);
        out.extend_from_slice(&GYRO_MAGIC);
        out.extend_from_slice(&GYRO_SECTION_VERSION.to_le_bytes());
        out.extend_from_slice(&self.window.to_le_bytes());
        out.extend_from_slice(&(self.pairs.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.ticks.to_le_bytes());
        out.extend_from_slice(&scale.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for &(i, j, w) in &self.pairs {
            if index16 {
                debug_assert!(i <= u16::MAX as u32 && j <= u16::MAX as u32);
                out.extend_from_slice(&(i as u16).to_le_bytes());
                out.extend_from_slice(&(j as u16).to_le_bytes());
            } else {
                out.extend_from_slice(&i.to_le_bytes());
                out.extend_from_slice(&j.to_le_bytes());
            }
            // Квантование: q = round(w / scale · 127), зажатый в ±127.
            // |q| < 1 → 0 → пара не попадает в контейнер (выше отфильтрованы).
            let q = (w / scale as f64 * GYRO_QUANT_MAX).round();
            let q = q.clamp(-GYRO_QUANT_MAX, GYRO_QUANT_MAX) as i8;
            out.push(q as u8);
        }
        Ok(out)
    }

    /// Кодирование секции (v2, gap-RLE, RQ23): нулевые зоны верхнего
    /// треугольника упакованы длинами прогонов — записи
    /// `varuint64(Δ слота) + i8 q` без явных индексов.
    ///
    /// Первый Δ — абсолютный слот первой пары, последующие Δ ≥ 1
    /// (строгий рост слотов). ВАЖНО: порядок кодирования — по СЛОТУ
    /// (`j`-строки треугольника), который не совпадает с лексикографическим
    /// порядком `(i, j)` канонизации v1: пара (0,6) имеет слот 15, а
    /// (3,5) — 10. Кодируем отсортированную по слоту копию; декодер
    /// возвращает пары в каноническом порядке `(i, j)` — потребители
    /// (трит-шифр, слияние, русла) не видят разницы кодеков.
    pub fn encode_rle(&self) -> Result<Vec<u8>> {
        let scale = self.scale();
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PqwError::BadValue(scale));
        }
        let mut slots: Vec<(u64, f64)> = self
            .pairs
            .iter()
            .map(|&(i, j, w)| (gyro_slot(i, j), w))
            .collect();
        slots.sort_unstable_by_key(|&(s, _)| s);
        let mut out = Vec::with_capacity(GYRO_HEADER_SIZE + slots.len() * 3);
        out.extend_from_slice(&GYRO_MAGIC);
        out.extend_from_slice(&GYRO_SECTION_VERSION_RLE.to_le_bytes());
        out.extend_from_slice(&self.window.to_le_bytes());
        out.extend_from_slice(&(self.pairs.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.ticks.to_le_bytes());
        out.extend_from_slice(&scale.to_le_bytes());
        out.extend_from_slice(&GYRO_CODEC_GAP_RLE.to_le_bytes());
        let mut prev_slot: u64 = 0;
        for (k, &(slot, w)) in slots.iter().enumerate() {
            let delta = if k == 0 { slot } else { slot - prev_slot };
            debug_assert!(k == 0 || delta >= 1, "дубликат слота в отсортированном списке");
            put_varint(&mut out, delta);
            let q = (w / scale as f64 * GYRO_QUANT_MAX).round();
            let q = q.clamp(-GYRO_QUANT_MAX, GYRO_QUANT_MAX) as i8;
            out.push(q as u8);
            prev_slot = slot;
        }
        Ok(out)
    }

    /// Лучший кодек по измерению: кодируем оба (v1 разреженный и v2
    /// gap-RLE) и берём меньший. Ничья — v1 (простой путь чтения).
    ///
    /// Возвращает байты секции и номер версии кодека в ней.
    pub fn encode_best(&self, index16: bool) -> Result<(Vec<u8>, u32)> {
        let sparse = self.encode(index16)?;
        let rle = self.encode_rle()?;
        if rle.len() < sparse.len() {
            Ok((rle, GYRO_SECTION_VERSION_RLE))
        } else {
            Ok((sparse, GYRO_SECTION_VERSION))
        }
    }
}

/// Разобранная гироскопная секция контейнера v3 (zero-copy не критична:
/// пары деквантуются в плотный Vec — их сотни, не миллионы).
#[derive(Clone, Debug, PartialEq)]
pub struct GyroSection {
    window: u32,
    ticks: u64,
    scale: f32,
    index16: bool,
    /// Версия кодека секции: 1 — разреженный, 2 — gap-RLE (RQ23).
    codec: u32,
    pairs: Vec<GyroPair>,
}

impl GyroSection {
    /// Полный разбор и валидация секции поверх среза.
    ///
    /// Проверяются: magic, версия (v1 разреженный / v2 gap-RLE), точная
    /// длина, конечный масштаб, диапазон `i < j < d_pol`, строгое
    /// возрастание `(i, j)`, `|q| ∈ [1, 127]`. `index16` относится
    /// только к v1: v2 хранит слоты varint без индексов.
    pub fn decode(bytes: &[u8], d_pol: u32, index16: bool) -> Result<GyroSection> {
        if bytes.len() < GYRO_HEADER_SIZE {
            return Err(PqwError::Truncated {
                need: GYRO_HEADER_SIZE,
                have: bytes.len(),
            });
        }
        if bytes[..4] != GYRO_MAGIC {
            return Err(PqwError::Layout("gyro section: bad magic"));
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != GYRO_SECTION_VERSION && version != GYRO_SECTION_VERSION_RLE {
            return Err(PqwError::UnsupportedVersion(version));
        }
        let window = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let ticks = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let scale = f32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let codec_field = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PqwError::BadValue(scale));
        }
        let (pairs, index16) = match version {
            GYRO_SECTION_VERSION_RLE => {
                if codec_field != GYRO_CODEC_GAP_RLE {
                    return Err(PqwError::Layout(
                        "gyro section v2: unknown codec code (expected 1 = gap-RLE)",
                    ));
                }
                Self::decode_rle_body(&bytes[GYRO_HEADER_SIZE..], count, scale, d_pol)?
            }
            _ => {
                if codec_field != 0 {
                    return Err(PqwError::ReservedBits {
                        value: codec_field as u64,
                    });
                }
                Self::decode_sparse_body(&bytes[GYRO_HEADER_SIZE..], count, scale, d_pol, index16)?
            }
        };
        Ok(GyroSection {
            window,
            ticks,
            scale,
            index16,
            codec: version,
            pairs,
        })
    }

    /// Разбор тела секции v1: записи фикс-ширины (u16|u32, u16|u32, i8).
    fn decode_sparse_body(
        body: &[u8],
        count: usize,
        scale: f32,
        d_pol: u32,
        index16: bool,
    ) -> Result<(Vec<GyroPair>, bool)> {
        let record = if index16 { 5usize } else { 9 };
        let expected = count * record;
        if body.len() != expected {
            return Err(PqwError::Layout(
                "gyro section: length does not match pair count",
            ));
        }
        let mut pairs: Vec<GyroPair> = Vec::with_capacity(count);
        let mut body = body;
        for _ in 0..count {
            let (i, j, q, rest) = if index16 {
                let i = u16::from_le_bytes([body[0], body[1]]) as u32;
                let j = u16::from_le_bytes([body[2], body[3]]) as u32;
                let q = body[4] as i8;
                (i, j, q, &body[5..])
            } else {
                let i = u32::from_le_bytes(body[0..4].try_into().unwrap());
                let j = u32::from_le_bytes(body[4..8].try_into().unwrap());
                let q = body[8] as i8;
                (i, j, q, &body[9..])
            };
            body = rest;
            if i >= j || j >= d_pol {
                return Err(PqwError::BadIndex { index: j, d_pol });
            }
            if q == 0 {
                return Err(PqwError::Layout("gyro section: zero weight quant"));
            }
            if let Some(prev) = pairs.last() {
                if (prev.i, prev.j) >= (i, j) {
                    return Err(PqwError::UnsortedTopology);
                }
            }
            pairs.push(GyroPair {
                i,
                j,
                weight: q as f64 * scale as f64 / GYRO_QUANT_MAX,
            });
        }
        Ok((pairs, index16))
    }

    /// Разбор тела секции v2 (gap-RLE): записи `varuint64(Δ слота) + i8 q`;
    /// слоты накапливаются, пары восстанавливаются через `slot_to_pair`,
    /// результат канонизируется сортировкой по `(i, j)` — потребитель видит
    /// тот же порядок, что и у секции v1.
    fn decode_rle_body(
        body: &[u8],
        count: usize,
        scale: f32,
        d_pol: u32,
    ) -> Result<(Vec<GyroPair>, bool)> {
        let mut pairs: Vec<GyroPair> = Vec::with_capacity(count);
        let mut pos = 0usize;
        let mut slot: u64 = 0;
        for k in 0..count {
            let delta = get_varint(body, &mut pos)?;
            if pos >= body.len() {
                return Err(PqwError::Truncated {
                    need: pos + 1,
                    have: body.len(),
                });
            }
            let q = body[pos] as i8;
            pos += 1;
            // Первый Δ — абсолютный слот (≥ 0), последующие ≥ 1:
            // строгое возрастание слотов = уникальность пар.
            if k > 0 && delta == 0 {
                return Err(PqwError::Layout(
                    "gyro section v2: zero slot gap (pairs must be unique)",
                ));
            }
            slot = slot
                .checked_add(delta)
                .ok_or(PqwError::Layout("gyro section v2: slot overflow"))?;
            let (i, j) = slot_to_pair(slot);
            if i >= j || j >= d_pol {
                return Err(PqwError::BadIndex { index: j, d_pol });
            }
            if q == 0 {
                return Err(PqwError::Layout("gyro section: zero weight quant"));
            }
            pairs.push(GyroPair {
                i,
                j,
                weight: q as f64 * scale as f64 / GYRO_QUANT_MAX,
            });
        }
        if pos != body.len() {
            return Err(PqwError::Layout(
                "gyro section v2: trailing bytes after the last record",
            ));
        }
        // Канонизация: порядок слотов (j-строки) → лексикографический (i, j).
        pairs.sort_unstable_by_key(|a| (a.i, a.j));
        Ok((pairs, false))
    }

    /// Окно направленного контекста W.
    pub fn window(&self) -> u32 {
        self.window
    }

    /// Счётчик тактов.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Масштаб квантования весов.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Ширина индексов пар (true → u16). Для v2 (gap-RLE) — всегда
    /// `false`: индексов в записи нет, слоты varint.
    pub fn index16(&self) -> bool {
        self.index16
    }

    /// Версия кодека секции: 1 — разреженный, 2 — gap-RLE (RQ23).
    pub fn codec(&self) -> u32 {
        self.codec
    }

    /// Деквантованные пары верхнего треугольника.
    pub fn pairs(&self) -> &[GyroPair] {
        &self.pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> GyroData {
        // Несортированный вход — конструктор обязан отсортировать.
        GyroData::new(
            256,
            10_000,
            vec![(40, 50, 3.0), (1, 9, -5.0), (1, 9, 0.0), (2, 7, 1.0)],
            4096,
        )
        .unwrap()
    }

    #[test]
    fn data_sorts_and_drops_zero() {
        let d = sample_data();
        // Нулевой вес (1,9,0.0) отброшен; пары отсортированы.
        assert_eq!(
            d.pairs(),
            &[(1u32, 9u32, -5.0), (2, 7, 1.0), (40, 50, 3.0)]
        );
        assert_eq!(d.window(), 256);
        assert_eq!(d.ticks(), 10_000);
        assert!((d.scale() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn data_rejects_bad_pairs() {
        // i >= j
        assert!(GyroData::new(1, 1, vec![(9, 1, 1.0)], 16).is_err());
        // j >= d_pol
        assert!(GyroData::new(1, 1, vec![(1, 16, 1.0)], 16).is_err());
        // дубликат пары
        assert!(GyroData::new(1, 1, vec![(1, 2, 1.0), (1, 2, 2.0)], 16).is_err());
        // пустой гироскоп → v3 невозможен
        assert!(GyroData::new(1, 1, vec![(1, 2, 0.0)], 16).is_err());
        // NaN
        assert!(GyroData::new(1, 1, vec![(1, 2, f64::NAN)], 16).is_err());
    }

    #[test]
    fn data_drops_subquant_pairs() {
        // Пара легче полкванта (5.0/254 ≈ 0.0197) выбрасывается:
        // её q = 0 не легален в секции. (Одиночная пара не отбрасывается
        // никогда: scale = её собственный вес → q = 127.)
        let d = GyroData::new(1, 1, vec![(1, 2, 5.0), (3, 4, 0.001)], 16).unwrap();
        assert_eq!(d.pairs(), &[(1u32, 2u32, 5.0)]);
    }

    #[test]
    fn section_roundtrip_u16() {
        let d = sample_data();
        let bytes = d.encode(true).unwrap();
        assert_eq!(bytes.len(), GYRO_HEADER_SIZE + 3 * 5);
        let s = GyroSection::decode(&bytes, 4096, true).unwrap();
        assert_eq!(s.window(), 256);
        assert_eq!(s.ticks(), 10_000);
        assert_eq!(s.pairs().len(), 3);
        // Деквантованные веса: q = round(w/scale·127) → граница ошибки
        // scale/(2·127) (полкванта решётки).
        let tol = 5.0 / GYRO_QUANT_MAX / 2.0 + 1e-12;
        for (p, &(i, j, w)) in s.pairs().iter().zip(d.pairs()) {
            assert_eq!((p.i, p.j), (i, j));
            let err = (p.weight - w).abs();
            assert!(err <= tol, "pair ({i},{j}): err {err} > {tol}");
        }
        // Знаки сохранены.
        assert!(s.pairs()[0].weight < 0.0);
        assert!(s.pairs()[1].weight > 0.0);
    }

    #[test]
    fn section_roundtrip_u32() {
        let d = GyroData::new(
            64,
            1,
            vec![(70_000, 80_000, 2.5), (65_536, 70_001, -1.0)],
            1_000_000,
        )
        .unwrap();
        let bytes = d.encode(false).unwrap();
        assert_eq!(bytes.len(), GYRO_HEADER_SIZE + 2 * 9);
        let s = GyroSection::decode(&bytes, 1_000_000, false).unwrap();
        assert_eq!(s.pairs().len(), 2);
        assert_eq!(s.pairs()[0].i, 65_536);
        assert_eq!(s.pairs()[1].j, 80_000);
        assert!((s.pairs()[1].weight - 2.5).abs() <= 2.5 / GYRO_QUANT_MAX / 2.0 + 1e-12);
    }

    // ===================== gap-RLE (v2, RQ23) =====================

    #[test]
    fn slot_pair_roundtrip_dense() {
        // Все пары маленькой решётки: слот биективен.
        let d = 12u32;
        let mut next = 0u64;
        for j in 1..d {
            for i in 0..j {
                assert_eq!(gyro_slot(i, j), next, "pair ({i},{j})");
                assert_eq!(slot_to_pair(next), (i, j), "slot {next}");
                next += 1;
            }
        }
        assert_eq!(next, d as u64 * (d as u64 - 1) / 2);
        // Граничные слоты больших решёток (d_pol ≥ 65536, слоты > 2^31).
        let far = gyro_slot(65_535, 65_536);
        assert!(far > 1u64 << 31);
        assert_eq!(slot_to_pair(far), (65_535, 65_536));
        assert_eq!(slot_to_pair(0), (0, 1));
        assert_eq!(slot_to_pair(1), (0, 2));
        assert_eq!(slot_to_pair(2), (1, 2));
        assert_eq!(slot_to_pair(3), (0, 3));
    }

    #[test]
    fn section_rle_roundtrip_clustered() {
        // Кластер соседних пар; порядок кодирования — по слоту ((3,5)=10,
        // (4,5)=11, (0,6)=15, (5,6)=20), порядок на выходе — канонический (i, j).
        let d = GyroData::new(
            8,
            777,
            vec![(3, 5, 1.0), (4, 5, -2.0), (5, 6, 3.0), (0, 6, 1.5)],
            65536,
        )
        .unwrap();
        let bytes = d.encode_rle().unwrap();
        let s = GyroSection::decode(&bytes, 65536, false).unwrap();
        assert_eq!(s.codec(), GYRO_SECTION_VERSION_RLE);
        assert!(!s.index16());
        assert_eq!(s.window(), 8);
        assert_eq!(s.ticks(), 777);
        assert_eq!(s.pairs().len(), 4);
        // Канонический порядок совпадает с v1-кодеком того же гироскопа.
        let s1 = GyroSection::decode(&d.encode(false).unwrap(), 65536, false).unwrap();
        assert_eq!(
            s.pairs().iter().map(|p| (p.i, p.j)).collect::<Vec<_>>(),
            s1.pairs().iter().map(|p| (p.i, p.j)).collect::<Vec<_>>(),
        );
        let orig = d.pairs();
        for (p, &(i, j, w)) in s.pairs().iter().zip(orig) {
            assert_eq!((p.i, p.j), (i, j));
            assert!((p.weight - w).abs() <= 3.0 / GYRO_QUANT_MAX / 2.0 + 1e-12);
        }
    }

    #[test]
    fn section_rle_roundtrip_scattered_large() {
        // Разбросанные пары большой решётки (d_pol > 65536): u32-режим v1
        // против varint-слотов v2 — обе записи декодируются одинаково.
        let pairs = vec![
            (0, 1, 1.0),
            (10_000, 90_000, -4.0),
            (65_536, 70_000, 2.0),
            (100_000, 100_001, -1.0),
        ];
        let d = GyroData::new(4, 1_000, pairs, 200_000).unwrap();
        let sparse = d.encode(false).unwrap();
        let rle = d.encode_rle().unwrap();
        let s1 = GyroSection::decode(&sparse, 200_000, false).unwrap();
        let s2 = GyroSection::decode(&rle, 200_000, false).unwrap();
        assert_eq!(s1.codec(), GYRO_SECTION_VERSION);
        assert_eq!(s2.codec(), GYRO_SECTION_VERSION_RLE);
        // Деквантованные веса совпадают (одна и та же сетка квантов).
        assert_eq!(s1.pairs().len(), s2.pairs().len());
        for (a, b) in s1.pairs().iter().zip(s2.pairs().iter()) {
            assert_eq!((a.i, a.j), (b.i, b.j));
            assert!((a.weight - b.weight).abs() < 1e-12);
        }
    }

    #[test]
    fn rle_compresses_clustered_topology() {
        // Кластер из 1000 соседних пар в решётке d_pol = 65536+
        // (u32-режим v1 = 9 Б/пару): v2 пишет 2 Б/пару → ≥ 4x сжатие.
        let mut pairs = Vec::new();
        // Строка j = 5000: пары (i, 5000) для i = 0..999 — соседние слоты.
        for i in 0..1000u32 {
            pairs.push((i, 5000, 1.0));
        }
        let d = GyroData::new(2, 5, pairs, 70_000).unwrap();
        let sparse = d.encode(false).unwrap();
        let rle = d.encode_rle().unwrap();
        assert!(
            sparse.len() as f64 / rle.len() as f64 >= 3.0,
            "sparse {} B vs rle {} B — обещано ≥ 3×",
            sparse.len(),
            rle.len()
        );
        // encode_best выбирает v2 для кластера.
        let (best, version) = d.encode_best(false).unwrap();
        assert_eq!(version, GYRO_SECTION_VERSION_RLE);
        assert_eq!(best.len(), rle.len());
        // Обратно: бит-в-бит та же топология.
        let s = GyroSection::decode(&best, 70_000, false).unwrap();
        assert_eq!(s.pairs().len(), 1000);
        assert_eq!((s.pairs()[0].i, s.pairs()[0].j), (0, 5000));
        assert_eq!((s.pairs()[999].i, s.pairs()[999].j), (999, 5000));
    }

    #[test]
    fn encode_best_prefers_sparse_when_rle_loses() {
        // Разбросанные пары маленькой решётки в u16-режиме: v1 = 5 Б/пару,
        // v2-слоты 1–2 Б varint + квант — сравнимо; главное — roundtrip
        // выбранного кодека сходится.
        let mut pairs = Vec::new();
        for k in 0..8u32 {
            pairs.push((k * 137 % 500, 500 + (k * 271 % 500), (k % 2) as f64 * 2.0 - 1.0));
        }
        let d = GyroData::new(1, 9, pairs, 1024).unwrap();
        let (best, version) = d.encode_best(true).unwrap();
        let s = GyroSection::decode(&best, 1024, version != GYRO_SECTION_VERSION_RLE).unwrap();
        assert_eq!(s.pairs().len(), d.pairs().len());
        let orig: Vec<(u32, u32)> = d.pairs().iter().map(|&(i, j, _)| (i, j)).collect();
        let got: Vec<(u32, u32)> = s.pairs().iter().map(|p| (p.i, p.j)).collect();
        assert_eq!(orig, got);
    }

    #[test]
    fn rle_decode_rejects_corruption() {
        let d = GyroData::new(4, 10, vec![(3, 5, 1.0), (4, 5, -2.0), (5, 6, 1.0)], 65536).unwrap();
        let bytes = d.encode_rle().unwrap();
        // Плохой код кодека (0 вместо 1).
        let mut b = bytes.clone();
        b[28..32].copy_from_slice(&0u32.to_le_bytes());
        assert!(GyroSection::decode(&b, 65536, false).is_err());
        // Нулевой квант.
        let mut b = bytes.clone();
        // Первая запись: varint(слот (3,5) = 10) = 1 байт + квант.
        b[GYRO_HEADER_SIZE + 1] = 0;
        assert!(GyroSection::decode(&b, 65536, false).is_err());
        // Усечение хвоста.
        assert!(GyroSection::decode(&bytes[..bytes.len() - 1], 65536, false).is_err());
        // Хвостовые байты.
        let mut b = bytes.clone();
        b.push(0);
        assert!(GyroSection::decode(&b, 65536, false).is_err());
        // Слот за границей решётки: первая Δ — varint(2^34).
        let mut b = bytes.clone();
        let big: [u8; 5] = [0x82, 0x80, 0x80, 0x80, 0x02];
        b[GYRO_HEADER_SIZE..GYRO_HEADER_SIZE + 5].copy_from_slice(&big);
        assert!(GyroSection::decode(&b, 65536, false).is_err());
        // Нулевой прогон между парами (дубликат слота).
        let mut b = bytes.clone();
        // Вторая запись: varint(Δ=1) на позиции 33 → перепишем в 0.
        b[GYRO_HEADER_SIZE + 2] = 0;
        assert!(GyroSection::decode(&b, 65536, false).is_err());
    }

    #[test]
    fn decode_rejects_corruption() {
        let bytes = sample_data().encode(true).unwrap();
        // Короткий буфер.
        assert!(GyroSection::decode(&bytes[..31], 4096, true).is_err());
        // Плохой magic.
        let mut b = bytes.clone();
        b[0] = b'X';
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Плохая версия секции (3 не существует).
        let mut b = bytes.clone();
        b[4] = 3;
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Ломаем счётчик пар.
        let mut b = bytes.clone();
        b[12] = 99;
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Нулевой масштаб.
        let mut b = bytes.clone();
        b[24..28].copy_from_slice(&0f32.to_le_bytes());
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Ненулевое reserved.
        let mut b = bytes.clone();
        b[28] = 1;
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Плохой индекс: j >= d_pol.
        let mut b = bytes.clone();
        b[GYRO_HEADER_SIZE + 2..GYRO_HEADER_SIZE + 4].copy_from_slice(&9999u16.to_le_bytes());
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Нулевой квант веса.
        let mut b = bytes.clone();
        b[GYRO_HEADER_SIZE + 4] = 0;
        assert!(GyroSection::decode(&b, 4096, true).is_err());
        // Несортированные пары: первая (i,j) больше второй.
        let mut b = bytes.clone();
        b[GYRO_HEADER_SIZE] = 30; // первая пара i=30 > второй (2,7)
        b[GYRO_HEADER_SIZE + 1] = 0;
        assert!(GyroSection::decode(&b, 4096, true).is_err());
    }
}

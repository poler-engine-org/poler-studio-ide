//! Десериализатор: zero-copy разбор контейнера поверх заимствованного среза
//! (обычный буфер или mmap-отображение).
//!
//! `from_bytes` выполняет полную структурную и семантическую валидацию
//! (заголовок, смещения, длины, сортированность индексов, триты).
//! Digest payload проверяется отдельно и лениво — [`PqwReader::verify_payload`]:
//! «мгновенный холодный старт» не должен упираться в хеширование гигабайтов.
//!
//! ## Три поколения формата
//!
//! * **v1** (`POLER_QW`): разрежённая LENS-топология + байты кривизны;
//! * **v2** (`POLER_Q2`): плотный массив упакованных тритов (4 дуги на байт),
//!   топологии нет. Канонический sparse-вид v2 — только ненулевые триты:
//!   явный `Zero` семантически равен отсутствующей дуге (честная монета,
//!   λ = ½), поэтому v1- и v2-контейнеры одного тритового состояния
//!   дают идентичную квантовую семантику. Сырой плотный вид v2 отдаёт
//!   [`PqwReader::iter_packed_trits`];
//! * **v3** (`POLER_Q3`): Packed4-фазы + гироскопная топология
//!   `J = A − Aᵀ` в топологической секции (RQ10) — верхний треугольник
//!   квантованных весов + счётчик тактов в reserved-слове заголовка.
//!   Доступ — [`PqwReader::gyro`];
//! * **v4** (`POLER_Q4`): v3 + секция лексикона `LEXI` сразу за
//!   гироскопной (RQ17) — обратная карта кодировщика (координата →
//!   доминантный токен). Смещение секции выводимо:
//!   `topology_offset + topology_len`, до конца файла. Доступ —
//!   [`PqwReader::lexicon`]; digest покрывает фазы + гироскоп + лексикон.
//! * **v5** (`POLER_Q5`): v4 + секция контекст-рефлекса `REFL` сразу за
//!   лексиконом до конца файла (RQ23) — динамический след диалога:
//!   события `(координата, полярность)` + имя собеседника + счётчик
//!   реплик. Длина LEXI выводится её собственным разбором
//!   ([`Lexicon::decode_prefix`]); REFL занимает остаток до EOF.
//!   Доступ — [`PqwReader::reflex`]; digest покрывает всё.

use std::borrow::Cow;

use crate::error::{PqwError, Result};
use crate::gyro::GyroSection;
use crate::header::{Header, HEADER_SIZE, OFF_RESERVED};
use crate::lexicon::Lexicon;
use crate::mcweeny;
use crate::phase::{packed_trit_at, PhaseByte, Trit, TritEncoding};
use crate::reflex::ReflexSection;
use crate::sha256::sha256_trunc24;
use crate::topology;
use crate::trit_bloch;

/// Одна хранимая дуга: индекс в `[0, d_pol)` + упакованная фаза.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arc {
    /// Индекс дуги в состоянии.
    pub index: u32,
    /// Упакованная фаза: трит + кривизна (v2: σ = 63 для ±1, 0 для Zero).
    pub phase: PhaseByte,
}

impl Arc {
    /// Деквантованное значение p̂.
    #[inline]
    pub fn p(self) -> f64 {
        self.phase.p()
    }

    /// Фазовый угол θ̂ = arccos(p̂).
    #[inline]
    pub fn theta(self) -> f64 {
        self.phase.theta()
    }
}

/// Zero-copy читатель `.poler` / `.pqw`.
pub struct PqwReader<'a> {
    header: Header,
    data: &'a [u8],
}

impl<'a> PqwReader<'a> {
    /// Полный разбор и валидация поверх среза (буфер или mmap).
    pub fn from_bytes(data: &'a [u8]) -> Result<PqwReader<'a>> {
        let header = Header::from_bytes(data)?;

        if header.is_packed() {
            if header.is_reflex() {
                return Self::validate_v5(header, data);
            }
            if header.is_lexicon() {
                return Self::validate_v4(header, data);
            }
            if header.is_gyro() {
                return Self::validate_gyro(header, data);
            }
            return Self::validate_packed(header, data);
        }

        let topo_off = header.topology_offset as usize;
        let topo_len = header.topology_len as usize;
        let phase_off = header.phase_offset as usize;
        let phase_len = header.phase_len as usize;
        let width = header.flags.index_width();

        if topo_off != HEADER_SIZE {
            return Err(PqwError::Layout("topology must start at 0x80"));
        }
        if header.nnz > u64::from(header.d_pol) {
            return Err(PqwError::Layout("nnz exceeds d_pol"));
        }
        if header.topology_len != header.nnz * width as u64 {
            return Err(PqwError::InconsistentTopology {
                field: "topology_len",
                expected: header.nnz * width as u64,
                actual: header.topology_len,
            });
        }
        if header.phase_len != header.nnz {
            return Err(PqwError::InconsistentTopology {
                field: "phase_len",
                expected: header.nnz,
                actual: header.phase_len,
            });
        }
        if phase_off != topo_off + topo_len {
            return Err(PqwError::Layout("phase blocks must follow the topology"));
        }
        let expected_len = phase_off + phase_len;
        if data.len() < expected_len {
            return Err(PqwError::Truncated {
                need: expected_len,
                have: data.len(),
            });
        }
        if data.len() > expected_len {
            return Err(PqwError::Layout("trailing bytes after the phase blocks"));
        }

        // Семантическая валидация payload: индексы и триты.
        let indices = topology::decode_indices(&data[topo_off..phase_off], header.flags.index16())?;
        topology::validate_indices(&indices, header.d_pol)?;
        for &raw in &data[phase_off..phase_off + phase_len] {
            PhaseByte::from_raw(raw)?;
        }

        Ok(PqwReader { header, data })
    }

    /// Валидация контейнера v3 (Packed4 + гироскопная топология):
    /// фазы как в v2, затем гироскопная секция `J = A − Aᵀ`.
    ///
    /// Сверяется и структура секции (magic/версия/длины/индексы/сортировка),
    /// и перекрёстная пара: reserved-слово заголовка = счётчику тактов секции.
    fn validate_gyro(header: Header, data: &'a [u8]) -> Result<PqwReader<'a>> {
        // Фазы — по правилам v2.
        let reader = Self::validate_packed_v3_phases(&header, data)?;

        // Гироскопная секция: ровно topology_len байтов после фаз,
        // никаких хвостовых данных.
        let d = header.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let topo_off = header.topology_offset as usize;
        let topo_len = header.topology_len as usize;
        if topo_off != HEADER_SIZE + packed_len {
            return Err(PqwError::Layout(
                "v3: gyro section must follow the phase blocks",
            ));
        }
        let expected_len = topo_off + topo_len;
        if data.len() < expected_len {
            return Err(PqwError::Truncated {
                need: expected_len,
                have: data.len(),
            });
        }
        if data.len() > expected_len {
            return Err(PqwError::Layout(
                "v3: trailing bytes after the gyro section",
            ));
        }
        let section = GyroSection::decode(
            &data[topo_off..expected_len],
            header.d_pol,
            header.flags.index16(),
        )?;

        // Перекрёстная проверка: счётчик тактов в reserved-слове.
        let ticks_reserved = u64::from_le_bytes(
            data[OFF_RESERVED..OFF_RESERVED + 8].try_into().unwrap(),
        );
        if ticks_reserved != section.ticks() {
            return Err(PqwError::Layout(
                "v3: reserved tick counter disagrees with the gyro section",
            ));
        }
        Ok(reader)
    }

    /// Фазовая часть v3 (те же правила, что и v2, но без ограничения
    /// «topology обязана быть пустой»: за фазами приходит гироскоп).
    fn validate_packed_v3_phases(header: &Header, data: &'a [u8]) -> Result<PqwReader<'a>> {
        let d = header.d_pol as usize;
        let packed_len = d.div_ceil(4);

        if header.phase_offset != HEADER_SIZE as u64 {
            return Err(PqwError::Layout(
                "v3 container: phases must start at 0x80",
            ));
        }
        if header.phase_len != packed_len as u64 {
            return Err(PqwError::InconsistentTopology {
                field: "phase_len",
                expected: packed_len as u64,
                actual: header.phase_len,
            });
        }
        if header.nnz > u64::from(header.d_pol) {
            return Err(PqwError::Layout("nnz exceeds d_pol"));
        }
        let expected_len = HEADER_SIZE + packed_len;
        if data.len() < expected_len {
            return Err(PqwError::Truncated {
                need: expected_len,
                have: data.len(),
            });
        }

        // Семантическая валидация тритов: коды 0b11 запрещены, пары
        // за пределами d_pol — Zero, nnz сходится с пересчётом.
        let packed = &data[HEADER_SIZE..expected_len];
        let mut nonzero: u64 = 0;
        for (bi, &b) in packed.iter().enumerate() {
            let mut bits = b;
            for j in 0..4 {
                match bits & 0b11 {
                    0 => {}
                    1 | 2 => {
                        if bi * 4 + j >= d {
                            return Err(PqwError::Layout(
                                "padding pair beyond d_pol must be zero",
                            ));
                        }
                        nonzero += 1;
                    }
                    _ => return Err(PqwError::ReservedTrit(b)),
                }
                bits >>= 2;
            }
        }
        if nonzero != header.nnz {
            return Err(PqwError::InconsistentTopology {
                field: "nnz",
                expected: nonzero,
                actual: header.nnz,
            });
        }
        Ok(PqwReader {
            header: header.clone(),
            data,
        })
    }

    /// Валидация контейнера v4 (v3 + лексикон `LEXI` за гироскопом):
    /// фазы и гироскопная секция — по правилам v3, затем секция
    /// лексикона от `topology_offset + topology_len` до конца файла.
    ///
    /// Проверяются и структура секции (magic/версия/d_pol/записи),
    /// и перекрёстная пара: reserved-слово заголовка = счётчику тактов
    /// гироскопной секции (как в v3).
    fn validate_v4(header: Header, data: &'a [u8]) -> Result<PqwReader<'a>> {
        // Фазы — по правилам v2/v3.
        let reader = Self::validate_packed_v3_phases(&header, data)?;

        let d = header.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let topo_off = header.topology_offset as usize;
        let topo_len = header.topology_len as usize;
        if topo_off != HEADER_SIZE + packed_len {
            return Err(PqwError::Layout(
                "v4: gyro section must follow the phase blocks",
            ));
        }
        let gyro_end = topo_off + topo_len;
        if data.len() < gyro_end {
            return Err(PqwError::Truncated {
                need: gyro_end,
                have: data.len(),
            });
        }
        // Гироскопная секция валидируется срезом ровно своей длины;
        // за ней обязан идти лексикон (флаг LEXICON = контейнер v4).
        let section = GyroSection::decode(
            &data[topo_off..gyro_end],
            header.d_pol,
            header.flags.index16(),
        )?;

        // Секция лексикона — от конца гироскопа до EOF, без хвостов.
        let lexi = Lexicon::decode(&data[gyro_end..], header.d_pol)?;
        if lexi.is_empty() {
            return Err(PqwError::Layout(
                "v4: lexicon section requires at least one entry",
            ));
        }

        // Перекрёстная проверка: счётчик тактов в reserved-слове.
        let ticks_reserved =
            u64::from_le_bytes(data[OFF_RESERVED..OFF_RESERVED + 8].try_into().unwrap());
        if ticks_reserved != section.ticks() {
            return Err(PqwError::Layout(
                "v4: reserved tick counter disagrees with the gyro section",
            ));
        }
        Ok(reader)
    }

    /// Валидация контейнера v5 (v4 + контекст-рефлекс `REFL` за
    /// лексиконом): фазы и гироскоп — по правилам v3, лексикон — по
    /// правилам v4 (но не до EOF: за ним рефлекс), затем секция REFL
    /// от конца лексикона до конца файла.
    ///
    /// Проверяются структура секции рефлекса (magic/версия/d_pol/
    /// события/имя) и перекрёстная пара: reserved-слово заголовка =
    /// счётчику тактов гироскопной секции (как в v3/v4).
    fn validate_v5(header: Header, data: &'a [u8]) -> Result<PqwReader<'a>> {
        // Фазы — по правилам v2/v3/v4.
        let reader = Self::validate_packed_v3_phases(&header, data)?;

        let d = header.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let topo_off = header.topology_offset as usize;
        let topo_len = header.topology_len as usize;
        if topo_off != HEADER_SIZE + packed_len {
            return Err(PqwError::Layout(
                "v5: gyro section must follow the phase blocks",
            ));
        }
        let gyro_end = topo_off + topo_len;
        if data.len() < gyro_end {
            return Err(PqwError::Truncated {
                need: gyro_end,
                have: data.len(),
            });
        }
        let section = GyroSection::decode(
            &data[topo_off..gyro_end],
            header.d_pol,
            header.flags.index16(),
        )?;

        // Лексикон — префикс-разбором (за ним рефлекс, EOF ≠ конец LEXI).
        let (_lexi, lexi_len) = Lexicon::decode_prefix(&data[gyro_end..], header.d_pol)?;
        let refl_start = gyro_end + lexi_len;
        if refl_start >= data.len() {
            return Err(PqwError::Layout(
                "v5: reflex section requires at least one byte after the lexicon",
            ));
        }
        // Секция контекст-рефлекса — от конца лексикона до EOF, без хвостов.
        let (_refl, refl_used) =
            ReflexSection::decode(&data[refl_start..], header.d_pol, header.flags.index16())?;
        if refl_used != data.len() - refl_start {
            return Err(PqwError::Layout(
                "v5: trailing bytes after the reflex section",
            ));
        }

        // Перекрёстная проверка: счётчик тактов в reserved-слове.
        let ticks_reserved =
            u64::from_le_bytes(data[OFF_RESERVED..OFF_RESERVED + 8].try_into().unwrap());
        if ticks_reserved != section.ticks() {
            return Err(PqwError::Layout(
                "v5: reserved tick counter disagrees with the gyro section",
            ));
        }
        Ok(reader)
    }

    /// Валидация контейнера v2 (Packed4): плотные триты без топологии.
    fn validate_packed(header: Header, data: &'a [u8]) -> Result<PqwReader<'a>> {
        let d = header.d_pol as usize;
        let packed_len = d.div_ceil(4);

        if header.topology_offset != HEADER_SIZE as u64 {
            return Err(PqwError::Layout("packed container: topology must be empty"));
        }
        if header.topology_len != 0 {
            return Err(PqwError::Layout("packed container: topology must be empty"));
        }
        if header.phase_offset != HEADER_SIZE as u64 {
            return Err(PqwError::Layout(
                "packed container: phases must start at 0x80",
            ));
        }
        if header.phase_len != packed_len as u64 {
            return Err(PqwError::InconsistentTopology {
                field: "phase_len",
                expected: packed_len as u64,
                actual: header.phase_len,
            });
        }
        if header.nnz > u64::from(header.d_pol) {
            return Err(PqwError::Layout("nnz exceeds d_pol"));
        }
        let expected_len = HEADER_SIZE + packed_len;
        if data.len() < expected_len {
            return Err(PqwError::Truncated {
                need: expected_len,
                have: data.len(),
            });
        }
        if data.len() > expected_len {
            return Err(PqwError::Layout("trailing bytes after the packed trits"));
        }

        // Семантическая валидация: коды 0b11 запрещены, пары за пределами
        // d_pol (хвост последнего байта) обязаны быть Zero, nnz сходится
        // с точным пересчётом ненулевых тритов.
        let packed = &data[HEADER_SIZE..expected_len];
        let mut nonzero: u64 = 0;
        for (bi, &b) in packed.iter().enumerate() {
            let mut bits = b;
            for j in 0..4 {
                match bits & 0b11 {
                    0 => {}
                    1 | 2 => {
                        // Пара за пределами d_pol — посторонние данные.
                        if bi * 4 + j >= d {
                            return Err(PqwError::Layout("padding pair beyond d_pol must be zero"));
                        }
                        nonzero += 1;
                    }
                    _ => return Err(PqwError::ReservedTrit(b)),
                }
                bits >>= 2;
            }
        }
        if nonzero != header.nnz {
            return Err(PqwError::InconsistentTopology {
                field: "nnz",
                expected: nonzero,
                actual: header.nnz,
            });
        }

        Ok(PqwReader { header, data })
    }

    /// Разобранный заголовок.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.header.d_pol
    }

    /// Число хранимых дуг: v1 — записанных дуг, v2 — ненулевых тритов.
    pub fn nnz(&self) -> u64 {
        self.header.nnz
    }

    /// Гиперпараметры из заголовка.
    pub fn hyperparams(&self) -> crate::header::HyperParams {
        self.header.hyper
    }

    /// Кодировка фазовых блоков (v1 → Curved, v2 → Packed4).
    pub fn encoding(&self) -> TritEncoding {
        self.header.encoding()
    }

    /// McWeeny-инвариант на момент записи: max |λ² − λ|.
    pub fn mcweeny_residual(&self) -> f64 {
        self.header.mcweeny_residual
    }

    fn topology_bytes(&self) -> &'a [u8] {
        let a = self.header.topology_offset as usize;
        let b = self.header.phase_offset as usize;
        &self.data[a..b]
    }

    /// Сырые фазовые блоки — zero-copy заимствование: v1 — nnz байтов
    /// кривизны, v2 — `ceil(d_pol/4)` упакованных байтов.
    pub fn phase_bytes(&self) -> &'a [u8] {
        let a = self.header.phase_offset as usize;
        let b = a + self.header.phase_len as usize;
        &self.data[a..b]
    }

    /// Индексы ненулевых дуг (возрастающие).
    /// В u32-режиме на little-endian — заимствование без копирования.
    ///
    /// Ошибка невозможна после `from_bytes` (длина топологии провалидирована).
    pub fn indices(&self) -> Cow<'a, [u32]> {
        if self.header.is_packed() {
            // v2: канонический sparse-вид — только ненулевые триты.
            Cow::Owned(self.arcs().map(|arc| arc.index).collect())
        } else {
            match topology::decode_indices(self.topology_bytes(), self.header.flags.index16()) {
                Ok(cow) => cow,
                Err(_) => Cow::Owned(Vec::new()), // unreachable после from_bytes
            }
        }
    }

    /// Итератор дуг — zero-copy, без аллокаций.
    ///
    /// v1 обходит топологию + байты кривизны; v2 сканирует плотный массив
    /// упакованных тритов и отдаёт только ненулевые (Zero ≡ фон ≡ честная
    /// монета). Триты v2 материализуются как `PhaseByte` с σ = 63:
    /// `p̂ = ±1` точно, `θ̂ = 0 / π`.
    pub fn arcs(&self) -> Arcs<'a> {
        if self.header.is_packed() {
            Arcs {
                inner: ArcInner::Packed {
                    data: self.phase_bytes(),
                    d: self.header.d_pol,
                    pos: 0,
                },
            }
        } else {
            Arcs {
                inner: ArcInner::Curved {
                    topo: self
                        .topology_bytes()
                        .chunks(self.header.flags.index_width()),
                    phases: self.phase_bytes().iter(),
                    width: self.header.flags.index_width(),
                },
            }
        }
    }

    /// Деквантованные дуги: `(index, p̂)`.
    pub fn decoded(&self) -> impl Iterator<Item = (u32, f64)> + 'a {
        self.arcs().map(|arc| (arc.index, arc.phase.p()))
    }

    /// Zero-copy итератор **плотного** упакованного массива v2: все `d_pol`
    /// позиций, включая Zero — сырой битовый вид файла.
    ///
    /// Для контейнера v1 — [`PqwError::NotPacked`].
    pub fn iter_packed_trits(&self) -> Result<PackedTrits<'a>> {
        if !self.header.is_packed() {
            return Err(PqwError::NotPacked);
        }
        Ok(PackedTrits {
            data: self.phase_bytes(),
            d: self.header.d_pol,
            pos: 0,
        })
    }

    /// RQ14: плотный поток углов Блоха `(индекс, θ)` из Packed4-блоков —
    /// zero-copy из mmap, θ по LUT (без acos). Только v2.
    pub fn bloch_angles(&self) -> Result<trit_bloch::BlochAngles<'a>> {
        if !self.header.is_packed() {
            return Err(PqwError::NotPacked);
        }
        Ok(trit_bloch::BlochAngles::new(
            self.phase_bytes(),
            self.header.d_pol,
        ))
    }

    /// RQ14: разреженный (LENS) поток дуг `(индекс, θ)` — только ненулевые
    /// триты; готовые дуги для продуктового анзаца. Только v2.
    pub fn bloch_arcs(&self) -> Result<trit_bloch::BlochArcs<'a>> {
        if !self.header.is_packed() {
            return Err(PqwError::NotPacked);
        }
        Ok(trit_bloch::BlochArcs::new(
            self.phase_bytes(),
            self.header.d_pol,
        ))
    }

    /// RQ14: статистика тритовой решётки (нулевое вздутие, баланс знаков).
    /// Только v2.
    pub fn bloch_counts(&self) -> Result<trit_bloch::TritCounts> {
        if !self.header.is_packed() {
            return Err(PqwError::NotPacked);
        }
        Ok(trit_bloch::counts(
            self.phase_bytes(),
            self.header.d_pol as usize,
        ))
    }

    /// Ленивая проверка целостности payload: SHA-256 (24 байта) поверх
    /// топологии + фазовых блоков (+ гироскопной секции в v3).
    pub fn verify_payload(&self) -> Result<()> {
        if sha256_trunc24(&self.data[HEADER_SIZE..]) == self.header.payload_digest {
            Ok(())
        } else {
            Err(PqwError::CorruptPayload)
        }
    }

    /// Гироскопная топология v3/v4/v5: пары `J = A − Aᵀ` + счётчик тактов.
    ///
    /// `None` для контейнеров v1/v2 (топологической секции нет).
    /// Ошибка невозможна после `from_bytes` (секция уже провалидирована).
    /// RQ23: секция может быть закодирована кодеком v1 (разреженный)
    /// или v2 (gap-RLE) — [`GyroSection::codec`].
    pub fn gyro(&self) -> Option<GyroSection> {
        if !self.header.flags.gyro() {
            return None;
        }
        let a = self.header.topology_offset as usize;
        let b = a + self.header.topology_len as usize;
        GyroSection::decode(&self.data[a..b], self.header.d_pol, self.header.flags.index16())
            .ok()
    }

    /// Секция лексикона v4/v5: обратная карта кодировщика
    /// (координата → доминантный токен).
    ///
    /// `None` для контейнеров v1/v2/v3 (лексикона нет — мозолю v3
    /// дообучение не знает слов). Ошибка невозможна после `from_bytes`.
    /// В v5 за лексиконом следует секция рефлекса — разбор
    /// останавливается на её границе.
    pub fn lexicon(&self) -> Option<Lexicon> {
        if !self.header.is_lexicon() {
            return None;
        }
        let a =
            self.header.topology_offset as usize + self.header.topology_len as usize;
        Lexicon::decode_prefix(&self.data[a..], self.header.d_pol)
            .ok()
            .map(|(lex, _)| lex)
    }

    /// Секция контекст-рефлекса v5: динамический след диалога
    /// (события `(координата, полярность)`, имя собеседника, реплики).
    ///
    /// `None` для контейнеров v1–v4 (рефлекса нет — мозг не помнит
    /// нить разговора за пределами русел J). Ошибка невозможна после
    /// `from_bytes`.
    pub fn reflex(&self) -> Option<ReflexSection> {
        if !self.header.is_reflex() {
            return None;
        }
        let gyro_end =
            self.header.topology_offset as usize + self.header.topology_len as usize;
        match Lexicon::decode_prefix(&self.data[gyro_end..], self.header.d_pol) {
            Ok((_, lexi_len)) => ReflexSection::decode(
                &self.data[gyro_end + lexi_len..],
                self.header.d_pol,
                self.header.flags.index16(),
            )
            .ok()
            .map(|(s, _)| s),
            Err(_) => None, // unreachable после from_bytes
        }
    }

    /// McWeeny-очистка хранимых дуг: `steps` итераций `p ← purify_p(p)`.
    pub fn purify_steps(&self, steps: usize) -> Vec<(u32, f64)> {
        self.decoded()
            .map(|(index, p)| {
                let mut p = p;
                for _ in 0..steps {
                    p = mcweeny::purify_p(p);
                }
                (index, p)
            })
            .collect()
    }

    /// Два шага McWeeny — типовой режим восстановления P² = P (1–2 такта).
    pub fn purified(&self) -> Vec<(u32, f64)> {
        self.purify_steps(2)
    }
}

/// Итератор дуг (см. [`PqwReader::arcs`]).
pub struct Arcs<'a> {
    inner: ArcInner<'a>,
}

enum ArcInner<'a> {
    /// v1: чанки топологии (u16/u32 LE) + по байту кривизны на дугу.
    Curved {
        topo: std::slice::Chunks<'a, u8>,
        phases: std::slice::Iter<'a, u8>,
        width: usize,
    },
    /// v2: плотный упакованный массив; выдаются только ненулевые триты.
    Packed { data: &'a [u8], d: u32, pos: u32 },
}

impl<'a> Iterator for Arcs<'a> {
    type Item = Arc;

    fn next(&mut self) -> Option<Arc> {
        match &mut self.inner {
            ArcInner::Curved {
                topo,
                phases,
                width,
            } => {
                let chunk = topo.next()?;
                let raw = *phases.next()?;
                let mut b = [0u8; 4];
                b[..*width].copy_from_slice(chunk);
                let index = u32::from_le_bytes(b);
                Some(Arc {
                    index,
                    phase: PhaseByte::from_validated(raw),
                })
            }
            ArcInner::Packed { data, d, pos } => {
                while *pos < *d {
                    let i = *pos;
                    *pos += 1;
                    let t = packed_trit_at(data, i as usize);
                    if t != Trit::Zero {
                        let phase = match t {
                            Trit::Pos => PhaseByte::encode(Trit::Pos, 63),
                            Trit::Neg => PhaseByte::encode(Trit::Neg, 63),
                            Trit::Zero => PhaseByte::encode(Trit::Zero, 0),
                        };
                        return Some(Arc { index: i, phase });
                    }
                }
                None
            }
        }
    }
}

/// Плотный итератор упакованных тритов v2 (см. [`PqwReader::iter_packed_trits`]).
pub struct PackedTrits<'a> {
    data: &'a [u8],
    d: u32,
    pos: u32,
}

impl<'a> Iterator for PackedTrits<'a> {
    type Item = (u32, Trit);

    fn next(&mut self) -> Option<(u32, Trit)> {
        if self.pos >= self.d {
            return None;
        }
        let i = self.pos;
        self.pos += 1;
        Some((i, packed_trit_at(self.data, i as usize)))
    }
}

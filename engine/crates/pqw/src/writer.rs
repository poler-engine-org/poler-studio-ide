//! Сериализатор: сборка контейнера `.poler` / `.pqw` из разреженного
//! фазового состояния.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use crate::checksum::fnv1a64;
use crate::error::{PqwError, Result};
use crate::gyro::GyroData;
use crate::header::{
    Flags, Header, HyperParams, FORMAT_VERSION, FORMAT_VERSION_V2, FORMAT_VERSION_V3,
    FORMAT_VERSION_V4, FORMAT_VERSION_V5, HEADER_SIZE, OFF_CHECKSUM, OFF_RESERVED,
};
use crate::lexicon::Lexicon;
use crate::mcweeny;
use crate::phase::{nearest_trit, pack_trit2, quantize, PhaseByte, Trit};
use crate::reflex::ReflexData;
use crate::sha256::sha256_trunc24;
use crate::topology;

/// Писатель `.poler` / `.pqw`.
///
/// ```
/// use pqw::PqwWriter;
///
/// let bytes = PqwWriter::new(8)?
///     .hyperparams(0.5, 0.25, 0.75, 0.125)
///     .add_phase(1, -0.9)?
///     .to_bytes()?;
/// assert_eq!(bytes.len(), 0x80 + 2 + 1); // заголовок + u16-индекс + фазовый байт
/// # Ok::<(), pqw::PqwError>(())
/// ```
pub struct PqwWriter {
    d_pol: u32,
    hyper: HyperParams,
    /// BTreeMap: строгая сортированность индексов + детерминизм сериализации.
    entries: BTreeMap<u32, f32>,
    force_index32: bool,
}

impl PqwWriter {
    /// Новый писатель для состояния размерности `d_pol` (≥ 1).
    pub fn new(d_pol: u32) -> Result<PqwWriter> {
        if d_pol == 0 {
            return Err(PqwError::BadDimension(d_pol));
        }
        Ok(PqwWriter {
            d_pol,
            hyper: HyperParams::default(),
            entries: BTreeMap::new(),
            force_index32: false,
        })
    }

    /// Гиперпараметры потока (builder).
    pub fn hyperparams(
        mut self,
        eta: f32,
        gamma: f32,
        rho: f32,
        epsilon_threshold: f32,
    ) -> PqwWriter {
        self.hyper = HyperParams {
            eta,
            gamma,
            rho,
            epsilon_threshold,
        };
        self
    }

    /// Запретить u16-топологию: индексы всегда u32.
    pub fn force_index32(mut self) -> PqwWriter {
        self.force_index32 = true;
        self
    }

    /// Явная дуга: индекс + значение `p ∈ [−1, 1]`.
    ///
    /// Не фильтруется LENS: экспертный режим для точечного контроля топологии.
    pub fn add_phase(&mut self, index: u32, p: f32) -> Result<&mut PqwWriter> {
        if index >= self.d_pol {
            return Err(PqwError::BadIndex {
                index,
                d_pol: self.d_pol,
            });
        }
        if !p.is_finite() || p < -1.0 || p > 1.0 {
            return Err(PqwError::BadValue(p));
        }
        if self.entries.contains_key(&index) {
            return Err(PqwError::DuplicateIndex(index));
        }
        self.entries.insert(index, p);
        Ok(self)
    }

    /// Плотное состояние длины `d_pol`: LENS оставляет только `|p| ≥ epsilon_threshold`.
    pub fn add_state(&mut self, state: &[f32]) -> Result<&mut PqwWriter> {
        if state.len() != self.d_pol as usize {
            return Err(PqwError::StateLen {
                expected: self.d_pol as usize,
                actual: state.len(),
            });
        }
        for (i, &p) in state.iter().enumerate() {
            if !p.is_finite() || p < -1.0 || p > 1.0 {
                return Err(PqwError::BadValue(p));
            }
            if p.abs() >= self.hyper.epsilon_threshold {
                self.entries.insert(i as u32, p);
            }
        }
        Ok(self)
    }

    /// Число хранимых дуг.
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Текущие гиперпараметры.
    pub fn hyper(&self) -> HyperParams {
        self.hyper
    }

    /// Будет ли использована u16-топология.
    pub fn uses_index16(&self) -> bool {
        !self.force_index32 && self.d_pol <= 65536
    }

    fn build(&self) -> Result<(Header, Vec<u8>)> {
        if !self.hyper.is_finite() {
            return Err(PqwError::BadValue(self.hyper.eta));
        }
        let index16 = self.uses_index16();
        let indices: Vec<u32> = self.entries.keys().copied().collect();
        let topology_bytes = topology::encode_indices(&indices, index16);
        let phases: Vec<u8> = self.entries.values().map(|&p| quantize(p).raw()).collect();

        // McWeeny-инвариант: max |λ²−λ| по деквантованным значениям дуг.
        let residual = mcweeny::idempotency_residual(
            phases.iter().map(|&raw| PhaseByte::from_validated(raw).p()),
        );

        let mut payload = Vec::with_capacity(topology_bytes.len() + phases.len());
        payload.extend_from_slice(&topology_bytes);
        payload.extend_from_slice(&phases);

        let header = Header {
            format_version: FORMAT_VERSION,
            d_pol: self.d_pol,
            hyper: self.hyper,
            mcweeny_residual: residual,
            payload_digest: sha256_trunc24(&payload),
            topology_offset: HEADER_SIZE as u64,
            topology_len: topology_bytes.len() as u64,
            phase_offset: (HEADER_SIZE + topology_bytes.len()) as u64,
            phase_len: phases.len() as u64,
            nnz: indices.len() as u64,
            flags: Flags::new(index16),
        };
        Ok((header, payload))
    }

    /// Сборка контейнера v2: плотный массив упакованных тритов
    /// (4 трита на байт, magic `POLER_Q2`), без топологии.
    ///
    /// Правило квантования дуги в трит: `|p| ≥ 0.5 → sign(p)`, иначе `Zero`
    /// (см. [`crate::phase::nearest_trit`]); ненакопленные координаты —
    /// фоновые монеты `Zero`. Заголовок: `nnz` — число ненулевых тритов,
    /// `phase_len = ceil(d_pol / 4)`, флаги нулевые, `mcweeny_residual = 0`
    /// (чистые триты лежат точно на многообразии P² = P).
    fn build_packed(&self) -> Result<(Header, Vec<u8>)> {
        if !self.hyper.is_finite() {
            return Err(PqwError::BadValue(self.hyper.eta));
        }
        let d = self.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let mut phases = vec![0u8; packed_len];
        let mut nonzero: u64 = 0;
        for (&i, &p) in self.entries.iter() {
            let t = nearest_trit(p);
            if t == Trit::Zero {
                continue;
            }
            nonzero += 1;
            let i = i as usize;
            phases[i / 4] |= pack_trit2(t) << (2 * (i % 4));
        }
        let header = Header {
            format_version: FORMAT_VERSION_V2,
            d_pol: self.d_pol,
            hyper: self.hyper,
            mcweeny_residual: 0.0,
            payload_digest: sha256_trunc24(&phases),
            topology_offset: HEADER_SIZE as u64,
            topology_len: 0,
            phase_offset: HEADER_SIZE as u64,
            phase_len: packed_len as u64,
            nnz: nonzero,
            flags: Flags::none(),
        };
        Ok((header, phases))
    }

    /// Сборка контейнера v3: Packed4-фазы + гироскопная топология
    /// `J = A − Aᵀ` в топологической секции (magic `POLER_Q3`).
    ///
    /// Фазы занимают `[0x80, 0x80 + ceil(d/4))`, гироскопная секция
    /// следует сразу за ними (`topology_offset`), reserved-слово заголовка
    /// хранит счётчик тактов `t`, digest покрывает фазы + гироскоп.
    ///
    /// Возвращает готовые байты заголовка (с тактами в reserved и
    /// пересчитанной checksum) и payload.
    ///
    /// RQ23: кодек гироскопа выбирается измерением — gap-RLE (v2) для
    /// кластеризованных топологий, разреженный (v1) иначе.
    fn build_v3(&self, gyro: &GyroData) -> Result<(Vec<u8>, Vec<u8>)> {
        if !self.hyper.is_finite() {
            return Err(PqwError::BadValue(self.hyper.eta));
        }
        let d = self.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let mut phases = vec![0u8; packed_len];
        let mut nonzero: u64 = 0;
        for (&i, &p) in self.entries.iter() {
            let t = nearest_trit(p);
            if t == Trit::Zero {
                continue;
            }
            nonzero += 1;
            let i = i as usize;
            phases[i / 4] |= pack_trit2(t) << (2 * (i % 4));
        }
        let index16 = self.uses_index16();
        let (gyro_bytes, _codec) = gyro.encode_best(index16)?;
        let mut payload = Vec::with_capacity(phases.len() + gyro_bytes.len());
        payload.extend_from_slice(&phases);
        payload.extend_from_slice(&gyro_bytes);
        let header = Header {
            format_version: FORMAT_VERSION_V3,
            d_pol: self.d_pol,
            hyper: self.hyper,
            mcweeny_residual: 0.0,
            payload_digest: sha256_trunc24(&payload),
            topology_offset: (HEADER_SIZE + packed_len) as u64,
            topology_len: gyro_bytes.len() as u64,
            phase_offset: HEADER_SIZE as u64,
            phase_len: packed_len as u64,
            nnz: nonzero,
            flags: Flags::v3(index16),
        };
        // reserved-слово = счётчик тактов гироскопа + пересчёт checksum.
        let mut hb = header.to_bytes();
        hb[OFF_RESERVED..OFF_RESERVED + 8].copy_from_slice(&gyro.ticks().to_le_bytes());
        let checksum = fnv1a64(&hb[..OFF_CHECKSUM]);
        hb[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        Ok((hb.to_vec(), payload))
    }

    /// Сериализация в память. Детерминизм: одинаковый набор дуг → идентичные байты.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(HEADER_SIZE + self.estimated_size());
        self.write_to_vec(&mut out)?;
        Ok(out)
    }

    /// Оценка размера контейнера v1 в байтах (заголовок + топология + фазы).
    pub fn estimated_size(&self) -> usize {
        let width = if self.uses_index16() { 2 } else { 4 };
        HEADER_SIZE + self.entries.len() * (width + 1)
    }

    /// Дописать контейнер в чужой буфер **без промежуточной сборки файла** —
    /// zero-storage стриминг: графовый поиск не касается диска вообще.
    ///
    /// Ранее записанное содержимое `out` сохраняется (контейнер дописывается
    /// в хвост), поэтому один буфер можно переиспользовать между чанками.
    pub fn write_to_vec(&self, out: &mut Vec<u8>) -> Result<()> {
        let (header, payload) = self.build()?;
        out.reserve(HEADER_SIZE + payload.len());
        out.extend_from_slice(&header.to_bytes());
        out.extend_from_slice(&payload);
        Ok(())
    }

    /// Дописать **упакованный** контейнер v2 (Packed4, 4 трита на байт)
    /// в чужой буфер без промежуточной сборки файла.
    ///
    /// `d = 65536` занимает `16 КиБ` фазовых блоков — против `64 КиБ`
    /// только байтов кривизны v1 (ровно 4x по фазовым блокам).
    pub fn write_packed_trits(&self, out: &mut Vec<u8>) -> Result<()> {
        let (header, payload) = self.build_packed()?;
        out.reserve(HEADER_SIZE + payload.len());
        out.extend_from_slice(&header.to_bytes());
        out.extend_from_slice(&payload);
        Ok(())
    }

    /// Сериализация в память в формате v2 (упакованные триты).
    pub fn to_bytes_packed(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(HEADER_SIZE + self.estimated_size_packed());
        self.write_packed_trits(&mut out)?;
        Ok(out)
    }

    /// Оценка размера контейнера v2 в байтах (заголовок + плотные триты).
    pub fn estimated_size_packed(&self) -> usize {
        HEADER_SIZE + (self.d_pol as usize).div_ceil(4)
    }

    /// Запись в файл.
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.to_bytes()?;
        let mut file = std::fs::File::create(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Дописать контейнер v3 (Packed4 + гироскоп) в чужой буфер.
    ///
    /// Гироскоп обязан содержать хотя бы одну пару (LENS: пустая
    /// циркуляция — это v2-контейнер без топологии).
    pub fn write_v3(&self, out: &mut Vec<u8>, gyro: &GyroData) -> Result<()> {
        let (hb, payload) = self.build_v3(gyro)?;
        out.reserve(HEADER_SIZE + payload.len());
        out.extend_from_slice(&hb);
        out.extend_from_slice(&payload);
        Ok(())
    }

    /// Сериализация в память в формате v3 (Packed4 + гироскоп).
    pub fn to_bytes_v3(&self, gyro: &GyroData) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(HEADER_SIZE + self.estimated_size_packed() + 32);
        self.write_v3(&mut out, gyro)?;
        Ok(out)
    }

    /// Запись контейнера v3 в файл.
    pub fn write_v3_to(&self, path: impl AsRef<Path>, gyro: &GyroData) -> Result<()> {
        let bytes = self.to_bytes_v3(gyro)?;
        let mut file = std::fs::File::create(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Запись упакованного контейнера v2 в файл.
    pub fn write_packed_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.to_bytes_packed()?;
        let mut file = std::fs::File::create(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Сборка контейнера v4: v3 (фазы + гироскоп) + секция лексикона
    /// `LEXI` сразу за гироскопной (magic `POLER_Q4`, флаг LEXICON).
    ///
    /// Смещение лексикона выводимо: `topology_offset + topology_len`;
    /// отдельного поля в заголовке не требуется. Digest покрывает
    /// фазы + гироскоп + лексикон целиком.
    fn build_v4(&self, gyro: &GyroData, lexicon: &Lexicon) -> Result<(Vec<u8>, Vec<u8>)> {
        if !self.hyper.is_finite() {
            return Err(PqwError::BadValue(self.hyper.eta));
        }
        let d = self.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let mut phases = vec![0u8; packed_len];
        let mut nonzero: u64 = 0;
        for (&i, &p) in self.entries.iter() {
            let t = nearest_trit(p);
            if t == Trit::Zero {
                continue;
            }
            nonzero += 1;
            let i = i as usize;
            phases[i / 4] |= pack_trit2(t) << (2 * (i % 4));
        }
        let index16 = self.uses_index16();
        let (gyro_bytes, _codec) = gyro.encode_best(index16)?;
        let lexi_bytes = lexicon.encode(self.d_pol);
        let mut payload = Vec::with_capacity(phases.len() + gyro_bytes.len() + lexi_bytes.len());
        payload.extend_from_slice(&phases);
        payload.extend_from_slice(&gyro_bytes);
        payload.extend_from_slice(&lexi_bytes);
        let header = Header {
            format_version: FORMAT_VERSION_V4,
            d_pol: self.d_pol,
            hyper: self.hyper,
            mcweeny_residual: 0.0,
            payload_digest: sha256_trunc24(&payload),
            topology_offset: (HEADER_SIZE + packed_len) as u64,
            topology_len: gyro_bytes.len() as u64,
            phase_offset: HEADER_SIZE as u64,
            phase_len: packed_len as u64,
            nnz: nonzero,
            flags: Flags::v4(index16),
        };
        // reserved-слово = счётчик тактов гироскопа + пересчёт checksum.
        let mut hb = header.to_bytes();
        hb[OFF_RESERVED..OFF_RESERVED + 8].copy_from_slice(&gyro.ticks().to_le_bytes());
        let checksum = fnv1a64(&hb[..OFF_CHECKSUM]);
        hb[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        Ok((hb.to_vec(), payload))
    }

    /// Дописать контейнер v4 (фазы + гироскоп + лексикон) в буфер.
    ///
    /// Гироскоп обязан содержать хотя бы одну пару, лексикон — хотя бы
    /// одну запись (LENS: пустые — это v2/v3-контейнеры без секций).
    pub fn write_v4(&self, out: &mut Vec<u8>, gyro: &GyroData, lexicon: &Lexicon) -> Result<()> {
        let (hb, payload) = self.build_v4(gyro, lexicon)?;
        out.reserve(HEADER_SIZE + payload.len());
        out.extend_from_slice(&hb);
        out.extend_from_slice(&payload);
        Ok(())
    }

    /// Сериализация в память в формате v4 (фазы + гироскоп + лексикон).
    pub fn to_bytes_v4(&self, gyro: &GyroData, lexicon: &Lexicon) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(
            HEADER_SIZE + self.estimated_size_packed() + 64 + lexicon.len() * 10,
        );
        self.write_v4(&mut out, gyro, lexicon)?;
        Ok(out)
    }

    /// Запись контейнера v4 в файл.
    pub fn write_v4_to(
        &self,
        path: impl AsRef<Path>,
        gyro: &GyroData,
        lexicon: &Lexicon,
    ) -> Result<()> {
        let bytes = self.to_bytes_v4(gyro, lexicon)?;
        let mut file = std::fs::File::create(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Сборка контейнера v5 (RQ23): v4 (фазы + гироскоп + лексикон) +
    /// секция контекст-рефлекса `REFL` сразу за лексиконом
    /// (magic `POLER_Q5`, флаг REFL).
    ///
    /// Смещение секции выводится разбором лексикона (длина LEXI
    /// известна из её заголовка); сама секция REFL заканчивается
    /// ровно в EOF. Digest покрывает фазы + гироскоп + лексикон +
    /// рефлекс целиком. `topology_len` по-прежнему описывает ТОЛЬКО
    /// гироскопную секцию.
    fn build_v5(
        &self,
        gyro: &GyroData,
        lexicon: &Lexicon,
        reflex: &ReflexData,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        if !self.hyper.is_finite() {
            return Err(PqwError::BadValue(self.hyper.eta));
        }
        let d = self.d_pol as usize;
        let packed_len = d.div_ceil(4);
        let mut phases = vec![0u8; packed_len];
        let mut nonzero: u64 = 0;
        for (&i, &p) in self.entries.iter() {
            let t = nearest_trit(p);
            if t == Trit::Zero {
                continue;
            }
            nonzero += 1;
            let i = i as usize;
            phases[i / 4] |= pack_trit2(t) << (2 * (i % 4));
        }
        let index16 = self.uses_index16();
        let (gyro_bytes, _codec) = gyro.encode_best(index16)?;
        let lexi_bytes = lexicon.encode(self.d_pol);
        let refl_bytes = reflex.encode_with_dpol(index16, self.d_pol)?;
        let mut payload = Vec::with_capacity(
            phases.len() + gyro_bytes.len() + lexi_bytes.len() + refl_bytes.len(),
        );
        payload.extend_from_slice(&phases);
        payload.extend_from_slice(&gyro_bytes);
        payload.extend_from_slice(&lexi_bytes);
        payload.extend_from_slice(&refl_bytes);
        let header = Header {
            format_version: FORMAT_VERSION_V5,
            d_pol: self.d_pol,
            hyper: self.hyper,
            mcweeny_residual: 0.0,
            payload_digest: sha256_trunc24(&payload),
            topology_offset: (HEADER_SIZE + packed_len) as u64,
            topology_len: gyro_bytes.len() as u64,
            phase_offset: HEADER_SIZE as u64,
            phase_len: packed_len as u64,
            nnz: nonzero,
            flags: Flags::v5(index16),
        };
        // reserved-слово = счётчик тактов гироскопа + пересчёт checksum.
        let mut hb = header.to_bytes();
        hb[OFF_RESERVED..OFF_RESERVED + 8].copy_from_slice(&gyro.ticks().to_le_bytes());
        let checksum = fnv1a64(&hb[..OFF_CHECKSUM]);
        hb[OFF_CHECKSUM..OFF_CHECKSUM + 8].copy_from_slice(&checksum.to_le_bytes());
        Ok((hb.to_vec(), payload))
    }

    /// Дописать контейнер v5 (фазы + гироскоп + лексикон + рефлекс)
    /// в буфер.
    ///
    /// Гироскоп и лексикон обязаны быть непустыми (контракт v3/v4);
    /// рефлекс — непустой след событий (иначе это v4).
    pub fn write_v5(
        &self,
        out: &mut Vec<u8>,
        gyro: &GyroData,
        lexicon: &Lexicon,
        reflex: &ReflexData,
    ) -> Result<()> {
        let (hb, payload) = self.build_v5(gyro, lexicon, reflex)?;
        out.reserve(HEADER_SIZE + payload.len());
        out.extend_from_slice(&hb);
        out.extend_from_slice(&payload);
        Ok(())
    }

    /// Сериализация в память в формате v5 (фазы + гироскоп + лексикон +
    /// контекст-рефлекс).
    pub fn to_bytes_v5(
        &self,
        gyro: &GyroData,
        lexicon: &Lexicon,
        reflex: &ReflexData,
    ) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(
            HEADER_SIZE + self.estimated_size_packed() + 64 + lexicon.len() * 10 + 1024,
        );
        self.write_v5(&mut out, gyro, lexicon, reflex)?;
        Ok(out)
    }

    /// Запись контейнера v5 в файл.
    pub fn write_v5_to(
        &self,
        path: impl AsRef<Path>,
        gyro: &GyroData,
        lexicon: &Lexicon,
        reflex: &ReflexData,
    ) -> Result<()> {
        let bytes = self.to_bytes_v5(gyro, lexicon, reflex)?;
        let mut file = std::fs::File::create(path.as_ref())?;
        file.write_all(&bytes)?;
        Ok(())
    }
}

//! Хранилище квантованных векторов: append-only сборщик (RAM) и
//! zero-copy mmap-представление (v2.0, Приоритет 4, кирпич 1).
//!
//! Формат файла (little-endian, выравнивание кодов до 8 Б):
//!
//! ```text
//! [0..8)    magic "PRBQ\x01\0\0\0"
//! [8..12)   u32 d          — исходная размерность
//! [12..16)  u32 d_pad      — степень двойки (коды d_pad/8 Б)
//! [16..20)  u32 count      — число векторов
//! [20..24)  u32 rot_seed_lo
//! [24..28)  u32 rot_seed_hi
//! [28..32)  u32 flags = 0
//! [32..)    f32 mu[count]           — 4 Б/вектор
//!           f32 delta[count]        — 4 Б
//!           f32 gamma[count]        — 4 Б
//!           u32 ids[count]          — 4 Б (внешние DocId)
//!           pad до 8
//!           u64 codes[count × d_pad/64]  — 1 бит на координату
//! ```
//!
//! Итог: 12 Б скаляров + 4 Б id + d_pad/8 Б кода на вектор.
//! Для 768-d (паддинг 1024): 144 Б против 3072 Б fp32 — 21.3×.
//!
//! Слот append-only: повторное встраивание документа добавляет новый
//! слот (философия VocabArena — ID стабильны, компакция отдельной
//! операцией). mmap-представление ничего не копирует: страницы кодов
//! подгружаются ядром по касанию — на 1B векторов ОЗУ занимает только
//! работающий набор, а не весь файл.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use memmap2::Mmap;

use super::rabitq::{Encoder, QueryPrep, SymSide, VecScalars};

/// Магия формата (версия 1).
const MAGIC: [u8; 8] = *b"PRBQ\x01\0\0\0";
/// Заголовок в байтах (до первого массива).
const HEADER: usize = 32;

/// Общий взгляд на хранилище для HNSW и оценок: слот → коды/скаляры.
pub trait CodeSource {
    /// Число векторов.
    fn len(&self) -> usize;
    /// Пуст ли индекс.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Паддинговая размерность (степень двойки).
    fn d_pad(&self) -> usize;
    /// Коды слота (`d_pad/64` слов).
    fn codes(&self, i: u32) -> &[u64];
    /// Скаляры слота.
    fn scalars(&self, i: u32) -> VecScalars;
    /// Внешний идентификатор слота (DocId).
    fn id(&self, i: u32) -> u32;

    /// Симметричная сторона слота (без аллокаций).
    fn side(&self, i: u32) -> SymSide<'_> {
        let sc = self.scalars(i);
        SymSide {
            codes: self.codes(i),
            mu: sc.mu,
            delta: sc.delta,
        }
    }

    /// Подготовка запроса полной точности через кодировщик хранилища.
    fn prepare_query(&self, q: &[f32]) -> QueryPrep;
}

// ---------------------------------------------------------------------------
// RAM-сборщик (append-only)
// ---------------------------------------------------------------------------

/// Собираемое хранилище: векторы кодируются при `push` и лежат
/// компактными массивами. После `save` открывается mmap-представление.
#[derive(Debug, Clone)]
pub struct QuantizedStore {
    encoder: Encoder,
    mu: Vec<f32>,
    delta: Vec<f32>,
    gamma: Vec<f32>,
    ids: Vec<u32>,
    codes: Vec<u64>,
}

impl QuantizedStore {
    /// Новое хранилище размерности `d`; сид вращения фиксирован и
    /// сериализуется (запросы другого хранилища невалидны).
    pub fn new(d: usize, rot_seed: u64) -> Self {
        Self {
            encoder: Encoder::new(d, rot_seed),
            mu: Vec::new(),
            delta: Vec::new(),
            gamma: Vec::new(),
            ids: Vec::new(),
            codes: Vec::new(),
        }
    }

    /// Добавляет вектор, возвращает номер слота (append-only).
    pub fn push(&mut self, id: u32, x: &[f32]) -> u32 {
        let wpv = self.encoder.words_per_vec();
        let mut codes = vec![0u64; wpv];
        let sc = self.encoder.encode_into(x, &mut codes);
        let slot = self.ids.len() as u32;
        self.mu.push(sc.mu);
        self.delta.push(sc.delta);
        self.gamma.push(sc.gamma);
        self.ids.push(id);
        self.codes.extend_from_slice(&codes);
        slot
    }

    /// Транслирует слот во внешний идентификатор.
    pub fn slot_id(&self, slot: u32) -> u32 {
        self.ids[slot as usize]
    }

    /// Слова кода на вектор.
    pub fn words_per_vec(&self) -> usize {
        self.encoder.words_per_vec()
    }

    /// Паддинговая размерность (степень двойки).
    pub fn d_pad(&self) -> usize {
        self.encoder.d_pad
    }

    /// Сохраняет в файл (flat-формат, выравнивание кодов до 8 Б).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let count = self.ids.len();
        let wpv = self.encoder.words_per_vec();
        let mut buf: Vec<u8> = Vec::with_capacity(HEADER + count * (16 + wpv * 8) + 8);
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&(self.encoder.d as u32).to_le_bytes());
        buf.extend_from_slice(&(self.encoder.d_pad as u32).to_le_bytes());
        buf.extend_from_slice(&(count as u32).to_le_bytes());
        let seed = self.encoder.rot_seed;
        buf.extend_from_slice(&(seed as u32).to_le_bytes());
        buf.extend_from_slice(&((seed >> 32) as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        for arr in [&self.mu, &self.delta, &self.gamma] {
            for v in arr.iter().take(count) {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in self.ids.iter().take(count) {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        // паддинг до 8-байтовой границы блока кодов
        let pad = (8 - (buf.len() % 8)) % 8;
        buf.extend(std::iter::repeat(0u8).take(pad));
        debug_assert_eq!(buf.len() % 8, 0);
        for w in self.codes.iter().take(count * wpv) {
            buf.extend_from_slice(&w.to_le_bytes());
        }
        let mut f = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        f.write_all(&buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(())
    }

    /// Открывает mmap-представление сохранённого хранилища.
    pub fn open(path: &Path) -> Result<QuantizedStoreView, String> {
        QuantizedStoreView::open(path)
    }
}

impl CodeSource for QuantizedStore {
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn d_pad(&self) -> usize {
        self.encoder.d_pad
    }
    fn codes(&self, i: u32) -> &[u64] {
        let wpv = self.encoder.words_per_vec();
        let s = i as usize * wpv;
        &self.codes[s..s + wpv]
    }
    fn scalars(&self, i: u32) -> VecScalars {
        let i = i as usize;
        VecScalars {
            mu: self.mu[i],
            delta: self.delta[i],
            gamma: self.gamma[i],
        }
    }
    fn id(&self, i: u32) -> u32 {
        self.ids[i as usize]
    }
    fn prepare_query(&self, q: &[f32]) -> QueryPrep {
        self.encoder.prepare_query(q)
    }
}

// ---------------------------------------------------------------------------
// Mmap-представление (zero-copy)
// ---------------------------------------------------------------------------

/// Хранилище поверх mmap: массивы читаются прямо из страниц файла
/// без копирования. Выравнивание кодов (8 Б) проверяется при открытии.
///
/// Совет ядру: `Random` — доступ к скалярам/кодам по слотам-индексам
/// при обходе графа непредсказуем; полный скан кодов (sequential)
/// вызовы делают поверх своего буфера.
pub struct QuantizedStoreView {
    mmap: Mmap,
    d: usize,
    d_pad: usize,
    count: usize,
    wpv: usize,
    encoder: Encoder,
    mu_off: usize,
    delta_off: usize,
    gamma_off: usize,
    ids_off: usize,
    codes_off: usize,
}

impl QuantizedStoreView {
    /// Открывает и валидирует файл хранилища.
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("mmap: {e}"))?;
        #[cfg(unix)]
        let _ = mmap.advise(memmap2::Advice::Random);
        if mmap.len() < HEADER {
            return Err("хранилище обрезано: меньше заголовка".into());
        }
        if mmap[0..8] != MAGIC {
            return Err("чужая магия: не PRBQ-хранилище".into());
        }
        let u32_at = |off: usize| u32::from_le_bytes(mmap[off..off + 4].try_into().unwrap());
        let d = u32_at(8) as usize;
        let d_pad = u32_at(12) as usize;
        let count = u32_at(16) as usize;
        let seed = u32_at(20) as u64 | ((u32_at(24) as u64) << 32);
        if d == 0 || d > (1 << 20) || !d_pad.is_power_of_two() || d_pad < 64 || d_pad < d {
            return Err(format!("битый заголовок: d={d}, d_pad={d_pad}"));
        }
        let wpv = d_pad / 64;
        let mu_off = HEADER;
        let delta_off = mu_off + count * 4;
        let gamma_off = delta_off + count * 4;
        let ids_off = gamma_off + count * 4;
        let codes_off = ids_off + count * 4 + ((8 - ((ids_off + count * 4) % 8)) % 8);
        let end = codes_off + count * wpv * 8;
        if mmap.len() < end {
            return Err(format!(
                "хранилище обрезано: {} Б, ожидалось {} Б",
                mmap.len(),
                end
            ));
        }
        if mmap.as_ptr() as usize % 8 != 0 || codes_off % 8 != 0 {
            return Err("выравнивание кодов нарушено — unsafe-доступ запрещён".into());
        }
        let encoder = Encoder::new(d, seed);
        Ok(Self {
            mmap,
            d,
            d_pad,
            count,
            wpv,
            encoder,
            mu_off,
            delta_off,
            gamma_off,
            ids_off,
            codes_off,
        })
    }

    /// Исходная размерность эмбеддингов.
    pub fn dim(&self) -> usize {
        self.d
    }

    /// Число векторов.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Слова кода на вектор.
    pub fn words_per_vec(&self) -> usize {
        self.wpv
    }

    /// Скаляры всех векторов одним срезом (копия — только для бенчей).
    pub fn mu_slice(&self) -> Vec<f32> {
        (0..self.count)
            .map(|i| f32::from_le_bytes(self.mmap[self.mu_off + i * 4..].try_into().unwrap()))
            .collect()
    }

    /// Байты кодов (плотность на диске == в ОЗУ при mmap).
    pub fn codes_bytes(&self) -> usize {
        self.count * self.wpv * 8
    }

    /// Байты скаляров + id.
    pub fn scalars_bytes(&self) -> usize {
        self.count * 16
    }
}

impl CodeSource for QuantizedStoreView {
    fn len(&self) -> usize {
        self.count
    }
    fn d_pad(&self) -> usize {
        self.d_pad
    }
    fn codes(&self, i: u32) -> &[u64] {
        let off = self.codes_off + i as usize * self.wpv * 8;
        // Безопасно: mmap живёт в self, смещение валидировано при open,
        // выравнивание 8 Б проверено, длина — wpv слов.
        let ptr = unsafe { self.mmap.as_ptr().add(off) as *const u64 };
        unsafe { std::slice::from_raw_parts(ptr, self.wpv) }
    }
    fn scalars(&self, i: u32) -> VecScalars {
        let i = i as usize;
        let f32_at = |off: usize| {
            f32::from_le_bytes(
                self.mmap[off + i * 4..off + i * 4 + 4].try_into().unwrap(),
            )
        };
        VecScalars {
            mu: f32_at(self.mu_off),
            delta: f32_at(self.delta_off),
            gamma: f32_at(self.gamma_off),
        }
    }
    fn id(&self, i: u32) -> u32 {
        let off = self.ids_off + i as usize * 4;
        u32::from_le_bytes(self.mmap[off..off + 4].try_into().unwrap())
    }
    fn prepare_query(&self, q: &[f32]) -> QueryPrep {
        self.encoder.prepare_query(q)
    }
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::rabitq::SplitMix64;

    fn gauss_vec(rng: &mut SplitMix64, d: usize) -> Vec<f32> {
        (0..d)
            .map(|_| {
                let u1 = rng.next_u01().max(1e-12);
                let u2 = rng.next_u01();
                ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
            })
            .collect()
    }

    #[test]
    fn push_slots_and_ids() {
        let mut st = QuantizedStore::new(64, 7);
        assert_eq!(st.len(), 0);
        let x = vec![1.0f32; 64];
        assert_eq!(st.push(10, &x), 0);
        assert_eq!(st.push(11, &x), 1);
        assert_eq!(st.len(), 2);
        assert_eq!(st.slot_id(0), 10);
        assert_eq!(CodeSource::id(&st, 1), 11);
    }

    #[test]
    fn save_open_roundtrip_bit_exact() {
        let mut rng = SplitMix64::new(9);
        let mut st = QuantizedStore::new(300, 0x5EED);
        let mut src: Vec<Vec<f32>> = Vec::new();
        for i in 0..200u32 {
            let x = gauss_vec(&mut rng, 300);
            src.push(x.clone());
            st.push(i, &x);
        }
        let tmp = std::env::temp_dir().join(format!("prbq-rt-{}.bin", std::process::id()));
        st.save(&tmp).unwrap();
        let view = QuantizedStore::open(&tmp).unwrap();
        assert_eq!(view.dim(), 300);
        assert_eq!(view.count(), 200);
        assert_eq!(view.words_per_vec(), 8); // 300 → d_pad 512
        for i in 0..200u32 {
            assert_eq!(view.codes(i), st.codes(i), "коды слота {i} разошлись");
            assert_eq!(view.scalars(i), st.scalars(i));
            assert_eq!(view.id(i), st.id(i));
        }
        // Запросы через оба представления совпадают
        let q = gauss_vec(&mut rng, 300);
        let p1 = st.prepare_query(&q);
        let p2 = view.prepare_query(&q);
        assert_eq!(p1.yq, p2.yq);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn open_rejects_garbage() {
        let tmp = std::env::temp_dir().join(format!("prbq-bad-{}.bin", std::process::id()));
        std::fs::write(&tmp, b"NOTMAGIC.........").unwrap();
        assert!(QuantizedStore::open(&tmp).is_err());
        let _ = std::fs::remove_file(&tmp);
        // Обрезанный файл
        let mut st = QuantizedStore::new(64, 1);
        st.push(0, &vec![0.5; 64]);
        let tmp2 = std::env::temp_dir().join(format!("prbq-cut-{}.bin", std::process::id()));
        st.save(&tmp2).unwrap();
        let full = std::fs::read(&tmp2).unwrap();
        std::fs::write(&tmp2, &full[..full.len() - 10]).unwrap();
        assert!(QuantizedStore::open(&tmp2).is_err());
        let _ = std::fs::remove_file(&tmp2);
    }

    #[test]
    fn codes_block_is_eight_byte_aligned() {
        // count=3: 32 + 3*20 = 92 → паддинг 4 → коды с 96 — кратны 8
        let mut st = QuantizedStore::new(64, 3);
        for i in 0..3u32 {
            st.push(i, &vec![i as f32; 64]);
        }
        let tmp = std::env::temp_dir().join(format!("prbq-al-{}.bin", std::process::id()));
        st.save(&tmp).unwrap();
        let view = QuantizedStore::open(&tmp).unwrap();
        let ptr = view.codes(0).as_ptr() as usize;
        assert_eq!(ptr % 8, 0, "коды не выровнены");
        assert!(view.codes(2).len() == 1);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn density_math() {
        // 768-d → d_pad 1024: 128 Б кода + 16 Б служебных на вектор
        let st = QuantizedStore::new(768, 1);
        assert_eq!(st.d_pad(), 1024);
        assert_eq!(st.words_per_vec(), 16);
        let per_vec = st.words_per_vec() * 8 + 16;
        let fp32 = 768 * 4;
        assert!(
            per_vec * 21 <= fp32 && per_vec * 22 >= fp32,
            "плотность 768-d вне диапазона 21-22×: {per_vec} vs {fp32}"
        );
    }
}

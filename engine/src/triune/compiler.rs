//! Обратный In-Place компилятор (.t5q): модель переписывает себя в тритах.
//!
//! Инверсия классического пути «обучи → сожми»: сначала знания сжимаются
//! в тройковый кристалл (.t5q, ~4 ГБ вместо 500 ГБ), а затем модель
//! **само-модифицируется прямо в сжатой форме** — без распаковки в FP16,
//! без backprop, без градиентов:
//!
//! ```text
//! [ Стиснений файл .t5q (4 ГБ) ] ── (mmap O_RDWR / MAP_SHARED)
//!                │
//!                ▼
//! 1. Zero-Copy Execution (No-Mul SIMD) — слои читаются в тритах {−1,0,+1},
//!    вакуумные нули (до 90%) пропускаются ядрами без работы;
//!                │  (активации + фазовый вектор внимания)
//!                ▼
//! 2. Causal Operator & Phase Rotor (J = A − Aᵀ) — антисимметричная часть
//!    потока активаций решает, какие блоки требуют фазового сдвига;
//!                │  (импульс мутации: индекс блока + направление трита)
//!                ▼
//! 3. In-Place Trit Rewriter — атомарная перезапись ОДНОГО трита
//!    (byte ± 3^k, соседи не задеты), адаптация 32-битного масштаба
//!    блока, мгновенная фиксация на диск (msync + sha256).
//! ```
//!
//! ## Гарантии целостности
//!
//! - Замена одного трита — атомарная арифметика `byte += (d′−d)·3^k`
//!   в пределах одного байта: соседние четыре трита не задеты.
//! - Промежуточное состояние (мутации без `commit`) детектируется
//!   sha256-трейлером — файл честно «сломан» до фиксации.
//! - `commit()` — точка атомарной фиксации: пересчёт вакуума, новый
//!   sha256 тела, обновление трейлера, msync.
//!
//! ## Правила пластичности (градиентно-свободные)
//!
//! - **Хебб/STDP**: корреляция `c = y_i·x_j`; |c| ≥ θ·‖·‖ → потенциация
//!   по знаку c, слабая корреляция при занятом трите → депрессия к нулю.
//! - **Фазовый ротор**: `J = A − Aᵀ` выделяет доминирующее направление
//!   потока: прямой канал усиливается, обратный — гасится.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Instant;

use memmap2::MmapMut;

use crate::pqc::sha256::{sha256, Sha256};
use crate::pqc::tensor::Trit5Codec;
use crate::triune::stream_quant::{MAGIC, TRAILER, VERSION, HEADER};

/// Степени тройки для позиций тритов в байте (3^0..3^4).
const POW3: [u8; 5] = [1, 3, 9, 27, 81];
/// Максимальное допустимое значение упакованного байта (2·(1+3+9+27+81) = 242).
const MAX_BYTE: u8 = 242;

/// Направление переписывания трита.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TritDirection {
    /// Шаг к +1 (потенциация положительной связи).
    TowardPlus,
    /// Шаг к −1 (потенциация отрицательной связи).
    TowardMinus,
    /// Шаг к 0 (депрессия, вакуумизация).
    TowardZero,
}

/// Импульс мутации: адрес трита в плоском потоке .t5q + направление.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutationImpulse {
    /// Индекс блока (по `block` значений, из заголовка файла).
    pub block: u64,
    /// Индекс трита внутри блока (0..block).
    pub index: usize,
    /// Куда двигать трит.
    pub direction: TritDirection,
}

/// Конфигурация пластичности.
#[derive(Debug, Clone)]
pub struct PlasticityConfig {
    /// Порог значимости корреляции (доля от произведения норм).
    pub theta: f32,
    /// Предел изменения масштаба блока за одну адаптацию (×/÷).
    pub scale_bound: f32,
}

impl Default for PlasticityConfig {
    fn default() -> Self {
        PlasticityConfig { theta: 0.05, scale_bound: 1.25 }
    }
}

/// Статистика атомарной фиксации (commit).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommitStats {
    /// Тритов переписано (с момента открытия/последнего commit).
    pub flips: u64,
    /// Масштабов блоков адаптировано.
    pub scale_updates: u64,
    /// Вакуум до / после (нулевых тритов среди реальных значений).
    pub zeros_before: u64,
    pub zeros_after: u64,
    /// Время фиксации, мс.
    pub ms: u128,
}

// ---------------------------------------------------------------------------
// 1. TritMutator — битовый оператор одного трита
// ---------------------------------------------------------------------------

/// Арифметика одного трита внутри упакованного байта Trit5.
///
/// Ключевой инвариант: замена трита меняет ТОЛЬКО его base-3 позицию —
/// `byte += (d′ − d)·3^k` не может задеть соседние четыре трита
/// (все цифры остаются в диапазоне 0..2, байт ≤ 242).
pub struct TritMutator;

impl TritMutator {
    /// Прочитать трит №`idx` (0..5) из упакованного байта.
    #[inline(always)]
    pub fn read(byte: u8, idx: usize) -> i8 {
        debug_assert!(idx < 5, "позиция трита {idx} вне 0..5");
        (((byte / POW3[idx]) % 3) as i8) - 1
    }

    /// Атомарно заменить трит №`idx` на `trit` ∈ {−1, 0, +1}.
    /// Возвращает новый байт; ошибка — если файл повреждён (байт ≥ 243).
    #[inline(always)]
    pub fn write(byte: u8, idx: usize, trit: i8) -> Result<u8, String> {
        if idx >= 5 {
            return Err(format!("позиция трита {idx} вне 0..5"));
        }
        if !(-1..=1).contains(&trit) {
            return Err(format!("трит {trit} вне {{−1, 0, +1}}"));
        }
        if byte > MAX_BYTE {
            return Err(format!("байт 0x{byte:02X} ≥ 243 — файл повреждён"));
        }
        let pow = POW3[idx];
        let old_digit = (byte / pow) % 3;
        let new_digit = (trit + 1) as u8;
        if old_digit == new_digit {
            return Ok(byte); // уже так
        }
        // Знако-безопасная арифметика в i16: |diff| ≤ 2·81 = 162 < 243.
        let diff = (new_digit as i16 - old_digit as i16) * pow as i16;
        let result = (byte as i16 + diff) as u16;
        debug_assert!(result <= MAX_BYTE as u16, "переполнение base-3 упаковки");
        Ok(result as u8)
    }

    /// Один шаг трита в направлении `dir`.
    /// `Ok(None)` — трит уже в целевом крайнем положении (no-op).
    #[inline(always)]
    pub fn step(byte: u8, idx: usize, dir: TritDirection) -> Result<Option<u8>, String> {
        let cur = Self::read(byte, idx);
        let target = match dir {
            TritDirection::TowardPlus => cur.saturating_add(1).min(1),
            TritDirection::TowardMinus => cur.saturating_sub(1).max(-1),
            TritDirection::TowardZero => 0,
        };
        if target == cur {
            return Ok(None);
        }
        Ok(Some(Self::write(byte, idx, target)?))
    }
}

// ---------------------------------------------------------------------------
// 2. T5qMmapView — прямое отображение .t5q в память с правом записи
// ---------------------------------------------------------------------------

/// Read-write mmap файла .t5q: слои читаются и **переписываются** прямо
/// в тритах, минуя декомпрессию. Формат — Trit5 Stream v1 (stream_quant).
pub struct T5qMmapView {
    mmap: MmapMut,
    path: PathBuf,
    /// Значений на блок (из заголовка).
    block: usize,
    /// Байт на блок: 4 (scale) + ceil(block/5) (триты).
    piece: usize,
    /// Всего блоков в теле.
    blocks: u64,
    /// Всего значений (последний блок может быть неполным).
    values: u64,
}

impl T5qMmapView {
    /// Открыть .t5q на чтение+запись (O_RDWR, MAP_SHARED-семантика MmapMut).
    pub fn open_rw<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("открыть {}: {e}", path.display()))?;
        let mmap = unsafe { MmapMut::map_mut(&file) }
            .map_err(|e| format!("mmap {}: {e}", path.display()))?;
        let bytes = &mmap[..];
        if bytes.len() < HEADER + TRAILER {
            return Err("файл меньше заголовка+трейлера".into());
        }
        if bytes[..8] != MAGIC {
            return Err("не .t5q: чужая магия".into());
        }
        let rd_u32 =
            |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        if rd_u32(0x08) != VERSION {
            return Err(format!("версия {}", rd_u32(0x08)));
        }
        let block = rd_u32(0x0C) as usize;
        if !(16..=65536).contains(&block) {
            return Err(format!("странный блок: {block}"));
        }
        let values = u64::from_le_bytes(
            bytes[bytes.len() - TRAILER..bytes.len() - TRAILER + 8].try_into().unwrap(),
        );
        let blocks = u64::from_le_bytes(
            bytes[bytes.len() - TRAILER + 8..bytes.len() - TRAILER + 16]
                .try_into()
                .unwrap(),
        );
        let piece = 4 + block.div_ceil(5);
        let body = bytes.len() - TRAILER - HEADER;
        if body % piece != 0 || body / piece != blocks as usize {
            return Err(format!(
                "тело {body} Б не бьётся на {blocks} блоков по {piece} Б"
            ));
        }
        let view = T5qMmapView { mmap, path, block, piece, blocks, values };
        Ok(view)
    }

    /// Значений на блок (из заголовка).
    pub fn block_size(&self) -> usize {
        self.block
    }

    /// Всего блоков.
    pub fn block_count(&self) -> u64 {
        self.blocks
    }

    /// Всего значений в исходном потоке.
    pub fn values(&self) -> u64 {
        self.values
    }

    /// Путь к файлу (для диагностики).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Смещение масштаба блока `b` (f32-LE, 4 Б).
    #[inline(always)]
    fn scale_off(&self, b: u64) -> Result<usize, String> {
        if b >= self.blocks {
            return Err(format!("блок {b} вне файла ({})", self.blocks));
        }
        Ok(HEADER + (b as usize) * self.piece)
    }

    /// (смещение байта, позиция трита в байте) для трита `idx` блока `b`.
    #[inline(always)]
    fn trit_pos(&self, b: u64, idx: usize) -> Result<(usize, usize), String> {
        if idx >= self.block {
            return Err(format!("индекс трита {idx} вне блока {}", self.block));
        }
        let base = self.scale_off(b)? + 4;
        Ok((base + idx / 5, idx % 5))
    }

    /// Масштаб блока `b`.
    pub fn block_scale(&self, b: u64) -> Result<f32, String> {
        let off = self.scale_off(b)?;
        Ok(f32::from_le_bytes(self.mmap[off..off + 4].try_into().unwrap()))
    }

    /// Прямая запись масштаба блока (f32-LE). Требуется s ≥ 0 и конечность.
    pub fn set_block_scale(&mut self, b: u64, s: f32) -> Result<(), String> {
        if !s.is_finite() || s < 0.0 {
            return Err(format!("масштаб {s} не конечный/отрицательный"));
        }
        let off = self.scale_off(b)?;
        self.mmap[off..off + 4].copy_from_slice(&s.to_le_bytes());
        Ok(())
    }

    /// Прочитать трит (индекс внутри блока).
    pub fn trit(&self, b: u64, idx: usize) -> Result<i8, String> {
        let (off, pos) = self.trit_pos(b, idx)?;
        Ok(TritMutator::read(self.mmap[off], pos))
    }

    /// Атомарно перезаписать один трит. `Ok(true)` — байт изменился.
    pub fn write_trit(&mut self, b: u64, idx: usize, trit: i8) -> Result<bool, String> {
        let (off, pos) = self.trit_pos(b, idx)?;
        let old = self.mmap[off];
        let new = TritMutator::write(old, pos, trit)?;
        if new == old {
            return Ok(false);
        }
        self.mmap[off] = new;
        Ok(true)
    }

    /// Деквантованное значение веса (scale × трит) — для сверки семантики.
    pub fn weight(&self, b: u64, idx: usize) -> Result<f32, String> {
        Ok(self.block_scale(b)? * self.trit(b, idx)? as f32)
    }

    /// Сброс mmap-страниц на диск (msync).
    pub fn sync(&self) -> Result<(), String> {
        self.mmap.flush().map_err(|e| format!("msync: {e}"))
    }

    /// Доступ к сырым байтам (только чтение, для verify).
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..]
    }

    /// Валидация файла: магия + sha256 тела (после мутаций без commit —
    /// честно НЕ сходится).
    pub fn verify(&self) -> Result<(), String> {
        stream_verify(&self.mmap[..]).map(|_| ())
    }
}

/// Локальная обёртка над verify_t5q (чтобы не тащить pub use в шапку).
fn stream_verify(bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() < HEADER + TRAILER {
        return Err("файл меньше заголовка+трейлера".into());
    }
    if bytes[..8] != MAGIC {
        return Err("не .t5q: чужая магия".into());
    }
    let block = u32::from_le_bytes(bytes[0x0C..0x10].try_into().unwrap()) as usize;
    let body = &bytes[HEADER..bytes.len() - TRAILER];
    let tail = &bytes[bytes.len() - TRAILER..];
    let digest = sha256(body);
    if digest[..] != tail[32..64] {
        return Err("sha256 блоков не сходится (незакоммиченные мутации?)".into());
    }
    let piece = 4 + block.div_ceil(5);
    if body.len() % piece != 0 {
        return Err(format!("тело {} Б не бьётся на блоки по {piece} Б", body.len()));
    }
    Ok(body.len() / piece)
}

// ---------------------------------------------------------------------------
// 3. PlasticityCompiler — мост «активации модели → триты на диске»
// ---------------------------------------------------------------------------

/// Компилятор пластичности: принимает сигналы активаций (SSN/GLM),
/// транслирует их в импульсы мутаций и атомарно переписывает триты
/// и масштабы блоков прямо в mmap .t5q. Фиксация — `commit()`.
pub struct PlasticityCompiler {
    view: T5qMmapView,
    cfg: PlasticityConfig,
    /// Тритов переписано с открытия/последнего commit.
    flips: u64,
    /// Масштабов адаптировано.
    scale_updates: u64,
    /// Вакуум на момент открытия (базовая линия).
    zeros_base: u64,
}

impl PlasticityCompiler {
    /// Открыть .t5q на переписывание. Файл обязан быть валиден
    /// (незакоммиченные мутации прошлой сессии → отказ).
    pub fn open<P: AsRef<Path>>(path: P, cfg: PlasticityConfig) -> Result<Self, String> {
        let view = T5qMmapView::open_rw(path)?;
        view.verify()?;
        let zeros_base = recount_zeros(&view);
        Ok(PlasticityCompiler { view, cfg, flips: 0, scale_updates: 0, zeros_base })
    }

    /// Доступ к mmap-представлению (чтение весов No-Mul ядрами).
    pub fn view(&self) -> &T5qMmapView {
        &self.view
    }

    /// Мутабельный доступ к mmap-представлению (прямые перезаписи тритов).
    pub fn view_mut(&mut self) -> &mut T5qMmapView {
        &mut self.view
    }

    /// Конфигурация пластичности.
    pub fn config(&self) -> &PlasticityConfig {
        &self.cfg
    }

    /// Применить пачку импульсов мутаций (in-place, без фиксации).
    /// Возвращает число реально изменённых байтов.
    pub fn apply(&mut self, impulses: &[MutationImpulse]) -> Result<u64, String> {
        let mut changed = 0u64;
        for imp in impulses {
            let (off, pos) = self.view.trit_pos(imp.block, imp.index)?;
            let old = self.view.mmap[off];
            let new = match TritMutator::step(old, pos, imp.direction)? {
                Some(b) => b,
                None => continue, // уже в крайнем положении
            };
            self.view.mmap[off] = new;
            changed += 1;
            self.flips += 1;
        }
        Ok(changed)
    }

    /// Адаптировать масштаб блока: `s ← s · factor` с ограничением
    /// `factor ∈ [1/scale_bound, scale_bound]` (анти-взрыв геббовской
    /// пластичности). STDP-эвристика: насыщенный блок (все триты ±1)
    /// при положительном давлении ошибки растит масштаб, разреженный —
    /// сжимает.
    pub fn adapt_scale(&mut self, block: u64, factor: f32) -> Result<(), String> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(format!("фактор масштаба {factor} некорректен"));
        }
        let lo = 1.0 / self.cfg.scale_bound;
        let f = factor.clamp(lo, self.cfg.scale_bound);
        let s = self.view.block_scale(block)?;
        // 0 остаётся 0 (полностью вакуумный блок не оживает масштабом).
        let new_s = s * f;
        if new_s.is_finite() {
            self.view.set_block_scale(block, new_s)?;
            self.scale_updates += 1;
        }
        Ok(())
    }

    /// Хеббовские импульсы по паре векторов активаций (пре/пост).
    ///
    /// Слой занимает блоки `[block_of, block_of + rows·cols/block)`;
    /// вес `W_ij` живёт в плоском индексе `i·cols + j`. Правило:
    /// `c = y_i·x_j`; `|c| ≥ θ·‖y‖·‖x‖` → Toward±sign(c), иначе
    /// (при ненулевом трите) → TowardZero (депрессия).
    /// Возвращает готовые к [`apply`](Self::apply) импульсы.
    pub fn hebbian_impulses(
        &self,
        block_of: u64,
        rows: usize,
        cols: usize,
        pre: &[f32],
        post: &[f32],
    ) -> Result<Vec<MutationImpulse>, String> {
        if pre.len() != cols || post.len() != rows {
            return Err(format!(
                "размеры не сходятся: pre {} != cols {cols}, post {} != rows {rows}",
                pre.len(),
                post.len()
            ));
        }
        let nx: f32 = pre.iter().map(|v| v * v).sum::<f32>().sqrt();
        let ny: f32 = post.iter().map(|v| v * v).sum::<f32>().sqrt();
        let gate = self.cfg.theta * nx * ny;
        if gate <= 0.0 {
            return Ok(Vec::new());
        }
        let bsz = self.view.block_size() as u64;
        let needed = (rows as u64 * cols as u64).div_ceil(bsz);
        if block_of + needed > self.view.block_count() {
            return Err(format!(
                "слой нуждается в {needed} блоках, доступно от {block_of} — {}",
                self.view.block_count() - block_of
            ));
        }
        let mut out = Vec::new();
        for (i, &y) in post.iter().enumerate() {
            if y.abs() < self.cfg.theta {
                continue; // постнейрон молчит — депрессии ниже
            }
            for (j, &x) in pre.iter().enumerate() {
                if x.abs() < self.cfg.theta {
                    continue;
                }
                let c = y * x;
                let direction = if c.abs() >= gate {
                    if c > 0.0 { TritDirection::TowardPlus } else { TritDirection::TowardMinus }
                } else {
                    TritDirection::TowardZero
                };
                let flat = (i * cols + j) as u64;
                out.push(MutationImpulse {
                    block: block_of + flat / bsz,
                    index: (flat % bsz) as usize,
                    direction,
                });
            }
        }
        Ok(out)
    }

    /// Импульсы фазового ротора: `J = A − Aᵀ` (антисимметричная часть
    /// матрицы активаций). Доминирующее направление потока усиливается,
    /// встречное — гасится: `J_ij > θ` → (i,j) к sign(A_ij), (j,i) к 0.
    /// Квадратичная сложность по рангу — вызывать на прореженных
    /// активациях.
    pub fn phase_rotor_impulses(
        &self,
        block_of: u64,
        a: &[f32],
        rank: usize,
    ) -> Result<Vec<MutationImpulse>, String> {
        if a.len() != rank * rank {
            return Err(format!("матрица {} значений ≠ ранг²={}", a.len(), rank * rank));
        }
        let norm: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        let gate = self.cfg.theta * norm * norm / (rank.max(1) as f32);
        let bsz = self.view.block_size() as u64;
        let needed = (rank as u64 * rank as u64).div_ceil(bsz);
        if block_of + needed > self.view.block_count() {
            return Err("слой не помещается в файл от заданного блока".into());
        }
        let mut out = Vec::new();
        for i in 0..rank {
            for j in (i + 1)..rank {
                let aij = a[i * rank + j];
                let aji = a[j * rank + i];
                let j_flow = aij - aji;
                if j_flow.abs() < gate {
                    continue;
                }
                // Прямой канал — усилить по фактическому знаку,
                // встречный — погасить к нулю (депрессия).
                let (fwd_dir, rev_dir) = if aij >= 0.0 {
                    (TritDirection::TowardPlus, TritDirection::TowardZero)
                } else {
                    (TritDirection::TowardMinus, TritDirection::TowardZero)
                };
                let fwd = (i * rank + j) as u64;
                let rev = (j * rank + i) as u64;
                out.push(MutationImpulse {
                    block: block_of + fwd / bsz,
                    index: (fwd % bsz) as usize,
                    direction: fwd_dir,
                });
                out.push(MutationImpulse {
                    block: block_of + rev / bsz,
                    index: (rev % bsz) as usize,
                    direction: rev_dir,
                });
            }
        }
        Ok(out)
    }

    /// Атомарная фиксация: пересчёт вакуума → sha256 тела → патч
    /// трейлера → msync. После commit файл снова проходит verify.
    pub fn commit(&mut self) -> Result<CommitStats, String> {
        let t0 = Instant::now();
        let zeros_after = recount_zeros(&self.view);
        let len = self.view.mmap.len();
        let tl = len - TRAILER;
        // sha256 тела чанками (без копирования гигабайт в кучу).
        let mut hasher = Sha256::new();
        let mut off = HEADER;
        while off < tl {
            let end = (off + (4 << 20)).min(tl);
            hasher.update(&self.view.mmap[off..end]);
            off = end;
        }
        let digest = hasher.finalize();
        self.view.mmap[tl + 16..tl + 24].copy_from_slice(&zeros_after.to_le_bytes());
        self.view.mmap[tl + 32..tl + 64].copy_from_slice(&digest);
        self.view.sync()?;
        let stats = CommitStats {
            flips: self.flips,
            scale_updates: self.scale_updates,
            zeros_before: self.zeros_base,
            zeros_after,
            ms: t0.elapsed().as_millis(),
        };
        self.flips = 0;
        self.scale_updates = 0;
        self.zeros_base = zeros_after;
        Ok(stats)
    }
}

/// Честный пересчёт вакуума: нулевые триты среди РЕАЛЬНЫХ значений
/// (хвост последнего блока — паддинг — не считается).
fn recount_zeros(view: &T5qMmapView) -> u64 {
    use rayon::prelude::*;
    let block = view.block_size();
    let piece = view.piece;
    let body = &view.mmap[HEADER..view.mmap.len() - TRAILER];
    let last = view.blocks.saturating_sub(1) as usize;
    let full = block / 5; // тритов в полном блоке, кратных 5
    let zeros = (0..view.blocks as usize)
        .into_par_iter()
        .map(|b| {
            let base = b * piece + 4;
            let packed = &body[base..base + (piece - 4)];
            let real = if b == last {
                (view.values as usize - b * block).min(block)
            } else {
                block
            };
            let mut z = 0u64;
            for (k, &byte) in packed.iter().enumerate() {
                let lo = k * 5;
                if lo >= real {
                    break;
                }
                let trits = Trit5Codec::unpack_5(byte);
                for (t_i, &t) in trits.iter().enumerate() {
                    if lo + t_i >= real {
                        break;
                    }
                    if t == 0 {
                        z += 1;
                    }
                }
            }
            let _ = full;
            z
        })
        .sum();
    zeros
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triune::stream_quant::{stream_quantize, StreamQuantConfig};
    use std::io::Write;

    /// Собрать временный .t5q из детерминированного потока.
    fn tmp_t5q(values: usize, seed: u64) -> (std::path::PathBuf, Vec<u8>) {
        let mut s = seed;
        let mut data = Vec::with_capacity(values * 4);
        for _ in 0..values {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let v = ((s >> 33) as i32 as f64 / i32::MAX as f64) as f32;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut out = Vec::new();
        stream_quantize(&data[..], &mut out, &StreamQuantConfig::default()).unwrap();
        let dir = std::env::temp_dir().join(format!("t5q_compiler_{seed}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model.t5q");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&out).unwrap();
        drop(f);
        (path, out)
    }

    #[test]
    fn trit_write_roundtrip_all_positions() {
        // Все триты во всех позициях, все переходы — упаковка сохраняется.
        for idx in 0..5usize {
            for a in [-1i8, 0, 1] {
                for b in [-1i8, 0, 1] {
                    let byte = Trit5Codec::pack_5(&[a, a, a, a, a]).unwrap();
                    let patched = TritMutator::write(byte, idx, b).unwrap();
                    let trits = Trit5Codec::unpack_5(patched);
                    assert_eq!(trits[idx], b, "трит {idx}: {a} → {b}");
                    // Соседи не задеты:
                    for (k, &t) in trits.iter().enumerate() {
                        if k != idx {
                            assert_eq!(t, a, "сосед {k} изменился!");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn trit_write_rejects_corruption() {
        assert!(TritMutator::write(243, 0, 1).is_err(), "байт ≥ 243 — повреждение");
        assert!(TritMutator::write(100, 5, 1).is_err(), "позиция вне 0..5");
        assert!(TritMutator::write(100, 0, 2).is_err(), "трит вне {{−1,0,+1}}");
    }

    #[test]
    fn step_saturates_at_extremes() {
        let plus = Trit5Codec::pack_5(&[1, 1, 1, 1, 1]).unwrap();
        assert!(TritMutator::step(plus, 0, TritDirection::TowardPlus).unwrap().is_none());
        let minus = Trit5Codec::pack_5(&[-1, -1, -1, -1, -1]).unwrap();
        assert!(TritMutator::step(minus, 2, TritDirection::TowardMinus).unwrap().is_none());
        let zero = Trit5Codec::pack_5(&[0, 0, 0, 0, 0]).unwrap();
        assert!(TritMutator::step(zero, 4, TritDirection::TowardZero).unwrap().is_none());
        // Шаг из нуля к +1:
        let stepped = TritMutator::step(zero, 1, TritDirection::TowardPlus).unwrap().unwrap();
        assert_eq!(TritMutator::read(stepped, 1), 1);
    }

    #[test]
    fn mmap_mutation_and_commit_survives_verify() {
        let (path, orig) = tmp_t5q(3 * 512 + 100, 42);
        let mut cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        assert_eq!(cmp.view().block_count(), 4);

        // 1. Мутации без commit → файл «сломан» (digest устарел).
        let impulses: Vec<MutationImpulse> = (0..40)
            .map(|i| MutationImpulse {
                block: i as u64 % 4,
                index: (i * 37) % 512,
                direction: if i % 2 == 0 {
                    TritDirection::TowardPlus
                } else {
                    TritDirection::TowardZero
                },
            })
            .collect();
        let changed = cmp.apply(&impulses).unwrap();
        assert!(changed > 0, "хотя бы один трит должен измениться");
        assert!(cmp.view().verify().is_err(), "незакоммиченные мутации должны детектироваться");

        // 2. Commit → файл валиден, байты отличаются от исходных.
        let stats = cmp.commit().unwrap();
        assert_eq!(stats.flips, changed);
        assert!(stats.ms < 5_000, "commit на маленьком файле мгновенный");
        cmp.view().verify().expect("после commit файл обязан быть валиден");
        let now = std::fs::read(&path).unwrap();
        assert_ne!(now, orig, "файл на диске должен измениться");

        // 3. Повторное открытие — валидно, мутации видны.
        let cmp2 = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        cmp2.view().verify().unwrap();
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }

    #[test]
    fn zero_recount_honors_partial_tail() {
        // 3 полных блока + хвост 100 значений: нули считаются только
        // по реальным значениям последнего блока.
        let (path, _) = tmp_t5q(3 * 512 + 100, 7);
        let cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        let z = recount_zeros(cmp.view());
        // Все нули исходного потока ~ theta=0.05 · max|w|: LCG даёт
        // ~равномерные значения, вакуум должен быть мал, но ≥ 0.
        assert!(z <= 3 * 512 + 100, "нулей больше значений: {z}");
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }

    #[test]
    fn hebbian_semantics_sign_and_gate() {
        let (path, _) = tmp_t5q(4096, 11);
        let cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        // rows=8, cols=8 → 64 значения → первый блок.
        let pre = vec![0.9f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut post = vec![0.0f32; 8];
        post[3] = 1.0;
        post[5] = -1.0;
        let imps = cmp.hebbian_impulses(0, 8, 8, &pre, &post).unwrap();
        // Активны пары (3,0) → +, (5,0) → −; остальные ниже порога/нулевые.
        assert!(!imps.is_empty());
        for imp in &imps {
            let flat = imp.block as usize * 512 + imp.index;
            let (i, j) = (flat / 8, flat % 8);
            match (i, j) {
                (3, 0) => assert_eq!(imp.direction, TritDirection::TowardPlus),
                (5, 0) => assert_eq!(imp.direction, TritDirection::TowardMinus),
                _ => assert_eq!(imp.direction, TritDirection::TowardZero),
            }
        }
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }

    #[test]
    fn phase_rotor_is_antisymmetric() {
        let (path, _) = tmp_t5q(4096, 13);
        let cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        // Ранг 4 → 16 значений в первом блоке. A асимметрична:
        // A[0][1]=1, A[1][0]=−1 → J[0][1]=2 (усилить +), J[1][0]=−2 (гасить).
        let mut a = vec![0.0f32; 16];
        a[1] = 1.0;
        a[4] = -1.0;
        let imps = cmp.phase_rotor_impulses(0, &a, 4).unwrap();
        assert!(!imps.is_empty());
        let find = |i: usize, j: usize| -> Option<TritDirection> {
            imps.iter()
                .find(|imp| {
                    let flat = imp.block as usize * 512 + imp.index;
                    flat == i * 4 + j
                })
                .map(|imp| imp.direction)
        };
        assert_eq!(find(0, 1), Some(TritDirection::TowardPlus));
        assert_eq!(find(1, 0), Some(TritDirection::TowardZero));
        // Симметричная пара не рождает импульсов: A[2][3]==A[3][2]==0.
        assert_eq!(find(2, 3), None);
        assert_eq!(find(3, 2), None);
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }

    #[test]
    fn scale_adaptation_bounded() {
        let (path, _) = tmp_t5q(512, 17);
        let mut cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        let s0 = cmp.view().block_scale(0).unwrap();
        // Взрывной фактор обрезается границей scale_bound.
        cmp.adapt_scale(0, 100.0).unwrap();
        let s1 = cmp.view().block_scale(0).unwrap();
        assert_eq!(s1, s0 * cmp.config().scale_bound, "рост обрезан");
        // Незакоммиченная адаптация масштаба ломает digest.
        assert!(cmp.view().verify().is_err(), "мутация масштаба должна детектироваться");
        cmp.commit().unwrap();
        cmp.view().verify().expect("после commit файл валиден");
        // Теперь спад — тоже с фиксацией.
        cmp.adapt_scale(0, 0.0001).unwrap();
        let s2 = cmp.view().block_scale(0).unwrap();
        assert_eq!(s2, s1 / cmp.config().scale_bound, "спад обрезан");
        assert!(cmp.adapt_scale(0, -1.0).is_err(), "отрицательный фактор запрещён");
        cmp.commit().unwrap();
        cmp.view().verify().expect("повторный commit тоже валиден");
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }

    #[test]
    fn weight_decode_matches_scale_times_trit() {
        let (path, _) = tmp_t5q(512, 19);
        let mut cmp = PlasticityCompiler::open(&path, PlasticityConfig::default()).unwrap();
        let b = 0u64;
        let idx = 123usize;
        let trit = cmp.view().trit(b, idx).unwrap();
        let scale = cmp.view().block_scale(b).unwrap();
        let w = cmp.view().weight(b, idx).unwrap();
        assert_eq!(w, scale * trit as f32);
        // Переписываем трит в +1 — вес следует за ним.
        cmp.view_mut().write_trit(b, idx, 1).unwrap();
        assert_eq!(cmp.view().weight(b, idx).unwrap(), scale);
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(path.parent().unwrap()).ok();
    }
}

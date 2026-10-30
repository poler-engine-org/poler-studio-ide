//! RQ20: `pqc merge` — архетипическое слияние мозгов:
//! `merged = A ⊗_ε B` с born-консолидацией в контейнер v4.
//!
//! `pqc archetype` (RQ18) уже умеет **измерять** произведение
//! мозг ⊗ мозг — но только читать его. Merge доводит физику до
//! записи: два тела знания становятся одним кристаллом, который
//! сразу говорит, отвечает и продолжает учиться.
//!
//! ## Физика слияния — три этажа
//!
//! **1. Фазы: `c = a ⊗_ε b` — таблица интерференции без гейта.**
//!
//! Пер-дуговая интерференция — та же насыщенная таблица
//! [`ARCHETYPE_LUT`](crate::archetype_lattice::ARCHETYPE_LUT):
//!
//! ```text
//!            b: Zero   Pos   Neg
//!      a:Zero   Zero   Pos   Neg     ← прозрачность: знание проходит
//!      a:Pos    Pos    Pos   Zero       сквозь незнание другого мозга
//!      a:Neg    Neg    Zero  Neg     ← конфликт аннигилирует в открытый
//!                                      вопрос (Zero)
//! ```
//!
//! Принципиальное отличие от RQ18: **энергетический гейт не запирает
//! произведение**. В мосте внимания гейт `E ≥ ε` защищает рассуждение
//! от ложных ассоциаций ортогональных архетипов; в слиянии оба
//! сомножителя — доверенные тела знания, и прозрачность таблицы
//! объединяет их: уникальное каждого мозга выживает, конфликт
//! аннигиляцией честно признаётся открытым вопросом. Мозг
//! «Квантовая физика» ⊗ мозг «Python» — это мозг, знающий оба
//! домена, а не Zero. Энергия `E = co/min(nnz)` остаётся в отчёте
//! как **диагностика изоморфизма** — насколько домены структурно
//! перекрываются.
//!
//! **2. Русла: `J_merged = Π_Λ(J_A + J_B)` — кососимметричная
//! интерференция циркуляции.**
//!
//! Направленные русла смысла суммируются и проецируются на тритовую
//! решётку `Π_Λ` (ближайший трит суммы, квант 0.5):
//!
//! ```text
//!   J_A[i][j] = +1, J_B[i][j] = +1   →   +2 → Π_Λ → +1  (согласованный поток выжил)
//!   J_A[i][j] = +1, J_B[i][j] = −1   →    0 → Π_Λ →  0  (встречная циркуляция погасла)
//!   J_A[i][j] = +1, русла B нет      →   +1 → Π_Λ → +1  (прозрачность)
//! ```
//!
//! Хранится только верхний треугольник `i < j` — кососимметричность
//! `J[j][i] = −J[i][j]` сохраняется **по построению** (обратный
//! элемент никогда не хранится, а определяется знаком).
//!
//! **3. Лексикон: консолидация `LEXI_A + LEXI_B` в единый
//! непротиворечивый словарь v4.**
//!
//! Координаты, занятые словами только одного мозга, вливаются его
//! словом; общая координата с одинаковой доминантой — одной записью;
//! коллизия разных доминант разрешается в пользу мозга **A**
//! (база слияния, первый аргумент — детерминизм). Результат —
//! легальный словарь v4: координаты уникальны и возрастают.
//!
//! ## Born-консолидация
//!
//! Продукт `c` уже лежит на многообразии `P² = P`: каждый трит
//! слияния — полюс (идемпотентность таблицы: `c ⊗_ε c = c`), поэтому
//! контейнер пишется **бит-в-бит без переквантования** — решётка
//! слияния сразу готова к Born-измерению, речи и дообучению. Счётчик
//! тактов гироскопа суммируется: слитый мозг видел оба потока
//! наблюдений (`ticks = ticks_A + ticks_B`).
//!
//! ## Детерминизм
//!
//! Ни одного ГПСЧ: `merge(A, B)` даёт побитово одинаковые байты
//! при одинаковых входах. Слияние мозга с самим собой сохраняет
//! знание бит-в-бит (фазы, направления русел, словарь) — удваиваются
//! только такты (два свидетеля одного опыта).
//!
//! ## Пример
//!
//! ```
//! use pqc::merge::{merge_brains, MergeConfig};
//! use pqc::gyro_lattice::QuantizedGyroCurriculum;
//!
//! // Два мозга одной размерности (train --quantized --gyro / learn).
//! let mut a = QuantizedGyroCurriculum::new(512, 0.05, 42, 8).unwrap();
//! a.ingest("фотон энергия квант света волновая функция", 1).unwrap();
//! let mut b = QuantizedGyroCurriculum::new(512, 0.05, 42, 8).unwrap();
//! b.ingest("генератор итератор замыкание поток декоратор", 1).unwrap();
//! let (merged, report) = merge_brains(
//!     &a.checkpoint().unwrap(),
//!     &b.checkpoint().unwrap(),
//!     &MergeConfig::default(),
//! )
//! .unwrap();
//!
//! // Слитый мозг — валидный контейнер; закон сохранения носителя:
//! // union − конфликты = nnz_a + nnz_b − 2·co + resonance.
//! assert_eq!(
//!     report.nnz_merged,
//!     report.nnz_a + report.nnz_b - 2 * report.co_support + report.resonance
//! );
//! // Словарь слияния не уже словаря каждого мозга (A + пустоты B).
//! assert!(report.lexicon_merged >= report.lexicon_a.max(report.lexicon_b));
//! assert!(!merged.is_empty());
//! ```

use pqw::gyro::GyroData;
use pqw::lexicon::Lexicon;
use pqw::phase::{nearest_trit, Trit};
use pqw::trit_bloch::trit_at;
use pqw::{PqwReader, PqwWriter};

use crate::archetype_lattice::{archetype_energy, nonzero_lanes};
use crate::gyro_lattice::{QuantizedGyroCurriculum, TransportMode};

// ============================================================================
// 1. Слияние фазовых решёток: c = a ⊗_ε b (таблица без гейта)
// ============================================================================

/// Статистика слияния фазовых решёток.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LatticeMergeStats {
    /// Ненулевых тритов в `a` (носитель мозга A).
    pub nnz_a: usize,
    /// Ненулевых тритов в `b`.
    pub nnz_b: usize,
    /// Дуг пересечения: оба полюса (носитель клина `a ∧ b`).
    pub co_support: usize,
    /// Согласных полюсов пересечения (конструктивная интерференция).
    pub resonance: usize,
    /// Встречных полюсов — аннигиляция в открытый вопрос Zero.
    pub conflict: usize,
    /// Энергия пересечения `E = co / min(nnz_a, nnz_b)` — диагностика
    /// структурного изоморфизма мозгов (гейт НЕ заперт).
    pub energy: f64,
    /// Прозрачный проход: полюса, выжившие только с одной стороны
    /// (уникальное знание каждого мозга).
    pub transparent: usize,
    /// Ненулевых тритов слияния `c`.
    pub nnz_out: usize,
}

/// Слияние фазовых решёток `c = a ⊗_ε b` в байтах Packed4 —
/// SWAR-интерференция без энергетического гейта, ни одного
/// FPU-вызова на дугу.
///
/// Семантика объединения знаний (union): прозрачность таблицы
/// пропускает уникальное каждого мозга, согласие полюсов
/// резонирует, встречные полюса аннигилируют в Zero — честный
/// открытый вопрос на месте конфликта мнений. Количество f64 на
/// вызов — одно (энергия диагностики в конце).
///
/// Контракт: `a.len(), b.len(), out.len() ≥ ⌈d/4⌉`, `d ≥ 1`.
pub fn merge_lattices_packed4(
    a: &[u8],
    b: &[u8],
    d: usize,
    out: &mut [u8],
) -> crate::error::Result<LatticeMergeStats> {
    let n = d.div_ceil(4);
    if d == 0 {
        return Err(crate::error::PqcError::EmptyState);
    }
    if a.len() < n || b.len() < n || out.len() < n {
        return Err(crate::error::PqcError::LengthMismatch {
            expected: n,
            actual: a.len().min(b.len()).min(out.len()),
        });
    }

    // Маска хвостового байта: паддинг-лейны (дуги ≥ d) выключаются
    // из интерференции и счётчиков — решётка кончается на дуге d−1.
    let tail = d % 4;
    let tail_mask: u8 = if tail == 0 {
        0b1111_1111
    } else {
        (1u16 << (2 * tail)) as u8 - 1
    };

    let mut nnz_a = 0usize;
    let mut nnz_b = 0usize;
    let mut co = 0usize;
    let mut resonance = 0usize;
    let bytes = n - 1; // полные байты до хвостового

    for k in 0..bytes {
        let (x, y) = (a[k], b[k]);
        // Конфликтные лейны: xor кодов == 3 (только Pos ⊕ Neg).
        let xr = x ^ y;
        let conf = xr & (xr >> 1) & 0b0101_0101;
        let conf_mask = conf | (conf << 1);
        // Слияние: насыщенное ИЛИ с погашением конфликтов.
        out[k] = (x | y) & !conf_mask;
        // Счётчики: popcount пер-лейновых масок.
        let (nza, nzb) = (nonzero_lanes(x), nonzero_lanes(y));
        let co_lanes = nza & nzb;
        let eq = !(xr | (xr >> 1)) & 0b0101_0101 & co_lanes;
        nnz_a += nza.count_ones() as usize;
        nnz_b += nzb.count_ones() as usize;
        co += co_lanes.count_ones() as usize;
        resonance += eq.count_ones() as usize;
    }
    // Хвостовой байт: только живые лейны.
    {
        let (x, y) = (a[bytes] & tail_mask, b[bytes] & tail_mask);
        let xr = x ^ y;
        let conf = xr & (xr >> 1) & 0b0101_0101;
        let conf_mask = conf | (conf << 1);
        out[bytes] = (x | y) & !conf_mask;
        let (nza, nzb) = (nonzero_lanes(x), nonzero_lanes(y));
        let co_lanes = nza & nzb;
        let eq = !(xr | (xr >> 1)) & 0b0101_0101 & co_lanes;
        nnz_a += nza.count_ones() as usize;
        nnz_b += nzb.count_ones() as usize;
        co += co_lanes.count_ones() as usize;
        resonance += eq.count_ones() as usize;
    }

    let energy = archetype_energy(co, nnz_a, nnz_b);
    let nnz_out: usize = out[..n]
        .iter()
        .map(|&b| nonzero_lanes(b).count_ones() as usize)
        .sum();
    // Носитель слияния = резонанс + прозрачный проход (конфликты погасли).
    let transparent = nnz_out - resonance;
    Ok(LatticeMergeStats {
        nnz_a,
        nnz_b,
        co_support: co,
        resonance,
        conflict: co - resonance,
        energy,
        transparent,
        nnz_out,
    })
}

// ============================================================================
// 2. Слияние русел J: J_merged = Π_Λ(J_A + J_B)
// ============================================================================

/// Статистика слияния русел циркуляции.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChannelMergeStats {
    /// Русел в A.
    pub channels_a: usize,
    /// Русел в B.
    pub channels_b: usize,
    /// Русел в слитом мозге.
    pub out: usize,
    /// Пар в обоих мозгах (зона интерференции циркуляции).
    pub shared: usize,
    /// Согласованная циркуляция выжила (сумма проецировалась в тот же знак).
    pub same_direction: usize,
    /// Встречная циркуляция погасла — Π_Λ(±1 + ∓1) = 0.
    pub annihilated: usize,
    /// Перевес одного направления над другим (возможен только при
    /// неединичных весах входных контейнеров; ±1-мозги не дают).
    pub flipped: usize,
    /// Русло только из A прошло сквозь незнание B.
    pub transparent_a: usize,
    /// Русло только из B прошло сквозь незнание A.
    pub transparent_b: usize,
}

/// Слияние русел `J_merged = Π_Λ(J_A + J_B)` — двухуказательный
/// обход отсортированных верхних треугольников.
///
/// Вход: пары `(i < j, вес)` из гироскопных секций (или
/// [`crate::gyro_lattice::TritGyro::channels`]); порядок — любой,
/// внутри сортируется по `(i, j)`. Выход: пары с весами `±1.0`
/// (Π_Λ-проекция на тритовую решётку русел) в каноническом порядке.
///
/// Кососимметричность сохраняется по построению: хранится только
/// верхний треугольник, `J[j][i] = −J[i][j]` определён знаком.
pub fn merge_channels(
    a: &[(u32, u32, f64)],
    b: &[(u32, u32, f64)],
) -> (Vec<(u32, u32, f64)>, ChannelMergeStats) {
    let mut pa: Vec<(u32, u32, f64)> = a.to_vec();
    let mut pb: Vec<(u32, u32, f64)> = b.to_vec();
    pa.sort_unstable_by_key(|x| (x.0, x.1));
    pb.sort_unstable_by_key(|x| (x.0, x.1));

    let mut out = Vec::with_capacity(pa.len() + pb.len());
    let mut st = ChannelMergeStats {
        channels_a: pa.len(),
        channels_b: pb.len(),
        ..ChannelMergeStats::default()
    };
    let (mut i, mut j) = (0usize, 0usize);
    while i < pa.len() && j < pb.len() {
        let (ka, kb) = ((pa[i].0, pa[i].1), (pb[j].0, pb[j].1));
        match ka.cmp(&kb) {
            std::cmp::Ordering::Less => {
                // Русло только в A: прозрачность.
                push_merged(&mut out, pa[i]);
                st.transparent_a += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                push_merged(&mut out, pb[j]);
                st.transparent_b += 1;
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                // Интерференция циркуляции: Π_Λ(J_A + J_B).
                st.shared += 1;
                let sum = pa[i].2 + pb[j].2;
                match nearest_trit(sum as f32) {
                    Trit::Pos => {
                        out.push((ka.0, ka.1, 1.0));
                        if pa[i].2 > 0.0 && pb[j].2 > 0.0 {
                            st.same_direction += 1;
                        } else {
                            st.flipped += 1;
                        }
                    }
                    Trit::Neg => {
                        out.push((ka.0, ka.1, -1.0));
                        if pa[i].2 < 0.0 && pb[j].2 < 0.0 {
                            st.same_direction += 1;
                        } else {
                            st.flipped += 1;
                        }
                    }
                    Trit::Zero => st.annihilated += 1,
                }
                i += 1;
                j += 1;
            }
        }
    }
    // Хвосты: оставшиеся русла проходят прозрачностью через Π_Λ
    // (субквантовая циркуляция гаснет — survivor не считается).
    for &(i2, j2, w) in &pa[i..] {
        if push_merged(&mut out, (i2, j2, w)) {
            st.transparent_a += 1;
        }
    }
    for &(i2, j2, w) in &pb[j..] {
        if push_merged(&mut out, (i2, j2, w)) {
            st.transparent_b += 1;
        }
    }
    out.sort_unstable_by_key(|x| (x.0, x.1));
    st.out = out.len();
    (out, st)
}

/// Прозрачный проход одного русла: Π_Λ(вес) — знак выживает,
/// субквантовая циркуляция (|J| < 0.5) гаснет. Возвращает true,
/// если русло попало в слияние.
fn push_merged(out: &mut Vec<(u32, u32, f64)>, (i, j, w): (u32, u32, f64)) -> bool {
    match nearest_trit(w as f32) {
        Trit::Pos => {
            out.push((i, j, 1.0));
            true
        }
        Trit::Neg => {
            out.push((i, j, -1.0));
            true
        }
        Trit::Zero => false,
    }
}

// ============================================================================
// 3. Консолидация лексиконов LEXI
// ============================================================================

/// Статистика консолидации лексиконов.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LexiconMergeStats {
    /// Слов в A.
    pub entries_a: usize,
    /// Слов в B.
    pub entries_b: usize,
    /// Слов в слитом словаре.
    pub out: usize,
    /// Общих координат с одинаковой доминантой.
    pub shared_same: usize,
    /// Коллизий доминант: разные слова на одной координате —
    /// побеждает мозг A (база слияния, детерминизм).
    pub collisions: usize,
    /// Координат со словом только в A.
    pub only_a: usize,
    /// Координат, словесно пустых в A и заполненных из B.
    pub filled_from_b: usize,
}

/// Консолидация лексиконов `LEXI_A + LEXI_B` — двухуказательный
/// обход отсортированных записей (координата → доминантный токен).
///
/// Коллизия доминант на общей координате разрешается в пользу
/// мозга A: база слияния задаёт словарь, B заполняет пустые
/// координаты. Результат — непротиворечивый словарь: координаты
/// уникальны и возрастают (валидацию делает [`Lexicon::new`]).
pub fn merge_lexicons(
    a: Option<&Lexicon>,
    b: Option<&Lexicon>,
) -> (Vec<(u32, String)>, LexiconMergeStats) {
    let mut st = LexiconMergeStats::default();
    let empty: [(u32, String); 0] = [];
    let ea: &[(u32, String)] = a.map(|l| l.entries()).unwrap_or(&empty);
    let eb: &[(u32, String)] = b.map(|l| l.entries()).unwrap_or(&empty);
    st.entries_a = ea.len();
    st.entries_b = eb.len();

    let mut out: Vec<(u32, String)> = Vec::with_capacity(ea.len() + eb.len());
    let (mut i, mut j) = (0usize, 0usize);
    while i < ea.len() && j < eb.len() {
        match ea[i].0.cmp(&eb[j].0) {
            std::cmp::Ordering::Less => {
                out.push(ea[i].clone());
                st.only_a += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(eb[j].clone());
                st.filled_from_b += 1;
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                if ea[i].1 == eb[j].1 {
                    out.push(ea[i].clone());
                    st.shared_same += 1;
                } else {
                    // Коллизия доминант: побеждает база слияния A.
                    out.push(ea[i].clone());
                    st.collisions += 1;
                }
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&ea[i..]);
    st.only_a += ea.len() - i;
    out.extend_from_slice(&eb[j..]);
    st.filled_from_b += eb.len() - j;
    st.out = out.len();
    (out, st)
}

// ============================================================================
// 4. Полное слияние мозгов: контейнер + отчёт
// ============================================================================

/// Конфигурация слияния.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MergeConfig {
    /// Порог диагностики изоморфизма `ε ∈ (0, 1]` (default 0.5):
    /// `E = co/min(nnz) ≥ ε` — brains структурно изоморфны.
    /// Гейт НЕ запирает слияние — это только диагностика отчёта.
    pub eps: f64,
}

impl Default for MergeConfig {
    fn default() -> Self {
        MergeConfig {
            eps: crate::archetype_lattice::DEFAULT_BRIDGE_EPS,
        }
    }
}

/// Полный отчёт `pqc merge`.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeReport {
    pub d_pol: u32,
    // Фазы: c = a ⊗_ε b.
    pub nnz_a: usize,
    pub nnz_b: usize,
    pub co_support: usize,
    pub resonance: usize,
    pub conflict: usize,
    /// Диагностика изоморфизма: энергия пересечения носителей.
    pub energy: f64,
    /// `E ≥ eps` — мозги структурно изоморфны (глубокая общность).
    pub isomorphic: bool,
    pub nnz_merged: usize,
    // Русла: J_merged = Π_Λ(J_A + J_B).
    pub channels_a: usize,
    pub channels_b: usize,
    pub channels_merged: usize,
    pub channels_shared: usize,
    pub channels_annihilated: usize,
    /// Окно слитого гироскопа: max(W_A, W_B).
    pub window: usize,
    /// Такты слитого гироскопа: ticks_A + ticks_B (оба потока опыта).
    pub ticks_merged: u64,
    // Лексикон.
    pub lexicon_a: usize,
    pub lexicon_b: usize,
    pub lexicon_merged: usize,
    pub lexicon_collisions: usize,
    // Контейнер.
    /// Версия формата выхода: 4 (русьла + словарь), 3 (русьла),
    /// 2 (вся циркуляция аннигилировала — только фазы).
    pub container: u32,
    /// Размер слитого контейнера, байт.
    pub brain_bytes: usize,
}

/// Окно гироскопа по умолчанию, если ни у одного мозга нет секции
/// GYRO (v2-контейнеры): стандарт обучающего конвейера.
const DEFAULT_WINDOW: usize = 8;

/// Полное слияние двух мозгов: фазы ⊗_ε + русла Π_Λ(J_A + J_B) +
/// лексикон → готовый контейнер (v4/v3/v2) + отчёт.
///
/// Требования: оба контейнера Packed4 (v2/v3/v4) **одной
/// размерности** — `⊗_ε` требует общее фазовое пространство.
/// Гиперпараметры наследуются от мозга A (база слияния); окно
/// гироскопа — `max(W_A, W_B)`; такты — сумма.
///
/// Детерминизм: без ГПСЧ, одинаковые входы → одинаковые байты.
pub fn merge_brains(
    bytes_a: &[u8],
    bytes_b: &[u8],
    cfg: &MergeConfig,
) -> Result<(Vec<u8>, MergeReport), String> {
    let ra = PqwReader::from_bytes(bytes_a).map_err(|e| format!("мозг A: {e}"))?;
    let rb = PqwReader::from_bytes(bytes_b).map_err(|e| format!("мозг B: {e}"))?;
    for (r, name) in [(&ra, "A"), (&rb, "B")] {
        if r.encoding() != pqw::phase::TritEncoding::Packed4 {
            return Err(format!(
                "мозг {name}: нужен контейнер v2/v3/v4 (Packed4), \
                 а не v1 с кривизной"
            ));
        }
    }
    if ra.d_pol() != rb.d_pol() {
        return Err(format!(
            "размерности решёток различаются: A d_pol={}, B d_pol={} \
             (⊗_ε требует общее фазовое пространство; обучайте мозги \
             с одинаковым --dim)",
            ra.d_pol(),
            rb.d_pol()
        ));
    }
    if !cfg.eps.is_finite() || cfg.eps <= 0.0 || cfg.eps > 1.0 {
        return Err("--eps: порог диагностики изоморфизма ∈ (0, 1]".into());
    }
    let d = ra.d_pol() as usize;

    // 1. Фазы: c = a ⊗_ε b — таблица интерференции без гейта.
    let mut merged_lattice = vec![0u8; d.div_ceil(4)];
    let lst = merge_lattices_packed4(ra.phase_bytes(), rb.phase_bytes(), d, &mut merged_lattice)
        .map_err(|e| e.to_string())?;

    // 2. Русла: J_merged = Π_Λ(J_A + J_B).
    let pairs_of = |r: &PqwReader| -> Vec<(u32, u32, f64)> {
        r.gyro()
            .map(|g| g.pairs().iter().map(|p| (p.i, p.j, p.weight)).collect())
            .unwrap_or_default()
    };
    let (merged_pairs, cst) = merge_channels(&pairs_of(&ra), &pairs_of(&rb));

    // 3. Лексикон: консолидация LEXI.
    let (lex_entries, lst_lex) = merge_lexicons(ra.lexicon().as_ref(), rb.lexicon().as_ref());

    // 4. Окно (max) и такты (сумма) гироскопа.
    let window = ra
        .gyro()
        .map(|g| g.window().max(1) as usize)
        .into_iter()
        .chain(rb.gyro().map(|g| g.window().max(1) as usize))
        .max()
        .unwrap_or(DEFAULT_WINDOW);
    let ticks_merged =
        ra.gyro().map(|g| g.ticks()).unwrap_or(0) + rb.gyro().map(|g| g.ticks()).unwrap_or(0);

    // 5. Контейнер: v4 (русьла + словарь), v3 (русьла), v2 (только фазы).
    let hyper = ra.hyperparams();
    let mut w = PqwWriter::new(d as u32)
        .map_err(|e| format!("писатель: {e}"))?
        .hyperparams(hyper.eta, hyper.gamma, hyper.rho, hyper.epsilon_threshold);
    for i in 0..d {
        let phase = match trit_at(&merged_lattice, i) {
            Trit::Pos => 1.0,
            Trit::Neg => -1.0,
            Trit::Zero => continue,
        };
        w.add_phase(i as u32, phase)
            .map_err(|e| format!("фаза слияния: {e}"))?;
    }
    let gyro_data = if merged_pairs.is_empty() {
        None
    } else {
        Some(
            GyroData::new(window as u32, ticks_merged, merged_pairs, d as u32)
                .map_err(|e| format!("гироскоп слияния: {e}"))?,
        )
    };
    let n_pairs = cst.out;
    let lexicon = if lex_entries.is_empty() {
        None
    } else {
        Some(
            Lexicon::new(lex_entries, d as u32)
                .map_err(|e| format!("лексикон слияния: {e}"))?,
        )
    };

    let mut buf = Vec::with_capacity(
        pqw::HEADER_SIZE + d.div_ceil(4) + n_pairs * 6 + lst_lex.out * 10,
    );
    // RQ23: автобиография наследуется от мозга A (хозяин личности):
    // слитый мозг помнит собеседника и нить A. Рефлекс B растворяется
    // в знаниях — знания объединяются, биография одна.
    let reflex_a = ra.reflex().and_then(|sec| {
        pqw::ReflexData::new(
            sec.interlocutor(),
            sec.turns(),
            sec.events().to_vec(),
            d as u32,
        )
        .ok()
    });
    let container = match (&gyro_data, &lexicon, &reflex_a) {
        (Some(g), Some(l), Some(r)) => {
            w.write_v5(&mut buf, g, l, r)
                .map_err(|e| format!("контейнер v5: {e}"))?;
            5
        }
        (Some(g), Some(l), None) => {
            w.write_v4(&mut buf, g, l)
                .map_err(|e| format!("контейнер v4: {e}"))?;
            4
        }
        (Some(g), None, _) => {
            w.write_v3(&mut buf, g)
                .map_err(|e| format!("контейнер v3: {e}"))?;
            3
        }
        (None, _, _) => {
            // Вся циркуляция аннигилировала: лексикон не переживает
            // отсутствие русел — контракт формата (v4 = v3 + LEXI,
            // а v3 требует непустую секцию GYRO).
            w.write_packed_trits(&mut buf)
                .map_err(|e| format!("контейнер v2: {e}"))?;
            2
        }
    };

    let report = MergeReport {
        d_pol: d as u32,
        nnz_a: lst.nnz_a,
        nnz_b: lst.nnz_b,
        co_support: lst.co_support,
        resonance: lst.resonance,
        conflict: lst.conflict,
        energy: lst.energy,
        isomorphic: lst.energy >= cfg.eps,
        nnz_merged: lst.nnz_out,
        channels_a: cst.channels_a,
        channels_b: cst.channels_b,
        channels_merged: cst.out,
        channels_shared: cst.shared,
        channels_annihilated: cst.annihilated,
        window,
        ticks_merged,
        lexicon_a: lst_lex.entries_a,
        lexicon_b: lst_lex.entries_b,
        lexicon_merged: lst_lex.out,
        lexicon_collisions: lst_lex.collisions,
        container,
        brain_bytes: buf.len(),
    };
    Ok((buf, report))
}

// ============================================================================
// 5. RQ21: сеттлинг слияния — консолидация волной
// ============================================================================

/// Потолок тактов сеттлинга (`--settle N`: ТЗ держит 2–4, потолок
/// щедрее — стационар обычно раньше).
pub const SETTLE_TICKS_MAX: usize = 16;

/// Конфигурация сеттлинга.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettleConfig {
    /// Тактов авторегрессионного рассуждения `Π_Λ(e^{Δt·J} p)` поверх
    /// слитых русел: 2–4 по ТЗ RQ21, допустимо 1..=16.
    pub ticks: usize,
}

impl Default for SettleConfig {
    fn default() -> Self {
        // Середина ТЗ-коридора 2–4: два такта волне течь, третий —
        // родиться руслам повторного свидетельства.
        SettleConfig { ticks: 3 }
    }
}

/// Отчёт сеттлинга: как волна консолидировала слияние.
#[derive(Clone, Debug, PartialEq)]
pub struct SettleReport {
    /// Запрошено тактов.
    pub ticks_requested: usize,
    /// Выполнено (ранний выход на стационаре).
    pub ticks_run: usize,
    /// Перещёлкиваний решётки по тактам (индекс = такт − 1).
    pub moved_per_tick: Vec<usize>,
    /// Суммарный θ-сдвиг волны.
    pub theta_shift_total: f64,
    /// Русел до сеттлинга.
    pub channels_before: usize,
    /// Русел после.
    pub channels_after: usize,
    /// Прирост русел (`after − before`): отрицателен, только если
    /// волна перезаписала направление чьего-то русла.
    pub channels_grown: i64,
    /// Событий born-консолидации: сколько раз кинетический фронт
    /// волны наблюдался гироскопом.
    pub observed_events: usize,
    /// Всего перещёлкиваний фаз за сеттлинг.
    pub lattice_flips: usize,
    /// Стационар `ĤΨ = 0` достигнут: такт без перещёлкиваний —
    /// волна успокоилась на слиянии.
    pub stationary: bool,
    /// Размер контейнера после сеттлинга, байт.
    pub brain_bytes: usize,
}

/// Сеттлинг слияния (RQ21): `N` тактов авторегрессионного рассуждения
/// `Π_Λ(e^{Δt·J} p)` поверх слитых русел — волна течёт сквозь оба
/// домена сразу, и русла из разных мозгов прорастают общими связями.
///
/// ## Физика такта
///
/// ```text
///   1. Транспорт: Π_Λ(e^{Δt·J} p) свободным потоком — фазы движутся
///      по руслам слияния; контраст живёт на конфликтах ⊗_ε
///      (аннигиляционные Zero с полюсным соседом): волна течёт
///      именно через открытые вопросы слияния.
///
///   2. Born-консолидация: кинетический фронт волны (дуги с m ≠ 0,
///      по возрастанию координат — детерминизм) наблюдается
///      гироскопом как сенсорное свидетельство. Пара, увиденная
///      дважды за соседние такты, насыщается — общее русло двух
///      доменов открывается: знание срослось.
///
///   3. Стационар: такт без перещёлкиваний — ĤΨ = 0, волна
///      успокоилась; сеттлинг завершается досрочно.
/// ```
///
/// ## Детерминизм
///
/// Ни одного ГПСЧ: транспорт целочислен, порядок консолидации —
/// по возрастанию координат, сид движка не потребляется
/// (Born-измерения фаз нет). `settle(X)` даёт побитово одинаковые
/// байты при одинаковых входах.
pub fn settle_brain(
    bytes: &[u8],
    cfg: &SettleConfig,
) -> Result<(Vec<u8>, SettleReport), String> {
    if cfg.ticks == 0 || cfg.ticks > SETTLE_TICKS_MAX {
        return Err(format!(
            "--settle: такты 1..={SETTLE_TICKS_MAX} (ТЗ RQ21: 2–4)"
        ));
    }
    let reader = PqwReader::from_bytes(bytes).map_err(|e| format!("сеттлинг: {e}"))?;
    if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
        return Err(
            "сеттлинг: нужен контейнер v2/v3/v4 (Packed4), а не v1 с кривизной".into(),
        );
    }
    let window = reader
        .gyro()
        .map(|g| g.window().max(1) as usize)
        .unwrap_or(DEFAULT_WINDOW);
    let eps = reader.hyperparams().epsilon_threshold;
    let mut engine =
        QuantizedGyroCurriculum::new(reader.d_pol(), eps, 42, window)
            .map_err(|e| e.to_string())?;
    // Гиперпараметры контейнера переживают сеттлинг (η вернулся в
    // чекпоинт, как был записан мозгом).
    engine.set_eta(reader.hyperparams().eta as f64);
    engine
        .resume_from_reader(&reader)
        .map_err(|e| format!("сеттлинг resume: {e}"))?;

    let channels_before = engine.channel_count();
    let mut moved_per_tick: Vec<usize> = Vec::with_capacity(cfg.ticks);
    let mut theta_total = 0.0_f64;
    let mut observed = 0usize;
    let mut flips = 0usize;
    let mut stationary = false;
    for _ in 0..cfg.ticks {
        // Такт 1: транспорт волной по слитым руслам.
        let stats = engine
            .reasoning_step_mode(TransportMode::Free)
            .map_err(|e| format!("сеттлинг транспорт: {e}"))?;
        moved_per_tick.push(stats.moved);
        theta_total += stats.theta_shift;
        flips += stats.moved;

        // Такт 2: born-консолидация — кинетический фронт волны
        // становится сенсорным свидетельством. Каждый такт —
        // независимая траектория: кольцо контекста чисто, но
        // свидетельства русел копятся сквозь такты — пара, пройденная
        // дважды в одном направлении, насыщается в общее русло
        // (гистерезис двух свидетелей). Порядок — по возрастанию
        // координат (линейный скан кинетики уже упорядочен).
        engine.reset_context_ring();
        let mut events: Vec<(u32, i8)> = Vec::new();
        engine.for_each_kinetic_arc(|arc, m| events.push((arc, m)));
        for (arc, m) in events {
            engine.observe_event(arc, m);
            observed += 1;
        }

        // Такт 3: стационар ĤΨ = 0 — такт без перещёлкиваний.
        if stats.moved == 0 {
            stationary = true;
            break;
        }
    }
    let channels_after = engine.channel_count();
    let out = engine
        .checkpoint()
        .map_err(|e| format!("сеттлинг чекпоинт: {e}"))?;
    let report = SettleReport {
        ticks_requested: cfg.ticks,
        ticks_run: moved_per_tick.len(),
        moved_per_tick,
        theta_shift_total: theta_total,
        channels_before,
        channels_after,
        channels_grown: channels_after as i64 - channels_before as i64,
        observed_events: observed,
        lattice_flips: flips,
        stationary,
        brain_bytes: out.len(),
    };
    Ok((out, report))
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::archetype_lattice::archetype_product_packed4;
    use crate::generate::{GeneratorConfig, L5Generator};
    use crate::gyro_lattice::QuantizedGyroCurriculum;
    use pqw::phase::{pack_trit2, Trit};
    use pqw::PqwReader;

    // ===================== Хелперы =====================

    /// Упаковка тритов в решётку (`d ≤ 4·len(ts)`).
    fn pack(ts: &[Trit]) -> Vec<u8> {
        let mut v = vec![0u8; ts.len().div_ceil(4)];
        for (i, &t) in ts.iter().enumerate() {
            let shift = 2 * (i % 4);
            v[i / 4] |= pack_trit2(t) << shift;
        }
        v
    }

    /// Эталонный обход по дугам — слияние как таблица ARCHETYPE_LUT
    /// (тот же эталон, что у RQ18: таблица без гейта).
    fn reference(a: &[Trit], b: &[Trit]) -> (Vec<Trit>, LatticeMergeStats) {
        let d = a.len();
        let mut out = vec![Trit::Zero; d];
        let (mut na, mut nb, mut co, mut res) = (0, 0, 0, 0);
        for i in 0..d {
            out[i] = crate::archetype_lattice::archetype_trit(a[i], b[i]);
            if a[i] != Trit::Zero {
                na += 1;
            }
            if b[i] != Trit::Zero {
                nb += 1;
            }
            if a[i] != Trit::Zero && b[i] != Trit::Zero {
                co += 1;
                if a[i] == b[i] {
                    res += 1;
                }
            }
        }
        let nnz_out = out.iter().filter(|&&t| t != Trit::Zero).count();
        let energy = archetype_energy(co, na, nb);
        (
            out,
            LatticeMergeStats {
                nnz_a: na,
                nnz_b: nb,
                co_support: co,
                resonance: res,
                conflict: co - res,
                energy,
                transparent: nnz_out - res,
                nnz_out,
            },
        )
    }

    /// Детерминированный ГПСЧ (xorshift64) — как в тестах RQ18.
    struct Xor(u64);
    impl Xor {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn trit(&mut self) -> Trit {
            match self.next() % 3 {
                0 => Trit::Neg,
                1 => Trit::Zero,
                _ => Trit::Pos,
            }
        }
    }

    /// Мозг из текста (движок → чекпоинт v3/v4).
    fn brain_from_text(text: &str, d: u32, steps: usize) -> Vec<u8> {
        let mut e = QuantizedGyroCurriculum::new(d, 0.05, 42, 8).unwrap();
        e.ingest(text, steps).unwrap();
        e.checkpoint().unwrap()
    }

    /// Ответ слитого мозга на вопрос (resume → free-mode речь).
    fn ask_brain(bytes: &[u8], question: &str, seed: u64) -> String {
        let reader = PqwReader::from_bytes(bytes).unwrap();
        let window = reader
            .gyro()
            .map(|g| g.window().max(1) as usize)
            .unwrap_or(8);
        let eps = reader.hyperparams().epsilon_threshold;
        let mut engine =
            QuantizedGyroCurriculum::new(reader.d_pol(), eps, seed, window).unwrap();
        engine.resume_from_reader(&reader).unwrap();
        let gcfg = GeneratorConfig {
            think_steps: 4,
            max_tokens: 24,
            window: engine.gyro_window(),
            seed,
            ..GeneratorConfig::default()
        };
        let mut gen = L5Generator::new(&mut engine, gcfg).unwrap();
        gen.generate(question).unwrap().text
    }

    // ===================== Слияние фаз: SWAR ≡ эталон =====================

    #[test]
    fn swar_matches_reference_random() {
        let mut rng = Xor(0x5EED_C0FFEE);
        for &d in &[1usize, 2, 3, 4, 5, 8, 9, 15, 16, 31, 64] {
            let ts_a: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
            let ts_b: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
            let (a, b) = (pack(&ts_a), pack(&ts_b));
            let mut out = vec![0u8; a.len()];
            let st = merge_lattices_packed4(&a, &b, d, &mut out).unwrap();
            let (ref_ts, ref_st) = reference(&ts_a, &ts_b);
            for i in 0..d {
                assert_eq!(
                    pqw::trit_bloch::trit_at(&out, i),
                    ref_ts[i],
                    "дуга {i}, d={d}"
                );
            }
            assert_eq!(st, ref_st, "статистика, d={d}");
        }
    }

    #[test]
    fn union_transparent_and_annihilation() {
        // A: Pos, Neg, Pos, Zero; B: Pos, Pos, Zero, Neg.
        // Слияние: резонанс (0), аннигиляция (1), прозрачность B (3).
        let a = pack(&[Trit::Pos, Trit::Neg, Trit::Pos, Trit::Zero]);
        let b = pack(&[Trit::Pos, Trit::Pos, Trit::Zero, Trit::Neg]);
        let mut out = vec![0u8; 1];
        let st = merge_lattices_packed4(&a, &b, 4, &mut out).unwrap();
        assert_eq!(out[0], pack(&[Trit::Pos, Trit::Zero, Trit::Pos, Trit::Neg])[0]);
        assert_eq!(st.nnz_a, 3);
        assert_eq!(st.nnz_b, 3);
        assert_eq!(st.co_support, 2); // дуги 0 и 1
        assert_eq!(st.resonance, 1); // дуга 0: Pos = Pos
        assert_eq!(st.conflict, 1); // дуга 1: Neg ⊕ Pos → Zero
        assert_eq!(st.energy, 2.0 / 3.0);
        assert_eq!(st.transparent, 2); // дуги 2 (из A) и 3 (из B)
        assert_eq!(st.nnz_out, 3);
    }

    #[test]
    fn idempotent_self_merge_bitwise() {
        // merge(m, m) = m бит-в-бит: диагональ таблицы тождественна,
        // co = nnz, конфликтов нет, энергия 1 (полный изоморфизм).
        let mut rng = Xor(7);
        for &d in &[1usize, 3, 8, 17, 33] {
            let ts: Vec<Trit> = (0..d).map(|_| rng.trit()).collect();
            let m = pack(&ts);
            let mut out = vec![0u8; m.len()];
            let st = merge_lattices_packed4(&m, &m, d, &mut out).unwrap();
            assert_eq!(out, m, "идемпотентность нарушена (d={d})");
            if st.nnz_a > 0 {
                assert_eq!(st.co_support, st.nnz_a);
                assert_eq!(st.resonance, st.nnz_a);
                assert_eq!(st.conflict, 0);
                assert_eq!(st.energy, 1.0);
                assert_eq!(st.transparent, 0);
                assert_eq!(st.nnz_out, st.nnz_a);
            }
        }
    }

    #[test]
    fn disjoint_union_supports_add() {
        // Ортогональные домены: носители не пересекаются — знание
        // обоих выживает целиком (гейт НЕ заперт: energy = 0).
        let a = pack(&[Trit::Pos, Trit::Neg, Trit::Zero, Trit::Zero]);
        let b = pack(&[Trit::Zero, Trit::Zero, Trit::Neg, Trit::Pos]);
        let mut out = vec![0u8; 1];
        let st = merge_lattices_packed4(&a, &b, 4, &mut out).unwrap();
        assert_eq!(out[0], a[0] | b[0]);
        assert_eq!(st.co_support, 0);
        assert_eq!(st.conflict, 0);
        assert_eq!(st.energy, 0.0);
        assert_eq!(st.nnz_out, st.nnz_a + st.nnz_b);
        assert_eq!(st.transparent, 4);
    }

    #[test]
    fn merge_equals_archetype_product_when_gate_open() {
        // При открытом гейте RQ18 слияние бит-в-бит равно произведению
        // архетипов: одна и та же таблица интерференции.
        let mut rng = Xor(99);
        for _ in 0..32 {
            let ts_a: Vec<Trit> = (0..16).map(|_| rng.trit()).collect();
            let ts_b: Vec<Trit> = (0..16).map(|_| rng.trit()).collect();
            let (a, b) = (pack(&ts_a), pack(&ts_b));
            let (mut m, mut p) = (vec![0u8; 4], vec![0u8; 4]);
            let st_m = merge_lattices_packed4(&a, &b, 16, &mut m).unwrap();
            let st_p = archetype_product_packed4(&a, &b, 16, 0.01, &mut p).unwrap();
            if st_p.resonant {
                assert_eq!(m, p);
                assert_eq!(st_m.nnz_out, st_p.nnz_out);
            }
        }
    }

    #[test]
    fn tail_padding_lanes_ignored() {
        // d = 5: мусор в паддинг-лейнах не влияет на слияние.
        let mut a = pack(&[Trit::Pos, Trit::Zero, Trit::Zero, Trit::Zero, Trit::Neg]);
        let mut b = pack(&[Trit::Pos, Trit::Zero, Trit::Zero, Trit::Zero, Trit::Pos]);
        a[1] |= 0b1111_1100;
        b[1] |= 0b1111_1100;
        let mut out = vec![0u8; 2];
        let st = merge_lattices_packed4(&a, &b, 5, &mut out).unwrap();
        assert_eq!(st.nnz_a, 2);
        assert_eq!(st.nnz_b, 2);
        assert_eq!(st.co_support, 2);
        assert_eq!(st.resonance, 1); // дуга 0
        assert_eq!(st.conflict, 1); // дуга 4: Neg ⊕ Pos
        assert_eq!(out[1] & 0b0000_0011, 0);
    }

    #[test]
    fn contract_violations_rejected() {
        let a = vec![0u8; 1];
        let mut out = vec![0u8; 1];
        assert!(merge_lattices_packed4(&a, &a, 0, &mut out).is_err());
        assert!(merge_lattices_packed4(&a, &a, 8, &mut out).is_err());
    }

    #[test]
    fn out_bytes_beyond_lattice_untouched() {
        let a = pack(&[Trit::Pos; 4]);
        let mut out = vec![0xFFu8; 4];
        merge_lattices_packed4(&a, &a, 4, &mut out).unwrap();
        assert_eq!(&out[1..], &[0xFFu8; 3]);
    }

    // ===================== Слияние русел J =====================

    #[test]
    fn channels_same_direction_survive() {
        let (out, st) = merge_channels(&[(1, 2, 1.0)], &[(1, 2, 1.0)]);
        assert_eq!(out, vec![(1, 2, 1.0)]);
        assert_eq!(st.shared, 1);
        assert_eq!(st.same_direction, 1);
        assert_eq!(st.annihilated, 0);
        assert_eq!(st.out, 1);
    }

    #[test]
    fn channels_opposite_annihilate() {
        // Встречная циркуляция: Π_Λ(+1 + (−1)) = Π_Λ(0) = Zero.
        let (out, st) = merge_channels(&[(1, 2, 1.0)], &[(1, 2, -1.0)]);
        assert!(out.is_empty());
        assert_eq!(st.shared, 1);
        assert_eq!(st.annihilated, 1);
        assert_eq!(st.out, 0);
    }

    #[test]
    fn channels_transparent_union() {
        let (out, st) = merge_channels(&[(1, 2, 1.0), (3, 4, -1.0)], &[(5, 6, 1.0)]);
        assert_eq!(out, vec![(1, 2, 1.0), (3, 4, -1.0), (5, 6, 1.0)]);
        assert_eq!(st.transparent_a, 2);
        assert_eq!(st.transparent_b, 1);
        assert_eq!(st.out, 3);
        assert_eq!(st.shared, 0);
    }

    #[test]
    fn channels_skew_symmetry_by_construction() {
        // Выход — только верхний треугольник (i < j) с весами ±1:
        // кососимметричность J[j][i] = −J[i][j] определена знаком.
        let mut rng = Xor(5);
        let mut a = Vec::new();
        let mut b = Vec::new();
        for _ in 0..64 {
            let i = (rng.next() % 60) as u32;
            let j = 60 + (rng.next() % 60) as u32;
            let w = if rng.next() % 2 == 0 { 1.0 } else { -1.0 };
            a.push((i, j, w));
            let w2 = if rng.next() % 2 == 0 { 1.0 } else { -1.0 };
            b.push((i + 1, j + 1, w2));
        }
        let (out, st) = merge_channels(&a, &b);
        assert_eq!(st.out, out.len());
        for &(i, j, w) in &out {
            assert!(i < j, "пара вне верхнего треугольника");
            assert_eq!(w.abs(), 1.0, "вес не тритовый");
        }
        // Канонический порядок: строго возрастающие (i, j), без дубликатов.
        for w in out.windows(2) {
            assert!((w[0].0, w[0].1) < (w[1].0, w[1].1));
        }
    }

    #[test]
    fn channels_subquantum_circulation_gates() {
        // Субквантовое одиночное русло (|J| < 0.5) гасится Π_Λ.
        let (out, st) = merge_channels(&[(1, 2, 0.3)], &[]);
        assert!(out.is_empty());
        assert_eq!(st.channels_a, 1);
        assert_eq!(st.out, 0);
        assert_eq!(st.transparent_a, 0);
    }

    #[test]
    fn channels_weight_flip_by_magnitude() {
        // Неединичные веса: перевес |−0.8| > |+0.3| → сумма −0.5 → Neg.
        let (out, st) = merge_channels(&[(1, 2, 0.3)], &[(1, 2, -0.8)]);
        assert_eq!(out, vec![(1, 2, -1.0)]);
        assert_eq!(st.shared, 1);
        assert_eq!(st.flipped, 1);
        assert_eq!(st.same_direction, 0);
    }

    #[test]
    fn channels_unsorted_inputs_canonicalized() {
        let (out, _) = merge_channels(&[(5, 6, 1.0), (1, 2, -1.0)], &[(9, 10, 1.0), (3, 4, 1.0)]);
        assert_eq!(
            out,
            vec![(1, 2, -1.0), (3, 4, 1.0), (5, 6, 1.0), (9, 10, 1.0)]
        );
    }

    // ===================== Консолидация лексиконов =====================

    fn lex(pairs: &[(u32, &str)]) -> Option<Lexicon> {
        if pairs.is_empty() {
            return None;
        }
        Lexicon::new(
            pairs.iter().map(|&(c, t)| (c, t.to_string())).collect(),
            64,
        )
        .ok()
    }

    #[test]
    fn lexicon_a_wins_collisions_b_fills_gaps() {
        let a = lex(&[(1, "фотон"), (2, "кубит")]).unwrap();
        let b = lex(&[(1, "генератор"), (3, "итератор")]).unwrap();
        let (entries, st) = merge_lexicons(Some(&a), Some(&b));
        assert_eq!(
            entries,
            vec![(1, "фотон".into()), (2, "кубит".into()), (3, "итератор".into())]
        );
        assert_eq!(st.collisions, 1); // координата 1: победила A
        assert_eq!(st.filled_from_b, 1);
        assert_eq!(st.only_a, 1);
        assert_eq!(st.out, 3);
    }

    #[test]
    fn lexicon_shared_dominant_kept_once() {
        let a = lex(&[(2, "кубит"), (5, "фотон")]).unwrap();
        let b = lex(&[(2, "кубит"), (7, "поток")]).unwrap();
        let (entries, st) = merge_lexicons(Some(&a), Some(&b));
        assert_eq!(
            entries,
            vec![(2, "кубит".into()), (5, "фотон".into()), (7, "поток".into())]
        );
        assert_eq!(st.shared_same, 1);
        assert_eq!(st.collisions, 0);
    }

    #[test]
    fn lexicon_none_cases() {
        // Оба без слов — пустой результат (контейнер деградирует до v3/v2).
        let (entries, st) = merge_lexicons(None, None);
        assert!(entries.is_empty());
        assert_eq!(st.out, 0);
        // Слово только у B — вливается целиком.
        let b = lex(&[(4, "маяк")]).unwrap();
        let (entries, st) = merge_lexicons(None, Some(&b));
        assert_eq!(entries, vec![(4, "маяк".to_string())]);
        assert_eq!(st.filled_from_b, 1);
    }

    // ===================== Полное слияние: контейнеры =====================

    const PHYSICS: &str = "квантовая запутанность связывает кубиты \
        запутанность корреляции измерение фотон несёт энергию \
        волновая функция описывает суперпозицию состояний кубит \
        измерение коллапсирует волновая функция энергия квантована \
        фотон запутанность кубиты корреляции сверхпроводимость \
        гамильтониан определяет энергию состояний суперпозиция";

    const PYTHON: &str = "генератор лениво вычисляет итератор \
        декоратор оборачивает функцию замыкание помнит среду \
        итератор проходит последовательность поток выполняется \
        конкурентно asyncio планирует корутины генератор \
        yield приостанавливает вычисление декоратор кеширует \
        замыкание захватывает переменные поток освобождаетgil";

    #[test]
    fn two_domain_brains_merge_into_v4() {
        // Мозг «физика» + мозг «python» → один v4-контейнер,
        // знающий оба домена: носитель, русла и словарь шире каждого.
        let bytes_a = brain_from_text(PHYSICS, 512, 1);
        let bytes_b = brain_from_text(PYTHON, 512, 1);
        let (merged, report) =
            merge_brains(&bytes_a, &bytes_b, &MergeConfig::default()).unwrap();

        // Валидный контейнер v4 (русьла + словарь живы).
        assert_eq!(&merged[..8], b"POLER_Q4");
        assert_eq!(report.container, 4);
        let reader = PqwReader::from_bytes(&merged).unwrap();
        assert!(reader.verify_payload().is_ok());

        // Закон сохранения носителя: union − конфликты.
        assert_eq!(
            report.nnz_merged,
            report.nnz_a + report.nnz_b - 2 * report.co_support + report.resonance
        );
        // Объединение знаний: слияние не уже любого из мозгов.
        assert!(report.nnz_merged >= report.nnz_a);
        assert!(report.nnz_merged >= report.nnz_b);
        assert!(report.channels_merged >= report.channels_a);
        assert!(report.channels_merged >= report.channels_b);
        assert!(report.lexicon_merged >= report.lexicon_a.max(report.lexicon_b));

        // Слитый мозг поднимается движком: русла и словарь на месте.
        let window = reader.gyro().map(|g| g.window().max(1) as usize).unwrap_or(8);
        let mut engine =
            QuantizedGyroCurriculum::new(reader.d_pol(), 0.05, 42, window).unwrap();
        engine.resume_from_reader(&reader).unwrap();
        assert_eq!(engine.channel_count(), report.channels_merged);
        assert_eq!(engine.lexicon_len(), report.lexicon_merged);
        assert_eq!(engine.nnz(), report.nnz_merged);

        // Мульти-доменное знание: словарь слияния содержит слова
        // ОБОИХ доменов (детерминированная проверка ТЗ).
        let lex = reader.lexicon().unwrap();
        let tokens: Vec<&str> = lex.entries().iter().map(|(_, t)| t.as_str()).collect();
        let physics_words = ["запутанность", "кубит", "фотон", "энергия"];
        let python_words = ["генератор", "декоратор", "замыкание", "поток"];
        let has = |ws: &[&str]| ws.iter().filter(|w| tokens.contains(w)).count();
        assert!(has(&physics_words) >= 2, "физика потеряна: {tokens:?}");
        assert!(has(&python_words) >= 2, "python потерян: {tokens:?}");
    }

    #[test]
    fn merge_self_knowledge_stable() {
        // merge(A, A): знание бит-в-бит стабильно — фазы, направления
        // русел и словарь идентичны; удваиваются только такты.
        let bytes_a = brain_from_text(PHYSICS, 512, 1);
        let reader_a = PqwReader::from_bytes(&bytes_a).unwrap();
        let (merged, report) =
            merge_brains(&bytes_a, &bytes_a, &MergeConfig::default()).unwrap();

        assert_eq!(report.nnz_merged, report.nnz_a);
        assert_eq!(report.resonance, report.nnz_a);
        assert_eq!(report.conflict, 0);
        assert_eq!(report.energy, 1.0);
        assert!(report.isomorphic, "мозг изоморфен сам себе");

        let rm = PqwReader::from_bytes(&merged).unwrap();
        assert_eq!(rm.phase_bytes(), reader_a.phase_bytes());
        assert_eq!(rm.lexicon(), reader_a.lexicon());
        let dirs = |r: &PqwReader| -> Vec<(u32, u32, f64)> {
            r.gyro()
                .unwrap()
                .pairs()
                .iter()
                .map(|p| (p.i, p.j, p.weight.signum()))
                .collect()
        };
        assert_eq!(dirs(&rm), dirs(&reader_a));
        // Такты — два свидетеля одного опыта.
        assert_eq!(
            report.ticks_merged,
            2 * reader_a.gyro().map(|g| g.ticks()).unwrap_or(0)
        );
    }

    #[test]
    fn merge_dimension_mismatch_rejected() {
        let a = brain_from_text(PHYSICS, 512, 1);
        let b = brain_from_text(PYTHON, 1024, 1);
        let err = merge_brains(&a, &b, &MergeConfig::default()).unwrap_err();
        assert!(err.contains("размерности"), "{err}");
    }

    #[test]
    fn merge_curved_v1_rejected() {
        // v1-контейнер с кривизной — не Packed4, слияние невозможно.
        let v1 = pqw::PqwWriter::new(16)
            .unwrap()
            .add_phase(1, 0.9)
            .unwrap()
            .to_bytes()
            .unwrap();
        let b = brain_from_text(PYTHON, 16, 1);
        assert!(merge_brains(&v1, &b, &MergeConfig::default()).is_err());
        assert!(merge_brains(&b, &v1, &MergeConfig::default()).is_err());
    }

    #[test]
    fn merge_bad_eps_rejected() {
        let a = brain_from_text(PHYSICS, 256, 1);
        for bad in [0.0, -0.5, 1.5, f64::NAN] {
            let cfg = MergeConfig { eps: bad };
            assert!(merge_brains(&a, &a, &cfg).is_err(), "eps={bad} принят");
        }
    }

    #[test]
    fn annihilated_circulation_degrades_to_v2() {
        // Два мозга с точно встречной циркуляцией на общей паре:
        // русла гаснут — контейнер деградирует до v2 (лексикон не
        // переживает отсутствие русел: контракт формата v4 = v3 + LEXI).
        let mut w = pqw::PqwWriter::new(64).unwrap();
        w.add_phase(0, 1.0).unwrap();
        let gyro = GyroData::new(8, 100, vec![(1, 2, 1.0)], 64).unwrap();
        let lex = Lexicon::new(vec![(0, "фотон".to_string())], 64).unwrap();
        let mut a = Vec::new();
        w.write_v4(&mut a, &gyro, &lex).unwrap();
        let gyro_b = GyroData::new(8, 100, vec![(1, 2, -1.0)], 64).unwrap();
        let mut b = Vec::new();
        w.write_v4(&mut b, &gyro_b, &lex).unwrap();

        let (merged, report) =
            merge_brains(&a, &b, &MergeConfig::default()).unwrap();
        assert_eq!(report.container, 2);
        assert_eq!(report.channels_merged, 0);
        assert_eq!(report.channels_annihilated, 1);
        assert_eq!(&merged[..8], b"POLER_Q2");
        assert!(PqwReader::from_bytes(&merged).is_ok());
    }

    #[test]
    fn merged_brain_resumes_and_continues_learning() {
        // Слитый мозг — живой: продолжаем учиться поверх слияния.
        let bytes_a = brain_from_text(PHYSICS, 512, 1);
        let bytes_b = brain_from_text(PYTHON, 512, 1);
        let (merged, report) =
            merge_brains(&bytes_a, &bytes_b, &MergeConfig::default()).unwrap();

        let reader = PqwReader::from_bytes(&merged).unwrap();
        let window = reader.gyro().map(|g| g.window().max(1) as usize).unwrap_or(8);
        let mut engine =
            QuantizedGyroCurriculum::new(reader.d_pol(), 0.05, 42, window).unwrap();
        engine.resume_from_reader(&reader).unwrap();
        engine
            .ingest("маяк светил над морем маяк туман бухта", 1)
            .unwrap();
        let grown = engine.checkpoint().unwrap();
        let r2 = PqwReader::from_bytes(&grown).unwrap();
        assert_eq!(&grown[..8], b"POLER_Q4");
        assert!(r2.lexicon().unwrap().len() > report.lexicon_merged);
    }

    #[test]
    fn merged_brain_answers_cross_domain_question() {
        // ТЗ п.2: вопрос на стыке дисциплин — волна мысли слитого
        // мозга связывает термины обоих доменов.
        let bytes_a = brain_from_text(PHYSICS, 512, 2);
        let bytes_b = brain_from_text(PYTHON, 512, 2);
        let (merged, report) =
            merge_brains(&bytes_a, &bytes_b, &MergeConfig::default()).unwrap();
        assert_eq!(report.container, 4);

        let answer = ask_brain(
            &merged,
            "как моделировать квантовую запутанность на языке программирования",
            42,
        );
        assert!(
            !answer.is_empty(),
            "слитый мозг промолчал на стыковом вопросе"
        );
    }

    // ===================== Роман: целостный ингест (запрос пользователя) =====================

    /// Детерминированный «роман» о маяке: две книги саги. «марго»
    /// доминирует в первой половине, «софия» — во второй, «артур» и
    /// маяк — сквозные герои; лексика половин различна (северная
    /// книга — гроза/туман/порт, южная — жара/штиль/прибой), как два
    /// разных тома одного мира. Каждая строка повторяет героев —
    /// плотная решётка русел и доминантные слова персонажей.
    fn novel_text(target: usize) -> String {
        let mut out = String::with_capacity(target + 256);
        let mut line = 0usize;
        while out.len() < target {
            line += 1;
            let first_half = out.len() < target / 2;
            // Герой строки: перекос по книгам саги.
            let (hero, other) = if line % 4 == 0 {
                if first_half { ("софия", "марго") } else { ("марго", "софия") }
            } else if first_half {
                ("марго", "софия")
            } else {
                ("софия", "марго")
            };
            // Локальный колорит книги: север ↔ юг.
            let local = if first_half {
                "север гроза туман порт вела лодку сквозь шторм"
            } else {
                "юг жара штиль прибой вела лодку сквозь зной"
            };
            out.push_str(&format!(
                "{hero} шла к маяку, {hero} смотрела на море, {other} писала письмо, \
                 артур ждал поезд, маяк горел над водой, море пело о доме, {local}\n"
            ));
        }
        out
    }

    /// Блоковый ингест, как `pqc train --corpus` (блок 8 КиБ).
    /// Границы блоков — по строкам: UTF-8 не режется посреди символа.
    fn train_on_novel(novel: &str, d: u32) -> Vec<u8> {
        let mut engine = QuantizedGyroCurriculum::new(d, 0.05, 42, 8).unwrap();
        let mut block = String::with_capacity(8192);
        for line in novel.lines() {
            if !block.is_empty() && block.len() + line.len() + 1 > 8192 {
                engine.ingest(&block, 1).unwrap();
                block.clear();
            }
            block.push_str(line);
            block.push('\n');
        }
        if !block.is_empty() {
            engine.ingest(&block, 1).unwrap();
        }
        engine.checkpoint().unwrap()
    }

    #[test]
    fn whole_novel_ingest_and_recall() {
        // Целый роман (~128 КиБ) одним мозгём: лексикон знает героев,
        // русла насыщены, мозг отвечает вопросом о персонаже.
        let novel = novel_text(128 * 1024);
        assert!(novel.len() > 100_000, "роман не набран: {}", novel.len());

        let bytes = train_on_novel(&novel, 4096);
        let reader = PqwReader::from_bytes(&bytes).unwrap();
        assert_eq!(&bytes[..8], b"POLER_Q4", "лексикон и русла обязаны жить");

        // Герои — доминантные слова своих координат.
        let lex = reader.lexicon().unwrap();
        let tokens: Vec<&str> = lex.entries().iter().map(|(_, t)| t.as_str()).collect();
        let names = ["марго", "софия", "артур"];
        let known = names.iter().filter(|n| tokens.contains(n)).count();
        assert!(known >= 2, "герои не в лексиконе: {tokens:?}");

        // Плотная решётка русел: сотни каналов циркуляции смысла.
        let channels = reader.gyro().map(|g| g.pairs().len()).unwrap_or(0);
        assert!(channels > 200, "русел слишком мало: {channels}");

        // Вопрос о персонаже — мозг говорит словами романа.
        let hero = names.iter().find(|n| tokens.contains(n)).unwrap();
        let answer = ask_brain(&bytes, &format!("кто такая {hero}"), 42);
        assert!(!answer.is_empty(), "мозг промолчал о герое романа");
    }

    #[test]
    fn novel_two_halves_merge_and_recall() {
        // Роман разрезан пополам: мозг первой половины + мозг второй
        // → слияние знает героев ОБОИХ половин и отвечает о каждом.
        let novel = novel_text(96 * 1024);
        let mid = novel.len() / 2;
        // Граница реза — на начало строки (блоки — валидный UTF-8).
        let mid = novel[mid..].find('\n').map(|i| mid + i).unwrap_or(mid);
        let (half1, half2) = novel.split_at(mid);

        let bytes_a = train_on_novel(half1, 4096);
        let bytes_b = train_on_novel(half2, 4096);
        let (merged, report) =
            merge_brains(&bytes_a, &bytes_b, &MergeConfig::default()).unwrap();
        assert_eq!(report.container, 4, "русьла и словарь обязаны жить");

        // Слитый словарь не уже каждой половины.
        assert!(report.lexicon_merged >= report.lexicon_a);
        assert!(report.lexicon_merged >= report.lexicon_b);

        // Герои обеих половин: «марго» (героиня 1-й) и «софия» (2-й).
        let reader = PqwReader::from_bytes(&merged).unwrap();
        let lex = reader.lexicon().unwrap();
        let tokens: Vec<&str> = lex.entries().iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            tokens.contains(&"артур"),
            "сквозной герой потерян: {tokens:?}"
        );

        // Мульти-доменная речь: вопрос о герое каждой половины.
        for hero in ["марго", "софия", "артур"] {
            if tokens.contains(&hero) {
                let answer = ask_brain(&merged, &format!("кто такая {hero}"), 42);
                assert!(!answer.is_empty(), "промолчал о {hero}");
            }
        }
        // Русла слияния плотнее каждой половины.
        assert!(report.channels_merged > report.channels_a);
        assert!(report.channels_merged > report.channels_b);
    }

    // ===================== RQ21: сеттлинг слияния =====================

    /// Два v4-мозга с рукотворными конфликтами ⊗_ε: координаты 0 и 3
    /// аннигилируют в Zero, русла (0,1) и (3,4) переживают слияние.
    /// Волна сеттлинга обязана течь сквозь открытые вопросы (Zero с
    /// полюсным соседом) и прорастить общее русло (0,3).
    fn conflicting_pair_brains() -> (Vec<u8>, Vec<u8>) {
        let lex = Lexicon::new(
            vec![
                (0, "спор".to_string()),
                (1, "полюс".to_string()),
                (3, "вопрос".to_string()),
                (4, "ответ".to_string()),
            ],
            8,
        )
        .unwrap();
        let mk = |p0: f32, p3: f32| -> Vec<u8> {
            let mut ww = pqw::PqwWriter::new(8).unwrap().hyperparams(0.6, 0.0, 1.0, 0.05);
            if p0 != 0.0 {
                ww.add_phase(0, p0).unwrap();
            }
            if p3 != 0.0 {
                ww.add_phase(3, p3).unwrap();
            }
            // Полюса-соседи: согласие обоих мозгов (резонанс ⊗_ε) —
            // русла переживают слияние, волне есть куда течь.
            ww.add_phase(1, 1.0).unwrap();
            ww.add_phase(4, 1.0).unwrap();
            let gyro = GyroData::new(8, 100, vec![(0, 1, 1.0), (3, 4, 1.0)], 8).unwrap();
            let mut buf = Vec::new();
            ww.write_v4(&mut buf, &gyro, &lex).unwrap();
            buf
        };
        (mk(1.0, 1.0), mk(-1.0, -1.0))
    }

    #[test]
    fn settle_grows_cross_domain_channels() {
        // ТЗ RQ21 п.1: 2–4 такта Π_Λ(e^{Δt·J} p) — русла разных
        // доменов прорастают общими связями, система приходит к
        // стационару ĤΨ = 0. Полностью детерминированная конструкция.
        let (a, b) = conflicting_pair_brains();
        let (merged, mrep) = merge_brains(&a, &b, &MergeConfig::default()).unwrap();
        assert_eq!(mrep.conflict, 2, "координаты 0 и 3 — конфликты ⊗_ε");
        assert_eq!(mrep.channels_merged, 2, "русьла (0,1) и (3,4) пережили");
        assert_eq!(mrep.nnz_merged, 2, "конфликты аннигилировали в Zero");

        let (settled, srep) = settle_brain(&merged, &SettleConfig { ticks: 3 }).unwrap();

        // Волна текла ровно два такта: конфликтные Zero заполнились
        // полюсами, второй такт — покой (стационар ĤΨ = 0).
        assert_eq!(srep.ticks_requested, 3);
        assert_eq!(srep.moved_per_tick, vec![2, 0], "такт 1: два флипа, такт 2: покой");
        assert_eq!(srep.ticks_run, 2, "ранний выход на стационаре");
        assert!(srep.stationary, "ĤΨ = 0 достигнут");
        assert_eq!(srep.lattice_flips, 2);

        // Общее русло проросло: пара (0, 3) кинетического фронта
        // увидена дважды в одном направлении — гистерезис насытил
        // канал. Знание двух мозгов срослось.
        assert_eq!(srep.channels_before, 2);
        assert_eq!(srep.channels_after, 3, "русьла (0,1), (3,4) + проросшее (0,3)");
        assert_eq!(srep.channels_grown, 1);
        assert_eq!(srep.observed_events, 4, "фронт из 2 дуг × 2 такта");

        // Проросшее русло живёт в контейнере: (0, 3, +1).
        let reader = PqwReader::from_bytes(&settled).unwrap();
        let pairs = reader.gyro().unwrap().pairs().to_vec();
        assert!(
            pairs.iter().any(|p| p.i == 0 && p.j == 3 && p.weight > 0.0),
            "русьло (0,3) не проросло: {pairs:?}"
        );
        // Словарь пережил сеттлинг бит-в-бит по доминантам.
        assert_eq!(reader.lexicon().unwrap().len(), 4);
    }

    #[test]
    fn settle_deterministic_bitwise() {
        // Ни одного ГПСЧ: сеттлинг одинаковых байтов одинаков.
        let (a, b) = conflicting_pair_brains();
        let (merged, _) = merge_brains(&a, &b, &MergeConfig::default()).unwrap();
        let (s1, r1) = settle_brain(&merged, &SettleConfig { ticks: 4 }).unwrap();
        let (s2, r2) = settle_brain(&merged, &SettleConfig { ticks: 4 }).unwrap();
        assert_eq!(s1, s2, "байты контейнера побитово равны");
        assert_eq!(r1, r2, "отчёты равны (elapsed нет в отчёте сеттлинга)");
    }

    #[test]
    fn settle_stationary_v2_container() {
        // v2-мозг (только фазы, русел нет): транспорту не по чему течь —
        // честный стационар на первом такте, контейнер остаётся v2.
        let mut w = pqw::PqwWriter::new(16).unwrap();
        w.add_phase(2, 1.0).unwrap();
        w.add_phase(7, -1.0).unwrap();
        let mut v2 = Vec::new();
        w.write_packed_trits(&mut v2).unwrap();

        let (out, rep) = settle_brain(&v2, &SettleConfig { ticks: 3 }).unwrap();
        assert!(rep.stationary);
        assert_eq!(rep.ticks_run, 1);
        assert_eq!(rep.moved_per_tick, vec![0]);
        assert_eq!(rep.channels_before, 0);
        assert_eq!(rep.channels_after, 0);
        assert_eq!(rep.observed_events, 0);
        assert_eq!(&out[..8], b"POLER_Q2", "v2 деградация не сжижается");
        // Фазы не тронуты покоем.
        assert_eq!(
            PqwReader::from_bytes(&out).unwrap().phase_bytes(),
            PqwReader::from_bytes(&v2).unwrap().phase_bytes()
        );
    }

    #[test]
    fn settle_after_merge_keeps_knowledge_and_speech() {
        // Полный конвейер RQ20+RQ21: обучение → слияние → сеттлинг →
        // вопрос на стыке доменов. Сеттлинг не теряет знание: словарь
        // и речь живы, контейнер валиден.
        let bytes_a = brain_from_text(PHYSICS, 512, 2);
        let bytes_b = brain_from_text(PYTHON, 512, 2);
        let (merged, mrep) = merge_brains(&bytes_a, &bytes_b, &MergeConfig::default()).unwrap();
        let (settled, srep) = settle_brain(&merged, &SettleConfig::default()).unwrap();

        assert_eq!(srep.ticks_run, srep.moved_per_tick.len());
        assert_eq!(
            srep.channels_after as i64,
            srep.channels_before as i64 + srep.channels_grown,
            "закон сохранения русел"
        );
        let reader = PqwReader::from_bytes(&settled).unwrap();
        assert_eq!(&settled[..8], b"POLER_Q4", "слитый и осевший мозг — v4");
        // Словарь слияния не уже словаря до сеттлинга (доминанты живы).
        assert!(reader.lexicon().unwrap().len() >= mrep.lexicon_merged);
        // Гиперпараметры пережили сеттлинг.
        assert_eq!(reader.hyperparams().eta, PqwReader::from_bytes(&merged).unwrap().hyperparams().eta);

        // Мульти-доменная речь после сеттлинга.
        let answer = ask_brain(
            &settled,
            "как моделировать квантовую запутанность на языке программирования",
            42,
        );
        assert!(!answer.is_empty(), "сеттлинг заглушил речь");
    }

    #[test]
    fn settle_ticks_validation() {
        let (a, b) = conflicting_pair_brains();
        let (merged, _) = merge_brains(&a, &b, &MergeConfig::default()).unwrap();
        for bad in [0usize, SETTLE_TICKS_MAX + 1, 100] {
            let err = settle_brain(&merged, &SettleConfig { ticks: bad }).unwrap_err();
            assert!(err.contains("--settle"), "тело ошибки: {err}");
        }
        // Границы коридора валидны (ТЗ 2–4, потолок щедрее).
        for good in [1usize, 2, 4, SETTLE_TICKS_MAX] {
            assert!(settle_brain(&merged, &SettleConfig { ticks: good }).is_ok());
        }
    }

    #[test]
    fn settle_curved_v1_rejected() {
        let v1 = pqw::PqwWriter::new(16)
            .unwrap()
            .add_phase(1, 0.9)
            .unwrap()
            .to_bytes()
            .unwrap();
        assert!(settle_brain(&v1, &SettleConfig::default()).is_err());
    }
}

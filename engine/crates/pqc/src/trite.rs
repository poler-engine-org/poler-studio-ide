//! Крипто-схема алгебры архетипа на тритах GF(3) (RQ13) — прецессионный
//! шифр уравнения файл 285 в дискретной фазовой алгебре.
//!
//! Уравнение то же, что и в RQ12: `p* = a ⊗_ε p* ⊕ m`, восстановление
//! `m = p* ⊕ (a ⊗_ε p*)` — но фазовое состояние живёт не в f32-углах, а в
//! тритах `{−1, 0, +1} ≅ GF(3)` — родном алфавите контейнеров POLER
//! (Packed4, v2/v3). Это закрывает обе проблемы RQ12, зафиксированные в
//! честных границах:
//!
//! 1. **Внутриблочная диффузия (лавина)**. В RQ12 проектор линеен и
//!    статичен: 1 бит → 1 фаза. Здесь блок — тритное состояние
//!    `x = m ⊕ s ⊕ mask`, которое **прецессирует** `T` тактов по руслам
//!    двух семейств:
//!    - *семантические* русла `J = A − Aᵀ` (поток смысла ключа);
//!    - *решётчатые* русла LENS — кольцо `d_pol`, три нечётных сдвига
//!      `(i, (i+s) mod d)` из знакочувствительного хеша ключа.
//!
//!    Один такт — GF(3)-сдвиг пары (дискретный аналог линеаризованного
//!    Курамото-транспорта `θ̇_i = −η·J_ij·(θ_j − θ_i)` из [`crate::gyro::precess_step`]):
//!
//!    ```text
//!    t_i' = (1+c)·t_i − c·t_j   (mod 3)
//!    t_j' =  c·t_i + (1−c)·t_j  (mod 3)
//!    ```
//!
//!    `det = 1` в любом кольце — инверсия точна: обратный порядок русл и
//!    `c → −c`. Знак веса русла задаёт направление закрутки
//!    `c ∈ {1, 2} = {+1, −1}`: зеркало `J → −J` меняет транспорт, но не
//!    проектор — ключи-отражения RQ12 расшифровывались, здесь — нет.
//!
//!    Почему мало чистой прецессии J: русла J покрывают только ядро
//!    потока (на ключе RQ11: 2651 русло, 538/4096 узлов; триты сообщения
//!    живут на холодных узлах в основном вне русел — там нет модовой
//!    энергии). Решётчатые русла дают глобальное покрытие кольца —
//!    измеренная лавина ~2/3 трит блока от одного бита (2/3 — потолок
//!    для сравнения случайных GF(3)-состояний).
//!
//! 2. **Расширение данных**. RQ12 хранил `d_pol × f32` (×43). Здесь —
//!    Packed4-триты (4 триты/байт, как контейнеры v2/v3): блок `d_pol/4`
//!    байт; ёмкость блока — не бит, а трит (упаковка 19 трит ↔ 30 бит,
//!    `3^19 > 2^30`): на ключе RQ11 3905 холодных трит ≈ 770 B сообщения
//!    на блок 1024 B → расширение ×1.34 (в 32 раза меньше ×43). Модель
//!    читает свои данные mmap-потоком Packed4 и разворачивает триты в
//!    углы Блоха на лету — прямой доступ к данным при обучении.
//!
//! Точность: вся арифметика целая (GF(3)) — расшифровка побитово точна,
//! без порогов и запасов декодирования (в RQ12 запас `min|δ̂ − ε/2|` был
//! 0.028 при пороге 0.05). Ключевой поток — терцили проекции `a·p₀`
//! (детерминированные пороги сортировкой — сбалансированные трети).
//!
//! Блочный режим CBC: блок 0 маскируется случайным IV (xoshiro256++),
//! блок `k` — тритами шифрблока `k−1`: смена одного бита каскадно
//! меняет все последующие блоки.
//!
//! Контейнер `.pqt` (magic `PQT1`, LE, zero-dep): заголовок 64 B
//! (d_pol, n_blocks, msg_len, k_modes, ticks, stride1/2, digest
//! sha256-24) + IV (`d_pol/4` B) + `n_blocks × d_pol/4` B Packed4-трит.
//!
//! Честные границы (по культуре аудита poler-os):
//! - схема **линейна над GF(3)**: известный открытый текст одного блока
//!   при известных руслах J восстанавливает ключевой поток на холодных
//!   позициях напрямую. Это сознательное свойство наблюдаемости — модель
//!   должна видеть свои веса и действия (проективная рефлексия, веса
//!   формально открыты); защита внешнего канала — задача TLS/RSA/PQC-
//!   контура, не этой схемы;
//! - лавина реальна (~2/3 трит от бита), но шифр остаётся линейным
//!   оператором — это диффузия, не нелинейность;
//! - длина сообщения читается из заголовка; целостность — только digest
//!   sha256-24 (сам линейный шифр без избыточности);
//! - детерминизм — в пределах одного бинарника (sin-тритификация
//!   ключевого потока);
//! - это исследовательская реализация уравнения файл 285, не
//!   production-шифр.

use crate::crypto::{bits_to_bytes, bytes_to_bits, get_u16, get_u32, get_u64, put_u16, put_u32, put_u64, CipherKey};
use crate::rng::Rng;
use crate::spin_avalanche::block_ticks;
use pqw::reader::PqwReader;
use pqw::sha256::sha256_trunc24;

/// Магия контейнера тритного шифртекста.
pub const TRITE_MAGIC: [u8; 4] = *b"PQT1";
/// Версия формата — линейная схема RQ13 (чистый транспорт).
pub const TRITE_VERSION: u16 = 1;
/// Версия формата — нелинейная схема RQ23: такт блока = транспорт +
/// спин-раунд (квадратичный T-проход GF(3),
/// [`crate::spin_avalanche`]). Шифртексты v1 расшифровываются как
/// раньше — версия в заголовке).
pub const TRITE_VERSION_V2: u16 = 2;
/// Размер заголовка `.pqt` (байт).
pub const TRITE_HEADER_SIZE: usize = 64;
/// Максимум тактов прецессии на блок (калибровка не выше).
pub const MAX_TICKS: u32 = 64;
/// Цель лавины при калибровке тактов (доля трит блока).
pub const AVALANCHE_TARGET: f64 = 0.5;
/// Порог лавины в тестах/отчёте: 2/3 — потолок GF(3)-состояний.
pub const AVALANCHE_CEILING: f64 = 2.0 / 3.0;

/// Русло прецессии в GF(3): пара узлов `(i, j)` и коэффициент закрутки
/// `c ∈ {1, 2} = {+1, −1}` (mod 3). Для русел J коэффициент — знак веса
/// потока; для решётчатых русел — бит знакочувствительного хеша ключа.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TritPair {
    pub i: u32,
    pub j: u32,
    pub c: u8,
}

/// Ключ трит-схемы: геометрия архетипа RQ12 + русла прецессии.
#[derive(Clone, Debug)]
pub struct TritKey {
    /// Размерность фазового пространства.
    pub d_pol: usize,
    /// Число мод проектора (ранг a = 2K).
    pub k_modes: usize,
    /// Ёмкость блока: число холодных трит сообщения.
    pub capacity_trites: usize,
    /// Холодные позиции (узлы с энергией мод ≤ 0.06/K).
    positions: Vec<u32>,
    /// Русла прецессии: семантические J + решётчатые LENS.
    pairs: Vec<TritPair>,
    /// Число семантических русел J ключа.
    pub raw_pairs: usize,
    /// Решётчатые сдвиги (s1, s2, s3).
    pub strides: (u32, u32, u32),
    /// Ключевой поток: тритификация `a·p₀`.
    keystream: Vec<u8>,
    /// Тактов линейной прецессии на блок (калибровка v1 по зондам).
    pub ticks: u32,
    /// Тактов комбинированного нелинейного такта v2 (транспорт + спин,
    /// RQ23): калибровка по тем же зондам на комбинированном
    /// преобразовании.
    ticks_v2: u32,
    /// Группа упаковки: `group_t` трит ↔ `group_b` бит.
    pub group_t: usize,
    pub group_b: usize,
    /// Максимальная Ritz-невязка мод.
    pub ritz_max: f64,
    /// Максимальная невязка ортонормальности ≡ идемпотентности.
    pub ortho_max: f64,
}

/// Сводка шифрования (трит-схема).
#[derive(Clone, Debug)]
pub struct EncryptReport {
    /// Версия схемы: 1 — линейная (RQ13), 2 — нелинейная спиновая (RQ23).
    pub version: u16,
    /// Блоков сообщения (IV хранится отдельно, в блоки не входит).
    pub blocks: usize,
    /// Размер открытого текста (байт).
    pub msg_len: usize,
    /// Размер шифртекста (байт).
    pub out_len: usize,
    /// Число мод проектора.
    pub k_modes: usize,
    /// Ёмкость блока (трит).
    pub capacity_trites: usize,
    /// Тактов прецессии на блок.
    pub ticks: u32,
    /// Спин-раундов на блок (v2: чередуются с транспортом; v1: 0).
    pub nl_rounds: u32,
    /// Всего русел (семантические + решётчатые).
    pub pairs_total: usize,
    /// Измеренная лавина: доля трит шифртекста, изменившихся от смены
    /// одного бита сообщения (с тем же IV — честная диффузия, RQ23).
    pub avalanche: f64,
    /// Расширение: `out_len / msg_len`.
    pub expansion: f64,
    /// hex digest sha256-24 (IV + блоки).
    pub digest_hex: String,
}

/// Сводка расшифровки (трит-схема).
#[derive(Clone, Debug)]
pub struct DecryptReport {
    pub blocks: usize,
    pub msg_len: usize,
    pub ticks: u32,
}

/// GF(3)-сдвиг пары: один шаг прецессии по руслу.
///
/// Прямой ход — коэффициент `c`, обратный — `−c` (той же парой).
/// `det = 1` в любом кольце: `(1+c)(1−c) + c² = 1`.
#[inline]
fn shear(x: &mut [u8], i: usize, j: usize, c: u8, inverse: bool) {
    debug_assert_ne!(i, j);
    let c = c as i32;
    let (ti, tj) = (x[i] as i32, x[j] as i32);
    if !inverse {
        x[i] = (((1 + c) * ti - c * tj).rem_euclid(3)) as u8;
        x[j] = ((c * ti + (1 - c) * tj).rem_euclid(3)) as u8;
    } else {
        x[i] = (((1 - c) * ti + c * tj).rem_euclid(3)) as u8;
        x[j] = ((-c * ti + (1 + c) * tj).rem_euclid(3)) as u8;
    }
}

/// Такт прецессии: все русла по порядку (прямой ход) или в обратном
/// порядке с обращёнными коэффициентами (инверсия).
pub fn transport_tick(pairs: &[TritPair], x: &mut [u8], inverse: bool) {
    if !inverse {
        for p in pairs {
            shear(x, p.i as usize, p.j as usize, p.c, false);
        }
    } else {
        for p in pairs.iter().rev() {
            shear(x, p.i as usize, p.j as usize, p.c, true);
        }
    }
}

/// `T` тактов прецессии. `transport(pairs, x, T, false)` затем
/// `transport(pairs, x, T, true)` — тождество (проверено тестами).
pub fn transport_ticks(pairs: &[TritPair], x: &mut [u8], ticks: u32, inverse: bool) {
    for _ in 0..ticks {
        transport_tick(pairs, x, inverse);
    }
}

/// Знакочувствительный хеш русел J (FNV-1a по `i, j, to_bits(w)`):
/// зеркало `J → −J` даёт другой хеш → другие решётчатые русла.
fn key_hash(raw: &[(u32, u32, f64)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &(i, j, w) in raw {
        h ^= u64::from(i);
        h = h.wrapping_mul(0x1000_0000_01b3);
        h ^= u64::from(j);
        h = h.wrapping_mul(0x1000_0000_01b3);
        h ^= w.to_bits();
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// Три нечётных решётчатых сдвига из хеша ключа: `s ∈ {1, 3, 5, …}`,
/// `s < d`. Нечётные сдвиги на кольце степени двойки взаимно просты с
/// `d`; gcd всех трёх с `d` гарантированно 1 (иначе сдвиг nudged +2) —
/// решётка русл покрывает всё кольцо. Три сдвига — три направления
/// случайного блуждания фазы: меньше коррелированных GF(3)-сокращений.
fn stride_params(h: u64, d: usize) -> (u32, u32, u32) {
    let half = (d as u64 / 2).max(1);
    let mut s1 = 1 + 2 * ((h % (d as u64 / 4).max(1)) as u32);
    let mut s2 = 1 + 2 * (((h >> 32) % half.saturating_sub(1).max(1)) as u32);
    let mut s3 = 1 + 2 * (((h >> 48) % half.saturating_sub(1).max(1)) as u32);
    let max_s = (d as u32 - 1).max(1);
    s1 = (s1.min(max_s) | 1).min(max_s);
    s2 = (s2.min(max_s) | 1).min(max_s);
    s3 = (s3.min(max_s) | 1).min(max_s);
    // Решётка {s1, s2, s3} обязана генерировать всё кольцо Z_d.
    while gcd3(u64::from(s1), u64::from(s2), u64::from(s3), d as u64) > 1 {
        s3 = (s3 + 2).min(max_s) | 1;
        if s3 >= max_s {
            s3 = 1;
            break; // d вырожденно мало — соседнее кольцо само покрывает
        }
    }
    (s1, s2, s3)
}

/// gcd трёх сдвигов и размерности кольца.
fn gcd3(a: u64, b: u64, c: u64, d: u64) -> u64 {
    fn gcd(mut x: u64, mut y: u64) -> u64 {
        while y != 0 {
            (x, y) = (y, x % y);
        }
        x
    }
    gcd(gcd(gcd(a, b), c), d)
}

/// Решётчатые русла LENS: кольцо `d_pol`, циклы `(i, (i+s) mod d)` для
/// трёх сдвигов. Полное покрытие узлов — глобальная диффузия.
fn stride_pairs(h: u64, d: usize) -> Vec<TritPair> {
    let (s1, s2, s3) = stride_params(h, d);
    let mut out = Vec::with_capacity(3 * d);
    for &s in &[s1, s2, s3] {
        let c = 1 + ((h >> (s % 60)) & 1) as u8;
        for i in 0..d as u32 {
            let j = (i + s) % d as u32;
            if i != j {
                out.push(TritPair { i, j, c });
            }
        }
    }
    out
}

/// Тритификация ключевого потока: терцили проекции `a·p₀` —
/// детерминированные пороги (сортировка значений, границы третей).
/// Сбалансированные трити по построению; фазы `θ = arccos(p̂)` живут
/// в полукруге [0, π], поэтому sin-секторы давали бы перекос в ноль.
fn tritify_terciles(values: &[f64]) -> Vec<u8> {
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let q1 = sorted[n / 3];
    let q2 = sorted[2 * n / 3];
    values
        .iter()
        .map(|&v| {
            if v < q1 {
                0
            } else if v < q2 {
                1
            } else {
                2
            }
        })
        .collect()
}

/// Биты → триты: группы по `gb` бит → `gt` трит (radix-3, старшая
/// трита первой). `3^gt > 2^gb` — взаимно однозначно.
pub fn bits_to_trites(bits: &[bool], group_t: usize, group_b: usize) -> Vec<u8> {
    let n_groups = bits.len().div_ceil(group_b);
    let mut trites = Vec::with_capacity(n_groups * group_t);
    for g in 0..n_groups {
        let mut word: u64 = 0;
        for k in 0..group_b {
            let idx = g * group_b + k;
            if idx < bits.len() && bits[idx] {
                word |= 1 << k;
            }
        }
        let mut digits = vec![0u8; group_t];
        for t in (0..group_t).rev() {
            digits[t] = (word % 3) as u8;
            word /= 3;
        }
        trites.extend_from_slice(&digits);
    }
    trites
}

/// Триты → биты (обратное к [`bits_to_trites`]); обрезка до `n_bits`.
pub fn trites_to_bits(trites: &[u8], n_bits: usize, group_t: usize, group_b: usize) -> Vec<bool> {
    let n_groups = trites.len() / group_t;
    let mut bits = Vec::with_capacity(n_groups * group_b);
    for g in 0..n_groups {
        let mut word: u64 = 0;
        for t in 0..group_t {
            word = word * 3 + u64::from(trites[g * group_t + t]);
        }
        for k in 0..group_b {
            bits.push(word >> k & 1 == 1);
        }
    }
    bits.truncate(n_bits);
    bits
}

/// Packed4: трита `i` → байт `i/4`, биты `2·(i%4)` — конвенция
/// контейнеров v2/v3 ([`pqw`]).
pub fn pack_trites(t: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; t.len().div_ceil(4)];
    for (i, &v) in t.iter().enumerate() {
        out[i / 4] |= (v & 3) << (2 * (i % 4));
    }
    out
}

/// Распаковка Packed4 (`n` трит).
pub fn unpack_trites(bytes: &[u8], n: usize) -> Vec<u8> {
    (0..n).map(|i| (bytes[i / 4] >> (2 * (i % 4))) & 3).collect()
}

/// Калибровка тактов: наименьшее `T ∈ {4, 8, 16, 32}`, при котором
/// лавина из одиночной триты на четырёх зондах (холодные позиции — там
/// живёт сообщение) достигает [`AVALANCHE_TARGET`]. Детерминирована —
/// расшифровщик воспроизводит то же `T`.
fn calibrate_ticks(pairs: &[TritPair], d: usize, positions: &[u32]) -> u32 {
    let n = positions.len();
    let probes: [usize; 4] = [
        positions[0] as usize,
        positions[n / 4] as usize,
        positions[n / 2] as usize,
        positions[n - 1] as usize,
    ];
    for &t in &[4u32, 8, 16, 32, MAX_TICKS] {
        let mut worst = 1.0_f64;
        for &p in &probes {
            let mut x = vec![0u8; d];
            x[p] = 1;
            transport_ticks(pairs, &mut x, t, false);
            let spread = x.iter().filter(|&&v| v != 0).count() as f64 / d as f64;
            worst = worst.min(spread);
        }
        if worst >= AVALANCHE_TARGET {
            return t;
        }
    }
    MAX_TICKS
}

/// Калибровка нелинейных тактов v2 (RQ23): тот же критерий (лавина
/// ≥ [`AVALANCHE_TARGET`] на четырёх зондах), но преобразование —
/// комбинированный такт «транспорт + спин-раунд» с лентой ключевого
/// потока. Спиновый слой заражает блок быстрее линейного транспорта —
/// `T₂ ≤ T₁`: нелинейность дешевле диффузии.
fn calibrate_ticks_v2(pairs: &[TritPair], ks: &[u8], d: usize, positions: &[u32]) -> u32 {
    let n = positions.len();
    let probes: [usize; 4] = [
        positions[0] as usize,
        positions[n / 4] as usize,
        positions[n / 2] as usize,
        positions[n - 1] as usize,
    ];
    for &t in &[1u32, 2, 4, 8, 16, 32, MAX_TICKS] {
        let mut worst = 1.0_f64;
        for &p in &probes {
            let mut x = vec![0u8; d];
            x[p] = 1;
            block_ticks(pairs, ks, &mut x, t, false);
            let spread = x.iter().filter(|&&v| v != 0).count() as f64 / d as f64;
            worst = worst.min(spread);
        }
        if worst >= AVALANCHE_TARGET {
            return t;
        }
    }
    MAX_TICKS
}

impl TritKey {
    /// Загрузка ключа из контейнера v3 (готовый `precess --out` /
    /// `train --gyro`): геометрия архетипа — как в RQ12 (`CipherKey`:
    /// авто-калибровка K, холодные позиции, ключевой поток) — плюс
    /// русла прецессии GF(3).
    pub fn from_reader(reader: &PqwReader, k_hint: usize) -> Result<TritKey, String> {
        let inner = CipherKey::from_reader(reader, k_hint)?;
        let d = inner.d_pol;
        let section = reader
            .gyro()
            .ok_or_else(|| "контейнер без топологической секции (v1/v2) — ключу нужен гироскоп J = A − Aᵀ".to_string())?;
        let raw: Vec<(u32, u32, f64)> = section
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        // Семантические русла: знак веса задаёт направление закрутки.
        let mut pairs: Vec<TritPair> = raw
            .iter()
            .filter(|&&(i, j, w)| i != j && w.is_finite() && w != 0.0)
            .map(|&(i, j, w)| TritPair {
                i,
                j,
                c: 1 + (w.to_bits() >> 63) as u8,
            })
            .collect();
        if pairs.is_empty() {
            return Err("гироскоп ключа пуст (нет пригодных русел J)".into());
        }
        let raw_pairs = pairs.len();
        // Решётчатые русла LENS: глобальное покрытие кольца d_pol.
        let h = key_hash(&raw);
        let strides = stride_params(h, d);
        pairs.extend(stride_pairs(h, d));
        // Холодные позиции и группа упаковки.
        let positions = inner.positions().to_vec();
        let cap = positions.len();
        let (group_t, group_b) = if cap >= 19 {
            (19, 30)
        } else if cap >= 5 {
            (5, 7)
        } else {
            return Err("ёмкость ключа меньше 5 трит — ключ непригоден для трит-схемы".into());
        };
        // Ключевой поток: терцили тритификации a·p₀.
        let keystream: Vec<u8> = tritify_terciles(&inner.keystream());
        let ticks = calibrate_ticks(&pairs, d, &positions);
        // RQ23: нелинейная калибровка — комбинированный такт.
        let ticks_v2 = calibrate_ticks_v2(&pairs, &keystream, d, &positions);
        Ok(TritKey {
            d_pol: d,
            k_modes: inner.k_modes,
            capacity_trites: cap,
            positions,
            pairs,
            raw_pairs,
            strides,
            keystream,
            ticks,
            ticks_v2,
            group_t,
            group_b,
            ritz_max: inner.ritz_max,
            ortho_max: inner.ortho_max,
        })
    }

    /// Холодные позиции блока (там живёт сообщение).
    pub fn positions(&self) -> &[u32] {
        &self.positions
    }

    /// Русла прецессии (семантические J + решётчатые LENS) — порядок
    /// определяет транспорт; слияние/чтение контейнера его сохраняет.
    pub fn pairs(&self) -> &[TritPair] {
        &self.pairs
    }

    /// Ключевой поток (триты 0..2) — лента коэффициентов спинового слоя.
    pub fn keystream(&self) -> &[u8] {
        &self.keystream
    }

    /// Тактов комбинированного нелинейного такта v2 (транспорт + спин).
    pub fn ticks_v2(&self) -> u32 {
        self.ticks_v2
    }

    /// Триты сообщения → блок состояния → преобразование → Packed4-байты.
    ///
    /// `version` выбирает слой диффузии: 1 — линейный транспорт RQ13
    /// (`ticks` тактов), 2 — комбинированный нелинейный такт RQ23
    /// (транспорт + спин-раунд × `ticks_v2`). Нелинейная схема —
    /// общедоступный примитив лавины ([`crate::spin_avalanche`]).
    pub fn encrypt_body(
        &self,
        msg: &[u8],
        rng: &mut Rng,
    ) -> (Vec<u8>, Vec<u8>, usize) {
        self.encrypt_body_version(msg, rng, TRITE_VERSION_V2)
    }

    /// Тело шифрования с явной версией схемы (v1 — legacy).
    pub fn encrypt_body_version(
        &self,
        msg: &[u8],
        rng: &mut Rng,
        version: u16,
    ) -> (Vec<u8>, Vec<u8>, usize) {
        let d = self.d_pol;
        let cap = self.capacity_trites;
        let bits = bytes_to_bits(msg);
        let trites = bits_to_trites(&bits, self.group_t, self.group_b);
        let n_blocks = trites.len().div_ceil(cap);
        // IV: случайные триты — рандомизирует цепочку CBC.
        let iv: Vec<u8> = (0..d).map(|_| (rng.next_u64() % 3) as u8).collect();
        let mut data: Vec<u8> = Vec::with_capacity(n_blocks * d.div_ceil(4));
        let mut prev = iv.clone();
        for b in 0..n_blocks {
            // Сообщение → холодные позиции.
            let mut x = vec![0u8; d];
            for j in 0..cap {
                let idx = b * cap + j;
                if idx < trites.len() {
                    x[self.positions[j] as usize] = trites[idx];
                }
            }
            // ⊕ ключевой поток и CBC-маска (GF(3)).
            for i in 0..d {
                x[i] = ((x[i] + self.keystream[i] + prev[i]) % 3) as u8;
            }
            // Диффузия: v2 — нелинейные такты (транспорт + спин),
            // v1 — чистый линейный транспорт RQ13.
            if version == TRITE_VERSION_V2 {
                block_ticks(&self.pairs, &self.keystream, &mut x, self.ticks_v2, false);
            } else {
                transport_ticks(&self.pairs, &mut x, self.ticks, false);
            }
            data.extend_from_slice(&pack_trites(&x));
            prev = x;
        }
        (pack_trites(&iv), data, n_blocks)
    }
}

/// Шифрование (трит-схема, RQ23): `m → p*` в Packed4-тритах.
///
/// По умолчанию — нелинейная схема v2: такт блока = линейный транспорт
/// русл + квадратичный спин-проход GF(3) (лавина + защита от линейного
/// криптоанализа). [`encrypt_v1`] пишет legacy-контейнер RQ13 (чистый
/// транспорт) — оба читаются [`decrypt`].
///
/// Лавина измеряется на месте: повторное шифрование с перевёрнутым
/// битом и **тем же IV** (клон rng ДО первого шифрования — честная
/// диффузия без шума цепочки, RQ23) и подсчёт изменившихся трит блоков.
pub fn encrypt(key: &TritKey, msg: &[u8], rng: &mut Rng) -> Result<(Vec<u8>, EncryptReport), String> {
    encrypt_version(key, msg, rng, TRITE_VERSION_V2)
}

/// Шифрование линейной схемой RQ13 (v1): чистый транспорт, без
/// спинового слоя — для воспроизведения старых прогонов.
pub fn encrypt_v1(
    key: &TritKey,
    msg: &[u8],
    rng: &mut Rng,
) -> Result<(Vec<u8>, EncryptReport), String> {
    encrypt_version(key, msg, rng, TRITE_VERSION)
}

fn encrypt_version(
    key: &TritKey,
    msg: &[u8],
    rng: &mut Rng,
    version: u16,
) -> Result<(Vec<u8>, EncryptReport), String> {
    // Клон ДО первого шифрования: зонд лавины получает тот же IV —
    // меряем диффузию, а не разницу случайных цепочек.
    let rng_probe = rng.clone();
    let (iv_bytes, data, n_blocks) = key.encrypt_body_version(msg, rng, version);
    // Лавина: 1 бит сообщения → доля изменившихся трит шифртекста.
    let avalanche = if msg.is_empty() || data.is_empty() {
        0.0
    } else {
        let mut flipped = msg.to_vec();
        flipped[msg.len() / 2] ^= 1;
        let (_, data2, _) = key.encrypt_body_version(&flipped, &mut rng_probe.clone(), version);
        let n = data.len() * 4;
        let t1 = unpack_trites(&data, n);
        let t2 = unpack_trites(&data2, n);
        t1.iter()
            .zip(t2.iter())
            .filter(|(a, b)| a != b)
            .count() as f64
            / n as f64
    };
    let (ticks_header, nl_rounds) = if version == TRITE_VERSION_V2 {
        (key.ticks_v2, key.ticks_v2)
    } else {
        (key.ticks, 0)
    };
    let digest = sha256_trunc24(&[&iv_bytes[..], &data[..]].concat());
    let mut out = Vec::with_capacity(TRITE_HEADER_SIZE + iv_bytes.len() + data.len());
    out.extend_from_slice(&TRITE_MAGIC);
    put_u16(&mut out, version);
    put_u16(&mut out, 0); // flags
    put_u32(&mut out, key.d_pol as u32);
    put_u32(&mut out, n_blocks as u32);
    put_u64(&mut out, msg.len() as u64);
    put_u32(&mut out, key.k_modes as u32);
    put_u32(&mut out, ticks_header);
    put_u16(&mut out, key.strides.0 as u16);
    put_u16(&mut out, key.strides.1 as u16);
    put_u16(&mut out, key.strides.2 as u16);
    put_u16(&mut out, 0); // паддинг до digest
    debug_assert_eq!(out.len(), TRITE_HEADER_SIZE - 24);
    out.extend_from_slice(&digest);
    out.extend_from_slice(&iv_bytes);
    out.extend_from_slice(&data);
    let out_len = out.len();
    let expansion = if msg.is_empty() {
        out_len as f64
    } else {
        out_len as f64 / msg.len() as f64
    };
    Ok((
        out,
        EncryptReport {
            version,
            blocks: n_blocks,
            msg_len: msg.len(),
            out_len,
            k_modes: key.k_modes,
            capacity_trites: key.capacity_trites,
            ticks: ticks_header,
            nl_rounds,
            pairs_total: key.pairs.len(),
            avalanche,
            expansion,
            digest_hex: digest.iter().map(|b| format!("{b:02x}")).collect(),
        },
    ))
}

/// Расшифровка (трит-схема): `m = p* ⊕ (a ⊗_ε p*)` в GF(3) — точно.
pub fn decrypt(key: &TritKey, cipher: &[u8]) -> Result<(Vec<u8>, DecryptReport), String> {
    if cipher.len() < TRITE_HEADER_SIZE {
        return Err("шифртекст короче заголовка".into());
    }
    if cipher[..4] != TRITE_MAGIC {
        return Err("не контейнер .pqt (магия PQT1 не найдена)".into());
    }
    let version = get_u16(cipher, 4);
    if version != TRITE_VERSION && version != TRITE_VERSION_V2 {
        return Err(format!("неподдерживаемая версия .pqt: {version}"));
    }
    let d = get_u32(cipher, 8) as usize;
    let n_blocks = get_u32(cipher, 12) as usize;
    let msg_len = get_u64(cipher, 16) as usize;
    let k_modes = get_u32(cipher, 24) as usize;
    let ticks = get_u32(cipher, 28);
    let strides = (
        u32::from(get_u16(cipher, 32)),
        u32::from(get_u16(cipher, 34)),
        u32::from(get_u16(cipher, 36)),
    );
    let digest_off = TRITE_HEADER_SIZE - 24;
    let expect_digest = &cipher[digest_off..digest_off + 24];
    let body = &cipher[TRITE_HEADER_SIZE..];

    if d != key.d_pol {
        return Err(format!(
            "d_pol шифртекста ({d}) != d_pol ключа ({}) — ключ не от этого шифртекста",
            key.d_pol
        ));
    }
    if k_modes != key.k_modes {
        return Err(format!(
            "число мод шифртекста ({k_modes}) != ключа ({}) — ключ не от этого шифртекста",
            key.k_modes
        ));
    }
    // Такты в заголовке — калибровка соответствующей версии схемы.
    let key_ticks = if version == TRITE_VERSION_V2 {
        key.ticks_v2
    } else {
        key.ticks
    };
    if ticks != key_ticks || strides != key.strides {
        return Err(format!(
            "параметры прецессии (такты {ticks} ≠ {key_ticks} / решётчатые русла) \
             не совпадают — ключ не от этого шифртекста"
        ));
    }
    let block_len = d.div_ceil(4);
    if body.len() != block_len * (n_blocks + 1) {
        return Err(format!(
            "тело шифртекста {body_len} B != (1 + {n_blocks}) × {block_len} B",
            body_len = body.len()
        ));
    }
    let digest = sha256_trunc24(body);
    if digest.as_slice() != expect_digest {
        return Err("digest sha256-24 не сходится — шифртекст повреждён".into());
    }
    // Ёмкость: msg_len байт должно помещаться в n_blocks блоков.
    // Группы упаковки могут пересекать границы блоков — считаем группы
    // по суммарному числу трит: floor(n_blocks·cap / group_t)·group_b.
    let cap = key.capacity_trites;
    let bits_cap = (n_blocks * cap / key.group_t) * key.group_b;
    if msg_len * 8 > bits_cap {
        return Err("msg_len больше ёмкости блоков шифртекста".into());
    }

    let iv = unpack_trites(&body[..block_len], d);
    let mut out_trites: Vec<u8> = Vec::with_capacity(n_blocks * cap);
    let mut prev = iv;
    for b in 0..n_blocks {
        let block = &body[block_len * (b + 1)..block_len * (b + 2)];
        let stored = unpack_trites(block, d); // шифрблок (после диффузии)
        let mut x = stored.clone();
        // Обратная диффузия: v2 — спин-инверсия + обратный транспорт,
        // v1 — чистый обратный транспорт RQ13.
        if version == TRITE_VERSION_V2 {
            block_ticks(&key.pairs, &key.keystream, &mut x, key.ticks_v2, true);
        } else {
            transport_ticks(&key.pairs, &mut x, key.ticks, true);
        }
        for j in 0..cap {
            let pos = position_of(key, j);
            let t =
                (x[pos] as i32 - key.keystream[pos] as i32 - prev[pos] as i32).rem_euclid(3) as u8;
            out_trites.push(t);
        }
        prev = stored; // CBC-маска следующего блока — шифрблок как есть
    }
    let bits = trites_to_bits(&out_trites, msg_len * 8, key.group_t, key.group_b);
    if bits.len() < msg_len * 8 {
        return Err("трит сообщения меньше msg_len".into());
    }
    let msg = bits_to_bytes(&bits);
    Ok((
        msg,
        DecryptReport {
            blocks: n_blocks,
            msg_len,
            ticks: key.ticks,
        },
    ))
}

/// Позиция триты `j` сообщения (холодный узел).
#[inline]
fn position_of(key: &TritKey, j: usize) -> usize {
    key.positions[j] as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::{GyroData, PqwWriter};

    /// Доля трит ключевого потока по классам (для теста баланса).
    fn keystream_fractions(key: &TritKey) -> [f64; 3] {
        let mut counts = [0usize; 3];
        for &t in &key.keystream {
            counts[t as usize] += 1;
        }
        [
            counts[0] as f64 / key.d_pol as f64,
            counts[1] as f64 / key.d_pol as f64,
            counts[2] as f64 / key.d_pol as f64,
        ]
    }

    /// Ключ-путь: d=256, русла 0→1→…→255 — моды синусоидальны и
    /// распределены по всей размерности.
    fn path_key() -> Vec<u8> {
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p = if i % 3 == 0 { -0.7 } else { 0.6 };
            w.add_phase(i, p).unwrap();
        }
        let pairs: Vec<(u32, u32, f64)> =
            (0u32..255).map(|i| (i, i + 1, 1.0)).collect();
        let gyro = GyroData::new(256, 1000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    /// Ключ-гребёнка: d=256, русла (i, i+1) и (i, i+4).
    fn chain_key() -> Vec<u8> {
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p = if i % 2 == 0 { -0.5 } else { 0.8 };
            w.add_phase(i, p).unwrap();
        }
        let mut pairs: Vec<(u32, u32, f64)> =
            (0u32..255).map(|i| (i, i + 1, 1.0)).collect();
        pairs.extend((0u32..252).map(|i| (i, i + 4, 0.6)));
        let gyro = GyroData::new(256, 5000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    /// Плотный циркулянтный ключ: d=256, русла (i, i+s) для сдвигов
    /// s ∈ {1, 5, 11, 23} — как crypto.rs, но свои веса/фазы по seed.
    fn dense_key(seed: u64) -> Vec<u8> {
        let mut rng = Rng::seed_from_u64(seed);
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p: f64 = rng.next_f64() * 1.6 - 0.8;
            w.add_phase(i, p as f32).unwrap();
        }
        let mut pairs = Vec::new();
        for &shift in &[1u32, 5, 11, 23] {
            for i in 0u32..(256 - shift) {
                let sign = if rng.next_u64() & 1 == 1 { 1.0 } else { -1.0 };
                pairs.push((i, i + shift, sign * (0.3 + 0.7 * rng.next_f64())));
            }
        }
        let gyro = GyroData::new(256, 9000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    /// Хаб-ключ как реальный RQ11: d=512, плотное ядро русел на ~60
    /// узлах (хабы с ε-воротами), остальные узлы изолированы — холодные.
    fn hub_key() -> Vec<u8> {
        let mut rng = Rng::seed_from_u64(2026);
        let mut w = PqwWriter::new(512).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..512 {
            let p: f64 = rng.next_f64() * 1.8 - 0.9;
            w.add_phase(i, p as f32).unwrap();
        }
        // Ядро: 60 хаб-узлов, ~500 уникальных русел между ними (i < j,
        // без дублей — как требует GyroData).
        let mut seen = std::collections::BTreeSet::new();
        let mut pairs = Vec::new();
        let mut rng2 = Rng::seed_from_u64(2027);
        while pairs.len() < 500 {
            let i = (rng2.next_u64() % 60) as u32;
            let j = (rng2.next_u64() % 60) as u32;
            if i < j && seen.insert((i, j)) {
                let sign = if rng2.next_u64() & 1 == 1 { 1.0 } else { -1.0 };
                pairs.push((i, j, sign * (0.3 + 0.7 * rng2.next_f64())));
            }
        }
        let _ = &mut rng;
        let gyro = GyroData::new(512, 7000, pairs, 512).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    fn load(bytes: &[u8]) -> TritKey {
        let reader = PqwReader::from_bytes(bytes).unwrap();
        TritKey::from_reader(&reader, 0).unwrap()
    }

    #[test]
    fn shear_invertible_all_coefficients() {
        // M(c)·M(−c) = I (mod 3) для обоих c и всех пар значений.
        for c in [1u8, 2] {
            for ti in 0u8..3 {
                for tj in 0u8..3 {
                    let mut x = [ti, tj];
                    shear(&mut x, 0, 1, c, false);
                    shear(&mut x, 0, 1, c, true);
                    assert_eq!(x, [ti, tj], "c={c} ti={ti} tj={tj}");
                }
            }
        }
    }

    #[test]
    fn transport_inverse_roundtrip() {
        let key = load(&chain_key());
        let mut rng = Rng::seed_from_u64(11);
        let mut x: Vec<u8> = (0..key.d_pol).map(|_| (rng.next_u64() % 3) as u8).collect();
        let orig = x.clone();
        transport_ticks(&key.pairs, &mut x, 32, false);
        assert_ne!(x, orig, "прецессия не изменила состояние");
        transport_ticks(&key.pairs, &mut x, 32, true);
        assert_eq!(x, orig, "обратная прецессия не тождественна");
    }

    #[test]
    fn transport_diffuses_isolated_positions() {
        // Хаб-ключ: триты сообщения живут на изолированных узлах —
        // чистая прецессия J их не трогает, решётчатые русла дают
        // лавину. Калибровка обязана это учитывать.
        let key = load(&hub_key());
        let mut worst = 1.0f64;
        for &pos in key.positions.iter().take(8) {
            let mut x = vec![0u8; key.d_pol];
            x[pos as usize] = 1;
            transport_ticks(&key.pairs, &mut x, key.ticks, false);
            let spread = x.iter().filter(|&&v| v != 0).count() as f64 / key.d_pol as f64;
            worst = worst.min(spread);
        }
        assert!(
            worst >= AVALANCHE_TARGET,
            "лавина на холодных позициях {worst:.3} < {AVALANCHE_TARGET} (тактов {})",
            key.ticks
        );
    }

    #[test]
    fn pack_roundtrip_and_convention() {
        // Самосогласованность + конвенция Packed4 (трита i → байт i/4,
        // биты 2·(i%4)) — как контейнеры v2/v3.
        let trites: Vec<u8> = (0..257).map(|i| (i % 3) as u8).collect();
        let packed = pack_trites(&trites);
        assert_eq!(packed.len(), 257usize.div_ceil(4));
        assert_eq!(packed[0], 0 | 1 << 2 | 2 << 4 | 0 << 6);
        assert_eq!(unpack_trites(&packed, 257), trites);
    }

    #[test]
    fn trite_packing_roundtrip() {
        for len in [0usize, 1, 2, 3, 17, 19, 30, 100, 777, 4096] {
            let msg: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(7)).collect();
            let bits = bytes_to_bits(&msg);
            let (gt, gb) = (19usize, 30usize);
            let trites = bits_to_trites(&bits, gt, gb);
            let bits2 = trites_to_bits(&trites, bits.len(), gt, gb);
            assert_eq!(bits, bits2, "len={len}");
            assert_eq!(bits_to_bytes(&bits2), msg, "len={len}");
        }
        // Малая группа 5↔7: 3^5 = 243 ≥ 2^7 = 128.
        let bits = vec![true; 7];
        let trites = bits_to_trites(&bits, 5, 7);
        assert_eq!(trites.len(), 5);
        assert_eq!(trites_to_bits(&trites, 7, 5, 7), bits);
    }

    #[test]
    fn keystream_tritified_balanced() {
        let key = load(&dense_key(5));
        let fracs = keystream_fractions(&key);
        for (t, &frac) in fracs.iter().enumerate() {
            assert!(
                frac < 0.6,
                "класс {t} ключевого потока занимает {frac:.2} — разбаланс"
            );
        }
        assert!(key.keystream.iter().any(|&t| t != 0), "ключевой поток нулевой");
    }

    #[test]
    fn roundtrip_chain_multiblock() {
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..3000u32).map(|i| (i * 37 + 11) as u8).collect();
        let mut rng = Rng::seed_from_u64(42);
        let (cipher, rep) = encrypt(&key, &msg, &mut rng).unwrap();
        assert!(rep.blocks >= 8, "мультиблочность: {} блоков", rep.blocks);
        let (plain, _) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg, "roundtrip не точен");
    }

    #[test]
    fn roundtrip_all_key_shapes() {
        for (name, bytes) in [
            ("path", path_key()),
            ("dense", dense_key(1)),
            ("hub", hub_key()),
        ] {
            let key = load(&bytes);
            let msg: Vec<u8> = (0..1000u32).map(|i| (i * 13 + 1) as u8).collect();
            let mut rng = Rng::seed_from_u64(7);
            let (cipher, _) = encrypt(&key, &msg, &mut rng).unwrap();
            let (plain, _) = decrypt(&key, &cipher).unwrap();
            assert_eq!(plain, msg, "ключ {name}: roundtrip не точен");
        }
    }

    #[test]
    fn roundtrip_empty_message() {
        let key = load(&path_key());
        let (cipher, rep) = encrypt(&key, &[], &mut Rng::seed_from_u64(1)).unwrap();
        assert_eq!(rep.blocks, 0);
        let (plain, dec) = decrypt(&key, &cipher).unwrap();
        assert!(plain.is_empty());
        assert_eq!(dec.msg_len, 0);
    }

    #[test]
    fn avalanche_single_bit() {
        // Лавина с ТЕМ ЖЕ IV (RQ23 — честная метрика без шума цепочки):
        // флип бита → доля изменившихся трит шифртекста. Репорт меряет
        // весь шифртекст: CBC-каскад разносит замену только ВПЕРЁД —
        // флип в середине даёт ≥ 25% по всем блокам (префиксные блоки
        // не тронуты), флип в первом байте — почти потолок 2/3
        // (см. avalanche_from_first_byte ниже).
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..600u32).map(|i| (i * 17 + 3) as u8).collect();
        let mut rng = Rng::seed_from_u64(42);
        let (_, rep) = encrypt(&key, &msg, &mut rng).unwrap();
        assert!(
            rep.avalanche >= 0.2,
            "лавина {:.3} < 0.2 (тактов {})",
            rep.avalanche,
            rep.ticks
        );
        assert!(
            rep.avalanche <= AVALANCHE_CEILING + 0.05,
            "лавина {:.3} выше потолка GF(3)",
            rep.avalanche
        );
        // Схема v2 по умолчанию: спин-раунды активны.
        assert_eq!(rep.version, TRITE_VERSION_V2);
        assert!(rep.nl_rounds >= 1);
    }

    // ===================== RQ23: схема v2 (спин-слой) =====================

    #[test]
    fn v2_roundtrip_large_message() {
        // Большое много-блочное сообщение через нелинейную схему —
        // расшифровка побитово точна.
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..20_000u32).map(|i| (i * 31 + 7) as u8).collect();
        let mut rng = Rng::seed_from_u64(5);
        let (cipher, rep) = encrypt(&key, &msg, &mut rng).unwrap();
        assert_eq!(rep.version, TRITE_VERSION_V2);
        assert!(rep.blocks > 1, "многоблочное сообщение");
        let (plain, dec) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg);
        assert_eq!(dec.blocks, rep.blocks);
        assert_eq!(dec.ticks, key.ticks_v2());
    }

    #[test]
    fn v1_ciphertext_still_decrypts() {
        // Legacy-шифртекст v1 (линейный транспорт) расшифровывается
        // после RQ23: версия в заголовке направляет на чистый транспорт.
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..900u32).map(|i| (i * 13 + 1) as u8).collect();
        let mut rng = Rng::seed_from_u64(3);
        let (cipher, rep) = encrypt_v1(&key, &msg, &mut rng).unwrap();
        assert_eq!(rep.version, TRITE_VERSION);
        assert_eq!(rep.nl_rounds, 0);
        assert_eq!(rep.ticks, key.ticks);
        let (plain, _) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg);
        // v1 и v2 шифртексты одного сообщения различаются (слой диффузии).
        let mut rng2 = Rng::seed_from_u64(3);
        let (cipher2, _) = encrypt(&key, &msg, &mut rng2).unwrap();
        assert_ne!(cipher, cipher2);
    }

    #[test]
    fn v2_rejects_wrong_ticks() {
        // Заголовок v2 с тактами v1 — ключ не от этого шифртекста.
        let key = load(&chain_key());
        if key.ticks == key.ticks_v2 {
            return; // вырожденный ключ: калибровки совпали
        }
        let msg = b"mismatch probe".to_vec();
        let mut rng = Rng::seed_from_u64(9);
        let (mut cipher, _) = encrypt(&key, &msg, &mut rng).unwrap();
        // Подменяем такты в заголовке (offset 28) + пересчитываем digest
        // не нужно — проверка тактов идёт ДО digest.
        cipher[28..32].copy_from_slice(&key.ticks.to_le_bytes());
        assert!(decrypt(&key, &cipher).is_err());
    }

    #[test]
    fn v2_deterministic_same_seed() {
        // Один сид → побитово одинаковый шифртекст (детерминизм IV).
        let key = load(&chain_key());
        let msg = b"determinism".to_vec();
        let (c1, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(77)).unwrap();
        let (c2, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(77)).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn ticks_v2_never_exceeds_linear() {
        // Нелинейный комбинированный такт достигает цели калибровки
        // не медленнее чистого транспорта (спин только добавляет
        // перемешивание) — на обоих тестовых ключах.
        for bytes in [path_key(), chain_key()] {
            let reader = pqw::PqwReader::from_bytes(&bytes).unwrap();
            let key = TritKey::from_reader(&reader, 0).unwrap();
            assert!(
                key.ticks_v2 <= key.ticks,
                "T2={} > T1={} — спин не должен замедлять диффузию",
                key.ticks_v2,
                key.ticks
            );
        }
    }

    #[test]
    fn avalanche_from_first_byte_near_ceiling() {
        // Флип ПЕРВОГО бита: CBC-каскад заражает все блоки от первого —
        // честная диффузия схемы без префиксного разведения ≈ 2/3.
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..600u32).map(|i| (i * 17 + 3) as u8).collect();
        // Тот же IV на оба прогона — изолируем диффузию от шума цепочки.
        let iv_seed = 4242u64;
        let mut rng = Rng::seed_from_u64(iv_seed);
        let (_, data, _) = key.encrypt_body_version(&msg, &mut rng, TRITE_VERSION_V2);
        let mut flipped = msg.clone();
        flipped[0] ^= 1;
        let mut rng2 = Rng::seed_from_u64(iv_seed);
        let (_, data2, _) = key.encrypt_body_version(&flipped, &mut rng2, TRITE_VERSION_V2);
        let n = data.len() * 4;
        let t1 = unpack_trites(&data, n);
        let t2 = unpack_trites(&data2, n);
        let av = t1.iter().zip(t2.iter()).filter(|(a, b)| a != b).count() as f64 / n as f64;
        assert!(
            av >= 0.5,
            "диффузия от первого бита {av:.3} < 0.5 — спин+транспорт обязаны"
        );
        assert!(av <= AVALANCHE_CEILING + 0.05);
    }

    #[test]
    fn avalanche_cascades_across_blocks() {
        // CBC: смена бита в первом блоке меняет ВСЕ последующие блоки.
        let key = load(&chain_key());
        let msg: Vec<u8> = (0..3000u32).map(|i| (i * 37 + 11) as u8).collect();
        let mut rng = Rng::seed_from_u64(9);
        let (c1, _) = encrypt(&key, &msg, &mut rng.clone()).unwrap();
        let mut msg2 = msg.clone();
        msg2[10] ^= 1;
        let (c2, _) = encrypt(&key, &msg2, &mut rng).unwrap();
        let d = key.d_pol;
        let block_len = d.div_ceil(4);
        let body1 = &c1[TRITE_HEADER_SIZE + block_len..];
        let body2 = &c2[TRITE_HEADER_SIZE + block_len..];
        let n_blocks = body1.len() / block_len;
        assert!(n_blocks >= 3);
        for b in 0..n_blocks {
            let blk1 = &body1[b * block_len..(b + 1) * block_len];
            let blk2 = &body2[b * block_len..(b + 1) * block_len];
            assert_ne!(blk1, blk2, "блок {b} не изменился — CBC-каскад нарушен");
        }
    }

    #[test]
    fn avalanche_from_key_phase_change() {
        // Другие фазы ключа (в т.ч. одна изменённая) → другой ключевой
        // поток и другие терцили → мусор или структурный отказ.
        let k1 = load(&dense_key(3));
        let k2 = load(&dense_key(4));
        let msg: Vec<u8> = (0..500u32).map(|i| (i * 29 + 5) as u8).collect();
        let (cipher, _) = encrypt(&k1, &msg, &mut Rng::seed_from_u64(2)).unwrap();
        match decrypt(&k2, &cipher) {
            Ok((plain, _)) => {
                let ber = plain
                    .iter()
                    .zip(msg.iter())
                    .filter(|(a, b)| a != b)
                    .count() as f64
                    / msg.len() as f64;
                assert!(ber > 0.4, "чужая фаза даёт BER {ber:.2} — слишком мало");
            }
            Err(_) => {} // структурный отказ тоже честный исход
        }
    }

    #[test]
    fn mirrored_key_no_longer_decodes() {
        // J → −J: проектор тот же (RQ12 ключи-отражения расшифровывались),
        // но знак веса задаёт направление закрутки GF(3) — транспорт
        // другой, расшифровка — мусор. Исправленная граница RQ12.
        let mut rng = Rng::seed_from_u64(77);
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p: f64 = rng.next_f64() * 1.6 - 0.8;
            w.add_phase(i, p as f32).unwrap();
        }
        let mut pairs = Vec::new();
        for &shift in &[1u32, 5, 11] {
            for i in 0u32..(256 - shift) {
                let sign = if rng.next_u64() & 1 == 1 { 1.0 } else { -1.0 };
                pairs.push((i, i + shift, sign * (0.4 + 0.6 * rng.next_f64())));
            }
        }
        let gyro = GyroData::new(256, 8000, pairs, 256).unwrap();
        let k1 = load(&w.to_bytes_v3(&gyro).unwrap());
        // Зеркало: те же фазы, все веса с обратным знаком.
        let mirrored: Vec<(u32, u32, f64)> = gyro
            .pairs()
            .iter()
            .map(|&(i, j, w)| (i, j, -w))
            .collect();
        let gyro_m = GyroData::new(256, 8000, mirrored, 256).unwrap();
        let k2 = load(&w.to_bytes_v3(&gyro_m).unwrap());
        assert_eq!(k1.k_modes, k2.k_modes);
        let msg: Vec<u8> = (0..400u32).map(|i| (i * 7 + 1) as u8).collect();
        let (cipher, _) = encrypt(&k1, &msg, &mut Rng::seed_from_u64(4)).unwrap();
        match decrypt(&k2, &cipher) {
            // Зеркало даёт другой хеш → другие решётчатые русла →
            // структурный отказ по параметрам прецессии — честный исход.
            Err(e) => assert!(
                e.contains("не от этого шифртекста"),
                "неожиданная ошибка: {e}"
            ),
            Ok((plain, _)) => {
                let ber = plain
                    .iter()
                    .zip(msg.iter())
                    .filter(|(a, b)| a != b)
                    .count() as f64
                    / msg.len() as f64;
                assert!(
                    ber > 0.4,
                    "зеркальный ключ −J даёт BER {ber:.2} — граница RQ12 не закрыта"
                );
            }
        }
    }

    #[test]
    fn wrong_key_yields_garbage() {
        let k1 = load(&dense_key(1));
        let k2 = load(&dense_key(2)); // другая структура русел
        let msg: Vec<u8> = (0..700u32).map(|i| (i * 11 + 3) as u8).collect();
        let (cipher, _) = encrypt(&k1, &msg, &mut Rng::seed_from_u64(5)).unwrap();
        match decrypt(&k2, &cipher) {
            Ok((plain, _)) => {
                let ber = plain
                    .iter()
                    .zip(msg.iter())
                    .filter(|(a, b)| a != b)
                    .count() as f64
                    / msg.len() as f64;
                assert!(ber > 0.4, "чужой ключ даёт BER {ber:.2}");
            }
            Err(_) => {} // структурный отказ — честный исход
        }
    }

    #[test]
    fn iv_randomizes_ciphertext() {
        let key = load(&path_key());
        let msg = b"same message twice".to_vec();
        let (c1, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(1)).unwrap();
        let (c2, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(2)).unwrap();
        assert_ne!(c1, c2, "IV не рандомизировал шифртекст");
        assert_eq!(decrypt(&key, &c1).unwrap().0, msg);
        assert_eq!(decrypt(&key, &c2).unwrap().0, msg);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let key = load(&path_key());
        let msg: Vec<u8> = (0..333u32).map(|i| i as u8).collect();
        let (c1, r1) = encrypt(&key, &msg, &mut Rng::seed_from_u64(42)).unwrap();
        let (c2, r2) = encrypt(&key, &msg, &mut Rng::seed_from_u64(42)).unwrap();
        assert_eq!(c1, c2);
        assert_eq!(r1.digest_hex, r2.digest_hex);
    }

    #[test]
    fn corruption_detected_by_digest() {
        let key = load(&path_key());
        let (mut cipher, _) = encrypt(&key, b"integrity check", &mut Rng::seed_from_u64(3)).unwrap();
        let last = cipher.len() - 1;
        cipher[last] ^= 0x55;
        let err = decrypt(&key, &cipher).unwrap_err();
        assert!(err.contains("digest"), "ошибка: {err}");
    }

    #[test]
    fn bad_magic_and_truncated_rejected() {
        let key = load(&path_key());
        let (mut cipher, _) = encrypt(&key, b"x", &mut Rng::seed_from_u64(1)).unwrap();
        assert!(decrypt(&key, &cipher[..TRITE_HEADER_SIZE - 1]).is_err());
        cipher[0] = b'X';
        assert!(decrypt(&key, &cipher).is_err());
    }

    #[test]
    fn key_requires_gyro_section() {
        // v2-контейнер без русел J — отказ, а не мусор.
        let mut w = PqwWriter::new(8).unwrap();
        w.add_phase(0, 0.5).unwrap();
        let v2 = w.to_bytes().unwrap();
        let reader = PqwReader::from_bytes(&v2).unwrap();
        let err = TritKey::from_reader(&reader, 0).unwrap_err();
        assert!(err.contains("гироскоп"));
    }

    #[test]
    fn expansion_beats_f32_scheme() {
        // RQ12: ×43 на ключе RQ11 (d=4096). Здесь: блок d/4 B, ёмкость
        // ~cap трит → расширение ≤ ×2 для мультиблочного сообщения
        // (заголовок+IV амортизируются).
        let key = load(&chain_key()); // d=256: блок 64 B, ёмкость ~45 B
        let msg: Vec<u8> = (0..10_000u32).map(|i| (i * 41 + 3) as u8).collect();
        let (cipher, rep) = encrypt(&key, &msg, &mut Rng::seed_from_u64(6)).unwrap();
        assert_eq!(cipher.len(), rep.out_len);
        assert!(
            rep.expansion <= 2.0,
            "расширение ×{:.2} — триты не экономят место",
            rep.expansion
        );
        assert_eq!(decrypt(&key, &cipher).unwrap().0, msg);
    }

    #[test]
    fn calibration_is_deterministic() {
        let a = load(&hub_key());
        let b = load(&hub_key());
        assert_eq!(a.ticks, b.ticks);
        assert_eq!(a.strides, b.strides);
        assert_eq!(a.keystream, b.keystream);
    }
}

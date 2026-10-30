//! Спиновая лавина GF(3) — нелинейный слой диффузии трит-шифра (RQ23).
//!
//! ## Физика
//!
//! Трит-шифр RQ13 линеен над GF(3): транспорт по руслам — произведение
//! линейных сдвигов `shear` ( det = 1), ключевой поток и CBC-маска —
//! сдвиги по модулю 3. Лавина там реальна (~2/3 трит блока), но это
//! **линейная диффузия**: известные пары «открытый текст → шифртекст»
//! восстанавливают линейную структуру ключевого потока алгебраически.
//!
//! Спиновый слой ломает линейность **квадратичным T-проходом**:
//!
//! ```text
//! x[i] ← x[i] + c[i] · x[i+1] · x[i+2]   (mod 3),  i убывает,
//! c[i] = ks[i] + 1 ∈ {1, 2}              — лента ключевого потока,
//! индексы кольцом (mod d)                — склейка конца с началом.
//! ```
//!
//! Три свойства делают его криптографически осмысленным:
//!
//! 1. **Нелинейность.** Член `x·x` — квадратичная форма: образ суммы не
//!    равен сумме образов (`spin(a ⊕ b) ≠ spin(a) ⊕ spin(b)` в общем
//!    случае) — линейный криптоанализ больше не вычитает ключ.
//! 2. **Точная обратимость.** Треугольная структура: прямой проход — `i`
//!    убывает, при обновлении `x[i]` соседи `x[i+1], x[i+2]` уже
//!    финальны; обратный — `i` возрастает, соседи ещё финальны, а
//!    кольцевые случаи (`i = d−1, d−2` читают восстановленные `x[0],
//!    x[1]`) сходятся в точности — детерминизм побитовый, расшифровка
//!    точна без порогов.
//! 3. **Лавина.** Замена одной триты каскадно заражает все младшие
//!    индексы (продукт меняет оба соседа вниз), а кольцо переносит
//!    волну на старшие — за один раунд меняется полблока, за два с
//!    транспортом — весь блок до потолка GF(3) (2/3).
//!
//! В схеме v2 такт блока = транспорт + спин-раунд: линейная прецессия
//! русл разносит триты по блоку, квадратичный проход перемешивает
//! нелинейно. Слои чередуются `T` тактов — одинаково в обе стороны
//! (инверсия в обратном порядке).

use crate::rng::Rng;
use crate::trite::{transport_tick, TritKey, TritPair};

/// Минимальный размер ленты ключевого потока для спин-прохода
/// (коэффициент на каждую координату кольца).
pub const SPIN_MIN_TAPE: usize = 2;

/// Один нелинейный спин-раунд GF(3): квадратичный T-проход по блоку.
///
/// Прямой ход (`inverse = false`): `i` от `d−1` к `0`,
/// `x[i] += c[i]·x[(i+1) mod d]·x[(i+2) mod d] (mod 3)`.
/// Обратный ход (`inverse = true`): `i` от `0` к `d−1`, вычитание того
/// же произведения — соседи в этот момент ещё держат финальные
/// (прямого хода) значения, поэтому восстановление точное.
///
/// `ks` — лента ключевого потока (триты 0..2): коэффициент
/// `c[i] = ks[i] + 1 ∈ {1, 2}` — ненулевой всегда, слой зависит от ключа.
/// Требует `x.len() ≥ 3` и `ks.len() == x.len()` (лента покрывает блок).
pub fn spin_round(x: &mut [u8], ks: &[u8], inverse: bool) {
    let d = x.len();
    debug_assert!(d >= 3, "spin_round: блок ≥ 3 трит");
    debug_assert_eq!(ks.len(), d, "spin_round: лента покрывает блок");
    if d < 3 || ks.len() != d {
        return; // дегенерат — проход бессмыслен, ничего не делаем
    }
    let c: Vec<i32> = ks.iter().map(|&k| k as i32 + 1).collect();
    if !inverse {
        for i in (0..d).rev() {
            let a = x[(i + 1) % d] as i32;
            let b = x[(i + 2) % d] as i32;
            x[i] = (x[i] as i32 + c[i] * a * b).rem_euclid(3) as u8;
        }
    } else {
        for i in 0..d {
            let a = x[(i + 1) % d] as i32;
            let b = x[(i + 2) % d] as i32;
            x[i] = (x[i] as i32 - c[i] * a * b).rem_euclid(3) as u8;
        }
    }
}

/// `rounds` спин-раундов подряд.
pub fn spin_rounds(x: &mut [u8], ks: &[u8], rounds: u32, inverse: bool) {
    for _ in 0..rounds {
        spin_round(x, ks, inverse);
    }
}

/// Один такт блока v2: линейный транспорт русл + нелинейный спин-раунд.
///
/// Прямой ход: `transport_tick` затем `spin_round`. Обратный:
/// `spin_round(inverse)` затем `transport_tick(inverse)` — обращение в
/// обратном порядке слоёв.
pub fn block_tick(pairs: &[TritPair], ks: &[u8], x: &mut [u8], inverse: bool) {
    if !inverse {
        transport_tick(pairs, x, false);
        spin_round(x, ks, false);
    } else {
        spin_round(x, ks, true);
        transport_tick(pairs, x, true);
    }
}

/// `ticks` тактов блока v2 (транспорт + спин, чередуясь).
pub fn block_ticks(pairs: &[TritPair], ks: &[u8], x: &mut [u8], ticks: u32, inverse: bool) {
    for _ in 0..ticks {
        block_tick(pairs, ks, x, inverse);
    }
}

/// Лавина чистого преобразования: доля позиций, изменившихся после
/// `ticks` тактов от одиночной триты-зонда на позиции `probe`.
///
/// Зонд: `x[probe] = 1`, фон нулевой → «изменилось» = «ненулевое»
/// (та же метрика, что у калибровки RQ13, но на комбинированном
/// нелинейном преобразовании).
pub fn transform_spread(pairs: &[TritPair], ks: &[u8], d: usize, probe: usize, ticks: u32) -> f64 {
    let mut x = vec![0u8; d];
    x[probe % d] = 1;
    block_ticks(pairs, ks, &mut x, ticks, false);
    x.iter().filter(|&&v| v != 0).count() as f64 / d as f64
}

/// Сводка измерения спиновой лавины на больших блоках данных.
#[derive(Clone, Debug)]
pub struct AvalancheStats {
    /// Зондов (флипнутых битов сообщения).
    pub probes: usize,
    /// Блоков шифртекста.
    pub blocks: usize,
    /// Трит в блоке (= d_pol).
    pub block_trites: usize,
    /// Размер открытого текста (байт).
    pub msg_len: usize,
    /// Тактов транспорта на блок.
    pub ticks: u32,
    /// Спин-раундов на блок (= тактов, слой чередуется с транспортом).
    pub nl_rounds: u32,
    /// Лавина по всем зондам: средняя доля изменившихся трит шифртекста
    /// (весь шифртекст — префиксные блоки до зонда не тронуты CBC).
    pub avalanche_mean: f64,
    /// Лавина от блока зонда включительно (честная диффузия без
    /// префиксного разведения — все блоки каскада от флипа вперёд).
    pub avalanche_from_probe: f64,
    /// Минимальная лавина по зондам.
    pub avalanche_min: f64,
    /// Максимальная лавина по зондам.
    pub avalanche_max: f64,
    /// Потолок лавины GF(3) (2/3 — доля различий двух случайных трит).
    pub ceiling: f64,
    /// Лавина каскада CBC: средняя по блокам ПОСЛЕ блока с флипом
    /// (как далеко распространяется замена по цепочке).
    pub cascade_after: f64,
    /// Лавина трансформа (зонды состояния, без шифрования):
    /// средняя доля заражённых позиций блока после ticks тактов.
    pub transform_spread_mean: f64,
    /// Хи-квадрат равномерности позиций изменений (df = блоков−1):
    /// малое значение — изменения распределены по блокам равномерно.
    pub chi2_blocks: f64,
    /// Время измерения.
    pub elapsed: std::time::Duration,
}

/// Измерение нелинейной спиновой лавины на большом сообщении (RQ23).
///
/// Физика опыта: сообщение шифруется дважды — базовое и с перевёрнутым
/// битом (IV фиксирован сидом: цепочки CBC сравнимы), триты шифртекстов
/// сравниваются блок за блоком. Каждый зонд флипает свой бит в
/// **открытом тексте**; счётчик ловит и внутриблочную диффузию (спин +
/// транспорт), и межблочный каскад CBC. Плюс чистые зонды состояния —
/// лавина самого преобразования без шифрования.
///
/// `probes` битовых зондов распределяются равномерно по сообщению
/// (минимум 1). Детерминизм полный: сид IV фиксирован, ноль ГПСЧ в
/// измерениях (ГПСЧ только для генерации псевдослучайного текста
/// вызовом `msg_from_seed`).
pub fn measure_avalanche(
    key: &TritKey,
    msg: &[u8],
    probes: usize,
) -> Result<AvalancheStats, String> {
    if msg.is_empty() {
        return Err("лавина: пустое сообщение — нечего зонди ярвать".into());
    }
    if key.d_pol < 3 {
        return Err("лавина: спин-слой требует d_pol ≥ 3".into());
    }
    let t0 = std::time::Instant::now();
    let iv_seed = 0xA11CE_u64; // фиксированный IV: цепочки CBC сравнимы между зондами
    let (_base_iv, data, n_blocks) = encrypt_body_seeded(key, msg, iv_seed)?;

    let probes = probes.max(1);
    let n = data.len() * 4;
    let base_trites = crate::trite::unpack_trites(&data, n);

    let mut mean = 0.0_f64;
    let mut mean_from_probe = 0.0_f64;
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut cascade_sum = 0.0_f64;
    let mut cascade_blocks = 0usize;
    // Счётчик изменений по блокам для хи-квадрат равномерности.
    let mut block_hits = vec![0u64; n_blocks];
    let mut total_hits = 0u64;

    for p in 0..probes {
        // Зонд: флип бита в позиции, равномерно распределённой по тексту.
        let pos = (p * msg.len()) / probes.max(1);
        let mut flipped = msg.to_vec();
        flipped[pos.min(msg.len() - 1)] ^= 1;
        let (_, data2, _) = encrypt_body_seeded(key, &flipped, iv_seed)?;
        let probe_trites = crate::trite::unpack_trites(&data2, n);

        // Изменения по тритам; блок k = trit_idx / d_pol.
        let mut changed = 0usize;
        let mut changed_by_block = vec![0u64; n_blocks];
        for (k, (a, b)) in base_trites.iter().zip(probe_trites.iter()).enumerate() {
            if a != b {
                changed += 1;
                let blk = k / key.d_pol;
                if blk < n_blocks {
                    changed_by_block[blk] += 1;
                }
            }
        }
        let frac = changed as f64 / n as f64;
        mean += frac;
        min = min.min(frac);
        max = max.max(frac);

        // Каскад CBC: блок зонда и все последующие (префикс не меняется).
        // Блок зонда (в тексте): pos байт → трита → блок.
        let trites_per_block = key.capacity_trites.max(1);
        let probe_block = (pos * 8 / trites_per_block.max(1)).min(n_blocks.saturating_sub(1));
        let mut changed_from_probe = 0u64;
        let mut trites_from_probe = 0u64;
        for (blk, &hits) in changed_by_block.iter().enumerate().skip(probe_block) {
            changed_from_probe += hits;
            trites_from_probe += key.d_pol as u64;
            if blk > probe_block {
                cascade_sum += hits as f64 / key.d_pol as f64;
                cascade_blocks += 1;
            }
        }
        if trites_from_probe > 0 {
            mean_from_probe += changed_from_probe as f64 / trites_from_probe as f64;
        }
        for (blk, &hits) in changed_by_block.iter().enumerate() {
            block_hits[blk] += hits;
            total_hits += hits;
        }
    }
    mean /= probes as f64;
    mean_from_probe /= probes as f64;

    // Хи-квадрат равномерности по блокам (df = n_blocks − 1).
    let expected = if n_blocks > 0 && total_hits > 0 {
        total_hits as f64 / n_blocks as f64
    } else {
        0.0
    };
    let chi2 = if expected > 0.0 {
        block_hits
            .iter()
            .map(|&h| {
                let diff = h as f64 - expected;
                diff * diff / expected
            })
            .sum()
    } else {
        0.0
    };

    // Лавина чистого трансформа: 4 зонда состояния (как калибровка).
    let d = key.d_pol;
    let ks = key.keystream();
    let cap = key.capacity_trites.max(1);
    let spread_probe = |idx: usize| {
        let pos = key
            .positions()
            .get(idx % cap)
            .copied()
            .unwrap_or(0) as usize;
        transform_spread(key.pairs(), ks, d, pos, key.ticks_v2())
    };
    let spread_mean = if cap >= 4 {
        (spread_probe(0) + spread_probe(cap / 4) + spread_probe(cap / 2) + spread_probe(cap - 1))
            / 4.0
    } else if cap >= 1 {
        spread_mean_fallback(key, ks, d)
    } else {
        0.0
    };

    Ok(AvalancheStats {
        probes,
        blocks: n_blocks,
        block_trites: key.d_pol,
        msg_len: msg.len(),
        ticks: key.ticks_v2(),
        nl_rounds: key.ticks_v2(),
        avalanche_mean: mean,
        avalanche_from_probe: mean_from_probe,
        avalanche_min: min,
        avalanche_max: max,
        ceiling: crate::trite::AVALANCHE_CEILING,
        cascade_after: if cascade_blocks > 0 {
            cascade_sum / cascade_blocks as f64
        } else {
            0.0
        },
        transform_spread_mean: spread_mean,
        chi2_blocks: chi2,
        elapsed: t0.elapsed(),
    })
}

fn spread_mean_fallback(key: &TritKey, ks: &[u8], d: usize) -> f64 {
    // Мало холодных позиций: зонды по всему кольцу.
    let n = 4.min(d);
    let mut sum = 0.0;
    for k in 0..n {
        sum += transform_spread(key.pairs(), ks, d, k * d / n.max(1), key.ticks_v2());
    }
    sum / n as f64
}

/// Детерминированное тело шифрования с фиксированным IV (для зондов
/// лавины: цепочки CBC между прогонами сравнимы). Возвращает
/// `(IV-байты, блоки Packed4, число блоков)`.
fn encrypt_body_seeded(
    key: &TritKey,
    msg: &[u8],
    iv_seed: u64,
) -> Result<(Vec<u8>, Vec<u8>, usize), String> {
    let mut rng = Rng::seed_from_u64(iv_seed);
    let (iv_bytes, data, n_blocks) = key.encrypt_body(msg, &mut rng);
    Ok((iv_bytes, data, n_blocks))
}

/// Проверка нелинейности: существует ли пара состояний, на которой
/// спин-слой нарушает аддитивность `spin(a ⊕ b) = spin(a) ⊕ spin(b)`
/// (GF(3)-суперпозиция). Линейный транспорт такой пары не имеет.
///
/// Возвращает `true`, если нелинейность доказана контрпримером.
pub fn spin_is_nonlinear(ks: &[u8], d: usize, ticks: u32) -> bool {
    if d < 4 {
        return false;
    }
    let pairs: Vec<TritPair> = Vec::new(); // чистый спин-слой без транспорта
    let mut a = vec![0u8; d];
    let mut b = vec![0u8; d];
    // a = e_0 + e_1, b = e_1 + e_2: продукты перекрываются на e_1.
    a[0] = 1;
    a[1] = 1;
    b[1] = 1;
    b[2] = 1;
    let a0 = a.clone();
    let b0 = b.clone();
    // Линейная комбинация: a + b (mod 3).
    let mut sum = a0.clone();
    for (s, &v) in sum.iter_mut().zip(b0.iter()) {
        *s = (*s + v) % 3;
    }
    block_ticks(&pairs, ks, &mut a, ticks, false);
    block_ticks(&pairs, ks, &mut b, ticks, false);
    block_ticks(&pairs, ks, &mut sum, ticks, false);
    // spin(a) + spin(b):
    let mut ab = a.clone();
    for (s, &v) in ab.iter_mut().zip(b.iter()) {
        *s = (*s + v) % 3;
    }
    ab != sum // нелинейность ⇔ образ суммы ≠ сумме образов
}

/// Детерминированный псевдослучайный текст для больших блоков
/// (зерно → поток байтов xoshiro, ноль аллокаций вне результата).
pub fn msg_from_seed(seed: u64, len: usize) -> Vec<u8> {
    let mut rng = Rng::seed_from_u64(seed);
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let chunk = rng.next_u64().to_le_bytes();
        let take = (len - out.len()).min(8);
        out.extend_from_slice(&chunk[..take]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trite::TritPair;

    fn tape(d: usize) -> Vec<u8> {
        // Детерминированная лента 0..2.
        (0..d).map(|i| (i % 3) as u8).collect()
    }

    /// Триты из сид-потока (значения 0..2 — GF(3), не сырые байты!).
    fn trits_from_seed(seed: u64, d: usize) -> Vec<u8> {
        msg_from_seed(seed, d).iter().map(|b| b % 3).collect()
    }

    /// Решётчатые русла как у шифра: три нечётных сдвига, полное
    /// покрытие кольца; закрутка c ∈ {1, 2} — как у stride_pairs RQ13
    /// (c=1 «переносит» триту, c=2 «размножает» — смешение закруток
    /// и даёт диффузию).
    fn lattice_pairs(d: usize) -> Vec<TritPair> {
        let mut out = Vec::new();
        for (f, &s) in [1usize, 3, 5].iter().enumerate() {
            let c = if f == 1 { 2 } else { 1 };
            for i in 0..d as u32 {
                let j = (i + s as u32) % d as u32;
                if i != j {
                    out.push(TritPair { i, j, c: c as u8 });
                }
            }
        }
        out
    }

    #[test]
    fn spin_round_inverts_exactly() {
        // Случайные GF(3)-состояния — прямой проход, затем обратный:
        // тождество. (Вход обязан быть тритами: слой работает в GF(3).)
        let d = 61usize;
        let ks = tape(d);
        let mut x = trits_from_seed(42, d);
        let orig = x.clone();
        spin_rounds(&mut x, &ks, 5, false);
        assert_ne!(x, orig, "пять раундов обязаны менять состояние");
        spin_rounds(&mut x, &ks, 5, true);
        assert_eq!(x, orig, "инверсия побитово точна");
    }

    #[test]
    fn spin_round_inverts_exactly_long_chain() {
        // Длинная цепочка раундов на большом блоке.
        let d = 1024usize;
        let ks = tape(d);
        let mut x = trits_from_seed(7, d);
        let orig = x.clone();
        spin_rounds(&mut x, &ks, 64, false);
        spin_rounds(&mut x, &ks, 64, true);
        assert_eq!(x, orig);
    }

    #[test]
    fn spin_round_inverts_on_random_tapes() {
        // Случайные ленты коэффициентов — инверсия точна на каждой.
        let d = 97usize;
        for seed in 0..8u64 {
            let ks = trits_from_seed(seed, d);
            let mut x = trits_from_seed(seed + 100, d);
            let orig = x.clone();
            spin_rounds(&mut x, &ks, 3, false);
            spin_rounds(&mut x, &ks, 3, true);
            assert_eq!(x, orig, "лента-сид {seed}");
        }
    }

    #[test]
    fn spin_layer_is_nonlinear() {
        // Контрпример аддитивности — доказательство нелинейности.
        assert!(spin_is_nonlinear(&tape(32), 32, 1));
        // Для сравнения: линейный транспорт аддитивен всегда
        // (проверять нечего — это свойство GF(3)-матрицы).
    }

    #[test]
    fn single_trit_avalanche_spreads() {
        // Одна трита + решётчатая топология шифра (три сдвига, полное
        // покрытие): комбинированный такт заражает ≥ трети блока за
        // 8 тактов. Изолированная трита не распространяется чистым
        // спином (произведение с нулём — нуль): транспорт раскидывает,
        // спин перемешивает нелинейно.
        let d = 128usize;
        let ks = tape(d);
        let pairs = lattice_pairs(d);
        let spread = transform_spread(&pairs, &ks, d, 5, 8);
        assert!(
            spread >= 0.3,
            "лавина от одной триты {spread:.3} < 0.3 — диффузия слаба"
        );
        // Кластер из соседних трит — спин перемешивает активнее
        // (произведения пар ненулевых соседей).
        let mut x = vec![0u8; d];
        for k in 0..4 {
            x[k] = 1;
        }
        block_ticks(&pairs, &ks, &mut x, 4, false);
        let cluster_spread = x.iter().filter(|&&v| v != 0).count() as f64 / d as f64;
        assert!(
            cluster_spread >= 0.3,
            "лавина кластера {cluster_spread:.3} < 0.3"
        );
    }

    #[test]
    fn block_tick_inverts_with_transport() {
        // Комбинированный такт (транспорт + спин) обратим точно
        // на GF(3)-состояниях.
        let d = 97usize;
        let ks = tape(d);
        let pairs: Vec<TritPair> = vec![
            TritPair { i: 3, j: 17, c: 1 },
            TritPair { i: 5, j: 90, c: 2 },
            TritPair { i: 40, j: 41, c: 1 },
        ];
        let mut x = trits_from_seed(99, d);
        let orig = x.clone();
        block_ticks(&pairs, &ks, &mut x, 16, false);
        assert_ne!(x, orig);
        block_ticks(&pairs, &ks, &mut x, 16, true);
        assert_eq!(x, orig);
    }

    #[test]
    fn measure_avalanche_reports_honest_metrics() {
        // Полный конвейер измерения на синтетическом ключе: лавина
        // в границах, от блока зонда — заметно выше средней (CBC-префикс),
        // каскад после зонда жив, хи-квадрат конечен.
        use crate::trite::TritKey;
        let d = 1024u32;
        let mut pairs = Vec::new();
        let mut k = 1u64;
        let next = |k: &mut u64| -> u64 {
            *k = k
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *k
        };
        while pairs.len() < 300 {
            let h = next(&mut k);
            let i = (h % (d as u64 / 2)) as u32;
            let j = d - 1 - ((h >> 32) % 64) as u32;
            if i < j && !pairs.iter().any(|&(pi, pj, _)| pi == i && pj == j) {
                pairs.push((i, j, if h & 1 == 0 { 1.0 } else { -1.0 }));
            }
        }
        let g = pqw::GyroData::new(8, 4321, pairs, d).unwrap();
        let lex = pqw::Lexicon::new(vec![(0, "фаза".into()), (1, "трит".into())], d).unwrap();
        let mut w = pqw::PqwWriter::new(d).unwrap();
        w.add_phase(0, 0.9).unwrap();
        let bytes = w.to_bytes_v4(&g, &lex).unwrap();
        let reader = pqw::PqwReader::from_bytes(&bytes).unwrap();
        let key = TritKey::from_reader(&reader, 0).unwrap();

        let msg = msg_from_seed(42, 8192);
        let stats = measure_avalanche(&key, &msg, 8).unwrap();
        assert_eq!(stats.probes, 8);
        assert!(stats.blocks >= 2, "многоблочный текст");
        assert!(stats.avalanche_mean > 0.0);
        assert!(stats.avalanche_max <= stats.ceiling + 0.05);
        // От блока зонда вперёд диффузия выше средней по всему тексту
        // (префиксные блоки CBC не трогает).
        assert!(
            stats.avalanche_from_probe >= stats.avalanche_mean,
            "from_probe {:.3} < mean {:.3}",
            stats.avalanche_from_probe,
            stats.avalanche_mean
        );
        assert!(stats.avalanche_from_probe >= 0.25);
        assert!(stats.cascade_after > 0.0);
        assert!(stats.chi2_blocks.is_finite());
        assert!(stats.transform_spread_mean >= 0.3);
        // Детерминизм измерения.
        let stats2 = measure_avalanche(&key, &msg, 8).unwrap();
        assert_eq!(stats.avalanche_mean, stats2.avalanche_mean);
    }

    #[test]
    fn msg_from_seed_is_deterministic() {
        assert_eq!(msg_from_seed(5, 100), msg_from_seed(5, 100));
        assert_ne!(msg_from_seed(5, 100), msg_from_seed(6, 100));
        assert_eq!(msg_from_seed(5, 3).len(), 3);
    }
}

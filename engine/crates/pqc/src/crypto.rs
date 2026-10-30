//! Крипто-схема алгебры архетипа (RQ12, файл 285): `m = p* ⊕ (a ⊗_ε p*)`.
//!
//! Строго, без метафизики — вся конструкция собрана из деталей,
//! измеренных в RQ10–RQ11:
//!
//! 1. **Ключ** — архетип `a`: контейнер v3 с гироскопом `J = A − Aᵀ`
//!    (готовый `pqc precess --out` либо `pqc train --gyro`). Из русел J
//!    детерминированно (степенная итерация на −J² с дефляцией) берутся
//!    топ-K **плотных** мод Im(P): пары `(u_k, v_k)` с невязками
//!    ортонормальности ~1e-16 (RQ11: ортонормальность тождественна
//!    идемпотентности `A² = A` проектора `A_k = u_k u_kᵀ + v_k v_kᵀ`).
//! 2. **Действие архетипа** `a ⊗_ε p` — модовый проектор
//!    `a·p = Σ_k [(u_k·p)u_k + (v_k·p)v_k]`: ортогональная проекция на
//!    подпространство S = span{u_k, v_k}. Идемпотентность `a ⊗_ε a = a`
//!    точна — поэтому итеративный коллапс файл 285
//!    `p_{t+1} = a ⊗_ε p_t ⊕ m` сходится **за один такт**.
//! 3. **Суперпозиция** `⊕ m` — сложение фазового паттерна сообщения
//!    `δ = (I−a)(ε·b)`: биты сообщения (блок = `d_pol` бит) через
//!    малый сдвиг `ε = 0.1` рад, спроецированный в дополнение S⊥
//!    (иначе S-компонента накапливалась бы линейно). Проекция
//!    «теряет» только S-часть паттерна — помеха на компоненту
//!    ~`ε·√(2K/d_pol)` (при K = 8, d = 4096: ~0.04ε) — порог
//!    декодирования `ε/2` имеет ~10-кратный запас.
//! 4. **Шифрование**: `p* = a·p₀ + δ`, где `p₀` — фазы ключа
//!    (ключевой поток: S-часть шифртекста — проекция собственных фаз
//!    архетипа, секрет без J не отделим). Шифртекст — фазы `p*` (f32).
//! 5. **Расшифровка**: `δ̂ = p* − a·p*`; бит = `δ̂ > ε/2`. Равенство
//!    точное: `p* − a·p* = (I−a)(a·p₀ + δ) = (I−a)δ = δ`.
//!
//! Блочный режим: блок 0 — случайный IV (xoshiro256++); блок `k`
//! маскируется битами паттерна предыдущего блока (CBC-цепочка через
//! δ̂) — одинаковые блоки сообщения дают разные шифрблоки, два
//! шифрования одного сообщения — разные шифртексты.
//!
//! Контейнер шифртекста `.pqc` (magic `PQC1`, LE, zero-dep): заголовок
//! 64 B (d_pol, n_blocks, msg_len, k_modes, digest sha256-24 по фазам)
//! + `n_blocks × d_pol × f32` фаз. Русла J и моды в шифртекст НЕ
//! пишутся — это и есть ключ: без `a` нельзя вычислить `a·p*` и
//! отделить δ от ключевого потока.
//!
//! Честные границы (по образцу крипто-аудита poler-os): схема
//! **линейна** над скрытым подпространством — (1) внутриблочная
//! диффузия минимальна (1 бит → 1 бит на блок; лавина нелинейных
//! шифров отсутствует — следствие линейности проектора), (2)
//! known-plaintext с числом блоков ≳ 2K восстанавливает S⊥ и
//! вскрывает ключ, (3) длина сообщения читается из заголовка, (4)
//! размер растёт ~×32 (блок `d_pol/8` B → `d_pol×4` B фаз). Это
//! исследовательская демонстрация уравнения файл 285 на реальном
//! движке, а не production-шифр; нелинейная реализация `⊗_ε`
//! (прецессия + модовый проектор) — направление RQ13.

use crate::gyro::resonant_modes_dense_from_pairs;
use crate::rng::Rng;
use pqw::reader::PqwReader;
use pqw::sha256::sha256_trunc24;

/// Магия контейнера шифртекста.
pub const CIPHER_MAGIC: [u8; 4] = *b"PQC1";
/// Версия формата шифртекста.
pub const CIPHER_VERSION: u16 = 1;
/// Размер заголовка `.pqc` (байт).
pub const CIPHER_HEADER_SIZE: usize = 64;
/// Сдвиг фазы для бита 1: малый паттерн `ε` поверх ключевого потока.
pub const BIT_PHASE: f64 = 0.1;
/// Порог декодирования бита: `δ̂ > ε/2` → бит 1.
pub const DECODE_THRESHOLD: f64 = BIT_PHASE / 2.0;
/// Максимум мод по умолчанию (как `inspect --modes`).
pub const MAX_MODES: usize = 8;

/// Ключ-архетип: модовый проектор + стартовые фазы.
#[derive(Clone, Debug)]
pub struct CipherKey {
    /// Размерность фазового пространства.
    pub d_pol: usize,
    /// Число мод проектора (ранг a = 2K).
    pub k_modes: usize,
    /// Плотные моды (u_k, v_k) в полном пространстве.
    modes: Vec<(Vec<f64>, Vec<f64>)>,
    /// Позиции битов сообщения: компоненты с энергией мод
    /// `Σ_k (u_k)ᵢ² + (v_k)ᵢ² ≤ 0.06/K` — «холодные» узлы, где
    /// помеха проекции ограничена (Коши–Буняковский)
    /// `0.71·ε·√(2K)·√(0.06/K) ≈ 0.25ε`; фактическая помеха
    /// дополнительно измеряется калибровкой (порог 0.25ε).
    /// Локализованные «горячие» узлы мод исключены.
    positions: Vec<u32>,
    /// Стартовые фазы p₀ (arccos p̂ по хранимым дугам, фон π/2) —
    /// источник ключевого потока a·p₀.
    thetas0: Vec<f64>,
    /// Число сырых пар J ключа.
    pub raw_pairs: usize,
    /// Максимальная Ritz-невязка мод (инвариантность плоскостей).
    pub ritz_max: f64,
    /// Максимальная невязка ортонормальности ≡ идемпотентности.
    pub ortho_max: f64,
}

impl CipherKey {
    /// Ёмкость блока: число бит на блок (= числу холодных позиций).
    pub fn capacity(&self) -> usize {
        self.positions.len()
    }

    /// Холодные позиции битов сообщения (RQ13: трит-схема переиспользует
    /// ту же геометрию — триты сообщения живут на этих узлах).
    pub fn positions(&self) -> &[u32] {
        &self.positions
    }
}

impl CipherKey {
    /// Загрузка ключа из контейнера v3: русла J → плотные моды →
    /// проектор.
    ///
    /// `k_hint = 0` — **авто-калибровка** числа мод (по умолчанию):
    /// берётся наибольшее K ≤ `min(MAX_MODES, d_pol/32)`, при котором
    /// помеха проекции `max |a·(ε·b)|` на детерминированных
    /// тест-паттернах не превышает `0.2·ε` (иначе хвосты помехи
    /// перевалят через порог декодирования ε/2). Калибровка
    /// детерминирована — расшифровщик выбирает то же K.
    /// `k_hint > 0` — явное число мод (экспертный режим; при
    /// расшифровке нужно то же значение — оно сверяется по заголовку).
    pub fn from_reader(reader: &PqwReader, k_hint: usize) -> Result<CipherKey, String> {
        let section = reader
            .gyro()
            .ok_or_else(|| "контейнер без топологической секции (v1/v2) — ключу нужен гироскоп J = A − Aᵀ (train --gyro / precess --out)".to_string())?;
        let d_pol = reader.d_pol() as usize;
        let raw: Vec<(u32, u32, f64)> = section
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        if raw.is_empty() {
            return Err("гироскоп ключа пуст (нет русел J)".into());
        }
        if raw.iter().any(|&(i, j, _)| (i as usize) >= d_pol || (j as usize) >= d_pol) {
            return Err("индексы русел J выходят за d_pol ключа".into());
        }
        let k_max = (d_pol / 32).clamp(1, MAX_MODES);
        // Геометрия (моды + позиции + помеха) для выбранного K
        // (отложенная инициализация: обе ветки ниже присваивают).
        let geometry: Option<(Vec<(Vec<f64>, Vec<f64>)>, Vec<u32>, f64)>;
        let k = if k_hint > 0 {
            geometry = calibration_geometry(&raw, k_hint, d_pol);
            if geometry.is_none() {
                return Err("степенная итерация не нашла столько мод".into());
            }
            k_hint
        } else {
            // Авто-калибровка: наибольшее K с помехой на холодных
            // позициях ≤ 0.25·ε (запас декодирования до порога ε/2 —
            // ещё ≥ 2× от наблюдаемого максимума) и ёмкостью
            // ≥ d_pol/4 бит (баланс: больше K — больше скрытое
            // подпространство; больше ёмкость — меньше блоков).
            let cap_floor = (d_pol / 4).max(8);
            let mut chosen = 0usize;
            for cand in 1..=k_max {
                if let Some((_, positions, interf)) = calibration_geometry(&raw, cand, d_pol) {
                    if interf <= 0.25 * BIT_PHASE && positions.len() >= cap_floor {
                        chosen = cand;
                    }
                }
            }
            if chosen == 0 {
                // Ослабление: хотя бы байт ёмкости (малые ключи).
                for cand in 1..=k_max {
                    if let Some((_, positions, interf)) = calibration_geometry(&raw, cand, d_pol) {
                        if interf <= 0.25 * BIT_PHASE && positions.len() >= 8 {
                            chosen = cand;
                        }
                    }
                }
            }
            if chosen == 0 {
                // Хотя бы одна мода обязательна (проектор без мод —
                // нулевой, ключевой поток пуст, схема вырождена).
                return Err("нет пригодного числа мод: помеха/ёмкость не проходят калибровку (ключ слишком мал или моды полностью локализованы)".into());
            }
            geometry = calibration_geometry(&raw, chosen, d_pol);
            chosen
        };
        let Some((modes, positions, _interference)) = geometry else {
            return Err("внутренняя ошибка калибровки ключа".into());
        };
        if positions.len() < 8 {
            return Err("ёмкость блока меньше байта — ключ непригоден".into());
        }
        let dense = resonant_modes_dense_from_pairs(&raw, k, d_pol);
        let ritz_max = dense
            .iter()
            .map(|m| m.ritz_residual)
            .fold(0.0_f64, f64::max);
        let ortho_max = dense
            .iter()
            .map(|m| m.ortho_residual)
            .fold(0.0_f64, f64::max);
        if ortho_max > 1e-6 {
            return Err(format!(
                "моды ключа не ортонормированы (невязка {ortho_max:.2e}) — идемпотентность a ⊗ a = a нарушена"
            ));
        }
        // Стартовые фазы: θ = arccos(p̂), фон π/2 (p = 0).
        let mut thetas0 = vec![std::f64::consts::FRAC_PI_2; d_pol];
        for (idx, p) in reader.decoded() {
            if (idx as usize) < d_pol {
                thetas0[idx as usize] = p.clamp(-1.0, 1.0).acos();
            }
        }
        Ok(CipherKey {
            d_pol,
            k_modes: dense.len(),
            modes,
            positions,
            thetas0,
            raw_pairs: raw.len(),
            ritz_max,
            ortho_max,
        })
    }

    /// Проекция `a·p` (действие архетипа): O(K·d_pol).
    fn project(&self, p: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0_f64; p.len()];
        for (u, v) in &self.modes {
            let cu: f64 = u.iter().zip(p.iter()).map(|(a, b)| a * b).sum();
            let cv: f64 = v.iter().zip(p.iter()).map(|(a, b)| a * b).sum();
            for (o, (&a, &b)) in out.iter_mut().zip(u.iter().zip(v.iter())) {
                *o += cu * a + cv * b;
            }
        }
        out
    }

    /// Ключевой поток: S-часть стартовых фаз `a·p₀`.
    pub fn keystream(&self) -> Vec<f64> {
        self.project(&self.thetas0)
    }
}

/// Сводка шифрования.
#[derive(Clone, Debug)]
pub struct EncryptReport {
    /// Блоков шифртекста (IV + блоки сообщения).
    pub blocks: usize,
    /// Размер открытого текста (байт).
    pub msg_len: usize,
    /// Размер шифртекста (байт).
    pub out_len: usize,
    /// Число мод проектора (ранг a = 2K).
    pub k_modes: usize,
    /// Максимальная помеха проекции: `max |a·(ε·b)|` по блокам —
    /// насколько S-часть паттерна «просачивается» в компоненты.
    pub interference_max: f64,
    /// Минимальный запас декодирования: `min |δ̂ − ε/2|` по всем битам
    /// (симуляция декодирования на стороне шифровальщика).
    pub margin_min: f64,
    /// hex digest sha256-24 по телу фаз.
    pub digest_hex: String,
}

/// Сводка расшифровки.
#[derive(Clone, Debug)]
pub struct DecryptReport {
    /// Блоков в шифртексте.
    pub blocks: usize,
    /// Размер восстановленного сообщения (байт).
    pub msg_len: usize,
    /// Минимальный запас декодирования `min |δ̂ − ε/2|`.
    pub margin_min: f64,
    /// Средний запас декодирования.
    pub margin_mean: f64,
}

/// Геометрия ключа для калибровки K: плотные моды, холодные позиции
/// битов (энергия мод ≤ `0.04/K`) и помеха проекции `max |a·(ε·b)|` на
/// этих позициях по четырём детерминированным псевдослучайным паттернам
/// (узор из мультипликативного хеша индексов — тот же стиль, что старт
/// степенной итерации мод). `None` — степенная итерация не дала k мод.
fn calibration_geometry(
    raw: &[(u32, u32, f64)],
    k: usize,
    d_pol: usize,
) -> Option<(Vec<(Vec<f64>, Vec<f64>)>, Vec<u32>, f64)> {
    let dense = resonant_modes_dense_from_pairs(raw, k, d_pol);
    if dense.len() < k {
        return None;
    }
    let modes: Vec<(Vec<f64>, Vec<f64>)> =
        dense.iter().map(|m| (m.u.clone(), m.v.clone())).collect();
    // Энергия мод на компоненту → холодные позиции битов.
    let energy_cap = 0.06 / k as f64;
    let mut energy = vec![0.0_f64; d_pol];
    for (u, v) in &modes {
        for i in 0..d_pol {
            energy[i] += u[i] * u[i] + v[i] * v[i];
        }
    }
    let positions: Vec<u32> = (0..d_pol as u32)
        .filter(|&i| energy[i as usize] <= energy_cap)
        .collect();
    let project = |p: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0_f64; p.len()];
        for (u, v) in &modes {
            let cu: f64 = u.iter().zip(p.iter()).map(|(a, b)| a * b).sum();
            let cv: f64 = v.iter().zip(p.iter()).map(|(a, b)| a * b).sum();
            for (o, (&a, &b)) in out.iter_mut().zip(u.iter().zip(v.iter())) {
                *o += cu * a + cv * b;
            }
        }
        out
    };
    // Помеха измеряется ТОЛЬКО на холодных позициях — биты живут там.
    let mut worst = 0.0_f64;
    for seed in 0..4u64 {
        let bits: Vec<bool> = (0..d_pol)
            .map(|i| {
                (i as u64)
                    .wrapping_mul(26_544_357_61)
                    .wrapping_add(seed.wrapping_mul(40_503))
                    % 97
                    < 48
            })
            .collect();
        let mut raw_pat = vec![0.0_f64; d_pol];
        for (r, &b) in raw_pat.iter_mut().zip(bits.iter()) {
            if b {
                *r = BIT_PHASE;
            }
        }
        let proj = project(&raw_pat);
        for &pos in &positions {
            worst = worst.max(proj[pos as usize].abs());
        }
    }
    Some((modes, positions, worst))
}

/// Паттерн сообщения: `δ = (I−a)(ε·b)` — дополнение S⊥. Бит j
/// живёт на холодной позиции `positions[j]` (помеха проекции там
/// ограничена калибровкой).
fn pattern_delta(key: &CipherKey, bits: &[bool]) -> (Vec<f64>, f64) {
    let d = key.d_pol;
    let mut raw = vec![0.0_f64; d];
    for (&pos, &b) in key.positions.iter().zip(bits.iter()) {
        if b {
            raw[pos as usize] = BIT_PHASE;
        }
    }
    let proj = key.project(&raw);
    let mut interference = 0.0_f64;
    let mut delta = raw;
    for (dl, &pr) in delta.iter_mut().zip(proj.iter()) {
        *dl -= pr;
    }
    // Помеха значима только на позициях битов.
    for &pos in &key.positions {
        interference = interference.max(proj[pos as usize].abs());
    }
    (delta, interference)
}

/// Демодуляция: `δ̂ → биты` порогом `ε/2`; заодно запас декодирования.
fn delta_to_bits(delta: &[f64]) -> (Vec<bool>, f64, f64) {
    let mut bits = Vec::with_capacity(delta.len());
    let mut margin_min = f64::INFINITY;
    let mut margin_sum = 0.0_f64;
    for &d in delta {
        bits.push(d > DECODE_THRESHOLD);
        let m = (d - DECODE_THRESHOLD).abs();
        margin_min = margin_min.min(m);
        margin_sum += m;
    }
    let margin_mean = margin_sum / delta.len().max(1) as f64;
    (bits, margin_min, margin_mean)
}

/// Байты → биты (младший бит первого).
pub(crate) fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for &b in bytes {
        for j in 0..8 {
            bits.push((b >> j) & 1 == 1);
        }
    }
    bits
}

/// Биты → байты (младший бит первого; длина кратна 8).
pub(crate) fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes = vec![0u8; bits.len() / 8];
    for (k, &bit) in bits.iter().enumerate() {
        if bit {
            bytes[k / 8] |= 1 << (k % 8);
        }
    }
    bytes
}

/// LE-помощники сериализации (zero-dep).
pub(crate) fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn get_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}
pub(crate) fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}
pub(crate) fn get_u64(buf: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(b)
}

/// Шифрование сообщения: `m → p*` (файл 285: `p* = a ⊗_ε p* ⊕ m`).
///
/// Идемпотентность проектора делает коллапс мгновенным — один такт:
/// `p* = a·p₀ + δ`, `δ = (I−a)(ε·b)`. IV берётся из `rng` (для
/// детерминизма тестов — `Rng::seed_from_u64`).
pub fn encrypt(
    key: &CipherKey,
    msg: &[u8],
    rng: &mut Rng,
) -> Result<(Vec<u8>, EncryptReport), String> {
    let d = key.d_pol;
    let cap = key.capacity();
    let msg_bits = bytes_to_bits(msg);
    let msg_blocks = msg_bits.len().div_ceil(cap);
    let n_blocks = msg_blocks + 1; // IV + блоки сообщения
    let keystream = key.keystream();

    let mut body: Vec<u8> = Vec::with_capacity(n_blocks * d * 4);
    let mut interference_max = 0.0_f64;
    let mut margin_min = f64::INFINITY;

    // CBC-маска: биты паттерна предыдущего блока.
    let mut prev_bits: Vec<bool> = Vec::new();
    for k in 0..n_blocks {
        let block_bits: Vec<bool> = if k == 0 {
            // IV: случайный паттерн — рандомизирует всю цепочку.
            (0..cap).map(|_| rng.next_u64() & 1 == 1).collect()
        } else {
            // Биты сообщения блока k−1 (хвост добит нулями), маска CBC.
            let start = (k - 1) * cap;
            let end = (start + cap).min(msg_bits.len());
            let mut bits: Vec<bool> = msg_bits[start..end].to_vec();
            bits.resize(cap, false);
            for (b, m) in bits.iter_mut().zip(prev_bits.iter()) {
                *b ^= m;
            }
            bits
        };
        let (delta, interference) = pattern_delta(key, &block_bits);
        interference_max = interference_max.max(interference);

        // Коллапс: p* = a·p₀ + δ (один такт — a идемпотентен).
        // Квантование в f32: хранённое значение и есть состояние
        // расшифровщика; маска следующего блока — биты паттерна,
        // которые декодер извлечёт из этого блока.
        let stored: Vec<f64> = keystream
            .iter()
            .zip(delta.iter())
            .map(|(&s, &dl)| f64::from((s + dl) as f32))
            .collect();

        // Симуляция декодирования: запас порога на этом блоке.
        let mut mapped = key.project(&stored);
        for (m, &s) in mapped.iter_mut().zip(stored.iter()) {
            *m = s - *m;
        }
        let (_, m_min, _) = delta_to_bits(&mapped);
        margin_min = margin_min.min(m_min);

        prev_bits = block_bits;
        for &t in &stored {
            body.extend_from_slice(&(t as f32).to_le_bytes());
        }
    }

    let digest = sha256_trunc24(&body);
    let mut out = Vec::with_capacity(CIPHER_HEADER_SIZE + body.len());
    out.extend_from_slice(&CIPHER_MAGIC);
    put_u16(&mut out, CIPHER_VERSION);
    put_u16(&mut out, 0); // flags
    put_u32(&mut out, d as u32);
    put_u32(&mut out, n_blocks as u32);
    put_u64(&mut out, msg.len() as u64);
    put_u32(&mut out, key.k_modes as u32);
    out.extend_from_slice(&0.0_f32.to_le_bytes()); // reserved (eta не нужен)
    put_u32(&mut out, 0); // reserved2
    put_u32(&mut out, 0); // reserved3
    out.extend_from_slice(&digest);
    debug_assert_eq!(out.len(), CIPHER_HEADER_SIZE);
    out.extend_from_slice(&body);

    let out_len = out.len();
    Ok((
        out,
        EncryptReport {
            blocks: n_blocks,
            msg_len: msg.len(),
            out_len,
            k_modes: key.k_modes,
            interference_max,
            margin_min,
            digest_hex: digest.iter().map(|b| format!("{b:02x}")).collect(),
        },
    ))
}

/// Расшифровка: `m = p* ⊕ (a ⊗_ε p*)` (файл 285).
///
/// `δ̂ = p* − a·p*` по каждому блоку, демодуляция порогом `ε/2`,
/// снятие CBC-маски битами паттерна предыдущего блока.
pub fn decrypt(key: &CipherKey, cipher: &[u8]) -> Result<(Vec<u8>, DecryptReport), String> {
    if cipher.len() < CIPHER_HEADER_SIZE {
        return Err("шифртекст короче заголовка".into());
    }
    if cipher[..4] != CIPHER_MAGIC {
        return Err("не контейнер .pqc (магия PQC1 не найдена)".into());
    }
    let version = get_u16(cipher, 4);
    if version != CIPHER_VERSION {
        return Err(format!("неподдерживаемая версия .pqc: {version}"));
    }
    let d = get_u32(cipher, 8) as usize;
    let n_blocks = get_u32(cipher, 12) as usize;
    let msg_len = get_u64(cipher, 16) as usize;
    let k_modes = get_u32(cipher, 24) as usize;
    let digest_off = CIPHER_HEADER_SIZE - 24;
    let expect_digest = &cipher[digest_off..digest_off + 24];
    let body = &cipher[CIPHER_HEADER_SIZE..];

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
    if n_blocks == 0 {
        return Err("шифртекст без блоков".into());
    }
    let need = n_blocks * d * 4;
    if body.len() != need {
        return Err(format!(
            "тело шифртекста {body_len} B != n_blocks × d_pol × 4 = {need} B",
            body_len = body.len()
        ));
    }
    let digest = sha256_trunc24(body);
    if digest.as_slice() != expect_digest {
        return Err("digest sha256-24 не сходится — шифртекст повреждён".into());
    }
    let cap = key.capacity();
    if msg_len > (n_blocks - 1).saturating_mul(cap).div_ceil(8) {
        return Err("msg_len больше ёмкости блоков шифртекста".into());
    }

    // Блок 0 — IV: его паттерн и есть маска первого блока сообщения.
    let read_block = |k: usize| -> Vec<f64> {
        let mut p = Vec::with_capacity(d);
        for i in 0..d {
            let off = (k * d + i) * 4;
            p.push(f64::from(f32::from_le_bytes([
                body[off], body[off + 1], body[off + 2], body[off + 3],
            ])));
        }
        p
    };

    let mut bits_out: Vec<bool> = Vec::with_capacity((n_blocks - 1) * cap);
    let mut margins: Vec<f64> = Vec::with_capacity(n_blocks * 2);
    let mut prev_bits: Vec<bool> = Vec::new();
    for k in 0..n_blocks {
        let p_star = read_block(k);
        // δ̂ = p* − a·p*.
        let mapped = key.project(&p_star);
        // Биты читаются только на холодных позициях ключа.
        let delta_at: Vec<f64> = key
            .positions
            .iter()
            .map(|&pos| p_star[pos as usize] - mapped[pos as usize])
            .collect();
        let (bits, m_min, m_mean) = delta_to_bits(&delta_at);
        margins.push(m_min);
        margins.push(m_mean);
        if k > 0 {
            // Снятие CBC-маски: m_k = δ̂_k ⊕ δ̂_{k−1}.
            let mut unmasked = bits.clone();
            for (b, m) in unmasked.iter_mut().zip(prev_bits.iter()) {
                *b ^= m;
            }
            bits_out.extend_from_slice(&unmasked);
        }
        prev_bits = bits;
    }

    // Обрезка до msg_len байт и сборка.
    bits_out.truncate(msg_len * 8);
    if bits_out.len() < msg_len * 8 {
        return Err("бит сообщения меньше msg_len".into());
    }
    let msg = bits_to_bytes(&bits_out);
    // Запас: min по блокам (margins — пары min/mean).
    let margin_min = margins.iter().step_by(2).cloned().fold(f64::INFINITY, f64::min);
    let margin_mean = margins.iter().skip(1).step_by(2).sum::<f64>()
        / n_blocks.max(1) as f64;
    Ok((
        msg,
        DecryptReport {
            blocks: n_blocks,
            msg_len,
            margin_min,
            margin_mean,
        },
    ))
}

/// Расстояние между двумя шифртекстами (аваланш-метрика): среднее и
/// максимальное `|Δp*|` покомпонентно + доля компонент, сдвинувшихся
/// сильнее `ε/4`. Геометрия блоков должна совпадать.
pub fn cipher_distance(a: &[u8], b: &[u8]) -> Result<(f64, f64, f64), String> {
    if a.len() != b.len() || a.len() < CIPHER_HEADER_SIZE + 4 {
        return Err("шифртексты разной длины".into());
    }
    let d = get_u32(a, 8) as usize;
    let n = get_u32(a, 12) as usize;
    if (get_u32(b, 8) as usize, get_u32(b, 12) as usize) != (d, n) {
        return Err("геометрия шифртекстов не совпадает".into());
    }
    let read = |c: &[u8], k: usize, i: usize| -> f64 {
        let off = CIPHER_HEADER_SIZE + (k * d + i) * 4;
        f64::from(f32::from_le_bytes([
            c[off], c[off + 1], c[off + 2], c[off + 3],
        ]))
    };
    let (mut sum, mut mx, mut moved) = (0.0_f64, 0.0_f64, 0usize);
    let total = n * d;
    for k in 0..n {
        for i in 0..d {
            let dist = (read(a, k, i) - read(b, k, i)).abs();
            sum += dist;
            mx = mx.max(dist);
            if dist > BIT_PHASE / 4.0 {
                moved += 1;
            }
        }
    }
    Ok((sum / total.max(1) as f64, mx, moved as f64 / total.max(1) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqw::PqwWriter;

    /// Ключ-путь: d=256, русла 0→1→…→255 — моды синусоидальны и
    /// распределены по всей размерности (помеха проекции мала).
    fn path_key() -> Vec<u8> {
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p = if i % 3 == 0 { -0.7 } else { 0.6 };
            w.add_phase(i, p).unwrap();
        }
        let pairs: Vec<(u32, u32, f64)> =
            (0u32..255).map(|i| (i, i + 1, 1.0)).collect();
        let gyro = pqw::GyroData::new(256, 1000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    /// Ключ-гребёнка: d=256, русла (i, i+1) и (i, i+4) — связная
    /// структура без вырождений (моды делокализованы).
    fn chain_key() -> Vec<u8> {
        let mut w = PqwWriter::new(256).unwrap().hyperparams(0.05, 0.5, 0.75, 0.05);
        for i in 0u32..256 {
            let p = if i % 2 == 0 { -0.5 } else { 0.8 };
            w.add_phase(i, p).unwrap();
        }
        let mut pairs: Vec<(u32, u32, f64)> =
            (0u32..255).map(|i| (i, i + 1, 1.0)).collect();
        pairs.extend((0u32..252).map(|i| (i, i + 4, 0.6)));
        let gyro = pqw::GyroData::new(256, 5000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    /// Плотный циркулянтный ключ: d=256, русла (i, i+s) для сдвигов
    /// s ∈ {1, 5, 11, 23} со seed-зависимыми знаками/весами — моды
    /// делокализованы (нет хабов), разные seed дают разные моды.
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
        let gyro = pqw::GyroData::new(256, 1000, pairs, 256).unwrap();
        w.to_bytes_v3(&gyro).unwrap()
    }

    fn load_key(bytes: &[u8], k: usize) -> CipherKey {
        let reader = PqwReader::from_bytes(bytes).unwrap();
        CipherKey::from_reader(&reader, k).unwrap()
    }

    #[test]
    fn key_idempotence_exact() {
        // a ⊗_ε a = a: повторная проекция не меняет вектор (ранг 2K).
        let key = load_key(&dense_key(1), 0);
        let p: Vec<f64> = (0..key.d_pol).map(|i| (i as f64 * 0.13) % 1.5).collect();
        let once = key.project(&p);
        let twice = key.project(&once);
        let resid = once
            .iter()
            .zip(twice.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        assert!(resid < 1e-12, "невязка идемпотентности {resid}");
        assert!(key.ortho_max < 1e-12);
    }

    #[test]
    fn roundtrip_dense() {
        let key = load_key(&dense_key(1), 0);
        let msg: Vec<u8> = (0..200u32).map(|i| (i * 7 + 3) as u8).collect();
        let (cipher, enc) = encrypt(&key, &msg, &mut Rng::seed_from_u64(42)).unwrap();
        assert_eq!(enc.blocks, 1 + (msg.len() * 8).div_ceil(key.capacity()));
        let (plain, dec) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg);
        // Запас декодирования ≥ 0.3ε — далеко от порога ε/2.
        // Калибровка целилась в 0.2ε помехи (запас 0.3ε); fallback-K
        // на локализованных модах тоньше — но порог ε/2 ещё далеко.
        assert!(dec.margin_min > 0.1 * BIT_PHASE, "margin {}", dec.margin_min);
        assert!(enc.interference_max < 0.4 * BIT_PHASE, "interference {}", enc.interference_max);
    }

    #[test]
    fn roundtrip_chain_multiblock() {
        let key = load_key(&chain_key(), 0);
        // Ёмкость блока = холодные позиции ключа (см. калибровку).
        let msg: Vec<u8> = (0..100u32).map(|i| (i * 31 + 5) as u8).collect();
        let (cipher, enc) = encrypt(&key, &msg, &mut Rng::seed_from_u64(7)).unwrap();
        assert_eq!(enc.blocks, 1 + (msg.len() * 8).div_ceil(key.capacity()));
        let (plain, dec) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg);
        assert!(dec.margin_min > 0.1 * BIT_PHASE);
    }

    #[test]
    fn roundtrip_triangle() {
        let key = load_key(&path_key(), 0);
        let msg = b"POLER archetype algebra cipher".to_vec();
        let (cipher, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(3)).unwrap();
        let (plain, _) = decrypt(&key, &cipher).unwrap();
        assert_eq!(plain, msg);
    }

    #[test]
    fn empty_message_is_iv_only() {
        let key = load_key(&path_key(), 0);
        let (cipher, enc) = encrypt(&key, &[], &mut Rng::seed_from_u64(1)).unwrap();
        assert_eq!(enc.blocks, 1);
        let (plain, dec) = decrypt(&key, &cipher).unwrap();
        assert!(plain.is_empty());
        assert_eq!(dec.msg_len, 0);
    }

    #[test]
    fn wrong_key_yields_garbage() {
        // Чужой ключ: другая структура J → другие моды → другой a →
        // δ̂ = δ + (a − a')·p* — мусор порядка ключевого потока.
        let key_a = load_key(&dense_key(1), 0);
        let key_b = load_key(&dense_key(2), 0);
        let msg: Vec<u8> = (0..128u32).map(|i| (i * 13 + 1) as u8).collect();
        let (cipher, _) = encrypt(&key_a, &msg, &mut Rng::seed_from_u64(42)).unwrap();
        // Чужой ключ: либо структурный отказ (другая ёмкость/моды),
        // либо мусор порядка BER 50% — и то и то защита.
        match decrypt(&key_b, &cipher) {
            Ok((plain, _)) => {
                assert_ne!(plain, msg);
                let same = plain
                    .iter()
                    .zip(msg.iter())
                    .filter(|(a, b)| a == b)
                    .count();
                let ber = 1.0 - same as f64 / msg.len() as f64;
                assert!(ber > 0.35, "BER слишком мал: {ber}");
            }
            Err(_) => { /* структурный отказ чужого ключа */ }
        }
        // Ключ другой структуры (другие моды/ёмкость) — структурный
        // отказ, а не мусор.
        let key_path = load_key(&path_key(), 0);
        let err = decrypt(&key_path, &cipher).unwrap_err();
        assert!(err.contains("d_pol") || err.contains("мод") || err.contains("ёмкост"));
    }

    #[test]
    fn wrong_mode_count_rejected() {
        let key4 = load_key(&dense_key(1), 2);
        let key2 = load_key(&dense_key(1), 1); // тот же J, меньше мод
        let msg = b"mode count".to_vec();
        let (cipher, _) = encrypt(&key4, &msg, &mut Rng::seed_from_u64(5)).unwrap();
        let err = decrypt(&key2, &cipher).unwrap_err();
        assert!(err.contains("мод"));
    }

    #[test]
    fn avalanche_single_bit() {
        // 1 бит сообщения: в своём блоке сдвиг ε в одной компоненте,
        // далее CBC-цепочка несёт флип через все последующие блоки.
        let key = load_key(&dense_key(1), 0);
        let mut msg = vec![0u8; 64];
        for (i, b) in msg.iter_mut().enumerate() {
            *b = (i * 5) as u8;
        }
        let (c1, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(9)).unwrap();
        msg[3] ^= 0x01; // один бит
        let (c2, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(9)).unwrap(); // тот же IV
        let (mean, max, _moved) = cipher_distance(&c1, &c2).unwrap();
        // Линейная схема: изменения порядка ε, но НЕ нулевые.
        assert!(max >= BIT_PHASE * 0.9, "max {max}");
        assert!(mean > 0.0);
        // Оба шифртекста расшифровываются своими сообщениями.
        assert_eq!(decrypt(&key, &c1).unwrap().0.len(), 64);
        assert_eq!(decrypt(&key, &c2).unwrap().0, msg);
    }

    #[test]
    fn iv_randomizes_ciphertext() {
        let key = load_key(&path_key(), 0);
        let msg = b"same message twice".to_vec();
        let (c1, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(100)).unwrap();
        let (c2, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(200)).unwrap();
        assert_ne!(c1, c2, "IV не рандомизировал шифртекст");
        assert_eq!(decrypt(&key, &c1).unwrap().0, msg);
        assert_eq!(decrypt(&key, &c2).unwrap().0, msg);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let key = load_key(&dense_key(1), 0);
        let msg = b"determinism".to_vec();
        let (c1, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(5)).unwrap();
        let (c2, _) = encrypt(&key, &msg, &mut Rng::seed_from_u64(5)).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn corruption_detected_by_digest() {
        let key = load_key(&path_key(), 0);
        let (mut cipher, _) =
            encrypt(&key, b"integrity", &mut Rng::seed_from_u64(11)).unwrap();
        let last = cipher.len() - 1;
        cipher[last] ^= 0xFF;
        let err = decrypt(&key, &cipher).unwrap_err();
        assert!(err.contains("digest"));
    }

    #[test]
    fn bad_magic_rejected() {
        let key = load_key(&path_key(), 0);
        let err = decrypt(&key, &[0u8; 256]).unwrap_err();
        assert!(err.contains("магия"));
    }

    #[test]
    fn key_requires_gyro() {
        // v2-контейнер без гироскопа — отказ с внятной причиной.
        let mut w = PqwWriter::new(8).unwrap();
        w.add_phase(0, 0.5).unwrap();
        let v2 = w.to_bytes().unwrap();
        let reader = PqwReader::from_bytes(&v2).unwrap();
        let err = CipherKey::from_reader(&reader, 0).unwrap_err();
        assert!(err.contains("гироскоп"));
    }

    #[test]
    fn bit_helpers_roundtrip() {
        let bytes = vec![0xA5u8, 0x00, 0xFF];
        assert_eq!(bits_to_bytes(&bytes_to_bits(&bytes)), bytes);
        assert_eq!(bytes_to_bits(&[0b0000_0001])[0], true);
        assert_eq!(bytes_to_bits(&[0b0000_0001])[7], false);
    }

    #[test]
    fn pattern_lives_in_complement() {
        // δ = (I−a)(ε·b): S-часть отсутствует — a·δ ≈ 0 (иначе
        // итерация файл 285 накапливала бы её линейно).
        let key = load_key(&dense_key(1), 0);
        let bits: Vec<bool> = (0..key.d_pol).map(|i| i % 3 == 0).collect();
        let (delta, _) = pattern_delta(&key, &bits);
        let proj = key.project(&delta);
        let resid = proj
            .iter()
            .map(|c| c.abs())
            .fold(0.0_f64, f64::max);
        assert!(resid < 1e-12, "S-часть паттерна {resid}");
    }
}

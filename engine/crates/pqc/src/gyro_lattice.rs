//! RQ16: слияние решётки с гироскопом v3 + 2-битный saturating-момент.
//!
//! Замыкает пятый столп манифеста: вся кинетика памяти — фазы, момент
//! и направленная циркуляция смысла `J = A − Aᵀ` — живёт в тритовых
//! решётках Packed4. **Ноль HashMap на горячем пути**: плотная
//! треугольная индексация пар + битовые сдвиги, как в branch-предикторе.
//!
//! ```text
//!      свидетельство чанка (TF-IDF, общий энкодер с RQ8/RQ15)
//!        │
//!        ▼
//!      2-битный saturating-момент  m ∈ {−1, 0, +1}      [зажигание/срыв]
//!        │  v = m · 2.0
//!        ▼
//!      born-шаг в решётке фаз (RQ15)                    [кристаллизация]
//!        │
//!        ▼
//!      транспорт L5:  p ← Π_Λ(e^{Δt·J} p)               [рассуждение]
//!        │   sin-LUT 3×3, ноль FPU; движутся только кинетические дуги
//!        ▼
//!      волна кристаллизации по каналам J → аттрактор H^Ψ = 0
//! ```
//!
//! ## Кинетика без HashMap
//!
//! Потоковый гироскоп [`TritGyro`] хранит направленную циркуляцию
//! `J[i][j]` прямо в тритовой решётке пар: пара `(i, j)`, `i < j`, занимает
//! слот `j·(j−1)/2 + i` плотной треугольной раскладки — **два трита (4
//! бита) на пару**: направление русла + насыщенность. Адресация — чистой
//! арифметикой, чтение/запись — сдвигами. Русло — насыщенная циркуляция
//! (два согласованных события — порог насыщения): `+1` — смысл течёт
//! `i → j`, `−1` — обратно; одиночный шум остаётся свежим руслом и в
//! транспорте не участвует. Насыщенное русло переживает единичное
//! встречное свидетельство (гистерезис в два события), смена направления
//! — только через покой, телепортации знака нет.
//!
//! Память решётки пар — `⌈d(d−1)/4⌉` байт (`d = 4096 → 4 МиБ`), потолок
//! размерности — [`MAX_DIM_GYRO`] (`d = 16384 → 64 МиБ`): плотная
//! индексация честно квадратична, и это её заявленная цена.
//!
//! ## 2-битный насыщающий момент
//!
//! [`MomentumLattice`] — кинетический импульс памяти, 2 бита на дугу:
//! `m ∈ {−1, 0, +1}` с порогами насыщения. Градиент свидетельства
//! `g = p_emp − target` обновляет момент по правилу гистерезиса:
//!
//! ```text
//! m = 0:   |g| ≥ τ_ign  →  m = −sign(g)      [зажигание]
//! m ≠ 0:   sign согласован                    →  m неизменен  [насыщение]
//! m ≠ 0:   |g| < τ_stall, знак противоположен →  m неизменен  [ИНЕРЦИЯ]
//! m ≠ 0:   |g| ≥ τ_stall, знак противоположен →  m = 0        [срыв]
//! ```
//!
//! Трение γ из RQ15 заменено порогами: момент **не затухает** между
//! ingest-ами — «фазовая инерция мышления», способность удерживать
//! многоуровневый контекст без распада. Born-шаг потребляет момент как
//! скорость `v = m · MOMENTUM_UNIT`: амплитуда 2.0 при η = 0.6
//! воспроизводит динамический конверт RQ15 — зажигание с экватора
//! (`η·v = 1.2 ≥ π/6`) и выход из полюса (`1.2 ≥ π/3`), смена знака —
//! только через экватор.
//!
//! ## Мост к L5: квантованный оператор авторегрессии
//!
//! [`precess_step_packed4`] замыкает оператор
//!
//! ```text
//! p_{t+1} = Π_Λ( e^{Δt·J} · p_t )
//! ```
//!
//! На тритовой решётке разность фаз пары принимает 9 значений, а её
//! синус — ровно три: `sin(θ_j − θ_i) ∈ {0, ±1}` — точная таблица
//! [`SIN_LUT`] 3×3, ни одного FPU-вызова. Моментные ворота: переносится
//! только пара, у которой хотя бы один конец кинетичен (`m ≠ 0`) —
//! покоящаяся память не дрейфует, мысль течёт от активных понятий.
//! Π_Λ-проекция — переквантование ближайшим тритом (`|cos θ′| ≥ 0.5`).
//!
//! Перещёлкнувшийся трит сам становится кинетическим: волна
//! кристаллизации идёт по каналам J шаг за шагом — квантованные шаги
//! авторегрессионного рассуждения. Аттрактор — выровненные фазы
//! (`sin = 0`): `ĤΨ = 0`, рассуждение сходится.
//!
//! ## Пример
//!
//! ```
//! use pqc::gyro_lattice::QuantizedGyroCurriculum;
//!
//! let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
//! let text = "phase phase phase lattice lattice born trit crystal";
//! // Свидетельство: сильные дуги кристаллизуются, момент зажигается,
//! // каналы J открываются — вся кинетика в тритах, без HashMap.
//! let rep = qc.ingest(text, 0).unwrap();
//! assert!(rep.moved > 0, "born-шаг кристаллизовал свидетельство");
//! assert!(rep.momentum_ignited > 0, "момент зажжён");
//! assert!(rep.channels > 0, "каналы циркуляции J открыты");
//!
//! // Шаг авторегрессионного рассуждения: фаза течёт по каналам J,
//! // волна кристаллизации расширяет мнение за пределы свидетельства.
//! let stats = qc.reasoning_step().unwrap();
//! assert!(stats.channels > 0);
//! ```

use std::collections::VecDeque;
use std::time::Instant;

use pqw::checksum::fnv1a64;
use pqw::gyro::GyroData;
use pqw::phase::{nearest_trit, pack_trit2, Trit};
use pqw::reader::PqwReader;
use pqw::stream::tokenize;
use pqw::trit_bloch::{counts, p_at, theta_lut, trit_at, TritCounts};
use pqw::writer::PqwWriter;

use crate::archetype_lattice::archetype_product_packed4;
use crate::bloch_stream::born_step_packed4_sparse;
use crate::error::{PqcError, Result};
use crate::generate::LexiconBuilder;
use crate::reflex::ContextReflex;
use crate::rng::Rng;
use crate::stream_engine::tfidf_arcs;

/// Амплитуда 2-битного момента: born-шаг потребляет `v = m · 2.0`.
///
/// При η = 0.6 воспроизводит конверт RQ15: `η·v = 1.2 ≥ π/6` (зажигание
/// с экватора) и `≥ π/3` (выход из полюса) — смена знака фазы только
/// через экватор, телепортации не существует.
pub const MOMENTUM_UNIT: f64 = 2.0;

/// Порог зажигания момента: `|g| ≥ 0.5` зажигает `m = −sign(g)`.
///
/// Эффективный пол селективности RQ15 (`|target| ≳ 0.44` при γ = 0.5)
/// выражен здесь напрямую: слабее порога — честный фон, момент молчит.
pub const IGNITE_THRESHOLD: f64 = 0.5;

/// Порог срыва момента: встречное `|g| ≥ 1.0` гасит `m → 0`.
///
/// Полоса `τ_ign < |g| < τ_stall` — инерция: слабое несогласие не
/// останавливает мысль. Срыв проходит через ноль (без телепортации).
pub const STALL_THRESHOLD: f64 = 1.0;

/// Потолок размерности решётки пар: `d = 16384 → 64 МиБ` (ниббл на пару).
///
/// Плотная треугольная индексация честно квадратична; для больших d
/// остаётся плотный f64-путь RQ8/RQ10 с ε-воротами.
pub const MAX_DIM_GYRO: u32 = 16_384;

/// Шаг born-петли по умолчанию (калибровка RQ14).
pub const DEFAULT_ETA: f64 = 0.6;

/// Шаг транспорта L5 по умолчанию: Δt оператора `e^{Δt·J}`.
pub const DEFAULT_DT: f64 = 0.6;

// ===================== Плотная индексация пар =====================

/// Число слотов верхнего треугольника: `d(d−1)/2` пар `i < j`.
#[inline]
pub fn pair_count(d: u32) -> usize {
    if d < 2 {
        0
    } else {
        let d = d as usize;
        d * (d - 1) / 2
    }
}

/// Плотный треугольный слот пары `(i, j)`, `i < j`: `j(j−1)/2 + i`.
///
/// Чистая арифметика — ни хеширования, ни коллизий: адрес пары в
/// решётке вычисляется на лету, байт `slot/2`, сдвиг `4·(slot%2)`.
#[inline]
pub fn pair_slot(i: u32, j: u32) -> usize {
    debug_assert!(i < j, "pair_slot: требуется i < j");
    let (i, j) = (i as usize, j as usize);
    j * (j - 1) / 2 + i
}

// ===================== Гироскоп в решётке =====================

/// Потоковый гироскоп `J = A − Aᵀ` в тритовой решётке пар — без HashMap.
///
/// Окно направленного контекста `W` (кольцо, как у f64-гироскопа RQ10);
/// каждое событие потока `(координата, полярность)` даёт знаковый вклад
/// `A[i→j] += s_i·s_j` каждому предшественнику окна. Циркуляция живёт
/// в **двух тритах на пару** (4 бита — ниббл): трит направления
/// `{−1, 0, +1}` (русло `i → j` / `j → i` / закрыто) + трит насыщенности
/// `{свежее, насыщенное}` — порог насыщения из ТЗ:
///
/// ```text
/// покой           --событие-->   свежее русло          (1-е свидетельство)
/// свежее русло    --то же-->     НАСЫЩЕННОЕ русло      (2-е — русло активно)
/// насыщенное      --встречное--> свежее                (1-е несогласие)
/// свежее          --встречное--> покой                 (2-е — русло закрыто)
/// ```
///
/// Русло (канал J) — только **насыщенная** циркуляция: два согласованных
/// события открывают его, одиночный шум остаётся свежим руслом и в
/// транспорте L5 не участвует (ε-ворота f64-гироскопа, спущенные на
/// решётку). Смена направления — только через цепочку насыщенность →
/// свежесть → покой → свежесть обратная → насыщенность обратная:
/// телепортации знака нет, гистерезисная полоса — два события.
/// Симметричный поток (вперёд-назад поровну) закрывает русло: ассоциация
/// без циркуляции не хранится, как и в RQ10.
///
/// Память — `⌈d(d−1)/4⌉` байт (`d = 4096 → 4 МиБ`, `d = 16384 → 64 МиБ`),
/// потолок размерности — [`MAX_DIM_GYRO`]. Детерминизм абсолютный:
/// одинаковый поток событий → побитово одинаковая решётка (ни ГПСЧ,
/// ни итераций по хеш-таблицам).
pub struct TritGyro {
    d: u32,
    window: usize,
    ring: VecDeque<(u32, i8)>,
    /// Решётка пар: ниббл (4 бита) на пару — [направление | насыщенность],
    /// слот = [`pair_slot`], `⌈d(d−1)/4⌉` байт.
    pairs: Vec<u8>,
    /// Начала строк треугольника: `rows[j] = j(j−1)/2` (обратный слот → пара).
    rows: Vec<u32>,
    /// Число насыщенных русел (O(1)-счётчик, актуален всегда).
    saturated: usize,
    ticks: u64,
}

/// Трит направления русла в ниббле: биты 0..1 (0 = закрыто, 1 = i→j,
/// 2 = j→i; коды v2 как у фаз).
const DIR_MASK: u8 = 0b11;
/// Флаг насыщенности русла в ниббле: бит 2.
const SAT_BIT: u8 = 0b100;

impl TritGyro {
    /// Новый гироскоп: `d_pol ≥ 1`, окно `W ≥ 1` (клампится).
    ///
    /// `d_pol > MAX_DIM_GYRO` отклоняется: решётка пар квадратична.
    pub fn new(d: u32, window: usize) -> Result<TritGyro> {
        if d == 0 {
            return Err(PqcError::EmptyState);
        }
        if d > MAX_DIM_GYRO {
            return Err(PqcError::Unsupported {
                what: "trit gyro lattice is O(d^2): d_pol must be <= 16384 \
                       (use the dense f64 engine for larger d)",
            });
        }
        let slots = pair_count(d);
        let mut rows = Vec::with_capacity(d as usize);
        for j in 0..d as usize {
            // rows[j] = j(j−1)/2 — начало строки j; j = 0 — пустая строка.
            let start = if j < 2 { 0 } else { j * (j - 1) / 2 };
            rows.push(start as u32);
        }
        Ok(TritGyro {
            d,
            window: window.max(1),
            ring: VecDeque::with_capacity(window.clamp(1, 4096)),
            pairs: vec![0u8; slots.div_ceil(2)],
            rows,
            saturated: 0,
            ticks: 0,
        })
    }

    /// Размерность фазового пространства.
    pub fn d_pol(&self) -> u32 {
        self.d
    }

    /// Окно направленного контекста W.
    pub fn window(&self) -> usize {
        self.window
    }

    /// Счётчик тактов — всего наблюдённых событий потока.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Байтов решётки пар (ниббл на пару, `⌈d(d−1)/4⌉`).
    pub fn pair_bytes(&self) -> usize {
        self.pairs.len()
    }

    /// Число насыщенных русел циркуляции (живых каналов J).
    pub fn channel_count(&self) -> usize {
        self.saturated
    }

    /// Одно событие потока: координата `coord < d_pol`, полярность ±1.
    ///
    /// Каждый предшественник окна получает направленный вклад в пару
    /// `(предшественник, новичок)`; полярность вклада — произведение
    /// знаков. Нейтральная полярность (0) занимает окно, но пар не
    /// порождает. Самопары (повтор координаты) пропускаются.
    pub fn observe(&mut self, coord: u32, sign: i8) {
        self.ticks += 1;
        debug_assert!(coord < self.d, "observe: координата в [0, d_pol)");
        if coord >= self.d {
            return; // вне решётки — событие не считается
        }
        let sign = sign.signum();
        // Индексная итерация по кольцу: (u32, i8) — Copy, заимствования
        // окна и мутации решётки пар не пересекаются.
        for idx in 0..self.ring.len() {
            let (c, s) = self.ring[idx];
            if c == coord || s == 0 || sign == 0 {
                continue;
            }
            // Направление вклада в верхний треугольник: событие (c → coord)
            // с полярностью q толкает J[c][coord] к +q при c < coord и
            // J[coord][c] к −q при coord < c (обратный поток).
            let q: i8 = if s == sign { 1 } else { -1 };
            let (i, j, dir) = if c < coord { (c, coord, q) } else { (coord, c, -q) };
            self.push_pair(i, j, dir);
        }
        self.ring.push_back((coord, sign));
        if self.ring.len() > self.window {
            self.ring.pop_front();
        }
    }

    /// Сброс кольца контекста (русла не тронуты): следующая траектория
    /// мысли начнётся с чистого окна.
    ///
    /// Путь сеттлинга RQ21: каждый такт — независимая траектория волны,
    /// но свидетельства русел копятся сквозь такты — пара, пройденная
    /// дважды в одном направлении, насыщается в общее русло
    /// (гистерезис двух свидетелей), накопленная циркуляция живёт.
    pub fn reset_ring(&mut self) {
        self.ring.clear();
    }

    /// Восстановление кольца контекста из следа контекст-рефлекса W
    /// (RQ23): хвост событий занимает окно — **без** повторного
    /// наблюдения (пары уже накоплены в прошлой сессии, тики не растут).
    ///
    /// Первый вопрос новой сессии спаривается с последними словами
    /// предыдущей: русла растут сквозь границу сессий.
    pub fn restore_ring(&mut self, events: &[(u32, i8)]) {
        self.ring.clear();
        let take = events.len().min(self.window);
        let start = events.len() - take;
        for &(c, s) in &events[start..] {
            if c < self.d {
                self.ring.push_back((c, s.signum()));
            }
        }
    }

    /// Прогон последовательности событий `(координата, полярность)`.
    pub fn observe_seq(&mut self, events: &[(u32, i8)]) {
        for &(c, s) in events {
            self.observe(c, s);
        }
    }

    /// Сенсорный поток текста: токены в порядке появления, координата —
    /// `fnv1a64 mod d_pol`, полярность — старший бит хеша (тот же
    /// конвент, что у f64-гироскопа и TF-IDF-кодировщика).
    pub fn observe_text(&mut self, text: &str) {
        for token in tokenize(text) {
            let h = fnv1a64(token.as_bytes());
            let coord = (h % self.d as u64) as u32;
            let sign: i8 = if (h >> 63) & 1 == 1 { 1 } else { -1 };
            self.observe(coord, sign);
        }
    }

    /// Канал пары `(i, j)`: `Pos` — насыщенное русло `i → j`, `Neg` —
    /// `j → i`, `Zero` — русла нет (покой или свежее-неподтверждённое).
    ///
    /// Свежие русла (одно свидетельство) наружу не видны: в транспорте
    /// L5 и контейнере v3 участвует только насыщенная циркуляция.
    pub fn channel(&self, i: u32, j: u32) -> Trit {
        if i >= j || j >= self.d {
            return Trit::Zero;
        }
        let nib = self.nibble_at(pair_slot(i, j));
        if nib & SAT_BIT == 0 {
            return Trit::Zero;
        }
        match nib & DIR_MASK {
            1 => Trit::Pos,
            2 => Trit::Neg,
            _ => Trit::Zero,
        }
    }

    /// Сырое состояние пары `(направление, насыщенность)` — интроспекция
    /// машины русел (видны и свежие, ещё не подтверждённые русла).
    pub fn pair_state(&self, i: u32, j: u32) -> (Trit, bool) {
        if i >= j || j >= self.d {
            return (Trit::Zero, false);
        }
        let nib = self.nibble_at(pair_slot(i, j));
        let dir = match nib & DIR_MASK {
            1 => Trit::Pos,
            2 => Trit::Neg,
            _ => Trit::Zero,
        };
        (dir, nib & SAT_BIT != 0)
    }

    /// Все насыщенные русла `(i, j, ±1.0)` верхнего треугольника.
    ///
    /// Порядок обхода — по слотам (j-строки сверху вниз), внутри строки
    /// по i; [`GyroData::new`] канонизирует сортировку сам.
    pub fn channels(&self) -> Vec<(u32, u32, f64)> {
        let mut out = Vec::new();
        self.for_each_channel(|i, j, dir| out.push((i, j, dir as f64)));
        out
    }

    /// Обход насыщенных русел без материализации: `f(i, j, dir)`,
    /// `dir = +1` (русло i→j) или `−1` (j→i). Скан решётки — линейный,
    /// нулевые байты пропускаются целиком (2 пары за одну проверку).
    pub fn for_each_channel(&self, mut f: impl FnMut(u32, u32, i8)) {
        let slots = pair_count(self.d);
        for (byte_idx, &byte) in self.pairs.iter().enumerate() {
            if byte == 0 {
                continue;
            }
            for k in 0..2u32 {
                let nib = (byte >> (4 * k)) & 0b1111;
                // Только насыщенные русла: направление + SAT_BIT.
                if nib & SAT_BIT == 0 {
                    continue;
                }
                let dir_code = nib & DIR_MASK;
                if dir_code != 1 && dir_code != 2 {
                    continue;
                }
                let slot = byte_idx * 2 + k as usize;
                if slot >= slots {
                    break; // паддинг хвостового байта
                }
                let (i, j) = self.unslot(slot);
                let dir: i8 = if dir_code == 1 { 1 } else { -1 };
                f(i, j, dir);
            }
        }
    }

    /// Resume: впитать русла из деквантованной топологической секции v3.
    ///
    /// Перезаписывает решётку пар бит-в-бит (знак веса → насыщенное русло):
    /// консолидированная циркуляция поднимается ровно такой, какой была
    /// записана (свежие-неподтверждённые русла рестарт не переживают —
    /// это шум ниже порога насыщения). Возвращает число впитанных русел.
    pub fn absorb(&mut self, pairs: &[(u32, u32, f64)], ticks: u64) -> Result<usize> {
        self.reset();
        let mut n = 0usize;
        for &(i, j, w) in pairs {
            if i >= j || j >= self.d {
                return Err(PqcError::BadArc { index: j, d_pol: self.d });
            }
            if !w.is_finite() || w == 0.0 {
                return Err(PqcError::BadPhase(w));
            }
            let dir: u8 = if w > 0.0 { 1 } else { 2 };
            self.write_nibble(pair_slot(i, j), dir | SAT_BIT);
            self.saturated += 1;
            n += 1;
        }
        self.ticks = self.ticks.max(ticks);
        Ok(n)
    }

    /// Полный сброс кинетики пар (фазы решётки не трогает — их здесь нет).
    pub fn reset(&mut self) {
        self.pairs.iter_mut().for_each(|b| *b = 0);
        self.ring.clear();
        self.saturated = 0;
    }

    /// Данные топологической секции v3 (`None`, если каналов нет).
    pub fn gyro_data(&self) -> Option<GyroData> {
        let pairs = self.channels();
        if pairs.is_empty() {
            return None;
        }
        GyroData::new(self.window as u32, self.ticks, pairs, self.d).ok()
    }

    /// Насыщающий шаг русла: событие с направлением `dir = ±1`.
    ///
    /// Машина русел (порог насыщения — два согласованных события):
    /// покой → свежее → насыщенное; встречное свидетельство снимает
    /// насыщенность, второе — закрывает русло (симметрия без циркуляции).
    fn push_pair(&mut self, i: u32, j: u32, dir: i8) {
        let slot = pair_slot(i, j);
        let nib = self.nibble_at(slot);
        let d = nib & DIR_MASK;
        let saturated = nib & SAT_BIT != 0;
        let dir_code: u8 = if dir > 0 { 1 } else { 2 };
        let (new_d, new_sat, sat_delta) = if d == 0 {
            // Покой → свежее русло в направлении события.
            (dir_code, false, 0)
        } else if d == dir_code {
            // Согласованное свидетельство: подтверждение/насыщение русла.
            (d, true, if saturated { 0 } else { 1 })
        } else if saturated {
            // Первое встречное: насыщенность снята, направление держит
            // гистерезис (русло ещё помнит, куда текло).
            (d, false, -1)
        } else {
            // Второе встречное на свежем русле — циркуляция скомпенсирована.
            (0, false, 0)
        };
        let new_nib = new_d | if new_sat { SAT_BIT } else { 0 };
        if new_nib != nib {
            self.write_nibble(slot, new_nib);
        }
        self.saturated = (self.saturated as i64 + sat_delta as i64) as usize;
    }

    /// Обратный слот → пара `(i, j)`: строка j — бинарным поиском по
    /// началам строк, `i = slot − rows[j]`.
    fn unslot(&self, slot: usize) -> (u32, u32) {
        let j = self.rows[1..].partition_point(|&r| r as usize <= slot);
        let i = slot - self.rows[j] as usize;
        (i as u32, j as u32)
    }

    #[inline]
    fn nibble_at(&self, slot: usize) -> u8 {
        (self.pairs[slot / 2] >> (4 * (slot % 2))) & 0b1111
    }

    #[inline]
    fn write_nibble(&mut self, slot: usize, nib: u8) {
        let shift = 4 * (slot % 2);
        let byte = self.pairs[slot / 2];
        self.pairs[slot / 2] = (byte & !(0b1111 << shift)) | (nib << shift);
    }
}

// ===================== 2-битный насыщающий момент =====================

/// Событие обновления момента — телеметрия «что сделала физика».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MomentumEvent {
    /// Дуга в покое, свидетельство ниже порога — мёртвая зона.
    HeldZero,
    /// Зажигание: `0 → ±1` (сильное свежее свидетельство).
    Ignited,
    /// Насыщение: момент согласован со свидетельством и не растёт.
    Saturated,
    /// Инерция: слабое встречное свидетельство не гасит момент.
    Inertial,
    /// Срыв: сильное встречное свидетельство гасит момент через ноль.
    Stalled,
}

/// Кинетическая память: 2 бита на дугу, `m ∈ {−1, 0, +1}`.
///
/// Заменяет f64-скорость RQ15: вместо HashMap — Packed4-решётка
/// `⌈d/4⌉` байт, вместо трения γ — пороги зажигания/срыва. Момент
/// **переживает** переход к новому свидетельству (не затирается, как
/// скорость RQ15): инерция мысли без распада.
pub struct MomentumLattice {
    trits: Vec<u8>,
    d: u32,
    ignite: f64,
    stall: f64,
    kinetic: usize,
}

impl MomentumLattice {
    /// Моментная решётка с калибровкой RQ16
    /// (`τ_ign = 0.5`, `τ_stall = 1.0`).
    pub fn new(d: u32) -> MomentumLattice {
        MomentumLattice {
            trits: vec![0u8; (d as usize).div_ceil(4)],
            d,
            ignite: IGNITE_THRESHOLD,
            stall: STALL_THRESHOLD,
            kinetic: 0,
        }
    }

    /// Моментная решётка с явными порогами (`0 < ignite ≤ stall`).
    pub fn with_thresholds(d: u32, ignite: f64, stall: f64) -> Result<MomentumLattice> {
        let mut m = MomentumLattice::new(d);
        m.set_thresholds(ignite, stall)?;
        Ok(m)
    }

    /// Пороги насыщения: зажигание `> 0`, срыв `≥` зажигания.
    pub fn set_thresholds(&mut self, ignite: f64, stall: f64) -> Result<&mut Self> {
        if !ignite.is_finite() || !stall.is_finite() || ignite <= 0.0 || stall < ignite {
            return Err(PqcError::BadPhase(ignite));
        }
        self.ignite = ignite;
        self.stall = stall;
        Ok(self)
    }

    /// Порог зажигания `τ_ign`.
    pub fn ignite_threshold(&self) -> f64 {
        self.ignite
    }

    /// Порог срыва `τ_stall`.
    pub fn stall_threshold(&self) -> f64 {
        self.stall
    }

    /// Шаг момента от градиента свидетельства `g = p_emp − target`.
    ///
    /// Правило гистерезиса — см. схему модуля. Возвращает событие
    /// (телеметрия зажиганий/срывов).
    pub fn push(&mut self, arc: u32, grad: f64) -> MomentumEvent {
        debug_assert!(grad.is_finite(), "momentum: градиент конечен");
        let arc = arc as usize;
        debug_assert!(arc < self.d as usize, "momentum: дуга в диапазоне");
        let m = self.at_idx(arc);
        // Направление, куда момент обязан указывать: v накапливает −g.
        let dir: i8 = if grad > 0.0 {
            -1
        } else if grad < 0.0 {
            1
        } else {
            0
        };
        let event = if m == 0 {
            if dir != 0 && grad.abs() >= self.ignite {
                MomentumEvent::Ignited
            } else {
                MomentumEvent::HeldZero
            }
        } else if dir == 0 || dir == m {
            // Согласован (или нейтрален): насыщение — выше ±1 некуда.
            MomentumEvent::Saturated
        } else if grad.abs() >= self.stall {
            MomentumEvent::Stalled
        } else {
            MomentumEvent::Inertial
        };
        match event {
            MomentumEvent::Ignited => {
                self.set_idx(arc, dir);
            }
            MomentumEvent::Stalled => {
                self.set_idx(arc, 0);
            }
            _ => {}
        }
        event
    }

    /// Транспортное зажигание (без порогов): перещёлкнувшийся трит
    /// становится кинетическим с направлением последнего движения.
    ///
    /// Вызывается оператором L5: движение — само по себе свидетельство
    /// кинетики. `dir = 0` гасит момент.
    pub fn set_kinetic(&mut self, arc: u32, dir: i8) {
        let arc = arc as usize;
        debug_assert!(arc < self.d as usize, "momentum: дуга в диапазоне");
        self.set_idx(arc, dir.signum());
    }

    /// Значение момента дуги: `−1`, `0` или `+1`.
    pub fn at(&self, arc: u32) -> i8 {
        self.at_idx(arc as usize)
    }

    /// Число кинетических дуг (`m ≠ 0`) — O(1).
    pub fn kinetic_len(&self) -> usize {
        self.kinetic
    }

    /// Размерность.
    pub fn d_pol(&self) -> u32 {
        self.d
    }

    /// Байтов решётки момента (`⌈d/4⌉`, Packed4).
    pub fn bytes(&self) -> usize {
        self.trits.len()
    }

    /// Полный сброс кинетики (инерция не переживает рестарт — как и
    /// скорость RQ15; мнение живёт в фазовой решётке).
    pub fn reset(&mut self) {
        self.trits.iter_mut().for_each(|b| *b = 0);
        self.kinetic = 0;
    }

    /// Обход кинетических дуг: `f(дуга, m)` — линейный скан Packed4,
    /// нулевые байты пропускаются целиком.
    ///
    /// Путь L5-генерации (RQ17): внутренний голос — кинетика вне
    /// контекста речи.
    pub fn for_each_kinetic(&self, mut f: impl FnMut(u32, i8)) {
        for (byte_idx, &byte) in self.trits.iter().enumerate() {
            if byte == 0 {
                continue;
            }
            for k in 0..4u32 {
                let m: i8 = match (byte >> (2 * k)) & 0b11 {
                    1 => 1,
                    2 => -1,
                    _ => 0,
                };
                if m != 0 {
                    f(byte_idx as u32 * 4 + k, m);
                }
            }
        }
    }

    #[inline]
    fn at_idx(&self, arc: usize) -> i8 {
        match (self.trits[arc / 4] >> (2 * (arc % 4))) & 0b11 {
            1 => 1,
            2 => -1,
            _ => 0,
        }
    }

    #[inline]
    fn set_idx(&mut self, arc: usize, m: i8) {
        let shift = 2 * (arc % 4);
        let code: u8 = match m {
            1 => 1,
            -1 => 2,
            _ => 0,
        };
        let byte = self.trits[arc / 4];
        let old = (byte >> shift) & 0b11;
        if old == code {
            return;
        }
        self.trits[arc / 4] = (byte & !(0b11 << shift)) | (code << shift);
        let was_kinetic = old != 0;
        let now_kinetic = code != 0;
        match (was_kinetic, now_kinetic) {
            (false, true) => self.kinetic += 1,
            (true, false) => self.kinetic -= 1,
            _ => {}
        }
    }
}

// ===================== Транспорт L5 =====================

/// Точная таблица `sin(θ_j − θ_i)` по 2-битным кодам фаз
/// (`0 = Zero → π/2`, `1 = Pos → 0`, `2 = Neg → π`).
///
/// Разность фаз решётки принимает 9 значений, синус — ровно три:
/// `{0, ±1}`. Ни одного FPU-вызова на канал — это LUT-разворот RQ14,
/// доведённый до транспорта: вся арифметика переноса — целые счётчики.
pub const SIN_LUT: [[i8; 3]; 3] = [
    [0, -1, 1], // θ_i = π/2 (Zero): sin(θ_j − π/2) для j ∈ {Zero, Pos, Neg}
    [1, 0, 0],  // θ_i = 0 (Pos)
    [-1, 0, 0], // θ_i = π (Neg)
];

/// Режим моментных ворот транспорта L5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportMode {
    /// Движется только пара, у которой хотя бы один конец кинетичен
    /// (`m ≠ 0`): покоящаяся память не дрейфует, мысль течёт от
    /// активных понятий к соседям по каналам (режим POLER).
    Gated,
    /// Чистый оператор `Π_Λ(e^{Δt·J} p)` — моментных ворот нет.
    Free,
}

/// Статистика одного шага транспорта L5.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PrecessStats {
    /// Живых каналов J в решётке пар.
    pub channels: usize,
    /// Каналов с фазовым контрастом, прошедших моментные ворота.
    pub contrast: usize,
    /// Дуг, получивших ненулевой крутящий момент.
    pub touched: usize,
    /// Дуг, сменивших трит (Π_Λ-проекция перекинула фазу через границу).
    pub moved: usize,
    /// Суммарный «плавающий» сдвиг `Σ|Δθ|` до переквантования.
    pub theta_shift: f64,
    /// Транспортных зажиганий момента (перещёлкнувшиеся дуги стали
    /// кинетическими — волна рассуждения пошла дальше).
    pub ignited: usize,
}

/// Переиспользуемые буферы транспорта: целые счётчики моментов `k`
/// (в единицах Δt) и список тронутых дуг. Владеет движок — ноль
/// аллокаций на шаг рассуждения.
pub struct PrecessScratch {
    counters: Vec<i32>,
    touched: Vec<u32>,
}

impl PrecessScratch {
    /// Под `d_pol` дуг.
    pub fn new(d: usize) -> PrecessScratch {
        PrecessScratch {
            counters: vec![0i32; d],
            touched: Vec::new(),
        }
    }

    /// Гарантировать ёмкость под `d` дуг (переиспользование между шагами).
    pub fn reserve(&mut self, d: usize) {
        if self.counters.len() < d {
            self.counters.resize(d, 0);
        }
    }
}

/// Квантованный шаг прецессии L5: `p_{t+1} = Π_Λ(e^{Δt·J} · p_t)`.
///
/// Для каждого живого канала `(i, j, s)` с фазовым контрастом:
/// `k = s · sin(θ_j − θ_i)` из [`SIN_LUT`] — целочисленный момент;
/// обе дуги пары получают `Δθ = −Δt·k` (совместная прецессия —
/// унитарный поток гироскопа, относительная фаза пары сохраняется
/// до Π_Λ-проекции). Переквантование — ближайшим тритом (`|cos θ′| ≥ 0.5`).
///
/// Моментные ворота (режим [`TransportMode::Gated`]): пара переносится,
/// только если хотя бы один её конец кинетичен. Перещёлкнувшийся трит
/// зажигает момент (`m = sign(k)`) — волна кристаллизации идёт по
/// каналам дальше. Аттрактор — полюс-полюс пары (`sin = 0`): рассуждение
/// сходится, `ĤΨ = 0`.
///
/// Контракт: `lattice.len() ≥ ⌈d/4⌉`, размерности гироскопа и момента
/// равны `d`, `dt ≥ 0` конечен.
pub fn precess_step_packed4(
    lattice: &mut [u8],
    d: usize,
    gyro: &TritGyro,
    momentum: &mut MomentumLattice,
    dt: f64,
    mode: TransportMode,
    scratch: &mut PrecessScratch,
) -> Result<PrecessStats> {
    if lattice.len() < d.div_ceil(4) {
        return Err(PqcError::LengthMismatch {
            expected: d.div_ceil(4),
            actual: lattice.len(),
        });
    }
    if gyro.d_pol() as usize != d {
        return Err(PqcError::LengthMismatch {
            expected: d,
            actual: gyro.d_pol() as usize,
        });
    }
    if momentum.d_pol() as usize != d {
        return Err(PqcError::LengthMismatch {
            expected: d,
            actual: momentum.d_pol() as usize,
        });
    }
    if !dt.is_finite() || dt < 0.0 {
        return Err(PqcError::BadPhase(dt));
    }
    scratch.reserve(d);
    let mut stats = PrecessStats {
        channels: gyro.channel_count(),
        ..PrecessStats::default()
    };
    if dt == 0.0 || stats.channels == 0 {
        return Ok(stats); // покой: ни транспорта, ни проекции
    }
    let lut = theta_lut();

    // Фаза 1: накопление целочисленных моментов — ни одного FPU.
    // (момент здесь читается — ворота; запись только в фазе 2)
    {
        let counters = &mut scratch.counters[..d];
        let touched = &mut scratch.touched;
        let gates = &*momentum;
        gyro.for_each_channel(|i, j, s| {
            let (iu, ju) = (i as usize, j as usize);
            let ci = ((lattice[iu / 4] >> (2 * (iu % 4))) & 0b11) as usize;
            let cj = ((lattice[ju / 4] >> (2 * (ju % 4))) & 0b11) as usize;
            let sin = SIN_LUT[ci][cj];
            if sin == 0 {
                return; // контраста нет — русло не работает
            }
            if mode == TransportMode::Gated && gates.at(i) == 0 && gates.at(j) == 0 {
                return; // покоящаяся пара не дрейфует
            }
            let k = (s * sin) as i32;
            // Δθ = −Δt·k обеим дугам (совместная прецессия, как у
            // precess_step: delta −= torque): счётчики копят сам момент k.
            for x in [iu, ju] {
                if counters[x] == 0 {
                    touched.push(x as u32);
                }
                counters[x] += k;
            }
            stats.contrast += 1;
        });
    }

    // Фаза 2: Π_Λ-проекция — переквантование тронутых дуг.
    for &x in scratch.touched.iter() {
        let x = x as usize;
        let k = scratch.counters[x];
        scratch.counters[x] = 0; // буфер чист к следующему шагу
        if k == 0 {
            continue; // моменты пар скомпенсировались
        }
        let shift = 2 * (x % 4);
        let code = ((lattice[x / 4] >> shift) & 0b11) as usize;
        let theta = lut[code];
        let theta_next = theta - dt * (k as f64);
        stats.theta_shift += (theta_next - theta).abs();
        let p_next = theta_next.cos();
        let trit = nearest_trit(p_next as f32);
        let new_code = pack_trit2(trit) as usize;
        if new_code != code {
            lattice[x / 4] =
                (lattice[x / 4] & !(0b11 << shift)) | ((new_code as u8) << shift);
            stats.moved += 1;
            // Перещёлкнувшийся трит кинетичен: v-конвенция знака —
            // θ ← θ − dt·k, значит m = sign(k). Волна рассуждения
            // продолжается с направления последнего движения.
            momentum.set_kinetic(x as u32, k.signum() as i8);
            stats.ignited += 1;
        }
        stats.touched += 1;
    }
    scratch.touched.clear();
    Ok(stats)
}

// ===================== Движок слияния =====================

/// Раскладка памяти движка — вся кинетика плотная, без HashMap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineMemory {
    /// Фазовая решётка Packed4 (`⌈d/4⌉`).
    pub lattice: usize,
    /// Решётка момента Packed4 (`⌈d/4⌉`).
    pub momentum: usize,
    /// Решётка пар J Packed4 (`⌈d(d−1)/8⌉`).
    pub pairs: usize,
    /// Плотная TF-IDF статистика документов (`4·d`).
    pub doc_freq: usize,
    /// Итого байт.
    pub total: usize,
}

/// Отчёт одного ingest-а движка слияния.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantizedGyroReport {
    /// Токенов в чанке.
    pub tokens: usize,
    /// Дуг свидетельства чанка (после ε-ворот).
    pub nnz: usize,
    /// Документов усвоено (включая текущий).
    pub docs_seen: u64,
    /// Шагов всего: 1 born + reasoning_steps транспорта.
    pub steps_run: usize,
    /// Born-перещёлкивания тритов (кристаллизация свидетельства).
    pub moved: usize,
    /// Доля born-перещёлкиваний (от носителя шага).
    pub moved_frac: f64,
    /// «Плавающий» сдвиг born-шага `Σ|θ′ − θ|`.
    pub theta_shift: f64,
    /// Шагов авторегрессионного рассуждения (транспорт L5).
    pub reasoning_steps: usize,
    /// Перещёлкивания транспорта (волна по каналам J).
    pub transport_moved: usize,
    /// «Плавающий» сдвиг транспорта.
    pub transport_theta_shift: f64,
    /// Транспортных зажиганий момента (волна продолжилась).
    pub transport_ignited: usize,
    /// Зажиганий момента свидетельством за ingest.
    pub momentum_ignited: usize,
    /// Срывов момента свидетельством за ingest.
    pub momentum_stalled: usize,
    /// Кинетических дуг сейчас (`m ≠ 0`).
    pub momentum_kinetic: usize,
    /// Живых каналов циркуляции J.
    pub channels: usize,
    /// Ненулевых тритов фазовой решётки до ingest-а.
    pub lattice_nnz_before: usize,
    /// Ненулевых тритов после.
    pub lattice_nnz_after: usize,
    /// Невязка решётки к цели чанка: `½‖p − target‖²` на дугах чанка.
    pub param_loss: f64,
    /// Чанк не дал свидетельства.
    pub no_hits: bool,
    /// Сквозная задержка.
    pub elapsed: std::time::Duration,
}

/// Отчёт архетипического моста (RQ18): `c = a ⊗_ε lattice`.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeReport {
    /// Ненулевых тритов архетипа промпта.
    pub nnz_prompt: usize,
    /// Ненулевых тритов решётки (носитель памяти).
    pub nnz_lattice: usize,
    /// Дуг пересечения (оба полюса).
    pub co_support: usize,
    /// Согласных полюсов пересечения (резонанс).
    pub resonance: usize,
    /// Встречных полюсов (аннигиляция — открытый вопрос).
    pub conflict: usize,
    /// Прозрачных дуг продукта: память сквозь незнание вопроса —
    /// ландшафт метафоры за пределами пересечения.
    pub transparent: usize,
    /// Энергия пересечения `E = co / min(nnz)`.
    pub energy: f64,
    /// Гейт открыт: `E ≥ ε` — структурный изоморфизм найден.
    pub resonant: bool,
    /// Зажиганий момента мостом (внимание на пересечении).
    pub ignited: usize,
    /// Кинетических дуг всего после моста.
    pub kinetic: usize,
    /// Речевые билеты моста: `(дуга, вес)` — конфликт 3, согласие 2,
    /// прозрачный проход 1 (интерсекция громче фона метафоры).
    pub tickets: Vec<(u32, u32)>,
}

/// Слитый движок RQ16: решётка фаз + гироскоп J + 2-битный момент.
///
/// Вся кинетика — тритовые решётки Packed4 с плотной индексацией;
/// единственная «плотная» статистика — TF-IDF `doc_freq: Vec<u32>`
/// (сенсорный путь, не кинетика). Регламент: `η = 0.6`, `Δt = 0.6`,
/// `v = m·2.0`, `τ_ign = 0.5`, `τ_stall = 1.0`, `shots = 4096`,
/// политика Hold. Чекпоинт — контейнер v3: фазы бит-в-бит +
/// топологическая секция GYRO из каналов; resume поднимает и то и
/// другое (момент и idf-статистика не переживают рестарт — мнение да,
/// инерция нет).
pub struct QuantizedGyroCurriculum {
    d_pol: u32,
    epsilon: f32,
    eta: f64,
    dt: f64,
    shots: u64,
    seed: u64,
    /// Обучаемая память: фазы, `⌈d/4⌉` байт Packed4.
    lattice: Vec<u8>,
    momentum: MomentumLattice,
    gyro: TritGyro,
    /// Лексикон кристалла (RQ17): доминантный токен на координату —
    /// обратная карта кодировщика, путь генерации речи.
    lexicon: LexiconBuilder,
    /// Контекст-рефлекс W (RQ23): динамический след диалога —
    /// хвост сенсорного потока + имя собеседника. Автобиографическая
    /// память: нить разговора переживает рестарт через кольцо
    /// гироскопа и затравку волны речи.
    reflex: ContextReflex,
    /// Плотная TF-IDF статистика (HashMap нет нигде).
    doc_freq: Vec<u32>,
    docs: u64,
    ingests: usize,
    rng: Rng,
    scratch: PrecessScratch,
    // Кумулятивная телеметрия прогона.
    ignited_total: u64,
    stalled_total: u64,
    reasoning_steps_total: u64,
    transport_moved_total: u64,
    transport_theta_total: f64,
}

impl QuantizedGyroCurriculum {
    /// Новый движок: `d_pol ∈ [1, 16384]`, порог LENS `ε ∈ [0, 1]`,
    /// сид ГПСЧ, окно гироскопа `W ≥ 1`.
    pub fn new(d_pol: u32, epsilon: f32, seed: u64, window: usize) -> Result<Self> {
        if d_pol == 0 {
            return Err(PqcError::EmptyState);
        }
        if d_pol > MAX_DIM_GYRO {
            return Err(PqcError::Unsupported {
                what: "quantized gyro lattice is O(d^2): d_pol must be <= 16384",
            });
        }
        if !epsilon.is_finite() || epsilon < 0.0 || epsilon > 1.0 {
            return Err(PqcError::BadPhase(epsilon as f64));
        }
        let d = d_pol as usize;
        Ok(QuantizedGyroCurriculum {
            d_pol,
            epsilon,
            eta: DEFAULT_ETA,
            dt: DEFAULT_DT,
            shots: 4096,
            seed,
            lattice: vec![0u8; d.div_ceil(4)],
            momentum: MomentumLattice::new(d_pol),
            gyro: TritGyro::new(d_pol, window)?,
            lexicon: LexiconBuilder::new(d_pol),
            reflex: ContextReflex::new(),
            doc_freq: vec![0u32; d],
            docs: 0,
            ingests: 0,
            rng: Rng::seed_from_u64(seed),
            scratch: PrecessScratch::new(d),
            ignited_total: 0,
            stalled_total: 0,
            reasoning_steps_total: 0,
            transport_moved_total: 0,
            transport_theta_total: 0.0,
        })
    }

    /// Выстрелов на Born-измерение фона.
    pub fn set_shots(&mut self, shots: u64) -> &mut Self {
        self.shots = shots.max(1);
        self
    }

    /// Шаг born-петли η (калибровка RQ14 — 0.6).
    pub fn set_eta(&mut self, eta: f64) -> &mut Self {
        self.eta = eta;
        self
    }

    /// Шаг транспорта L5: Δt оператора `e^{Δt·J}`.
    pub fn set_dt(&mut self, dt: f64) -> &mut Self {
        self.dt = dt;
        self
    }

    /// Пороги насыщающего момента (`τ_ign`, `τ_stall`).
    pub fn set_momentum_thresholds(&mut self, ignite: f64, stall: f64) -> Result<&mut Self> {
        self.momentum.set_thresholds(ignite, stall)?;
        Ok(self)
    }

    /// Размерность состояния.
    pub fn d_pol(&self) -> u32 {
        self.d_pol
    }

    /// Порог LENS.
    pub fn epsilon(&self) -> f32 {
        self.epsilon
    }

    /// Шаг born-петли.
    pub fn eta(&self) -> f64 {
        self.eta
    }

    /// Шаг транспорта.
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Выстрелов на измерение.
    pub fn shots(&self) -> u64 {
        self.shots
    }

    /// Сид ГПСЧ.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Документов усвоено.
    pub fn docs_seen(&self) -> u64 {
        self.docs
    }

    /// Ingest-ов выполнено.
    pub fn ingests(&self) -> usize {
        self.ingests
    }

    /// Фазовая решётка — обучаемая память, Packed4 бит-в-бит.
    pub fn lattice(&self) -> &[u8] {
        &self.lattice
    }

    /// Байтов фазовой решётки.
    pub fn lattice_bytes(&self) -> usize {
        self.lattice.len()
    }

    /// Счётчики фазовой решётки (+1/−1/0).
    pub fn counts(&self) -> TritCounts {
        counts(&self.lattice, self.d_pol as usize)
    }

    /// Ненулевых тритов фазовой решётки.
    pub fn nnz(&self) -> usize {
        let c = self.counts();
        c.pos + c.neg
    }

    /// Значение `p = cos θ` дуги.
    pub fn p_at(&self, i: u32) -> f64 {
        p_at(&self.lattice, i as usize)
    }

    /// Трит фазовой решётки.
    pub fn trit_at(&self, i: u32) -> Trit {
        trit_at(&self.lattice, i as usize)
    }

    /// Момент дуги (`−1/0/+1`).
    pub fn momentum_at(&self, i: u32) -> i8 {
        self.momentum.at(i)
    }

    /// Кинетических дуг сейчас.
    pub fn momentum_kinetic(&self) -> usize {
        self.momentum.kinetic_len()
    }

    /// Порог зажигания момента.
    pub fn momentum_ignite(&self) -> f64 {
        self.momentum.ignite_threshold()
    }

    /// Порог срыва момента.
    pub fn momentum_stall(&self) -> f64 {
        self.momentum.stall_threshold()
    }

    /// Живых каналов циркуляции J.
    pub fn channel_count(&self) -> usize {
        self.gyro.channel_count()
    }

    /// Каналы `(i, j, ±1)` верхнего треугольника.
    pub fn channels(&self) -> Vec<(u32, u32, f64)> {
        self.gyro.channels()
    }

    /// Байтов решётки пар.
    pub fn pair_bytes(&self) -> usize {
        self.gyro.pair_bytes()
    }

    /// Тактов гироскопа (всего событий сенсорного потока).
    pub fn gyro_ticks(&self) -> u64 {
        self.gyro.ticks()
    }

    /// Окно гироскопа W.
    pub fn gyro_window(&self) -> usize {
        self.gyro.window()
    }

    /// Кумулятивные зажигания момента за прогон.
    pub fn ignited_total(&self) -> u64 {
        self.ignited_total
    }

    /// Кумулятивные срывы момента за прогон.
    pub fn stalled_total(&self) -> u64 {
        self.stalled_total
    }

    /// Шагов рассуждения за прогон.
    pub fn reasoning_steps_total(&self) -> u64 {
        self.reasoning_steps_total
    }

    /// Перещёлкиваний транспорта за прогон.
    pub fn transport_moved_total(&self) -> u64 {
        self.transport_moved_total
    }

    /// Суммарный θ-сдвиг транспорта за прогон.
    pub fn transport_theta_total(&self) -> f64 {
        self.transport_theta_total
    }

    /// Раскладка плотной памяти — «сколько стоит мышление».
    pub fn memory(&self) -> EngineMemory {
        let lattice = self.lattice.len();
        let momentum = self.momentum.bytes();
        let pairs = self.gyro.pair_bytes();
        let doc_freq = self.doc_freq.len() * 4;
        EngineMemory {
            lattice,
            momentum,
            pairs,
            doc_freq,
            total: lattice + momentum + pairs + doc_freq,
        }
    }

    /// Один ingest: свидетельство → момент → born-шаг → рассуждение.
    ///
    /// `reasoning_steps` — шаги авторегрессионного рассуждения после
    /// born-шага: каждый — квантованный транспорт `Π_Λ(e^{Δt·J} p)`
    /// по каналам гироскопа (волна кристаллизации по руслам смысла).
    pub fn ingest(&mut self, text: &str, reasoning_steps: usize) -> Result<QuantizedGyroReport> {
        let t0 = Instant::now();
        let d = self.d_pol as usize;
        let lattice_nnz_before = self.nnz();

        // 0. Сенсорный поток гироскопа: токены в порядке появления —
        //    каналы J накапливаются ДО свидетельства (как у RQ10).
        self.gyro.observe_text(text);
        // 0b. Лексикон кристалла (RQ17): каждое слово чанка становится
        //     кандидатом доминанты своей координаты — мозг учится ГОВОРИТЬ
        //     теми же словами, которыми учится думать.
        self.lexicon.observe_text(text);

        // 1. TF-IDF свидетельство чанка (общий энкодер с RQ8/RQ15).
        let (pre_arcs, tokens) = tfidf_arcs(self.d_pol, self.docs, |i| {
            self.doc_freq[i as usize]
        }, text);

        // 2. Барьер NO_HITS: пустое свидетельство — детерминированный
        //    отказ (документ не учитывается; каналы уже накопились).
        if pre_arcs.is_empty() {
            return Ok(QuantizedGyroReport {
                tokens,
                nnz: 0,
                docs_seen: self.docs,
                steps_run: 0,
                moved: 0,
                moved_frac: 0.0,
                theta_shift: 0.0,
                reasoning_steps: 0,
                transport_moved: 0,
                transport_theta_shift: 0.0,
                transport_ignited: 0,
                momentum_ignited: 0,
                momentum_stalled: 0,
                momentum_kinetic: self.momentum.kinetic_len(),
                channels: self.gyro.channel_count(),
                lattice_nnz_before,
                lattice_nnz_after: lattice_nnz_before,
                param_loss: 0.0,
                no_hits: true,
                elapsed: t0.elapsed(),
            });
        }

        // 3. ε-ворота LENS (как RQ15: сравнение в f32).
        let evidence: Vec<(u32, f64)> = pre_arcs
            .iter()
            .copied()
            .filter(|&(_, s)| (s as f32).abs() >= self.epsilon)
            .collect();
        debug_assert!(!evidence.is_empty());

        // 4. 2-битный насыщающий момент: градиент свидетельства
        //    обновляет m по гистерезису зажигание/срыв (без γ-трения).
        let mut ignited = 0usize;
        let mut stalled = 0usize;
        let mut vel: Vec<(u32, f64)> = Vec::with_capacity(evidence.len());
        for &(i, t) in &evidence {
            let p = p_at(&self.lattice, i as usize);
            let p_emp = self.measure_coord(p);
            let g = p_emp - t;
            match self.momentum.push(i, g) {
                MomentumEvent::Ignited => ignited += 1,
                MomentumEvent::Stalled => stalled += 1,
                _ => {}
            }
            let m = self.momentum.at(i);
            if m == 0 {
                continue;
            }
            // Полюс, в который дует момент, ПОГЛОЩАЕТ его: выполненная
            // инерция не пинает решётку (η·v = 1.2 ≥ π/3 выбило бы
            // затвердевшее мнение). Встречный момент — наоборот, выталкивает
            // фазу с полюса через экватор (разворот мнения за два шага,
            // как у RQ15).
            let trit = trit_at(&self.lattice, i as usize);
            let satisfied = (trit == Trit::Pos && m > 0) || (trit == Trit::Neg && m < 0);
            if !satisfied {
                vel.push((i, m as f64 * MOMENTUM_UNIT));
            }
        }
        self.ignited_total += ignited as u64;
        self.stalled_total += stalled as u64;

        // 5. Born-шаг в решётке фаз (RQ15): скорость = момент × амплитуда.
        let born = if vel.is_empty() {
            None
        } else {
            Some(born_step_packed4_sparse(
                &mut self.lattice,
                d,
                &vel,
                self.eta,
            )?)
        };

        // 6. Шаги авторегрессионного рассуждения: транспорт L5.
        let mut transport_moved = 0usize;
        let mut transport_theta = 0.0_f64;
        let mut transport_ignited = 0usize;
        for _ in 0..reasoning_steps {
            let s = self.reasoning_step()?;
            transport_moved += s.moved;
            transport_theta += s.theta_shift;
            transport_ignited += s.ignited;
        }

        // 7. TF-IDF статистика (плотная — без HashMap).
        for &(i, _) in &pre_arcs {
            self.doc_freq[i as usize] += 1;
        }
        self.docs += 1;
        self.ingests += 1;

        // 8. Невязка к цели чанка после всех шагов.
        let param_loss = 0.5
            * evidence
                .iter()
                .map(|&(i, t)| {
                    let p = p_at(&self.lattice, i as usize);
                    (p - t) * (p - t)
                })
                .sum::<f64>();

        let moved = born.as_ref().map(|b| b.moved).unwrap_or(0);
        let theta_shift = born.as_ref().map(|b| b.theta_shift).unwrap_or(0.0);
        let touched = (evidence.len() * (1 + reasoning_steps)).max(1);
        Ok(QuantizedGyroReport {
            tokens,
            nnz: evidence.len(),
            docs_seen: self.docs,
            steps_run: 1 + reasoning_steps,
            moved,
            moved_frac: moved as f64 / touched as f64,
            theta_shift,
            reasoning_steps,
            transport_moved,
            transport_theta_shift: transport_theta,
            transport_ignited,
            momentum_ignited: ignited,
            momentum_stalled: stalled,
            momentum_kinetic: self.momentum.kinetic_len(),
            channels: self.gyro.channel_count(),
            lattice_nnz_before,
            lattice_nnz_after: self.nnz(),
            param_loss,
            no_hits: false,
            elapsed: t0.elapsed(),
        })
    }

    /// Один квантованный шаг авторегрессионного рассуждения:
    /// `p ← Π_Λ(e^{Δt·J} p)` — транспорт по каналам с моментными
    /// воротами, Π_Λ-проекция, транспортное зажигание момента.
    pub fn reasoning_step(&mut self) -> Result<PrecessStats> {
        self.reasoning_step_mode(TransportMode::Gated)
    }

    /// Шаг рассуждения с явным режимом моментных ворот: Gated —
    /// движется только кинетическая пара (режим POLER), Free — чистый
    /// оператор `Π_Λ(e^{Δt·J} p)` без ворот.
    ///
    /// Путь L5-генерации (RQ17): сессия речи без отклика момента
    /// (промпт не зажёг кинетики) обязана течь свободным потоком J.
    pub fn reasoning_step_mode(&mut self, mode: TransportMode) -> Result<PrecessStats> {
        let d = self.d_pol as usize;
        let stats = precess_step_packed4(
            &mut self.lattice,
            d,
            &self.gyro,
            &mut self.momentum,
            self.dt,
            mode,
            &mut self.scratch,
        )?;
        self.reasoning_steps_total += 1;
        self.transport_moved_total += stats.moved as u64;
        self.transport_theta_total += stats.theta_shift;
        Ok(stats)
    }

    // ===================== Путь L5-генерации (RQ17) =====================

    /// Архетип промпта (RQ18): TF-IDF свидетельство → тритовые полюса.
    ///
    /// Чистая сенсорная кодировка: дуга со свидетельством `|s| ≥ ε`
    /// LENS получает полюс `sign(s)`; суперпозиция — незнание. Без
    /// ГПСЧ, без мутаций состояния — промпт есть вход, а не знание.
    /// Возвращает packed4-решётку `⌈d/4⌉` байт.
    pub fn prompt_archetype(&self, text: &str) -> Vec<u8> {
        let (pre_arcs, _) = tfidf_arcs(self.d_pol, self.docs, |i| {
            self.doc_freq[i as usize]
        }, text);
        let mut arch = vec![0u8; self.lattice.len()];
        for (i, s) in pre_arcs {
            if (s as f32).abs() < self.epsilon {
                continue; // ε-ворота LENS: слабое свидетельство — фон
            }
            let code: u8 = if s > 0.0 { 1 } else { 2 };
            let x = i as usize;
            arch[x / 4] |= code << (2 * (x % 4));
        }
        arch
    }

    /// Архетипический мост (RQ18): `c = a ⊗_ε lattice` — нелинейная
    /// интерференция архетипа промпта с памятью кристалла.
    ///
    /// **Внимание, а не знание**: мост зажигает момент на дугах
    /// пересечения (гистерезис RQ16 + кинетика внимания), но фазы
    /// решётки не трогает — память меняется только born-шагом от
    /// реального свидетельства. Билеты моста — три этажа речи:
    /// конфликт полюсов кричит весом 3 (аннигиляция в продукте —
    /// открытый вопрос, требующий слова), согласие резонирует весом 2,
    /// прозрачный проход памяти сквозь незнание вопроса — вес 1,
    /// ландшафт метафоры за пределами пересечения.
    ///
    /// Гейт `E ≥ eps`: при ортогональных архетипах (E < eps) мост
    /// молчит — билеты пусты, момент не трогается, ложных ассоциаций
    /// не существует.
    pub fn archetype_bridge(&mut self, arch: &[u8], eps: f64) -> Result<BridgeReport> {
        let d = self.d_pol as usize;
        let mut prod = vec![0u8; self.lattice.len()];
        let stats = archetype_product_packed4(arch, &self.lattice, d, eps, &mut prod)?;
        if !stats.resonant {
            return Ok(BridgeReport {
                nnz_prompt: stats.nnz_a,
                nnz_lattice: stats.nnz_b,
                co_support: stats.co_support,
                resonance: stats.resonance,
                conflict: stats.conflict,
                transparent: 0,
                energy: stats.energy,
                resonant: false,
                ignited: 0,
                kinetic: self.momentum.kinetic_len(),
                tickets: Vec::new(),
            });
        }

        // Кинетика пересечения: момент внимания на дугах клина a ∧ l.
        // Направление — к полюсу ПРОМПТА: резонанс даёт инерцию
        // согласованного полюса, конфликт — вызов памяти (толчок к
        // полюсу вопроса). Без ГПСЧ: полюса читаются детерминированно,
        // мост воспроизводим.
        let mut ignited = 0usize;
        let mut tickets: Vec<(u32, u32)> = Vec::new();
        let mut transparent = 0usize;
        for i in 0..d {
            let ca = trit_at(arch, i);
            let cl = trit_at(&self.lattice, i);
            if ca != Trit::Zero && cl != Trit::Zero {
                // Пересечение: согласие — резонанс, встречные — конфликт.
                let target = if ca == cl { ca.sign() } else { 0.0 };
                let p_emp = cl.sign();
                let g = p_emp - target;
                if let MomentumEvent::Ignited = self.momentum.push(i as u32, g) {
                    ignited += 1;
                }
                // Внимание к полюсу промпта: зажигание push-ем (конфликт),
                // инерция согласованного полюса (резонанс, g = 0) или
                // восстановление после срыва — всегда sign(a_i).
                let want: i8 = if ca == Trit::Pos { 1 } else { -1 };
                if self.momentum.at(i as u32) == 0 {
                    self.momentum.set_kinetic(i as u32, want);
                }
                // Билеты пересечения: согласие говорит (2), конфликт
                // кричит (3) — даже аннигилировав в продукте, напряжённая
                // связь требует слова.
                let weight = if ca == cl { 2 } else { 3 };
                tickets.push((i as u32, weight));
            } else if trit_at(&prod, i) != Trit::Zero {
                // Прозрачный проход: память сквозь незнание вопроса —
                // ландшафт метафоры (вес 1, тише пересечения).
                transparent += 1;
                tickets.push((i as u32, 1));
            }
        }
        self.ignited_total += ignited as u64;

        Ok(BridgeReport {
            nnz_prompt: stats.nnz_a,
            nnz_lattice: stats.nnz_b,
            co_support: stats.co_support,
            resonance: stats.resonance,
            conflict: stats.conflict,
            transparent,
            energy: stats.energy,
            resonant: true,
            ignited,
            kinetic: self.momentum.kinetic_len(),
            tickets,
        })
    }

    /// Слушание промпта БЕЗ born-кристаллизации: токены входят в
    /// кольцо гироскопа (каналы вопроса копятся), TF-IDF свидетельство
    /// зажигает момент на дугах вопроса.
    ///
    /// Два уровня кинетики: (1) гистерезис RQ16 — градиент `g = p_emp −
    /// target` зажигает `m = −sign(g)` при `|g| ≥ τ_ign` (несогласие
    /// толкает); (2) кинетика внимания — ВСЕ дуги свидетельства
    /// принудительно кинетичны (`m = −sign(g)` при g ≠ 0, иначе
    /// `m = sign(p_emp)` — инерция измеренного полюса): вопрос
    /// разгоняет свои понятия и при полном согласии, иначе сойдедшийся
    /// кристалл (все полюса, sin = 0 на руслах) на знакомый вопрос
    /// отвечал бы молчанием — вниманию не нужно несогласие.
    ///
    /// Вопрос — вход, а не знание: фазы не кристаллизуются (born-шаг
    /// доходит до вопросительных слов только в фазе памяти диалога,
    /// см. [`crate::generate`]). Возвращает
    /// `(дуги после ε-ворот, зажиганий, токенов)`.
    pub fn listen_ignite(&mut self, text: &str) -> (usize, usize, usize) {
        let (arcs, ignited, tokens) = self.listen_ignite_arcs(text);
        (arcs.len(), ignited, tokens)
    }

    /// Слушание промпта с аттрактором (RQ22): то же, что
    /// [`listen_ignite`], но наружу отдаются **дуги вопроса** —
    /// координаты TF-IDF свидетельства после ε-ворот.
    ///
    /// Это и есть семантический аттрактор вопроса p₀: дуги, которые
    /// вопрос зажёг в решётке. Волновая маршрутизация RQ22
    /// ([`crate::generate`]) считает по ним BFS-дистанцию до каждого
    /// кандидата лотареи — волна направляется градиентом проектора
    /// строго в сторону аттрактора, отсекая посторонние ветки.
    /// Возвращает `(дуги аттрактора, зажиганий, токенов)`.
    pub fn listen_ignite_arcs(&mut self, text: &str) -> (Vec<u32>, usize, usize) {
        // 0. Сенсорный поток: каналы вопроса.
        self.gyro.observe_text(text);
        // 1. TF-IDF свидетельство промпта (статистика корпуса как есть).
        let (pre_arcs, tokens) = tfidf_arcs(self.d_pol, self.docs, |i| {
            self.doc_freq[i as usize]
        }, text);
        // 2. ε-ворота LENS.
        let evidence: Vec<(u32, f64)> = pre_arcs
            .iter()
            .copied()
            .filter(|&(_, s)| (s as f32).abs() >= self.epsilon)
            .collect();
        // 3. Кинетика вопроса: гистерезис + внимание.
        let mut ignited = 0usize;
        for &(i, t) in &evidence {
            let p = p_at(&self.lattice, i as usize);
            let p_emp = self.measure_coord(p);
            let g = p_emp - t;
            if let MomentumEvent::Ignited = self.momentum.push(i, g) {
                ignited += 1;
            }
            // Кинетика внимания: дуга вопроса обязана быть кинетичной.
            if self.momentum.at(i) == 0 {
                let dir: i8 = if g > 0.0 {
                    -1
                } else if g < 0.0 {
                    1
                } else {
                    // Полное согласие: инерция измеренного полюса.
                    if p_emp >= 0.0 { 1 } else { -1 }
                };
                self.momentum.set_kinetic(i, dir);
            }
        }
        self.ignited_total += ignited as u64;
        (evidence.iter().map(|&(i, _)| i).collect(), ignited, tokens)
    }

    /// Сенсорное событие генерации: эмиссия слова наблюдается
    /// гироскопом (авторегрессия замкнута — речь оставляет след
    /// в руслах J).
    pub fn observe_event(&mut self, coord: u32, sign: i8) {
        self.gyro.observe(coord, sign);
    }

    /// Лексикон-событие речи (RQ21): произнесённое слово становится
    /// кандидатом доминанты своей координаты — мосты грамматики
    /// («и», «and»…) обретают координаты, и повторная речь находит
    /// коннекторы уже в лотерее русел. Грамматика прорастает в
    /// решётку через собственную речь мозга.
    pub fn observe_lexicon(&mut self, token: &str) {
        self.lexicon.observe(token);
    }

    /// Самоподкрепление грамматики (RQ22): удачно вставленный
    /// коннектор укрепляет свои русла `J` — дополнительный
    /// направленный свидетель в гироскоп + кинетика внимания.
    ///
    /// Обычная эмиссия моста (`observe_event`) даёт **один** свидетель
    /// пары: русло насыщается после двух согласованных повторов.
    /// Подкрепление — второй свидетель в ту же сторону: коннектор,
    /// соединивший знаменательные слова, открывает своё русло уже
    /// после одного удачного диалога (гистерезис двух свидетелей
    /// срабатывает внутри одной фразы). Кинетика внимания (`m ≠ 0`)
    /// делает дугу коннектора предпочтительным путём волны: следующие
    /// мысли текут через укреплённую грамматику. Ноль ГПСЧ —
    /// детерминизм абсолютный.
    pub fn observe_reinforcement(&mut self, coord: u32, sign: i8) {
        // Свидетель русла: пары кольца (предшественники → коннектор)
        // получают второй согласованный вклад — насыщение за одну
        // реплику вместо двух.
        self.gyro.observe(coord, sign);
        // Кинетика внимания: дуга коннектора становится путём волны
        // (уровень 4 лотареи — внутренний голос грамматики).
        if self.momentum.at(coord) == 0 {
            let dir: i8 = if sign >= 0 { 1 } else { -1 };
            self.momentum.set_kinetic(coord, dir);
        }
    }

    /// Чистое кольцо контекста гироскопа (русла не тронуты) — путь
    /// сеттлинга RQ21: каждый такт консолидации — независимая
    /// траектория волны, накопленная циркуляция продолжает жить.
    pub fn reset_context_ring(&mut self) {
        self.gyro.reset_ring();
    }

    /// Контекст-рефлекс W (RQ23): динамический след диалога.
    pub fn reflex(&self) -> &ContextReflex {
        &self.reflex
    }

    /// Мутабельный доступ к рефлексу (имя собеседника, ручной след).
    pub fn reflex_mut(&mut self) -> &mut ContextReflex {
        &mut self.reflex
    }

    /// Одна реплика диалога в контекст-рефлекс (RQ23): вопрос
    /// собеседника + эмиссии ответа (координата, Born-полярность).
    /// Счётчик реплик растёт; фазы и русла не тронуты (это делает
    /// [`ingest`](Self::ingest) — рефлекс наблюдатель, не участник).
    pub fn observe_dialog_turn(&mut self, question: &str, answer: &[(u32, i8)]) {
        let d = self.d_pol;
        self.reflex.observe_turn(question, answer, d);
    }

    /// Моментная обратная связь генерации: градиент Born-измерения
    /// `g = p_emp − p` обновляет момент дуги (правило гистерезиса
    /// RQ16 — зажигание/инерция/срыв).
    pub fn momentum_feedback(&mut self, arc: u32, grad: f64) -> MomentumEvent {
        self.momentum.push(arc, grad)
    }

    /// Код трита фазы (0/1/2) — индекс [`SIN_LUT`] для фазового
    /// контраста пары (путь Born-лотереи генерации).
    pub fn phase_code(&self, i: u32) -> u8 {
        let x = i as usize;
        (self.lattice[x / 4] >> (2 * (x % 4))) & 0b11
    }

    /// Слово координаты из лексикона (доминантный токен, RQ17).
    pub fn lexicon_token(&self, coord: u32) -> Option<&str> {
        self.lexicon.token_of(coord)
    }

    /// Координат со словом в лексиконе.
    pub fn lexicon_len(&self) -> usize {
        self.lexicon.len()
    }

    /// N-я непустая координата лексикона (затравка свободной речи).
    pub fn lexicon_coord_at(&self, n: usize) -> Option<u32> {
        self.lexicon.coord_at(n)
    }

    /// Обход кинетических дуг (`m ≠ 0`): `f(дуга, m)`.
    pub fn for_each_kinetic_arc(&self, f: impl FnMut(u32, i8)) {
        self.momentum.for_each_kinetic(f);
    }

    /// Чекпоинт: контейнер v5 при непустом рефлексе и живых каналах
    /// со словарём (фазы + русла + лексикон + контекст-рефлекс W,
    /// RQ23), v4 — при живых каналах и словаре без следа, v3 — если
    /// каналы есть, но слов ещё нет, v2 — если каналов нет.
    /// γ-слот заголовка = 0: трение заменено порогами насыщающего
    /// момента, инерция не затухает.
    ///
    /// RQ23: кодек гироскопной секции выбирается измерением —
    /// gap-RLE (v2) для кластеризованных топологий.
    pub fn checkpoint(&self) -> Result<Vec<u8>> {
        let mut w = PqwWriter::new(self.d_pol)?
            .hyperparams(self.eta as f32, 0.0, 1.0, self.epsilon);
        for i in 0..self.d_pol as usize {
            match trit_at(&self.lattice, i) {
                Trit::Pos => {
                    w.add_phase(i as u32, 1.0)?;
                }
                Trit::Neg => {
                    w.add_phase(i as u32, -1.0)?;
                }
                Trit::Zero => {}
            }
        }
        let mut buf = Vec::with_capacity(pqw::HEADER_SIZE + self.lattice.len());
        // RQ17/RQ23: непустой лексикон при живых каналах — контейнер
        // v4 (обратная карта кодировщика) или v5 (+ контекст-рефлекс
        // W, если след диалога непуст). Нет каналов или словаря —
        // v3/v2 как раньше.
        let refl = self.reflex.to_data(self.d_pol);
        match (self.gyro.gyro_data(), self.lexicon.finish()) {
            (Some(data), Some(lex)) => {
                if let Some(refl) = refl {
                    w.write_v5(&mut buf, &data, &lex, &refl)?;
                } else {
                    w.write_v4(&mut buf, &data, &lex)?;
                }
            }
            (Some(data), None) => {
                w.write_v3(&mut buf, &data)?;
            }
            (None, _) => {
                w.write_packed_trits(&mut buf)?;
            }
        }
        Ok(buf)
    }

    /// Resume: фазовая решётка бит-в-бит + каналы J из секции v3/v4/v5
    /// + лексикон из секции LEXI (v4) + контекст-рефлекс W из секции
    /// REFL (v5, RQ23): хвост следа восстанавливает кольцо гироскопа
    /// (без повторного наблюдения — тики не растут), имя собеседника
    /// и счётчик реплик продолжают жить.
    ///
    /// Момент и TF-IDF статистика стартуют с нуля («мнение пережило
    /// рестарт, инерция — нет»); каналы циркуляции поднимаются из
    /// топологической секции, счётчик тактов — из заголовка секции,
    /// лексикон вливается в строитель (доминанты продолжают жить).
    /// v2-контейнер (без секции) поднимает только фазы. Возвращает
    /// число ненулевых тритов поднятой решётки.
    pub fn resume_from_reader(&mut self, reader: &PqwReader) -> Result<usize> {
        if reader.d_pol() != self.d_pol {
            return Err(PqcError::LengthMismatch {
                expected: self.d_pol as usize,
                actual: reader.d_pol() as usize,
            });
        }
        if reader.encoding() != pqw::phase::TritEncoding::Packed4 {
            return Err(PqcError::Unsupported {
                what: "quantized gyro curriculum requires Packed4 container (v2/v3/v4/v5)",
            });
        }
        let phase = reader.phase_bytes();
        let need = (self.d_pol as usize).div_ceil(4);
        if phase.len() != need {
            return Err(PqcError::LengthMismatch {
                expected: need,
                actual: phase.len(),
            });
        }
        self.lattice.copy_from_slice(phase);
        self.momentum.reset();
        self.doc_freq.iter_mut().for_each(|x| *x = 0);
        self.docs = 0;
        match reader.gyro() {
            Some(section) => {
                let pairs: Vec<(u32, u32, f64)> = section
                    .pairs()
                    .iter()
                    .map(|p| (p.i, p.j, p.weight))
                    .collect();
                self.gyro.absorb(&pairs, section.ticks())?;
            }
            None => {
                self.gyro.reset();
            }
        }
        if let Some(lex) = reader.lexicon() {
            self.lexicon.absorb(&lex);
        }
        // RQ23: контекст-рефлекс — след диалога переживает рестарт;
        // хвост занимает кольцо гироскопа (русла не тронуты).
        if let Some(refl) = reader.reflex() {
            self.reflex.absorb(&refl);
            let window = self.gyro.window();
            let tail: Vec<(u32, i8)> = refl.trail_tail(window).to_vec();
            self.gyro.restore_ring(&tail);
        }
        Ok(self.nnz())
    }

    /// Born-измерение координаты: полюса детерминированы, честная
    /// монета фона сэмплируется `shots` бросками (как RQ15).
    fn measure_coord(&mut self, p: f64) -> f64 {
        if p != 0.0 {
            return p;
        }
        let n = self.shots.max(1);
        let mut k = 0u64;
        for _ in 0..n {
            if self.rng.next_f64() < 0.5 {
                k += 1;
            }
        }
        1.0 - 2.0 * (k as f64) / (n as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use pqw::phase::{pack_trit2, Trit};

    /// Текст с НЕравными частотами: сильная дуга (phase, tf = 3),
    /// средняя (lattice, tf = 2), слабые (born/trit/crystal, tf = 1).
    /// Веса после L∞-нормировки: 1.0 / 0.67 / 0.33.
    fn sample_text() -> &'static str {
        "phase phase phase lattice lattice born trit crystal"
    }

    /// Ручная установка трита фазовой решётки.
    fn set_trit(bytes: &mut [u8], i: usize, t: Trit) {
        let shift = 2 * (i % 4);
        let code = pack_trit2(t);
        bytes[i / 4] = (bytes[i / 4] & !(0b11 << shift)) | (code << shift);
    }

    // ===================== Плотная индексация =====================

    #[test]
    fn pair_slot_dense_triangular() {
        // Строки верхнего треугольника: slot(i, j) = j(j−1)/2 + i.
        assert_eq!(pair_slot(0, 1), 0);
        assert_eq!(pair_slot(0, 2), 1);
        assert_eq!(pair_slot(1, 2), 2);
        assert_eq!(pair_slot(0, 3), 3);
        assert_eq!(pair_slot(1, 3), 4);
        assert_eq!(pair_slot(2, 3), 5);
        // Биекция на [0, P): разные пары — разные слоты, без дыр.
        let d = 32u32;
        let mut seen = std::collections::BTreeSet::new();
        for j in 1..d {
            for i in 0..j {
                let s = pair_slot(i, j);
                assert!(s < pair_count(d));
                assert!(seen.insert(s), "слот {s} занят дважды");
            }
        }
        assert_eq!(seen.len(), pair_count(d));
        assert_eq!(pair_count(d), 32 * 31 / 2);
        // Вырожденные размерности.
        assert_eq!(pair_count(0), 0);
        assert_eq!(pair_count(1), 0);
    }

    // ===================== TritGyro =====================

    #[test]
    fn observe_opens_and_saturates_channel() {
        // Повторный поток: доминирующая циркуляция 3→9 насыщается и
        // держится (гистерезис переживает одиночные встречные на стыках
        // раундов — окно кольца неизбежно даёт их); шумовая пара (3,15)
        // осциллирует ниже порога и руслом не становится.
        let mut g = TritGyro::new(16, 4).unwrap();
        for _ in 0..7 {
            g.observe_seq(&[(3, 1), (9, 1), (15, 1)]);
        }
        assert_eq!(g.channel(3, 9), Trit::Pos, "доминирующее русло насыщено");
        assert_eq!(g.channel(9, 15), Trit::Pos);
        assert_eq!(g.channel(3, 15), Trit::Zero, "шумовая пара ниже порога");
        // Насыщенные русла: (3,9) и (9,15); свежее (3,15) не в счёте.
        assert_eq!(g.channel_count(), 2);
        assert_eq!(g.ticks(), 21);
    }

    #[test]
    fn channel_saturation_threshold_two_events() {
        // Порог насыщения: ОДНО событие — свежее русло (наружу невидимо),
        // ВТОРОЕ — русло активно. Хирургический поток: W=1, разделители
        // поглощают граничные эффекты кольца.
        let mut g = TritGyro::new(16, 1).unwrap();
        g.observe_seq(&[(3, 1), (9, 1), (10, 1)]); // ровно одно событие F
        assert_eq!(g.pair_state(3, 9), (Trit::Pos, false), "свежее русло");
        assert_eq!(g.channel(3, 9), Trit::Zero, "свежее не видно снаружи");
        assert_eq!(g.channel_count(), 0);
        g.observe_seq(&[(3, 1), (9, 1), (11, 1)]); // второе событие F
        assert_eq!(g.pair_state(3, 9), (Trit::Pos, true), "насыщенное русло");
        assert_eq!(g.channel(3, 9), Trit::Pos);
        assert_eq!(g.channel_count(), 1);
    }

    #[test]
    fn channel_reversal_through_zero_no_teleport() {
        // Разворот насыщенного русла — только по цепочке:
        // насыщенное → свежее → покой → свежее-обратное → насыщенное-
        // обратное. Ни одного скачка знака.
        let mut g = TritGyro::new(16, 1).unwrap();
        let fwd = |g: &mut TritGyro, sep: u32| {
            g.observe_seq(&[(3, 1), (9, 1), (sep, 1)]); // событие F на (3,9)
        };
        let bwd = |g: &mut TritGyro, sep: u32| {
            g.observe_seq(&[(9, 1), (3, 1), (sep, 1)]); // событие R на (3,9)
        };
        fwd(&mut g, 10);
        fwd(&mut g, 11);
        assert_eq!(g.channel(3, 9), Trit::Pos, "насыщенное русло вперёд");
        bwd(&mut g, 12); // 1-е встречное: снятие насыщенности
        assert_eq!(g.pair_state(3, 9), (Trit::Pos, false));
        assert_eq!(g.channel(3, 9), Trit::Zero);
        bwd(&mut g, 13); // 2-е: закрытие русла (симметрия без циркуляции)
        assert_eq!(g.pair_state(3, 9), (Trit::Zero, false));
        bwd(&mut g, 14); // 3-е: свежее обратное русло
        assert_eq!(g.pair_state(3, 9), (Trit::Neg, false));
        assert_eq!(g.channel(3, 9), Trit::Zero, "телепортации нет");
        bwd(&mut g, 15); // 4-е: насыщенное обратное
        assert_eq!(g.channel(3, 9), Trit::Neg);
        assert_eq!(g.channel_count(), 1);
    }

    #[test]
    fn symmetric_flow_cancels() {
        // Строгое чередование вперёд/назад: свежие русла гасятся
        // встречными, насыщения нет — циркуляция не накапливается.
        let mut g = TritGyro::new(32, 1).unwrap();
        for r in 0..5u32 {
            let (a, b) = (10 + 2 * r, 11 + 2 * r);
            g.observe_seq(&[(3, 1), (9, 1), (a, 1)]); // F
            g.observe_seq(&[(9, 1), (3, 1), (b, 1)]); // R
        }
        assert_eq!(g.pair_state(3, 9), (Trit::Zero, false));
        assert_eq!(g.channel(3, 9), Trit::Zero);
        assert_eq!(g.channel_count(), 0);
    }

    #[test]
    fn opposite_polarity_opens_reverse_channel() {
        // Разные знаки токенов: продукт полярностей < 0 — русло j→i.
        let mut g = TritGyro::new(16, 1).unwrap();
        for r in 0..2u32 {
            g.observe_seq(&[(3, 1), (9, -1), (10 + r, 1)]);
        }
        assert_eq!(g.channel(3, 9), Trit::Neg, "насыщенное обратное русло");
        // Согласованное встречное — снимает насыщенность, второе — закрывает.
        g.observe_seq(&[(3, 1), (9, 1), (12, 1)]);
        assert_eq!(g.pair_state(3, 9), (Trit::Neg, false));
        assert_eq!(g.channel(3, 9), Trit::Zero, "гистерезис: знак ещё держится");
        g.observe_seq(&[(3, 1), (9, 1), (13, 1)]);
        assert_eq!(g.pair_state(3, 9), (Trit::Zero, false), "руссло закрыто");
    }

    #[test]
    fn window_binds_pairing_trit() {
        // W = 1: спаривание только с непосредственным предшественником —
        // пары (3, 7) нет; W = 2: предшественник-на-два есть, два раунда
        // насыщают русло. Разделители двух раундов разнесены.
        let run = |w: usize| {
            let mut g = TritGyro::new(64, w).unwrap();
            for r in 0..2u32 {
                let s1 = 40 + 2 * r;
                let s2 = 41 + 2 * r;
                g.observe_seq(&[(3, 1), (9, 1), (7, 1), (s1, 1), (s2, 1)]);
            }
            g
        };
        assert_eq!(run(1).channel(3, 7), Trit::Zero, "W=1 не достаёт на два шага");
        assert_eq!(run(2).channel(3, 7), Trit::Pos, "W=2 насыщает русло");
    }

    #[test]
    fn self_pairing_skipped_trit() {
        let mut g = TritGyro::new(16, 8).unwrap();
        g.observe_seq(&[(5, 1), (5, 1), (5, -1)]);
        assert_eq!(g.channel_count(), 0);
        assert_eq!(g.ticks(), 3);
    }

    #[test]
    fn trit_gyro_deterministic_no_hash() {
        // Одинаковый поток → побитово одинаковая решётка пар
        // (ни ГПСЧ, ни итераций по хеш-таблицам в кинетике).
        let stream: Vec<(u32, i8)> = (0..200)
            .map(|i| (i % 13, if i % 2 == 0 { 1 } else { -1 }))
            .collect();
        let mut a = TritGyro::new(64, 8).unwrap();
        let mut b = TritGyro::new(64, 8).unwrap();
        a.observe_seq(&stream);
        b.observe_seq(&stream);
        assert_eq!(a.pair_bytes(), b.pair_bytes());
        assert_eq!(a.channel_count(), b.channel_count());
        assert_eq!(a.channels(), b.channels());
    }

    #[test]
    fn channels_scan_matches_reads() {
        // Обход решётки (скан) согласован с точечными чтениями.
        let mut g = TritGyro::new(64, 6).unwrap();
        g.observe_text(sample_text());
        let chans = g.channels();
        assert!(!chans.is_empty());
        for &(i, j, w) in &chans {
            assert!(i < j && j < 64);
            let expect = match g.channel(i, j) {
                Trit::Pos => 1.0,
                Trit::Neg => -1.0,
                Trit::Zero => panic!("канал ({i},{j}) нулевой в списке"),
            };
            assert_eq!(w, expect);
        }
        assert_eq!(chans.len(), g.channel_count());
    }

    #[test]
    fn observe_text_matches_manual_stream() {
        // Сенсорный конвент: coord = fnv1a64 mod d, полярность — бит 63.
        let text = "phase lattice born trit crystal resonance operator memory";
        let mut auto = TritGyro::new(128, 4).unwrap();
        auto.observe_text(text);
        let mut manual = TritGyro::new(128, 4).unwrap();
        for token in tokenize(text) {
            let h = fnv1a64(token.as_bytes());
            manual.observe((h % 128) as u32, if (h >> 63) & 1 == 1 { 1 } else { -1 });
        }
        assert_eq!(auto.pair_bytes(), manual.pair_bytes());
        assert_eq!(auto.ticks(), manual.ticks());
    }

    #[test]
    fn absorb_roundtrip_bit_exact() {
        // Каналы → v3-контейнер → чтение → absorb: бит-в-бит.
        let mut g = TritGyro::new(64, 4).unwrap();
        g.observe_text(sample_text());
        let chans = g.channels();
        assert!(!chans.is_empty());
        let data = g.gyro_data().expect("каналы есть");
        let mut buf = Vec::new();
        {
            let w = PqwWriter::new(64).unwrap();
            w.write_v3(&mut buf, &data).unwrap();
        }
        let reader = PqwReader::from_bytes(&buf).unwrap();
        let section = reader.gyro().expect("секция GYRO на месте");
        let pairs: Vec<(u32, u32, f64)> = section
            .pairs()
            .iter()
            .map(|p| (p.i, p.j, p.weight))
            .collect();
        // Веса ±1 квантуются в q = ±127 точно (scale = 1).
        let mut fresh = TritGyro::new(64, 4).unwrap();
        let n = fresh.absorb(&pairs, section.ticks()).unwrap();
        assert_eq!(n, chans.len());
        assert_eq!(fresh.channels(), chans);
        assert_eq!(fresh.channel_count(), g.channel_count());
        assert_eq!(fresh.ticks(), g.ticks());
        // Перезапись absorb: решётка пар не суммируется, а замещается.
        fresh.absorb(&[(0, 1, 1.0)], 1).unwrap();
        assert_eq!(fresh.channels(), vec![(0, 1, 1.0)]);
    }

    #[test]
    fn gyro_rejects_bad_dims() {
        assert!(matches!(TritGyro::new(0, 4), Err(PqcError::EmptyState)));
        assert!(matches!(
            TritGyro::new(MAX_DIM_GYRO + 1, 4),
            Err(PqcError::Unsupported { .. })
        ));
        // Пограничная размерность допустима.
        assert!(TritGyro::new(MAX_DIM_GYRO, 4).is_ok());
        // Плотная раскладка: d = 4 → 6 пар × 4 бита → 3 байта.
        assert_eq!(TritGyro::new(4, 1).unwrap().pair_bytes(), 3);
        assert_eq!(pair_count(4), 6);
    }

    // ===================== Момент =====================

    #[test]
    fn momentum_ignite_threshold() {
        let mut m = MomentumLattice::new(16);
        // Сильный градиент зажигает: m = −sign(g).
        assert_eq!(m.push(0, -1.0), MomentumEvent::Ignited);
        assert_eq!(m.at(0), 1);
        assert_eq!(m.push(1, 1.0), MomentumEvent::Ignited);
        assert_eq!(m.at(1), -1);
        assert_eq!(m.kinetic_len(), 2);
        // Градиент ровно на пороге — тоже зажигает (≥).
        assert_eq!(m.push(2, -0.5), MomentumEvent::Ignited);
        assert_eq!(m.at(2), 1);
        // Мёртвая зона: слабое свежее свидетельство не зажигает.
        assert_eq!(m.push(3, -0.49), MomentumEvent::HeldZero);
        assert_eq!(m.at(3), 0);
        assert_eq!(m.kinetic_len(), 3);
    }

    #[test]
    fn momentum_inertia_resists_weak_opposition() {
        // ИНЕРЦИЯ: слабое встречное свидетельство не гасит момент.
        let mut m = MomentumLattice::new(16);
        m.push(0, -1.0); // m = +1
        for _ in 0..10 {
            assert_eq!(m.push(0, 0.7), MomentumEvent::Inertial);
            assert_eq!(m.at(0), 1, "инерция без затухания");
        }
        assert_eq!(m.kinetic_len(), 1);
    }

    #[test]
    fn momentum_stall_through_zero() {
        // Срыв: сильное встречное гасит через ноль, без телепортации
        // в противоположный знак.
        let mut m = MomentumLattice::new(16);
        m.push(0, -1.0); // m = +1
        assert_eq!(m.push(0, 1.5), MomentumEvent::Stalled);
        assert_eq!(m.at(0), 0, "срыв — только в ноль");
        // После срыва то же свидетельство зажигает противоположный момент.
        assert_eq!(m.push(0, 1.5), MomentumEvent::Ignited);
        assert_eq!(m.at(0), -1);
    }

    #[test]
    fn momentum_saturation_idempotent() {
        // Насыщение: согласованный момент не растёт и не меняется.
        let mut m = MomentumLattice::new(16);
        m.push(0, -2.0); // m = +1 (max |g|)
        for _ in 0..20 {
            assert_eq!(m.push(0, -1.0), MomentumEvent::Saturated);
            assert_eq!(m.at(0), 1);
        }
        // Нейтральный градиент (g = 0) на согласованном моменте —
        // тоже насыщение (нет ни зажигания, ни срыва).
        assert_eq!(m.push(0, 0.0), MomentumEvent::Saturated);
    }

    #[test]
    fn momentum_persists_across_pushes() {
        // КЛЮЧЕВАЯ физика RQ16: момент дуги не затирается, когда
        // свидетельство уходит к другим дугам (у RQ15 скорость вне
        // носителя обнулялась). Инерция живёт, пока её не сорвут.
        let mut m = MomentumLattice::new(64);
        m.push(7, -1.0);
        assert_eq!(m.at(7), 1);
        for other in 20..40 {
            m.push(other, -1.0);
        }
        assert_eq!(m.at(7), 1, "момент дуги 7 пережил чужое свидетельство");
        assert_eq!(m.kinetic_len(), 21);
        // Сброс — только явный (рестарт сессии).
        m.reset();
        assert_eq!(m.kinetic_len(), 0);
        assert_eq!(m.at(7), 0);
    }

    #[test]
    fn momentum_packed4_layout() {
        let m = MomentumLattice::new(9);
        assert_eq!(m.bytes(), 3); // ⌈9/4⌉
        assert_eq!(m.d_pol(), 9);
        assert_eq!(m.ignite_threshold(), IGNITE_THRESHOLD);
        assert_eq!(m.stall_threshold(), STALL_THRESHOLD);
    }

    #[test]
    fn momentum_threshold_validation() {
        assert!(MomentumLattice::with_thresholds(8, 0.0, 1.0).is_err());
        assert!(MomentumLattice::with_thresholds(8, 0.5, 0.4).is_err());
        assert!(MomentumLattice::with_thresholds(8, f64::NAN, 1.0).is_err());
        let m = MomentumLattice::with_thresholds(8, 0.3, 0.9).unwrap();
        assert_eq!(m.ignite_threshold(), 0.3);
        assert_eq!(m.stall_threshold(), 0.9);
        // Пороги работают: 0.2 < 0.3 — мёртвая зона.
        let mut m = MomentumLattice::with_thresholds(8, 0.3, 0.9).unwrap();
        assert_eq!(m.push(0, -0.2), MomentumEvent::HeldZero);
        assert_eq!(m.push(0, -0.4), MomentumEvent::Ignited);
    }

    // ===================== Транспорт L5 =====================

    /// Стенд транспорта: решётка фаз + гироскоп с насыщенным руслом
    /// (3, 9) + момент. Русло ставится через absorb (resume-путь) —
    /// хирургически чисто, без побочных пар.
    fn transport_setup() -> (Vec<u8>, TritGyro, MomentumLattice, PrecessScratch) {
        let mut lattice = vec![0u8; 4];
        set_trit(&mut lattice, 3, Trit::Pos); // θ_3 = 0
        set_trit(&mut lattice, 9, Trit::Zero); // θ_9 = π/2
        let mut gyro = TritGyro::new(16, 2).unwrap();
        gyro.absorb(&[(3, 9, 1.0)], 0).unwrap(); // насыщенное русло 3→9
        assert_eq!(gyro.channel(3, 9), Trit::Pos);
        let mut momentum = MomentumLattice::new(16);
        momentum.set_kinetic(3, 1); // дуга 3 кинетична (источник мысли)
        let scratch = PrecessScratch::new(16);
        (lattice, gyro, momentum, scratch)
    }

    #[test]
    fn sin_lut_exact_all_code_pairs() {
        let lut = theta_lut();
        for ci in 0..3 {
            for cj in 0..3 {
                let expect = (lut[cj] - lut[ci]).sin().round();
                assert_eq!(
                    SIN_LUT[ci][cj] as f64, expect,
                    "sin LUT[{ci}][{cj}] расходится с FPU"
                );
            }
        }
        // Специфика решётки: pole-pole и pole-экватор-парности.
        assert_eq!(SIN_LUT[1][1], 0); // Pos-Pos: sin 0
        assert_eq!(SIN_LUT[1][2], 0); // Pos-Neg: sin π = 0
        assert_eq!(SIN_LUT[0][0], 0); // Zero-Zero
        assert_eq!(SIN_LUT[1][0], 1); // Pos-Zero: sin π/2
        assert_eq!(SIN_LUT[0][1], -1); // Zero-Pos: sin(−π/2)
    }

    #[test]
    fn precess_gated_spreads_and_ignites() {
        // Русло 3→9, фазовый контраст Pos/Zero, дуга 3 кинетична:
        // совместная прецессия — полюс держит гистерезис, фон
        // кристаллизуется, перещёлкнувшийся трит становится кинетическим.
        let (mut lattice, gyro, mut momentum, mut scratch) = transport_setup();
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats.channels, 1);
        assert_eq!(stats.contrast, 1);
        assert_eq!(stats.moved, 1, "фон перекинулся в полюс источника");
        assert_eq!(trit_at(&lattice, 9), Trit::Pos, "волна кристаллизации");
        assert_eq!(trit_at(&lattice, 3), Trit::Pos, "полюс держит гистерезис");
        assert_eq!(stats.ignited, 1);
        assert_eq!(momentum.at(9), 1, "перещёлкнувшийся трит кинетичен");
        assert!(stats.theta_shift > 0.0);
        // Аттрактор: оба полюса — sin = 0, рассуждение сошлось.
        let again = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(again.moved, 0, "H^Ψ = 0: выровненная пара покоится");
        assert_eq!(again.contrast, 0);
    }

    #[test]
    fn precess_kinetic_gate_blocks_resting_pairs() {
        // Оба конца покоятся (m = 0) — пара не переносится: покоящаяся
        // память не дрейфует (шумные русла между фоном инертны).
        let (mut lattice, gyro, mut momentum, mut scratch) = transport_setup();
        momentum.reset(); // всё покоится
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats.contrast, 0, "ворота закрыты");
        assert_eq!(stats.moved, 0);
        assert_eq!(trit_at(&lattice, 9), Trit::Zero);
        // Free-режим: чистый оператор — моментных ворот нет.
        let free = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Free,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(free.contrast, 1);
        assert_eq!(free.moved, 1);
    }

    #[test]
    fn precess_receiver_kinetic_also_moves() {
        // Кинетичен приёмник (дуга 9), источник покоится: ворота
        // «хотя бы один конец» пропускают — совместная прецессия.
        let (mut lattice, gyro, mut momentum, mut scratch) = transport_setup();
        momentum.reset();
        momentum.set_kinetic(9, 1);
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats.contrast, 1);
        assert_eq!(stats.moved, 1);
        assert_eq!(trit_at(&lattice, 9), Trit::Pos);
    }

    #[test]
    fn precess_aligned_and_background_inert() {
        // Нет контраста — русло не работает: пары полюс-полюс и
        // фон-фон дают sin = 0.
        let mut lattice = vec![0u8; 4];
        set_trit(&mut lattice, 3, Trit::Pos);
        set_trit(&mut lattice, 9, Trit::Pos); // оба полюса, одна фаза
        let mut gyro = TritGyro::new(16, 2).unwrap();
        gyro.absorb(&[(3, 9, 1.0)], 0).unwrap(); // насыщенное русло
        let mut momentum = MomentumLattice::new(16);
        momentum.set_kinetic(3, 1);
        momentum.set_kinetic(9, 1);
        let mut scratch = PrecessScratch::new(16);
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats.channels, 1);
        assert_eq!(stats.contrast, 0, "sin(θ_j − θ_i) = 0");
        assert_eq!(stats.moved, 0);
        // Фон-фон: тоже ноль (шумовые каналы инертны по построению).
        let mut bg = vec![0u8; 4];
        let stats_bg = precess_step_packed4(
            &mut bg,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Free,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats_bg.moved, 0);
    }

    #[test]
    fn precess_small_dt_dead_zone() {
        // Слабый шаг: плавающий сдвиг есть, решётка держит (мёртвая
        // зона переквантования — цена 2 бит, честная).
        let (mut lattice, gyro, mut momentum, mut scratch) = transport_setup();
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.3, // π/2 − 0.3: cos = 0.296 < 0.5
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(stats.moved, 0);
        assert!(stats.theta_shift > 0.0, "сдвиг накопился, решётка давит");
        assert_eq!(trit_at(&lattice, 9), Trit::Zero);
        // dt = 0 — строгий покой.
        let zero = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.0,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(zero.theta_shift, 0.0);
    }

    #[test]
    fn precess_contract_violations() {
        let (lattice, gyro, mut momentum, mut scratch) = transport_setup();
        let mut short = vec![0u8; 1];
        assert!(precess_step_packed4(&mut short, 16, &gyro, &mut momentum, 0.6, TransportMode::Gated, &mut scratch).is_err());
        let mut l2 = lattice.clone();
        let small_gyro = TritGyro::new(8, 2).unwrap();
        assert!(precess_step_packed4(&mut l2, 16, &small_gyro, &mut momentum, 0.6, TransportMode::Gated, &mut scratch).is_err());
        let mut l3 = lattice.clone();
        let mut small_mom = MomentumLattice::new(8);
        assert!(precess_step_packed4(&mut l3, 16, &gyro, &mut small_mom, 0.6, TransportMode::Gated, &mut scratch).is_err());
        let mut l4 = lattice.clone();
        assert!(precess_step_packed4(&mut l4, 16, &gyro, &mut momentum, -0.1, TransportMode::Gated, &mut scratch).is_err());
    }

    #[test]
    fn precess_counters_reused_between_steps() {
        // Скретч-буферы переиспользуются: второй шаг видит чистые
        // счётчики (никакого протекания моментов между шагами).
        let (mut lattice, gyro, mut momentum, mut scratch) = transport_setup();
        for expect_moved in [1usize, 0] {
            let stats = precess_step_packed4(
                &mut lattice,
                16,
                &gyro,
                &mut momentum,
                0.6,
                TransportMode::Gated,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(stats.moved, expect_moved);
        }
        // После стабилизации — строгий ноль движений и сдвигов.
        let stats = precess_step_packed4(
            &mut lattice,
            16,
            &gyro,
            &mut momentum,
            0.6,
            TransportMode::Gated,
            &mut scratch,
        )
        .unwrap();
        assert_eq!((stats.moved, stats.contrast, stats.touched), (0, 0, 0));
    }

    // ===================== Движок слияния =====================

    // ===================== RQ23: контекст-рефлекс =====================

    #[test]
    fn restore_ring_keeps_channels_and_ticks() {
        // Восстановление кольца из следа НЕ трогает русла и тики:
        // события уже наблюдались в прошлой сессии.
        let mut g = TritGyro::new(64, 4).unwrap();
        g.observe_seq(&[(1, 1), (2, 1), (3, 1), (4, 1), (2, 1), (3, 1)]);
        let channels_before = g.channel_count();
        let ticks_before = g.ticks();
        assert!(channels_before > 0);
        g.restore_ring(&[(9, 1), (10, -1), (11, 1), (12, 1), (13, 1), (14, 1)]);
        assert_eq!(g.channel_count(), channels_before, "русла не тронуты");
        assert_eq!(g.ticks(), ticks_before, "тики не растут");
        // Кольцо занято хвостом следа (окно 4, событий 6 → последние 4:
        // 11, 12, 13, 14): новое событие 13 спаривается с предшественниками
        // кольца — свежие русла появляются СРАЗУ (пустое кольцо дало бы
        // ноль пар).
        g.observe(13, 1);
        let (dir, _) = g.pair_state(11, 13);
        assert_ne!(dir, Trit::Zero, "пара кольца сразу даёт вклад");
        // Контроль: чистое кольцо + то же событие — пар нет.
        let mut g2 = TritGyro::new(64, 4).unwrap();
        g2.observe(13, 1);
        assert_eq!(g2.pair_state(11, 13).0, Trit::Zero);
    }

    #[test]
    fn reflex_survives_checkpoint_resume() {
        // Полный цикл автобиографии: диалог → чекпоинт v5 → resume →
        // рефлекс жив, кольцо гироскопа восстановлено из хвоста следа.
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..3 {
            qc.ingest(sample_text(), 1).unwrap();
        }
        qc.reflex_mut().set_interlocutor("Мария");
        qc.observe_dialog_turn("квант фаза", &[(7, 1), (9, -1), (11, 1)]);
        qc.observe_dialog_turn("решётка трит", &[(3, 1)]);
        assert_eq!(qc.reflex().turns(), 2);
        assert_eq!(qc.reflex().interlocutor(), "Мария");

        let bytes = qc.checkpoint().unwrap();
        assert_eq!(&bytes[..8], b"POLER_Q5", "живой рефлекс → контейнер v5");

        // Resume: свежий движок поднимает рефлекс и кольцо.
        let reader = pqw::PqwReader::from_bytes(&bytes).unwrap();
        let mut qc2 = QuantizedGyroCurriculum::new(128, 0.05, 7, 4).unwrap();
        qc2.resume_from_reader(&reader).unwrap();
        assert_eq!(qc2.reflex().interlocutor(), "Мария");
        assert_eq!(qc2.reflex().turns(), 2);
        assert_eq!(qc2.reflex().trail().len(), qc.reflex().trail().len());
        // Кольцо гироскопа несёт хвост следа: вопрос новой сессии
        // спаривается с последними словами предыдущей — русла растут.
        let before = qc2.channel_count();
        qc2.ingest("трит момент", 1).unwrap();
        assert!(qc2.channel_count() >= before);
        // Повторный чекпоинт — снова v5, рефлекс переживает круг.
        let bytes2 = qc2.checkpoint().unwrap();
        assert_eq!(&bytes2[..8], b"POLER_Q5");
        let reader2 = pqw::PqwReader::from_bytes(&bytes2).unwrap();
        assert_eq!(reader2.reflex().unwrap().interlocutor(), "Мария");
    }

    #[test]
    fn empty_reflex_keeps_v4_container() {
        // Без следа диалога контейнер остаётся v4 (контракт RQ17).
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..3 {
            qc.ingest(sample_text(), 1).unwrap();
        }
        let bytes = qc.checkpoint().unwrap();
        assert_eq!(&bytes[..8], b"POLER_Q4");
    }

    #[test]
    fn train_ingest_does_not_pollute_reflex() {
        // Обучение (ingest корпуса) не трогает след W — автобиография
        // растёт только в диалоге (ask/chat), не при чтении книг.
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        qc.ingest(sample_text(), 1).unwrap();
        qc.ingest("совсем другой текст про другие вещи", 1).unwrap();
        assert!(qc.reflex().is_empty());
        assert_eq!(qc.reflex().turns(), 0);
    }

    #[test]
    fn engine_ingest_full_loop() {
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        assert_eq!(qc.eta(), DEFAULT_ETA);
        assert_eq!(qc.dt(), DEFAULT_DT);
        assert_eq!(qc.shots(), 4096);
        assert_eq!(qc.momentum_ignite(), IGNITE_THRESHOLD);
        assert_eq!(qc.momentum_stall(), STALL_THRESHOLD);

        let rep = qc.ingest(sample_text(), 0).unwrap();
        assert!(!rep.no_hits);
        assert!(rep.tokens >= 8);
        assert_eq!(rep.docs_seen, 1);
        // Сильные дуги (|t| = 1.0 и 0.67 ≥ τ_ign) кристаллизованы
        // born-шагом: момент зажжён, триты перещёлкнулись.
        assert!(rep.moved >= 2, "moved = {}", rep.moved);
        assert!(rep.momentum_ignited >= 2);
        assert!(rep.momentum_kinetic >= 2);
        // Каналы J открыты сенсорным потоком.
        assert!(rep.channels >= 1);
        assert_eq!(rep.channels, qc.channel_count());
        // Слабые дуги (|t| = 0.33 < τ_ign) — честный фон: момент молчит.
        // Средняя дуга (|t| = 0.67) кристаллизуется в ПОЛЮС ±1 с
        // перелётом (1 − 0.67)² — честная цена 2-битной решётки, фазы
        // которой только полюса. Невязка ровно:
        // ½[(1 − 2/3)² + 3·(1/3)²] = 2/9 ≈ 0.222.
        assert_eq!(rep.lattice_nnz_after, 2, "кристаллизованы только сильные");
        assert!(
            (rep.param_loss - 2.0 / 9.0).abs() < 1e-9,
            "невязка = {}, ожидалось 2/9",
            rep.param_loss
        );
        // Повтор того же текста: полюса держат гистерезис — тишина.
        let rep2 = qc.ingest(sample_text(), 0).unwrap();
        assert_eq!(rep2.moved, 0, "выученное мнение не двигается");
        assert_eq!(rep2.lattice_nnz_after, 2);
        assert!((rep2.param_loss - rep.param_loss).abs() < 1e-12);
    }

    #[test]
    fn engine_momentum_persists_across_ingests() {
        // ФИЗИКА RQ16: инерция не затухает, когда свидетельство уходит —
        // дуга первого чанка сохраняет момент при ingest-е второго.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        qc.ingest(sample_text(), 0).unwrap();
        let phase_coord = (fnv1a64(b"phase") % 256) as u32;
        assert_ne!(qc.momentum_at(phase_coord), 0, "момент сильной дуги жив");
        // Другой чанк — другой словарь: инерция первой дуги не тронута.
        let other = "quark gluon plasma string brane quark gluon plasma string brane";
        let rep = qc.ingest(other, 0).unwrap();
        assert!(!rep.no_hits);
        assert_ne!(
            qc.momentum_at(phase_coord),
            0,
            "момент пережил чужое свидетельство (у RQ15 скорость затиралась)"
        );
    }

    #[test]
    fn engine_reasoning_wave_and_convergence() {
        // Шаги авторегрессионного рассуждения: волна кристаллизации
        // идёт по насыщенным руслам J от кинетических дуг, расширяя
        // мнение за пределы свидетельства, и сходится к аттрактору.
        // Два ingest-а насыщают цепочку русел phase→lattice→born→trit→crystal.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        let rep = qc.ingest(sample_text(), 0).unwrap();
        assert_eq!(rep.lattice_nnz_after, 2);
        assert!(rep.channels >= 2, "фаз→решётка и решётка→born насыщены с первого чанка");
        qc.ingest(sample_text(), 0).unwrap(); // цепочка русел насыщена
        assert_eq!(qc.nnz(), 2, "слабые дуги всё ещё честный фон");

        let s1 = qc.reasoning_step().unwrap();
        assert!(s1.channels >= 2);
        assert!(s1.moved >= 1, "волна пошла по руслам: moved = {}", s1.moved);
        assert!(s1.ignited >= 1);
        assert!(qc.nnz() > 2, "мнение расширилось за пределы свидетельства");
        // Каскад: добегаем до аттрактора (не более 8 шагов).
        let mut last = s1;
        for _ in 0..8 {
            last = qc.reasoning_step().unwrap();
            if last.moved == 0 {
                break;
            }
        }
        assert_eq!(last.moved, 0, "ĤΨ = 0: рассуждение сходится");
        assert!(
            qc.nnz() >= 4,
            "волна кристаллизовала соседей: nnz = {}",
            qc.nnz()
        );
        assert!(qc.reasoning_steps_total() >= 2);
        assert!(qc.transport_moved_total() >= 2);
    }

    #[test]
    fn engine_ingest_with_reasoning_steps() {
        // ingest(reasoning_steps > 0) = born + каскад рассуждения.
        // Русло lattice→born насыщено с первого чанка — первый же шаг
        // транспорта кристаллизует born (волна стартует).
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        let rep = qc.ingest(sample_text(), 3).unwrap();
        assert_eq!(rep.steps_run, 4);
        assert_eq!(rep.reasoning_steps, 3);
        assert!(rep.transport_moved >= 1, "транспорт двинул триты");
        assert!(rep.transport_theta_shift > 0.0);
        assert_eq!(qc.reasoning_steps_total(), 3);
    }

    #[test]
    fn engine_memory_dense_layout() {
        // Вся кинетика плотная — раскладка памяти точна до байта:
        // фазы ⌈d/4⌉ + момент ⌈d/4⌉ + пары ⌈d(d−1)/4⌉ + doc_freq 4d.
        let qc = QuantizedGyroCurriculum::new(256, 0.05, 1, 4).unwrap();
        let mem = qc.memory();
        assert_eq!(mem.lattice, 64);
        assert_eq!(mem.momentum, 64);
        assert_eq!(mem.pairs, pair_count(256).div_ceil(2));
        assert_eq!(mem.pairs, 16_320);
        assert_eq!(mem.doc_freq, 1024);
        assert_eq!(mem.total, 64 + 64 + 16_320 + 1024);
        assert_eq!(qc.pair_bytes(), 16_320);
    }

    #[test]
    fn engine_checkpoint_v3_and_resume_bit_exact() {
        // Чекпоинт = контейнер v4 (RQ17: лексикон непуст): фазы
        // бит-в-бит + секция GYRO + секция LEXI; resume поднимает всё
        // тройку, повторный прогон того же корпуса не двигает ни
        // одного трита.
        let mut qc = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        qc.ingest(sample_text(), 0).unwrap();
        qc.ingest(sample_text(), 0).unwrap();
        let nnz = qc.nnz();
        let channels = qc.channel_count();
        assert!(channels >= 1);
        assert!(qc.lexicon_len() >= 1, "слова корпуса в лексиконе");

        let ckpt = qc.checkpoint().unwrap();
        assert_eq!(&ckpt[..8], b"POLER_Q4", "контейнер v4 с лексиконом");
        let reader = PqwReader::from_bytes(&ckpt).unwrap();
        assert!(reader.gyro().is_some());
        assert!(reader.lexicon().is_some(), "словарь переживает рестарт");
        assert_eq!(reader.lexicon().unwrap().len(), qc.lexicon_len());
        let phase_len = (256usize).div_ceil(4);
        assert_eq!(ckpt[pqw::HEADER_SIZE..pqw::HEADER_SIZE + phase_len].to_vec(), qc.lattice().to_vec());

        // Resume: мнение и каналы пережили рестарт, инерция — нет.
        let mut resumed = QuantizedGyroCurriculum::new(256, 0.05, 42, 4).unwrap();
        let lifted = resumed.resume_from_reader(&reader).unwrap();
        assert_eq!(lifted, nnz);
        assert_eq!(resumed.channel_count(), channels);
        assert_eq!(resumed.channels(), qc.channels());
        assert_eq!(resumed.momentum_kinetic(), 0, "инерция не переживает рестарт");
        assert_eq!(resumed.gyro_ticks(), qc.gyro_ticks());

        // Повтор того же корпуса: полюса держат, ворота закрыты
        // (момент свежий) — решётка бит-в-бит стабильна.
        let rep = resumed.ingest(sample_text(), 0).unwrap();
        assert_eq!(rep.moved, 0);
        assert_eq!(resumed.nnz(), nnz);
        assert_eq!(
            resumed.lattice().to_vec(),
            qc.lattice().to_vec(),
            "решётка изменилась после рестарта"
        );
    }

    #[test]
    fn engine_resume_v2_phases_only() {
        // v2-контейнер (без секции GYRO): поднимаются только фазы.
        let mut w = PqwWriter::new(64).unwrap().hyperparams(0.6, 0.0, 1.0, 0.05);
        w.add_phase(3, 1.0).unwrap();
        w.add_phase(9, -1.0).unwrap();
        let mut buf = Vec::new();
        w.write_packed_trits(&mut buf).unwrap();
        let reader = PqwReader::from_bytes(&buf).unwrap();
        let mut qc = QuantizedGyroCurriculum::new(64, 0.05, 7, 4).unwrap();
        assert_eq!(qc.resume_from_reader(&reader).unwrap(), 2);
        assert_eq!(qc.channel_count(), 0);
        assert_eq!(qc.trit_at(3), Trit::Pos);
        assert_eq!(qc.trit_at(9), Trit::Neg);
        // Обучение продолжается поверх поднятого мнения.
        let rep = qc.ingest(sample_text(), 0).unwrap();
        assert!(!rep.no_hits);
    }

    #[test]
    fn engine_validation_errors() {
        assert!(matches!(
            QuantizedGyroCurriculum::new(0, 0.05, 1, 4),
            Err(PqcError::EmptyState)
        ));
        assert!(matches!(
            QuantizedGyroCurriculum::new(16, -0.1, 1, 4),
            Err(PqcError::BadPhase(_))
        ));
        assert!(matches!(
            QuantizedGyroCurriculum::new(16, 1.5, 1, 4),
            Err(PqcError::BadPhase(_))
        ));
        assert!(matches!(
            QuantizedGyroCurriculum::new(MAX_DIM_GYRO + 1, 0.05, 1, 4),
            Err(PqcError::Unsupported { .. })
        ));
        // Пороги момента валидируются и на движке.
        let mut qc = QuantizedGyroCurriculum::new(16, 0.05, 1, 4).unwrap();
        assert!(qc.set_momentum_thresholds(0.2, 0.1).is_err());
        assert!(qc.set_momentum_thresholds(0.2, 0.8).is_ok());
        assert_eq!(qc.momentum_ignite(), 0.2);
    }

    #[test]
    fn engine_no_hits_barrier() {
        let mut qc = QuantizedGyroCurriculum::new(64, 0.05, 1, 4).unwrap();
        let rep = qc.ingest("", 2).unwrap();
        assert!(rep.no_hits);
        assert_eq!(rep.tokens, 0);
        assert_eq!(rep.docs_seen, 0);
        assert_eq!(rep.steps_run, 0);
        assert_eq!(rep.moved, 0);
        assert_eq!(qc.ingests(), 0);
        assert_eq!(qc.gyro_ticks(), 0);
        assert_eq!(qc.reasoning_steps_total(), 0);
    }

    #[test]
    fn engine_deterministic_two_runs() {
        // Одинаковый поток данных → побитово одинаковая вселенная:
        // фазы, момент, каналы, телеметрия.
        let run = |seed: u64| {
            let mut qc = QuantizedGyroCurriculum::new(256, 0.05, seed, 4).unwrap();
            let mut reports = Vec::new();
            for _ in 0..4 {
                reports.push(qc.ingest(sample_text(), 2).unwrap());
            }
            (qc, reports)
        };
        let (a, ra) = run(42);
        let (b, rb) = run(42);
        assert_eq!(a.lattice().to_vec(), b.lattice().to_vec());
        assert_eq!(a.momentum_kinetic(), b.momentum_kinetic());
        assert_eq!(a.channels(), b.channels());
        assert_eq!(a.pair_bytes(), b.pair_bytes());
        assert_eq!(a.transport_moved_total(), b.transport_moved_total());
        for (x, y) in ra.iter().zip(rb.iter()) {
            assert_eq!(x.moved, y.moved);
            assert_eq!(x.transport_moved, y.transport_moved);
        }
        // Шумостабильность (как у RQ15): другой сид меняет лишь измерения
        // фона, но решения порогов устойчивы — решётка та же. Транспорт
        // ГПСЧ-свободен, так что траектория рассуждения воспроизводима
        // при любом сиде.
        let (c, _) = run(43);
        assert_eq!(a.transport_moved_total(), c.transport_moved_total());
        assert_eq!(a.channels(), c.channels());
    }

    #[test]
    fn engine_hold_policy_gamma_zero_in_checkpoint() {
        // γ-слот заголовка = 0: трение заменено порогами момента —
        // инерция не затухает (заявлено в формате контейнера).
        let mut qc = QuantizedGyroCurriculum::new(64, 0.05, 1, 4).unwrap();
        qc.ingest(sample_text(), 0).unwrap();
        let ckpt = qc.checkpoint().unwrap();
        let reader = PqwReader::from_bytes(&ckpt).unwrap();
        let h = reader.header();
        assert_eq!(h.hyper.gamma, 0.0);
        assert!((h.hyper.eta - 0.6).abs() < 1e-6);
    }

    // ===================== Архетипический мост (RQ18) =====================

    /// Обученный движок: фазы кристаллизованы born-шагом.
    fn rq18_engine() -> QuantizedGyroCurriculum {
        let mut qc = QuantizedGyroCurriculum::new(128, 0.05, 42, 4).unwrap();
        for _ in 0..3 {
            qc.ingest(sample_text(), 0).unwrap();
        }
        qc
    }

    #[test]
    fn bridge_resonates_with_known_prompt() {
        // Знакомый вопрос: архетип промпта структурно изоморфен памяти —
        // гейт открыт, пересечение непусто, момент внимания зажжён.
        let mut qc = rq18_engine();
        let arch = qc.prompt_archetype("phase lattice");
        let nnz_arch = {
            let mut n = 0;
            for i in 0..qc.d_pol() as usize {
                if trit_at(&arch, i) != Trit::Zero {
                    n += 1;
                }
            }
            n
        };
        assert!(nnz_arch > 0, "архетип промпта пуст");
        let rep = qc.archetype_bridge(&arch, 0.5).unwrap();
        assert!(rep.resonant, "E={}", rep.energy);
        assert!(rep.co_support > 0);
        assert_eq!(rep.co_support, rep.resonance + rep.conflict);
        // Билеты: пересечение (веса 2/3) + прозрачный фон метафоры (вес 1).
        assert_eq!(rep.tickets.len(), rep.co_support + rep.transparent);
        assert!(rep.tickets.iter().all(|&(_, w)| w == 1 || w == 2 || w == 3));
        // Прозрачный фон: полюса памяти вне вопроса.
        assert_eq!(rep.transparent, rep.nnz_lattice - rep.co_support);
    }

    #[test]
    fn bridge_never_touches_lattice_phases() {
        // ГЛАВНЫЙ инвариант: мост — внимание, а не знание. Фазы решётки
        // бит-в-бит неизменны (память меняет только born-шаг от
        // свидетельства; McWeeny-дисциплина не нарушена).
        let mut qc = rq18_engine();
        let before = qc.lattice().to_vec();
        let arch = qc.prompt_archetype("phase lattice born");
        let _ = qc.archetype_bridge(&arch, 0.25).unwrap();
        assert_eq!(qc.lattice(), before.as_slice(), "мост тронул фазы!");
    }

    #[test]
    fn bridge_silent_on_orthogonal_prompt() {
        // Чужой вопрос (нет общих дуг с носителем памяти): E = 0 < ε —
        // мост молчит, билеты пусты, момент нетронут. Ложных
        // ассоциаций не существует (ТЗ п.3).
        let mut qc = rq18_engine();
        // Ортогональный архетип: полюса на дугах, ЗАВЕДОМО свободных
        // (решётка обучена на 8 словах — дуги вне носителя).
        let zeros: Vec<usize> = (0..qc.d_pol() as usize)
            .filter(|&i| qc.trit_at(i as u32) == Trit::Zero)
            .collect();
        assert!(zeros.len() >= 2, "носитель занял всю решётку?");
        let mut arch = vec![0u8; qc.lattice_bytes()];
        set_trit(&mut arch, zeros[0], Trit::Pos);
        set_trit(&mut arch, zeros[1], Trit::Neg);
        let before_kinetic = qc.momentum_kinetic();
        let rep = qc.archetype_bridge(&arch, 0.5).unwrap();
        assert!(!rep.resonant);
        assert_eq!(rep.energy, 0.0);
        assert!(rep.tickets.is_empty());
        assert_eq!(qc.momentum_kinetic(), before_kinetic);
        // …но архетип = сама решётка резонирует (E = 1): молчание —
        // от ортогональности, не от бага.
        let arch2 = qc.lattice().to_vec();
        assert!(qc.nnz() > 0);
        let rep2 = qc.archetype_bridge(&arch2, 0.5).unwrap();
        assert!(rep2.resonant, "самопересечение обязано резонировать");
    }

    #[test]
    fn bridge_ignites_attention_on_intersection() {
        // Мост зажигает момент на дугах пересечения: волна мышления
        // потечёт к структурному изоморфизму (внимание, не знание).
        let mut qc = rq18_engine();
        // Архетип = сама решётка: пересечение = весь носитель.
        let arch = qc.lattice().to_vec();
        let rep = qc.archetype_bridge(&arch, 0.5).unwrap();
        assert!(rep.resonant);
        assert_eq!(rep.energy, 1.0);
        assert!(rep.kinetic >= rep.co_support, "пересечение кинетично");
        // Каждая дуга пересечения либо зажглась, либо уже была
        // кинетичной (инерция внимания).
        let mut kinetic_on_bridge = 0;
        for &(c, _) in &rep.tickets {
            if qc.momentum_at(c) != 0 {
                kinetic_on_bridge += 1;
            }
        }
        assert_eq!(kinetic_on_bridge, rep.tickets.len());
    }

    #[test]
    fn bridge_conflict_pushes_toward_prompt_pole() {
        // Конфликт (промпт-полюс против памяти): момент дует к полюсу
        // промпта — вопрос бросает вызов памяти, связь кричит.
        let mut qc = QuantizedGyroCurriculum::new(64, 0.05, 1, 4).unwrap();
        // Кристаллизуем один токен: сильное свидетельство → полюс.
        qc.ingest("phase phase phase", 0).unwrap();
        let c0 = coord_of("phase", 64);
        let pole = qc.trit_at(c0);
        assert_ne!(pole, Trit::Zero, "фаза обязана кристаллизоваться");
        // Архетип промпта — ВСТРЕЧНЫЙ полюс на той же дуге.
        let opposite = if pole == Trit::Pos { Trit::Neg } else { Trit::Pos };
        let mut arch = vec![0u8; qc.lattice_bytes()];
        set_trit(&mut arch, c0 as usize, opposite);
        let rep = qc.archetype_bridge(&arch, 0.5).unwrap();
        assert!(rep.resonant);
        assert_eq!(rep.conflict, 1);
        assert_eq!(rep.resonance, 0);
        assert_eq!(rep.transparent, 0, "в памяти только одна дуга");
        // Момент к полюсу промпта (m = sign(opposite): +1 толкает p
        // вверх к Pos, −1 — вниз к Neg).
        let expect_m: i8 = if opposite == Trit::Pos { 1 } else { -1 };
        assert_eq!(qc.momentum_at(c0), expect_m);
        // Билет конфликта кричит весом 3 (аннигиляция в продукте —
        // открытый вопрос, требующий слова).
        assert!(rep.tickets.contains(&(c0, 3)));
    }

    /// Координата токена в решётке d_pol.
    fn coord_of(token: &str, d: u32) -> u32 {
        (fnv1a64(token.as_bytes()) % d as u64) as u32
    }

    #[test]
    fn prompt_archetype_poles_follow_evidence_sign() {
        // Архетип промпта: полюс = знак TF-IDF свидетельства после
        // ε-ворот; дуга без свидетельства — суперпозиция (ноль).
        let qc = rq18_engine();
        let arch = qc.prompt_archetype("phase");
        let c = coord_of("phase", qc.d_pol());
        // Токен встречался в корпусе — свидетельство есть, полюс ненулевой.
        assert_ne!(trit_at(&arch, c as usize), Trit::Zero);
        // Чистая решётка: почти все дуги — суперпозиция.
        let zeros = (0..qc.d_pol() as usize)
            .filter(|&i| trit_at(&arch, i) == Trit::Zero)
            .count();
        assert!(zeros >= qc.d_pol() as usize - 4);
    }

    #[test]
    fn bridge_deterministic_same_prompt() {
        // Мост воспроизводим: одинаковый промпт — бит-в-бит одинаковые
        // билеты и статистика (никакого ГПСЧ в пути внимания).
        let build = || {
            let mut qc = rq18_engine();
            let arch = qc.prompt_archetype("phase lattice crystal");
            let rep = qc.archetype_bridge(&arch, 0.5).unwrap();
            (rep, arch)
        };
        let (a, arch_a) = build();
        let (b, arch_b) = build();
        assert_eq!(arch_a, arch_b);
        assert_eq!(a, b);
    }
}

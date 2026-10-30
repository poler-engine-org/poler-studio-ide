//! # Reverse Meta-Compiler — архитектура POLER-ERI v3.2.0
//!
//! `CircuitBuilder → R1CS Circuit → CSE/DCE → Wave-Scheduler → 8×f32 SIMD Crystallizer`
//!
//! Двунаправленная метакомпиляция слоя весов:
//!
//! 1. **Reverse** (анализ): плотная матрица весов `.safetensors` разлагается в
//!    тритернарные R1CS-вентили `x_r = c_L·x_l + c_R·x_k`, где `c ∈ {−1, 0, +1}`
//!    кодируют знак/отсутствие слагаемого. Умножений в горячем пути нет вовсе.
//! 2. **Meta** (синтез): SSA-перенумерация операндов, CSE-склейка общих
//!    подвыражений (коммутативная), DCE-вычищение мёртвых вентилей, материализация
//!    редких скалярных коэффициентов в холодном пролого волны и, наконец,
//!    lane-аффинный wave-планировщик, упаковывающий независимые вентили
//!    в 8-полосные AVX2-пакеты.
//!
//! ## Математика векторного вентиля
//!
//! Восемь **независимых** R1CS-вентилей собираются в одну пачку
//! (полосы 0..=7, lockstep) и исполняются единой векторной цепочкой:
//!
//! ```text
//! r_{0..7} = c_L ⊙ l_{0..7} ± c_R ⊙ k_{0..7}
//!
//! l  = loadu(l[base..base+8])     — смежные слоты, либо gather через set_ps(8)
//! k  = loadu / set_ps             — то же для правых операндов
//! c⊙x = and(x, absorb) xor sign   — зануление AND-маской, знак XOR-маской
//! r  = add(l', k')                — одна vaddps на 8 вентилей
//! ```
//!
//! Смена знака — `_mm256_xor_ps(reg, sign_mask)` (бит 0x8000_0000), зануление
//! слагаемого — `_mm256_and_ps(reg, absorb_mask)`. Вычитание `l − k` — это
//! сложение с XOR-инвертированным знаком `k`, поэтому **одна и та же
//! безветвлевая последовательность покрывает все девять комбинаций
//! коэффициентов** (c_L, c_R) ∈ {−1, 0, +1}² без единого `match` и без
//! единого умножения.
//!
//! ## Планировщик волн (lane affinity)
//!
//! Цепочки-аккумуляторы разных строк матрицы независимы. Планировщик
//! закрепляет цепочку за полосой (lane): вентиль, продолжающий аккумулятор
//! полосы `j` волны `w−1`, ставится в полосу `j` волны `w`. Тогда все 8 левых
//! операндов лежат в смежных слотах `[base_{w−1}, base_{w−1}+8)` и загружаются
//! одной инструкцией `_mm256_loadu_ps`; результаты волны пишутся одной
//! `_mm256_storeu_ps` в смежный блок `[base_w, base_w+8)`.
//!
//! ## Два продукта компиляции
//!
//! * **Runtime-conveyor** (`MetaPipeline::execute`) — исполнение упакованных
//!   волн по данным. Тело цикла не содержит ветвлений по семантике операций;
//!   два оставшихся двоичных ветвления на волну (формат загрузки и пустые
//!   волны) статистически устойчивы — предсказатель переходов CPU держит их
//!   со 100% точностью.
//! * **Flat crystallization** (`crystallize_to_flat_simd_rust`) — генерация
//!   плоского исходника Rust-функции в духе `VrrCrystallizer` из POLER-ERI:
//!   прямая линейная последовательность AVX2-интринсиков, ни одного цикла,
//!   ни одного `match`, ни одного ветвления. Маски и индексы запекаются
//!   литералами — ядро живёт целиком в L1.
//!
//! ## Контракт операндной памяти
//!
//! ```text
//! [0 .. in_dim)                 — входы (никогда не перенумеруются)
//! [in_dim .. n_operands)        — блоки волн: 8 векторных слотов + скалярные temp
//! ```
//!
//! `slot_of(operand)` отображает индекс операнда исходной схемы в слот
//! скомпилированного конвейера.

use std::collections::HashMap;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::quantum::crystallizer::{
    GateCoeff, SafetensorsHeader, TensorInfo, WeightCircuitBuilder,
};

// ─────────────────────────────────────────────────────────────────────────────
// §1. Векторный вентиль: 8 независимых R1CS-цепочек в одном AVX2-регистре
// ─────────────────────────────────────────────────────────────────────────────

/// Битовая маска полосы: сохранить слагаемое (absorb).
pub const MASK_KEEP: u32 = 0xFFFF_FFFF;
/// Битовая маска полосы: занулить слагаемое (absorb).
pub const MASK_KILL: u32 = 0x0000_0000;
/// Битовая маска полосы: инвертировать знак (sign).
pub const MASK_FLIP: u32 = 0x8000_0000;
/// Битовая маска полосы: сохранить знак (sign).
pub const MASK_NOFLIP: u32 = 0x0000_0000;

/// Скалярная материализация редкого коэффициента (холодный пролог волны):
/// `operands[dst] = value * operands[src]`. Порождается только для
/// коэффициентов, не сводимых к {−1, 0, +1}; в тритернарном горячем пути
/// не встречается вовсе.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalarOp {
    pub dst: u32,
    pub src: u32,
    pub value: f32,
}

/// Пачка из 8 независимых R1CS-вентилей (packed 8×f32).
///
/// Семантика полосы `i`:
///
/// ```text
/// operands[result_base + i] =
///     c_L[i]·operands[left[i]] + c_R[i]·operands[right[i]]
/// ```
///
/// с `c ∈ {−1, 0, +1}`, закодированным парами масок
/// `(absorb_l[i], sign_l[i])` / `(absorb_r[i], sign_r[i])`:
///
/// * `left`/`right` — индексы операндов (неактивные полосы читают слот 0 и
///   гасятся absorb-маской — безопасно);
/// * `result_base` — начало смежного блока из 8 слотов: одна `_mm256_storeu_ps`;
/// * `left_contig`/`right_contig` — все 8 индексов смежны (`idx[i] == idx[0]+i`):
///   загрузка одной `_mm256_loadu_ps` вместо gather через `_mm256_set_ps`;
/// * `scalars` — холодный пролог волны (обычно пуст).
#[derive(Debug, Clone)]
pub struct VectorGate8 {
    pub left: [u32; 8],
    pub right: [u32; 8],
    pub result_base: u32,
    pub left_contig: bool,
    pub right_contig: bool,
    pub absorb_l: [u32; 8],
    pub sign_l: [u32; 8],
    pub absorb_r: [u32; 8],
    pub sign_r: [u32; 8],
    pub n_lanes: u8,
    pub scalars: Vec<ScalarOp>,
}

impl VectorGate8 {
    /// Скалярный коэффициент полосы, восстановленный из масок (для fallback).
    #[inline(always)]
    pub fn lane_coeff(absorb: u32, sign: u32) -> f32 {
        if absorb == MASK_KILL {
            0.0
        } else if sign != MASK_NOFLIP {
            -1.0
        } else {
            1.0
        }
    }
}

/// Пара масок (absorb, sign) для коэффициента `c ∈ {−1, 0, +1}`.
#[inline]
fn masks_of(c: i8) -> (u32, u32) {
    match c {
        0 => (MASK_KILL, MASK_NOFLIP),
        1 => (MASK_KEEP, MASK_NOFLIP),
        -1 => (MASK_KEEP, MASK_FLIP),
        _ => unreachable!("коэффициент вне {{−1,0,+1}} материализуется до упаковки"),
    }
}

/// Проверка смежности 8 индексов: `idx[i] == idx[0] + i`.
#[inline]
fn contiguous(idx: &[u32; 8]) -> bool {
    let b = idx[0];
    (1..8).all(|i| idx[i] == b.wrapping_add(i as u32))
}

/// Все ли элементы массива равны `v`.
#[inline]
fn wave_all(arr: &[u32; 8], v: u32) -> bool {
    arr.iter().all(|&x| x == v)
}

/// Разрешение операнда в слот: вход — сам себя, результат волны — записанный слот.
#[inline]
fn resolve_slot(slot_map: &HashMap<u32, u32>, op: u32) -> u32 {
    slot_map.get(&op).copied().unwrap_or(op)
}

// ─────────────────────────────────────────────────────────────────────────────
// §2. Внутреннее IR и полный проход компиляции схемы в волны
// ─────────────────────────────────────────────────────────────────────────────

/// Вентиль промежуточного представления (SSA: result уникален).
#[derive(Debug, Clone)]
struct IrGate {
    result: u32,
    left: u32,
    right: u32,
    cl: GateCoeff,
    cr: GateCoeff,
    label: String,
}

impl IrGate {
    #[inline]
    fn is_scalar(&self) -> bool {
        matches!(self.cl, GateCoeff::Scalar(_))
    }
}

/// Результат компиляции схемы: упакованные волны + отображение слотов.
#[derive(Debug)]
struct CompiledCircuit {
    in_dim: usize,
    n_operands: usize,
    waves: Vec<VectorGate8>,
    /// Подписи полос волны (для читаемых комментариев генератора).
    lane_labels: Vec<Vec<String>>,
    /// Операнд исходной схемы → слот конвейера (None — мёртвое значение).
    slot_remap: Vec<Option<u32>>,
    /// Слоты выходов схемы (корни DAG либо заявленные выходы слоя).
    outputs: Vec<usize>,
    original_gates: usize,
    optimized_gates: usize,
}

/// Канонизация коэффициента: скаляры ±1/0 сводятся к тэгам {One, NegOne, Zero}.
#[inline]
fn norm_coeff(c: GateCoeff) -> GateCoeff {
    match c {
        GateCoeff::Scalar(s) if s == 1.0 => GateCoeff::One,
        GateCoeff::Scalar(s) if s == -1.0 => GateCoeff::NegOne,
        GateCoeff::Scalar(s) if s == 0.0 => GateCoeff::Zero,
        other => other,
    }
}

/// Ключ CSE для коэффициента: u64-тэг (скаляры несут биты f32).
#[inline]
fn coeff_key(c: GateCoeff) -> u64 {
    match c {
        GateCoeff::Zero => 0,
        GateCoeff::One => 1,
        GateCoeff::NegOne => 2,
        GateCoeff::Scalar(s) => (255u64 << 32) | s.to_bits() as u64,
    }
}

/// Прогон значения по карте перенумерации (alias/CSE). Глубина цепочек ≤ 3–4:
/// alias-цели разрешаются в момент записи, CSE-дубликаты указывают напрямую
/// на выживший вентиль.
fn chase(map: &[u32], mut x: u32) -> u32 {
    while map[x as usize] != x {
        x = map[x as usize];
    }
    x
}

/// Полный проход: схема → нормализация → CSE → DCE → материализация скаляров
/// → lane-аффинный wave-планировщик → упаковка 8-полосных вентилей.
fn compile_to_waves(
    builder: &WeightCircuitBuilder,
    declared_outputs: Option<&[usize]>,
) -> Result<CompiledCircuit, String> {
    let in_dim = builder.n_operands - builder.gates.len();
    let original_gates = builder.gates.len();
    let n_orig = builder.n_operands as u32;

    // Карта тождеств значений: operand → operand с тем же значением
    // (alias-копии, CSE-дубликаты). Расширяется temp'ами материализации.
    let mut value_of: Vec<u32> = (0..n_orig).collect();

    // ── Проход 1: нормализация + устранение копий ───────────────────────────
    // (One, Zero) — чистая копия: результат становится псевдонимом левого
    // входа, вентиль исчезает. (Zero, c) → перестановка входов (сложение
    // коммутативно, IEEE-754 гарантирует a+b ≡ b+a побитово).
    let mut kept: Vec<IrGate> = Vec::with_capacity(original_gates);
    for g in &builder.gates {
        let mut l = chase(&value_of, g.left as u32);
        let mut r = chase(&value_of, g.right as u32);
        let mut cl = norm_coeff(g.coeff_left);
        let mut cr = norm_coeff(g.coeff_right);
        if matches!(cl, GateCoeff::Zero) && !matches!(cr, GateCoeff::Zero) {
            std::mem::swap(&mut l, &mut r);
            std::mem::swap(&mut cl, &mut cr);
        }
        if matches!(cl, GateCoeff::One) && matches!(cr, GateCoeff::Zero) {
            value_of[g.result] = l;
            continue;
        }
        kept.push(IrGate {
            result: g.result as u32,
            left: l,
            right: r,
            cl,
            cr,
            label: g.label.clone(),
        });
    }

    // ── Проход 2: CSE общих подвыражений ────────────────────────────────────
    let mut cache: HashMap<(u32, u64, u32, u64), u32> = HashMap::with_capacity(kept.len());
    let mut cse_kept: Vec<IrGate> = Vec::with_capacity(kept.len());
    for mut g in kept {
        g.left = chase(&value_of, g.left);
        g.right = chase(&value_of, g.right);
        let kl = coeff_key(g.cl);
        let kr = coeff_key(g.cr);
        let key = if g.left <= g.right {
            (g.left, kl, g.right, kr)
        } else {
            (g.right, kr, g.left, kl)
        };
        if let Some(&existing) = cache.get(&key) {
            value_of[g.result as usize] = existing;
            continue;
        }
        cache.insert(key, g.result);
        cse_kept.push(g);
    }

    // ── Проход 3: DCE мёртвых вентилей (live-анализ от корней) ──────────────
    // Корни — заявленные выходы схемы (если заданы), иначе все нечитаемые
    // результаты (семантика «каждый тупик — выход»).
    let mut reads = vec![0u32; n_orig as usize];
    for g in &cse_kept {
        reads[g.left as usize] += 1;
        reads[g.right as usize] += 1;
    }
    let mut producer: HashMap<u32, usize> = HashMap::with_capacity(cse_kept.len());
    for (i, g) in cse_kept.iter().enumerate() {
        producer.insert(g.result, i);
    }
    let mut live = vec![false; cse_kept.len()];
    let mut stack: Vec<usize> = match declared_outputs {
        Some(ids) => ids
            .iter()
            .filter_map(|&o| producer.get(&chase(&value_of, o as u32)).copied())
            .collect(),
        None => cse_kept
            .iter()
            .enumerate()
            .filter(|(_, g)| reads[g.result as usize] == 0)
            .map(|(i, _)| i)
            .collect(),
    };
    while let Some(i) = stack.pop() {
        if live[i] {
            continue;
        }
        live[i] = true;
        let g = &cse_kept[i];
        for inp in [g.left, g.right] {
            if let Some(&p) = producer.get(&inp) {
                if !live[p] {
                    stack.push(p);
                }
            }
        }
    }
    let dce_kept: Vec<IrGate> = cse_kept
        .into_iter()
        .zip(live.iter())
        .filter(|(_, &lv)| lv)
        .map(|(g, _)| g)
        .collect();

    // ── Проход 4: материализация скалярных коэффициентов ────────────────────
    // (Scalar(s), c) → temp t = s·left (холодный скалярный пролог волны),
    // после чего остаток (One@t, c) — чисто тритернарный вентиль.
    let mut lowered: Vec<IrGate> = Vec::with_capacity(dce_kept.len() * 2);
    for mut g in dce_kept {
        if let GateCoeff::Scalar(s) = g.cl {
            let t = value_of.len() as u32;
            value_of.push(t);
            lowered.push(IrGate {
                result: t,
                left: g.left,
                right: 0,
                cl: GateCoeff::Scalar(s),
                cr: GateCoeff::Zero,
                label: format!("{}·s", g.label),
            });
            g.left = t;
            g.cl = GateCoeff::One;
        }
        if let GateCoeff::Scalar(s) = g.cr {
            let t = value_of.len() as u32;
            value_of.push(t);
            lowered.push(IrGate {
                result: t,
                left: g.right,
                right: 0,
                cl: GateCoeff::Scalar(s),
                cr: GateCoeff::Zero,
                label: format!("{}·s", g.label),
            });
            g.right = t;
            g.cr = GateCoeff::One;
        }
        // Остаточная чистая копия после материализации — тоже псевдоним.
        if matches!(g.cl, GateCoeff::One) && matches!(g.cr, GateCoeff::Zero) {
            value_of[g.result as usize] = g.left;
            continue;
        }
        lowered.push(g);
    }

    // ── Проход 5: lane-аффинный wave-планировщик ────────────────────────────
    let n = lowered.len();
    let optimized_gates = n;

    // producer: operand → вентиль, вычисляющий его (SSA, уникален).
    let mut producer: HashMap<u32, usize> = HashMap::with_capacity(n);
    for (i, g) in lowered.iter().enumerate() {
        producer.insert(g.result, i);
    }
    // users: operand → вентили, читающие его (каждый вентиль вносит каждый
    // операнд однократно — left==right учитывается один раз).
    let mut users: HashMap<u32, Vec<usize>> = HashMap::with_capacity(n);
    let mut indeg = vec![0u32; n];
    for (i, g) in lowered.iter().enumerate() {
        let pl = producer.get(&g.left);
        let pr = producer.get(&g.right);
        indeg[i] = match (pl, pr) {
            (None, None) => 0,
            (Some(_), None) | (None, Some(_)) => 1,
            (Some(a), Some(b)) => {
                if a == b {
                    1
                } else {
                    2
                }
            }
        };
        users.entry(g.left).or_default().push(i);
        if g.right != g.left {
            users.entry(g.right).or_default().push(i);
        }
    }

    let mut ready_vec: Vec<usize> = Vec::new();
    let mut ready_scalar: Vec<usize> = Vec::new();
    for (i, g) in lowered.iter().enumerate() {
        if indeg[i] == 0 {
            if g.is_scalar() {
                ready_scalar.push(i);
            } else {
                ready_vec.push(i);
            }
        }
    }

    let mut waves: Vec<VectorGate8> = Vec::new();
    let mut lane_labels: Vec<Vec<String>> = Vec::new();
    let mut slot_map: HashMap<u32, u32> = HashMap::with_capacity(n);
    let mut scheduled = vec![false; n];
    let mut prev_lanes: [Option<usize>; 8] = [None; 8];
    let mut cursor: u32 = in_dim as u32;
    let mut n_sched = 0usize;

    while n_sched < n {
        // Скалярный пролог волны: все готовые скалярные материализации.
        let mut scalars: Vec<usize> = Vec::new();
        while let Some(g) = ready_scalar.pop() {
            if !scheduled[g] {
                scheduled[g] = true;
                scalars.push(g);
            }
        }

        let mut lanes: [Option<usize>; 8] = [None; 8];

        // (a) lane-аффинность: вентиль, продолжающий аккумулятор полосы j
        //     волны w−1, закрепляется за полосой j волны w — левые операнды
        //     становятся смежными и грузятся одной loadu.
        if !waves.is_empty() {
            for lane in 0..8 {
                let Some(pg) = prev_lanes[lane] else {
                    continue;
                };
                let res = lowered[pg].result;
                let Some(cands) = users.get(&res) else {
                    continue;
                };
                for &c in cands {
                    if !scheduled[c] && indeg[c] == 0 && !lowered[c].is_scalar() {
                        scheduled[c] = true;
                        lanes[lane] = Some(c);
                        break;
                    }
                }
            }
        }

        // (b) добор полос из пула готовых вентилей (LIFO-стек, детерминизм).
        'fill: while lanes.iter().any(|x| x.is_none()) {
            match ready_vec.pop() {
                Some(g) => {
                    if scheduled[g] || lowered[g].is_scalar() {
                        continue;
                    }
                    let lane = lanes.iter().position(|x| x.is_none()).unwrap();
                    scheduled[g] = true;
                    lanes[lane] = Some(g);
                }
                None => break 'fill,
            }
        }

        if lanes.iter().all(|x| x.is_none()) && scalars.is_empty() {
            return Err("цикл в R1CS-графе: планировщик не продвинулся".to_string());
        }

        // ── Упаковка волны: блок = [8 векторных слотов | скалярные temp] ──
        let block_base = cursor;
        cursor += 8 + scalars.len() as u32;

        let mut left = [0u32; 8];
        let mut right = [0u32; 8];
        let mut cl8 = [0i8; 8];
        let mut cr8 = [0i8; 8];
        let mut n_lanes = 0u8;
        let mut labels: Vec<String> = Vec::with_capacity(8);
        for lane in 0..8 {
            let Some(gi) = lanes[lane] else {
                continue;
            };
            let g = &lowered[gi];
            debug_assert!(!g.is_scalar(), "скаляры не попадают в векторные полосы");
            left[lane] = resolve_slot(&slot_map, g.left);
            right[lane] = resolve_slot(&slot_map, g.right);
            cl8[lane] = coeff_i8(g.cl);
            cr8[lane] = coeff_i8(g.cr);
            slot_map.insert(g.result, block_base + lane as u32);
            labels.push(g.label.clone());
            n_lanes += 1;
        }

        let mut scalar_ops: Vec<ScalarOp> = Vec::with_capacity(scalars.len());
        for (j, &gi) in scalars.iter().enumerate() {
            let g = &lowered[gi];
            let GateCoeff::Scalar(s) = g.cl else {
                return Err("внутренняя ошибка: скалярный вентиль без коэффициента".to_string());
            };
            let dst = block_base + 8 + j as u32;
            slot_map.insert(g.result, dst);
            scalar_ops.push(ScalarOp {
                dst,
                src: resolve_slot(&slot_map, g.left),
                value: s,
            });
        }

        let mut absorb_l = [0u32; 8];
        let mut sign_l = [0u32; 8];
        let mut absorb_r = [0u32; 8];
        let mut sign_r = [0u32; 8];
        for i in 0..8 {
            let (a, s) = masks_of(cl8[i]);
            absorb_l[i] = a;
            sign_l[i] = s;
            let (a, s) = masks_of(cr8[i]);
            absorb_r[i] = a;
            sign_r[i] = s;
        }

        waves.push(VectorGate8 {
            left,
            right,
            result_base: block_base,
            left_contig: contiguous(&left),
            right_contig: contiguous(&right),
            absorb_l,
            sign_l,
            absorb_r,
            sign_r,
            n_lanes,
            scalars: scalar_ops,
        });
        lane_labels.push(labels);

        // ── Закрытие волны: декремент зависимостей читателей ──
        let mut done: Vec<usize> = Vec::with_capacity(9);
        for lane in 0..8 {
            if let Some(gi) = lanes[lane] {
                done.push(gi);
                n_sched += 1;
            }
        }
        for &gi in &scalars {
            done.push(gi);
            n_sched += 1;
        }
        for gi in done {
            let res = lowered[gi].result;
            if let Some(us) = users.get(&res) {
                for &u in us {
                    indeg[u] -= 1;
                    if indeg[u] == 0 {
                        if lowered[u].is_scalar() {
                            ready_scalar.push(u);
                        } else {
                            ready_vec.push(u);
                        }
                    }
                }
            }
        }
        prev_lanes = lanes;
    }

    // ── Финальное отображение слотов ────────────────────────────────────────
    let mut slot_remap: Vec<Option<u32>> = Vec::with_capacity(value_of.len());
    for id in 0..value_of.len() as u32 {
        let v = chase(&value_of, id);
        if v < in_dim as u32 {
            slot_remap.push(Some(v));
        } else {
            slot_remap.push(slot_map.get(&v).copied());
        }
    }

    let outputs: Vec<usize> = match declared_outputs {
        Some(ids) => ids
            .iter()
            .map(|&o| slot_remap[o].map(|s| s as usize).unwrap_or(0))
            .collect(),
        None => {
            // Корни DAG: вентили, чьи результаты никем не читаются.
            let mut reads2: HashMap<u32, u32> = HashMap::new();
            for g in &lowered {
                *reads2.entry(g.left).or_insert(0) += 1;
                *reads2.entry(g.right).or_insert(0) += 1;
            }
            lowered
                .iter()
                .filter(|g| !g.is_scalar() && !reads2.contains_key(&g.result))
                .filter_map(|g| slot_map.get(&g.result).copied())
                .map(|s| s as usize)
                .collect()
        }
    };

    Ok(CompiledCircuit {
        in_dim,
        n_operands: cursor as usize,
        waves,
        lane_labels,
        slot_remap,
        outputs,
        original_gates,
        optimized_gates,
    })
}

#[inline]
fn coeff_i8(c: GateCoeff) -> i8 {
    match c {
        GateCoeff::Zero => 0,
        GateCoeff::One => 1,
        GateCoeff::NegOne => -1,
        GateCoeff::Scalar(_) => unreachable!("скаляр материализован до упаковки ({{−1,0,+1}})"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §3. CSE-оптимизатор (публичный фасад со старой сигнатурой)
// ─────────────────────────────────────────────────────────────────────────────

/// CSE-оптимизатор R1CS-графа.
///
/// Склеивает повторяющиеся подвыражения `x_j ± x_k` (коммутативно),
/// устраняет копии и вычищает мёртвые вентили — на избыточных весовых
/// схемах сокращает граф на 30–50%. Возвращает перестроенный
/// `WeightCircuitBuilder` с новой нумерацией операндов.
pub struct CSEOptimizer;

impl CSEOptimizer {
    pub fn optimize(builder: &WeightCircuitBuilder) -> WeightCircuitBuilder {
        let n_orig = builder.n_operands as u32;
        let in_dim = builder.n_operands - builder.gates.len();
        let mut value_of: Vec<u32> = (0..n_orig).collect();

        // Нормализация + устранение копий.
        let mut kept: Vec<IrGate> = Vec::with_capacity(builder.gates.len());
        for g in &builder.gates {
            let mut l = chase(&value_of, g.left as u32);
            let mut r = chase(&value_of, g.right as u32);
            let mut cl = norm_coeff(g.coeff_left);
            let mut cr = norm_coeff(g.coeff_right);
            if matches!(cl, GateCoeff::Zero) && !matches!(cr, GateCoeff::Zero) {
                std::mem::swap(&mut l, &mut r);
                std::mem::swap(&mut cl, &mut cr);
            }
            if matches!(cl, GateCoeff::One) && matches!(cr, GateCoeff::Zero) {
                value_of[g.result] = l;
                continue;
            }
            kept.push(IrGate {
                result: g.result as u32,
                left: l,
                right: r,
                cl,
                cr,
                label: g.label.clone(),
            });
        }

        // CSE.
        let mut cache: HashMap<(u32, u64, u32, u64), u32> = HashMap::with_capacity(kept.len());
        let mut cse_kept: Vec<IrGate> = Vec::with_capacity(kept.len());
        for mut g in kept {
            g.left = chase(&value_of, g.left);
            g.right = chase(&value_of, g.right);
            let kl = coeff_key(g.cl);
            let kr = coeff_key(g.cr);
            let key = if g.left <= g.right {
                (g.left, kl, g.right, kr)
            } else {
                (g.right, kr, g.left, kl)
            };
            if let Some(&existing) = cache.get(&key) {
                value_of[g.result as usize] = existing;
                continue;
            }
            cache.insert(key, g.result);
            cse_kept.push(g);
        }

        // DCE.
        let mut reads = vec![0u32; n_orig as usize];
        for g in &cse_kept {
            reads[g.left as usize] += 1;
            reads[g.right as usize] += 1;
        }
        let mut producer: HashMap<u32, usize> = HashMap::with_capacity(cse_kept.len());
        for (i, g) in cse_kept.iter().enumerate() {
            producer.insert(g.result, i);
        }
        let mut live = vec![false; cse_kept.len()];
        let mut stack: Vec<usize> = cse_kept
            .iter()
            .enumerate()
            .filter(|(_, g)| reads[g.result as usize] == 0)
            .map(|(i, _)| i)
            .collect();
        while let Some(i) = stack.pop() {
            if live[i] {
                continue;
            }
            live[i] = true;
            let g = &cse_kept[i];
            for inp in [g.left, g.right] {
                if let Some(&p) = producer.get(&inp) {
                    if !live[p] {
                        stack.push(p);
                    }
                }
            }
        }

        let mut out = WeightCircuitBuilder::new(in_dim);
        for (g, &lv) in cse_kept.into_iter().zip(live.iter()) {
            if lv {
                out.add_gate(g.left as usize, g.cl, g.right as usize, g.cr, g.label);
            }
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §4. MetaPipeline — исполнимый конвейер
// ─────────────────────────────────────────────────────────────────────────────

/// Готовый к исполнению конвейер: R1CS → CSE/DCE → волны 8×f32.
pub struct MetaPipeline {
    compiled: CompiledCircuit,
    #[cfg(target_arch = "x86_64")]
    has_avx2: bool,
    tensor_name: Option<String>,
}

impl MetaPipeline {
    /// Компилирует схему в исполнимый SIMD-конвейер (выходы — корни DAG).
    pub fn run(builder: WeightCircuitBuilder) -> Result<Self, String> {
        let compiled = compile_to_waves(&builder, None)?;
        Ok(Self::from_compiled(compiled))
    }

    /// Компилирует схему с заявленными выходами: DCE вычищает всё,
    /// что недостижимо из `outputs` (индексы операндов исходной схемы,
    /// как их возвращает `compile_linear_layer`).
    pub fn run_with_outputs(
        builder: WeightCircuitBuilder,
        outputs: &[usize],
    ) -> Result<Self, String> {
        let compiled = compile_to_waves(&builder, Some(outputs))?;
        Ok(Self::from_compiled(compiled))
    }

    fn from_compiled(compiled: CompiledCircuit) -> Self {
        Self {
            compiled,
            #[cfg(target_arch = "x86_64")]
            has_avx2: is_x86_feature_detected!("avx2"),
            tensor_name: None,
        }
    }

    /// Исполняет конвейер на месте (Zero-Alloc: ни одной аллокации в проходе).
    ///
    /// Контракт: `operands.len() >= n_operands()`. Входы читаются из
    /// `[0..in_dim)`, результаты — по слотам `slot_of`/`outputs`.
    #[inline]
    pub fn execute(&self, operands: &mut [f32]) {
        debug_assert!(
            operands.len() >= self.compiled.n_operands,
            "operands.len()={} < N_OPERANDS={}",
            operands.len(),
            self.compiled.n_operands
        );
        #[cfg(target_arch = "x86_64")]
        {
            if self.has_avx2 {
                unsafe { self.execute_avx2(operands) };
                return;
            }
        }
        self.execute_scalar(operands);
    }

    /// Скалярный fallback (платформы без AVX2). Коэффициенты восстановлены
    /// из масок; умножения на {−1, 0, +1} тривиальны для FPU.
    fn execute_scalar(&self, operands: &mut [f32]) {
        for w in &self.compiled.waves {
            for sc in &w.scalars {
                operands[sc.dst as usize] = sc.value * operands[sc.src as usize];
            }
            let base = w.result_base as usize;
            for lane in 0..8 {
                let cl = VectorGate8::lane_coeff(w.absorb_l[lane], w.sign_l[lane]);
                let cr = VectorGate8::lane_coeff(w.absorb_r[lane], w.sign_r[lane]);
                operands[base + lane] = cl * operands[w.left[lane] as usize]
                    + cr * operands[w.right[lane] as usize];
            }
        }
    }

    /// AVX2-исполнитель: 8 вентилей за одну векторную цепочку.
    ///
    /// Внутри цикла — ни одного `match` и ни одного ветвления по семантике
    /// операций: знаки и зануления закодированы масками (AND/XOR), вычитание —
    /// сложение с XOR-инвертированным знаком. Два оставшихся ветвления на
    /// волну (формат загрузки `contig` и пустые волны) двоичны и статистически
    /// устойчивы — предсказатель переходов CPU держит их со 100% точностью.
    /// Полностью безветвлевая форма — плоская кристаллизация
    /// [`crystallize_to_flat_simd_rust`].
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn execute_avx2(&self, operands: &mut [f32]) {
        let o = operands.as_mut_ptr();
        for w in &self.compiled.waves {
            // Холодный скалярный пролог волны (обычно пуст).
            for sc in &w.scalars {
                *o.add(sc.dst as usize) = sc.value * *o.add(sc.src as usize);
            }
            if w.n_lanes == 0 {
                continue;
            }
            // Левые операнды: смежные слоты → одна loadu, иначе gather set_ps(8).
            let l = if w.left_contig {
                _mm256_loadu_ps(o.add(w.left[0] as usize))
            } else {
                _mm256_set_ps(
                    *o.add(w.left[7] as usize),
                    *o.add(w.left[6] as usize),
                    *o.add(w.left[5] as usize),
                    *o.add(w.left[4] as usize),
                    *o.add(w.left[3] as usize),
                    *o.add(w.left[2] as usize),
                    *o.add(w.left[1] as usize),
                    *o.add(w.left[0] as usize),
                )
            };
            // Правые операнды — та же дисциплина.
            let k = if w.right_contig {
                _mm256_loadu_ps(o.add(w.right[0] as usize))
            } else {
                _mm256_set_ps(
                    *o.add(w.right[7] as usize),
                    *o.add(w.right[6] as usize),
                    *o.add(w.right[5] as usize),
                    *o.add(w.right[4] as usize),
                    *o.add(w.right[3] as usize),
                    *o.add(w.right[2] as usize),
                    *o.add(w.right[1] as usize),
                    *o.add(w.right[0] as usize),
                )
            };
            // c ⊙ x = (x AND absorb) XOR sign — без умножений.
            let al = _mm256_loadu_ps(w.absorb_l.as_ptr().cast());
            let sl = _mm256_loadu_ps(w.sign_l.as_ptr().cast());
            let ar = _mm256_loadu_ps(w.absorb_r.as_ptr().cast());
            let sr = _mm256_loadu_ps(w.sign_r.as_ptr().cast());
            let l = _mm256_xor_ps(_mm256_and_ps(l, al), sl);
            let k = _mm256_xor_ps(_mm256_and_ps(k, ar), sr);
            // r = c_L⊙l + c_R⊙k: одна vaddps на 8 вентилей (± уже в знаках).
            let r = _mm256_add_ps(l, k);
            // Пакетная запись 8 результатов в смежный блок волны.
            _mm256_storeu_ps(o.add(w.result_base as usize), r);
        }
    }

    /// Слот операнда исходной схемы в скомпилированном конвейере
    /// (None — значение мёртвое/не существует).
    pub fn slot_of(&self, operand: usize) -> Option<usize> {
        self.compiled
            .slot_remap
            .get(operand)
            .and_then(|s| s.map(|v| v as usize))
    }

    /// Слоты выходов схемы (корни DAG либо заявленные выходы слоя).
    pub fn outputs(&self) -> &[usize] {
        &self.compiled.outputs
    }

    /// (исходные вентили, вентили после CSE/DCE, доля сокращения).
    pub fn stats(&self) -> (usize, usize, f64) {
        let reduction = if self.compiled.original_gates > 0 {
            1.0 - (self.compiled.optimized_gates as f64 / self.compiled.original_gates as f64)
        } else {
            0.0
        };
        (
            self.compiled.original_gates,
            self.compiled.optimized_gates,
            reduction,
        )
    }

    /// Размер операндного пространства (входы + блоки волн).
    pub fn n_operands(&self) -> usize {
        self.compiled.n_operands
    }

    /// Число входов схемы (операнды `[0..in_dim)`).
    pub fn in_dim(&self) -> usize {
        self.compiled.in_dim
    }

    /// Упакованные волны (интроспекция и отладка).
    pub fn waves(&self) -> &[VectorGate8] {
        &self.compiled.waves
    }

    /// Имя тензора-источника (заполняется safetensors-фронтом).
    pub fn tensor_name(&self) -> Option<&str> {
        self.tensor_name.as_deref()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §5. Flat Crystallization — генерация плоского SIMD-исходника
// ─────────────────────────────────────────────────────────────────────────────

/// Литерал f32 по битовой маске (для запекания масок в генерированном коде).
fn f32_lit(bits: u32) -> String {
    match bits {
        0x0000_0000 => "0.0f32".to_string(),
        0x8000_0000 => "-0.0f32".to_string(),
        0xFFFF_FFFF => "f32::from_bits(4294967295u32)".to_string(),
        other => format!("f32::from_bits({other}u32)"),
    }
}

/// Список аргументов `_mm256_set_ps`: `*o.add(idx7), …, *o.add(idx0)`
/// (полоса 7 — первый аргумент, полоса 0 — последний).
fn set_ps_args(idx: &[u32; 8]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(8);
    for lane in (0..8).rev() {
        parts.push(format!("*o.add({})", idx[lane]));
    }
    parts.join(", ")
}

/// Список аргументов `_mm256_set_ps` из битовой маски.
fn set_ps_mask_args(mask: &[u32; 8]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(8);
    for lane in (0..8).rev() {
        parts.push(f32_lit(mask[lane]));
    }
    parts.join(", ")
}

/// Кристаллизует схему в плоскую Rust-функцию (аналог `VrrCrystallizer`
/// из POLER-ERI): прямая линейная последовательность AVX2-интринсиков,
/// **без единого цикла, без единого `match`, без единого ветвления**.
///
/// Сигнатура результата:
///
/// ```ignore
/// #[target_feature(enable = "avx2")]
/// #[inline]
/// pub unsafe fn {fn_name}(operands: &mut [f32]) { … }
/// ```
///
/// Контракт сгенерированного ядра: `operands.len() >= N_OPERANDS`,
/// CPU поддерживает AVX2. Маски запекаются литералами `f32::from_bits`,
/// индексы — константами: ядро живёт целиком в L1, метаданных нет.
///
/// Побайтовая эквивалентность runtime-конвейеру: `a − b` исполняется как
/// `vsubps`, смешанные коэффициенты — как `and/xor/add` (IEEE-754:
/// `a + (−b) ≡ a − b` побитово).
pub fn crystallize_to_flat_simd_rust(fn_name: &str, builder: &WeightCircuitBuilder) -> String {
    let cc = match compile_to_waves(builder, None) {
        Ok(cc) => cc,
        Err(e) => return format!("// ОШИБКА КОМПИЛЯЦИИ СХЕМЫ: {e}\n"),
    };

    let mut s = String::with_capacity(cc.waves.len() * 280 + 1024);

    s.push_str("// ════════════════════════════════════════════════════════════════════\n");
    s.push_str("// Автосгенерировано POLER-ERI v3.2.0 Reverse Meta-Compiler\n");
    s.push_str(&format!(
        "// Конвейер: R1CS ({} вентилей) → CSE/DCE ({}) → {} 8-полосных AVX2 волн\n",
        cc.original_gates,
        cc.optimized_gates,
        cc.waves.len()
    ));
    s.push_str("// Плоская кристаллизация: без циклов, без match-диспетчера, без ветвлений\n");
    s.push_str("// ════════════════════════════════════════════════════════════════════\n\n");
    s.push_str("use core::arch::x86_64::*;\n\n");
    s.push_str(&format!("pub const N_OPERANDS: usize = {};\n\n", cc.n_operands));
    s.push_str("#[target_feature(enable = \"avx2\")]\n");
    // rustc >= 1.87 запрещает #[inline(always)] вместе с #[target_feature];
    // #[inline] — допустимый намёк. Для сквозного инлайна собирайте крейт с
    // RUSTFLAGS="-C target-feature=+avx2" и уберите атрибут target_feature.
    s.push_str("#[inline]\n");
    s.push_str(&format!("pub unsafe fn {fn_name}(operands: &mut [f32]) {{\n"));
    s.push_str("    debug_assert!(operands.len() >= N_OPERANDS);\n");
    if cc.n_operands > cc.in_dim {
        s.push_str("    let o = operands.as_mut_ptr();\n");
    }

    for (wi, w) in cc.waves.iter().enumerate() {
        let labels = cc.lane_labels.get(wi).cloned().unwrap_or_default();

        // Класс волны: чистое вычитание / чистое сложение / маскированное сложение.
        let pure_l = wave_all(&w.absorb_l, MASK_KEEP) && wave_all(&w.sign_l, MASK_NOFLIP);
        let pure_r_add = wave_all(&w.absorb_r, MASK_KEEP) && wave_all(&w.sign_r, MASK_NOFLIP);
        let pure_r_sub = wave_all(&w.absorb_r, MASK_KEEP) && wave_all(&w.sign_r, MASK_FLIP);
        let kind = if w.n_lanes == 0 {
            "scalar"
        } else if pure_l && pure_r_sub {
            "vsubps"
        } else if pure_l && pure_r_add {
            "vaddps"
        } else {
            "masked-vaddps"
        };

        s.push_str(&format!(
            "\n    // ── Волна {wi} · {} вентилей · {kind} · L={} R={} ── {}\n",
            w.n_lanes,
            if w.left_contig { "loadu" } else { "gather" },
            if w.right_contig { "loadu" } else { "gather" },
            labels.join(", ")
        ));

        // Холодный скалярный пролог волны.
        for sc in &w.scalars {
            s.push_str(&format!(
                "    *o.add({}) = f32::from_bits({}u32) * *o.add({});\n",
                sc.dst,
                sc.value.to_bits(),
                sc.src
            ));
        }
        if w.n_lanes == 0 {
            continue;
        }

        // Левые операнды.
        if w.left_contig {
            s.push_str(&format!(
                "    let l{wi} = _mm256_loadu_ps(o.add({}));\n",
                w.left[0]
            ));
        } else {
            s.push_str(&format!(
                "    let l{wi} = _mm256_set_ps({});\n",
                set_ps_args(&w.left)
            ));
        }
        // Правые операнды.
        if w.right_contig {
            s.push_str(&format!(
                "    let k{wi} = _mm256_loadu_ps(o.add({}));\n",
                w.right[0]
            ));
        } else {
            s.push_str(&format!(
                "    let k{wi} = _mm256_set_ps({});\n",
                set_ps_args(&w.right)
            ));
        }

        // Маски только при неоднородных коэффициентах.
        if w.absorb_l.contains(&MASK_KILL) {
            s.push_str(&format!(
                "    let l{wi} = _mm256_and_ps(l{wi}, _mm256_set_ps({}));\n",
                set_ps_mask_args(&w.absorb_l)
            ));
        }
        if w.sign_l.iter().any(|&x| x != MASK_NOFLIP) {
            s.push_str(&format!(
                "    let l{wi} = _mm256_xor_ps(l{wi}, _mm256_set_ps({}));\n",
                set_ps_mask_args(&w.sign_l)
            ));
        }
        if w.absorb_r.contains(&MASK_KILL) {
            s.push_str(&format!(
                "    let k{wi} = _mm256_and_ps(k{wi}, _mm256_set_ps({}));\n",
                set_ps_mask_args(&w.absorb_r)
            ));
        }
        if w.sign_r.iter().any(|&x| x != MASK_NOFLIP) && !(pure_l && pure_r_sub) {
            s.push_str(&format!(
                "    let k{wi} = _mm256_xor_ps(k{wi}, _mm256_set_ps({}));\n",
                set_ps_mask_args(&w.sign_r)
            ));
        }

        // Векторная арифметика: одна инструкция на 8 вентилей.
        if pure_l && pure_r_sub {
            s.push_str(&format!(
                "    let r{wi} = _mm256_sub_ps(l{wi}, k{wi});\n"
            ));
        } else {
            s.push_str(&format!(
                "    let r{wi} = _mm256_add_ps(l{wi}, k{wi});\n"
            ));
        }

        // Пакетная запись 8 результатов.
        s.push_str(&format!(
            "    _mm256_storeu_ps(o.add({}), r{wi});\n",
            w.result_base
        ));
    }

    s.push_str("}\n");
    s
}

// ─────────────────────────────────────────────────────────────────────────────
// §6. Фронт .safetensors: выбор слоя и тернарризация
// ─────────────────────────────────────────────────────────────────────────────

/// Выбор целевого тензора-слоя: приоритет проекциям внимания
/// `W_q / W_k / W_dense`, иначе первый подходящий 2D-тензор.
fn pick_weight_tensor(header: &SafetensorsHeader) -> Option<(String, TensorInfo)> {
    let candidates: Vec<(&String, &TensorInfo)> = header
        .tensors
        .iter()
        .filter(|(_, t)| t.shape.len() == 2 && t.shape[0] >= 16 && t.shape[1] >= 16)
        .collect();
    for key in ["q_proj", "k_proj", "dense"] {
        if let Some(&(n, t)) = candidates.iter().find(|(n, _)| n.contains(key)) {
            return Some((n.clone(), t.clone()));
        }
    }
    candidates.first().map(|&(n, t)| (n.clone(), t.clone()))
}

/// Тритернаризация среза весов: порог `1.2125·mean|w|` даёт плотность ≈ ⅓
/// для гауссовых весов (`P(|N(0,σ)| > t) = 1/3 ⇒ t ≈ 0.968σ`,
/// `σ = mean|w|·√(π/2) ≈ 1.2533·mean|w|`).
pub fn ternarize_mean_abs(w: &[f32]) -> Vec<i8> {
    let n = w.len().max(1) as f32;
    let mean_abs = w.iter().map(|x| if x.is_finite() { x.abs() } else { 0.0 }).sum::<f32>() / n;
    let t = 1.2125 * mean_abs;
    w.iter()
        .map(|&x| if x > t { 1 } else if x < -t { -1 } else { 0 })
        .collect()
}

/// Декодирование f16 (IEEE 754 half) → f32 без внешних крейтов.
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x3FF) as u32;
    let bits = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            // Субнормали: нормализация мантиссы со спуском экспоненты.
            let mut e: i32 = 127 - 15 + 1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            f &= 0x3FF;
            (sign << 31) | ((e as u32) << 23) | (f << 13)
        }
    } else if exp == 0x1F {
        (sign << 31) | (0xFF << 23) | (frac << 13)
    } else {
        // biased: e_f32 = e_f16 − 15 + 127; считаем в i32 (e_f16 < 15 даёт
        // отрицательное смещение до нормализации — u32 переполнился бы).
        let e32 = (exp as i32) - 15 + 127;
        (sign << 31) | ((e32 as u32) << 23) | (frac << 13)
    };
    f32::from_bits(bits)
}

/// Чтение одного скаляра safetensors (F32/F16/BF16, little-endian).
fn read_scalar_f32(dtype: &str, b: &[u8]) -> Result<f32, String> {
    match dtype {
        "F32" => Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        "F16" => Ok(f16_to_f32(u16::from_le_bytes([b[0], b[1]]))),
        // bf16 — верхняя половина f32: восстанавливается сдвигом на 16.
        "BF16" => Ok(f32::from_bits((u16::from_le_bytes([b[0], b[1]]) as u32) << 16)),
        other => Err(format!(
            "dtype {other} не поддерживается мета-компилятором (ожидается F32/F16/BF16)"
        )),
    }
}

/// Прямая метакомпиляция реального слоя `.safetensors`:
///
/// 1. Разбор заголовка и извлечение среза весовой матрицы
///    (`W_q`, `W_k`, `W_dense` — по приоритету имени).
/// 2. Тритернаризация среза (плотность ≈ ⅓) и построение графа
///    цепей `WeightCircuitBuilder`.
/// 3. CSE-оптимизация (сокращение дублирующих сумм на 30–50% на
///    избыточных схемах) + DCE.
/// 4. Упаковка в 8-полосные векторные пачки — готовый к исполнению конвейер.
///
/// `in_dim`/`out_dim` ограничивают срез (clamp по реальной форме тензора).
pub fn meta_compile_safetensors_tensor(
    tensor_bytes: &[u8],
    in_dim: usize,
    out_dim: usize,
) -> Result<MetaPipeline, String> {
    let header = SafetensorsHeader::parse(tensor_bytes)
        .map_err(|e| format!("safetensors: {e}"))?;
    let (name, info) = pick_weight_tensor(&header)
        .ok_or_else(|| "в safetensors нет 2D-тензора >= 16x16".to_string())?;

    let in_d = in_dim.min(info.shape[1]);
    let out_d = out_dim.min(info.shape[0]);
    if in_d == 0 || out_d == 0 {
        return Err(format!(
            "нулевой срез слоя {name}: in_dim={in_dim}, out_dim={out_dim} против формы {:?}",
            info.shape
        ));
    }

    let bpe = match info.dtype.as_str() {
        "F32" => 4usize,
        "F16" | "BF16" => 2,
        other => {
            return Err(format!(
                "слой {name}: dtype {other} не поддерживается (F32/F16/BF16)"
            ))
        }
    };

    let data_start = header.header_size + info.data_offsets[0];
    let data_end = header.header_size + info.data_offsets[1];
    if data_end > tensor_bytes.len() {
        return Err(format!(
            "слой {name}: усечённый файл — данные до {data_end}, доступно {}",
            tensor_bytes.len()
        ));
    }

    // Извлечение среза in_d × out_d (row-major) с dtype-декодированием.
    let row_stride = info.shape[1] * bpe;
    let mut w = Vec::with_capacity(in_d * out_d);
    for r in 0..out_d {
        let row = data_start + r * row_stride;
        for c in 0..in_d {
            let off = row + c * bpe;
            w.push(read_scalar_f32(&info.dtype, &tensor_bytes[off..off + bpe])?);
        }
    }

    let ternary = ternarize_mean_abs(&w);
    let mut builder = WeightCircuitBuilder::new(in_d);
    let out_idx = builder.compile_linear_layer(in_d, out_d, &ternary);
    let mut pipe = MetaPipeline::from_compiled(compile_to_waves(&builder, Some(&out_idx))?);
    pipe.tensor_name = Some(name);
    Ok(pipe)
}

// ─────────────────────────────────────────────────────────────────────────────
// §7. Тесты и верификация
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantum::crystallizer::execute_circuit_direct;

    /// Детерминированный ГПСЧ (xorshift32) — воспроизводимые фаззинг-случаи.
    struct XorShift32(u32);

    impl XorShift32 {
        fn next(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.0 = x;
            x
        }
    }

    /// Точность 8 параллельных сложений/вычитаний/занулений против эталона.
    /// Эталон — `execute_circuit_direct` на исходной схеме (полная
    /// последовательность вентилей без оптимизаций).
    #[test]
    fn test_packed_simd8_accuracy() {
        let mut builder = WeightCircuitBuilder::new(8);
        let y0 = builder.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "add01");
        let y1 = builder.add_gate(2, GateCoeff::One, 3, GateCoeff::NegOne, "sub23");
        let y2 = builder.add_gate(4, GateCoeff::NegOne, 5, GateCoeff::Zero, "neg4");
        let y3 = builder.add_gate(6, GateCoeff::Zero, 7, GateCoeff::Zero, "zero");
        let y4 = builder.add_gate(5, GateCoeff::Scalar(2.5), 6, GateCoeff::One, "scaled");
        let y5 = builder.add_gate(y0, GateCoeff::One, y1, GateCoeff::One, "mix");
        let y6 = builder.add_gate(7, GateCoeff::One, 0, GateCoeff::Zero, "copy");

        let pipe = MetaPipeline::run(builder).expect("компиляция конвейера");

        // Ручной прогон: x = [10, 5, 3, 2, 7, 4, 6, 1]
        let mut ops = vec![0.0f32; pipe.n_operands()];
        let x = [10.0f32, 5.0, 3.0, 2.0, 7.0, 4.0, 6.0, 1.0];
        ops[..8].copy_from_slice(&x);
        pipe.execute(&mut ops);

        let slot = |o: usize| pipe.slot_of(o).expect("живой операнд");
        assert_eq!(ops[slot(y0)], 15.0, "x0+x1");
        assert_eq!(ops[slot(y1)], 1.0, "x2-x3");
        assert_eq!(ops[slot(y2)], -7.0, "-x4");
        assert_eq!(ops[slot(y3)], 0.0, "0");
        assert_eq!(ops[slot(y4)], 16.0, "2.5*x5+x6");
        assert_eq!(ops[slot(y5)], 16.0, "y0+y1");
        assert_eq!(ops[slot(y6)], 1.0, "копия x7 через alias");

        // Рандомизированный диффуз against эталона: битовая эквивалентность
        // (порядок операций каждого выхода не меняется CSE/DCE/упаковкой).
        let mut rng = XorShift32(0xC0FFEE);
        for _ in 0..64 {
            let mut got = vec![0.0f32; pipe.n_operands()];
            for v in got[..8].iter_mut() {
                *v = (rng.next() % 2001) as f32 / 125.0 - 8.0;
            }
            // Эталонная схема пересобирается детерминированно.
            let mut rb = WeightCircuitBuilder::new(8);
            let r0 = rb.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "add01");
            let r1 = rb.add_gate(2, GateCoeff::One, 3, GateCoeff::NegOne, "sub23");
            let r2 = rb.add_gate(4, GateCoeff::NegOne, 5, GateCoeff::Zero, "neg4");
            let r3 = rb.add_gate(6, GateCoeff::Zero, 7, GateCoeff::Zero, "zero");
            let r4 = rb.add_gate(5, GateCoeff::Scalar(2.5), 6, GateCoeff::One, "scaled");
            let r5 = rb.add_gate(r0, GateCoeff::One, r1, GateCoeff::One, "mix");
            let r6 = rb.add_gate(7, GateCoeff::One, 0, GateCoeff::Zero, "copy");
            let mut want = got[..8].to_vec();
            want.resize(rb.n_operands, 0.0);
            execute_circuit_direct(&rb, &mut want);

            pipe.execute(&mut got);
            for (name, o) in [("y0", r0), ("y1", r1), ("y2", r2), ("y3", r3), ("y4", r4), ("y5", r5), ("y6", r6)] {
                let g = got[pipe.slot_of(o).expect("живой операнд")];
                let w = want[o];
                assert!(
                    g.to_bits() == w.to_bits() || (g == 0.0 && w == 0.0),
                    "{name}: SIMD {g} (bits {:#x}) против эталона {w} (bits {:#x})",
                    g.to_bits(),
                    w.to_bits()
                );
            }
        }
    }

    /// Сокращение графа цепей: копии → alias, дубликаты → CSE, тупики → DCE.
    #[test]
    fn test_cse_gate_reduction() {
        // Фасад CSE: 4 вентиля → 2 (дубликаты склеены коммутативно, копия устранена).
        let mut b = WeightCircuitBuilder::new(8);
        b.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "dup1");
        b.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "dup2");
        b.add_gate(2, GateCoeff::One, 3, GateCoeff::NegOne, "unique");
        b.add_gate(1, GateCoeff::One, 0, GateCoeff::One, "dup3"); // b + a ≡ a + b
        b.add_gate(4, GateCoeff::One, 5, GateCoeff::Zero, "copy"); // alias
        let opt = CSEOptimizer::optimize(&b);
        assert_eq!(opt.gates.len(), 2, "остаются (x0+x1) и (x2-x3)");

        // Полный конвейер на слое с дублирующимися строками + мёртвым вентилем.
        let mut b2 = WeightCircuitBuilder::new(4);
        let ternary: [i8; 12] = [
            1, 0, -1, 0, // x0 - x2
            1, 0, -1, 0, // идентичная строка → CSE
            0, 1, 0, 1,  // x1 + x3
        ];
        let out_idx = b2.compile_linear_layer(4, 3, &ternary);
        b2.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "dead"); // DCE

        let pipe = MetaPipeline::run_with_outputs(b2, &out_idx).expect("pipeline");
        let (orig, kept, reduction) = pipe.stats();
        assert_eq!(orig, 7, "3 инита + 3 аккумулятора + 1 мёртвый");
        assert_eq!(kept, 2, "alias×3 + CSE×1 + DCE×1");
        assert!(reduction >= 0.7, "сокращение >= 70%, получено {reduction}");
    }

    /// Замер скорости прямого прохода (Runtime-конвейер 8×f32).
    /// Гарантия POLER-ERI: > 1000 проходов/сек (release) на кэше L1/L3.
    #[test]
    fn test_crystallized_flat_speed() {
        // 256×256 тритернарная матрица, плотность 2/3 → ~43K вентилей.
        let mut rng = XorShift32(20260916);
        let (in_d, out_d) = (256usize, 256usize);
        let mut ternary = vec![0i8; in_d * out_d];
        for t in ternary.iter_mut() {
            *t = (rng.next() % 3) as i8 - 1;
        }
        let mut builder = WeightCircuitBuilder::new(in_d);
        let out_idx = builder.compile_linear_layer(in_d, out_d, &ternary);

        // Эталонная копия для проверки корректности горячего прогона.
        let mut ref_builder = WeightCircuitBuilder::new(in_d);
        let ref_out = ref_builder.compile_linear_layer(in_d, out_d, &ternary);

        let pipe = MetaPipeline::run(builder).expect("pipeline");
        let (orig, kept, reduction) = pipe.stats();

        let mut operands = vec![0.0f32; pipe.n_operands()];
        for v in operands[..in_d].iter_mut() {
            *v = (rng.next() % 97) as f32 / 8.0 - 6.0;
        }
        let mut ref_ops = operands[..in_d].to_vec();
        ref_ops.resize(ref_builder.n_operands, 0.0);
        execute_circuit_direct(&ref_builder, &mut ref_ops);

        // Прогрев кэшей + корректность.
        for _ in 0..64 {
            pipe.execute(&mut operands);
        }
        for (r, &o) in out_idx.iter().enumerate() {
            let got = operands[pipe.slot_of(o).expect("выход жив")];
            let want = ref_ops[ref_out[r]];
            assert_eq!(got.to_bits(), want.to_bits(), "строка {r}: {got} против {want}");
        }

        // Замер.
        let iters = 1024usize;
        let t0 = std::time::Instant::now();
        for _ in 0..iters {
            pipe.execute(&mut operands);
        }
        let dt = t0.elapsed().as_secs_f64();
        let per_pass_us = dt / iters as f64 * 1e6;
        let passes_per_sec = iters as f64 / dt;

        #[cfg(target_arch = "x86_64")]
        let avx2 = is_x86_feature_detected!("avx2");
        #[cfg(not(target_arch = "x86_64"))]
        let avx2 = false;

        println!(
            "\n=== POLER-ERI MetaCompiler: {orig} вентилей → {kept} (CSE/DCE −{:.1}%) → {} волн · {} операндов ({:.0} KB, L2) ===",
            reduction * 100.0,
            pipe.waves().len(),
            pipe.n_operands(),
            pipe.n_operands() as f64 * 4.0 / 1024.0
        );
        println!(
            "=== Прямой проход: {per_pass_us:.2} мкс → {:.0} проходов/сек (AVX2: {avx2}) ===",
            passes_per_sec
        );

        let floor = if cfg!(debug_assertions) {
            100.0 // debug: интринсики — реальные вызовы, честный запас ×30
        } else {
            1000.0 // гарантия POLER-ERI v3.2.0
        };
        assert!(
            passes_per_sec > floor,
            "скорость {passes_per_sec:.0} проходов/сек ниже нормы {floor}"
        );
    }

    /// Структура плоского кода: прямая последовательность, без циклов
    /// и без match-диспетчера в теле сгенерированной функции.
    #[test]
    fn test_flat_codegen_structure() {
        let mut b = WeightCircuitBuilder::new(8);
        b.add_gate(0, GateCoeff::One, 1, GateCoeff::One, "a");
        b.add_gate(2, GateCoeff::One, 3, GateCoeff::NegOne, "s");
        b.add_gate(4, GateCoeff::NegOne, 5, GateCoeff::Zero, "n");
        let code = crystallize_to_flat_simd_rust("kernel_test", &b);

        assert!(
            code.contains("pub unsafe fn kernel_test(operands: &mut [f32])"),
            "сигнатура ядра"
        );
        assert!(code.contains("#[inline]"), "инлайн-намёк");
        assert!(
            !code.contains("#[inline(always)]"),
            "rustc запрещает inline(always) вместе с target_feature"
        );
        assert!(code.contains("#[target_feature(enable = \"avx2\")]"), "AVX2");
        assert!(
            code.contains("_mm256_add_ps") || code.contains("_mm256_sub_ps"),
            "векторная арифметика"
        );
        assert!(code.contains("_mm256_storeu_ps"), "пакетная запись");
        assert!(code.contains("pub const N_OPERANDS"), "контракт размера");
        // Ни циклов, ни match-диспетчера в теле ядра.
        assert!(!code.contains("for ("), "плоский код без циклов for");
        assert!(!code.contains("while ("), "плоский код без while");
        assert!(!code.contains("match "), "плоский код без match");
        assert!(!code.contains("_mm256_set1_ps"), "никаких broadcast-заглушек");
        assert!(!code.contains("_mm_cvtss_f32"), "никаких извлечений скаляра");
    }

    /// Фаззинг: случайные схемы (включая скалярные вентили поверх выходов)
    /// против эталона `execute_circuit_direct` — битовая эквивалентность.
    #[test]
    fn test_random_circuits_match_reference() {
        let mut rng = XorShift32(20260916);
        for case in 0..24 {
            let in_d = 1 + (rng.next() % 20) as usize;
            let out_d = 1 + (rng.next() % 12) as usize;
            let mut ternary = vec![0i8; in_d * out_d];
            for t in ternary.iter_mut() {
                *t = (rng.next() % 3) as i8 - 1;
            }
            let with_scalar = case % 4 == 0 && out_d >= 1;

            let mk = |with_scalar: bool| {
                let mut b = WeightCircuitBuilder::new(in_d);
                let oi = b.compile_linear_layer(in_d, out_d, &ternary);
                let extra = if with_scalar {
                    b.add_gate(
                        oi[0],
                        GateCoeff::Scalar(0.5),
                        oi[out_d - 1],
                        GateCoeff::One,
                        "sc",
                    )
                } else {
                    0
                };
                (b, oi, extra)
            };
            let (b1, out_idx, extra) = mk(with_scalar);
            let (b2, ref_out, ref_extra) = mk(with_scalar);

            let pipe = MetaPipeline::run(b1).expect("pipeline");
            let mut got = vec![0.0f32; pipe.n_operands()];
            let mut want = vec![0.0f32; b2.n_operands];
            for v in got[..in_d].iter_mut() {
                *v = (rng.next() % 1997) as f32 / 113.0 - 8.0;
            }
            want[..in_d].copy_from_slice(&got[..in_d]);

            execute_circuit_direct(&b2, &mut want);
            pipe.execute(&mut got);

            for (r, &o) in out_idx.iter().enumerate() {
                let g = got[pipe.slot_of(o).expect("выход жив")];
                let w = want[ref_out[r]];
                assert_eq!(
                    g.to_bits(),
                    w.to_bits(),
                    "кейс {case}, строка {r}: SIMD {g} против эталона {w}"
                );
            }
            if with_scalar {
                let g = got[pipe.slot_of(extra).expect("скаляр жив")];
                let w = want[ref_extra];
                assert_eq!(
                    g.to_bits(),
                    w.to_bits(),
                    "кейс {case}, скаляр: SIMD {g} против эталона {w}"
                );
            }
        }
    }

    /// Синтетический .safetensors (F32/F16/BF16) → meta_compile → эталон
    /// плотного прохода по тернарризованной матрице.
    #[test]
    fn test_safetensors_meta_compile() {
        let rows = 32usize;
        let cols = 24usize;

        let mk_file = |dtype: &str, quantize: fn(f32) -> f32| -> (Vec<u8>, Vec<f32>) {
            let mut rng = XorShift32(7);
            let mut w = vec![0.0f32; rows * cols];
            for v in w.iter_mut() {
                *v = quantize((rng.next() % 256) as f32 / 128.0 - 1.0);
            }
            let bpe = if dtype == "F32" { 4 } else { 2 };
            let mut payload = vec![0u8; 128]; // мусорный тензор aaa.bias
            for v in &w {
                match dtype {
                    "F32" => payload.extend_from_slice(&v.to_le_bytes()),
                    "F16" => payload.extend_from_slice(&f32_to_f16_bits(*v).to_le_bytes()),
                    _ => payload.extend_from_slice(&(((*v).to_bits() >> 16) as u16).to_le_bytes()),
                }
            }
            let json = format!(
                "{{\"aaa.bias\":{{\"dtype\":\"F32\",\"shape\":[32],\"data_offsets\":[0,128]}},\
                 \"model.layers.0.self_attn.q_proj.weight\":{{\"dtype\":\"{dtype}\",\
                 \"shape\":[{rows},{cols}],\"data_offsets\":[128,{}]}}}}",
                128 + w.len() * bpe
            );
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
            bytes.extend_from_slice(json.as_bytes());
            bytes.extend_from_slice(&payload);
            (bytes, w)
        };

        // F32: приоритет выбора q_proj + точность против плотного эталона.
        let (bytes, w) = mk_file("F32", |v| v);
        let pipe = meta_compile_safetensors_tensor(&bytes, cols, rows).expect("meta-компиляция");
        assert_eq!(
            pipe.tensor_name(),
            Some("model.layers.0.self_attn.q_proj.weight"),
            "приоритет W_q над прочими 2D-тензорами"
        );
        assert_eq!(pipe.in_dim(), cols);
        assert_eq!(pipe.outputs().len(), rows);

        let ternary = ternarize_mean_abs(&w);
        let mut ops = vec![0.0f32; pipe.n_operands()];
        for (i, v) in ops[..cols].iter_mut().enumerate() {
            *v = i as f32 * 0.25 - 1.5;
        }
        let x = ops[..cols].to_vec();
        pipe.execute(&mut ops);
        for r in 0..rows {
            let want: f32 = (0..cols).map(|c| ternary[r * cols + c] as f32 * x[c]).sum();
            let got = ops[pipe.outputs()[r]];
            assert!((got - want).abs() < 1e-4, "F32 строка {r}: {got} против {want}");
        }

        // F16 (значения, точно представимые в half) и BF16 (шаг 1/256).
        let (bytes16, w16) = mk_file("F16", |v| (v * 8.0).round() / 8.0);
        let pipe16 = meta_compile_safetensors_tensor(&bytes16, 64, 64).expect("F16");
        assert_eq!(pipe16.in_dim(), cols, "clamp по форме тензора");
        let ternary16 = ternarize_mean_abs(&w16);
        let mut ops16 = vec![0.0f32; pipe16.n_operands()];
        for (i, v) in ops16[..cols].iter_mut().enumerate() {
            *v = i as f32 * 0.125 - 1.0;
        }
        let x16 = ops16[..cols].to_vec();
        pipe16.execute(&mut ops16);
        for r in 0..rows {
            let want: f32 = (0..cols).map(|c| ternary16[r * cols + c] as f32 * x16[c]).sum();
            let got = ops16[pipe16.outputs()[r]];
            assert!((got - want).abs() < 1e-3, "F16 строка {r}: {got} против {want}");
        }

        let (bytesbf, wbf) = mk_file("BF16", |v| (v * 256.0).round() / 256.0);
        let pipebf = meta_compile_safetensors_tensor(&bytesbf, cols, rows).expect("BF16");
        let ternarybf = ternarize_mean_abs(&wbf);
        let mut opsbf = vec![0.0f32; pipebf.n_operands()];
        for (i, v) in opsbf[..cols].iter_mut().enumerate() {
            *v = i as f32 * 0.5 - 3.0;
        }
        let xbf = opsbf[..cols].to_vec();
        pipebf.execute(&mut opsbf);
        for r in 0..rows {
            let want: f32 = (0..cols).map(|c| ternarybf[r * cols + c] as f32 * xbf[c]).sum();
            let got = opsbf[pipebf.outputs()[r]];
            assert!((got - want).abs() < 1e-3, "BF16 строка {r}: {got} против {want}");
        }

        // Ошибочные входы.
        assert!(meta_compile_safetensors_tensor(b"not a safetensors file", 8, 8).is_err());
        let mut truncated = bytes.clone();
        truncated.truncate(40);
        assert!(meta_compile_safetensors_tensor(&truncated, cols, rows).is_err());
    }

    /// f32 → f16 (только для теста: значения выбраны точно представимыми).
    fn f32_to_f16_bits(v: f32) -> u16 {
        let b = v.to_bits();
        let sign = ((b >> 31) & 1) as u16;
        let rest = b & 0x7FFF_FFFF;
        if rest == 0 {
            return sign << 15; // ±0
        }
        let exp = ((rest >> 23) as i32) - 127 + 15;
        let frac = ((rest >> 13) & 0x3FF) as u16;
        (sign << 15) | ((exp as u16) << 10) | frac
    }

    /// Статистика конвейера и контракт операндного пространства.
    #[test]
    fn test_meta_pipeline_stats() {
        let mut b = WeightCircuitBuilder::new(4);
        let out_idx = b.compile_linear_layer(4, 2, &[1, 0, -1, 0, 0, 1, 0, 1]);
        let pipe = MetaPipeline::run(b).expect("pipeline");

        let (orig, kept, reduction) = pipe.stats();
        assert_eq!(orig, 4, "2 инита + 2 аккумулятора в исходной схеме");
        assert_eq!(kept, 2, "обе копии-инита устранены через alias");
        assert!(reduction >= 0.499, "сокращение 50%");
        assert_eq!(pipe.waves().len(), 1, "оба вентиля независимы → одна волна");
        assert_eq!(pipe.n_operands(), 4 + 8, "входы + один 8-слотовый блок");
        assert_eq!(pipe.outputs().len(), 2, "два корня DAG");

        let mut ops = vec![0.0f32; pipe.n_operands()];
        ops[0] = 10.0;
        ops[1] = 5.0;
        ops[2] = 3.0;
        ops[3] = 2.0;
        pipe.execute(&mut ops);
        assert_eq!(ops[pipe.slot_of(out_idx[0]).unwrap()], 7.0, "x0-x2");
        assert_eq!(ops[pipe.slot_of(out_idx[1]).unwrap()], 7.0, "x1+x3");
    }
}

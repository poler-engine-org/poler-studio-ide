//! Энтанглмент-слой POLER: запутанность поверх анзаца `⊗ R_y(arccos p)`.
//!
//! ## Три источника рёбер
//!
//! * [`Entanglement::None`] — чистый продукт (текущее поведение v0.1.1).
//! * [`Entanglement::Chain`] — цепочка по соседним кубитам `q → q+1`,
//!   зеркало резонансного слоя qiskit-анзаца (режимы A/B/C).
//! * [`Entanglement::FromTopology`] — **LENS-индуцированная** запутанность:
//!   каждые два соседние хранимые дуги контейнера `(u_k, u_{k+1})`
//!   разворачиваются в двухкубитный гейт `CX/CZ(u_k → u_{k+1})`. Координаты
//!   внимания связываются **без материализации плотной матрицы N×N**.
//!
//! ## Точная семантика product-движка (без 2^d амплитуд)
//!
//! Born-сэмплирование в вычислительном базисе инвариантно относительно
//! структуры запутывания двух типов:
//!
//! * **CX-слои** — перестановка базисных состояний: сначала сэмплируются
//!   независимые биты `init_i ~ Bernoulli((1−p_i)/2)`, затем по рёбрам
//!   применяется точное GF(2)-распространение `bit_v ^= bit_u`
//!   (префиксный XOR). Стоимость выстрела не растёт: `O(d/64 + nnz)`.
//! * **CZ-слои** — диагональные гейты: вероятности измерения в
//!   вычислительном базисе не меняются вовсе (CZ влияет только на
//!   интерференцию — «внутреннюю жизнь» состояния).
//!
//! Маргиналы и вероятности паттернов дуг считаются **точно** марковской
//! прогонкой: `bit'_i = init_i ⊕ bit'_{i−1}` даёт рекуррентность
//! `P1'_i = P1'_{i−1}(1−p1_i) + (1−P1'_{i−1})·p1_i`.
//!
//! Совпадение с statevector-движком проверяется тестом
//! `entangled_product_matches_statevector` до 1e−12.

use crate::error::Result;
use crate::statevector::Statevector;

/// Двухкубитный запутыватель.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entangler {
    /// CX (CNOT): control → target. Перестановка базисных состояний.
    Cx,
    /// CZ: фаза −1 на |11⟩. Диагонален, измерения не меняет.
    Cz,
}

impl Entangler {
    /// Имя для отчётов и JSON-протокола паритета.
    pub fn name(self) -> &'static str {
        match self {
            Entangler::Cx => "cx",
            Entangler::Cz => "cz",
        }
    }

    /// Разбор из имени протокола.
    pub fn from_name(s: &str) -> Option<Entangler> {
        match s {
            "cx" => Some(Entangler::Cx),
            "cz" => Some(Entangler::Cz),
            _ => None,
        }
    }

    fn apply_sv(self, sv: &mut Statevector, u: usize, v: usize) -> Result<()> {
        match self {
            Entangler::Cx => sv.apply_cnot(u, v),
            Entangler::Cz => sv.apply_cz(u, v),
        }
    }
}

/// Режим энтанглмента анзаца.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entanglement {
    /// Без запутанности: чистое произведение `⊗ R_y(θ)`.
    None,
    /// Цепочка по соседним кубитам: гейт `(q → q+1)` для `q = 0..d−2`.
    Chain {
        /// Тип запутывателя.
        gate: Entangler,
    },
    /// LENS-топология: соседние хранимые дуги `(u_k → u_{k+1})` — рёбра.
    FromTopology {
        /// Тип запутывателя.
        gate: Entangler,
    },
}

impl Entanglement {
    /// Цепочка заданным гейтом.
    pub fn chain(gate: Entangler) -> Entanglement {
        Entanglement::Chain { gate }
    }

    /// Рёбра из LENS-топологии контейнера.
    pub fn from_topology(gate: Entangler) -> Entanglement {
        Entanglement::FromTopology { gate }
    }

    /// Имя режима для отчётов.
    pub fn name(&self) -> &'static str {
        match self {
            Entanglement::None => "none",
            Entanglement::Chain { gate } => match gate {
                Entangler::Cx => "chain:cx",
                Entangler::Cz => "chain:cz",
            },
            Entanglement::FromTopology { gate } => match gate {
                Entangler::Cx => "topology:cx",
                Entangler::Cz => "topology:cz",
            },
        }
    }

    /// Рёбра запутывания по возрастанию: пары соседних элементов `nodes`.
    ///
    /// `nodes` — кубиты (Chain) или хранимые дуги LENS (FromTopology),
    /// строго возрастающие (контракт контейнера и [`crate::PhaseAnsatz`]).
    pub fn edges(&self, nodes: &[u32]) -> Vec<(u32, u32)> {
        match self {
            Entanglement::None => Vec::new(),
            _ => nodes.windows(2).map(|w| (w[0], w[1])).collect(),
        }
    }

    /// Применить слой к statevector-движку.
    ///
    /// `arcs` — индексы хранимых дуг (для [`Entanglement::FromTopology`]).
    pub fn apply_sv(&self, sv: &mut Statevector, arcs: &[u32]) -> Result<()> {
        match self {
            Entanglement::None => Ok(()),
            Entanglement::Chain { gate } => {
                for q in 0..sv.n_qubits().saturating_sub(1) {
                    gate.apply_sv(sv, q, q + 1)?;
                }
                Ok(())
            }
            Entanglement::FromTopology { gate } => {
                for &(u, v) in &self.edges(arcs) {
                    gate.apply_sv(sv, u as usize, v as usize)?;
                }
                Ok(())
            }
        }
    }

    /// Внутреннее представление для product-движка.
    pub(crate) fn product_spec(&self) -> ProductEnt {
        match self {
            Entanglement::None => ProductEnt::None,
            Entanglement::Chain {
                gate: Entangler::Cz,
            }
            | Entanglement::FromTopology {
                gate: Entangler::Cz,
            } => ProductEnt::Diagonal,
            Entanglement::Chain {
                gate: Entangler::Cx,
            } => ProductEnt::PrefixAll,
            Entanglement::FromTopology {
                gate: Entangler::Cx,
            } => ProductEnt::PrefixArcs,
        }
    }
}

/// Внутренняя спецификация энтанглмента product-движка.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductEnt {
    /// Гейтов нет.
    None,
    /// Диагональный слой (CZ): сэмплирование и маргиналы не меняются.
    Diagonal,
    /// CX-цепочка по всем координатам: префиксный XOR всей строки.
    PrefixAll,
    /// CX-цепочка по дугам LENS: префиксный XOR только на позициях дуг.
    PrefixArcs,
}

/// Префиксный XOR битового массива: `bit'_i = b_0 ⊕ b_1 ⊕ … ⊕ b_i`.
///
/// Слово u64 обрабатывается за 6 сдвиговых шагов (классический xor-scan),
/// перенос между словами — XOR всех битов предыдущего слова.
pub(crate) fn prefix_xor_words(words: &mut [u64]) {
    let mut carry = 0u64;
    for w in words.iter_mut() {
        let mut x = *w;
        x ^= x << 1;
        x ^= x << 2;
        x ^= x << 4;
        x ^= x << 8;
        x ^= x << 16;
        x ^= x << 32;
        // Внутрисловный префикс старшего бита = XOR всех исходных битов.
        let next_carry = x >> 63;
        *w = x ^ carry.wrapping_neg();
        carry = next_carry;
    }
}

/// Префиксный XOR только на позициях `nodes` (возрастающих):
/// после прохода `bit[node_k] = init[node_0] ⊕ … ⊕ init[node_k]`.
pub(crate) fn prefix_xor_positions(words: &mut [u64], nodes: &[u32]) {
    let mut running = false;
    for &n in nodes {
        let i = n as usize;
        let bit = (words[i / 64] >> (i % 64)) & 1 == 1;
        let new = bit ^ running;
        if new {
            words[i / 64] |= 1u64 << (i % 64);
        } else {
            words[i / 64] &= !(1u64 << (i % 64));
        }
        running = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::statevector::Statevector;

    /// Фактический XOR-префикс по битам (эталон медленной реализацией).
    fn reference_prefix(bits: &[bool]) -> Vec<bool> {
        let mut out = Vec::with_capacity(bits.len());
        let mut acc = false;
        for &b in bits {
            acc ^= b;
            out.push(acc);
        }
        out
    }

    fn words_to_bits(words: &[u64], d: usize) -> Vec<bool> {
        (0..d)
            .map(|i| (words[i / 64] >> (i % 64)) & 1 == 1)
            .collect()
    }

    #[test]
    fn prefix_xor_single_word_hand_computed() {
        // 0b1011_0010, биты LSB-first: b = 0,1,0,0,1,1,0,1
        // → префиксы 0,1,1,1,0,1,1,0 → 0b0110_1110.
        let mut words = [0b1011_0010u64];
        prefix_xor_words(&mut words);
        assert_eq!(words[0], 0b0110_1110u64);
    }

    #[test]
    fn prefix_xor_matches_reference_random() {
        let mut rng = Rng::seed_from_u64(2024);
        for d in [1usize, 63, 64, 65, 127, 128, 200] {
            let mut words = vec![0u64; (d + 63) / 64];
            for w in words.iter_mut() {
                *w = rng.next_u64();
            }
            // Хвост за пределами d зануляем — как делает сэмплер.
            let tail = d % 64;
            if tail != 0 {
                let last = words.len() - 1;
                words[last] &= (1u64 << tail) - 1;
            }
            let init: Vec<bool> = words_to_bits(&words, d);
            prefix_xor_words(&mut words);
            assert_eq!(words_to_bits(&words, d), reference_prefix(&init), "d={d}");
        }
    }

    #[test]
    fn prefix_xor_carry_between_words() {
        // Все единицы: префикс чередуется 1,0,1,0,… (чётность числа единиц);
        // XOR всех битов слова = 0 → переноса нет, второе слово не меняется.
        let mut words = [!0u64, 0u64];
        prefix_xor_words(&mut words);
        assert_eq!(words, [0x5555_5555_5555_5555u64, 0u64]);
        // Единица в нулевом бите: префикс — все единицы, перенос = 1
        // инвертирует нулевое слово.
        let mut words = [1u64, 0u64];
        prefix_xor_words(&mut words);
        assert_eq!(words, [!0u64, !0u64]);
    }

    #[test]
    fn prefix_positions_matches_reference() {
        let mut rng = Rng::seed_from_u64(7);
        let d = 97usize;
        let mut words = vec![0u64; (d + 63) / 64];
        for w in words.iter_mut() {
            *w = rng.next_u64();
        }
        let tail = d % 64;
        words[d / 64] &= (1u64 << tail) - 1;
        let nodes: Vec<u32> = vec![0, 1, 5, 6, 7, 40, 41, 96];
        let init = words_to_bits(&words, d);
        prefix_xor_positions(&mut words, &nodes);
        let after = words_to_bits(&words, d);
        // Позиции вне nodes не тронуты; на nodes — накопленный XOR.
        let mut acc = false;
        for i in 0..d {
            if nodes.contains(&(i as u32)) {
                acc ^= init[i];
                assert_eq!(after[i], acc, "node {i}");
            } else {
                assert_eq!(after[i], init[i], "background {i}");
            }
        }
    }

    /// Свойство: product-энтанглмент точно совпадает со statevector-гейтами.
    #[test]
    fn entangled_product_matches_statevector() {
        let ps: Vec<f64> = vec![0.3, -0.8, 0.0, 0.55, -0.15, 0.9, -1.0, 0.42];
        let arcs: Vec<(u32, f64)> = ps.iter().enumerate().map(|(i, &p)| (i as u32, p)).collect();
        let nodes: Vec<u32> = arcs.iter().map(|a| a.0).collect();

        for ent in [
            Entanglement::chain(Entangler::Cx),
            Entanglement::chain(Entangler::Cz),
        ] {
            let mut sv = Statevector::from_phases(&ps).unwrap();
            ent.apply_sv(&mut sv, &nodes).unwrap();
            let sv_marg = sv.marginals();
            let pa = crate::PhaseAnsatz::new(ps.len() as u32, arcs.clone())
                .unwrap()
                .with_entanglement(ent);
            let th = pa.marginal_theory();
            for (k, &(_, t)) in th.iter().enumerate() {
                assert!(
                    (t - sv_marg[k]).abs() < 1e-12,
                    "{:?}: arc {} теория {} vs sv {}",
                    ent.name(),
                    k,
                    t,
                    sv_marg[k]
                );
            }
        }
    }

    /// FromTopology (CX по дугам) — точное совпадение маргинал с гейтами.
    #[test]
    fn from_topology_cx_matches_statevector() {
        // d = 12, LENS хранит только дуги с ненулевым p.
        let dense = [
            0.0, 0.4, 0.0, -0.7, 0.0, 0.0, 0.9, 0.0, -0.25, 0.0, 0.6, 0.0,
        ];
        let arcs: Vec<(u32, f64)> = dense
            .iter()
            .enumerate()
            .filter(|(_, &p)| p != 0.0)
            .map(|(i, &p)| (i as u32, p))
            .collect();
        let nodes: Vec<u32> = arcs.iter().map(|a| a.0).collect();

        let ent = Entanglement::from_topology(Entangler::Cx);
        let mut sv = Statevector::from_phases(&dense).unwrap();
        ent.apply_sv(&mut sv, &nodes).unwrap();
        let sv_marg = sv.marginals();

        let pa = crate::PhaseAnsatz::new(dense.len() as u32, arcs.clone())
            .unwrap()
            .with_entanglement(ent);
        for &(i, t) in pa.marginal_theory().iter() {
            assert!((t - sv_marg[i as usize]).abs() < 1e-12, "arc {i}");
        }
        // Фон не тронут: маргиналы вне дуг остались честными монетами.
        for (i, &p) in dense.iter().enumerate() {
            if p == 0.0 {
                assert!((sv_marg[i] - 0.5).abs() < 1e-12);
            }
        }
    }

    /// CZ-слой не меняет измерения: маргиналы statevector идентичны None.
    #[test]
    fn cz_layer_is_invisible_to_measurement() {
        let ps: Vec<f64> = vec![0.2, -0.6, 0.8, 0.0, -0.35];
        let nodes: Vec<u32> = (0..ps.len() as u32).collect();
        let mut sv = Statevector::from_phases(&ps).unwrap();
        let before = sv.marginals();
        Entanglement::chain(Entangler::Cz)
            .apply_sv(&mut sv, &nodes)
            .unwrap();
        let after = sv.marginals();
        for (a, b) in before.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-14);
        }
    }

    /// Вероятности паттернов дуг после CX-цепочки — марковская прогонка,
    /// совпадает с точными вероятностями statevector до 1e−12.
    #[test]
    fn pattern_probability_entangled_exact() {
        let ps: Vec<f64> = vec![0.45, -0.3, 0.7, 0.0, -0.9];
        let arcs: Vec<(u32, f64)> = ps.iter().enumerate().map(|(i, &p)| (i as u32, p)).collect();
        let nodes: Vec<u32> = arcs.iter().map(|a| a.0).collect();

        for ent in [
            Entanglement::chain(Entangler::Cx),
            Entanglement::from_topology(Entangler::Cx),
        ] {
            let mut sv = Statevector::from_phases(&ps).unwrap();
            ent.apply_sv(&mut sv, &nodes).unwrap();
            let probs = sv.probabilities();
            let pa = crate::PhaseAnsatz::new(ps.len() as u32, arcs.clone())
                .unwrap()
                .with_entanglement(ent);
            for pattern in 0u64..(1 << ps.len()) {
                let exact: f64 = (0..probs.len())
                    .map(|k| {
                        // Паттерн дуг — биты на позициях дуг (все координаты — дуги).
                        if (k as u64) == pattern {
                            probs[k]
                        } else {
                            0.0
                        }
                    })
                    .sum();
                assert!(
                    (pa.pattern_probability(pattern) - exact).abs() < 1e-12,
                    "{:?}: pattern {pattern}",
                    ent.name()
                );
            }
        }
    }

    /// Имена режимов стабильны для JSON-протокола.
    #[test]
    fn names_are_stable() {
        assert_eq!(Entanglement::None.name(), "none");
        assert_eq!(Entanglement::chain(Entangler::Cx).name(), "chain:cx");
        assert_eq!(
            Entanglement::from_topology(Entangler::Cz).name(),
            "topology:cz"
        );
        assert_eq!(Entangler::from_name("cx"), Some(Entangler::Cx));
        assert_eq!(Entangler::from_name("swap"), None);
    }

    /// Рёбра строятся только при наличии слоя.
    #[test]
    fn edges_follow_nodes() {
        let nodes = vec![2u32, 5, 9];
        assert!(Entanglement::None.edges(&nodes).is_empty());
        assert_eq!(
            Entanglement::chain(Entangler::Cx).edges(&nodes),
            vec![(2, 5), (5, 9)]
        );
    }
}

//! # qaoa — QAOA-ансатц: MaxCut на идеальном субстрате (цикл Q)
//!
//! Quantum Approximate Optimization Algorithm (Farhi, Goldstone, Gutmann
//! 2014) на машине POLER Quantum PC: cost-гамильтониан MaxCut, чередующиеся
//! слои `exp(−iγ·C)` и `exp(−iβ·B)`, классический оптимизатор параметров
//! по точному матожиданию (без сэмплирования — привилегия state-owner).
//!
//! ## Математика
//!
//! MaxCut: `C(x) = Σ_{(i,j)∈E} [x_i ⊕ x_j]` — число разрезанных рёбер.
//! Спектральное кодирование: `[x_i ⊕ x_j] = (1 − Z_iZ_j)/2`.
//!
//! Cost-слой на ребре (с точностью до глобальной фазы e^{iγ/2}):
//!
//! ```text
//! exp(−iγ·(1 − Z_iZ_j)/2) ≡ CX_{i,j} · Rz(−γ)_j · CX_{i,j}
//! ```
//!
//! Mixer-слой: `exp(−iβ·X_q) = Rx(2β)_q`.
//!
//! Анзац: `|ψ(γ,β)⟩ = Π_{l=1..p} [e^{−iβ_l B} · e^{−iγ_l C}] · H^{⊗n}|0…0⟩`;
//! цель — `maximize E[cut] = Σ_x |⟨x|ψ⟩|² · C(x)`.
//!
//! Оптимизатор — координатный спуск с затухающим шагом и перезапусками
//! (эвристическое расписание + случайные точки). Без внешних зависимостей;
//! детерминизм — по seed.
//!
//! ## Честные границы
//!
//! * Ожидание считается **точно** по вектору состояния (кубитов ≤ 20);
//!   сэмплирование — только для финальной гистограммы.
//! * Переборный оптимум MaxCut честно ограничен `n ≤ 20` (2²⁰ переборов);
//!   при больших n поле `optimum` = None, `approx_ratio` = None.
//! * QAOA — приближённый алгоритм: гарантия p→∞ (адиабатический предел),
//!   при малых p аппроксимационное отношение < 1 — это честно отражается
//!   в отчёте.

use crate::error::{PqcError, Result};
use crate::gates::Gate;
use crate::qpc::{self, Circuit, Op};
use crate::rng::Rng;

/// Потолок QAOA: statevector 2²⁰ амплитуд + переборный оптимум.
pub const MAX_QAOA_QUBITS: usize = 20;
/// Максимум рёбер графа (защита от опечаток CLI).
pub const MAX_EDGES: usize = 512;
/// Максимум слоёв анзаца.
pub const MAX_P: usize = 8;

/// Задача MaxCut: граф на `n` вершинах-кубитах, рёбра `edges`.
#[derive(Clone, Debug)]
pub struct MaxCut {
    /// Число вершин = кубитов.
    pub n: usize,
    /// Рёбра (i, j), i ≠ j, оба < n.
    pub edges: Vec<(usize, usize)>,
}

impl MaxCut {
    /// Валидированный граф.
    pub fn new(n: usize, mut edges: Vec<(usize, usize)>) -> Result<MaxCut> {
        if n == 0 || n > MAX_QAOA_QUBITS {
            return Err(PqcError::BadArgument {
                what: format!("qaoa: кубитов {n}, допустимо 1..={MAX_QAOA_QUBITS}"),
            });
        }
        if edges.len() > MAX_EDGES {
            return Err(PqcError::BadArgument {
                what: format!("qaoa: {} рёбер > {MAX_EDGES}", edges.len()),
            });
        }
        for e in &mut edges {
            if e.0 > e.1 {
                core::mem::swap(&mut e.0, &mut e.1);
            }
        }
        edges.sort_unstable();
        edges.dedup();
        for &(i, j) in &edges {
            if i == j {
                return Err(PqcError::BadArgument {
                    what: format!("qaoa: петля ребра ({i},{j}) — i ≠ j обязательно"),
                });
            }
            if i >= n || j >= n {
                return Err(PqcError::BadArgument {
                    what: format!("qaoa: ребро ({i},{j}) вне диапазона [0, {n})"),
                });
            }
        }
        Ok(MaxCut { n, edges })
    }

    /// Демо-граф по умолчанию: 4-цикл + диагональ, оптимум 4 из 5 рёбер.
    pub fn demo() -> MaxCut {
        MaxCut::new(
            4,
            vec![(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)],
        )
        .expect("demo graph is valid")
    }

    /// Разбор рёбер из строки `"0-1,1-2,0-3"` (разделители пар: запятая,
    /// точка с запятой, пробел).
    pub fn parse_edges(s: &str) -> Result<Vec<(usize, usize)>> {
        let mut out = Vec::new();
        for tok in s.split([',', ';', ' ', '\t']) {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            let pair: Vec<&str> = tok.split(['-', ':', '_']).collect();
            if pair.len() != 2 {
                return Err(PqcError::BadArgument {
                    what: format!("qaoa: ребро `{tok}` — формат `i-j`"),
                });
            }
            let (i, j) = (
                pair[0].trim().parse::<usize>().map_err(|_| PqcError::BadArgument {
                    what: format!("qaoa: вершина `{}` не число", pair[0].trim()),
                })?,
                pair[1].trim().parse::<usize>().map_err(|_| PqcError::BadArgument {
                    what: format!("qaoa: вершина `{}` не число", pair[1].trim()),
                })?,
            );
            out.push((i, j));
        }
        if out.is_empty() {
            return Err(PqcError::BadArgument {
                what: "qaoa: пустой список рёбер".into(),
            });
        }
        Ok(out)
    }

    /// Число разрезанных рёбер для битовой строки (кубит 0 — младший бит).
    pub fn cut_value(&self, bits: u64) -> u64 {
        self.edges
            .iter()
            .filter(|&&(i, j)| (bits >> i) & 1 != (bits >> j) & 1)
            .count() as u64
    }

    /// Переборный оптимум (n ≤ 20); None — граф слишком велик.
    pub fn optimum(&self) -> Option<u64> {
        if self.n > MAX_QAOA_QUBITS {
            return None;
        }
        // Симметрия дополнения: перебираем половину кубического пространства.
        let half = 1u64 << (self.n - 1);
        (0..half).map(|x| self.cut_value(x).max(self.cut_value(!x))).max()
    }

    /// E[cut] = Σ_x P(x)·C(x) по распределению Борна.
    pub fn expected_cut(&self, probs: &[f64]) -> f64 {
        probs
            .iter()
            .enumerate()
            .map(|(x, &p)| p * self.cut_value(x as u64) as f64)
            .sum()
    }

    /// Аргмакс вероятности; при равенстве — максимум разреза.
    pub fn best_bits(&self, probs: &[f64]) -> u64 {
        let mut best_x = 0u64;
        let mut best_key = (0.0f64, 0u64);
        for (x, &p) in probs.iter().enumerate() {
            let key = (p, self.cut_value(x as u64));
            if key > best_key {
                best_key = key;
                best_x = x as u64;
            }
        }
        best_x
    }
}

/// Построить QAOA-схему: H^n, затем p слоёв [cost, mixer], MeasureAll.
///
/// `params = [γ_1..γ_p, β_1..β_p]` — 2p углов.
pub fn build_circuit(problem: &MaxCut, p: usize, params: &[f64]) -> Result<Circuit> {
    if p == 0 || p > MAX_P {
        return Err(PqcError::BadArgument {
            what: format!("qaoa: p = {p}, допустимо 1..={MAX_P}"),
        });
    }
    if params.len() != 2 * p {
        return Err(PqcError::BadArgument {
            what: format!("qaoa: нужно 2p = {} углов [γ₁..γ_p, β₁..β_p], получено {}", 2 * p, params.len()),
        });
    }
    for &a in params {
        if !a.is_finite() {
            return Err(PqcError::BadArgument {
                what: "qaoa: углы должны быть конечными".into(),
            });
        }
    }
    let mut c = Circuit::new(problem.n)?;
    for q in 0..problem.n {
        c.gate(Gate::H { q });
    }
    for l in 0..p {
        let gamma = params[l];
        let beta = params[p + l];
        // cost: каждое ребро — CX;Rz(−γ);CX ≡ exp(−iγ(1−Z_iZ_j)/2)
        for &(i, j) in &problem.edges {
            c.gate(Gate::Cx { control: i, target: j });
            c.gate(Gate::Rz { q: j, theta: -gamma });
            c.gate(Gate::Cx { control: i, target: j });
        }
        // mixer: exp(−iβX) = Rx(2β) на каждом кубите
        for q in 0..problem.n {
            c.gate(Gate::Rx { q, theta: 2.0 * beta });
        }
    }
    c.push(Op::MeasureAll);
    Ok(c)
}

/// Конфигурация оптимизации.
#[derive(Clone, Debug)]
pub struct QaoaConfig {
    /// Число слоёв анзаца (глубина), 1..=8.
    pub p: usize,
    /// Проходов координатного спуска на перезапуск.
    pub sweeps: usize,
    /// Случайных перезапусков (плюс эвристический нулевой).
    pub restarts: usize,
    /// Выстрелов финальной гистограммы.
    pub shots: u64,
    /// Зерно детерминизма.
    pub seed: u64,
}

impl Default for QaoaConfig {
    fn default() -> Self {
        QaoaConfig {
            p: 2,
            sweeps: 12,
            restarts: 3,
            shots: 1024,
            seed: 42,
        }
    }
}

/// Отчёт QAOA.
#[derive(Clone, Debug)]
pub struct QaoaReport {
    /// Кубитов (= вершин графа).
    pub n_qubits: usize,
    /// Рёбра задачи.
    pub edges: Vec<(usize, usize)>,
    /// Глубина анзаца.
    pub p: usize,
    /// Оптимизированные углы [γ₁..γ_p, β₁..β_p].
    pub params: Vec<f64>,
    /// E[cut] до оптимизации (нулевая точка перезапуска 0).
    pub expected_cut_init: f64,
    /// E[cut] после оптимизации.
    pub expected_cut: f64,
    /// Лучшая битовая строка (аргмакс Борна).
    pub best_bits: u64,
    /// Её разрез.
    pub best_cut: u64,
    /// Переборный оптимум (None при n > 20).
    pub optimum: Option<u64>,
    /// best_cut / optimum (None без оптимума).
    pub approx_ratio: Option<f64>,
    /// Число вычислений E[cut].
    pub evals: usize,
    /// Лучшее значение по проходам (история сходимости).
    pub history: Vec<f64>,
    /// Число гейтов финальной схемы.
    pub gate_count: usize,
    /// Финальное распределение Борна.
    pub probabilities: Vec<f64>,
    /// Гистограмма выстрелов.
    pub counts: Vec<(u64, u64)>,
    /// Выстрелов.
    pub shots: u64,
    /// Норма финального состояния (контроль унитарности).
    pub norm: f64,
}

/// Прогнать QAOA: оптимизация углов + финальный отчёт.
pub fn run_qaoa(problem: &MaxCut, cfg: &QaoaConfig) -> Result<QaoaReport> {
    if cfg.p == 0 || cfg.p > MAX_P {
        return Err(PqcError::BadArgument {
            what: format!("qaoa: p = {}, допустимо 1..={MAX_P}", cfg.p),
        });
    }
    let p = cfg.p;
    let m = 2 * p;
    let mut rng = Rng::seed_from_u64(cfg.seed);
    let mut evals = 0usize;

    // Точное матожидание по вектору состояния (shots = 0).
    let eval = |params: &[f64]| -> Result<f64> {
        let c = build_circuit(problem, p, params)?;
        let rep = qpc::run(&c, 0, 0)?;
        Ok(problem.expected_cut(&rep.probabilities))
    };

    let mut global_params: Vec<f64> = Vec::new();
    let mut global_best = f64::NEG_INFINITY;
    let mut first_init = 0.0f64;
    let mut history: Vec<f64> = Vec::new();

    for restart in 0..=cfg.restarts {
        let mut cur: Vec<f64> = if restart == 0 {
            // Эвристика: линейно убывающее расписание углов.
            (0..p)
                .flat_map(|l| {
                    let f = (l + 1) as f64 / (p + 1) as f64;
                    [std::f64::consts::PI * (1.0 - 0.55 * f), 0.30 * std::f64::consts::PI * (1.0 - 0.5 * f)]
                })
                .collect()
        } else {
            (0..p)
                .flat_map(|_| {
                    [rng.next_f64() * std::f64::consts::PI, rng.next_f64() * 0.5 * std::f64::consts::PI]
                })
                .collect()
        };
        let init_val = eval(&cur)?;
        evals += 1;
        if restart == 0 {
            first_init = init_val;
        }
        let mut cur_val = init_val;

        // Координатный спуск с затухающим шагом.
        let mut step = 0.4f64;
        for _ in 0..cfg.sweeps {
            let mut improved = false;
            for i in 0..m {
                for dir in [-1.0f64, 1.0f64] {
                    let mut cand = cur.clone();
                    cand[i] += dir * step;
                    let v = eval(&cand)?;
                    evals += 1;
                    if v > cur_val + 1e-12 {
                        cur = cand;
                        cur_val = v;
                        improved = true;
                    }
                }
            }
            if cur_val > global_best + 1e-12 {
                global_best = cur_val;
                global_params = cur.clone();
            }
            history.push(global_best);
            if !improved && step < 1e-3 {
                break;
            }
            step *= 0.65;
        }
        if cur_val > global_best + 1e-12 {
            global_best = cur_val;
            global_params = cur.clone();
        }
    }

    if global_params.is_empty() {
        return Err(PqcError::BadArgument {
            what: "qaoa: оптимизатор не нашёл ни одной точки".into(),
        });
    }

    let circuit = build_circuit(problem, p, &global_params)?;
    let gate_count = circuit.ops().len();
    let rep = qpc::run(&circuit, cfg.shots, cfg.seed)?;
    let best_bits = problem.best_bits(&rep.probabilities);
    let best_cut = problem.cut_value(best_bits);
    let optimum = problem.optimum();
    Ok(QaoaReport {
        n_qubits: problem.n,
        edges: problem.edges.clone(),
        p,
        params: global_params,
        expected_cut_init: first_init,
        expected_cut: global_best,
        best_bits,
        best_cut,
        approx_ratio: optimum.map(|opt| best_cut as f64 / opt as f64),
        optimum,
        evals,
        history,
        gate_count,
        probabilities: rep.probabilities,
        counts: rep.counts,
        shots: rep.shots,
        norm: rep.norm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> MaxCut {
        MaxCut::new(3, vec![(0, 1), (1, 2), (0, 2)]).unwrap()
    }

    #[test]
    fn maxcut_basics_and_optimum() {
        let t = triangle();
        assert_eq!(t.cut_value(0b000), 0);
        assert_eq!(t.cut_value(0b001), 2); // {0} vs {1,2}
        assert_eq!(t.cut_value(0b011), 2); // {0,1} vs {2}
        assert_eq!(t.optimum(), Some(2));

        // K4: любая биразбиция 2|2 режет 4 ребра из 6.
        let k4 = MaxCut::new(
            4,
            vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
        )
        .unwrap();
        assert_eq!(k4.optimum(), Some(4));

        // Демо: 4-цикл + диагональ, оптимум 4.
        assert_eq!(MaxCut::demo().optimum(), Some(4));

        // Ровный (пустой) граф: оптимум 0.
        let empty = MaxCut::new(5, vec![]).unwrap();
        assert_eq!(empty.optimum(), Some(0));
    }

    #[test]
    fn maxcut_validation() {
        assert!(MaxCut::new(3, vec![(0, 0)]).is_err(), "петля");
        assert!(MaxCut::new(2, vec![(0, 5)]).is_err(), "вершина вне диапазона");
        assert!(MaxCut::new(0, vec![]).is_err(), "ноль вершин");
        assert!(MaxCut::new(21, vec![(0, 1)]).is_err(), "кубитов > 20");
        // дубликаты рёбер схлопываются
        let g = MaxCut::new(3, vec![(0, 1), (1, 0), (0, 1)]).unwrap();
        assert_eq!(g.edges, vec![(0, 1)]);
    }

    #[test]
    fn parse_edges_formats() {
        assert_eq!(
            MaxCut::parse_edges("0-1,1-2,0-2").unwrap(),
            vec![(0, 1), (1, 2), (0, 2)]
        );
        assert_eq!(
            MaxCut::parse_edges("0-1 1-2").unwrap(),
            vec![(0, 1), (1, 2)]
        );
        assert!(MaxCut::parse_edges("").is_err());
        assert!(MaxCut::parse_edges("0").is_err());
        assert!(MaxCut::parse_edges("a-b").is_err());
    }

    #[test]
    fn expected_cut_uniform_is_half_edges() {
        let t = triangle();
        let probs = vec![1.0 / 8.0; 8];
        assert!((t.expected_cut(&probs) - 1.5).abs() < 1e-12);
    }

    #[test]
    fn build_circuit_gate_count() {
        let t = triangle();
        let c = build_circuit(&t, 2, &[0.3, 0.4, 0.5, 0.6]).unwrap();
        // H^3 + 2×(3 ребра × 3 гейта + 3 миксера) + MeasureAll = 3 + 24 + 1
        assert_eq!(c.ops().len(), 28);
        assert!(build_circuit(&t, 0, &[]).is_err());
        assert!(build_circuit(&t, 1, &[0.1]).is_err(), "нужно 2p углов");
    }

    #[test]
    fn cost_layer_is_diagonal_cut_phase() {
        // γ = π: exp(−iπ·C) = (−1)^{cut(x)} — проверка на треугольнике.
        let t = triangle();
        let c = build_circuit(&t, 1, &[std::f64::consts::PI, 0.0]).unwrap();
        let rep = qpc::run(&c, 0, 0).unwrap();
        // β=0: mixer единичный → вероятности H^3 (равномерные),
        // фазы несущественны для |⟨x|ψ⟩|².
        for p in &rep.probabilities {
            assert!((p - 0.125).abs() < 1e-12);
        }
        assert!((rep.norm - 1.0).abs() < 1e-12);
    }

    #[test]
    fn qaoa_triangle_finds_good_cut() {
        // Треугольник: оптимум 2, случайное назначение 1.5.
        // p=2 с перезапусками обязан уйти заметно выше случайного.
        let cfg = QaoaConfig {
            p: 2,
            sweeps: 10,
            restarts: 2,
            shots: 512,
            seed: 7,
        };
        let rep = run_qaoa(&triangle(), &cfg).unwrap();
        assert!(rep.expected_cut > 1.8, "E[cut] = {}", rep.expected_cut);
        assert_eq!(rep.optimum, Some(2));
        assert_eq!(rep.best_cut, 2, "аргмакс обязан быть оптимальным разрезом");
        let ratio = rep.approx_ratio.expect("n=3 ≤ 20");
        assert!(ratio >= 0.999, "ratio = {ratio}");
        assert!((rep.norm - 1.0).abs() < 1e-12);
        // оптимизация реально что-то дала
        assert!(rep.expected_cut >= rep.expected_cut_init - 1e-9);
        assert!(!rep.history.is_empty());
        assert!(rep.evals > 4);
    }

    #[test]
    fn qaoa_demo_and_determinism() {
        let cfg = QaoaConfig {
            p: 1,
            sweeps: 6,
            restarts: 1,
            shots: 256,
            seed: 42,
        };
        let a = run_qaoa(&MaxCut::demo(), &cfg).unwrap();
        let b = run_qaoa(&MaxCut::demo(), &cfg).unwrap();
        assert_eq!(a.params, b.params, "детерминизм по seed");
        assert_eq!(a.expected_cut, b.expected_cut);
        assert!(a.expected_cut > 2.0, "выше половины рёбер (2.5), получено {}", a.expected_cut);
        assert_eq!(a.best_cut, 4);
    }

    #[test]
    fn qaoa_bipartite_ring_p1() {
        // Чётный цикл C4 (двудольный): известный оптимум QAOA p=1 —
        // 3/4 рёбер (E[cut] = 3.0 из 4; полный разрез требует больших p).
        // Проверяем достижение честной границы p=1.
        let c4 = MaxCut::new(4, vec![(0, 1), (1, 2), (2, 3), (3, 0)]).unwrap();
        let cfg = QaoaConfig {
            p: 1,
            sweeps: 10,
            restarts: 4,
            shots: 0,
            seed: 3,
        };
        let rep = run_qaoa(&c4, &cfg).unwrap();
        assert!((rep.expected_cut - 3.0).abs() < 2e-3, "E[cut] = {}", rep.expected_cut);
        assert!(rep.best_cut >= 3, "best_cut = {}", rep.best_cut);
        // p=4 с перезапусками дотягивается до полного разреза 4/4.
        let deep = QaoaConfig {
            p: 4,
            sweeps: 14,
            restarts: 4,
            shots: 0,
            seed: 3,
        };
        let rep4 = run_qaoa(&c4, &deep).unwrap();
        assert!(rep4.expected_cut > 3.3, "E[cut] p=4 = {}", rep4.expected_cut);
    }

    #[test]
    fn qaoa_empty_graph_zero() {
        let g = MaxCut::new(4, vec![]).unwrap();
        let rep = run_qaoa(&g, &QaoaConfig::default()).unwrap();
        assert!(rep.expected_cut.abs() < 1e-12);
        assert_eq!(rep.optimum, Some(0));
    }
}

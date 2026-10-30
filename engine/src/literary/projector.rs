//! L Логика: проектор причинности Π_Λ = I − J_cᵀ(J_cJ_cᵀ)⁻¹J_c.
//!
//! Ортогональная проекция на нуль-пространство матрицы логических
//! ограничений J_c: любая компонента обновления, нарушающая закон
//! причинности (например, p₀ − p₁ = 0 — «событие не может изменить
//! собственную причину»), математически аннигилируется до нуля.
//! Галлюцинации и противоречия не «фильтруются постфактум» — у них
//! нет направления в пространстве, в котором живёт траектория.
//!
//! Свойства (проверены тестами):
//! - **идемпотентность** Π(Πv) = Πv — повторная проекция ничего не меняет;
//! - **самосопряжённость** ⟨Πv, w⟩ = ⟨v, Πw⟩ — проекция ортогональна;
//! - **убивание нарушений** J_c·Πv = 0 с машинной точностью;
//! - вектор из null(J_c) проходит насквозь без искажений.
//!
//! Ограничения задаются тройками (i, j, c): строка J_c = c·(e_i − e_j),
//! т.е. требование c·(p_i − p_j) = 0. По умолчанию движок накладывает
//! закон p₀ − p₁ = 0 (связь первых двух осей — «причинный замок»);
//! агент может задать свой набор (например, эквивалентность финальных
//! осей: развязка не противоречит завязке).

use super::linalg::{dot, gauss_jordan_inverse, norm2, Mat};

/// Проектор причинности: Π_Λ = I − J_cᵀ·G⁻¹·J_c, G = J_cJ_cᵀ.
pub struct CausalProjector {
    /// Число осей фазового пространства.
    pub dims: usize,
    /// Матрица ограничений (rows × dims).
    pub j_c: Mat,
    /// Обращённая грамиана G = J_cJ_cᵀ (rows × rows).
    gram_inv: Mat,
    /// Признак тривиальности (нет ограничений): Π = I, ноль работы.
    trivial: bool,
}

impl CausalProjector {
    /// Построение из ограничений (i, j, c): c·(p_i − p_j) = 0.
    /// Пустой набор — тождественный проектор (Π = I).
    pub fn new(constraints: &[(usize, usize, f32)], dims: usize) -> Result<Self, String> {
        if dims == 0 {
            return Err("проектор: фазовое пространство пусто (dims = 0)".into());
        }
        for &(i, j, _) in constraints {
            if i >= dims || j >= dims {
                return Err(format!(
                    "ограничение ({i}, {j}) вне фазового пространства (dims = {dims})"
                ));
            }
            if i == j {
                return Err(format!("самопетля ограничения ({i}, {i}) бессмысленна"));
            }
        }
        if constraints.is_empty() {
            return Ok(Self {
                dims,
                j_c: Mat::zeros(0, dims),
                gram_inv: Mat::zeros(0, 0),
                trivial: true,
            });
        }
        let rows = constraints.len();
        let mut j_c = Mat::zeros(rows, dims);
        for (r, &(i, j, c)) in constraints.iter().enumerate() {
            j_c.set(r, i, c);
            j_c.set(r, j, -c);
        }
        // Грамиана G = J_c·J_cᵀ (rows × rows, SPD при невырожденных строках).
        let g = j_c.matmul(&j_c.transpose());
        // Обращение: Гаусс—Жордан; вырожденность лечится регуляризацией
        // Тихонова G + εI (ε = 1e-6) — дубликаты ограничений не роняют
        // проектор, а сливаются.
        let gram_inv = match gauss_jordan_inverse(&g) {
            Ok(m) => m,
            Err(e) => {
                let eps = 1e-6f32;
                let mut reg = g.clone();
                for r in 0..rows {
                    let v = reg.at(r, r) + eps;
                    reg.set(r, r, v);
                }
                gauss_jordan_inverse(&reg).map_err(|e2| {
                    format!("проектор: грамиана вырождена ({e}; после регуляризации: {e2})")
                })?
            }
        };
        Ok(Self {
            dims,
            j_c,
            gram_inv,
            trivial: false,
        })
    }

    /// Проекция вектора: v ↦ v − J_cᵀ·G⁻¹·(J_c·v).
    pub fn project(&self, v: &[f32]) -> Vec<f32> {
        debug_assert_eq!(v.len(), self.dims);
        if self.trivial {
            return v.to_vec();
        }
        let jv = self.j_c.matvec(v); // rows
        let lam = self.gram_inv.matvec(&jv); // rows
        // v − J_cᵀ·λ
        let mut out = v.to_vec();
        for r in 0..self.j_c.rows {
            let row = self.j_c.row(r);
            let lr = lam[r];
            for (o, &jc) in out.iter_mut().zip(row) {
                *o -= lr * jc;
            }
        }
        out
    }

    /// Норма нарушения причинности ‖J_c·p‖₂ — сколько «галлюцинации»
    /// в текущем состоянии (0 — чисто).
    pub fn residual(&self, p: &[f32]) -> f32 {
        if self.trivial {
            return 0.0;
        }
        norm2(&self.j_c.matvec(p))
    }

    /// Число независимых ограничений (строки J_c).
    pub fn rank(&self) -> usize {
        self.j_c.rows
    }

    /// Канонический закон движка: p₀ − p₁ = 0.
    pub fn canonical_lock(dims: usize) -> Result<Self, String> {
        if dims < 2 {
            return Err("причинный замок требует dims ≥ 2".into());
        }
        Self::new(&[(0, 1, 1.0)], dims)
    }

    /// Скалярное произведение проекций (диагностика самосопряжённости).
    pub fn gram_energy(&self, v: &[f32], w: &[f32]) -> f32 {
        dot(&self.project(v), &self.project(w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_kills_violations_exactly() {
        let p = CausalProjector::canonical_lock(4).unwrap();
        let v = vec![1.0, 0.0, 0.5, -0.25];
        let pv = p.project(&v);
        // закон p0 − p1 = 0 выполняется с машинной точностью
        assert!((pv[0] - pv[1]).abs() < 1e-5, "pv = {pv:?}");
        // вектор из null(J_c) (p0 == p1) проходит насквозь
        let clean = vec![2.0, 2.0, 7.0, -3.0];
        let pc = p.project(&clean);
        for (a, b) in pc.iter().zip(&clean) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn projection_is_idempotent_and_self_adjoint() {
        let p = CausalProjector::new(&[(0, 2, 1.0), (1, 3, 0.5)], 6).unwrap();
        let v = vec![0.3, -1.2, 0.7, 2.2, -0.4, 0.9];
        let w = vec![-0.8, 0.5, 1.1, -0.3, 0.2, -1.7];
        let pv1 = p.project(&v);
        let pv2 = p.project(&pv1);
        for (a, b) in pv1.iter().zip(&pv2) {
            assert!((a - b).abs() < 1e-4, "идемпотентность: {pv1:?} vs {pv2:?}");
        }
        // самосопряжённость: ⟨Πv, w⟩ == ⟨v, Πw⟩
        let lhs = dot(&pv1, &w);
        let rhs = dot(&v, &p.project(&w));
        assert!((lhs - rhs).abs() < 1e-4, "{lhs} vs {rhs}");
        // проекция не удлиняет
        assert!(norm2(&pv1) <= norm2(&v) + 1e-5);
    }

    #[test]
    fn trivial_duplicate_and_invalid_constraints() {
        // пустой набор — тождество
        let t = CausalProjector::new(&[], 3).unwrap();
        assert_eq!(t.rank(), 0);
        assert_eq!(t.project(&[1.0, 2.0, 3.0]), vec![1.0, 2.0, 3.0]);
        assert_eq!(t.residual(&[1.0, 2.0, 3.0]), 0.0);
        // дубликаты сливаются через регуляризацию, не падают
        let d = CausalProjector::new(&[(0, 1, 1.0), (0, 1, 1.0), (0, 1, 2.0)], 4).unwrap();
        let pd = d.project(&[1.0, 0.0, 0.0, 0.0]);
        assert!((pd[0] - pd[1]).abs() < 1e-3, "слившиеся дубликаты: {pd:?}");
        // самопетля и выход за dims — ошибки
        assert!(CausalProjector::new(&[(2, 2, 1.0)], 4).is_err());
        assert!(CausalProjector::new(&[(0, 9, 1.0)], 4).is_err());
        assert!(CausalProjector::canonical_lock(1).is_err());
        assert!(CausalProjector::new(&[], 0).is_err());
    }

    #[test]
    fn residual_measures_hallucination_mass() {
        let p = CausalProjector::new(&[(0, 1, 2.0)], 3).unwrap();
        assert!(p.residual(&[1.0, 1.0, 5.0]).abs() < 1e-6);
        let r = p.residual(&[1.0, 0.0, 0.0]);
        assert!((r - 2.0).abs() < 1e-5, "‖J_c p‖ = 2·|p0−p1| = 2, got {r}");
    }
}

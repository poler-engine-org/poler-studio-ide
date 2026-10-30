//! # Квантовый субстрат УДЕ: P-поток Ауфбау с γ-прецесией (цикл G)
//!
//! Прямая инстанциация §2.2 Тома VII (ЕДИНОЕ ДИСКРЕТНОЕ УРАВНЕНИЕ):
//!
//! ```text
//! P_{t+1} = Q( P_t − η·[P_t,[P_t,F]] + η·γ·[F,P_t] + μ·(N − Tr P_t)/n·I )
//! Q(X) = 3X² − 2X³   (квантователь МакВини — механизм измерения УДЕ)
//! ```
//!
//! * `[P,[P,F]]` — диссипатор Ауфбау с сертификатом Ляпунова
//!   dE/dτ = −‖[H,P]‖²_HS ≤ 0 (Thm F.8b);
//! * `[F,P]` — ротор (изоспектральный в непрерывном пределе, Thm F.4/F.5);
//!   при γ > 0 занятое подпространство прецессирует — открытая строка
//!   §6.5 Тома VII, закрываемая здесь численно;
//! * химический потенциал μ(N−Tr P)/n возвращает след точно (§2.1-Шаг 4);
//! * режим SCF: F[P] = H + U·diag(diag P) — самосогласованное поле
//!   (хаббардовское среднее), вторая открытая строка цикла G.
//!
//! ## Честная физика γ > 0
//!
//! Роторная добавка делает за шаг работу dE_rot = ηγ·Re Tr([H,F]·P):
//! при F = H она тождественно нулевая ([H,H] = 0) — прецессия крутит
//! траекторию, не меняя энергии; при F ≠ H (SCF) ротор качает энергию —
//! это различие квантифицировано в отчёте (`rotor_work` по шагам).

use crate::complex::Cx;
use crate::error::{PqcError, Result};
use crate::rng::Rng;

/// Плотная комплексная матрица n×n (row-major).
#[derive(Clone, Debug)]
pub struct CMat {
    n: usize,
    data: Vec<Cx>,
}

impl CMat {
    /// Нулевая матрица.
    pub fn zeros(n: usize) -> CMat {
        CMat {
            n,
            data: vec![Cx::ZERO; n * n],
        }
    }

    /// Единичная.
    pub fn identity(n: usize) -> CMat {
        let mut m = CMat::zeros(n);
        for i in 0..n {
            m.data[i * n + i] = Cx::ONE;
        }
        m
    }

    /// Размер.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Доступ по (i, j).
    pub fn at(&self, i: usize, j: usize) -> Cx {
        self.data[i * self.n + j]
    }

    /// Мутация по (i, j).
    pub fn set(&mut self, i: usize, j: usize, v: Cx) {
        self.data[i * self.n + j] = v;
    }

    /// Матричное умножение.
    pub fn mul(&self, rhs: &CMat) -> Result<CMat> {
        if self.n != rhs.n {
            return Err(PqcError::BadArgument {
                what: "matrix dimension mismatch".into(),
            });
        }
        let n = self.n;
        let mut out = CMat::zeros(n);
        for i in 0..n {
            for k in 0..n {
                let a = self.data[i * n + k];
                if a == Cx::ZERO {
                    continue;
                }
                for j in 0..n {
                    out.data[i * n + j] += a * rhs.data[k * n + j];
                }
            }
        }
        Ok(out)
    }

    /// Поэлементная сумма.
    pub fn add(&self, rhs: &CMat) -> Result<CMat> {
        if self.n != rhs.n {
            return Err(PqcError::BadArgument {
                what: "matrix dimension mismatch".into(),
            });
        }
        Ok(CMat {
            n: self.n,
            data: self
                .data
                .iter()
                .zip(rhs.data.iter())
                .map(|(a, b)| *a + *b)
                .collect(),
        })
    }

    /// Умножение на скаляр.
    pub fn scale(&self, k: f64) -> CMat {
        CMat {
            n: self.n,
            data: self.data.iter().map(|a| a.scale(k)).collect(),
        }
    }

    /// Эрмитово сопряжение.
    pub fn conj_t(&self) -> CMat {
        let n = self.n;
        let mut out = CMat::zeros(n);
        for i in 0..n {
            for j in 0..n {
                out.data[j * n + i] = self.data[i * n + j].conj();
            }
        }
        out
    }

    /// След (вещественная часть — эрмитовы матрицы).
    pub fn trace(&self) -> f64 {
        (0..self.n).map(|i| self.data[i * self.n + i].re).sum()
    }

    /// ‖A‖²_HS (Frobenius).
    pub fn hs_norm_sq(&self) -> f64 {
        self.data.iter().map(|a| a.norm_sq()).sum()
    }

    /// Коммутатор [A, B].
    pub fn commutator(a: &CMat, b: &CMat) -> Result<CMat> {
        a.mul(b)?.add(&b.mul(a)?.scale(-1.0))
    }

    /// Дефект эрмитовости ‖A − A†‖_HS.
    pub fn hermiticity_defect(&self) -> f64 {
        let d = self
            .conj_t()
            .add(&self.scale(-1.0))
            .expect("same dim");
        d.hs_norm_sq().sqrt()
    }

    /// Случайная эрмитова матрица (сид → воспроизводимость; диагональ
    /// вещественна, недиагональ — комплексная с равномерными частями).
    pub fn random_hermitian(n: usize, rng: &mut Rng, amp: f64) -> Result<CMat> {
        if n == 0 || n > 16 {
            return Err(PqcError::BadArgument {
                what: "substrate dimension must be in 1..=16".into(),
            });
        }
        let mut m = CMat::zeros(n);
        for i in 0..n {
            m.set(i, i, Cx::new(amp * (2.0 * rng.next_f64() - 1.0), 0.0));
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let v = Cx::new(
                    amp * (2.0 * rng.next_f64() - 1.0),
                    amp * (2.0 * rng.next_f64() - 1.0),
                );
                m.set(i, j, v);
                m.set(j, i, v.conj());
            }
        }
        Ok(m)
    }

    /// Стартовая матрица плотности: заполнение N/n плюс малый шум,
    /// спектр заведомо внутри (0, 1) при fill ≤ 1 и малом шуме.
    pub fn initial_density(n: usize, particles: usize, rng: &mut Rng) -> Result<CMat> {
        if particles == 0 || particles > n {
            return Err(PqcError::BadArgument {
                what: "particle count must be in 1..=dim".into(),
            });
        }
        let fill = particles as f64 / n as f64;
        let mut p = CMat::identity(n).scale(fill);
        for i in 0..n {
            p.set(i, i, Cx::new(fill + 0.02 * (2.0 * rng.next_f64() - 1.0), 0.0));
        }
        Ok(p)
    }
}

/// Квантователь МакВини Q(X) = 3X² − 2X³ (идемпотентизация).
pub fn mcweeney(x: &CMat) -> Result<CMat> {
    let x2 = x.mul(x)?;
    let x3 = x2.mul(x)?;
    x2.scale(3.0).add(&x3.scale(-2.0))
}

/// Конфигурация субстрата.
#[derive(Clone, Copy, Debug)]
pub struct SubstrateConfig {
    /// Шаг дискретизации η.
    pub eta: f64,
    /// Роторная связь γ (0 = чистая диссипация; > 0 = прецессия).
    pub gamma: f64,
    /// Химический потенциал μ (сила восстановления следа).
    pub mu: f64,
    /// Число частиц N.
    pub particles: usize,
    /// Число шагов.
    pub steps: usize,
    /// SCF-связь U (0 = фиксированный фокиан F = H).
    pub scf_u: f64,
}

impl Default for SubstrateConfig {
    fn default() -> Self {
        SubstrateConfig {
            eta: 0.05,
            gamma: 0.0,
            mu: 1.0,
            particles: 2,
            steps: 400,
            scf_u: 0.0,
        }
    }
}

/// Одна точка траектории.
#[derive(Clone, Copy, Debug)]
pub struct SubstratePoint {
    /// Шаг.
    pub step: usize,
    /// Tr P.
    pub trace: f64,
    /// Tr P² (чистота: N для проектора).
    pub purity: f64,
    /// Tr H·P (энергия).
    pub energy: f64,
    /// ‖[H,P]‖²_HS — плотность сертификата Ляпунова.
    pub lyapunov: f64,
    /// ‖P² − P‖_HS — дефект идемпотентности.
    pub defect: f64,
    /// Работа ротора за шаг: ηγ·Re Tr([H,F]·P).
    pub rotor_work: f64,
    /// ‖P_t − P_{t−1}‖_HS — скорость прецессии.
    pub precession_speed: f64,
}

/// Итог отчёта.
#[derive(Clone, Debug)]
pub struct SubstrateReport {
    /// Конфигурация.
    pub config: SubstrateConfig,
    /// Размерность.
    pub dim: usize,
    /// Гамильтониан H (для репликации вовне).
    pub hamiltonian: CMat,
    /// Стартовая P₀ (для репликации вовне).
    pub p0: CMat,
    /// Точки траектории (каждые trace_every шагов + последняя).
    pub points: Vec<SubstratePoint>,
    /// Финальная P.
    pub final_p: CMat,
    /// Нарушения монотонности энергии (число шагов, где энергия выросла
    /// больше чем на 1e-12; при γ>0 и F≠H ожидаемо ненулевые).
    pub energy_violations: usize,
    /// Суммарная |работа ротора| за траекторию.
    pub rotor_work_total: f64,
    /// Максимальный дрейф следа в полёте.
    pub max_trace_drift: f64,
    /// Финальный дефект идемпотентности.
    pub final_defect: f64,
    /// Финальный коммутатор ‖[F,P]‖_HS (стационарность).
    pub final_commutator: f64,
    /// SCF: финальная невязка самосогласованности ‖F[P] − F[P_prev]‖_HS.
    pub scf_residual: f64,
}

/// Фокиан: F = H при U = 0; F = H + U·diag(Re diag P) в режиме SCF.
fn fockian(h: &CMat, p: &CMat, u: f64) -> CMat {
    if u == 0.0 {
        return h.clone();
    }
    let n = h.n();
    let mut f = h.clone();
    for i in 0..n {
        let d = p.at(i, i).re;
        f.set(i, i, f.at(i, i) + Cx::new(u * d, 0.0));
    }
    f
}

/// Прогнать P-поток УДЕ §2.2.
pub fn run_flow(h: &CMat, p0: &CMat, cfg: SubstrateConfig, trace_every: usize) -> Result<SubstrateReport> {
    let n = h.n();
    if p0.n() != n {
        return Err(PqcError::BadArgument {
            what: "H and P0 dimension mismatch".into(),
        });
    }
    if cfg.eta <= 0.0 || !cfg.eta.is_finite() || cfg.steps == 0 {
        return Err(PqcError::BadArgument {
            what: "eta must be > 0 and finite; steps >= 1".into(),
        });
    }
    let mut p = p0.clone();
    let mut prev = p0.clone();
    let mut points = Vec::new();
    let mut energy_violations = 0usize;
    let mut rotor_work_total = 0.0f64;
    let mut max_trace_drift = 0.0f64;
    let mut last_energy = (h.mul(&p)?).trace();
    let mut scf_residual = 0.0;

    let mut f = fockian(h, &p, cfg.scf_u);
    for step in 1..=cfg.steps {
        // Силы УДЕ.
        let pf = CMat::commutator(&p, &f)?; // [P, F]
        let dissipator = CMat::commutator(&p, &pf)?; // [P, [P, F]]
        let rotor = CMat::commutator(&f, &p)?; // [F, P] = −[P, F]

        // Химический потенциал: μ(N − Tr P)/n · I.
        let shift = cfg.mu * (cfg.particles as f64 - p.trace()) / n as f64;

        // Шаг УДЕ: P ← Q(P − η·D + ηγ·R + shift·I).
        let mut next = p
            .add(&dissipator.scale(-cfg.eta))?
            .add(&rotor.scale(cfg.eta * cfg.gamma))?;
        for i in 0..n {
            next.set(i, i, next.at(i, i) + Cx::new(shift, 0.0));
        }
        p = mcweeney(&next)?;

        // Работа ротора за шаг: ηγ·Re Tr([H,F]·P) — полный след произведения,
        // не только диагональ (диагональ коммутатора эрмитовых матриц чисто
        // мнима — короткий след тождественно нулевой).
        let hf = CMat::commutator(h, &f)?;
        let rot_work = (hf.mul(&p)?.trace()) * cfg.eta * cfg.gamma;

        // Метрики.
        let hp = h.mul(&p)?;
        let energy = hp.trace();
        let comm_hp = CMat::commutator(h, &p)?;
        let lyapunov = comm_hp.hs_norm_sq();
        let pp = p.mul(&p)?;
        let defect = pp.add(&p.scale(-1.0))?.hs_norm_sq().sqrt();
        let purity = pp.trace();
        let trace_drift = (p.trace() - cfg.particles as f64).abs();
        max_trace_drift = max_trace_drift.max(trace_drift);
        let precession_speed = p.add(&prev.scale(-1.0))?.hs_norm_sq().sqrt();
        if energy > last_energy + 1e-12 {
            energy_violations += 1;
        }
        rotor_work_total += rot_work.abs();
        last_energy = energy;

        // SCF: обновляем фокиан от новой плотности, считаем невязку.
        let f_new = fockian(h, &p, cfg.scf_u);
        scf_residual = f_new.add(&f.scale(-1.0))?.hs_norm_sq().sqrt();
        f = f_new;

        if step % trace_every.max(1) == 0 || step == cfg.steps {
            points.push(SubstratePoint {
                step,
                trace: p.trace(),
                purity,
                energy,
                lyapunov,
                defect,
                rotor_work: rot_work,
                precession_speed,
            });
        }
        prev = p.clone();
    }

    let final_defect = points
        .last()
        .map(|pt| pt.defect)
        .unwrap_or(f64::INFINITY);
    let final_commutator = CMat::commutator(&f, &p)?.hs_norm_sq().sqrt();

    Ok(SubstrateReport {
        config: cfg,
        dim: n,
        hamiltonian: h.clone(),
        p0: p0.clone(),
        points,
        final_p: p,
        energy_violations,
        rotor_work_total,
        max_trace_drift,
        final_defect,
        final_commutator,
        scf_residual,
    })
}

/// Стандартный прогон: случайный H и P₀ по сиду (для CLI и верификатора).
pub fn run_random(dim: usize, seed: u64, cfg: SubstrateConfig, trace_every: usize) -> Result<SubstrateReport> {
    let mut rng = Rng::seed_from_u64(seed);
    let h = CMat::random_hermitian(dim, &mut rng, 1.0)?;
    let p0 = CMat::initial_density(dim, cfg.particles, &mut rng)?;
    run_flow(&h, &p0, cfg, trace_every)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    #[test]
    fn aufbau_converges_gamma_zero() {
        let cfg = SubstrateConfig {
            eta: 0.05,
            gamma: 0.0,
            mu: 1.0,
            particles: 2,
            steps: 800,
            scf_u: 0.0,
        };
        let rep = run_random(4, 42, cfg, 100).unwrap();
        let last = rep.points.last().unwrap();
        assert!(last.defect < 1e-10, "idempotent P*: {}", last.defect);
        assert!((last.trace - 2.0).abs() < 1e-9, "trace: {}", last.trace);
        assert!(last.lyapunov < 1e-10, "stationarity [H,P]: {}", last.lyapunov);
        // Честная монотонность дискретной итерации: квантователь Маквини
        // вносит ограниченные транзитные подъёмы (измерение!), но чистый
        // спуск энергии гарантирован: нарушений ≤ 3% шагов и финал ниже старта.
        let first = rep.points.first().unwrap();
        assert!(
            rep.energy_violations <= rep.config.steps / 33,
            "transient violations: {} of {}",
            rep.energy_violations,
            rep.config.steps
        );
        assert!(last.energy < first.energy, "net energy decrease");
        // Чистота проектора ранга 2 равна 2.
        assert!((last.purity - 2.0).abs() < 1e-9, "purity: {}", last.purity);
    }

    #[test]
    fn gamma_precession_preserves_energy_when_f_equals_h() {
        // F = H (SCF off): ротор не делает работу — Tr([H,F]P) = 0 точно.
        let cfg = SubstrateConfig {
            eta: 0.05,
            gamma: 0.3,
            mu: 1.0,
            particles: 2,
            steps: 400,
            scf_u: 0.0,
        };
        let rep = run_random(4, 7, cfg, 50).unwrap();
        for pt in &rep.points {
            assert!(
                pt.rotor_work.abs() < 1e-12,
                "rotor work with F=H must vanish: {}",
                pt.rotor_work
            );
        }
        // Энергия при F = H: ротор не работает, диссипатор доминирует —
        // чистый спуск с ограниченными транзитами квантователя (≤3% шагов).
        assert!(
            rep.energy_violations <= rep.config.steps / 33,
            "transient violations: {} of {}",
            rep.energy_violations,
            rep.config.steps
        );
        let last = rep.points.last().unwrap();
        assert!(last.defect < 1e-8, "gamma precession still converges");
    }

    #[test]
    fn scf_mode_pumps_energy_through_rotor() {
        // F ≠ H (U ≠ 0): ротор качает энергию — честная физика прецессии.
        let cfg = SubstrateConfig {
            eta: 0.05,
            gamma: 0.3,
            mu: 1.0,
            particles: 2,
            steps: 300,
            scf_u: 0.8,
        };
        let rep = run_random(4, 11, cfg, 30).unwrap();
        assert!(
            rep.rotor_work_total > 1e-6,
            "rotor work with F!=H must be nonzero: {}",
            rep.rotor_work_total
        );
    }

    #[test]
    fn chemical_potential_restores_trace() {
        let cfg = SubstrateConfig {
            eta: 0.08,
            gamma: 0.0,
            mu: 2.0,
            particles: 3,
            steps: 600,
            scf_u: 0.0,
        };
        let rep = run_random(6, 3, cfg, 100).unwrap();
        let last = rep.points.last().unwrap();
        assert!(
            (last.trace - 3.0).abs() < 1e-8,
            "trace restored: {}",
            last.trace
        );
    }

    #[test]
    fn mcweeney_is_idempotent_on_projector() {
        // Q(P*) = P* для точного проектора.
        let mut p = CMat::identity(3);
        p.set(2, 2, Cx::ZERO);
        let q = mcweeney(&p).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((q.at(i, j).re - p.at(i, j).re).abs() < TOL);
                assert!(q.at(i, j).im.abs() < TOL);
            }
        }
    }
}

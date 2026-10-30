//! Ψ Интенция: канонический интегратор Литературного Двигателя.
//!
//! Собирает все фазы в одно уравнение обновления латентного состояния
//!
//! `p_{t+1} = p_t − η_eff·Π_Λ(∇F(p_t) + D·p_t + γ·J·p_t)
//!            + η_r·Π_Λ(κ·(echo_t − p_t))`
//!
//! - **∇F** — градиент свободной энергии `F = ‖p − Ω(o)‖²_G + λ·R_L(p)`:
//!   притяжение замысла (первый член) + логическая регуляризация
//!   `R_L = ‖J_c·p‖²` (наказание за галлюцинации);
//! - **D·p** — стабилизатор Ляпунова (мушиная метрика, см.
//!   [`super::flybridge`]): диссипация семантических пертурбаций;
//! - **γJ·p** — циркуляция смыслов: ротор живого мозга закручивает
//!   траекторию (без мухи — нулевой член, чистая физика текста);
//! - **echo** — темпоральное эхо R[n]: градиент энергии значимости
//!   `ε = κ/2·‖p − echo‖²` в форме тяги к резонансной памяти.
//!
//! Управление:
//! - **пластичность** `η_eff = η·(1 + 0.2·ε)/(1 + Σ(t))` — при высоком
//!   ε (новая истина) система деформируется смелее, при росте фазовой
//!   инерции Σ(t) шаг затухает (душа нарратива не теряется);
//! - **No Excuses**: если F растёт 3 шага подряд — семантический тупик,
//!   обновление преломляется призмой (энергия сохраняется);
//! - **Observer-Kill**: при `H^Ψ < θ` два шага подряд субъективные
//!   искажения (резонансное эхо) подавляются до нуля — чистая истина;
//! - **сверхпроводимость**: `F < θ` и `Σ < θ` — когнитивный покой,
//!   генерация завершена.

use super::echo::TemporalEcho;
use super::flybridge::{FieldProjection, FlyCast};
use super::linalg::{norm2, sub};
use super::prism::refract;
use super::projector::CausalProjector;
use super::qualia::{cosine_topology, perceive, Archetype, QualiaField, ARCHETYPES};

/// Гиперпараметры POLER[Ψ] (значения по умолчанию — из манифеста
/// Литературного Двигателя).
#[derive(Clone, Debug)]
pub struct LiteraryParams {
    /// η — шаг интегратора (0.1).
    pub eta: f32,
    /// η_r — резонансный шаг (0.05).
    pub eta_r: f32,
    /// ρ — затухание эха (0.9).
    pub rho: f32,
    /// κ — масштаб энергии значимости (1.2).
    pub kappa: f32,
    /// γ — баланс циркуляции J (1.0).
    pub gamma: f32,
    /// λ — вес логической регуляризации (1e-2).
    pub lambda_rl: f32,
    /// Число осей фазового пространства (64).
    pub dims: usize,
    /// Максимум шагов генерации (48).
    pub max_steps: usize,
    /// Порог когнитивного покоя H^Ψ (1e-3).
    pub theta: f32,
    /// Мёртвая зона Trit5-квантования (0.05).
    pub trit_theta: f32,
    /// No-Mul режим: p ← dequant(quant(p)) после каждого шага.
    pub no_mul: bool,
}

impl Default for LiteraryParams {
    fn default() -> Self {
        Self {
            eta: 0.1,
            eta_r: 0.05,
            rho: 0.9,
            kappa: 1.2,
            gamma: 1.0,
            lambda_rl: 1e-2,
            dims: 64,
            max_steps: 48,
            theta: 1e-3,
            trit_theta: 0.05,
            no_mul: false,
        }
    }
}

/// Режим когнитивного состояния системы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsiMode {
    /// Притяжение к замыслу (обычная динамика).
    Attraction,
    /// Преломление тупика через призму (No Excuses).
    Refraction,
    /// Observer-Kill: субъективные искажения подавлены.
    ObserverKill,
    /// Сверхпроводимость смысла: H^Ψ ≈ 0.
    Superconductivity,
}

/// Телеметрия одного шага (инженерный отчёт для агента).
#[derive(Clone, Debug)]
pub struct StepTelemetry {
    /// Номер шага (1-based).
    pub step: usize,
    /// Свободная энергия F.
    pub f_free: f32,
    /// Энергия значимости ε («смысловое удивление»).
    pub epsilon: f32,
    /// Топологическая кривизна Σ(t) — фазовая инерция.
    pub sigma_t: f32,
    /// Норма резонансной памяти ‖echo‖.
    pub resonance_norm: f32,
    /// Фазовая инерция: 1 − cos(p_t, p_{t+1}).
    pub phase_inertia: f32,
    /// Нарушение причинности ‖J_c·p‖.
    pub causality_residual: f32,
    /// Пластичность 0.2·ε.
    pub plasticity: f32,
    /// H^Ψ — обобщённая дистанция до когнитивного покоя.
    pub h_psi: f32,
    /// Режим состояния.
    pub mode: PsiMode,
    /// Число преломлений призмы к этому шагу (кумулятивно).
    pub refractions: usize,
    /// Норма шага ‖Δp‖.
    pub step_norm: f32,
}

/// Архетипический доминанта фазы (для разметки актов).
#[derive(Clone, Debug)]
pub struct ArchetypeDominant {
    pub name: &'static str,
    pub weight: f32,
}

/// Срез траектории для акта нарратива.
#[derive(Clone, Debug)]
pub struct ActReport {
    /// Индексы шагов акта (половина-open, 0 = исходное состояние).
    pub steps: (usize, usize),
    /// Доминирующие архетипы акта (топ-3 по средней близости).
    pub dominants: Vec<ArchetypeDominant>,
    /// F в начале и конце акта.
    pub f_start: f32,
    pub f_end: f32,
}

/// Итоговый отчёт генерации.
#[derive(Clone, Debug)]
pub struct NarrativeReport {
    /// Параметры запуска.
    pub params: LiteraryParams,
    /// Число сделанных шагов.
    pub steps: usize,
    /// Дошёл ли до сверхпроводимости.
    pub converged: bool,
    /// Финальные метрики.
    pub final_f: f32,
    pub final_h_psi: f32,
    pub final_sigma: f32,
    /// Причинность чиста?
    pub causality_clean: bool,
    /// Полная телеметрия по шагам.
    pub telemetry: Vec<StepTelemetry>,
    /// Акты: завязка / развитие / развязка.
    pub acts: [ActReport; 3],
    /// Топ-термы замысла.
    pub intent_terms: Vec<(String, f32)>,
    /// Драматургические пары (если муха подключена).
    pub dramatic_pairs: Vec<super::flybridge::DramaticPair>,
    /// Число преломлений призмы.
    pub refractions: usize,
    /// Детерминированный текст нарратива (резонансный отклик).
    pub text: String,
}

/// Литературный Двигатель: состояние + фазовая динамика.
pub struct LiteraryEngine {
    pub params: LiteraryParams,
    /// Латентное состояние p_t.
    pub p: Vec<f32>,
    /// Наблюдение Ω(o) замысла.
    pub obs: Vec<f32>,
    /// Поле перцепции (для отчётов).
    pub field: QualiaField,
    /// Проектор причинности Π_Λ.
    pub projector: CausalProjector,
    /// Темпоральное эхо R[n].
    pub echo: TemporalEcho,
    /// Мушиная калибровка (None — чистая физика текста).
    pub cast: Option<FlyCast>,
    /// Фазовые проекции касты (J_lit, D_lit).
    projection: Option<FieldProjection>,
    /// Ось-векторы архетипов (для призмы; спроецированы в null(J_c)).
    archetype_axes: Vec<Vec<f32>>,
    /// Предыдущее состояние (для Σ(t) и phase inertia).
    prev: Vec<f32>,
    /// Счётчик роста F (детектор тупика).
    f_rise_streak: usize,
    /// Кумулятивные преломления.
    refractions: usize,
    /// Серия покоя H^Ψ < θ.
    calm_streak: usize,
    /// Режим текущего состояния.
    mode: PsiMode,
    /// Резонансное эхо активно (до Observer-Kill).
    echo_alive: bool,
    /// Счётчик шагов (независим от горизонта эха: глубина памяти
    /// насыщается на horizon, а нумерация траектории — вечна).
    step_count: usize,
}

impl LiteraryEngine {
    /// Новый движок: замысел → перцепция → нулевое состояние p₀
    /// (система стартует из тишины и притягивается к смыслу).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: LiteraryParams,
        intent: &str,
        constraints: &[(usize, usize, f32)],
        cast: Option<FlyCast>,
    ) -> Result<Self, String> {
        let dims = params.dims.clamp(16, 256);
        let mut params = params;
        params.dims = dims;
        let field = perceive(intent, dims);
        let projector = if constraints.is_empty() {
            CausalProjector::canonical_lock(dims)?
        } else {
            CausalProjector::new(constraints, dims)?
        };
        // Ось-векторы архетипов в фазовом пространстве, спроецированные
        // в null(J_c) — легальные направления приёма призмы.
        let archetype_axes: Vec<Vec<f32>> = ARCHETYPES
            .iter()
            .map(|a: &Archetype| {
                let mut v = vec![0.0f32; dims];
                for kw in a.keywords {
                    v[super::qualia::axis_of(kw, dims)] += 1.0;
                }
                let pv = projector.project(&super::linalg::normalize(&v));
                super::linalg::normalize(&pv)
            })
            .collect();
        // Проекция касты в поле (если муха подключена).
        let projection = cast.as_ref().map(|c| c.project_to_field(dims));
        let n_steps_cap = params.max_steps.clamp(1, 512);
        params.max_steps = n_steps_cap;
        Ok(Self {
            echo: TemporalEcho::new(params.rho, 32.min(n_steps_cap), dims),
            p: vec![0.0; dims],
            obs: field.obs.clone(),
            field,
            projector,
            cast,
            projection,
            archetype_axes,
            prev: vec![0.0; dims],
            f_rise_streak: 0,
            refractions: 0,
            calm_streak: 0,
            mode: PsiMode::Attraction,
            echo_alive: true,
            step_count: 0,
            params,
        })
    }

    /// Свободная энергия F = ‖p − Ω(o)‖²_G + λ·‖J_c·p‖².
    /// Метрика G — единичная (мушиная диссипация входит отдельным
    /// членом D·p, а не в метрику нормы — сохраняем каноническую форму
    /// уравнения из манифеста).
    ///
    /// v0.87.0 VAULT-ASM: предиктивный член ‖p−o‖² считается микроядром
    /// `poler_fep_energy` (AVX2+FMA, 8 lanes) — порт fep_loss.rs из
    /// сокровищницы; λ-член остаётся в Rust (разреженный проектор J_c).
    pub fn free_energy(&self) -> f32 {
        let pred = crate::asm::fep_asm::energy(&self.p, &self.obs, None, 0.0);
        let logic = self.projector.residual(&self.p);
        pred + self.params.lambda_rl * logic * logic
    }

    /// Градиент ∇F = 2(p − o) + 2λ·J_cᵀ(J_c·p).
    ///
    /// v0.87.0 VAULT-ASM: член 2(p−o) — микроядро `poler_fep_grad`
    /// (G=I, λ=0 — точный контракт формулы).
    fn grad_f(&self) -> Vec<f32> {
        let mut g = vec![0.0f32; self.params.dims];
        crate::asm::fep_asm::grad(&self.p, &self.obs, None, 0.0, &mut g);
        if self.projector.rank() > 0 {
            let jc_p = self.projector.j_c.matvec(&self.p);
            let mut corr = vec![0.0f32; self.params.dims];
            for r in 0..self.projector.j_c.rows {
                let row = self.projector.j_c.row(r);
                let c = 2.0 * self.params.lambda_rl * jc_p[r];
                for (o, &jv) in corr.iter_mut().zip(row) {
                    *o += c * jv;
                }
            }
            for (gv, cv) in g.iter_mut().zip(&corr) {
                *gv += cv;
            }
        }
        g
    }

    /// H^Ψ — обобщённая дистанция до когнитивного покоя: нормированные
    /// F, остаток причинности и инерция складываются в [0, ~3].
    /// v0.90 АЗУ: нормировка через div_safe_f32 — полюс x = −1 (знаменатель
    /// ровно 0) насыщает ±MAX вместо NaN-отравления конвейера, 0/0 → 0.
    fn h_psi(f: f32, residual: f32, sigma: f32) -> f32 {
        let norm = |x: f32| crate::asm::zero_asm::div_safe_f32(x, 1.0 + x);
        norm(f) + norm(residual) + norm(sigma)
    }

    /// Один шаг фазового обновления p_t → p_{t+1}. Возвращает телеметрию.
    pub fn step(&mut self, observation: Option<&str>) -> StepTelemetry {
        self.step_count += 1;
        let step_no = self.step_count;
        // Новое наблюдение обновляет Ω(o) (эволюция замысла).
        if let Some(text) = observation {
            let f = perceive(text, self.params.dims);
            if f.n_tokens > 0 {
                self.obs = f.obs;
            }
        }
        // Эхо до шага: тяга резонанса.
        let echo_state = if self.echo_alive { self.echo.echo() } else { vec![0.0; self.params.dims] };
        let surprise = self.echo.surprise(&self.p, self.params.kappa);
        let f_before = self.free_energy();

        // Полный drift: ∇F + D·p + γ·J·p (+ тяга эха отдельным шагом η_r).
        let mut drift = self.grad_f();
        if let Some(proj) = &self.projection {
            let dp = proj.d_lit.matvec(&self.p);
            let jp = proj.j_lit.matvec(&self.p);
            for (d, (&dv, &jv)) in drift.iter_mut().zip(dp.iter().zip(&jp)) {
                *d += dv + self.params.gamma * jv;
            }
        }
        // Резонансное слагаемое: +κ(echo − p) — градиентный спуск по ε.
        let mut resonance = vec![0.0f32; self.params.dims];
        if self.echo_alive {
            for (r, (&e, &pv)) in resonance.iter_mut().zip(echo_state.iter().zip(&self.p)) {
                *r = self.params.kappa * (e - pv);
            }
        }

        // Пластичность и фазовая инерция управляют эффективным шагом.
        let prev_sigma = Self::sigma_of(&self.prev, &self.p);
        let plasticity = 0.2 * surprise;
        let eta_eff = self.params.eta * (1.0 + plasticity) / (1.0 + prev_sigma);
        let eta_r_eff = self.params.eta_r * (1.0 + plasticity) / (1.0 + prev_sigma);

        // Полное обновление (до проекции).
        let mut update = drift;
        for (u, &r) in update.iter_mut().zip(&resonance) {
            *u = -eta_eff * *u + eta_r_eff * r;
        }

        // No Excuses: тупик = F растёт 3 шага подряд.
        let f_prev_streak = self.f_rise_streak;
        let mut prism_mode = None;
        if f_prev_streak >= 3 {
            let res = refract(&update, |v| self.projector.project(v), &self.archetype_axes);
            self.refractions += 1;
            prism_mode = Some(res.mode);
            update = res.refracted;
        }

        // Проекция причинности — всегда последняя стена.
        self.p = self.projector.project(&{
            let mut next = self.p.clone();
            for (nv, &u) in next.iter_mut().zip(&update) {
                *nv += u;
            }
            next
        });

        // No-Mul: раунд-трип через троичную решётку.
        if self.params.no_mul {
            let (_, back) = super::trit::roundtrip(&self.p, self.params.trit_theta);
            self.p = back;
        }

        // Энергетический потолок фазового пространства: защита от
        // численного разгона (NaN/inf невозможны по построению, но
        // экстремальные γ/η могут раскачать норму — принудительный
        // возврат на единичную сферу сохраняет физику смысла).
        let pn = norm2(&self.p);
        if !pn.is_finite() || pn > 1e3 {
            let fixed = super::linalg::normalize(&self.p);
            self.p = if fixed.iter().all(|&x| x.is_finite()) {
                fixed
            } else {
                vec![0.0; self.params.dims]
            };
        }

        // Метрики после шага.
        let f_after = self.free_energy();
        let step_norm = norm2(&sub(&self.p, &self.prev));
        let phase_inertia = 1.0 - super::linalg::cosine(&self.prev, &self.p);
        let sigma_t = Self::sigma_of(&self.prev, &self.p);
        let causality = self.projector.residual(&self.p);
        let h_psi = Self::h_psi(f_after, causality, sigma_t);

        // Детектор тупика для следующего шага.
        self.f_rise_streak = if f_after > f_before + 1e-9 {
            self.f_rise_streak + 1
        } else {
            0
        };

        // Observer-Kill / сверхпроводимость.
        if h_psi < self.params.theta {
            self.calm_streak += 1;
            if self.calm_streak >= 2 {
                if self.echo_alive {
                    self.echo_alive = false;
                    self.mode = PsiMode::ObserverKill;
                } else {
                    self.mode = PsiMode::Superconductivity;
                }
            }
        } else {
            self.calm_streak = 0;
            self.mode = PsiMode::Attraction;
        }
        if let Some(m) = prism_mode {
            self.mode = PsiMode::Refraction;
            let _ = m;
        }

        // Состояние в резонансную память.
        let _ = self.echo.push(&self.p);
        self.prev = self.p.clone();

        StepTelemetry {
            step: step_no,
            f_free: f_after,
            epsilon: surprise,
            sigma_t,
            resonance_norm: norm2(&echo_state),
            phase_inertia,
            causality_residual: causality,
            plasticity,
            h_psi,
            mode: self.mode,
            refractions: self.refractions,
            step_norm,
        }
    }

    /// Σ(t): относительная кривизна траектории.
    fn sigma_of(prev: &[f32], cur: &[f32]) -> f32 {
        let d = sub(cur, prev);
        let n = norm2(cur);
        if n < 1e-12 {
            return 0.0;
        }
        (norm2(&d) / n).min(1.0)
    }

    /// Полная генерация: шаги до сверхпроводимости (или исчерпания
    /// лимита) + детерминированный текст нарратива.
    pub fn generate(&mut self) -> NarrativeReport {
        let mut telemetry = Vec::with_capacity(self.params.max_steps);
        let params_snapshot = self.params.clone();
        // Снапшоты состояния на границах актов (третьи доли траектории).
        let mut snapshots: Vec<Vec<f32>> = Vec::with_capacity(3);
        let total = self.params.max_steps;
        for i in 0..total {
            let t = self.step(None);
            let stop = t.mode == PsiMode::Superconductivity;
            telemetry.push(t);
            let done = i + 1;
            let third = (total / 3).max(1);
            if done == third || done == 2 * third || done == total || stop {
                snapshots.push(self.p.clone());
            }
            if stop {
                break;
            }
        }
        let converged = self.mode == PsiMode::Superconductivity;
        // Снапшоты на границы актов (недостающие — финальным состоянием).
        let final_p = self.p.clone();
        while snapshots.len() < 3 {
            snapshots.push(final_p.clone());
        }
        // Акты: третьи доли траектории + архетипические доминанты снапшотов.
        let acts = Self::build_acts(&telemetry, &snapshots, self.params.dims);
        // Драматургические пары мухи.
        let dramatic_pairs = match (&self.cast, &self.projection) {
            (Some(c), Some(pr)) => c.dramatic_pairs(pr, 5),
            _ => Vec::new(),
        };
        let text = self.render_treatment(&telemetry, &acts, converged, &dramatic_pairs);
        NarrativeReport {
            params: params_snapshot,
            steps: telemetry.len(),
            converged,
            final_f: telemetry.last().map(|t| t.f_free).unwrap_or(0.0),
            final_h_psi: telemetry.last().map(|t| t.h_psi).unwrap_or(0.0),
            final_sigma: telemetry.last().map(|t| t.sigma_t).unwrap_or(0.0),
            causality_clean: self.projector.residual(&self.p) < 1e-3,
            telemetry,
            acts,
            intent_terms: self.field.term_mass.clone(),
            dramatic_pairs,
            refractions: self.refractions,
            text,
        }
    }

    /// Разметка актов по третям траектории; доминанты — косинусная
    /// топология снапшотов состояния против осей архетипов.
    fn build_acts(
        tel: &[StepTelemetry],
        snapshots: &[Vec<f32>],
        dims: usize,
    ) -> [ActReport; 3] {
        let n = tel.len();
        let third = (n / 3).max(1);
        let empty_act = || ActReport {
            steps: (0, 0),
            dominants: Vec::new(),
            f_start: 0.0,
            f_end: 0.0,
        };
        let mut out: [ActReport; 3] = [empty_act(), empty_act(), empty_act()];
        for a in 0..3 {
            let s = (a * third).min(n);
            let e = if a == 2 { n } else { ((a + 1) * third).min(n) };
            let e = e.max(s);
            let slice = if s < e { &tel[s..e] } else { &tel[n.saturating_sub(1)..] };
            // Доминанты акта: снапшот состояния → косинусы с архетипами.
            let snap = &snapshots[a.min(snapshots.len().saturating_sub(1))];
            let mut ranked: Vec<(&'static str, f32)> = ARCHETYPES
                .iter()
                
                .map(|a| {
                    let mut axis = vec![0.0f32; dims];
                    for kw in a.keywords {
                        axis[super::qualia::axis_of(kw, dims)] += 1.0;
                    }
                    (a.name, super::linalg::cosine(snap, &axis))
                })
                .collect();
            ranked.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(y.0)));
            out[a] = ActReport {
                steps: (s, e),
                dominants: ranked
                    .into_iter()
                    .take(3)
                    .map(|(name, weight)| ArchetypeDominant { name, weight })
                    .collect(),
                f_start: slice.first().map(|t| t.f_free).unwrap_or(0.0),
                f_end: slice.last().map(|t| t.f_free).unwrap_or(0.0),
            };
        }
        out
    }

    /// Детерминированный текст: резонансный отклик на замысел.
    ///
    /// Не стохастический «генератор токенов», а расчётная разметка
    /// траектории: три акта, архетипические доминанты, конфликты
    /// (роторные пары мухи при калибровке), финальная телеметрия.
    fn render_treatment(
        &self,
        tel: &[StepTelemetry],
        acts: &[ActReport; 3],
        converged: bool,
        dramatic_pairs: &[super::flybridge::DramaticPair],
    ) -> String {
        let top = cosine_topology(&self.field);
        let mut s = String::with_capacity(2048);
        s.push_str("ПОЛЕ ЗАМЫСЛА\n");
        let terms: Vec<String> = self
            .field
            .term_mass
            .iter()
            .take(5)
            .map(|(t, w)| format!("{t} ({w:.2})"))
            .collect();
        s.push_str(&format!("  Инвариант: {}\n", terms.join(", ")));
        s.push_str(&format!(
            "  Архетипы: {}\n",
            top.iter()
                .take(3)
                .map(|&(i, w)| format!("{} ({:.2})", ARCHETYPES[i].name, w))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        s.push_str("ТРАЕКТОРИЯ\n");
        for (i, act) in acts.iter().enumerate() {
            let name = ["Акт I — Завязка", "Акт II — Развитие", "Акт III — Развязка"][i];
            let doms: Vec<String> = act
                .dominants
                .iter()
                .map(|d| format!("{} ({:.2})", d.name, d.weight))
                .collect();
            s.push_str(&format!(
                "  {}: шаги {}..{}, F {:0.4} → {:0.4}, доминанты: {}\n",
                name,
                act.steps.0,
                act.steps.1,
                act.f_start,
                act.f_end,
                doms.join(", ")
            ));
        }
        if !dramatic_pairs.is_empty() {
            s.push_str("КОНФЛИКТЫ (роторные пары мухи)\n");
            for p in dramatic_pairs.iter().take(3) {
                s.push_str(&format!(
                    "  {} [{}] → доминирует над → {} [{}], J = {:+.1}\n",
                    p.u, p.u_archetype, p.v, p.v_archetype, p.j
                ));
            }
        }
        let last = tel.last();
        s.push_str("ТЕЛЕМЕТРИЯ\n");
        if let Some(t) = last {
            s.push_str(&format!(
                "  Шагов: {}, F = {:0.5}, ε = {:0.5}, Σ(t) = {:0.5}, H^Ψ = {:0.5}\n",
                t.step, t.f_free, t.epsilon, t.sigma_t, t.h_psi
            ));
            s.push_str(&format!(
                "  Причинность: {} (остаток {:0.2e}), преломлений призмы: {}\n",
                if self.projector.residual(&self.p) < 1e-3 { "чиста" } else { "есть шум" },
                self.projector.residual(&self.p),
                t.refractions
            ));
        }
        s.push_str(if converged {
            "  Режим: СВЕРХПРОВОДИМОСТЬ СМЫСЛА — H^Ψ = 0 достигнут, каждое предложение является резонансным откликом на замысел.\n"
        } else {
            "  Режим: асимптотическое приближение — H^Ψ > θ, увеличьте шаги или ослабьте ограничения.\n"
        });
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::connectome::test_support::synth;
    use crate::graph::connectome::Connectome;

    const INTENT: &str = "герой идёт в поход против тьмы и бездны, наставник даёт совет и знание";

    #[test]
    fn engine_converges_to_intent() {
        let mut eng = LiteraryEngine::new(LiteraryParams::default(), INTENT, &[], None).unwrap();
        let report = eng.generate();
        // F падает на порядок: старт ‖o‖² = 1 → должен дойти ниже 0.01
        assert!(
            report.final_f < 0.01,
            "F должен упасть ниже 0.01, got {}",
            report.final_f
        );
        // Причинность чиста
        assert!(report.causality_clean);
        // Телеметрия: F монотонно не растёт (порядок сохраняется с
        // допуском на диссипативные осцилляции эха)
        let mut rises = 0;
        for w in report.telemetry.windows(2) {
            if w[1].f_free > w[0].f_free + 1e-4 {
                rises += 1;
            }
        }
        assert!(rises <= 2, "F стабильно падает, подъёмов: {rises}");
        // Текст отчёта содержит все секции
        assert!(report.text.contains("ПОЛЕ ЗАМЫСЛА"));
        assert!(report.text.contains("ТРАЕКТОРИЯ"));
        assert!(report.text.contains("ТЕЛЕМЕТРИЯ"));
        assert!(report.text.contains("Акт III"));
        // Доминанты актов заполнены
        assert_eq!(report.acts.len(), 3);
        assert!(!report.acts[0].dominants.is_empty());
    }

    #[test]
    fn observer_kill_and_superconductivity() {
        let mut p = LiteraryParams::default();
        p.max_steps = 200;
        p.theta = 5e-3; // разумный порог покоя
        let mut eng = LiteraryEngine::new(p, "тишина и покой зимнего сада", &[], None).unwrap();
        let report = eng.generate();
        // Достижима сверхпроводимость (F → 0 на простом замысле)
        assert!(
            report.converged || report.final_h_psi < 0.05,
            "конвергенция: converged={}, H={}",
            report.converged,
            report.final_h_psi
        );
        // Режим в телеметрии деградирует до ObserverKill/Superconductivity
        let modes: Vec<PsiMode> = report.telemetry.iter().map(|t| t.mode).collect();
        assert!(
            modes.contains(&PsiMode::ObserverKill) || modes.contains(&PsiMode::Superconductivity),
            "Observer-Kill обязан сработать: {modes:?}"
        );
    }

    #[test]
    fn causality_lock_holds_throughout() {
        // Жёсткое ограничение: оси 0 и 2 эквивалентны.
        let mut p = LiteraryParams::default();
        p.max_steps = 24;
        let mut eng =
            LiteraryEngine::new(p, INTENT, &[(0, 2, 1.0)], None).unwrap();
        for _ in 0..24 {
            let t = eng.step(None);
            assert!(t.causality_residual < 1e-3, "шаг {}: остаток {}", t.step, t.causality_residual);
        }
        // И в самом состоянии
        assert!((eng.p[0] - eng.p[2]).abs() < 1e-3);
    }

    #[test]
    fn no_mul_mode_converges_loosely() {
        let mut p = LiteraryParams::default();
        p.no_mul = true;
        p.max_steps = 64;
        let mut eng = LiteraryEngine::new(p, INTENT, &[], None).unwrap();
        let report = eng.generate();
        // Троичная решётка грубее: F < 0.05 — разумный потолок
        assert!(
            report.final_f < 0.05,
            "No-Mul F = {} (допуск 0.05)",
            report.final_f
        );
        assert!(report.causality_clean);
    }

    #[test]
    fn fly_calibrated_engine_survives_and_circulates() {
        // Синтетический коннектом + каста: динамика не расходится,
        // причинность чиста, конфликты заполнены.
        let con = Connectome::from_raw(&synth(
            false,
            &[(0, 1, 10, 1), (0, 2, 5, 0), (1, 3, 3, 2), (2, 3, 7, 1), (3, 1, 9, 0), (1, 2, 4, 5)],
        ))
        .unwrap();
        let cast = super::super::flybridge::build_cast(&con, &[0], 2, 16).unwrap();
        let mut p = LiteraryParams::default();
        p.max_steps = 32;
        let mut eng = LiteraryEngine::new(p, INTENT, &[], Some(cast)).unwrap();
        let report = eng.generate();
        // Состояние не разнесло (диссипация Ляпунова работает)
        let pn = norm2(&eng.p);
        assert!(pn.is_finite() && pn < 10.0, "‖p‖ = {pn}");
        assert!(report.causality_clean);
        // Драматургические пары есть
        assert!(!report.dramatic_pairs.is_empty(), "мушиные конфликты обязаны проявиться");
        assert!(report.text.contains("КОНФЛИКТЫ"));
    }

    #[test]
    fn engine_deterministic() {
        let run = || {
            let mut eng = LiteraryEngine::new(LiteraryParams::default(), INTENT, &[], None).unwrap();
            eng.generate();
            eng.p.clone()
        };
        let a = run();
        let b = run();
        assert_eq!(a, b, "движок побитово детерминирован");
    }

    #[test]
    fn evolving_observation_pulls_state() {
        let mut eng = LiteraryEngine::new(LiteraryParams::default(), "герой и тень", &[], None).unwrap();
        eng.step(None);
        let f_before = eng.free_energy();
        // Замысел эволюционирует: наблюдение меняет Ω(o)
        eng.step(Some("тишина зимнего сада и покой"));
        let f_after_pull = eng.free_energy();
        // Энергия относительно нового наблюдения обязана быть ненулевой
        // (система ещё не притянута) — а затем падать
        assert!(f_after_pull > 0.0);
        for _ in 0..30 {
            eng.step(None);
        }
        assert!(eng.free_energy() < f_before + 1.0);
    }

    #[test]
    fn guards_and_edge_cases() {
        // dims клампится, пустой замысел легален (пустое поле)
        let mut p = LiteraryParams::default();
        p.dims = 4;
        let eng = LiteraryEngine::new(p, "", &[], None).unwrap();
        assert_eq!(eng.params.dims, 16, "dims клампится к 16");
        // Ограничение вне диапазона — ошибка
        assert!(LiteraryEngine::new(LiteraryParams::default(), "текст", &[(0, 999, 1.0)], None).is_err());
    }
}

#[cfg(test)]
mod golden_tests {
    use super::*;
    use crate::graph::connectome::Connectome;
    use crate::literary::flybridge::build_cast;

    const INTENT: &str = "герой идёт в поход против тьмы и бездны, наставник даёт совет и знание";

    fn core() -> Connectome {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/flywire-connectome/flywire_v783_core.csr.zst");
        assert!(p.exists(), "артефакт {} не найден", p.display());
        Connectome::load(&p).unwrap()
    }

    // Золото L1: чистая физика текста (без мухи) сходится в
    // сверхпроводимость; мушиная калибровка удерживает предельный цикл —
    // живой мозг не даёт нарративу замереть (циркуляция J вечна).
    #[test]
    fn golden_l1_pure_vs_fly_calibrated() {
        // 1) Чистый движок: сходимость до H^Ψ ≈ 0.
        let mut pure = LiteraryEngine::new(LiteraryParams::default(), INTENT, &[], None).unwrap();
        let r_pure = pure.generate();
        assert!(r_pure.converged, "чистый движок обязан достичь покоя");
        assert!(r_pure.final_f < 1e-4, "F = {}", r_pure.final_f);
        assert!(r_pure.causality_clean);

        // 2) Мушиная калибровка: предельный цикл вокруг аттрактора.
        let con = core();
        let cast = build_cast(&con, &[0], 2, 48).unwrap();
        let mut fly = LiteraryEngine::new(
            LiteraryParams::default(),
            INTENT,
            &[],
            Some(cast),
        )
        .unwrap();
        let r_fly = fly.generate();
        // Цикл не замер: сошлось = false, F выходит на плато ~0.257.
        assert!(!r_fly.converged, "муха не даёт замереть: цикл обязан жить");
        assert!(
            (r_fly.final_f - 0.2572).abs() < 5e-3,
            "плато цикла F = {}",
            r_fly.final_f
        );
        assert!(r_fly.causality_clean, "причинность чиста даже в цикле");
        assert_eq!(r_fly.refractions, 0, "тупиков нет — призма молчит");
        // Состояние ограничено (диссипация Ляпунова держит).
        let pn = norm2(&fly.p);
        assert!(pn.is_finite() && pn < 10.0, "‖p‖ = {pn}");
        // Драматургия отразилась в отчёте.
        assert!(!r_fly.dramatic_pairs.is_empty());
        assert!(r_fly.text.contains("КОНФЛИКТЫ"));
        // Полный прогон (48 шагов на 64 осях + каста) — миллисекунды.
    }

    // No-Mul на реальной мухе: троичная решётка держит цикл в допуске.
    #[test]
    fn golden_l1_fly_no_mul() {
        let con = core();
        let cast = build_cast(&con, &[0], 2, 48).unwrap();
        let mut p = LiteraryParams::default();
        p.no_mul = true;
        p.max_steps = 64;
        let mut eng = LiteraryEngine::new(p, INTENT, &[], Some(cast)).unwrap();
        let r = eng.generate();
        assert!(r.causality_clean);
        let pn = norm2(&eng.p);
        assert!(pn.is_finite() && pn < 10.0, "‖p‖ = {pn}");
        // Квантование расширяет предельный цикл (мёртвая зона 5%
        // масштаба грубит вращение): F оседает ~1.6 против 0.257 у f32 —
        // плата за No-Mul, цикл при этом остаётся ограниченным.
        assert!(r.final_f < 2.5, "No-Mul F = {}", r.final_f);
    }
}

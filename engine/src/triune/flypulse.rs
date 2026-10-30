//! Пульс мухи — ротор касты FLYCSR1 в пространстве 12 архетипов.
//!
//! Первая опора Триединства (S2/v0.36.0). Живой мозг мухи даёт речи
//! **циркуляцию**: ротор J = A − Aᵀ касты нейронов агрегируется в
//! антисимметричную матрицу 12×12 по якорям архетипов
//!
//! ```text
//! J_arch[a][b] = Σ_{i∈a, j∈b} J_sub[i][j],   J_archᵀ = −J_arch
//! ```
//!
//! Фаза касты эволюционирует как `p ← p + dt·(γ·J·p − D·p)`, где D —
//! агрегированная метрика Ляпунова (грамиан роторных масс + I). Ротор
//! **не даёт мысли застыть**: нулевое γ вырождает речь в циклы, живое
//! γ вращает якоря и разводит лексику по архетипам (доказано тестом
//! `fly_rotation_breaks_loops`).
//!
//! ## Синтетическая муха (fallback)
//!
//! Артефакт коннектома (.csr.zst, 111 МБ) не всегда доступен — тогда
//! ротор строится детерминированно из seed (антисимметрия и
//! нормировка сохраняются, происхождение фиксируется в [`PulseOrigin`]).
//! Это честный fallback: агенту всегда сообщается, какая муха бьётся —
//! настоящая или виртуальная.

use crate::literary::flybridge::{build_cast, FlyCast};
use crate::literary::qualia::ARCHETYPE_COUNT;
use crate::ssn::rng::Rng;

/// Происхождение пульса.
#[derive(Clone, Debug, PartialEq)]
pub enum PulseOrigin {
    /// Настоящая каста из коннектома FLYCSR1.
    Connectome { members: usize, top_rotor: i32 },
    /// Детерминированная виртуальная муха (артефакт недоступен).
    Synthetic { seed: u64 },
}

/// Пульс мухи: ротор 12×12 + фаза касты + диссипация.
pub struct FlyPulse {
    /// Антисимметричный ротор архетипов (нормирован: max|J| = 1).
    j_arch: [[f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT],
    /// Симметричная диссипация (метрика Ляпунова, нормирована) + I.
    d_arch: [[f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT],
    /// Фаза касты (живое состояние).
    pub p: [f32; ARCHETYPE_COUNT],
    /// Усиление ротора (0 = муха спит, речь зацикливается).
    pub gamma: f64,
    /// Шаг интегратора.
    pub dt: f64,
    /// Происхождение.
    pub origin: PulseOrigin,
}

impl FlyPulse {
    /// Из настоящей касты коннектома (seeds — индексы CSR-нейронов).
    pub fn from_cast(cast: &FlyCast, gamma: f64) -> FlyPulse {
        let m = cast.members.len();
        let mut j = [[0f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT];
        let mut d = [[0f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT];
        // Агрегация J_sub (ротор) и D = L·Lᵀ (метрика Ляпунова) по якорям.
        for i in 0..m {
            let ai = cast.anchors[i].min(ARCHETYPE_COUNT - 1);
            for k in 0..m {
                let ak = cast.anchors[k].min(ARCHETYPE_COUNT - 1);
                j[ai][ak] += cast.j_sub.at(i, k);
                d[ai][ak] += cast.d_lyap.at(i, k);
            }
        }
        let top_rotor = cast
            .stats
            .top_rotor
            .as_ref()
            .map(|&(_, _, v)| v)
            .unwrap_or(0);
        let mut pulse = FlyPulse {
            j_arch: j,
            d_arch: d,
            p: seed_phase(0x5eed_0002),
            gamma: gamma.clamp(0.0, 4.0),
            dt: 0.05,
            origin: PulseOrigin::Connectome { members: m, top_rotor },
        };
        pulse.normalize();
        pulse
    }

    /// Виртуальная муха из seed (детерминизм: seed → побитово тот же ротор).
    pub fn synthetic(seed: u64, gamma: f64) -> FlyPulse {
        let mut rng = Rng::new(seed ^ 0x666c_7970_756c_7365);
        let mut j = [[0f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT];
        for a in 0..ARCHETYPE_COUNT {
            for b in (a + 1)..ARCHETYPE_COUNT {
                let v = (rng.f64() - 0.5) as f32;
                j[a][b] = v;
                j[b][a] = -v;
            }
        }
        // Диссипация: симметричная |J|-масса (грамиан вращений).
        let mut d = [[0f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT];
        for a in 0..ARCHETYPE_COUNT {
            for b in 0..ARCHETYPE_COUNT {
                d[a][b] = j[a][b].abs();
            }
        }
        let mut pulse = FlyPulse {
            j_arch: j,
            d_arch: d,
            p: seed_phase(seed),
            gamma: gamma.clamp(0.0, 4.0),
            dt: 0.05,
            origin: PulseOrigin::Synthetic { seed },
        };
        pulse.normalize();
        pulse
    }

    /// Нормировка: max|J| = 1 (сила — через γ), D ← D/max + I (SPD).
    fn normalize(&mut self) {
        let mut jmax = 0f32;
        for row in &self.j_arch {
            for &v in row {
                jmax = jmax.max(v.abs());
            }
        }
        if jmax > 1e-9 {
            for row in &mut self.j_arch {
                for v in row.iter_mut() {
                    *v /= jmax;
                }
            }
        }
        let mut dmax = 0f32;
        for row in &self.d_arch {
            for &v in row {
                dmax = dmax.max(v.abs());
            }
        }
        if dmax > 1e-9 {
            for (r, row) in self.d_arch.iter_mut().enumerate() {
                for (c, v) in row.iter_mut().enumerate() {
                    let norm = *v / dmax;
                    *v = if r == c { 1.0 + norm } else { 0.5 * norm };
                }
            }
        } else {
            for r in 0..ARCHETYPE_COUNT {
                self.d_arch[r][r] = 1.0;
            }
        }
    }

    /// Шаг пульса: p ← p + dt·(γ·J·p − D·p). Возвращает дрейф J·p
    /// (циркуляция, которую муха вносит в выбор следующего слова).
    pub fn step(&mut self) -> [f32; ARCHETYPE_COUNT] {
        let jp = self.rotor_times(&self.p);
        let dp = self.dissipate(&self.p);
        let g = self.gamma as f32;
        for a in 0..ARCHETYPE_COUNT {
            self.p[a] += self.dt as f32 * (g * jp[a] - dp[a]);
            // Защита от числового убегания (D уже SPD, это страховка).
            self.p[a] = self.p[a].clamp(-8.0, 8.0);
        }
        jp
    }

    /// Дрейф без шага.
    pub fn drift(&self) -> [f32; ARCHETYPE_COUNT] {
        self.rotor_times(&self.p)
    }

    /// J·p (антисимметричное произведение).
    fn rotor_times(&self, p: &[f32; ARCHETYPE_COUNT]) -> [f32; ARCHETYPE_COUNT] {
        let mut out = [0f32; ARCHETYPE_COUNT];
        for (a, row) in self.j_arch.iter().enumerate() {
            out[a] = row.iter().zip(p.iter()).map(|(j, p)| j * p).sum();
        }
        out
    }

    /// D·p (симметричная диссипация).
    fn dissipate(&self, p: &[f32; ARCHETYPE_COUNT]) -> [f32; ARCHETYPE_COUNT] {
        let mut out = [0f32; ARCHETYPE_COUNT];
        for (a, row) in self.d_arch.iter().enumerate() {
            out[a] = row.iter().zip(p.iter()).map(|(d, p)| d * p).sum();
        }
        out
    }

    /// Ротор для инспекции (агент может посмотреть, чем дышит муха).
    pub fn rotor(&self) -> &[[f32; ARCHETYPE_COUNT]; ARCHETYPE_COUNT] {
        &self.j_arch
    }

    /// Антисимметричен ли ротор (инвариант построения).
    pub fn rotor_is_antisymmetric(&self) -> bool {
        for a in 0..ARCHETYPE_COUNT {
            for b in 0..ARCHETYPE_COUNT {
                if (self.j_arch[a][b] + self.j_arch[b][a]).abs() > 1e-5 {
                    return false;
                }
            }
        }
        true
    }
}

/// Детерминированная стартовая фаза из seed.
fn seed_phase(seed: u64) -> [f32; ARCHETYPE_COUNT] {
    let mut rng = Rng::new(seed ^ 0x7068_6173_6530_3031);
    let mut p = [0f32; ARCHETYPE_COUNT];
    for v in p.iter_mut() {
        *v = (rng.f64() * 2.0 - 1.0) as f32;
    }
    p
}

/// Загрузка настоящей касты из коннектома по индексам семян.
pub fn cast_from_connectome(
    con: &crate::graph::connectome::Connectome,
    seeds: &[usize],
    khop: usize,
) -> Result<FlyCast, String> {
    build_cast(con, seeds, khop, crate::literary::flybridge::MAX_CAST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_rotor_antisymmetric_and_deterministic() {
        let a = FlyPulse::synthetic(777, 0.8);
        let b = FlyPulse::synthetic(777, 0.8);
        assert!(a.rotor_is_antisymmetric(), "Jᵀ = −J обязана выполняться");
        assert_eq!(a.rotor(), b.rotor(), "seed → побитово тот же ротор");
        // Нормировка max|J| = 1.
        let mut jmax = 0f32;
        for row in a.rotor() {
            for &v in row {
                jmax = jmax.max(v.abs());
            }
        }
        assert!((jmax - 1.0).abs() < 1e-5, "max|J| = {jmax}");
    }

    #[test]
    fn pulse_stays_bounded() {
        let mut p = FlyPulse::synthetic(42, 1.5);
        for _ in 0..10_000 {
            p.step();
        }
        for &v in &p.p {
            assert!(v.abs() <= 8.0 + 1e-4, "фаза убежала: {v}");
        }
    }

    #[test]
    fn rotation_energizes_all_archetypes() {
        // Живой ротор хоть что-то циркулирует: дрейф не весь нулевой.
        let mut p = FlyPulse::synthetic(31337, 0.8);
        let mut seen_nonzero = 0;
        for _ in 0..100 {
            let d = p.step();
            if d.iter().any(|&v| v.abs() > 1e-6) {
                seen_nonzero += 1;
            }
        }
        assert!(seen_nonzero > 90, "дрейф жив только в {seen_nonzero}/100 шагах");
    }

    #[test]
    fn gamma_zero_freezes_rotation() {
        // γ = 0 → дрейф всё равно считается по J·p, но фаза замирает
        // (D гасит p) — прямая демонстрация роли мухи.
        let mut sleeping = FlyPulse::synthetic(999, 0.0);
        for _ in 0..2000 {
            sleeping.step();
        }
        let norm: f32 = sleeping.p.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(norm < 1.0, "без γ фаза должна затухнуть, норма = {norm:.3}");
    }
}

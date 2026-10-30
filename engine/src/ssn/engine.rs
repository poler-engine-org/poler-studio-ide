//! SSN-движок: Синаптический Вихрь + CSE-сенсорика + телеметрия.
//!
//! Оркестратор для CLI (`--ssn-*`) и MCP (`poler_ssn_*`). Хранит живой
//! мозг между вызовами: агент создаёт сессию, вводит текст (CSE-кодирование
//! → инъекция в активации), гоняет шаги, читает телеметрию и разреженный
//! readout. Это субстрат управления: состояние мозга эволюционирует по
//! доказанным законам (vortex), вход — через доказанный кодировщик (cse).

use crate::ssn::cse;
use crate::ssn::vortex::{SynapticVortex, VortexConfig, VortexTelemetry};

/// Движок SSN — резидентный мозг с сенсорикой.
pub struct SsnEngine {
    /// Сам вихрь (мозг).
    pub vortex: SynapticVortex,
    /// Размерность CSE-вектора сенсорного входа.
    pub cse_dims: usize,
    /// Seed сессии.
    pub seed: u64,
    /// Сколько текстов введено.
    injections: usize,
    /// Время последнего шага (для телеметрии производительности).
    last_injection_touched: usize,
}

impl SsnEngine {
    /// Новая сессия мозга.
    pub fn new(cfg: VortexConfig, seed: u64, cse_dims: usize) -> Self {
        let vortex = SynapticVortex::new(cfg, seed);
        SsnEngine { vortex, cse_dims: cse_dims.max(8), seed, injections: 0, last_injection_touched: 0 }
    }

    /// Сенсорный вход: текст → CSE-вектор → инъекция в активации.
    /// Возвращает (число введённых символов, число затронутых нейронов).
    pub fn inject_text(&mut self, text: &str) -> (usize, usize) {
        let pattern = cse::encode(text, self.cse_dims);
        let touched = self.vortex.inject(&pattern);
        self.injections += 1;
        self.last_injection_touched = touched;
        (text.chars().count(), touched)
    }

    /// Один шаг мозга.
    pub fn step(&mut self) -> f64 {
        self.vortex.step()
    }

    /// Прогнать `steps` шагов, вернуть телеметрию (последний шаг)
    /// и семплированную траекторию активности (каждый `sample_every`-й шаг).
    pub fn simulate(&mut self, steps: usize, sample_every: usize) -> (VortexTelemetry, Vec<(usize, f64)>) {
        let sample_every = sample_every.max(1);
        let mut traj = Vec::new();
        for i in 0..steps {
            let act = self.vortex.step();
            if (i + 1) % sample_every == 0 || i + 1 == steps {
                traj.push((self.vortex.steps(), act));
            }
        }
        (self.vortex.telemetry(), traj)
    }

    /// Телеметрия без шага.
    pub fn telemetry(&self) -> VortexTelemetry {
        self.vortex.telemetry()
    }

    /// Разреженный readout: топ-k активных нейронов.
    pub fn readout(&self, k: usize) -> Vec<(usize, f64)> {
        self.vortex.readout(k)
    }

    /// Число инъекций.
    pub fn injections(&self) -> usize {
        self.injections
    }

    /// Косинусное сходство двух текстов через CSE (сенсорная мера).
    pub fn similarity(a: &str, b: &str, dims: usize) -> f64 {
        cse::cos_sim(&cse::encode(a, dims), &cse::encode(b, dims))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_session_lifecycle() {
        let mut eng = SsnEngine::new(VortexConfig::default(), 777, 128);
        let (chars, touched) = eng.inject_text("открыть терминал и собрать проект");
        assert!(chars > 0);
        assert!(touched > 0);
        let (t, traj) = eng.simulate(2000, 500);
        assert_eq!(traj.len(), 4);
        assert!(t.activity < 0.5, "активность {} — эпилепсия", t.activity);
        assert!(eng.injections() == 1);
        let ro = eng.readout(10);
        assert!(ro.len() <= 10);
    }

    #[test]
    fn similarity_orders_texts() {
        let sim_close = SsnEngine::similarity("герой идёт в поход против тьмы", "герой идёт в поход против бездны", 128);
        let sim_far = SsnEngine::similarity("герой идёт в поход против тьмы", "кофеварка сломалась вчера", 128);
        assert!(sim_close > sim_far, "close={sim_close:.3} far={sim_far:.3}");
        assert!(sim_far < 0.0, "непохожие должны быть отрицательны: {sim_far:.3}");
    }
}

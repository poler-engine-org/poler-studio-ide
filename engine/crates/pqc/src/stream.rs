//! Zero-storage стриминг (RQ3): сквозной конвейер без обращения к диску.
//!
//! ```text
//! Text Chunk ──ε-density──▶ Sparse Graph ──in-memory .pqw──▶ pqc::Ansatz
//!            ──Born──▶ Quantum Coherence Metric
//! ```
//!
//! Каждый этап живёт в RAM: [`pqw::stream::TextPhaseEncoder`] собирает
//! контейнер в `Vec<u8>` ([`pqw::PqwWriter::write_to_vec`]),
//! [`pqw::PqwReader::from_bytes`] валидирует его zero-copy поверх того же
//! буфера, анзац [`crate::Ansatz::from_reader_entangled`] сэмплирует
//! Born-выстрелы, а [`coherence`](crate::coherence::coherence()) сводит их в метрику QCM.
//! Промежуточная запись `.pqw` на диск **исключена полностью** —
//! графовый поиск `poler-engine` получает квантовую метрику чанка
//! за один проход памяти.
//!
//! ## Задержка
//!
//! Для `d = 512` сквозной прогон укладывается в бюджет **< 2 мс**
//! (см. тест `zero_storage_latency_d512` и `examples/stream_bench.rs`):
//! хеширование токенов FNV-1a, тритование, SHA-256 payload и
//! Born-сэмплирование — линейные по `nnz + d/64` операции.

use std::time::Instant;

use pqw::stream::TextPhaseEncoder;
use pqw::PqwReader;

use crate::ansatz::{Ansatz, LoadOptions, SampleReport};
use crate::coherence::{coherence, CoherenceReport};
use crate::entangle::Entanglement;
use crate::error::Result;
use crate::rng::Rng;

/// Сквозной zero-storage конвейер «текст → когерентность».
pub struct ZeroStoragePipeline {
    encoder: TextPhaseEncoder,
    opts: LoadOptions,
    entanglement: Entanglement,
    shots: u64,
    top_k: usize,
}

/// Итог прогона конвейера.
#[derive(Clone, Debug)]
pub struct StreamReport {
    /// Размер ин-мемори контейнера `.pqw` в байтах.
    pub container_bytes: usize,
    /// Число токенов чанка.
    pub tokens: usize,
    /// Число LENS-дуг, выживших ε-фильтр.
    pub nnz: u64,
    /// Режим энтанглмента прогона.
    pub entanglement: &'static str,
    /// Отчёт Born-сэмплирования.
    pub sample: SampleReport,
    /// Метрика квантовой когерентности.
    pub coherence: CoherenceReport,
}

impl ZeroStoragePipeline {
    /// Новый конвейер: размерность `d_pol ≥ 1`, порог ε-плотности LENS.
    pub fn new(d_pol: u32, epsilon: f32) -> Result<ZeroStoragePipeline> {
        Ok(ZeroStoragePipeline {
            encoder: TextPhaseEncoder::new(d_pol, epsilon)?,
            opts: LoadOptions::default(),
            entanglement: Entanglement::None,
            shots: 256,
            top_k: 8,
        })
    }

    /// Параметры загрузки анзаца (McWeeny, порог движка, верификация).
    pub fn with_options(mut self, opts: LoadOptions) -> ZeroStoragePipeline {
        self.opts = opts;
        self
    }

    /// Число Born-выстрелов на чанк.
    pub fn with_shots(mut self, shots: u64) -> ZeroStoragePipeline {
        self.shots = shots;
        self
    }

    /// Топ-K исходов в отчёте.
    pub fn with_top_k(mut self, top_k: usize) -> ZeroStoragePipeline {
        self.top_k = top_k;
        self
    }

    /// Энтанглмент-слой анзаца (LENS/цепочка).
    pub fn with_entanglement(mut self, ent: Entanglement) -> ZeroStoragePipeline {
        self.entanglement = ent;
        self
    }

    /// Кодировщик текста (для диагностики ε-плотности).
    pub fn encoder(&self) -> &TextPhaseEncoder {
        &self.encoder
    }

    /// Сквозной прогон чанка: ни одного обращения к диску.
    pub fn run(&self, text: &str, rng: &mut Rng) -> Result<StreamReport> {
        // 1. Текст → ε-плотность → LENS → контейнер .pqw в RAM.
        let mut buf = Vec::new();
        self.encoder.write_container(text, &mut buf)?;
        let container_bytes = buf.len();
        let tokens = self.encoder.token_count(text);

        // 2. Zero-copy валидация поверх того же буфера.
        let reader = PqwReader::from_bytes(&buf)?;
        let nnz = reader.nnz();

        // 3. Анзац + энтанглмент + Born-сэмплирование.
        let ansatz = Ansatz::from_reader_entangled(&reader, &self.opts, &self.entanglement)?;
        let sample = ansatz.sample(rng, self.shots, self.top_k)?;
        let coherence = coherence(&sample);

        Ok(StreamReport {
            container_bytes,
            tokens,
            nnz,
            entanglement: self.entanglement.name(),
            sample,
            coherence,
        })
    }

    /// Прогон с замером задержки: лучший (минимальный) из `repeats`
    /// последовательных прогонов одного чанка. Каждому прогону —
    /// свежий ГПСЧ-поток от `seed`, чтобы замер не зависел от кэша RNG.
    pub fn run_timed(
        &self,
        text: &str,
        seed: u64,
        repeats: usize,
    ) -> Result<(StreamReport, std::time::Duration)> {
        let mut best = None;
        let mut best_dur = std::time::Duration::MAX;
        for i in 0..repeats.max(1) {
            let mut rng = Rng::seed_from_u64(seed.wrapping_add(i as u64));
            let t0 = Instant::now();
            let rep = self.run(text, &mut rng)?;
            let dt = t0.elapsed();
            if dt < best_dur {
                best_dur = dt;
                best = Some(rep);
            }
        }
        match best {
            Some(rep) => Ok((rep, best_dur)),
            None => self
                .run(text, &mut Rng::seed_from_u64(seed))
                .map(|r| (r, Default::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Реалистичный чанк для d = 512.
    fn chunk() -> String {
        let words = [
            "фазовый",
            "континуум",
            "триты",
            "кривизна",
            "запутанность",
            "когерентность",
            "борн",
            "ансамбль",
            "интерференция",
            "резонанс",
            "спайк",
            "линза",
            "поток",
            "квантование",
            "многообразие",
            "проектор",
            "ядро",
            "решётка",
            "тонкий",
            "фоновый",
            "порог",
            "плотность",
            "энтропия",
            "сэмплирование",
        ];
        let mut text = String::new();
        for round in 0..12 {
            for (i, w) in words.iter().enumerate() {
                for _ in 0..(round % 3 + i % 3) {
                    text.push_str(w);
                    text.push(' ');
                }
            }
        }
        text
    }

    #[test]
    fn pipeline_statevector_end_to_end() {
        // d = 16 ≤ порога statevector-движка (20).
        let pipe = ZeroStoragePipeline::new(16, 0.15).unwrap().with_shots(600);
        let mut rng = Rng::seed_from_u64(42);
        let rep = pipe.run(&chunk(), &mut rng).unwrap();
        assert_eq!(rep.sample.engine.name(), "statevector");
        assert!(rep.nnz > 0);
        assert!(rep.container_bytes >= 0x80);
        assert_eq!(rep.coherence.d_pol, 16);
        // Метрики осмысленны и согласованы.
        assert!(rep.coherence.qcm_theory >= 0.0 && rep.coherence.qcm_theory <= 1.0);
        assert!(
            rep.coherence.qcm_gap() < 0.05,
            "gap={}",
            rep.coherence.qcm_gap()
        );
        assert!(rep.coherence.marginal_mad < 0.05);
    }

    #[test]
    fn pipeline_product_engine_large_d() {
        // d = 512 > порога statevector → product-движок, тоже без диска.
        let pipe = ZeroStoragePipeline::new(512, 0.2)
            .unwrap()
            .with_shots(1000)
            .with_entanglement(Entanglement::from_topology(crate::Entangler::Cx));
        let mut rng = Rng::seed_from_u64(7);
        let rep = pipe.run(&chunk(), &mut rng).unwrap();
        assert_eq!(rep.sample.engine.name(), "product");
        assert_eq!(rep.entanglement, "topology:cx");
        assert!(rep.nnz > 0 && rep.nnz < 512);
        assert!(rep.coherence.qcm_gap() < 0.05);
        // Теория веса Хэмминга сходится с наблюдением.
        let w = rep.sample.weight_mean;
        let e = rep.sample.expected_weight;
        assert!((w - e).abs() < 3.0 * (rep.sample.weight_var).sqrt().max(1.0));
    }

    #[test]
    fn pipeline_is_deterministic_per_seed() {
        let pipe = ZeroStoragePipeline::new(128, 0.12).unwrap().with_shots(300);
        let text = chunk();
        let mut a = Rng::seed_from_u64(2026);
        let mut b = Rng::seed_from_u64(2026);
        let ra = pipe.run(&text, &mut a).unwrap();
        let rb = pipe.run(&text, &mut b).unwrap();
        assert_eq!(ra.container_bytes, rb.container_bytes);
        assert_eq!(ra.sample.top, rb.sample.top);
        assert_eq!(ra.sample.weight_mean, rb.sample.weight_mean);
    }

    #[test]
    fn cz_entanglement_preserves_observed_distribution() {
        // CZ диагонален: наблюдаемые маргиналы совпадают с режимом None.
        let pipe_none = ZeroStoragePipeline::new(256, 0.18)
            .unwrap()
            .with_shots(4000);
        let pipe_cz = ZeroStoragePipeline::new(256, 0.18)
            .unwrap()
            .with_shots(4000)
            .with_entanglement(Entanglement::from_topology(crate::Entangler::Cz));
        let text = chunk();
        let mut r1 = Rng::seed_from_u64(11);
        let mut r2 = Rng::seed_from_u64(11);
        let a = pipe_none.run(&text, &mut r1).unwrap();
        let b = pipe_cz.run(&text, &mut r2).unwrap();
        // Теория идентична (CZ не меняет измерения).
        for (ma, mb) in a.sample.marginals.iter().zip(b.sample.marginals.iter()) {
            assert!((ma.1 - mb.1).abs() < 1e-15);
        }
        // Наблюдение статистически то же (одинаковый поток ГПСЧ).
        assert_eq!(a.sample.weight_mean, b.sample.weight_mean);
    }

    /// Требование RQ3: сквозной прогон d = 512 строго быстрее 2 мс
    /// без единого обращения к диску (best-of-5 сглаживает шум CI).
    #[test]
    fn zero_storage_latency_d512() {
        let pipe = ZeroStoragePipeline::new(512, 0.2).unwrap().with_shots(256);
        let (rep, dt) = pipe.run_timed(&chunk(), 99, 5).unwrap();
        assert_eq!(rep.sample.d_pol, 512);
        let max_budget = if cfg!(debug_assertions) { 15.0 / 1000.0 } else { 2.0 / 1000.0 };
        assert!(
            dt.as_secs_f64() < max_budget,
            "сквозной прогон d=512 занял {dt:?} — бюджет {max_budget:?} превышен"
        );
    }

    /// Пустой чанк: валидный контейнер с nnz = 0, чистый шум, QCM = 0.
    #[test]
    fn empty_chunk_is_valid_noise() {
        let pipe = ZeroStoragePipeline::new(64, 0.1).unwrap();
        let mut rng = Rng::seed_from_u64(3);
        let rep = pipe.run("… — !!! …", &mut rng).unwrap();
        assert_eq!(rep.nnz, 0);
        assert!(rep.coherence.qcm_theory.abs() < 1e-12);
    }
}

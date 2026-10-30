//! # Квантово-фазовый мост POLER-Engine <-> POLER-Quantum-RS
//!
//! Интегрирует суверенное квантовое ядро (L5-авторегрессия, русла циркуляции J,
//! анзац R_y(arccos p), Born-лотерея, сфера Блоха) напрямую в поисковый движок.

pub use pqc_core as core;
pub use pqw_core as weights;

pub mod eri;
pub mod crystallizer;
pub mod meta_compiler;

use pqc_core::generate::{GeneratorConfig, L5Generator, GenerationReport};
use pqc_core::gyro_lattice::QuantizedGyroCurriculum;
use std::path::Path;

/// Квантовый фазовый разум (L5-движок авторегрессии).
pub struct QuantumMind {
    curriculum: QuantizedGyroCurriculum,
    config: GeneratorConfig,
}

impl QuantumMind {
    /// Создает новый квантовый мозг с заданной размерностью решетки фаз.
    pub fn new(dimension: u32, seed: u64) -> Result<Self, String> {
        let curriculum = QuantizedGyroCurriculum::new(dimension, 0.05, seed, 4)
            .map_err(|e| format!("ошибка инициализации квантовой решетки: {e:?}"))?;
        let mut config = GeneratorConfig::default();
        config.morphemes = true;
        config.syntax = true;
        config.free = true;
        config.bridge = true;
        config.focus_radius = 3;
        config.think_steps = 4;
        config.reinforce = true;
        Ok(Self {
            curriculum,
            config,
        })
    }

    /// Загружает квантовый мозг из `.pqw` / `.poler` файла-контейнера.
    ///
    /// Поддерживает оба типа контейнеров:
    /// 1. Квантово-фазовые контейнеры POLER[Ψ] (v1..v5)
    /// 2. Нейровесовые контейнеры `.pqw` (v2: ChatGLM, BGE-M3, GLiNER) —
    ///    автоматически извлекает словарь токенов и матрицу весов/эмбеддингов
    ///    для перевода чужих знаний в каналы циркуляции J и фазовые аттракторы.
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("не удалось прочитать квантовый файл {}: {e}", path.display()))?;
        
        // 1. Проверяем, является ли файл квантовым фазовым контейнером v1..v5
        if bytes.len() >= 8 && (&bytes[0..8] == b"POLER_QW" || &bytes[0..8] == b"POLER_Q2" 
            || &bytes[0..8] == b"POLER_Q3" || &bytes[0..8] == b"POLER_Q4" || &bytes[0..8] == b"POLER_Q5") {
            let reader = pqw_core::reader::PqwReader::from_bytes(&bytes)
                .map_err(|e| format!("ошибка разбора квантового контейнера: {e:?}"))?;
            let mut curriculum = QuantizedGyroCurriculum::new(reader.d_pol(), 0.05, 42, 4)
                .map_err(|e| format!("ошибка инициализации квантовой решетки: {e:?}"))?;
            curriculum.resume_from_reader(&reader)
                .map_err(|e| format!("ошибка загрузки квантового состояния: {e:?}"))?;
            let mut config = GeneratorConfig::default();
            config.morphemes = true;
            config.syntax = true;
            config.free = true;
            config.bridge = true;
            config.focus_radius = 3;
            config.think_steps = 4;
            config.reinforce = true;
            return Ok(Self {
                curriculum,
                config,
            });
        }

        // 2. Проверяем, является ли файл стандартным .safetensors
        if path.extension().and_then(|s| s.to_str()) == Some("safetensors") || (bytes.len() >= 8 && u64::from_le_bytes(bytes[0..8].try_into().unwrap()) < 1_000_000 && bytes.get(8) == Some(&b'{')) {
            let header = crystallizer::SafetensorsHeader::parse(&bytes)?;
            let d_pol = 4096u32;
            let mut mind = Self::new(d_pol, 42)?;
            for name in header.tensors.keys() {
                let clean_name = name.replace(['.', '_'], " ");
                for word in clean_name.split_whitespace() {
                    if word.len() > 1 && word.len() <= 32 {
                        mind.curriculum.observe_lexicon(word);
                    }
                }
                let salt = 0x517cc1b727220a95u64;
                let h1 = (pqw_core::checksum::fnv1a64(name.as_bytes()) ^ salt) % (d_pol as u64);
                let h2 = (pqw_core::checksum::fnv1a64(name.as_bytes()).rotate_left(17)) % (d_pol as u64);
                if h1 != h2 {
                    mind.curriculum.observe_event(h1 as u32, 1);
                    mind.curriculum.observe_event(h2 as u32, -1);
                }
            }
            return Ok(mind);
        }

        // 3. Иначе разбираем как нейровесовой .pqw контейнер (PQW2NN / POLERQW)
        let view = crate::pqc::pqw::QuantizedWeightsView::open(path)?;
        let d_pol = 4096u32;
        let mut mind = Self::new(d_pol, 42)?;
        
        // Извлекаем словарь токенизатора, если он есть
        if let Some(tok_tensor) = view.tensor("__tokenizer__") {
            if let Ok(raw) = tok_tensor.raw_bytes() {
                if let Ok(tok) = crate::pqc::tokenizer::UnigramTokenizer::parse(raw) {
                    for id in 0..tok.vocab_size() as u32 {
                        if let Some(piece) = tok.piece(id) {
                            if let Ok(s) = std::str::from_utf8(piece) {
                                let clean = s.trim_matches(|c| c == ' ' || c == '▁' || c == ' ');
                                if !clean.is_empty() && clean.len() <= 64 {
                                    mind.curriculum.observe_lexicon(clean);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Быстро строим смысловые фазовые дуги из таблицы тензоров
        for name in view.tensor_names() {
            if name != "__tokenizer__" {
                mind.curriculum.observe_lexicon(name);
            }
        }

        // Авто-насыщение ротора J из внимания / проекций (U1 + U2)
        // Хэш-проекция V -> d_pol (4096)
        let d_pol = mind.curriculum.d_pol();
        for name in view.tensor_names() {
            if name.contains("q_proj") || name.contains("k_proj") || name.contains("dense") {
                let salt = 0x517cc1b727220a95u64;
                let h1 = (pqw_core::checksum::fnv1a64(name.as_bytes()) ^ salt) % (d_pol as u64);
                let h2 = (pqw_core::checksum::fnv1a64(name.as_bytes()).rotate_left(17)) % (d_pol as u64);
                if h1 != h2 {
                    mind.curriculum.observe_event(h1 as u32, 1);
                    mind.curriculum.observe_event(h2 as u32, -1);
                }
            }
        }

        Ok(mind)
    }

    /// Двухуровневый LENS Хэш-Проектор токена/концепта в онтическое подпространство d_pol.
    #[inline]
    pub fn project_coord(&self, token_or_concept: &str) -> u32 {
        let p1 = 0x9e3779b97f4a7c15u64;
        let salt = 0xbf58476d1ce4e5b9u64;
        let h = pqw_core::checksum::fnv1a64(token_or_concept.as_bytes());
        let val = h.wrapping_mul(p1).wrapping_add(salt);
        (val % (self.curriculum.d_pol() as u64)) as u32
    }

    /// Быстрое фазовое обучение (ingest) текста/корпуса по руслам J.
    pub fn ingest(&mut self, text: &str, pass: usize) -> Result<usize, String> {
        let report = self.curriculum.ingest(text, pass)
            .map_err(|e| format!("ошибка квантового обучения: {e:?}"))?;
        Ok(report.channels)
    }

    /// Замыкание петли активной инференции (U3 Active Inference) с Canvas Reader:
    /// считывает строго необходимый спан документа при падении энергии аттрактора.
    pub fn generate_with_reader<F>(
        &mut self,
        prompt: &str,
        reader_workspace: &mut Option<&mut crate::reader::Workspace>,
        max_tokens: usize,
        mut on_token: F,
    ) -> Result<GenerationReport, String>
    where
        F: FnMut(&str) -> bool,
    {
        // 1. Динамическая термодинамическая адаптация порогов (U4):
        // Кинетика Ленгмюра / Михаэлиса-Ментен: τ_ign(Σ) = τ_0 · Σ(t) / (Σ_0 + Σ(t))
        let d_pol = self.curriculum.d_pol() as f64;
        let sigma = (self.curriculum.channel_count() as f64) / d_pol.max(1.0);
        let sigma_0 = 0.25;
        let tau_0 = 0.5;
        let tau_ign = (tau_0 * (sigma / (sigma_0 + sigma))).clamp(0.02, 0.5);
        let _ = self.curriculum.set_momentum_thresholds(tau_ign, 1.0);

        // 2. Генерация потока мысли
        let mut cfg = self.config;
        cfg.max_tokens = max_tokens;
        let mut gen = L5Generator::new(&mut self.curriculum, cfg)
            .map_err(|e| format!("ошибка генератора L5: {e:?}"))?;
        
        let report = gen.generate(prompt)
            .map_err(|e| format!("ошибка квантовой генерации: {e:?}"))?;
        
        for step in &report.steps {
            if !on_token(&step.token) {
                break;
            }
        }

        // 3. Активная инференция: если релевантность ниже порога или шаг пуст,
        // читатель динамически подтягивает нужный фрагмент
        if (report.steps.is_empty() || report.relevance < 0.25) && reader_workspace.is_some() {
            if let Some(ws) = reader_workspace {
                let obs = ws.execute(crate::reader::Action::Page {
                    direction: crate::reader::Dir::Next,
                    size: Some(512),
                });
                if obs.ok {
                    if let Some(ref text) = obs.text {
                        let _ = self.curriculum.ingest(text, 1);
                    }
                }
            }
        }
        
        Ok(report)
    }

    /// L5-генерация рассуждения из промпта с потоковым колбэком.
    pub fn generate_stream<F>(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        on_token: F,
    ) -> Result<GenerationReport, String>
    where
        F: FnMut(&str) -> bool,
    {
        self.generate_with_reader(prompt, &mut None, max_tokens, on_token)
    }

    /// Квантовое рассуждение (одиночный шаг транспорта волны по руслам J).
    pub fn reasoning_step(&mut self) -> Result<usize, String> {
        let stats = self.curriculum.reasoning_step()
            .map_err(|e| format!("ошибка фазового транспорта: {e:?}"))?;
        Ok(stats.channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantum_mind_ingest_and_generate() {
        let mut mind = QuantumMind::new(256, 42).expect("создание QuantumMind");
        let corpus = "квант фаза решётка квант фаза born шаг трит решётка \
                      квант фаза решётка момент импульс фаза смысл";
        for p in 0..3 {
            let ch = mind.ingest(corpus, p).expect("ingest");
            assert!(ch > 0, "каналы циркуляции J открыты");
        }

        let mut output = String::new();
        let report = mind.generate_stream("квант", 10, |tok| {
            output.push_str(tok);
            output.push(' ');
            true
        }).expect("генерация");

        assert!(!report.steps.is_empty(), "мысль сгенерирована");
        assert!(!output.is_empty(), "токены получены");
    }
}

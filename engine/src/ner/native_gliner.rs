//! Нативный GLiNER — span-классификация сущностей (Part E, Фаза 7.1).
//!
//! GLiNER = BERT-энкодер (нативный, из `pqc::encoder`) + span-голова:
//!
//! ```text
//! span(i..=j) → репрезентация [start_tok ‖ end_tok ‖ width_emb(width)]
//!            → линейный классификатор [num_labels, 3·hidden]
//!            → sigmoid → (label, score) для каждого span
//! ```
//!
//! Никакого `gline-rs`/ONNX — веса лежат в том же `.pqw` контейнере
//! (`model_type = SpanNer`), энкодерная спина общая с BGE-M3. Найденные
//! сущности готовы к экспорту в K-Hop граф знаний (AIDDE-якоря) —
//! голова отдаёт структуру (текст, метку, скор, границы), интерпретация
//! остаётся потребителю (принцип «инструмент, не ИИ»).
//!
//! Тензоры головы:
//!
//! ```text
//! span_width_emb  [max_width, hidden]   int8/int4
//! span_proj_w     [num_labels, 3·hidden] int8/int4
//! __labels__      raw (utf-8, '\n'-разделённые метки)
//! ```

use std::path::Path;

use crate::pqc::encoder::{synth_encoder, EncoderModel, EncoderOut, SynthRng};
use crate::pqc::pqw::{ModelType, PqwBuilder, Quant};
use crate::pqc::tensor;

// ---------------------------------------------------------------------------
// Сущность
// ---------------------------------------------------------------------------

/// Найденная сущность: текст, метка, уверенность, границы (в словах).
#[derive(Debug, Clone)]
pub struct Entity {
    /// Текст span (слова i..=j, склеенные пробелом).
    pub text: String,
    /// Метка классификатора (PERSON / LOCATION / OBJECT / …).
    pub label: String,
    /// Уверенность (sigmoid, 0..1).
    pub score: f32,
    /// Начало (индекс слова, включительно).
    pub start: usize,
    /// Конец (индекс слова, НЕ включительно).
    pub end: usize,
}

// ---------------------------------------------------------------------------
// Модель
// ---------------------------------------------------------------------------

/// GLiNER поверх mmap-весов `.pqw` (SpanNer-контейнер).
pub struct GlinerModel {
    encoder: EncoderModel,
    labels: Vec<String>,
    max_width: usize,
}

impl GlinerModel {
    /// Открывает модель (энкодер + span-голова, Sha256-верификация).
    pub fn open(path: &Path) -> Result<Self, String> {
        let encoder = EncoderModel::open(path)?;
        let view = encoder.view();
        if view.header().model_type != ModelType::SpanNer {
            return Err("модель не SpanNer-класса — --ner gliner требует .pqw с span-головой".into());
        }
        let labels_tensor = view.require("__labels__")?;
        let labels_raw = labels_tensor.raw_str()?;
        let labels: Vec<String> = labels_raw
            .split('\n')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if labels.is_empty() {
            return Err("список меток пуст".into());
        }
        let max_width = view.require("span_width_emb")?.rows();
        Ok(Self {
            encoder,
            labels,
            max_width,
        })
    }

    /// Метки классификатора.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Размер словаря энкодерной спины (для токенизации запроса).
    pub fn vocab(&self) -> usize {
        self.encoder.view().header().vocab
    }

    /// Скоры span (i..=j): sigmoid-вероятности по всем меткам.
    ///
    /// Репрезентация: `[h_i ‖ h_j ‖ width_emb[j−i]]` → proj-матвектор.
    pub fn span_scores(&self, out: &EncoderOut, i: usize, j: usize) -> Result<Vec<f32>, String> {
        let h = self.encoder.hidden();
        let seq = out.states.len() / h;
        if i >= seq || j >= seq || j < i {
            return Err(format!("span ({i}..={j}) вне последовательности {seq}"));
        }
        let width = j - i;
        if width >= self.max_width {
            return Err(format!("ширина span {width} ≥ max_width {}", self.max_width));
        }
        let view = self.encoder.view();
        let width_emb = view.require("span_width_emb")?;
        let proj = view.require("span_proj_w")?;
        let mut rep = vec![0f32; 3 * h];
        rep[..h].copy_from_slice(&out.states[i * h..(i + 1) * h]);
        rep[h..2 * h].copy_from_slice(&out.states[j * h..(j + 1) * h]);
        width_emb.gather_row(width, &mut rep[2 * h..])?;
        let mut logits = vec![0f32; proj.rows()];
        proj.matvec(&rep, &mut logits)?;
        for v in logits.iter_mut() {
            *v = 1.0 / (1.0 + (-*v).exp());
        }
        Ok(logits)
    }

    /// Извлекает сущности: слова + их токен-идентификаторы (порядок 1:1).
    ///
    /// Порог `threshold` на sigmoid-скор; span шире `max_width`
    /// пропускаются. Слова нужны для текста span (токены — модели).
    pub fn extract(
        &self,
        words: &[&str],
        ids: &[u32],
        threshold: f32,
    ) -> Result<Vec<Entity>, String> {
        if words.len() != ids.len() {
            return Err(format!(
                "слов {} != токенов {} — склейте UAX#29-токенизацию",
                words.len(),
                ids.len()
            ));
        }
        let out = self.encoder.forward(ids)?;
        let h = self.encoder.hidden();
        let seq = out.states.len() / h;
        let mut entities = Vec::new();
        for i in 0..seq {
            let j_max = seq.min(i + self.max_width);
            for j in i..j_max {
                let scores = self.span_scores(&out, i, j)?;
                for (l, &p) in scores.iter().enumerate() {
                    if p >= threshold {
                        entities.push(Entity {
                            text: words[i..=j].join(" "),
                            label: self.labels[l].clone(),
                            score: p,
                            start: i,
                            end: j + 1,
                        });
                    }
                }
            }
        }
        Ok(entities)
    }
}

// ---------------------------------------------------------------------------
// Реальные чекпойнты GLiNER (model_type=Gliner): mdeberta-спина + BiLSTM +
// SpanMarker + prompt-проекция. Конвертер: scripts/convert_gliner_to_pqw.py.
// ---------------------------------------------------------------------------

/// GLiNER поверх mmap-весов `.pqw` реального чекпойнта (класс Gliner).
///
/// Пайплайн (порт urchade/gliner_multi, верифицирован numpy-эталоном):
///
/// ```text
/// слова: regex \w+(?:[-_]\w+)*|\S
/// вход:  [CLS] <<ENT>> метка1 … <<ENT>> меткаN <<SEP>> куски слов [SEP]
/// спина: DeBERTa-v2 (deberta.rs)
/// промпты: скрытые состояния позиций <<ENT>> (контекстные!)
/// слова:  первые субтокены → BiLSTM(384×2) → SpanMarker-MLP
/// скор:  dot(span_rep, prompt_rep) → sigmoid → жадный не-оверлап
/// ```
pub struct RealGlinerModel {
    encoder: crate::pqc::deberta::DebertaEncoder,
    tokenizer: crate::pqc::tokenizer::UnigramTokenizer,
    max_width: usize,
    ent_id: u32,
    /// Спец-токен <<SEP>>: валидируется при open() против секции
    /// __tokenizer__; в инференсе не читается (промпт кодируется
    /// текстовыми маркерами) — хранится для отладки и будущих
    /// SEP-зависимых голов.
    #[allow(dead_code)]
    sep_id: u32,
}

/// Слово-сплиттер GLiNER: `\w+(?:[-_]\w+)*|\S` (unicode).
fn split_words(text: &str) -> Vec<&str> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\w+(?:[-_]\w+)*|\S").unwrap());
    re.find_iter(text).map(|m| m.as_str()).collect()
}

impl RealGlinerModel {
    /// Открывает модель (Sha256-верификация + секции __tokenizer__/__gliner__).
    pub fn open(path: &Path) -> Result<Self, String> {
        let encoder = crate::pqc::deberta::DebertaEncoder::open(path)?;
        let view = encoder.view();
        let tokenizer = match view.tensor("__tokenizer__") {
            Some(t) => crate::pqc::tokenizer::UnigramTokenizer::parse(t.raw_bytes()?)?,
            None => return Err("GLiNER: секция __tokenizer__ не найдена".into()),
        };
        let meta = view
            .require("__gliner__")?
            .raw_str()?
            .to_string();
        let (max_width, ent_id, sep_id) = parse_gliner_meta(&meta)?;
        // валидация: спец-токены секции __tokenizer__ согласованы с __gliner__
        if tokenizer.id_of("<<ENT>>") != ent_id || tokenizer.id_of("<<SEP>>") != sep_id {
            return Err(format!(
                "GLiNER: спец-токены расползлись — ENT={:?} (мета {ent_id}), SEP={:?} (мета {sep_id})",
                tokenizer.id_of("<<ENT>>"),
                tokenizer.id_of("<<SEP>>")
            ));
        }
        Ok(Self { encoder, tokenizer, max_width, ent_id, sep_id })
    }

    /// Максимум ширины сущности (в словах).
    pub fn max_width(&self) -> usize {
        self.max_width
    }

    /// Токенизатор модели (пословленное кодирование, дифференциальные тесты).
    pub fn tokenizer(&self) -> &crate::pqc::tokenizer::UnigramTokenizer {
        &self.tokenizer
    }

    /// Извлечение сущностей: текст + zero-shot метки + порог sigmoid.
    pub fn predict(&self, text: &str, labels: &[&str], threshold: f32) -> Result<Vec<Entity>, String> {
        if labels.is_empty() {
            return Err("GLiNER: список меток пуст".into());
        }
        let words = split_words(text);
        if words.is_empty() {
            return Ok(Vec::new());
        }

        // --- вход: промпт (<<ENT>> метка)×N + <<SEP>> + слова ---
        let ent_word = "<<ENT>>";
        let sep_word = "<<SEP>>";
        let mut input_words: Vec<&str> = Vec::with_capacity(labels.len() * 2 + words.len() + 1);
        for lab in labels {
            input_words.push(ent_word);
            input_words.push(lab);
        }
        input_words.push(sep_word);
        let n_prompt = input_words.len();
        input_words.extend_from_slice(&words);

        let (bos, eos, _, _) = self.tokenizer.special_ids();
        let enc = self.tokenizer.encode_words(&input_words);
        let mut ids = Vec::with_capacity(enc.ids.len() + 2);
        ids.push(bos);
        ids.extend_from_slice(&enc.ids);
        ids.push(eos);

        // --- спина ---
        let h = self.encoder.hidden();
        let states = self.encoder.forward(&ids)?;

        // --- промпты: контекстные состояния позиций <<ENT>> ---
        let mut prompt_rows: Vec<usize> = Vec::with_capacity(labels.len());
        for (pos, &id) in ids.iter().enumerate() {
            if id == self.ent_id {
                prompt_rows.push(pos);
            }
        }
        if prompt_rows.len() != labels.len() {
            return Err(format!(
                "GLiNER: найдено {} маркеров <<ENT>> (ожидалось {})",
                prompt_rows.len(),
                labels.len()
            ));
        }

        // --- слова: первые субтокены текстовых слов ---
        let view = self.encoder.view();
        let mut words_emb = vec![0f32; words.len() * h];
        for w in 0..words.len() {
            let w_global = n_prompt + w;
            let Some(fp_global) = enc.first_piece.get(w_global).copied().flatten() else {
                return Err(format!("GLiNER: слово {:?} без кусков", words[w]));
            };
            let pos = fp_global as usize + 1; // +1 за [CLS]
            words_emb[w * h..(w + 1) * h].copy_from_slice(&states[pos * h..(pos + 1) * h]);
        }

        // --- BiLSTM поверх слов ---
        let lstm = self.lstm_forward(&words_emb)?;

        // --- SpanMarker: project_start/end → relu(concat) → out_project ---
        let w_count = words.len();
        let mut start_rep = vec![0f32; w_count * h];
        let mut end_rep = vec![0f32; w_count * h];
        self.span_mlp(&lstm, &mut start_rep, &mut end_rep)?;

        // --- prompt-проекция ---
        let mut prompts_emb = vec![0f32; labels.len() * h];
        for (c, &pos) in prompt_rows.iter().enumerate() {
            prompts_emb[c * h..(c + 1) * h].copy_from_slice(&states[pos * h..(pos + 1) * h]);
        }
        let prompt_rep = self.prompt_mlp(&prompts_emb, labels.len())?;

        // --- скоры + декод ---
        let out_w = view.require("span_out_w")?;
        let out_b = view.tensor("span_out_b");
        let mut span_vec = vec![0f32; 2 * h];
        let mut rep = vec![0f32; h];
        let mut found: Vec<(f32, usize, usize, usize)> = Vec::new(); // (score, i, j, label)
        for i in 0..w_count {
            for wd in 0..self.max_width {
                let j = i + wd;
                if j >= w_count {
                    break;
                }
                span_vec[..h].copy_from_slice(&start_rep[i * h..(i + 1) * h]);
                span_vec[h..].copy_from_slice(&end_rep[j * h..(j + 1) * h]);
                relu_inplace(&mut span_vec);
                out_w.matvec(&span_vec, &mut rep)?;
                if let Some(b) = &out_b {
                    add_bias_inplace(&mut rep, Some(b))?;
                }
                for (c, _) in labels.iter().enumerate() {
                    let mut dot = 0f32;
                    for (a, b) in rep.iter().zip(&prompt_rep[c * h..(c + 1) * h]) {
                        dot += a * b;
                    }
                    let p = 1.0 / (1.0 + (-dot).exp());
                    if p >= threshold {
                        found.push((p, i, j, c));
                    }
                }
            }
        }
        // жадный не-оверлап по убыванию скора (flat NER)
        found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut taken: Vec<(usize, usize)> = Vec::new();
        let mut entities = Vec::new();
        for &(score, i, j, c) in &found {
            let overlaps = taken.iter().any(|&(i2, j2)| i <= j2 && i2 <= j);
            if !overlaps {
                taken.push((i, j));
                entities.push(Entity {
                    text: words[i..=j].to_vec().join(" "),
                    label: labels[c].to_string(),
                    score,
                    start: i,
                    end: j + 1,
                });
            }
        }
        entities.sort_by_key(|e| e.start);
        Ok(entities)
    }

    /// BiLSTM над словами: [W × h] → [W × h] (fwd ‖ bwd по 384).
    fn lstm_forward(&self, words_emb: &[f32]) -> Result<Vec<f32>, String> {
        let view = self.encoder.view();
        let h = self.encoder.hidden();
        let w = words_emb.len() / h;
        let h2 = h / 2;
        let ih = view.require("lstm_ih_w")?;
        let hh = view.require("lstm_hh_w")?;
        let ih_b = view.require("lstm_ih_b")?.f32s()?.to_vec();
        let hh_b = view.require("lstm_hh_b")?.f32s()?.to_vec();
        let ih_r = view.require("lstm_ih_w_r")?;
        let hh_r = view.require("lstm_hh_w_r")?;
        let ih_b_r = view.require("lstm_ih_b_r")?.f32s()?.to_vec();
        let hh_b_r = view.require("lstm_hh_b_r")?.f32s()?.to_vec();

        let mut out = vec![0f32; w * h];
        let mut gates = vec![0f32; 4 * h2];
        let mut gates2 = vec![0f32; 4 * h2];
        let mut hc = vec![0f32; 2 * h2]; // h ‖ c
        let mut hc2 = vec![0f32; 2 * h2];

        for t in 0..w {
            // прямой
            ih.matvec(&words_emb[t * h..(t + 1) * h], &mut gates)?;
            hh.matvec(&hc[..h2], &mut gates2)?;
            for g in 0..4 * h2 {
                gates[g] += gates2[g];
            }
            for (g, &v) in gates.iter_mut().zip(&ih_b) { *g += v; }
            for (g, &v) in gates.iter_mut().zip(&hh_b) { *g += v; }
            lstm_step(&mut gates, &mut hc, h2);
            out[t * h..t * h + h2].copy_from_slice(&hc[..h2]);
            // обратный
            let tb = w - 1 - t;
            ih_r.matvec(&words_emb[tb * h..(tb + 1) * h], &mut gates)?;
            hh_r.matvec(&hc2[..h2], &mut gates2)?;
            for g in 0..4 * h2 {
                gates[g] += gates2[g];
            }
            for (g, &v) in gates.iter_mut().zip(&ih_b_r) { *g += v; }
            for (g, &v) in gates.iter_mut().zip(&hh_b_r) { *g += v; }
            lstm_step(&mut gates, &mut hc2, h2);
            out[tb * h + h2..(tb + 1) * h].copy_from_slice(&hc2[..h2]);
        }
        Ok(out)
    }

    /// SpanMarker-MLP: project_start/end (768→1536→768) для каждого слова.
    fn span_mlp(&self, lstm_out: &[f32], start_rep: &mut [f32], end_rep: &mut [f32]) -> Result<(), String> {
        let view = self.encoder.view();
        let h = self.encoder.hidden();
        let w = lstm_out.len() / h;
        let s0 = view.require("span_start_0_w")?;
        let s3 = view.require("span_start_3_w")?;
        let e0 = view.require("span_end_0_w")?;
        let e3 = view.require("span_end_3_w")?;
        let s0b = view.require("span_start_0_b")?.f32s()?.to_vec();
        let s3b = view.require("span_start_3_b")?.f32s()?.to_vec();
        let e0b = view.require("span_end_0_b")?.f32s()?.to_vec();
        let e3b = view.require("span_end_3_b")?.f32s()?.to_vec();
        let wide = 2 * h;
        let mut mid = vec![0f32; wide];
        for t in 0..w {
            let xt = &lstm_out[t * h..(t + 1) * h];
            // start
            s0.matvec(xt, &mut mid)?;
            for (m, &v) in mid.iter_mut().zip(&s0b) { *m += v; }
            relu_inplace(&mut mid);
            s3.matvec(&mid, &mut start_rep[t * h..(t + 1) * h])?;
            for (m, &v) in start_rep[t * h..(t + 1) * h].iter_mut().zip(&s3b) { *m += v; }
            // end
            e0.matvec(xt, &mut mid)?;
            for (m, &v) in mid.iter_mut().zip(&e0b) { *m += v; }
            relu_inplace(&mut mid);
            e3.matvec(&mid, &mut end_rep[t * h..(t + 1) * h])?;
            for (m, &v) in end_rep[t * h..(t + 1) * h].iter_mut().zip(&e3b) { *m += v; }
        }
        Ok(())
    }

    /// prompt-MLP: 768→3072→768 для каждой метки.
    fn prompt_mlp(&self, prompts_emb: &[f32], n_labels: usize) -> Result<Vec<f32>, String> {
        let view = self.encoder.view();
        let h = self.encoder.hidden();
        let wide = view.require("prompt_0_w")?.rows();
        let p0 = view.require("prompt_0_w")?;
        let p3 = view.require("prompt_3_w")?;
        let p0b = view.require("prompt_0_b")?.f32s()?.to_vec();
        let p3b = view.require("prompt_3_b")?.f32s()?.to_vec();
        let mut mid = vec![0f32; wide];
        let mut out = vec![0f32; n_labels * h];
        for c in 0..n_labels {
            p0.matvec(&prompts_emb[c * h..(c + 1) * h], &mut mid)?;
            for (m, &v) in mid.iter_mut().zip(&p0b) { *m += v; }
            relu_inplace(&mut mid);
            p3.matvec(&mid, &mut out[c * h..(c + 1) * h])?;
            for (m, &v) in out[c * h..(c + 1) * h].iter_mut().zip(&p3b) { *m += v; }
        }
        Ok(out)
    }
}

/// Шаг LSTM: gates [i‖f‖g‖o] + состояние hc (h‖c) → новое hc.
///
/// Порядок гейтов — torch-конвенция [i, f, g, o].
fn lstm_step(gates: &mut [f32], hc: &mut [f32], h2: usize) {
    let (h_state, c_state) = hc.split_at_mut(h2);
    for i in 0..h2 {
        let gi = gates[i];
        let gf = gates[h2 + i];
        let gg = gates[2 * h2 + i];
        let go = gates[3 * h2 + i];
        let sig = |x: f32| 1.0 / (1.0 + (-x).exp());
        let c = sig(gf) * c_state[i] + sig(gi) * gg.tanh();
        c_state[i] = c;
        h_state[i] = sig(go) * c.tanh();
    }
}

fn relu_inplace(v: &mut [f32]) {
    for x in v.iter_mut() {
        if *x < 0.0 {
            *x = 0.0;
        }
    }
}

fn add_bias_inplace(v: &mut [f32], bias: Option<&crate::pqc::pqw::TensorView>) -> Result<(), String> {
    if let Some(b) = bias {
        let bs = b.f32s()?;
        if bs.len() != v.len() {
            return Err(format!("bias {} != вектор {}", bs.len(), v.len()));
        }
        for (o, &x) in v.iter_mut().zip(bs) {
            *o += x;
        }
    }
    Ok(())
}

/// Разбор секции __gliner__ (json: max_width, ent_id, sep_id, flert_id).
fn parse_gliner_meta(meta: &str) -> Result<(usize, u32, u32), String> {
    let mut max_width = 0usize;
    let mut ent_id = u32::MAX;
    let mut sep_id = u32::MAX;
    // минимальный json-парсер плоских полей (формат конвертера стабилен)
    for kv in meta.trim_matches(|c| c == '{' || c == '}').split(',') {
        let mut it = kv.splitn(2, ':');
        let key = it.next().unwrap_or("").trim().trim_matches('"');
        let val = it.next().unwrap_or("").trim();
        match key {
            "max_width" => max_width = val.parse().map_err(|_| "max_width не число")?,
            "ent_id" => ent_id = val.parse().map_err(|_| "ent_id не число")?,
            "sep_id" => sep_id = val.parse().map_err(|_| "sep_id не число")?,
            _ => {}
        }
    }
    if max_width == 0 || ent_id == u32::MAX || sep_id == u32::MAX {
        return Err(format!("__gliner__: неполные метаданные ({meta})"));
    }
    Ok((max_width, ent_id, sep_id))
}

// ---------------------------------------------------------------------------
// Синтетическая модель (тесты + --pqw-selftest)
// ---------------------------------------------------------------------------

/// fp32-эталон span-головы (дифференциальные тесты).
pub struct Fp32Gliner {
    pub encoder: crate::pqc::encoder::Fp32Encoder,
    pub width: Vec<f32>,
    pub proj: Vec<f32>,
    pub labels: Vec<String>,
    pub hidden: usize,
}

/// Генерирует синтетический GLiNER: энкодер + span-голова.
#[allow(clippy::too_many_arguments)]
pub fn synth_gliner(
    seed: u64,
    layers: usize,
    hidden: usize,
    heads: usize,
    intermediate: usize,
    vocab: usize,
    max_pos: usize,
    labels: &[&str],
    quant: Quant,
) -> (PqwBuilder, Fp32Gliner) {
    // Энкодерная спина (ModelType меняется на SpanNer после сборки).
    let (mut b, encoder) = synth_encoder(seed, layers, hidden, heads, intermediate, vocab, max_pos, quant);
    b.model_type = ModelType::SpanNer;
    // Свежий ГПСЧ для головы (детерминизм: сид с солью).
    let mut rng = SynthRng(seed ^ 0xC0FF_EE00_1234_5678);
    let max_width = 16usize;
    let n_labels = labels.len();

    let mut w = vec![0f32; max_width * hidden];
    rng.fill(&mut w, -0.2, 0.2);
    match quant {
        Quant::Int8 => {
            let (q, s) = tensor::quant_i8_per_row(&w, max_width, hidden);
            b.add_i8("span_width_emb", vec![max_width, hidden], &q, &s);
        }
        Quant::Int4 => {
            let (q, s) = tensor::quant_i4_per_row(&w, max_width, hidden);
            b.add_i4("span_width_emb", vec![max_width, hidden], &q, &s);
        }
        Quant::Trit5 => {
            let (q, s) = tensor::quant_trit5_per_row(&w, max_width, hidden);
            b.add_trit5("span_width_emb", vec![max_width, hidden], &q, &s);
        }
        Quant::F32 => {
            b.add_f32("span_width_emb", vec![max_width, hidden], &w);
        }
    }

    let mut proj = vec![0f32; n_labels * 3 * hidden];
    rng.fill(&mut proj, -0.15, 0.15);
    match quant {
        Quant::Int8 => {
            let (q, s) = tensor::quant_i8_per_row(&proj, n_labels, 3 * hidden);
            b.add_i8("span_proj_w", vec![n_labels, 3 * hidden], &q, &s);
        }
        Quant::Int4 => {
            let (q, s) = tensor::quant_i4_per_row(&proj, n_labels, 3 * hidden);
            b.add_i4("span_proj_w", vec![n_labels, 3 * hidden], &q, &s);
        }
        Quant::Trit5 => {
            let (q, s) = tensor::quant_trit5_per_row(&proj, n_labels, 3 * hidden);
            b.add_trit5("span_proj_w", vec![n_labels, 3 * hidden], &q, &s);
        }
        Quant::F32 => {
            b.add_f32("span_proj_w", vec![n_labels, 3 * hidden], &proj);
        }
    }
    b.add_raw("__labels__", labels.join("\n").as_bytes());

    (
        b,
        Fp32Gliner {
            encoder,
            width: w,
            proj,
            labels: labels.iter().map(|s| s.to_string()).collect(),
            hidden,
        },
    )
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pqc::encoder::reference_forward;

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let ip = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let na = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        let nb = b.iter().map(|v| v * v).sum::<f32>().sqrt();
        ip / (na * nb)
    }

    #[test]
    fn gliner_extracts_wellformed_entities() {
        let path = std::env::temp_dir().join(format!(
            "poler-gliner-struct-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = synth_gliner(
            3,
            1,
            32,
            4,
            64,
            64,
            48,
            &["PERSON", "LOCATION", "OBJECT"],
            Quant::Int8,
        );
        builder.write_to(&path).unwrap();
        let model = GlinerModel::open(&path).unwrap();
        let words = ["Вэнс", "прибыл", "на", "Архисферу", "вечером"];
        let ids: Vec<u32> = words.iter().map(|w| w.len() as u32 % 64).collect();
        let ents = model.extract(&words, &ids, 0.5).unwrap();
        for e in &ents {
            assert!(!e.text.is_empty());
            assert!(model.labels().contains(&e.label), "метка {}", e.label);
            assert!(e.start < 5 && e.end <= 5 && e.start < e.end);
            assert!((0.5..=1.0).contains(&e.score));
            // Текст span == склейка слов.
            assert_eq!(e.text, words[e.start..e.end].join(" "));
        }
        // Синтетические веса дают случайные скоры — но структура обязана
        // быть согласованной; сущности хотя бы иногда появляются.
        assert!(!ents.is_empty(), "случайные веса всё же должны что-то дать");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gliner_span_scores_match_fp32_reference() {
        let path = std::env::temp_dir().join(format!(
            "poler-gliner-diff-{}.pqw",
            std::process::id()
        ));
        let (builder, fp32) = synth_gliner(
            5,
            1,
            32,
            4,
            64,
            64,
            48,
            &["A", "B"],
            Quant::Int8,
        );
        builder.write_to(&path).unwrap();
        let model = GlinerModel::open(&path).unwrap();
        let _words = ["альфа", "бета", "гамма"];
        let ids: Vec<u32> = vec![10, 20, 30];
        let out = model.encoder.forward(&ids).unwrap();
        let native = model.span_scores(&out, 0, 2).unwrap();

        // Эталон: fp32-энкодер + наивная голова.
        let states = reference_forward(&fp32.encoder, &ids);
        let h = fp32.hidden;
        let mut rep = vec![0f32; 3 * h];
        rep[..h].copy_from_slice(&states[..h]);
        rep[h..2 * h].copy_from_slice(&states[2 * h..3 * h]);
        let width = 2;
        rep[2 * h..].copy_from_slice(&fp32.width[width * h..(width + 1) * h]);
        let reference: Vec<f32> = (0..2)
            .map(|l| {
                let mut lg = 0f32;
                for (a, b) in fp32.proj[l * 3 * h..(l + 1) * 3 * h].iter().zip(&rep) {
                    lg += a * b;
                }
                1.0 / (1.0 + (-lg).exp())
            })
            .collect();
        let c = cosine(&native, &reference);
        assert!(c > 0.999, "косинус span-скоров = {c}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gliner_rejects_wrong_model_type() {
        // Чистый энкодер не должен открываться как GLiNER.
        let path = std::env::temp_dir().join(format!(
            "poler-gliner-wrong-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = crate::pqc::encoder::synth_encoder(
            1,
            1,
            16,
            2,
            32,
            32,
            32,
            Quant::Int8,
        );
        builder.write_to(&path).unwrap();
        assert!(GlinerModel::open(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gliner_word_token_count_mismatch_rejected() {
        let path = std::env::temp_dir().join(format!(
            "poler-gliner-mm-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = synth_gliner(2, 1, 16, 2, 32, 32, 32, &["X"], Quant::Int8);
        builder.write_to(&path).unwrap();
        let model = GlinerModel::open(&path).unwrap();
        assert!(model.extract(&["a", "b"], &[1], 0.5).is_err());
        let _ = std::fs::remove_file(&path);
    }
}

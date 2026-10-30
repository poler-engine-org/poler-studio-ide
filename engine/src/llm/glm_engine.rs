//! Локальный LLM-инференс — GLM-декодер (Part F, Фаза 12.1–12.6).
//!
//! Архитектура ChatGLM-класса на чистом Rust поверх `.pqw`-весов:
//!
//! ```text
//! Token Embedding (int8/int4, mmap zero-copy)
//!   └─ N × Transformer Block (pre-norm):
//!        ├─ RMSNorm → Q/K/V проекции (K,V — kv_heads голов = MQA/GQA)
//!        ├─ RoPE (табличные cos/sin — без тяжёлого trig в рантайме)
//!        ├─ softmax(Q·Kᵀ/√d)·V — каузальное внимание поверх KV-арены
//!        ├─ SwiGLU FFN: down( silu(gate·x) ⊙ (up·x) )
//!        └─ [MoE]: роутер топ-k экспертов — неактивные эксперты
//!             НЕ ЧИТАЮТСЯ (mmap-стриминг: страницы не подтягиваются)
//!   └─ Final RMSNorm → LM Head (tied с эмбеддингами, если нет отдельного)
//! ```
//!
//! KV-кеш — арена с плоскими массивами `layers × cap × kv_heads × hd`,
//! без аллокаций в шаге генерации (инкрементальный токен = один проход).
//! Сэмплирование: greedy / temperature / top-p (SplitMix64 — детерминизм
//! при фиксированном сиде).

use std::path::Path;

use crate::pqc::encoder::SynthRng;
use crate::pqc::pqw::{ModelType, PqwBuilder, Quant, QuantizedWeightsView};
use crate::pqc::tensor;

// ---------------------------------------------------------------------------
// KV-кеш — арена без аллокаций в шаге генерации
// ---------------------------------------------------------------------------

/// Плоская KV-арена: `k` и `v` по `[layers][cap][kv_heads][head_dim]`.
///
/// Выделение один раз на генерацию; `append` — копирование hd·kv_heads
/// чисел, чтение — срезы без аллокаций (задача 12.4).
pub struct KvArena {
    #[allow(dead_code)] // симметрия API: layers нужен для диагностики MoE-арен
    layers: usize,
    cap: usize,
    kv_heads: usize,
    head_dim: usize,
    k: Vec<f32>,
    v: Vec<f32>,
    len: usize,
}

impl KvArena {
    pub fn new(layers: usize, cap: usize, kv_heads: usize, head_dim: usize) -> Self {
        let n = layers * cap * kv_heads * head_dim;
        Self {
            layers,
            cap,
            kv_heads,
            head_dim,
            k: vec![0f32; n],
            v: vec![0f32; n],
            len: 0,
        }
    }

    /// Число записанных позиций.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Ёмкость (максимум позиций).
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Записывает K/V токена на позицию `pos` слоя `layer`:
    /// `k_token`/`v_token` — `kv_heads·hd` чисел.
    ///
    /// Позиция задаётся ЯВНО (а не через внутренний счётчик): один токен
    /// пишется на всех L слоях, но занимает РОВНО ОДНУ позицию арены.
    pub fn append(&mut self, layer: usize, pos: usize, k_token: &[f32], v_token: &[f32]) {
        debug_assert_eq!(k_token.len(), self.kv_heads * self.head_dim);
        debug_assert!(pos < self.cap);
        let base = ((layer * self.cap + pos) * self.kv_heads) * self.head_dim;
        self.k[base..base + k_token.len()].copy_from_slice(k_token);
        self.v[base..base + v_token.len()].copy_from_slice(v_token);
        if pos + 1 > self.len {
            self.len = pos + 1;
        }
    }

    /// K-голова `kv_head` позиции `pos` слоя `layer`.
    pub fn k_at(&self, layer: usize, pos: usize, kv_head: usize) -> &[f32] {
        let base = ((layer * self.cap + pos) * self.kv_heads + kv_head) * self.head_dim;
        &self.k[base..base + self.head_dim]
    }

    /// V-голова `kv_head` позиции `pos` слоя `layer`.
    pub fn v_at(&self, layer: usize, pos: usize, kv_head: usize) -> &[f32] {
        let base = ((layer * self.cap + pos) * self.kv_heads + kv_head) * self.head_dim;
        &self.v[base..base + self.head_dim]
    }
}

// ---------------------------------------------------------------------------
// MoE-роутер (потоковый доступ к экспертам)
// ---------------------------------------------------------------------------

/// Маршрутизация MoE: softmax по гейт-логитам → топ-k экспертов,
/// веса перенормированы в сумму 1. Выбор детерминирован (ties → младший
/// индекс). Веса НЕвыбранных экспертов не читаются вовсе — их
/// mmap-страницы остаются на диске (стриминг 70B-моделей, F.3).
pub fn moe_route(gate_logits: &[f32], top_k: usize) -> (Vec<usize>, Vec<f32>) {
    let n = gate_logits.len();
    let k = top_k.min(n);
    let mut probs = gate_logits.to_vec();
    tensor::softmax_inplace(&mut probs);
    // Топ-k по вероятности; при равенстве — младший индекс.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| probs[b].partial_cmp(&probs[a]).unwrap().then(a.cmp(&b)));
    let chosen: Vec<usize> = idx[..k].to_vec();
    let sum: f32 = chosen.iter().map(|&i| probs[i]).sum();
    let weights: Vec<f32> = if sum > 0.0 {
        chosen.iter().map(|&i| probs[i] / sum).collect()
    } else {
        vec![1.0 / k as f32; k]
    };
    (chosen, weights)
}

// ---------------------------------------------------------------------------
// Сэмплирование
// ---------------------------------------------------------------------------

/// Стратегия сэмплирования следующего токена.
#[derive(Debug, Clone)]
pub enum Sampling {
    /// argmax (детерминирован всегда).
    Greedy,
    /// softmax(logits/t) + мультиномиальный выбор (сид → детерминизм).
    Temperature { t: f32, seed: u64 },
    /// top-p (nucleus): отсечение хвоста распределения, затем выбор.
    TopP { p: f32, temperature: f32, seed: u64 },
}

/// Выбирает токен из логитов по стратегии.
pub fn sample_token(logits: &[f32], s: &Sampling) -> u32 {
    match s {
        Sampling::Greedy => tensor::argmax(logits) as u32,
        Sampling::Temperature { t, seed } => {
            let scaled: Vec<f32> = logits.iter().map(|&v| v / t.max(1e-6)).collect();
            sample_categorical(&scaled, *seed)
        }
        Sampling::TopP {
            p,
            temperature,
            seed,
        } => {
            let scaled: Vec<f32> = logits.iter().map(|&v| v / temperature.max(1e-6)).collect();
            sample_topp(&scaled, *p, *seed)
        }
    }
}

fn sample_categorical(probs_logits: &[f32], seed: u64) -> u32 {
    let mut p = probs_logits.to_vec();
    tensor::softmax_inplace(&mut p);
    let mut rng = SynthRng(seed);
    let u = rng.uniform(0.0, 1.0);
    let mut cum = 0f32;
    for (i, &v) in p.iter().enumerate() {
        cum += v;
        if u <= cum {
            return i as u32;
        }
    }
    (p.len() - 1) as u32
}

fn sample_topp(logits: &[f32], p_cut: f32, seed: u64) -> u32 {
    let mut probs = logits.to_vec();
    tensor::softmax_inplace(&mut probs);
    let mut idx: Vec<usize> = (0..probs.len()).collect();
    idx.sort_by(|&a, &b| probs[b].partial_cmp(&probs[a]).unwrap().then(a.cmp(&b)));
    let mut kept = Vec::new();
    let mut cum = 0f32;
    for &i in &idx {
        kept.push(i);
        cum += probs[i];
        if cum >= p_cut {
            break;
        }
    }
    let sum: f32 = kept.iter().map(|&i| probs[i]).sum();
    let mut rng = SynthRng(seed);
    let u = rng.uniform(0.0, 1.0) * sum;
    let mut acc = 0f32;
    for &i in &kept {
        acc += probs[i];
        if u <= acc {
            return i as u32;
        }
    }
    kept[kept.len() - 1] as u32
}

// ---------------------------------------------------------------------------
// Детокенизатор UAX#29-класса
// ---------------------------------------------------------------------------

/// Сшивает токены-слова в текст по UAX#29-подобным правилам: пробел
/// между словами, НЕТ пробела перед `. , ! ? ; : … ) ] } » ”` и ПОСЛЕ
/// `( [ { « “`. Для синтетических словарей и будущих BPE-токенов.
pub fn detokenize_join(words: &[&str]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            out.push_str(w);
            continue;
        }
        let no_space_before = w.starts_with(|c: char| {
            matches!(
                c,
                '.' | ',' | '!' | '?' | ';' | ':' | '…' | ')' | ']' | '}' | '»' | '”' | '-'
            )
        });
        let prev = words[i - 1];
        let no_space_after_prev =
            prev.ends_with(['(', '[', '{', '«', '“']);
        if no_space_before || no_space_after_prev {
            out.push_str(w);
        } else {
            out.push(' ');
            out.push_str(w);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Модель
// ---------------------------------------------------------------------------

/// GLM-декодер поверх mmap-весов `.pqw`.
pub struct GlmModel {
    view: QuantizedWeightsView,
    tokenizer: Option<crate::pqc::tokenizer::UnigramTokenizer>,
    hidden: usize,
    heads: usize,
    head_dim: usize,
    kv_heads: usize,
    layers: usize,
    intermediate: usize,
    vocab: usize,
    max_pos: usize,
    experts: usize,
    top_k: usize,
    /// ChatGLM-класс: scores/(layer_number·√hd), слои с 1.
    qk_layer_scaling: bool,
    /// eps RMSNorm из заголовка (ChatGLM3: 1e-5).
    rms_eps: f32,
    rope: tensor::RopeTable,
}

impl GlmModel {
    /// Открывает модель (Sha256-верификация + валидация архитектуры).
    pub fn open(path: &Path) -> Result<Self, String> {
        let view = QuantizedWeightsView::open(path)?;
        let h = view.header();
        if h.model_type != ModelType::Decoder {
            return Err(format!(
                "модель не декодер (model_type={:?}) — --llm требует GLM-класс",
                h.model_type
            ));
        }
        let head_dim = if h.head_dim > 0 { h.head_dim } else { 1 };
        if h.hidden == 0 || h.heads == 0 || h.hidden % h.heads != 0 {
            return Err(format!(
                "архитектура невалидна: hidden={} heads={}",
                h.hidden, h.heads
            ));
        }
        if h.kv_heads == 0 || h.kv_heads > h.heads {
            return Err(format!("kv_heads={} невалидно", h.kv_heads));
        }
        let tokenizer = match view.tensor("__tokenizer__") {
            Some(t) => Some(crate::pqc::tokenizer::UnigramTokenizer::parse(t.raw_bytes()?)?),
            None => None,
        };
        // ChatGLM3 — half-rotary: вращаются только первые 64 из 128
        // измерений головы (reference: rotary_dim = kv_channels,
        // RotaryEmbedding(rotary_dim // 2) → apply rot_dim = 64;
        // частоты 10000^(-2i/64), хвост — pass-through).
        let rope = tensor::RopeTable::new_partial(head_dim, head_dim / 2, h.max_pos);
        Ok(Self {
            hidden: h.hidden,
            heads: h.heads,
            head_dim,
            kv_heads: h.kv_heads,
            layers: h.layers,
            intermediate: h.intermediate,
            vocab: h.vocab,
            max_pos: h.max_pos,
            experts: h.experts,
            top_k: h.top_k.max(1),
            qk_layer_scaling: h.qk_layer_scaling(),
            rms_eps: h.rms_eps,
            rope,
            tokenizer,
            view,
        })
    }

    /// Доступ к mmap-представлению квантованных весов.
    pub fn view(&self) -> &QuantizedWeightsView {
        &self.view
    }

    /// Токенизатор модели (если секция __tokenizer__ встроена в .pqw).
    pub fn tokenizer(&self) -> Option<&crate::pqc::tokenizer::UnigramTokenizer> {
        self.tokenizer.as_ref()
    }

    /// Размер словаря.
    pub fn vocab(&self) -> usize {
        self.vocab
    }

    /// MoE-конфигурация (0 = плотный FFN).
    pub fn experts(&self) -> usize {
        self.experts
    }

    /// Инкрементальный проход ОДНОГО токена: обновляет KV-арену и пишет
    /// логиты следующего токена. Ядро генерации (F.4, конвейер 3).
    pub fn forward_pos(
        &self,
        token: u32,
        pos: usize,
        kv: &mut KvArena,
        logits: &mut [f32],
    ) -> Result<(), String> {
        if pos >= self.max_pos || pos >= kv.cap() {
            return Err(format!("позиция {pos} вне max_pos/cap"));
        }
        if logits.len() != self.vocab {
            return Err(format!("буфер логитов {} != vocab {}", logits.len(), self.vocab));
        }
        let h = self.hidden;
        let hd = self.head_dim;
        let kv_dim = self.kv_heads * hd;
        let q_dim = self.heads * hd;

        let emb = self.view.require("word_embeddings")?;
        let token = token as usize;
        if token >= emb.rows() {
            return Err(format!("токен {token} вне словаря ({})", emb.rows()));
        }
        let mut x = vec![0f32; h];
        emb.gather_row(token, &mut x)?;

        let mut hn = vec![0f32; h];
        let mut q = vec![0f32; q_dim];
        let mut k = vec![0f32; kv_dim];
        let mut v = vec![0f32; kv_dim];
        let mut ctx = vec![0f32; q_dim];
        let mut attn_out = vec![0f32; h];
        let mut scores = vec![0f32; pos + 1];

        for layer in 0..self.layers {
            let p = |s: &str| format!("layers.{layer}.{s}");

            // ChatGLM-класс (apply_query_key_layer_scaling): дополнительно
            // делится на номер слоя (нумерация с 1) — иначе поздние слои
            // перегревают softmax.
            let scale = if self.qk_layer_scaling {
                1.0 / ((hd as f32).sqrt() * (layer as f32 + 1.0))
            } else {
                1.0 / (hd as f32).sqrt()
            };

            // --- Внимание (pre-norm) ---
            let attn_norm = self.view.require(&p("attn_norm_gamma"))?;
            tensor::rms_norm(&x, attn_norm.f32s()?, self.rms_eps, &mut hn);

            let q_w = self.view.tensor(&p("attn_q_w"))
                .or_else(|| self.view.tensor(&p("q_proj_w")))
                .ok_or_else(|| format!("тензор «{}» отсутствует в .pqw", p("attn_q_w")))?;
            let k_w = self.view.tensor(&p("attn_k_w"))
                .or_else(|| self.view.tensor(&p("k_proj_w")))
                .ok_or_else(|| format!("тензор «{}» отсутствует в .pqw", p("attn_k_w")))?;
            let v_w = self.view.tensor(&p("attn_v_w"))
                .or_else(|| self.view.tensor(&p("v_proj_w")))
                .ok_or_else(|| format!("тензор «{}» отсутствует в .pqw", p("attn_v_w")))?;

            q_w.matvec(&hn, &mut q)?;
            k_w.matvec(&hn, &mut k)?;
            v_w.matvec(&hn, &mut v)?;

            // ChatGLM3 (add_qkv_bias): bias QKV-проекции хранится одним
            // f32-тензором [q_dim + 2·kv_dim] и добавляется посрезово.
            if let Some(b) = self.view.tensor(&p("qkv_b")) {
                let bf = b.f32s()?;
                if bf.len() != q_dim + 2 * kv_dim {
                    return Err(format!(
                        "тензор {}: длина {} != q_dim+2·kv_dim = {}",
                        p("qkv_b"),
                        bf.len(),
                        q_dim + 2 * kv_dim
                    ));
                }
                for i in 0..q_dim {
                    q[i] += bf[i];
                }
                for i in 0..kv_dim {
                    k[i] += bf[q_dim + i];
                    v[i] += bf[q_dim + kv_dim + i];
                }
            }

            // RoPE: каждую q-голову и kv-голову по позиции pos.
            for head in 0..self.heads {
                self.rope.rotate(&mut q[head * hd..(head + 1) * hd], pos);
            }
            for head in 0..self.kv_heads {
                self.rope.rotate(&mut k[head * hd..(head + 1) * hd], pos);
            }
            kv.append(layer, pos, &k, &v);

            // GQA-раскладка HF/ChatGLM: q-головы сгруппированы БЛОКАМИ
            // по heads/kv_heads (expand+view в modeling_chatglm: головы
            // 0..n_rep-1 → группа 0), а не чередованием. MQA/MHA
            // вырождаются в head и 0 соответственно.
            let n_rep = self.heads / self.kv_heads;
            for head in 0..self.heads {
                let kv_head = head / n_rep;
                let qs = &q[head * hd..(head + 1) * hd];
                for j in 0..=pos {
                    scores[j] = tensor::dot_f32(qs, kv.k_at(layer, j, kv_head)) * scale;
                }
                tensor::softmax_inplace(&mut scores);
                let cs = &mut ctx[head * hd..(head + 1) * hd];
                cs.fill(0.0);
                for (j, &w) in scores.iter().enumerate() {
                    let vj = kv.v_at(layer, j, kv_head);
                    for (o, &vv) in cs.iter_mut().zip(vj) {
                        *o += w * vv;
                    }
                }
            }
            let o_w = self.view.tensor(&p("attn_o_w"))
                .or_else(|| self.view.tensor(&p("out_proj_w")))
                .ok_or_else(|| format!("тензор «{}» отсутствует в .pqw", p("attn_o_w")))?;
            o_w.matvec(&ctx, &mut attn_out)?;
            for i in 0..h {
                x[i] += attn_out[i];
            }

            // --- FFN (pre-norm): SwiGLU или MoE ---
            let ffn_norm = self.view.require(&p("ffn_norm_gamma"))?;
            tensor::rms_norm(&x, ffn_norm.f32s()?, self.rms_eps, &mut hn);

            if self.experts > 0 {
                // Роутер: гейт по hn → топ-k экспертов → взвешенная сумма.
                let gate_w = self.view.require(&p("moe_gate_w"))?;
                let mut gate_logits = vec![0f32; self.experts];
                gate_w.matvec(&hn, &mut gate_logits)?;
                let (chosen, weights) = moe_route(&gate_logits, self.top_k);
                let mut expert_out = vec![0f32; h];
                let mut acc = vec![0f32; h];
                let mut gate_buf = vec![0f32; self.intermediate];
                let mut up_buf = vec![0f32; self.intermediate];
                for (&e, &w) in chosen.iter().zip(&weights) {
                    let ep = |s: &str| format!("layers.{layer}.experts.{e}.{s}");
                    self.view.require(&ep("ffn_gate_w"))?.matvec(&hn, &mut gate_buf)?;
                    self.view.require(&ep("ffn_up_w"))?.matvec(&hn, &mut up_buf)?;
                    // SwiGLU: silu(gate) ⊙ up.
                    for i in 0..self.intermediate {
                        gate_buf[i] = tensor::silu(gate_buf[i]) * up_buf[i];
                    }
                    self.view.require(&ep("ffn_down_w"))?.matvec(&gate_buf, &mut expert_out)?;
                    for i in 0..h {
                        acc[i] += w * expert_out[i];
                    }
                }
                for i in 0..h {
                    x[i] += acc[i];
                }
            } else {
                let mut gate_buf = vec![0f32; self.intermediate];
                let mut up_buf = vec![0f32; self.intermediate];
                self.view.require(&p("ffn_gate_w"))?.matvec(&hn, &mut gate_buf)?;
                self.view.require(&p("ffn_up_w"))?.matvec(&hn, &mut up_buf)?;
                for i in 0..self.intermediate {
                    gate_buf[i] = tensor::silu(gate_buf[i]) * up_buf[i];
                }
                self.view.require(&p("ffn_down_w"))?.matvec(&gate_buf, &mut attn_out)?;
                for i in 0..h {
                    x[i] += attn_out[i];
                }
            }
        }

        // --- Финальная норма + LM head ---
        let final_norm = self.view.require("final_norm_gamma")?;
        tensor::rms_norm(&x, final_norm.f32s()?, self.rms_eps, &mut hn);
        match self.view.tensor("lm_head_w") {
            Some(lm) => lm.matvec(&hn, logits)?,
            None => {
                // Tied weights: lm_head == word_embeddings (транспон. проход).
                let emb = self.view.require("word_embeddings")?;
                emb.matvec(&hn, logits)?
            }
        }
        Ok(())
    }

    /// Полный проход всех позиций с нуля (эталон инкрементальности:
    /// логиты позиции t должны совпадать с forward_pos-проходом).
    pub fn full_logits(&self, ids: &[u32]) -> Result<Vec<Vec<f32>>, String> {
        let mut kv = KvArena::new(self.layers, self.max_pos, self.kv_heads, self.head_dim);
        let mut out = Vec::with_capacity(ids.len());
        let mut logits = vec![0f32; self.vocab];
        for (t, &tok) in ids.iter().enumerate() {
            self.forward_pos(tok, t, &mut kv, &mut logits)?;
            out.push(logits.clone());
        }
        Ok(out)
    }

    /// Авторегрессионная генерация с потоковым колбэком (streaming).
    /// Колбэк вызывается на каждый сгенерированный токен: `on_token(token_id, token_text) -> bool`
    /// (возврат `false` останавливает генерацию досрочно).
    pub fn generate_stream<F>(
        &self,
        prompt: &[u32],
        max_new: usize,
        sampling: &Sampling,
        seed: u64,
        mut on_token: F,
    ) -> Result<Vec<u32>, String>
    where
        F: FnMut(u32, &str) -> bool,
    {
        if prompt.is_empty() {
            return Err("LLM: пустой промпт".into());
        }
        let total = prompt.len() + max_new;
        if total > self.max_pos {
            return Err(format!(
                "промпт+генерация ({total}) длиннее max_pos={}",
                self.max_pos
            ));
        }
        let mut kv = KvArena::new(self.layers, self.max_pos, self.kv_heads, self.head_dim);
        let mut logits = vec![0f32; self.vocab];
        let mut step = 0u64;
        for &tok in prompt {
            self.forward_pos(tok, kv.len(), &mut kv, &mut logits)?;
        }
        let mut generated = Vec::with_capacity(max_new);
        let is_eos_token = |tok: u32| -> bool {
            if let Some(tok_engine) = &self.tokenizer {
                let (_, eos_id, _, _) = tok_engine.special_ids();
                tok == eos_id || tok == 2 || tok == 64795 || tok == 64797
            } else {
                false
            }
        };
        for _ in 0..max_new {
            let s = match sampling {
                Sampling::Greedy => Sampling::Greedy,
                Sampling::Temperature { t, .. } => Sampling::Temperature {
                    t: *t,
                    seed: seed.wrapping_add(step),
                },
                Sampling::TopP {
                    p,
                    temperature,
                    ..
                } => Sampling::TopP {
                    p: *p,
                    temperature: *temperature,
                    seed: seed.wrapping_add(step),
                },
            };
            let tok = sample_token(&logits, &s);
            if is_eos_token(tok) {
                break;
            }
            generated.push(tok);
            step += 1;

            let piece_str = if let Some(tok_engine) = &self.tokenizer {
                tok_engine.detokenize(&[tok])
            } else {
                format!(" t{tok}")
            };
            let should_continue = on_token(tok, &piece_str);
            if !should_continue {
                break;
            }

            if generated.len() < max_new {
                self.forward_pos(tok, kv.len(), &mut kv, &mut logits)?;
            }
        }
        Ok(generated)
    }

    /// Авторегрессионная генерация: прогон промпта → `max_new` токенов.
    pub fn generate(
        &self,
        prompt: &[u32],
        max_new: usize,
        sampling: &Sampling,
        seed: u64,
    ) -> Result<Vec<u32>, String> {
        self.generate_stream(prompt, max_new, sampling, seed, |_, _| true)
    }
}

// ---------------------------------------------------------------------------
// Синтетический декодер (тесты + --pqw-selftest)
// ---------------------------------------------------------------------------

/// fp32-эталон декодера (дифференциальные тесты).
pub struct Fp32Glm {
    pub word: Vec<f32>,
    pub layers: Vec<Fp32GlmLayer>,
    pub final_norm_g: Vec<f32>,
    /// LM Head [vocab, hidden] (отдельный тензор в .pqw).
    pub lm: Vec<f32>,
    pub hidden: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub vocab: usize,
    pub experts: usize,
    pub top_k: usize,
    /// Семантика ChatGLM-класса (флаги .pqw).
    pub qk_layer_scaling: bool,
    pub rms_eps: f32,
}

pub struct Fp32GlmLayer {
    pub q_w: Vec<f32>,
    pub k_w: Vec<f32>,
    pub v_w: Vec<f32>,
    pub o_w: Vec<f32>,
    pub attn_norm_g: Vec<f32>,
    pub ffn_norm_g: Vec<f32>,
    /// ChatGLM3: bias QKV [q_dim + 2·kv_dim] (пусто = без bias).
    pub qkv_b: Vec<f32>,
    /// MoE: гейт-матрица [experts, hidden] и тройки экспертов
    /// (gate, up, down). Плотный FFN — один «эксперт» в moe_experts[0].
    pub moe_gate: Vec<f32>,
    pub moe_experts: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>,
}

/// Генерирует синтетический GLM-декодер (dense или MoE) в двух видах:
/// квантованный `.pqw`-билдер + fp32-эталон.
#[allow(clippy::too_many_arguments)]
pub fn synth_glm(
    seed: u64,
    layers: usize,
    hidden: usize,
    heads: usize,
    kv_heads: usize,
    intermediate: usize,
    vocab: usize,
    max_pos: usize,
    quant: Quant,
    experts: usize,
    top_k: usize,
) -> (PqwBuilder, Fp32Glm) {
    let mut rng = SynthRng(seed);
    let head_dim = hidden / heads;
    let q_dim = heads * head_dim;
    let kv_dim = kv_heads * head_dim;
    let mut b = PqwBuilder::new(
        ModelType::Decoder,
        quant,
        layers,
        hidden,
        heads,
        intermediate,
        vocab,
        max_pos,
    )
    .with_kv_heads(kv_heads);
    if experts > 0 {
        let bm = b.with_moe(experts, top_k);
        b = bm;
    }

    // Ссылочная семантика — старые файлы без флагов (масштаб 1/√hd, eps 1e-6).
    let qk_layer_scaling = false;
    let rms_eps = 1e-6;

    let mat = |name: &str,
                   rows: usize,
                   cols: usize,
                   b: &mut PqwBuilder,
                   rng: &mut SynthRng|
     -> Vec<f32> {
        let mut w = vec![0f32; rows * cols];
        rng.fill(&mut w, -0.15, 0.15);
        match quant {
            Quant::Int8 => {
                let (q, s) = tensor::quant_i8_per_row(&w, rows, cols);
                b.add_i8(name, vec![rows, cols], &q, &s);
            }
            Quant::Int4 => {
                let (q, s) = tensor::quant_i4_per_row(&w, rows, cols);
                b.add_i4(name, vec![rows, cols], &q, &s);
            }
            Quant::Trit5 => {
                let (q, s) = tensor::quant_trit5_per_row(&w, rows, cols);
                b.add_trit5(name, vec![rows, cols], &q, &s);
            }
            Quant::F32 => {
                b.add_f32(name, vec![rows, cols], &w);
            }
        }
        w
    };

    let word = mat("word_embeddings", vocab, hidden, &mut b, &mut rng);
    let mut fp32_layers = Vec::new();
    for i in 0..layers {
        let p = |s: &str| format!("layers.{i}.{s}");
        let q_w = mat(&p("attn_q_w"), q_dim, hidden, &mut b, &mut rng);
        let k_w = mat(&p("attn_k_w"), kv_dim, hidden, &mut b, &mut rng);
        let v_w = mat(&p("attn_v_w"), kv_dim, hidden, &mut b, &mut rng);
        let o_w = mat(&p("attn_o_w"), hidden, q_dim, &mut b, &mut rng);
        // Нормы близки к 1 (RMSNorm-гаммы).
        let mut attn_norm_g = vec![0f32; hidden];
        rng.fill(&mut attn_norm_g, 0.9, 1.1);
        b.add_f32(&p("attn_norm_gamma"), vec![hidden], &attn_norm_g);
        let mut ffn_norm_g = vec![0f32; hidden];
        rng.fill(&mut ffn_norm_g, 0.9, 1.1);
        b.add_f32(&p("ffn_norm_gamma"), vec![hidden], &ffn_norm_g);

        let mut moe_gate = Vec::new();
        let mut moe_experts = Vec::new();
        // ChatGLM3-стиль: небольшой ненулевой bias QKV.
        let mut qkv_b = vec![0f32; q_dim + 2 * kv_dim];
        rng.fill(&mut qkv_b, -0.05, 0.05);
        b.add_f32(&p("qkv_b"), vec![qkv_b.len()], &qkv_b);
        if experts > 0 {
            // Гейт-роутер — маленький, храним fp32.
            moe_gate = vec![0f32; experts * hidden];
            rng.fill(&mut moe_gate, -0.3, 0.3);
            b.add_f32(&p("moe_gate_w"), vec![experts, hidden], &moe_gate);
            for e in 0..experts {
                let ep = |s: &str| format!("layers.{i}.experts.{e}.{s}");
                let g = mat(&ep("ffn_gate_w"), intermediate, hidden, &mut b, &mut rng);
                let u = mat(&ep("ffn_up_w"), intermediate, hidden, &mut b, &mut rng);
                let d = mat(&ep("ffn_down_w"), hidden, intermediate, &mut b, &mut rng);
                moe_experts.push((g, u, d));
            }
        } else {
            let g = mat(&p("ffn_gate_w"), intermediate, hidden, &mut b, &mut rng);
            let u = mat(&p("ffn_up_w"), intermediate, hidden, &mut b, &mut rng);
            let d = mat(&p("ffn_down_w"), hidden, intermediate, &mut b, &mut rng);
            moe_experts.push((g, u, d));
        }
        fp32_layers.push(Fp32GlmLayer {
            q_w,
            k_w,
            v_w,
            o_w,
            attn_norm_g,
            ffn_norm_g,
            qkv_b,
            moe_gate,
            moe_experts,
        });
    }
    let mut final_g = vec![0f32; hidden];
    rng.fill(&mut final_g, 0.9, 1.1);
    b.add_f32("final_norm_gamma", vec![hidden], &final_g);
    // lm_head отдельным тензором (не tied) — эталон хранит fp32-копию.
    let lm = mat("lm_head_w", vocab, hidden, &mut b, &mut rng);

    let fp32 = Fp32Glm {
        word,
        layers: fp32_layers,
        final_norm_g: final_g,
        lm,
        hidden,
        heads,
        kv_heads,
        head_dim,
        vocab,
        experts,
        top_k,
        qk_layer_scaling,
        rms_eps,
    };
    (b, fp32)
}

/// Наивный fp32-эталон полного прохода декодера (каузальный).
pub fn reference_glm_logits(w: &Fp32Glm, ids: &[u32]) -> Vec<Vec<f32>> {
    let h = w.hidden;
    let hd = w.head_dim;
    let heads = w.heads;
    let kv_heads = w.kv_heads;
    let seq = ids.len();
    // Half-rotary ChatGLM3: первые 64 из 128 dims (см. конструктор движка).
    let rope = tensor::RopeTable::new_partial(hd, hd / 2, seq.max(1));
    // Скрытые состояния всех позиций.
    let mut hs: Vec<Vec<f32>> = ids
        .iter()
        .map(|&id| w.word[id as usize * h..(id as usize + 1) * h].to_vec())
        .collect();
    for (layer_idx, layer) in w.layers.iter().enumerate() {
        // Тот же масштаб, что в движке: 1/√hd или 1/((layer+1)·√hd).
        let scale = if w.qk_layer_scaling {
            1.0 / ((hd as f32).sqrt() * (layer_idx as f32 + 1.0))
        } else {
            1.0 / (hd as f32).sqrt()
        };
        let q_dim = heads * hd;
        let kv_dim = kv_heads * hd;
        let has_qkv_b = !layer.qkv_b.is_empty();
        // Проекции + RoPE для всех позиций.
        let mut qs: Vec<Vec<f32>> = Vec::with_capacity(seq);
        let mut ks: Vec<Vec<f32>> = Vec::with_capacity(seq);
        let mut vs: Vec<Vec<f32>> = Vec::with_capacity(seq);
        for t in 0..seq {
            let mut hn = vec![0f32; h];
            tensor::rms_norm(&hs[t], &layer.attn_norm_g, w.rms_eps, &mut hn);
            let mut q = vec![0f32; heads * hd];
            let mut k = vec![0f32; kv_heads * hd];
            let mut v = vec![0f32; kv_heads * hd];
            for i in 0..heads * hd {
                q[i] = dot_naive(&layer.q_w[i * h..(i + 1) * h], &hn);
            }
            for i in 0..kv_heads * hd {
                k[i] = dot_naive(&layer.k_w[i * h..(i + 1) * h], &hn);
                v[i] = dot_naive(&layer.v_w[i * h..(i + 1) * h], &hn);
            }
            if has_qkv_b {
                for i in 0..q_dim {
                    q[i] += layer.qkv_b[i];
                }
                for i in 0..kv_dim {
                    k[i] += layer.qkv_b[q_dim + i];
                    v[i] += layer.qkv_b[q_dim + kv_dim + i];
                }
            }
            for head in 0..heads {
                rope.rotate(&mut q[head * hd..(head + 1) * hd], t);
            }
            for head in 0..kv_heads {
                rope.rotate(&mut k[head * hd..(head + 1) * hd], t);
            }
            qs.push(q);
            ks.push(k);
            vs.push(v);
        }
        // Каузальное внимание (блочная GQA-раскладка, как в движке).
        for t in 0..seq {
            let mut ctx = vec![0f32; heads * hd];
            let n_rep = heads / kv_heads;
            for head in 0..heads {
                let kv_head = head / n_rep;
                let mut scores = vec![0f32; t + 1];
                for j in 0..=t {
                    let mut d = 0f32;
                    for d_i in 0..hd {
                        d += qs[t][head * hd + d_i] * ks[j][kv_head * hd + d_i];
                    }
                    scores[j] = d * scale;
                }
                tensor::softmax_inplace(&mut scores);
                for d_i in 0..hd {
                    let mut acc = 0f32;
                    for (j, &sw) in scores.iter().enumerate() {
                        acc += sw * vs[j][kv_head * hd + d_i];
                    }
                    ctx[head * hd + d_i] = acc;
                }
            }
            let mut o = vec![0f32; h];
            for i in 0..h {
                o[i] = dot_naive(&layer.o_w[i * (heads * hd)..(i + 1) * (heads * hd)], &ctx);
            }
            for i in 0..h {
                hs[t][i] += o[i];
            }
        }
        // FFN (SwiGLU / MoE).
        for t in 0..seq {
            let mut hn = vec![0f32; h];
            tensor::rms_norm(&hs[t], &layer.ffn_norm_g, w.rms_eps, &mut hn);
            let (g0, _, _) = &layer.moe_experts[0];
            let intermediate = g0.len() / h;
            let mut add = vec![0f32; h];
            if w.experts > 0 {
                let mut gate_logits = vec![0f32; w.experts];
                for e in 0..w.experts {
                    gate_logits[e] = dot_naive(&layer.moe_gate[e * h..(e + 1) * h], &hn);
                }
                let (chosen, weights) = moe_route(&gate_logits, w.top_k);
                for (&e, &wt) in chosen.iter().zip(&weights) {
                    let (g, u, d) = &layer.moe_experts[e];
                    let mut gi = vec![0f32; intermediate];
                    let mut ui = vec![0f32; intermediate];
                    for i in 0..intermediate {
                        gi[i] = dot_naive(&g[i * h..(i + 1) * h], &hn);
                        ui[i] = dot_naive(&u[i * h..(i + 1) * h], &hn);
                    }
                    for i in 0..intermediate {
                        gi[i] = tensor::silu(gi[i]) * ui[i];
                    }
                    for i in 0..h {
                        add[i] += wt * dot_naive(&d[i * intermediate..(i + 1) * intermediate], &gi);
                    }
                }
            } else {
                let (g, u, d) = &layer.moe_experts[0];
                let intermediate = g.len() / h;
                let mut gi = vec![0f32; intermediate];
                let mut ui = vec![0f32; intermediate];
                for i in 0..intermediate {
                    gi[i] = dot_naive(&g[i * h..(i + 1) * h], &hn);
                    ui[i] = dot_naive(&u[i * h..(i + 1) * h], &hn);
                }
                for i in 0..intermediate {
                    gi[i] = tensor::silu(gi[i]) * ui[i];
                }
                for i in 0..h {
                    add[i] = dot_naive(&d[i * intermediate..(i + 1) * intermediate], &gi);
                }
            }
            for i in 0..h {
                hs[t][i] += add[i];
            }
        }
    }
    // Финальная норма + LM Head по fp32-копии.
    let mut out = Vec::with_capacity(seq);
    for t in 0..seq {
        let mut hn = vec![0f32; h];
        tensor::rms_norm(&hs[t], &w.final_norm_g, w.rms_eps, &mut hn);
        let mut logits = vec![0f32; w.vocab];
        for v in 0..w.vocab {
            logits[v] = dot_naive(&w.lm[v * h..(v + 1) * h], &hn);
        }
        out.push(logits);
    }
    out
}

fn dot_naive(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_arena_append_read_roundtrip() {
        let mut kv = KvArena::new(2, 4, 1, 8);
        let k: Vec<f32> = (0..8u32).map(|i| i as f32).collect();
        let v: Vec<f32> = (8..16u32).map(|i| i as f32).collect();
        kv.append(1, 0, &k, &v);
        assert_eq!(kv.len(), 1);
        assert_eq!(kv.k_at(1, 0, 0), &k[..]);
        assert_eq!(kv.v_at(1, 0, 0), &v[..]);
        // Другой слой не затронут.
        assert!(kv.k_at(0, 0, 0).iter().all(|&x| x == 0.0));
        // Тот же токен на другом слое — len не растёт дважды.
        kv.append(0, 0, &k, &v);
        assert_eq!(kv.len(), 1);
    }

    #[test]
    fn moe_route_topk_and_weights() {
        let (chosen, weights) = moe_route(&[0.1, 3.0, 0.2, 2.0], 2);
        assert_eq!(chosen, vec![1usize, 3]);
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(weights[0] > weights[1]);
        // top_k > n → все эксперты.
        let (c2, w2) = moe_route(&[1.0, 2.0], 5);
        assert_eq!(c2.len(), 2);
        assert_eq!(w2.len(), 2);
    }

    #[test]
    fn sampling_greedy_is_argmax() {
        let logits = [0.1f32, -2.0, 5.0, 3.0];
        assert_eq!(sample_token(&logits, &Sampling::Greedy), 2);
    }

    #[test]
    fn sampling_temperature_deterministic_by_seed() {
        let logits = [0.1f32, 2.0, 1.5, 0.7, 1.2];
        let a = sample_token(&logits, &Sampling::Temperature { t: 1.0, seed: 42 });
        let b = sample_token(&logits, &Sampling::Temperature { t: 1.0, seed: 42 });
        assert_eq!(a, b);
        let c = sample_token(&logits, &Sampling::Temperature { t: 1.0, seed: 43 });
        // Разные сиды МОГУТ совпасть, но распределение смещено к 1;
        // проверяем только детерминизм одного сида.
        let _ = c;
    }

    #[test]
    fn sampling_low_temperature_converges_to_greedy() {
        let logits = [0.1f32, 0.2, 4.0, 0.3];
        for seed in 0..20u64 {
            let t = sample_token(&logits, &Sampling::Temperature { t: 0.01, seed });
            assert_eq!(t, 2, "t→0 должен сходиться к argmax (seed={seed})");
        }
    }

    #[test]
    fn sampling_topp_keeps_head() {
        let mut logits = vec![0f32; 100];
        logits[7] = 10.0; // доминирующий токен
        logits[3] = 8.0;
        for seed in 0..20u64 {
            let tok = sample_token(
                &logits,
                &Sampling::TopP {
                    p: 0.9,
                    temperature: 1.0,
                    seed,
                },
            );
            assert!(
                tok == 7 || tok == 3,
                "top-p должен держать голову распределения (seed={seed}, tok={tok})"
            );
        }
    }

    #[test]
    fn detokenize_join_rules() {
        assert_eq!(
            detokenize_join(&["Вэнс", "сказал", "«", "Архисфера", "»", "!"]),
            "Вэнс сказал «Архисфера»!"
        );
        assert_eq!(
            detokenize_join(&["модель", ",", "что", "это", "?"]),
            "модель, что это?"
        );
    }

    #[test]
    fn detokenize_join_empty_and_single() {
        assert_eq!(detokenize_join(&[]), "");
        assert_eq!(detokenize_join(&["ток"]), "ток");
    }

    #[test]
    fn glm_generate_greedy_deterministic() {
        let path = std::env::temp_dir().join(format!("poler-glm-{}.pqw", std::process::id()));
        let (builder, _) = synth_glm(9, 2, 32, 4, 1, 48, 64, 32, Quant::Int8, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        let a = model.generate(&[1, 2, 3], 8, &Sampling::Greedy, 0).unwrap();
        let b = model.generate(&[1, 2, 3], 8, &Sampling::Greedy, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
        assert!(a.iter().all(|&t| (t as usize) < model.vocab()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_int8_matches_fp32_reference() {
        // Дифференциал: инкрементальный int8-декодер (SIMD + KV-арена)
        // против наивного fp32-эталона (полный пересчёт, скалярные циклы).
        let path = std::env::temp_dir().join(format!(
            "poler-glm-diff-{}.pqw",
            std::process::id()
        ));
        let (builder, fp32) = synth_glm(21, 2, 32, 4, 1, 48, 64, 24, Quant::Int8, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        let ids: Vec<u32> = vec![1, 5, 2, 9, 3];
        let full = model.full_logits(&ids).unwrap();
        let reference = reference_glm_logits(&fp32, &ids);
        let cos = |a: &[f32], b: &[f32]| {
            let ip = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
            let na = a.iter().map(|v| v * v).sum::<f32>().sqrt();
            let nb = b.iter().map(|v| v * v).sum::<f32>().sqrt();
            ip / (na * nb)
        };
        for t in 0..ids.len() {
            let c = cos(&full[t], &reference[t]);
            assert!(c > 0.999, "t={t}: косинус логитов int8 vs fp32 = {c}");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_incremental_is_reproducible() {
        // Инвариант KV-кеша: перезапуск прохода с чистой ареной даёт
        // побитово те же логиты (никакого скрытого состояния между runs).
        let path = std::env::temp_dir().join(format!(
            "poler-glm-kv-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = synth_glm(21, 2, 32, 4, 2, 48, 64, 24, Quant::Int8, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        let ids: Vec<u32> = vec![1, 5, 2, 9, 3];
        let full = model.full_logits(&ids).unwrap();
        let mut kv = KvArena::new(2, 24, 2, 8);
        let mut logits = vec![0f32; 64];
        for (t, &tok) in ids.iter().enumerate() {
            model.forward_pos(tok, t, &mut kv, &mut logits).unwrap();
            for i in 0..64 {
                assert!(
                    (logits[i] - full[t][i]).abs() < 1e-6,
                    "t={t} i={i}: {} vs {}",
                    logits[i],
                    full[t][i]
                );
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_moe_generate_and_route_consistency() {
        let path = std::env::temp_dir().join(format!(
            "poler-glm-moe-{}.pqw",
            std::process::id()
        ));
        // 3 эксперта, топ-2: взвешенная комбинация проверяется косвенно —
        // генерация стабильна и не выходит за словарь.
        let (builder, _) = synth_glm(77, 2, 32, 4, 1, 48, 64, 32, Quant::Int8, 3, 2);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        assert_eq!(model.experts(), 3);
        let out = model.generate(&[4, 4, 4], 6, &Sampling::Greedy, 0).unwrap();
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|&t| (t as usize) < model.vocab()));
        let again = model.generate(&[4, 4, 4], 6, &Sampling::Greedy, 0).unwrap();
        assert_eq!(out, again, "MoE-генерация детерминирована");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_oversize_prompt_rejected() {
        let path = std::env::temp_dir().join(format!(
            "poler-glm-size-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = synth_glm(5, 1, 16, 2, 1, 24, 32, 8, Quant::Int8, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        let long: Vec<u32> = (0..10u32).collect();
        assert!(model.generate(&long, 2, &Sampling::Greedy, 0).is_err());
        assert!(model.generate(&[0], 0, &Sampling::Greedy, 0).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_int4_differential_vs_int8_shape() {
        // int4-модель обязана открываться и генерировать (точность —
        // вопрос конвертера; здесь — работоспособность пути).
        let path = std::env::temp_dir().join(format!(
            "poler-glm-i4-{}.pqw",
            std::process::id()
        ));
        let (builder, _) = synth_glm(13, 1, 16, 2, 1, 24, 32, 16, Quant::Int4, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        let out = model.generate(&[1, 2], 4, &Sampling::Greedy, 0).unwrap();
        assert_eq!(out.len(), 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_chatglm3_semantics_differential() {
        // ChatGLM3-семантика против эталона: GQA-блоки (head/(heads/kv)),
        // qkv-bias, послойный масштаб внимания, eps=1e-5. Движок обязан
        // воспроизводить эталон побитово-близко на всех этих модификациях.
        let path = std::env::temp_dir().join(format!(
            "poler-glm-cglm3-{}.pqw",
            std::process::id()
        ));
        // Int8 (как в эталонном дифференциале): изолируем именно
        // семантику ChatGLM3, а не шум Int4-квантования.
        let (builder, fp32) = synth_glm(101, 2, 32, 4, 2, 48, 64, 24, Quant::Int8, 0, 0);
        builder
            .with_qk_layer_scaling()
            .with_rms_eps(1e-5)
            .write_to(&path)
            .unwrap();
        let mut fp32 = fp32;
        fp32.qk_layer_scaling = true;
        fp32.rms_eps = 1e-5;
        let model = GlmModel::open(&path).unwrap();
        assert!(
            model.view().header().qk_layer_scaling(),
            "флаг qk_layer_scaling не прочитан"
        );
        assert!(
            (model.view().header().rms_eps - 1e-5).abs() < 1e-12,
            "eps не прочитан из заголовка"
        );
        let ids: Vec<u32> = vec![1, 5, 2, 9, 3];
        let full = model.full_logits(&ids).unwrap();
        let reference = reference_glm_logits(&fp32, &ids);
        let cos = |a: &[f32], b: &[f32]| {
            let ip = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
            let na = a.iter().map(|v| v * v).sum::<f32>().sqrt();
            let nb = b.iter().map(|v| v * v).sum::<f32>().sqrt();
            ip / (na * nb)
        };
        for t in 0..ids.len() {
            let c = cos(&full[t], &reference[t]);
            assert!(c > 0.999, "t={t}: косинус ChatGLM3-семантики = {c}");
        }
        // GQA-блоки реально влияют на выход: с чередованием (старый баг)
        // косинус с эталоном был бы < 1 при kv_heads=2 < heads.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn glm_legacy_files_default_semantics() {
        // Старые .pqw (без новых флагов): eps=1e-6, без послойного
        // масштабирования — обратная совместимость формата.
        let path = std::env::temp_dir().join(format!(
            "poler-glm-legacy-{}.pqw",
            std::process::id()
        ));
        let (builder, fp32) = synth_glm(33, 1, 16, 2, 1, 24, 32, 16, Quant::Int8, 0, 0);
        builder.write_to(&path).unwrap();
        let model = GlmModel::open(&path).unwrap();
        assert!(!model.view().header().qk_layer_scaling());
        assert!((model.view().header().rms_eps - 1e-6).abs() < 1e-12);
        let ids: Vec<u32> = vec![3, 1, 4];
        let full = model.full_logits(&ids).unwrap();
        let reference = reference_glm_logits(&fp32, &ids);
        let cos = |a: &[f32], b: &[f32]| {
            let ip = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
            let na = a.iter().map(|v| v * v).sum::<f32>().sqrt();
            let nb = b.iter().map(|v| v * v).sum::<f32>().sqrt();
            ip / (na * nb)
        };
        for t in 0..ids.len() {
            let c = cos(&full[t], &reference[t]);
            assert!(c > 0.999, "t={t}: косинус legacy-семантики = {c}");
        }
        let _ = std::fs::remove_file(&path);
    }
}


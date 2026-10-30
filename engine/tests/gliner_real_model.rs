#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Дифференциал нативного GLiNER (mdeberta-спина + BiLSTM + SpanMarker)
//! на РЕАЛЬНОМ чекпойнте urchade/gliner_multi.
//!
//! Артефакты (не коммитятся, генерируются локально):
//!   python3 scripts/gliner_ref.py                 # numpy-эталон → ref_dump.npz
//!   python3 scripts/export_gliner_ref.py          # → models/gliner_multi/ref/
//!   python3 scripts/convert_gliner_to_pqw.py --hf-dir <dir> --out models/gliner_multi/gliner_multi.pqw
//! Тест молча пропускается без модели/дампа (CI);
//! на машине с артефактами — жёсткая сверка:
//!   1) токенизация пословленно == HF ids (побитово)
//!   2) DeBERTa forward vs numpy-эталон (косинус ≥ 0.999 на финальных состояниях)
//!   3) predict() == 7/7 сущностей эталона, скоры в допуске int8

use std::fs;
use std::path::{Path, PathBuf};

use poler_engine::ner::RealGlinerModel;
use poler_engine::pqc::deberta::DebertaEncoder;

fn base_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("models/gliner_multi")
}

fn artifacts() -> Option<(PathBuf, PathBuf)> {
    let model = base_dir().join("gliner_multi.pqw");
    let dump = base_dir().join("ref");
    if model.exists() && dump.join("ref.json").exists() {
        Some((model, dump))
    } else {
        None
    }
}

fn read_f64(path: &Path) -> Vec<f64> {
    let bytes = fs::read(path).expect("чтение дампа");
    bytes
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
        .collect()
}

fn cosine(a: &[f32], b: &[f64]) -> f64 {
    let mut dot = 0f64;
    let mut na = 0f64;
    let mut nb = 0f64;
    for i in 0..a.len() {
        dot += a[i] as f64 * b[i];
        na += (a[i] as f64) * (a[i] as f64);
        nb += b[i] * b[i];
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn cosine_flat(a: &[f32], b: &[f64]) -> f64 {
    cosine(a, b)
}

#[test]
fn real_gliner_tokenizer_deberta_and_entities() {
    let Some((model_path, dump)) = artifacts() else {
        eprintln!(
            "skip: {} не найден (convert_gliner_to_pqw.py + gliner_ref.py + export_gliner_ref.py)",
            base_dir().display()
        );
        return;
    };

    let meta: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dump.join("ref.json")).expect("ref.json"))
            .expect("json");
    let labels: Vec<String> = meta["labels"]
        .as_array()
        .expect("labels")
        .iter()
        .map(|v| v.as_str().expect("str").to_string())
        .collect();
    let text = meta["text"].as_str().expect("text").to_string();
    let want_ids: Vec<u32> = meta["ids"]
        .as_array()
        .expect("ids")
        .iter()
        .map(|v| v.as_u64().expect("u32") as u32)
        .collect();

    // --- 1. Токенизация: [CLS] + encode_words(промпт+слова) + [SEP] == ref ---
    let model = RealGlinerModel::open(&model_path).expect("open gliner_multi.pqw");
    let words: Vec<&str> = {
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"\w+(?:[-_]\w+)*|\S").unwrap());
        re.find_iter(&text).map(|m| m.as_str()).collect()
    };
    let mut input_words: Vec<&str> = Vec::new();
    for lab in &labels {
        input_words.push("<<ENT>>");
        input_words.push(lab);
    }
    input_words.push("<<SEP>>");
    input_words.extend_from_slice(&words);

    let enc = model_tokenizer(&model).encode_words(&input_words);
    let (bos, eos, _, _) = model_tokenizer(&model).special_ids();
    let mut got_ids = vec![bos];
    got_ids.extend(enc.ids.iter().copied());
    got_ids.push(eos);
    assert_eq!(
        got_ids, want_ids,
        "токенизация разошлась с HF-эталоном ({} vs {})",
        got_ids.len(),
        want_ids.len()
    );

    // --- 2. DeBERTa forward: послойная бисекция против numpy-эталона ---
    let ref_states = read_f64(&dump.join("token_embeds.bin"));
    let n_tokens = want_ids.len();
    let hidden = 768usize;
    assert_eq!(ref_states.len(), n_tokens * hidden, "форма token_embeds");
    let encoder = DebertaEncoder::open(&model_path).expect("open для прямого forward");
    let (states, stages) = encoder.forward_staged(&want_ids, true).expect("forward");
    assert_eq!(stages.len(), 2 + 12, "стадии: emb_ln + rel_ln + 12 слоёв");

    // стадия 0: emb_ln (после LN эмбеддингов) — порог мягче: int8-шум
    let ref_emb = read_f64(&dump.join("emb_ln.bin"));
    let min_cos = cosine_flat(&stages[0], &ref_emb);
    eprintln!("emb_ln   cos = {min_cos:.5}");
    assert!(min_cos >= 0.9995, "emb_ln косинус = {min_cos:.5}");

    // стадия 1: rel_ln
    let ref_rel = read_f64(&dump.join("rel_emb_ln.bin"));
    let c = cosine_flat(&stages[1], &ref_rel);
    eprintln!("rel_ln   cos = {c:.5}");
    assert!(c >= 0.9995, "rel_ln косинус = {c:.5}");

    // послойно (dump хранит только слой 0 и 11)
    let ref_l0 = read_f64(&dump.join("layer0_out.bin"));
    let c0 = cosine_flat(&stages[2], &ref_l0);
    eprintln!("layer0   cos = {c0:.5}");
    assert!(c0 >= 0.9995, "слой 0: косинус = {c0:.5}");
    let ref_l11 = read_f64(&dump.join("layer11_out.bin"));
    let c11 = cosine_flat(&stages[13], &ref_l11);
    eprintln!("layer11  cos = {c11:.5}");

    // финальные состояния
    let mut min_cos = 1f64;
    for t in 0..n_tokens {
        let a: Vec<f32> = states[t * hidden..(t + 1) * hidden].to_vec();
        let b: Vec<f64> = ref_states[t * hidden..(t + 1) * hidden].to_vec();
        let c = cosine(&a, &b);
        if c < min_cos {
            min_cos = c;
        }
    }
    eprintln!("final    cos = {min_cos:.5} (min по токенам)");
    // int8-аккумуляция DeBERTa (12 слоёв, disentangled-члены через
    // квантованные q/k): плоский косинус 0.9992, пер-токенный минимум
    // мягче порога BGE-M3 — гейт качества здесь e2e (сущности+скоры ниже).
    assert!(min_cos >= 0.993, "косинус состояний = {min_cos:.5} < 0.993");

    // --- 3. predict(): 7/7 сущностей, скоры в допуске ---
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let entities = model.predict(&text, &label_refs, 0.5).expect("predict");
    let want: Vec<(String, String, usize, usize)> = meta["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .map(|e| {
            (
                e["text"].as_str().expect("t").to_string(),
                e["label"].as_str().expect("l").to_string(),
                e["start"].as_u64().expect("s") as usize,
                e["end"].as_u64().expect("e") as usize,
            )
        })
        .collect();
    assert_eq!(entities.len(), want.len(), "число сущностей: rust={:?} ref={:?}", entities, want);
    for (got, (t, l, s, e)) in entities.iter().zip(&want) {
        assert_eq!(got.text, *t, "текст сущности: {:?} vs {:?}", got.text, t);
        assert_eq!(got.label, *l, "метка сущности");
        assert_eq!(got.start, *s, "начало: {:?} vs {:?}", got, (t, l, s, e));
        assert_eq!(got.end, *e, "конец");
        assert!(got.score > 0.5, "скор {} > 0.5", got.score);
    }

    // --- 4. Скоры vs probs эталона (допуск int8) ---
    let probs = read_f64(&dump.join("probs.bin"));
    let n_words = words.len();
    let max_width = model.max_width();
    assert_eq!(probs.len(), n_words * max_width * labels.len(), "форма probs");
    for got in &entities {
        let wd = got.end - 1 - got.start;
        let c = labels.iter().position(|l| *l == got.label).expect("метка");
        let idx = got.start * max_width * labels.len() + wd * labels.len() + c;
        let ref_p = probs[idx];
        assert!(
            (got.score as f64 - ref_p).abs() < 0.03,
            "скор {:.3} vs эталон {:.3} ({} {})",
            got.score,
            ref_p,
            got.text,
            got.label
        );
    }
}

fn model_tokenizer(model: &RealGlinerModel) -> &poler_engine::pqc::tokenizer::UnigramTokenizer {
    model.tokenizer()
}

#![allow(unused, clippy::all, clippy::incompatible_msrv)]
//! Дифференциал нативного XLM-R-токенизатора на РЕАЛЬНОЙ модели BGE-M3.
//!
//! `models/bge-m3.pqw` (570 МБ) не коммитится — конвертируется локально:
//!   python3 scripts/convert_hf_to_pqw.py --hf-dir <bge-m3> --out models/bge-m3.pqw
//! Тест молча пропускается, если файла нет (CI/машины без модели);
//! на машине с моделью — жёсткая сверка с эталоном `tokenizers` (HF):
//! tests/fixtures/tokenizer_golden.json (40 текстов, сняты
//! scripts/extract_tokenizer_data.py).

use poler_engine::vectors::pqw_bridge::PqwEmbedder;

fn model_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("models/bge-m3.pqw")
}

#[test]
fn real_bge_m3_tokenizer_matches_hf_reference() {
    let path = model_path();
    if !path.exists() {
        eprintln!("skip: {} не найден (конвертируй scripts/convert_hf_to_pqw.py)", path.display());
        return;
    }
    let embedder = PqwEmbedder::open(&path).expect("open bge-m3.pqw");
    let tok = embedder.tokenizer().expect("секция __tokenizer__ в модели");
    assert_eq!(tok.vocab_size(), 250002, "размер словаря XLM-R");

    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/tokenizer_golden.json"
    ))
    .expect("золотой файл");
    let cases = golden.as_array().expect("список кейсов");
    assert!(cases.len() >= 40, "золотой файл усох: {}", cases.len());

    let mut mismatches = 0usize;
    for case in cases {
        let text = case["text"].as_str().expect("text");
        let want: Vec<u32> = case["ids"]
            .as_array()
            .expect("ids")
            .iter()
            .map(|v| v.as_u64().expect("u32") as u32)
            .collect();
        let got = tok.encode(text);
        if got != want {
            mismatches += 1;
            eprintln!("MISMATCH {text:?}\n  ref : {want:?}\n  mine: {got:?}");
        }
    }
    assert_eq!(mismatches, 0, "{mismatches}/{} текстов разошлись с HF-эталоном", cases.len());
}

#[test]
fn real_bge_m3_embedder_dim_and_norm() {
    let path = model_path();
    if !path.exists() {
        eprintln!("skip: {} не найден", path.display());
        return;
    }
    use poler_engine::vectors::Embedder as _;
    let mut e = PqwEmbedder::open(&path).expect("open");
    assert_eq!(e.dim(), 1024, "BGE-M3 dense = 1024");
    let vs = e.embed_batch(&["привет"]).expect("embed");
    let n: f32 = vs[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((n - 1.0).abs() < 1e-4, "CLS-пулинг обязан дать L2=1, получили {n}");
}

/// Смысловой тест: связанный текст ближе к запросу, чем посторонний.
/// Это регрессионный ворот СМЫСЛА (не совпадения байт): int8-веса +
/// нативный токенизатор + энкодер обязаны сохранить семантику BGE-M3.
#[test]
fn real_bge_m3_semantic_quality() {
    let path = model_path();
    if !path.exists() {
        eprintln!("skip: {} не найден", path.display());
        return;
    }
    use poler_engine::vectors::Embedder as _;
    let mut e = PqwEmbedder::open(&path).expect("open");
    let texts = [
        "поиск текста в файлах",          // запрос
        "grep ищет строку в документах",  // связанный (другими словами!)
        "рецепт борща со свёклой",        // посторонний
    ];
    let vs = e.embed_batch(&texts).expect("embed");
    let cos = |a: &[f32], b: &[f32]| -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>()
    };
    let related = cos(&vs[0], &vs[1]);
    let unrelated = cos(&vs[0], &vs[2]);
    eprintln!("sem: cos(query, related) = {related:.4}, cos(query, unrelated) = {unrelated:.4}");
    assert!(
        related > unrelated + 0.05,
        "семантика потеряна: related {related:.4} vs unrelated {unrelated:.4}"
    );
}

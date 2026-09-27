//! Real-model smoke test of `OnnxEmbedder` (ignored by default: the model and ONNX Runtime live
//! outside the repository). Run it with
//!
//! ```sh
//! STRATA_EMBED_MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2 \
//! STRATA_ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0 \
//!   cargo test -p strata-ai --test onnx_real -- --ignored
//! ```
//!
//! (docs/RUNBOOK.md §10 describes how to obtain both.)
#![cfg(feature = "onnx")]
#![allow(clippy::expect_used)] // tests: expect with messages

use std::path::PathBuf;

use strata_ai::embed::onnx::{HfEncoder, OnnxEmbedder, OnnxEmbedderConfig, TextEncoder};
use strata_ai::embed::pooling::dot;
use strata_ai::{CpuGate, Embedder};

fn config() -> OnnxEmbedderConfig {
    let dir = std::env::var("STRATA_EMBED_MODEL_DIR")
        .unwrap_or_else(|_| "/opt/models/granite-embedding-97m-multilingual-r2".into());
    let lib = std::env::var("STRATA_ONNXRUNTIME_LIB")
        .unwrap_or_else(|_| "/opt/onnxruntime/lib/libonnxruntime.so.1.30.0".into());
    OnnxEmbedderConfig::granite_97m_r2_quint8(PathBuf::from(dir), PathBuf::from(lib))
}

#[derive(serde::Deserialize)]
struct Reference {
    text: String,
    ids: Vec<u32>,
    vector: Vec<f32>,
}

#[tokio::test]
#[ignore = "needs the granite model and libonnxruntime (see the module docs)"]
async fn granite_matches_the_python_reference_for_arabic_english_and_mixed_text() {
    let cfg = config();
    let refs: Vec<Reference> = serde_json::from_str(include_str!(
        "fixtures/embeddings/granite_quint8_reference.json"
    ))
    .expect("reference fixture");
    let texts: Vec<String> = refs.iter().map(|r| r.text.clone()).collect();

    // Tokenisation matches the Python tokenizers package exactly (CLS … SEP).
    let encoder = HfEncoder::load(&cfg.model_dir.join(&cfg.tokenizer_file), cfg.max_tokens)
        .expect("tokenizer");
    for r in &refs {
        assert_eq!(encoder.encode(&r.text).expect("ids"), r.ids, "{}", r.text);
    }
    assert_eq!(encoder.pad_id(), 179_935);

    let e = OnnxEmbedder::load(&cfg, CpuGate::new()).expect("model loads");
    assert_eq!(
        (e.dims(), e.model_id()),
        (
            384,
            "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model_quint8_avx2"
        )
    );
    assert_eq!(e.worker_priority().await, Ok(19));
    let out = e.embed(&texts).await.expect("embed");
    assert_eq!(out.len(), refs.len());
    for (v, r) in out.iter().zip(&refs) {
        assert_eq!(v.model_id, e.model_id());
        assert_eq!(v.vector.len(), 384);
        assert!(
            (dot(&v.vector, &v.vector) - 1.0).abs() < 1e-5,
            "unit length"
        );
        let cos = dot(&v.vector, &r.vector);
        assert!(cos > 0.9999, "{}: cosine {cos} to the reference", r.text);
    }
    let sim = |a: usize, b: usize| dot(&out[a].vector, &out[b].vector);
    eprintln!(
        "en~ar {:.4}  en~mixed {:.4}  ar~mixed {:.4}  en~banana {:.4}  ar~banana {:.4}",
        sim(0, 1),
        sim(0, 2),
        sim(1, 2),
        sim(0, 3),
        sim(1, 3)
    );
    // Embedded alone equals embedded in a call with others (no padding across lengths).
    let alone = e.embed(&texts[1..2]).await.expect("embed");
    assert!(dot(&alone[0].vector, &out[1].vector) > 0.99999);
}

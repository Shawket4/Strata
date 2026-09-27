//! `OnnxEmbedder`: in-process ONNX inference with `ort` + `tokenizers` (D9 = a).
//!
//! - One dedicated worker thread owns the session and runs every batch. At start it lowers its
//!   own scheduling priority (`setpriority(PRIO_PROCESS, gettid(), nice)`, per-thread on
//!   Linux), and ONNX Runtime runs with one intra-op and one inter-op thread, so inference
//!   happens on that low-priority thread only and API requests keep priority (§9.1b).
//! - Each [`Embedder::embed`] call holds the exclusive side of the [`CpuGate`] for its whole
//!   duration: one call at a time, never concurrently with a `claude -p` process.
//! - Texts are truncated to `max_tokens` and grouped into batches whose padded size stays under
//!   `max_batch_tokens`, bounding peak memory on the 4 GB VPS.
//! - ONNX Runtime is loaded at run time from `onnxruntime_lib` (`ort` feature `load-dynamic`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc;

use tokio::sync::oneshot;

use super::pooling::{PaddedBatch, l2_normalize, pad, plan_batches, pool};
use super::{EmbedError, Embedder, Embedding, Pooling};
use crate::gate::CpuGate;

/// Configuration of [`OnnxEmbedder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxEmbedderConfig {
    /// Model directory (the Hugging Face repository layout).
    pub model_dir: PathBuf,
    /// ONNX file, relative to `model_dir`.
    pub model_file: PathBuf,
    /// `tokenizer.json`, relative to `model_dir`.
    pub tokenizer_file: PathBuf,
    /// `libonnxruntime.so` (ONNX Runtime ≥ 1.22).
    pub onnxruntime_lib: PathBuf,
    /// Model ID stored with every vector.
    pub model_id: String,
    /// Output dimensions.
    pub dims: usize,
    /// Pooling.
    pub pooling: Pooling,
    /// Tokens per text (longer texts are truncated; special tokens included).
    pub max_tokens: usize,
    /// Padded tokens per batch.
    pub max_batch_tokens: usize,
    /// Nice value of the worker thread (0–19; 19 = lowest priority).
    pub nice: i32,
}

impl OnnxEmbedderConfig {
    /// IBM granite-embedding-97m-multilingual-r2, int8 (quint8, AVX2) ONNX export as published
    /// in the model repository (`onnx/model_quint8_avx2.onnx`), CLS pooling, 384 dimensions.
    pub fn granite_97m_r2(model_dir: PathBuf, onnxruntime_lib: PathBuf) -> Self {
        Self {
            model_dir,
            model_file: PathBuf::from("onnx/model_quint8_avx2.onnx"),
            tokenizer_file: PathBuf::from("tokenizer.json"),
            onnxruntime_lib,
            model_id: "ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model_quint8_avx2"
                .to_owned(),
            dims: 384,
            pooling: Pooling::Cls,
            max_tokens: 2048,
            max_batch_tokens: 8192,
            nice: 19,
        }
    }
}

/// Turns text into token IDs (special tokens included, truncated).
pub trait TextEncoder: Send + 'static {
    /// Token IDs of `text`.
    fn encode(&self, text: &str) -> Result<Vec<u32>, EmbedError>;
    /// Padding token ID.
    fn pad_id(&self) -> u32;
}

/// Runs the model on a padded batch.
pub trait EmbeddingSession: Send + 'static {
    /// The first output as `(shape, values)`: `[batch, seq, dim]` token states or
    /// `[batch, dim]` sentence vectors.
    fn run(&mut self, batch: &PaddedBatch) -> Result<(Vec<usize>, Vec<f32>), EmbedError>;
}

/// Pipeline settings shared by the real and test sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineSettings {
    /// Model ID.
    pub model_id: String,
    /// Dimensions.
    pub dims: usize,
    /// Pooling.
    pub pooling: Pooling,
    /// Padded tokens per batch.
    pub max_batch_tokens: usize,
    /// Worker nice value.
    pub nice: i32,
}

enum Job {
    Embed {
        texts: Vec<String>,
        reply: oneshot::Sender<Result<Vec<Vec<f32>>, EmbedError>>,
    },
    Priority {
        reply: oneshot::Sender<Result<i32, EmbedError>>,
    },
}

/// The ONNX embedder.
#[derive(Debug)]
pub struct OnnxEmbedder {
    model_id: String,
    dims: usize,
    gate: CpuGate,
    jobs: mpsc::Sender<Job>,
}

impl OnnxEmbedder {
    /// Loads the model described by `cfg` on a new worker thread.
    pub fn load(cfg: &OnnxEmbedderConfig, gate: CpuGate) -> Result<Self, EmbedError> {
        let settings = PipelineSettings {
            model_id: cfg.model_id.clone(),
            dims: cfg.dims,
            pooling: cfg.pooling,
            max_batch_tokens: cfg.max_batch_tokens,
            nice: cfg.nice,
        };
        let cfg = cfg.clone();
        Self::spawn(settings, gate, move || {
            let encoder = HfEncoder::load(&cfg.model_dir.join(&cfg.tokenizer_file), cfg.max_tokens)?;
            let session = OrtSession::load(&cfg.onnxruntime_lib, &cfg.model_dir.join(&cfg.model_file))?;
            Ok((Box::new(encoder) as Box<dyn TextEncoder>, Box::new(session) as Box<dyn EmbeddingSession>))
        })
    }

    /// An embedder over the given parts (tests use a fake session and encoder).
    pub fn with_parts(
        encoder: Box<dyn TextEncoder>,
        session: Box<dyn EmbeddingSession>,
        settings: PipelineSettings,
        gate: CpuGate,
    ) -> Result<Self, EmbedError> {
        Self::spawn(settings, gate, move || Ok((encoder, session)))
    }

    fn spawn(
        settings: PipelineSettings,
        gate: CpuGate,
        init: impl FnOnce() -> Result<(Box<dyn TextEncoder>, Box<dyn EmbeddingSession>), EmbedError> + Send + 'static,
    ) -> Result<Self, EmbedError> {
        let (jobs, rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), EmbedError>>();
        let worker_settings = settings.clone();
        std::thread::Builder::new()
            .name("strata-embed".into())
            .spawn(move || {
                lower_priority(worker_settings.nice);
                let (encoder, mut session) = match init() {
                    Ok(parts) => {
                        let _ = ready_tx.send(Ok(()));
                        parts
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Embed { texts, reply } => {
                            let out = embed_texts(encoder.as_ref(), session.as_mut(), &worker_settings, &texts);
                            let _ = reply.send(out);
                        }
                        Job::Priority { reply } => {
                            let p = rustix::process::getpriority_process(Some(rustix::thread::gettid()))
                                .map_err(|e| EmbedError::Inference(format!("getpriority: {e}")));
                            let _ = reply.send(p);
                        }
                    }
                }
            })
            .map_err(|e| EmbedError::Load(format!("worker thread: {e}")))?;
        ready_rx.recv().map_err(|_| EmbedError::Stopped)??;
        Ok(Self {
            model_id: settings.model_id,
            dims: settings.dims,
            gate,
            jobs,
        })
    }

    /// The worker thread's current nice value.
    pub async fn worker_priority(&self) -> Result<i32, EmbedError> {
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Priority { reply })
            .map_err(|_| EmbedError::Stopped)?;
        rx.await.map_err(|_| EmbedError::Stopped)?
    }
}

/// Lowers the calling thread's priority (Linux: the nice value is per thread).
fn lower_priority(nice: i32) {
    if let Err(e) = rustix::process::setpriority_process(Some(rustix::thread::gettid()), nice) {
        tracing::warn!(error = %e, "could not lower embedding worker priority");
    }
}

/// Tokenise, batch, run, pool, normalise.
fn embed_texts(
    encoder: &dyn TextEncoder,
    session: &mut dyn EmbeddingSession,
    s: &PipelineSettings,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, EmbedError> {
    let seqs: Vec<Vec<u32>> = texts
        .iter()
        .map(|t| encoder.encode(t))
        .collect::<Result<_, _>>()?;
    let lengths: Vec<usize> = seqs.iter().map(Vec::len).collect();
    let mut out = Vec::with_capacity(texts.len());
    for range in plan_batches(&lengths, s.max_batch_tokens) {
        let batch = pad(&seqs[range], encoder.pad_id());
        let (shape, values) = session.run(&batch)?;
        let mut vectors = match shape.as_slice() {
            [b, seq, dim] if *b == batch.batch && *seq == batch.seq => {
                pool(&values, &batch.attention_mask, *b, *seq, *dim, s.pooling)?
            }
            [b, dim] if *b == batch.batch && values.len() == b * dim => {
                values.chunks(*dim).map(<[f32]>::to_vec).collect()
            }
            other => {
                return Err(EmbedError::Output(format!(
                    "output shape {other:?} for a batch of {}×{}",
                    batch.batch, batch.seq
                )));
            }
        };
        for v in &mut vectors {
            if v.len() != s.dims {
                return Err(EmbedError::Output(format!("{} dimensions, expected {}", v.len(), s.dims)));
            }
            l2_normalize(v);
        }
        out.extend(vectors);
    }
    Ok(out)
}

#[async_trait::async_trait]
impl Embedder for OnnxEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dims(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let _exclusive = self.gate.embed().await;
        let (reply, rx) = oneshot::channel();
        self.jobs
            .send(Job::Embed {
                texts: texts.to_vec(),
                reply,
            })
            .map_err(|_| EmbedError::Stopped)?;
        let vectors = rx.await.map_err(|_| EmbedError::Stopped)??;
        Ok(vectors
            .into_iter()
            .map(|vector| Embedding {
                model_id: self.model_id.clone(),
                vector,
            })
            .collect())
    }
}

/// `tokenizers` encoder with truncation.
pub struct HfEncoder {
    tokenizer: tokenizers::Tokenizer,
    pad_id: u32,
}

impl std::fmt::Debug for HfEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HfEncoder").field("pad_id", &self.pad_id).finish_non_exhaustive()
    }
}

impl HfEncoder {
    /// Loads `tokenizer.json`; truncates to `max_tokens` (special tokens included).
    pub fn load(path: &Path, max_tokens: usize) -> Result<Self, EmbedError> {
        let tokenizer = tokenizers::Tokenizer::from_file(path)
            .map_err(|e| EmbedError::Load(format!("tokenizer {}: {e}", path.display())))?;
        Self::new(tokenizer, max_tokens)
    }

    /// Wraps a tokenizer (padding is done by the pipeline, not the tokenizer).
    pub fn new(mut tokenizer: tokenizers::Tokenizer, max_tokens: usize) -> Result<Self, EmbedError> {
        let pad_id = tokenizer.get_padding().map_or(0, |p| p.pad_id);
        tokenizer.with_padding(None);
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: max_tokens,
                ..tokenizers::TruncationParams::default()
            }))
            .map_err(|e| EmbedError::Load(format!("truncation: {e}")))?;
        Ok(Self { tokenizer, pad_id })
    }
}

impl TextEncoder for HfEncoder {
    fn encode(&self, text: &str) -> Result<Vec<u32>, EmbedError> {
        self.tokenizer
            .encode(text, true)
            .map(|e| e.get_ids().to_vec())
            .map_err(|e| EmbedError::Tokenize(e.to_string()))
    }

    fn pad_id(&self) -> u32 {
        self.pad_id
    }
}

static ORT_INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// ONNX Runtime session.
pub struct OrtSession {
    session: ort::session::Session,
    inputs: Vec<String>,
}

impl std::fmt::Debug for OrtSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrtSession").field("inputs", &self.inputs).finish_non_exhaustive()
    }
}

impl OrtSession {
    /// Loads ONNX Runtime from `lib` (once per process) and the model at `model`.
    pub fn load(lib: &Path, model: &Path) -> Result<Self, EmbedError> {
        ORT_INIT
            .get_or_init(|| {
                ort::init_from(lib)
                    .map_err(|e| format!("onnxruntime {}: {e}", lib.display()))?
                    .with_name("strata-embed")
                    .with_telemetry(false)
                    .commit();
                Ok(())
            })
            .clone()
            .map_err(EmbedError::Load)?;
        let load = |e: ort::Error<ort::session::builder::SessionBuilder>| EmbedError::Load(e.to_string());
        let session = ort::session::Session::builder()
            .map_err(|e| EmbedError::Load(e.to_string()))?
            .with_intra_threads(1)
            .map_err(load)?
            .with_inter_threads(1)
            .map_err(load)?
            .with_parallel_execution(false)
            .map_err(load)?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(load)?
            .commit_from_file(model)
            .map_err(|e| EmbedError::Load(format!("model {}: {e}", model.display())))?;
        let inputs: Vec<String> = session.inputs().iter().map(|i| i.name().to_owned()).collect();
        for required in ["input_ids", "attention_mask"] {
            if !inputs.iter().any(|n| n == required) {
                return Err(EmbedError::Load(format!("model has no input {required}: {inputs:?}")));
            }
        }
        Ok(Self { session, inputs })
    }
}

impl EmbeddingSession for OrtSession {
    fn run(&mut self, batch: &PaddedBatch) -> Result<(Vec<usize>, Vec<f32>), EmbedError> {
        let err = |e: ort::Error| EmbedError::Inference(e.to_string());
        let shape = [batch.batch, batch.seq];
        let mut values: Vec<(String, ort::session::SessionInputValue<'_>)> = Vec::new();
        for name in &self.inputs {
            let data = match name.as_str() {
                "input_ids" => batch.input_ids.clone(),
                "attention_mask" => batch.attention_mask.clone(),
                "token_type_ids" => vec![0; batch.input_ids.len()],
                other => return Err(EmbedError::Inference(format!("unsupported model input {other}"))),
            };
            let tensor = ort::value::Tensor::from_array((shape, data)).map_err(err)?;
            values.push((name.clone(), tensor.into()));
        }
        let outputs = self.session.run(values).map_err(err)?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(err)?;
        let shape: Vec<usize> = shape.iter().map(|&d| usize::try_from(d).unwrap_or(0)).collect();
        Ok((shape, data.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;
    use pretty_assertions::assert_eq;

    use super::*;

    /// Splits on whitespace; token = word length; adds CLS=1 and SEP=2.
    struct WordLenEncoder;

    impl TextEncoder for WordLenEncoder {
        fn encode(&self, text: &str) -> Result<Vec<u32>, EmbedError> {
            let mut ids = vec![1];
            ids.extend(text.split_whitespace().map(|w| u32::try_from(w.chars().count()).unwrap_or(0) + 10));
            ids.push(2);
            Ok(ids)
        }
        fn pad_id(&self) -> u32 {
            0
        }
    }

    /// Hidden state of token t = [id, id * position, 1]; records batch shapes.
    struct FakeSession {
        shapes: std::sync::Arc<std::sync::Mutex<Vec<(usize, usize)>>>,
    }

    impl EmbeddingSession for FakeSession {
        fn run(&mut self, batch: &PaddedBatch) -> Result<(Vec<usize>, Vec<f32>), EmbedError> {
            self.shapes.lock().expect("lock").push((batch.batch, batch.seq));
            let mut v = Vec::new();
            for b in 0..batch.batch {
                for t in 0..batch.seq {
                    #[allow(clippy::cast_precision_loss)]
                    let id = batch.input_ids[b * batch.seq + t] as f32;
                    #[allow(clippy::cast_precision_loss)]
                    let pos = t as f32;
                    v.extend([id, id * pos, 1.0]);
                }
            }
            Ok((vec![batch.batch, batch.seq, 3], v))
        }
    }

    fn embedder(pooling: Pooling, max_batch_tokens: usize) -> (OnnxEmbedder, std::sync::Arc<std::sync::Mutex<Vec<(usize, usize)>>>, CpuGate) {
        let shapes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let gate = CpuGate::new();
        let e = OnnxEmbedder::with_parts(
            Box::new(WordLenEncoder),
            Box::new(FakeSession { shapes: shapes.clone() }),
            PipelineSettings {
                model_id: "fake-model@1".into(),
                dims: 3,
                pooling,
                max_batch_tokens,
                nice: 19,
            },
            gate.clone(),
        )
        .expect("spawn");
        (e, shapes, gate)
    }

    #[tokio::test]
    async fn cls_pooling_normalises_and_tags_every_vector_with_the_model_id() {
        let (e, shapes, _) = embedder(Pooling::Cls, 100);
        let out = e
            .embed(&["a bb".into(), "ccc".into()])
            .await
            .expect("embed");
        // CLS token id 1 at position 0 → [1, 0, 1] / sqrt(2) for both texts.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(
            out,
            vec![
                Embedding { model_id: "fake-model@1".into(), vector: vec![h, 0.0, h] },
                Embedding { model_id: "fake-model@1".into(), vector: vec![h, 0.0, h] },
            ]
        );
        assert_eq!(*shapes.lock().expect("lock"), vec![(2, 4)]);
        assert_eq!(e.model_id(), "fake-model@1");
        assert_eq!(e.dims(), 3);
    }

    #[tokio::test]
    async fn mean_pooling_ignores_padding_and_batches_follow_the_token_budget() {
        let (e, shapes, _) = embedder(Pooling::Mean, 6);
        let out = e
            .embed(&["a".into(), "bb".into(), "a b c d".into()])
            .await
            .expect("embed");
        // "a" → ids [1, 11, 2]: mean of [1,0,1],[11,11,1],[2,4,1] = [14/3, 5, 1]
        let mut first = vec![14.0 / 3.0, 5.0, 1.0];
        l2_normalize(&mut first);
        assert_eq!(out[0].vector, first);
        // Lengths 3, 3, 6 with a budget of 6 padded tokens: [0, 1] (2×3 = 6), then [2] (1×6).
        assert_eq!(*shapes.lock().expect("lock"), vec![(2, 3), (1, 6)]);
        assert_eq!(out.len(), 3);
        for v in &out {
            assert!((crate::embed::pooling::dot(&v.vector, &v.vector) - 1.0).abs() < 1e-6);
        }
    }

    #[tokio::test]
    async fn embedding_waits_while_a_claude_call_holds_the_gate() {
        let (e, _, gate) = embedder(Pooling::Cls, 100);
        let llm = gate.llm().await;
        let texts = vec!["x".to_owned()];
        let mut fut = Box::pin(e.embed(&texts));
        assert!((&mut fut).now_or_never().is_none(), "blocked by the llm guard");
        drop(llm);
        assert_eq!(fut.await.expect("embed").len(), 1);
    }

    #[tokio::test]
    async fn worker_thread_runs_at_the_configured_nice_value() {
        let (e, _, _) = embedder(Pooling::Cls, 100);
        assert_eq!(e.worker_priority().await, Ok(19));
    }

    #[tokio::test]
    async fn wrong_dimensions_are_an_output_error() {
        let shapes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let e = OnnxEmbedder::with_parts(
            Box::new(WordLenEncoder),
            Box::new(FakeSession { shapes }),
            PipelineSettings { model_id: "m".into(), dims: 384, pooling: Pooling::Cls, max_batch_tokens: 100, nice: 19 },
            CpuGate::new(),
        )
        .expect("spawn");
        assert_eq!(
            e.embed(&["x".into()]).await,
            Err(EmbedError::Output("3 dimensions, expected 384".into()))
        );
        assert_eq!(e.embed(&[]).await, Ok(vec![]));
    }

    /// A tiny WordLevel tokenizer with a CLS/SEP template, as in granite's tokenizer.json.
    const TINY_TOKENIZER: &str = r#"{
      "version": "1.0",
      "truncation": null,
      "padding": {"strategy": "BatchLongest", "direction": "Right", "pad_to_multiple_of": null, "pad_id": 3, "pad_type_id": 0, "pad_token": "<pad>"},
      "added_tokens": [
        {"id": 0, "content": "<cls>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
        {"id": 1, "content": "<sep>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
        {"id": 3, "content": "<pad>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}
      ],
      "normalizer": null,
      "pre_tokenizer": {"type": "Whitespace"},
      "post_processor": {
        "type": "TemplateProcessing",
        "single": [{"SpecialToken": {"id": "<cls>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 0}}, {"SpecialToken": {"id": "<sep>", "type_id": 0}}],
        "pair": [{"Sequence": {"id": "A", "type_id": 0}}, {"Sequence": {"id": "B", "type_id": 1}}],
        "special_tokens": {
          "<cls>": {"id": "<cls>", "ids": [0], "tokens": ["<cls>"]},
          "<sep>": {"id": "<sep>", "ids": [1], "tokens": ["<sep>"]}
        }
      },
      "decoder": null,
      "model": {"type": "WordLevel", "vocab": {"<cls>": 0, "<sep>": 1, "<unk>": 2, "<pad>": 3, "عقد": 4, "وطنية": 5, "contract": 6, "safe": 7}, "unk_token": "<unk>"}
    }"#;

    #[test]
    fn hf_encoder_adds_special_tokens_truncates_and_reads_the_pad_id() {
        let tok = tokenizers::Tokenizer::from_bytes(TINY_TOKENIZER).expect("tokenizer");
        let enc = HfEncoder::new(tok, 4).expect("encoder");
        assert_eq!(enc.pad_id(), 3);
        assert_eq!(enc.encode("عقد وطنية").expect("ids"), vec![0, 4, 5, 1]);
        assert_eq!(enc.encode("contract in the safe").expect("ids"), vec![0, 6, 2, 1]);
        assert_eq!(enc.encode("").expect("ids"), vec![0, 1]);
    }

    #[test]
    fn missing_runtime_library_is_a_load_error() {
        let cfg = OnnxEmbedderConfig::granite_97m_r2("/nonexistent/model".into(), "/nonexistent/libonnxruntime.so".into());
        let err = OnnxEmbedder::load(&cfg, CpuGate::new()).expect_err("no model");
        assert!(matches!(err, EmbedError::Load(_)), "{err:?}");
    }
}

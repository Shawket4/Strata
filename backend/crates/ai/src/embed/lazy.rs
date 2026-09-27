//! `LazyEmbedder`: loads the model on demand and unloads it after an idle period (PLAN §9.1b).
//!
//! The fp32 granite model takes ≈ 390 MB while loaded; the VPS has 4 GB. The embedder is
//! therefore only resident while it is being used: the first [`Embedder::embed`] call loads
//! it (a second or two, on the blocking pool while holding the exclusive side of the
//! [`CpuGate`], so a load never overlaps a `claude -p` process either), and
//! [`LazyEmbedder::unload_if_idle`] drops it once no call has run for `idle` (the composition
//! root calls it periodically through [`LazyEmbedder::spawn_reaper`]). Dropping the last
//! handle of the inner embedder stops its worker thread and frees the model.
//!
//! Time comes from the injected [`Clock`], so the idle rule is tested with a fake clock.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, Utc};
use strata_common::Clock;

use super::{EmbedError, Embedder, Embedding};
use crate::gate::CpuGate;

/// Builds the inner embedder. Called on the blocking pool; may block while the model loads.
pub type EmbedderLoader = Arc<dyn Fn() -> Result<Arc<dyn Embedder>, EmbedError> + Send + Sync>;

#[derive(Default)]
struct Slot {
    embedder: Option<Arc<dyn Embedder>>,
    in_flight: usize,
    last_used: Option<DateTime<Utc>>,
}

/// An [`Embedder`] that loads its model on first use and unloads it when idle.
pub struct LazyEmbedder {
    model_id: String,
    dims: usize,
    loader: EmbedderLoader,
    clock: Arc<dyn Clock>,
    idle: chrono::Duration,
    gate: Option<CpuGate>,
    slot: Mutex<Slot>,
    load_lock: tokio::sync::Mutex<()>,
    loads: AtomicU64,
    unloads: AtomicU64,
}

impl fmt::Debug for LazyEmbedder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyEmbedder")
            .field("model_id", &self.model_id)
            .field("dims", &self.dims)
            .field("idle", &self.idle)
            .field("loaded", &self.is_loaded())
            .finish_non_exhaustive()
    }
}

/// Decrements the in-flight count and stamps the last use, even when the call is cancelled.
struct InFlight<'a>(&'a LazyEmbedder);

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        let now = self.0.clock.now();
        let mut slot = self.0.lock();
        slot.in_flight = slot.in_flight.saturating_sub(1);
        slot.last_used = Some(now);
    }
}

impl LazyEmbedder {
    /// An embedder for `model_id` (`dims` dimensions) built by `loader` when first needed and
    /// dropped after `idle` without calls. With a `gate`, loading holds its exclusive side.
    pub fn new(
        model_id: impl Into<String>,
        dims: usize,
        loader: EmbedderLoader,
        clock: Arc<dyn Clock>,
        idle: Duration,
        gate: Option<CpuGate>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            dims,
            loader,
            clock,
            idle: chrono::Duration::from_std(idle).unwrap_or(chrono::Duration::MAX),
            gate,
            slot: Mutex::new(Slot::default()),
            load_lock: tokio::sync::Mutex::new(()),
            loads: AtomicU64::new(0),
            unloads: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The loaded embedder with the in-flight count raised, if loaded.
    fn enter(&self) -> Option<Arc<dyn Embedder>> {
        let mut slot = self.lock();
        let e = slot.embedder.clone()?;
        slot.in_flight += 1;
        Some(e)
    }

    async fn load(&self) -> Result<Arc<dyn Embedder>, EmbedError> {
        let _gate = match &self.gate {
            Some(g) => Some(g.embed().await),
            None => None,
        };
        let loader = self.loader.clone();
        let started = std::time::Instant::now();
        let embedder = tokio::task::spawn_blocking(move || loader())
            .await
            .map_err(|e| EmbedError::Load(format!("loader task: {e}")))??;
        if embedder.model_id() != self.model_id || embedder.dims() != self.dims {
            return Err(EmbedError::Load(format!(
                "loaded model {} ({} dims), expected {} ({} dims)",
                embedder.model_id(),
                embedder.dims(),
                self.model_id,
                self.dims
            )));
        }
        self.loads.fetch_add(1, Ordering::Relaxed);
        tracing::info!(
            model = %self.model_id,
            millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "embedding model loaded"
        );
        Ok(embedder)
    }

    /// Unloads the model when it is loaded, no call is running, and the last call ended at
    /// least `idle` ago. Returns whether it unloaded.
    pub fn unload_if_idle(&self) -> bool {
        let now = self.clock.now();
        let dropped = {
            let mut slot = self.lock();
            let idle_long_enough = slot
                .last_used
                .is_none_or(|at| now.signed_duration_since(at) >= self.idle);
            if slot.embedder.is_some() && slot.in_flight == 0 && idle_long_enough {
                slot.embedder.take()
            } else {
                None
            }
        };
        match dropped {
            Some(embedder) => {
                // Dropping the last handle stops the inner worker and frees the model.
                drop(embedder);
                self.unloads.fetch_add(1, Ordering::Relaxed);
                tracing::info!(model = %self.model_id, "embedding model unloaded after idle period");
                true
            }
            None => false,
        }
    }

    /// Times the model was loaded.
    pub fn loads(&self) -> u64 {
        self.loads.load(Ordering::Relaxed)
    }

    /// Times the model was unloaded for being idle.
    pub fn unloads(&self) -> u64 {
        self.unloads.load(Ordering::Relaxed)
    }

    /// Calls [`Self::unload_if_idle`] every `period` until the returned task is aborted.
    pub fn spawn_reaper(self: &Arc<Self>, period: Duration) -> tokio::task::JoinHandle<()> {
        let me = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(period);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                me.unload_if_idle();
            }
        })
    }
}

#[async_trait::async_trait]
impl Embedder for LazyEmbedder {
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
        let embedder = if let Some(e) = self.enter() {
            e
        } else {
            let _loading = self.load_lock.lock().await;
            if let Some(e) = self.enter() {
                e
            } else {
                let e = self.load().await?;
                let mut slot = self.lock();
                slot.embedder = Some(e.clone());
                slot.in_flight += 1;
                e
            }
        };
        let _in_flight = InFlight(self);
        embedder.embed(texts).await
    }

    fn is_loaded(&self) -> bool {
        self.lock().embedder.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use futures::FutureExt;
    use pretty_assertions::assert_eq;
    use strata_common::FakeClock;

    use super::*;

    /// Returns `[len, 1]` normalised; counts live instances (dropped when unloaded).
    #[derive(Debug)]
    struct Counting {
        live: Arc<AtomicUsize>,
        /// `(started, release)`: signals `started`, then waits for `release`.
        gate: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
    }

    impl Drop for Counting {
        fn drop(&mut self) {
            self.live.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl Embedder for Counting {
        fn model_id(&self) -> &'static str {
            "m@1"
        }
        fn dims(&self) -> usize {
            2
        }
        async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, EmbedError> {
            if let Some((started, release)) = &self.gate {
                started.notify_one();
                release.notified().await;
            }
            Ok(texts
                .iter()
                .map(|_| Embedding {
                    model_id: "m@1".into(),
                    vector: vec![1.0, 0.0],
                })
                .collect())
        }
    }

    fn lazy(
        clock: &FakeClock,
        live: &Arc<AtomicUsize>,
        gate: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
    ) -> LazyEmbedder {
        let live2 = live.clone();
        let loader: EmbedderLoader = Arc::new(move || {
            live2.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(Counting {
                live: live2.clone(),
                gate: gate.clone(),
            }) as Arc<dyn Embedder>)
        });
        LazyEmbedder::new(
            "m@1",
            2,
            loader,
            Arc::new(clock.clone()),
            Duration::from_secs(300),
            None,
        )
    }

    #[tokio::test]
    async fn loads_on_first_use_and_unloads_after_the_idle_period() {
        let clock = FakeClock::at_default_epoch();
        let live = Arc::new(AtomicUsize::new(0));
        let e = lazy(&clock, &live, None);
        assert_eq!((e.is_loaded(), e.loads(), live.load(Ordering::SeqCst)), (false, 0, 0));
        assert_eq!((e.model_id(), e.dims()), ("m@1", 2));
        // Nothing to unload before the first use.
        assert!(!e.unload_if_idle());

        let out = e.embed(&["a".into()]).await.expect("embed");
        assert_eq!(out[0].vector, vec![1.0, 0.0]);
        assert_eq!((e.is_loaded(), e.loads(), live.load(Ordering::SeqCst)), (true, 1, 1));
        // A second call reuses the loaded model.
        e.embed(&["b".into()]).await.expect("embed");
        assert_eq!(e.loads(), 1);

        clock.advance(chrono::Duration::seconds(299));
        assert!(!e.unload_if_idle(), "idle 299 s < 300 s");
        assert!(e.is_loaded());
        clock.advance(chrono::Duration::seconds(1));
        assert!(e.unload_if_idle(), "idle 300 s");
        assert_eq!((e.is_loaded(), e.unloads(), live.load(Ordering::SeqCst)), (false, 1, 0));
        assert!(!e.unload_if_idle(), "already unloaded");

        // The next call loads it again.
        e.embed(&["c".into()]).await.expect("embed");
        assert_eq!((e.is_loaded(), e.loads(), live.load(Ordering::SeqCst)), (true, 2, 1));
        // Empty input never loads anything.
        clock.advance(chrono::Duration::seconds(300));
        assert!(e.unload_if_idle());
        assert_eq!(e.embed(&[]).await, Ok(vec![]));
        assert_eq!((e.is_loaded(), e.loads()), (false, 2));
    }

    #[tokio::test]
    async fn a_running_call_keeps_the_model_and_the_idle_period_restarts_after_it() {
        let clock = FakeClock::at_default_epoch();
        let live = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let e = Arc::new(lazy(&clock, &live, Some((started.clone(), release.clone()))));
        let call = tokio::spawn({
            let e = e.clone();
            async move { e.embed(&["x".to_owned()]).await }
        });
        started.notified().await;
        assert!(e.is_loaded());
        clock.advance(chrono::Duration::seconds(3600));
        assert!(!e.unload_if_idle(), "in flight");
        release.notify_one();
        call.await.expect("task").expect("embed");
        // Finished just now: not idle yet.
        assert!(!e.unload_if_idle());
        clock.advance(chrono::Duration::seconds(300));
        assert!(e.unload_if_idle());
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn load_failures_are_returned_and_retried_on_the_next_call() {
        let clock = FakeClock::at_default_epoch();
        let attempts = Arc::new(AtomicUsize::new(0));
        let a2 = attempts.clone();
        let loader: EmbedderLoader = Arc::new(move || {
            a2.fetch_add(1, Ordering::SeqCst);
            Err(EmbedError::Load("model file missing".into()))
        });
        let e = LazyEmbedder::new(
            "m@1",
            2,
            loader,
            Arc::new(clock.clone()),
            Duration::from_secs(300),
            Some(CpuGate::new()),
        );
        assert_eq!(
            e.embed(&["a".into()]).await,
            Err(EmbedError::Load("model file missing".into()))
        );
        assert_eq!(
            e.embed(&["a".into()]).await,
            Err(EmbedError::Load("model file missing".into()))
        );
        assert_eq!((attempts.load(Ordering::SeqCst), e.loads(), e.is_loaded()), (2, 0, false));
    }

    #[tokio::test]
    async fn a_loaded_model_with_another_id_is_refused() {
        let clock = FakeClock::at_default_epoch();
        let live = Arc::new(AtomicUsize::new(0));
        let live2 = live.clone();
        let loader: EmbedderLoader = Arc::new(move || {
            live2.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(Counting {
                live: live2.clone(),
                gate: None,
            }) as Arc<dyn Embedder>)
        });
        let e = LazyEmbedder::new(
            "other@2",
            2,
            loader,
            Arc::new(clock),
            Duration::from_secs(1),
            None,
        );
        assert_eq!(
            e.embed(&["a".into()]).await,
            Err(EmbedError::Load(
                "loaded model m@1 (2 dims), expected other@2 (2 dims)".into()
            ))
        );
        assert_eq!(live.load(Ordering::SeqCst), 0, "the wrong model was dropped");
    }

    #[tokio::test]
    async fn loading_waits_for_a_running_claude_call() {
        let clock = FakeClock::at_default_epoch();
        let live = Arc::new(AtomicUsize::new(0));
        let live2 = live.clone();
        let loader: EmbedderLoader = Arc::new(move || {
            live2.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(Counting {
                live: live2.clone(),
                gate: None,
            }) as Arc<dyn Embedder>)
        });
        let gate = CpuGate::new();
        let e = LazyEmbedder::new(
            "m@1",
            2,
            loader,
            Arc::new(clock),
            Duration::from_secs(300),
            Some(gate.clone()),
        );
        let llm = gate.llm().await;
        let texts = vec!["x".to_owned()];
        let mut call = Box::pin(e.embed(&texts));
        assert!((&mut call).now_or_never().is_none());
        assert_eq!(live.load(Ordering::SeqCst), 0, "not loaded while claude runs");
        drop(llm);
        call.await.expect("embed");
        assert_eq!(live.load(Ordering::SeqCst), 1);
    }
}

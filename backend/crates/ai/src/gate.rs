//! CPU gate between `claude -p` calls and local embedding batches (PLAN §9.1b).
//!
//! The VPS has one core. Embedding batches run one at a time and never while a `claude -p`
//! process is running; `claude -p` calls may overlap each other (their own concurrency limit
//! applies). That is a readers–writer lock: CLI calls take the shared side, an embedding batch
//! takes the exclusive side. Tokio's `RwLock` is fair (FIFO), so neither side starves.

use std::sync::Arc;

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// Shared gate; clones refer to the same lock.
#[derive(Debug, Clone, Default)]
pub struct CpuGate {
    lock: Arc<RwLock<()>>,
}

impl CpuGate {
    /// A new, open gate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Held for the lifetime of one `claude -p` process.
    pub async fn llm(&self) -> OwnedRwLockReadGuard<()> {
        Arc::clone(&self.lock).read_owned().await
    }

    /// Held for one embedding batch.
    pub async fn embed(&self) -> OwnedRwLockWriteGuard<()> {
        Arc::clone(&self.lock).write_owned().await
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;

    use super::*;

    #[tokio::test]
    async fn embedding_waits_for_llm_calls_and_llm_calls_wait_for_embedding() {
        let gate = CpuGate::new();
        let llm_a = gate.llm().await;
        // Two CLI calls may overlap.
        let llm_b = gate.llm().now_or_never().expect("second llm guard is immediate");

        let mut embed = Box::pin(gate.embed());
        assert!((&mut embed).now_or_never().is_none(), "embedding must wait");
        drop(llm_a);
        assert!((&mut embed).now_or_never().is_none(), "still one llm call");
        drop(llm_b);
        let embed_guard = embed.now_or_never().expect("embedding proceeds");

        assert!(
            gate.llm().now_or_never().is_none(),
            "llm waits for the batch"
        );
        assert!(gate.embed().now_or_never().is_none(), "one batch at a time");
        drop(embed_guard);
        assert!(gate.llm().now_or_never().is_some());
    }
}

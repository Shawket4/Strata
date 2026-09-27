//! Op receipts: a sync op's result stored atomically with the write that applies it (PLAN
//! §7.5 Sync, §16.4 "replayed pushes are idempotent").
//!
//! The push layer wraps each op in an [`OpReceipt`] scope and, right before the vault call
//! whose write completes the op, arms it with a [`ResultHook`]. The write that consumes the
//! hook (the first one in that call) runs it **after its index update, inside the same scoped
//! transaction**, with an [`AfterWrite`] view of the vault after the write; the result bytes
//! are inserted into `idempotency` in that transaction, recorded in the write's git commit
//! as trailers, and handed back to the push layer ([`OpReceipt::settled`]) once the
//! transaction committed:
//!
//! ```text
//! user: create notes/A.md
//!
//! Strata-Op: 01J…            (the op ID)
//! Strata-Device: 01J…        (the pushing device)
//! Strata-Result: gqZzdGF0…   (base64url of the MessagePack result, no padding)
//! ```
//!
//! A crash between the git commit and the database commit leaves the commit (with the result)
//! but neither the index update nor the `idempotency` row; reconciliation finds the commits
//! after `sync_epochs.vault_head` and re-inserts their results
//! ([`crate::reconcile`]), so the replayed op is answered with the original bytes and not
//! applied again. A crash before the git commit is undone by the write journal (see
//! [`crate::store`]), so the replay applies the op exactly once.
//!
//! Writes that only touch the database (suggestion replies, conflict suggestions, suggestion
//! decisions) settle the receipt in their own transaction the same way (no trailer).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use strata_common::{DeviceId, NoteId, OpId};
use strata_index::ScopedTx;
use strata_index::repo::sync as log;

use crate::error::{Result, VaultError};
use crate::state::VaultState;

/// Builds an op's encoded result from the vault as the write left it.
pub type ResultHook = Box<dyn FnOnce(&AfterWrite<'_>) -> std::result::Result<Vec<u8>, String> + Send>;

tokio::task_local! {
    static RECEIPT: Arc<Slot>;
}

/// The receipt state shared by the push layer and the writer.
pub(crate) struct Slot {
    op_id: OpId,
    device: DeviceId,
    inner: Mutex<SlotState>,
}

#[derive(Default)]
struct SlotState {
    hook: Option<ResultHook>,
    settled: Option<Vec<u8>>,
}

impl Slot {
    fn state(&self) -> std::sync::MutexGuard<'_, SlotState> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn take_hook(&self) -> Option<ResultHook> {
        self.state().hook.take()
    }

    /// Records the bytes stored by a committed transaction.
    pub(crate) fn mark_settled(&self, bytes: Vec<u8>) {
        self.state().settled = Some(bytes);
    }
}

/// The receipt of one pushed op (see the module docs).
#[derive(Clone)]
pub struct OpReceipt {
    slot: Arc<Slot>,
}

impl std::fmt::Debug for OpReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpReceipt")
            .field("op_id", &self.slot.op_id)
            .field("device", &self.slot.device)
            .finish_non_exhaustive()
    }
}

impl OpReceipt {
    /// A receipt for `op_id` pushed by `device`.
    pub fn new(op_id: OpId, device: DeviceId) -> Self {
        Self {
            slot: Arc::new(Slot {
                op_id,
                device,
                inner: Mutex::new(SlotState::default()),
            }),
        }
    }

    /// Runs `fut` with this receipt in scope: vault calls made inside see it.
    pub async fn scope<F: Future>(&self, fut: F) -> F::Output {
        RECEIPT.scope(self.slot.clone(), fut).await
    }

    /// Arms the receipt: the next write in scope stores the result `hook` builds.
    pub fn arm(&self, hook: ResultHook) {
        self.slot.state().hook = Some(hook);
    }

    /// Disarms it (a call that did not write leaves the hook unused).
    pub fn disarm(&self) {
        self.slot.state().hook = None;
    }

    /// Runs `fut` armed with `hook`, then disarms.
    pub async fn armed<F: Future>(&self, hook: ResultHook, fut: F) -> F::Output {
        self.arm(hook);
        let out = fut.await;
        self.disarm();
        out
    }

    /// Records that the caller stored `bytes` for this op in a database-only transaction of
    /// its own that committed (ops that never reach the vault writer).
    pub fn mark_settled(&self, bytes: Vec<u8>) {
        self.slot.mark_settled(bytes);
    }

    /// The op ID.
    pub fn op_id(&self) -> OpId {
        self.slot.op_id
    }

    /// The pushing device.
    pub fn device(&self) -> DeviceId {
        self.slot.device
    }

    /// The result bytes a committed write stored for this op, if one did.
    pub fn settled(&self) -> Option<Vec<u8>> {
        self.slot.state().settled.clone()
    }
}

/// The receipt in scope of the current task, if any.
pub(crate) fn current() -> Option<Arc<Slot>> {
    RECEIPT.try_with(Clone::clone).ok()
}

/// A write settled in a transaction that has not committed yet.
pub(crate) struct Pending {
    pub(crate) slot: Arc<Slot>,
    pub(crate) bytes: Vec<u8>,
}

impl Pending {
    /// The commit-message trailers recording this result.
    pub(crate) fn trailers(&self) -> String {
        format!(
            "\n\n{OP_TRAILER}{}\n{DEVICE_TRAILER}{}\n{RESULT_TRAILER}{}",
            self.slot.op_id,
            self.slot.device,
            URL_SAFE_NO_PAD.encode(&self.bytes)
        )
    }

    /// After the transaction committed: the push layer may use the bytes.
    pub(crate) fn commit(self) {
        self.slot.mark_settled(self.bytes);
    }
}

/// Runs the armed hook of `slot` (if any) against `after` and stores the result under the
/// op ID in `tx`. Returns what to record once `tx` commits.
pub(crate) async fn settle(
    slot: Option<&Arc<Slot>>,
    tx: &mut ScopedTx,
    after: &AfterWrite<'_>,
    now: DateTime<Utc>,
) -> Result<Option<Pending>> {
    let Some(slot) = slot else { return Ok(None) };
    let Some(hook) = slot.take_hook() else {
        return Ok(None);
    };
    let bytes = hook(after).map_err(|e| VaultError::Internal(format!("op result: {e}")))?;
    let stored = log::idempotency_put(tx, slot.op_id, slot.device, &bytes, now).await?;
    Ok(Some(Pending {
        slot: slot.clone(),
        bytes: stored.result,
    }))
}

/// The vault as a write left it (before its git commit), for [`ResultHook`]s.
#[derive(Debug, Clone, Copy)]
pub struct AfterWrite<'a> {
    state: Option<&'a VaultState>,
    files: Option<&'a BTreeMap<String, Option<Vec<u8>>>>,
}

impl<'a> AfterWrite<'a> {
    /// A write that changed `files` and left the writer state `state`.
    pub(crate) fn new(
        state: Option<&'a VaultState>,
        files: &'a BTreeMap<String, Option<Vec<u8>>>,
    ) -> Self {
        Self {
            state,
            files: Some(files),
        }
    }

    /// A database-only write (no files changed; no writer state at hand).
    pub(crate) const fn database_only() -> Self {
        Self {
            state: None,
            files: None,
        }
    }

    /// Path and version of the live note `id`.
    pub fn note(&self, id: NoteId) -> Option<(&'a str, &'a str)> {
        self.state?
            .note(id)
            .map(|(p, m)| (p, m.version.as_str()))
    }

    /// Version of the live note `id`.
    pub fn note_version(&self, id: NoteId) -> Option<String> {
        self.note(id).map(|(_, v)| v.to_owned())
    }

    /// Line version of the task `block_id` in a note this write changed.
    pub fn task_line_version(&self, block_id: &str) -> Option<String> {
        self.files?.iter().find_map(|(path, content)| {
            let bytes = content.as_ref().filter(|_| path.ends_with(".md"))?;
            let text = String::from_utf8_lossy(bytes);
            let doc = vault_format::Document::parse(&text);
            sync_model::apply::find_task(doc.body(), block_id).map(|(span, _)| {
                sync_model::apply::task_line_version(&doc.body()[span])
                    .as_str()
                    .to_owned()
            })
        })
    }
}

const OP_TRAILER: &str = "Strata-Op: ";
const DEVICE_TRAILER: &str = "Strata-Device: ";
const RESULT_TRAILER: &str = "Strata-Result: ";

/// An op result recorded in a commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The op.
    pub op_id: OpId,
    /// The device that pushed it.
    pub device: DeviceId,
    /// The encoded result.
    pub result: Vec<u8>,
}

/// The op result recorded in a raw commit `message`, if it has the three trailers.
pub fn parse_trailers(message: &str) -> Option<Recorded> {
    let (_, block) = message.rsplit_once("\n\n")?;
    let mut op_id = None;
    let mut device = None;
    let mut result = None;
    for line in block.lines() {
        if let Some(v) = line.strip_prefix(OP_TRAILER) {
            op_id = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix(DEVICE_TRAILER) {
            device = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix(RESULT_TRAILER) {
            result = URL_SAFE_NO_PAD.decode(v.trim()).ok();
        }
    }
    Some(Recorded {
        op_id: op_id?,
        device: device?,
        result: result?,
    })
}

/// `message` without a trailing `Strata-*` trailer block (what history shows).
pub fn strip_trailers(message: &str) -> &str {
    match message.rsplit_once("\n\n") {
        Some((head, block))
            if !block.is_empty() && block.lines().all(|l| l.starts_with("Strata-")) =>
        {
            head
        }
        _ => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn pending(bytes: &[u8]) -> Pending {
        Pending {
            slot: Arc::new(Slot {
                op_id: "01J9ZZZZZZZZZZZZZZZZZZZZZ1".parse().expect("op"),
                device: "01J9ZZZZZZZZZZZZZZZZZZZZZ2".parse().expect("device"),
                inner: Mutex::new(SlotState::default()),
            }),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn trailers_round_trip_and_strip() {
        let message = format!("user: create notes/A.md{}", pending(&[0x81, 0xa1, 0x61, 0xc3]).trailers());
        assert_eq!(
            message,
            "user: create notes/A.md\n\nStrata-Op: 01J9ZZZZZZZZZZZZZZZZZZZZZ1\n\
             Strata-Device: 01J9ZZZZZZZZZZZZZZZZZZZZZ2\nStrata-Result: gaFhww"
        );
        assert_eq!(
            parse_trailers(&message),
            Some(Recorded {
                op_id: "01J9ZZZZZZZZZZZZZZZZZZZZZ1".parse().expect("op"),
                device: "01J9ZZZZZZZZZZZZZZZZZZZZZ2".parse().expect("device"),
                result: vec![0x81, 0xa1, 0x61, 0xc3],
            })
        );
        assert_eq!(strip_trailers(&message), "user: create notes/A.md");
    }

    #[test]
    fn messages_without_trailers_are_left_alone() {
        assert_eq!(parse_trailers("user: create notes/A.md"), None);
        assert_eq!(strip_trailers("user: create notes/A.md"), "user: create notes/A.md");
        let body = "system: recovered changes\n\nfree text";
        assert_eq!(parse_trailers(body), None);
        assert_eq!(strip_trailers(body), body);
        // An incomplete block is not a recorded result.
        assert_eq!(
            parse_trailers("user: x\n\nStrata-Op: 01J9ZZZZZZZZZZZZZZZZZZZZZ1"),
            None
        );
    }

    #[test]
    fn after_write_finds_task_line_versions_in_changed_files() {
        let mut files = BTreeMap::new();
        files.insert(
            "notes/T.md".to_owned(),
            Some(b"- [ ] Pay rent ^t-01j9aaaaaaaaaaaaaaaaaaaaaa\n".to_vec()),
        );
        files.insert(".meta/notes/x.json".to_owned(), Some(b"{}".to_vec()));
        files.insert("notes/Gone.md".to_owned(), None);
        let after = AfterWrite::new(None, &files);
        let body = "- [ ] Pay rent ^t-01j9aaaaaaaaaaaaaaaaaaaaaa\n";
        let (span, _) =
            sync_model::apply::find_task(body, "t-01j9aaaaaaaaaaaaaaaaaaaaaa").expect("task");
        let expected = sync_model::apply::task_line_version(&body[span])
            .as_str()
            .to_owned();
        assert!(expected.starts_with("sha256:"), "{expected}");
        assert_eq!(
            after.task_line_version("t-01j9aaaaaaaaaaaaaaaaaaaaaa"),
            Some(expected)
        );
        assert_eq!(after.task_line_version("t-missing"), None);
        assert_eq!(after.note_version(NoteId::from_ulid(ulid::Ulid::nil())), None);
        assert_eq!(AfterWrite::database_only().task_line_version("t-x"), None);
    }
}

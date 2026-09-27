//! Per-op push results (`POST /sync/push`, §7.5, §12.4).

use dedupe::DuplicateCandidate;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{OpError, SyncOp, Version};

/// Minimal RFC 7807 problem carried by a rejected op (the API maps it to its problem DTO).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// Problem type slug (`version_conflict`, `invalid_name`, `not_found`, …).
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short human-readable summary.
    pub title: String,
    /// HTTP status the same failure would have on the REST endpoint.
    pub status: u16,
    /// Details.
    pub detail: Option<String>,
}

/// How the server settled a version conflict (D19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConflictResolution {
    /// The edits overlapped: the server kept its version and saved the client's content as
    /// a conflict copy note next to it, so nothing is lost. The client shows the conflict
    /// (hunks from [`crate::merge()`]) and resolves it with a normal update.
    ConflictCopy {
        /// ID of the conflict copy note.
        note_id: Ulid,
        /// Its path.
        path: String,
        /// Its version.
        version: Version,
    },
    /// The op cannot apply to the current server state (e.g. a move of a note the server
    /// already moved or deleted); the server state stands.
    ServerKept {
        /// Why.
        reason: String,
    },
}

/// Result of one op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OpResult {
    /// Applied. `merged` is true when the server 3-way merged a stale edit cleanly (D19).
    Applied {
        /// The entity's new version (`None` for ops without a versioned entity).
        new_version: Option<Version>,
        /// Whether a clean 3-way merge was needed.
        #[serde(default)]
        merged: bool,
    },
    /// The base version was stale and the edit could not be merged cleanly.
    Conflict {
        /// The server's current version.
        server_version: Option<Version>,
        /// What the server did.
        resolution: ConflictResolution,
    },
    /// A create without `force` matched existing items (§9.7).
    Duplicate {
        /// Ranked candidates.
        candidates: Vec<DuplicateCandidate>,
    },
    /// The op was invalid or not allowed.
    Rejected {
        /// Why.
        problem: Problem,
    },
}

/// Result of one op, keyed by its idempotency key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpOutcome {
    /// The op.
    pub op_id: Ulid,
    /// Its result (stored under `op_id`; replays return it unchanged).
    pub result: OpResult,
}

/// `POST /sync/push` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushRequest {
    /// Ops, applied in order.
    pub ops: Vec<SyncOp>,
}

/// `POST /sync/push` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushResponse {
    /// One result per op, in request order.
    pub results: Vec<OpOutcome>,
}

/// A push response that does not answer its request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PushMismatch {
    /// Different number of results.
    #[error("{ops} ops but {results} results")]
    Count {
        /// Ops sent.
        ops: usize,
        /// Results received.
        results: usize,
    },
    /// A result is for another op.
    #[error("result {index} is for op {got}, expected {expected}")]
    OpId {
        /// Position.
        index: usize,
        /// Expected op ID.
        expected: Ulid,
        /// Received op ID.
        got: Ulid,
    },
}

impl PushRequest {
    /// Validates every op envelope ([`SyncOp::validate`]); returns the first error with its
    /// index.
    pub fn validate(&self) -> Result<(), (usize, OpError)> {
        for (i, op) in self.ops.iter().enumerate() {
            op.validate().map_err(|e| (i, e))?;
        }
        Ok(())
    }
}

impl PushResponse {
    /// Checks that the results answer `request` one to one, in order.
    pub fn check_answers(&self, request: &PushRequest) -> Result<(), PushMismatch> {
        if self.results.len() != request.ops.len() {
            return Err(PushMismatch::Count {
                ops: request.ops.len(),
                results: self.results.len(),
            });
        }
        for (index, (op, res)) in request.ops.iter().zip(&self.results).enumerate() {
            if op.op_id != res.op_id {
                return Err(PushMismatch::OpId {
                    index,
                    expected: op.op_id,
                    got: res.op_id,
                });
            }
        }
        Ok(())
    }
}

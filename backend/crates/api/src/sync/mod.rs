//! Offline-first sync (PLAN §7.4 `change_log`/`idempotency`/`sync_epochs`, §7.5 Sync, §12.3–12.5,
//! D19): bootstrap snapshots, the changes feed and the op push. Handlers are in
//! [`crate::routes::sync`]; the design is written up in `docs/ARCHITECTURE.md`, "Sync and
//! events".
//!
//! - [`records`]: the records clients cache (`sync-model` [`sync_model::Record`]), read from
//!   the index and the vault files, for bootstrap pages and change records.
//! - [`push`]: applying pushed ops through the vault writer (one commit per op), the D19
//!   merge, duplicate checks, and the idempotency store.
//! - [`wire`]: the contract schemas mirroring the `sync-model` types the handlers encode.

pub mod push;
pub mod records;
pub mod wire;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use actix_web::web;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use strata_common::UserId;
use strata_vault::VaultService;

use crate::events::{BusConfig, EventBus};
use crate::wire::{Problem, ProblemType};

/// Limits of the sync endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncConfig {
    /// Records per bootstrap page when the client does not ask for fewer.
    pub bootstrap_page: u32,
    /// Largest `limit` of `GET /sync/changes` (and its default).
    pub changes_page: u32,
    /// Most ops in one push.
    pub max_ops: usize,
    /// Revisions searched for the base version of a stale update (D19).
    pub merge_history: usize,
    /// Body limit of `POST /sync/push`.
    pub push_body_limit: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            bootstrap_page: 200,
            changes_page: 500,
            max_ops: 500,
            merge_history: 200,
            push_body_limit: 16 * 1024 * 1024,
        }
    }
}

/// Shared sync state (`web::Data<SyncState>`): the event bus and one push lock per user, so a
/// user's pushes (and the idempotency check-then-store of each op) never interleave.
#[derive(Debug)]
pub struct SyncState {
    bus: Arc<EventBus>,
    locks: Mutex<HashMap<UserId, Arc<tokio::sync::Mutex<()>>>>,
    /// Limits.
    pub config: SyncConfig,
}

impl SyncState {
    /// State publishing to `bus`.
    pub fn new(bus: Arc<EventBus>, config: SyncConfig) -> Self {
        Self {
            bus,
            locks: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// The event bus.
    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }

    /// The push lock of `user`.
    pub fn lock(&self, user: UserId) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(user)
            .or_default()
            .clone()
    }
}

/// Wires sync and events into an app: creates the event bus, registers it as the vault's
/// commit listener and returns the app data to register (`web::Data<EventBus>`,
/// `web::Data<SyncState>`).
pub fn install(
    vault: &VaultService,
    bus: BusConfig,
    config: SyncConfig,
) -> (web::Data<EventBus>, web::Data<SyncState>) {
    let bus = Arc::new(EventBus::new(bus));
    vault.set_listener(bus.clone());
    let state = web::Data::new(SyncState::new(bus.clone(), config));
    (web::Data::from(bus), state)
}

/// Position of a bootstrap in progress (opaque to clients: base64url `MessagePack`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapCursor {
    /// Epoch of the snapshot.
    pub epoch: u64,
    /// Log position captured on the first page.
    pub seq: u64,
    /// Section being read.
    pub section: u8,
    /// Last key returned in that section.
    pub after: Option<String>,
}

impl BootstrapCursor {
    /// The opaque string.
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(crate::wire::encode(self).unwrap_or_default())
    }

    /// Parses a cursor from `GET /sync/bootstrap?cursor=`.
    pub fn decode(s: &str) -> Result<Self, Problem> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| invalid_cursor())?;
        rmp_serde::from_slice(&bytes).map_err(|_| invalid_cursor())
    }
}

fn invalid_cursor() -> Problem {
    crate::vault::invalid_parameter("cursor", "invalid_cursor", "the cursor is not valid")
}

/// `410 epoch_changed`.
pub fn epoch_changed(current: i32) -> Problem {
    Problem::new(ProblemType::EpochChanged).with_detail(format!(
        "the sync epoch is now {current}: bootstrap again"
    ))
}

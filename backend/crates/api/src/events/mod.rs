//! The per-user event bus behind `GET /events` (PLAN §7.5 Events, D24).
//!
//! - Producers publish [`Event`]s for one user: the vault store through its commit listener
//!   ([`EventBus`] implements [`CommitListener`], so notes, relations, entities, tasks, custody
//!   and suggestions arrive right after each committed write, in commit order), the sync push
//!   (conflict records), and anything else through [`EventBus::publish`] (jobs, clusters).
//! - Each user has a [`ReplayBuffer`] of the last events (so a reconnecting client resumes
//!   from its last seq) and a broadcast channel for live subscribers. Seqs are per user and
//!   per process; they start after the bus's base (the composition root uses the start time,
//!   so seqs of a restarted server never repeat old ones and a stale `resume_from` gets
//!   `reset`).
//! - A connection only ever subscribes to its authenticated user's channel (principle 7).
//! - Account state: the stream watches the revocation set. When the caller's account is
//!   disabled, scheduled for deletion or purged, the bus publishes one `account.disabled`
//!   event to that user's channel; every connection forwards it, then sends a terminal
//!   `error` frame and closes. A connection whose own session is revoked (sign-out, device
//!   removed) gets an `unauthorized` error frame and closes.
//!
//! Events carry IDs and new versions only; clients pull `GET /sync/changes` for data.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use futures_util::Stream;
use serde::{Deserialize, Serialize};
use strata_common::{SessionId, UserId};
use strata_vault::events::NoteChange;
use strata_vault::{CommitListener, Committed};
use tokio::sync::{broadcast, watch};
use ulid::Ulid;
use utoipa::ToSchema;

use crate::auth::RevocationSet;
use crate::auth::revocation::Revoked;
use crate::routes::inbox::SuggestionStatus;
use crate::routes::notes::NoteKind;
use crate::wire::ws::{Frame, ReplayBuffer};
use crate::wire::{Problem, ProblemType};

/// Why the account stream ended (`account.disabled`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountClosure {
    /// Disabled by an admin.
    Disabled,
    /// Scheduled for deletion (the account is export-only now, D25).
    DeletionPending,
    /// Deleted.
    Deleted,
}

/// One event of the `/events` stream. Internally tagged by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum Event {
    /// A note appeared (created, captured, imported, restored).
    #[serde(rename = "note.created")]
    NoteCreated {
        /// Note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Path.
        path: String,
        /// Kind.
        kind: NoteKind,
        /// New version.
        version: String,
    },
    /// A note's content changed.
    #[serde(rename = "note.updated")]
    NoteUpdated {
        /// Note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Path.
        path: String,
        /// Kind.
        kind: NoteKind,
        /// New version.
        version: String,
    },
    /// A note moved or was renamed.
    #[serde(rename = "note.moved")]
    NoteMoved {
        /// Note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Previous path.
        old_path: String,
        /// New path.
        path: String,
        /// Kind.
        kind: NoteKind,
        /// New version.
        version: String,
    },
    /// A note went to the trash (or was removed).
    #[serde(rename = "note.deleted")]
    NoteDeleted {
        /// Note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Last path.
        path: String,
        /// Kind.
        kind: NoteKind,
    },
    /// A relation appeared.
    #[serde(rename = "relation.added")]
    RelationAdded {
        /// Source note.
        #[schema(value_type = String, format = "ulid")]
        src_id: Ulid,
        /// Target note.
        #[schema(value_type = String, format = "ulid")]
        dst_id: Ulid,
        /// Relation type (`related`, `works-at`, …).
        relation: String,
    },
    /// A relation disappeared.
    #[serde(rename = "relation.removed")]
    RelationRemoved {
        /// Source note.
        #[schema(value_type = String, format = "ulid")]
        src_id: Ulid,
        /// Target note.
        #[schema(value_type = String, format = "ulid")]
        dst_id: Ulid,
        /// Relation type.
        relation: String,
    },
    /// A suggestion was created.
    #[serde(rename = "suggestion.created")]
    SuggestionCreated {
        /// Suggestion ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Note it concerns.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<String>, format = "ulid")]
        note_id: Option<Ulid>,
        /// Kind (`duplicate`, `conflict`, …).
        kind: String,
    },
    /// A suggestion was decided or replied to.
    #[serde(rename = "suggestion.updated")]
    SuggestionUpdated {
        /// Suggestion ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Note it concerns.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<String>, format = "ulid")]
        note_id: Option<Ulid>,
        /// Kind.
        kind: String,
        /// Status now.
        status: SuggestionStatus,
    },
    /// A background job finished.
    #[serde(rename = "job.completed")]
    JobCompleted {
        /// Job ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Job kind (`embed`, `link`, `file_inbox`, …).
        kind: String,
        /// Note it worked on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<String>, format = "ulid")]
        note_id: Option<Ulid>,
    },
    /// A background job failed for good.
    #[serde(rename = "job.failed")]
    JobFailed {
        /// Job ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Job kind.
        kind: String,
        /// Note it worked on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<String>, format = "ulid")]
        note_id: Option<Ulid>,
    },
    /// Cluster assignments or names changed.
    #[serde(rename = "cluster.updated")]
    ClusterUpdated {
        /// The clusters that changed.
        cluster_ids: Vec<String>,
    },
    /// A person, company, document or place was created.
    #[serde(rename = "entity.created")]
    EntityCreated {
        /// Entity note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Kind.
        kind: NoteKind,
        /// New version.
        version: String,
    },
    /// An entity note changed (fields, aliases, AI sections, rename).
    #[serde(rename = "entity.updated")]
    EntityUpdated {
        /// Entity note ID.
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// Kind.
        kind: NoteKind,
        /// New version.
        version: String,
    },
    /// An entity was merged into another.
    #[serde(rename = "entity.merged")]
    EntityMerged {
        /// The entity merged away (now in the trash).
        #[schema(value_type = String, format = "ulid")]
        id: Ulid,
        /// The survivor.
        #[schema(value_type = String, format = "ulid")]
        into_id: Ulid,
        /// Kind.
        kind: NoteKind,
        /// The survivor's new version.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// Reconciliation recorded integrity warnings (`GET /integrity`).
    #[serde(rename = "integrity.warning")]
    IntegrityWarning {
        /// Number of new warnings.
        count: u32,
    },
    /// The account was disabled, scheduled for deletion or deleted; the stream ends.
    #[serde(rename = "account.disabled")]
    AccountDisabled {
        /// Why.
        reason: AccountClosure,
    },
    /// A task line changed or disappeared.
    #[serde(rename = "task.changed")]
    TaskChanged {
        /// Block ID (`t-…`).
        id: String,
        /// The note holding the line.
        #[schema(value_type = String, format = "ulid")]
        note_id: Ulid,
        /// The line's version now; absent when the task is gone.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// A document's custody history changed.
    #[serde(rename = "custody.changed")]
    CustodyChanged {
        /// Document note.
        #[schema(value_type = String, format = "ulid")]
        document_id: Ulid,
        /// Its new version.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
    /// A device setting changed.
    #[serde(rename = "device.settings_changed")]
    DeviceSettingsChanged {
        /// Device.
        #[schema(value_type = String, format = "ulid")]
        device_id: Ulid,
    },
}

fn note_kind(k: domain::NoteKind) -> NoteKind {
    k.into()
}

fn status_of(s: &str) -> SuggestionStatus {
    match s {
        "accepted" => SuggestionStatus::Accepted,
        "rejected" => SuggestionStatus::Rejected,
        "superseded" => SuggestionStatus::Superseded,
        _ => SuggestionStatus::Pending,
    }
}

/// The events of one commit notice, in a fixed order: per note (ID order) its `note.*` event
/// then its `entity.*` event, then `entity.merged`, relations (removed/added, sorted), tasks,
/// custody, suggestions, integrity warnings.
#[allow(clippy::too_many_lines)] // one arm per event kind
pub fn events_of(c: &Committed) -> Vec<Event> {
    let mut out = Vec::new();
    let merged = c.merged;
    let mut survivor_version = None;
    for n in &c.notes {
        let id = n.id.as_ulid();
        let kind = note_kind(n.kind);
        let version = n.version.clone().unwrap_or_default();
        out.push(match n.change {
            NoteChange::Created => Event::NoteCreated {
                id,
                path: n.path.clone(),
                kind,
                version: version.clone(),
            },
            NoteChange::Updated => Event::NoteUpdated {
                id,
                path: n.path.clone(),
                kind,
                version: version.clone(),
            },
            NoteChange::Moved => Event::NoteMoved {
                id,
                old_path: n.old_path.clone().unwrap_or_default(),
                path: n.path.clone(),
                kind,
                version: version.clone(),
            },
            NoteChange::Deleted => Event::NoteDeleted {
                id,
                path: n.path.clone(),
                kind,
            },
        });
        if merged.is_some_and(|(_, s)| s == n.id) {
            survivor_version.clone_from(&n.version);
        }
        let in_merge = merged.is_some_and(|(l, s)| l == n.id || s == n.id);
        if n.kind.is_entity() && !in_merge {
            match n.change {
                NoteChange::Created => out.push(Event::EntityCreated { id, kind, version }),
                NoteChange::Updated | NoteChange::Moved => {
                    out.push(Event::EntityUpdated { id, kind, version });
                }
                NoteChange::Deleted => {}
            }
        }
    }
    if let Some((loser, survivor)) = merged {
        let kind = c
            .notes
            .iter()
            .find(|n| n.id == loser || n.id == survivor)
            .map_or(NoteKind::Person, |n| note_kind(n.kind));
        out.push(Event::EntityMerged {
            id: loser.as_ulid(),
            into_id: survivor.as_ulid(),
            kind,
            version: survivor_version,
        });
    }
    for r in &c.relations {
        let (src_id, dst_id, relation) = (r.src.as_ulid(), r.dst.as_ulid(), r.rel.clone());
        out.push(if r.added {
            Event::RelationAdded {
                src_id,
                dst_id,
                relation,
            }
        } else {
            Event::RelationRemoved {
                src_id,
                dst_id,
                relation,
            }
        });
    }
    for t in &c.tasks {
        out.push(Event::TaskChanged {
            id: t.id.clone(),
            note_id: t.note_id.as_ulid(),
            version: t.version.clone(),
        });
    }
    for d in &c.custody {
        out.push(Event::CustodyChanged {
            document_id: d.document_id.as_ulid(),
            version: d.version.clone(),
        });
    }
    for s in &c.suggestions {
        let (id, note_id, kind) = (
            s.id.as_ulid(),
            s.note_id.map(|n| n.as_ulid()),
            s.kind.clone(),
        );
        out.push(if s.created {
            Event::SuggestionCreated { id, note_id, kind }
        } else {
            Event::SuggestionUpdated {
                id,
                note_id,
                kind,
                status: status_of(&s.status),
            }
        });
    }
    if c.integrity_warnings > 0 {
        out.push(Event::IntegrityWarning {
            count: c.integrity_warnings,
        });
    }
    out
}

/// Buffered events per user and live broadcast capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusConfig {
    /// Events retained per user for resuming clients.
    pub replay_capacity: usize,
    /// Live events a slow subscriber may fall behind before it gets `reset`.
    pub broadcast_capacity: usize,
    /// Seqs start after this value (e.g. the start time in milliseconds × 1000).
    pub first_seq_after: u64,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            replay_capacity: 512,
            broadcast_capacity: 1024,
            first_seq_after: 0,
        }
    }
}

struct Channel {
    buffer: ReplayBuffer<Event>,
    live: broadcast::Sender<(u64, Event)>,
    closed: Option<AccountClosure>,
}

/// The event bus (cheap to share via `Arc`).
pub struct EventBus {
    config: BusConfig,
    users: Mutex<HashMap<UserId, Channel>>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// What a new connection starts with.
#[derive(Debug)]
pub struct Subscription {
    /// Frames to send first (replay or `reset`).
    pub replay: Vec<Frame<Event>>,
    /// Live events.
    pub live: broadcast::Receiver<(u64, Event)>,
    /// Head seq when subscribing.
    pub head: u64,
}

impl EventBus {
    /// A bus with `config`.
    pub fn new(config: BusConfig) -> Self {
        Self {
            config,
            users: Mutex::new(HashMap::new()),
        }
    }

    fn with_channel<R>(&self, user: UserId, f: impl FnOnce(&mut Channel) -> R) -> R {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        let channel = users.entry(user).or_insert_with(|| Channel {
            buffer: ReplayBuffer::starting_after(
                self.config.replay_capacity,
                self.config.first_seq_after,
            ),
            live: broadcast::channel(self.config.broadcast_capacity.max(1)).0,
            closed: None,
        });
        f(channel)
    }

    /// Publishes `events` to `user`'s stream, in order. Returns the seq of the last one.
    pub fn publish(&self, user: UserId, events: impl IntoIterator<Item = Event>) -> u64 {
        self.with_channel(user, |ch| {
            for event in events {
                let seq = ch.buffer.push(event.clone());
                // No live subscriber is fine: the buffer keeps it for resuming clients.
                let _ = ch.live.send((seq, event));
            }
            ch.buffer.last_seq()
        })
    }

    /// Seq of `user`'s newest event.
    pub fn head(&self, user: UserId) -> u64 {
        self.with_channel(user, |ch| ch.buffer.last_seq())
    }

    /// Subscribes to `user`'s stream: the frames to replay after `resume_from` (or `reset`)
    /// and a receiver for everything published afterwards (no gap, no overlap).
    pub fn subscribe(&self, user: UserId, resume_from: Option<u64>) -> Subscription {
        self.with_channel(user, |ch| Subscription {
            replay: ch.buffer.resume(resume_from),
            live: ch.live.subscribe(),
            head: ch.buffer.last_seq(),
        })
    }

    /// Publishes `account.disabled` for `user` once per closure reason.
    pub fn close_account(&self, user: UserId, reason: AccountClosure) {
        let fresh = self.with_channel(user, |ch| {
            let fresh = ch.closed != Some(reason);
            ch.closed = Some(reason);
            fresh
        });
        if fresh {
            self.publish(user, [Event::AccountDisabled { reason }]);
        }
    }

    /// Forgets a closure (the account was re-enabled), so a later one is announced again.
    pub fn reopen_account(&self, user: UserId) {
        self.with_channel(user, |ch| ch.closed = None);
    }
}

impl CommitListener for EventBus {
    fn committed(&self, user: UserId, notice: &Committed) {
        let events = events_of(notice);
        if !events.is_empty() {
            self.publish(user, events);
        }
    }
}

/// The problem of the terminal frame after `account.disabled`.
pub fn closure_problem(reason: AccountClosure) -> Problem {
    match reason {
        AccountClosure::Disabled => Problem::new(ProblemType::AccountDisabled),
        AccountClosure::DeletionPending => Problem::new(ProblemType::AccountDeletionPending),
        AccountClosure::Deleted => Problem::new(ProblemType::Unauthorized),
    }
}

/// What the revocation set says about a connection.
fn account_state(
    revocations: &RevocationSet,
    session: SessionId,
    user: UserId,
) -> Result<(), Result<AccountClosure, ()>> {
    match revocations.check(session, user) {
        Ok(flags) if flags.deletion_pending => Err(Ok(AccountClosure::DeletionPending)),
        Ok(_) => Ok(()),
        Err(Revoked::UserDisabled) => Err(Ok(AccountClosure::Disabled)),
        Err(Revoked::UserPurged) => Err(Ok(AccountClosure::Deleted)),
        Err(Revoked::Session) => Err(Err(())),
    }
}

struct Conn {
    bus: Arc<EventBus>,
    revocations: Arc<RevocationSet>,
    user: UserId,
    session: SessionId,
    queue: VecDeque<Frame<Event>>,
    live: broadcast::Receiver<(u64, Event)>,
    watch: Option<watch::Receiver<u64>>,
    last: u64,
    done: bool,
}

impl Conn {
    /// Queues the frames for `seq`/`event`; returns false when the event was already sent.
    fn accept(&mut self, seq: u64, event: Event) {
        if seq <= self.last {
            return;
        }
        self.last = seq;
        let closure = match &event {
            Event::AccountDisabled { reason } => Some(*reason),
            _ => None,
        };
        self.queue.push_back(Frame::Data {
            seq,
            payload: event,
        });
        if let Some(reason) = closure {
            self.queue.push_back(Frame::Error {
                seq,
                problem: closure_problem(reason),
            });
        }
    }

    /// Re-checks the caller; queues the terminal frame for a revoked session, or has the bus
    /// announce an account closure (delivered through the live channel).
    fn recheck(&mut self) {
        match account_state(&self.revocations, self.session, self.user) {
            Ok(()) => self.bus.reopen_account(self.user),
            Err(Ok(reason)) => self.bus.close_account(self.user, reason),
            Err(Err(())) => self.queue.push_back(Frame::Error {
                seq: self.last,
                problem: Problem::new(ProblemType::Unauthorized),
            }),
        }
    }

    async fn next_frame(&mut self) -> Option<Frame<Event>> {
        loop {
            if self.done {
                return None;
            }
            if let Some(frame) = self.queue.pop_front() {
                if matches!(frame, Frame::Error { .. }) {
                    self.done = true;
                }
                return Some(frame);
            }
            let changed = async {
                match self.watch.as_mut() {
                    Some(w) => w.changed().await.is_ok(),
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                biased;
                ok = changed => {
                    if ok {
                        self.recheck();
                    } else {
                        self.watch = None;
                    }
                }
                received = self.live.recv() => match received {
                    Ok((seq, event)) => self.accept(seq, event),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let head = self.bus.head(self.user);
                        self.last = head;
                        return Some(Frame::Reset { seq: head });
                    }
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
            }
        }
    }
}

/// The frames of one `/events` connection of `user` (session `session`), resuming after
/// `resume_from`.
pub fn connection(
    bus: Arc<EventBus>,
    revocations: Arc<RevocationSet>,
    user: UserId,
    session: SessionId,
    resume_from: Option<u64>,
) -> impl Stream<Item = Frame<Event>> + Unpin + 'static {
    // Subscribe to revocation changes first, so a change after the subscription below is seen.
    let watch = revocations.subscribe();
    let sub = bus.subscribe(user, resume_from);
    let mut conn = Conn {
        bus,
        revocations,
        user,
        session,
        queue: VecDeque::new(),
        live: sub.live,
        watch: Some(watch),
        last: resume_from.unwrap_or(sub.head).min(sub.head),
        done: false,
    };
    for frame in sub.replay {
        match frame {
            Frame::Data { seq, payload } => conn.accept(seq, payload),
            Frame::Reset { seq } => {
                conn.last = seq;
                conn.queue.push_back(Frame::Reset { seq });
            }
            other => conn.queue.push_back(other),
        }
    }
    if resume_from.is_none() {
        conn.last = sub.head;
    }
    conn.recheck();
    Box::pin(futures_util::stream::unfold(conn, |mut conn| async move {
        conn.next_frame().await.map(|f| (f, conn))
    }))
}

#[cfg(test)]
mod tests;

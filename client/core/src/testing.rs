//! Test support (headless tests, PLAN §16.4): an in-memory fake sync server speaking the
//! `sync-model` types with idempotent pushes, epochs, the D19 update decision and scripted
//! results, and a fake account API. Deterministic: no clock, no network.
//!
//! The fake server applies ops with the same pure function the client uses
//! ([`crate::store::write::apply_to_note`], i.e. `sync-model` rules and `vault-format`), which
//! is what the real server does through the shared crates (L16). Versions are content hashes
//! (`sync_model::Version`), exactly as on the real server.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use sync_model::changes::NoteRecord;
use sync_model::{UpdateDecision, decide_update};
use ulid::Ulid;
use vault_format::PathIndex;

use crate::net::{AccountApi, MeInfo, NetError, SessionTokens, SyncApi, Tokens};
use crate::store::notes::NoteState;
use crate::store::write::{Links, apply_to_note, task_op_id};
use crate::sync::model::{
    BootstrapPage, ChangeRecord, ChangesPage, ConflictResolution, EntityType, Op, OpOutcome,
    OpResult, Problem, Record, SyncOp, Version,
};
use crate::view::model::{AdminUserItem, Platform};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A server note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerNote {
    /// Path.
    pub path: String,
    /// Content.
    pub content: String,
}

impl ServerNote {
    /// Its version.
    pub fn version(&self) -> Version {
        Version::of_text(&self.content)
    }
}

#[derive(Debug, Default)]
struct State {
    epoch: u64,
    seq: u64,
    next_id: u128,
    notes: BTreeMap<Ulid, ServerNote>,
    history: HashMap<Version, String>,
    extra: BTreeMap<String, Record>,
    log: Vec<ChangeRecord>,
    results: HashMap<Ulid, OpResult>,
    scripted: VecDeque<(String, OpResult)>,
    offline: bool,
    page_size: usize,
    pushes: Vec<Vec<SyncOp>>,
    applied_ops: Vec<Ulid>,
    bootstrap_calls: u32,
    changes_calls: Vec<(u64, u64)>,
}

struct ServerLinks<'a>(&'a BTreeMap<Ulid, ServerNote>);

impl Links for ServerLinks<'_> {
    fn link_for(&self, id: Ulid) -> Option<String> {
        let index = PathIndex::new(self.0.values().map(|n| n.path.as_str()));
        self.0.get(&id).map(|n| index.link_text_for(&n.path))
    }
}

fn problem(problem_type: &str, status: u16) -> OpResult {
    OpResult::Rejected {
        problem: Problem {
            problem_type: problem_type.to_owned(),
            title: problem_type.to_owned(),
            status,
            detail: None,
        },
    }
}

/// An in-memory sync server.
#[derive(Debug, Clone)]
pub struct FakeServer {
    state: Arc<Mutex<State>>,
}

impl Default for FakeServer {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeServer {
    /// An empty server at epoch 1, bootstrap pages of 2 records.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                epoch: 1,
                page_size: 2,
                next_id: 1,
                ..State::default()
            })),
        }
    }

    fn log(s: &mut State, change: impl FnOnce(u64, u64) -> ChangeRecord) {
        s.seq += 1;
        let c = change(s.seq, s.epoch);
        s.log.push(c);
    }

    fn note_record(id: Ulid, n: &ServerNote) -> Record {
        Record::Note(NoteRecord {
            id,
            path: n.path.clone(),
            content: n.content.clone(),
            version: n.version(),
            kind: crate::format::parse_note(&n.path, &n.content).kind,
            summary: None,
        })
    }

    fn put(s: &mut State, id: Ulid, n: ServerNote) {
        s.history.insert(n.version(), n.content.clone());
        let record = Self::note_record(id, &n);
        s.notes.insert(id, n);
        Self::log(s, |seq, epoch| ChangeRecord::upsert(seq, epoch, record));
    }

    fn remove(s: &mut State, id: Ulid) {
        s.notes.remove(&id);
        Self::log(s, |seq, epoch| {
            ChangeRecord::delete(seq, epoch, EntityType::Note, &id.to_string())
        });
    }

    /// Bootstrap page size.
    pub fn set_page_size(&self, n: usize) {
        lock(&self.state).page_size = n.max(1);
    }

    /// Makes every call fail as unreachable (or reachable again).
    pub fn set_offline(&self, offline: bool) {
        lock(&self.state).offline = offline;
    }

    /// A change made by another device: creates or replaces a note. Returns its version.
    pub fn remote_upsert(&self, id: &str, path: &str, content: &str) -> Version {
        let mut s = lock(&self.state);
        let n = ServerNote {
            path: path.to_owned(),
            content: content.to_owned(),
        };
        let v = n.version();
        let id = Ulid::from_string(id).unwrap_or_default();
        Self::put(&mut s, id, n);
        v
    }

    /// A deletion by another device.
    pub fn remote_delete(&self, id: &str) {
        let mut s = lock(&self.state);
        Self::remove(&mut s, Ulid::from_string(id).unwrap_or_default());
    }

    /// Adds a non-note record (suggestion, cluster, …) to the snapshot and the change log.
    pub fn remote_record(&self, record: Record) {
        let mut s = lock(&self.state);
        s.extra.insert(
            format!("{}/{}", record.entity_type().as_str(), record.entity_id()),
            record.clone(),
        );
        Self::log(&mut s, |seq, epoch| ChangeRecord::upsert(seq, epoch, record));
    }

    /// The server rebuilt without preserving seqs: clients must re-bootstrap.
    pub fn bump_epoch(&self) {
        let mut s = lock(&self.state);
        s.epoch += 1;
        s.log.clear();
    }

    /// The next push of an op on `entity_id` gets `result` instead of being applied.
    pub fn script(&self, entity_id: &str, result: OpResult) {
        lock(&self.state)
            .scripted
            .push_back((entity_id.to_owned(), result));
    }

    /// The server's notes.
    pub fn notes(&self) -> BTreeMap<Ulid, ServerNote> {
        lock(&self.state).notes.clone()
    }

    /// Every push request received.
    pub fn pushes(&self) -> Vec<Vec<SyncOp>> {
        lock(&self.state).pushes.clone()
    }

    /// Op IDs actually applied (each at most once, however often replayed).
    pub fn applied_ops(&self) -> Vec<Ulid> {
        lock(&self.state).applied_ops.clone()
    }

    /// `(since, epoch)` of every changes request.
    pub fn changes_calls(&self) -> Vec<(u64, u64)> {
        lock(&self.state).changes_calls.clone()
    }

    /// Number of bootstrap page requests.
    pub fn bootstrap_calls(&self) -> u32 {
        lock(&self.state).bootstrap_calls
    }

    fn target_note(s: &mut State, op: &Op) -> Result<Option<Ulid>, OpResult> {
        if let Some(task) = task_op_id(op) {
            let found = s.notes.iter().find(|(_, n)| {
                let doc = vault_format::Document::parse(&n.content);
                sync_model::apply::find_task(doc.body(), task).is_some()
            });
            return found
                .map(|(id, _)| Some(*id))
                .ok_or_else(|| problem("not_found", 404));
        }
        Ok(match op {
            Op::TaskCreate(p) => match p.note_id {
                Some(n) => Some(n),
                None => {
                    let home = s
                        .notes
                        .iter()
                        .find(|(_, n)| n.path == crate::store::write::TASK_HOME)
                        .map(|(id, _)| *id);
                    Some(home.unwrap_or_else(|| {
                        s.next_id += 1;
                        Ulid::from_parts(0x0000_5E2F_E000, s.next_id)
                    }))
                }
            },
            Op::NoteCreate(p) => Some(p.id),
            Op::Capture(p) => Some(p.id),
            Op::EntityCreate(p) => Some(p.id),
            Op::DocumentCreate(p) => Some(p.id),
            Op::PlaceCreate(p) => Some(p.id),
            Op::NoteUpdate(p) => Some(p.id),
            Op::NoteMove(p) => Some(p.id),
            Op::NoteDelete(p) => Some(p.id),
            Op::RelationAdd(r) | Op::RelationRemove(r) => Some(r.src_id),
            Op::RelationRetype(r) => Some(r.src_id),
            Op::EntityPatch(p) | Op::DocumentPatch(p) | Op::PlacePatch(p) => Some(p.id),
            _ => None,
        })
    }

    fn apply(s: &mut State, op: &SyncOp) -> OpResult {
        if let Some(i) = s.scripted.iter().position(|(e, _)| *e == op.entity_id)
            && let Some((_, r)) = s.scripted.remove(i)
        {
            return r;
        }
        let note_id = match Self::target_note(s, &op.op) {
            Ok(Some(id)) => id,
            Ok(None) => {
                return OpResult::Applied {
                    new_version: None,
                    merged: false,
                };
            }
            Err(r) => return r,
        };
        let current = s.notes.get(&note_id).cloned();
        let mut merged = false;
        let mut effective = op.op.clone();
        if let (Op::NoteUpdate(u), Some(cur), Some(base)) = (&op.op, &current, &op.base_version) {
            let base_text = s.history.get(base).cloned();
            match decide_update(base, base_text.as_deref(), &cur.content, &u.content) {
                UpdateDecision::FastForward => {}
                UpdateDecision::AlreadyApplied => {
                    return OpResult::Applied {
                        new_version: Some(cur.version()),
                        merged: false,
                    };
                }
                UpdateDecision::Merged(text) => {
                    merged = true;
                    effective = Op::NoteUpdate(sync_model::ops::NoteUpdate {
                        id: u.id,
                        content: text,
                    });
                }
                UpdateDecision::Conflict(_) => {
                    return OpResult::Conflict {
                        server_version: Some(cur.version()),
                        resolution: ConflictResolution::ServerKept {
                            reason: "overlapping_edits".to_owned(),
                        },
                    };
                }
            }
        }
        let state = current.map(|n| NoteState {
            path: n.path,
            content: n.content,
        });
        let links = ServerLinks(&s.notes);
        match apply_to_note(state, &effective, &links) {
            Ok(Some(next)) => {
                let n = ServerNote {
                    path: next.path,
                    content: next.content,
                };
                let v = n.version();
                Self::put(s, note_id, n);
                OpResult::Applied {
                    new_version: Some(v),
                    merged,
                }
            }
            Ok(None) => {
                Self::remove(s, note_id);
                OpResult::Applied {
                    new_version: None,
                    merged: false,
                }
            }
            Err(crate::CoreError::NotFound { .. }) => problem("not_found", 404),
            Err(_) => problem("invalid_body", 422),
        }
    }
}

impl SyncApi for FakeServer {
    fn bootstrap(&self, cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        let r = (|| {
            let mut s = lock(&self.state);
            if s.offline {
                return Err(NetError::Offline("fake".into()));
            }
            s.bootstrap_calls += 1;
            let mut all: Vec<Record> = s
                .notes
                .iter()
                .map(|(id, n)| Self::note_record(*id, n))
                .collect();
            all.extend(s.extra.values().cloned());
            let start = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
            let end = (start + s.page_size).min(all.len());
            Ok(BootstrapPage {
                epoch: s.epoch,
                seq: s.seq,
                records: all[start.min(end)..end].to_vec(),
                next_cursor: (end < all.len()).then(|| end.to_string()),
            })
        })();
        Box::pin(async move { r })
    }

    fn changes(
        &self,
        since: u64,
        epoch: u64,
        limit: u32,
    ) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        let r = (|| {
            let mut s = lock(&self.state);
            if s.offline {
                return Err(NetError::Offline("fake".into()));
            }
            s.changes_calls.push((since, epoch));
            if epoch != s.epoch {
                return Err(NetError::EpochChanged);
            }
            let limit = usize::try_from(limit).unwrap_or(usize::MAX);
            let after: Vec<&ChangeRecord> = s.log.iter().filter(|c| c.seq > since).collect();
            let page: Vec<ChangeRecord> = after.iter().take(limit).map(|c| (*c).clone()).collect();
            Ok(ChangesPage {
                epoch: s.epoch,
                next_seq: page.last().map_or(since.max(s.seq), |c| c.seq),
                has_more: after.len() > page.len(),
                changes: page,
            })
        })();
        Box::pin(async move { r })
    }

    fn push(&self, ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        let r = (|| {
            let mut s = lock(&self.state);
            if s.offline {
                return Err(NetError::Offline("fake".into()));
            }
            s.pushes.push(ops.clone());
            let mut out = Vec::new();
            for op in &ops {
                let result = if let Some(r) = s.results.get(&op.op_id) {
                    r.clone()
                } else {
                    let r = match op.validate() {
                        Ok(()) => Self::apply(&mut s, op),
                        Err(_) => problem("invalid_body", 422),
                    };
                    s.results.insert(op.op_id, r.clone());
                    if matches!(r, OpResult::Applied { .. }) {
                        s.applied_ops.push(op.op_id);
                    }
                    r
                };
                out.push(OpOutcome {
                    op_id: op.op_id,
                    result,
                });
            }
            Ok(out)
        })();
        Box::pin(async move { r })
    }
}

/// A scripted account API.
#[derive(Debug, Default)]
pub struct FakeAccountApi {
    /// `(username, password)` → the user's profile.
    pub users: Mutex<HashMap<(String, String), MeInfo>>,
    /// Error `login` returns for a username (pending, rejected, disabled, …).
    pub login_errors: Mutex<HashMap<String, NetError>>,
    /// Error the next `refresh` returns.
    pub refresh_error: Mutex<Option<NetError>>,
    /// Error `me` returns.
    pub me_error: Mutex<Option<NetError>>,
    /// Calls made, in order (`login:<user>`, `refresh:<token>`, `logout`, `me`, …).
    pub calls: Mutex<Vec<String>>,
    counter: Mutex<u32>,
    by_token: Mutex<HashMap<String, MeInfo>>,
}

impl FakeAccountApi {
    /// Adds a user.
    pub fn add_user(&self, username: &str, password: &str, id: &str, timezone: &str) {
        lock(&self.users).insert(
            (username.to_owned(), password.to_owned()),
            MeInfo {
                id: id.to_owned(),
                username: username.to_owned(),
                display_name: username.to_owned(),
                role: "member".to_owned(),
                status: "active".to_owned(),
                timezone: timezone.to_owned(),
                ui_language: "en".to_owned(),
                deletion_at: None,
                password_change_required: false,
            },
        );
    }

    /// Changes a user's profile (status, role, deletion date, …).
    pub fn update_user(&self, username: &str, f: impl Fn(&mut MeInfo)) {
        for ((u, _), me) in lock(&self.users).iter_mut() {
            if u == username {
                f(me);
            }
        }
        for me in lock(&self.by_token).values_mut() {
            if me.username == username {
                f(me);
            }
        }
    }

    fn tokens(&self, me: &MeInfo) -> SessionTokens {
        let mut n = lock(&self.counter);
        *n += 1;
        let t = SessionTokens {
            user_id: me.id.clone(),
            device_id: "01K5DSSE00000000000000DEV1".to_owned(),
            session_id: "01K5DSSE00000000000000SES1".to_owned(),
            access_token: format!("access-{n}"),
            access_expires_at: DateTime::<Utc>::UNIX_EPOCH,
            refresh_token: format!("refresh-{n}"),
            refresh_expires_at: DateTime::<Utc>::UNIX_EPOCH,
            export_only: me.status == "deletion_pending",
            password_change_required: me.password_change_required,
        };
        lock(&self.by_token).insert(t.refresh_token.clone(), me.clone());
        lock(&self.by_token).insert(t.access_token.clone(), me.clone());
        t
    }

    fn call(&self, c: String) {
        lock(&self.calls).push(c);
    }
}

impl AccountApi for FakeAccountApi {
    fn signup(
        &self,
        _server_url: String,
        username: String,
        _password: String,
        _display_name: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        self.call(format!("signup:{username}"));
        Box::pin(async move { Ok(username) })
    }

    fn login(
        &self,
        _server_url: String,
        username: String,
        password: String,
        _device_name: String,
        _platform: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        self.call(format!("login:{username}"));
        let r = if let Some(e) = lock(&self.login_errors).get(&username) {
            Err(e.clone())
        } else {
            match lock(&self.users).get(&(username, password)).cloned() {
                Some(me) => Ok(self.tokens(&me)),
                None => Err(NetError::InvalidCredentials),
            }
        };
        Box::pin(async move { r })
    }

    fn refresh(
        &self,
        _server_url: String,
        refresh_token: String,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        self.call(format!("refresh:{refresh_token}"));
        let r = if let Some(e) = lock(&self.refresh_error).take() {
            Err(e)
        } else {
            let me = lock(&self.by_token).remove(&refresh_token);
            match me {
                Some(me) => Ok(self.tokens(&me)),
                None => Err(NetError::Unauthorized),
            }
        };
        Box::pin(async move { r })
    }

    fn logout(&self, _server_url: String, _tokens: Tokens) -> BoxFuture<'_, Result<(), NetError>> {
        self.call("logout".to_owned());
        Box::pin(async { Ok(()) })
    }

    fn me(&self, _server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        self.call("me".to_owned());
        Box::pin(async move {
            if let Some(e) = lock(&self.me_error).clone() {
                return Err(e);
            }
            let token = tokens
                .access_token()
                .await
                .map_err(|e| NetError::Protocol(e.to_string()))?
                .unwrap_or_default();
            lock(&self.by_token)
                .get(&token)
                .cloned()
                .ok_or(NetError::Unauthorized)
        })
    }

    fn set_device_reminders(
        &self,
        _server_url: String,
        _tokens: Tokens,
        device_id: String,
        enabled: bool,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        self.call(format!("reminders:{device_id}:{enabled}"));
        Box::pin(async { Ok(()) })
    }

    fn admin_users(
        &self,
        _server_url: String,
        _tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<AdminUserItem>, NetError>> {
        self.call("admin_users".to_owned());
        Box::pin(async { Ok(Vec::new()) })
    }
}

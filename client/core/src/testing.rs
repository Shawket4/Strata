//! Test support (headless tests, PLAN §16.4): an in-memory fake sync server with idempotent
//! pushes, epochs and scripted results, and a fake account API. Deterministic: versions are
//! counters, no clock, no network.
//!
//! The fake server applies ops with the same pure function the client uses
//! ([`crate::store::write::apply_to_note`], i.e. `vault-format`), which is what the real server
//! does through the shared crates (L16).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;

use crate::net::{AccountApi, MeInfo, NetError, SessionTokens, SyncApi, Tokens};
use crate::store::notes::NoteState;
use crate::store::write::apply_to_note;
use crate::sync::model::{
    BootstrapPage, Change, ChangeRecord, ChangesPage, OpOutcome, OpPayload, OpResult, PushOp,
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
    /// Version.
    pub version: String,
}

#[derive(Debug, Default)]
struct State {
    epoch: u64,
    seq: u64,
    version: u64,
    notes: BTreeMap<String, ServerNote>,
    extra: BTreeMap<String, ChangeRecord>,
    log: Vec<(u64, ChangeRecord)>,
    results: HashMap<String, OpResult>,
    scripted: VecDeque<(String, OpResult)>,
    offline: bool,
    page_size: usize,
    pushes: Vec<Vec<PushOp>>,
    applied_ops: Vec<String>,
    bootstrap_calls: u32,
    changes_calls: Vec<(u64, u64)>,
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
                ..State::default()
            })),
        }
    }

    fn next_version(s: &mut State) -> String {
        s.version += 1;
        format!("v{}", s.version)
    }

    fn log(s: &mut State, record: ChangeRecord) {
        s.seq += 1;
        let seq = s.seq;
        s.log.push((seq, record));
    }

    fn upsert_record(id: &str, n: &ServerNote) -> ChangeRecord {
        ChangeRecord::NoteUpsert {
            id: id.to_owned(),
            path: n.path.clone(),
            content: n.content.clone(),
            version: n.version.clone(),
        }
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
    pub fn remote_upsert(&self, id: &str, path: &str, content: &str) -> String {
        let mut s = lock(&self.state);
        let version = Self::next_version(&mut s);
        let n = ServerNote {
            path: path.to_owned(),
            content: content.to_owned(),
            version: version.clone(),
        };
        let record = Self::upsert_record(id, &n);
        s.notes.insert(id.to_owned(), n);
        Self::log(&mut s, record);
        version
    }

    /// A deletion by another device.
    pub fn remote_delete(&self, id: &str) {
        let mut s = lock(&self.state);
        s.notes.remove(id);
        Self::log(&mut s, ChangeRecord::NoteDelete { id: id.to_owned() });
    }

    /// Adds a non-note record (suggestion, cluster, …) to the snapshot and the change log.
    pub fn remote_record(&self, key: &str, record: ChangeRecord) {
        let mut s = lock(&self.state);
        s.extra.insert(key.to_owned(), record.clone());
        Self::log(&mut s, record);
    }

    /// The server rebuilt its index without preserving seqs: clients must re-bootstrap.
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
    pub fn notes(&self) -> BTreeMap<String, ServerNote> {
        lock(&self.state).notes.clone()
    }

    /// Every push request received.
    pub fn pushes(&self) -> Vec<Vec<PushOp>> {
        lock(&self.state).pushes.clone()
    }

    /// Op IDs actually applied (each at most once, however often replayed).
    pub fn applied_ops(&self) -> Vec<String> {
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

    fn apply(s: &mut State, op: &PushOp) -> OpResult {
        if let Some(i) = s.scripted.iter().position(|(e, _)| *e == op.entity_id)
            && let Some((_, r)) = s.scripted.remove(i)
        {
            return r;
        }
        let note_id = match &op.payload {
            OpPayload::TaskCreate { note_id, .. } => note_id.clone(),
            OpPayload::TaskUpdate { .. }
            | OpPayload::TaskComplete { .. }
            | OpPayload::TaskCancel { .. }
            | OpPayload::TaskReopen
            | OpPayload::TaskDelete => {
                let found = s.notes.iter().find(|(_, n)| {
                    vault_format::tasks::extract_tasks(&n.content)
                        .iter()
                        .any(|t| t.task.block_id() == Some(op.entity_id.as_str()))
                });
                match found {
                    Some((id, _)) => id.clone(),
                    None => {
                        return OpResult::Rejected {
                            problem_type: "not_found".into(),
                            status: 404,
                        };
                    }
                }
            }
            OpPayload::SuggestionAccept
            | OpPayload::SuggestionReject
            | OpPayload::RelinkRequest => {
                return OpResult::Applied { new_version: None };
            }
            _ => op.entity_id.clone(),
        };
        let current = s.notes.get(&note_id).cloned();
        let is_update = matches!(op.payload, OpPayload::NoteUpdate { .. });
        if is_update
            && let (Some(cur), Some(base)) = (&current, &op.base_version)
            && &cur.version != base
        {
            return OpResult::Conflict {
                server_version: cur.version.clone(),
                server_content: Some(cur.content.clone()),
                resolution: "needs_user".into(),
            };
        }
        let state = current.map(|n| NoteState {
            path: n.path,
            content: n.content,
        });
        match apply_to_note(&note_id, &op.entity_id, state, &op.payload) {
            Ok(Some(next)) => {
                let version = Self::next_version(s);
                let n = ServerNote {
                    path: next.path,
                    content: next.content,
                    version: version.clone(),
                };
                let record = Self::upsert_record(&note_id, &n);
                s.notes.insert(note_id, n);
                Self::log(s, record);
                OpResult::Applied {
                    new_version: Some(version),
                }
            }
            Ok(None) => {
                s.notes.remove(&note_id);
                Self::log(s, ChangeRecord::NoteDelete { id: note_id });
                OpResult::Applied { new_version: None }
            }
            Err(crate::CoreError::NotFound { .. }) => OpResult::Rejected {
                problem_type: "not_found".into(),
                status: 404,
            },
            Err(_) => OpResult::Rejected {
                problem_type: "invalid_body".into(),
                status: 422,
            },
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
            let mut all: Vec<ChangeRecord> = s
                .notes
                .iter()
                .map(|(id, n)| Self::upsert_record(id, n))
                .collect();
            all.extend(s.extra.values().cloned());
            let start = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
            let end = (start + s.page_size).min(all.len());
            let total = u32::try_from(all.len().div_ceil(s.page_size).max(1)).unwrap_or(1);
            Ok(BootstrapPage {
                epoch: s.epoch,
                start_seq: s.seq,
                records: all[start.min(end)..end].to_vec(),
                next_cursor: (end < all.len()).then(|| end.to_string()),
                total_pages: Some(total),
            })
        })();
        Box::pin(async move { r })
    }

    fn changes(&self, since: u64, epoch: u64, limit: u32) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
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
            let after: Vec<&(u64, ChangeRecord)> =
                s.log.iter().filter(|(seq, _)| *seq > since).collect();
            let page: Vec<Change> = after
                .iter()
                .take(limit)
                .map(|(seq, record)| Change {
                    seq: *seq,
                    record: record.clone(),
                })
                .collect();
            Ok(ChangesPage {
                next_seq: page.last().map_or(since.max(s.seq), |c| c.seq),
                has_more: after.len() > page.len(),
                changes: page,
            })
        })();
        Box::pin(async move { r })
    }

    fn push(&self, ops: Vec<PushOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
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
                    let r = Self::apply(&mut s, op);
                    s.results.insert(op.op_id.clone(), r.clone());
                    if matches!(r, OpResult::Applied { .. }) {
                        s.applied_ops.push(op.op_id.clone());
                    }
                    r
                };
                out.push(OpOutcome {
                    op_id: op.op_id.clone(),
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

    fn refresh(&self, _server_url: String, refresh_token: String) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
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

    fn admin_users(&self, _server_url: String, _tokens: Tokens) -> BoxFuture<'_, Result<Vec<AdminUserItem>, NetError>> {
        self.call("admin_users".to_owned());
        Box::pin(async { Ok(Vec::new()) })
    }
}

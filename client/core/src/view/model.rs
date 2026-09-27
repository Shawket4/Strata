//! View-models: the contract between the core and Dart (PLAN §11 screens, §13 item 9).
//!
//! Every screen renders exactly one of these types, streamed from the core (L15). They are
//! plain data (no methods Dart would call back into), immutable on the Dart side, and carry
//! everything the screen shows: display strings are already chosen, lists already sorted and
//! filtered. Timestamps are UTC instants; calendar dates are dates in the user's timezone.
//!
//! Screens whose data does not exist locally yet return a well-defined [`Availability`] state
//! instead of failing.

// Screens are streamed as whole values; variant sizes don't matter, and plain enums map to
// sealed classes in Dart one to one.
#![allow(clippy::large_enum_variant)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

// ---------------------------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------------------------

/// Whether an online-only or not-yet-built feature can be used right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// Usable.
    Available,
    /// Needs the server; the device is offline (§12.6).
    Offline,
    /// Not available yet: the server has no endpoint for it or the feature is not built.
    NotYetAvailable,
    /// The account may not use it (e.g. Admin → Users for members, export-only sessions).
    NotAllowed,
}

/// Network reachability as the sync engine last saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// No request has completed yet.
    Unknown,
    /// The last request reached the server.
    Online,
    /// The last request could not reach the server.
    Offline,
}

/// What the sync engine is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncPhase {
    /// Nothing running.
    #[default]
    Idle,
    /// Downloading the full snapshot (first login or epoch change).
    Bootstrapping,
    /// Pushing outbox ops.
    Pushing,
    /// Pulling changes.
    Pulling,
    /// Waiting to retry after a failure.
    Backoff,
}

/// The sync engine's current activity (progress details of [`SyncPhase`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncActivity {
    /// Phase.
    pub phase: SyncPhase,
    /// Bootstrap pages applied.
    pub pages_done: u32,
    /// Bootstrap pages in total, when the server says.
    pub pages_total: Option<u32>,
    /// Ops in the push in flight.
    pub ops: u32,
    /// When the next attempt runs (backoff).
    pub retry_at: Option<DateTime<Utc>>,
}

/// The compact sync indicator (§12.5: online/offline, pending ops, last sync, conflicts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPill {
    /// Reachability.
    pub connectivity: Connectivity,
    /// Current activity.
    pub activity: SyncActivity,
    /// Unsynced ops.
    pub pending_ops: u32,
    /// Unresolved conflicts.
    pub conflicts: u32,
    /// Duplicate prompts awaiting a choice.
    pub duplicates: u32,
    /// Last successful sync.
    pub last_sync_at: Option<DateTime<Utc>>,
}

/// A note in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteListItem {
    /// Note ID.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Vault path.
    pub path: String,
    /// Kind (`note`, `person`, …).
    pub kind: String,
    /// First line of body text (plain, ≤ 160 characters).
    pub snippet: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Last local change.
    pub updated_at: DateTime<Utc>,
    /// Has unsynced changes.
    pub pending_sync: bool,
}

/// A reference to another note/entity (resolved when `id` is set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    /// The note ID when the link resolves locally.
    pub id: Option<String>,
    /// Display title.
    pub title: String,
}

/// A citation `[[Note#^block]]` resolved for navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The cited note, when it resolves.
    pub note_id: Option<String>,
    /// Link text target (`Note`).
    pub target: String,
    /// Block ID (without `^`) or heading.
    pub anchor: Option<String>,
}

/// A bullet of an AI section with its citations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedBullet {
    /// Text without the date prefix and citations.
    pub text: String,
    /// Leading `YYYY-MM-DD` (Timeline, Custody).
    pub date: Option<NaiveDate>,
    /// Citations.
    pub citations: Vec<Citation>,
}

/// One frontmatter property for the properties panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyItem {
    /// Key as written.
    pub key: String,
    /// Values (a scalar is one value).
    pub values: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Session and accounts (§11 screens 1, 14, 15; §12.7)
// ---------------------------------------------------------------------------------------------

/// Platform of this install (device registration, notification mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Android.
    Android,
    /// iOS.
    Ios,
    /// macOS.
    Macos,
    /// Windows.
    Windows,
    /// Linux.
    Linux,
}

/// How reminders reach the user on this platform (§12.5b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationMode {
    /// The OS schedules notifications (Android, iOS, macOS, Windows).
    OsScheduled,
    /// No OS scheduler (Linux): the core fires `show_now` while the app runs.
    WhileRunning,
}

/// What `init_core` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreConfig {
    /// App-support directory (from `path_provider` in `strata_bridge`).
    pub app_data_dir: String,
    /// Platform.
    pub platform: Platform,
    /// Default device name for login (e.g. the host name).
    pub default_device_name: String,
    /// Server URL suggested on the login screen when none was used before.
    pub default_server_url: Option<String>,
}

/// The signed-in account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    /// User ID.
    pub user_id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// `admin` | `member`.
    pub role: String,
    /// Shortcut for `role == admin`.
    pub is_admin: bool,
    /// Server URL.
    pub server_url: String,
    /// IANA timezone.
    pub timezone: String,
    /// UI language (`en` | `ar`).
    pub ui_language: String,
}

/// An account with a local database on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownAccountItem {
    /// User ID.
    pub user_id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Server URL.
    pub server_url: String,
}

/// Which screen the account state leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// `init_core` has not run.
    NotInitialised,
    /// No account is active: the login screen.
    SignedOut,
    /// Signed in and usable.
    Active,
    /// An admin reset the password: change it before anything else.
    PasswordChangeRequired,
    /// The server disabled the account (§12.7): warn; local data is wiped after the user
    /// acknowledges.
    Disabled,
    /// The account is scheduled for deletion (D25): export-only; local data is read-only.
    DeletionPending,
}

/// The app's account state: decides between the login screen, the main shell and the
/// restricted screens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionState {
    /// Which screen.
    pub kind: SessionKind,
    /// The account (every kind but `NotInitialised` and `SignedOut`).
    pub account: Option<AccountSummary>,
    /// `SignedOut`: accounts that still have local data (switch without re-downloading).
    pub known_accounts: Vec<KnownAccountItem>,
    /// `SignedOut`: prefill for the server URL field.
    pub server_url: Option<String>,
    /// `SignedOut`: prefill for the device name field.
    pub device_name: String,
    /// `Disabled` / `DeletionPending`: ops that never synced (listed and exportable).
    pub unsynced_ops: u32,
    /// `DeletionPending`: when the account is purged.
    pub deletion_at: Option<DateTime<Utc>>,
    /// `DeletionPending`: whole days until then.
    pub days_remaining: Option<u32>,
}

impl SessionState {
    /// A state of `kind` with every detail empty.
    pub fn of(kind: SessionKind) -> Self {
        Self {
            kind,
            account: None,
            known_accounts: Vec::new(),
            server_url: None,
            device_name: String::new(),
            unsynced_ops: 0,
            deletion_at: None,
            days_remaining: None,
        }
    }
}

/// What `sign_in` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInRequest {
    /// Server base URL.
    pub server_url: String,
    /// Username.
    pub username: String,
    /// Password.
    pub password: String,
    /// Name of this device.
    pub device_name: String,
}

/// What `sign_up` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignUpRequest {
    /// Server base URL.
    pub server_url: String,
    /// Username.
    pub username: String,
    /// Password.
    pub password: String,
    /// Display name.
    pub display_name: String,
}

/// Result of `sign_up`: the account waits for approval (D22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignUpOutcome {
    /// Username as registered.
    pub username: String,
}

/// Result of `sign_out`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignOutOutcome {
    /// Signed out; the account's local database and tokens are deleted. `false`: ops have not
    /// synced — offer "Sync now / Sign out anyway / Cancel" (§12.7).
    pub signed_out: bool,
    /// Unsynced ops (when not signed out).
    pub unsynced_ops: u32,
}

// ---------------------------------------------------------------------------------------------
// Home and tasks (§11 screens 2, 12; D28)
// ---------------------------------------------------------------------------------------------

/// Task lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Open (incl. custom statuses).
    Open,
    /// Done.
    Done,
    /// Cancelled.
    Cancelled,
}

/// A reminder of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderItem {
    /// Wall-clock time as written (`YYYY-MM-DD HH:MM`).
    pub local: String,
    /// The instant it fires (user's timezone, DST resolved).
    pub at: DateTime<Utc>,
}

/// A task in a list or detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    /// Block ID (`t-…`).
    pub id: String,
    /// The note holding the line.
    pub note_id: String,
    /// That note's title.
    pub note_title: String,
    /// Description (no emoji fields).
    pub description: String,
    /// State.
    pub state: TaskState,
    /// Priority (`highest` … `lowest`, `normal`).
    pub priority: String,
    /// 📅 due.
    pub due: Option<NaiveDate>,
    /// ⏳ scheduled.
    pub scheduled: Option<NaiveDate>,
    /// ✅ done date.
    pub done: Option<NaiveDate>,
    /// Recurrence phrase as written (`every month on the 1st`).
    pub recurrence: Option<String>,
    /// `false` when the phrase is outside the grammar ("recurrence not understood").
    pub recurrence_understood: bool,
    /// Reminders.
    pub reminders: Vec<ReminderItem>,
    /// Linked notes/entities (wikilinks in the line).
    pub links: Vec<EntityRef>,
    /// Has unsynced changes.
    pub pending_sync: bool,
}

/// Tasks grouped for Home (compact) and the Tasks screen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskSections {
    /// Open, due before today.
    pub overdue: Vec<TaskItem>,
    /// Open, due or scheduled today.
    pub today: Vec<TaskItem>,
    /// Open, due after today.
    pub upcoming: Vec<TaskItem>,
    /// Open recurring tasks (any due date).
    pub recurring: Vec<TaskItem>,
    /// Open without any date.
    pub no_date: Vec<TaskItem>,
}

/// Home / Capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeView {
    /// Most recently changed notes (≤ 10).
    pub recent_notes: Vec<NoteListItem>,
    /// Inbox items awaiting review.
    pub inbox_count: u32,
    /// Task sections (compact shows Today / Upcoming / Recurring here, D28).
    pub tasks: TaskSections,
    /// Sync indicator.
    pub sync: SyncPill,
}

/// The Tasks destination (medium/expanded) incl. completed tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksView {
    /// Open tasks grouped.
    pub sections: TaskSections,
    /// Done and cancelled tasks, most recent first (≤ 50).
    pub done: Vec<TaskItem>,
}

/// A task to create (from the task editor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDraft {
    /// The note to add it to (`None`: `tasks/Tasks.md`).
    pub note_id: Option<String>,
    /// Description (may contain wikilinks).
    pub description: String,
    /// 📅
    pub due: Option<NaiveDate>,
    /// ⏳
    pub scheduled: Option<NaiveDate>,
    /// 🔁 phrase (`every month on the 1st`).
    pub recurrence: Option<String>,
    /// Reminders (local wall-clock times).
    pub reminders: Vec<NaiveDateTime>,
    /// Priority (`highest` … `lowest`; `None` = normal).
    pub priority: Option<String>,
}

/// Field edits of a task: `None` leaves a field unchanged; the `clear_*` flags remove it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskPatch {
    /// New description.
    pub text: Option<String>,
    /// New 📅.
    pub due: Option<NaiveDate>,
    /// Remove 📅.
    pub clear_due: bool,
    /// New 🔁 phrase.
    pub recurrence: Option<String>,
    /// Remove 🔁.
    pub clear_recurrence: bool,
    /// Replace all reminders.
    pub reminders: Option<Vec<NaiveDateTime>>,
    /// New priority (`normal` removes it).
    pub priority: Option<String>,
}

/// Task detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskScreen {
    /// Task ID asked for.
    pub id: String,
    /// The task (`None`: not found locally).
    pub task: Option<TaskItem>,
    /// The raw task line (editor).
    pub line: String,
    /// Completed occurrences of the same recurring task in the same note, newest first.
    pub history: Vec<TaskItem>,
}

// ---------------------------------------------------------------------------------------------
// Inbox and suggestions (§11 screens 3, 11, 12a)
// ---------------------------------------------------------------------------------------------

/// An existing item a create resembles.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateItem {
    /// Existing item ID.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Excerpt.
    pub snippet: Option<String>,
    /// `exact` | `near` | `semantic`.
    pub match_level: String,
    /// Score in `[0, 1]`.
    pub score: f64,
}

/// What a suggestion proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    /// AI filing of a capture (`title`, `folder`, `tags`).
    Filing,
    /// "Who is “بابا”?": link to a candidate or create a person (`mention`, `candidates`).
    EntityLinkOrCreate,
    /// A custody event to confirm (`document`, `line`, `confidence`).
    Custody,
    /// The item resembles existing items (`duplicates`).
    Duplicate,
    /// A low-confidence relation (`target`, `rel_type`, `confidence`, `reason`).
    Relation,
    /// A task proposed from a capture (`line`).
    Task,
    /// A kind this app version cannot show (`server_kind`).
    Unsupported,
}

/// The details of a suggestion; which fields are set depends on [`SuggestionKind`].
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionDetail {
    /// Kind.
    pub kind: SuggestionKind,
    /// Filing: title.
    pub title: String,
    /// Filing: folder.
    pub folder: String,
    /// Filing: tags.
    pub tags: Vec<String>,
    /// Link-or-create: the mention.
    pub mention: String,
    /// Link-or-create: candidates.
    pub candidates: Vec<EntityRef>,
    /// Custody: the document.
    pub document: Option<EntityRef>,
    /// Custody / task: the proposed line.
    pub line: String,
    /// Custody / relation: confidence.
    pub confidence: Option<f64>,
    /// Duplicate: candidates.
    pub duplicates: Vec<CandidateItem>,
    /// Relation: target.
    pub target: Option<EntityRef>,
    /// Relation: type.
    pub rel_type: String,
    /// Relation: reason.
    pub reason: String,
    /// Unsupported: the server's kind string.
    pub server_kind: String,
}

impl SuggestionDetail {
    /// An empty detail of `kind`.
    pub fn of(kind: SuggestionKind) -> Self {
        Self {
            kind,
            title: String::new(),
            folder: String::new(),
            tags: Vec::new(),
            mention: String::new(),
            candidates: Vec::new(),
            document: None,
            line: String::new(),
            confidence: None,
            duplicates: Vec::new(),
            target: None,
            rel_type: String::new(),
            reason: String::new(),
            server_kind: String::new(),
        }
    }
}

/// A suggestion.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestionItem {
    /// ID.
    pub id: String,
    /// The note it is about.
    pub note_id: Option<String>,
    /// `pending` | `accepted` | `rejected`.
    pub status: String,
    /// What it proposes.
    pub detail: SuggestionDetail,
    /// Created at.
    pub created: DateTime<Utc>,
    /// The user's accept/reject has not synced yet.
    pub pending_sync: bool,
}

/// A capture in the inbox.
#[derive(Debug, Clone, PartialEq)]
pub struct InboxItem {
    /// Note ID.
    pub note_id: String,
    /// Title (file name until filed).
    pub title: String,
    /// Body text (≤ 280 characters).
    pub text: String,
    /// Created at.
    pub created: DateTime<Utc>,
    /// Pending suggestions for this capture.
    pub suggestions: Vec<SuggestionItem>,
    /// Not synced yet.
    pub pending_sync: bool,
}

/// Inbox.
#[derive(Debug, Clone, PartialEq)]
pub struct InboxView {
    /// Captures, newest first.
    pub captures: Vec<InboxItem>,
    /// Pending suggestions not tied to an inbox capture (entity link-or-create, custody, …).
    pub suggestions: Vec<SuggestionItem>,
}

/// One "Already exists" prompt (§11 screen 12a).
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicatePrompt {
    /// The create op awaiting a choice.
    pub op_id: String,
    /// Kind of item being created (`note`, `task`, `person`, …).
    pub kind: String,
    /// Title of the item being created.
    pub title: String,
    /// Candidates, best first.
    pub candidates: Vec<CandidateItem>,
}

/// Every open duplicate prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicatePromptsView {
    /// Prompts, oldest first.
    pub prompts: Vec<DuplicatePrompt>,
}

/// The user's answer to a duplicate prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateChoice {
    /// Create anyway (resent with `force`; the server records "keep both").
    CreateAnyway,
    /// Don't create it (also used after "Open existing").
    Discard,
}

/// Result of a create intent.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateOutcome {
    /// The new item's ID (`None`: nothing was created).
    pub id: Option<String>,
    /// The local duplicate check's candidates when nothing was created; call again with
    /// `force` to create anyway.
    pub candidates: Vec<CandidateItem>,
}

// ---------------------------------------------------------------------------------------------
// Notes (§11 screen 4) and lists
// ---------------------------------------------------------------------------------------------

/// A typed relation chip.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationChip {
    /// Relation type.
    pub rel_type: String,
    /// Target.
    pub target: EntityRef,
    /// `user` | `ai`.
    pub by: String,
    /// AI confidence.
    pub confidence: Option<f64>,
    /// AI reason.
    pub reason: Option<String>,
}

/// A note linking here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkItem {
    /// Source note.
    pub note_id: String,
    /// Its title.
    pub title: String,
}

/// Backlinks of one kind (`link` for body links, or a relation type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkGroup {
    /// `link` or the relation type.
    pub kind: String,
    /// Sources, by title.
    pub items: Vec<BacklinkItem>,
}

/// Editor highlight kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// Frontmatter block.
    Frontmatter,
    /// Heading.
    Heading,
    /// `[[link]]`.
    WikiLink,
    /// `![[embed]]`.
    Embed,
    /// `#tag`.
    Tag,
    /// `^block`.
    BlockId,
    /// Task line.
    TaskLine,
    /// Code.
    Code,
}

/// An editor highlight span (UTF-16 offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHint {
    /// Kind.
    pub kind: HintKind,
    /// Start (inclusive).
    pub start: u32,
    /// End (exclusive).
    pub end: u32,
}

/// Sync state of one note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSyncKind {
    /// Nothing pending.
    Synced,
    /// Ops waiting.
    Pending,
    /// An edit conflicts with the server (open the conflict screen).
    Conflict,
}

/// Sync state of one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSyncState {
    /// Kind.
    pub kind: NoteSyncKind,
    /// Ops waiting.
    pub pending_ops: u32,
    /// The conflicting op.
    pub conflict_op_id: Option<String>,
}

/// A note.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteView {
    /// ID.
    pub id: String,
    /// Path.
    pub path: String,
    /// Display title.
    pub title: String,
    /// Kind.
    pub kind: String,
    /// Full markdown.
    pub content: String,
    /// Server version the local state is based on.
    pub version: Option<String>,
    /// Frontmatter properties (non-relation keys) in file order.
    pub properties: Vec<PropertyItem>,
    /// Outgoing typed relations.
    pub relations: Vec<RelationChip>,
    /// Incoming links and relations, grouped.
    pub backlinks: Vec<BacklinkGroup>,
    /// Tags.
    pub tags: Vec<String>,
    /// Tasks in this note.
    pub tasks: Vec<TaskItem>,
    /// Editor highlight spans.
    pub hints: Vec<EditorHint>,
    /// Sync state.
    pub sync: NoteSyncState,
    /// History and revert (online only).
    pub history: Availability,
}

/// The note screen.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteScreen {
    /// ID asked for.
    pub id: String,
    /// The note (`None`: unknown or deleted).
    pub note: Option<NoteView>,
}

/// A folder in the notes tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderItem {
    /// Path (`notes/sales`).
    pub path: String,
    /// Last segment.
    pub name: String,
    /// Notes directly inside.
    pub note_count: u32,
}

/// The Notes destination: a folder's subfolders and notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesListView {
    /// Folder shown (`""` for the vault root).
    pub folder: String,
    /// Direct subfolders, by name.
    pub folders: Vec<FolderItem>,
    /// Notes directly in the folder, by title.
    pub notes: Vec<NoteListItem>,
}

// ---------------------------------------------------------------------------------------------
// Directory, entities, documents, places (§11 screens 7, 7a, 7b, 8)
// ---------------------------------------------------------------------------------------------

/// Directory tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryTab {
    /// People.
    People,
    /// Companies.
    Companies,
    /// Documents.
    Documents,
    /// Places.
    Places,
}

/// A directory row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryItem {
    /// Note ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Role / industry / document status and location / parent place.
    pub subtitle: Option<String>,
    /// Aliases (both scripts).
    pub aliases: Vec<String>,
}

/// Counts per tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DirectoryCounts {
    /// People.
    pub people: u32,
    /// Companies.
    pub companies: u32,
    /// Documents.
    pub documents: u32,
    /// Places.
    pub places: u32,
}

/// The directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryView {
    /// Tab shown.
    pub tab: DirectoryTab,
    /// Search text (matches aliases in both scripts after normalisation).
    pub query: String,
    /// Rows, by title.
    pub items: Vec<DirectoryItem>,
    /// Counts (unfiltered).
    pub counts: DirectoryCounts,
}

/// A person or company page.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    /// ID.
    pub id: String,
    /// `person` | `company`.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Properties (role, industry, contact fields, …).
    pub properties: Vec<PropertyItem>,
    /// `## Summary` text.
    pub summary: Option<String>,
    /// `## Insights`.
    pub insights: Vec<CitedBullet>,
    /// `## Open items`.
    pub open_items: Vec<CitedBullet>,
    /// `## Timeline`, newest first.
    pub timeline: Vec<CitedBullet>,
    /// Notes mentioning the entity (links or `people:`/`companies:`), newest first.
    pub mentions: Vec<NoteListItem>,
    /// Entity-to-entity relations (both directions).
    pub related: Vec<RelationChip>,
    /// Documents this person holds / last held, or this company's documents.
    pub documents: Vec<DocumentBrief>,
    /// Has unsynced changes.
    pub pending_sync: bool,
}

/// A document in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBrief {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// `stored`, `checked-out`, ….
    pub status: Option<String>,
    /// Where it is.
    pub location: Option<EntityRef>,
    /// Who holds it.
    pub holder: Option<EntityRef>,
}

/// One custody event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustodyItem {
    /// Date.
    pub date: NaiveDate,
    /// Event type (`stored-at`, `handed-to`, …).
    pub kind: String,
    /// Document (on place pages).
    pub document: Option<EntityRef>,
    /// Place.
    pub place: Option<EntityRef>,
    /// Person.
    pub person: Option<EntityRef>,
    /// Third party.
    pub counterparty: Option<EntityRef>,
    /// Citations.
    pub citations: Vec<Citation>,
}

/// A document page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentView {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// `doc-type`.
    pub doc_type: Option<String>,
    /// `copy`.
    pub copy: Option<String>,
    /// `status`.
    pub status: Option<String>,
    /// `expires`.
    pub expires: Option<NaiveDate>,
    /// Place breadcrumb, outermost first (`Nasr City office › Safe`).
    pub location: Vec<EntityRef>,
    /// Holder.
    pub holder: Option<EntityRef>,
    /// Last holder.
    pub last_holder: Option<EntityRef>,
    /// Custody history, newest first.
    pub custody: Vec<CustodyItem>,
    /// Copies of this document (and the original, for a copy).
    pub copies: Vec<EntityRef>,
    /// Companies/people it concerns.
    pub concerns: Vec<EntityRef>,
}

/// A place page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceView {
    /// ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Enclosing places, outermost first.
    pub breadcrumb: Vec<EntityRef>,
    /// Direct sub-places.
    pub sub_places: Vec<EntityRef>,
    /// Documents here or in any nested place, by title.
    pub documents: Vec<DocumentBrief>,
    /// Custody events at this place or nested places, newest first (≤ 20).
    pub recent_movements: Vec<CustodyItem>,
}

/// Which page an ID leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityPageKind {
    /// Unknown ID (or not an entity).
    NotFound,
    /// Person/company.
    Entity,
    /// Document.
    Document,
    /// Place.
    Place,
}

/// Entity, document or place screen (one of the pages is set, per `kind`).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityScreen {
    /// ID asked for.
    pub id: String,
    /// Kind.
    pub kind: EntityPageKind,
    /// Person/company page.
    pub entity: Option<EntityView>,
    /// Document page.
    pub document: Option<DocumentView>,
    /// Place page.
    pub place: Option<PlaceView>,
}

// ---------------------------------------------------------------------------------------------
// Search, Ask, graph (§11 screens 5, 6, 9, 10)
// ---------------------------------------------------------------------------------------------

/// Search mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Local full-text (works offline).
    Keyword,
    /// Embeddings (server only).
    Semantic,
    /// Both (server only).
    Hybrid,
}

/// A search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Note ID.
    pub note_id: String,
    /// Title.
    pub title: String,
    /// Path.
    pub path: String,
    /// Kind.
    pub kind: String,
    /// Snippet around the match (plain text).
    pub snippet: String,
}

/// Search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchView {
    /// Query.
    pub query: String,
    /// Mode used.
    pub mode: SearchMode,
    /// Results, best first.
    pub results: Vec<SearchHit>,
    /// Whether the asked mode could run (semantic/hybrid need the server).
    pub availability: Availability,
}

/// A chat message in Ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskMessage {
    /// `user` | `assistant`.
    pub role: String,
    /// Text.
    pub text: String,
    /// Citations of an answer.
    pub citations: Vec<Citation>,
}

/// Ask (online only, §12.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskView {
    /// Whether Ask can be used.
    pub availability: Availability,
    /// The conversation.
    pub messages: Vec<AskMessage>,
}

/// A node of a graph view, with its position.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// Note ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Kind (`note`, `person`, …).
    pub kind: String,
    /// Ring (local graph: 0 = the focused note) or 0.
    pub depth: u8,
    /// Cluster (global map).
    pub cluster_id: Option<String>,
    /// Links + relations touching the node.
    pub degree: u32,
    /// Position (layout units; the renderer scales).
    pub x: f64,
    /// Position.
    pub y: f64,
}

/// An edge of a local graph.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    /// Source note.
    pub src: String,
    /// Target note.
    pub dst: String,
    /// `graph-algo` edge kind (`link`, `embed`, `relation:<type>`, `mention`, …).
    pub kind: String,
    /// `user` | `ai` (relations).
    pub by: Option<String>,
    /// AI confidence.
    pub confidence: Option<f64>,
}

/// A note's neighbourhood (depth 1–3) from cached links and relations, laid out radially
/// (local mind map, D4).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalGraphView {
    /// The focused note.
    pub center: String,
    /// Whether the note exists.
    pub found: bool,
    /// Depth used.
    pub depth: u8,
    /// Nodes, focus first, then by ring.
    pub nodes: Vec<GraphNode>,
    /// Edges.
    pub edges: Vec<GraphEdge>,
}

/// A cluster region label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterLabel {
    /// Cluster ID.
    pub id: String,
    /// Name (AI-named or user-named).
    pub name: String,
    /// Member count.
    pub size: u32,
}

/// The global map (D3): every note with force-layout positions (cached and warm-started).
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalGraphView {
    /// Nodes.
    pub nodes: Vec<GraphNode>,
    /// Edges.
    pub edges: Vec<GraphEdge>,
    /// Cluster labels, by name.
    pub clusters: Vec<ClusterLabel>,
}

// ---------------------------------------------------------------------------------------------
// Sync status and conflicts (§11 screen 12b)
// ---------------------------------------------------------------------------------------------

/// Outbox op status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxStatus {
    /// Waiting.
    Pending,
    /// Being pushed.
    Inflight,
    /// Conflict awaiting resolution.
    Conflict,
    /// Duplicate prompt awaiting a choice.
    Duplicate,
}

/// A queued op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxItem {
    /// Op ID.
    pub op_id: String,
    /// PLAN kind (`note.update`, …).
    pub kind: String,
    /// Title of the note it touches.
    pub title: Option<String>,
    /// Status.
    pub status: OutboxStatus,
    /// Push attempts.
    pub attempts: u32,
    /// Queued at.
    pub created: DateTime<Utc>,
}

/// A conflict in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictItem {
    /// Op ID.
    pub op_id: String,
    /// Note ID.
    pub note_id: String,
    /// Note title.
    pub title: String,
    /// When it happened.
    pub created: DateTime<Utc>,
}

/// A rolled-back op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionItem {
    /// Op ID.
    pub op_id: String,
    /// PLAN kind.
    pub kind: String,
    /// Problem type slug.
    pub problem_type: String,
    /// Localisation key (`error.<problem>`).
    pub message_key: String,
}

/// Sync status screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStatusView {
    /// The pill.
    pub pill: SyncPill,
    /// A full snapshot is cached.
    pub bootstrap_complete: bool,
    /// Last error (localisation key).
    pub last_error: Option<String>,
    /// Queued ops, in push order.
    pub outbox: Vec<OutboxItem>,
    /// Conflicts.
    pub conflicts: Vec<ConflictItem>,
    /// Rolled-back ops to acknowledge.
    pub rejections: Vec<RejectionItem>,
}

/// How to resolve one conflicting hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkChoiceKind {
    /// Keep the local side.
    Ours,
    /// Keep the server side.
    Theirs,
    /// Keep the base.
    Base,
    /// Body only: local lines, then the server's.
    OursThenTheirs,
    /// Body only: the server's lines, then local ones.
    TheirsThenOurs,
    /// Body only: [`HunkChoice::text`] instead.
    Text,
}

/// A choice for one hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkChoice {
    /// Hunk ID ([`ConflictHunkView::id`]).
    pub hunk: u32,
    /// The choice.
    pub choice: HunkChoiceKind,
    /// Replacement text for [`HunkChoiceKind::Text`] (ends with a line terminator).
    pub text: Option<String>,
}

/// How to resolve a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    /// Push the local content over the server's.
    KeepMine,
    /// Drop the local edit.
    KeepServer,
    /// Push [`ConflictResolution::content`].
    Merged,
    /// Resolve the merge preview hunk by hunk ([`ConflictResolution::choices`],
    /// `sync-model` `Conflicted::resolve`).
    Hunks,
}

/// How to resolve a conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictResolution {
    /// Kind.
    pub kind: ResolutionKind,
    /// `Merged`: the merged markdown.
    pub content: Option<String>,
    /// `Hunks`: one choice per hunk.
    pub choices: Vec<HunkChoice>,
}

/// One conflicting hunk of a merge preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictHunkView {
    /// Hunk ID.
    pub id: u32,
    /// `frontmatter:<key>` or `body:<line>`.
    pub location: String,
    /// Kind (`frontmatter_key`, `body`, `task_line`, …).
    pub kind: String,
    /// Base text.
    pub base: String,
    /// Local text.
    pub ours: String,
    /// Server text.
    pub theirs: String,
}

/// One conflict, for side-by-side resolution (D19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictDetail {
    /// Note ID.
    pub note_id: String,
    /// Title.
    pub title: String,
    /// The base both edits started from.
    pub base: Option<String>,
    /// The local version.
    pub local: Option<String>,
    /// The server version (once pulled).
    pub server: Option<String>,
    /// Local 3-way merge preview (conflict markers where hunks overlap).
    pub merged_preview: Option<String>,
    /// Whether the preview merged cleanly.
    pub merge_clean: Option<bool>,
    /// Conflicting hunks of the preview (resolve with [`ResolutionKind::Hunks`]).
    pub hunks: Vec<ConflictHunkView>,
}

/// The conflict screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictScreen {
    /// Op ID asked for.
    pub op_id: String,
    /// The conflict (`None`: unknown or resolved).
    pub conflict: Option<ConflictDetail>,
}

// ---------------------------------------------------------------------------------------------
// Settings, admin (§11 screens 13, 14)
// ---------------------------------------------------------------------------------------------

/// Notification permission as last reported by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPermission {
    /// Not reported yet.
    Unknown,
    /// Granted.
    Granted,
    /// Denied: "Reminders are off on this device".
    Denied,
}

/// Reminder settings of this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemindersSetting {
    /// Reminders on for this device.
    pub enabled: bool,
    /// Platform permission.
    pub permission: NotificationPermission,
    /// How reminders are delivered here.
    pub mode: NotificationMode,
    /// Reminders currently scheduled with the OS.
    pub scheduled: u32,
    /// Time used for date-only reminders (`HH:MM`).
    pub default_time: String,
}

/// Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsView {
    /// The account.
    pub account: AccountSummary,
    /// Reminders on this device.
    pub reminders: RemindersSetting,
    /// Devices list and revoke (online).
    pub devices: Availability,
    /// AI thresholds, auto-file, budget, AI status (server endpoints pending).
    pub ai: Availability,
    /// Export/import (online).
    pub export: Availability,
    /// Integrity warnings (server endpoint pending).
    pub integrity: Availability,
    /// Admin → Users entry.
    pub admin: Availability,
}

/// An account in Admin → Users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUserItem {
    /// ID.
    pub id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Role.
    pub role: String,
    /// Status.
    pub status: String,
    /// Created.
    pub created: DateTime<Utc>,
    /// Scheduled purge.
    pub deletion_at: Option<DateTime<Utc>>,
    /// The user downloaded their export.
    pub export_downloaded_at: Option<DateTime<Utc>>,
}

/// Admin → Users (online only, admins only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUsersView {
    /// Whether it can be shown.
    pub availability: Availability,
    /// Pending approvals, oldest first.
    pub pending: Vec<AdminUserItem>,
    /// Every other account, by username.
    pub users: Vec<AdminUserItem>,
}

// ---------------------------------------------------------------------------------------------
// Reminders (§12.5b)
// ---------------------------------------------------------------------------------------------

/// What the notification adapter must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationOpKind {
    /// `zonedSchedule(id, at, …)`.
    Schedule,
    /// Replace a scheduled notification (same ID, new time or text).
    Update,
    /// `cancel(id)`.
    Cancel,
    /// Show immediately (Linux, while running).
    ShowNow,
}

/// An operation for the notification adapter in Dart (`flutter_local_notifications`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationOp {
    /// What to do.
    pub kind: NotificationOpKind,
    /// Stable ID.
    pub id: i32,
    /// When (UTC; `Schedule` / `Update`).
    pub at: Option<DateTime<Utc>>,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Task block ID (payload for Done/Snooze).
    pub task_id: String,
}

impl NotificationOp {
    /// `Cancel { id }`.
    pub fn cancel(id: i32) -> Self {
        Self {
            kind: NotificationOpKind::Cancel,
            id,
            at: None,
            title: String::new(),
            body: String::new(),
            task_id: String::new(),
        }
    }
}

/// What the platform said about an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationResult {
    /// Done.
    Ok,
    /// Notification or exact-alarm permission denied.
    PermissionDenied,
    /// Too many pending notifications.
    PlatformLimit,
}

/// A notification action tapped by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationActionKind {
    /// Mark the task done.
    Done,
    /// Remind again later.
    Snooze,
}

/// A notification action tapped by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationAction {
    /// Kind.
    pub kind: NotificationActionKind,
    /// `Snooze`: minutes from now.
    pub minutes: u32,
}

/// App lifecycle events forwarded by Dart (sync and reminder triggers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycle {
    /// Foreground.
    Resumed,
    /// Background.
    Paused,
}

// ---------------------------------------------------------------------------------------------
// Errors at the bridge
// ---------------------------------------------------------------------------------------------

/// A failed call, as Dart receives it (thrown as an exception). `code` is typed; `message_key`
/// is the localisation key (`error.<code>`); the other fields carry the error's IDs and codes
/// (never user content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreFailure {
    /// Stable code (`offline`, `not_found`, `pending_changes`, …).
    pub code: String,
    /// Localisation key.
    pub message_key: String,
    /// Argument or item concerned (`path`, `note`, …).
    pub field: Option<String>,
    /// Reason code.
    pub reason: Option<String>,
    /// Count (`pending_changes`).
    pub count: Option<u32>,
    /// HTTP status (`server`).
    pub status: Option<u16>,
}

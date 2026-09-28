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

/// Direction of a piece of user content, from its first strong character (Unicode P2,
/// PLAN §11: per paragraph/line). `Neutral`: no strong character (digits, punctuation) —
/// the ambient direction applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDir {
    /// Left to right (Latin, …).
    Ltr,
    /// Right to left (Arabic, …).
    Rtl,
    /// No strong character.
    Neutral,
}

/// A span of a displayed text (UTF-16 offsets into that text, end exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    /// Start (inclusive).
    pub start: u32,
    /// End (exclusive).
    pub end: u32,
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

/// What the sync pill shows (the core picks one state by priority: conflict > duplicates >
/// syncing > paused > offline > error > synced).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPillKind {
    /// Everything synced.
    Synced,
    /// The server cannot be reached; ops are queued.
    Offline,
    /// A push/pull is running (see `progress_*`).
    Syncing,
    /// A conflict waits for the user.
    Conflict,
    /// An "Already exists" prompt waits for the user.
    Duplicates,
    /// The user paused sync.
    Paused,
    /// The last cycle failed (not offline); retrying.
    Error,
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
    /// Ops of this cycle's push already answered.
    pub ops_done: u32,
    /// Ops to push in this cycle (all pending when it started).
    pub ops_total: u32,
    /// Changes pulled so far in this cycle.
    pub pulled: u32,
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
    /// The state to show.
    pub display: SyncPillKind,
    /// `Syncing`: work done ("Syncing 12/40").
    pub progress_done: u32,
    /// `Syncing`: work in total (0 when unknown).
    pub progress_total: u32,
    /// Last successful sync as a local time label ("14:32", "Sat 18:40"), in the account's
    /// time zone and UI language.
    pub last_sync_label: Option<String>,
    /// The pill's text ("Synced", "Offline · 3 queued", "Syncing 12/40", "1 conflict").
    pub label: String,
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Direction of the snippet.
    pub snippet_dir: TextDir,
    /// `updated_at` for lists ("14:31" today, "Sat" this week, "21 Sep", "21 Sep 2025").
    pub updated_label: String,
    /// Outgoing links and relations ("4 links").
    pub link_count: u32,
    /// Matched spans in `snippet` (search hits, mentions of an entity).
    pub highlights: Vec<TextSpan>,
}

/// A reference to another note/entity (resolved when `id` is set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    /// The note ID when the link resolves locally.
    pub id: Option<String>,
    /// Display title.
    pub title: String,
    /// The target's kind (`note`, `person`, `company`, `document`, `place`, …) when resolved.
    pub kind: Option<String>,
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
    /// Direction of `text`.
    pub dir: TextDir,
    /// `date` as a label ("12 Sep").
    pub date_label: Option<String>,
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
    /// Avatar initials ("SN" for Sara Nabil).
    pub initials: String,
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
    /// Avatar initials.
    pub initials: String,
}

/// A device signed in to the account (Settings → Devices, account sheet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceItem {
    /// Device ID.
    pub id: String,
    /// Name.
    pub name: String,
    /// Platform (`android`, `ios`, `macos`, `windows`, `linux`).
    pub platform: String,
    /// Last request from it.
    pub last_seen: DateTime<Utc>,
    /// `last_seen` as a label ("Active now", "14:32", "Sat").
    pub last_seen_label: String,
    /// When it signed in.
    pub signed_in: DateTime<Utc>,
    /// `signed_in` as a label ("12 Sep 2026").
    pub signed_in_label: String,
    /// This install.
    pub is_this_device: bool,
    /// Reminders are delivered to it (D27).
    pub reminders_enabled: bool,
}

/// A sign-up or sign-in waiting for an admin's decision (D22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    /// Username.
    pub username: String,
    /// Server.
    pub server_url: String,
    /// When the account was requested (sign-up) or first found pending on this device.
    pub requested_at: DateTime<Utc>,
    /// "sent 2 hours ago".
    pub requested_label: String,
    /// Last "Check again".
    pub last_checked_at: Option<DateTime<Utc>>,
    /// "Last checked 14:32".
    pub last_checked_label: Option<String>,
    /// Whether "Check again" can run without asking for the password again (the password
    /// is kept in memory only, never on disk).
    pub can_check: bool,
}

/// Password strength for the sign-up meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordLevel {
    /// Shorter than the server's minimum.
    TooShort,
    /// Long enough, one kind of character.
    Weak,
    /// Two or three kinds, or long.
    Fair,
    /// Four kinds, or very long.
    Strong,
}

/// Result of `password_strength`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordStrength {
    /// Level.
    pub level: PasswordLevel,
    /// Characters typed.
    pub length: u32,
    /// The server's minimum (default configuration).
    pub min_length: u32,
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
    /// Signed up (or signed in) and waiting for an admin's approval (D22).
    PendingApproval,
    /// An admin rejected the sign-up.
    Rejected,
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
    /// `DeletionPending`: `deletion_at` as a local date in the account's time zone
    /// ("11 Oct 2026").
    pub deletion_label: Option<String>,
    /// `PendingApproval` / `Rejected`: the request.
    pub pending: Option<PendingApproval>,
    /// Size of the last downloaded export (`DeletionPending`, after `download_export`).
    pub export_size_bytes: Option<u64>,
    /// Notes in the last downloaded export.
    pub export_note_count: Option<u32>,
    /// `export_size_bytes` / `export_note_count` as "18.4 MB · 412 notes".
    pub export_label: Option<String>,
    /// This device as the server lists it (account sheet), once fetched.
    pub this_device: Option<DeviceItem>,
    /// Devices of the account ("Devices 3"), once fetched.
    pub device_count: Option<u32>,
    /// Admins: accounts waiting for approval ("2 pending"), once fetched.
    pub pending_approvals: Option<u32>,
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
            deletion_label: None,
            pending: None,
            export_size_bytes: None,
            export_note_count: None,
            export_label: None,
            this_device: None,
            device_count: None,
            pending_approvals: None,
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
    /// The wall-clock time (date-only reminders use the default reminder time): pass it back
    /// to `remove_reminder`.
    pub local_at: NaiveDateTime,
    /// Time of day ("09:00").
    pub time_label: String,
    /// Relative to the due date ("on the day", "30 days before"), or the date when the task
    /// has no due date ("Thu 1 Oct").
    pub offset_label: String,
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
    /// Direction of `description`.
    pub description_dir: TextDir,
    /// Path of the note holding the line ("tasks/Tasks.md").
    pub note_path: String,
    /// 1-based line number of the task in that file.
    pub line_number: u32,
    /// Due date as a label ("Today", "Tomorrow", "Tue 29 Sep", "1 Oct 2027").
    pub due_label: Option<String>,
    /// Open and due before today: "3 days late".
    pub lateness_label: Option<String>,
    /// Done/cancelled: "on time" / "2 days late" (history rows).
    pub completion_label: Option<String>,
    /// Open and recurring: "next in 4 days" (time to the due/scheduled date).
    pub next_in_label: Option<String>,
    /// Where it came from ("From document expiry" for tasks linking a document with an
    /// expiry date), when known.
    pub origin_label: Option<String>,
    /// Open and due before today.
    pub is_overdue: bool,
}

/// Upcoming tasks of one day (or "LATER").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGroup {
    /// Header ("TUE 29 SEP", "LATER").
    pub label: String,
    /// The day (`None` for the "later" group).
    pub date: Option<NaiveDate>,
    /// Tasks, in section order.
    pub tasks: Vec<TaskItem>,
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
    /// `upcoming` grouped by day for the next 7 days, then one "later" group.
    pub upcoming_groups: Vec<TaskGroup>,
    /// Overdue + today ("Today 2").
    pub today_count: u32,
}

/// A capture in the Home inbox preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxPreviewItem {
    /// The capture's note.
    pub note_id: String,
    /// Text (first line, ≤ 120 characters).
    pub text: String,
    /// Direction of `text`.
    pub text_dir: TextDir,
    /// What the AI proposes ("→ Weekly invoicing request — Acme", "Who is “بابا”?").
    pub summary: String,
    /// Needs a decision only the user can make.
    pub needs_you: bool,
}

/// An open item of a person or company (`## Open items`), for Home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenItem {
    /// Stable ID (`<entity id>:<n>`).
    pub id: String,
    /// Text.
    pub text: String,
    /// Direction of `text`.
    pub text_dir: TextDir,
    /// The person/company.
    pub person: EntityRef,
    /// First citation.
    pub citation: Option<Citation>,
    /// Done (always `false` until the format records done items).
    pub done: bool,
}

/// One entry of the AI activity feed.
#[derive(Debug, Clone, PartialEq)]
pub struct AiActivityItem {
    /// When ("14:05").
    pub at_label: String,
    /// `relation_added` | `contradiction` | `timeline` | `custody_applied`.
    pub kind: String,
    /// One-line summary.
    pub summary: String,
    /// Source.
    pub source: Option<EntityRef>,
    /// Target.
    pub target: Option<EntityRef>,
    /// Relation type.
    pub rel_type: Option<String>,
    /// Confidence.
    pub confidence: Option<f64>,
    /// Suggestion to undo it.
    pub undo_suggestion_id: Option<String>,
}

/// Which notes the "Recent" block lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentFilter {
    /// Most recently edited.
    Edited,
    /// Most recently created.
    Created,
    /// Captures the AI filed (notes with `source:` that left the inbox).
    FiledByAi,
}

/// The "Recent" block with a filter (`HomeExpanded`).
#[derive(Debug, Clone, PartialEq)]
pub struct RecentNotesView {
    /// Filter.
    pub filter: RecentFilter,
    /// Notes (≤ 20).
    pub notes: Vec<NoteListItem>,
}

/// Navigation counts and pinned notes (sidebar, rail, bottom bar).
#[derive(Debug, Clone, PartialEq)]
pub struct NavView {
    /// Inbox items awaiting review.
    pub inbox_count: u32,
    /// Open tasks due today or overdue ("Tasks 3").
    pub tasks_due_count: u32,
    /// Live notes.
    pub notes_count: u32,
    /// People + companies + documents + places.
    pub directory_count: u32,
    /// Map clusters.
    pub cluster_count: u32,
    /// Pinned notes, in pin order.
    pub pinned: Vec<NoteListItem>,
    /// Sync indicator.
    pub sync: SyncPill,
}

/// Home / Capture.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeView {
    /// Most recently changed notes (≤ 10).
    pub recent_notes: Vec<NoteListItem>,
    /// Inbox items awaiting review.
    pub inbox_count: u32,
    /// Task sections (compact shows Today / Upcoming / Recurring here, D28).
    pub tasks: TaskSections,
    /// Sync indicator.
    pub sync: SyncPill,
    /// "Sunday 27 September".
    pub today_label: String,
    /// "Good afternoon, Shawket".
    pub greeting: String,
    /// The account's display name.
    pub display_name: String,
    /// The newest captures with the AI's proposal (≤ 3).
    pub inbox_preview: Vec<InboxPreviewItem>,
    /// Captures needing a decision only the user can make.
    pub needs_you_count: u32,
    /// Contradictions to review.
    pub contradictions_count: u32,
    /// Summary line ("1 needs you · 1 contradiction to review"); empty when nothing waits.
    pub inbox_summary: String,
    /// AI activity feed state (the server has no activity feed yet).
    pub ai_activity: Availability,
    /// AI activity entries, newest first.
    pub ai_activity_items: Vec<AiActivityItem>,
    /// "3 relations added, 1 contradiction found".
    pub ai_activity_headline: String,
    /// Open items roll-up state.
    pub open_items: Availability,
    /// Open items of people and companies, newest entity first (≤ 10).
    pub open_item_list: Vec<OpenItem>,
    /// Pinned notes.
    pub pinned: Vec<NoteListItem>,
}

/// The Tasks destination (medium/expanded) incl. completed tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksView {
    /// Open tasks grouped.
    pub sections: TaskSections,
    /// Done and cancelled tasks, most recent first (≤ 50).
    pub done: Vec<TaskItem>,
    /// Open tasks ("Open · 5").
    pub open_count: u32,
    /// Completed since Monday of this week ("4 done this week").
    pub done_this_week: u32,
    /// "4 done this week".
    pub done_this_week_label: String,
    /// Notes containing open tasks ("6 notes contain tasks").
    pub notes_with_tasks: u32,
}

/// A note that can hold new tasks (new-task sheet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskHomeItem {
    /// Note ID (`None`: `tasks/Tasks.md` does not exist yet and will be created).
    pub note_id: Option<String>,
    /// Title.
    pub title: String,
    /// Path.
    pub path: String,
    /// The default home (`tasks/Tasks.md`).
    pub is_default: bool,
    /// Open tasks in it.
    pub open_tasks: u32,
}

/// Candidate homes for a new task: the default first, then notes holding tasks, by title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskHomesView {
    /// Homes.
    pub homes: Vec<TaskHomeItem>,
}

/// How often a recurrence repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceFrequency {
    /// Days.
    Daily,
    /// Weeks.
    Weekly,
    /// Months.
    Monthly,
    /// Years.
    Yearly,
}

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekdayKind {
    /// Monday.
    Mon,
    /// Tuesday.
    Tue,
    /// Wednesday.
    Wed,
    /// Thursday.
    Thu,
    /// Friday.
    Fri,
    /// Saturday.
    Sat,
    /// Sunday.
    Sun,
}

/// Which day of the month a monthly/yearly rule falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthDayMode {
    /// The due date's own day.
    SameDay,
    /// `month_days` (1–31, clamped to the month).
    Days,
    /// The last day of the month.
    LastDay,
    /// `nth` `nth_weekday` ("2nd Wednesday", `nth` = −1 for "last").
    NthWeekday,
}

/// The recurrence editor's structured form (Tasks plugin grammar, `vault-format`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceForm {
    /// Frequency.
    pub frequency: RecurrenceFrequency,
    /// Every N periods (≥ 1).
    pub interval: u32,
    /// Weekly: days (empty = the due date's weekday).
    pub weekdays: Vec<WeekdayKind>,
    /// Monthly/yearly: the day rule.
    pub month_day_mode: MonthDayMode,
    /// `Days`: days of the month.
    pub month_days: Vec<u32>,
    /// `NthWeekday`: position (1–5, −1…−5).
    pub nth: i32,
    /// `NthWeekday`: weekday.
    pub nth_weekday: Option<WeekdayKind>,
    /// Yearly: months 1–12 (empty = the due date's month).
    pub months: Vec<u32>,
    /// Count from the completion date ("when done").
    pub when_done: bool,
}

/// A recurrence phrase compiled from a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceCompose {
    /// The phrase to store (`every month on the 1st`).
    pub phrase: String,
    /// Whether the grammar understands it (always `true` for a valid form).
    pub understood: bool,
    /// Human summary in the UI language ("Every month on the 1st").
    pub label: String,
}

/// One upcoming occurrence of a recurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrencePreviewItem {
    /// The date.
    pub date: NaiveDate,
    /// "Thu 1 Oct 2026" / "Sun 1 Nov".
    pub label: String,
    /// The first entry is the current due date.
    pub is_due: bool,
}

/// What a "chip" of the parsed task text stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskChipKind {
    /// 📅
    Due,
    /// 🔁
    Recurrence,
    /// ⏰
    Reminder,
    /// A linked note/entity.
    Link,
    /// Priority.
    Priority,
}

/// One "Understood as" chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskChip {
    /// Kind.
    pub kind: TaskChipKind,
    /// Label ("Tomorrow", "every Monday", "09:00", "Acme").
    pub label: String,
}

/// A new task's text as the core understands it (`TaskEditCompact` "Understood as").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDraftPreview {
    /// Description without the understood phrases.
    pub description: String,
    /// Direction of `description`.
    pub description_dir: TextDir,
    /// 📅
    pub due: Option<NaiveDate>,
    /// Due label.
    pub due_label: Option<String>,
    /// 🔁 phrase.
    pub recurrence: Option<String>,
    /// Reminders (local wall-clock).
    pub reminders: Vec<NaiveDateTime>,
    /// Priority (`high`, …).
    pub priority: Option<String>,
    /// Linked notes/entities.
    pub links: Vec<EntityRef>,
    /// Chips, in text order.
    pub chips: Vec<TaskChip>,
    /// The draft to pass to `create_task` (home note left to the caller).
    pub draft: TaskDraft,
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
    /// "tasks/Tasks.md · line 14".
    pub location_label: String,
    /// Devices reminders go to (`Pixel 9, MacBook Pro`), once the device list was fetched.
    pub delivery_label: Option<String>,
    /// "Next occurrence in 4 days".
    pub next_occurrence_label: Option<String>,
    /// The recurrence as a form (`None`: no recurrence or not understood).
    pub recurrence_form: Option<RecurrenceForm>,
    /// Next occurrences (≤ 3), from the due date.
    pub recurrence_preview: Vec<RecurrencePreviewItem>,
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
    /// Vault path of the existing item's note, when known locally ("tasks/Tasks.md").
    pub path: Option<String>,
    /// Why it matched ("Same title", "Very similar text", "Similar meaning").
    pub reason: String,
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
    /// An edit made offline conflicted; the server kept both (`copy`).
    Conflict,
    /// Two stored items look like duplicates (nightly sweep, `duplicates`).
    Duplicates,
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
    /// Custody: the resulting location.
    pub location: Option<EntityRef>,
    /// Custody: the resulting holder.
    pub holder: Option<EntityRef>,
    /// Custody: the resulting last holder.
    pub last_holder: Option<EntityRef>,
    /// Custody: documents the event may be about ("Which contract?"); pick one with
    /// `accept_suggestion_choice`.
    pub document_choices: Vec<EntityRef>,
    /// The resolved timeline date of a mention, when the AI provides one.
    pub timeline: Option<TimelineChip>,
    /// Conflict / duplicates: the other note (conflict copy, second item).
    pub other: Option<EntityRef>,
}

/// "Timeline · Mon 28 Sep (from “بكرة”)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineChip {
    /// Resolved date label.
    pub date_label: String,
    /// The phrase it came from.
    pub source_phrase: String,
    /// Entities whose timeline gets the entry.
    pub targets: Vec<EntityRef>,
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
            location: None,
            holder: None,
            last_holder: None,
            document_choices: Vec::new(),
            timeline: None,
            other: None,
        }
    }
}

/// A message of a suggestion's thread (§9.8 "threaded suggestions").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessage {
    /// Reply ID.
    pub id: String,
    /// `user` | `ai`.
    pub author: String,
    /// Text.
    pub text: String,
    /// Direction of `text`.
    pub text_dir: TextDir,
    /// "14:05".
    pub created_label: String,
    /// Not synced yet.
    pub pending_sync: bool,
}

/// A suggestion.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent display flags
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
    /// `created` as a label ("09:47", "Sat 18:40").
    pub created_label: String,
    /// The capture text behind a suggestion not shown under its capture.
    pub source_text: Option<String>,
    /// Direction of `source_text`.
    pub source_dir: TextDir,
    /// The AI applied it on its own (D30 custody above the threshold): show "Applied
    /// automatically" with Undo / Looks right.
    pub auto_applied: bool,
    /// Plain Accept can run (no choice missing).
    pub can_accept: bool,
    /// Only the user can decide (who-is, duplicates, a document choice).
    pub needs_you: bool,
    /// Replies, oldest first.
    pub thread: Vec<ThreadMessage>,
}

/// A capture in the inbox.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent display flags
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
    /// Direction of `text`.
    pub text_dir: TextDir,
    /// `created` as a label ("09:47", "Sat 18:40").
    pub created_label: String,
    /// Where it came from ("Typed on Pixel 8", "Voice"), from the note's `source:`.
    pub source_label: Option<String>,
    /// The AI's filing confidence, when it proposed a filing.
    pub filing_confidence: Option<f64>,
    /// Some suggestion needs the user.
    pub needs_you: bool,
    /// Every suggestion can be accepted as is ("Accept all ready").
    pub ready: bool,
    /// It resembles existing items (a `duplicate` suggestion is pending).
    pub is_duplicate: bool,
}

/// Inbox filter tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxFilter {
    /// Everything.
    All,
    /// Captures and suggestions needing the user.
    NeedsYou,
    /// Conflicts and contradictions.
    Conflicts,
}

/// Inbox.
#[derive(Debug, Clone, PartialEq)]
pub struct InboxView {
    /// Captures, newest first.
    pub captures: Vec<InboxItem>,
    /// Pending suggestions not tied to an inbox capture (entity link-or-create, custody, …).
    pub suggestions: Vec<SuggestionItem>,
    /// Filter applied.
    pub filter: InboxFilter,
    /// Captures ready to accept as proposed ("Accept all ready").
    pub ready_count: u32,
    /// Captures and suggestions needing the user (unfiltered count).
    pub needs_you_count: u32,
    /// Conflicts and contradictions (unfiltered count).
    pub conflicts_count: u32,
    /// Everything (unfiltered count).
    pub all_count: u32,
}

/// Edits to a proposal before accepting it (`InboxExpanded`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuggestionEdits {
    /// Filing: title.
    pub title: Option<String>,
    /// Filing: folder.
    pub folder: Option<String>,
    /// Filing: tags.
    pub tags: Option<Vec<String>>,
    /// Task: text.
    pub text: Option<String>,
    /// Task: due date.
    pub due: Option<NaiveDate>,
    /// Task: recurrence phrase.
    pub recurrence: Option<String>,
}

/// Link-or-create answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOrCreateKind {
    /// Link the mention to an existing entity (it becomes an alias).
    Link,
    /// Create a new person/company named `name` with the mention as alias.
    Create,
}

/// The user's answer to "Who is “بابا”?".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkOrCreateChoice {
    /// Kind.
    pub kind: LinkOrCreateKind,
    /// `Link`: the entity.
    pub entity_id: Option<String>,
    /// `Create`: the new entity's name.
    pub name: Option<String>,
    /// `Create`: `person` (default) or `company`.
    pub entity_kind: Option<String>,
    /// `Create`: skip the duplicate check.
    pub force: bool,
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
    /// Localised label of the type ("works at" for `works-at`).
    pub rel_label: String,
    /// When the AI added it ("14:05"); `None` when the server does not say.
    pub created_label: Option<String>,
    /// Blocks the AI cited for it.
    pub citations: Vec<Citation>,
}

/// A note linking here.
#[derive(Debug, Clone, PartialEq)]
pub struct BacklinkItem {
    /// Source note.
    pub note_id: String,
    /// Its title.
    pub title: String,
    /// Direction of the title.
    pub title_dir: TextDir,
    /// The linking sentence (the line holding the link), ≤ 200 characters.
    pub snippet: Option<String>,
    /// Direction of the snippet.
    pub snippet_dir: TextDir,
    /// `user` | `ai` for relations.
    pub by: Option<String>,
    /// AI confidence.
    pub confidence: Option<f64>,
}

/// Backlinks of one kind (`link` for body links, or a relation type).
#[derive(Debug, Clone, PartialEq)]
pub struct BacklinkGroup {
    /// `link` or the relation type.
    pub kind: String,
    /// Sources, by title.
    pub items: Vec<BacklinkItem>,
    /// Localised label of `kind` ("Links", "works at").
    pub label: String,
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
    /// `**bold**` / `__bold__`.
    Bold,
    /// `*italic*` / `_italic_`.
    Italic,
    /// `~~strike~~`.
    Strike,
    /// `==mark==`.
    Mark,
    /// A line whose first strong character is right-to-left (lay it out RTL).
    RtlLine,
    /// A line whose first strong character is left-to-right.
    LtrLine,
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
    /// `WikiLink` / `Embed`: the resolved note.
    pub target_id: Option<String>,
    /// `WikiLink` / `Embed`: heading or block (without `^`).
    pub target_anchor: Option<String>,
    /// `TaskLine`: the task's block ID.
    pub task_id: Option<String>,
    /// `Heading`: level 1–6 (0 otherwise).
    pub level: u8,
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
    /// Creating it found an existing item ("Already exists"; open the duplicate sheet).
    Duplicate,
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
    /// The create op awaiting a duplicate choice.
    pub duplicate_op_id: Option<String>,
    /// The status line ("Saved · v7", "Saved on this device · 2 changes to sync",
    /// "Conflict", "Already exists").
    pub label: String,
}

/// One revision of a note (history panel, online).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Commit ID.
    pub commit: String,
    /// "v7" (1 = oldest).
    pub version_label: String,
    /// Commit message.
    pub message: String,
    /// `user` | `ai` | `system`.
    pub author: String,
    /// When.
    pub at: DateTime<Utc>,
    /// "Today 14:31", "Sat 18:40", "12 Sep".
    pub at_label: String,
    /// Reverting to it is possible (not the current revision, not a deletion).
    pub can_revert: bool,
}

/// A line of a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Unchanged.
    Same,
    /// Only in the newer text.
    Added,
    /// Only in the older text.
    Removed,
}

/// One line of a diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Kind.
    pub kind: DiffLineKind,
    /// Line number in the older text (1-based).
    pub old_line: Option<u32>,
    /// Line number in the newer text (1-based).
    pub new_line: Option<u32>,
    /// The line (no terminator).
    pub text: String,
    /// Its direction.
    pub dir: TextDir,
}

/// A revision compared with the current note (history panel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDiffView {
    /// Note.
    pub note_id: String,
    /// The revision's commit.
    pub commit: String,
    /// "+2 lines, 1 removed".
    pub summary: String,
    /// Lines: the revision (old) against the current content (new).
    pub lines: Vec<DiffLine>,
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Version of `content` (pass it to `update_note` as `base_version`).
    pub content_version: String,
    /// "v7" once the history was fetched (`refresh_history`).
    pub version_label: Option<String>,
    /// "Created 18 Sep".
    pub created_label: Option<String>,
    /// "Edited today 14:31".
    pub edited_label: Option<String>,
    /// Who made the last change, once the history was fetched ("Shawket", "AI").
    pub edited_by: Option<String>,
    /// Words in the body.
    pub word_count: u32,
    /// Incoming links and relations ("Backlinks 6").
    pub backlink_count: u32,
    /// Revisions, newest first (once fetched while online).
    pub history_entries: Vec<HistoryEntry>,
    /// Pinned to the sidebar.
    pub pinned: bool,
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
    /// Root → this folder (the root has `path` `""`).
    pub breadcrumb: Vec<FolderItem>,
    /// Notes directly in this folder.
    pub note_count: u32,
}

/// What the token before the caret asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// Nothing to complete.
    None,
    /// `[[query` → notes.
    WikiLink,
    /// `@query` → people and companies.
    Mention,
    /// `#query` → tags.
    Tag,
    /// `[[Note#^query` → blocks of that note.
    BlockRef,
}

/// One completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Shown text.
    pub label: String,
    /// Secondary text (path, kind, count, block text).
    pub detail: String,
    /// Text replacing `replace_start..replace_end`.
    pub insert_text: String,
    /// Note/entity the item points to.
    pub target_id: Option<String>,
    /// `person` | `company` | … for mentions.
    pub entity_kind: Option<String>,
    /// Direction of `label`.
    pub label_dir: TextDir,
}

/// Editor completions at the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completions {
    /// Kind.
    pub kind: CompletionKind,
    /// Replace from (UTF-16, inclusive).
    pub replace_start: u32,
    /// Replace to (UTF-16, exclusive).
    pub replace_end: u32,
    /// The typed query.
    pub query: String,
    /// Items, best first (≤ 20).
    pub items: Vec<CompletionItem>,
}

/// A tag with its note count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagItem {
    /// Tag (without `#`).
    pub tag: String,
    /// Notes carrying it.
    pub count: u32,
}

/// A block of a note (block reference picker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockItem {
    /// Existing block ID (without `^`), or `None` for a block without one (inserting a
    /// reference to it adds an ID).
    pub block_id: Option<String>,
    /// Plain text (≤ 160 characters).
    pub text: String,
    /// Direction.
    pub text_dir: TextDir,
    /// 1-based line of the block's start.
    pub line: u32,
}

/// Result of `insert_mention`: the new content and caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionEdit {
    /// New full content (the link inserted and the entity added to `people:` /
    /// `companies:`); save it with `update_note`.
    pub content: String,
    /// Caret after the inserted link (UTF-16).
    pub cursor: u32,
}

/// A relation type the user can pick (properties panel, retype).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTypeItem {
    /// Key (`works-at`).
    pub key: String,
    /// Localised label ("works at").
    pub label: String,
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
    /// Kind (`person`, `company`, `document`, `place`).
    pub kind: String,
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Avatar initials ("AS").
    pub initials: String,
    /// Notes mentioning it ("14 mentions").
    pub mention_count: u32,
    /// Last time a note mentioning it (or it) changed.
    pub last_active: Option<DateTime<Utc>>,
    /// "Today" / "Thu 24 Sep".
    pub last_active_label: Option<String>,
    /// People: role.
    pub role: Option<String>,
    /// People: company (`works-at` / `companies:`).
    pub company: Option<EntityRef>,
    /// Companies: industry.
    pub industry: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Documents: `status`.
    pub status: Option<String>,
    /// Documents: `doc-type`.
    pub doc_type: Option<String>,
    /// Documents: place breadcrumb, outermost first.
    pub location: Vec<EntityRef>,
    /// Documents: holder.
    pub holder: Option<EntityRef>,
    /// Documents: last holder.
    pub last_holder: Option<EntityRef>,
    /// Documents: "Last with Shady · 20 Sep".
    pub holder_label: Option<String>,
    /// Documents: `copy`.
    pub copy: Option<String>,
    /// Documents: `expires`.
    pub expires: Option<NaiveDate>,
    /// Documents: "Expires 1 Mar 2027".
    pub expires_label: Option<String>,
    /// Documents: expires within 60 days (or expired).
    pub expiring_soon: bool,
    /// Places: enclosing places, outermost first.
    pub breadcrumb: Vec<EntityRef>,
    /// Places: documents here or in nested places.
    pub document_count: u32,
    /// Has open items (people/companies).
    pub has_open_items: bool,
}

/// Directory filters (all optional; empty = no filter).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectoryFilter {
    /// Every tag must be present.
    pub tags: Vec<String>,
    /// People: role (exact, case-insensitive).
    pub role: Option<String>,
    /// People: company ID.
    pub company_id: Option<String>,
    /// Companies: industry.
    pub industry: Option<String>,
    /// Documents: type.
    pub doc_type: Option<String>,
    /// Documents: status.
    pub status: Option<String>,
    /// Documents: stored in this place or a nested one.
    pub place_id: Option<String>,
    /// Documents: held by this person.
    pub holder_id: Option<String>,
    /// Documents: expiring within 60 days.
    pub expiring: bool,
    /// People/companies with open items.
    pub has_open_items: bool,
}

/// Directory order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySort {
    /// A–Z.
    Name,
    /// Most recently active first.
    LastActive,
    /// Documents: most recently moved first.
    RecentlyMoved,
}

/// A filter chip with its count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterOption {
    /// Facet (`tag`, `role`, `company`, `industry`, `doc_type`, `status`, `place`, `holder`,
    /// `expiring`, `has_open_items`).
    pub facet: String,
    /// Value (tag, role, ID, …; empty for boolean facets).
    pub value: String,
    /// Label.
    pub label: String,
    /// Rows of the tab (query applied) matching it.
    pub count: u32,
    /// Currently applied.
    pub selected: bool,
}

/// A labelled group of directory rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySection {
    /// "Recently active", "All people · A–Z".
    pub label: String,
    /// Rows.
    pub items: Vec<DirectoryItem>,
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
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryView {
    /// Tab shown.
    pub tab: DirectoryTab,
    /// Search text (matches aliases in both scripts after normalisation).
    pub query: String,
    /// Rows, by title.
    pub items: Vec<DirectoryItem>,
    /// Counts (unfiltered).
    pub counts: DirectoryCounts,
    /// Filter applied.
    pub filter: DirectoryFilter,
    /// Order applied to `items`.
    pub sort: DirectorySort,
    /// Filter chips with counts.
    pub filter_options: Vec<FilterOption>,
    /// Rows split for display: "Recently active" (≤ 3, when sorted by name and no query)
    /// then everything A–Z.
    pub sections: Vec<DirectorySection>,
    /// Pending entity suggestions for this tab (who-is, merges, custody).
    pub suggestions: Vec<SuggestionItem>,
    /// Documents expiring soon (badge of the "Expiring" chip).
    pub expiring_count: u32,
}

/// What merging two entities would move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePreview {
    /// The entity that disappears.
    pub source: EntityRef,
    /// The survivor.
    pub into: EntityRef,
    /// Aliases the survivor gains (incl. the source's name).
    pub aliases: Vec<String>,
    /// Notes whose links move.
    pub mention_count: u32,
    /// Relations that move.
    pub relation_count: u32,
}

/// A new document (Directory → Add document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDraft {
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// `doc-type`.
    pub doc_type: Option<String>,
    /// `original` | `copy` | `certified-copy`.
    pub copy: Option<String>,
    /// The original, for a copy.
    pub copy_of: Option<String>,
    /// Companies it concerns.
    pub companies: Vec<String>,
    /// People it concerns.
    pub people: Vec<String>,
    /// Expiry.
    pub expires: Option<NaiveDate>,
}

/// A new place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceDraft {
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Enclosing place.
    pub parent_id: Option<String>,
    /// Address.
    pub address: Option<String>,
}

/// A recorded custody event (document page → Record a move).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustodyDraft {
    /// Event type (`stored-at`, `moved-to`, `handed-to`, `returned-by`, `sent-to`,
    /// `received-from`, `lost`, `found`, `destroyed`).
    pub kind: String,
    /// Place.
    pub place_id: Option<String>,
    /// Person.
    pub person_id: Option<String>,
    /// Third party.
    pub counterparty_id: Option<String>,
    /// Date.
    pub date: NaiveDate,
}

/// A place in the custody place picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceOption {
    /// Place ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Enclosing places, outermost first.
    pub breadcrumb: Vec<EntityRef>,
    /// Nesting depth (0 = top level).
    pub depth: u32,
    /// The document is there now (`place_options(document_id)`).
    pub is_current: bool,
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Initials.
    pub initials: String,
    /// Path ("people/ahmed-samir.md").
    pub path: String,
    /// Tags.
    pub tags: Vec<String>,
    /// The user-owned `## Notes` body (edit with `update_user_notes`).
    pub user_notes: String,
    /// Citations in `## Summary`.
    pub summary_citations: Vec<Citation>,
    /// "AI-maintained · updated 2h ago", when the server says when the AI last wrote.
    pub ai_updated_label: Option<String>,
    /// Open items ("2 open · 1 done").
    pub open_count: u32,
    /// Done open items.
    pub done_count: u32,
    /// Notes mentioning it (all, even when `mentions` is truncated).
    pub mention_count: u32,
    /// "last active today".
    pub last_active_label: Option<String>,
    /// `## Summary` direction.
    pub summary_dir: TextDir,
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
    /// `doc-type`.
    pub doc_type: Option<String>,
    /// Last holder.
    pub last_holder: Option<EntityRef>,
    /// Place breadcrumb, outermost first.
    pub location_path: Vec<EntityRef>,
    /// Expires within 60 days (or expired).
    pub expiring_soon: bool,
    /// Direction of the title.
    pub title_dir: TextDir,
}

/// A place in a place page's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNode {
    /// The place.
    pub place: EntityRef,
    /// Depth below the page's place (1 = direct sub-place).
    pub depth: u32,
    /// Documents stored there (not nested).
    pub document_count: u32,
    /// Its parent (the page's place for depth 1).
    pub parent_id: String,
}

/// One custody event.
#[derive(Debug, Clone, PartialEq)]
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
    /// `user` (recorded by the user, no citation) | `ai`.
    pub by: String,
    /// AI confidence, when the AI says.
    pub confidence: Option<f64>,
    /// The AI decision behind it, for "Undo".
    pub decision_id: Option<String>,
    /// Localisation key of the sentence (`custody.stored_at`, `custody.handed_to`, …).
    pub sentence_key: String,
    /// Who acted (the person for `handed-to`/`returned-by`, the third party for
    /// `sent-to`/`received-from`).
    pub actor: Option<EntityRef>,
    /// Where it went (the place, or the person/third party it went to).
    pub destination: Option<EntityRef>,
    /// Ready sentence in the UI language ("Shady returned it to Safe").
    pub sentence: String,
    /// "20 Sep".
    pub date_label: String,
    /// Place pages: the event happened at this place itself (not a nested one).
    pub here: bool,
}

/// A document page.
#[derive(Debug, Clone, PartialEq)]
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Path.
    pub path: String,
    /// Has unsynced changes.
    pub pending_sync: bool,
    /// "Expires 1 Mar 2027".
    pub expires_label: Option<String>,
    /// Expires within 60 days (or expired).
    pub expiring_soon: bool,
    /// The open task that renews it (a task linking the document).
    pub renewal_task: Option<TaskItem>,
    /// Notes mentioning it, newest first.
    pub mentions: Vec<NoteListItem>,
    /// Copies with their location and holder.
    pub copy_briefs: Vec<DocumentBrief>,
    /// The user-owned `## Notes` body.
    pub user_notes: String,
    /// "Last with Shady · 20 Sep".
    pub holder_label: Option<String>,
}

/// A place page.
#[derive(Debug, Clone, PartialEq)]
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Every nested place, depth first (children by title).
    pub tree: Vec<PlaceNode>,
    /// Documents last stored here (or nested) that a person holds now ("Out with people").
    pub out_with_people: Vec<DocumentBrief>,
    /// The user-owned `## Notes` body.
    pub user_notes: String,
    /// Path.
    pub path: String,
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
#[derive(Debug, Clone, PartialEq)]
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Direction of the snippet.
    pub snippet_dir: TextDir,
    /// Matched terms in `snippet`.
    pub highlights: Vec<TextSpan>,
    /// Relevance (higher is better; mode-specific scale).
    pub score: f64,
}

/// Search results.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchView {
    /// Query.
    pub query: String,
    /// Mode used.
    pub mode: SearchMode,
    /// Results, best first.
    pub results: Vec<SearchHit>,
    /// Whether the asked mode could run (semantic/hybrid need the server).
    pub availability: Availability,
    /// Modes usable right now (keyword always; semantic/hybrid when online).
    pub available_modes: Vec<SearchMode>,
    /// Folder the search was limited to.
    pub folder: Option<String>,
}

/// A piece of an answer: text, or a citation marker at that position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskSpan {
    /// Text of the run (empty for a citation marker).
    pub text: String,
    /// 1-based citation index (`None` for text).
    pub citation: Option<u32>,
}

/// A note cited by an answer, with its cited blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskSource {
    /// Note.
    pub note_id: String,
    /// Title.
    pub title: String,
    /// Path.
    pub path: String,
    /// Block IDs cited in it.
    pub anchors: Vec<String>,
    /// Citation indexes pointing here.
    pub indexes: Vec<u32>,
}

/// What Ask searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskScopeKind {
    /// The whole vault.
    All,
    /// One entity and what mentions it.
    Entity,
    /// A folder.
    Folder,
}

/// A scope the user can pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskScope {
    /// Kind.
    pub kind: AskScopeKind,
    /// Entity ID or folder path (`None` for all).
    pub value: Option<String>,
    /// "All notes", "Acme", "notes/sales".
    pub label: String,
}

/// AI provider state and today's budget (Ask header, Settings → AI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiStatusView {
    /// AI is enabled for the account.
    pub enabled: bool,
    /// Provider name.
    pub provider: Option<String>,
    /// Paused until … ("Paused until 14:00").
    pub paused_label: Option<String>,
    /// Queued jobs.
    pub queue_depth: u32,
    /// Share of today's budget used (0–100).
    pub budget_used_percent: u32,
    /// "62% used".
    pub budget_label: String,
    /// Notes with current embeddings (0–100), when embeddings exist.
    pub embedding_percent: Option<u32>,
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
    /// Message ID (an answer's ID is the server's Ask ID).
    pub id: String,
    /// The answer is still streaming.
    pub streaming: bool,
    /// Text runs and citation markers in answer order.
    pub spans: Vec<AskSpan>,
    /// Cited notes, grouped.
    pub sources: Vec<AskSource>,
    /// "Scope: Acme".
    pub scope_label: String,
    /// Cited notes.
    pub source_count: u32,
    /// "14:05".
    pub created_label: String,
    /// Direction of `text`.
    pub dir: TextDir,
    /// Why the answer stopped early (`error.ai_paused`, `error.ai_unavailable`, `stopped`).
    pub error_key: Option<String>,
    /// Saved as this note (`save_answer_as_note`).
    pub saved_note_id: Option<String>,
}

/// Ask (online only, §12.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskView {
    /// Whether Ask can be used.
    pub availability: Availability,
    /// The conversation.
    pub messages: Vec<AskMessage>,
    /// Scopes to choose from (All notes, people and companies, top folders).
    pub scopes: Vec<AskScope>,
    /// An answer is streaming (show Stop).
    pub streaming: bool,
    /// AI status, once fetched.
    pub ai_status: Option<AiStatusView>,
}

/// The block a citation points to (source preview).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationPreview {
    /// Note (`None` when it does not resolve locally).
    pub note_id: Option<String>,
    /// Title.
    pub title: String,
    /// Path.
    pub path: String,
    /// The block's text (`None`: anchor not found).
    pub block_text: Option<String>,
    /// Direction of `block_text`.
    pub block_dir: TextDir,
    /// Heading above the block.
    pub heading: Option<String>,
    /// Note date ("12 Sep 2026", from `created`).
    pub date_label: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
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
    /// Direction of the title.
    pub title_dir: TextDir,
    /// Short AI summary (hover card), when the server has one.
    pub summary: Option<String>,
    /// Last change ("today", "21 Sep").
    pub updated_label: String,
    /// Label priority: 0 = always labelled (hubs), higher = only when zoomed in further.
    pub label_rank: u32,
    /// A hub (label always shown).
    pub is_hub: bool,
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
    /// Stable edge ID (`<src>|<kind>|<dst>`).
    pub id: String,
    /// Relation type for `relation:<type>` edges.
    pub rel_type: Option<String>,
    /// Localised label ("works at", "link", "mentions").
    pub label: String,
    /// The AI's reason, when it gave one.
    pub reason: Option<String>,
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
    /// Relations of the focused note ("8 relations · 2 by AI").
    pub relation_count: u32,
    /// Of which by the AI.
    pub ai_relation_count: u32,
    /// "8 relations · 2 by AI".
    pub relation_label: String,
    /// The focused note's summary.
    pub summary: Option<String>,
    /// Saving a layout as `.canvas` (the server has no endpoint for canvas files yet).
    pub save_layout: Availability,
    /// Proposing a relation type with AI on drag-to-relate (needs the AI endpoint).
    pub propose_relation: Availability,
}

/// A point of a region outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphPoint {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
}

/// Which graph the global map shows (PLAN §10 entity lens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphLens {
    /// Every note.
    Notes,
    /// People and their relations / co-mentions.
    People,
    /// Companies and their relations / co-mentions.
    Companies,
}

/// Global map filters, applied in the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFilter {
    /// Edge kinds to keep (`link`, `embed`, `relation`, `mention`, `concept`, `entity`,
    /// `custody`, `part-of-place`, `similarity`); empty = all.
    pub edge_kinds: Vec<String>,
    /// Node kinds to keep (`note`, `person`, …); empty = all.
    pub node_kinds: Vec<String>,
    /// Include AI similarity edges (needs the server's graph endpoint).
    pub similarity: bool,
    /// Keep only this cluster (and edges inside it).
    pub cluster: Option<String>,
    /// Lens.
    pub lens: GraphLens,
    /// Selected node: its neighbours are listed in `GlobalGraphView::neighbours`.
    pub focus: Option<String>,
}

/// A kind with its count (filter panel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCount {
    /// Kind.
    pub kind: String,
    /// Localised label.
    pub label: String,
    /// Count before filtering.
    pub count: u32,
}

/// A cluster region label.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterLabel {
    /// Cluster ID.
    pub id: String,
    /// Name (AI-named or user-named).
    pub name: String,
    /// Member count.
    pub size: u32,
    /// Label anchor (centroid of the members).
    pub x: f64,
    /// Label anchor.
    pub y: f64,
    /// Region outline (convex hull of the members, padded), clockwise.
    pub hull: Vec<GraphPoint>,
    /// Region radius around the anchor (for a disc when the hull has < 3 points).
    pub radius: f64,
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
    /// Filter applied.
    pub filter: GraphFilter,
    /// Edges per kind before filtering.
    pub edge_counts: Vec<KindCount>,
    /// Nodes per kind before filtering.
    pub node_counts: Vec<KindCount>,
    /// Neighbours of `filter.focus` (to keep bright), by ID.
    pub neighbours: Vec<String>,
    /// Similarity edges (server graph endpoint; not in the contract yet).
    pub similarity: Availability,
}

/// A node position of a mind-map layout (`save_layout`).
#[derive(Debug, Clone, PartialEq)]
pub struct NodePosition {
    /// Node ID.
    pub id: String,
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
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
    /// What it does ("+2 lines, 1 changed", "works at → Acme", the capture's text).
    pub detail: String,
    /// Direction of `detail`.
    pub detail_dir: TextDir,
    /// "14:32".
    pub created_label: String,
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
    /// "14:41".
    pub created_label: String,
}

/// One line of the sync log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncLogItem {
    /// When.
    pub at: DateTime<Utc>,
    /// "14:32:10".
    pub at_label: String,
    /// `synced` | `failed` | `bootstrap` | `paused` | `resumed` | `reset` | `account`.
    pub kind: String,
    /// "Pushed 3 · pulled 12", or the error key.
    pub detail: String,
}
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
    /// The user paused sync.
    pub paused: bool,
    /// Backoff: seconds between retries now ("every 30 s").
    pub retry_interval_secs: Option<u32>,
    /// Backoff: "next at 14:47:30".
    pub next_retry_label: Option<String>,
    /// "Retrying automatically every 30 s · next at 14:47:30".
    pub retry_label: Option<String>,
    /// Last cycles, newest first (≤ 50).
    pub log: Vec<SyncLogItem>,
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

/// How a line of one conflict column changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange {
    /// Same as the base.
    Same,
    /// Added on this side.
    Added,
    /// Removed on this side (base column: removed by either side).
    Removed,
    /// Changed on this side only.
    Changed,
    /// Changed on both sides (the conflict).
    ChangedBoth,
}

/// A line of a conflict column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotatedLine {
    /// 1-based line number in this column's text.
    pub line: u32,
    /// The line (no terminator).
    pub text: String,
    /// Its change.
    pub change: LineChange,
    /// Direction.
    pub dir: TextDir,
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
    /// Keep both versions as separate notes: the server's stays, the local edit is kept
    /// in the conflict copy the server created (or created now).
    SaveBothAsCopies,
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
    /// "Line 6", "Property “tags”", "Line endings".
    pub location_label: String,
    /// Choices valid for this hunk.
    pub allowed_choices: Vec<HunkChoiceKind>,
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
    /// Note path.
    pub path: String,
    /// "This device · today 14:41 · edited offline".
    pub local_origin_label: String,
    /// "Server · v8" (or "Server" until pulled).
    pub server_origin_label: String,
    /// The base, annotated.
    pub base_lines: Vec<AnnotatedLine>,
    /// The local version, annotated against the base.
    pub local_lines: Vec<AnnotatedLine>,
    /// The server version, annotated against the base.
    pub server_lines: Vec<AnnotatedLine>,
    /// The server already saved the local edit as a conflict copy (path).
    pub conflict_copy_path: Option<String>,
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
    /// Snooze length used by the notification's Snooze action.
    pub snooze_minutes: u32,
    /// Quiet hours on.
    pub quiet_enabled: bool,
    /// Quiet hours start (`HH:MM`).
    pub quiet_from: String,
    /// Quiet hours end (`HH:MM`); reminders inside are delivered then.
    pub quiet_until: String,
}

/// An integrity warning (Settings → Integrity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityItem {
    /// ID.
    pub id: String,
    /// Kind (`out_of_band_edit`, …).
    pub kind: String,
    /// Localisation key (`integrity.<kind>`).
    pub message_key: String,
    /// Vault path.
    pub path: Option<String>,
    /// "12 Sep 14:31".
    pub created_label: String,
}

/// Result of an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// Files imported.
    pub imported: u32,
    /// Entries skipped (hidden files, …).
    pub skipped: u32,
}

/// Result of an export saved to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    /// Where it was written.
    pub path: String,
    /// Size.
    pub size_bytes: u64,
    /// Notes (`.md` entries) in it.
    pub note_count: u32,
    /// "18.4 MB · 412 notes".
    pub label: String,
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
    /// Devices of the account, once fetched (`refresh_settings`), by last activity.
    pub device_list: Vec<DeviceItem>,
    /// AI status, once fetched.
    pub ai_status: Option<AiStatusView>,
    /// Integrity warnings, once fetched, newest first.
    pub integrity_warnings: Vec<IntegrityItem>,
    /// Last `refresh_settings` ("Updated 14:32").
    pub refreshed_label: Option<String>,
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
    /// Initials.
    pub initials: String,
    /// The signed-in admin's own row ("you"; no actions).
    pub is_self: bool,
    /// "Requested 2 hours ago" / "Joined 12 Sep 2026".
    pub created_label: String,
    /// "Deleted on 11 Oct 2026" when scheduled.
    pub deletion_label: Option<String>,
    /// Must change the password (after a reset).
    pub password_change_required: bool,
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
    /// Search text applied (username / display name, both scripts).
    pub query: String,
}

/// A new account created by an admin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUserRequest {
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Initial password.
    pub password: String,
    /// `admin` | `member`.
    pub role: String,
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
    /// Any other platform error.
    Failed,
}

/// A notification action tapped by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationActionKind {
    /// Mark the task done.
    Done,
    /// Remind again later.
    Snooze,
}

/// A notification action tapped by the user (Snooze uses the device's snooze length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationAction {
    /// Kind.
    pub kind: NotificationActionKind,
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

# Core gaps found by the Flutter UI

View-model fields and intents the Flutter screens need from the Rust core (PLAN L15: the UI renders what the core provides and never computes it). Each entry names what is missing and what the screen renders meanwhile.

## App shell, accounts, settings, admin, sync (Flutter pass: apps/strata, features/accounts|settings|admin|sync)

### Sync pill display state (`SyncPill`)
- **Needed:** `SyncPill.display: SyncPillKind { synced, offline, syncing, conflict, duplicates }` plus the values the chosen state shows: `progressDone`/`progressTotal` for "Syncing 12/40" and the local `lastSyncLabel` ("14:32" in the account's time zone). Today the pill would have to pick a priority between `connectivity`, `activity.phase`, `pendingOps`, `conflicts` and `duplicates` and compute done/total (`ops - pendingOps`), which is logic.
- **Rendered meanwhile:** tone/icon/copy 1:1 from `connectivity` with the queued count ("Offline · 3 queued", "Synced"), a conflict badge when `conflicts > 0`, a spinner when `activity.phase != idle`.

### Sidebar and rail counts
- **Needed:** one stream (or `HomeView` fields) with the navigation counts of SCREEN_SPEC: tasks due (sidebar "Tasks 3"), notes total, directory total, map cluster count. Only `HomeView.inboxCount` exists; the others are not shown.
- **Needed:** pinned notes for the sidebar "Pinned" section (`pinned: Vec<NoteListItem>` in `HomeView` or a `watch_pinned` stream) and a pin/unpin intent.

### Accounts
- **Pending approval as a session state:** `SessionKind::PendingApproval` (and `Rejected`) with the username and `requested_at`, plus a `check_approval()` intent. Today "Check again" re-sends the `SignInRequest` the UI keeps in memory, and "sent 2 hours ago" / "Last checked 14:32" cannot be rendered.
- **Change password:** `change_password(current, new)` intent (Settings → Account and `SessionKind::PasswordChangeRequired`). The screen renders the fields with the submit disabled.
- **UI language and time zone:** `set_ui_language(code)` and `set_timezone(iana)` intents (the values are shown read-only from `AccountSummary`).
- **Account disabled:** `export_unsynced(path)` intent ("Export them first") — disabled meanwhile.
- **Deletion pending:** `download_export(path)` (`GET /me/export`), `export_unsynced(path)` and `delete_account_now()` intents, plus `export_size_bytes`/`export_note_count` ("18.4 MB · 412 notes") on `SessionState`; the buttons are disabled meanwhile. `SessionState.deletionAt` should also come with the account-timezone local date (the UI formats `toLocal()`).
- **Account sheet:** the current device's name and sign-in date, the device count ("Devices 3") and the pending-approval count ("2 pending") — `SessionState.deviceName` is documented as the signed-out prefill only.
- **Avatar initials** ("SN" for Sara Nabil): `initials` on `AccountSummary`/`AdminUserItem`/`KnownAccountItem` (the UI shows a person icon meanwhile).
- **Sign-up:** password strength (`password_strength(pw) -> {level, length, min_length}`) and username availability (`check_username(server, name)`) for the SignupCompact meter and "@sara.n is available". Only the confirm-password equality is checked in the form.

### Settings
- **Devices:** `SettingsView.devices` is only an `Availability`. Needed: `devices: Vec<DeviceItem { id, name, platform, last_seen, is_this_device, reminders_enabled }>` and intents `rename_device(id, name)`, `revoke_device(id)`, `set_device_reminders(id, enabled)` (per-device toggle of ReminderNotifications "Deliver to").
- **Reminders:** quiet hours (`quiet_from`, `quiet_until`, enabled), snooze length and default time as settable values with intents (`set_default_reminder_time`, `set_quiet_hours`, `set_snooze_minutes`). Only `default_time` is shown today (read-only).
- **AI / integrity / export-import:** data and intents behind the `Availability` flags (AI status, thresholds, auto-file, budget "62% used"; integrity warnings; `export_vault(path)` / `import_files(paths)`). The sections render the availability state and disabled actions.

### Admin → Users
- **Intents:** `approve_user(id)`, `reject_user(id)`, `set_user_role(id, role)`, `disable_user(id)`, `enable_user(id)`, `reset_password(id) -> one_time_password`, `schedule_deletion(id) -> deletion_at`, `cancel_deletion(id)`, `create_user(...)`. All controls are rendered disabled; the one-time password and schedule-deletion dialogs exist and take the core's values.
- **Fields:** `AdminUserItem.device_count`, `last_active_at`, `is_self` ("you", no actions on your own row), `requested_from_device` ("Sara's iPad"), and `deletion_at` for the confirmation ("deleted on 11 Oct 2026") before scheduling. Search needs a `query` parameter on `load_admin_users` (no filtering in Dart).

### Sync status and conflicts
- **Outbox rows:** a per-op detail line (`"+2 lines, 1 changed"`, `"contradicts → Discount policy"`, the capture excerpt); only `kind`, `title` and `status` exist.
- **Retry schedule:** "Retrying automatically every 30 s · next at 14:47:30" — `retry_at` exists only inside `activity` during backoff; the interval is not exposed.
- **Pull progress:** "Then pulling 37 changes from 2 other devices" and "~20 s left" are not in `SyncActivity`.
- **Pause sync / sync log** (SyncMedium) — no intents.
- **Conflict screen:** per-line diff annotations for the three columns (`added` / `removed` / `changed on both sides` spans with line numbers), the versions' origins ("MacBook Pro · today 14:41 · edited offline", "v8, from phone"), the note path, and a readable hunk location ("Line 6" instead of `body:6`). The UI shows the raw texts and the `location` code.
- **Allowed hunk choices:** `ConflictHunkView.allowed_choices: Vec<HunkChoiceKind>` — the UI offers `oursThenTheirs` and `text` only when `kind == "body"`, which mirrors a sync-model rule.
- **Own text:** `HunkChoiceKind::Text` expects a trailing line terminator; the core should normalise the text it receives rather than the UI appending `\n`.
- **"Save both as copies"** (ConflictExpanded footer) — no `ResolutionKind` for it.

### Reminders adapter (§12.5b)
- **Snooze length:** `NotificationAction.minutes` must be chosen by Dart. Needed: the core decides (a `snooze_minutes` in `RemindersSetting`, or `NotificationAction` without minutes). The adapter sends a named interim constant (15, from the design copy).
- **Failure result:** `NotificationResult` has no generic `failed`; platform errors other than permission map to `platform_limit`.
- **Stream per session:** `watch_notification_ops` fails when no session is active and the provider is `keepAlive`; the adapter re-subscribes (invalidates) whenever the shell mounts. A session-independent stream (empty while signed out) would remove that.
- **Launch/tap context:** notification taps carry only the op ID and the task ID payload; if the core wants the Done action to address a task directly (`task_id`) the op should say which one to use.

## Notes and editor (Flutter pass: features/notes, features/editor)

### Save with the view's version
- **Needed:** `update_note(id, content, base_version)` — the editor saves the full markdown it built on `NoteView.version`; the core should compare it with the note's current base and answer with a conflict (or the D19 merge) instead of silently applying an edit made against a stale view. Today `updateNote(id, content)` has no version, so the UI sends `id` + full markdown only.
- **Needed:** a display version for the status line ("Saved · v7"): `NoteView.version_label` (or `version_number`) from the history the server keeps. `NoteView.version` is a content hash; the UI shows "Saved" / "Saved on this device · N changes to sync" / "Conflict" from `sync` only.
- **Needed:** a note-level duplicate state (`NoteSyncState.duplicate_op_id` or `NoteSyncKind::Duplicate`) so the note view can hand an "Already exists" prompt for *this* note to the duplicate sheet; `DuplicatePrompt` has no note ID, so the note view shows nothing.

### Editor hints (`EditorHint` / `editor_hints`)
- **Needed:** `EditorHint.target_id: Option<String>` (resolved note ID) and `target_anchor` on `WikiLink`/`Embed` spans, so tapping a wikilink opens the note. Today the editor forwards the link's source text (`[[Note|alias]]`) to `onOpenLink`; the host cannot resolve it without logic.
- **Needed:** `EditorHint.task_id` on `TaskLine` spans. The editor takes the ID from the `BlockId` span inside the task-line span (the span text minus `^`) and looks it up in `NoteView.tasks`.
- **Needed:** `Heading` spans with the level (`HintKind::Heading { level }` as a field, e.g. `EditorHint.level: u8`); all headings are styled alike today.
- **Needed:** per-line paragraph direction (`EditorHint` of kind `Direction { rtl }` per line, or `line_directions: Vec<bool>`), from the first strong character. `super_editor` only looks at the first non-space character (wrong for `- خصم`, `## الفرضيات`), so the editor applies the Unicode first-strong rule (P2) itself as a layout step.
- **Needed:** emphasis spans (`**bold**`, `_italic_`, `~~strike~~`, `==mark==`, inline code already exists) so the editor can style them; markers are shown as typed today.
- **Needed (`NoteListItem`, `NoteView`, `BacklinkGroup`):** a `lang`/direction hint for titles, snippets and backlink titles (the Arabic note list rows and the Arabic title of NoteExpandedDarkAr); rendered direction-neutral with `TextAlign.start` meanwhile.

### Editor completions
- **Needed:** `editor_completions(note_id, content, cursor) -> Completions { kind: wikilink | mention | tag | block_ref, replace_start, replace_end, items: [{ label, detail, insert_text, target_id, entity_kind }] }`. The editor detects the token before the caret itself (`[[query`, `@query`, `#query`, `[[Note#^`) and fills the list from `search(query, keyword)` (notes) and `watch_directory(people|companies, query)` (mentions).
- **Needed:** a tag list for `#` autocomplete (vault tags with counts, filtered by prefix). The tag panel shows "Tag suggestions aren't available yet."
- **Needed:** the block list of a note for the block reference picker (`[[Note#^`): `blocks(note_id) -> [{ block_id, text }]`. The picker shows "Block references can't be listed yet."
- **Needed:** `insert_mention(note_id, content, cursor_range, entity_id) -> { content, cursor }` that writes the link (path-disambiguated when titles collide, PLAN §6.3) and adds the entity to `people:`/`companies:` in one op. Meanwhile the editor replaces `@query` with `[[<title>]]` and calls `add_relation(src_id, dst_id, "people"|"companies")`; `[[` completion inserts `<title>]]`.

### Note view (properties, backlinks, history, list)
- **Needed:** history entries and revert: `NoteView.history` is only an `Availability`. Needed `history: Vec<HistoryEntry { version_label, message, author, at_label, can_revert }>` (online) and `revert_note(id, version)` + a diff view model. The panel renders the four availability states.
- **Needed:** backlink details: `BacklinkItem.snippet` (the linking sentence), `by`/`confidence` for AI relations, and a total `backlink_count` on `NoteView` (the "Backlinks 6" tab badge; the UI does not sum the groups).
- **Needed:** `NoteView.created_label` / `edited_label` / `edited_by` ("Created 18 Sep · edited today 14:31 by Shawket") and `word_count`; `NoteListItem.updated_label` ("14:31", "Sat", "21 Sep" — relative to today in the account's time zone). Not shown meanwhile.
- **Needed:** `NotesListView.breadcrumb: Vec<FolderItem>` (root → current) and the folder's own `note_count`; the list shows "Notes › notes/sales" with the root as the only link.
- **Needed:** folder-scoped filtering ("Filter notes in sales"): `search` has no folder parameter, so the list search is vault-wide.
- **Needed (`RelationChip`):** `created_label` and cited evidence (`citations: Vec<Citation>`) for the AI reason card ("confidence 0.72 · 14:05", `^a1b2` quotes), and an intent to add a relation from the Properties panel with an AI-proposed type ("+ Add relation"). The card shows confidence and reason only; add-relation is not offered.
- **Wiring note (maps feature):** the note's local mini-graph slot shows a placeholder with "Open map" (`NotesScreen.onOpenLocalMap(noteId)`); `strata_maps` exports no embeddable mini-graph widget over `watch_local_graph` yet. When it does, the slot (`LocalGraphSlot`) should host it.

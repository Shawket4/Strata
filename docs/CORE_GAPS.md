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

## Home, inbox, tasks, duplicate prompt (Flutter pass: features/home|inbox|tasks)

### Display labels in the user's time zone (all three screens)
- **Needed:** display-ready local labels next to every instant the UI shows: `NoteListItem.updated_label` ("2d", "1w"), `InboxItem.created_label` ("09:47", "Sat 18:40"), `SuggestionItem.created_label`, `ReminderItem.time_label` ("09:00") and `offset_label` ("on the day", "30 days before"). Converting UTC instants to the account's time zone or computing relative ages is logic.
- **Rendered meanwhile:** date-only values (`TaskItem.due/done`, UTC midnight) are formatted with gen-l10n date formats; `NoteListItem.updatedAt` is formatted as a UTC day ("27 Sep"); reminders show `ReminderItem.local` verbatim ("2026-10-01 09:00"); capture times are not shown.
- **Needed:** a language/direction hint for user content (`lang: "ar" | "en" | "mixed"` or a `TextDirection` per paragraph) on `InboxItem.text`, `NoteListItem.title/snippet`, `TaskItem.description`. The UI renders them direction-neutral (`TextAlign.start`, ambient direction).

### Home (`HomeView`)
- **Header:** today's date and the greeting ("Sunday 27 September · good afternoon, Shawket"): `today_label`, `greeting`, `display_name`. Not rendered.
- **Inbox preview:** the first captures with their proposal summary ("→ Weekly invoicing request — Acme", "Needs you · Who is “بابا”?"): `inbox_preview: Vec<InboxPreviewItem { note_id, text, summary, needs_you }>` and `needs_you_count` / `contradictions_count` ("1 needs you · 1 contradiction to review"). Only `inboxCount` is rendered (count card).
- **AI activity feed:** `ai_activity: Availability` + `Vec<AiActivityItem { at_label, kind (relation_added | contradiction | timeline | custody_applied), summary, source/target refs, rel_type, confidence, undo_suggestion_id }>` and the headline ("3 relations added, 1 contradiction found"). Rendered as "Not available yet".
- **Open items roll-up:** `open_items: Availability` + `Vec<OpenItem { id, text, person: EntityRef, citation, done }>` and a `complete_open_item(id)` intent. Rendered as "Not available yet".
- **Recent notes:** link count per note (`NoteListItem.link_count`, "4 links") and the Edited / Created / Filed-by-AI filters of HomeExpanded (`watch_recent(filter)`). Not rendered.
- **Today block count:** a single "due today incl. overdue" count (`tasks_today_count`, "Today 2") — the block shows the two sections without a summed count.

### Inbox (`InboxView`, `SuggestionItem`)
- **Capture-level intents:** `accept_capture(note_id)` / `reject_capture(note_id)` (and a bulk `accept_captures(ids)`), so the core decides what Accept means for a capture with several suggestions. Meanwhile Accept / Reject (card, bulk bar, A / R keys) forward `acceptSuggestion` / `rejectSuggestion` for every suggestion of the capture in the order the core lists them.
- **Edit the proposal:** `accept_suggestion_with(id, edits { title, folder, tags, relations })` for the editable title / folder / tags / relations of InboxExpanded. Meanwhile "Edit" opens the capture note (`onOpenNote(noteId)`).
- **Accept all ready / filters:** `ready_count` and `accept_all_ready()`; filter tabs All / Needs you / Conflicts need `needs_you_count`, `conflicts_count` and a `watch_inbox(filter)` parameter. Not rendered (filtering is logic).
- **Capture metadata:** `InboxItem.source` ("Typed on Pixel 8", "voice"), the AI filing confidence on the capture row, and `SuggestionItem.source_text` (the capture text behind a standalone suggestion such as "بابا عايز يشوف الأرقام بكرة").
- **Link-or-create:** `resolve_link_or_create(suggestion_id, choice: Link { entity_id } | Create { name })` that adds the mention as an alias. Meanwhile "Link to existing person…" is disabled ("not available yet") and "Create person…" calls `createEntity(kind: "person", name, aliases: [mention], force: false)` (duplicate candidates shown in place, Create anyway resends with `force: true`) and then `acceptSuggestion(id)` — please confirm this sequence or replace it with the single intent.
- **Timeline chip:** the resolved timeline date of a mention ("Timeline · Mon 28 Sep (from “بكرة”)", "Nov 2026 — rates +8% on both pages"): `SuggestionDetail.timeline: Option<{ date_label, source_phrase, targets }>`. Not rendered.
- **Custody:** an explicit `auto_applied: bool` (today the UI maps `status == "accepted"` to "Applied automatically"), `undo_suggestion(id)` (Undo currently sends `rejectSuggestion(id)`), `acknowledge_suggestion(id)` for "Looks right" (not rendered), the resulting location / holder / last-holder fields (`location: EntityRef`, `holder`, `last_holder`) and `accept_suggestion_choice(id, document_id)` for the ambiguous "Which contract?" choice (Accept is disabled while `candidates` is non-empty).
- **Duplicate-flagged captures:** confirm the mapping Create anyway → `rejectSuggestion(id)`, Discard → `acceptSuggestion(id)`, Open existing → navigation only; or add `resolve_duplicate_suggestion(id, DuplicateChoice)`. The candidate's kind for routing Open existing is `CandidateItem.kind` (the app shell routes by kind).
- **Suggestion threads (reply):** no view-model or intent exists (`thread: Vec<ThreadMessage>`, `reply_to_suggestion(id, text)`). Not rendered.

### Tasks (`TasksView`, `TaskScreen`, `TaskItem`)
- **Grouping and labels:** upcoming grouped by day with headers ("TUE 29 SEP", "LATER"), "3 days late", "next in 4 days", "Next occurrence in 4 days", history "on time" / "2 days late", "4 done this week", the open total ("Open · 5"), "6 notes contain tasks", reminder offset ("30 days before", "on the day"), "From document expiry": `TaskItem.due_label`, `lateness_label`, `next_in_label`, `origin_label`, `TaskSections.upcoming_groups: Vec<{ label, tasks }>`, `TasksView.open_count`, `done_this_week`, `notes_with_tasks`. Rendered meanwhile: flat sections with the formatted due date and an "Overdue" style for the overdue section.
- **Task line location:** `TaskScreen.line_number` ("tasks/Tasks.md · line 14") and the home note's path (`TaskItem.note_path`); the UI shows `noteTitle`.
- **Reminders editing:** `add_reminder(task_id, local)` / `remove_reminder(task_id, local)` (or `ReminderItem.local` as a `DateTime` so `TaskPatch.reminders` can be rebuilt without parsing a string). "Add reminder" in task detail is disabled meanwhile; the new-task sheet sends picked `DateTime`s in `TaskDraft.reminders`. Delivery devices ("to Pixel 9, MacBook Pro") are not rendered.
- **Recurrence editor:** a structured rule and its compiler: `recurrence_form(phrase) -> RecurrenceForm { frequency, interval, by_month_day | nth_weekday | last_day, ends: Never | On(date) | After(n) }`, `compose_recurrence(form) -> { phrase, understood }` and the preview `recurrence_preview(phrase_or_form, from) -> Vec<{ date_label }>` ("Thu 1 Oct 2026 · due, Sun 1 Nov, Tue 1 Dec"). Meanwhile the editor saves the typed phrase verbatim (`updateTask(patch: TaskPatch(recurrence: phrase))`, "Stop repeating" sends `clearRecurrence: true`); the frequency / interval / on / ends controls are shown disabled and the preview says "not available yet".
- **New task parsing:** `parse_task_text(text) -> TaskDraftPreview { description, due, due_label, recurrence, reminders, links: Vec<EntityRef>, chips }` for the "Understood as" chips of TaskEditCompact. Meanwhile the sheet sends the text as `TaskDraft.description` with the due date / repeat phrase / reminders the user sets by hand.
- **Home note picker:** a list of candidate home notes (`watch_task_homes()`) for the new-task sheet; it uses the caller's `noteId` or the core default (`noteId: null`).

### Duplicate prompt (`DuplicatePrompt`, `CandidateItem`)
- **Candidate details:** `CandidateItem.path` ("tasks/Tasks.md") and a match reason ("A recurring task already covers …") are not in the view-model; the sheet shows kind, snippet, match level and score.

## Directory, entity pages, documents & places, maps, Ask & search (Flutter pass: features/directory|documents|maps|ask)

### Directory (`DirectoryView` / `DirectoryItem`)
- **Row fields:** `initials` (avatars "AS"; a kind glyph is shown meanwhile), `mention_count` ("14 mentions"), `last_active` (+ its label "Today" / "Thu 24 Sep"), `role` and `company: EntityRef` as separate fields (today one `subtitle` string, so the company can't be a link or carry its kind dot), `tags`. Documents rows: `status`, `location: Vec<EntityRef>` (breadcrumb), `holder` / `last_holder` with the date ("Last with Shady · 20 Sep"), `copy`, `expires` and an `expiring_soon` flag (the "Expiring" filter badge). Places rows: parent breadcrumb as refs. Only `subtitle` is rendered today.
- **Sort and filters:** `watch_directory(tab, query, filter: DirectoryFilter { tags, role, company, industry, doc_type, status, place, holder, expiring, has_open_items }, sort: DirectorySort { name, last_active, recently_moved })` plus the filter options with counts (`DirectoryView.filter_options`). Filter chips are rendered disabled ("Filters aren't available yet"); rows keep the core's order.
- **Sections:** "Recently active" vs "All people · A–Z" (PeopleCompact) needs the core to split or label the rows.
- **Directory suggestions:** a `DirectoryView.suggestions` (or a `watch_directory_suggestions`) with only the entity suggestions for the tab (who-is, merge proposals). The strip shows `InboxView.suggestions` as-is (every kind rendered, none filtered).
- **Suggestion answers with a choice:** link-or-create needs `accept_suggestion_with(id, SuggestionChoice { link_to: id | create })`; merge proposals need the merge target. Only plain accept / reject are wired.
- **Create documents and places:** `create_entity` accepts `person | company | concept`; the "Add document" / "Place inside" actions need `document` / `place` kinds (with `part-of` / initial location). Not rendered for those tabs.

### Entity page (`EntityView`)
- **Merge:** `merge_entities(source_id, into_id)` (+ preview of what moves: aliases, mentions, relations). "Merge…" is disabled.
- **Repoint an AI link (D13):** `repoint_relation(src, dst, rel_type, new_dst)` (and for mentions `repoint_mention(note_id, mention_block, new_entity)`). The Repoint button is disabled; Reject uses `remove_relation` — a dedicated `reject_relation` that records the rejection (§6.5, "never re-proposed") and returns an undo token would match D13 better.
- **User notes section:** `EntityView.user_notes: String` (the `## Notes` body) and `update_user_notes(id, text)` (the AI never edits that section; the UI must not splice markdown). The same for `DocumentView`/`PlaceView`. A placeholder is shown.
- **Summary citations and freshness:** `summary_citations: Vec<Citation>` and `ai_updated_at` ("AI-maintained · updated 2h ago"); open-item state (`done: bool`) and a `complete_open_item` intent for the checkboxes; counts for "2 open · 1 done".
- **Header facts:** `last_active` ("last active today"), `mention_count` when `mentions` is truncated, `tags`, `path` (breadcrumb "people/ahmed-samir.md"), property provenance ("Phone · added by you") and `add_property` / `add_alias` intents.
- **Mentions:** `NoteListItem.highlight: Vec<(start, end)>` spans for the matched mention in the snippet (the design marks "أحمد"), and `lang`/direction per snippet.
- **Entity relation label:** `RelationChip.rel_label` (localisable "works at" for `works-at`); the raw type is shown.

### Documents and places
- **Record a move:** `record_custody(document_id, CustodyDraft { kind, place_id?, person_id?, counterparty_id?, date, note })` → op id. The form (event type, place picker with nested places, holder, date, note) is complete; submit is disabled with a notice. The place picker also needs each place's full breadcrumb (`DirectoryItem.breadcrumb`) and the current location flag.
- **AI custody entries:** `CustodyItem.by` (`user` | `ai`), `confidence`, `decision_id` and `undo_ai_decision(decision_id)` for "AI · 0.93 · Undo". Not rendered.
- **Custody sentence:** a localisation-ready form per event (`kind` + roles is rendered as "date · type" and linked refs; "Shady returned it to Safe" needs the core to say which ref is the actor vs the destination for each kind).
- **Expiry and renewal:** `DocumentView.renewal_task: Option<TaskItem>` (the "Renew · task, remind 1 Mar" link) and `expiring_soon`. "Renewal: Not available yet" is shown.
- **Document mentions and counts:** `DocumentView.mentions: Vec<NoteListItem>` (context panel "Mentioning notes"), `path`, `pending_sync`; copy rows with their `location`/`holder` (today `copies: Vec<EntityRef>`).
- **Place page:** nested tree deeper than one level (`sub_places` with their own children and `document_count`), each document's `holder` / `last_holder` / `doc_type` in `DocumentBrief`, "Out with people" (documents that belong here but are held now), and whether a movement row is "here" vs a nested place. Only direct sub-places are shown.

### Maps
- **Global map filters and lens in the core:** `global_graph(filter: GraphFilter { edge_kinds, node_kinds, similarity, cluster, lens: notes|people|companies })` with per-kind counts for the filter panel (`GlobalGraphView.edge_counts`, `node_counts`). Meanwhile edge/node filters and cluster focus are applied as paint-time layer visibility and dimming of the core's data; the lens (People / Companies) and the AI-similarity toggle are disabled; no counts.
- **Cluster regions and labels:** `ClusterLabel.{x, y}` (label anchor) and region geometry (`hull: Vec<(f32, f32)>` or centre + radii) from `graph-algo`. Regions are drawn as the union of soft discs around clustered nodes; region labels are not drawn (cluster names are in the filters panel).
- **Label priority:** `GraphNode.label_rank` (or `is_hub`) so zoom-dependent labels don't depend on a Dart threshold (the painter uses the GraphLanguage rule "degree ≥ 6" as a design constant).
- **Hover card:** `GraphNode.summary` (PLAN §6: node payload includes a short summary) and `updated`.
- **Edges:** `GraphEdge.reason` (the edge sheet shows "The AI's reason … isn't shown here yet"), `GraphEdge.rel_type` as its own field (the UI maps the full `kind` string 1:1), and edge IDs.
- **Mind map:** `save_layout(center_id, positions)` → `.canvas` (button disabled), `propose_relation(src, dst)` for drag-to-relate (AI proposes the type; only the hint is shown), a selected node's `summary`, and a relation count split ("8 relations · 2 by AI").
- **Neighbourhood for highlighting:** the global map uses `watch_local_graph(id, 1)` to know which nodes to keep bright on selection; a `neighbours` list on the selection would avoid a second stream.

### Ask and search
- **Asking:** `ask(question, scope) -> conversation id` with a streaming view (`AskMessage.streaming: bool`, partial text updates), `stop_ask()`, `new_conversation()`, and `AskView.scopes: Vec<AskScope>` (All notes / an entity / a folder). The composer and "New conversation" are disabled; only "All notes" is shown.
- **Inline citations:** `AskMessage.spans` (text runs and citation positions) so chips sit where the answer cites them; today citations follow the paragraph. Sources grouped by note ("^a1b2 ^c4d7" on one row) need `sources: Vec<{note_id, title, anchors}>`.
- **Save as note:** `save_answer_as_note(message_id)` (the core writes the note with citations as links, §9.5). Disabled.
- **Answer metadata:** `AskMessage.{scope, source_count, created_at}` ("Scope: Acme · 3 sources · 14:05"), AI provider status and daily budget (`AskView.ai_status`, `budget_used`).
- **Source preview:** the cited block's text (`resolve_citation(note_id, anchor) -> {block_text, heading, path, date}`); the preview shows title, path and tags only.
- **Per-paragraph direction:** a `lang`/direction hint on `AskMessage`, `SearchHit.snippet`, `NoteListItem.snippet`, `CitedBullet.text` and graph node titles (PLAN §11: direction by first strong character) — today they use the ambient direction with `TextAlign.start`.
- **Search:** `SearchHit.highlights` (match spans), `score`, and `SearchView.available_modes` (so unavailable modes can be disabled before searching; today availability is only known after the search runs).

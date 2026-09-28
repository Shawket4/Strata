# Core gaps found by the Flutter UI

View-model fields and intents the Flutter screens need from the Rust core (PLAN L15: the UI renders what the core provides and never computes it). Each entry names what was missing and its status after the core-gaps pass: **✅ resolved** (what the core now provides; the facade function or field to use) or **⏳ open** (why, and what the screen keeps rendering). Resolved entries are ready for the feature UIs to adopt; the pass itself changed only the call sites needed to keep the workspace compiling, plus the maps filtering (moved to the core).

## App shell, accounts, settings, admin, sync (Flutter pass: apps/strata, features/accounts|settings|admin|sync)

### Sync pill display state (`SyncPill`)
- **Display state.** ✅ resolved: `SyncPill.display: SyncPillKind { synced, offline, syncing, conflict, duplicates, paused, error }` (the core picks the priority), `progress_done` / `progress_total` ("Syncing 12/40"), `last_sync_label` (account time zone) and a ready `label`.

### Sidebar and rail counts
- **Navigation counts.** ✅ resolved: `watch_nav() -> NavView { inbox_count, tasks_due_count, notes_count, directory_count, cluster_count, pinned }`.
- **Pinned notes.** ✅ resolved: `NavView.pinned` / `HomeView.pinned` and `pin_note(id, pinned)`.

### Accounts
- **Pending approval as a session state.** ✅ resolved: `SessionKind::PendingApproval` / `Rejected` with `SessionState.pending: PendingApproval { username, server_url, requested_at, requested_label, last_checked_at, last_checked_label, can_check }`, `check_approval()` (re-signs in with the request kept in memory only) and `dismiss_pending()`. The router routes both kinds to the approval screen.
- **Change password.** ✅ resolved: `change_password(current, new) -> SessionState` (also leaves `PasswordChangeRequired`).
- **UI language and time zone.** ✅ resolved: `set_ui_language(code)`, `set_timezone(iana)`, `set_display_name(name)` (`PATCH /me`); every label is rebuilt in the new zone/language.
- **Account disabled.** ✅ resolved: `export_unsynced(path) -> count` (the unsynced ops as a Markdown file).
- **Deletion pending.** ✅ resolved: `download_export(path) -> ExportSummary` (`GET /me/export`), `export_unsynced(path)`, `delete_account_now(force)` (`POST /me/delete/confirm`), `SessionState.export_size_bytes` / `export_note_count` / `export_label` ("18.4 MB · 412 notes") and `deletion_label` (the account-zone local date).
- **Account sheet.** ✅ resolved: `SessionState.this_device: DeviceItem`, `device_count`, `pending_approvals` (admins).
- **Avatar initials.** ✅ resolved: `initials` on `AccountSummary`, `KnownAccountItem`, `AdminUserItem`, `DirectoryItem`, `EntityView`.
- **Sign-up.** ✅ resolved: `password_strength(password) -> PasswordStrength { level, length, min_length }`. ⏳ open: username availability — the API has no availability endpoint (usernames are checked only on `POST /auth/signup`, and exposing a lookup would allow account enumeration); the form keeps showing the server's answer after submit.

### Settings
- **Devices.** ✅ resolved: `SettingsView.device_list: Vec<DeviceItem { id, name, platform, last_seen(+label), signed_in(+label), is_this_device, reminders_enabled }>`, `refresh_settings()`, `rename_device(id, name)`, `revoke_device(id)`, `set_device_reminders(id, enabled)`.
- **Reminders.** ✅ resolved: `RemindersSetting.{snooze_minutes, quiet_enabled, quiet_from, quiet_until}` with `set_default_reminder_time`, `set_quiet_hours`, `set_snooze_minutes`.
- **AI / integrity / export-import.** ✅ resolved: `SettingsView.ai_status: AiStatusView { enabled, provider, paused_label, queue_depth, budget_used_percent, budget_label, embedding_percent }`, `integrity_warnings: Vec<IntegrityItem>`, `export_vault(path) -> ExportSummary`, `import_vault(path) -> ImportSummary` (a zip of Markdown, `POST /vault/import`). ⏳ open: editable AI thresholds / auto-file toggles — server-side settings with no endpoint yet (read-only in `AiStatusView`).

### Admin → Users
- **Intents.** ✅ resolved: `approve_user`, `reject_user`, `set_user_role`, `set_user_enabled` (disable/enable), `reset_password -> one_time_password`, `schedule_deletion`, `cancel_deletion`, `create_user(NewUserRequest)`; each returns the updated `AdminUserItem`.
- **Fields.** ✅ resolved: `is_self`, `created_label`, `deletion_at` + `deletion_label`, `password_change_required`, `initials`; search via `load_admin_users(query)` (filtered in the core). ⏳ open: `device_count`, `last_active_at`, `requested_from_device` — not in the admin users API (`AdminUser` has no device or activity data); the rows omit them.

### Sync status and conflicts
- **Outbox rows.** ✅ resolved: `OutboxItem.detail` ("+2 lines, 1 changed", "contradicts → Discount policy", the capture excerpt), `detail_dir`, `created_label`.
- **Retry schedule.** ✅ resolved: `SyncStatusView.retry_interval_secs`, `next_retry_label`, `retry_label` ("Retrying automatically every 30 s · next at 14:47:30").
- **Pull progress.** ✅ resolved: `SyncActivity.ops_done`, `ops_total`, `pulled`. ⏳ open: "from 2 other devices" and "~20 s left" — the changes feed carries neither the originating device nor a size estimate.
- **Pause sync / sync log.** ✅ resolved: `set_sync_paused(paused)` (`SyncStatusView.paused`, pill `paused`) and `SyncStatusView.log: Vec<SyncLogItem>`.
- **Conflict screen.** ✅ resolved: per-line annotations `ConflictDetail.{base_lines, local_lines, server_lines}: Vec<AnnotatedLine { number, text, change: added | removed | changed | same }>`, `local_origin_label` / `server_origin_label`, `path`, `ConflictHunkView.location_label` ("Line 6"), `conflict_copy_path`.
- **Allowed hunk choices.** ✅ resolved: `ConflictHunkView.allowed_choices` (the sync-model rule lives in the core).
- **Own text.** ✅ resolved: the core adds a missing final line terminator to `HunkChoiceKind::Text`.
- **"Save both as copies".** ✅ resolved: `ResolutionKind::SaveBothAsCopies` (keeps the server version and stores the local text as a conflict copy note).

### Reminders adapter (§12.5b)
- **Snooze length.** ✅ resolved: `NotificationAction` has no `minutes`; the core uses `RemindersSetting.snooze_minutes`. The adapter's interim constant is removed.
- **Failure result.** ✅ resolved: `NotificationResult::Failed`.
- **Stream per session.** ✅ resolved: `watch_notification_ops` is session-independent (empty while signed out; the notify hub survives account switches).
- **Launch/tap context.** ✅ resolved: `NotificationOp.task_id` carries the task the Done/Snooze action addresses.

## Notes and editor (Flutter pass: features/notes, features/editor)

### Save with the view's version
- **Base version.** ✅ resolved: `update_note(id, content, base_version)`; a stale base is 3-way merged (D19) or refused with `stale_edit`. The editor now sends `NoteView.content_version` of the text it loaded.
- **Display version.** ✅ resolved: `NoteView.version_label` ("v7", from the server history) and `content_version`; `NoteSyncState.label` is the ready status line.
- **Note-level duplicate state.** ✅ resolved: `NoteSyncKind::Duplicate` + `NoteSyncState.duplicate_op_id`.

### Editor hints (`EditorHint` / `editor_hints`)
- **Link targets.** ✅ resolved: `EditorHint.target_id` / `target_anchor` on `WikiLink` / `Embed`.
- **Task IDs.** ✅ resolved: `EditorHint.task_id` on `TaskLine`.
- **Heading levels.** ✅ resolved: `EditorHint.level`.
- **Line direction.** ✅ resolved: `HintKind::RtlLine` / `LtrLine` spans per line (Unicode P2 first-strong rule).
- **Emphasis.** ✅ resolved: `HintKind::Bold`, `Italic`, `Strike`, `Mark` (styled by the editor).
- **Title/snippet direction.** ✅ resolved: `title_dir` / `snippet_dir` on `NoteListItem`, `NoteView`, `BacklinkItem` (`TextDir { ltr, rtl, neutral }`).

### Editor completions
- **Completions.** ✅ resolved: `editor_completions(note_id, content, cursor) -> Completions { kind: wiki_link | mention | tag | block_ref | none, replace_start, replace_end, query, items: [CompletionItem { label, detail, insert_text, target_id, entity_kind, label_dir }] }` (UTF-16 offsets).
- **Tags.** ✅ resolved: `tags(prefix) -> Vec<TagItem { tag, count }>`.
- **Blocks.** ✅ resolved: `note_blocks(note_id) -> Vec<BlockItem>`.
- **Mentions.** ✅ resolved: `insert_mention(content, start, end, entity_id) -> MentionEdit { content, cursor }` (path-disambiguated link + `people:` / `companies:` in one content change for `update_note`).

### Note view (properties, backlinks, history, list)
- **History and revert.** ✅ resolved: `refresh_history(note_id)` fills `NoteView.history_entries: Vec<HistoryEntry { commit, version_label, message, author, at_label, can_revert }>`; `note_revision_diff(note_id, commit) -> NoteDiffView` and `revert_note(note_id, commit)` (online).
- **Backlink details.** ✅ resolved: `BacklinkItem.{snippet, snippet_dir, by, confidence}`, `BacklinkGroup.label`, `NoteView.backlink_count`.
- **Created/edited labels.** ✅ resolved: `NoteView.{created_label, edited_label, edited_by, word_count}`, `NoteListItem.updated_label`.
- **Breadcrumb.** ✅ resolved: `NotesListView.breadcrumb` and `note_count`.
- **Folder-scoped filtering.** ✅ resolved: `search_in_folder(query, mode, folder)`.
- **Relation chips.** ✅ resolved: `RelationChip.{rel_label, created_label, citations, decision_id}`; `relation_types()` lists the addable types with labels. ⏳ open: "+ Add relation" with an **AI-proposed type** — no AI endpoint proposes a type for a user-chosen pair (`LocalGraphView.propose_relation` stays `NotYetAvailable`); the user picks the type from `relation_types()`.
- **Mini-graph slot (maps feature).** ⏳ open (UI wiring only): `strata_maps` exports `MiniGraph`; hosting it in `LocalGraphSlot` is a feature-UI change outside this pass.

## Home, inbox, tasks, duplicate prompt (Flutter pass: features/home|inbox|tasks)

### Display labels in the user's time zone (all three screens)
- **Local labels.** ✅ resolved: `NoteListItem.updated_label`, `InboxItem.created_label`, `SuggestionItem.created_label`, `ReminderItem.{local_at, time_label, offset_label}`, task labels (below) — all in the account's zone and UI language (English/Arabic, CLDR plurals), DST-correct (Africa/Cairo tested across the October switch).
- **Content direction.** ✅ resolved: `InboxItem.text_dir`, `NoteListItem.title_dir/snippet_dir`, `TaskItem.description_dir`, `SuggestionItem.source_dir`.

### Home (`HomeView`)
- **Header.** ✅ resolved: `today_label`, `greeting`, `display_name`.
- **Inbox preview.** ✅ resolved: `inbox_preview: Vec<InboxPreviewItem { note_id, text, text_dir, summary, needs_you }>`, `needs_you_count`, `contradictions_count`, `inbox_summary`.
- **AI activity feed.** ✅ resolved: `ai_activity` + `ai_activity_items: Vec<AiActivityItem { at_label, kind, summary, source, target, rel_type, confidence, undo_suggestion_id, decision_id, reverted }>` and `ai_activity_headline`, from the server's AI decisions (`refresh_ai_activity()`); undo/correct with `reject_ai_decision`, `repoint_ai_decision`, `retype_ai_decision` (D13).
- **Open items roll-up.** ✅ resolved: `open_items` + `open_item_list: Vec<OpenItem { id, text, text_dir, person, citation, done }>` from the entities' `## Open items`. ⏳ open: `complete_open_item(id)` — open items are AI-maintained bullets without a done marker in the vault format (PLAN §6); a done state needs a vault-format decision (the checkbox stays read-only, `done` is always false).
- **Recent notes.** ✅ resolved: `NoteListItem.link_count` and `watch_recent(RecentFilter { edited, created, filed_by_ai })`.
- **Today count.** ✅ resolved: `TaskSections.today_count`.

### Inbox (`InboxView`, `SuggestionItem`)
- **Shared payload schema.** ✅ resolved: suggestion payloads are decoded with `sync_model::SuggestionPayload` (kinds `duplicate`, `duplicates`, `filing`, `entity_link`, `custody`, `task`, `correction`, `conflict`); an unknown kind, or a payload that does not match its kind's schema, is kept as `SuggestionKind::Unsupported` with its `server_kind`. `SuggestionKind` follows the shared kinds (`EntityLink`, `Correction`; the guessed `EntityLinkOrCreate` and `Relation` are gone). Tasks show `title`, the Tasks `line`, `date_label`, `recurrence`, `entities`; corrections show the user's words (`title`), the model's `question` and the first fix (`rel_type`, `target`, `reason`, `confidence`).
- **Capture-level intents.** ✅ resolved: `accept_capture(note_id)`, `reject_capture(note_id)`, `accept_captures(ids)`.
- **Edit the proposal.** ✅ resolved: `accept_suggestion_with(id, SuggestionEdits { title, folder, tags, text, due, recurrence, reminders, target_id, aliases })` → `suggestion.accept` with edits. Forced relink: `request_relink(note_id)` (`relink.request`).
- **Accept all ready / filters.** ✅ resolved: `accept_all_ready()`, `watch_inbox_filtered(InboxFilter { all, needs_you, conflicts })`, `InboxView.{ready_count, needs_you_count, conflicts_count, all_count}`, `InboxItem.{ready, needs_you, is_duplicate}`.
- **Capture metadata.** ✅ resolved: `InboxItem.source_label`, `SuggestionItem.source_text` (the filing confidence is not part of the shared `filing` payload, so it is not shown). ⏳ open: the capturing device ("Typed on Pixel 8") — the sync record of a capture carries no device; only a `source:` property is shown.
- **Link-or-create.** ✅ resolved: `resolve_link_or_create(suggestion_id, LinkOrCreateChoice { kind: link | create, entity_id, name, entity_kind, force })` (accepts with `target_id` + the mention as an alias; a create uses the payload's entity kind unless overridden). The detail comes from the shared `entity_link` payload: `SuggestionKind::EntityLink` with `mention`, `entity_kind`, `target` (proposed), `candidates`, `is_nickname`, `confidence`, `reason`, `decision_id`.
- **Timeline chip.** ⏳ open: `SuggestionDetail.timeline: Option<TimelineChip>` exists, but the server's suggestion payloads carry no resolved timeline date yet, so it is always `None`.
- **Custody.** ✅ resolved: from the shared `custody` payload — `SuggestionDetail.{document, line (the custody line it would write), date, date_label, location, holder, last_holder (the state it results in, by vault-format's rules), document_choices, quote, confidence, reason, decision_id}`, `accept_suggestion_choice(id, document_id)`, `undo_suggestion(id)`, `acknowledge_suggestion(id)`. Custody suggestions are the events the AI did *not* apply; applied ones are AI decisions (activity feed, `reject_ai_decision`), so `SuggestionItem.auto_applied` is always false.
- **Duplicate-flagged captures.** ✅ resolved: `resolve_capture_duplicate(suggestion_id, DuplicateChoice)` (Create anyway / Discard; Open existing is navigation by `CandidateItem.kind`).
- **Suggestion threads.** ✅ resolved: `SuggestionItem.thread: Vec<ThreadMessage>` (author `user` / `ai` from the record's `ReplyAuthor`) and `reply_to_suggestion(id, text)`.

### Tasks (`TasksView`, `TaskScreen`, `TaskItem`)
- **Grouping and labels.** ✅ resolved: `TaskItem.{due_label, lateness_label, completion_label, next_in_label, origin_label, is_overdue}`, `TaskSections.upcoming_groups: Vec<TaskGroup { label, tasks }>`, `TasksView.{open_count, done_this_week, done_this_week_label, notes_with_tasks}`, `TaskScreen.next_occurrence_label`.
- **Task line location.** ✅ resolved: `TaskItem.{note_path, line_number}`, `TaskScreen.location_label`.
- **Reminders editing.** ✅ resolved: `add_reminder(task_id, local)`, `remove_reminder(task_id, local)`, `ReminderItem.local_at`; `TaskScreen.delivery_label` ("to Pixel 9, MacBook Pro").
- **Recurrence editor.** ✅ resolved: `recurrence_form(phrase) -> Option<RecurrenceForm>`, `compose_recurrence(form) -> RecurrenceCompose { phrase, understood }`, `recurrence_preview(phrase, from, count) -> Vec<RecurrencePreviewItem>`, `TaskScreen.recurrence_form` / `recurrence_preview`; the phrase grammar is shared with `vault-format` (`RecurrenceRule::to_phrase`, L16). ⏳ open: an "ends" rule (on date / after n) — the Tasks recurrence grammar has no end clause; adding one is a vault-format decision.
- **New task parsing.** ✅ resolved: `parse_task_text(text) -> TaskDraftPreview { description, due, due_label, recurrence, reminders, priority, links, chips, draft }` (English + Arabic phrases, `@mentions` resolved to links).
- **Home note picker.** ✅ resolved: `watch_task_homes() -> TaskHomesView`.

### Duplicate prompt (`DuplicatePrompt`, `CandidateItem`)
- **Candidate details.** ✅ resolved: `CandidateItem.path` and `reason`.

## Directory, entity pages, documents & places, maps, Ask & search (Flutter pass: features/directory|documents|maps|ask)

### Directory (`DirectoryView` / `DirectoryItem`)
- **Row fields.** ✅ resolved: `initials`, `mention_count`, `last_active` + `last_active_label`, `role`, `company: EntityRef`, `industry`, `tags`, `status`, `doc_type`, `location` (breadcrumb refs), `holder`, `last_holder`, `holder_label`, `copy`, `expires` + `expires_label`, `expiring_soon`, `breadcrumb`, `document_count`, `has_open_items`, `title_dir`.
- **Sort and filters.** ✅ resolved: `watch_directory_filtered(tab, query, DirectoryFilter, DirectorySort { name, last_active, recently_moved })` with `DirectoryView.filter_options: Vec<FilterOption { facet, value, label, count, selected }>` and `expiring_count`.
- **Sections.** ✅ resolved: `DirectoryView.sections: Vec<DirectorySection { label, items }>`.
- **Directory suggestions.** ✅ resolved: `DirectoryView.suggestions` (entity suggestions of the tab only).
- **Suggestion answers with a choice.** ✅ resolved: `resolve_link_or_create`, `accept_suggestion_choice`, `merge_entities`.
- **Create documents and places.** ✅ resolved: `create_document(DocumentDraft, force)`, `create_place(PlaceDraft, force)`.

### Entity page (`EntityView`)
- **Merge.** ✅ resolved: `merge_preview(source_id, into_id) -> MergePreview` and `merge_entities(source_id, into_id)`.
- **Repoint an AI link (D13).** ✅ resolved: `repoint_relation(src, dst, rel_type, new_dst)`, `reject_relation(src, dst, rel_type)` (recorded as rejected, never re-proposed), and for server-side AI decisions (mentions included) `repoint_ai_decision(decision_id, target_id, hint)` / `reject_ai_decision`.
- **User notes section.** ✅ resolved: `EntityView.user_notes` / `DocumentView.user_notes` / `PlaceView.user_notes` and `update_user_notes(id, text)` (the core edits only `## Notes`).
- **Summary citations and freshness.** ✅ resolved: `summary_citations`, `summary_dir`, `open_count` / `done_count`. ⏳ open: `ai_updated_label` stays `None` — the synced note record has no "AI section updated at" time (the server keeps it in job state only); `complete_open_item` as under Home.
- **Header facts.** ✅ resolved: `last_active_label`, `mention_count`, `tags`, `path`, `set_property` / `remove_property`, `add_alias` / `remove_alias`. ⏳ open: property provenance ("added by you") — frontmatter keeps no per-property author.
- **Mentions.** ✅ resolved: `NoteListItem.highlights: Vec<HighlightSpan>` (UTF-16) and `snippet_dir`.
- **Entity relation label.** ✅ resolved: `RelationChip.rel_label`.

### Documents and places
- **Record a move.** ✅ resolved: `record_custody(document_id, CustodyDraft)`; the picker uses `place_options(document_id) -> Vec<PlaceOption { id, title, breadcrumb, depth, is_current }>`.
- **AI custody entries.** ✅ resolved: `CustodyItem.{by, confidence}`; undo through the inbox (`undo_suggestion`) or `reject_ai_decision`. ⏳ open: `CustodyItem.decision_id` — a custody line in the vault does not reference the server's AI decision, so the page cannot link a line to its decision (`None`).
- **Custody sentence.** ✅ resolved: `CustodyItem.{sentence, sentence_key, actor, destination, date_label, here}`.
- **Expiry and renewal.** ✅ resolved: `DocumentView.renewal_task`, `expiring_soon`, `expires_label`.
- **Document mentions and counts.** ✅ resolved: `DocumentView.{mentions, path, pending_sync, copy_briefs, holder_label}`.
- **Place page.** ✅ resolved: `PlaceView.tree: Vec<PlaceNode>` (every depth, with `document_count`), `DocumentBrief.{holder, last_holder, doc_type, location_path, expiring_soon}`, `out_with_people`, `CustodyItem.here`.

### Maps
- **Global map filters and lens in the core.** ✅ resolved: `global_graph_filtered(GraphFilter { edge_kinds, node_kinds, similarity, cluster, lens: notes | people | companies, focus, include_tags })` with `edge_counts` / `node_counts` (before filtering) and `neighbours`; edge kinds match a family (`relation`) or a full kind (`relation:supports`). The global map now sends its filter choices to the core and the painter only paints (the Dart visibility filtering is removed); the mind map's edge toggles use `watch_local_graph_filtered(id, depth, edge_kinds)`.
- **Same graph as the server (L16).** ✅ resolved: stored relations map to edge kinds through the shared `domain::GraphEdgeKind::of_relation` (place nesting appears once as `part-of-place`), `document:copy-of` edges, the tag toggle (`tag:<tag>` nodes, `tag` edges), `GraphNode.{kind: GraphNodeKind, path, updated}`.
- **Cluster regions and labels.** ✅ resolved: `ClusterLabel.{x, y, hull, radius}` (convex hull padded around the members).
- **Label priority.** ✅ resolved: `GraphNode.label_rank` and `is_hub`.
- **Hover card.** ✅ resolved: `GraphNode.summary` and `updated_label`.
- **Edges.** ✅ resolved: `GraphEdge.{id, rel_type, label, reason}`.
- **Mind map.** ✅ resolved: `save_layout(center_id, name, positions)` → `maps/<name>.canvas` (JSON Canvas via `PUT /maps`), `LocalGraphView.{summary, relation_count, ai_relation_count, relation_label}`. ⏳ open: `propose_relation(src, dst)` — no endpoint proposes a relation type for a pair (`propose_relation: NotYetAvailable`).
- **Similarity.** ✅ resolved: `refresh_similarity()` fetches the server's similarity edges (`GET /graph`); `GraphFilter.similarity` shows them.
- **Neighbourhood for highlighting.** ✅ resolved: `GraphFilter.focus` → `GlobalGraphView.neighbours`.

### Ask and search
- **Asking.** ✅ resolved: `ask(question, AskScope) -> answer id` streaming into `watch_ask()` (`AskMessage.streaming`, partial text), `stop_ask()`, `new_conversation()`, `AskView.scopes`.
- **Inline citations.** ✅ resolved: `AskMessage.spans: Vec<AskSpan>` and `sources: Vec<AskSource { note_id, title, path, anchors, indexes }>`.
- **Save as note.** ✅ resolved: `save_answer_as_note(message_id)`.
- **Answer metadata.** ✅ resolved: `AskMessage.{scope_label, source_count, created_label, error_key}`, `AskView.ai_status`.
- **Source preview.** ✅ resolved: `resolve_citation(note_id, anchor) -> CitationPreview`.
- **Per-paragraph direction.** ✅ resolved: `AskMessage.dir`, `SearchHit.{title_dir, snippet_dir}`, `NoteListItem.snippet_dir`, `CitedBullet.dir`, `GraphNode.title_dir`.
- **Search.** ✅ resolved: `SearchHit.{highlights, score}`, `SearchView.available_modes`; `search` and `search_in_folder` run the server's semantic/hybrid search online (keyword mode and offline use the local index).

## Coordinator notes (to resolve in the core-gaps pass)
- **Maps filtering in the core.** ✅ resolved (see Maps): the global map and the mind map ask the core for filtered view-models; `GraphPaintOptions` / `MindMapCanvas` no longer hide anything.
- **`EntityScreen` widget rename.** ⏳ open: a feature-UI rename; the widget still clashes with the view-model name (imports use `hide`/prefixes). Out of scope of this pass (no feature UI rewrites).
- **Content direction everywhere.** ✅ resolved in the core (every text field above has a `*_dir`); adopting them is per feature UI.
- **Shared test helpers.** ⏳ open: feature packages still carry their own `test/helpers`; `strata_state/testing.dart` now has a fixture for every view-model type (128) to build on.
- **Explicit inbox intents.** ✅ resolved in the core: `resolve_capture_duplicate`, `undo_suggestion` / `reject_ai_decision`, `accept_capture`, `reject_capture`, `accept_suggestion_with`, `resolve_link_or_create`. The inbox UI still uses the provisional mappings until its next pass.

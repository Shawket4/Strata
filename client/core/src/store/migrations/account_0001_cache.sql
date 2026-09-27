-- Account database v1 (PLAN §12.2, §12.7, D14): the full local cache of one account's server
-- data, the outbox, sync state, conflicts, device settings and the session tokens.

-- The signed-in account (one row).
CREATE TABLE account (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    user_id TEXT NOT NULL,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    server_url TEXT NOT NULL,
    timezone TEXT NOT NULL,
    ui_language TEXT NOT NULL,
    deletion_at TEXT,
    password_change_required INTEGER NOT NULL DEFAULT 0,
    disabled_warned_at TEXT
);

-- Session tokens, unencrypted by the owner's choice (D14).
CREATE TABLE auth_tokens (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    device_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    access_token TEXT NOT NULL,
    access_expires_at TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    refresh_expires_at TEXT NOT NULL,
    export_only INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

-- Per-device settings (reminders on/off, notification permission, default reminder time).
CREATE TABLE device_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Notes: the current local view (server base + live outbox ops re-applied) and the server
-- base the ops were applied to.
CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    content TEXT NOT NULL,
    frontmatter BLOB NOT NULL,
    created TEXT,
    updated TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    base_exists INTEGER NOT NULL DEFAULT 0,
    base_path TEXT,
    base_content TEXT,
    base_version TEXT,
    local_updated_at TEXT NOT NULL
);
CREATE INDEX notes_path ON notes (path);
CREATE INDEX notes_updated ON notes (deleted, local_updated_at);

CREATE TABLE tags (
    note_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);
CREATE INDEX tags_tag ON tags (tag);

-- Body wikilinks and embeds.
CREATE TABLE links (
    note_id TEXT NOT NULL,
    ord INTEGER NOT NULL,
    dst_id TEXT,
    dst_raw TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('link', 'embed')),
    anchor TEXT,
    PRIMARY KEY (note_id, ord)
);
CREATE INDEX links_dst ON links (dst_id);

-- Frontmatter relations (directed, stored on the source note).
CREATE TABLE relations (
    src_id TEXT NOT NULL,
    rel_type TEXT NOT NULL,
    dst_raw TEXT NOT NULL,
    dst_id TEXT,
    PRIMARY KEY (src_id, rel_type, dst_raw)
);
CREATE INDEX relations_dst ON relations (dst_id);

-- AI provenance of relations (from the server's sidecar metadata).
CREATE TABLE relation_meta (
    src_id TEXT NOT NULL,
    dst_id TEXT NOT NULL,
    rel_type TEXT NOT NULL,
    by TEXT NOT NULL,
    confidence REAL,
    reason TEXT,
    PRIMARY KEY (src_id, dst_id, rel_type)
);

CREATE TABLE rejected (
    src_id TEXT NOT NULL,
    dst_id TEXT NOT NULL,
    rel_type TEXT NOT NULL,
    at TEXT NOT NULL,
    PRIMARY KEY (src_id, dst_id, rel_type)
);

CREATE TABLE entities (
    note_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT,
    industry TEXT
);

CREATE TABLE entity_aliases (
    note_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    alias_normalized TEXT NOT NULL,
    PRIMARY KEY (note_id, alias)
);
CREATE INDEX entity_aliases_normalized ON entity_aliases (alias_normalized);

CREATE TABLE places (
    note_id TEXT PRIMARY KEY,
    parent_id TEXT,
    parent_raw TEXT
);

CREATE TABLE documents (
    note_id TEXT PRIMARY KEY,
    doc_type TEXT,
    copy TEXT,
    copy_of_id TEXT,
    location_id TEXT,
    location_raw TEXT,
    holder_id TEXT,
    holder_raw TEXT,
    last_holder_id TEXT,
    last_holder_raw TEXT,
    status TEXT,
    expires TEXT
);

CREATE TABLE custody_events (
    document_id TEXT NOT NULL,
    ord INTEGER NOT NULL,
    type TEXT NOT NULL,
    at TEXT NOT NULL,
    place_id TEXT,
    place_raw TEXT,
    person_id TEXT,
    person_raw TEXT,
    counterparty_id TEXT,
    counterparty_raw TEXT,
    citations BLOB NOT NULL,
    PRIMARY KEY (document_id, ord)
);

-- Tasks: checklist lines (§6.11). `id` is the `^t-…` block ID, or `line:<note>:<n>` for a
-- line the server has not given an ID yet.
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    line_no INTEGER NOT NULL,
    line TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    due TEXT,
    scheduled TEXT,
    start TEXT,
    done_at TEXT,
    cancelled_at TEXT,
    recurrence_raw TEXT,
    rrule TEXT,
    recurrence_error TEXT
);
CREATE INDEX tasks_note ON tasks (note_id);

-- Reminder markers `(@YYYY-MM-DD HH:mm)`; `remind_time` is '' for date-only reminders.
CREATE TABLE task_reminders (
    task_id TEXT NOT NULL,
    remind_date TEXT NOT NULL,
    remind_time TEXT NOT NULL,
    PRIMARY KEY (task_id, remind_date, remind_time)
);

CREATE TABLE inbox (
    note_id TEXT PRIMARY KEY,
    created TEXT NOT NULL
);

CREATE TABLE suggestions (
    id TEXT PRIMARY KEY,
    note_id TEXT,
    kind TEXT NOT NULL,
    payload BLOB NOT NULL,
    status TEXT NOT NULL,
    base_status TEXT NOT NULL,
    created TEXT NOT NULL
);

CREATE TABLE clusters (
    note_id TEXT PRIMARY KEY,
    cluster_id INTEGER NOT NULL
);

CREATE TABLE cluster_names (
    cluster_id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

-- Full-text index over text-normalize output (Arabic letter variants, tashkeel, case).
CREATE VIRTUAL TABLE notes_fts USING fts5 (
    note_id UNINDEXED,
    title,
    body,
    tags,
    tokenize = 'unicode61 remove_diacritics 0'
);

-- Pending mutations (§12.2), pushed in `ord` order.
CREATE TABLE outbox (
    op_id TEXT PRIMARY KEY,
    ord INTEGER NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    local_entity TEXT NOT NULL,
    base_version TEXT,
    payload BLOB NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('pending', 'inflight', 'done', 'conflict', 'duplicate', 'rejected')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created TEXT NOT NULL
);
CREATE INDEX outbox_status ON outbox (status, ord);
CREATE INDEX outbox_local_entity ON outbox (local_entity, ord);

CREATE TABLE sync_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    epoch INTEGER,
    cursor_seq INTEGER NOT NULL DEFAULT 0,
    bootstrap_cursor TEXT,
    bootstrap_complete INTEGER NOT NULL DEFAULT 0,
    bootstrap_pages INTEGER NOT NULL DEFAULT 0,
    last_pull_at TEXT,
    last_push_at TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);
INSERT INTO sync_state (singleton) VALUES (1);

-- Push results of kind `conflict` (D19): the local edit, the server version and the local
-- 3-way merge preview.
CREATE TABLE conflicts (
    op_id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    local_payload BLOB NOT NULL,
    base_content TEXT,
    local_content TEXT,
    server_version TEXT NOT NULL,
    server_content TEXT,
    merged_preview TEXT,
    merge_clean INTEGER,
    resolution TEXT,
    created TEXT NOT NULL
);

-- Push results of kind `duplicate` awaiting the user's choice (§9.7).
CREATE TABLE duplicates (
    op_id TEXT PRIMARY KEY,
    candidates BLOB NOT NULL,
    created TEXT NOT NULL
);

-- Push results of kind `rejected` (rolled back), until the user dismisses them.
CREATE TABLE rejections (
    op_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    problem_type TEXT NOT NULL,
    status INTEGER NOT NULL,
    created TEXT NOT NULL
);

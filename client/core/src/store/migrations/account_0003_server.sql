-- Account database v3: data the connected server provides beyond v1 (AI summaries, relation
-- timestamps, suggestion threads), the events stream position, pause, the sync log, cached
-- online-only reads (devices, history, AI status, integrity, export info), pinned notes and
-- acknowledged AI decisions.

ALTER TABLE notes ADD COLUMN summary TEXT;
ALTER TABLE relation_meta ADD COLUMN created TEXT;
ALTER TABLE suggestions ADD COLUMN replies BLOB;
ALTER TABLE sync_state ADD COLUMN events_seq INTEGER;
ALTER TABLE sync_state ADD COLUMN paused INTEGER NOT NULL DEFAULT 0;

-- Online-only reads kept for display while offline (`devices`, `history:<note>`, `ai_status`,
-- `integrity`, `export`, `admin_pending`): MessagePack values.
CREATE TABLE remote_cache (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    fetched_at TEXT NOT NULL
);

-- The last sync cycles (newest kept, see `store::sync_log`).
CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at TEXT NOT NULL,
    kind TEXT NOT NULL,
    detail TEXT NOT NULL
);

-- Notes pinned to the sidebar on this device, in pin order.
CREATE TABLE pinned_notes (
    note_id TEXT PRIMARY KEY,
    ord INTEGER NOT NULL
);

-- AI decisions the user confirmed ("Looks right"), hidden from the inbox.
CREATE TABLE acknowledged_suggestions (
    id TEXT PRIMARY KEY,
    at TEXT NOT NULL
);

-- The content a `note.update` op was made against, so queued edits can be rebased with a
-- 3-way merge after a pull brings a newer server version (D19 on the device).
ALTER TABLE outbox ADD COLUMN base_content TEXT;

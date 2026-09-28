-- A note's AI follow-up thread (owner decision 2026-09-28), as the server syncs it: the
-- messages (MessagePack `Vec<ThreadMessage>`), oldest first. Server-only state: replaced by
-- every sync record, dropped with the note's tombstone and re-sent in full by a bootstrap.
CREATE TABLE note_threads (
    note_id  TEXT PRIMARY KEY,
    messages BLOB NOT NULL
);

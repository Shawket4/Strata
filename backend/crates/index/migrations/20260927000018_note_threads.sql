-- A note's AI follow-up thread (owner decision 2026-09-28): derived from
-- `.meta/threads/<note-id>.json` (rebuilt by `stratad reindex`, principle 1). `thread` is the
-- file's JSON as written; `version` its content hash, so an unchanged write logs nothing. The
-- row is a function of the file alone, so an incremental index equals a rebuilt one.
-- A thread goes with its note (purge cascades); devices drop it with the note's tombstone.
CREATE TABLE note_threads (
    user_id uuid NOT NULL,
    note_id uuid NOT NULL,
    thread  text NOT NULL,
    version text NOT NULL,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
SELECT strata_make_user_owned('note_threads');

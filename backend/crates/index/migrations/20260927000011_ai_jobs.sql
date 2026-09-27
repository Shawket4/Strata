-- AI jobs (PLAN §9.1b, §9.2, §9.5–§9.7): chunk positions for citations, note-level vectors,
-- and per-item vectors for semantic duplicate detection. All derived: rebuilt by the `embed`
-- job from the vault (principle 1); every vector row stores the model that produced it, so a
-- model change is detected row by row and triggers a resumable full re-embed.

-- Chunks are heading-aware groups of consecutive blocks (~300–500 tokens). A chunk is cited
-- through its first block: `anchor_len` bytes of `text` are that block, `block_id` its `^id`
-- (NULL until one is appended for a citation). Offsets are body byte offsets in the note
-- version `content_hash`.
ALTER TABLE chunks
    ADD COLUMN ord          integer NOT NULL DEFAULT 0 CHECK (ord >= 0),
    ADD COLUMN heading_path text NOT NULL DEFAULT '',
    ADD COLUMN start_offset integer NOT NULL DEFAULT 0 CHECK (start_offset >= 0),
    ADD COLUMN end_offset   integer NOT NULL DEFAULT 0,
    ADD COLUMN anchor_len   integer NOT NULL DEFAULT 0 CHECK (anchor_len >= 0),
    ADD COLUMN content_hash text NOT NULL DEFAULT '',
    ADD CONSTRAINT chunks_offsets CHECK (end_offset >= start_offset);
CREATE INDEX chunks_note_ord ON chunks (user_id, note_id, ord);

-- One vector per note: the L2-normalised mean of its chunk vectors (§9.1b), for similarity
-- edges (§9.6) and note-level retrieval. `content_hash` is the note version it was computed
-- from; the embed job skips a note whose version and model are unchanged.
CREATE TABLE note_vectors (
    user_id      uuid NOT NULL,
    note_id      uuid NOT NULL,
    model        text NOT NULL,
    content_hash text NOT NULL,
    embedding    vector(384) NOT NULL,
    chunk_count  integer NOT NULL CHECK (chunk_count >= 0),
    updated      timestamptz NOT NULL,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
-- No HNSW index: nearest-neighbour queries are exact scans of one user's rows. An approximate
-- index scan returns the globally nearest candidates first and row-level security then drops
-- other users' rows, which can leave a user with too few (or no) results; pgvector < 0.8 has
-- no iterative scans to compensate. At the target scale (~10k notes per user) the exact scan
-- takes milliseconds.
CREATE INDEX note_vectors_model ON note_vectors (user_id, model);
SELECT strata_make_user_owned('note_vectors');

-- One vector per duplicate-check item (`dedupe_keys` item: note title, capture text, entity
-- name, task text), embedded from the same text the exact and near levels compare (§9.7).
-- No foreign key: task items are task lines; rows whose item disappeared are removed by the
-- embed and dedupe jobs and ignored by every query (joined with `dedupe_keys`).
CREATE TABLE dedupe_vectors (
    user_id   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind      text NOT NULL,
    item_id   text NOT NULL,
    note_id   uuid,
    model     text NOT NULL,
    -- SHA-256 (hex) of the embedded text.
    text_hash text NOT NULL,
    embedding vector(384) NOT NULL,
    updated   timestamptz NOT NULL,
    PRIMARY KEY (user_id, kind, item_id)
);
CREATE INDEX dedupe_vectors_note ON dedupe_vectors (user_id, note_id);
CREATE INDEX dedupe_vectors_model ON dedupe_vectors (user_id, model, kind);
SELECT strata_make_user_owned('dedupe_vectors');

-- LLM verdicts on borderline semantic pairs (§9.7), so the nightly sweep asks once per pair
-- and item text: a verdict applies while both items still have the text it judged (SHA-256
-- of model + text, as in `dedupe_vectors.text_hash`). IDs are ordered (`a_id < b_id`, byte
-- order).
CREATE TABLE dedupe_verdicts (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    a_id    text NOT NULL,
    b_id    text NOT NULL,
    a_hash  text NOT NULL,
    b_hash  text NOT NULL,
    verdict text NOT NULL CHECK (verdict IN ('duplicate', 'distinct', 'uncertain')),
    reason  text NOT NULL DEFAULT '',
    at      timestamptz NOT NULL,
    PRIMARY KEY (user_id, a_id, b_id),
    CHECK (a_id < b_id COLLATE "C")
);
SELECT strata_make_user_owned('dedupe_verdicts');

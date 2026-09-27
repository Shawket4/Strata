-- Derived note index (PLAN §7.4). Every table: user_id first in the PK, composite foreign keys
-- that include user_id (a row can never reference another user's row), standard RLS policy.

CREATE TABLE notes (
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id           uuid NOT NULL,
    path         text NOT NULL,
    title        text NOT NULL,
    kind         text NOT NULL DEFAULT 'note'
        CHECK (kind IN ('note', 'concept', 'person', 'company', 'document', 'place')),
    lang         text CHECK (lang IN ('ar', 'en', 'mixed')),
    created      timestamptz NOT NULL,
    updated      timestamptz NOT NULL,
    -- The note's version (PLAN §7.2), e.g. `sha256:<hex>`.
    content_hash text NOT NULL,
    word_count   integer NOT NULL DEFAULT 0 CHECK (word_count >= 0),
    -- Soft-deleted (moved to .trash/).
    trashed      boolean NOT NULL DEFAULT false,
    -- `simple`-config tsvector over text already normalised in Rust (/crates/text-normalize):
    -- title weight A, tags B, body C.
    search       tsvector NOT NULL DEFAULT ''::tsvector,
    PRIMARY KEY (user_id, id),
    UNIQUE (user_id, path)
);
CREATE INDEX notes_search ON notes USING gin (search);
CREATE INDEX notes_updated ON notes (user_id, updated DESC, id DESC);
SELECT strata_make_user_owned('notes');

CREATE TABLE aliases (
    user_id uuid NOT NULL,
    note_id uuid NOT NULL,
    alias   text NOT NULL,
    PRIMARY KEY (user_id, note_id, alias),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
CREATE INDEX aliases_alias ON aliases (user_id, alias);
SELECT strata_make_user_owned('aliases');

CREATE TABLE tags (
    user_id uuid NOT NULL,
    note_id uuid NOT NULL,
    tag     text NOT NULL,
    PRIMARY KEY (user_id, note_id, tag),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
CREATE INDEX tags_tag ON tags (user_id, tag);
SELECT strata_make_user_owned('tags');

-- Body wikilinks/embeds, in document order (`ord`).
CREATE TABLE links (
    user_id  uuid NOT NULL,
    src_id   uuid NOT NULL,
    ord      integer NOT NULL CHECK (ord >= 0),
    dst_id   uuid,
    dst_raw  text NOT NULL,
    kind     text NOT NULL CHECK (kind IN ('link', 'embed')),
    anchor   text,
    block_id text,
    PRIMARY KEY (user_id, src_id, ord),
    FOREIGN KEY (user_id, src_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, dst_id) REFERENCES notes (user_id, id) ON DELETE SET NULL (dst_id)
);
CREATE INDEX links_dst ON links (user_id, dst_id) WHERE dst_id IS NOT NULL;
SELECT strata_make_user_owned('links');

-- Typed, directed relations (frontmatter keys + sidecar provenance).
CREATE TABLE relations (
    user_id     uuid NOT NULL,
    src_id      uuid NOT NULL,
    dst_id      uuid NOT NULL,
    type        text NOT NULL CHECK (type ~ '^[a-z][a-z-]*$'),
    by          text NOT NULL CHECK (by IN ('user', 'ai')),
    confidence  real CHECK (confidence BETWEEN 0 AND 1),
    reason      text,
    created     timestamptz NOT NULL,
    decision_id uuid,
    PRIMARY KEY (user_id, src_id, dst_id, type),
    FOREIGN KEY (user_id, src_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, dst_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CHECK (src_id <> dst_id)
);
CREATE INDEX relations_dst ON relations (user_id, dst_id);
SELECT strata_make_user_owned('relations');

-- Rejected AI edges: never re-added for the (src, dst, type) triple (PLAN §6.5).
-- No foreign keys: the rejection memory is mirrored in .meta/ and outlives note deletion.
CREATE TABLE rejected (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    src_id  uuid NOT NULL,
    dst_id  uuid NOT NULL,
    type    text NOT NULL,
    at      timestamptz NOT NULL,
    PRIMARY KEY (user_id, src_id, dst_id, type)
);
SELECT strata_make_user_owned('rejected');

CREATE TABLE blocks (
    user_id      uuid NOT NULL,
    note_id      uuid NOT NULL,
    block_id     text NOT NULL,
    heading_path text NOT NULL DEFAULT '',
    text         text NOT NULL,
    start_offset integer NOT NULL CHECK (start_offset >= 0),
    end_offset   integer NOT NULL,
    PRIMARY KEY (user_id, note_id, block_id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CHECK (end_offset >= start_offset)
);
SELECT strata_make_user_owned('blocks');

-- Retrieval chunks with 384-dim embeddings (L19); model id stored per vector (§9.1b).
CREATE TABLE chunks (
    user_id     uuid NOT NULL,
    id          uuid NOT NULL,
    note_id     uuid NOT NULL,
    block_id    text,
    text        text NOT NULL,
    token_count integer NOT NULL CHECK (token_count >= 0),
    embedding   vector(384),
    model       text,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CONSTRAINT chunks_model_iff_embedding CHECK ((embedding IS NULL) = (model IS NULL))
);
CREATE INDEX chunks_note ON chunks (user_id, note_id);
CREATE INDEX chunks_embedding ON chunks USING hnsw (embedding vector_cosine_ops);
SELECT strata_make_user_owned('chunks');

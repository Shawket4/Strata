-- People, companies, documents, places (PLAN §6.7, §6.12) and correction memory (§9.8).

CREATE TABLE entities (
    user_id      uuid NOT NULL,
    note_id      uuid NOT NULL,
    kind         text NOT NULL CHECK (kind IN ('person', 'company', 'document', 'place')),
    display_name text NOT NULL,
    role         text,
    industry     text,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
CREATE INDEX entities_kind ON entities (user_id, kind, display_name);
SELECT strata_make_user_owned('entities');

CREATE TABLE entity_aliases (
    user_id          uuid NOT NULL,
    note_id          uuid NOT NULL,
    alias            text NOT NULL,
    -- Arabic normalisation + lowercase + transliteration key, computed in Rust.
    alias_normalized text NOT NULL,
    PRIMARY KEY (user_id, note_id, alias),
    FOREIGN KEY (user_id, note_id) REFERENCES entities (user_id, note_id) ON DELETE CASCADE
);
CREATE INDEX entity_aliases_exact ON entity_aliases (user_id, alias_normalized);
CREATE INDEX entity_aliases_trgm ON entity_aliases USING gin (alias_normalized gin_trgm_ops);
SELECT strata_make_user_owned('entity_aliases');

-- block_id = '' means a note-level mention (people:/companies: keys); PK columns can't be NULL.
CREATE TABLE mentions (
    user_id    uuid NOT NULL,
    entity_id  uuid NOT NULL,
    note_id    uuid NOT NULL,
    block_id   text NOT NULL DEFAULT '',
    first_seen timestamptz NOT NULL,
    last_seen  timestamptz NOT NULL,
    PRIMARY KEY (user_id, entity_id, note_id, block_id),
    FOREIGN KEY (user_id, entity_id) REFERENCES entities (user_id, note_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CHECK (last_seen >= first_seen)
);
CREATE INDEX mentions_note ON mentions (user_id, note_id);
SELECT strata_make_user_owned('mentions');

CREATE TABLE clusters (
    user_id    uuid NOT NULL,
    note_id    uuid NOT NULL,
    cluster_id bigint NOT NULL,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE
);
CREATE INDEX clusters_cluster ON clusters (user_id, cluster_id);
SELECT strata_make_user_owned('clusters');

CREATE TABLE cluster_names (
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    cluster_id bigint NOT NULL,
    name       text NOT NULL,
    PRIMARY KEY (user_id, cluster_id)
);
SELECT strata_make_user_owned('cluster_names');

-- Places nest via part-of; queries use recursive CTEs.
CREATE TABLE places (
    user_id   uuid NOT NULL,
    note_id   uuid NOT NULL,
    parent_id uuid,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES entities (user_id, note_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, parent_id) REFERENCES places (user_id, note_id) ON DELETE SET NULL (parent_id),
    CHECK (parent_id <> note_id)
);
CREATE INDEX places_parent ON places (user_id, parent_id);
SELECT strata_make_user_owned('places');

CREATE TABLE documents (
    user_id        uuid NOT NULL,
    note_id        uuid NOT NULL,
    doc_type       text NOT NULL,
    copy           text NOT NULL CHECK (copy IN ('original', 'certified copy', 'copy', 'digital')),
    copy_of        uuid,
    location_id    uuid,
    holder_id      uuid,
    last_holder_id uuid,
    status         text NOT NULL
        CHECK (status IN ('stored', 'checked-out', 'with-third-party', 'lost', 'destroyed')),
    expires        date,
    PRIMARY KEY (user_id, note_id),
    FOREIGN KEY (user_id, note_id) REFERENCES entities (user_id, note_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, copy_of) REFERENCES documents (user_id, note_id) ON DELETE SET NULL (copy_of),
    FOREIGN KEY (user_id, location_id) REFERENCES places (user_id, note_id) ON DELETE SET NULL (location_id),
    FOREIGN KEY (user_id, holder_id) REFERENCES entities (user_id, note_id) ON DELETE SET NULL (holder_id),
    FOREIGN KEY (user_id, last_holder_id) REFERENCES entities (user_id, note_id) ON DELETE SET NULL (last_holder_id),
    CHECK (copy_of <> note_id)
);
CREATE INDEX documents_location ON documents (user_id, location_id);
CREATE INDEX documents_holder ON documents (user_id, holder_id);
CREATE INDEX documents_expires ON documents (user_id, expires) WHERE expires IS NOT NULL;
SELECT strata_make_user_owned('documents');

CREATE TABLE custody_events (
    user_id         uuid NOT NULL,
    id              uuid NOT NULL,
    document_id     uuid NOT NULL,
    type            text NOT NULL CHECK (type IN (
        'stored-at', 'moved-to', 'handed-to', 'returned-by', 'sent-to', 'received-from',
        'lost', 'found', 'destroyed')),
    at              timestamptz NOT NULL,
    place_id        uuid,
    person_id       uuid,
    counterparty_id uuid,
    by              text NOT NULL CHECK (by IN ('user', 'ai')),
    confidence      real CHECK (confidence BETWEEN 0 AND 1),
    source_note_id  uuid,
    source_block_id text,
    created         timestamptz NOT NULL,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id, document_id) REFERENCES documents (user_id, note_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, place_id) REFERENCES places (user_id, note_id) ON DELETE SET NULL (place_id),
    FOREIGN KEY (user_id, person_id) REFERENCES entities (user_id, note_id) ON DELETE SET NULL (person_id),
    FOREIGN KEY (user_id, counterparty_id) REFERENCES entities (user_id, note_id) ON DELETE SET NULL (counterparty_id),
    FOREIGN KEY (user_id, source_note_id) REFERENCES notes (user_id, id) ON DELETE SET NULL (source_note_id)
);
CREATE INDEX custody_events_document ON custody_events (user_id, document_id, at DESC, id DESC);
SELECT strata_make_user_owned('custody_events');

-- "Ahmed at Acme = Ahmed Samir" (PLAN §9.8), fed to future resolution prompts.
CREATE TABLE disambiguation_hints (
    user_id            uuid NOT NULL,
    id                 uuid NOT NULL,
    entity_id          uuid NOT NULL,
    hint               text NOT NULL,
    source_decision_id uuid,
    created            timestamptz NOT NULL,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id, entity_id) REFERENCES entities (user_id, note_id) ON DELETE CASCADE
);
CREATE INDEX disambiguation_hints_entity ON disambiguation_hints (user_id, entity_id, created);
SELECT strata_make_user_owned('disambiguation_hints');

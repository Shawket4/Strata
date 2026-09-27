-- Per-user application state (not derived): jobs, suggestions, AI memory, settings, sync.

CREATE TABLE jobs (
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id           uuid NOT NULL,
    kind         text NOT NULL CHECK (kind ~ '^[a-z][a-z_]*$'),
    note_id      uuid,
    -- MessagePack job parameters.
    payload      bytea NOT NULL DEFAULT '\x'::bytea,
    status       text NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed')),
    attempts     integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts integer NOT NULL CHECK (max_attempts >= 1),
    run_after    timestamptz NOT NULL,
    locked_at    timestamptz,
    last_error   text,
    -- Debounce key: at most one queued job per (kind, dedupe_key); re-enqueueing moves run_after.
    dedupe_key   text,
    created      timestamptz NOT NULL,
    updated      timestamptz NOT NULL,
    PRIMARY KEY (user_id, id),
    CHECK ((status = 'running') = (locked_at IS NOT NULL))
);
-- Claim path: WHERE status = 'queued' AND run_after <= $now ORDER BY run_after, id
-- FOR UPDATE SKIP LOCKED LIMIT 1 (RLS adds user_id = …).
CREATE INDEX jobs_runnable ON jobs (user_id, run_after, id) WHERE status = 'queued';
CREATE INDEX jobs_running ON jobs (user_id, locked_at) WHERE status = 'running';
CREATE UNIQUE INDEX jobs_debounce ON jobs (user_id, kind, dedupe_key)
    WHERE status = 'queued' AND dedupe_key IS NOT NULL;
SELECT strata_make_user_owned('jobs');

-- Keep job_wakeups (global scheduler hints) current: any row that becomes queued lowers or
-- creates its user's wakeup. Runs as the invoking role (strata_app has DML on job_wakeups).
CREATE FUNCTION strata_jobs_wakeup() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    INSERT INTO job_wakeups AS w (user_id, run_after)
    VALUES (NEW.user_id, NEW.run_after)
    ON CONFLICT (user_id) DO UPDATE SET run_after = LEAST(w.run_after, EXCLUDED.run_after);
    RETURN NULL;
END
$$;
CREATE TRIGGER jobs_wakeup
    AFTER INSERT OR UPDATE OF status, run_after ON jobs
    FOR EACH ROW WHEN (NEW.status = 'queued')
    EXECUTE FUNCTION strata_jobs_wakeup();

CREATE TABLE suggestions (
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id         uuid NOT NULL,
    note_id    uuid,
    kind       text NOT NULL CHECK (kind ~ '^[a-z][a-z_]*$'),
    -- MessagePack payload (PLAN §7.4).
    payload    bytea NOT NULL,
    -- `superseded`: replaced by a re-proposal after a threaded reply (§9.8).
    status     text NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'superseded')),
    created    timestamptz NOT NULL,
    updated    timestamptz NOT NULL,
    decided_at timestamptz,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CHECK ((status IN ('accepted', 'rejected')) = (decided_at IS NOT NULL))
);
CREATE INDEX suggestions_status ON suggestions (user_id, status, created, id);
CREATE INDEX suggestions_note ON suggestions (user_id, note_id) WHERE note_id IS NOT NULL;
SELECT strata_make_user_owned('suggestions');

-- Threaded replies on a suggestion ("no, the Petrol Arrows one", §9.8).
CREATE TABLE suggestion_replies (
    user_id       uuid NOT NULL,
    suggestion_id uuid NOT NULL,
    id            uuid NOT NULL,
    author        text NOT NULL CHECK (author IN ('user', 'ai')),
    body          text NOT NULL,
    created       timestamptz NOT NULL,
    PRIMARY KEY (user_id, suggestion_id, id),
    FOREIGN KEY (user_id, suggestion_id) REFERENCES suggestions (user_id, id) ON DELETE CASCADE
);
SELECT strata_make_user_owned('suggestion_replies');

-- Every AI decision with an ID and its source (§9.8): the context the correction loop feeds
-- the model ("last ~50 decisions with IDs, sources and targets").
CREATE TABLE ai_decisions (
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id              uuid NOT NULL,
    kind            text NOT NULL CHECK (kind IN (
        'link', 'entity_mention', 'relation', 'custody_event', 'task_suggestion', 'filing',
        'concept', 'correction')),
    source_note_id  uuid,
    source_block_id text,
    target_type     text NOT NULL,
    target_id       text NOT NULL,
    summary         text NOT NULL DEFAULT '',
    confidence      real CHECK (confidence BETWEEN 0 AND 1),
    job_id          uuid,
    suggestion_id   uuid,
    git_commit      text,
    created         timestamptz NOT NULL,
    reverted_at     timestamptz,
    PRIMARY KEY (user_id, id)
);
CREATE INDEX ai_decisions_recent ON ai_decisions (user_id, created DESC, id DESC);
CREATE INDEX ai_decisions_source ON ai_decisions (user_id, source_note_id);
SELECT strata_make_user_owned('ai_decisions');

CREATE TABLE ai_usage (
    user_id         uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day             date NOT NULL,
    provider        text NOT NULL,
    model           text NOT NULL,
    calls           integer NOT NULL DEFAULT 0 CHECK (calls >= 0),
    input_tokens    bigint NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens   bigint NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    -- Estimated cost in micro-USD (integer money, no floats).
    est_cost_micros bigint NOT NULL DEFAULT 0 CHECK (est_cost_micros >= 0),
    PRIMARY KEY (user_id, day, provider, model)
);
SELECT strata_make_user_owned('ai_usage');

CREATE TABLE settings (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    key     text NOT NULL,
    -- MessagePack value.
    value   bytea NOT NULL,
    updated timestamptz NOT NULL,
    PRIMARY KEY (user_id, key)
);
SELECT strata_make_user_owned('settings');

-- Per-user sync counters. `last_seq` is bumped under this row's lock by every change-log append,
-- so seq order equals commit order per user (a global bigserial would let a reader skip a row
-- committed late with a lower seq). A rebuild that cannot preserve seq bumps `epoch`.
CREATE TABLE sync_epochs (
    user_id  uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    epoch    integer NOT NULL CHECK (epoch >= 1),
    last_seq bigint NOT NULL CHECK (last_seq >= 0),
    updated  timestamptz NOT NULL
);
SELECT strata_make_user_owned('sync_epochs');

CREATE TABLE change_log (
    user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    seq         bigint NOT NULL CHECK (seq >= 1),
    epoch       integer NOT NULL,
    entity_type text NOT NULL CHECK (entity_type ~ '^[a-z][a-z_]*$'),
    entity_id   text NOT NULL,
    op          text NOT NULL CHECK (op IN ('upsert', 'delete')),
    version     text,
    at          timestamptz NOT NULL,
    PRIMARY KEY (user_id, seq)
);
SELECT strata_make_user_owned('change_log');

-- Sync push replay protection: result stored as MessagePack under the client's op_id.
CREATE TABLE idempotency (
    user_id   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    op_id     uuid NOT NULL,
    device_id uuid NOT NULL,
    result    bytea NOT NULL,
    created   timestamptz NOT NULL,
    PRIMARY KEY (user_id, op_id)
);
CREATE INDEX idempotency_created ON idempotency (user_id, created);
SELECT strata_make_user_owned('idempotency');

-- Normalised duplicate keys (§9.7). item_id is text: tasks are keyed by block ID.
CREATE TABLE dedupe_keys (
    user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind         text NOT NULL CHECK (kind ~ '^[a-z][a-z_]*$'),
    item_id      text NOT NULL,
    exact_key    text NOT NULL,
    trigram_text text NOT NULL,
    PRIMARY KEY (user_id, kind, item_id)
);
CREATE INDEX dedupe_keys_exact ON dedupe_keys (user_id, kind, exact_key);
CREATE INDEX dedupe_keys_trgm ON dedupe_keys USING gin (trigram_text gin_trgm_ops);
SELECT strata_make_user_owned('dedupe_keys');

-- "Create anyway": the pair is never flagged again. Stored ordered (a_id < b_id, byte order = Rust `str` order).
CREATE TABLE dedupe_keep_both (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind    text NOT NULL,
    a_id    text NOT NULL,
    b_id    text NOT NULL,
    at      timestamptz NOT NULL,
    PRIMARY KEY (user_id, kind, a_id, b_id),
    CHECK (a_id < b_id COLLATE "C")
);
SELECT strata_make_user_owned('dedupe_keep_both');

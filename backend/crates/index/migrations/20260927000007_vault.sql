-- Vault store (PLAN §7.3): integrity warnings raised by reconciliation (out-of-band edits,
-- uncommitted changes recovered, sidecar drift repaired, crash leftovers removed), surfaced by
-- `GET /integrity`. App state, not derived: a reindex keeps them.

CREATE TABLE integrity_warnings (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id      uuid NOT NULL,
    kind    text NOT NULL CHECK (kind ~ '^[a-z][a-z_]*$'),
    -- Vault-relative path concerned, if any.
    path    text,
    -- Content-free description.
    detail  text NOT NULL,
    created timestamptz NOT NULL,
    PRIMARY KEY (user_id, id)
);
CREATE INDEX integrity_warnings_created ON integrity_warnings (user_id, created DESC, id DESC);
SELECT strata_make_user_owned('integrity_warnings');

-- Duplicate keys as the shared `dedupe` crate produces them (PLAN §9.7, L16): one row per
-- name of an item (entities have several), each with its exact key, trigram text and
-- transliteration key, and the item itself (MessagePack `dedupe::Item`) so candidates found by
-- SQL are decided by `dedupe::check` exactly as the client core decides offline.
ALTER TABLE dedupe_keys ADD COLUMN key_no smallint NOT NULL DEFAULT 0 CHECK (key_no >= 0);
ALTER TABLE dedupe_keys ADD COLUMN phonetic_key text;
ALTER TABLE dedupe_keys ADD COLUMN item bytea;
ALTER TABLE dedupe_keys DROP CONSTRAINT dedupe_keys_pkey;
ALTER TABLE dedupe_keys ADD PRIMARY KEY (user_id, kind, item_id, key_no);
CREATE INDEX dedupe_keys_phonetic ON dedupe_keys (user_id, kind, phonetic_key)
    WHERE phonetic_key IS NOT NULL;

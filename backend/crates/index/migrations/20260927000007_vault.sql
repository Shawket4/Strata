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

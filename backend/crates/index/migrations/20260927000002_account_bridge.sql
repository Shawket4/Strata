-- Devices, sessions and refresh tokens (PLAN §8, D6). User-owned (user_id + RLS) but also
-- reachable by strata_accounts through `strata_accounts_access` (see foundation migration).

CREATE TABLE devices (
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id            uuid NOT NULL,
    name          text NOT NULL,
    platform      text NOT NULL CHECK (platform IN ('android', 'ios', 'macos', 'windows', 'linux')),
    created       timestamptz NOT NULL,
    last_seen     timestamptz NOT NULL,
    push_provider text NOT NULL DEFAULT 'none' CHECK (push_provider IN ('fcm', 'apns', 'wns', 'none')),
    push_token    text,
    push_updated  timestamptz,
    PRIMARY KEY (user_id, id),
    CONSTRAINT devices_push_token_iff_provider CHECK ((push_provider = 'none') = (push_token IS NULL))
);
SELECT strata_make_account_bridge('devices');
-- Devices are created at login (accounts); the app lists, renames, sets push tokens, removes.
REVOKE INSERT ON devices FROM strata_app;

CREATE TABLE sessions (
    user_id        uuid NOT NULL,
    id             uuid NOT NULL,
    device_id      uuid NOT NULL,
    created        timestamptz NOT NULL,
    expires        timestamptz NOT NULL,
    revoked_at     timestamptz,
    revoked_reason text CHECK (revoked_reason IN (
        'logout', 'device_removed', 'user_disabled', 'deletion_scheduled', 'refresh_reuse', 'expired', 'admin')),
    -- Export-only session of a deletion_pending account (D25).
    export_only    boolean NOT NULL DEFAULT false,
    PRIMARY KEY (user_id, id),
    UNIQUE (id),
    FOREIGN KEY (user_id, device_id) REFERENCES devices (user_id, id) ON DELETE CASCADE,
    CONSTRAINT sessions_revoked_reason CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);
CREATE INDEX sessions_device ON sessions (user_id, device_id);
CREATE INDEX sessions_revoked ON sessions (revoked_at) WHERE revoked_at IS NOT NULL;
SELECT strata_make_account_bridge('sessions');
-- The app may list and revoke (logout) its user's sessions, never create or delete them.
REVOKE INSERT, DELETE ON sessions FROM strata_app;

CREATE TABLE refresh_tokens (
    user_id     uuid NOT NULL,
    token_hash  bytea NOT NULL,
    session_id  uuid NOT NULL,
    issued      timestamptz NOT NULL,
    expires     timestamptz NOT NULL,
    -- Set when rotated; presenting a used token again is reuse (revokes the session).
    used_at     timestamptz,
    PRIMARY KEY (user_id, token_hash),
    UNIQUE (token_hash),
    FOREIGN KEY (user_id, session_id) REFERENCES sessions (user_id, id) ON DELETE CASCADE
);
CREATE INDEX refresh_tokens_session ON refresh_tokens (user_id, session_id);
SELECT strata_make_account_bridge('refresh_tokens');
-- Refresh tokens are the accounts layer's business only.
REVOKE ALL ON refresh_tokens FROM strata_app;

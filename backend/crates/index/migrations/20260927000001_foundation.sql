-- Strata foundation: scope function, table helper, and the global account tables (PLAN §5.2, §7.4).
-- Runs as `strata_owner` inside schema `strata` (created by bootstrap; see src/bootstrap.rs).
-- Extensions `vector` and `pg_trgm` are created by bootstrap (superuser), because pgvector is
-- not a trusted extension.

-- The user the current transaction is scoped to, or NULL when unscoped. After a
-- transaction-local set_config ends, the setting reads as '' (not NULL) for the rest of the
-- session, hence NULLIF: an unscoped query must see zero rows, never all rows.
CREATE FUNCTION strata_current_user() RETURNS uuid
    LANGUAGE sql STABLE PARALLEL SAFE
    AS $$ SELECT NULLIF(pg_catalog.current_setting('strata.user_id', true), '')::uuid $$;

-- Applies the standard user-owned treatment to a table: RLS enabled + forced, the single
-- standard policy, DML for strata_app only, nothing for strata_accounts or PUBLIC.
-- Every migration that adds a user-owned table calls this (the schema audit test enforces it).
CREATE FUNCTION strata_make_user_owned(tbl regclass) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tbl);
    EXECUTE format('ALTER TABLE %s FORCE ROW LEVEL SECURITY', tbl);
    EXECUTE format(
        'CREATE POLICY strata_user_isolation ON %s '
        'USING (user_id = strata_current_user()) WITH CHECK (user_id = strata_current_user())',
        tbl);
    EXECUTE format('REVOKE ALL ON %s FROM PUBLIC, strata_accounts', tbl);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %s TO strata_app', tbl);
END
$$;
REVOKE ALL ON FUNCTION strata_make_user_owned(regclass) FROM PUBLIC;

-- Account-bridge tables (devices, sessions, refresh tokens) are user-owned with the standard
-- policy for strata_app, plus one extra policy letting strata_accounts reach every user's rows
-- (login creates devices/sessions; refresh looks tokens up by hash before any user is known).
CREATE FUNCTION strata_make_account_bridge(tbl regclass) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    PERFORM strata_make_user_owned(tbl);
    EXECUTE format(
        'CREATE POLICY strata_accounts_access ON %s TO strata_accounts USING (true) WITH CHECK (true)',
        tbl);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON %s TO strata_accounts', tbl);
END
$$;
REVOKE ALL ON FUNCTION strata_make_account_bridge(regclass) FROM PUBLIC;

-- ---------------------------------------------------------------------------------------------
-- Global tables: no user_id scoping, no RLS; reachable by strata_accounts (plus the narrow
-- strata_app grants listed below).
-- ---------------------------------------------------------------------------------------------

CREATE TABLE users (
    id                    uuid PRIMARY KEY,
    username              text NOT NULL,
    -- Normalised (case, Unicode confusables) by the accounts layer; uniqueness is on this.
    username_normalized   text NOT NULL UNIQUE,
    display_name          text NOT NULL,
    password_hash         text NOT NULL,
    role                  text NOT NULL CHECK (role IN ('admin', 'member')),
    status                text NOT NULL
        CHECK (status IN ('pending', 'active', 'disabled', 'rejected', 'deletion_pending')),
    created               timestamptz NOT NULL,
    updated               timestamptz NOT NULL,
    approved_by           uuid REFERENCES users (id) ON DELETE SET NULL,
    approved_at           timestamptz,
    rejected_at           timestamptz,
    disabled_at           timestamptz,
    deletion_requested_by uuid REFERENCES users (id) ON DELETE SET NULL,
    deletion_requested_at timestamptz,
    -- When the purge runs (end of the grace period, D25). Set iff status = deletion_pending.
    deletion_at           timestamptz,
    export_downloaded_at  timestamptz,
    CONSTRAINT users_deletion_at_iff_pending
        CHECK ((status = 'deletion_pending') = (deletion_at IS NOT NULL)),
    CONSTRAINT users_active_is_approved
        CHECK (status NOT IN ('active', 'disabled', 'deletion_pending') OR approved_at IS NOT NULL)
);
CREATE INDEX users_status ON users (status, created);

CREATE TABLE invites (
    id         uuid PRIMARY KEY,
    token_hash bytea NOT NULL UNIQUE,
    role       text NOT NULL CHECK (role IN ('admin', 'member')),
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created    timestamptz NOT NULL,
    expires    timestamptz NOT NULL,
    used_at    timestamptz,
    used_by    uuid REFERENCES users (id) ON DELETE SET NULL
);

-- Survives user deletion on purpose: no foreign keys.
CREATE TABLE audit_log (
    id       uuid PRIMARY KEY,
    actor_id uuid,
    action   text NOT NULL,
    target   text NOT NULL,
    at       timestamptz NOT NULL
);
CREATE INDEX audit_log_at ON audit_log (at DESC, id DESC);

-- Scheduler hints: which users have queued jobs and the earliest run_after. Holds no content,
-- only user IDs and a timestamp, so the fair job runner can find work without an unscoped read
-- of `jobs` (which RLS would hide). Maintained by a trigger on `jobs` (see app-state migration).
CREATE TABLE job_wakeups (
    user_id   uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    run_after timestamptz NOT NULL
);
CREATE INDEX job_wakeups_run_after ON job_wakeups (run_after, user_id);

REVOKE ALL ON users, invites, audit_log, job_wakeups FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE, DELETE ON users, invites TO strata_accounts;
GRANT SELECT, INSERT ON audit_log TO strata_accounts;
-- strata_app: non-secret user columns (no password hash), audit appends (purge job), wakeups.
GRANT SELECT (id, username, display_name, role, status) ON users TO strata_app;
GRANT INSERT ON audit_log TO strata_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON job_wakeups TO strata_app;

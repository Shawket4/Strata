# Strata — Architecture

`PLAN.md` is the spec; this file records how it is built. Sections are added by the component that owns them.

## Database, roles and RLS

Implemented in `backend/crates/index` (`strata-index`); spec: PLAN §5.2, §7.4, L22, D21.

### Layout
- One PostgreSQL database (`strata` in production). All Strata tables live in schema **`strata`**, owned by `strata_owner`. The `vector` (pgvector) and `pg_trgm` extensions live in `public`. Every role's `search_path` is `strata, public`: it is set per database by bootstrap (`ALTER ROLE … IN DATABASE … SET search_path`) and also pinned by `strata_index::pool::connect_options`.
- Migrations are forward-only SQL files in `backend/crates/index/migrations/`, embedded with `sqlx::migrate!` (`strata_index::MIGRATOR`) and run by `strata_owner` (`strata_index::migrate`). Re-running is a no-op.

### Bootstrap (superuser, before migrations)
`strata_index::bootstrap` (or its psql form, `bootstrap_script(db, passwords)`):
1. `ensure_roles` creates the three cluster-wide roles if they are missing and re-asserts `LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION`. It runs in one transaction that holds a cluster-wide `SHARE UPDATE EXCLUSIVE` lock on `pg_authid`, and it only runs `ALTER ROLE` when a role's attributes differ. Without this, concurrent callers failed with "tuple concurrently updated"; a regression test covers it.
2. `prepare_database(conn, db)` (connected to `db`): `REVOKE ALL ON DATABASE … FROM PUBLIC`, `CONNECT` for the three roles, creates the extensions (pgvector is not a trusted extension, so the superuser must do it), creates the `strata` schema owned by `strata_owner`, grants `USAGE` to the app and accounts roles, and sets each role's `search_path`. Everything is parameterised by database name (identifiers quoted) and idempotent.

### Roles
| Role | Purpose | Privileges |
|---|---|---|
| `strata_owner` | owns the schema, runs migrations | owner of every table; **also bound by RLS** (`FORCE`) |
| `strata_app` | all request and job work | `NOBYPASSRLS`, owns nothing; DML on user-owned tables; `SELECT (id, username, display_name, role, status)` on `users` (no password hash); `INSERT` on `audit_log`; DML on `job_wakeups`; the limited bridge grants below |
| `strata_accounts` | login, signup, admin user management | DML on `users`, `invites`; `SELECT, INSERT` on `audit_log`; DML on the bridge tables; **no grants on any other user-owned table** (permission denied, SQLSTATE 42501) |

### Table classes
- **Global** (no RLS; allow-list `schema_audit::GLOBAL_TABLES`): `users` (every status plus approval, deletion and export fields), `invites`, `audit_log` (no foreign keys, so it outlives purged users), `job_wakeups`, `_sqlx_migrations`.
- **User-owned** (everything else, currently 34 tables): `user_id uuid NOT NULL` is the first primary-key column, RLS is `ENABLE`d and `FORCE`d, and the table has exactly one policy `strata_user_isolation … USING (user_id = strata_current_user()) WITH CHECK (user_id = strata_current_user())`. Migrations apply this with `SELECT strata_make_user_owned('<table>')`. Foreign keys between user-owned tables are composite and include `user_id`, so a row can never reference another user's row. Every user-owned row cascades from `users(id)`.
- **Account bridge** (`devices`, `sessions`, `refresh_tokens`; `strata_make_account_bridge`): user-owned as above, plus one extra policy `strata_accounts_access TO strata_accounts USING (true) WITH CHECK (true)`. **Why:** PLAN §7.4 lists devices and sessions as per-user state, but login must create them and refresh must find a session by token hash before any user is known, and the accounts role has no user scope. The app role keeps scoped access for `GET /devices`, push registration, `DELETE /devices/{id}` and logout: `devices` SELECT/UPDATE/DELETE, `sessions` SELECT/UPDATE, and nothing on `refresh_tokens`. Sessions belong to devices (composite foreign key, `ON DELETE CASCADE`), and refresh tokens belong to sessions.

`strata_current_user()` returns `NULLIF(current_setting('strata.user_id', true), '')::uuid`. After a transaction-local setting ends, Postgres reports `''`, not NULL, for the rest of the session. `NULLIF` makes both cases NULL, so `user_id = NULL` matches **zero rows** and never all rows. A malformed value raises an error (22P02) instead of widening access.

### Schema (PLAN §7.4, plus these additions)
Migrations `…001` foundation/global, `…002` account bridge, `…003` notes (`notes` with a `search tsvector` column and GIN index, `aliases`, `tags`, `links`, `relations`, `rejected`, `blocks`, `chunks` with `embedding vector(384)`, an HNSW cosine index and a per-row model), `…004` entities (`entities`, `entity_aliases` with a trigram GIN index, `mentions`, `clusters`, `cluster_names`, `places`, `documents`, `custody_events`, `disambiguation_hints`), `…005` tasks (`tasks`, `task_reminders`, `notification_log` with `UNIQUE (user_id, task_id, remind_at, device_id)`), `…006` app state (`jobs`, `suggestions`, `suggestion_replies`, `ai_decisions`, `ai_usage`, `settings`, `sync_epochs`, `change_log`, `idempotency`, `dedupe_keys` with a trigram GIN index, `dedupe_keep_both`).

Decisions not spelled out in the plan:
- **Change log sequencing.** `change_log.seq` is not a global `bigserial`. Each append bumps `sync_epochs.last_seq` for the user and holds that row lock until commit. Seqs are therefore gapless and in commit order per user. With a bigserial, a reader could pass seq *n+1* while *n* was still uncommitted and skip it forever. `bump_epoch` increments the epoch, keeps seq monotonic, and deletes the old epoch's log.
- **Job discovery.** `jobs` is user-owned, so an unscoped worker sees nothing. `job_wakeups(user_id, run_after)` is a global table with no content, only user IDs and a time. A trigger on `jobs` keeps it current, so the fair runner can find users with due work (`AppDb::due_job_users`). It then claims inside that user's scope with `FOR UPDATE SKIP LOCKED`. `refresh_wakeup` locks the hint row before recounting, which makes it race-free with concurrent enqueues. Debounce: a unique partial index on `(user_id, kind, dedupe_key) WHERE status = 'queued'`; re-enqueueing moves `run_after`.
- `mentions.block_id` is `''` for note-level mentions, because primary-key columns cannot be NULL.
- `tasks.id` and `dedupe_keys.item_id` are text: task identity is the `^t-<ulid>` block ID.
- `dedupe_keep_both` stores pairs ordered with `a_id < b_id COLLATE "C"`, which is byte order and matches Rust `str` ordering.
- `suggestions.status` adds `superseded`, used when the AI re-proposes after a threaded reply (§9.8). `ai_decisions` is the log the correction loop reads.
- Money is stored as integer micro-USD (`ai_usage.est_cost_micros`).
- Known performance caveat: `pg_trgm`'s `%` and the full-text `@@` operators are not leakproof. Under RLS the planner may therefore evaluate the `user_id` qual first rather than use the GIN index. At the target scale (about 10k notes per user) this is a filtered scan of one user's rows.

### Enforcement (tests)
- `schema_audit::audit` enumerates `pg_catalog` and reports any non-allow-listed table that lacks `user_id` first in the primary key, lacks RLS enabled or forced, lacks the exact standard policy, has any other policy (bridge tables may carry exactly the accounts policy), or grants anything to `PUBLIC` or `strata_accounts`. The CI test asserts an empty result, and a negative test proves each kind of mistake is caught.
- RLS suite (`backend/crates/index/tests/rls.rs`) seeds every user-owned table for two users and checks, for every table enumerated from the catalog:
  - the app role unscoped sees zero rows;
  - the owner unscoped sees zero rows;
  - a scope sees exactly its own rows;
  - an insert or update with the other user's `user_id` fails the `WITH CHECK` (42501);
  - `strata_accounts` gets permission denied on every statement;
  - a pooled connection never carries a scope past its transaction;
  - purging a user removes exactly their rows.

## UserScope

Spec: PLAN §5.2 ("`UserScope` = a transaction"), principle 7. Code: `strata_index::scope`.

```
AppDb::new(strata_app pool) ──► (AppDb, ScopeIssuer)        composition root (stratad)
ScopeIssuer::issue(user_id) ──► UserScope                   auth middleware / job runner only
AppDb::begin(&UserScope)    ──► ScopedTx                    handlers, jobs
repo::*::fn(&mut ScopedTx, …)                               data layer
```

- **`ScopeIssuer`** is the capability to mint scopes. It has a private field, so the only way to get one is `AppDb::new`. The composition root gives it to the auth middleware, which resolves the session once per request, and to the job runner, which scopes each job to its owner. Nothing else holds it.
- **`UserScope`** is `Copy` proof of one authenticated user. Its field is private and it has no public constructor. `compile_fail` doctests prove that code outside the crate cannot build a `UserScope` or a `ScopeIssuer`.
- **`ScopedTx`** wraps `sqlx::Transaction<'static, Postgres>`. `AppDb::begin` runs `SELECT set_config('strata.user_id', $1, true)` and verifies that the returned value matches. Because the setting is transaction-local, commit, rollback, or drop without commit all clear it, and a pooled connection can never leak it. A test on a one-connection pool checks all three cases and confirms that the next checkout of the same backend PID sees no scope and zero rows.
- **Repositories** (`strata_index::repo::{notes, graph, entities, tasks, sync, jobs, suggestions, dedupe, settings, devices}`) take `&mut ScopedTx` and never take a raw user ID. Inserts write `user_id = strata_current_user()`, so a row always belongs to the scope. RLS filters every read and write, so a foreign ID behaves like a missing one: `None` or `false`, which the API maps to 404.
- `ScopedTx::conn()` exposes the connection for queries that no repository covers yet. RLS still applies. Never run a session-level `SET strata.user_id` on it.
- Account endpoints use **`AccountsDb`** (`strata_accounts` pool), which has no path to vault data: users, invites, the audit log, devices, sessions, and refresh-token rotation with reuse detection (a replayed token revokes the session).
- Tests get all of this from `strata-testkit`: `TestDb` (a fresh database cloned from a migrated template, a pool per role, `app_db`, `issuer`, `accounts_db`, a `FakeClock`, deterministic ULIDs), `TestUser`, and `TempDataRoot`.

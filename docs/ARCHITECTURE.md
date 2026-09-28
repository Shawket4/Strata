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
- **User-owned** (everything else, currently 35 tables): `user_id uuid NOT NULL` is the first primary-key column, RLS is `ENABLE`d and `FORCE`d, and the table has exactly one policy `strata_user_isolation … USING (user_id = strata_current_user()) WITH CHECK (user_id = strata_current_user())`. Migrations apply this with `SELECT strata_make_user_owned('<table>')`. Foreign keys between user-owned tables are composite and include `user_id`, so a row can never reference another user's row. Every user-owned row cascades from `users(id)`.
- **Account bridge** (`devices`, `sessions`, `refresh_tokens`; `strata_make_account_bridge`): user-owned as above, plus one extra policy `strata_accounts_access TO strata_accounts USING (true) WITH CHECK (true)`. **Why:** PLAN §7.4 lists devices and sessions as per-user state, but login must create them and refresh must find a session by token hash before any user is known, and the accounts role has no user scope. The app role keeps scoped access for `GET /devices`, push registration, `DELETE /devices/{id}` and logout: `devices` SELECT/UPDATE/DELETE, `sessions` SELECT/UPDATE, and nothing on `refresh_tokens`. Sessions belong to devices (composite foreign key, `ON DELETE CASCADE`), and refresh tokens belong to sessions.

`strata_current_user()` returns `NULLIF(current_setting('strata.user_id', true), '')::uuid`. After a transaction-local setting ends, Postgres reports `''`, not NULL, for the rest of the session. `NULLIF` makes both cases NULL, so `user_id = NULL` matches **zero rows** and never all rows. A malformed value raises an error (22P02) instead of widening access.

### Schema (PLAN §7.4, plus these additions)
Migrations `…001` foundation/global, `…002` account bridge, `…003` notes (`notes` with a `search tsvector` column and GIN index, `aliases`, `tags`, `links`, `relations`, `rejected`, `blocks`, `chunks` with `embedding vector(384)`, an HNSW cosine index and a per-row model), `…004` entities (`entities`, `entity_aliases` with a trigram GIN index, `mentions`, `clusters`, `cluster_names`, `places`, `documents`, `custody_events`, `disambiguation_hints`), `…005` tasks (`tasks`, `task_reminders`, `notification_log` with `UNIQUE (user_id, task_id, remind_at, device_id)`), `…006` app state (`jobs`, `suggestions`, `suggestion_replies`, `ai_decisions`, `ai_usage`, `settings`, `sync_epochs`, `change_log`, `idempotency`, `dedupe_keys` with a trigram GIN index, `dedupe_keep_both`), `…007` vault store (`integrity_warnings`; `dedupe_keys` gains `key_no` in the primary key, `phonetic_key` and the serialized `item`, so an item stores one row per name — see "Vault store"), `…010` account columns (`devices.reminders_enabled`, `users.must_change_password`; the data move runs per user inside a transaction-local scope because forced RLS binds the migration role too). `…009` is reserved for sync/events.

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
- Account endpoints use **`AccountsDb`** (`strata_accounts` pool), which has no path to vault data: users, invites, the audit log, devices, sessions, and refresh-token rotation with reuse detection (a replayed token revokes the session). `AccountsDb::begin` returns an `AccountsTx`: login's device, session and refresh-token writes (plus a rehash) and a refresh rotation's spend, device touch and new token are each one transaction, so a failure part-way leaves no orphan rows and an unrotated token stays valid (tested by injecting a failing trigger).
- Tests get all of this from `strata-testkit`: `TestDb` (a fresh database cloned from a migrated template, a pool per role, `app_db`, `issuer`, `accounts_db`, a `FakeClock`, deterministic ULIDs), `TestUser`, and `TempDataRoot`.

## Auth

Spec: PLAN §5.2, §7.5 "Auth & devices" and "Account & admin", §8, §15, D6 = b, D22 = b, D25 = a. Code: `strata_api::auth` (middleware, tokens, services), `strata_api::routes::{auth, me, devices, admin}` (endpoints), `stratad` (composition root, startup checks, background tasks).

### Tokens
- **Access tokens** are JWS compact tokens, `alg: EdDSA` (Ed25519, RFC 8037), signed with `jsonwebtoken` (RustCrypto backend). Claims: `iss`, `aud`, `sub` (user), `sid` (session), `did` (device), `role`, `st` (account status: `active` or `deletion_pending`), `iat`, `exp`, `ver` (claims layout, currently 1). The header carries `kid` = the first 16 hex digits of SHA-256 of the public key. Lifetime 15 minutes (`auth.access_token_ttl_secs`). Verification checks algorithm, `kid`, signature, issuer, audience and version, and checks expiry against the injected `Clock` (never system time), so tests drive it with the fake clock.
- **The key** is an Ed25519 private key in PKCS#8 PEM (RFC 8410) at `auth.signing_key_file`, created by `stratad keygen` (mode 0600) and loaded at startup; `openssl genpkey -algorithm ed25519` produces the same format. Rotating it invalidates access tokens only; refresh tokens keep working, so devices recover within one refresh.
- **Refresh tokens** are 256 random bits (base64url). Only SHA-256 hashes are stored (`refresh_tokens`). Each is single-use: `POST /auth/refresh` spends it and returns a new pair for the same session. Presenting a spent token again is reuse: `AccountsDb::use_refresh_token` revokes the whole device session in the same transaction, and the handler adds the session to the revocation set. Sessions have an absolute lifetime (`auth.session_ttl_days`, default 90); a refresh re-reads the user and revokes a session its account can no longer hold (disabled, or an export-only session after a cancelled deletion).
- The `role` claim is informational: admin endpoints re-read the caller's row (`role = admin` and `status = active`) on every request, so a demotion applies on the next request and a promotion without re-login.

### Middleware and `UserScope`
`auth::middleware::authenticate` wraps the whole `/api/v1` scope (registered in `app::api_v1`). For a request with `Authorization: Bearer …` it verifies the token, checks the revocation set, applies the account restrictions below, mints the `UserScope` **once** through the `ScopeIssuer` (a private field of `Authenticator`, so no handler can mint a scope) and stores an `AuthContext` in the request extensions. Handlers take the `Authenticated` extractor (`401 unauthorized` when absent) and open transactions with `app_db.begin(auth.scope())`. Requests without a token pass through, so public endpoints (health, signup, login, refresh) need nothing special; an invalid, expired or revoked token is `401` on any route.

Restrictions:
- **Export-only session** (token `st = deletion_pending`, or the user is flagged `deletion_pending`): only `GET /me`, `GET /me/export`, `POST /me/confirm-deletion` and `POST /auth/logout`; everything else — including routes added later by other modules and unknown paths — is `403 account_deletion_pending`.
- **Temporary password** (admin reset): only `GET /me`, `PATCH /me` and logout; everything else is `403 password_change_required` until the user sets a new password.
- Every secured operation documents `401` and `403` (`#/components/responses/Restricted`) in the contract; the OpenAPI conventions add them automatically.

### Immediate revocation (single process)
The revocation set (`auth::revocation::RevocationSet`) holds revoked session IDs, per-user flags (`disabled`, `deletion_pending`, `must_change_password`) and purge tombstones. It is loaded at startup and reloaded every `auth.revocation_reload_secs` (default 30) from `sessions` (revoked, unexpired) and `users` (disabled, deletion pending, temporary password). The request that changes an account updates the set synchronously, so the change holds from the very next request:
- blocking changes (disable, deletion scheduling, password reset) update the set **before** the database write, so a failure fails closed; unblocking changes (enable, cancel deletion, password changed) update it after the write commits;
- a local change made while a reload is in flight wins over that reload's snapshot (per-user change versions), so a stale snapshot never resurrects access;
- revoked sessions are remembered at least one access-token lifetime past their revocation (device removal deletes the session rows, so the database alone would forget them); purged users get a tombstone for the same period.

**Design constraint: one `stratad` process.** Strata runs one process per deployment (one VPS, one systemd unit, PLAN §14). The revocation set and the rate limiters are in-process. With several processes a revocation made by one would reach the others only at their next reload (≤ `revocation_reload_secs`), and rate limits would be per process; running more than one process requires a shared revocation channel (e.g. `LISTEN/NOTIFY`) first.

### Accounts
- **Usernames** (`auth::username`): NFKC case folding (`NFKC(case_fold(NFKC(s)))`), then 3–32 letters/marks/digits of any script plus `.`, `_`, `-`, starting with a letter or digit. `users.username_normalized` stores the UTS #39 confusable **skeleton** (`unicode-security`) of the folded form, so `ahmed`, `AHMED`, `ＡＨＭＥＤ` and `аhmed` (Cyrillic `а`) collide (`409 username_taken`), and login looks accounts up by skeleton.
- **Passwords**: Argon2id (`argon2`), cost from `auth.argon2` (default 19 MiB, 2 passes, 1 lane); verification uses the parameters in the stored PHC string and login rehashes when the configured cost changed. Unknown usernames spend the same hashing work against a dummy hash. An admin reset sets `users.must_change_password` (migration `…010`; it replaced the former `temporary:` hash prefix, migrating and stripping existing prefixed hashes); the flag drives the password-change restriction and is cleared when the user sets a password. A password change needs the current password and signs out every other session.
- **Sign-up** (D22) creates a `pending` account: no vault, no session; login answers `403 account_pending` / `account_rejected` / `account_disabled` only after the password verified (a wrong password is always `401 invalid_credentials`). **Approval** creates `<data_root>/users/<id>/vault` and `git init`s it through the `VaultProvisioner` trait (`GitVaultProvisioner` is the basic implementation; the vault store may supply its own). Admin-created accounts (`POST /admin/users`, `stratad create-user`) are active with a vault at once.
- **Rate limits** (`auth::rate_limit`): exact sliding windows with the injected clock — login per client IP and per username skeleton, sign-up per IP and globally, plus the pending-account cap (`accounts.max_pending_signups`); `POST /capture` and `POST /ask` per user (`capture_per_user`, `ask_per_user`; the sync push's `capture` op is not limited, so an offline outbox always drains). Refusals are `429 rate_limited` with `Retry-After`. The client IP is the socket peer, or the forwarded address when `auth.trust_forwarded_for` (behind nginx only).
- **Audit log**: every admin action writes one entry (`user.create`, `user.approve`, `user.reject`, `user.disable`, `user.enable`, `user.role.admin|member`, `user.password_reset`, `user.delete.schedule`, `user.delete.cancel`), sign-up writes `user.signup`, the purge writes `user.purge` (actor NULL).
- **Devices**: listing, renaming and removal run in the caller's scope, so another user's device ID is `404`. Removing a device deletes its sessions and refresh tokens (cascade) and revokes them in memory. `reminders_enabled` (D27) is the `devices.reminders_enabled` column (default `true`; migration `…010` moved the former `device.<id>.reminders_enabled` settings rows into it and deleted them).
- **Settings** (`GET/PATCH /me`): UI language, timezone (IANA, validated) and free-form preferences are MessagePack values in the user-owned `settings` table, read and written in the caller's scope; display name and password live in `users` (accounts role).

### Deletion (D25)
`DELETE /admin/users/{id}` sets `deletion_pending` with `deletion_at = now + accounts.deletion_grace_days`, flags the user in the revocation set and revokes every session. The user can still log in; the session is export-only. `GET /me/export` streams a zip of the caller's own vault (path from the `UserScope`, never from the request; `.git/` and symlinks excluded) from a blocking writer through a bounded channel, and records `export_downloaded_at` before the response ends; admins only see that timestamp and have no route to the export. `GET /admin/settings` gives admins the grace period (`deletion_grace_secs`) so Admin → Users can show the purge date before a deletion is scheduled. `POST /admin/users/{id}/cancel-deletion` restores `active` and ends the export-only sessions. `POST /me/confirm-deletion` brings `deletion_at` forward and purges at once. The purge job (`purge_due_accounts(now)`, run every `accounts.purge_interval_secs` by `stratad`) tombstones the user in the revocation set, removes `<data_root>/users/<id>` (renamed aside first, then deleted), then in one transaction deletes the `users` row — every user-owned row cascades from it — and appends one `user.purge` audit entry.

### `stratad`
`serve` refuses to start unless every configured secret file (signing key, AI key, push credentials) is a regular file with no group/other permissions, the key loads, the data root exists, and the database is `UTF8` with an `LC_CTYPE` other than `C`/`POSIX` (PLAN §14: `pg_trgm` must treat Arabic letters as word characters). It opens one pool per role (`strata_app` → `AppDb` + `ScopeIssuer`, `strata_accounts` → `AccountsDb`), loads the revocation set, starts the reload and purge tasks, and serves until SIGINT/SIGTERM (graceful, 30 s). Logs are JSON (`tracing`); request logs carry method, path without query (long segments elided), status and duration — never headers, bodies or queries. The vault store is the auth layer's provisioner and is reconciled for every active user in the background at startup (see "Vault store"). Other subcommands: `keygen`, `create-user [--admin]` (password on stdin), `verify --user` / `reindex --user` (user ID or username; run with the server stopped), `openapi`, `migrate` (as `strata_owner`), `bootstrap-roles` (apply as superuser, or `--print` the SQL). `check-config` validates the configuration and prints every effective setting (database passwords masked) without connecting anywhere. Operations are described in `docs/RUNBOOK.md`.

Configuration (`strata_common::Config`, loaded by `config::env`): one `STRATA_` variable per setting, named after its path with `__` between the parts (`STRATA_DATABASE__APP_URL`); defaults, then the env file (`--env-file` / `STRATA_ENV_FILE`, else `./.env`; parsed by `dotenvy` into values without touching the process environment), then the process environment. Each variable has a typed parser and renderer (a table built by a macro from the field path, so names cannot drift from the struct); `Config::validate` keeps the range and cross-field rules, and every message names variables, never values. Secrets stay files referenced by path and checked for permissions at startup.

## Shared crates

Spec: L16, PLAN §5.1. Pure Rust crates under `/crates` used by both the backend and the client core: no I/O, no async, no clock, no randomness except explicitly seeded. `vault-format`, `text-normalize` and `domain` are documented in `docs/VAULT_FORMAT.md` and their module docs; this section covers `sync-model`, `item-render`, `graph-algo` and `dedupe`.

### `sync-model` (§7.5 Sync, §12.3–12.5, D19)
- **Ops.** `SyncOp { op_id, entity_id, base_version, op }`, where `op` is the adjacently tagged `Op` (`{kind: "note.update", payload: {…}}`) covering every §7.5 mutation: `note.create/update/move/delete`, `capture`, `relation.add/remove/retype`, `suggestion.accept/reject/reply`, `entity.create/patch/merge`, `document.create/patch/custody`, `place.create/patch`, `task.create/update/complete/cancel/reopen/delete`, `relink.request`, `device.settings`. The outbox stores the same parts as columns (`Op::kind`, `Op::payload_bytes`, `Op::from_parts`). `SyncOp::validate` enforces that `entity_id` is derived from the payload, that `base_version` is present exactly for edits of existing content (`OpKind::requires_base_version`), and `force` exists only on duplicate-checked creates. Task edits use the version of the task **line** (`apply::task_line_version`), so edits to other lines of the same note never conflict. `task.update` uses patch semantics (absent = unchanged, `nil` = clear). `task.complete` carries a client-generated `next_id` so both sides write the same next occurrence. Entity/document/place patches carry `set_lists` (whole list values; one value of a user field is written as a scalar) and `document.custody` an optional user `note` (last on the custody line). Every op that makes a note carries the device's creation time `created` (UTC, required; `Op::created`): `note.create`, `capture`, `entity/document/place.create`, `task.create` (plus `home_id`, the ID of a `tasks/Tasks.md` it makes) and `suggestion.accept` (notes the acceptance creates).
- **Suggestion payloads** (`suggestions`). The schema of every suggestion kind the server creates, stored and synced as the named-map MessagePack of the kind's struct (the kind is the record's `kind`): `duplicate` `DuplicatePayload{candidates: [DuplicateItem]}`, `duplicates` `DuplicatesPayload{a, b, reason}`, `filing` `FilingPayload`, `entity_link` `EntityLinkPayload`, `custody` `CustodyPayload` (participants `CustodyTarget`), `task` `TaskPayload`, `correction` `CorrectionPayload` (`CorrectionFix`, `CorrectionHint`), `conflict` `ConflictPayload`. `SuggestionPayload` is the typed union (internally tagged `{type: "<kind>", …}` as a value of its own): `decode(kind, bytes)` (unknown kind or bad bytes → `PayloadError`, shown generically, never dropped), `to_bytes()`, `SuggestionRecord::decode_payload`. Kind names are `suggestions::kinds`. The backend writes and reads only these types; the contract's `SuggestionPayload` (`routes::inbox`) is built from them in one place (`conflict` stays `opaque` there). Reply threads are on the record: `SuggestionReplyRecord{id, text, at, author: user|ai}` (`author` absent from older servers = `user`).
- **Results.** `OpResult` = `applied{new_version, merged}` | `conflict{server_version, resolution: conflict_copy{…} | server_kept{reason}}` | `duplicate{candidates}` (dedupe's `DuplicateCandidate`) | `rejected{problem}`. `PushResponse::check_answers` verifies one result per op, in order.
- **Pull.** `ChangeRecord { seq, epoch, entity_type, entity_id, version, change: upsert{record} | delete }`, `BootstrapPage`, `ChangesPage`. Records exist only for state that is not in note text (relations with provenance, rejections, suggestions, clusters, settings, device settings, keep-both pairs); everything derivable from a note (tags, links, frontmatter relations, entity/document/place fields, custody state, tasks) is derived by the client with `vault-format`, exactly like the indexer. `SyncCursor::advance` rejects a page as a whole on epoch change (→ re-bootstrap), out-of-order seqs, a `next_seq` behind the last change, or a header that does not match its record.
- **Versions.** `Version` = `sha256:<64 hex>` of the exact bytes (BOM and line endings included), the same spelling as `If-Match` and the sidecar `content_hash`.
- **3-way merge (D19).** `merge(base, ours, theirs) -> MergeOutcome::{Clean(text), Conflicted{merged_with_markers, hunks, auto_resolved, template}}`:
  - Frontmatter is merged key by key on top of ours' parsed frontmatter (untouched entries keep their bytes): one-sided changes apply (deleted keys stay deleted); known list keys (`aliases`, `tags`, every relation key) merge as sets with deletions respected (`(o ∩ t) ∪ (o \ b) ∪ (t \ b)`, ours' order then theirs' additions); `updated` takes the later instant; any other key changed differently on both sides is a conflict hunk and the merged file keeps ours' value. Invalid YAML falls back to a whole-file line merge.
  - The body is merged with diff3 built on `similar`'s Myers diff (line level). Changes overlap when their base ranges intersect, when an insertion falls strictly inside the other side's change, or when both insert at the same point; an insertion exactly at the boundary of the other side's change is not a conflict (its place is unambiguous). Identical changes collapse; pure-insertion conflicts are trimmed to the lines that differ. The grouping is independent of side labels, so clean merges commute.
  - Line endings: a side that converted the whole file between LF and CRLF is diffed in the base style and the conversion is re-applied to the result; mixed files compare byte-exactly. A missing final newline is merged as a separate three-way flag.
  - Task lines are atomic (line-level merging never splices within a line; a toggle vs a text edit is a `task_line` conflict). A clean result that would contain a task block ID more often than either side (both sides moved the task to different places) turns those placements into `task_placement` conflicts.
  - `merged_with_markers` has git-style diff3 markers in the body and is `None` when a frontmatter key conflicts (markers would break the YAML); `Conflicted::resolve(&[(hunk_id, Choice)])` rebuilds the file from per-hunk choices (`ours`, `theirs`, `base`, `ours_then_theirs`, `theirs_then_ours`, custom text, or a frontmatter value).
  - `decide_update(base_version, base, current, edit)` is the shared D19 policy: fast-forward, already applied, merged, or conflict (the server then writes a conflict copy).
  - Known diff3 behaviour (as in git): a move is a delete plus an insert, so a line one side moved and the other deleted reappears at its new place.
- **Apply rules** (`apply`): relation add/remove/retype on the source frontmatter (empty relation keys are removed), entity/document/place patches (`id`/`kind` immutable; relation lists, `aliases`, and the custody fields `location`/`holder`/`last-holder`/`status` are refused because they change through their own ops), custody events (the `## Custody` section is rewritten newest first, created before `## Notes` if missing, and the frontmatter recomputed from all events; unparseable custody lines abort instead of being dropped), and task edits by block ID via `vault-format`'s task functions.

### `item-render` (§6.4, §6.6, §6.7, §6.9, §6.11, §6.12, D19)
How new items are rendered, called by the vault store when it writes and by the client core's optimistic apply (`store::write::apply_to_note`), so for the same input both write the same bytes. Pure; callers pass IDs, timestamps and the paths already taken. **Times are UTC**: the functions take `DateTime<Utc>` and write whole seconds with `Z`.
- **Paths** (`paths`): capture `inbox/YYYY-MM-DD-HHmmss.md` in UTC; entity/document/place/concept `<kind folder>/<sanitised name>.md`; a taken name gets ` 2`, ` 3`, … (only `.md` notes directly in the folder count, case-insensitively); conflict copies `<stem> (conflict YYYY-MM-DD HHmmss[ N]).md` in UTC on both sides (the server's copies and the device's fallback); inbox membership is a file directly in `inbox/` (index, jobs and the device use this one rule).
- **Notes** (`note`, `capture`, `entity`): the `id`/`created`/`updated` stamp, `new_note` (a `note.create`: `id`, `created` unless the content has one, `updated` = the creation time) and `with_id`; a capture is `id` + `created` + the text ending in a line break; an entity note is `kind`, `title` (the **title rule**: whenever the stem of the path it is written at differs from the name — `Ahmed 2.md` gets `title: Ahmed` — for user and AI creates alike), cleaned `aliases`/`tags`, user fields, `part-of`, `created`/`updated` = the op's creation time, then user relations, over a `## Notes` body; concepts get `## Summary` with the summary as one paragraph. `EntitySpec::from_{entity,document,place}_create` and `document_fields`/`document_relations`/`place_fields` turn op payloads into specs (the push handler uses the same conversions); `EntitySpec::path(taken)` and `render(id, path, created)`.
- **Tasks** (`task`): block IDs `t-<lower-case ULID>`, the text of an accepted task suggestion (title + missing entity links) and its preview line, `heading_date` (the creation time's date in the account's time zone) and `stamp_home` (a new `tasks/Tasks.md` gets the op's `home_id` and creation time; an existing home only gains a missing `created`). Creating a task is `sync_model::apply::apply_task_create` on both sides (`## <Month> <YYYY>` headings in `tasks/Tasks.md`, VAULT_FORMAT §9.5).
- Every input comes from the op (IDs, the device's creation time, `home_id`), so only names taken on the server but not yet pulled can differ until the next pull replaces the base. `client/core/tests/render_parity.rs` runs every create and custody op on a real vault and on the device and compares the files byte for byte, with no adjustment.

### `dedupe` (§9.7)
- Both sides generate candidates from their own store — the backend with SQL (`dedupe_keys` equality, `pg_trgm` `similarity() >= trigram_floor`, transliteration keys), the client from SQLite — using `CandidateQuery::for_item`, then call `check(new, candidates, thresholds, keep_both)` for the final decision, so the offline prompt and the server's `duplicate{}` agree. Stored items keep `Item::keys()` (`exact_keys`, `trigram_texts`, `phonetic_keys`, one entry per name).
- **Exact**: equal key. The key is `text-normalize::dedupe_key` of the prepared text plus light English plural folding (not for people/companies); tasks append the canonical RRULE (parts sorted) and the normalised linked entities. **Near**: `pg_trgm` similarity of prepared texts ≥ the stricter near threshold of the two kinds (`domain::DedupeThresholds`, overridable per kind), or — people, companies, aliases — equal transliteration keys (≥ 3 consonant classes) confirmed by spelling similarity ≥ 0.75 (same script) or scored 0.75 across scripts. **Semantic**: cosine supplied by the backend ≥ the confirmed threshold, or ≥ the candidate threshold with an LLM confirmation; unconfirmed borderline candidates are returned in `needs_confirmation`.
- Captures and tasks drop a task-intent preamble ("remind me to", "don't forget to", "فكرني", …) before comparing, so "remind me to make Watanya's invoice" matches the monthly task "Make Watanya's ETA invoice" (near, 0.84).
- Compatible kinds: notes ↔ notes, captures; captures ↔ captures, notes, tasks; tasks ↔ tasks, captures; entity kinds ↔ their own kind; aliases ↔ every named kind.
- Keep-both pairs (`KeepBoth`, stored ordered by byte order like `dedupe_keep_both`) suppress a match when recorded under either item's kind; `KeepBoth::for_forced_create` lists the pairs to record on `force`. Ranking: level (exact, near, semantic), then score descending, then ID. `sweep` finds pairs within a blocked candidate set for the nightly job.

### `graph-algo` (§10, D10, D3/D4)
- `Graph` (node kinds `domain::GraphNodeKind`, typed directed edges `domain::GraphEdgeKind` with `by`/confidence, CSR adjacency both ways) built from edge lists; `neighbourhood(graph, focus, depth 1..=3, filter)` with edge/node kind filters (edges followed in both directions); `co_mentions` for the entity lens (per note mentioning m lens entities, each pair gains `1/(m−1)`).
- `WeightedGraph::project` makes the undirected weighted projection for clustering and layout: user edges 3, AI edges with confidence ≥ 0.85 weight 2, other AI edges 1; similarity edges excluded by default; parallel edges summed.
- **Leiden** (`leiden`, `leiden_from`): modularity with resolution γ; fast local moving, refinement (well-connected nodes merged into well-connected refined communities, randomness θ = 0.01 on the raw weight scale), aggregation on the refined partition (falling back to the unrefined one when refinement merges nothing, which guarantees termination); whole runs repeat until stable. The seeded xoshiro256** RNG is implemented in-crate so results never change with a dependency upgrade. Communities are numbered by their smallest node.
- **Cluster matching** (`match_clusters`): greedy by Jaccard (then overlap, old ID, new index) with a minimum Jaccard of 0.25; splits keep the ID on the better match, merges keep the best-matching ID and retire the rest; new clusters get IDs from the caller's injected generator.
- **Force layout** (`ForceLayout`): Fruchterman–Reingold forces with Barnes–Hut repulsion (θ = 0.9), a spring to the origin, displacement capped by a cooling temperature; `step()` returns positions for streaming; `warm()` keeps previous positions, places new nodes near their placed neighbours and starts at 0.25·k; positions are clamped to a bound and never NaN. `energy()` gives the potential of the force field for tests.
- **Radial layout** (`radial_layout`): focus at the origin, rings by BFS depth, wedges proportional to BFS-subtree leaves, children ordered by a barycenter sweep that is undone whenever it does not reduce edge crossings (never worse than node order).
- Benchmarks (`cargo bench -p graph-algo`, planted-partition graph with 10k nodes / 40k edges, dev container): one force step ≈ 16 ms, a full Leiden run ≈ 110 ms, projection build ≈ 5 ms.

## AI foundation

Implemented in `backend/crates/ai` (`strata-ai`); spec: PLAN §9, L5, L18, L19, D9, D20, D23. Pure
infrastructure: the filing, linking, entity, custody and ask jobs build on it.

```
job / handler ──► AiService::complete::<T>(caller, prompt, input) ──► ProviderRouter (per username)
                    │ BudgetGuard.check ─► provider.complete_json ─► BudgetGuard.record        │
                    │ validate against the prompt's schema; invalid → retry ≤ 2 with feedback ▼
                    └─► Structured<T> { value, provider, model, prompt id+version, attempts }
                                                  ClaudeCliProvider  |  AnthropicApiProvider  |  FakeLlmProvider
```

- **`LlmProvider`** (`complete_json`, `stream`, `health`) is transport only. `complete_json`
  returns the value *with* its usage and model (`JsonCompletion`), because every call is budgeted.
  Requests (`JsonRequest`, `ChatRequest`) carry the prompt id + version, system prompt, user
  content (JSON input), schema, max tokens, and the `AiCaller` (the user's `UserScope` for budget
  writes — never a raw user ID — and the username for routing).
- **`ClaudeCliProvider`** (default, D20 = a): one `claude -p` process per call through a
  configurable launcher (production: `sudo -u strata-ai` + a root-owned wrapper, RUNBOOK §9),
  cleared environment (API keys and bare mode refused), empty scratch directory, all tools, MCP
  servers, settings files and slash commands off, Strata's prompt as the system prompt, content on
  stdin. Both call kinds use `--output-format stream-json --verbose` because only that format
  reports `rate_limit_event`s (reset times); `complete_json` adds `--json-schema` (the
  `$schema` URI is dropped: the CLI rejects draft 2020-12's) and reads `structured_output`,
  `stream` adds `--include-partial-messages` and relays top-level `text_delta`s. Usage limits
  become `Paused { until }` (event reset time, epoch in the message, or `resets 3pm (Zone)`,
  else 30 min) and no process starts until then. Timeouts SIGTERM then SIGKILL the process
  group. A semaphore (default 1) limits processes; stderr is captured (bounded) only to classify
  failures and never logged or returned.
- **`AnthropicApiProvider`** (D23 alternative): Messages API over reqwest, structured output via
  `output_config.format` with the schema reduced to what the API accepts
  (`schema::api_compatible`: numeric/length/pattern/array-size keywords are stripped and enforced
  locally instead), SSE streaming with an incremental parser, retries with backoff and
  `retry-after` via an injectable `Sleeper`; a persisting 429 becomes a pause. Cost estimates use
  per-model prices (cache writes 1.25×, reads 0.1×).
- **Validation and retries**: `AiService` compiles the request schema (jsonschema, draft 2020-12),
  validates each reply, and on a violation or non-JSON reply retries at most
  `MAX_INVALID_OUTPUT_RETRIES = 2` times, appending the violations to the user content; then
  `AiError::InvalidOutput` with content-free violation summaries. Provider failures other than
  invalid output are not retried here.
- **Budget guard**: per-user daily token/cost caps and a global token/cost cap, days in one
  budget timezone. Usage goes to `ai_usage` (user-owned, RLS) and `ai_usage_global` (global,
  per-day counters only; migration `…008`, on the audit allow-list) in one scoped transaction. A
  reached cap returns `AiError::Paused { reason: UserBudget | GlobalBudget, until: next day }`;
  jobs reschedule, never fail. The check runs before each call, so a day may exceed its cap by
  the calls already in flight.
- **`AiStatus`** (for `GET /ai/status`): provider name/model/health, the effective pause (budget
  first, then provider), `queue_depth` (filled by the job runner), today's user and global usage,
  the caps, and the embedding model.
- **Prompts**: `prompts/<id>.v<N>.md` (+ `.schema.json`), embedded and SHA-256-hashed by
  `build.rs` into `prompts::PROMPTS`; the build fails if a prompt lacks the shared language
  instruction (`prompts/_language.md`). A test pins every (id, version, hash), so a prompt edit
  without a version bump fails CI. Prompts: `inbox_filing`, `linking` (relations, concepts,
  mentions, entity relations, custody, task suggestions), `summary`, `entity_insights`,
  `custody`, `correction`, `duplicate_confirm`, `ask` (streamed markdown with `[[ref]]`
  citations, no schema), `cluster_naming`, `digest`. Typed outputs are in `outputs`; every schema
  closes every object and lists every property as required (optional values are nullable).
- **`FakeLlmProvider`** (feature `test-support`, re-exported by `strata-testkit`): replays
  fixtures keyed by (prompt id, version, SHA-256 of system + NUL + user); a missing fixture fails
  with the hash and the path to record it at; `FixtureRecorder` records from a real provider.
- **Embeddings**: `Embedder` → `Embedding { model_id, vector }`. `OnnxEmbedder` (feature `onnx`)
  runs `ort` (ONNX Runtime loaded at run time) + `tokenizers` on one worker thread at nice 19 with
  single-threaded ONNX Runtime, CLS pooling + L2 normalisation (granite-embedding r2, per its model
  card), truncation to 2048 tokens, and batching only of equal-length texts (the quint8 export is
  not padding-invariant). Each `embed` call holds the exclusive side of `CpuGate`; each `claude -p`
  process holds the shared side, so embeddings never overlap a CLI call (§9.1b).
- **Wiring (`stratad::ai::build`, run by `serve`)**: settings live in `STRATA_AI__…`
  (`DEFAULT_PROVIDER`, `USER_PROVIDERS` as `username=provider` pairs, `CLAUDE_CLI__…`,
  `ANTHROPIC_API__…`, `EMBEDDING__…`) and `STRATA_BUDGETS__…` (caps plus `TIMEZONE`, default
  `STRATA_DEFAULT_TIMEZONE`); `deploy/stratad.env.example` documents every variable with its
  default. A provider is built only when some user is routed to it
  (default: `claude_cli` for everyone, D23; the API key file is read only then, and config
  validation refuses an `anthropic_api` route without `api_key_file`). Default API model
  `claude-opus-5-5` with its $4 / $20 per MTok prices (configurable; cache reads are estimated
  at 0.1× input, above that model's actual rate). The embedder loads only when `model_dir` and
  `onnxruntime_lib` exist; otherwise, or when loading fails, embeddings are off with one warning
  naming the reason. The `AiService` is registered as app data; no handler uses it yet.

Verified against real binaries in the dev container: Claude Code 2.1.283 (flags, JSON and
stream-JSON shapes, `--json-schema` behaviour, a real summary + streamed ask with citation through
`AiService`), and the granite quint8 ONNX export with ONNX Runtime 1.30.0 (Rust vectors match the
Python reference to cosine > 0.9999). The Messages API is exercised only against a local mock with
the documented shapes.

## Vault store

Spec: PLAN §5, §6, §7.2–7.3, §7.5, §9.7. Crate `backend/crates/vault` (`strata-vault`); HTTP in `backend/crates/api/src/routes/{notes,search,inbox,relations,entities,documents,tasks,vault_ops}.rs`.

- **One writer per user.** `VaultService` (cheap to clone, `web::Data` in the app) keeps one tokio task per user fed by an unbounded `mpsc` of boxed jobs; every write runs on it, one at a time, so two writes to one vault never interleave while different users never wait for each other. Reads of files and the index run concurrently outside the actor (`ready()` only makes sure the vault was loaded and reconciled once). A job that errors or panics drops the in-memory state and sets `repair`, so the next job reloads and re-derives everything from the files (reported as `index_repaired`).
- **One write** = compute the file changes → `finish()`: (1) the write journal `.git/strata-write-journal` (`HEAD` before the write and the paths it changes, written atomically) and the atomic file writes (`.strata-tmp-*` in the same directory, `fsync`, `rename`, `fsync` the directory; excluded from git via `.git/info/exclude`); (2) in one `ScopedTx` the synchronous index update of every affected note (the changed notes plus notes whose links name a changed path), `change_log` rows, job rows (`embed`, `file_inbox`) and any rows the operation adds with them (a capture's `duplicate` suggestion); (3) the armed op receipt of a pushed op, if any (`strata_vault::receipt`: its result stored in `idempotency` in the same transaction); (4) one git commit of exactly those paths (git2, vendored libgit2, author `Strata`), with the op's result as trailers, and the journal removed; (5) the commit recorded as `sync_epochs.vault_head` and the transaction committed. Messages: `user: <op> <path>`, `ai: <job> <path>`, `system: recovered changes` / `initialize vault` / `repair sidecars`.
- **All or nothing across crashes.** A failed or interrupted write leaves the writer to reload. Before step 4 nothing is committed anywhere: on load the journal's paths are restored to their `HEAD` content (or removed), `interrupted_write_rolled_back` per path. After step 4 git has the write but the database may not: `HEAD` is ahead of `vault_head`, so reconciliation walks the commits after it (first parents, at most 10 000), re-inserts the op results recorded in their trailers (`op_result_recovered`) and re-derives what differs from the index (reported as `index_repaired`), then records `HEAD`. Commit trailers (hidden from history, `git::CommitInfo::message`):
  ```text
  user: create notes/A.md

  Strata-Op: <op ULID>
  Strata-Device: <device ULID>
  Strata-Result: <base64url, no padding, of the MessagePack result>
  ```
- **Write cost does not grow with the vault.** The commit tree is `HEAD`'s tree with exactly the written paths upserted or removed (`git2::build::TreeUpdateBuilder`), so only the trees on those paths' ancestor directories are rehashed; the git index is still updated for the same paths so the working tree stays clean (`commit_all` writes the tree before the index, so the tree cache is persisted). The writer's `VaultState` keeps the link-resolution `PathIndex` and a reverse link map (link name → notes linking to it) up to date across writes instead of rebuilding them from every file. Every statement of the index update is an index lookup of the written notes' rows: the before/after snapshots read only the keep-both pairs the written sidecars derive (not the whole table), relation lookups are one index probe per side, keep-both inserts and change-log rows are one statement per batch, and `dedupe_keys` / `dedupe_keep_both` have item-ID indexes (migration 015). `tests/write_cost.rs` guards this without timing: the index update of a write reads no relation's rows of a 450-note vault (transaction statistics), and one commit writes the same objects whatever the number of files. What still grows with the vault is the git index file (read and rewritten per commit, about 14 ms at 10 000 files) and the trigram candidate scan of the duplicate check (see the RLS caveat above).
- **Derivation** (`derive::derive`) is a pure function of path, text, sidecar and the vault's path index: note row (title, kind, `sha256:` version, search tsvector input), tags, aliases, links, relations with provenance, rejections, blocks, entity/document/place rows, custody events (IDs are deterministic ULIDs from the event text, so a re-derive is idempotent), tasks and reminders, dedupe keys. `stratad reindex` deletes every derived row and derives the whole vault again; a test asserts the result equals the incremental index row by row.
- **Optimistic concurrency.** `If-Match` carries the file version (`sync-model` `Version`); a stale one is `409 version_conflict` with `current_version`. Task edits use the version of the task **line** (`task_line_version`), so edits to other lines never conflict.
- **Timestamps.** Every time is stored and written in UTC (`…Z`, whole seconds); the user's time zone (setting `timezone`, else `default_timezone`) only dates things (task month headings, completion and custody dates, merge headings) and display. A create writes the device's creation time from the request or op as `created`/`updated` (`created` of a user decision on AI output, `AiChangeSet::created_at`; AI jobs use the server's clock) and never the receive time; a creation time more than `VaultConfig::max_future_skew_secs` (config `max_future_skew_secs`, default 300) ahead of the server's clock is `CreatedInFuture` → `422 created_in_future`, and nothing is written. `updated` changes only on user creates and full-content updates (the server's clock) — not on AI edits, task operations, entity patches or custody events — so the server writes the same bytes as `sync-model`'s client-side apply.
- **Move/rename** rewrites every link and relation that resolved to the moved paths (vault-wide, `vault-format` `rewrite`) in the same commit; `.meta/` sidecars are keyed by note ID and never move. **Delete** moves to `.trash/<path>` (still indexed, `trashed`), **restore** moves back (a free name if taken), **purge** removes file and sidecar and the index rows.
- **History and revert.** History follows renames (git2 similarity). `revert_note` writes the file as of a commit; `revert_commit` applies the inverse of a whole commit (typically `ai:`) with a git 3-way revert and, when git reports a conflict, a structured fallback: per frontmatter key and a line merge (`merge_file`) of the body, so an AI commit can be undone after later user edits to other parts of the same note. Remaining conflicts are `409 revert_conflict`.
- **Duplicates** (§9.7) use the `dedupe` crate end to end: each item stores `Item::keys()` in `dedupe_keys` (one row per name, `key_no`), candidates come from SQL (exact keys, `pg_trgm` similarity, phonetic keys), and `dedupe::check` decides. Creates of notes, tasks, entities, documents and places answer `409 duplicate_candidates` unless `force`; a forced create records keep-both pairs (`KeepBoth::for_forced_create`) in `dedupe_keep_both` and in the sidecar — notes in the standard `keep_both` list, other items (tasks, entities by alias) in an additive sidecar extension `keep_both_items: [{kind, a, b, at}]` — so the pair survives a reindex and is never flagged again. `POST /capture` is never refused: the capture is committed first and duplicates become a `duplicate` suggestion; accepting it records keep-both.
- **Relations.** User edges are frontmatter lists; AI edges add sidecar provenance in an `ai:` commit. Removing or retyping an AI edge records a rejection (sidecar + `rejected`) that `ai_add_relations` respects.
- **Entities, documents, places.** Created in `people/`, `companies/`, `documents/`, `places/`; field edits use `sync-model`'s entity patch rules; merge rewrites links to the survivor, unions aliases, moves `## Notes` under a dated sub-heading and trashes the merged note, in one commit. Custody events recorded through the API carry `by: user`: the line cites the source note when one is given and has no citation otherwise (the `vault-format` grammar allows citation-free lines for user-recorded events; AI custody content must still cite, enforced by `validate_content`). Place nesting and "documents in this place" are recursive CTEs; cycles are refused.
- **Reconciliation** (§7.3) runs when a vault is first loaded (in the background for every active user at `stratad serve` startup): removes temp files, commits uncommitted changes as `system: recovered changes`, gives notes without an ID (and tasks without a block ID) one, repairs sidecars from frontmatter (frontmatter wins; orphans removed), re-derives notes whose file differs from the index and purges notes whose file is gone; it also rolls back a journaled write that never committed and recovers the op results of commits the index never committed with (see "All or nothing across crashes"). Every finding is an `integrity_warnings` row (`GET /integrity`). `stratad verify` runs the same comparison without changing anything; `stratad reindex` rebuilds.
- **Export/import.** Export is a deterministic zip (sorted entries, fixed timestamps and permissions) of the vault with `.meta/` and `.trash/`, without `.git/`, plus `.obsidian/app.json`; export → import → export is byte-identical. Import rejects the whole archive on a symlink, absolute path, `..`, backslash, drive prefix or size limit (entries, entry, total; checked while decompressing), skips hidden entries other than `.meta/`/`.trash/` (and `.gitkeep` silently), assigns missing IDs (an entry at a path that already holds a note keeps that note's ID, whatever its frontmatter says), keeps every other byte, and is one `user: import <n> files` commit, so reverting it undoes the import. The zip is built in memory (bounded by the import limits and the 256 MiB body limit).
- **Isolation.** Every path comes from the `UserScope`; every ID lookup goes through the user's own state and RLS, so a foreign ID answers exactly like a nonexistent one (`404`). Path parameters are validated (`vault-format` path rules) before touching the disk.
- **Not yet:** semantic and hybrid search answer `503 ai_unavailable` until the AI subsystem is wired; `/entities/{id}/refresh` (AI) is Phase 4.

## Client core

`client/core` (`strata-core`) is the client's logic (PLAN §12, L15): storage, sync, search, graph, auth, reminders and every view-model. Flutter renders what it emits and forwards intents; `client/app/packages/strata_bridge` holds only the generated bindings, the platform build glue and two helpers (`loadStrataCore`, `strataAppDataDirectory`). Every module is tested headless in Rust.

### Layering (§12.1)

`api/` (the flutter_rust_bridge facade: plain functions and `StreamSink`s, errors as `CoreFailure`) → `session/` (the `Core` of the install and the `Session` of the active account; intents) → `view/` (`model` = the Dart contract, `build` = pure builders from SQLite, `hub` = watchers) → `store/` (SQLite, migrations, outbox, write path, derived index), `sync/` (engine and record application), `search/` (FTS5, offline dedupe), `graph/` (graph-algo; edge kinds from `domain::GraphEdgeKind::of_relation`), `auth/` (token provider, account modes), `format/` (vault-format adapters, editor hints and completions, labels in the account's time zone and UI language, text direction, recurrence forms and previews, task-text parsing, line diffs), `net/` (the `AccountApi`/`SyncApi`/`EventsApi` traits over `strata-client`), `notify/` (reminder planning). Shared rules come from the shared crates only (L16): `sync-model` for ops, results, records, versions, merge and apply rules; `item-render` for new items' paths and markdown; `dedupe` for duplicate checks; `graph-algo` for layouts; `text-normalize` for search text.

### Storage

- One SQLite file per account, `<app-data>/strata/accounts/<user_id>.sqlite3` (rusqlite, bundled SQLite with FTS5, WAL), plus `strata/registry.sqlite3` (known accounts, last server URL, device name). Accounts never share a file; switching closes one connection and opens another.
- Forward-only migrations (`PRAGMA user_version`, each in one transaction; a newer schema is refused): v1 the §12.2 cache (notes with their server base, derived tags/links/relations/entities/places/documents/custody/tasks/reminders, inbox, suggestions, clusters, settings, keep-both, graph positions, `notes_fts`, outbox, sync state, conflicts, duplicates, rejections) and `auth_tokens` (D14); v2 `scheduled_notifications`; v3 the server-backed state (`notes.summary`, relation creation times, suggestion replies, the `/events` resume seq, sync pause, `remote_cache` for online-only reads such as devices, AI status, integrity, history and AI decisions, `sync_log`, pinned notes, acknowledged suggestions, and `outbox.base_content` for rebasing).
- **State model.** Each note stores the server's base (`base_*`, content-hash version) and the current content = base + live outbox ops folded by `store::write::apply_to_note`, which applies `sync-model`'s rules. A pull updates the base and rebuilds; a rejected op is removed and the entity rebuilt (the rollback). Derived tables and FTS are re-indexed in the same transaction.
- **Write path (§12.3).** An intent validates, applies optimistically, appends one outbox op (`sync-model` `Op`, client ULIDs for op and entity IDs, the base version: note content hash or task-line version) and emits the affected view-models — one transaction, then one notification of the changed `Topics`. Creates run the offline duplicate check (`dedupe` with candidates from SQLite) unless forced; capture is never refused.
- **Sign-out (§12.7)** refuses while ops are unsynced unless forced, then cancels reminders, revokes the session (best effort) and deletes the account's file (with its tokens) and registry row. A refused refresh ends the session but keeps the data (sign in again to continue).

### Sync (§12.4–12.5)

One cycle = push, then pull. Push sends pending ops in order (entities with an open conflict or duplicate prompt wait) and handles each `OpResult`: `applied` (op done; the base becomes the op's result only when its hash equals `new_version`, else the next pull brings it), `conflict` (a `sync_model::merge` 3-way preview and a `conflicts` row; resolving queues a forced op), `duplicate` (a prompt with the candidates; open existing / keep both / cancel), `rejected` (rollback + a typed rejection row). Pull bootstraps page by page (resumable cursor; mark-and-sweep via `bootstrap_seen`; changes are then pulled after the *oldest* page's position so edits made while paging are replayed) and otherwise pulls changes since the saved seq; `410`/epoch change re-bootstraps. Every step is its own transaction and ops are replayed with the same op ID (the server is idempotent by op ID), so a crash at any step loses and duplicates nothing (proptest over every crash point). Failures back off exponentially (2 s doubling, max 5 min); triggers are start, resume, write, events frame, a 60 s foreground timer, retry and "Sync now".

- **Rebase after pull (D19 on the device).** A queued `note.update` stores the content it was made against (`outbox.base_content`). Whenever a pull brings a new server version of a note, `sync::rebase` rewrites the note's *pending* ops in outbox order: an update is `sync_model::merge`d (its base, its content, the new server state) and on a clean merge takes the merged text and the new state's version as its base, so the push fast-forwards; a conflicting merge is left for the server's conflict answer. Other ops that need a base get the base version of the state they now apply to. A push whose merged result arrives while more ops of the same note are queued triggers a pull before the next batch.
- **Events.** `EventsApi` wraps the generated `/events` subscription (`reconnect: false`; the runtime loop reconnects with the engine's backoff). The resume seq is saved per account; a device that never received one resumes from 0 (the server replays what it buffers or answers `reset`), so nothing between the last pull and the connection is lost. `changed` and `reset` trigger a pull; `account.disabled` moves the session to its restricted state (disabled / deletion pending) and ends the stream.
- **Real server tests.** `client/core/tests/server_e2e.rs` runs client cores against the production app in-process (`strata_api::testing::TestServer`, per-test PostgreSQL database, fake clock): two devices bootstrap, pull changes, push (applied, merged, conflict with a conflict copy, duplicate and "create anyway"), rebase queued edits after a pull, resume `/events` from the saved seq, see `account.disabled` mid-stream, and run the admin and account intents.

### Reminders (D27 = a)

`notify::plan` lists the next 60 reminders of open tasks (device toggle on, account usable) with stable IDs from `(task block id, remind_at)`; `recompute` diffs the plan against `scheduled_notifications` and emits `schedule`/`cancel`/`update` ops on a stream the Dart adapter executes, reporting results back (permission denied, failure). Done and Snooze actions go through the outbox like any other task edit. The toggle off or sign-out cancels everything. On Linux (no OS scheduling) a core timer emits `show_now` when a reminder is due.

### The bridge

- flutter_rust_bridge 2.13.0: `rust_input: crate::api,crate::view::model`, Dart output in `strata_bridge/lib/src/generated` (regenerate with `melos run gen:bridge`, which also rustfmts `client/core/src/frb_generated.rs`).
- **No data-carrying enums cross the bridge.** frb would generate `freezed` classes for them and the logic guard forbids `freezed`, so every Dart-facing type is a struct or a plain enum (e.g. `SessionState { kind, account, … }`, `NotificationOp { kind, … }`), and `CoreError` crosses as the `CoreFailure` struct (`code`, `message_key` = `error.<code>`, details).
- cargokit is vendored at `client/core/cargokit` (outside the Flutter workspace, so the analyzer, formatter and logic guard never see its Dart build tool); the plugin's per-platform build files point at it. `melos run build:core` builds the host library the `strata_bridge` smoke test loads.

### Online features

`ClientAccountApi` covers every account and online endpoint the screens use: auth and `/me` (password, UI language, time zone, display name, export download, deletion confirmation), devices (list, rename, revoke, per-device reminders), admin users (list with query, approve, reject, role, enable/disable, reset password, schedule/cancel deletion, create), note history/revision/revert, semantic/hybrid `/search`, AI status, integrity, vault export/import, `/ask` + its stream (`watch_ask`), AI decisions (activity feed, reject/repoint/retype, D13), similarity edges (`GET /graph`) and `PUT /maps` (mind-map layouts as JSON Canvas). Results of online-only reads are cached per account (`remote_cache`) so the screens render the last answer offline with an `Offline` availability. The global and local maps are still computed locally with `graph-algo` from the synced cache (same edge kinds as the server's graph); the server's graph endpoints are used for similarity only. Creates are rendered with the server's shared renderers (`item-render`, `sync-model`) from the op alone, so the device's bytes are the server's.

### Time zones

Times are UTC in storage, on the wire and in files; every surface converts them to the account's time zone (`format::labels`, and `format::property_display_in` shows `created`/`updated` properties as local date and time). The account's zone is the `timezone` setting; an account without one takes the **device's** zone: `CoreEnv::device_timezone` is read from the operating system (`iana-time-zone`, every platform, no Dart involved), is the account row's zone until `/me` answers, and after the first bootstrap `Core::adopt_device_timezone` (run after each sync cycle) sets it with `PATCH /me` when no `timezone` setting has synced. A zone the user chose is never replaced.

## Sync and events

Spec: PLAN §7.4 (`change_log`, `idempotency`, `sync_epochs`), §7.5 Sync and Events, §12.3–12.5, D19, D24, principle 7. Code: `strata_api::sync` (`records`, `push`, `wire`), `strata_api::events`, `strata_api::routes::{sync, events}`; vault side `strata_vault::{events, diff}`. Wired by `strata_api::sync::install` (tests) and `stratad::serve::install_sync`.

### Wire types
Handlers encode and decode the shared `sync-model` types directly (`PushRequest`, `PushResponse`, `BootstrapPage`, `ChangesPage`), so device and server cannot disagree (L16). `sync::wire` mirrors them field for field as the contract schemas (`Sync*`); a unit test encodes a value of every op kind, result, record and page with `sync-model`, decodes it as the mirror, re-encodes and compares bytes. The push response is assembled from the stored result bytes (see idempotency).

### Records and the change log
- Records are `sync-model` `Record`s: notes with full content, version and kind (entities, documents, places, the task home and inbox notes are notes; tags, links, frontmatter relations, entity/document/place fields, custody state and tasks are derived from the content on the device with `vault-format`, as the server's indexer does), relations with provenance, rejected edges, suggestions with their threads, cluster assignments and names, user settings (`sync_model::settings`: a scalar as its text — strings, `true`/`false`, integers, shortest round-trip floats; a map setting such as `preferences` as one record per scalar entry keyed `<setting>.<entry>`; arrays, nil and nested maps are not synced), device settings (`devices.reminders_enabled` as `<device>:reminders_enabled`) and keep-both pairs. Relations and cluster assignments are sent only between live (untrashed) notes.
- The vault writes the `change_log` in the same scoped transaction as the index: `note` rows (upsert/delete, from `sync_paths`) as before, and now also `relation`, `rejected` and `keep_both` rows. Each write snapshots those derived rows around the affected notes before and after re-deriving them (`vault::diff::Snapshot`: relations from or to the notes, their rejections, keep-both pairs when a sidecar changed, plus task-row hashes and custody event IDs for the notice) and appends one row per difference. Reconciliation does the same. Suggestions (`suggestion`) append their own rows. Settings are logged where they change: `PATCH /me` appends a `setting` row per record whose text changed (`sync_model::settings::changes`: upserts, and tombstones for removed map entries; an unchanged value logs nothing), and `PATCH /devices/{id}` and the `device.settings` op share `sync::push::set_device_reminders` (a `device_setting` row `<device>:reminders_enabled` only when the value changes); `DELETE /devices/{id}` logs its tombstone.

### Bootstrap (`GET /sync/bootstrap?cursor=&limit=`)
Nine sections in a fixed order (notes, relations, rejected, suggestions, cluster assignments, cluster names, settings, device settings, keep-both), each read with keyset paging (`after` the last key, byte order). The first page captures `(epoch, seq)` from `sync_epochs`; the opaque cursor (base64url MessagePack `{epoch, seq, section, after}`) carries them, so every page reports the same snapshot position. Later pages show the *current* state of what they cover: an item that exists throughout is returned exactly once; anything that changed after `seq` is also in the changes feed after `seq` and re-applying a full record is idempotent; a note whose file moved while a page was read is skipped (its move is after `seq`). An epoch bump between pages answers `410 epoch_changed`. Note files are read directly (atomic renames), falling back to the writer when the file moved; the AI summary comes from the sidecar.

### Changes (`GET /sync/changes?since=&epoch=&limit=`)
Log rows after `since` in commit order (`sync_epochs` row lock makes seqs gapless and commit-ordered). Within one page only the newest row of an entity is kept and turned into its current record, or a tombstone (`change: delete`) when it no longer exists (deleted, trashed, relation gone). `next_seq` is the last row read (dropped duplicates leave seq gaps, which `SyncCursor::advance` allows); `has_more` when more rows wait. A different epoch is `410 epoch_changed` (detail names the current epoch).

### Push (`POST /sync/push`)
- Ops apply in order, each as its own vault write through the user's writer actor (one git commit per op, the same commit the REST endpoint for that mutation makes; the device's outbox is a list of intents and each stays revertible on its own). Batching several ops into one commit would need a multi-op transaction in the vault store and would make one bad op roll back the others; not done. A user's pushes are serialised (a per-user async mutex in `SyncState`). Limits: 500 ops, 16 MiB body.
- **Idempotency, exactly once.** A push first calls `VaultService::recover` (loads the vault if the writer has no state, which rolls back or recovers an interrupted write, see "Vault store"). Before an op runs, `idempotency` is looked up by `op_id` (user scope, RLS); a hit is answered with the stored bytes. Otherwise the op runs inside an `OpReceipt` scope (a tokio task-local the writer picks up when the job is queued): right before the vault call that completes the op, the push arms it with a hook that builds the result from the vault as the write left it (`AfterWrite`: live note versions from the writer state, task line versions from the changed files), and the write stores the encoded result (named-map MessagePack) in its own transaction and records it in its commit's trailers. The bytes come back through `OpReceipt::settled` once the transaction committed. A call that does not write (a rejected, duplicate or already-applied op) leaves the hook unused; that result is stored afterwards (nothing to lose: no effect was made). Database-only writes store it in their own transaction: `create_suggestion` (a conflict copy's suggestion — the copy note before it is unarmed and found again by ID on a replay), suggestion replies and decisions (the keep-both writes of a decision run with the receipt suspended; they are idempotent), `relink.request` and `device.settings`. A server failure (`5xx`, including a failed write) fails the push without storing anything for that op; the device retries it. The response is built by splicing the stored result bytes (`{results: [{op_id, result}]}`), so a replay is byte-identical and makes no commit.
- **D19 (`note.update`).** Current base → written as is. Stale base → the base content is looked up newest-first in the note's git history (`VaultService::content_at_version`, 200 revisions), or among contents this push already submitted for the note (the server stamps `id`/`created`/`updated`, so a device's version of its own create never appears in history); `sync_model::decide_update` then gives already-applied (`applied`, current version), a clean 3-way merge (written with `If-Match` = the current version, `applied{merged: true}`; retried from the start if another write won), or a conflict: the server keeps its version, creates a conflict copy `<stem> (conflict YYYY-MM-DD HHmmss).md` next to the note with the device's content under the op's ULID as note ID (forced: it resembles the note by design; a retry finds it), records a `conflict` suggestion on the note (ID = op ID; payload `{op_id, copy_id, copy_path, base_version, server_version, hunks}`), and answers `conflict{server_version, conflict_copy{note_id, path, version}}`. An unknown base merges against an empty base (a conflict unless identical).
- Other ops: creates honour client ULIDs (notes, captures, entities, documents, places, task block IDs, a new `tasks/Tasks.md`'s `home_id`, suggestion reply IDs) and the device's creation time (`created`; a stale one keeps its day, one beyond the skew is `rejected{422 created_in_future}`); a note create whose path was taken meanwhile is written at `<stem> 2.md` (never lost) with `title: <stem>` unless the content has its own title (the title rule, `item_render::note::titled_for_path`; the device applies the same rule through `new_note_at` when it knows the path is taken, so both write identical bytes). Creates without `force` answer `duplicate{candidates}` (dedupe `DuplicateCandidate`, `id` = the stored item ID). Entity/document/place patches are field level and re-applied onto the current version when stale (`merged: true`); aliases are added/removed against the current list. Task edits (`If-Match` = line version) answer `conflict{server_kept}` when the line changed, unless the edit is already in effect (complete of a done task, …); the device supplies `done`/`date` and the next occurrence's block ID. Move ignores the base (moves never conflict with content edits); delete with a stale base keeps the note (`server_kept`), an already-trashed note is `applied`. Relation remove of an absent edge between live notes is `applied`. Deciding an already decided suggestion is `server_kept`. `suggestion.accept` with edits is `POST /suggestions/{id}/accept-with-edits` (`VaultService::decide_suggestion_with`: filing `title`/`tags`/`folder`, the entity to link or the new entity's name and aliases, the custody participant, task fields); edits on a kind without them answer `rejected{422 invalid_body}` and write nothing. `document.create` writes its `copy-of`, `companies` and `people` links in the create's own commit (`create_entity_linked`; a missing target refuses the create with `404`, nothing written). Duplicate candidates report `exact`, `near` or `semantic` (the vault's `Candidate::semantic`). `relink.request` queues a forced run exactly as `POST /notes/{id}/relink` (`strata_jobs::link::enqueue_forced_in`, in the transaction that stores the op's result: `file_inbox` for an inbox capture, `link` otherwise; a queued job of that kind for the note becomes the forced one); `device.settings` writes the column and logs `device_setting`.
- **Isolation.** Every lookup runs in the caller's scope; another user's IDs behave like missing ones (`rejected{404 not_found}`) and nothing is written. `op_id`s are per user.

### Events (`GET /events`, WebSocket)
- `EventBus` keeps per user a `ReplayBuffer` (512 events) and a broadcast channel (1024). Seqs are per user and start after `BusConfig::first_seq_after` (stratad: the start time in microseconds, so a seq from a previous process life is ahead of the head and gets `reset`).
- Producers: the vault store calls its `CommitListener` (the bus) on the writer actor right after each committed write with a `Committed` notice built during `sync_paths` (notes created/updated/moved/deleted with kind, path and version; the merge hint; relation, task and custody diffs; suggestions; integrity warnings from reconciliation), so events arrive in commit order and never for rolled-back writes. `events_of` maps a notice to events in a fixed order: per note (ID order) `note.*` then `entity.created|updated` for people, companies, documents and places, then `entity.merged`, relations (sorted), `task.changed`, `custody.changed`, `suggestion.created|updated`, `integrity.warning`. The sync push adds `device.settings_changed`. `job.completed|failed` and `cluster.updated` are defined and published through `EventBus::publish` by their producers (the job runner and clustering, not yet built).
- A connection subscribes to its authenticated user's channel only (auth on upgrade: the bearer middleware; `401` without a token). It replays after `resume_from` or sends `reset`, then forwards live events (a subscriber that lags past the broadcast capacity gets `reset`). Heartbeat, idle timeout and slow-consumer handling are `wire::ws::serve` with `WsConfig` (overridable as app data).
- Account state: `RevocationSet` exposes a `watch` channel bumped on every change. Each connection re-checks its session and user when it moves: a disabled, deletion-pending or purged account makes the bus publish one `account.disabled{reason}` to that user's channel; every connection forwards it, sends a terminal `error` frame (`account_disabled`, `account_deletion_pending` or `unauthorized`) and closes. A revoked session (sign-out, device removed) ends with an `unauthorized` error frame.

### Tests
`backend/crates/api/tests/sync_push.rs` (every op kind with exact results and one commit each, clean merge vs conflict copy with its content and suggestion, duplicate then forced retry, byte-identical replay without a second commit, ordering within a push, per-op envelope errors, isolation, `suggestion.accept` with edits and forced `relink.request`), `sync_feed.rs` (paging stability under concurrent writes then convergence with a fresh bootstrap, exact change pages with tombstones and per-page dedupe, `410` and a malformed cursor, feed isolation), `sync_events.rs` (exact event sequence through the generated `Subscription`, frames validated against the contract on a raw socket, resume after reconnect and `reset`, isolation and `401` on upgrade, `account.disabled` then the terminal error), `sync_convergence.rs` (proptest, deterministic RNG: two offline devices of one user with coalescing outboxes and the server converge; conflict copies hold the device's text; replays are byte-identical), `sync_crash.rs` (a crash injected — vault feature `fault-injection`, `VaultService::inject_crash` — after the file write, after the git commit and after the database commit of note create/update, task create, linked document create and capture; after a restart the replay equals an uninterrupted push byte for byte, with exactly one commit carrying the result trailers, the same files and change-log rows, and the exact recovery warnings), `creation_times.rs` (ops created offline on Monday and pushed on Wednesday keep Monday in UTC, the title rule for a taken name, `422 created_in_future` over sync and REST validated against the contract; the vault side in `backend/crates/vault/tests/creation_times.rs`, the device side in `client/core/tests/creation_times.rs`), `sync_settings.rs` (`PATCH /me`, non-string and map settings, `PATCH`/`DELETE /devices/{id}` and `device.settings` through the changes feed and bootstrap; unchanged values log nothing). Unit tests: `events::tests`, `sync::wire::tests`, `vault::events`, `vault::receipt`, `vault::journal`, `sync_model::settings`.

## Jobs and AI pipelines

Spec: PLAN §5.2 (fair scheduling), §7.4 (`jobs`, `chunks`), §7.5 Search/AI/Events, §9.1b, §9.2, §9.5–§9.7, principles 3–7. Code: `backend/crates/jobs` (`strata-jobs`), `strata_ai::embed::{lazy, fake}`, `strata_vault::{ops::ai, semantic}`, `strata_api::{ai, routes::ai, routes::search}`, wired by `stratad::jobs`. This section supersedes the "not yet" notes of "AI foundation" (the `AiService` is now used by jobs and handlers; the embedder loads on demand) and "Vault store" (semantic and hybrid search exist).

### Job runner (`strata_jobs::runner`)
- **Discovery and claiming.** `AppDb::due_job_users` (the content-free `job_wakeups` hints) lists users with due work; each claim runs in that user's `ScopedTx` with `FOR UPDATE SKIP LOCKED`, restricted to the kinds this process has handlers for and capacity left (`repo::claim_of_kinds`), so concurrent runners never claim a job twice (tested with three runners on one database). Jobs of other kinds (`link`, `file_inbox`, … until their handlers exist) stay queued; a user whose due work is only of such kinds is skipped for `jobs.idle_recheck_secs` — never while one of their jobs is running (it may queue follow-ups), and the check reads the queue only after locking the wakeup row, so a follow-up committed concurrently is always seen (a race found by the tests).
- **Fairness.** A pass visits due users least recently served first and claims one job per user per round, within the capacity free when the pass started. A user flooding the queue gets one slot per round (tested: alternation while the other user has work).
- **Limits.** `jobs.max_concurrency` (default 2) across users; per kind `JobHandler::max_concurrency` (embedding 1). Classes: `Embed` (low priority, gate with `claude -p`), `Llm` (budgets, pauses, `ai.daily_job_limit` per UTC day, in memory), `Light`.
- **Outcomes.** Done → `complete`, `job.completed`. `Retry` → re-queued after `backoff_base · 2^(attempt−1)` capped at `backoff_max`; out of attempts → `failed`, `job.failed`. `Fatal` → `failed` at once. `Paused` (budget or provider limit, from `AiError::Paused`) → back to the queue for the pause without spending the attempt (`repo::release_paused`), and further LLM jobs of that user (user budget) or of everyone (global budget, provider limits) are not claimed until then. Panics count as retryable failures.
- **Events.** A `JobEvents` sink; `stratad::jobs::BusEvents` publishes `job.completed|failed` on the user's `/events` stream.
- **Shutdown and restart.** `RunnerHandle::shutdown(grace)` stops claiming and waits for running jobs; anything still running stays `running` and `Runner::recover_stale` re-queues it at the next start (one process per deployment).
- **Scheduler.** `Scheduler::ensure` keeps one queued job per periodic kind per active or deletion-pending user (debounce key = cadence), at the next `jobs.nightly_hour` in `default_timezone` (DST gaps move forward); `stratad` runs it at start and hourly. `enqueue_for_all` fans out one-off work (the embedding backfill at start).
- **Status.** `strata_jobs::status::ai_status` fills `AiStatus.queue_depth` (queued + running jobs of the user) and `embedding_progress` (live notes with a current vector / live notes).

### Embeddings (`strata_ai::embed::lazy`, `strata_jobs::{chunk, embed, vectors}`)
- **Model.** Default configuration is the fp32 export `onnx/model.onnx` (owner decision 2026-09-27), model ID `…@onnx/model`, padding-invariant (`pad_batches = true`); the quint8 export stays available (`OnnxEmbedderConfig::granite_97m_r2_quint8`). A changed model ID makes every stored vector stale.
- **On demand.** `LazyEmbedder` loads the model at the first `embed` (on the blocking pool, holding the exclusive side of the `CpuGate`, so a load never overlaps `claude -p`) and `unload_if_idle` drops it after `ai.embedding.idle_unload_secs` (default 300) without calls; `stratad` checks every 30 s. In-flight calls keep it loaded; the idle period restarts after the last call. Time from the injected clock (fake-clock tests); a real-model test (ignored by default) covers load → unload → reload.
- **Chunking.** Units are `vault-format` blocks (headings only delimit); consecutive units pack into chunks of ~300–500 estimated tokens (per word: ⌈ASCII/4⌉ + ⌈other/3⌉); a full chunk (≥ 300) ends at a heading; an oversized block splits at line/sentence/word boundaries. A chunk is cited through its first block (`block_id`, `anchor_len`, body offsets and the note version are stored). Embedded text: title, heading path, chunk text.
- **`embed` job.** Skips when the note vector has the current model and note version; otherwise embeds one chunk per call, stores chunks with the model ID and the note vector = L2-normalised mean of the chunk vectors (`note_vectors`), enqueues `summarize`, and embeds the note's duplicate-check items (title/text, task lines) whose text changed (`dedupe_vectors`). Trashed or deleted notes lose their vectors. Without a model it completes without vectors.
- **`embed_backfill`.** Enqueues `embed` for stale notes (no current vector) in batches, keeping at most 25 queued, and re-enqueues itself 30 s later until nothing is stale: first import, reindex and model changes are resumable across restarts, with progress in `AiStatus`.
- **Exact nearest-neighbour scans.** Vector queries compute distances over one user's rows (materialised CTE) instead of the HNSW index: under RLS an approximate scan returns the globally nearest candidates first and the policy then drops other users' rows, which can starve a user; pgvector < 0.8 has no iterative scans. Milliseconds at ~10k notes per user.

### Summaries (`summarize`)
`summary` prompt on the note (body ≤ 24k characters), written by `VaultService::ai_set_summary` into the sidecar (`summary` + `content_hash` = the summarised version) as one `ai: summarize <path>` commit touching only `.meta/notes/<id>.json`. Skipped when the sidecar already holds a summary of this version (no LLM call), when the note changed meanwhile, or when AI is disabled for the user.

### Search, similarity, Ask
- `GET /search?mode=semantic|hybrid` uses `Retriever` (registered through `strata_api::ai::AiApi`): semantic = best chunk per note by cosine; hybrid = reciprocal-rank fusion (k = 60) of the normalised full-text ranking and the semantic ranking. Without an embedding model the modes keep answering `503 ai_unavailable`; an embedder failure is `503` with "could not run"; keyword search never depends on AI.
- Similarity edges (§9.6): `Retriever::similar` / `retrieval::similar_notes` — top-n notes by note-vector cosine at or above a floor, computed on request, never stored (for the graph API).
- Ask (§9.5): `POST /ask {question, scope?}` checks routing and budget, retrieves (hybrid chunks, or keyword chunks computed from the note text when embeddings are off), gives each source a ref `Link#^id` (existing block ID, else a fresh `ask-xxxxxx`), starts the provider stream and returns `{id}`. The answer runs in the background into `AskRegistry` (per user, 30 min after completion, at most 16 per user); `GET /ask/{id}` (WebSocket, D24, resumable, `404` for another user's ID) sends `tokens` batches, then one `citation` per cited source (note ID, path, title, the block ID that now exists, a resolvable target), then `done` with the final text, then `end`; failures end with an `error` frame (`ai_paused`, `ai_unavailable`). Only the cited new IDs are appended, in one `ai: ask <path>` commit (`VaultService::ai_cite_blocks`: relocates blocks by text when the note changed, skips headings and taken IDs); refs the sources do not contain are unlinked. `POST /ask/{id}/save {title?, force?}` waits for the answer and creates `notes/<title>.md` (question as a quote, answer with citation links, `## Sources`). WebSocket upgrades are `GET`, hence the start/stream split; the stream is in the contract as `ask_stream` with payload `AskFrame` (path parameters of stream operations are now documented). Problems: `503 ai_unavailable` (AI disabled or not configured), `503 ai_paused` (new problem type; detail `<reason> until <time>`), `422 invalid_body` for an empty question.
- `GET /ai/status`: provider, pause, queue depth, today's usage and caps, embedding model (loaded or not) and the caller's embedding progress.

### Duplicates, semantic level (§9.7)
- **On create.** `SemanticDupSource` (`VaultService::set_semantic`) embeds the new item's text only when the model is already loaded and the text has ≤ 600 characters, compares it with `dedupe_vectors` of compatible kinds and hands `dedupe::check` the cosines (inside a savepoint, so a failure never aborts the create's transaction). Borderline scores are not confirmed synchronously; only cosines ≥ the kind's confirmed threshold answer `409 duplicate_candidates` with `match_level: semantic`. Vault `Candidate` gained `semantic: bool` (level stays `Near` in the vault enum so existing matches compile). With the model unloaded or AI off, exact and near run as before.
- **Nightly `dedupe` sweep.** For every item with a vector: nearest items of compatible kinds above the kind's candidate threshold form a block; `dedupe::sweep` gives exact/near pairs within it and `dedupe::check` with cosine evidence gives semantic ones; borderline pairs get one `duplicate_confirm` call each (≤ 20 per run) whose verdict is remembered per pair and item text (`dedupe_verdicts`). Keep-both pairs are suppressed by the shared crate; pairs already suggested (any status) are skipped. Each new pair is one `duplicates` suggestion (payload `DuplicatesPayload {a, b, reason}`; wire `SuggestionPayload::duplicates`). Rejecting it records keep-both for the pair; accepting merges the pair (see "Duplicates and thresholds" below; the device shows which item survives before the user accepts).

### Schema (migration `…011`)
`chunks` gains `ord`, `heading_path`, `start_offset`, `end_offset`, `anchor_len`, `content_hash`; new user-owned tables `note_vectors`, `dedupe_vectors` (no foreign key: task items) and `dedupe_verdicts`. All derived and rebuilt by the jobs (backfill); every vector row stores its model.

### Tests
`backend/crates/jobs/tests/`: `runner.rs` (fairness, exactly-once under three concurrent runners, backoff timeline with the fake clock, fatal errors and events, user and provider pauses resuming without spending attempts, debounce, unhandled kinds, per-kind and daily limits, shutdown and stale recovery, scheduler), `embed.rs` (chunk rows and block IDs, the normalised mean numerically, idempotency, re-embed on change, model change as a resumable backfill with progress, trashed notes, task item vectors, no model), `summarize.rs` (exact prompt input, one sidecar-only `ai:` commit, no second call, disabled user, budget pause to the next day), `search.rs` (exact semantic scores and hybrid RRF order on Arabic/English notes, similarity edges, isolation), `ask.rs` (prompt sources, event sequence, citations resolved to real blocks with the missing ID appended in one commit, keyword-only Ask, disabled/empty/paused), `dedupe.rs` (semantic create check only when loaded, keep-both on force, nightly suggestions for exact/confirmed/LLM-confirmed pairs, keep-both and distinct verdicts respected, nothing new on the second night, rejection records keep-both). `backend/crates/api/tests/ai_api.rs`: the same through the generated client with contract-validated responses and frames (Ask frame sequence, resume, save as note, `404` for another user, `ai_unavailable`/`ai_paused`, AI status, principle 6 with no AI and with a failing model, semantic `409`). Unit tests in every module; `stratad/tests/jobs_wiring.rs`.

## Graph

Spec: PLAN §6.5 (`.meta/clusters.json`), §6.8, §7.5 Graph, §9.2 `cluster`, §9.6, §10, D3, D10, principle 7. Code: `backend/crates/graph` (`strata-graph`: `load`, `assemble`, `query`, `similarity`, `summaries`, `maps`, `cluster`, `service`), `strata_api::graph` (`GraphApi`, problems, `BusClusterEvents`), `strata_api::routes::{graph, maps}`, the vault hook `strata_vault::ops::files`, wired by `stratad::jobs::{handlers, periodic, graph_api}`.

### Graph assembly
- **One read per request.** `load::load` reads the caller's graph in one `ScopedTx` (RLS limits every row to the scope; nothing takes a user ID): live notes (id, title, kind, path, lang, updated) and every edge between live notes. Edge sources: resolved `links` → `link`/`embed` (one per source, target, kind); `relations` → `relation:<type>` for note relation types (`part-of` between two places → `part-of-place`), `concepts` → `concept`, `people`/`companies` → `mention`, entity relation types → `entity:<type>`; `documents` (state of the newest custody event) → `custody:location|holder|last-holder` with that event's `by`/confidence; document relation types (`copy-of`, §6.12) → `document:copy-of` (an additive edge kind; the mapping is `domain::GraphEdgeKind::of_relation`, shared with the client core). Cluster assignment and names come from `clusters`/`cluster_names`. Tags (`load::load_tags`) are read only when a request asks for tag nodes.
- **Payloads** (`assemble`, pure, canonical order: nodes by ID — notes by ULID, then tag nodes by key —, edges by source/target/kind, clusters by ID): node = id, title, kind, path, cluster_id, degree (edges of *this* response), lang, updated, short summary, depth (local only); edge = source, target, kind string, by, confidence, reason, weight (similarity cosine / co-mention strength), notes (co-mention count). No positions (D3: the client core lays out with `graph-algo`). Hover summaries are the sidecar `summary` (§6.5) collapsed and cut to 200 characters; the sidecar folder is listed once per request on the blocking pool.
- **Tag nodes** (§10 optional toggle, `include_tags=true` on `/graph` and `/graph/local/{id}`; ignored with a lens): one node per tag compared without case (as in Obsidian), ID `tag:<lowercase tag>`, titled by its smallest spelling, kind `tag`, no path/updated/cluster; one `tag` edge (by `user`) from every live note carrying it (frontmatter or body). In a local graph, notes sharing a tag are two hops apart. On the wire node/edge IDs are strings (a note's ULID, or `tag:…`); note payload bytes are unchanged.
- **Filters.** `types` = edge kinds (`relation`, `entity`, `custody`, `document` expand to their families; `tag`; `co-mention` is the lens kind); `kinds` = node kinds (the note kinds and `tag`). They are separate parameters because `concept`, `document` and `tag` name both a node and an edge kind (the plan lists only `types`; `kinds` is additive). Unknown values are `422 invalid_parameter` with a field error (`unknown_edge_type`, `unknown_node_kind`, `unknown_lens`, `invalid_depth`).
- **Entity lens** (`lens=people|companies`): the lens's entities as nodes, `entity:*` edges among them, and co-mention edges from `graph_algo::co_mentions` over the `mention` edges (per note naming `m` lens entities, each pair gains `1/(m−1)`; `weight` = strength, `notes` = count, no `by`).
- **Local** (`/graph/local/{id}?depth=1..3`): `graph_algo::neighbourhood` over the typed graph (edges followed both ways, allowed edge/node kinds, the focus always kept, every allowed edge among the included nodes). A filter naming only `co-mention` leaves the focus alone. A foreign or trashed focus is `404`.
- **Similarity** (§9.6, `include_similarity=true`): never stored. `SimilaritySource` is the seam; `NoteVectorSimilarity` reads the `embed` job's `note_vectors` for the configured model. Local graphs use the jobs crate's `retrieval::similar_notes` for the focus (its neighbours join at depth 1). The global graph needs top-n of every note: an exact all-pairs pass in Rust over the `max_notes` (default 2,000) most recently updated vectors (default top 5, floor 0.75), merged into unordered pairs; beyond the cap the response says `similarity: truncated`. Without an embedding model the graph still answers with `similarity: unavailable` (principle 6).
- **Performance** (`backend/crates/graph/tests/perf.rs`): 10,000 notes, 40,000 edges, a summary per note, 200 clusters — median 0.53 s per assembly in the debug test profile (4 vCPU Xeon @ 2.1 GHz, PostgreSQL on the same host); the gate is 2 s.

### Maps (`GET /maps`, `GET/PUT /maps/{id}`)
- A map is `maps/<id>.canvas` (JSON Canvas 1.0, `vault_format::canvas`); the ID is the file name without `.canvas` and follows the vault file-name rules. The file travels as a string (`content`) in the MessagePack envelope, like a note's markdown; responses add `version` and the file nodes resolved to note IDs.
- `PUT` parses and validates the canvas (structure, unique IDs, dangling edges, colours, sizes, subpaths) and checks that every file node names an existing visible vault file (live note or attachment); every problem is one field error of a single `422 invalid_body` (`invalid_canvas`, `duplicate_id`, `dangling_edge`, `bad_color`, `bad_subpath`, `bad_size`, `invalid_file`, `unknown_file`). It stores Obsidian's layout (`Canvas::to_json`) as one `user: save map maps/<id>.canvas` commit. Create = no `If-Match` (the file must not exist); replace = `If-Match` with the current version; otherwise `409 version_conflict` with `current_version`.
- **Rename.** The vault's move path already rewrites file nodes of every canvas with `Canvas::rename_file` in the move commit; the API test covers it end to end.
- **Vault hook** (`strata_vault::ops::files`): `VaultService::write_file(FileWrite { path, content, expect: Any|Absent|Version, author, op, index })` writes one non-note file (a content file other than `.md`, or `.meta/clusters.json`) on the user's writer actor: the precondition is checked there, the optional index hook runs in the same `ScopedTx` as the commit's index update, and it is one commit (`user:`/`ai:`). Unchanged content writes nothing. `read_file_bytes` reads any vault file for the scope.

### Clustering (`cluster` job)
- **Trigger.** Nightly (`Periodic { kind: "cluster", Nightly }`, added by `stratad::jobs::periodic`) and `POST /graph/recluster` (queues a `cluster` job with debounce key `manual`, so a queued run is reused; `202 {job_id, run_after}`; per-user in-process limit 3/hour → `429 rate_limited` with `Retry-After`). Class `Llm` (naming calls are budgeted like other LLM work).
- **Leiden** (D10): `graph_algo::leiden_from` on `WeightedGraph::project` of every non-similarity edge (user 3, AI ≥ 0.85 → 2, other AI 1), fixed seed, started from the previous assignment for stability, resolution from the user's preference `graph.cluster_resolution` (`PATCH /me` preferences; `(0, 10]`, default 1.0). Communities of fewer than 3 notes stay unclustered.
- **Stable IDs**: `graph_algo::match_clusters` against the previous `.meta/clusters.json` (Jaccard ≥ 0.25; splits keep the ID on the better match, merges retire the others). Fresh IDs come from `next_id` in the file (an unknown field kept by `vault_format::Clusters::extra`), so a retired ID is never reused.
- **Names**: only new clusters, clusters whose members changed, and clusters listed under `unnamed` go to `cluster_naming` (≤ 40 per call; input = up to 10 member titles by weighted degree then title, up to 5 concepts by frequency, the previous name). A user-given name (`named_by: user`) is never replaced. When naming fails or AI is off/paused for the user, a cluster keeps its previous name or gets `Cluster <id>` and is listed in `unnamed`, so the next run names it; the clustering itself is still written.
- **Write**: when the result differs from the previous file (ignoring `generated`), one `ai: cluster .meta/clusters.json` commit through `write_file` (expecting the version read at the start; a concurrent change retries the job), whose index hook replaces `clusters` and `cluster_names` (joined with `notes`, so a note purged meanwhile is skipped) and appends `change_log` rows `cluster_assignment` (per note) and `cluster_name` (per cluster), upsert or delete, for exactly the differences — `/sync/changes` then serves them as the existing `sync-model` cluster records. After the commit, `ClusterEvents::clusters_updated` publishes one `cluster.updated {cluster_ids}` (new, changed, renamed and retired IDs) through `EventBus::publish` (`strata_api::graph::BusClusterEvents`).
- **User rename** (`PATCH /graph/clusters/{id}` {name}, `strata_graph::cluster::rename`): the name (whitespace collapsed, 1–100 characters, else `422 invalid_body` `empty_name`/`name_too_long`) is written to `.meta/clusters.json` with `named_by: user` (the cluster leaves `unnamed`; `generated` and members unchanged) in one `user: rename cluster .meta/clusters.json` commit through `write_file` (expecting the version read; a concurrent `cluster` run is retried on the new content), whose index hook updates the rows and logs the `cluster_name` upsert; then `cluster.updated {[id]}` on the caller's stream. The same user-given name again writes nothing. Unknown or foreign IDs are `404`. The job never re-names a `named_by: user` cluster.
- **Rows ↔ file** (`strata_vault::clusters`): `snapshot` + `replace` write the rows of a file and log exactly the differences; used by the job, the rename and `stratad reindex`, which reloads the rows from `.meta/clusters.json` after rebuilding the notes (nothing logged when unchanged; a missing file clears them).
- No migration was needed: `clusters` and `cluster_names` exist since `…004`.

### Tests
`backend/crates/graph/tests/`: `graph.rs` (fixture vault with every node and edge kind: exact degrees, edge lists and provenance, one full node payload, type and kind filters, people/company lens with co-mention weights, local depths 1–3 with edge/node filters and the focus-only case, trashed notes, sidecar summaries, similarity edges exact/truncated/unavailable and local, isolation, `document:copy-of` edges and filters, tag nodes on/off with case folding, filters, local two-hop tags, lens and trash), `cluster.rs` (first run: exact prompt input, file, tables, change log, event, one commit touching only the file; rerun makes nothing; a changed cluster alone is renamed and keeps its ID; retirement and `next_id`; AI down → placeholders then named later, later outage keeps names; AI disabled; resolution preference; through the runner; isolation; deterministic planning and user names kept; reindex reloads the rows from the file and logs only differences; user rename: one `user:` commit, rows, change log, event, no-op repeat, problems, and the next run keeps the name and never asks the AI for it), `perf.rs` (10k gate). `backend/crates/api/tests/graph_api.rs`: every endpoint through the generated client with contract validation (exact graph payloads, lens, local, parameter problems, 404 across users, map CRUD/validation/409/rename rewrite, recluster reuse and `429` with `Retry-After`, `cluster.updated` on the bus, cluster rename end to end with re-clustering, tag nodes and `document:copy-of` on the wire). `stratad/tests/graph_wiring.rs`: handler and nightly registration, similarity follows the embedding configuration.

### Open
- Virtual `cluster` nodes (§10) are not emitted (clusters are a separate list for region labels).
- ~~Client offline graph mapping~~: done — `client/core/src/graph` uses `domain::GraphEdgeKind::of_relation`, emits `document:copy-of` edges and `tag:<tag>` nodes (`GraphFilter.include_tags`), and its nodes carry `GraphNodeKind`, `path` and `updated` like the server's.

## AI pipelines (linking, filing, entity insights, corrections, digest)

Spec: PLAN §6.5–§6.7, §6.11, §6.12, §9.2–§9.4, §9.7, §9.8, D13 = b, D30. Code:
`strata_jobs::{pipeline, link, file_inbox, insights, correct, digest, dates, thresholds}`,
`strata_vault::ops::{ai_apply, ai_decide}`, `strata_api::routes::ai_pipelines`, prompts
`linking.v2`, `inbox_filing.v2`, `entity_insights.v2` (their v1 files are retired: no fixture
or stored reply references them), wired by `stratad::jobs`. Migration `…014` adds
`ai_decisions.rel_type`, `mention` and `detail`.

### One job, one commit
- A job decides *what* to write and hands the vault an `AiChangeSet`; `Core::ai_apply` writes
  every file it touches (source note, sidecars, entity pages, documents, new concept notes,
  `_ai/` notes, block IDs, a filing move with its link rewrites) as **one** `ai: <job> <path>`
  commit, and inserts the job's AI decisions, suggestions, hints, AI thread replies and
  follow-up jobs in the **same** transaction as the index update (`finish_then`). The
  decisions applied by the commit get its ID afterwards; suggestion events follow the
  commit's own notice (`relation.added|removed`, `entity.updated`, then
  `suggestion.created|updated`).
- `expect_version` makes a job whose note changed meanwhile write nothing (`Stale`): the new
  version has its own job. `mark_linked` stores the version *after* the write as
  `last_linked_hash` (the AI's own frontmatter edits and block IDs change the file), so the
  follow-up `embed` → `link` makes no call.
- User decisions on AI output (accepting a suggestion, repointing) use the same machinery
  with `Author::User` (`user: accept <kind> <path>`, `user: repoint <path>`).

### Triggers
- `embed` (every content write) queues `link` 30 s after the run, debounced per note
  (`EmbedHandler::with_link_delay`); `link` skips when `last_linked_hash` equals the version,
  notes of other kinds than `note`, and inbox captures (their `file_inbox` job links them —
  one call, not two). `POST /notes/{id}/relink` queues a forced run (inbox: `file_inbox`).
- `file_inbox` (queued by the vault for every inbox write) skips a capture already filed at
  its version (sidecar `filed`).
- Linking and filing queue `entity_insights` for every entity they touch or that the note
  mentions, 5 minutes later, debounced per entity; `entity_insights_sweep` (nightly) queues it
  for every entity; each run skips when the SHA-256 of its prompt input equals the entity
  sidecar's `insights_hash`. `POST /entities/{id}/refresh` forces one.
- `digest` runs weekly (`jobs.digest_weekday`, default Monday, at `jobs.nightly_hour`).
- `correct` is queued by filing when the filing call flags the capture as a correction
  (`is_correction`) and by `POST /ask` when the question passes a cue pre-filter
  (`correct::looks_like_correction`: "is not", "actually", "wrong", "=", "the X … is Y",
  "مش", "غلط", "قصدي", "اللي … هو", …); the model decides.
- `suggestion_reply` is queued by the vault in the transaction that stores a user reply to
  an AI suggestion (REST and sync alike).

### Linking and filing (`pipeline::Planner`)
- **Prompt context.** The note's citable blocks (headings excluded; blocks without an ID get
  `b-` + 6 hex of the text's SHA-256, appended only when a decision cites them); candidates =
  most similar notes by note vector (with a model) ∪ keyword hits (up to 12 distinct
  normalised words of 4+ characters, OR-ed), kind `note` only, minus the note and every
  target it has a rejection for, at most 20, with sidecar summaries; every concept (name,
  aliases); entities whose name or alias occurs in the text (normalised whole words, or the
  `text-normalize` transliteration key of 1–3-word windows against names of as many words:
  "Ahmad Sameer" finds "أحمد سمير"), entities linked from the note, entities similar by
  embedding (≥ 0.6), places nested in or enclosing an offered place — each with its
  disambiguation hints and `part_of`; the note's rejections and rejected mentions.
- **Relations** to candidates at or above `thresholds.relation` are written with sidecar
  provenance (`by: ai`, confidence, reason, `provider/model`, created); `duplicates` is never
  applied: a `duplicates` suggestion (skipped when the pair was suggested before or kept both).
  AI edges of the note (note relations and mention keys) that this run did not return are
  removed; user edges are never touched; rejected edges are never re-added.
- **Concepts** at or above the threshold link to an existing concept (by ID, or a name/alias
  equal after normalisation or with trigram similarity ≥ 0.8) or create
  `concepts/<Name>.md` (`kind: concept`, AI-owned `## Summary` from the model's one-sentence
  definition).
- **Entity mentions (§6.7, D13 = b).** Auto-linked (`people:`/`companies:` with provenance)
  only when the model names exactly one offered entity of the right kind with confidence ≥
  the threshold and no other candidate. Nicknames and kinship terms link only through an
  existing alias (normalised equality), whatever the model says. Everything else is an
  `entity_link` suggestion — `ambiguous` (candidates), `low_confidence` (a proposed entity),
  `nickname`, or `new` (never created automatically) — unless the note's rejected mentions
  or rejected links cover it or the same suggestion is pending. Accepting links the note,
  adds the mention (and `aliases` edits, both scripts) to the entity, or creates it (name:
  the `title` edit, else the mention). Rejecting records the mention (sidecar
  `rejected_mentions`) and the proposed link (sidecar `rejected`).
- **Entity relations** between resolved entities are written on the subject entity's page
  (`works-at`, `client-of`, … with provenance in its sidecar).
- **Custody (D30).** Every participant resolves by the model's ID (right kind), a mention
  resolved in this run, a single alias match, or ambiguous mention candidates. Applied —
  custody line citing `[[Note#^block]]` and the derived frontmatter (`sync-model`
  `record_custody`) — only at or above `thresholds.custody` with every participant resolved,
  nickname people only through an alias, and no recorded event newer than it; otherwise a
  `custody` suggestion (`unknown`, `ambiguous`, `low_confidence`, `conflict`) whose cited
  block gets its ID now. A repeated event (same date, type, citing this note) is skipped.
- **Tasks** are always `task` suggestions (title, due, recurrence phrase, reminders, entity
  IDs); accepting writes the Obsidian Tasks line to `tasks/Tasks.md` (`user: task create`).
- **Dates** (custody, task due, timeline): an explicit `YYYY-MM-DD` in the cited text, else a
  known relative phrase resolved against the note's `created` (`today`/`tomorrow`/`yesterday`,
  `النهارده`/`بكرة`/`امبارح`, `بعد بكرة`, "next Sunday", "الخميس الجاي", …), else `created`
  when the model says so, else the model's date (`jobs::dates`).
- **Filing (§9.3).** `inbox_filing.v2` returns the filing (title, ≤ 5 tags, folder — only an
  existing user folder or `notes`; never `inbox`, entity, concept or AI folders) plus the
  linking extraction (people/companies as mentions). With the user setting `auto_file` (a
  MessagePack boolean, `GET`/`PUT /ai/settings`, default off) title, tags and the move go into
  the job's commit (`ai: file_inbox inbox/… -> notes/Clients/…`, every inbound link
  rewritten); otherwise a `filing` suggestion, whose acceptance (optionally with `title`,
  `tags`, `folder` edits via `POST /suggestions/{id}/accept-with-edits`) performs the same in
  one `user:` commit.

### Entity insights (§6.7)
Input: the entity (name, aliases, descriptive properties — `role`, `industry`, `doc-type`,
`copy`, `expires`; never contact fields — and hints) and up to 25 live `note` notes that
relate or link to it, newest first, each flagged `one_line` when its body has one non-empty
line. Validation before writing: bullets keep only citations of input blocks (none left →
rejected as uncited); insights and open items citing only one-line captures are rejected
(speculation); bullets and summary sentences with an e-mail address or a 7+-digit number are
dropped (dates are not numbers); timeline dates follow the rules above, newest first. The
sections are rendered with `[[Note#^block]]` citations and written by
`sections::replace_ai_sections` (which validates them and keeps every user section byte for
byte); persons, companies and places get Summary, Insights, Open items, Timeline, documents
Summary only; frontmatter is never touched.

### Corrections (§9.8)
- Every AI decision is an `ai_decisions` row with its kind, source note and block, target,
  relation key or custody type, mention, one-line summary, confidence, suggestion and commit.
  `GET /ai-decisions` is the activity feed.
- `POST /ai-decisions/{id}/repoint|retype|reject` (and accepting a `correction` suggestion,
  and confident corrections in words) go through `Core::plan_fix`: the old link is removed and
  recorded as rejected, a repointed link is added as a user edge, a rejected custody event's
  line is removed (frontmatter recomputed), a pending suggestion is accepted with the new
  target or rejected; the decision gets `reverted_at`. A repointed entity mention stores a
  hint (`"Ahmed" in "Acme call" = Ahmed Fathy`, or the given text).
- Corrections in words: the `correction` prompt gets the message, the last 50 open decisions
  (newest first, with source titles and dates, targets and names) and the candidate entities.
  Applied in one `ai: correct <path>` commit only when not ambiguous and every fix is valid and
  at or above `thresholds.relation`; otherwise a `correction` suggestion with the model's
  question.
- Threads: the reply job sends the thread to the same prompt; a repoint re-proposes (the old
  suggestion becomes `superseded`, the new one carries the target, an AI reply says so), a
  reject withdraws it, anything else is answered in the thread.
- Hints are stored in `disambiguation_hints` and mirrored in the entity's sidecar (`hints`);
  every later linking, filing and insights prompt shows them. `stratad reindex` (which clears
  the entity rows, and with them their hints) rebuilds the table from the sidecar mirror of
  every entity (`reconcile::rebuild_hints`: ID, text and time from the sidecar; the source
  decision carried over from the previous row with the same ID).

### Duplicates and thresholds (§9.7)
- Accepting a `duplicates` suggestion merges the pair (the item created first survives; the shared rule is `sync_model::suggestions::DuplicatesPayload::survivor`, which the device also uses for `SuggestionDetail.merge_label`):
  entities through the entity merge; notes through `Core::merge_notes` — the survivor keeps its
  path, ID, frontmatter and body, gains the loser's title and aliases as aliases, its tags and
  relation lists (links to either note dropped), the loser's body under
  `## Merged from <title> (<date>)` unless the bodies are equal, every link and sidecar
  reference retargeted, the loser trashed — one `user: merge <loser> -> <survivor>` commit;
  task pairs: the newer line is cancelled. Rejecting keeps both (unchanged).
- `thresholds.dedupe.<kind>` defaults are the calibrated `domain::DedupeThresholds` values
  (near; semantic = the candidate level; aliases have no semantic level), kept equal by a
  unit test, so an unchanged configuration behaves exactly as tested. `near` is the trigram
  level of the create check (`VaultConfig::near_thresholds`) and of the nightly sweep;
  `semantic` is the semantic candidate level (`jobs::thresholds::dedupe_thresholds`); the
  no-confirmation level stays the tested default unless the configured value is higher.

### Tests
`backend/crates/jobs/tests/`: `link.rs` (debounce, exact input, threshold and provenance,
`duplicates` suggestion and merge on accept, rejected edges never re-added, stale AI edges
removed with user edges kept, not-JSON replies retried then the job retried, budget pause to
the next day, isolation), `entities.rs` (Arabic/Latin aliases and transliteration, entity
relations, ambiguous mention → suggestion → reply re-proposes → accept, nickname →
link-or-create → alias → next mention auto-linked), `custody.rs` (the Watanya example in
English and Arabic, ambiguous "gave the contract to Shady" → accept with the chosen document,
below threshold, conflict with a newer event), `insights.rs` (uncited, speculative and contact
bullets rejected, "بكرة" resolved, user sections and contact fields untouched, no second call),
`filing.rs` (auto-file off: suggestions and accept-with-edits; on: filed in the job's commit),
`corrections.rs` (a capture repoints the decision, the hint changes the next resolution),
`digest.rs` (exact input and note). `backend/crates/api/tests/ai_pipelines_api.rs`: the
endpoints through the generated client with contract validation and isolation.

### Open
- Trashing an entity drops its hint rows with the entity row; restoring it brings them back
  only at the next `stratad reindex` (the sidecar mirror keeps them).

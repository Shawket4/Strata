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
Migrations `…001` foundation/global, `…002` account bridge, `…003` notes (`notes` with a `search tsvector` column and GIN index, `aliases`, `tags`, `links`, `relations`, `rejected`, `blocks`, `chunks` with `embedding vector(384)`, an HNSW cosine index and a per-row model), `…004` entities (`entities`, `entity_aliases` with a trigram GIN index, `mentions`, `clusters`, `cluster_names`, `places`, `documents`, `custody_events`, `disambiguation_hints`), `…005` tasks (`tasks`, `task_reminders`, `notification_log` with `UNIQUE (user_id, task_id, remind_at, device_id)`), `…006` app state (`jobs`, `suggestions`, `suggestion_replies`, `ai_decisions`, `ai_usage`, `settings`, `sync_epochs`, `change_log`, `idempotency`, `dedupe_keys` with a trigram GIN index, `dedupe_keep_both`), `…007` vault store (`integrity_warnings`; `dedupe_keys` gains `key_no` in the primary key, `phonetic_key` and the serialized `item`, so an item stores one row per name — see "Vault store").

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
- **Passwords**: Argon2id (`argon2`), cost from `auth.argon2` (default 19 MiB, 2 passes, 1 lane); verification uses the parameters in the stored PHC string and login rehashes when the configured cost changed. Unknown usernames spend the same hashing work against a dummy hash. An admin reset stores `temporary:<PHC>`; the flag drives the password-change restriction. A password change needs the current password and signs out every other session.
- **Sign-up** (D22) creates a `pending` account: no vault, no session; login answers `403 account_pending` / `account_rejected` / `account_disabled` only after the password verified (a wrong password is always `401 invalid_credentials`). **Approval** creates `<data_root>/users/<id>/vault` and `git init`s it through the `VaultProvisioner` trait (`GitVaultProvisioner` is the basic implementation; the vault store may supply its own). Admin-created accounts (`POST /admin/users`, `stratad create-user`) are active with a vault at once.
- **Rate limits** (`auth::rate_limit`): exact sliding windows with the injected clock — login per client IP and per username skeleton, sign-up per IP and globally, plus the pending-account cap (`accounts.max_pending_signups`). Refusals are `429 rate_limited` with `Retry-After`. The client IP is the socket peer, or the forwarded address when `auth.trust_forwarded_for` (behind nginx only).
- **Audit log**: every admin action writes one entry (`user.create`, `user.approve`, `user.reject`, `user.disable`, `user.enable`, `user.role.admin|member`, `user.password_reset`, `user.delete.schedule`, `user.delete.cancel`), sign-up writes `user.signup`, the purge writes `user.purge` (actor NULL).
- **Devices**: listing, renaming and removal run in the caller's scope, so another user's device ID is `404`. Removing a device deletes its sessions and refresh tokens (cascade) and revokes them in memory. `reminders_enabled` (D27) lives in the user's `settings` table under `device.<id>.reminders_enabled` (default `true`, removed with the device) until the `devices` table gets a column for it.
- **Settings** (`GET/PATCH /me`): UI language, timezone (IANA, validated) and free-form preferences are MessagePack values in the user-owned `settings` table, read and written in the caller's scope; display name and password live in `users` (accounts role).

### Deletion (D25)
`DELETE /admin/users/{id}` sets `deletion_pending` with `deletion_at = now + accounts.deletion_grace_days`, flags the user in the revocation set and revokes every session. The user can still log in; the session is export-only. `GET /me/export` streams a zip of the caller's own vault (path from the `UserScope`, never from the request; `.git/` and symlinks excluded) from a blocking writer through a bounded channel, and records `export_downloaded_at` before the response ends; admins only see that timestamp and have no route to the export. `POST /admin/users/{id}/cancel-deletion` restores `active` and ends the export-only sessions. `POST /me/confirm-deletion` brings `deletion_at` forward and purges at once. The purge job (`purge_due_accounts(now)`, run every `accounts.purge_interval_secs` by `stratad`) tombstones the user in the revocation set, removes `<data_root>/users/<id>` (renamed aside first, then deleted), then in one transaction deletes the `users` row — every user-owned row cascades from it — and appends one `user.purge` audit entry.

### `stratad`
`serve` refuses to start unless every configured secret file (signing key, AI key, push credentials) is a regular file with no group/other permissions, the key loads, the data root exists, and the database is `UTF8` with an `LC_CTYPE` other than `C`/`POSIX` (PLAN §14: `pg_trgm` must treat Arabic letters as word characters). It opens one pool per role (`strata_app` → `AppDb` + `ScopeIssuer`, `strata_accounts` → `AccountsDb`), loads the revocation set, starts the reload and purge tasks, and serves until SIGINT/SIGTERM (graceful, 30 s). Logs are JSON (`tracing`); request logs carry method, path without query (long segments elided), status and duration — never headers, bodies or queries. The vault store is the auth layer's provisioner and is reconciled for every active user in the background at startup (see "Vault store"). Other subcommands: `keygen`, `create-user [--admin]` (password on stdin), `verify --user` / `reindex --user` (user ID or username; run with the server stopped), `openapi`, `migrate` (as `strata_owner`), `bootstrap-roles` (apply as superuser, or `--print` the SQL). Operations are described in `docs/RUNBOOK.md`.

## Shared crates

Spec: L16, PLAN §5.1. Pure Rust crates under `/crates` used by both the backend and the client core: no I/O, no async, no clock, no randomness except explicitly seeded. `vault-format`, `text-normalize` and `domain` are documented in `docs/VAULT_FORMAT.md` and their module docs; this section covers `sync-model`, `graph-algo` and `dedupe`.

### `sync-model` (§7.5 Sync, §12.3–12.5, D19)
- **Ops.** `SyncOp { op_id, entity_id, base_version, op }`, where `op` is the adjacently tagged `Op` (`{kind: "note.update", payload: {…}}`) covering every §7.5 mutation: `note.create/update/move/delete`, `capture`, `relation.add/remove/retype`, `suggestion.accept/reject/reply`, `entity.create/patch/merge`, `document.create/patch/custody`, `place.create/patch`, `task.create/update/complete/cancel/reopen/delete`, `relink.request`, `device.settings`. The outbox stores the same parts as columns (`Op::kind`, `Op::payload_bytes`, `Op::from_parts`). `SyncOp::validate` enforces that `entity_id` is derived from the payload, that `base_version` is present exactly for edits of existing content (`OpKind::requires_base_version`), and `force` exists only on duplicate-checked creates. Task edits use the version of the task **line** (`apply::task_line_version`), so edits to other lines of the same note never conflict. `task.update` uses patch semantics (absent = unchanged, `nil` = clear). `task.complete` carries a client-generated `next_id` so both sides write the same next occurrence.
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

Verified against real binaries in the dev container: Claude Code 2.1.283 (flags, JSON and
stream-JSON shapes, `--json-schema` behaviour, a real summary + streamed ask with citation through
`AiService`), and the granite quint8 ONNX export with ONNX Runtime 1.30.0 (Rust vectors match the
Python reference to cosine > 0.9999). The Messages API is exercised only against a local mock with
the documented shapes.

## Vault store

Spec: PLAN §5, §6, §7.2–7.3, §7.5, §9.7. Crate `backend/crates/vault` (`strata-vault`); HTTP in `backend/crates/api/src/routes/{notes,search,inbox,relations,entities,documents,tasks,vault_ops}.rs`.

- **One writer per user.** `VaultService` (cheap to clone, `web::Data` in the app) keeps one tokio task per user fed by an unbounded `mpsc` of boxed jobs; every write runs on it, one at a time, so two writes to one vault never interleave while different users never wait for each other. Reads of files and the index run concurrently outside the actor (`ready()` only makes sure the vault was loaded and reconciled once). A job that errors or panics drops the in-memory state and sets `repair`, so the next job reloads and re-derives everything from the files (reported as `index_repaired`).
- **One write** = compute the file changes → `finish()`: atomic writes (`.strata-tmp-*` in the same directory, `fsync`, `rename`, `fsync` the directory; excluded from git via `.git/info/exclude`), one git commit of exactly those paths (git2, vendored libgit2, author `Strata`), then in one `ScopedTx` the synchronous index update of every affected note (the changed notes plus notes whose links name a changed path), `change_log` rows, and job rows (`embed`, `file_inbox`); commit. Messages: `user: <op> <path>`, `ai: <job> <path>`, `system: recovered changes` / `initialize vault` / `repair sidecars`.
- **Derivation** (`derive::derive`) is a pure function of path, text, sidecar and the vault's path index: note row (title, kind, `sha256:` version, search tsvector input), tags, aliases, links, relations with provenance, rejections, blocks, entity/document/place rows, custody events (IDs are deterministic ULIDs from the event text, so a re-derive is idempotent), tasks and reminders, dedupe keys. `stratad reindex` deletes every derived row and derives the whole vault again; a test asserts the result equals the incremental index row by row.
- **Optimistic concurrency.** `If-Match` carries the file version (`sync-model` `Version`); a stale one is `409 version_conflict` with `current_version`. Task edits use the version of the task **line** (`task_line_version`), so edits to other lines never conflict.
- **Timestamps.** Frontmatter `created`/`updated` are written in the user's time zone (setting `timezone`, else `default_timezone`). `updated` changes only on user creates and full-content updates — not on AI edits, task operations, entity patches or custody events — so the server writes the same bytes as `sync-model`'s client-side apply.
- **Move/rename** rewrites every link and relation that resolved to the moved paths (vault-wide, `vault-format` `rewrite`) in the same commit; `.meta/` sidecars are keyed by note ID and never move. **Delete** moves to `.trash/<path>` (still indexed, `trashed`), **restore** moves back (a free name if taken), **purge** removes file and sidecar and the index rows.
- **History and revert.** History follows renames (git2 similarity). `revert_note` writes the file as of a commit; `revert_commit` applies the inverse of a whole commit (typically `ai:`) with a git 3-way revert and, when git reports a conflict, a structured fallback: per frontmatter key and a line merge (`merge_file`) of the body, so an AI commit can be undone after later user edits to other parts of the same note. Remaining conflicts are `409 revert_conflict`.
- **Duplicates** (§9.7) use the `dedupe` crate end to end: each item stores `Item::keys()` in `dedupe_keys` (one row per name, `key_no`), candidates come from SQL (exact keys, `pg_trgm` similarity, phonetic keys), and `dedupe::check` decides. Creates of notes, tasks, entities, documents and places answer `409 duplicate_candidates` unless `force`; a forced create records keep-both pairs (`KeepBoth::for_forced_create`) in `dedupe_keep_both` and in the sidecar — notes in the standard `keep_both` list, other items (tasks, entities by alias) in an additive sidecar extension `keep_both_items: [{kind, a, b, at}]` — so the pair survives a reindex and is never flagged again. `POST /capture` is never refused: the capture is committed first and duplicates become a `duplicate` suggestion; accepting it records keep-both.
- **Relations.** User edges are frontmatter lists; AI edges add sidecar provenance in an `ai:` commit. Removing or retyping an AI edge records a rejection (sidecar + `rejected`) that `ai_add_relations` respects.
- **Entities, documents, places.** Created in `people/`, `companies/`, `documents/`, `places/`; field edits use `sync-model`'s entity patch rules; merge rewrites links to the survivor, unions aliases, moves `## Notes` under a dated sub-heading and trashes the merged note, in one commit. Custody events recorded through the API carry `by: user`; since the shared line grammar still requires a citation, the line cites the source note when given, else the document itself. Place nesting and "documents in this place" are recursive CTEs; cycles are refused.
- **Reconciliation** (§7.3) runs when a vault is first loaded (in the background for every active user at `stratad serve` startup): removes temp files, commits uncommitted changes as `system: recovered changes`, gives notes without an ID (and tasks without a block ID) one, repairs sidecars from frontmatter (frontmatter wins; orphans removed), re-derives notes whose file differs from the index and purges notes whose file is gone. Every finding is an `integrity_warnings` row (`GET /integrity`). `stratad verify` runs the same comparison without changing anything; `stratad reindex` rebuilds.
- **Export/import.** Export is a deterministic zip (sorted entries, fixed timestamps and permissions) of the vault with `.meta/` and `.trash/`, without `.git/`, plus `.obsidian/app.json`; export → import → export is byte-identical. Import rejects the whole archive on a symlink, absolute path, `..`, backslash, drive prefix or size limit (entries, entry, total; checked while decompressing), skips hidden entries other than `.meta/`/`.trash/` (and `.gitkeep` silently), assigns missing IDs, keeps every other byte, and is one `user: import <n> files` commit, so reverting it undoes the import. The zip is built in memory (bounded by the import limits and the 256 MiB body limit).
- **Isolation.** Every path comes from the `UserScope`; every ID lookup goes through the user's own state and RLS, so a foreign ID answers exactly like a nonexistent one (`404`). Path parameters are validated (`vault-format` path rules) before touching the disk.
- **Not yet:** semantic and hybrid search answer `503 ai_unavailable` until the AI subsystem is wired; `/entities/{id}/refresh` (AI) is Phase 4.

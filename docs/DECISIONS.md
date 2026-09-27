# Strata — Decisions log

Records every locked decision, principle change, and owner pick. `PLAN.md` is the spec; this file is the history of how it got there. Newest first.

## 2026-09-27

### Embeddings: fp32, chunk-averaged note vectors, idle offload
- fp32 ONNX weights (int8 was not faster on the target CPU and separated Arabic/English worse); note vectors are the mean of chunk vectors (~300–500 tokens per chunk) to bound memory; the model unloads after an idle period (default 5 min) and reloads on demand.

### Implementation decisions (sync-model, graph-algo, dedupe)
- `tasks/Tasks.md` groups tasks under `## <Month YYYY>` headings (English month name), oldest month first; a task created with a named home note is appended to the end of that note's body.
- Manual custody events recorded by the user through the API carry `by: user` and need no citation; the citation requirement applies to AI-produced events.
- Task edits use the hash of the task line as their base version, so edits to other lines of the same note never conflict.
- Clustering/layout edge weights: user edges 3, AI edges with confidence ≥ 0.85 weight 2, other AI edges 1; similarity edges excluded. Cluster identity kept by greedy Jaccard matching (≥ 0.25).
- Duplicate detection: exact keys fold simple English plurals; captures and tasks ignore a leading "remind me to" / "don't forget to" / "فكرني"; a transliteration-key match counts as near only at ≥ 0.75 similarity; captures are checked against tasks.
- Merge limitation (as in git): a line moved on one side and deleted on the other reappears at its new place.

### Implementation decisions (vault-format)
Gaps in PLAN §6 filled while building `crates/vault-format` (details in `docs/VAULT_FORMAT.md`); the owner may revisit any of them.
1. **One global canonical frontmatter key order** for every note kind: `id, kind, title, aliases, tags, created, updated, source, lang`, then entity/document/place property keys (`role, industry, website, phone, email, address, doc-type, copy, location, holder, last-holder, expires, status`), then all relation keys (note relations, `concepts/people/companies`, entity relations, `copy-of`), then unknown keys in original order. PLAN's person/document examples interleave `companies`/`people` among entity fields; those are illustrative only.
2. **Canonical task field order is the Obsidian Tasks plugin's** (description, reminders, then priority 🔁 ➕ 🛫 ⏳ 📅 ❌ ✅, then `^id`), because the plugin only reads fields at the end of a line. Lines in PLAN's example layout (`📅 … (@…) [[…]] ^id`) are fully parsed and preserved on edit.
3. **Custody line grammar**: `- YYYY-MM-DD — <type> [[primary]] [at|to|in [[place]]] [by [[person]]] [with|from [[party]]] — [[citation]]…`, plus fixed rules for the frontmatter state each event type produces (PLAN gave only a prose example).
4. **New `.meta/` shapes**: a `keep_both` list in the per-note sidecar mirrors duplicate "keep both" decisions (PLAN §7.4 names no location); `.meta/clusters.json` is `{version, generated, algorithm, clusters:[{id, name, named_by, notes}]}`.
5. **Month-end clamping** for recurrences ("the 31st" is 28/29 Feb, 30 Apr), expressed in RRULE as `BYMONTHDAY=28,29,30,31;BYSETPOS=-1`, instead of RFC 5545's skipping; plain `every month` from the 31st drifts after a short month exactly as the Tasks plugin does. Completing a recurring task sets the new line's ➕ to the completion date when the task has one.
6. **Links in unknown frontmatter keys are not rewritten on rename** (unknown keys are preserved exactly, PLAN §6.4); only known link-holding keys and body links follow moves. Ambiguous link names prefer exact case, then the linking note's folder; otherwise they are reported and never rewritten.
- Per L16, vault-format's vocabularies come from `domain`: added `domain::DocType`, `domain::MentionType` (`concepts/people/companies`) and `domain::DocumentRelationType` (`copy-of`); frontmatter values outside a vocabulary are kept verbatim as `Open::Other`.

### L16 widened — shared crates for all shared logic
- New shared crates `sync-model` (ops, change records, 3-way merge), `graph-algo` (neighbourhoods, Leiden, layouts), `dedupe` (exact/near detection, ranking). Rule: anything both the backend and the client core need goes into a shared crate.

### D27 revised — reminders: (a) local-first now, server push later
- The client core computes each device's notification plan and emits schedule/cancel operations; a Dart adapter applies them with flutter_local_notifications; the OS fires them even when the app is closed. Linux fires only while the app runs. Server push (FCM/APNs/WNS) comes later as a backup, de-duplicated by stable reminder IDs (PLAN §12.5b). Replaces the earlier (b) choice.

### Remaining open decisions resolved by the owner
- **D2 (b)** rich-text editor package with markdown round-trip · **D3 (a)** CustomPainter, layout in Rust · **D4 (b)** widget-based mind-map canvas · **D6 (b)** signed access + rotating refresh tokens, immediate revocation via a session revocation set · **D7 (a)** public HTTPS + app auth, domain later · **D9 (a)** in-process ONNX (ort) · **D10 (b)** Leiden · **D11 (b)** Riverpod (plus melos and a modern Flutter stack, PLAN §11.1) · **D13 (b)** new entities always suggested, plus the correction loop (repoint, corrections in words, threaded suggestions, disambiguation hints; PLAN §9.8) · **D14** tokens in the account's local database, unencrypted · **D15 (c)** own client generator (typify + template, one MessagePack `send()`) · **D19 (a)** 3-way merge · **D20 (a)** `claude -p` default, Anthropic API provider also built · **D23** `claude -p` serves all accounts for now, provider selectable per user in config · **D24 (a)** WebSocket with binary MessagePack frames.

### D29 — Documents and places: (b), extended
- New entity kinds `document` and `place`. Places nest (`part-of`), so "the safe at the Nasr City office" is two places. Documents track `location` (a place), `holder` (who has it now), `last-holder` (derived), `status`, copies (`copy-of`), expiry, and a cited **Custody** history; frontmatter always reflects the newest custody event (PLAN §6.12).
### D30 — Custody updates: (a) automatic, with (b) as fallback
- Applied automatically when confidence ≥ the custody threshold (default 0.85) and every entity resolves unambiguously; otherwise, or on conflict with a newer event, it becomes an inbox suggestion. Every automatic change is one revertible `ai:` commit.

### Scope: tasks, reminders, duplicate detection
- **D26 = (b):** tasks are Obsidian Tasks checklist lines (`- [ ] … 🔁 every month on the 1st 📅 … (@… 09:00) ^t-<ulid>`); recurrence phrase kept verbatim and compiled to RRULE; completion follows Tasks-plugin semantics (done line + new next line); default home `tasks/Tasks.md` (PLAN §6.11).
- **D27 = (b), (c) later:** reminders pushed by the server (FCM Android, APNs iOS/macOS, WNS Windows; live event stream on Linux); local-notification fallback to be added later (PLAN §12.5b).
- **D28 = (c):** compact shows tasks on Home (Today / Upcoming / Recurring); rail and sidebar get a Tasks destination; rail and sidebar scroll.
- **Duplicate detection for every kind** (exact / near without AI, semantic with AI), `409 duplicate_candidates` + `force`, captures never blocked, keep-both pairs remembered (PLAN §9.7).

### D25 — Export for a deleted user: (a) in-app pickup during a grace period
- Admin deletion schedules the account as `deletion_pending` (default 14 days), revokes its sessions, and allows only an export-only sign-in (`GET /me/export`, `POST /me/confirm-deletion`). The purge runs when the period ends or the user confirms. Admins can cancel and can see whether the export was downloaded, never the export itself. No email infrastructure needed.

### D22 — Account creation: (b) open self-signup with admin approval
- New accounts register via `POST /auth/signup` and stay `pending` until an admin approves; the user directory is created on approval. Adds signup/approve/reject endpoints, `pending`/`rejected` statuses, signup rate limits and a pending cap, and sign-up + approval-queue screens (PLAN §7.5, §8, §11, §15, §16).

### D21 — Per-user storage isolation: (b) shared database + row-level security, on PostgreSQL
- **New L22:** server database is the existing PostgreSQL instance on the VPS. One `strata` database; every user-owned row carries `user_id`; RLS enabled and forced on every user-owned table; scope set per transaction (`strata.user_id`), unset scope sees zero rows.
- Roles: `strata_owner` (migrations), `strata_app` (requests/jobs, NOBYPASSRLS, owns nothing), `strata_accounts` (account tables only, no grants on vault data).
- SQLite FTS5 / sqlite-vec replaced by `tsvector` over Rust-normalised text + `pg_trgm`, and pgvector (HNSW). Vaults stay as per-user directories. Clients keep local SQLite.

### Backups out of scope
- Owner handles backups with their own DevOps scripts (PostgreSQL WAL + full backups already running; vault directories too). D12 withdrawn; Phase 6 no longer includes backups or a restore drill.

### Owner direction: Flutter everywhere, no web client
- **Changed L3:** the Angular PWA is dropped. The client is one Flutter app for Android, iOS, macOS, Windows, and Linux with **adaptive layouts** per size class (PLAN §11).
- **Changed L4, L14, L15, L16:** "mobile" becomes "client"; the Rust client core and offline-first behaviour apply on every platform.
- **Changed L13:** only a generated Rust client; no TypeScript layer.
- **Withdrawn:** D5 (API routing for browsers), D16 (TS generator), D17 (Angular toolchain), D18 (Angular data layer).
- **Replaced:** D2–D4 are now Flutter editor / graph rendering decisions (still open, needed in Phase 0).
- **Re-scoped:** D11 is now UI binding only (the editor question lives in D2). D14 covers all five platforms.
- **Roadmap:** Phase 2 = client core, Phase 3 = Flutter app core, Phase 4 = AI linking, Phase 5 = graph (was 5a/5b/2/3/4).

### Owner direction: MessagePack wire format, contract-first, exhaustive tests
- **New L21:** every request/response body, error, sync payload, and streamed event is MessagePack, described by the OpenAPI contract. JSON stays only in on-disk formats that require it (`.canvas`, `.meta/*.json`) and the OpenAPI document itself (PLAN §7.7).
- **New principle 8:** contract first, tested always; untested code does not merge.
- **Rewritten §16 Testing strategy:** exact-outcome assertions, determinism rules, property tests, fuzzing, mutation testing, contract conformance, isolation suite, client-core state-machine sync tests, Flutter widget + golden tests at every size class × theme × direction × text scale, accessibility checks, full-stack E2E on Linux desktop and Android, coverage and performance gates.
- **New D24 (open):** streaming transport for `/events` and `/ask` with MessagePack frames. Needed by Phase 1.
- **Re-scoped D15 (open):** Rust client generator must emit MessagePack.

### Plan update: multi-user and entity rules
- **New L20:** multi-user with fully isolated accounts; roles `admin` and `member`.
- **New principle 7:** tenant isolation; foreign IDs return `404`.
- **New D21 (open):** per-user storage isolation. Needed by Phase 1.
- **New D22 (open):** account creation. Needed by Phase 1.
- **New D23 (open):** AI for users other than the owner. Needed by Phase 4 (AI linking).
- **Entity rules (§6.7):** nicknames/kinship terms never auto-created; accepting a link suggestion adds the alias; no speculation in Insights/Open items; Timeline date resolution rules.

### Phase 0 — brand (approved in session)
- Standalone brand, not part of any other product family.
- Symbol: heavy monoline "S" of three stacked strata (64 grid, 11 px stroke, round caps); favicon form at 14–15 px stroke.
- Lead colour **tide `#2477B3`**; app icons are **abyss `#0F1B26` tiles with a tide mark**. Palette "coastal": abyss `#0F1B26`, mist `#F1F5F7`, harbour `#7C8C96`, tide `#2477B3` (text use `#1D5C8C`), surf `#6CB4DD`, sand `#D8B47E` (strata-band seam only).
- Type: Quicksand Bold (Latin wordmark, placeholder until drawn), Cairo (UI, Arabic, content; Arabic wordmark placeholder), IBM Plex Mono (technical).
- Canvas: https://claude.ai/artifact/PttxTsS31VynwkgAC2cx7F

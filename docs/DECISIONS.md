# Strata — Decisions log

Records every locked decision, principle change, and owner pick. `PLAN.md` is the spec; this file is the history of how it got there. Newest first.

## 2026-09-28

### Implementation decisions (fixed server address)
Gaps filled while implementing the owner decision "No server address in the UI"; the owner may revisit any of them.
- **Name and value:** `STRATA_SERVER_URL` (`--dart-define`) → `CoreConfig.server_url`, plus `CoreConfig.release_build` (Flutter's `kReleaseMode`). The Build workflow sets `STRATA_SERVER_URL: ${{ vars.STRATA_SERVER_URL || 'https://strata-ai.duckdns.org' }}` at the workflow level and fails the Android and macOS jobs unless it is `https://<host>` without spaces.
- **Rule (core, `net::server_url::fixed`):** trimmed, trailing `/` dropped; `https://<host>` is accepted in every build; plain `http://` only to this device (`localhost`, `127.0.0.1`, `::1`) and only in debug builds; blank, no scheme, another scheme or no host is refused. The check runs once, when the core opens (`init_core`); a refusal is the new error `misconfigured_build` (field `server_url`, reason `missing` / `not_https` / `insecure_http`) and the app shows a fatal "This build can't start" screen without Retry instead of the boot-failure screen.
- **Nothing is stored per account:** the `server_url` columns of the account and registry databases, the registry's last-used and pending-request server values, `SessionState.server_url`, `KnownAccountItem.server_url`, `PendingApproval.server_url` and the `server_url` of `SignInRequest` / `SignUpRequest` are gone (the v1 migrations were edited: no installs exist yet). `AccountSummary.server_url` stays and is the build's address.
- **Where the address still shows:** only as the existing read-only "Server" row of Settings → Account. It was removed from sign-in (and its known-account rows), sign-up (field and "on <server>" subtitle), the approval screen, the disabled / deletion / password-change screens' header caption and the sync panel's details. The sign-in `insecure_http` message is gone; "Can't reach the server" no longer says "check the address".

### Implementation decisions (UI follow-ups)
Gaps filled while closing the "Still open" core items of the UI adoption pass; the owner may revisit any of them.
- **Title rule for plain notes (changes the scope entry below):** a `note.create` whose path was taken lands at `<stem> 2.md` (3, …) with `title: <stem>` unless its content has a title of its own. Shared in `item_render::note::{titled_for_path, new_note_at}`; the server applies it on push, the device when it already knows the path is taken, so both write the same bytes.
- **Custody notes:** the optional note of "Record a move" is written last on the custody line (`— <note>`, one line). A note made only of wikilinks is refused (it would read back as citations). A draft without a date is today in the account's time zone, computed by the core. `document.custody` and `POST /documents/{id}/custody` carry `note`.
- **List properties:** `entity/document/place.patch` gain `set_lists` (key → values, the whole value replaced; blanks and repeats dropped, tags lose `#`; an empty list removes the key). A user field with one value is written as a scalar; `aliases`/`tags` stay lists; relation keys, `id`/`kind` and the custody fields are refused.
- **Accepting a nightly-sweep `duplicates` suggestion merges the pair.** PLAN §9.2 (jobs table) says the nightly sweep proposes "`duplicates` merges as suggestions (never auto-merge)"; the survivor rule (created first, ties by ID; task pairs keep the line whose block ID sorts first and cancel the other) was already the server's and is now shared (`DuplicatesPayload::survivor`), so the inbox says what accepting keeps before the user accepts.
- **Purge date before deletion:** new `GET /admin/settings` → `{deletion_grace_secs}` (admins only), not in PLAN §7.5; the core shows "Deleted on <date>" (account zone) in the confirmation.
- **Time-zone list:** canonical IANA region zones of `chrono-tz` plus `UTC` (legacy aliases left out), with the current offset. Arabic names cover the regions and a curated set of about 90 cities; other cities keep their Latin name (no CLDR data is bundled).
- **File dialogs:** the app shell provides a `FilePicker` (`file_selector`) through `filePickerProvider`; features ask it for a path with a fixed suggested name (`strata-vault.zip`, `strata-export.zip`, `strata-unsynced.md`).
- **Server AI summaries** use `·` instead of `→` (`"Ahmed" · Ahmed Samir`); summaries stored before this change keep their arrows.

### Implementation decisions (creation times, UTC)
Gaps filled while implementing the owner decision below; the owner may revisit any of them.
- **Which ops carry `created`:** `note.create`, `capture`, `entity/document/place.create`, `task.create` and `suggestion.accept` (an acceptance can create an entity note or `tasks/Tasks.md`), and the REST create bodies (`POST /notes`, `/capture`, `/entities`, `/documents`, `/places`, `/tasks`, `/ask/{id}/save`). `POST /suggestions/{id}/accept` and `accept-with-edits` stay without a body time: they are online-only, so the decision time is the server's clock. Conflict copies are made by the server when it finds the conflict and carry its clock.
- **Refusal:** a `created` more than `max_future_skew_secs` (default 300) ahead of the server's clock is `422 created_in_future` (new problem type; exactly the limit is accepted); any past time is accepted.
- **Capture file names are UTC:** PLAN §6.9 names `inbox/YYYY-MM-DD-HHmmss.md` without a zone; they follow the UTC rule for file names.
- **`task.create` carries `home_id`,** the device's ID for a `tasks/Tasks.md` the create makes (ignored when the note exists), so both sides write identical bytes; the month heading is the creation time's date in the account's time zone.
- **Device time zone:** read from the operating system in the Rust core (`iana-time-zone`), not passed through the bridge; after the first bootstrap an account with no synced `timezone` setting gets the device's zone via `PATCH /me`.
- **Title rule scope:** entity, document, place and concept notes (user and AI). ~~A `note.create` whose path was taken meanwhile is still written at `<stem> 2.md` without a `title`.~~ Extended to `note.create` the same day (see "UI follow-ups" above).

### Implementation decisions (app ID, default server)
Gaps filled while implementing the owner decision below; the owner may revisit any of them. The server-address items are superseded by "fixed server address" above.
- **Loopback means exactly** `localhost`, `127.0.0.1` and `::1` (any port, any case); other `127.x` addresses and hosts that merely start with them are refused. The check runs in the core before any request (sign-in and sign-up) as `invalid_input` / field `server_url` / reason `insecure_http`, with its own en/ar message. Addresses without a scheme are left to the network layer as before.
- **The build default is plain plumbing:** Dart passes `String.fromEnvironment('STRATA_DEFAULT_SERVER')` as `CoreConfig.default_server_url`; the core treats blank as none and trims it. The session's `server_url` stays "last address used on this device, else the build default".
- **CI fails** the Android and macOS builds when `STRATA_DEFAULT_SERVER` is set but not `https://…`, warns when it is unset, and checks the APK package name and the macOS bundle identifier.
- **Other IDs:** the Windows notification app user model ID is `com.shawket.strata`, the Windows version resource's company is `com.shawket`, the FFI plugin's Android namespace is `com.shawket.strata.bridge`; the Android notification channel stays `reminders` (not derived from the app ID).

### Bands on phones without the sand seam (owner)
- Owner, 2026-09-28: "bands on phones without the sand seam". Compact layouts (the signed-out screens' 72-high band, the splash) keep the strata bands but drop the sand seam (`StrataBands(showSeam: false)`); medium and expanded keep the seam. The compact and medium bottom bands run to the screen edge, under the system navigation/gesture inset, at full depth above it (`bleed` continues the last band).

### Default device name and untyped errors (implementation)
- **Default device name:** the app shell reads raw device facts with `device_info_plus` (allowed in the shell only) and passes them in `CoreConfig.device`; the core picks and cleans the name: Android the device-name setting (usually the marketing name), else manufacturer + model ("Samsung SM-S921B"); iOS the device name unless generic, else the commercial model ("iPhone 16 Pro"); macOS the computer name; Windows and Linux the host name. `localhost` and `.local`-style suffixes never count; the platform's generic name is the last fallback. The field stays editable.
- **Untyped errors:** an error that is not a `CoreFailure` (an FRB-surfaced panic, an unmapped transport error) in the account screens is reported through `FlutterError.reportError` (no user data) and shown as "Couldn't reach the server. Check your connection and try again."; busy states reset.

### TLS crypto provider: ring (owner)
- rustls uses the `ring` crypto provider everywhere (apps and stratad) instead of aws-lc-rs: smaller binaries (several MB per app architecture) and simpler cross-compilation. aws-lc-rs is no longer in the dependency graph; `strata_client::ensure_crypto_provider()` installs ring where clients are built.

### TLS in the apps (owner)
- The apps trust the bundled Mozilla root certificates (webpki-roots) through one shared rustls configuration (ring provider) for HTTPS and WebSockets, not the platform trust store: Android needs JNI set-up for the platform verifier, and its store cannot be read for WebSockets. User-installed CAs are not trusted. Found in production 2026-09-28: sign-in on the Android release build never reached the server.

### No server address in the UI (owner)
- The apps have **no server field**: the server address is fixed at build time and never shown or editable. Release builds without an address fail; debug builds may pass one for local testing. The current CI build keeps the field only until it is green; then the field is removed and the app builds wait for the owner's domain, which goes into the build workflow.

### App ID, server address, HTTPS first (owner)
- **App ID:** `com.shawket.strata` on every platform.
- **Server:** the VPS is `187.124.33.153`; a domain will point at it. HTTPS through the nginx reverse proxy is set up for the domain **before** CI builds the apps that people install. CI bakes the HTTPS address in as the default server (`STRATA_DEFAULT_SERVER` repository variable); the field stays editable. The app refuses plain `http://` except loopback (SSH-tunnel testing).

### Builds, deployment config and container plan (owner)
- **CI builds artifacts:** the `stratad` Linux x86_64 release binary, the Flutter app for macOS (ad-hoc signed `.app`) and Android (APK). App icons and splash screens are set for every platform from the brand marks.
- **Configuration via `.env`:** `stratad` reads its configuration from environment variables, loaded from a `.env` file (the owner places it on the VPS). The binary is deployed manually for now; automated deployment comes later.
- **Docker later:** a future image contains only the backend. PostgreSQL runs on the host (shared with other containers), and the embedding model and ONNX Runtime stay on the host, mounted read-only; embeddings remain in-process (D9 unchanged).

### Implementation decisions (configuration and builds)
Filling gaps of the owner decision above; the owner may revisit any of them.
- **Variable names:** `STRATA_` + the setting's path with `__` between the parts, upper-case (`STRATA_DATABASE__APP_URL`, `STRATA_THRESHOLDS__DEDUPE__NOTE__NEAR`); `deploy/stratad.env.example` lists all 103. Lists are written plainly: the `claude` launcher as words separated by spaces, per-user providers as `user=provider` pairs separated by commas.
- **Strictness:** every name in the env file must be a setting (typos stop `stratad` with a "did you mean"); in the process environment only `STRATA_…__…` names must be settings, so the test and tool variables (`STRATA_TEST_DATABASE_URL`, …) keep working. Errors never echo a value. `stratad check-config` prints the effective settings with database passwords masked.
- **Artifacts:** Android ships a universal APK plus one per ABI; macOS a universal (arm64 + x86_64) app, ad-hoc signed, as zip and dmg; without the signing secrets the APKs are debug-signed and labelled so.
- **Release fixes found while packaging:** release Android builds lacked the `INTERNET` permission and the sandboxed macOS release lacked `network.client` (and `files.user-selected.read-write` for the file picker); both added. Two build blockers in the cargokit integration were fixed: cargokit looked for `libstrata-core.*` (the package name) instead of `libstrata_core.*` (the `[lib] name`) and silently shipped no Rust core (Linux install failed; Android and macOS would have lacked or failed to link it), and the Linux/Windows plugin CMake file included cargokit through Flutter's plugin symlink, which CMake resolves lexically.

### File picker and label separators (owner)
- **File picker:** export/import paths come from the native OS picker (`file_selector`, allowed in the app shell only); it returns a path and the core does the rest.
- **No arrow glyphs in core labels:** labels use words or `·` instead of `→` (e.g. "Moved from Safe to Office"). Cairo has no arrow, and a fixed arrow points the wrong way in RTL.

### Times, creation stamps and titles (owner)
- **Times:** the server stores and writes every time in UTC; every surface converts to the client's time zone for display. File names that contain a time (conflict copies) use UTC on both sides, because they are vault content shared by all devices and Obsidian.
- **Device creation time is required:** every create op (entity, document, place, task note) carries the device's `created` time, and the server writes it as `created`/`updated`. It never substitutes its receive time, so an item created offline on Monday keeps Monday after syncing on Wednesday. The server refuses times implausibly far in the future. Device and server write identical bytes.
- **Title rule:** `title` is written whenever the file stem differs from the item's name (for example `Ahmed 2.md` gets `title: Ahmed`), for user and AI creates alike.

### Graph API
- `GET /graph` and `/graph/local` take `types` (edge kinds) and an additional `kinds` (node kinds) filter, because "concept" is both a node and an edge kind. Global similarity edges are computed exactly over the 2,000 most recently updated notes (response marked `truncated` beyond that). Clusters smaller than 3 notes stay unclustered; resolution is the user preference `graph.cluster_resolution` (default 1.0); user-given cluster names are never replaced.

### Accessibility: button fill token
- Filled buttons and the FAB use `accentFill` `#1D5C8C` + white (light, 7.1:1) and surf `#6CB4DD` + abyss (dark, 7.6:1); tide `#2477B3` remains the lead colour for the mark, icons and large graphics. White on tide fell to 4.16:1 under the hover overlay. Dark danger buttons use abyss text on `#E07A66`.

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

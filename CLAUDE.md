# Strata — project conventions

Strata is a personal, AI-powered knowledge system: Obsidian-compatible markdown vaults on a VPS, a Rust (Actix) API, and one Flutter app for Android, iOS, macOS, Windows and Linux with a Rust client core. Multi-user with fully isolated accounts.

## Source of truth
- **`PLAN.md` is the spec.** Read it before any work. If implementation reveals a gap or contradiction, stop and ask the owner; don't improvise a new direction.
- **`docs/DECISIONS.md`** records every decision. Open decisions are presented to the owner neutrally (trade-offs, no favourite) and never settled by you.
- Other docs: `docs/ENGINEERING.md` (coding rules), `docs/ARCHITECTURE.md`, `docs/VAULT_FORMAT.md`, `docs/WIRE_FORMAT.md`, `docs/TESTING.md`, `docs/RUNBOOK.md`, `design/SCREEN_SPEC.md` (UI tokens and screen rules), design canvas: https://claude.ai/artifact/PttxTsS31VynwkgAC2cx7F
- The owner writes in English and Arabic. Communication: concise, no filler.

## Architecture rules that are never broken
- **Markdown in the vault is the only source of truth**; the database is a rebuildable cache (plus app state).
- **All vault reads/writes go through the API.** The AI never edits the user's prose. Every write is a git commit (`user:` / `ai:`).
- **Tenant isolation (principle 7):** every request, job, query and file operation is scoped to one user. Data-layer functions take a `ScopedTx`/`UserScope`, **never a raw user ID**. Foreign IDs return `404`.
- **PostgreSQL with forced row-level security** on every user-owned table (`user_id` first in the PK, standard policy). New user-owned tables must pass the schema-audit test. Roles: `strata_owner` (migrations), `strata_app` (requests/jobs, NOBYPASSRLS), `strata_accounts` (account tables only, no access to vault data). The database needs a UTF-8, non-`C` character locale.
- **Contract first, MessagePack everywhere (L21):** every endpoint is in the OpenAPI contract; bodies are `application/vnd.msgpack` (structs as named maps), errors are `application/problem+msgpack`, streams are WebSocket binary MessagePack frames. JSON only for on-disk formats that require it and the OpenAPI document.
- **Shared crates for shared logic (L16):** anything both the backend and the client core need lives in `/crates` (`vault-format`, `text-normalize`, `domain`, `sync-model`, `graph-algo`, `dedupe`, and new ones as needed). Never duplicate logic across the two sides; never implement shared logic on one side only.
- **No logic in Dart (L15):** Flutter renders view-models streamed from the Rust core and forwards intents. Providers only adapt streams and inject the bridge. The logic guard enforces the package allow-list.

## Rust
- Edition 2024, toolchain pinned (`rust-toolchain.toml`). Workspace lints apply to every crate; `unwrap`/`todo!`/`dbg!` are denied (tests may use `unwrap`/`expect` via `clippy.toml`).
- Shared dependency versions in root `[workspace.dependencies]`. Crate versions move fast: **read docs.rs for the resolved version before using an API**; don't code from memory.
- Time via `Clock`, IDs via `IdGenerator` (both injectable) — never `Utc::now()` in logic.
- Errors: `thiserror` per crate, mapped to problem details at the API edge.
- **Adding an endpoint:** handler with `#[utoipa::path]` + `MsgPack<T>` → mount in `app::routes` → add to `ApiDoc` → run `api/generate.sh` (regenerates `api/openapi.json` and the Rust client) → tests validate every response with `Contract::validate_response`. CI fails if generated output is stale (`api/generate.sh --check`).

## Tests (the bar; PLAN §16)
- Exact assertions (full payloads, bytes, problem types). "Is ok" / "status 200" alone is not a test.
- Deterministic: fake clock, seeded RNG, fake LLM, temp dirs, per-test PostgreSQL database from `strata-testkit`. No sleeps; no network.
- Property tests (`proptest`) for parsers, merges and sync; fuzz targets for parsers; mutation testing on shared crates; coverage ≥ 90% lines.
- Flutter: widget + alchemist golden tests at compact 390×844, medium 1024×768, expanded 1440×900, large 1920×1080 × light/dark × LTR/RTL × text scale 1.0/2.0, plus accessibility guidelines. Integration and full-stack E2E on Linux desktop and Android.
- No retries in CI; a flaky test is a bug. Every bug fix starts with a failing test.

## Flutter
- Melos + pub workspace in `client/app`. Scripts: `melos run analyze` (runs `flutter analyze` and `dart analyze` for the riverpod_lint plugin), `format-check`, `logic-guard`, `test`, `test:goldens`, `gen`.
- Stack: Riverpod 3 (+ generator, riverpod_lint), flutter_hooks, go_router (+ builder, `StatefulShellRoute`), very_good_analysis (constructors use Dart 3.13's `const new(...)` style), alchemist goldens, mocktail. Design system in `strata_ui` (tokens as `ThemeExtension`s, `AdaptiveScaffold`, shared widgets); strings in `strata_l10n` (en + ar).
- Adaptive, not just responsive: bottom bar (compact, 5 items), rail (medium), sidebar (expanded), distinct pane structures.

## Local environment (cloud container)
- PostgreSQL 16 + pgvector on `127.0.0.1:5432` (user `postgres`, trust); data dir `/opt/pgdata`, start with `su postgres -c "/usr/lib/postgresql/16/bin/pg_ctl -D /opt/pgdata -l /tmp/pg.log start"`. Tests read `STRATA_TEST_DATABASE_URL`.
- Flutter SDK at `/opt/flutter` (add `/opt/flutter/bin` and `/opt/pub-cache/bin` to `PATH`, `PUB_CACHE=/opt/pub-cache`).
- Before handing work over: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `api/generate.sh --check`, and the melos scripts — all green.

## Git
- Work on the designated branch; imperative commit subjects with a body explaining why. Don't open a PR unless asked.

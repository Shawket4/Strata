# Engineering conventions

Everyone (human or agent) working in this repo follows these rules. `PLAN.md` is the spec; `docs/TESTING.md` expands §16.

## Rust
- Edition 2024, toolchain pinned in `rust-toolchain.toml`. Workspace lints in the root `Cargo.toml` apply to every crate (`[lints] workspace = true`). `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- No `unwrap()` outside tests; `expect()` only with a message that states the invariant. No `todo!`/`unimplemented!`/`dbg!`.
- Errors: `thiserror` enums per crate; `anyhow` only in binaries and tests. API errors map to RFC 7807 problem details (`strata-common::problem`).
- Shared dependency versions live in `[workspace.dependencies]`; crates use `dep = { workspace = true }`. When adopting a crate, read its docs for the resolved version (versions move fast; don't code from memory).
- Public items get doc comments. Modules stay small and focused.
- Time: never call `Utc::now()` directly in logic — take a `Clock` (`strata-common::clock`). Randomness: injectable RNG/ID generator.
- No `unsafe`.

## Tests (PLAN §16)
- Assert exact values: full payloads, bytes, error variants/problem types. `assert!(result.is_ok())` alone is not a test.
- Deterministic: fake clock, seeded RNG, fake LLM, temp dirs. No sleeps for timing.
- PostgreSQL tests: `STRATA_TEST_DATABASE_URL` (default `postgres://postgres@127.0.0.1:5432/postgres`) is an admin URL; `strata-testkit` creates a fresh database per test from a migrated template and drops it afterwards. Tests never share databases.
- Property tests with `proptest`; snapshot tests with `insta` (review snapshots, never blindly accept).
- Every bug fix starts with a failing test.

## Workflow
- Local database for development: PostgreSQL 16 + pgvector on 127.0.0.1:5432 (user `postgres`, trust auth in the dev container).
- Before handing work over: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (or `cargo nextest run --workspace`) all green.
- Commit messages: imperative subject, body explains why.

## Flutter (PLAN §11.1)
- Dart holds no logic (L15). Widgets render view-models from the Rust core and forward intents.
- `very_good_analysis` + `riverpod_lint`; `flutter analyze` with zero issues; `dart format` enforced.
- Widget + golden tests at every size class (compact 390×844, medium 1024×768, expanded 1440×900, large 1920×1080), light/dark, LTR/RTL, text scale 1.0/2.0.

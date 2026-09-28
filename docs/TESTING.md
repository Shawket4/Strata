# Testing

`PLAN.md` §16 is the spec; this file is how it is done in this repository: the conventions,
every suite and how to run it, the environment it needs, how fixtures and goldens are
maintained, and the coverage and performance gates with the numbers last measured.

## 1. Conventions (PLAN §16.1)

- **Exact outcomes.** Assert full decoded payloads, file bytes, commit messages, event
  sequences, status codes and problem types (`assert_eq!` on whole values; `pretty_assertions`
  for readable diffs). A test that only checks "no error" or "status 200" is incomplete.
- **Deterministic.** Time comes from `FakeClock` (`strata_common::Clock`), IDs from
  `SequentialIdGenerator` (`IdGenerator`), randomness from seeded generators (proptest with a
  fixed seed where a test drives its own runner), AI from the fake provider, files from temp
  directories. No network, no sleeps for timing: advance the fake clock or synchronise
  explicitly.
- **Isolated.** Every test builds its own world through `strata-testkit`: a fresh PostgreSQL
  database cloned from a migrated template (`TestDb`), a temp data root (`TempDataRoot`), its
  own users. Tests run in parallel.
- **No retries.** A flaky test is a bug and is fixed, never re-run (CI has no retry step).
- **Every bug fix starts with a failing regression test**, named for what it guards and, for a
  fuzzer finding, carrying the seed and case that found it.
- **Contract conformance everywhere.** Every HTTP response an API test sees is validated with
  `strata_api::contract::Contract::validate_response` (the generated client's
  `ResponseObserver` does it for every call; raw requests validate explicitly). Validation is
  strict: undocumented members, statuses and media types are violations.
- **Definition of done:** code, tests at every layer touched, contract regenerated
  (`api/generate.sh`), docs updated, CI green.
- Lints apply to tests; tests may use `expect`/`unwrap` (`clippy.toml`), and long table-driven
  tests allow `clippy::too_many_lines` locally.

## 2. Environment

| Variable | Used by | Meaning |
|---|---|---|
| `STRATA_TEST_DATABASE_URL` | every PostgreSQL test (`strata-testkit`) | Admin (superuser) URL; default `postgres://postgres@127.0.0.1:5432/postgres`. The cluster must be UTF-8 with a non-`C` character locale (`C.UTF-8`). |
| `STRATA_TEST_ROLE_PASSWORD` | `strata-testkit` | Optional password set on `strata_owner`/`strata_app`/`strata_accounts` (unset: trust auth, as in the dev container). |
| `STRATA_UPDATE_GOLDEN` | `strata-api` `tests/golden.rs` | Rewrite the wire-format golden hex files instead of comparing. |
| `INSTA_UPDATE` / `cargo insta review` | `vault-format` snapshot tests | Accept or review changed `tests/snapshots/*.snap`. |
| `STRATA_FUZZ_CASES`, `STRATA_FUZZ_SEED` | `strata-api` `tests/schema_fuzz.rs` | Case count (default 4 per operation) and seed (default `0x5354524154410016`) of the schema-driven fuzzer. |
| `STRATA_PERF_FILES` | `strata-api` `tests/performance.rs` | Size of the synthetic vault (default 10 000; budgets apply only at ≥ 10 000). |
| `STRATA_CLAUDE_COMMAND` | `strata-ai` `tests/claude_real.rs` (ignored) | Path of a logged-in `claude` CLI for the real-provider smoke test. |
| `STRATA_EMBED_MODEL_DIR`, `STRATA_ONNXRUNTIME_LIB` | `strata-ai` `tests/onnx_real.rs` (ignored) | Embedding model directory and ONNX Runtime library for the real-model tests. |
| `PROPTEST_CASES` | proptest suites without an explicit `ProptestConfig::with_cases` | Case count for long local runs. |

Local PostgreSQL in the dev container: `su postgres -c "/usr/lib/postgresql/16/bin/pg_ctl -D /opt/pgdata -l /tmp/pg.log start"`.
Flutter: add `/opt/flutter/bin` and `/opt/pub-cache/bin` to `PATH`, `PUB_CACHE=/opt/pub-cache`.

## 3. Suites and how to run them

### 3.1 Rust unit and integration tests

```sh
cargo test --workspace                  # everything below that is not #[ignore]d
cargo test -p <crate> --lib             # unit tests of one crate
cargo test -p strata-api --test <name>  # one integration target
cargo test --workspace --doc            # doctests (compile_fail proofs of UserScope etc.)
```

- **Shared crates** (`crates/`): unit, golden (`insta` snapshots in `vault-format`), property
  (`proptest`) and table tests; pure, no database.
- **Backend crates** (`backend/crates/*/tests`): integration tests against a real PostgreSQL
  (per-test database from `TestDb`) and real temp git repositories; no storage mocks. RLS and
  the schema audit live in `strata-index`; per-feature API suites in `strata-api/tests`, each
  with a harness module (`common`, `vault_harness`, `sync_harness`, `ai_harness`,
  `graph_harness`, `hardening`) that starts the production app in process (`TestServer`) with
  the fake clock and contract-checked clients.
- **Client core** (`client/core/tests`): headless view-model streams, sync state machine
  (proptest, with `sync.proptest-regressions`), write path, reminders, accounts; `server_e2e.rs`
  runs against a real in-process backend.
- `stratad` (`backend/bin/stratad/tests`): startup checks, commands and wiring.

### 3.2 Contract

```sh
api/generate.sh           # regenerate api/openapi.json and the Rust client after an API change
api/generate.sh --check   # CI: fail if either is stale
```

Conformance is asserted in every API test (see §1). `strata-api` `tests/contract.rs` tests the
validator itself.

### 3.3 Wire-format goldens

`strata-api` `tests/golden.rs` compares byte-exact MessagePack fixtures in
`backend/crates/api/tests/golden/*.hex` (documented in `docs/WIRE_FORMAT.md`). Regenerate after
an intended change and review the diff:

```sh
STRATA_UPDATE_GOLDEN=1 cargo test -p strata-api --test golden
git diff backend/crates/api/tests/golden
```

`vault-format` snapshots: `cargo insta review` (or `INSTA_UPDATE=always cargo test -p vault-format`).

### 3.4 Fuzz smoke (cargo-fuzz, PLAN §16.2)

Targets in `crates/vault-format/fuzz/fuzz_targets`: `document`, `frontmatter`, `body`,
`task_line`. Nightly toolchain and `cargo-fuzz` required.

```sh
cd crates/vault-format/fuzz
for t in $(cargo fuzz list); do cargo fuzz run "$t" -- -max_total_time=60; done   # CI smoke
cargo fuzz run document -- -max_total_time=3600                                  # long run
```

A crash becomes a committed regression test in `vault-format` (the input as a golden case).

### 3.5 Schema-driven fuzzer (PLAN §7.6, §16.3)

`backend/crates/api/tests/schema_fuzz.rs` generates requests for **every operation in the
contract** from its OpenAPI schemas (`tests/hardening/generate.rs`): schema-valid bodies,
query strings, headers and path parameters (fixture IDs of the right kind, random ULIDs,
hostile mixed-script text), plus mutated bodies (dropped or mistyped members, unknown members,
100 000-character strings, 70-deep nesting, lying MessagePack headers, truncation, trailing
bytes, extension types, `nil`, empty and random bodies, a JSON content type) and random or
hostile zip archives for import. It runs them as an authenticated member (admin operations as
an admin) against the complete production composition and asserts for every response: status
below 500, conformance to the contract for that operation, none of a second user's IDs or text,
and never the server's data root. Operations are visited round-robin with a `proptest`
ChaCha runner seeded from `STRATA_FUZZ_SEED`, so a run is reproducible; a failure prints the
case number, seed and the request (target and body hex).

```sh
cargo test -p strata-api --test schema_fuzz                          # normal: 4 cases per operation
STRATA_FUZZ_CASES=20000 STRATA_FUZZ_SEED=7 \
  cargo test -p strata-api --test schema_fuzz -- --nocapture          # long run, prints statistics
```

Operations added to the contract are fuzzed automatically. Findings become regression tests in
the same file (`regression_*`).

{{FUZZ_STATS}}

### 3.6 Security checklist (PLAN §15)

`backend/crates/api/tests/security_checklist.rs`, one named test per checklist item, generated
from the contract where the item concerns "every endpoint":

| §15 item | Test |
|---|---|
| No endpoint returns filesystem paths outside the vault | `no_operation_returns_a_filesystem_path`: calls every documented operation (valid requests, fixtures of every kind, then error paths that name files) and scans every string in every response and header for the data root, absolute paths and `..` segments not sent by the client. |
| Path traversal on every path parameter | `path_traversal_on_every_parameter_is_refused`: 18 encodings (`../`, `%2e%2e/`, double encoding, backslashes, absolute and data-root paths, drive letters, NUL, fullwidth dots, two-dot leader, division slash, overlong UTF-8, the neighbour's vault) into every path, query and header parameter of every secured operation, sent byte-exact with a raw HTTP client (`reqwest` would normalise them): `404` for path parameters, `404`/`422` for IDs and paths in queries, an empty `200` for free-text queries; never `5xx`, never planted outside content, nothing written outside the vaults, the neighbour's vault untouched. |
| Import zip: symlinks, absolute paths, `..`, oversized entries | `import_rejects_hostile_archives_whole` (exact problems; nothing committed, imported or written outside) and `import_stops_zip_bombs_while_decompressing` (headers that lie about sizes; a >90:1 archive whose entries are each within limits but not in total). |
| Rate limits on login, signup, capture, ask | `rate_limit_login`, `rate_limit_signup`, `rate_limit_capture`, `rate_limit_ask`: exact `429 rate_limited` problems and `Retry-After`, per key, reset by the fake clock. |
| Content never in logs or error messages | `content_never_reaches_logs_or_error_messages`: a full flow carrying a sentinel (sign-up, logins, notes, invalid paths and types, search, capture, tasks, entities, maps, settings, import, Ask, sync push, stale update, embedding job) with every `tracing` event at TRACE captured through `stratad`'s request logger; no log line and no problem `title`/`detail`/`errors` may contain the sentinel. |
| Secrets file permission check at startup | `startup_refuses_secret_files_open_to_others`: every configured secret (signing key, Anthropic key, FCM, APNs, WNS) under every group/other mode, `0400`/`0600` accepted, directories, missing files and symlinks to open files refused through `stratad::serve::prepare`; the error never quotes the secret. |
| MessagePack decode limits | `msgpack_decode_limits_hold_on_every_body`: depth bombs (arrays, maps), 4 GiB string/binary headers, 4 G-element array/map headers, trailing bytes, truncation, extension types, invalid UTF-8, the reserved marker, empty bodies → `422 invalid_body` with the exact code, and a declared body above any route limit → `413`, on every operation with a MessagePack body. |
| Tenant isolation, every endpoint | `tenant_isolation_sweep_answers_404_for_foreign_ids`: two users with identical titles; for every operation, the second user's own requests never carry the first user's IDs or text, and the first user's IDs in every path, query and body ID position answer `404 not_found`; the first user's history and export are byte-identical afterwards. |

```sh
cargo test -p strata-api --test security_checklist
```

### 3.7 Performance (PLAN §16.7)

**Timed suite** (`#[ignore]`d; run alone, in release mode):

```sh
cargo test --release -p strata-api --test performance -- --ignored --nocapture --test-threads=1
cargo test -p strata-graph --test perf             # GET /graph assembly gate at 10k (debug)
```

`strata_testkit::SyntheticVault` generates the 10 000-file vault deterministically (seed
2026): 9 450 notes in 20 topic folders (one third Arabic titles, mixed Arabic/English prose),
200 people, 100 companies, 50 nested places, 100 documents, `tasks/Tasks.md`, body wikilinks,
frontmatter relations (`related`, `part-of`, `people`, `companies`), tags, block IDs and
Obsidian Tasks lines. `tests/performance.rs` imports it (`POST /import`), requires zero
integrity warnings, and measures the operations below against the budgets.

**Criterion benchmarks** (`cargo bench -p <crate>`; reports in `target/criterion`):
`graph-algo` (`graph`: Leiden, force-layout step, projection at 10k nodes / 40k edges),
`vault-format` (`format`: parse, render, canonical render, body analysis, task extraction on a
1 MiB note; parse+analyse and edit+render of 1 000 notes), `text-normalize` (`normalize`:
search normalisation and transliteration keys over 1 MiB of mixed text; dedupe keys, trigrams
and trigram similarity over 10 000 titles).

{{PERF}}

### 3.8 Mutation testing (PLAN §16.2)

```sh
cargo install cargo-mutants
cargo mutants -p vault-format -p text-normalize -p domain --timeout 180 --no-shuffle
```

A surviving mutant fails CI unless it is justified in a comment next to the code (an
equivalent mutant) and excluded explicitly.

### 3.9 Flutter (PLAN §16.5)

From `client/app` (Melos + pub workspace):

```sh
melos bootstrap
melos run analyze            # flutter analyze + dart analyze (riverpod_lint), zero issues
melos run format-check
melos run logic-guard        # L15: dependency allow-list and Dart folder rules
melos run test               # widget, accessibility and golden tests of every package + tool tests
melos run test:goldens       # alchemist goldens only (CI variant, bundled fonts)
melos run test:goldens:update   # regenerate goldens; review every image diff before committing
```

Widget tests run every screen at compact 390×844, medium 1024×768, expanded 1440×900 and large
1920×1080, with live resizes across breakpoints; goldens cover every screen and key state ×
size class × light/dark × LTR (English) / RTL (Arabic) × text scale 1.0/2.0, rendered with the
bundled fonts (only the CI variant is committed; platform-font variants are ignored). Every
widget test also runs the tap-target, labelled-tap-target and text-contrast guidelines. Widgets
are fed view-model fixtures of the types the core streams (no logic in Dart to test, L15).

**Integration tests (`integration_test`) and full-stack E2E (`e2e/`)** are not written yet;
PLAN §16.5–16.6 describe them (the real app with the real core against a spawned `stratad` on
a temp data root, Linux desktop headless and an Android emulator). When they land, they run
with `flutter test integration_test -d linux` (under `xvfb-run` in CI) and
`flutter test integration_test -d <emulator>`.

### 3.10 Real-provider and real-model tests (ignored)

```sh
STRATA_CLAUDE_COMMAND=/usr/bin/claude cargo test -p strata-ai --test claude_real -- --ignored
STRATA_EMBED_MODEL_DIR=… STRATA_ONNXRUNTIME_LIB=… cargo test -p strata-ai --test onnx_real -- --ignored
```

Never run in CI (they need a login or model files and spend usage).

## 4. Fake LLM fixtures

`FakeLlmProvider` (`strata-ai`, feature `test-support`, re-exported by `strata-testkit`)
replays fixtures keyed by (prompt ID, prompt version, SHA-256 of `system` + NUL + `user`). A
test either `push`es fixtures in memory (`Fixture::json`, `Fixture::stream`, `Fixture::error`)
or points the provider at a directory (`FakeLlmProvider::from_dir`), where a fixture lives at
`<dir>/<prompt id>/v<version>/<input hash>.json`. A missing fixture fails with
`ProviderError::MissingFixture`, naming the prompt, version, hash and the exact path to record
it at.

Recording (by hand, never in CI): wrap a real provider in `FixtureRecorder::new(provider,
dir)` and run the flow once; each successful response is written at its fixture path. Review
the JSON (it must satisfy the prompt's schema and contain no private data), commit it next to
the test, and bump the prompt version when the prompt text changes (a test pins every prompt's
hash, so an unversioned edit fails CI and invalidates the fixture keys).

The hardening suites use `CannedLlm` (`tests/hardening/mod.rs`) instead: they generate
questions, which fixture keys cannot anticipate, and only need a stream to exist.

## 5. Coverage gate (PLAN §16.7)

CI: `cargo llvm-cov nextest --workspace --no-fail-fast --fail-under-lines 90 --lcov --output-path lcov.info`
(lines ≥ 90 %; the threshold only goes up; branch coverage is tracked, not gated).

Locally, with limited disk, build once and report once:

```sh
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --no-report            # instrumented build + all tests
cargo llvm-cov report --summary-only              # one combined report
cargo llvm-cov report --html                      # target/llvm-cov/html
cargo llvm-cov clean --profraw-only               # drop the raw profiles afterwards
```

{{COVERAGE}}

## 6. CI jobs (`.github/workflows/ci.yml`)

| Job | Runs |
|---|---|
| `rust` | fmt, clippy (`-D warnings`), all tests under `cargo llvm-cov nextest` with the 90 % line gate, doctests |
| `contract` | `api/generate.sh --check` |
| `fuzz-smoke` | every `vault-format` fuzz target for 60 s |
| `mutants` | `cargo mutants` on `vault-format`, `text-normalize`, `domain` |
| `flutter` | format check, analyze, logic guard, widget + golden tests |

The security checklist and the schema fuzzer (at its default case count) run in the `rust` job
as ordinary integration tests. The performance suite and long fuzz campaigns are run by hand
(and on a schedule once one exists) because their timings depend on the machine.

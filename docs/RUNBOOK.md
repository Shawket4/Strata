# Runbook

Operating `stratad` on the VPS (PLAN §14). **First install: follow
[`deploy/VPS_SETUP.md`](../deploy/VPS_SETUP.md) step by step**; this runbook explains the parts
and covers operations. Commands assume the service user `strata`, the settings file
`/etc/strata/stratad.env` and the data root `/srv/strata`. Every subcommand takes
`--env-file <path>` (or `STRATA_ENV_FILE`); without it, `stratad` reads `.env` from its working
directory if one exists.

## 1. Configuration (`.env`)

Every setting is one environment variable: `STRATA_` + the setting's path with `__` between
the parts, upper-case (`bind` → `STRATA_BIND`, `database.app_url` → `STRATA_DATABASE__APP_URL`,
`ai.claude_cli.command` → `STRATA_AI__CLAUDE_CLI__COMMAND`, `thresholds.dedupe.note.near` →
`STRATA_THRESHOLDS__DEDUPE__NOTE__NEAR`). **The full list, with defaults and comments, is
`deploy/stratad.env.example`** (a test keeps it in step with `strata_common::Config`); start from
it and change what differs.

Precedence: built-in defaults → the env file → the process environment (a real environment
variable always wins). The file is parsed with `dotenvy` (values only; stratad's environment is
not modified):

- `NAME=value`, one per line, `#` comments. Single-quote a value that contains spaces, `#`, `$`
  or quotes (`'postgres://strata_app:pa$$word@127.0.0.1/strata'`); in unquoted and
  double-quoted values `$NAME` is expanded.
- Numbers, `true`/`false`, paths and text as written. `STRATA_AI__CLAUDE_CLI__COMMAND` is the
  program and its arguments separated by spaces; `STRATA_AI__USER_PROVIDERS` is
  `username=provider` pairs separated by commas. An empty value unsets an optional setting.
- Every name in the file must be a setting (a typo stops `stratad` with a "did you mean"). In
  the environment, a `STRATA_…__…` name that is not a setting is an error; other `STRATA_`
  names (test and tool variables) are ignored.
- Errors name the variable and never repeat a value (database URLs carry passwords); invalid
  settings exit with status 2 before anything else happens.

Minimal `/etc/strata/stratad.env` (owner `root:strata`, mode `0640`; it holds database
passwords):

```sh
STRATA_DATA_ROOT=/srv/strata
STRATA_BIND=127.0.0.1:8080
STRATA_DEFAULT_TIMEZONE=Africa/Cairo
STRATA_DATABASE__OWNER_URL='postgres://strata_owner:…@127.0.0.1/strata'
STRATA_DATABASE__APP_URL='postgres://strata_app:…@127.0.0.1/strata'
STRATA_DATABASE__ACCOUNTS_URL='postgres://strata_accounts:…@127.0.0.1/strata'
STRATA_AUTH__SIGNING_KEY_FILE=/etc/strata/token-signing-key.pem
# only because nginx is the sole client of 127.0.0.1:8080:
STRATA_AUTH__TRUST_FORWARDED_FOR=true
```

Check a file without connecting anywhere; it prints every effective setting as env lines
(database passwords masked as `***`):

```sh
stratad --env-file /etc/strata/stratad.env check-config
```

With `STRATA_AUTH__TRUST_FORWARDED_FOR`, the login and sign-up rate limits key on the address
nginx reports, so nginx must **overwrite** the header rather than append to what the client
sent: `proxy_set_header X-Forwarded-For $remote_addr;` (not `$proxy_add_x_forwarded_for`) and
no `Forwarded` header passed through.

`STRATA_MAX_FUTURE_SKEW_SECS` (default 300) is how far ahead of the server's clock a device's
creation time may be before a create is refused with `422 created_in_future`: a device whose
clock is wrong sees its creates rejected until the clock is fixed; items created offline in the
past are always accepted with their own time.

Secrets are never values in the file: the signing key, the Anthropic API key and push
credentials are **file paths** (`…_FILE` / `…_PATH` variables) whose files must be regular files
with no group/other permissions (§4), or `serve` refuses to start.

The data root must exist and belong to the service user only:

```sh
install -d -o strata -g strata -m 0700 /srv/strata
```

## 2. Create the database (UTF-8, non-C locale)

Production runs PostgreSQL 17 with pgvector 0.8.2 (Debian 13); development and CI use
PostgreSQL 16. Either works (pgvector ≥ 0.6, `pg_trgm`).

The `strata` database **must** be UTF-8 with a character locale other than `C`/`POSIX`;
otherwise `pg_trgm` treats Arabic letters as non-word characters and server and offline
duplicate scores diverge. `stratad serve` checks this at startup and refuses to run on a
database that fails it. Create it from `template0` so the locale can be chosen:

```sql
CREATE DATABASE strata
  ENCODING 'UTF8'
  LC_COLLATE 'C.UTF-8'
  LC_CTYPE 'C.UTF-8'
  TEMPLATE template0;
```

Check an existing database:

```sql
SELECT datname, pg_encoding_to_char(encoding), datcollate, datctype
FROM pg_database WHERE datname = 'strata';
```

A database with `LC_CTYPE = C` cannot be changed in place: dump it, recreate it as above,
restore.

## 3. Bootstrap roles and grants (superuser)

`bootstrap-roles` creates the roles `strata_owner`, `strata_app` and `strata_accounts`
(`LOGIN NOSUPERUSER NOBYPASSRLS …`), sets their passwords from the configured URLs, and
prepares the database (connect grants, `vector` and `pg_trgm` extensions, schema `strata`,
`search_path`). It is idempotent. Either apply it directly:

```sh
stratad --env-file /etc/strata/stratad.env bootstrap-roles \
  --superuser-url 'postgres://postgres@/postgres?host=/var/run/postgresql'
```

or review and run the SQL yourself (connect to the `strata` database). The printed SQL holds
the passwords in plain text: keep the file private (mode 0600) and turn statement logging off
for the session so a slow statement never writes a password to the server log:

```sh
(umask 077 && { echo 'SET log_min_duration_statement = -1;'
  stratad --env-file /etc/strata/stratad.env bootstrap-roles --print; } > bootstrap.sql)
sudo -u postgres psql -d strata < bootstrap.sql && rm bootstrap.sql
```

Then apply the migrations as `strata_owner` (safe to re-run; run after every upgrade):

```sh
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
```

## 4. Generate the token signing key

Access tokens are signed with an Ed25519 key (PKCS#8 PEM). Create it once at the configured
path (`STRATA_AUTH__SIGNING_KEY_FILE`). `/etc/strata` is `root:strata 0750`, so run `keygen`
as root and hand the key to the service user:

```sh
stratad --env-file /etc/strata/stratad.env keygen
chown strata:strata /etc/strata/token-signing-key.pem
```

The file is written with mode `0600`; `keygen` refuses to overwrite an existing key. Every
secret file (`STRATA_AUTH__SIGNING_KEY_FILE`, `STRATA_AI__ANTHROPIC_API__API_KEY_FILE`, the
`STRATA_PUSH__…_PATH` credentials) must be a regular file with no group/other permissions, or
`serve` refuses to start.

**Rotation.** `stratad keygen --force` (then the `chown` again) replaces the key. Access tokens
signed with the old key stop working at once; clients refresh (refresh tokens are unaffected)
and continue. Restart `stratad` after rotating.

## 5. Create the first admin

```sh
read -rs PW && printf '%s\n' "$PW" | \
  sudo -u strata stratad --env-file /etc/strata/stratad.env create-user \
    --username owner --display-name 'Owner' --admin
```

The password is read from standard input (first line); it must meet
`STRATA_AUTH__MIN_PASSWORD_LENGTH` (default 10). The account is active immediately, its vault
`/srv/strata/users/<id>/vault` is created as a git repository, and a `user.create` audit entry
is written. Further accounts sign up in the app and wait for approval (Admin → Users), or an
admin creates them (`POST /api/v1/admin/users`).

## 6. Run

The systemd unit is `deploy/stratad.service` (shipped in the release tarball; install it as
`/etc/systemd/system/stratad.service`):

```ini
[Service]
User=strata
Group=strata
ExecStart=/usr/local/bin/stratad --env-file /etc/strata/stratad.env serve
Environment=RUST_LOG=info
Restart=on-failure
TimeoutStopSec=40
# No NoNewPrivileges: stratad runs claude through sudo (§9).
```

`RUST_LOG` belongs in the unit (or the environment), not in the env file: the file holds
`STRATA_` settings only. An override for one setting can go in a drop-in
(`systemctl edit stratad` → `Environment=STRATA_JOBS__MAX_CONCURRENCY=1`); it wins over the file.

Startup checks, in order: secret file permissions → signing key loads → data root exists →
database encoding UTF8 and `LC_CTYPE` not `C`/`POSIX` → revocation set loads. Any failure is
logged (JSON on stderr) and the process exits non-zero with the reason. `SIGTERM` shuts down
gracefully (in-flight requests get 30 s).

Run exactly **one** `stratad` process: revocations and rate limits are held in memory
(`docs/ARCHITECTURE.md` "Auth").

Liveness (GET): `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/api/v1/health`
prints `200` (use the port of `STRATA_BIND`).

**Manual deploy / upgrade** (until deploys are automated): download the release tarball and its
`.sha256` (§13: Releases → `latest` or the tag) to the VPS, verify it (`sha256sum -c stratad-*-linux-x86_64.tar.gz.sha256`), unpack it, then

```sh
install -m 0755 stratad /usr/local/bin/stratad
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
systemctl restart stratad
```

The env file stays in place across upgrades; compare it with the new `stratad.env.example`
for added settings (unknown or renamed ones stop `stratad` with the variable's name).

## 7. Account operations

- **Approve / reject sign-ups, disable, reset a password, change a role, schedule or cancel a
  deletion**: Admin → Users in the app (the `/api/v1/admin/users` endpoints). Every action is
  written to `audit_log`.
- **Password reset** returns a one-time temporary password (shown once); the user must change
  it at the next sign-in, and all their sessions end immediately.
- **Deletion** (D25): the account becomes `deletion_pending` for
  `STRATA_ACCOUNTS__DELETION_GRACE_DAYS` (default 14); the user can sign in only to download
  their export or confirm the deletion. The purge runs every
  `STRATA_ACCOUNTS__PURGE_INTERVAL_SECS` and removes the user's directory and
  every row of theirs, leaving one `user.purge` audit entry.
- **Audit trail**: `SELECT * FROM strata.audit_log ORDER BY at DESC LIMIT 50;` (as a
  superuser or `strata_accounts`).

## 8. Regenerating the API contract

After changing endpoints: `api/generate.sh` (writes `api/openapi.json` and the generated
client); CI runs `api/generate.sh --check`. `stratad openapi --out api/openapi.json` writes the
contract alone.

## 9. AI: `claude -p` as a dedicated user (L18, D20 = a, PLAN §9.1)

`stratad` runs every LLM call as a short-lived `claude -p` process (`strata_ai::ClaudeCliProvider`).
The process runs as a separate OS user, **`strata-ai`**, that is logged in to Claude Code and has
no access to vaults or the database; the model gets text on stdin and returns JSON on stdout,
and only `stratad` writes the vault (principle 2).

Measured on the dev container (Claude Code 2.1.283): one structured call ≈ 2 s wall clock,
≈ 0.75 s CPU, ≈ 220 MB peak RSS. Concurrency defaults to one process.

**What `stratad` passes** (verified against Claude Code 2.1.283; recheck after CLI upgrades):

```
claude -p --output-format stream-json --verbose --tools "" --strict-mcp-config \
  --setting-sources "" --disable-slash-commands --no-session-persistence \
  --system-prompt <strata prompt> [--model <m>] (--json-schema <schema> | --include-partial-messages)
```

`--tools ""` removes every built-in tool; `--strict-mcp-config` without `--mcp-config` loads no
MCP server; `--setting-sources ""` ignores user/project/local settings files (hooks, permissions);
the user content goes on stdin, never in argv. Bare mode (`--bare`, `CLAUDE_CODE_SIMPLE`) is never
used, because it ignores the subscription login. The child's environment is cleared and
rebuilt from an allow-list; `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_SIMPLE` and
the Bedrock/Vertex/Foundry switches are refused by config validation, so billing stays on the
subscription.

**Setup** (as root):

```sh
useradd --system --create-home --home-dir /var/lib/strata-ai --shell /usr/sbin/nologin strata-ai
install -d -o strata-ai -g strata-ai -m 0700 /var/lib/strata-ai/scratch   # empty working dir
# The data root stays private to the service user: this must fail with "Permission denied".
sudo -u strata-ai ls /srv/strata
```

Install Claude Code where `strata-ai` can run it (e.g. `/usr/local/bin/claude`), then log in
once as that user with the owner's subscription:

```sh
sudo -u strata-ai -H claude          # run /login, then /exit
```

(Credentials land in `/var/lib/strata-ai/.claude/`, readable only by `strata-ai`. Alternatively
create a long-lived token with `claude setup-token` and store it in a `0600` file owned by
`strata-ai` that the wrapper below exports as `CLAUDE_CODE_OAUTH_TOKEN`.)

Wrapper `/usr/local/lib/strata/claude-ai` (owner `root:root`, mode `0755`), the only command
`strata` may run as `strata-ai`:

```sh
#!/bin/sh
# Runs `claude` for stratad as strata-ai: fixed working dir, scrubbed environment.
cd /var/lib/strata-ai/scratch || exit 1
exec /usr/bin/env -i \
  HOME=/var/lib/strata-ai \
  PATH=/usr/local/bin:/usr/bin:/bin \
  LANG=C.UTF-8 \
  CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
  DISABLE_AUTOUPDATER=1 \
  /usr/local/bin/claude "$@"
```

`/etc/sudoers.d/strata-ai` (mode `0440`, check with `visudo -c`):

```
strata ALL=(strata-ai) NOPASSWD: /usr/local/lib/strata/claude-ai
```

Configure the launcher in the env file:

```sh
STRATA_AI__CLAUDE_CLI__COMMAND='sudo -n -u strata-ai /usr/local/lib/strata/claude-ai'
STRATA_AI__CLAUDE_CLI__SCRATCH_DIR=/var/lib/strata-ai/scratch
STRATA_AI__CLAUDE_CLI__LAUNCH_DIR=/
# defaults: STRATA_AI__CLAUDE_CLI__MAX_CONCURRENCY=1, STRATA_AI__CLAUDE_CLI__TIMEOUT_SECS=300
```

`stratad` starts the launcher in `LAUNCH_DIR`, not in the scratch directory: that one is
`strata-ai`'s (`0700`), so `strata` cannot enter it and every call would fail with `failed to
start claude: PermissionDenied`; the wrapper changes into it as `strata-ai`. Without a launcher
(`COMMAND` is `claude` itself, development) leave `LAUNCH_DIR` unset: the process then starts in
`SCRATCH_DIR`. At startup `stratad` warns when the directory it starts `claude` in is missing or
cannot be entered.

On timeout `stratad` sends SIGTERM to the process group (sudo relays it to `claude`), then
SIGKILL after `STRATA_AI__CLAUDE_CLI__KILL_GRACE_SECS`. Because `claude` runs through `sudo`,
the systemd unit must not set `NoNewPrivileges`. By default every account uses this provider (D23); see §11
for per-user overrides.

Check the whole chain as `strata` (spends a tiny amount of subscription usage):

```sh
echo 'Say OK' | sudo -u strata sudo -n -u strata-ai /usr/local/lib/strata/claude-ai \
  -p --output-format json --tools "" --setting-sources "" --no-session-persistence
```

**Usage limits.** The subscription is shared with the owner's interactive Claude Code use. When
the CLI reports a usage limit (`rate_limit_event` with `status: "rejected"`, an API 429, or a
"usage limit / hit your limit" message), the provider pauses until the reported reset time (or
30 minutes when none is given); jobs wait and resume, and `GET /ai/status` shows
`paused: provider_usage_limit` with the time. No process is started while paused.

**Outages.** When `claude` cannot start, times out or its login is broken, jobs retry with
backoff (30 s doubling, up to `max_attempts`) and then fail, marked as provider failures. Once an
AI job succeeds again, that user's marked jobs are queued again at once and every other user's at
the next hourly scheduler pass. Any failed job can also be retried by its owner (Settings → AI →
Retry, `POST /ai/jobs/retry`); `GET /ai/status` counts them in `failed_jobs`. To requeue by hand
in `psql`, `UPDATE strata.jobs SET status = 'queued', attempts = 0, run_after = now(),
provider_failure = false WHERE status = 'failed' AND …;` works from any session (the wakeup
trigger pins its own `search_path`).

## 10. AI: local embeddings (L19, D9 = a, PLAN §9.1b)

Model: `ibm-granite/granite-embedding-97m-multilingual-r2` (Apache-2.0, ModernBERT, 384
dimensions, **CLS pooling + L2 normalisation** per its model card). The Hugging Face repository
ships ONNX exports; Strata uses the int8 one, `onnx/model_quint8_avx2.onnx` (≈ 94 MiB, needs
AVX2), with `tokenizer.json`. Inputs `input_ids`, `attention_mask`; output `last_hidden_state`.

```sh
install -d -o strata -g strata -m 0755 /opt/models
cd /opt/models && git lfs install && \
  git clone --depth 1 https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2
# only these files are needed:
#   onnx/model_quint8_avx2.onnx  tokenizer.json  1_Pooling/config.json  config.json
```

If a release ever lacks the ONNX files, export and quantise with Optimum (on a workstation):

```sh
pip install "optimum[onnxruntime]" sentence-transformers
optimum-cli export onnx --model ibm-granite/granite-embedding-97m-multilingual-r2 \
  --task feature-extraction granite-onnx/
optimum-cli onnxruntime quantize --onnx_model granite-onnx/ --avx2 -o granite-onnx-int8/
# → granite-onnx-int8/model_quantized.onnx; point the embedder's model file at it and give the
#   vectors a new model id (a model change triggers a full re-embed).
```

ONNX Runtime is loaded at run time (the `ort` crate's `load-dynamic`; ≥ 1.22, tested with
1.30.0). Take `libonnxruntime.so.<version>` from the official release tarball
(`onnxruntime-linux-x64-<version>.tgz`, `lib/`) or from the `onnxruntime` wheel on PyPI
(`onnxruntime/capi/`), install it as e.g. `/opt/onnxruntime/lib/libonnxruntime.so.1.30.0`, and
delete the downloaded archive.

Point `stratad` at the files (restart to apply); until both paths exist, embeddings are off and
startup logs `embeddings disabled` with the reason:

```sh
STRATA_AI__EMBEDDING__MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2
STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0
```

Runtime behaviour: one worker thread at nice 19, ONNX Runtime with one intra-op and one inter-op
thread, one embedding call at a time and never while a `claude -p` process runs; texts are
truncated to 2048 tokens (default). Measured on the dev container with the quint8 export and one
thread: a 2 700-token text takes ≈ 10 s and ≈ 1.9 GB peak RSS (attention grows quadratically), so
long notes must be chunked; short texts take milliseconds.

The quint8 export's output changes when a text is padded inside a batch (cosine 0.96–0.99 to the
same text embedded alone; the fp32 export gives 1.0), so the embedder only batches texts of equal
token length (`STRATA_AI__EMBEDDING__PAD_BATCHES=false`).

Smoke test against the real model (compares with reference vectors from the Python
`onnxruntime` + `tokenizers` packages, `backend/crates/ai/tests/fixtures/embeddings/`):

```sh
STRATA_EMBED_MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2 \
STRATA_ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0 \
  cargo test -p strata-ai --test onnx_real -- --ignored
```

## 11. AI: Anthropic API provider (D20, D23)

Selectable per user (`STRATA_AI__USER_PROVIDERS=<username>=anthropic_api`, several pairs
separated by commas, the username exactly as stored) or as the default
(`STRATA_AI__DEFAULT_PROVIDER=anthropic_api`). The key lives in a file
(`STRATA_AI__ANTHROPIC_API__API_KEY_FILE`, e.g. `/etc/strata/anthropic.key`), owner `strata`,
mode `0600`;
`stratad` refuses to start when a user is routed to the API without a key file, or when the file
is readable by group or others, and never logs the key. Default model `claude-opus-5-5`
(`STRATA_AI__ANTHROPIC_API__MODEL`, with its prices in `…__INPUT_MICROS_PER_MTOK` /
`…__OUTPUT_MICROS_PER_MTOK` for cost caps); transient errors (408/409/429/5xx/529) are retried with exponential
backoff honouring `retry-after`; a 429 that outlasts the retries pauses that user's AI work.

Budgets (`STRATA_BUDGETS__…`): per-user and global daily token and cost caps, counted in
calendar days of `STRATA_BUDGETS__TIMEZONE` (default `STRATA_DEFAULT_TIMEZONE`); totals are in `strata.ai_usage` (per user) and `strata.ai_usage_global` (per
day). A reached cap pauses AI work until the next day starts; nothing fails.

```sql
SELECT * FROM strata.ai_usage_global ORDER BY day DESC LIMIT 7;
```

## 12. AI: background jobs, fp32 embeddings, Ask (PLAN §9.1b, §9.2, §9.5)

### The fp32 model (default)
Since the owner decision of 2026-09-27 the embedder uses the full-precision export
`onnx/model.onnx` of the same repository and revision as §10 (≈ 390 MB). Fetch it next to the
other files and verify it against the checksum the repository states (the Git LFS pointer of the
file carries its SHA-256):

```sh
cd /opt/models/granite-embedding-97m-multilingual-r2
REV=835ad14087e140460703cf0fae09f97d469d65c2
BASE=https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2
curl -fsSL "$BASE/raw/$REV/onnx/model.onnx" | grep '^oid sha256:' | cut -d: -f2 > /tmp/model.onnx.sha256
curl -fL -o onnx/model.onnx.part "$BASE/resolve/$REV/onnx/model.onnx"
echo "$(cat /tmp/model.onnx.sha256)  onnx/model.onnx.part" | sha256sum -c - \
  && mv onnx/model.onnx.part onnx/model.onnx
echo "$(cat /tmp/model.onnx.sha256)  onnx/model.onnx" >> SHA256SUMS
rm -f /tmp/model.onnx.sha256
```

The defaults (`deploy/stratad.env.example`) then need only the two paths of §10. The model ID
stored with every vector changes (`…@onnx/model`), so the first start after switching queues a
resumable re-embed of every note (below). To stay on the int8 file, set
`STRATA_AI__EMBEDDING__MODEL_FILE=onnx/model_quint8_avx2.onnx`,
`STRATA_AI__EMBEDDING__MODEL_ID=ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model_quint8_avx2`
and `STRATA_AI__EMBEDDING__PAD_BATCHES=false`.

The model is not loaded at startup: the first embedding loads it (a second or two) and it is
unloaded after `STRATA_AI__EMBEDDING__IDLE_UNLOAD_SECS` (default 300) without calls. Real-model checks
(fp32 padding invariance and agreement with the quint8 reference; load → idle unload → reload):

```sh
STRATA_EMBED_MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2 \
STRATA_ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0 \
  cargo test -p strata-ai --test onnx_real -- --ignored
```

### Background jobs
`stratad serve` runs the job runner (`STRATA_JOBS__…`: `MAX_CONCURRENCY` 2,
`POLL_INTERVAL_SECS`, `BACKOFF_BASE_SECS`/`BACKOFF_MAX_SECS`, `NIGHTLY_HOUR` 3 in the default
timezone, `SHUTDOWN_GRACE_SECS`). Kinds: `embed` (after each content change), `embed_backfill` (at every
start when embeddings are configured, until every note is current), `summarize` (after `embed`;
sidecar summary, `ai: summarize <path>`), `dedupe` (nightly semantic duplicate sweep producing
`duplicates` suggestions). Jobs of kinds without a handler yet (`link`, `file_inbox`) stay
queued. A reached budget or provider usage limit pauses LLM jobs (they wait, attempts are not
spent); `STRATA_AI__DAILY_JOB_LIMIT` caps LLM jobs per UTC day (in memory, reset at restart).
Jobs left `running` by a stopped process are re-queued at the next start.

Inspect a user's queue (as the owner role, inside the user's scope):

```sql
BEGIN;
SELECT set_config('strata.user_id', '<user uuid>', true);
SELECT kind, status, count(*), min(run_after) FROM strata.jobs GROUP BY 1, 2 ORDER BY 1, 2;
SELECT kind, attempts, last_error, run_after FROM strata.jobs WHERE status = 'failed' ORDER BY updated DESC LIMIT 20;
-- embedding coverage with the current model
SELECT count(v.note_id) AS embedded, count(*) AS notes FROM strata.notes n
  LEFT JOIN strata.note_vectors v ON v.note_id = n.id AND v.user_id = n.user_id
   AND v.content_hash = n.content_hash
 WHERE NOT n.trashed;
ROLLBACK;
```

`GET /api/v1/ai/status` shows the same for the signed-in user (queue depth, pause, usage,
embedding model loaded or not, embedded / total notes).

### Ask
`POST /api/v1/ask {question, scope?}` returns an answer ID; the client streams it from the
WebSocket `GET /api/v1/ask/{id}` (token batches, citations, `done`, `end`). Answers are kept in
memory for 30 minutes (lost at restart); `POST /api/v1/ask/{id}/save` writes one to `notes/`.
Citing a block without an ID appends `^ask-xxxxxx` to it in one `ai: ask <path>` commit
(revertible like any AI commit). `503 ai_paused` / `ai_unavailable` mean the budget or provider
limit is reached, or AI is off for the account.

## 13. Build artifacts (CI) and signing

`.github/workflows/build.yml` runs on pushes to `main` and `claude/**`, pull requests, `v*`
tags and by hand (Actions → Build → Run workflow). The artifacts are on the run's summary page
(kept 90 days by default). When all three build jobs are green, the final `release` job (pushes
and manual runs, not pull requests) publishes every file as a GitHub release: a `v*` tag gets
the release of that name, any other push replaces the rolling pre-release **`latest`** (its tag
moves to the new commit). Each app's `SHA256SUMS` is renamed `SHA256SUMS-<artifact>` there. The
repository is private, so a download on the VPS needs a token:
`GH_TOKEN=<token> gh release download latest -R Shawket4/Strata -p 'stratad-*'` (or copy the
files over).

| Artifact | Contents |
|---|---|
| `stratad-linux-x86_64` | `stratad-<version>-linux-x86_64.tar.gz` (the stripped `stratad`, `stratad.env.example`, `stratad.service`, `README.md`, `VPS_SETUP.md`) and its `.sha256`. |
| `strata-android-release-signed` or `strata-android-debug-signed` | `strata-<version>-android-universal-<kind>.apk`, one APK per ABI (`arm64-v8a`, `armeabi-v7a`, `x86_64`) and `SHA256SUMS`. |
| `strata-macos-universal-adhoc` | `Strata-<version>-macos-universal.zip` and `.dmg` (arm64 + x86_64, ad-hoc signed) and `SHA256SUMS`. |
| `strata-android-debug-symbols`, `strata-macos-debug-symbols` | The Dart split debug info of the obfuscated builds (`app.android-<arch>.symbols`, `app.darwin-<arch>.symbols`), kept 90 days. Not published with the release. |

`<version>` is the tag (`v0.1.0`) or `sha-<7 hex>` for other builds; Android and macOS builds
use the workflow run number as the build number (Android `versionCode`), so a newer CI build
installs over an older one.

**Release build settings.** The root `Cargo.toml` `[profile.release]` sets `lto = "thin"`,
`codegen-units = 1` and `strip = true` (`opt-level` stays 3; `panic` stays `unwind`, which
flutter_rust_bridge relies on). It applies to `stratad` and to the Rust core, which cargokit
builds with `cargo build --release` inside the workspace (no per-target overrides;
`.cargo/config.toml` only sets `incremental = false`). The Android and macOS Dart code is built
with `--obfuscate --split-debug-info=build/debug-info/<platform>`; the symbol files are
uploaded as `strata-<platform>-debug-symbols` (the `release` job skips `*-debug-symbols`
artifacts). Keep a copy of the symbols of any build you ship past 90 days. To read an
obfuscated Dart stack trace, download that run's symbols artifact and run
`flutter symbolize -i <stack.txt> -d <debug-info dir>/app.android-arm64.symbols` (the file
for the device's architecture, e.g. `app.darwin-arm64.symbols` on Apple silicon). The
symbols only match the exact build that produced them.

**App ID.** `com.shawket.strata` on every platform: Android `applicationId`/`namespace`
(`MainActivity` in `com/shawket/strata`), the iOS and macOS bundle identifier (test targets
`com.shawket.strata.RunnerTests`), the Linux `APPLICATION_ID` (desktop entry
`com.shawket.strata.desktop`, which also names the data directory
`~/.local/share/com.shawket.strata`) and the Windows notification app user model ID. The
workflow checks the APK package name and the macOS bundle identifier;
`brand_assets_test.dart` checks the platform files. APNs pushes use it as the topic
(`STRATA_PUSH__APNS_TOPIC=com.shawket.strata`).

**Server address.** The apps have no server field: every account uses one server, fixed at
build time with `--dart-define=STRATA_SERVER_URL=<url>` and never shown or editable (except
read-only under Settings → Account). The Build workflow sets
`STRATA_SERVER_URL: ${{ vars.STRATA_SERVER_URL || 'https://strata-ai.duckdns.org' }}` at the
workflow level, so the owner's server is the default and a repository **variable** of the same
name (Settings → Secrets and variables → Actions → **Variables**) overrides it; a value that is
not `https://…` fails the Android and macOS jobs. The value goes to the Rust core at startup
(`CoreConfig.server_url`, with `release_build`), which uses it for sign-up, sign-in, sync and
every other request. A blank address, one without `https://`, or plain `http://` stops the
core from opening (`misconfigured_build`) and the app shows a "This build can't start" screen
instead; plain `http://` to this device (`localhost`, `127.0.0.1`, `[::1]`) is accepted in debug
builds only, for testing through an SSH tunnel (`ssh -N -L 8080:127.0.0.1:8080 <vps>`, then
`flutter run --dart-define=STRATA_SERVER_URL=http://127.0.0.1:8080`). For a local release
build use the same flag, e.g.
`flutter build linux --release --dart-define=STRATA_SERVER_URL=https://strata-ai.duckdns.org`.

**stratad.** Built on Ubuntu 22.04 (glibc 2.35) so it runs on Debian 12+, Ubuntu 22.04+ and
other glibc ≥ 2.35 systems. It is deliberately not a static musl build: `ort` loads ONNX
Runtime at run time with `dlopen`. Deploy it as in §6 ("Manual deploy / upgrade") or, the first
time, [`deploy/VPS_SETUP.md`](../deploy/VPS_SETUP.md).

**Android.** A universal APK (installs on any device) plus one APK per ABI (about a third of
the size; phones need `arm64-v8a`). Signing is chosen per run:

- With the repository secrets below, release APKs are signed with your key
  (`strata-android-release-signed`). Create the key once and keep it safe (losing it means
  users must uninstall to update):

  ```sh
  keytool -genkeypair -v -keystore strata-release.jks -alias strata -keyalg RSA -keysize 4096 \
    -validity 10000
  base64 -w0 strata-release.jks     # the value of ANDROID_KEYSTORE_BASE64
  ```

  | Secret (Settings → Secrets and variables → Actions) | Value |
  |---|---|
  | `ANDROID_KEYSTORE_BASE64` | the keystore file, base64-encoded |
  | `ANDROID_KEYSTORE_PASSWORD` | the keystore password |
  | `ANDROID_KEY_ALIAS` | the key alias (`strata` above) |
  | `ANDROID_KEY_PASSWORD` | the key password |

- Without them (and for pull requests from forks), the APKs are signed with the runner's
  debug key (`strata-android-debug-signed`, with a warning in the run). They sideload fine, but
  every run has a different debug key, so installing a newer one over an older one fails with
  a signature mismatch: uninstall first. `android/app/build.gradle.kts` reads the keystore
  from `ANDROID_KEYSTORE_PATH` (CI decodes the secret to a temporary file); no keystore is
  committed.

Install: copy the APK to the phone and open it (allow "install unknown apps" for the file
manager), or `adb install -r strata-…-arm64-v8a-….apk`.

**macOS.** A universal app (cargokit builds the Rust core for every architecture of the
release build and joins them with `lipo`; the workflow checks both are present), ad-hoc
signed (`codesign --force --deep --sign -`, entitlements kept) and verified with
`codesign --verify --deep --strict`. It is not notarised, so Gatekeeper blocks the first
launch ("cannot be opened because the developer cannot be verified"). Either:

- right-click (Control-click) `Strata.app` → **Open** → **Open**, once; or
- remove the quarantine flag: `xattr -dr com.apple.quarantine /Applications/Strata.app`.

On macOS 15, if the right-click route shows no Open button, use System Settings → Privacy &
Security → "Open Anyway" after the first attempt.

**App icons and splash screens** come from `design/brand/*.svg`:
`client/app/apps/strata/tool/brand/generate.sh` renders the source PNGs into
`apps/strata/assets/brand/` (plus the 9-size Windows `.ico` and the Linux 256 px icon), then
runs `flutter_launcher_icons` (a pinned global tool: its `cli_util` constraint conflicts with
melos) and `flutter_native_splash` (a dev dependency). The generated platform files are
committed; `apps/strata/test/brand_assets_test.dart` checks every size and that every native
launch colour equals the Flutter splash background (mist light, abyss dark).

## 14. Later: Docker

Not built yet (owner decision 2026-09-28). The plan:

- The image contains **only the backend** (`stratad` on a slim glibc base, e.g.
  `debian:bookworm-slim`, running as a non-root user). No PostgreSQL, model or ONNX Runtime
  inside.
- **PostgreSQL stays on the host**, shared with the other containers: the container reaches
  it over the host network or a Unix socket mount, with the same three role URLs.
- The **embedding model and ONNX Runtime stay on the host** and are mounted **read-only**
  (e.g. `-v /opt/models:/opt/models:ro -v /opt/onnxruntime:/opt/onnxruntime:ro`); embeddings
  remain in-process (D9 unchanged), so `STRATA_AI__EMBEDDING__MODEL_DIR` and
  `…__ONNXRUNTIME_LIB` point at the mounts.
- The data root is a volume; settings come from the same env file, mounted read-only and
  read by `stratad --env-file` (not `docker run --env-file`, which does not understand the
  quoting of §1); the signing key and other secret files are mounted read-only with mode
  `0600`.
- `claude -p` needs its own design for containers (it runs through `sudo` as `strata-ai` on
  the host today).

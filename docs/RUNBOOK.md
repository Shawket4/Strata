# Runbook

Operating `stratad` on the VPS (PLAN §14). Commands assume the service user `strata`, the
config at `/etc/strata/stratad.toml` and the data root `/srv/strata`. Every subcommand takes
`--config <path>` (or `STRATA_CONFIG`); `STRATA__<SECTION>__<KEY>` environment variables
override single keys (e.g. `STRATA__DATABASE__APP_URL`).

## 1. Configuration

Minimal `/etc/strata/stratad.toml` (owner `root:strata`, mode `0640`; it holds database
passwords):

```toml
data_root = "/srv/strata"
bind = "127.0.0.1:8080"
default_timezone = "Africa/Cairo"

[database]
owner_url = "postgres://strata_owner:…@127.0.0.1/strata"
app_url = "postgres://strata_app:…@127.0.0.1/strata"
accounts_url = "postgres://strata_accounts:…@127.0.0.1/strata"
max_connections = 5

[auth]
signing_key_file = "/etc/strata/token-signing-key.pem"
trust_forwarded_for = true   # only because nginx is the sole client of 127.0.0.1:8080
```

With `trust_forwarded_for`, the login and sign-up rate limits key on the address nginx
reports, so nginx must **overwrite** the header rather than append to what the client sent:
`proxy_set_header X-Forwarded-For $remote_addr;` (not `$proxy_add_x_forwarded_for`) and no
`Forwarded` header passed through.

`max_future_skew_secs` (default 300) is how far ahead of the server's clock a device's creation
time may be before a create is refused with `422 created_in_future`: a device whose clock is
wrong sees its creates rejected until the clock is fixed; items created offline in the past are
always accepted with their own time.

Every key, with its default and a comment, is in `deploy/stratad.example.toml` (a test keeps it
in step with `strata_common::Config::default`); start from it and change what differs.

The data root must exist and belong to the service user only:

```sh
install -d -o strata -g strata -m 0700 /srv/strata
```

## 2. Create the database (UTF-8, non-C locale)

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
sudo -u strata stratad -c /etc/strata/stratad.toml bootstrap-roles \
  --superuser-url 'postgres://postgres@/postgres?host=/var/run/postgresql'
```

or review and run the SQL yourself (connect to the `strata` database):

```sh
stratad -c /etc/strata/stratad.toml bootstrap-roles --print > bootstrap.sql
sudo -u postgres psql -d strata -f bootstrap.sql
```

Then apply the migrations as `strata_owner` (safe to re-run; run after every upgrade):

```sh
sudo -u strata stratad -c /etc/strata/stratad.toml migrate
```

## 4. Generate the token signing key

Access tokens are signed with an Ed25519 key (PKCS#8 PEM). Create it once, as the service
user, at the configured path:

```sh
sudo install -d -o strata -g strata -m 0700 /etc/strata/keys   # if you keep keys apart
sudo -u strata stratad -c /etc/strata/stratad.toml keygen
```

The file is written with mode `0600`; `keygen` refuses to overwrite an existing key. Every
secret file (`auth.signing_key_file`, `ai.anthropic_api.api_key_file`, push credentials) must be a regular
file with no group/other permissions, or `serve` refuses to start.

**Rotation.** `stratad keygen --force` replaces the key. Access tokens signed with the old key
stop working at once; clients refresh (refresh tokens are unaffected) and continue. Restart
`stratad` after rotating.

## 5. Create the first admin

```sh
read -rs PW && printf '%s\n' "$PW" | \
  sudo -u strata stratad -c /etc/strata/stratad.toml create-user \
    --username owner --display-name 'Owner' --admin
```

The password is read from standard input (first line); it must meet
`auth.min_password_length` (default 10). The account is active immediately, its vault
`/srv/strata/users/<id>/vault` is created as a git repository, and a `user.create` audit entry
is written. Further accounts sign up in the app and wait for approval (Admin → Users), or an
admin creates them (`POST /api/v1/admin/users`).

## 6. Run

systemd unit (excerpt):

```ini
[Service]
User=strata
Environment=STRATA_CONFIG=/etc/strata/stratad.toml
Environment=RUST_LOG=info
ExecStart=/usr/local/bin/stratad serve
Restart=on-failure
```

Startup checks, in order: secret file permissions → signing key loads → data root exists →
database encoding UTF8 and `LC_CTYPE` not `C`/`POSIX` → revocation set loads. Any failure is
logged (JSON on stderr) and the process exits non-zero with the reason. `SIGTERM` shuts down
gracefully (in-flight requests get 30 s).

Run exactly **one** `stratad` process: revocations and rate limits are held in memory
(`docs/ARCHITECTURE.md` "Auth").

Liveness: `curl -H 'Accept: application/vnd.msgpack' http://127.0.0.1:8080/api/v1/health`.

## 7. Account operations

- **Approve / reject sign-ups, disable, reset a password, change a role, schedule or cancel a
  deletion**: Admin → Users in the app (the `/api/v1/admin/users` endpoints). Every action is
  written to `audit_log`.
- **Password reset** returns a one-time temporary password (shown once); the user must change
  it at the next sign-in, and all their sessions end immediately.
- **Deletion** (D25): the account becomes `deletion_pending` for `accounts.deletion_grace_days`
  (default 14); the user can sign in only to download their export or confirm the deletion.
  The purge runs every `accounts.purge_interval_secs` and removes the user's directory and
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

Configure the launcher in `stratad.toml`:

```toml
[ai.claude_cli]
command = ["sudo", "-n", "-u", "strata-ai", "/usr/local/lib/strata/claude-ai"]
scratch_dir = "/var/lib/strata-ai/scratch"
max_concurrency = 1
timeout_secs = 300
```

On timeout `stratad` sends SIGTERM to the process group (sudo relays it to `claude`), then
SIGKILL after `kill_grace_secs`. By default every account uses this provider (D23); see §11
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

```toml
[ai.embedding]
model_dir = "/opt/models/granite-embedding-97m-multilingual-r2"
onnxruntime_lib = "/opt/onnxruntime/lib/libonnxruntime.so.1.30.0"
```

Runtime behaviour: one worker thread at nice 19, ONNX Runtime with one intra-op and one inter-op
thread, one embedding call at a time and never while a `claude -p` process runs; texts are
truncated to 2048 tokens (default). Measured on the dev container with the quint8 export and one
thread: a 2 700-token text takes ≈ 10 s and ≈ 1.9 GB peak RSS (attention grows quadratically), so
long notes must be chunked; short texts take milliseconds.

The quint8 export's output changes when a text is padded inside a batch (cosine 0.96–0.99 to the
same text embedded alone; the fp32 export gives 1.0), so the embedder only batches texts of equal
token length (`pad_batches = false`).

Smoke test against the real model (compares with reference vectors from the Python
`onnxruntime` + `tokenizers` packages, `backend/crates/ai/tests/fixtures/embeddings/`):

```sh
STRATA_EMBED_MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2 \
STRATA_ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0 \
  cargo test -p strata-ai --test onnx_real -- --ignored
```

## 11. AI: Anthropic API provider (D20, D23)

Selectable per user (`[ai.user_providers]`, `<username> = "anthropic_api"`, the username exactly
as stored) or as the default (`ai.default_provider`). The key lives in a file
(`ai.anthropic_api.api_key_file`, e.g. `/etc/strata/anthropic.key`), owner `strata`, mode `0600`;
`stratad` refuses to start when a user is routed to the API without a key file, or when the file
is readable by group or others, and never logs the key. Default model `claude-opus-5-5`
(`ai.anthropic_api.model`, with its prices in `input_micros_per_mtok` / `output_micros_per_mtok`
for cost caps); transient errors (408/409/429/5xx/529) are retried with exponential
backoff honouring `retry-after`; a 429 that outlasts the retries pauses that user's AI work.

Budgets (`[budgets]`): per-user and global daily token and cost caps, counted in calendar days
of `budgets.timezone` (default `default_timezone`); totals are in `strata.ai_usage` (per user) and `strata.ai_usage_global` (per
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

The defaults (`deploy/stratad.example.toml`) then need only the two paths of §10. The model ID
stored with every vector changes (`…@onnx/model`), so the first start after switching queues a
resumable re-embed of every note (below). To stay on the int8 file, set `model_file =
"onnx/model_quint8_avx2.onnx"`, `model_id =
"ibm-granite/granite-embedding-97m-multilingual-r2@onnx/model_quint8_avx2"` and `pad_batches =
false`.

The model is not loaded at startup: the first embedding loads it (a second or two) and it is
unloaded after `ai.embedding.idle_unload_secs` (default 300) without calls. Real-model checks
(fp32 padding invariance and agreement with the quint8 reference; load → idle unload → reload):

```sh
STRATA_EMBED_MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2 \
STRATA_ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0 \
  cargo test -p strata-ai --test onnx_real -- --ignored
```

### Background jobs
`stratad serve` runs the job runner (`[jobs]`: `max_concurrency` 2, `poll_interval_secs`,
`backoff_base_secs`/`backoff_max_secs`, `nightly_hour` 3 in `default_timezone`,
`shutdown_grace_secs`). Kinds: `embed` (after each content change), `embed_backfill` (at every
start when embeddings are configured, until every note is current), `summarize` (after `embed`;
sidecar summary, `ai: summarize <path>`), `dedupe` (nightly semantic duplicate sweep producing
`duplicates` suggestions). Jobs of kinds without a handler yet (`link`, `file_inbox`) stay
queued. A reached budget or provider usage limit pauses LLM jobs (they wait, attempts are not
spent); `ai.daily_job_limit` caps LLM jobs per UTC day (in memory, reset at restart).
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

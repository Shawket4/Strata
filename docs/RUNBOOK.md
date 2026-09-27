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

Defaults for everything else (token lifetimes, Argon2 cost, rate limits, deletion grace
period, purge and revocation-reload intervals) are in `strata_common::Config::default` and
`docs/ARCHITECTURE.md` "Auth".

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
secret file (`auth.signing_key_file`, `ai.api_key_file`, push credentials) must be a regular
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

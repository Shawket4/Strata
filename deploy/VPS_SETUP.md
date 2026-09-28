# VPS setup (first install, manual deploy)

Everything to do on the VPS to run `stratad` v0.1. Assumes Debian/Ubuntu, root access, and an
existing PostgreSQL cluster (the one with your WAL/full backups). Paths match
`docs/RUNBOOK.md`; every setting is in `deploy/stratad.env.example`.

Layout when done:

| What | Where | Owner / mode |
|---|---|---|
| Binary | `/usr/local/bin/stratad` | root, 0755 |
| Settings | `/etc/strata/stratad.env` | root:strata, 0640 |
| Token signing key | `/etc/strata/token-signing-key.pem` | strata, 0600 |
| Vaults (data root) | `/srv/strata` | strata, 0700 |
| Embedding model | `/opt/models/granite-embedding-97m-multilingual-r2` | root, read-only for strata |
| ONNX Runtime | `/opt/onnxruntime/lib/libonnxruntime.so.1.30.0` | root, read-only |
| claude -p user | `strata-ai`, home `/var/lib/strata-ai` | strata-ai, 0700 |

## 0. Base packages and time

```sh
apt update && apt install -y nginx sudo curl ca-certificates chrony ufw
timedatectl set-ntp true        # device creation times are checked against this clock
ufw allow OpenSSH && ufw allow 80,443/tcp && ufw enable   # 8080 stays private
```

No system `git` is needed (libgit2 is built into the binary).

## 1. Service user and directories

```sh
useradd --system --home-dir /srv/strata --shell /usr/sbin/nologin strata
install -d -o strata -g strata -m 0700 /srv/strata
install -d -o root -g strata -m 0750 /etc/strata
```

## 2. PostgreSQL: extensions, database, roles

Needs PostgreSQL 16 (tested) with **pgvector ≥ 0.6** and **pg_trgm** available:

```sh
apt install -y postgresql-16-pgvector        # match your major version; pg_trgm ships with postgresql-contrib
```

Create the database with a UTF-8, non-`C` character locale (required; `serve` refuses otherwise):

```sh
sudo -u postgres psql -c "CREATE DATABASE strata ENCODING 'UTF8' LC_COLLATE 'C.UTF-8' LC_CTYPE 'C.UTF-8' TEMPLATE template0;"
```

Pick three strong passwords (letters and digits only avoids URL-encoding) and put them in the
env file first (step 4). Then create the roles, extensions and grants. Either review the SQL:

```sh
stratad --env-file /etc/strata/stratad.env bootstrap-roles --print > /tmp/bootstrap.sql
sudo -u postgres psql -d strata -f /tmp/bootstrap.sql && rm /tmp/bootstrap.sql
```

or apply it directly with a superuser URL:

```sh
stratad --env-file /etc/strata/stratad.env bootstrap-roles \
  --superuser-url 'postgres://postgres@/postgres?host=/var/run/postgresql'
```

Roles created: `strata_owner` (migrations), `strata_app` (requests and jobs, `NOBYPASSRLS`),
`strata_accounts` (accounts only). Your existing WAL/full backups cover this database like any
other.

If `pg_hba.conf` requires passwords for local TCP, the three roles need a `host strata ...
127.0.0.1/32 scram-sha-256` line (reload PostgreSQL after editing).

## 3. The binary

From CI: download the `stratad-linux-x86_64` artifact of a green `build` workflow run (a zip
holding `stratad-<version>-linux-x86_64.tar.gz` and its `.sha256`), or the two files from the
draft GitHub Release of a `v*` tag. Copy them to the VPS, check and install:

```sh
sha256sum -c stratad-*-linux-x86_64.tar.gz.sha256
tar xzf stratad-*-linux-x86_64.tar.gz
cd stratad-*-linux-x86_64/     # stratad, stratad.env.example, stratad.service, README.md, VPS_SETUP.md
install -m 0755 stratad /usr/local/bin/stratad
stratad --version
```

The binary needs glibc ≥ 2.35 (Debian 12+, Ubuntu 22.04+).

(Or build it yourself on an Ubuntu 22.04+ machine: `cargo build --release -p stratad`.)

## 4. Settings (`/etc/strata/stratad.env`)

```sh
install -o root -g strata -m 0640 stratad.env.example /etc/strata/stratad.env
```

Each setting is one `STRATA_…` line; the file lists all of them with their defaults (commented
lines are defaults you can leave alone). Single-quote any value that contains spaces, `#`,
`$` or quotes.

Set at least:

```sh
STRATA_DATA_ROOT=/srv/strata
STRATA_BIND=127.0.0.1:8080
STRATA_DEFAULT_TIMEZONE=Africa/Cairo
STRATA_DATABASE__OWNER_URL='postgres://strata_owner:PASSWORD1@127.0.0.1/strata'
STRATA_DATABASE__APP_URL='postgres://strata_app:PASSWORD2@127.0.0.1/strata'
STRATA_DATABASE__ACCOUNTS_URL='postgres://strata_accounts:PASSWORD3@127.0.0.1/strata'
STRATA_AUTH__SIGNING_KEY_FILE=/etc/strata/token-signing-key.pem
STRATA_AUTH__TRUST_FORWARDED_FOR=true
STRATA_AI__CLAUDE_CLI__COMMAND='sudo -n -u strata-ai /usr/local/lib/strata/claude-ai'
STRATA_AI__CLAUDE_CLI__SCRATCH_DIR=/var/lib/strata-ai/scratch
STRATA_AI__EMBEDDING__MODEL_DIR=/opt/models/granite-embedding-97m-multilingual-r2
STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB=/opt/onnxruntime/lib/libonnxruntime.so.1.30.0
```

Validate (prints every effective setting, passwords masked, connects nowhere):

```sh
stratad --env-file /etc/strata/stratad.env check-config
```

## 5. Migrations, signing key, first admin

```sh
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
stratad --env-file /etc/strata/stratad.env keygen          # as root: /etc/strata is not writable by strata
chown strata:strata /etc/strata/token-signing-key.pem
read -rs PW && printf '%s\n' "$PW" | sudo -u strata stratad --env-file /etc/strata/stratad.env \
  create-user --username owner --display-name 'Shawket' --admin
```

`keygen` writes the key with mode 0600 and never overwrites one. Run `migrate` after every
upgrade.

## 6. Embeddings: model and ONNX Runtime (files on the VPS, loaded in-process)

```sh
install -d -m 0755 /opt/models/granite-embedding-97m-multilingual-r2/onnx /opt/models/granite-embedding-97m-multilingual-r2/1_Pooling
cd /opt/models/granite-embedding-97m-multilingual-r2
REV=835ad14087e140460703cf0fae09f97d469d65c2
BASE=https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2
for f in tokenizer.json config.json 1_Pooling/config.json; do curl -fL -o "$f" "$BASE/resolve/$REV/$f"; done
curl -fsSL "$BASE/raw/$REV/onnx/model.onnx" | grep '^oid sha256:' | cut -d: -f2 > /tmp/model.sha256
curl -fL -o onnx/model.onnx.part "$BASE/resolve/$REV/onnx/model.onnx"      # ≈ 390 MB
echo "$(cat /tmp/model.sha256)  onnx/model.onnx.part" | sha256sum -c - && mv onnx/model.onnx.part onnx/model.onnx
rm /tmp/model.sha256

install -d -m 0755 /opt/onnxruntime/lib
cd /tmp && curl -fLO https://github.com/microsoft/onnxruntime/releases/download/v1.30.0/onnxruntime-linux-x64-1.30.0.tgz
tar xzf onnxruntime-linux-x64-1.30.0.tgz
install -m 0644 onnxruntime-linux-x64-1.30.0/lib/libonnxruntime.so.1.30.0 /opt/onnxruntime/lib/
rm -rf onnxruntime-linux-x64-1.30.0*
```

The model is loaded on the first embedding and unloaded after 5 idle minutes (≈ 390 MB while
loaded). Until both paths exist, embeddings are off and startup logs `embeddings disabled`.

## 7. AI: `claude -p` as a separate user

```sh
useradd --system --create-home --home-dir /var/lib/strata-ai --shell /usr/sbin/nologin strata-ai
install -d -o strata-ai -g strata-ai -m 0700 /var/lib/strata-ai/scratch
sudo -u strata-ai ls /srv/strata      # must fail: "Permission denied"
```

Install Claude Code system-wide so it is `/usr/local/bin/claude` (for example
`npm install -g @anthropic-ai/claude-code` with Node 18+, or the native installer followed by
a copy/symlink into `/usr/local/bin`), then log in once as `strata-ai` with your subscription:

```sh
sudo -u strata-ai -H claude           # /login, then /exit
```

Wrapper `/usr/local/lib/strata/claude-ai` (root:root, 0755):

```sh
install -d -m 0755 /usr/local/lib/strata
cat > /usr/local/lib/strata/claude-ai <<'EOF'
#!/bin/sh
cd /var/lib/strata-ai/scratch || exit 1
exec /usr/bin/env -i HOME=/var/lib/strata-ai PATH=/usr/local/bin:/usr/bin:/bin LANG=C.UTF-8 \
  CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 DISABLE_AUTOUPDATER=1 /usr/local/bin/claude "$@"
EOF
chmod 0755 /usr/local/lib/strata/claude-ai
echo 'strata ALL=(strata-ai) NOPASSWD: /usr/local/lib/strata/claude-ai' > /etc/sudoers.d/strata-ai
chmod 0440 /etc/sudoers.d/strata-ai && visudo -c
```

Check the chain (uses a tiny bit of subscription):

```sh
echo 'Say OK' | sudo -u strata sudo -n -u strata-ai /usr/local/lib/strata/claude-ai \
  -p --output-format json --tools "" --setting-sources "" --no-session-persistence
```

## 8. systemd

Install the unit shipped in the tarball (`stratad.service`, the same as `deploy/stratad.service`):

```sh
install -m 0644 stratad.service /etc/systemd/system/stratad.service
```

It reads:

```ini
[Unit]
Description=Strata API
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
User=strata
Group=strata
ExecStart=/usr/local/bin/stratad --env-file /etc/strata/stratad.env serve
Environment=RUST_LOG=info
Restart=on-failure
RestartSec=5
TimeoutStopSec=40
PrivateTmp=yes
ProtectHome=yes
# No NoNewPrivileges: stratad runs claude through sudo (step 7).

[Install]
WantedBy=multi-user.target
```

```sh
systemctl daemon-reload && systemctl enable --now stratad
journalctl -u stratad -f          # JSON logs; startup checks are listed in RUNBOOK §6
curl -s -o /dev/null -w '%{http_code}\n' -H 'Accept: application/vnd.msgpack' http://127.0.0.1:8080/api/v1/health   # 200
```

Run exactly one `stratad` (rate limits and revocations are in memory).

## 9. nginx and TLS

Do this **before** building the apps people install: CI bakes the HTTPS address in as the
default server.

1. At your DNS provider, add an `A` record for the domain (e.g. `strata.example.com`) → `187.124.33.153`
   (and an `AAAA` record if the VPS has IPv6). Check: `dig +short strata.example.com` prints the IP.
2. `apt install -y certbot python3-certbot-nginx`, then create `/etc/nginx/sites-available/strata`
   with your domain in place of `strata.example.com`:

```nginx
map $http_upgrade $connection_upgrade { default upgrade; '' close; }

server {
    listen 80;
    server_name strata.example.com;
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl http2;
    server_name strata.example.com;
    # ssl_certificate lines are added by certbot

    add_header Strict-Transport-Security "max-age=31536000" always;
    client_max_body_size 512m;          # vault import zips

    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;   # overwrite, never append
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;          # /events and Ask streams (WebSocket)
        proxy_set_header Connection $connection_upgrade;
        proxy_buffering off;
        proxy_read_timeout 1h;
        proxy_send_timeout 1h;
    }
}
```

```sh
ln -s /etc/nginx/sites-available/strata /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx
certbot --nginx -d strata.example.com     # issues the certificate and sets up renewal
```

In the app, sign in with server `https://strata.example.com`.

Then check from your machine: `curl -sI https://strata.example.com/api/v1/health` returns
`200`, and set the GitHub repository variable `STRATA_DEFAULT_SERVER` to
`https://strata.example.com` (Settings → Secrets and variables → Actions → **Variables**, a
variable, not a secret), then re-run the Build workflow. CI passes it to the apps with
`--dart-define=STRATA_DEFAULT_SERVER=…`: sign-in and sign-up start with it filled in (still
editable). Unset, the server field starts empty; a value that is not `https://…` fails the
build (RUNBOOK §13).

**Before the domain exists:** the app refuses plain `http://` addresses except this device
(`localhost`, `127.0.0.1`, `[::1]`), so `http://187.124.33.153` does not work. Test from the
macOS app through an SSH tunnel (`ssh -N -L 8080:127.0.0.1:8080 you@vps`, server
`http://127.0.0.1:8080`). Android blocks plain HTTP as well, so it needs the domain and HTTPS.

## 10. Upgrades

```sh
install -m 0755 stratad /usr/local/bin/stratad
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
systemctl restart stratad
```

## 11. Checks after install

- `systemctl status stratad` is active; the log has no `embeddings disabled` line.
- Sign in as `owner` in the app; create a note; the vault has a commit:
  `sudo -u strata git -C /srv/strata/users/<id>/vault log --oneline | head` (needs `apt install git`, optional).
- AI status in Settings → AI shows the provider ready and embeddings loaded after the first note.
- Vault files are not in your PostgreSQL backups: back up `/srv/strata` separately (your script).

## Later

- Automated deploys from CI.
- A Docker image with only the backend: host PostgreSQL and the host model / ONNX Runtime
  directories mounted read-only; embeddings stay in-process.

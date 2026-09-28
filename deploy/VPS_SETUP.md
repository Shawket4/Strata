# VPS setup (first install, manual deploy)

Everything to do on the VPS to run `stratad` v0.1 at `https://strata-ai.duckdns.org`. Assumes
Debian/Ubuntu, root access, and an existing PostgreSQL cluster (the one with your WAL/full
backups); the box may be shared with other services, so inspect before changing anything.
Paths match `docs/RUNBOOK.md`; every setting is in `deploy/stratad.env.example`.

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

## 0. Base packages, time, firewall, port

```sh
apt update && apt install -y nginx sudo curl ca-certificates ufw
timedatectl                     # "System clock synchronized: yes", "NTP service: active"
```

Device creation times are checked against this clock. Install `chrony` only if no NTP client
is active (on Debian, `apt install chrony` removes `systemd-timesyncd`); with timesyncd
installed, `timedatectl set-ntp true` is enough.

Firewall: inspect the existing rules first (`ufw status verbose`, `iptables -S`). If ufw is
active, only add what is missing (`ufw allow 80,443/tcp`); if no firewall is active, enable ufw
only after allowing SSH: `ufw allow OpenSSH && ufw allow 80,443/tcp && ufw enable`.

Port: `stratad` must bind a loopback port that is not open publicly. The default is
`127.0.0.1:8080`; check `ss -ltnp` and the ufw rules, and if 8080 is taken or open to the
public on this box, pick another free port (the production install uses `127.0.0.1:8096`,
because ufw opens 8080 there). Use the same port in `STRATA_BIND` (step 4), the nginx
`proxy_pass` (step 9) and the local health check (step 8).

No system `git` is needed (libgit2 is built into the binary).

## 1. Service user and directories

```sh
useradd --system --home-dir /srv/strata --shell /usr/sbin/nologin strata
install -d -o strata -g strata -m 0700 /srv/strata
install -d -o root -g strata -m 0750 /etc/strata
```

## 2. PostgreSQL: extensions, database, roles

Needs PostgreSQL 16 or 17 with **pgvector ≥ 0.6** and **pg_trgm** available (production runs
17 with pgvector 0.8.2). Install pgvector only if it is missing:

```sh
sudo -u postgres psql -c "SELECT name, default_version FROM pg_available_extensions WHERE name IN ('vector','pg_trgm');"
apt install -y postgresql-17-pgvector        # only if 'vector' is missing; <major> of your cluster
```

Create the database with a UTF-8, non-`C` character locale (required; `serve` refuses otherwise):

```sh
sudo -u postgres psql -c "CREATE DATABASE strata ENCODING 'UTF8' LC_COLLATE 'C.UTF-8' LC_CTYPE 'C.UTF-8' TEMPLATE template0;"
```

Pick three strong passwords (letters and digits only avoids URL-encoding) and put them in the
env file first (step 4). Then create the roles, extensions and grants. Either review the SQL,
which holds the passwords in plain text: keep the file at mode 0600, and the first line turns
statement logging off so a slow statement never writes a password to the PostgreSQL log:

```sh
(umask 077 && { echo 'SET log_min_duration_statement = -1;'
  stratad --env-file /etc/strata/stratad.env bootstrap-roles --print; } > /root/strata-bootstrap.sql)
less /root/strata-bootstrap.sql
sudo -u postgres psql -d strata < /root/strata-bootstrap.sql && rm /root/strata-bootstrap.sql
```

or apply it directly with a superuser URL:

```sh
stratad --env-file /etc/strata/stratad.env bootstrap-roles \
  --superuser-url 'postgres://postgres@/postgres?host=/var/run/postgresql'
```

Roles created: `strata_owner` (migrations), `strata_app` (requests and jobs, `NOBYPASSRLS`),
`strata_accounts` (accounts only). Your existing WAL/full backups cover this database like any
other.

If `pg_hba.conf` already has `host all all 127.0.0.1/32 scram-sha-256`, nothing to do.
Otherwise the three roles need a `host strata strata_owner,strata_app,strata_accounts
127.0.0.1/32 scram-sha-256` line (then **reload** PostgreSQL, never restart).

## 3. The binary

Every green Build run publishes a GitHub release: pushes replace the rolling pre-release
**`latest`**, a `v*` tag gets a release of that name. Take the tarball and its `.sha256` from
Releases → `latest` (or the tag). A branch build is named
`stratad-sha-<short sha>-linux-x86_64.tar.gz` (e.g. `stratad-sha-4ff0b09-linux-x86_64.tar.gz`),
a tag build `stratad-<version>-linux-x86_64.tar.gz` (e.g. `stratad-v0.1.0-linux-x86_64.tar.gz`).
The repository is private: on the VPS download with the GitHub CLI and a token that can read
it, or copy the two files over (`scp`):

```sh
GH_TOKEN=<token> gh release download latest -R Shawket4/Strata -p 'stratad-*'   # or: -R … v0.1.0
```

Check and install:

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
STRATA_BIND=127.0.0.1:8080          # the loopback port chosen in step 0
STRATA_DEFAULT_TIMEZONE=Africa/Cairo
STRATA_DATABASE__OWNER_URL='postgres://strata_owner:PASSWORD1@127.0.0.1/strata'
STRATA_DATABASE__APP_URL='postgres://strata_app:PASSWORD2@127.0.0.1/strata'
STRATA_DATABASE__ACCOUNTS_URL='postgres://strata_accounts:PASSWORD3@127.0.0.1/strata'
STRATA_AUTH__SIGNING_KEY_FILE=/etc/strata/token-signing-key.pem
STRATA_AUTH__TRUST_FORWARDED_FOR=true
STRATA_AI__CLAUDE_CLI__COMMAND='sudo -n -u strata-ai /usr/local/lib/strata/claude-ai'
STRATA_AI__CLAUDE_CLI__SCRATCH_DIR=/var/lib/strata-ai/scratch
STRATA_AI__CLAUDE_CLI__LAUNCH_DIR=/   # the wrapper changes into the scratch dir as strata-ai
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

Install Claude Code system-wide as `/usr/local/bin/claude`. Without Node: download the
native binary of the current stable release and check it against that release's manifest
(needs `jq`):

```sh
B=https://downloads.claude.ai/claude-code-releases
V=$(curl -fsSL "$B/stable")
curl -fL -o /tmp/claude "$B/$V/linux-x64/claude"
SUM=$(curl -fsSL "$B/$V/manifest.json" | jq -r '.platforms["linux-x64"].checksum')
echo "$SUM  /tmp/claude" | sha256sum -c - && install -m 0755 /tmp/claude /usr/local/bin/claude
rm /tmp/claude && claude --version
```

(Alternative with Node 18+: `npm install -g @anthropic-ai/claude-code`.) Then log in once as
`strata-ai` with your subscription:

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
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/api/v1/health   # 200 (your port from step 0)
```

Run exactly one `stratad` (rate limits and revocations are in memory).

## 9. nginx and TLS

The apps talk only to `https://strata-ai.duckdns.org`: their server is fixed in the Build
workflow (`STRATA_SERVER_URL` in `.github/workflows/build.yml`, RUNBOOK §13) and cannot be
changed in the app. Set up HTTPS before installing the apps.

1. DNS: `strata-ai.duckdns.org` is a DuckDNS name, so its record is set on
   [duckdns.org](https://www.duckdns.org) (signed in: the `strata-ai` domain's IP →
   `187.124.33.153`, plus the IPv6 address if the VPS has one). Check:
   `dig +short strata-ai.duckdns.org` prints `187.124.33.153`.
2. `apt install -y certbot python3-certbot-nginx`.
3. First a port-80-only site, so certbot can answer the challenge. Create
   `/etc/nginx/sites-available/strata`:

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name strata-ai.duckdns.org;
    location / { return 404; }
}
```

```sh
ln -s /etc/nginx/sites-available/strata /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx
certbot certonly --nginx -d strata-ai.duckdns.org --deploy-hook "systemctl reload nginx"
```

4. Then replace the file with the full site (the `ssl_certificate` files exist now; a single
   HTTPS block before the certificate exists fails `nginx -t`). `http2 on;` needs nginx ≥ 1.25;
   drop the `[::]` lines if the VPS has no IPv6; use your port from step 0 in `proxy_pass`:

```nginx
map $http_upgrade $strata_connection_upgrade { default upgrade; '' close; }

server {
    listen 80;
    listen [::]:80;
    server_name strata-ai.duckdns.org;
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl;
    listen [::]:443 ssl;
    http2 on;
    server_name strata-ai.duckdns.org;
    ssl_certificate /etc/letsencrypt/live/strata-ai.duckdns.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/strata-ai.duckdns.org/privkey.pem;

    add_header Strict-Transport-Security "max-age=31536000" always;
    client_max_body_size 512m;          # vault import zips

    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;   # overwrite, never append
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;          # /events and Ask streams (WebSocket)
        proxy_set_header Connection $strata_connection_upgrade;
        proxy_buffering off;
        proxy_read_timeout 1h;
        proxy_send_timeout 1h;
    }
}
```

```sh
nginx -t && systemctl reload nginx
curl -s -o /dev/null -w '%{http_code}\n' https://strata-ai.duckdns.org/api/v1/health   # 200
certbot renew --dry-run
```

Then run the Build workflow (or re-run it) so the apps people install are built for this
server.

**Testing without the domain:** release builds refuse any address but `https://`. A debug
build may use this device over plain HTTP through an SSH tunnel:
`ssh -N -L 8080:127.0.0.1:8080 you@vps` (your port on the VPS side), then
`flutter run --dart-define=STRATA_SERVER_URL=http://127.0.0.1:8080`.

## 10. Upgrading the server

Download the new tarball and its `.sha256` into an empty directory (step 3: Releases →
`latest` or the tag, so the globs below match one file), verify the checksum **before**
unpacking, then install, migrate and restart:

```sh
sha256sum -c stratad-*-linux-x86_64.tar.gz.sha256
tar xzf stratad-*-linux-x86_64.tar.gz && cd stratad-*-linux-x86_64/
install -m 0755 stratad /usr/local/bin/stratad
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
systemctl restart stratad
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/api/v1/health   # 200 (your port)
```

Compare `/etc/strata/stratad.env` with the new `stratad.env.example` for added settings
(unknown or renamed ones stop `stratad` with the variable's name).

## 11. Checks after install

- `systemctl status stratad` is active; the log has no `embeddings disabled` line.
- Sign in as `owner` in the app; create a note; the vault has a commit:
  `sudo -u strata git -C /srv/strata/users/<id>/vault log --oneline | head` (needs `apt install git`, optional).
- AI status in Settings → AI shows the provider ready and embeddings loaded after the first note.
- Vault files are not in your PostgreSQL backups: backing up `/srv/strata` is still open
  (deferred by the owner).

## 12. Automated deploys

CI deploys every green build of the default branch: see
[`ci-deploy/README.md`](ci-deploy/README.md) (a `strata-deploy` user whose key can only run
the deploy script, and the `production` environment secrets).

## Later

- Backups of `/srv/strata` (open, deferred by the owner).
- A Docker image with only the backend: host PostgreSQL and the host model / ONNX Runtime
  directories mounted read-only; embeddings stay in-process.

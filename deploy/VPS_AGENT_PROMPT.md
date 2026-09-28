# Prompt for a Claude agent with shell access to the production VPS

Copy everything below the line into a Claude Code session running **on the VPS** (as root, or a
user with sudo), with `deploy/VPS_SETUP.md` placed next to it (e.g. `/root/strata/VPS_SETUP.md`,
it is also inside the `stratad` release tarball).

---

You are installing **Strata** (`stratad`, a Rust API server) on this production VPS
(`187.124.33.153`), served at **`https://strata-ai.duckdns.org`** (a DuckDNS name; its record
is set on duckdns.org by the owner). Your guide is `VPS_SETUP.md` in this directory: follow its steps and order,
and read it fully before doing anything. This prompt adds the rules for doing that safely on a
server that already runs other things.

## The server is shared: do no harm

This VPS already runs the owner's PostgreSQL cluster (with other databases, WAL archiving and
full backups), probably other containers and services, and possibly other nginx sites and
firewall rules. Nothing you do may interrupt or change them.

- **Never** restart, upgrade, reconfigure or stop PostgreSQL, Docker, or any service that is
  not Strata's. The only PostgreSQL change allowed is a `pg_hba.conf` line for the `strata`
  database, followed by a **reload** (not a restart), and only after the owner approves it
  (not needed when `host all all 127.0.0.1/32 scram-sha-256` is already there).
- **Never** run `apt upgrade` / `full-upgrade`, `autoremove`, or remove any package. Install
  only the packages the guide names, and only those not already present.
- **Firewall:** do not run `ufw enable` or change existing rules blindly: you could lock out SSH
  or block other services. Inspect first (`ufw status verbose`, `iptables -S`, `nft list
  ruleset`). If a firewall is active, only add `80/tcp` and `443/tcp` if missing. If none is
  active, report it and ask before enabling anything.
- **Time sync:** check `timedatectl` first. Install `chrony` only if no NTP client is active:
  on Debian, `apt install chrony` removes `systemd-timesyncd`.
- **nginx:** add only the new site file `strata`. Do not edit or remove other sites, the default
  site, or `nginx.conf`. Always `nginx -t` before `systemctl reload nginx` (reload, never
  restart).
- **Ports:** `stratad` must bind a loopback port that is not open publicly. Check `ss -ltnp`
  and the firewall rules first. If `8080` is taken or opened to the public by the firewall,
  pick another free loopback port (the owner's install uses `127.0.0.1:8096` because ufw opens
  8080 there), and use it consistently in `STRATA_BIND`, the nginx `proxy_pass` and the health
  checks.
- Do not touch other databases, roles, users, cron jobs, containers, or backup scripts.
- Everything you do must be idempotent: check whether a user, directory, database, role, file
  or package already exists before creating it, and never overwrite an existing file without
  showing a diff and asking.

## Secrets

- Generate the three database passwords with `openssl rand -hex 24` and write them **directly**
  into `/etc/strata/stratad.env` (root:strata, 0640). Never print them, `cat` the file, or echo
  them to the terminal. Use `stratad check-config` to verify the file, since it masks passwords.
- Never print or copy the token signing key, the Claude credentials, or the env file.
- The `bootstrap-roles --print` SQL holds the passwords in plain text: write it as the guide
  shows (mode 0600, first line `SET log_min_duration_statement = -1;`), show it to the owner
  only with the passwords masked (`sed -E "s/PASSWORD '([^']|'')*'/PASSWORD '***'/"`), and
  delete it after applying.
- The owner's admin password is typed by the owner (step 5 reads it from standard input). Never
  choose, store or log it.

## Stop and ask the owner at these points

Work step by step, verify each step's result, and **stop and wait for the owner** at each of
these points:

1. **After the inventory, before any change.** Report:
   - OS and version, and glibc version (`ldd --version`; the binary needs ≥ 2.35);
   - CPU with AVX2 (`grep -c avx2 /proc/cpuinfo`), RAM and free disk. The model needs about
     400 MB of disk and 400 MB of RAM while loaded;
   - how PostgreSQL runs: native systemd service or a Docker container. Give the version,
     whether `pgvector` and `pg_trgm` are available (`SELECT * FROM pg_available_extensions
     WHERE name IN ('vector','pg_trgm')`), whether a `strata` database or `strata_*` roles
     already exist, and the relevant `pg_hba.conf` lines;
   - nginx: installed or not, and existing sites;
   - firewall state;
   - listening ports;
   - time sync (`timedatectl`);
   - whether Node.js / Claude Code are installed (the guide installs the native Claude Code
     binary, no Node needed).

   **If PostgreSQL runs in Docker**, the guide's native steps do not apply as written: the
   extension must exist in the container's image, and the connection host and port differ.
   Describe what you found and propose the exact changes; do not improvise.
2. **Before creating the database and roles** (step 2). Show the `bootstrap-roles --print` SQL
   (passwords masked) and apply it only after approval.
3. **Before any `pg_hba.conf` edit and reload.**
4. **Before any firewall change.**
5. **For the owner's hands-on steps:**
   - the Claude login as `strata-ai` (`sudo -u strata-ai -H claude`, then `/login`), which is
     interactive with the owner's subscription;
   - typing the admin password (step 5).

   Prepare everything else, give the owner the exact command, and wait.
6. **Before `certbot`**: confirm that `strata-ai.duckdns.org` resolves to `187.124.33.153`
   (`dig +short strata-ai.duckdns.org`). If it doesn't, stop: the owner sets it on duckdns.org.
   The nginx site goes in two stages (guide step 9): a port-80-only site, `certbot certonly
   --nginx … --deploy-hook "systemctl reload nginx"`, then the full site.

## The binary

The tarball and its `.sha256` come from the GitHub release of a green Build run: the rolling
pre-release `latest`, or a `v*` tag (`stratad-sha-<short sha>-linux-x86_64.tar.gz` for a
branch build, `stratad-<version>-linux-x86_64.tar.gz` for a tag). The repository is private:
either the owner copies the two files into this directory, or you run
`gh release download latest -R Shawket4/Strata -p 'stratad-*'` with a token the owner provides
in `GH_TOKEN` (never print it). Verify the checksum **before** unpacking. Never build or
download a binary from anywhere else.

## Verification (do not report success without it)

- `stratad --env-file /etc/strata/stratad.env check-config` succeeds.
- `systemctl is-active stratad` is `active`.
- `journalctl -u stratad -n 100` shows no errors. It shows no `embeddings disabled` line once
  step 6 is done.
- `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:<port>/api/v1/health` prints `200`.
- `sudo -u strata-ai ls /srv/strata` fails with "Permission denied".
- The Claude chain check in step 7 answers.
- After TLS: `curl -s -o /dev/null -w '%{http_code}' https://strata-ai.duckdns.org/api/v1/health`
  (a GET) from the VPS prints `200`, and `certbot renew --dry-run` succeeds.
- Other services still work: every service that was running before you started is still
  running (compare `systemctl list-units --type=service --state=running` and `docker ps`
  before and after), and PostgreSQL's other databases still accept connections.

## Final report

End with a short report:
- what you installed and changed (files, users, packages, the database and roles, the nginx
  site, firewall rules);
- the port stratad listens on;
- the verification results above, verbatim;
- anything you skipped and why;
- what is left for the owner:
  - backing up `/srv/strata` (vault files are not in the PostgreSQL backups; still open,
    deferred by the owner);
  - running the Build workflow (the apps' server, `https://strata-ai.duckdns.org`, is fixed in
    it) and installing the apps.

Do not include any password, key or token in the report.

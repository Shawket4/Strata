# Automated deploys (CI → VPS)

`.github/workflows/deploy.yml` deploys `stratad` to the VPS when **Build and CI are both green**
for a push to the default branch (whichever finishes last triggers it), or by hand from
Actions → Deploy → Run workflow (optionally with a Build run ID; a rollback is a deploy of an
older green run). It replaces the manual upgrade of [`VPS_SETUP.md`](../VPS_SETUP.md) §10.

What happens on the server (`deploy-stratad`, as root):

1. read the tarball from stdin (≤ 200 MB) and check its SHA-256;
2. refuse links, special files and any path other than `<package>/<file>`;
3. run the new binary's `--version` and `check-config` against `/etc/strata/stratad.env`
   (as `strata`);
4. stop here if the binary is the one already installed (only a health check);
5. `migrate` with the new binary (the running one keeps serving), keep the old binary as
   `/usr/local/bin/stratad.prev`, swap, `systemctl restart stratad`;
6. health check (`GET /api/v1/health` = 200 and still up 5 s later); otherwise put the previous
   binary back and restart. Migrations are forward-only and stay applied.

The last 3 releases are kept in `/var/lib/strata-deploy/releases/`; every attempt is logged to
`/var/log/strata-deploy.log`. A changed `stratad.service` in the package is reported, never
installed: update the unit by hand. The CI output names versions and steps only (the repository
is public): it never prints settings or the service log.

## Access

The only credential is an SSH key in the GitHub environment **`production`** (deployments
allowed from the default branch and `main` only):

| Environment secret | Value |
|---|---|
| `STRATA_DEPLOY_SSH_KEY` | private ed25519 key (its only copy; rotate by making a new one) |
| `STRATA_DEPLOY_KNOWN_HOSTS` | the VPS host key line (`ssh-keyscan -t ed25519 <ip>`, checked against `/etc/ssh/ssh_host_ed25519_key.pub`) |
| `STRATA_DEPLOY_HOST` | the VPS address |

On the VPS the key belongs to `strata-deploy` and can do exactly one thing:

```
# /var/lib/strata-deploy/.ssh/authorized_keys (root:root 0644, like the home and .ssh)
restrict,command="/usr/local/lib/strata/deploy-entry" ssh-ed25519 AAAA… github-actions@Shawket4/Strata deploy

# /etc/sudoers.d/strata-deploy (0440)
strata-deploy ALL=(root) NOPASSWD: /usr/local/lib/strata/deploy-stratad
```

No shell, no forwarding, no pty; `deploy-entry` accepts only
`deploy <sha256> <stratad-…-linux-x86_64>` and `deploy-stratad` validates both again. The two
scripts are installed by hand (root:root 0755 in `/usr/local/lib/strata/`) and CI never updates
them: after changing them here, copy them to the server.

## One-time server setup

```sh
install -o root -g root -m 0755 deploy-entry deploy-stratad /usr/local/lib/strata/
useradd --system --home-dir /var/lib/strata-deploy --no-create-home --shell /bin/sh strata-deploy
install -d -o root -g root -m 0755 /var/lib/strata-deploy /var/lib/strata-deploy/.ssh
install -d -o root -g root -m 0711 /var/lib/strata-deploy/releases
ssh-keygen -t ed25519 -N '' -C 'github-actions@Shawket4/Strata deploy' -f strata_deploy   # anywhere
printf 'restrict,command="/usr/local/lib/strata/deploy-entry" %s\n' "$(cat strata_deploy.pub)" \
  > /var/lib/strata-deploy/.ssh/authorized_keys
echo 'strata-deploy ALL=(root) NOPASSWD: /usr/local/lib/strata/deploy-stratad' > /etc/sudoers.d/strata-deploy
chmod 0440 /etc/sudoers.d/strata-deploy && visudo -c
gh secret set STRATA_DEPLOY_SSH_KEY --env production < strata_deploy && shred -u strata_deploy
```

## Manual deploy or rollback from a machine

```sh
ssh strata-deploy@<vps> "deploy $(cut -d' ' -f1 <pkg>.tar.gz.sha256) <pkg>" < <pkg>.tar.gz
```

(with the deploy key), or as root: `/usr/local/lib/strata/deploy-stratad <sha256> <pkg> < <pkg>.tar.gz`.

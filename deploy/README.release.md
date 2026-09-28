# stratad — Linux x86_64 release

Built by CI (`.github/workflows/build.yml`) on Ubuntu 22.04 (glibc 2.35): runs on Debian 12+,
Ubuntu 22.04+ and other glibc ≥ 2.35 distributions. Not a static build: ONNX Runtime is loaded
at run time from the path in `STRATA_AI__EMBEDDING__ONNXRUNTIME_LIB`.

| File | What |
|---|---|
| `stratad` | the server binary (stripped) |
| `stratad.env.example` | every setting (`STRATA_…` variables) with its default |
| `stratad.service` | systemd unit (reads `/etc/strata/stratad.env`) |
| `VPS_SETUP.md` | first install, step by step |

Quick start (details in `VPS_SETUP.md`):

```sh
install -m 0755 stratad /usr/local/bin/stratad
install -d -o root -g strata -m 0750 /etc/strata
install -o root -g strata -m 0640 stratad.env.example /etc/strata/stratad.env   # then edit it
stratad --env-file /etc/strata/stratad.env check-config
sudo -u strata stratad --env-file /etc/strata/stratad.env migrate
install -m 0644 stratad.service /etc/systemd/system/stratad.service
systemctl daemon-reload && systemctl enable --now stratad
```

Upgrade: replace the binary, run `migrate`, `systemctl restart stratad`.

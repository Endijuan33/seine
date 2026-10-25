# Production collector

- Host: `root@2.31.36.180` (Hetzner, Falkenstein; 4 vCPU, 8 GB RAM, 80 GB disk).
- OS: Ubuntu 26.04 LTS; distro Docker Engine and Compose packages.
- Checkout: `/opt/seine`, canonical `master`.
- Ingest: `https://telemetry.seineminer.com/v1/reports`.
- DNS: direct A record, Cloudflare proxy disabled.
- Secrets: `/opt/seine/telemetry/deploy/.env`, root-only permissions.
- Published ports: SSH 22, HTTP 80 (redirect/ACME), HTTPS 443. PostgreSQL and
  collector ports are private to the Compose network. SSH is key-only.
- Docker restarts containers after reboot; unattended OS security updates are enabled.
- Recovery backups: daily under `/var/backups/seine-telemetry`, seven days of
  reports per backup, seven-day backup retention. Local storage only: server loss
  also loses these backups. Do not enable long-lived VM snapshots containing reports.

## Operations

```sh
ssh root@2.31.36.180
cd /opt/seine
docker compose -f telemetry/deploy/compose.yaml --env-file telemetry/deploy/.env ps
systemctl status seine-telemetry-backup.timer seine-telemetry-check.timer
systemctl start seine-telemetry-check.service
journalctl -u seine-telemetry-check.service -n 20
```

The five-minute check verifies local HTTPS certificate/response, database queries,
retention progress, disk headroom, backup freshness and sustained inventory-only
reporting (sample completeness). A failed check appears in
systemd/journald. No external alert destination is configured. Public DNS/reachability
should also be checked from outside the server:

```sh
curl -sS -o /dev/null -w '%{http_code}\n' https://telemetry.seineminer.com/healthz
```

Expect HTTP 204. This public endpoint alone does not verify the database.

Schema 2 miners require the upgraded collector; deploy it before the miner release.
The upgraded collector still accepts schema 1 clients. After this upgrade, the
completeness check may continue failing for old pool miners until they upgrade
or their inventory-only reports leave the six-hour window. Use the agent
briefing's per-version data-quality result to distinguish that known client
problem from a collector, backup or database failure.

For deployment, first review the intended commit and create a fresh recovery backup.
Then fast-forward the checkout and rebuild only the collector:

```sh
systemctl start seine-telemetry-backup.service
git pull --ff-only origin master
docker compose -f telemetry/deploy/compose.yaml --env-file telemetry/deploy/.env up -d --build collector
systemctl start seine-telemetry-check.service
```

Keep the previous collector image ID before an upgrade for rollback. Do not run
`docker compose down -v`: the volumes hold reports and TLS certificates.
See [the operator guide](../README.md#recovery-backups) for restore procedures;
always verify a restore in a separate database before replacing live data.

Official miner builds use the GitHub repository variable `SEINE_TELEMETRY_URL`.
Changing it affects future builds; shipped clients keep their saved endpoint and
opt-out preference. The policy is published at <https://seineminer.com/privacy/>.

# Hardware telemetry

Seine hardware and performance reporting, with a separate Rust/Axum
collector and PostgreSQL database. Pool Stratum, wallet and daemon contracts are
unchanged. A collector outage drops reports without stopping mining.

## Miner preferences and collection

Telemetry is enabled by default when a collector URL is configured, for both
interactive and unattended mining. There is no startup prompt or routine notice.
Users can disable it persistently with `--telemetry off`. The user-facing disclosure
is [PRIVACY.md](../PRIVACY.md), also served at `/privacy/` on the website.
Benchmark runs do not upload reports.

For an operator-provided collector:

```sh
seine --telemetry on --telemetry-url https://telemetry.example.com/v1/reports
seine --telemetry off
```

Use these alongside the normal mining arguments on a fresh installation.
Preferences are saved to `<data-dir>/seine.telemetry.json`. Changing the URL resets
the random installation ID and preserves any explicit opt-out. Disabling is checked
before every upload;
an already in-flight request may finish. Deleting the settings file disables
future uploads from the running worker; a future launch uses the default enabled
setting if a collector is configured.

No production domain is hard-coded. After provisioning and testing the collector,
the release builder can set `SEINE_TELEMETRY_URL` to the complete HTTPS ingest URL
when building Seine. This supplies the default destination with no startup prompt.
Until then, ordinary builds make no telemetry requests.

Official release builds read the GitHub repository variable
`SEINE_TELEMETRY_URL`; set it to the verified HTTPS ingest URL before tagging a
release. Clearing it disables the default in future builds, not already
distributed binaries. Release archives include `PRIVACY.md`. The website workflow
also publishes changes made only to that policy file.

The first report is sent after five minutes; subsequent reports are hourly.
Requests have a five-second total timeout, no redirects, no offline spool and no
immediate retries. HTTP is allowed only for loopback testing. CPU-only builds need
no database or collector dependencies.

The strict upload schema is in [src/lib.rs](src/lib.rs), with an example in
[fixture.json](fixture.json). It includes:

- CPU model/vendor/family/model number, logical and physical core counts,
  rounded-up GiB RAM, OS/version, native/WSL classification and supported kernels.
- Miner version, target, build features and whether it is a debug build.
- Active mining backend model and memory where available, enabled workers/lanes,
  CPU kernel and requested page policy, GPU register/launch settings where available.
- Per-backend hashes and observation duration, plus a flag excluding windows with
  changed settings or missed telemetry samples from performance comparisons.

GPU details describe active mining devices, rather than every installed adapter.
Metal currently reports backend/memory/lanes; the CPU model identifies the Apple
system. Requested CPU page policy is not proof of actual huge-page backing, and
GPU launch settings are configured limits, not every adaptive launch's depth.
These self-reported live rates include machine load and mining idle time; use them
to prioritize controlled optimization experiments, not as benchmark proof.

Wallets, worker names, hostnames, hardware serials, MAC addresses, pool URLs,
credentials, command lines and raw logs are excluded. The installation ID is
random, persisted locally, and unrelated to a wallet or hardware identifier.
It permits deduplication and is pseudonymous, not anonymous. The collector sees
the connection IP but does not store it. The supplied Caddy config suppresses both
access and error request logging; any additional CDN/proxy needs the same policy.

## Deploy

Start with one small Linux server (planning estimate: 2 vCPU, 2–4 GiB RAM), running
collector, PostgreSQL and Caddy together. A separate DB host is unnecessary for
the initial rollout. Provisioning, DNS changes and deployment are separate from
landing this code.

Point the chosen hostname at that server, allow ports 80/443, clone the repo, and
create `telemetry/deploy/.env` (ignored by Git):

```dotenv
TELEMETRY_HOST=telemetry.example.com
TELEMETRY_DB_PASSWORD=replace_with_a_random_hex_password
```

Use a random hex password (for example `openssl rand -hex 32`) so it is safe in
the PostgreSQL connection string. Then:

```sh
docker compose -f telemetry/deploy/compose.yaml --env-file telemetry/deploy/.env up -d --build
```

Only Caddy publishes ports. PostgreSQL and the collector stay on the private
Compose network; database and certificate state use persistent volumes. Caddy
obtains HTTPS certificates. Configure disk monitoring and backups with retention
no longer than the data policy. The collector reconnects after DB connection loss.
`GET /healthz` is process liveness, not database readiness; an ingest check exercises
the DB. No admin or raw-report HTTP endpoint is exposed.

Reports are capped at 32 KiB and validated against a versioned allowlist. Admission
is limited to 20 reports/second globally and one per installation per five minutes,
with at most eight waiting database requests. IDs are client-generated: these
controls bound normal ingestion, but do not authenticate self-reported hardware.
Database report IDs deduplicate replays. Quotas reset on collector restart.

Raw reports expire after 30 days, pruned hourly in bounded batches. The analysis
views use this retained window; no permanent device inventory is kept. At 1,000
always-running installations, expect about 24,000 reports/day. At an assumed
4 KiB per report, that is about 3 GB of raw payload per 30 days, before PostgreSQL
indexes, WAL and backups. Revisit disk sizing using measured production growth.

### Recovery backups

For a checkout at `/opt/seine`, install the supplied daily backup timer:

```sh
sudo install -m 644 telemetry/deploy/seine-telemetry-backup.{service,timer} /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now seine-telemetry-backup.timer
sudo systemctl start seine-telemetry-backup.service
```

`backup.sh` writes private schema and compressed CSV files under
`/var/backups/seine-telemetry`. Each backup contains only the most recent seven
days of reports and expires after seven days, so backed-up records stay well
inside the 30-day raw-data limit. Expiration runs even if the database is down.
The timer must keep running; purge expired backups before bringing an offline
host back into service. Full-volume snapshots must not retain telemetry beyond
the same limit. These local backups protect against accidental database changes,
not loss of the server; an off-host copy needs the same access and expiry rules.

Test recovery into a **separate empty database**, using PostgreSQL 17 or later:

```sh
psql "$RESTORE_DATABASE_URL" -v ON_ERROR_STOP=1 -f /path/to/backup/schema.sql
gzip -dc /path/to/backup/reports.csv.gz | psql "$RESTORE_DATABASE_URL" -v ON_ERROR_STOP=1 -c 'COPY telemetry_reports (report_id, installation_id, received_at, payload) FROM STDIN WITH (FORMAT csv)'
```

Check `systemctl status seine-telemetry-backup.timer`, backup file freshness and
free disk space during operations. Restored records retain their original receipt
times and remain subject to collector cleanup.

## Analysis

Connect privately with `psql` or a preferred SQL client:

```sql
SELECT * FROM telemetry_cpu_population ORDER BY installations DESC;
SELECT * FROM telemetry_backend_performance ORDER BY installations DESC;

-- Active GPU inventory, one latest report per installation.
SELECT sample #>> '{settings,device_model}' AS gpu,
       count(DISTINCT installation_id) AS installations
FROM telemetry_installations
CROSS JOIN LATERAL jsonb_array_elements(payload->'backends') AS sample
WHERE sample #>> '{settings,backend}' IN ('nvidia', 'amd', 'metal')
GROUP BY 1 ORDER BY 2 DESC;
```

Popularity counts installations, not messages or necessarily people. Performance
first takes each installation's median, then reports population median/p10/p90
for the same version, CPU, OS and backend settings. Debug builds, unstable windows
and samples shorter than a minute are excluded. Results describe reporting installations and may not represent all miners.

## Verification and showcase

```sh
cargo +1.93.0 test -p seine-telemetry --features server
cargo +1.93.0 test --bin seine --no-default-features hardware_telemetry
cargo +1.93.0 check --workspace --all-targets --features seine-telemetry/server,amd
```

For a disposable local PostgreSQL and collector, set `TELEMETRY_TEST_DATABASE`
to that database's connection string, then run `python3 telemetry/test_integration.py`.
It writes synthetic reports and checks validation, throttling, storage and analysis.
Do not run it against production.

Run the real collector against that disposable DB:

```sh
TELEMETRY_DATABASE_URL='host=127.0.0.1 port=55439 user=postgres dbname=telemetry' \
  cargo run -p seine-telemetry --features server
python3 telemetry/showcase/serve.py
```

The stable local showcase is <http://localhost:9081>. It exercises real collector
responses through a local fixture proxy. Its checkbox controls fixture submissions
only; it is not a replacement for the miner's persistent preference. It supports disabled,
ready, loading, success, validation errors, empty reports, throttling and collector
unavailability. No real miner or hardware workload is needed.

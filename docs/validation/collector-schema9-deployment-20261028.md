# Schema-9 collector deployment receipt — October 28, 2026

Deployed under the owner's standing permission for collector updates (given
October 28, 2026). The collector was built from tested commit
`36d26b5c8274b13800e6bede493ae4c1b3c98801`. Replacement began at **15:17:14 EDT**;
post-replacement infrastructure checks passed at **15:17:26 EDT**. This is the
replacement interval, not a measured client outage duration.

Only Compose service `collector` was replaced. No database migration, pool,
Stratum, daemon or HTTPS restart, miner release, or test Discord message occurred.
The schema-9 fields (`hardware.numa_nodes`, `hardware.resources.requested_threads`,
`hardware.resources.cpu_autotune_threads`, the `auto_update` shutdown origin) still
require a miner release containing schema 9.

## Source and rollback

- Immutable build inputs: `/opt/seine-collector-releases/36d26b5`, a `git archive`
  of the tested commit's `Cargo.toml`, `Cargo.lock`, `pow-kernel`, `pow-spec` and
  `telemetry` directory, built with `telemetry/deploy/Dockerfile` (locked Rust 1.93).
- Image tag: `seine-telemetry-reviewed:36d26b5c8274b13800e6bede493ae4c1b3c98801`,
  revision label `org.opencontainers.image.revision` = the tested commit.
- Running image: `sha256:d42f0422d0f5aad803086a0ff7c7bc12387991d5c6efec7525d3bde7770f51dd`.
- Previous image: `sha256:05ed598904739b0366c1b7216f564baa5df04d4720f4765e9089f1aa64c3fd5a`
  (schema 8), also tagged `seine-telemetry-rollback:20261028T191339Z`.
- Root-only state: `/var/lib/seine-telemetry-schema9.um8YAZnA`. `candidate.yaml`
  selects the deployed image; `rollback.yaml` selects the previous image; the
  directory also holds the build log, before/after container identities and the
  post-activation schema mix. `/var/lib/seine-telemetry-active-schema9-state`
  points to this directory.
- The production checkout `/opt/seine` remains at `af236c2` with its pre-existing
  `telemetry/alerts.py` edit unchanged (SHA-256 compared before and after). The
  running source is the immutable archive, not that checkout.

For subsequent Compose operations, retain the candidate override. For rollback:

```sh
docker compose \
  --project-directory /opt/seine/telemetry/deploy \
  --env-file /opt/seine/telemetry/deploy/.env \
  -f /opt/seine/telemetry/deploy/compose.yaml \
  -f /var/lib/seine-telemetry-schema9.um8YAZnA/rollback.yaml \
  up -d --no-deps --no-build collector
/opt/seine/telemetry/deploy/check.sh --infrastructure-only </dev/null
```

Rollback restores schema-8 acceptance; schema-9 miners would then be rejected.
Do not release schema-9 miners against a rolled-back schema-8 collector.

## Verification

Before replacement, at the tested commit on a development machine:

- `cargo test --locked -p seine-telemetry --features server`: 16 + 1 tests passed.
- `telemetry/test_integration.py` passed against a disposable collector build and
  PostgreSQL 17 (schema 1–8 compatibility, cohort separation, paired trials,
  shutdown requests, context persistence, strict rejection).
- Schema-9 acceptance against the same disposable collector: a schema-9 report
  derived from a real schema-8 payload with `numa_nodes`, `requested_threads` and
  `cpu_autotune_threads` was accepted (HTTP 204) and stored with those fields; a
  schema-9 report without the new fields and an unchanged schema-8 report were
  accepted; schema 8 plus `numa_nodes`, schema 9 with `numa_nodes = 0`, and
  schema 10 were rejected (HTTP 422). No synthetic reports were sent to production.

On the production host:

- Fresh recovery backup completed before replacement.
- HTTPS, database, retention, disk and backup checks passed before and after.
- Running image ID equals the built candidate; the revision label matches the commit.
- Database and HTTPS container IDs and start timestamps were unchanged.
- Public `https://telemetry.seineminer.com/healthz` returned HTTP 204 afterward.
- Natural schema-8 miner reports arrived within ten seconds of activation (two by
  15:17:40 EDT); legacy schemas remain accepted.

Collector rollout does not backfill fields absent from older miners. Schema-9
counts stay zero until a compatible miner release ships and is adopted.

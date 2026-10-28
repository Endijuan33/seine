# Schema-10 collector deployment receipt — October 28, 2026

Deployed under the owner's standing permission for collector updates. The
collector was built from tested commit
`c12ff005e8b3a17e1d3a5a407f8c9e7846d69c5c`. Replacement began at **16:22:51 EDT**;
post-replacement infrastructure checks passed at **16:23:02 EDT**. This is the
replacement interval, not a measured client outage duration.

Only Compose service `collector` was replaced. No database migration, pool,
Stratum, daemon or HTTPS restart, miner release, or test Discord message occurred.
Schema 10 adds `hardware.memory_setup` (module count, DDR type, registered flag,
lowest rated and configured MT/s); it still requires a miner release containing
schema 10. The schema-9 collector of the same day is recorded in
[its receipt](collector-schema9-deployment-20261028.md).

## Source and rollback

- Immutable build inputs: `/opt/seine-collector-releases/c12ff00`, a `git archive`
  of the tested commit's `Cargo.toml`, `Cargo.lock`, `pow-kernel`, `pow-spec` and
  `telemetry` directory, built with `telemetry/deploy/Dockerfile` (locked Rust 1.93).
- Image tag: `seine-telemetry-reviewed:c12ff005e8b3a17e1d3a5a407f8c9e7846d69c5c`,
  revision label `org.opencontainers.image.revision` = the tested commit.
- Running image: `sha256:224d54cc893244597eaf6e81635869d4b7c0b724977cddd1daf538d89e1de9e8`.
- Previous image: `sha256:d42f0422d0f5aad803086a0ff7c7bc12387991d5c6efec7525d3bde7770f51dd`
  (schema 9), also tagged `seine-telemetry-rollback:20261028T202121Z`.
- Root-only state: `/var/lib/seine-telemetry-schema10.MvfZkwA6` (`candidate.yaml`,
  `rollback.yaml`, build log, before/after container identities, post-activation
  schema mix). `/var/lib/seine-telemetry-active-schema10-state` points to it.
- The production checkout `/opt/seine` and its pre-existing `telemetry/alerts.py`
  edit remain unchanged (SHA-256 compared before and after).

For subsequent Compose operations, retain the candidate override. For rollback:

```sh
docker compose \
  --project-directory /opt/seine/telemetry/deploy \
  --env-file /opt/seine/telemetry/deploy/.env \
  -f /opt/seine/telemetry/deploy/compose.yaml \
  -f /var/lib/seine-telemetry-schema10.MvfZkwA6/rollback.yaml \
  up -d --no-deps --no-build collector
/opt/seine/telemetry/deploy/check.sh --infrastructure-only </dev/null
```

Rollback restores schema-9 acceptance; schema-10 miners would then be rejected.
Do not release schema-10 miners against a rolled-back collector.

## Verification

Before replacement, at the tested commit on a development machine:

- `cargo test --locked -p seine-telemetry --features server`: 17 + 1 tests passed.
- `telemetry/test_integration.py` passed against a disposable collector build and
  PostgreSQL 17 (schema 1–8 behaviours).
- Schema-10 acceptance against the same disposable collector: a schema-10 report
  derived from a real schema-8 payload with `memory_setup` was accepted (HTTP 204)
  and stored with its fields; a schema-10 report without the new field and the
  unchanged schema-8 report were accepted; schema 9 plus `memory_setup`,
  `modules = 0`, `rated_mts = 0`, an unknown `memory_setup` field, and schema 11
  were rejected (HTTP 422). No synthetic reports were sent to production.

On the production host:

- Fresh recovery backup completed before replacement.
- HTTPS, database, retention, disk and backup checks passed before and after.
- Running image ID equals the built candidate; the revision label matches the commit.
- Database and HTTPS container IDs and start timestamps were unchanged.
- Public `https://telemetry.seineminer.com/healthz` returned HTTP 204 afterward.
- A natural schema-9 report (a v0.2.45 miner) arrived within ten seconds of
  activation; legacy schemas remain accepted.

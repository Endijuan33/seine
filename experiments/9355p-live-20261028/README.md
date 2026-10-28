# EPYC 9355P, default settings, 15 minutes of mining (2026-10-28)

**97.1 H/s** while mining (median of minutes 2-15); 96.4 H/s averaged over all 15
minutes from a cold start.

- Machine: AMD EPYC 9355P (Zen 5, 32 cores / 64 threads), 192 GiB RAM, Ubuntu 24.04, bare metal.
- seine: source revision `c9c96389bbcf61e459e49a4b3904dcbe3b2e5dfa`, built with `scripts/cohort/build_candidate.sh`.
- Settings: defaults. No tuning flags and an empty data directory, so seine chose 64 threads,
  the `avx512-stream` kernel and transparent huge pages on its own.
- Method: seine mined against a local Stratum fixture that accepts every hash as a share,
  with networking isolated (`scripts/cohort/remote_live_compete.sh`, `pool_fixture.py`).
  The rate is accepted hashes divided by the 900 seconds since launch, so it includes
  startup and the developer-fee slice.

| Minute | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| H/s | 92.9 | 96.8 | 96.8 | 97.1 | 97.2 | 97.1 | 97.2 | 97.2 | 96.8 | 97.2 | 92.2 | 96.3 | 97.1 | 97.1 | 96.8 |

Machine-readable summary: [summary.json](summary.json).

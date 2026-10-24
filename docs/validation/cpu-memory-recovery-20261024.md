# CPU memory-capacity recovery validation

A native campaign lifecycle reproduced the same bug on Graviton3 and Ice Lake:
a miner restarted with `memory.high=3G` stayed at one worker after the limit
rose to 12 GiB. Startup RAM capacity had fixed the controller maximum, parked
backend slots/nonce stride, and retained-cache ceiling.

The repair derives the parked-slot ceiling from permitted CPUs and explicit
worker bounds. Startup still seeds from the current RAM budget, and every
active arena remains constrained by the refreshed allowance. Recovery has a
30-second headroom debounce, respects allocation backoff, and gets a count
probe opportunity even when kernel comparison would otherwise hold the count.
A short withdrawn headroom spike does not interrupt a comparison.

## Checks

- Standalone review commit `4a622f3`: CPU-only workspace tests, **484 passed,
  four ignored**. This branch stacks on the kernel-retry fix in PR #41.
- Pinned Rust 1.93.0 formatting of changed Rust files and `git diff --check`: pass.
- Independent review by the existing autotune owner: no remaining blocker.
- Combined campaign candidate `1246967`: CPU-only workspace **485 passed,
  four ignored**. The additional test belongs to the separate ARM candidate.
- Combined strict all-target Clippy: 124 existing diagnostics across binary and
  binary-test targets; none in the three changed implementation files. This
  is not reported as a clean workspace lint run.
- Native fixed-candidate resource lifecycle: pending. Acceptance requires
  full-size reference parity, real two-arena growth and kernel switches,
  productive work under CPU/memory pressure, constrained restart then actual
  resident anonymous memory of at least 4 GiB after recovery, preserved cache
  hints, and a fixed-thread opt-out run.

Original failed lifecycle archives and their interpretations are retained in
the architecture campaign. The failure is not hidden by extending its timeout.

# Automatic streaming on Ryzen — native validation

Baseline: v0.2.22 (`6e54117`). Candidate production source: `a13502a1a2a76e2825c4ae1e05ee81dc0a086859`.
Owner: architecture session `01a16bd3-6dc7-70f2-a185-b86550c4dbeb`.

The only production change permits the supported streaming kernel in automatic
trials on AMD family 26/model 68. Turin remains eligible. No startup kernel,
worker count, placement, comparison threshold, cache schema or tuner state
machine changes. The candidate-list fingerprint invalidates incompatible old
kernel decisions. Repeated measurements must still select the winner.

Native fixed-kernel evidence lives on `codex/high-end-streaming-20261025` under
`experiments/high-end-streaming-20261025/`. Genoa has a confirmed explicit-kernel
gain, but automatic eligibility remains pending native productive validation.
There is no new AWS rental or budget authorization in this follow-up.

## Native Windows protocol

Build default release features using native MSVC Rust 1.93.0, two compile jobs,
and a separate target directory. Run the native workspace tests. Preserve the
actual source commit, source archive and executable hashes. An initial build
invocation had a mistyped embedded commit; it was stopped before measurements
and restarted with the exact commit read from the source manifest.

The timed runner (`native_run.py`) requires its own running gpuq exclusive job,
checks Gaming Mode, and has a 45-minute controller deadline. It uses ordinary
pages and a kill-on-close Windows Job Object with a 40 GiB aggregate commitment
ceiling. Mining arenas never exceed 32 GiB. Each native benchmark has a separate
180-second deadline; every child is task-owned and stopped in `finally`.
This reservation additionally ends before 2026-10-25 02:24 UTC so the research
session can collect and clean its expiring cloud rentals. Admission requires at
least 35 minutes remaining; a late queue start is rejected before hashing.

1. Four alternating pairs compare streaming at eight spread physical cores and
   sixteen physical cores: 20-second warmup, two 20-second measured rounds.
   Rates include the completion fence. No worker-count default is inferred from
   one pair or a short screening result.
2. Two opposite-order screens compare eight workers spread across CCDs, confined
   to the 96 MiB CCD, and confined to the 32 MiB CCD. These are placement screens,
   not sufficient evidence for a production affinity change.
3. The real native pool runtime connects only to an owned loopback fixture.
   A 540-second cold episode uses automatic tuning with an 8–16 worker safety
   range; a 150-second restart reuses only its own cache. Neither forces a kernel.
4. A separate 300-second cold episode adds two bounded CPU loads from seconds
   45–125. Preserve the complete trajectory, including inconclusive trials.
5. A 75-second restart with a 95% memory reserve checks parked startup without
   allocating pressure buffers or overwriting the saved evidence. This models a
   reduced memory budget; it is not an actual OS memory-pressure experiment.

All episodes end before the 600-second donation grace period. The fixture uses an
impossible target and no wallet with funds. No shared hosts file, firewall,
browser, GPU process or system memory policy is changed.

The first lifecycle attempt was discarded: the fixture incorrectly parsed blank
keepalive lines as JSON and repeatedly disconnected the miner. The completed 14
fixed-kernel reports and full-size parity preceded that failure and remain valid.
The owned reservation was canceled, all children exited, and the initial cold
cache was isolated. `fixture.py` now ignores blank keepalives; `test_fixture.py`
checks that later requests still work on the same connection. The lifecycle-only
resume verifies the frozen binary and each accepted file hash, preserves the first
benchmark protocol/job identity, and starts with a fresh cold cache. No discarded
productive sample is reused as automatic-selection evidence.

## Integration and handoff

The research session owns PR50 joint tuning in `seine-cpu-followups`; do not alter
its state machine independently. It released the desktop after its builds and
upload completed. Native timings require that hold until an explicit release.
Stop all owned processes, collect/check raw evidence, remove the owned native
temporary tree, release the reservation, and notify research before completion.

Production eligibility is a review candidate until native results are recorded.
Failures and unconfirmed convergence must remain visible; do not replace them
with a forced streaming default or silently reuse older measurements.

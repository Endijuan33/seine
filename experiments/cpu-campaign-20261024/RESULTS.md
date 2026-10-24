# CPU architecture campaign results

**Work in progress.** Baseline v0.2.21 (`359265e`), current 2 GiB Argon2id PoW.
All rates below are completed hashes divided by actual elapsed time, including
round-end fences. They describe the tested configuration, not a guaranteed gain
on another machine. No campaign PR has been merged or released.

## Architecture evidence collected

| Machine / configuration | Control → candidate H/s | Evidence and disposition |
| --- | ---: | --- |
| Native Windows Ryzen 9950X3D, 1 worker, regular pages, AVX2 → AVX-512 | 2.416 → 2.621 | Four alternating pairs, all positive (+2.55% to +11.12%). Supports existing automatic ISA comparison. |
| Same Ryzen, 8 workers spread across both CCDs | 7.224 → 7.528 | Four pairs, +0.53% to +6.65%; retain automatic per-machine selection. |
| Same Ryzen, 4 physical cores → four SMT threads on two physical cores | 6.482 → 6.174 | Four pairs, every SMT arrangement slower (−3.11% to −6.18%); supports physical-first placement. |
| Same Ryzen, CCD/cache placement | Inconclusive | Four-pair CCD/spreading comparisons do not justify a new default. The 1-worker CCD series includes a −36.77% reversal; retained rather than discarded. |
| Graviton3 / Neoverse-V1, 1 worker, 2 MiB HugeTLB, NEON → final `f20e7bb` SHA3 + 64-byte prefetch | 0.824 → 1.230 | Four corrected alternating pairs, +49.17% to +49.42%; full-size runtime parity passed. |
| Same Graviton3, 16 physical workers, 2 MiB HugeTLB | 12.520 → 18.170 | Four corrected alternating pairs, +44.31% to +45.91%; frozen generic CPU-only binaries. |
| Same Graviton3, regular pages, 1 / 8 workers | 0.762 → 1.038 / 5.892 → 7.807 | Four corrected alternating pairs each, +35.52% to +36.72% / +32.23% to +32.86%. |
| Graviton2 / Neoverse-N1 without SHA3, 1 / 16 workers, 2 MiB HugeTLB, baseline → final `f20e7bb` portable NEON | 0.680 → 0.707 / 10.609 → 11.047 | Four corrected alternating pairs: +3.81% to +3.95% / +3.83% to +4.55%. Full-size dirty-reuse parity and unsupported SHA3 rejection passed. This is the fallback path, not SHA3 execution. |
| Graviton4 / Neoverse-V2, 1 / 16 workers, 2 MiB HugeTLB, NEON → runtime SHA3 + prefetch | 0.891 → 1.373 / 13.800 → 21.503 | Four corrected, balanced, actual-order-verified pairs: +53.94% to +54.33% / +55.11% to +57.00%. Final636native lifecycle passed; f20x86-boundary change is near-flat on ARM (two pairs,16-worker mean−0.29%). |
| Intel Xeon Platinum 8375C / Ice Lake, 1 / 12 workers, 2 MiB HugeTLB, published AVX2 → AVX-512 | 1.449 → 1.853 / 11.139 → 12.118 | Two screening passes (order limitation below); full AVX-512 remains the stronger shipping path here. |
| Same Ice Lake, experimental 256-bit AVX-512, 1 / 12 workers | 1.552 / 11.913 | Slower than full AVX-512 in this screen; remains benchmark-only. |
| Same Ice Lake, streaming ring at 12 workers | 12.191 | No material improvement over published full AVX-512 (12.112); slower than separately rebuilt default-feature full AVX-512 (12.450). No promotion. |
| Intel Xeon Platinum 8488C / Sapphire Rapids, 1 / 12 workers, 2 MiB HugeTLB, AVX2 → AVX-512 | 1.617 → 1.726 / 12.512 → 13.099 | Four longer repeated pairs (AVX2-relative order limited), full AVX-512 +6.13% to +7.97% / +4.05% to +5.27%. Also leads across the preliminary worker-count screen. |
| Same Sapphire Rapids, experimental 256-bit AVX-512, 1 / 12 workers | 1.654 / 12.737 | Four longer pairs: full AVX-512 +4.10% to +4.62% / +2.22% to +3.10% over this path; no promotion. |
| Same Sapphire Rapids, streaming ring at 8 / 12 workers | 11.666 / 12.973 | Two screens (order limitation below); full AVX-512 11.605 / 12.969. No material gain; no promotion. |
| Intel Xeon 6975P-C / C8i, 1 / 12 workers, 2 MiB HugeTLB, AVX2 → AVX-512 | 1.809 → 1.938 / 12.428 → 12.916 | Two screens (order limitation below); full AVX-512 leads at all tested counts. |
| Same Xeon 6975P-C, experimental 256-bit AVX-512, 1 / 12 workers | 1.859 / 12.448 | Does not beat full AVX-512; no promotion. |
| Xeon 6975P-C, final `f20e7bb` versus baseline, 12 workers, full AVX-512 | 12.486 → 12.324 | Four longer corrected pairs, all negative (−0.063% to −3.719%), mean −1.304%. Earlier two short pairs also negative. Unresolved regression signal; PR #44 remains draft. |
| Skylake / Xeon 8124M, joint tuning prototype, four 900-second fresh-process/cache ABBA windows | 6.241 → 6.152 logged H/s | Both paired differences negative (−2.006%, −0.836%); aggregate −1.424%. No promotion; subsequent lifecycle intentionally deferred. These include startup/trials/shutdown and are not fixed-kernel rates. |
| Intel Xeon Platinum 8275CL / Cascade Lake, 12 workers, full AVX-512, baseline → combined `636e688` | 9.303 → 9.036 | Four corrected longer pairs lose 2.60–3.34%; regression retained and repaired below. |
| Same Cascade Lake, baseline → x86-boundary fix `f20e7bb` | 9.303 → 9.267 | Four pairs: −0.93% to +0.004%; about −0.39% mean. Consistently ahead of the regressed build; no x86 speedup claim. |
| Same Cascade Lake, published AVX2 / full AVX-512 / experimental 256-bit path, 12 workers | 10.247 / 9.254 / 10.405 | Four corrected longer pairs. AVX2 beats full-width here; narrow path's small advantage does not justify promotion. |
| Haswell / Xeon E5-2666 v3, final AVX2 regression, 1 / 11 workers | 1.175 → 1.175 / 9.722 → 9.679 | Two balanced pairs; eleven-worker differences −1.30% and +0.41%, so no repeatable loss or gain established. |
| Zen 1 / EPYC 7571, final AVX2 regression, 1 / 16 workers | 0.901 → 0.905 / 4.594 → 4.596 | Two balanced pairs and native full-size parity; sixteen-worker differences −0.027% to +0.111%, near-flat. |
| Zen 2 / EPYC 7R32, final AVX2 regression, 1 / 12 workers | 1.883 → 1.885 / 6.012 → 6.007 | Two balanced pairs and native full-size parity; twelve-worker differences −0.088% to −0.078%, near-flat. |
| Same Ice Lake, independent-hash interleaving, equal 4 GiB arenas | 1.977 versus ordinary SMT 2.232 | Rejected early after both run orders; best interleaving is about 11.4% slower than the same-physical-core SMT control. |

Final Graviton2/3 corrected confirmations passed. Fresh Xeon 6975P-C confirmation
retains a negative final-build signal, so ARM PR #44 remains draft. Final Sapphire
Rapids and Ice Lake checks are in progress.
EPYC evidence from the coordinated session is described separately below; it is
not relabelled as a v0.2.21 campaign measurement.

## Reproducibility and limitations

The [evidence directory](evidence/) keeps normalized per-round hash counts,
actual elapsed/fence times, page backing, error counters, build fingerprints and
raw-report SHA256 values. Windows result manifests additionally retain each
logical CPU mask, comparison role and pair order. Original reports and complete
SHA-verified cloud archives remain in ignored campaign data; the public
[archive index](evidence/archive-index.json) records their digests and verification.
The
[controllers](README.md#scripts) reproduce the configurations.

Some older shell controllers inherited `SEINE_GIT_COMMIT` into every comparison
process. The report's Git label can therefore name the same revision for distinct
executables. Raw labels remain unchanged. The public
[binary-provenance index](evidence/binary-provenance.json) records the actual
experiment binary SHA256 values, compiled source fingerprints and frozen source
mappings. These identities, rather than an overridable Git annotation, establish
which builds were compared. This includes the final Graviton4 boundary check:
its earlier binary is `636e688`, even though both raw Git labels say `f20e7bb`.

Native Windows tests used the published MSVC release. WSL's synthetic single-L3
topology was not used to infer CCD placement. Windows explicit large-page
allocation failed with error 1450, so the reported desktop comparisons require
regular backing and remain limited to eight workers / 16 GiB arenas. No native
Windows large-page or multi-processor-group claim is made.

Graviton3 uses 64-byte cache lines and has runtime SHA3 support. The candidate
combines XAR-based XOR/rotate with complete 64-byte prefetch coverage. Separate
screens isolated the SHA3 and prefetch contributions before combining them.
The baseline NEON path remains available; startup preference stays unchanged.
Native Graviton2 / Neoverse-N1, which lacks SHA3, passed three full-size
reference nonces and dirty reuse; unsupported SHA3 selection is rejected and
NEON remains active. Apple Silicon candidate performance remains untested.

The first Ice Lake streaming comparison is **excluded**: a shared Cargo target
reused an experimental library in the purported baseline. Symbol inspection
exposed the contamination. Fresh isolated builds, symbol checks and full-size
reference parity precede the replacement measurements above. The published
release screen and separately built interhash runner were unaffected.

The interhash timebox ended early at 11:44 UTC. Every mode used two full arenas:
sequential 1.867 H/s, pair chunks 1/4/16 at 1.977/1.875/1.843, ordinary workers on
one logical CPU 1.832, on both SMT siblings of one physical core 2.232, and on two
physical cores 3.599. All four direct modes passed full-size reference parity,
three nonce pairs and dirty reuse. This rejects the experiment on this Ice Lake
configuration; it does not prove a universal result across architectures.
The [retained interhash evidence](evidence/interhash-rejected.json) includes all
fourteen timed reports, completed run order, physical-core groups, full-size
parity lines and original archive/file digests.

## Benchmark order review and corrected confirmation

At 15:48 UTC, controller review found that variant rotation used the index in
an already-reversed worker/configuration loop. Reversing both loops can cancel:
the two-variant/two-worker ARM matrices always ran baseline first at one worker
and candidate first at sixteen. Those earlier gains did not meet
the intended per-worker order-reversal requirement. Raw results remain intact;
normalized evidence is labelled `order_limited`, and corrected native Graviton2,
Graviton3 and Graviton4 confirmation is required before making the ARM PR ready.

Graviton4 and fresh Graviton2/3 completed the corrected four-pass protocol with
gains in every pair; their original interrupted/order-limited runs remain archived.
Graviton2/3 used the final frozen `f20e7bb` binary. Graviton3 also passed four
corrected pairs at each regular-page worker count.

The fix rotates by each configuration's stable original index and asserts the
actual completed variant sequence reverses on every pass. The in-progress
Graviton4 matrix was interrupted, archived and replaced. Final x86 regressions
and subsequent architecture screens use the corrected controller. Earlier Intel
screens retain the limitation; Sapphire Rapids full-width versus 256-bit relative
order did reverse, but its comparison against AVX2 did not. The separate native
Windows four-pair controller and direct interhash reversal are unaffected.
No code candidate or individual sample was altered to obtain a better result.

## Resource lifecycle findings

Both Graviton3 and Ice Lake reproduced a constrained-start recovery bug: a
process restarted under `memory.high=3G` remained capped at one worker after
the limit rose to 12 GiB. Earlier pressure, zero-worker parking, productive
recovery and CPU-load phases passed, but the restart failure prevented the
subsequent opt-out phase. These runs are recorded as failures, not partial
passes. Graviton3 independently confirmed automatic SHA3 selection at 194.1
seconds and a later two-to-three-worker increase before the restart.

The repair separates parked worker capacity and nonce stride from changing RAM
limits, while active arenas remain bounded by the current allowance. Recovery
gets a debounced opportunity even during kernel comparisons. The standalone
repair passes 484 CPU-only workspace tests (four ignored) and independent
review by the autotune owner. Native reruns additionally require at least
4 GiB of resident anonymous memory after restart recovery, so a requested
thread count alone cannot hide an undersized backend. The complete fixed Sapphire Rapids rerun passed all nine phases, including
fixed-thread opt-out. Restart recovery reached three workers and 6.028 GiB
anonymous residency; no OOM kills or quarantine occurred. Both full-arena
growth/switch tests, full-size ISA parity, 485 workspace tests and strict kernel
Clippy passed. The same complete lifecycle also passed on Graviton2, including
three workers and 6.028 GiB anonymous residency after constrained restart.

Two repeated, order-limited Ice Lake regression pairs compare the ARM-change
build against its separately rebuilt CPU-only baseline: approximately −0.29%
at one worker and −0.27% at twelve. Each worker count kept the same variant order
in both passes. These small negatives and that limitation are retained explicitly.
Full reference parity and native workspace tests passed.

The follow-up review found a separate startup regression in the recovery repair:
parked OS threads can exhaust a tight PID budget. A network-free local backend
probe with explicit 16-CPU affinity, `pids.max=12` and `memory.high=3G` starts
with the legacy one-slot capacity, but the new sixteen-slot capacity fails at
worker 9 with EAGAIN. Both use one enabled arena. Owned test units were cleaned
up. The [reproducer patch](pid-startup-probe.patch), [controller](pid_probe.py)
and [evidence](evidence/pid-startup-regression.json) are retained. The autotune owner implemented focused hardening: nested cgroup PID headroom
bounds parked slots, adaptive resource-related spawn failures clean up and retry
with fewer workers, and manual counts retain RAM-safe limits. The same real OS
probe now passes with four provisioned slots and unchanged stride 16; the legacy
one-slot control also passes. This probe deliberately bypasses preflight and is
not a full pool runtime test. PR #43 is ready for review after combined native pool
startup and resource lifecycle checks passed on Graviton4 and Xeon 6975P-C.
The final combined candidate `636e688` passes all-feature/all-target Linux
compilation and scoped Rust formatting. A separate read-only review by the
research owner found no blocker in nonce stride, teardown/retry, PID headroom,
manual bounds or platform-scoped resource errors. Native confirmation passed;
the reserve remains explicitly best effort against concurrent tasks.

The first full-pool limit of 24 tasks was insufficient even for v0.2.21: its
existing sysinfo/Rayon pool creates sixteen threads during startup discovery,
and the baseline then fails spawning the pool client. The candidate instead
reports insufficient headroom before provisioning. Both failed runs are retained;
the revised full-pool acceptance budget is 40 tasks, still below the combined
CPU-sized parked pool plus runtime requirement. No Rayon environment override
is used and product source remains frozen. Reducing startup's persistent Rayon
thread footprint is a separate future optimization.

The frozen final combined candidate `636e688` passed **495 CPU-only workspace
tests** (five resource-dependent tests ignored), full-size scalar/AVX2/AVX-512/256
parity, both explicit real-arena tests and strict kernel Clippy on Xeon 6975P-C.
Its complete nine-phase lifecycle passed, including restart growth to three
workers / 6.028 GiB anonymous residency, no OOM/quarantine and opt-out. The
40-task full-pool check provisioned nine CPU slots, observed at most 35 tasks
and 84 completed hashes before clean shutdown. Graviton4 also passed all nine
phases, with three workers /6.02784GiB after restart and no OOM/quarantine. Its
40-task check observed nine slots,35tasks and54completedhashes.

Corrected-order final x86 regression pairs at one/twelve workers show AVX2 mean
changes of +0.26% / −0.48% (the latter pairs range −1.13% to +0.16%) and full
AVX-512 +0.53% / +0.58%. These small differences are retained without claiming
an x86 kernel optimization. Both builds use the same CPU-only generic profile.
All sixteen reports passed actual-order verification; a deliberately partial
first-pass export was rejected without creating a confirmation artifact.

## Cross-architecture code-generation regression investigation

Cascade Lake exposed a repeatable approximately 3% full-width AVX-512 slowdown
in the combined `636e688` candidate at twelve workers. The same frozen binaries
were near-flat on Xeon 6975P-C. This is retained as a regression, not dismissed
because the ARM gains are larger.

Static inspection of the actual miner found that the ARM refactor's small
wrapper caused x86 fill loops to inline into the dispatch function. The enclosing
function grew from 1,780 to 32,130 bytes and hot-loop register allocation changed.
That size includes alternate ISA paths which are not all executed by one hash.
The AVX-512 compression instructions and referenced constants remained identical.

An isolated four-line change, `f20e7bb`, keeps the x86 fill wrapper out of line
without applying that attribute to ARM. The resulting dispatch and all four
fill functions match the baseline's normalized instruction sequences and sizes.
See [static evidence](evidence/x86-codegen-review.json). Static equivalence does
not establish performance recovery by itself. The completed four-pair native
comparison measured 9.303 / 9.036 / 9.267 H/s respectively: the fix consistently
beats the regressed build while retaining a roughly −0.39% baseline-relative mean.
The ARM check is near-flat (sixteen-worker pairs −0.712% and +0.142%). The fix
is now incorporated in the ARM PR. Corrected final Graviton2/3 confirmation
passed. Fresh final-binary Xeon 6975P-C results retain a negative signal:
two short full-width pairs at twelve workers were −0.271% and −1.274%; four
longer balanced pairs were −0.063%, −0.071%, −3.719% and −1.480%. The longer
means are 12.486 versus 12.324 H/s (−1.304%). Both sides show throughput drift;
these measurements do not establish a stable effect size or isolate the causal
change within the combined stack. They cannot be discarded: actual order,
executable identities, 24 GiB backing, zero errors and full-size parity passed
independent audit. Normalized instruction equivalence did not establish runtime
performance equivalence. **PR #44 remains draft pending investigation.**

The separate combined integration branch includes PRs #40, #41, #43, #44 and
#45 plus this candidate fix. It passes 497 workspace tests (five ignored),
all-feature/all-target Linux compilation, Windows GNU test cross-compilation,
scoped formatting and strict kernel Clippy. Cross-compilation is not native
Windows execution. [Evidence](evidence/integration-check.json) retains log hashes.

## Reviewable changes

All proposed production behavior is automatic by default. ARM startup retains
portable NEON; runtime feature detection only admits SHA3 as a challenger, and
productive-work confirmation selects a winner. The existing
`--disable-cpu-autotune-threads` disables both automatic count and kernel tuning.
Explicit fixed counts retain the default ISA unless tuning is explicitly enabled.
Resource-lifecycle acceptance includes the opt-out case. These changes do not
introduce an offline startup benchmark or require users to select a CPU family.

- [PR #40](https://github.com/zpalmtree/seine/pull/40): Windows tuning-cache
  identity includes permitted CPUs and canonical physical-core groups. Native
  mask tests and CPU-only workspace tests passed. No placement policy change.
- [PR #41](https://github.com/zpalmtree/seine/pull/41): inconclusive kernel
  confirmations retry after five minutes without caching a loss; aborted new
  contexts do not inherit an old 30-minute delay. CPU-only workspace tests and
  independent review by the autotune owner passed. Stable below-threshold
  results retain their existing policy.
- [PR #43](https://github.com/zpalmtree/seine/pull/43): recoverable CPU
  worker capacity after temporary RAM limits, stacked on #41. Native fixed
  lifecycle passed on Sapphire Rapids and Graviton2. Final PID-safe combined
  native confirmation passed on Graviton4 and Xeon 6975P-C.
- [PR #45](https://github.com/zpalmtree/seine/pull/45): native CPU-only Linux
  ARM release archive with kernel tests and full-size reference verification.
  Native source build, package contents/digests and YAML checks pass. The
  Ubuntu 24.04 ARM archive requires glibc 2.39 (verified by ELF inspection),
  documented with a source-build fallback for older systems; the hosted
  release workflow has not been run.
- [Draft PR #44](https://github.com/zpalmtree/seine/pull/44): runtime ARM
  SHA3/prefetch candidate, stacked on #43. Native full-size parity, small
  boundary tests, real-arena worker switches and Graviton3 automatic selection
  passed. Graviton4 and complete repaired lifecycle confirmation passed, as did
  corrected final Graviton2/3 confirmation. Fresh Xeon 6975P-C final-build checks
  retain a negative throughput signal. This combined-stack result does not
  isolate the causal change to #44; the draft gate remains closed. Cascade Lake's
  earlier inlining regression was reduced, with the residual negative disclosed.

Strict workspace/binary Clippy is not clean at baseline. On Ice Lake, baseline
and candidate all-target runs produced the same 65 error-message lines (63
underlying diagnostics plus build summaries); strict kernel-crate Clippy passed.
The Windows cache PR's binary-only check found 61 existing diagnostics, all in
source files byte-identical to its parent. These are disclosed, not counted as
passing checks. The final combined all-target check reported 124 existing
diagnostics, with none on changed lines; strict kernel-crate Clippy passed.

[Draft PR #46](https://github.com/zpalmtree/seine/pull/46) separately prototypes
joint worker-count/kernel exploration. It restores the original configuration
for confirmation and requires a repeated outer comparison when both change.
Review found and fixed a quarantine/survivor cancellation issue. The frozen
review build passes 506 workspace tests (five ignored), full-size local parity,
all-feature compilation and Windows test cross-compilation. Native full-size parity and all four 900-second fresh-process/cache ABBA windows
completed on an actual Skylake Xeon 8124M guest. The prototype's logged completed
work was lower in both comparisons (−2.006% and −0.836%; aggregate −1.424%).
Execution health checks passed with no swap or OOM; they do not establish a gain.
This is not a rerun of the earlier Cascade Lake crossover because the CPU differs.
The subsequent prototype lifecycle was intentionally interrupted to prioritize
final Intel regressions within the deadline. Raw `FAILED / InterruptedError(15)`
is preserved with a scope-stop explanation and successful unit/hugepage cleanup;
it is neither a product failure nor a lifecycle pass. The prototype remains
excluded from accepted defaults. See [joint evidence](evidence/skylake-joint-confirm.json).

The rejected or deferred policy changes are explicit: no new CCD preference,
no automatic 256-bit AVX-512 path, no Intel streaming-ring default, and no
independent-hash interleaving default. Their controls and negative results remain
available for future work. The coordinated EPYC ring is a separate unpromoted
candidate, described below.

## Coordinated EPYC evidence

The EPYC owner completed its work and released all cloud resources before this
campaign acquired the shared 32-vCPU quota. On its older base `785a06f`, a 64 KiB
streaming ring on EPYC 9R45 at 24 workers measured 45.143 H/s versus 35.328 AVX2,
with three alternating pairs (+27.34% to +28.42%), full-size parity, dirty reuse
and sentinel checks. One-worker and eight-worker screens did not establish a
benefit. The candidate still needs a distinct production identity and applicable
worker-switch validation before promotion. No DRAM-bandwidth claim is inferred
from its available performance counters.

## Remaining hardware and experience gaps

- Final-candidate native Zen 3 and Zen 4/5 EPYC validation remains separate work.
  Existing coordinated results use older sources and are not relabelled as
  fresh measurements of this campaign's frozen combined candidate.
- The 9950X3D campaign used regular pages and at most eight workers. Native
  Windows large pages, higher worker counts, final combined-binary performance
  and machines spanning Windows processor groups remain unverified.
- Apple Silicon, Intel hybrid P/E-core machines and multi-socket NUMA systems
  need native candidate measurements. Cloud results apply to the observed guest
  topology; they do not establish bare-metal bandwidth or frequency behavior.
- The separate joint-count/kernel prototype still needs repeatable end-to-end
  benefit across more than one architecture before becoming a default.
  Its fixed-kernel results alone cannot establish that policy benefit.
- Startup discovery's persistent Rayon thread pool remains an opportunity to
  reduce overhead and support tighter PID budgets. The campaign's 24-task
  failures affected the baseline too; they were not hidden by an environment
  override.

GPU, a new PoW and energy measurements were intentionally outside this campaign.

## Resources and completion

The first fresh final-regression C8i rental encountered an SSH connection timeout
before creating its campaign directory or running a benchmark. Its instance and
disk were verified deleted, and its charges remain in the ledger. The replacement
uses bounded retries only for proven pre-connection timeouts; command execution
errors are never replayed. This is an operational failure, not a benchmark result.

Current resource accounting, additional hardware gaps, final PRs and verified
cleanup will be added before the 20:27:13 UTC deadline. Every lease has an
independent guest shutdown and EC2 terminate-on-shutdown behavior. The campaign
reserves at most $40 incremental cloud spending and stops new experiments by
17:57:13 UTC to protect the final 2.5-hour confirmation/review/cleanup window.

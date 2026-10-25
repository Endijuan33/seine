# Turin productive kernel-sample windows

Prepared on 2026-10-25 while separate frozen Turin and Genoa validations were
running. **Experimental branch only; no native measurement or shipping change
yet.** Earlier candidate-order and streaming-ring experiments retain priority.

## Hypothesis and candidate

The existing kernel tuner spends at least 30 seconds on every A/B/A sample and
requires two successful cycles before promoting a challenger. Its samples often
accumulate hundreds or thousands of hashes on the qualified EPYC 9355P. Shorter
high-count samples might reduce time spent on slower kernels and reach useful
worker counts earlier. They might also increase drift/retries or make poorer
choices; a lower nominal tuning duration is not evidence of a productive gain.

For AuthenticAMD family 1Ah/model 02h with a supported streaming candidate and
the default six-second CPU-autotune setting, allow a kernel sample to finish
after **both 10 seconds and 256 completed hashes**. Otherwise retain the existing
30–60-second sample window and minimum 32 hashes. Explicitly lengthened tuning
settings and all other identities keep their policy. The 2% gain threshold,
duration-dependent quantization margin, 5% baseline drift bound, two confirming
A/B/A cycles, switch acknowledgements, retry/backoff, cache context and resource
safeguards remain unchanged. The affected cache configuration gets a new policy
identifier; unaffected configurations keep their existing identifier.

No hash kernel, count-search policy, affinity, page mode, power cap, candidate
eligibility or challenger ordering is changed by this experiment.

## Preregistered native protocol

Do not start until prior native work has completed, its evidence is locally
verified, and host settings are restored. Freeze the baseline revision, exact
candidate patch, executable checksums, toolchain and controller files before
launch. Both builds must share any previously accepted improvements; never
attribute unrelated earlier changes to this comparison. The baseline will be
the then-current accepted policy. A rejected earlier experiment is not included.

Apply `raw-trial-audit.patch` to **both** measurement builds. This experiment-only
logging patch appends each completed comparison's exact hash counts and sample
durations to its existing decision message. The winner cache alone is insufficient:
a later comparison can replace the earlier promotion evidence for that context.
Record this shared instrumentation in both build identities and preserve its
patch; it is not part of the proposed shipping policy. `audit_decisions.py`
recomputes every logged decision, including both confirmation cycles, drift,
quantization and minimum sample requirements, directly from those raw values.

Use the same qualified EPYC 9355P, 32 cores/64 threads, 192 GiB across twelve
DDR5-4800 channels, one NUMA node and stock 280 W cap. Build portable CPU-only
Rust 1.93.0 executables with identical flags. Reserve explicit 2 MiB HugeTLB;
leave kernel and worker counts automatic, with all 64 logical CPUs available.

1. Verify reference parity and 21 full-size dirty-arena kernel transitions.
2. Run a separate 60-second plumbing invocation, excluded from comparisons.
3. Check fixed streaming at 32 workers in three alternating build-control pairs:
   20-second warmup and two measured 20-second rounds per invocation.
4. Run four paired cold-cache productive windows of 1,200 seconds per invocation,
   alternating control/candidate then candidate/control. Every leg gets its own
   empty cache. Keep every result and record actual commands and trajectories.
5. Restart each leg from its own resulting cache for 180 seconds, reversing the
   cold-run order. Preserve count/kernel cache restoration messages and context.
6. Run the established ordinary-page four-CPU resource lifecycle checks: memory
   pressure, worker parking, contention, constrained restart, recovery and opt-out.
7. Restore settings and collect checksummed raw logs, caches, counters, reports,
   frozen sources and executable identities before further native work or deletion.

The primary metric is completed-hash lower bound divided by service-start through
stopped-service wall time. Adoption requires the cold paired geometric gain's
lower descriptive 95% bound to exceed **+1%**, cached lower bound at least **-2%**,
all correctness/resource gates, and manual review of short-sample decisions,
drift/retries, selected counts and cache restoration. Verify every promotion
against its recorded durations and counts; do not assume 30-second samples.
Require short samples to have actually occurred in the candidate cold runs;
warm cached runs may legitimately have no new kernel comparisons.
Do not substitute shorter windows or fewer pairs if time becomes tight. Preserve
rejected results and their exact patch without enabling the policy on master.

This is productive-throughput validation, not a package-power claim. Any power
claim would need a separate paired energy protocol after this gate passes.

`run.py` wraps frozen copies of the existing full-socket productive controller
and helpers. The helper service prefix becomes `seine-short-windows-` so its
units cannot overwrite earlier runs. Record each original and copied file hash.
Use outer unit `seine-amd-short-windows.service` with a 14,460-second maximum,
SIGINT shutdown and bounded control-group cleanup. The wrapper requires
`prior-evidence-collected.json` with verified archives and restored settings for
both prior native experiments, and `run-authorization.json` with the exact
ledger-recorded rental reservation, funded-credit horizon and controller/deletion
deadlines. Provider deletion must precede both the funded horizon and goal deadline. It rejects
other active campaign units before running the controller's own preflight.

## Time and rental constraints

No rental extension is scheduled by this document. Launch only when at least
four hours of controller time plus collection/termination margin remain before
the campaign's **14:42:52 EDT** deadline. Record an exact controller deadline and
provider-deletion deadline in the campaign ledger before starting. If that
cannot fit after the prior experiments, keep this as an unmeasured candidate.

At $1.20/hour, up to eleven total Cherry rental hours would reserve $13.20;
with the existing $3.25 Genoa allowance, that is $16.45 of the approved $25.
Round rental exposure conservatively, include provisioning and earlier phases,
and terminate promptly when finished. The dollar cap does not extend the goal's
time limit, and provider shutdown alone does not stop Cherry billing.

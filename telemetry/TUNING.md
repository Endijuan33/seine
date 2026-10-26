# Paired CPU trial telemetry

Schema 6 adds bounded, typed evidence from a miner's own baseline/candidate
comparisons. The contract is [src/tuning.rs](src/tuning.rs); the complete synthetic
example is [tuning-fixture.json](tuning-fixture.json). Fleet averages do not become
automatic defaults.

## Producer contract

`Recorder::trial_elapsed_ms()` supplies a common run-relative monotonic clock.
`Recorder::record_trial(TrialOutcome)` accepts completed, inconclusive and aborted
CPU kernel, worker-count and joint trials. The productive CPU kernel producer
landed in `334b827`; worker-count and joint producers remain pending. Kernel
records currently lack measured allowed-CPU and placement context, so they count
toward coverage but cannot qualify hardware hints. Collector deployment and miner
release are separate from landing code. See [the roadmap](ROADMAP.md).

Record actual baseline/candidate settings, before/after resource context, and
each counted window's settings/context. Member slots are stable indices in this
process's CPU backend list (not the mixed CPU/GPU list), below 128, sorted and
unique; include all members in a joint comparison.
Settings include workers, public kernel identifier, requested affinity/page policy
and allocator-observed backing by page type. Do not substitute desired settings
or zero for an unknown observation.

Resources include allowed CPU count, available parallelism, effective memory limit,
available memory and an optional topology/placement fingerprint. Hash only allowed
CPU, NUMA, L3 and affinity inputs: never hostnames, wallets, configuration values,
paths or other identifiers. Unknowns remain null. The digest is pseudonymous
topology evidence, not proof of an independently operated physical host.

Completed trials contain three or six ordered counted windows: baseline/candidate/
baseline, optionally repeated. Each records hashes, elapsed milliseconds, arm,
context and whether it changed. Incomplete trials may retain fewer windows; do not
discard them because they cannot establish a gain. Record the actual decision and
applied arm; execution failure may have no applied arm and an empty after-context.

Policy includes revision, minimum hashes, minimum sample duration, maximum baseline
drift, fixed gain margin and quantization rule. Optional `short_sample_ms` and
`short_minimum_hashes` must appear together. Each window must satisfy either the
full duration/hash rule or the shorter duration/higher-hash rule; never mix one
rule's duration with the other's hash threshold. For example, 10 seconds/256 hashes
or 30 seconds/32 hashes permits a 10-second/300-hash baseline and a 30-second/
64-hash candidate in the same trial, but rejects a 10-second/32-hash window.
Omitting both optional fields preserves single-rule policies such as 45 seconds/
2048 hashes. `three_window_hash` uses one hash
of resolution from each A/B/A window; a zero fixed margin does not disable that
guard. Policies form separate cohorts. Analysis applies the recorded alternative
rules per window and requires a nonzero measured baseline. Do not change runtime
acceptance rules to fit telemetry.

Costs are optional measured milliseconds. Switching, per-window settling and
observed no-job idle must not overlap each other or counted measurement time.
Unknown costs stay null. Switching/settling can include productive hashing and
are not idle. `idle_ms` means observed time without useful mining work, not total
tuning duration. Start/end use the recorder clock and enclose all known costs and
counted durations; the end cannot exceed enclosing report uptime.

## Bounds, privacy and delivery

Validation uses fixed enums, numeric bounds and public identifier formats, with
no arbitrary log/text field. Each outcome has at most eight CPU members per
context and six samples. A nonblocking FIFO retains eight outcomes and assigns
monotonic trial IDs within a random run ID. Invalid, contended and full-queue
records increment explicit omission counts.

The reporting worker releases the queue lock before serialization, retaining at
most 16 KiB of trial evidence inside the existing 32 KiB report limit. Earlier
outcomes survive regardless of result; trimming increments omission counts.
Those counts cannot reconstruct the missing outcomes. Trial recording performs
no network or disk IO in mining control loops.

Trial-only final reports can precede the first regular upload. Failed delivery
uses the single pending report, 24-hour expiry and persisted five-minute retry
cooldown. IDs survive replay. Opt-out applies to new and pending evidence, and
benchmark mode remains excluded. Abrupt termination may lose queued trials.

## Analysis and research hints

`python3 telemetry/agent_report.py` runs [tuning_report.sql](tuning_report.sql)
read-only alongside health, inventory, performance and reliability. It analyzes
seven days of release reports without requiring a 24-hour trial span. Duplicate
installation/run/trial IDs count once; conflicting copies are excluded. Freshness
uses the earliest retained receipt for a trial within the analysis window.

Qualified A/B/A cycles require actual settings matching the declared arms, stable
context, complete resource/placement/source/mode evidence, sufficient hashes and
duration, and acceptable baseline drift. Free memory is observed but not a cohort
key; limits and topology are. Candidate rate is compared with the combined
before/after baseline rate. Drift uses the before baseline as denominator;
quantization follows the recorded policy.

Each installation contributes one median ratio per exact hardware/environment,
source-fingerprint, mode, settings, resource-context and policy cohort. Release
labels may differ when the source fingerprint matches; versions/build revisions
remain provenance. Repeated trials do not become extra hosts. Cost summaries
also report how many costs were measured.

At least five qualified installations are needed for a ranked research hint.
CPU-kernel gains require a reported gain, acknowledged candidate settings and two
A/B/A cycles, each above its own policy margin plus quantization. A strong first
cycle cannot average away a failed
confirmation. An installation contributes gain support only when a majority of
all its observed trials in that context meet this rule, including aborted and
inconclusive trials in the denominator. Raw positive ratios remain descriptive.

`observational_support` requires a positive cohort median, support from at least
60% of all observed installations in that context, and an acknowledged, resolved
gain received within 48 hours. Otherwise the label is `mixed_or_limited_evidence`. These are
explicit research-screening labels, not calibrated probabilities or confidence
intervals. Percentiles describe spread; installations are not independently
verified hosts. No-confirmed-gain cohorts remain visible and do not prove regression.

Each hint includes `evidence_coverage` with all observed outcomes and their
installation-weighted wall/idle/switching costs and measurement counts. Top-level
cost summaries cover qualified comparisons only; the nested all-outcome costs
also expose unsuccessful tuning effort. Unknown costs stay null. Coverage-only,
missing initial context and staged trial-kind counts are explicit and can overlap.
Worker-count/joint trials remain coverage-only until their producers and explicit
throughput/efficiency objective contracts are validated.

Hints prioritize experiments within exact hardware/context and baseline/candidate
pairs. Sorting ratios across different baselines is not a highest-H/s comparison.
Every hint has `automatic_default_eligible: false` and requires native validation,
correctness checks and independently verified host evidence before shipping a
default. The runtime does not download or depend on this analysis.

## Rollout

Deploy the schema-6 collector before releasing schema-6 miners. It accepts schemas
1–5 too; no database migration is needed. Deploy the SQL and reporting script
with it. Runtime integration is separate: collector upgrade alone does not mean
the fleet sends paired trials.

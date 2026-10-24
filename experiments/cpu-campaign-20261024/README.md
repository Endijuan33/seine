# CPU architecture campaign, 2026-10-24

Baseline: released **v0.2.21**, source
`359265e325d85c9c7b6e3eed7b5b126fcec63f9c`. Work began at 10:27:13 UTC.
New experiments stop by 17:57:13 UTC; confirmation, review, documentation and
cleanup have the final 2.5 hours, ending no later than 20:27:13 UTC.

This directory holds reproducible campaign controllers and the eventual results.
It does not change default kernel selection. Raw reports, downloaded binaries,
cloud credentials and private resource identifiers stay in ignored
`data/cpu-campaign-20261024/`. No GPU or energy measurements are part of this work.
The current PoW remains Argon2id v1.3, 2 GiB, one pass and one lane per hash.

## Experiment plan and evidence rules

| Area | Question | Required control |
| --- | --- | --- |
| Ryzen 9950X3D | Cache CCD, frequency CCD, spreading, physical cores and SMT | Same binary, page backing, worker count and total RAM; reversed order, then longer paired runs |
| Native Graviton | Generic NEON versus SHA3-enabled NEON; targeted follow-ups | Native full-size RustCrypto reference parity; generic build kept as fallback control |
| Intel Xeon | AVX2, AVX-512, experimental 256-bit AVX-512 by worker count | Same binary, physical-first CPU list, verified regular/HugeTLB backing |
| Independent hashes | Can DD-only interleaving hide latency on one core? | Two full arenas in every mode; sequential dual-arena, ordinary same-core workers, SMT and separate-core controls identified separately |
| Automatic tuning | Resource changes, cache reuse, process restart and opt-out | Real released miner against a loopback pool fixture; bounded cgroups and no public pool traffic |

Full-size reference parity precedes timing. Rates count all completed hashes and
include fence/overrun time. Short screens select follow-up work; they do not prove
a production gain. Longer alternating runs must beat the strongest applicable
shipping control without hiding worker-count regressions. Failed samples and
losing experiments are retained as evidence. Nothing experimental enters default
dispatch solely because the hardware advertises an ISA.

## Initial observations

Native Windows reports two L3 groups on this 9950X3D: 96 MiB on logical CPUs
0–15 and 32 MiB on 16–31. Adjacent logical CPU pairs are SMT siblings. WSL reports
only one L3 group and therefore cannot establish physical CCD placement here.
The native large-page pilot failed with Windows allocation error 1450; subsequent
desktop comparisons explicitly require regular backing, at most eight workers
(16 GiB arenas), plus at least 10 GiB of free host-memory headroom before launch.
This is a shared desktop, so longer comparisons must account for host variability.

The Windows tuning-cache fingerprint currently omits allowed CPU IDs. The
campaign includes a separate correctness fix so equal-sized masks selecting
different CCDs cannot reuse each other's tuning identity. This is independent of
any proposed change to worker placement.

The interhash experiment is isolated in branch
`codex/interhash-campaign-20261024`, initially commit `8432f45`, with its feature
disabled by default. The development/initial-screen time box is 10:44–12:14 UTC.
Small reference tests cover slice boundaries, multiple kernels and dirty reuse;
native full-size parity and performance remain required. Historical Zen 3
interleaving losses are recorded in CPU journal attempt 34. Historical Apple
attempt 31 mixes total pair throughput and per-lane rates; this campaign compares
total completed throughput directly and reports allocated cores and RAM.

## Cloud accounting and ownership

`cloud.py` uses the existing `seine-bench` profile in `us-east-1`. It checks the
shared Standard-vCPU quota before every launch. Existing EPYC work owns all
32 vCPUs until its cleanup notification; this campaign must not race that lease.
Only resources tagged `Experiment=cpu-campaign-20261024` are owned here.

Live AWS Pricing API queries at 10:29 UTC returned:

| Instance | Linux On-Demand USD/hour |
| --- | ---: |
| m7g.4xlarge | 0.65280 |
| m8g.4xlarge | 0.71808 |
| c6i.4xlarge | 0.68000 |
| c7i.4xlarge | 0.71400 |
| c8i.4xlarge | 0.74968 |

Separate live API queries confirmed gp3 at $0.08/GB-month and in-use public IPv4
at $0.005/hour. Each launch reserves three hours of instance/storage/address cost
plus a $1 transfer contingency against the **$40 incremental cap**. Root disks
are 16 GiB gp3 with deletion on termination. No extra provisioned IOPS/throughput
are requested. The ledger deliberately retains every reservation after cleanup,
making its total a conservative cumulative cap rather than reusable credit.

The guest schedules an absolute-deadline shutdown before installing tools. EC2
converts that shutdown to termination independently of the controlling session.
The controller verifies `/run/systemd/shutdown/scheduled` before experiments.
Leases last at most 150 minutes and must expire by 20:00 UTC. Normal completion
terminates earlier and verifies instance termination, root-volume deletion and
eventual deletion of the task-owned security group and SSH keys.

## Scripts

- `bootstrap.sh`: install the pinned toolchain on a disposable guest, build from
  the exact baseline archive, and run native reference checks and screening.
- `screen.py`: alternating ISA/build screening with page, error, OOM, swap and
  steal-time checks; restores the guest's initially empty HugeTLB pool.
- `interhash_screen.py`: three alternating equal-RAM passes after full-size
  parity; uses the separately built research-only executable.
- `resource_lifecycle.py`: real miner process tests under bounded CPU/memory
  controls. A service-private hosts file redirects the built-in developer pool
  to the local fixture, with additional loopback-only IP restrictions.
- `cloud.py`: launch/inspect/copy/terminate task-owned leases and record cleanup.

Native Windows scripts and topology artifacts currently live with raw data and
will be reduced to reproducible evidence before the campaign PR is finalized.

## Primary references for ARM investigation

- [Arm instruction-set reference](https://documentation-service.arm.com/static/67e40f3398aa3c3b6eea6a85): XAR is an XOR followed by a per-64-bit-lane rotation and requires FEAT_SHA3.
- [AWS Graviton technical guide](https://aws.github.io/graviton/): generation and architecture context; guest feature detection remains authoritative for execution.

## Results

Pending native screens and confirmation. No performance change has been accepted
into production by this campaign yet.

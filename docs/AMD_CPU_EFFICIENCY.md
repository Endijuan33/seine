# AMD CPU efficiency campaign

Prepared 2026-10-25. Hardware measurements are pending; the local WSL host has
no package-energy counters. This protocol measures the current legacy 2 GiB
Argon2id workload. Requalify conclusions if the PoW changes; these measurements
do not predict performance for a different algorithm.

## Current baseline

Use current `master`, not the older `codex/linux-numa-defaults` checkout. The
streaming kernel and productive joint count/kernel tuning have already landed.
[Performance](PERFORMANCE.md) records 49.39 H/s on a 32-worker Turin VM and
10.31 H/s on the native Windows 9950X3D streaming test. Neither establishes
package or wall energy efficiency, and the VM result is not a full-socket rate.
The [energy harness](CPU_ENERGY.md) already measures package joules over a full
invocation; its worker sweep now includes explicit streaming-kernel selection.

## Throughput work independent of energy-counter access

The next narrow eligibility investigation is **Genoa EPYC 9R14**, family
25/model 17. [Existing native evidence](../experiments/high-end-streaming-20261025/RESULTS.md)
measured streaming at 35.698 H/s versus AVX2 at 28.067 H/s at 24 workers
(+27.19% across four alternating pairs), but automatic streaming trials still
exclude this identity. The eight-worker screen was effectively flat. These
results support investigating an additional candidate, not forcing streaming
or assuming a worker count.

Before enabling it, validate the candidate-set change through productive
cold/cached runs, restart and resource-pressure recovery on Genoa. Keep the
runtime ISA check and repeated-gain gate, and verify that old candidate-set
caches are rejected. A throughput-only Linux host can answer this question;
power counters are needed for the separate efficiency campaign below.

The native tuner constructor now uses real CPU identity even in a test binary.
Synthetic fixtures explicitly request a neutral identity. Previously,
`cfg!(test)` removed streaming from the native EPYC test's candidates, making
its eligibility assertion impossible to pass. The ignored 24-worker EPYC test
is restricted to the validated Turin identity and still requires an owned host
with reserved HugeTLB memory; it is not a Genoa validation substitute.

After Genoa eligibility, investigate cold-start discovery cost on Turin: the
[productive campaign](../experiments/cpu-followups-20261025/joint-native-results.json)
recorded +3.53% cold versus +19.94% cached. Compare challenger ordering with
fresh caches and interleaved runs before changing policy. Preserve all
challengers, confirmation requirements and memory-pressure behavior.

## Rental shortlist

Public listings checked on 2026-10-25; prices and inventory must be rechecked
before ordering. No rental has been created for this campaign.

| Provider / configuration | Listed rate | Fit |
| --- | --- | --- |
| [Cherry EPYC 9355](https://www.cherryservers.com/pricing/dedicated-servers/amd-epyc-9355?billing=3), 32 cores, 192 GB DDR5 | EUR 1.026/hour | First candidate for a short bare-metal Zen 5 session; installed DIMM layout and counter access unverified |
| [Latitude f4.metal.large](https://www.latitude.sh/pricing), EPYC 9275F, 24 cores, 768 GB | From USD 2.62/hour | Alternative Zen 5 host; installed channels and energy counters also need verification |
| [Latitude m4.metal.xlarge](https://www.latitude.sh/pricing), EPYC 9455P, 48 cores, 768 GB | From USD 4.76/hour | Larger Zen 5 comparison at higher rental cost |
| [Scaleway EM-I520E](https://www.scaleway.com/en/pricing/elastic-metal/), EPYC 8434P | EUR 1.562/hour before tax in PAR-1 | Revisit for energy on the previously measured family; does not answer the 12-channel Turin question |

At Cherry's displayed base rate, two hours is EUR 2.052 before any taxes,
configuration changes or account-funding requirements. Reserve time for setup,
verification, collecting results and deleting the rental as well as hashing.
The authenticated USD checkout on 2026-10-25 shows USD 1.20/hour for the
prebuilt Lithuania 192 GB configuration (plan 984, hourly cycle 37), or
USD 2.40 for two hours. Although the general card top-up form permits USD 1,
deployment requires enough available credit for **eight hours (USD 9.60)**.
The deployment attempt was blocked by this balance requirement; no server was
created. Account credit is separate from actual rental expenditure.

The first card payment was also held pending identity verification. The portal
states that unverified pending transactions are refunded after 48 hours.
[Cherry's verification guide](https://www.cherryservers.com/knowledge/docs/usage-billing/identity-verification)
describes an iDenfy identity-document and camera flow that the account holder
must complete. Establish these funding and identity requirements before asking
an operator to deposit money; a small minimum top-up does not establish that
the same balance permits deployment.
A CPU's supported channel count and a total RAM figure do not establish the
installed DIMM/channel population. Get the actual layout before selecting a
configuration for the 12-channel experiment.

## Host qualification

Required for the proposed first session:

- Exclusive Linux bare-metal access with host root, an exact CPU identity and
  all 12 channels populated for the memory-scaling comparison. Record DIMM
  locators, configured speeds, socket/NUMA/L3 topology and allowed CPU masks.
- Readable, advancing package counters via Linux powercap. Check all sockets;
  record counter range and ensure the sample interval bounds wraps. If these
  counters are absent, this harness cannot establish H/W on that host.
- Provider permission for sustained offline Argon2 benchmarking, and access to
  supported package-power controls if a power-limit experiment is planned.
- For whole-system H/W, a separate calibrated wall/PDU energy source covering
  the same interval. Package counters exclude unmeasured DRAM and supply losses.

Useful read-only checks, saving their output with the experiment:

```bash
lscpu -e=CPU,CORE,SOCKET,NODE,CACHE,ONLINE
numactl --hardware
sudo dmidecode --type memory
python3 -B scripts/bench_cpu_energy.py probe
```

DIMM serial numbers and remote host identifiers should stay in private evidence.
Record and later restore any changed huge-page, governor, power or BIOS settings.
Use the host's `gpuq` policy where present. Keep other agents' workloads off the
measurement host. Do not run monitoring or miner workloads on the desktop merely
to fill the lack of a qualified remote sensor.

## Measurement order

1. Freeze the source revision and binary SHA-256. Confirm the requested kernels
   match full-size reference hashes on the host before collecting performance.
   Build host-native variants on that host if needed, in separate output trees.
2. Capture a short one-worker energy point to qualify the counter and report
   pipeline. It is a plumbing check, not an efficiency claim.
3. On the 32-core host, run the documented 8/16/24/32-worker sweep with fixed
   streaming kernel and verified page backing. Use four alternating pairs and
   a fresh output directory. Reserve at least 64 GiB of arenas plus headroom.
4. Confirm the efficiency shortlist against the observed peak using longer,
   fresh paired runs. If the plateau is between coarse points, test neighboring
   counts before confirmation. Repeat with another supported kernel separately
   where the throughput evidence warrants it.
5. At a fixed shortlisted count, compare stock package limits with a supported
   lower limit in alternating runs. Record the exact effective limits. Do not
   use TDP as measured watts. Restore the original setting afterward.
6. Validate cold and cached productive mining selection using a local fixture
   pool with the existing evidence harness; keep its throughput and latency
   results distinct from the invocation energy metric. Offline fixed-kernel
   wins alone do not justify an automatic policy change.
7. Copy the raw measurements and checksums off-host, restore settings, stop
   owned processes, delete the rental, and verify billing resources are gone.

Stop a campaign on reference mismatch, counter/report failure, swap/OOM,
backend errors, changed measurement conditions or exhaustion of its time
budget. Preserve the rejected evidence with its reason. Publish no H/W ranking
until a qualified host supplies real energy measurements.

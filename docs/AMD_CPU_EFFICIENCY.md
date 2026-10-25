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
USD 2.40 for two hours. The card top-up form permits USD 1 minimum; account
credit is separate from actual rental expenditure. No server has been ordered.
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

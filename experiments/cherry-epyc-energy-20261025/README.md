# EPYC 9355 package-energy qualification

One hourly Cherry Servers EPYC 9355, 192 GB configuration, ordered on
2026-10-25. Rate: USD 1.20/hour. Delete by **05:46 EDT**, including provisioning
in the two-hour limit. The USD 13 account credit is separate from the USD 2.40
planned rental expenditure. The operational ledger and private host inventory
remain under ignored `data/cherry-epyc-energy-20261025/`.

This is the legacy 2 GiB Argon2id workload. The frozen miner source is
`36f40e6381ddc99d5b97b8cb3b5436b119a335c6`; its binary SHA-256 is
`0831080222054ab087cfc0c6253eafa79a4870a5ed4be741f8f6c15db9ff496d`.
The controller, energy harness and binary hashes travel with the evidence.
No miner default or power-limit change is part of this measurement.

## Protocol, before observing measurements

1. Use `qualify.py` to capture actual CPU identity, physical topology, DIMM
   locators and configured speeds, page-pool state, kernel and package-energy
   availability. Review the installed DIMMs before describing twelve populated
   channels. Raw DMI serial numbers stay private.
2. If necessary, load the stock kernel's `intel_rapl_msr` module and repeat the
   sensor probe separately, preserving the initial result and module state.
   Despite its name, the [upstream driver supports AMD family 1Ah](https://github.com/torvalds/linux/blob/master/drivers/powercap/intel_rapl_msr.c).
   Do not install an out-of-tree driver or change counter permissions globally.
3. Require readable, advancing package counters, no swap/OOM, sufficient RAM,
   and full-size reference parity through all 21 guarded dirty-arena transitions.
   The image's unused `/swap.img` is recorded, disabled before measurement, and
   restored by the controller's cleanup. No samples were collected in the first
   preflight, which correctly refused the image's active swap configuration.
   If the host cannot provide qualified energy measurements, collect its
   inventory and terminate early instead of reporting a watts estimate.
4. Take a 20-second one-worker plumbing measurement. Reserve 64 GiB of explicit
   2 MiB HugeTLB pages only on this exclusively owned host, recording the prior
   pool size for restoration. Use the existing harness to test 8/16/24/32 workers,
   fixed `avx512-256-stream`, automatic affinity, stock power policies,
   60-second windows, four alternating pairs against 24 workers, ten-second
   settling, and a 2,400-second sweep limit.
5. Review the measured throughput/energy frontier. If a different worker count
   has lower joules/hash while retaining 95% of the observed peak rate, confirm
   it against the peak count with fresh alternating pairs and 120-second windows.
   Otherwise retain the result as an exploratory sweep, without manufacturing
   an efficiency winner. Neighboring counts can be a separate future experiment.
6. Stop owned workloads, restore the initial page pool and any module state
   changed for this task, copy all reports and raw counter samples with hashes,
   then delete the server and verify zero hourly usage. Keep time for collection
   and deletion; do not extend the rental to finish a measurement phase.

Energy covers each complete miner invocation, including allocation, startup,
fences and teardown. It is CPU package energy, not wall-plug energy or separately
measured DRAM consumption. Do not compare these invocation rates directly to
steady-state kernel benchmark H/s or substitute TDP for measured power.

These observations can inform the next worker/power-policy investigation.
They do not independently justify an automatic miner policy change: productive
cold/cached selection and resource recovery need validation for that change.

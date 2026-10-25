# AMD performance campaign — 2026-10-25

Research status at **10:40 EDT**: five completed datasets provide **29 reusable
benchmark records**. The full short-sample tuning experiment is still running;
the combined page/power comparison is prepared but unmeasured. This is an
interim research report, not a claim that the campaign or rental cleanup is done.

## Measured results

| Experiment | Exact measured scope | Result | Disposition |
|---|---|---|---|
| EPYC 9R14 / c7a.8xlarge | Automatic cold discovery, two 900-second pairs | +2.85% productive H/s | Streaming eligibility accepted for family 19h/model 11h |
| Same Genoa VM | Each run's own-cache restart, two 180-second pairs | +7.99% productive H/s | Same accepted eligibility change; separate cached phase |
| EPYC 9355P | Automatic cold discovery, four 1,200-second pairs | +0.685% productive H/s; first streaming confirmation observed about 90 seconds earlier | Challenger ordering accepted for family 1Ah/model 02h |
| Same Turin host | Each run's own-cache restart, four 180-second pairs | −0.092%, interval spans zero | Preserved alongside accepted cold-discovery result |
| EPYC 9355P, 32 → 64 fixed workers | Fresh 60-second whole-invocation pairs, stock cap | +11.54% H/s, −8.12% CPU-package J/hash | Manual SMT scaling evidence |
| EPYC 9355P, 280 → 240 W cap | Four fresh 120-second pairs, 64 fixed workers | −13.41% CPU-package J/hash, retaining 98.42% H/s | Manual power configuration; 66.56 H/s at measured 213.03 package W |
| EPYC 9355P, 2 MiB → 1 GiB pages | Four fresh 120-second pairs, same executable and 64 workers | +0.908% H/s, −0.330% CPU-package J/hash | Manual page configuration; measured package watts rose 0.575% |
| EPYC 9355P, 64 → 16 streaming ring blocks | Four fresh confirmation pairs at 64 workers | +0.367% backend H/s | Rejected: lower descriptive interval did not exceed the preregistered +1% gate |

Automatic productive rates are logged completed-hash lower bounds over service
start through shutdown. Explicit HugeTLB pools were prepared by the experiment
controllers. These observations do not establish untouched-host page provisioning
or fully converged worker counts. The Genoa allocation was a VM, not a full
socket; its physical memory channels and speed were unknown.

The native Turin host had 32 cores/64 threads, one socket/NUMA node, and 192 GiB
across twelve DDR5-4800 channels. Package energy excludes DRAM and wall losses.
Do not multiply these independent gains, mix durations, or call a configured cap
measured power. Four within-host pairs describe this session, not all CPUs with
the same model. Neither the manual power nor page comparison changed defaults.

## Evidence and graph inputs

The [machine-readable index](benchmark-index.json) links all JSON/CSV datasets,
record IDs, file hashes, metric scopes and adoption status. The index contains no
synthetic combined result or cross-hardware ranking. Revalidate it with:

```sh
/usr/bin/python3 experiments/amd-campaign-20261025/build_index.py --check
```

- [Package energy, workers and power cap](../cherry-epyc-energy-20261025/RESULTS.md): nine records.
- [Genoa automatic streaming eligibility](../genoa-streaming-20261025/RESULTS.md): four records.
- [Turin automatic challenger ordering](../amd-overnight-20261025/RESULTS.md): four records.
- [Streaming ring screen and rejected confirmation](../amd-ring-locality-20261025/RESULTS.md): ten records.
- [Explicit 1 GiB pages](../amd-page-size-20261025/RESULTS.md): two records.

Each dataset preserves frozen source/binary identity, settings, repetitions,
variability, raw numerators and observation hashes. Raw archives remain private
because they contain operational access details and inventory serials. Their
checksums and reproducible analysis are included in the individual reports.

The original Turin resource fixture failed to create reclaim pressure. Its
FAILED archive is preserved. Acceptance uses a separately repeated fixture with
the identical candidate binary, actual memory-high/PSI evidence, and restored
settings. It does not relabel the failed original. Kernel variants passed 21
full-size reference/dirty-arena/guard transitions; the 1 GiB verifier additionally
confirmed actual 1 GiB mapping. Ignored hardware tests are disclosed in each
integration report, not counted as native passes.

## Remaining work in this campaign

The [short-sample experiment](../amd-trial-windows-20261025/README.md) compares
unchanged kernel code with shorter, high-count tuning samples. Its full four-pair
cold/cache protocol and real-reclaim lifecycle must finish before adoption. Early
logs confirm that the candidate exercises 10-second samples and that its logged
decisions agree with raw counts; this is not a throughput conclusion.

The [combined configuration protocol](../amd-efficient-config-20261025/README.md)
compares 2 MiB/280 W against 1 GiB/240 W directly. It may start only after the full
previous experiment and evidence collection, with enough time for all four pairs
and cleanup. Until measured, its gain is unknown. If the complete protocol cannot
fit, preserve it as unmeasured rather than publishing a partial comparison.

## Separate software follow-up: cached startup

The earlier Turin cached logs provide a concrete follow-up beyond sample length.
In `joint-warm-candidate-1`, the miner starts from its remembered 48 workers,
probes 64 workers at 32.9 seconds, then restores streaming at 36.9 seconds. The
preserved cache contains a valid 48-worker streaming winner as well as the
64-worker winner; this was not an absent-cache case.

A source-level hypothesis is startup capacity accounting: `Controller::start`
initially calls `limits(&cfg, 0)` after backends are initialized, while subsequent
ticks include those backends' committed arena bytes. The resulting apparent
headroom increase can set `recovery_pending`, which defers a new kernel action
during the baseline phase. Verify that sequence with exact capacity/snapshot
tracing and a targeted regression test before changing it. An earlier restoration
is a proposed experiment, not another measured or shipping improvement. Keep
real resource recovery, allocation cooldowns, cache-context checks and kernel
acknowledgements intact; test both native restart behavior and constrained
recovery before adoption.

The [snapshot review](cached-startup-review.json) strengthens this hypothesis
across all eight earlier cached runs. At the first observed 47–48-worker
allocation, about 10 seconds after launch, even host headroom with no reserve
would cover only 42–43 workers if owned arenas were omitted. Using the recorded
child-cgroup limit and no reclaimable-cache credit gives an estimated 27–28
workers without those arenas, versus 75 with them; the later count-probe logs
also report 75 available, capped at 64. These calculations are not captured
startup-controller values: exact startup tracing is still needed. Reproduce the
review with `review_cached_startup.py COLLECTED OUTPUT.json`; it verifies the
original archive and preserves individual log/snapshot hashes. This diagnostic
does not add a hashrate record or change the original failed-fixture status.

## AMD GPU build preparation

Commit `4c2605e` fixes a reproduced AMD-only build failure: shared hash-finalization
helpers were incorrectly compiled only with NVIDIA or Metal enabled. The
`--no-default-features --features amd` build now passes 20 non-hardware AMD tests;
its three native GPU tests remain ignored. CPU-only and default builds also pass
`cargo check`. The [bring-up guide](../../docs/AMD_BRINGUP.md) now separates ABI
plausibility from wave32 eligibility, verifies exact hardware test selectors and
requires measured evidence rather than a bandwidth-derived hashrate prediction.
This is build preparation, not native HIP validation or a GPU performance point.

## Next hardware investigations

1. **Ryzen 7950X and 9950X under native Linux.** Current aggregate inventory makes
   these useful targets, but no qualifying longitudinal performance cohort exists
   yet. Inspect exact identity, DIMM speed/population and package-energy access.
   Test kernel eligibility, worker/SMT scaling and cold/cache behavior with paired
   controls; do not apply server results or the Turin short-sample rule by analogy.
2. **Fully populated faster-memory Turin.** This rental used DDR5-4800. AMD lists
   twelve channels and up to 6400 MT/s for the
   [EPYC 9355P](https://www.amd.com/en/products/processors/server/epyc/9005-series/amd-epyc-9355p.html).
   The memory-throughput counters and small kernel-buffer improvement make memory
   configuration a useful hypothesis to test. A theoretical bandwidth increase is
   not a measured hashrate gain; verify the server's actual supported DIMM layout
   and operating speed before paying for another comparison.
3. **Multiple NUMA nodes and complete-system efficiency.** The qualified host had
   one NUMA node, so it cannot prove placement wins on multi-node EPYC. Measure
   local versus cross-node allocation on suitable hardware, alongside whole-system
   power where available. Current package-only results cannot rank wall efficiency
   or total ownership cost against consumer systems.
4. **A qualified wave32 Radeon for native HIP bring-up.** The current backend needs
   wave32 and has not been validated by this campaign. RX 7900 XTX is present in
   AMD's [ROCm 7.2.1 Linux support matrix](https://rocm.docs.amd.com/projects/radeon-ryzen/en/latest/docs/compatibility/compatibilityrad/native_linux/native_linux_compatibility.html),
   which itself points newer SDK releases to unified documentation. Pin the exact
   OS/driver/HIP combination, verify `/dev/kfd`, device properties, full-size hash
   parity and cancellation before lane or power sweeps. Vendor support is not
   Seine validation. No compatible rental was established in this campaign; the
   AWS G4ad check found no offering in the configured region. No AMD GPU H/s or
   efficiency point is claimed.

## Rentals and accounting

The approved campaign limit is USD 25. Current reservations are USD 13.20 for
Cherry and USD 3.25 for AWS, USD 16.45 total. Account top-ups are credit funding,
not a second rental charge. Final Cherry consumption and cleanup are pending.

The AWS instance, root volume and security group were removed at **07:44 EDT**
after collection; its conservative duration-based estimate is USD 2.765 including
IPv4, storage and a transfer allowance, not a provider invoice. The active Cherry
rental must be terminated through the provider by **14:25 EDT**; shutting down
the OS alone does not stop billing. The full goal ends at **14:42:52 EDT**.

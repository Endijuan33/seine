# Large servers: NUMA, L3 groups and memory headroom

Seine is built to fill big machines. On Linux, the default `--cpu-affinity auto`
spreads workers across usable NUMA nodes and across L3 cache groups within each
node, using one thread per physical core before SMT siblings. This also spreads
workers on single-node servers with multiple L3 groups. On a rented EPYC 8434P,
L3 spreading improved a 24-worker benchmark by **16.7%**.

## How placement works

- Respects the process CPU and memory masks.
- Each worker pins itself before allocating its 2 GiB arena, so Linux's local
  allocation policy places memory near that worker. Existing memory policies still
  apply; placement does not enforce memory binding or guarantee local huge-page
  availability.
- Missing or inconsistent cache data keeps NUMA-only placement. A single node with
  one cache group (or unavailable cache data), and incomplete NUMA/core topology,
  keep OS CPU order.

Opt-outs:

- `--cpu-affinity os`: pin in OS CPU order without topology reordering.
- `--cpu-affinity off`: let the scheduler place workers.

Compare `auto` against `os` with the same thread count and verified page backing
before attributing a hashrate change to placement. HugeTLB pools need enough pages
on the nodes hosting workers; a sufficient global count alone does not guarantee
local placement. See [Linux HugePages](linux-hugepages.md).

## Memory headroom

Automatic thread sizing and autotuning subtract 5% of effective total RAM from the
available budget, in addition to any GPU/Metal reservation. Adjust this with
`--cpu-memory-reserve-percent <0..99>`; `0` restores the previous budget.

Explicit `--threads` / `--cpu-threads-per-instance` counts bypass this reserve
(unless thread autotuning is explicitly requested) but keep the existing memory
checks. The minimum of one worker per CPU instance still applies.

Automatic CPU workers also release memory when headroom falls and return once
conditions stabilise, including pausing every CPU worker if no complete arena
fits.

## Kernel selection on EPYC

Automatic tuning compares CPU kernels (AVX2, AVX-512 and, on validated EPYC
processors, a streaming-store kernel) while mining, and only switches after two
confirmed gains at the current worker count and page backing. No configuration is
needed. Results for each EPYC generation are on the [performance page](../PERFORMANCE.md).

Autotune caches are invalidated when the allowed CPU set or detected NUMA/L3 layout
changes, including changes that leave the CPU count unchanged.

Multi-socket NUMA hosts have not been benchmarked yet. Method and limitations are
in the [CPU optimization log](../../CPU_OPTIMIZATION_LOG.md).

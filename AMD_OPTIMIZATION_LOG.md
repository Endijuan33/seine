# AMD Optimization Log

This log tracks HIP backend (feature `amd`) tuning attempts and measured outcomes,
mirroring `CPU_OPTIMIZATION_LOG.md`. The bring-up gates and first hardware results
are in `docs/AMD_BRINGUP.md`.

## 2026-10-28 MI325X (CDNA3, gfx942, wave64): lane memory, barriers, prefetch

DigitalOcean `gpu-mi325x1-256gb` (nyc2, ROCm 6.4.1, 261,824 MiB VRAM visible),
`--bench-kind backend --bench-secs 60 --bench-rounds 2 --bench-warmup-rounds 1`,
source `c2f2ff2` plus the candidate. Every candidate passed the GPU-vs-CPU
differential (`gpu_solution_target_bracket_matches_cpu_reference`) before it was
benchmarked. Evidence: `benchmark-evidence/hashrate-20261026/amd-mi325x-tune/`
(workspace). `rocm-smi` package power is the average over the sweep.

| Hypothesis | Lanes | Δ | Verdict |
|---|---|---|---|
| 60 s rounds instead of 20 s remove round-end losses that made 78 lanes look flat | 64 / 78 | 20.22 / 22.03 H/s (20 s rounds: 18.47 / 18.44) | measurement fix: lanes still scale linearly at 78 |
| Allocate lane memory in 32-lane chunks (one hipMalloc of >~157 GiB fails on this card, which capped the probe at 78 lanes with 100 GiB free) | 64 / 96 / 125 | 19.73 / 27.01 / **35.34** vs 22.03 at the old 78-lane cap (**+60%**) | adopted (`LANE_CHUNK_LANES = 32`, device-side chunk table) |
| Wave-scoped barrier (`__builtin_amdgcn_fence` wavefront + `wave_barrier`) instead of `__syncthreads()`: the cooperative group is one wavefront, and the block barrier waited on every outstanding global store | 64 / 125 | 21.31 / **37.39** vs 19.73 / 35.34 (+8% / +5.8%); the landed default re-run at 125 lanes gave 35.68 (+1% over the chunked block-sync run) | adopted as default (never below block sync across three runs; run-to-run spread on this card is about 3%); `SEINE_AMD_BLOCK_SYNC=1` restores `__syncthreads()` |
| Load the next data-independent block's reference one block ahead (register prefetch into a preloaded `compress` variant) | 64 / 125 | 18.07 / 32.91 (block sync) and 19.07 / 33.16 (wave sync) vs 19.73 / 35.34 and 21.31 / 37.39 (−7% to −11%) | rejected (bit-exact but slower: extra live registers and loads compete with the current block) |

Power during the 125-lane runs was 316-348 W average (max 344-365 W), so the
adopted pair moves the card from 0.07 to about 0.10-0.11 H/s per watt. Still untested:
a wave64-native kernel (one lane per 64-thread wavefront), launch depth, and any
RDNA (wave32) card, which is the fleet target.

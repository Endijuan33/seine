# README hardware benchmarks, 2026-10-25

Whole-VM CPU hashrates for the README, one AWS us-east-1 instance size per CPU
generation (all 32 vCPU) so the bars are comparable. Miner under test:
**seine v0.2.22** (`6e5411768d5d`, release assets checksum-verified with
`gh release download`), CPU backend only, current PoW (Argon2id v1.3, 2 GiB, one
pass, one lane per hash). On Graviton, the upcoming runtime-guarded SHA3+prefetch
NEON kernel from draft PR #44 (`f20e7bb`) is measured alongside the release NEON.

<!-- RESULTS -->

## Protocol

Fixed in `hostrun.py` before any launch; identical on every host.

1. Fresh instance (Ubuntu 24.04, 16 GiB gp3), nothing else running. The guest
   schedules `shutdown -h +40` as its first user-data action and EC2
   shutdown behaviour is `terminate`; both are verified before work begins.
2. Every bundle file is SHA256-checked on the guest before and after timing.
3. Full-size (2 GiB) reference parity against RustCrypto `argon2` for every
   kernel that gets timed (`verify_cpu_kernels`). A kernel is timed only if its
   covering verifier passed it.
4. Worker counts: 16, 24 and the largest count *W* for which
   `ceil(1.05 * W * 1024)` 2 MiB HugeTLB pages plus 5 GiB of ordinary RAM fit in
   `MemAvailable` (capped at 32). The pool starts empty, is reserved once for the
   largest count (+5%), verified (`HugePages_Free`), and restored to 0 at the end.
5. Each invocation: `seine --bench --bench-kind backend --backend cpu --threads N
   --cpu-page-mode large --cpu-affinity auto --bench-cpu-kernel K
   --disable-cpu-autotune-threads --bench-secs 20 --bench-rounds 2
   --bench-warmup-rounds 1 --bench-output report.json` with a fresh `--data-dir`
   and `SEINE_*`/`BNMINER_*` variables scrubbed from the environment.
6. The kernel x worker grid is run once, then again in exactly reversed order.
7. The best grid cell is then confirmed with best / runner-up / runner-up / best
   invocations, the runner-up being the second-best kernel at the same worker
   count (on hosts with only one kernel: the same kernel at the second-best worker count).
8. An invocation is accepted only if its report proves the requested worker
   count, kernel (`cpu_kernel == cpu_kernel_requested`), `large` page mode,
   `auto` affinity, autotune off, CPU-only backend, native runtime, current PoW,
   full explicit-HugeTLB backing of `workers x 2 GiB` with zero regular/heap/THP
   workers, zero allocation/dropped-event/assignment/control failures,
   `hashes == counted + late`, all lanes active, plus no swap-in/out or OOM kill,
   steal below 2%, and no change to the HugeTLB pool during the run.
9. **Metric:** completed H/s = sum(completed hashes) / sum(actual elapsed seconds
   *including* the round-end fence) over the two measured rounds. The report's
   `avg_hps` counts only hashes inside the configured window and is not used.
   The headline per host is the mean over all accepted invocations of the
   best cell (2 grid + 2 confirmation = n 4).

## Reproduction

- `campaign.py run <label>`: launch, verify timer, upload, run `hostrun.py`,
  collect + verify, terminate + verify root-volume deletion (in `finally`).
- `run_queue.py <labels...>`: sequential priority queue with a budget stop.
- `cloud.py`: lease ledger, pricing, quota checks, tag-verified termination,
  audit and final cleanup (adapted from `cpu-campaign-20261024/cloud.py`).
- `summarize.py`: builds `summary.json` from verified evidence and the ledger.

Raw evidence (reports, logs, hardware dumps, archives, ledger) is kept in the
ignored `data/readme-hardware-20261025/`.

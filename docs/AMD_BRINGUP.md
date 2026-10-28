# AMD (HIP/ROCm) Backend Bring-Up Plan

Target: **RX 7900 XTX (gfx1100, RDNA3, 24 GB, wave32)** on a rented Linux
ROCm box. The backend is scaffolded to compile and unit-test on any x86_64
Linux host with no AMD hardware or ROCm installation; everything below is the
first-hardware iteration checklist.

## Status / scope

- `--features amd` (OFF by default). All HIP/hipRTC FFI is hand-declared and
  dlopen'd at runtime (`libamdhip64.so`, `libhiprtc.so`) — no ROCm headers or
  toolkit needed at build time.
- Kernel: HIP port of the CUDA Argon2id fill/seed/eval kernels
  (`src/backend/amd/amd_kernel.hip`), keeping A29 full-warp cooperative
  mapping, A77/A78 unrolled loads/stores, A81 closed-form G-round indexing,
  and A82 shuffle fusion. The A73 dead-code-prefetch codegen hack and its
  first-8-threads guard are NVIDIA/PTXAS-specific and were omitted.
- Wavefront strategy: the cooperative group is 32 threads per lane and the
  thread block is one wavefront. hipRTC compiles with
  `--offload-arch=<gcnArchName>` plus `-mno-wavefrontsize64` on wave32 devices
  (RDNA gfx10-gfx12, one lane per wavefront) or `-mwavefrontsize64` on wave64
  devices (CDNA gfx9 such as MI300X, two lanes per wavefront via
  `SEINE_WARPS_PER_BLOCK=2`; every `__shfl` uses a width of 32 so shuffles stay
  inside each lane's half), with a retry without the flag if a hipRTC build
  rejects it. Any other reported `warpSize` is refused at startup. The
  cooperative barrier is wave-scoped (`__builtin_amdgcn_fence` at wavefront
  scope plus `__builtin_amdgcn_wave_barrier()`), with `SEINE_AMD_BLOCK_SYNC=1`
  restoring `__syncthreads()`. Lane memory is allocated in 32-lane chunks
  (`LANE_CHUNK_LANES`) addressed through a device-side pointer table. Both
  passed the differential on an MI325X on 2026-10-28.
- Autotune: stub. Lanes derive from free VRAM with allocation/probe backoff
  (11 × 2 GiB lanes expected on 24 GB); launch depth fixed at 1. Regcap/depth
  sweeps and any AMD-specific scheduling work come after this checklist
  passes.

## First hardware results (2026-10-28, AMD Instinct MI325X)

Rental: DigitalOcean `gpu-mi325x1-256gb` in nyc2 ($3.80/h, lease `amd1` via
`scripts/cohort/do.py`), image `digitaloceanai-rocmsoftware` (Ubuntu 24.04,
ROCm 6.4.1, HIP 6.4.43482), host 20 vCPU EPYC 9575F. Device: `AMD Instinct
Mi325X VF`, `gfx942:sramecc+:xnack-`, wavefront 64, 261,824 MiB VRAM visible.
Source `c2f2ff2` (includes the wave64 port, 92a6cce), rustc 1.93.0, run by
`scripts/cohort/remote_amd_bringup.sh`; evidence in
`benchmark-evidence/hashrate-20261026/amd-mi325x/results` (not in git).

Gates, in order:

1. (a) unit tests: 21 passed (3 hardware tests ignored, run next).
2. (a) `hip_device_props_parse_sanely`: the legacy `hipDeviceProp_t` layout holds
   on ROCm 6.4 (wavefront 64, `gfx942`, memory non-zero).
3. (a) `hiprtc_kernel_compiles_for_device_arch`: 1.9 s with
   `--offload-arch=gfx942:sramecc+:xnack- -mwavefrontsize64
   -DSEINE_WARPS_PER_BLOCK=2U`; no fallback needed.
4. (b) `gpu_solution_target_bracket_matches_cpu_reference`: passed (163 s, debug
   build), so seed, 2 GiB fill, last-block export and target evaluation are
   bit-exact with two lanes per wave64 wavefront.

Benchmarks (release build, `--bench-secs 20 --bench-rounds 3`, backend kind with
one warmup round):

| Lanes | Kind | avg H/s | median | min | max | late % |
|---|---|---|---|---|---|---|
| 1 | kernel | 0.315 | 0.315 | 0.312 | 0.319 | 0 |
| 1 | kernel-effective | 0.350 | 0.350 | 0.349 | 0.350 | 0 |
| 1 | backend | 0.319 | 0.326 | 0.294 | 0.336 | 0 |
| 2 | backend | 0.573 | 0.578 | 0.554 | 0.587 | 0 |
| 4 | backend | 1.18 | 1.18 | 1.16 | 1.19 | 0 |
| 8 | backend | 2.29 | 2.27 | 2.26 | 2.36 | 0 |
| 16 | backend | 4.74 | 4.73 | 4.71 | 4.78 | 0 |
| 32 | backend | 9.36 | 9.42 | 9.19 | 9.48 | 0 |
| 64 | backend | 18.47 | 19.06 | 17.25 | 19.09 | 0 |
| 78 (96 requested) | backend | 18.44 | 18.07 | 17.86 | 19.40 | 16.7 |

Reading: one lane fills 2 GiB in about 3.2 s (0.29-0.32 H/s per lane) and the
rate is linear to 64 lanes, then flat at 78, so this part tops out near 18.5 H/s
on the current kernel. That is roughly 100 GB/s of memory traffic against about
6 TB/s of HBM3E, so the chain is latency-bound per lane exactly as on NVIDIA;
two lanes per wave64 wavefront leave almost every CU idle. The 16.7 % late
hashes at 78 lanes are round-end preemption of 3 s hashes in a 20 s window
(fences completed in 0.1-2.3 s, so the cancel flag is observed). The engine
sized 78 lanes where 261,824 MiB minus the reserve would hold about 125;
confirm what `hipMemGetInfo` reports as free on the VF before trusting the
VRAM-derived lane cap on big cards.

(e) Pool smoke test: `--backend amd --amd-max-lanes 64 --pool-url
stratum+tcp://bntpool.com:3333` for 400 s mined at 19.9 H/s across five job
changes; 3 shares submitted, 3 accepted, none stale, so bntpool validates
hashes from this backend. A separate 60 s x 2 round run at 64 lanes gave
19.92 H/s (19.73-20.11). `rocm-smi` package power during the smoke test averaged
273 W over 45 samples (125 W idle at the start, 290-298 W mining), about
0.07 H/s per watt, below a Zen 5 desktop CPU on the same hash.

Same-day tuning on the same card (details and rejected variants in
`AMD_OPTIMIZATION_LOG.md`, 60 s rounds): the 78-lane cap was a per-allocation
limit (one `hipMalloc` above about 157 GiB fails while 261 GiB is free), so lane
memory is now allocated in 32-lane chunks behind a device-side pointer table,
which reaches 125 lanes and 35.3 H/s (+60% over 22.0 at 78 lanes); the
cooperative barrier is now wave-scoped instead of `__syncthreads()`, which adds
1-8% depending on the run (35.7-37.4 H/s at 125 lanes; the card's run-to-run
spread is about 3%); a one-block-ahead reference prefetch for the
data-independent half was bit-exact but 7-11% slower and was dropped. Package
power stayed near 320 W, so the card now does about 0.11 H/s per watt.

The kernel compiles for every RDNA target with the wave32 options the engine
uses (`hipcc --offload-arch=<arch> -mno-wavefrontsize64 -DSEINE_WAVE_SYNC=1
-DSEINE_WARPS_PER_BLOCK=1U`, ROCm 6.4.1 clang 19): gfx1030, gfx1100, gfx1101,
gfx1102, gfx1151 (Strix Halo) and gfx1200/gfx1201 all build cleanly (2026-10-28,
static check only; no RDNA device has executed it).

Follow-ups, in order of expected value: a run on an RDNA wave32 card, which is
the fleet target and has not executed the kernel or the wave-scoped barrier yet;
a wave64-native kernel (one lane per 64-thread wavefront, or four lanes per wave
with 16-wide shuffles) and launch depth > 1 for CDNA. Do not quote these numbers as AMD GPU
performance for the README until an RDNA card has been measured.

## Qualify the rental and software stack

The first native HIP result exists as of 2026-10-28 (see "First hardware
results" below): an AMD Instinct MI325X (CDNA3, gfx942, wave64) on DigitalOcean
passed every gate in (a) and (b) and benchmarked 18.5 H/s at 64 lanes. No RDNA
(wave32) card has run yet; the RX 7900 XTX target below is still unmeasured.
Confirm the exact GPU, native device access and complete OS/kernel/driver/HIP
combination before reserving a machine. A provider's "AMD" label or installed
shared libraries do not prove the device is usable by this backend.

- Pin a combination listed in AMD's
  [Linux support matrix](https://rocm.docs.amd.com/projects/radeon-ryzen/en/latest/docs/compatibility/compatibilityrad/native_linux/native_linux_compatibility.html).
  As checked on 2026-10-25, that page lists RX 7900 XTX for ROCm 7.2.1 and
  explains that releases from Core SDK 7.13.0 use unified documentation.
  Select the matching version's matrix; do not assume "6.x or newer" is a
  qualified stack. Vendor support does not establish Seine correctness.
- The host must load `amdgpu`; verify the container/VM exposes usable
  `/dev/kfd` and `/dev/dri`. Record GPU name, PCI ID, `gcnArchName`, HIP-reported
  wavefront size, VRAM, driver/runtime/hipRTC versions, OS and kernel.
- Install build deps: `rustup` (repo pins 1.93.0 via rust-toolchain.toml),
  `build-essential`, `git`.
- If the loader cannot find the libraries, add ROCm to the search path:
  `export LD_LIBRARY_PATH=/opt/rocm/lib:$LD_LIBRARY_PATH`.

For the proposed RX 7900 XTX target, confirm `gfx1100`, approximately 24 GB
VRAM, and HIP-reported wavefront size **32**. Record actual available memory;
the number of usable 2 GiB arenas can be lower than the nominal VRAM capacity.

## Verification sequence

Run steps in order; each gate must pass before the next is meaningful.

### (a) Unit tests, including the ignored hardware tests

```bash
cargo +1.93.0 test --locked --no-default-features --features amd backend::amd:: -- --test-threads=1
cargo +1.93.0 test --locked --no-default-features --features amd backend::amd::engine::tests::hip_device_props_parse_sanely -- --ignored --exact --nocapture
cargo +1.93.0 test --locked --no-default-features --features amd backend::amd::engine::tests::hiprtc_kernel_compiles_for_device_arch -- --ignored --exact --nocapture
```

The checks, in dependency order (run each hardware test separately and stop on
failure; disabling default features avoids unrelated NVIDIA hardware tests):

The compile and differential hardware tests currently use the first enumerated
device. Confirm that it is the intended wave32 GPU before running them; checking
another device's properties is insufficient.

1. `backend::amd::hip_ffi::…loader_reports_missing_rocm…` (not ignored; on
   a pass alone does not prove ROCm loaded, because the test also accepts the
   expected missing-library error. The subsequent device query must succeed).
2. `backend::amd::engine::tests::hip_device_props_parse_sanely` — validates
   the hand-declared legacy (`R0000`) `hipDeviceProp_t` ABI: wavefront size
   must read a plausible 32 or 64 and `gcnArchName` must start with `gfx`.
   Separately require **32 for the selected mining device**; a plausible wave64
   device can pass the ABI test but is unsupported by this backend. **If the ABI test fails, the
   struct layout in `src/backend/amd/hip_ffi.rs` is wrong for the installed
   ROCm — fix offsets before anything else; every later step depends on it.**
3. `backend::amd::engine::tests::hiprtc_kernel_compiles_for_device_arch` —
   first real hipRTC compile of the ported kernel. Compile-log errors surface
   in the test failure message.
4. `backend::amd::tests::gpu_solution_target_bracket_matches_cpu_reference`

### (b) Small-scale correctness differential vs the CPU backend

This mirrors the NVIDIA backend's ignored GPU-vs-CPU differential
(`gpu_solution_target_bracket_matches_cpu_reference` in
`src/backend/nvidia.rs`, which brackets each nonce with target == CPU hash
(must solve) and target == hash − 1 (must not solve)):

```bash
cargo +1.93.0 test --locked --no-default-features --features amd backend::amd::tests::gpu_solution_target_bracket_matches_cpu_reference -- --ignored --exact --nocapture
```

A pass proves the full pipeline — seed blocks, 2 GiB Argon2id lane fill,
last-block export, Blake2b target evaluation — is bit-exact against
`blocknet-pow-kernel` for several nonces. If hashes mismatch, bisect with the
seed kernel first (dump `seed_blocks` DtoH and compare against the CPU H0
path) before suspecting the compression loop.

### (c) Single-lane kernel benchmark

```bash
cargo +1.93.0 run --locked --release --no-default-features --features amd -- --bench --bench-kind kernel \
  --backend amd --amd-max-lanes 1 --bench-secs 20 --bench-rounds 3 \
  --bench-output bench-amd-1lane.json
```

Record the measured rate without an expected H/s target. Also run
`--bench-kind kernel-effective` to measure the benchmark's effective wall-time
scope; keep that result separate from kernel timing.

### (d) Lane scaling on 24 GB

```bash
for lanes in 1 2 4 8 11; do
  cargo +1.93.0 run --locked --release --no-default-features --features amd -- --bench --bench-kind backend \
    --backend amd --amd-max-lanes $lanes --bench-secs 20 --bench-rounds 3 \
    --bench-warmup-rounds 1 --bench-output bench-amd-${lanes}lane.json
done
```

- 11 lanes is the expected maximum: 24 576 MiB total − 384 MiB reserve →
  ~23.6 GiB budget / 2 GiB per lane (the engine backs off automatically if
  the display or other processes hold VRAM).
- Treat this ascending sweep as screening only. Confirm any chosen lane count
  with fresh interleaved paired runs, recording actual allocated lanes,
  warmup, duration, completed hashes, variability and thermal conditions.
  Do not infer AMD hashrate or scaling from NVIDIA measurements or theoretical
  memory bandwidth. Keep failed allocations and negative results.
- Check `late_hash_pct` in the report: with depth 1 and cancel checkpoints
  every 64 blocks, round-end preemption should be comparable to the NVIDIA
  backend's; a large value suggests the device-side cancel flag isn't being
  observed (verify the `volatile` load in the kernel actually bypasses
  caches on RDNA3 — see "Known risks" below).

### (e) Mining smoke test (pool)

```bash
cargo +1.93.0 run --locked --release --no-default-features --features amd -- --backend amd --address YOUR_BLOCKNET_ADDRESS
```

Confirm: backend line shows `amd` with its actual allocated lane count; cancel/fence startup probe
passes (the backend advertises cooperative deadlines, so it is NOT gated
behind `--allow-best-effort-deadlines`); shares from backend `amd` are
accepted by the pool.

## Evidence and power scope

Preserve source revision, binary and kernel-source checksums, compiler options,
commands, full JSON reports, correctness/cancel logs and the exact device/software
identity. Export completed observations with their benchmark kind, numerator,
denominator, repetitions and uncertainty. A screen or ignored test is not native
validation, and no result from this guide should enter README/website graphs
until its evidence is collected and reviewed.

For efficiency, record the actual sensor and measurement interval. A configured
board power limit is not measured watts. If only sampled board power is
available, disclose the sampling/integration method and missing samples; do not
present it as a hardware energy counter. Keep GPU-board, CPU-package and wall
power distinct, and leave unavailable measurements null. Confirm any power
setting with a new paired hashrate/energy comparison before recommending it.

## Known risks to verify on hardware (in priority order)

1. **`hipDeviceProp_t` legacy ABI layout** — covered by step (a)/test 2.
2. **hipRTC option acceptance** — `--offload-arch` with a full target id
   (`gfx1100:sramecc-:xnack-`) and `-mno-wavefrontsize64`; the engine
   already retries without the wave flag and surfaces the compile log.
3. **Cancel-flag visibility during a running kernel** — the interrupt
   controller writes via a non-blocking stream while the fill kernel polls a
   `volatile` global pointer. Verify with a mid-round cancel (`Ctrl-C`
   during mining or the step-(d) round fences) that fences complete in
   ≪ 1 s rather than after a full launch.
4. **`__shfl` on 64-bit values under wave32** — exercised by the (b)
   differential; a wrong-width shuffle shows up as hash mismatches.
5. **Multi-device** — a bare `--backend amd` on a 2-GPU rental now
   auto-expands to one instance per wave32 device (`--amd-devices 0,1`
   pins the same topology explicitly); device binding is per worker
   thread (`bind_thread()`), which this verifies.

## After bring-up (deferred tuning surface)

Only after (a)–(e) pass, in an AMD_OPTIMIZATION_LOG.md mirroring the NVIDIA
process: launch-depth sweep (`hashes_per_launch_per_lane`), cancel-check
cadence, waves-per-CU occupancy experiments, `s_setprio`/scheduling hints,
`__builtin_amdgcn` barrier relaxations replacing `__syncthreads()`,
wave64-native kernel for CDNA, and a persisted autotune cache keyed like the
NVIDIA one (device, arch, hiprtc version, kernel fingerprint).

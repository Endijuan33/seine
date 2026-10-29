# Faraday miner readiness

Source: the maintainer packet archived in the sibling pool's `docs/faraday/`
(2026-10-16) and the fork's core spec §6 (BLAKE2b decided 2026-10-26).
Activation: height 103,680. **This build is not a Faraday miner.** The BLAKE2b
kernels exist but mining still refuses fork-height work. Official PoW vectors,
the final header and PQ address rules are pending. See the pool's
`docs/FARADAY_UPSTREAM_QUESTIONS.md` for the questions prepared for upstream.

## Implemented

- BLAKE2b-512 PoW (2026-10-28): `pow-spec` has the reference hash
  (`blake2b_pow_hash`, mainnet/testnet personalization) and
  `PowAlgorithm::for_height`. `pow-kernel` has `Blake2bPowMidstate`, which
  caches the template prefix so the 156-byte header costs one compression per
  nonce (about 8 MH/s per desktop core, portable build). The NVIDIA engine
  (`src/backend/nvidia/blake2b.rs`) matches the CPU kernel's hits and measured
  15 GH/s on an RTX 5090. Neither is wired into mining yet. Vectors come from
  Python's `hashlib.blake2b` until upstream publishes official ones.
- `pow-spec` owns an interim height guard. Pool and direct modes refuse work
  from 103,680 onward through backend teardown. Direct mode also refuses missing
  height. The guard follows template height, not time; it is not a BLAKE2b switch.
- Runtime decoding and NVIDIA/AMD/Metal buffers use the actual checked header
  length (8..4,096 bytes), replacing fixed 92-byte enforcement and Metal's
  truncating copy. The cap is a resource bound, not a final consensus layout.
- The CPU Argon2 reference agrees with the library for 156/512-byte headers at
  reduced test memory. This establishes variable-size support, not Faraday PoW.
- Encoded addresses are bounded at 2,048 bytes. PQ-sized addresses return an
  explicit unsupported-codec error. A complete long login, 156-byte job header,
  difficulty above 2^53 and large nonce range round-trip exactly in Rust JSON.
- Finished nonce windows are retired even when no solution was found. CPU idle
  telemetry includes solutions still in its event forwarder; runtime event queues
  drain before completion. Refill uses a fresh login on the same configured pool,
  waits for queued/in-flight submit acknowledgements, and is tied to the old job
  ID so a delayed request cannot interrupt a newer job. This uses the existing
  protocol; there is no new mandatory pool capability. Reconnect latency remains
  a cost to measure when sizing GPU windows.
- Metal active-lane telemetry now clears at completion/cancellation, allowing the
  runtime to recognize idle work. This needs macOS/device verification.
- Network estimates use 300/600 seconds by era and exclude pre-fork intervals
  after the difficulty reset. Existing UI units already include KH/s/MH/s/GH/s.
- Independent target vectors cover non-power-of-two difficulty, 1e12, values
  above JavaScript's exact range and `u64::MAX`. The existing exact division
  differs from the packet's power-of-two claim; upstream clarification is pending.

Constants currently apply to every connection. Obtain testnet/network activation
rules before using Faraday testnet. Legacy benchmark fixtures remain 92 bytes.

## Remaining release work

| Area | Work |
| --- | --- |
| CPU/GPU | Replace the interim stop with height-based dispatch to the BLAKE2b CPU and NVIDIA paths. Add algorithm capabilities to assignments, separate tuning/state by algorithm, release old Argon2 arenas, and implement or explicitly reject Faraday on AMD/Metal. Swap in official vectors. |
| Nonces | Pool windows are now reserved durably by header; miner refill drains solutions/submits before reconnecting. Default remains 1e6. Benchmark larger windows, refill overhead, cancellation, hot backend changes and mixed devices at actual GPU rates. |
| PQ identity | Implement final codec/network/key rules, real end-to-end login fixtures and developer-fee PQ identity. Validate CLI/TUI/control API handling and full address copying. A new login alone does not migrate old pool balances. |
| Submission | Verify compact submit, renewal, full-block fallback, SSE, same-height refresh and expiry against final tree-root/proof contracts. Preserve explicit daemon/pool targets. |
| API/UI | Version new algorithm/state fields and exact numeric representations where consumers need values above 2^53. Show upgrade-required state clearly. |

## Acceptance gates

1. Pin final upstream spec/build, vectors and testnet activation rules.
2. CPU and each supported GPU backend match hash/target/header vectors before
   optimization, including nonce extremes, target equality and tree-root mutation.
3. Pool and direct modes pass legacy -> fork -> reorg -> fork, stale/late solution,
   memory release, shutdown, unsupported-backend and nonce exhaustion/refill tests.
4. PQ login and developer-fee sessions work against the upgraded pool/daemon.
5. Benchmark throughput, share rates and cancellation under `gpuq` admission.
6. Release before height 103,580 in coordination with payout hold and migration;
   remove the interim stop only as part of the verified Faraday implementation.

## Verification

Using Rust 1.93.0 and `--locked`: 69 mining tests, 51 pool/protocol tests and
18 CPU lifecycle/event tests passed, including delayed solutions, delayed ACKs,
new-job cancellation of refill, empty-solution windows and the maximum nonce.
Earlier checks also passed three address tests, six target/type tests, two
pow-spec tests and the variable-header CPU reference test. NVIDIA and AMD code compile. GPU kernels were not
executed; Metal needs a macOS build/device check. No GPU workload or deployment
was launched. Existing dead-code warnings remain (including `pidfile_path`). Final PoW/PQ
vectors and Faraday testnet interoperability remain release blockers.

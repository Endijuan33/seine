# Streaming-ring candidate handoff

`streaming-candidate.patch` is the exact kernel-only experimental patch against
785a06f. It does not include the shared arena-audit hook used to validate page
placement. The measured executable is identified in `frozen-kernels.json` and in
each paired protocol. `streaming_ring.rs` and `prepare_kernel.py` explain and
regenerate the implementation. Production source files remain unchanged.

The prototype uses explicit benchmark selection of `avx512-256`; it replaces
that implementation only in its isolated executable. Integration should give
the candidate an unambiguous identity and an experimental feature gate so the
original 256-bit path remains independently measurable. It requires AVX2,
AVX-512F and AVX-512VL. Do not enable the path on ISA support alone before
correctness and performance validation on the intended architecture.

The useful outcome is at 24 workers on the measured EPYC 9R45 guest. It regresses
at one worker and is flat at eight. Automatic kernel/worker tuning belongs to
the architecture-dispatch owner; there is no supported universal worker cutoff
or blanket replacement. Any cache fingerprint must distinguish implementations.

Memory and correctness requirements:

- Preserve the entire consensus 2 GiB arena, one lane and one pass. The 64 KiB
  ring is additional cached storage, not a time-memory tradeoff.
- For `cur - reference < 64`, read the ring. At exactly 64, use the arena: the
  ring slot aliases the destination. References are strictly behind the output
  only because this is the fixed one-pass, one-lane path.
- Fence previous NT batches before an older arena reference can be read, and
  fence the final batch before finalization consumes the final arena block.
  Keep complete, aligned cache-line destination writes.
- The prototype allocates the ring per hash. Hoisting it into persistent worker
  state is a separate change requiring dirty-reuse, kernel-switch and performance
  validation; do not silently substitute that change for the measured binary.
- Existing validation includes all supported kernel outputs against RustCrypto
  at 2 GiB; three additional full-size streaming inputs with repeated dirty reuse;
  small ring/slice/rounding boundaries; and sentinel blocks around the arena.
  Production integration should additionally exercise both directions of live
  kernel switching on one persistent arena, nonce continuity, cancellation and
  normal backend error/accounting behavior.

Long backend confirmation retains counted hashes, late hashes, elapsed time,
the end fence, allocation backing and per-run health counters in the raw reports.
The reported rates include completed work and its elapsed time. Evidence is
within one guest/host; it does not establish a general percentage for all EPYC
servers. CPU PMCs can support the traffic hypothesis, but the guest has no memory
controller or L3 PMU for direct socket bandwidth measurements.

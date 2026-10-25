# NVIDIA setup guide — draft evidence

Prepared October 25, 2026, 5:50 PM EDT. Source checkout: `seine@77a3a97`;
released binary inspected: v0.2.26 (`cc4af9e`). No mining or benchmarks were run.

## Reader job and overlap

Help a Windows or Linux x86_64 user prepare the NVIDIA runtime, choose CPU-plus-GPU
or GPU-only mining, select a device, and verify the NVIDIA backend's activity.
The nearest owner is `mine-blocknet-on-windows`: that article owns ZIP verification
and first-run Windows setup. Link to it instead of repeating its checksum tutorial.
`seine-dashboard-explained` owns the complete panel tour; the pool tracking guide
owns zero pool hashrate. This draft adds NVIDIA-specific selection and diagnosis.

This is preparatory work authorized while Search Console processes its first data.
It does not change the measurement-baseline decision or claim proven search demand.
The backlog's per-card performance comparison is not complete. No new hashrate,
CPU-plus-GPU uplift, power, profitability, or card compatibility table is asserted.

## Source ledger

| Claim | Evidence |
|---|---|
| Windows/Linux x86_64 release names and NVIDIA package requirements | `README.md:39–47`; `docs/guides/nvidia.md:20–33` |
| Packaged Windows DLLs, CUDA 12.8.61 runtime | `.github/workflows/release.yml:83–103`; `Cargo.toml:28` pins the CUDA 12.8 loader |
| Default CPU plus detected NVIDIA selection; explicit backend lists | `src/config.rs:373–386,1475–1507`; `docs/guides/nvidia.md:3–16` |
| Comma-separated backend/device flags | `src/config.rs:373–386`; released v0.2.26 `--help`, lines 54–64 |
| NVIDIA autotune cache filename and data directory | `src/config.rs:130,370,513–515,1155–1158`; `docs/guides/nvidia.md:35–41` |
| GPU lane arena, available-memory budget and allocation checks | `src/backend/nvidia.rs:530–585,2874–2882,2929–2940,3350–3358`; existing fact-sheet current Argon2id memory parameter |
| Device rows, startup progress, per-backend share messages | Published `seine-dashboard-explained`, especially Devices and log sections; existing fact-sheet dashboard evidence |
| Driver download | https://www.nvidia.com/en-us/drivers/ (checked October 25) |
| NVRTC runtime compilation and matching builtins library | https://docs.nvidia.com/cuda/nvrtc/ (checked October 25); platform-specific packaging claims come from the release workflow, not the latest CUDA docs |
| Article image | Existing real download-page capture `/assets/blog/windows-download.png`; caption identifies the Windows archive table, not an NVIDIA mining session |

## Verification and publication work

- Executed the existing released Linux binary with `--version` and `--help` only.
  It reported `seine v0.2.26`; backend/device/cache options match the text.
  Commands have been checked against the CLI definition and release help, not
  executed as mining sessions on Windows or Linux NVIDIA hardware.
- Verified both the Linux x86_64 and Windows x86_64 archives against the saved
  published SHA256SUMS.txt. The inspected Linux CLI matches its archived binary;
  the Windows ZIP contains the executable and both documented NVRTC DLLs.
- Strict editorial check passed. Wording overlap was at most 0.1% with an existing
  post; the distinct-intent assessment above is still necessary.
- Keep `draft: true`; no keyword-map ownership change or translation placeholders.
- Before publication, capture an actual NVIDIA device row and accepted share from
  a controlled session without exposing wallet details. Do not reuse the existing
  Apple M4 Max screenshot as GPU evidence.
- Complete repeatable per-card and CPU-plus-GPU measurements if the article is to
  answer the backlog's performance question. Use the repository benchmark process
  and GPU admission; do not turn live telemetry into benchmark proof.
- Then settle the English content, translate into the other 19 languages, preview
  the article/social card, and publish with its keyword-map row. Recheck runtime
  details against the release being recommended at that time.

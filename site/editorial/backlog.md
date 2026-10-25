# Backlog

Researched topics not yet written, in priority order. Batch 1 (2026-10-25) took the hardware,
Mac, Argon2id, what-is-Blocknet, Blocknet ID and privacy topics; pool-side topics went to the
bntpool.com blog. Each owns one search job; check
`keyword-map.md` before starting, and move the row there when the post is published. Evidence
and SERP notes for each topic are in `research/2026-10-keyword-research.md`.

The owner chose (2026-10-25) to write Argon2id-specific posts now, for the traffic before the
Faraday upgrade (planned ~2027-02-16). Date them, and plan a rewrite or retirement when the
proof of work changes.

| # | Slug | Primary cluster | Search job | Evidence needed |
|---|---|---|---|---|
| 1 | `mine-blocknet-nvidia-gpu` | blocknet gpu mining; mine blocknet rtx 4090 / 5090 | Can my NVIDIA card mine Blocknet, how much does it add, how much VRAM | Per-card H/s; CPU+GPU totals; corrects the "GPUs don't help" claim ranking today |
| 2 | `argon2id-vs-randomx` | randomx vs argon2; blocknet vs monero mining | How Argon2id differs from RandomX for a Monero miner, and which machines suit which | Same-machine XMRig vs seine runs on 4-6 hosts |
| 3 | `how-much-ram-to-mine-blocknet` | ram per thread mining; argon2id 2gb per thread | How many threads my RAM allows, and why a 1 GB VPS fails | Thread-scaling on 2-channel vs 12-channel machines |
| 4 | `blocknet-mining-pools` | blocknet pool; blocknet mining pool list | Which pools exist and how fees and payouts compare | Dated table of every pool; disclose that we run bntpool.com |
| 5 | `blocknet-faraday-upgrade-miners` | blocknet faraday; blocknet hard fork 2027 | What miners must change for Faraday, and when | Final public spec and a seine release that supports it; publish around Jan 2027 |
| 6 | `seine-pool-shows-zero-hashrate` | pool hashrate lower than local; shares rejected | Troubleshoot hashing locally with nothing on the pool | Real error strings from seine and the pool |
| 7 | `antivirus-flags-crypto-miner` | windows defender crypto miner; is seine safe | Antivirus quarantined the miner: verify it | Checksum walkthrough; warning about third-party repacks |

Dropped 2026-10-25 per owner: a BNT-vs-BLOCK explainer (assume readers know Blocknet) and
"is CPU mining worth it" (invites an unfavorable verdict).

## Blocknet topics (general blog)

Coin-level posts that build search presence for Blocknet itself and link to seine and
bntpool.com. Facts come from fact-sheet section 1; demand is unverified (no SERP check yet), so
check search results before drafting.

| Slug | Primary cluster | Search job |
|---|---|---|
| `blocknet-faraday-upgrade` | blocknet faraday; blocknet post-quantum | What the planned upgrade brings, attributed to blocknetcrypto.com |

Low-confidence demand, revisit with Search Console data: HiveOS/mmpOS packaging, AVX-512 on
Zen 5, laptop mining.

## Off-site work that matters as much as posts

- Get Blocknet and seine listed on miningpoolstats, hashrate.no, whattomine and Kryptex (deferred
  by the owner on 2026-10-25; requests go out from the owner's accounts).
- Ask upstream for a "Mine" link from blocknetcrypto.com to seineminer.com.
- Give bntpool.com a crawlable title and description ("Blocknet mining pool").
- Rewrite the seine and blocknet-pool GitHub descriptions with "Blocknet miner/pool".
- Offer corrections to the pages ranking for our terms (powhub.top, cassh100k's guide).

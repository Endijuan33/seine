# Backlog

Researched topics not yet written, in priority order. Each owns one search job; check
`keyword-map.md` before starting, and move the row there when the post is published. Evidence
and SERP notes for each topic are in `research/2026-10-keyword-research.md`.

The owner chose (2026-10-25) to write Argon2id-specific posts now, for the traffic before the
Faraday upgrade (planned ~2027-02-16). Date them, and plan a rewrite or retirement when the
proof of work changes.

| # | Slug | Primary cluster | Search job | Evidence needed |
|---|---|---|---|---|
| 1 | `blocknet-mining-hardware-hashrate` | blocknet hashrate; best CPU for blocknet mining; best hardware for argon2id mining | Which machine to mine on and what H/s it will do | Benchmark table with method, consumer chips (7950X, 9950X, base M4), wall power |
| 2 | `crypto-mining-on-mac-apple-silicon` | mining on mac m4; mac mini mining; crypto mining apple silicon | Is my M-series Mac any good for mining, and how do I start | M-series tier table, RAM-per-thread limits for 16/24/36/48 GB, thermals |
| 3 | `argon2id-proof-of-work-explained` | argon2id proof of work; argon2id mining; memory-hard proof of work | What Argon2id does when it's used for mining, and why 2 GiB | A diagram of one hash; memory-bandwidth scaling data |
| 4 | `mine-blocknet-nvidia-gpu` | blocknet gpu mining; mine blocknet rtx 4090 / 5090 | Can my NVIDIA card mine Blocknet, how much does it add, how much VRAM | Per-card H/s; CPU+GPU totals; corrects the "GPUs don't help" claim ranking today |
| 5 | `argon2id-vs-randomx` | randomx vs argon2; blocknet vs monero mining | How Argon2id differs from RandomX for a Monero miner, and which machines suit which | Same-machine XMRig vs seine runs on 4-6 hosts |
| 6 | `how-much-ram-to-mine-blocknet` | ram per thread mining; argon2id 2gb per thread | How many threads my RAM allows, and why a 1 GB VPS fails | Thread-scaling on 2-channel vs 12-channel machines |
| 7 | `huge-pages-argon2id-mining` | huge pages mining; enable 1GB pages; huge pages windows 11 | Whether huge pages help Argon2id and how to enable them | A/B with 2 MiB, 1 GiB and off on several hosts |
| 8 | `numa-dual-socket-epyc-mining` | numa mining; dual socket epyc mining | Avoid losing hashrate across NUMA nodes on big servers | Before/after for the NUMA/L3 spreading change |
| 9 | `blocknet-mining-pools` | blocknet pool; blocknet mining pool list | Which pools exist and how fees and payouts compare | Dated table of every pool; disclose that we run bntpool.com |
| 10 | `blocknet-wallet-address-for-mining` | blocknet wallet; blocknet address | Where to get an address to mine to, keeping keys off the rig | Official wallet walkthrough |
| 11 | `blocknet-faraday-upgrade-miners` | blocknet faraday; blocknet hard fork 2027 | What miners must change for Faraday, and when | Final public spec and a seine release that supports it; publish around Jan 2027 |
| 12 | `seine-pool-shows-zero-hashrate` | pool hashrate lower than local; shares rejected | Troubleshoot hashing locally with nothing on the pool | Real error strings from seine and the pool |
| 13 | `antivirus-flags-crypto-miner` | windows defender crypto miner; is seine safe | Antivirus quarantined the miner: verify it | Checksum walkthrough; warning about third-party repacks |

Dropped 2026-10-25 per owner: a BNT-vs-BLOCK explainer (assume readers know Blocknet) and
"is CPU mining worth it" (invites an unfavorable verdict).

## Blocknet topics (general blog)

Coin-level posts that build search presence for Blocknet itself and link to seine and
bntpool.com. Facts come from fact-sheet section 1; demand is unverified (no SERP check yet), so
check search results before drafting.

| Slug | Primary cluster | Search job |
|---|---|---|
| `what-is-blocknet` | what is blocknet; blocknet privacy coin | A positive overview: private by default, payable by name, fair launch, mineable on ordinary computers |
| `blocknet-id-pay-by-name` | blocknet id; blocknet shortname; pay crypto by name | Get a Blocknet ID shortname and get paid to `@name` |
| `blocknet-encrypted-memos` | blocknet memo; encrypted crypto memo | Use the encrypted 128-byte memo on every payment, such as for invoices |
| `blocknet-payment-links` | blocknet payment link; bntpay | Request a payment with a link or a `blocknet://` URI |
| `blocknet-privacy-features` | blocknet ring signatures; stealth addresses | How Blocknet keeps every payment private: ring of 16, stealth addresses, Bulletproofs, Dandelion++ |
| `blocknet-fair-launch` | blocknet premine; blocknet fair launch | No premine and no dev fund, shown from the genesis block |
| `blocknet-emission-schedule` | blocknet supply; blocknet block reward | The reward schedule and supply, as a table |
| `blocknet-faraday-upgrade` | blocknet faraday; blocknet post-quantum | What the planned upgrade brings, attributed to blocknetcrypto.com |

Low-confidence demand, revisit with Search Console data: HiveOS/mmpOS packaging, AVX-512 on
Zen 5, laptop mining.

## Off-site work that matters as much as posts

- Get Blocknet and seine listed on miningpoolstats, hashrate.no, whattomine and Kryptex.
- Ask upstream for a "Mine" link from blocknetcrypto.com to seineminer.com.
- Give bntpool.com a crawlable title and description ("Blocknet mining pool").
- Rewrite the seine and blocknet-pool GitHub descriptions with "Blocknet miner/pool".
- Offer corrections to the pages ranking for our terms (powhub.top, cassh100k's guide).

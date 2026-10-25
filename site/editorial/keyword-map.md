# Keyword ownership map

Which seineminer.com URL owns which search intent. It records editorial intent, not current
rankings. The build fails if a published post has no row here.

## Rules

1. Each URL owns one primary keyword cluster and one distinct search job.
2. Read this map before proposing, drafting, renaming or retargeting a post.
3. A supporting phrase may appear naturally on related pages, but only its owner optimizes its
   title, H1, description and core answer around it.
4. Broad hubs summarize and route; narrow spokes answer one task in depth. Their jobs differ.
5. Don't retarget a URL away from its cluster to chase a broader term. Add a new URL only when
   the intent is genuinely different; otherwise strengthen the owner.
6. An ownership change updates this file in the same commit, with the old owner, new owner,
   overlap risk and redirect plan.
7. Translations inherit their English row. A Chinese-only target needs its own row naming the
   Chinese phrase (for example 挖矿 / 挖礦 variants).

## Name collisions

"Blocknet" also names an older project (BLOCK token) and "BNT" is Bancor's ticker, so titles
pair "Blocknet" with "mining", "miner" or "Argon2id" rather than standing alone. Posts don't
explain the collision to readers (owner decision 2026-10-25: assume the reader knows Blocknet).

## Current ownership

| Primary keyword cluster | URL | Search job | Supporting phrases |
|---|---|---|---|
| how to mine blocknet | `/blog/how-to-mine-blocknet/` | Get a newcomer from zero to accepted shares on any OS, and set expectations for payouts and hashrate | blocknet mining guide, blocknet miner, mine Blocknet BNT, bntpool setup, Blocknet Argon2id miner, 如何挖 Blocknet |
| what is blocknet | `/blog/what-is-blocknet/` | Explain what Blocknet is and does for someone new, then route to names, privacy and mining | Blocknet privacy coin, Blocknet cryptocurrency, Blocknet fair launch, Blocknet Faraday upgrade, Blocknet 是什么 |
| blocknet privacy | `/blog/blocknet-privacy-features/` | Explain each layer of Blocknet's mandatory privacy and what it means when you pay | blocknet ring signatures, blocknet stealth address, blocknet ring size, Blocknet 隐私 |
| blocknet id | `/blog/blocknet-id-pay-by-name/` | Claim a Blocknet ID shortname and get paid to `@name` (wallet, payment links, pool payouts) | blocknet.id, blocknet shortname, pay by name blocknet, mine to @name, Blocknet ID 短名称 |
| argon2id proof of work | `/blog/argon2id-proof-of-work-explained/` | Explain how Blocknet's Argon2id PoW works and why RAM and bandwidth decide hashrate | argon2id mining, argon2id parameters, memory-hard proof of work, blocknet 2 GB per thread, Argon2id 工作量证明 |
| best hardware for blocknet mining | `/blog/blocknet-mining-hardware-hashrate/` | Pick or size a mining machine: fastest hardware, threads per GB of RAM, expected H/s, CPU and GPU together | blocknet hashrate, blocknet mining RAM per thread, best CPU for Blocknet, Argon2id mining hardware, 挖 Blocknet 用什么硬件 |
| mine blocknet on mac | `/blog/mine-blocknet-on-mac/` | Set up and size Blocknet mining on an Apple Silicon Mac | blocknet apple silicon, blocknet m4 max hashrate, mine blocknet macbook, seine macos, 在 Mac 上挖 Blocknet |
| blocknet memo | `/blog/blocknet-encrypted-memos/` | Add an encrypted memo (invoice, order number, note) to a Blocknet payment, know who can read it, and find received memos | blocknet encrypted memo, blocknet payment memo, blocknet send memo, blocknet memo length, Blocknet 加密备注 / 加密備註 |
| blocknet payment link | `/blog/blocknet-payment-links/` | Write a `blocknet://` or bntpay.com link that requests an amount (with memo and `$name`) and know what pasting it does | bntpay link, blocknet uri, blocknet://, request BNT payment, blocknet invoice link, Blocknet 付款链接 / 付款連結 |
| huge pages blocknet mining | `/blog/huge-pages-blocknet-mining/` | Reserve huge pages (Linux/WSL 2 MiB and 1 GiB) or Windows large pages for seine: why they help Argon2id, pages per thread, exact commands, page modes | blocknet hugepages, seine hugepages, vm.nr_hugepages mining, 1GB huge pages argon2id, windows large pages mining, lock pages in memory seine, Blocknet 大页内存 / 大分頁 |
| mine blocknet on servers | `/blog/mine-blocknet-on-servers/` | Run seine on many-core servers and rigs: NUMA/L3 placement and its measured gains, headless runs, `--service`, the control API, live events and Prometheus | blocknet mining server, blocknet NUMA, EPYC blocknet mining, seine headless, seine control API, seine Prometheus metrics, 在服务器上挖 Blocknet / 在伺服器上挖 Blocknet |
| get a blocknet wallet for mining | `/blog/blocknet-wallet-for-mining/` | Create an official Blocknet wallet, back up the seed, get the address to mine to and watch pool payouts arrive | blocknet wallet, blocknet address, blocknet recovery seed, blocknet desktop wallet, blocknet cli wallet, 获取 Blocknet 钱包地址 / 取得 Blocknet 錢包地址 |
| seine dashboard | `/blog/seine-dashboard-explained/` | Read seine's terminal dashboard: every panel and label, live auto-tuning in the log, keyboard thread controls | seine hashrate current vs average, seine accepted rejected shares, seine change threads while mining, seine tui, seine --ui plain, seine 仪表盘 / 儀表板 |

## Boundaries

| Broad owner | Narrow owner | Boundary |
|---|---|---|
| `/blog/how-to-mine-blocknet/` | `/blog/blocknet-mining-hardware-hashrate/` | The guide shows one best-per-generation table and links out; the hardware post owns buying advice and full per-chip results. |
| `/blog/what-is-blocknet/` | `/blog/blocknet-privacy-features/`, `/blog/blocknet-id-pay-by-name/` | The overview summarizes privacy and names in a paragraph each and links out; the narrow posts own the mechanics. |
| `/blog/blocknet-mining-hardware-hashrate/` | `/blog/mine-blocknet-on-mac/` | The hardware post has one Mac row; the Mac post owns macOS setup and thread sizing by Mac memory. |
| `/blog/argon2id-proof-of-work-explained/` | `/blog/blocknet-mining-hardware-hashrate/` | The explainer owns why memory decides hashrate; the hardware post owns which machine to buy or use. |
| `/blog/blocknet-privacy-features/` | `/blog/blocknet-encrypted-memos/` | The privacy post explains in one section who can read a memo and why it is padded; the memos post owns adding, sending and reading memos. |
| `/blog/blocknet-id-pay-by-name/` | `/blog/blocknet-payment-links/` | The Blocknet ID post shows the two link forms with a name in one section; the payment-links post owns the full link format, encoding and the paste-to-confirm flow. |
| `/blog/blocknet-payment-links/` | `/blog/blocknet-encrypted-memos/` | The payment-links post says a link can carry a memo; the memos post owns memo encryption, padding and the `send` memo argument. |
| `/blog/how-to-mine-blocknet/` | `/blog/blocknet-wallet-for-mining/`, `/blog/seine-dashboard-explained/` | The guide has one bullet on getting an address and one line on the dashboard; the narrow posts own the wallet walkthrough and the panel-by-panel dashboard tour. |
| `/blog/blocknet-wallet-for-mining/` | `/blog/blocknet-id-pay-by-name/` | The wallet post mentions shortnames in one sentence; the Blocknet ID post owns names. |
| `/blog/argon2id-proof-of-work-explained/`, `/blog/blocknet-mining-hardware-hashrate/`, `/blog/how-to-mine-blocknet/` | `/blog/huge-pages-blocknet-mining/` | The broader posts keep a one-line huge-pages tip and link out; the huge-pages post owns sizing, commands and page modes. |
| `/blog/blocknet-mining-hardware-hashrate/` | `/blog/mine-blocknet-on-servers/` | The hardware post owns which machine to buy; the servers post owns running seine on big machines and rigs (placement, headless, control API). |
| `/blog/mine-blocknet-on-servers/` | `/blog/huge-pages-blocknet-mining/` | The servers post notes that placement and huge pages stack and that pools must be node-local; the huge-pages post owns reserving pages. |

bntpool.com's blog (blocknet-pool repo, `frontend/blog/`) owns pool-side jobs (joining the pool,
PPLNS payouts, miner stats, the reward schedule, the fair launch). Don't target those here.

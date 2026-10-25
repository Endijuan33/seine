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

## Boundaries

| Broad owner | Narrow owner | Boundary |
|---|---|---|
| `/blog/how-to-mine-blocknet/` | (future) `/blog/blocknet-mining-hardware-hashrate/` | The guide shows one best-per-generation table and links out; the hardware post owns buying advice and full per-chip results. |

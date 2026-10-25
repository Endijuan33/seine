# Writing guide (brief for any writing agent)

You're writing a post for the seineminer.com blog. seine is a free, open-source (BSD-3) miner
for Blocknet, a privacy cryptocurrency with a memory-hard Argon2id proof of work. The blog
exists to bring people searching about Blocknet, mining and Argon2id to seine and to the
bntpool.com pool. Follow this file, [`README.md`](README.md) (workflow, post format,
translations) and [`fact-sheet.md`](fact-sheet.md) exactly.

## Before you write

- Read `keyword-map.md`. Your post owns one search job that no other URL owns. Put the
  one-sentence intent statement from README step 1 in your batch notes.
- Read `src/content/blog/how-to-mine-blocknet/index.mdx` for tone and component use.
- Every number, fee, date, hardware result and protocol parameter comes from the fact sheet or
  from your batch's `research.md` with a source. If a fact isn't there, find a source and add
  it, or write around it. Arithmetic on sourced numbers is fine ("8 threads × 2 GB = 16 GB");
  invented "typical user" statistics are not.

## Disambiguation

"Blocknet" is also the name of an older, unrelated project (the BLOCK token, proof of stake),
and "BNT" is Bancor's ticker. Google's AI answer for "blocknet mining" currently says the coin
can't be mined. So:

- Make it unambiguous early in every coin-specific post which Blocknet this is: the Argon2id
  privacy coin at blocknetcrypto.com, launched February 2026. A short `<Note>` works.
- Never use the `$BNT` cashtag, never link blocknet.org, and never describe the BLOCK project's
  features as ours. State the difference factually; don't disparage the other project.

## Claims you must not make

From `marketing/brand/taglines.md`, the owner's decisions and the fact sheet. `npm run check`
fails the build on most of them.

- No "untraceable" or "anonymous" (匿名, 不可追踪); say "private".
- No "most private coin" or "more private than Monero".
- Post-quantum: today's chain uses classical elliptic-curve cryptography, so never call it
  post-quantum. You may describe the Faraday upgrade's planned post-quantum spend proofs,
  attributed to blocknetcrypto.com and in the future tense.
- No profit promises, "passive income" or "risk-free". Profitability depends on coin price,
  network difficulty and electricity cost; give the reader the arithmetic and a way to measure.
- No unannounced Faraday details. Only what blocknetcrypto.com states publicly.
- No AMD GPU, Windows ARM64 or production Metal support claims.
- seine's term is **threads** (the `--threads` flag), never "workers". On the pool, a "worker
  name" is fine because that's what the pool calls it.
- Don't lead with fees. Mention seine's dev fee (1% on bntpool.com, 2.5% elsewhere) and the
  pool's fee only where a reader needs them, such as a cost question in a how-to.

## House style

- Plain and direct, second person. Describe what the software does. No marketing
  constructions: no "X, not Y" ("measured, not promised"), no taglines ("built to…", "every
  hash", "start today"), no hype, no emoji, no filler intros. The first paragraph says what
  the reader will be able to do.
- American English. Sentence-case headings.
- Hardware: names people know in tables ("AMD EPYC · Zen 5", "Apple M4 Max") with the exact
  chip alongside. Show the best measured result per generation and leave out weak showings.
- Commands in fenced code blocks; flags, file names and addresses in backticks.
- Explain why, not just what: why a thread needs 2 GB, why huge pages help. That's what makes
  a post worth ranking over a forum thread.
- Original evidence wins: our benchmarks, a dashboard screenshot, a measured before/after.
  Say how and when it was measured.
- Link to seine's docs on GitHub for deep reference (performance tables, huge pages, large
  servers) instead of copying them.
- Chinese translations follow the same rules and stay plain and literal.

## Finish line

1. `npm run check -- <slug> --strict` passes.
2. `npm run overlap -- <slug>`: the closest posts are reviewed and the boundary is recorded in
   `keyword-map.md`.
3. Every factual claim traces to the fact sheet or batch research.
4. The post stays `draft: true` until the user approves it; translation and publishing are
   README steps 6-7.

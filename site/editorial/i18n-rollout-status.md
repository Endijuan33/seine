# 20-language rollout: handoff (2026-10-25)

Delete this file when the rollout is merged. Owner wants the same 20 locales as Sol Incinerator.
**User rule: don't spam subagents.** The last session ran ~20 agents at once and burned the
weekly limit. Run one or two agents at a time (or work inline), and say how many before starting.

## Where things are

- seine worktree `/home/zp/Code/blocknet/seine-i18n`, branch `claude/i18n-20`, pushed nowhere
  yet, rebased on seine master as of batch 3 (2056522). Everything is committed.
- Infra is done: 20 locales in `src/i18n/locales.ts` + `messages/*.json`, language menu,
  RTL for Arabic, language-suggest banner (`src/components/LanguageSuggestion.astro`),
  locale number formats on the landing page, OG cards (ar/th fall back to English cards).
  A stub smoke build produced 301 pages.
- The build and `npm run check` fail until **every published post has all 19 translations
  reviewed**. That's by design; don't weaken it to ship.
- Check one locale: `node scripts/check-content.mjs <slug> --locale fr --strict`
  (or omit the slug for all posts). Hash: `node scripts/check-content.mjs --hash <slug>`.

| Locale | State |
|---|---|
| zh-Hans, zh-Hant, es, pt-BR, ru, ja, ko, de, it | done: translated + separately reviewed, strict check passes |
| id | 23 drafted; 16 reviewed, 7 left (listed below) |
| tr | 23 drafted; 8 reviewed |
| vi | 23 drafted; 8 reviewed |
| nl | 23 drafted; 1 reviewed (huge-pages) |
| fr, pl, ar, uk | 23 drafted; none reviewed |
| el | 12 drafted (argon2id, encrypted-memos, id-pay-by-name, hardware-hashrate, payment-links, how-to-mine, huge-pages, mac, servers, old-pc, pow-vs-pos, ring-signatures); 11 missing; none reviewed |
| th | not started |

Reviewed so far in partial locales (`translation: reviewed` in front matter is the source of
truth; `grep -L '^translation: reviewed' src/content/blog/*/<code>.mdx` lists what's left):
- id left: how-to-mine-blocknet, ring-signatures-explained, seed-phrase-explained,
  seine-dashboard-explained, stealth-addresses-explained, what-is-a-privacy-coin, what-is-blocknet.
- tr: the 7 alphabetically-first A-half posts + huge-pages.
- vi: first 4 A-half posts + first 4 B-half posts.

Interrupted drafts all pass the structure checks, but the uk A half and the el files were
written by agents cut off by the usage limit: the reviewer should confirm each file is complete
against the English (no truncated final sections).

## How to finish

Rules: `editorial/translation-guide.md` (numbers and dates in local format, glossary).
Briefs: `editorial/translation-terms/_translator-brief.md` and `_reviewer-brief.md`.
Per-locale term decisions: `editorial/translation-terms/<code>.md` (read before translating
or reviewing; append new decisions). Posts were split into halves: A = the 12 alphabetically
first slugs (argon2id … how-to-mine-blocknet), B = the other 11.

Order of work:
1. Review the drafted locales (a different agent from the translator, per the guide). Set
   `translation: reviewed` only after the post-by-post review; keep `sourceHash`.
   Known items for reviewers: nl quotes were partly left in English (translate them like every
   other locale); fr needs U+00A0 before `: ; ? !` after inline code too, and one consistent
   "ring signature" term (signature de cercle vs en anneau); tr settles "bellek alanı" for the
   2 GiB arena; id/vi Defender buttons use the local Windows labels.
2. Translate the 11 missing el posts, then review el.
3. Translate and review th (Gregorian years, CJK/Thai title budget ~30 chars).
4. `npm run check -- --strict && npm run build` in `site/`, then visual check in the shared
   browser: /ar/ (RTL), the language menu, the banner (set localStorage
   `seine:language:preference:v1` to a code on an English page; remove it afterwards).
5. Rebase on seine master, fast-forward master, push (CI deploys seineminer.com), run
   `npm run indexnow`. Ping blocknet-4f (landing/design owner) for a visual look if around.

## bntpool.com blog (second half of the rollout)

- Worktree `/home/zp/Code/blocknet/blocknet-pool-blog`, branch `claude/pool-blog-i18n`
  (one WIP commit on top of master 1c170e7). Infra done and smoke-built (220 pages): 20 locales
  in `frontend/scripts/blog-lib.mjs`, catalogs in `frontend/blog/messages/*.json`, `<details>`
  language menu, CSP-safe banner `frontend/blog/lang.js` fed by a JSON data block, RTL CSS.
- Still needed: translate + review the 10 pool posts (`frontend/blog/content/<slug>/`) into the
  17 new locales as `<code>.md` (same rules; blog links `/blog/<path>/<slug>/`, app links
  unprefixed, seineminer links `https://seineminer.com/<path>/...`). Check with
  `node scripts/check-blog.mjs <slug> --locale es` from `frontend/`.
- Then `npm --prefix frontend run build`, rebase on master, push, deploy with
  `./scripts/deploy_bntpool.sh --api-only` (never restart Stratum for this). Production deploy
  of the earlier blog was approved; the pool also has pending master commit 1c170e7
  (Checkpoints wording) that ships with the next API deploy.

## Open questions for the owner (unanswered)

- Raise bntpool.com `latest_miner_version` 0.2.15 → 0.2.23 (root-owned prod config
  `/etc/blocknet/pool/config.json`; marks miners on older seine as outdated)?
- GPU post: include the RTX 5090 (~8 H/s) figure or not?

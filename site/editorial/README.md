# seineminer.com blog pipeline

How articles for seineminer.com get chosen, researched, written, translated, checked and
published. The blog covers Blocknet in general (its privacy features, payment by name, wallet,
upgrades) and mining it, so it builds search traffic for both the coin and the miner. That
traffic should turn into Blocknet users, seine downloads and bntpool.com miners.

**First rule:** everything promotes Blocknet, bntpool.com and seine, with nothing negative
about any of them. See the top of [`writing-guide.md`](writing-guide.md). Write it without
thin keyword-permutation pages, which Google's scaled-content policy penalizes.

| File | What it is |
|---|---|
| [`keyword-map.md`](keyword-map.md) | Which URL owns which search intent. Read it before proposing or retargeting a post. |
| [`backlog.md`](backlog.md) | Researched topics not yet written, in priority order. |
| [`fact-sheet.md`](fact-sheet.md) | The only permitted source of facts and numbers, each with a source. |
| [`writing-guide.md`](writing-guide.md) | House style, claims to avoid, and the brief for a writing agent. |
| [`research/`](research/) | Dated search-landscape research behind the backlog. |
| `batches/<yyyy-mm>-<name>/` | Per-batch research notes and source evidence. |
| `../scripts/check-content.mjs` | Editorial and translation checks (`npm run check`) and the overlap report (`npm run overlap`). |

## Writing a post, step by step

1. **Pick one search job.** Take a topic from `backlog.md` (or propose one) and read
   `keyword-map.md`. Write one sentence: the reader's distinct job, the nearest existing owner,
   and the evidence or procedure this post adds. If the honest answer is "the same thing,
   reworded", improve the existing post instead.
2. **Research.** Make `batches/<yyyy-mm>-<name>/research.md`. Record every fact you'll use that
   isn't in `fact-sheet.md`, with its source (repo `path:line`, official URL, or a measurement
   with its command and date), and add durable facts to the fact sheet in the same change. No
   invented numbers, user statistics, prices or competitor features.
3. **Draft in English.** Create `src/content/blog/<slug>/index.mdx` with `draft: true`. The
   slug is the primary keyword, lowercase and hyphenated; it becomes the URL and never changes.
4. **Check.** `npm run check -- <slug> --strict` and `npm run overlap -- <slug>`. Read the
   closest matches: low wording overlap can still mean duplicate intent.
5. **Review.** Re-read every number and claim against the fact sheet and check each link.
   Preview with `npm run dev` (drafts are hidden; remove `draft: true` locally to see one) and
   look at the social card at `/blog/og/en/<slug>.jpg`.
6. **Translate** (step 3 of the translation section below) once the English is approved.
7. **Publish.** Remove `draft: true`, add the post's row to `keyword-map.md`, remove it from
   `backlog.md`, and commit everything together. The build refuses a published post without a
   keyword-map row or without reviewed, current translations. Pushing to `master` deploys.
8. **After publishing.** Google reads `sitemap.xml` (the domain property is verified in Search
   Console) and IndexNow notifies Bing on each deploy; request indexing for a new post with URL
   inspection in Search Console if you want it sooner. Submit Chinese pages to Baidu Ziyuan,
   and link to the post from somewhere relevant (README, release notes, pool FAQ, a reply on X). After 4-8 weeks, read the Search Console queries and
   strengthen what ranks 5-20 rather than adding near-duplicates.

## Post format

```yaml
---
title: How to mine Blocknet on a Mac           # the H1 and social title, in plain words people search
seoTitle: Mine Blocknet on Apple Silicon       # optional; only when the title is over 60 characters
description: 140-160 characters that lead with the answer or benefit and contain the primary keyword.
date: 2026-10-25
updated: 2026-11-10                            # optional; bump on material revisions only
tags: [Mining guide, macOS, Apple Silicon]     # 2-6
faq:                                           # 3-5; rendered on the page and as FAQPage data
  - q: Can you mine Blocknet on a MacBook Air?
    a: One line of plain text, 40-60 words, that answers fully without the article.
related: [how-to-mine-blocknet]                # 2-4 slugs once there are 3+ posts
draft: true
---
```

Body rules (`npm run check` enforces most of them):

- 900-1,600 words. Sections start at `##`; the title is the page's only H1.
- Phrase at least two headings as the question a searcher types ("How much RAM does Blocknet
  mining need?") and answer it in the first sentence underneath.
- Put a 40-60 word `<Answer>` directly under the one or two most important question headings.
  It is the snippet and AI-answer candidate, so it must make sense on its own:

  ```mdx
  <Answer>
  Blocknet's proof of work uses about 2 GB of memory per hash, so each mining thread...
  </Answer>
  ```

- `<Note>` and `<Warning>` render labelled asides. Tables, fenced code and images are normal
  Markdown. MDX treats `{` and `<` as code, so write them as `\{` and `&lt;` in prose.
- Link to home sections (`/#download`, `/#start`, `/#performance`) and 1-2 other posts as
  `/blog/<slug>/` with the trailing slash. The check fails on internal links that would 404.
- Don't write an FAQ or "Related reading" section in the body; front matter renders them. The
  download call to action is added to the end of every post automatically.

## Translations (Simplified and Traditional Chinese)

Every published post exists in English, `zh-Hans` (at `/zh-hans/`) and `zh-Hant` (at
`/zh-hant/`). A translation sits beside its source as `<slug>/zh-Hans.mdx` and
`<slug>/zh-Hant.mdx` and carries only translated fields:

```yaml
---
title: …
description: …
tags: […]
faq: [...]                  # same number of entries as the source
sourceHash: 8a26ef5e623a    # npm run check -- --hash <slug>
translation: reviewed       # draft until a separate review pass
---
```

1. Translate with the active model; don't call external translation APIs. zh-Hans follows
   mainland usage (线程, 内存, 软件, 显卡, 默认); zh-Hant follows Taiwan usage (執行緒, 記憶體,
   軟體, 顯示卡, 預設), not a character conversion of zh-Hans. Keep it plain and literal.
2. Keep exactly: numbers, chip names, commands, code blocks, external URLs, `@name`, `BNT`,
   product names, and lowercase "seine". Prefix internal links with the locale
   (`/zh-hans/#download`, `/zh-hant/blog/<slug>/`).
3. Do a separate review pass for meaning, omissions, additions and terminology, preferably by a
   different agent, then set `translation: reviewed`. Don't claim an independent review that
   didn't happen.
4. Any later edit to `index.mdx` changes its hash, and the build fails until each translation's
   changed passages are updated and its `sourceHash` matches again. Re-translate only what
   changed.

Interface strings live in `src/i18n/ui.ts`. The Chinese catalogs are typed against the English
keys, so a missing string fails the build.

## What the build does for SEO

For every page in every locale: canonical URL, `hreflang` alternates (plus `x-default`),
title and description, Open Graph and Twitter tags with a generated 1200x630 card per post
and locale, and JSON-LD that matches the visible page (`Organization`, `WebSite`,
`SoftwareApplication`, `Blog`, `BlogPosting`, `BreadcrumbList`, and `FAQPage` when a post has
an FAQ). Also `sitemap.xml` with alternates and `lastmod`, an RSS feed per locale, `robots.txt`,
`llms.txt`, responsive AVIF/WebP images, and self-hosted fonts (Google Fonts is blocked in
mainland China). The Pages preview host (`*.pages.dev`) sends `X-Robots-Tag: noindex`.

# seineminer.com

Static site for seineminer.com, hosted on Cloudflare Pages (project `seineminer`).

```sh
pip install -r site/requirements.txt
python3 site/build.py                       # writes site/dist
python3 -m http.server -d site/dist 8791    # preview
```

- `templates/`: the shared page shell (`base.html`) and the home page.
- `blog/<slug>.md`: one file per post, published at `/blog/<slug>/`. Front matter needs
  `title`, `description` (used for search results and link previews) and `date`;
  `image` and `draft: true` are optional. Pushing a post to `master` deploys it.
- The build adds the blog index, `/blog/rss.xml`, `sitemap.xml`, `robots.txt`, structured
  data for search engines, and copies the icon, social card and chart from `docs/assets`.

Deploys run from `.github/workflows/site.yml` on pushes to `master` that touch the site
or brand assets. It needs the `CLOUDFLARE_API_TOKEN` (Pages: Edit) and
`CLOUDFLARE_ACCOUNT_ID` repository secrets. Manual deploy:
`npx wrangler pages deploy site/dist --project-name seineminer --branch main`.

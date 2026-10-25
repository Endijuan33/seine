#!/usr/bin/env python3
"""Build seineminer.com into site/dist.

    pip install -r site/requirements.txt
    python3 site/build.py

Pages: the home page (templates/home.html) and one page per blog post. Write a post as
site/blog/<slug>.md with a front-matter block:

    ---
    title: How to mine Blocknet on Windows
    description: One or two sentences for search results and link previews.
    date: 2026-10-25
    image: /assets/social-card.jpg   (optional, absolute path or URL)
    draft: true                      (optional, skips the post)
    ---

The build also writes the blog index, an RSS feed, sitemap.xml, robots.txt and 404.html,
and copies brand assets from docs/assets so the site and README stay in sync.
"""

import datetime as dt
import html
import json
import re
import shutil
from email.utils import format_datetime
from pathlib import Path

import markdown

SITE = Path(__file__).resolve().parent
ROOT = SITE.parent
DIST = SITE / "dist"
BASE_URL = "https://seineminer.com"
DEFAULT_IMAGE = BASE_URL + "/assets/social-card.jpg"
BRAND_ASSETS = {
    "docs/assets/brand/icon.svg": "icon.svg",
    "docs/assets/brand/icon-32.png": "icon-32.png",
    "docs/assets/brand/icon-180.png": "icon-180.png",
    "docs/assets/brand/icon-512.png": "icon-512.png",
    "docs/assets/brand/social-card.jpg": "social-card.jpg",
    "docs/assets/charts/best.svg": "best.svg",
}


def read(path):
    return (SITE / path).read_text()


def render(content, *, title, description, url, image=DEFAULT_IMAGE, og_type="website", extra_head="", scripts=""):
    page = read("templates/base.html")
    for key, value in {
        "title": html.escape(title, quote=True),
        "description": html.escape(description, quote=True),
        "url": url,
        "image": image,
        "og_type": og_type,
        "extra_head": extra_head,
        "content": content,
        "scripts": scripts,
    }.items():
        page = page.replace("{{" + key + "}}", value)
    return page


def json_ld(data):
    return '<script type="application/ld+json">' + json.dumps(data, separators=(",", ":")) + "</script>"


def parse_post(path):
    text = path.read_text()
    m = re.match(r"^---\n(.*?)\n---\n(.*)$", text, re.S)
    if not m:
        raise SystemExit(f"{path}: missing front matter")
    meta = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            meta[k.strip()] = v.strip()
    for key in ("title", "description", "date"):
        if key not in meta:
            raise SystemExit(f"{path}: front matter needs '{key}'")
    meta["slug"] = path.stem
    meta["date"] = dt.date.fromisoformat(meta["date"])
    meta["draft"] = meta.get("draft", "").lower() in ("true", "yes", "1")
    image = meta.get("image") or DEFAULT_IMAGE
    meta["image"] = image if image.startswith("http") else BASE_URL + image
    meta["html"] = markdown.markdown(m.group(2), extensions=["fenced_code", "tables", "sane_lists"])
    meta["url"] = f"{BASE_URL}/blog/{meta['slug']}/"
    return meta


def nice_date(d):
    return d.strftime("%-d %B %Y")


def build():
    if DIST.exists():
        shutil.rmtree(DIST)
    (DIST / "assets").mkdir(parents=True)
    for f in (SITE / "assets").iterdir():
        shutil.copy2(f, DIST / "assets" / f.name)
    for src, name in BRAND_ASSETS.items():
        shutil.copy2(ROOT / src, DIST / "assets" / name)

    urls = [(BASE_URL + "/", dt.date.today())]

    # home
    home_ld = json_ld({
        "@context": "https://schema.org", "@type": "SoftwareApplication", "name": "seine",
        "applicationCategory": "UtilitiesApplication", "operatingSystem": "Windows, macOS, Linux",
        "description": "The fastest miner for the Blocknet cryptocurrency, for CPUs and NVIDIA GPUs.",
        "url": BASE_URL + "/", "downloadUrl": "https://github.com/zpalmtree/seine/releases/latest",
        "license": "https://opensource.org/licenses/BSD-3-Clause",
        "offers": {"@type": "Offer", "price": "0", "priceCurrency": "USD"},
    })
    (DIST / "index.html").write_text(render(
        read("templates/home.html"),
        title="seine — the fastest Blocknet miner",
        description="seine is the fastest miner for Blocknet. Tuned kernels for every modern CPU and NVIDIA GPU, "
                    "zero configuration. Free and open source for Windows, Linux and macOS.",
        url=BASE_URL + "/",
        extra_head='<link rel="preload" as="image" href="/assets/hero.jpg">\n' + home_ld,
        scripts=read("templates/home.js.html"),
    ))

    # posts
    posts = sorted((parse_post(p) for p in (SITE / "blog").glob("*.md")), key=lambda p: p["date"], reverse=True)
    posts = [p for p in posts if not p["draft"]]
    for p in posts:
        body = f'''<main class="wrap">
  <header class="page-head">
    <div class="kicker"><a href="/blog/" style="text-decoration:none">Blog</a></div>
    <h1>{html.escape(p["title"])}</h1>
    <div class="meta"><time datetime="{p["date"].isoformat()}">{nice_date(p["date"])}</time></div>
  </header>
  <article class="prose">
{p["html"]}
    <aside class="post-cta"><b>Try seine</b><a class="btn primary" href="/#download">Download for free</a></aside>
  </article>
</main>'''
        ld = json_ld({
            "@context": "https://schema.org", "@type": "BlogPosting", "headline": p["title"],
            "description": p["description"], "datePublished": p["date"].isoformat(), "image": p["image"],
            "mainEntityOfPage": p["url"], "author": {"@type": "Organization", "name": "seine"},
            "publisher": {"@type": "Organization", "name": "seine",
                          "logo": {"@type": "ImageObject", "url": BASE_URL + "/assets/icon-512.png"}},
        })
        out = DIST / "blog" / p["slug"]
        out.mkdir(parents=True)
        (out / "index.html").write_text(render(body, title=f'{p["title"]} · seine', description=p["description"],
                                               url=p["url"], image=p["image"], og_type="article", extra_head=ld))
        urls.append((p["url"], p["date"]))

    items = "\n".join(
        f'    <li><a href="/blog/{p["slug"]}/"><time datetime="{p["date"].isoformat()}">{nice_date(p["date"])}</time>'
        f'<h2>{html.escape(p["title"])}</h2><p>{html.escape(p["description"])}</p></a></li>' for p in posts
    ) or "    <li><p>No posts yet.</p></li>"
    (DIST / "blog").mkdir(exist_ok=True)
    (DIST / "blog" / "index.html").write_text(render(
        f'<main class="wrap"><header class="page-head"><div class="kicker">Blog</div><h1>Mining Blocknet, faster</h1>'
        f'<div class="meta">Guides, benchmarks and release notes from the seine team.</div></header>\n'
        f'  <ul class="posts">\n{items}\n  </ul>\n</main>',
        title="Blog · seine", description="Guides, benchmarks and release notes for seine, the fastest Blocknet miner.",
        url=BASE_URL + "/blog/"))
    urls.append((BASE_URL + "/blog/", posts[0]["date"] if posts else dt.date.today()))

    # RSS
    rss_items = "".join(
        f"<item><title>{html.escape(p['title'])}</title><link>{p['url']}</link><guid>{p['url']}</guid>"
        f"<pubDate>{format_datetime(dt.datetime.combine(p['date'], dt.time(12), dt.timezone.utc))}</pubDate>"
        f"<description>{html.escape(p['description'])}</description></item>" for p in posts)
    (DIST / "blog" / "rss.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>seine blog</title>'
        f"<link>{BASE_URL}/blog/</link><description>Guides, benchmarks and release notes for seine.</description>"
        f"{rss_items}</channel></rss>\n")

    # sitemap, robots, 404, headers
    (DIST / "sitemap.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        + "".join(f"  <url><loc>{u}</loc><lastmod>{d.isoformat()}</lastmod></url>\n" for u, d in urls) + "</urlset>\n")
    (DIST / "robots.txt").write_text(f"User-agent: *\nAllow: /\nSitemap: {BASE_URL}/sitemap.xml\n")
    (DIST / "404.html").write_text(render(
        '<main class="wrap"><header class="page-head"><div class="kicker">404</div><h1>That page slipped through the net</h1>'
        '<div class="meta"><a href="/">Back to seineminer.com</a></div></header></main>',
        title="Not found · seine", description="Page not found.", url=BASE_URL + "/404"))
    (DIST / "_headers").write_text("/assets/*\n  Cache-Control: public, max-age=86400\n")
    print(f"built {len(posts)} post(s) into {DIST.relative_to(ROOT)}")


if __name__ == "__main__":
    build()

#!/usr/bin/env python3
"""Checks a tutorial site built by scripts/build_site.sh.

Usage: check_site.py [--base-path PATH] SITE_DIR

English pages are at the root of SITE_DIR and Japanese pages under ja/.
PATH is the URL path the site is served from ("/" by default); links that
start with "/" are read relative to it. The checks:

- .nojekyll, source-revision.txt, index.html and ja/index.html exist, and
  each language has a search index;
- every page has <html lang="en">, or lang="ja" under ja/;
- no page contains an unexpanded mdBook directive or snippet anchor;
- both languages have the same chapter pages in the same order, and every
  chapter links, with hreflang, to the same chapter in the other language;
- every local href and src resolves to a file in the site, and every
  fragment to an id in the target page;
- external links use https and an allowed host, and links into the
  repository point at blob/main or tree/main and a path that exists here.

Prints a summary and exits 0 when everything passes; otherwise every problem
goes to stderr and the exit status is 1. Standard library only.
"""

import posixpath
import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit

ROOT = Path(__file__).resolve().parent.parent
REPO = "https://github.com/efg-technologies/senu"
ALLOWED_HOSTS = {"modelcontextprotocol.io", "rust.sdk.modelcontextprotocol.io", "rustup.rs"}
NOT_CHAPTERS = {"404.html", "print.html", "toc.html"}
LEFTOVER = re.compile(r"\{\{#|ANCHOR(_END)?:")


class Page(HTMLParser):
    def __init__(self, text):
        super().__init__(convert_charrefs=True)
        self.lang, self.base, self.ids, self.links, self.alternates = None, None, set(), [], []
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "html":
            self.lang = attrs.get("lang")
        if tag == "base" and attrs.get("href"):
            self.base = attrs["href"]
        for name in ("id", "name") if tag == "a" else ("id",):
            if attrs.get(name):
                self.ids.add(attrs[name])
        for name in ("href", "src"):
            if attrs.get(name) is not None and tag != "base":
                self.links.append(attrs[name])
        if tag == "a" and attrs.get("hreflang") and attrs.get("href"):
            self.alternates.append((attrs["hreflang"], attrs["href"]))


class Site:
    def __init__(self, root, base):
        self.root, self.base, self.pages, self.problems = root, base, {}, []

    def page(self, rel):
        if rel not in self.pages:
            self.pages[rel] = Page((self.root / rel).read_text(encoding="utf-8"))
        return self.pages[rel]

    def problem(self, message):
        self.problems.append(message)

    def resolve(self, rel, href):
        """The site file and fragment a local link points to, or None if it leaves the site."""
        page = self.page(rel)
        url = urljoin(self.base + rel, page.base or "")
        target = urlsplit(urljoin(url, href))
        raw = unquote(target.path)
        path = posixpath.normpath(raw)
        if raw.endswith("/") and not path.endswith("/"):
            path += "/"
        if path + "/" == self.base:
            path = self.base
        if not path.startswith(self.base):
            return None
        inside = path[len(self.base):]
        if inside == "" or inside.endswith("/"):
            inside += "index.html"
        return inside, unquote(target.fragment)

    def check_link(self, rel, href):
        if href.startswith(("mailto:", "javascript:")) or href in ("", "#"):
            if href.startswith("javascript:"):
                self.problem(f"{rel}: javascript link")
            return
        if href.startswith("#"):
            # Checked as a reference into the same page, even under <base>.
            if unquote(href[1:]) not in self.page(rel).ids:
                self.problem(f"{rel}: {href} -> no such id on the page")
            return
        parts = urlsplit(href)
        if parts.scheme or href.startswith("//"):
            self.check_external(rel, href, parts)
            return
        resolved = self.resolve(rel, href)
        if resolved is None:
            self.problem(f"{rel}: {href} points outside the site base {self.base}")
            return
        target, fragment = resolved
        if not (self.root / target).is_file():
            self.problem(f"{rel}: {href} -> missing {target}")
        elif fragment and target.endswith(".html") and fragment not in self.page(target).ids:
            self.problem(f"{rel}: {href} -> no id {fragment!r} in {target}")

    def check_external(self, rel, href, parts):
        if parts.scheme != "https":
            self.problem(f"{rel}: {href} is not https")
        elif parts.netloc == "github.com":
            path = parts.path.rstrip("/")
            prefix = urlsplit(REPO).path
            if path == prefix:
                return
            match = re.fullmatch(re.escape(prefix) + r"/(blob|tree)/main/(.+)", path)
            if not match:
                self.problem(f"{rel}: {href} is not a blob/main or tree/main link into {REPO}")
                return
            kind, inside = match.groups()
            local = ROOT / unquote(inside)
            if not (local.is_file() if kind == "blob" else local.is_dir()):
                self.problem(f"{rel}: {href} names {inside}, which is not a {'file' if kind == 'blob' else 'directory'} here")
        elif parts.netloc not in ALLOWED_HOSTS:
            self.problem(f"{rel}: {href} uses a host that is not allowed")


def chapter_order(site, toc):
    """The chapter pages that toc.html links to, in order."""
    order = []
    for href in site.page(toc).links:
        if href.endswith(".html") and not urlsplit(href).scheme:
            resolved = site.resolve(toc, href)
            if resolved and resolved[0] not in order:
                order.append(resolved[0])
    return order


def check(root, base):
    site = Site(root, base)
    for required in (".nojekyll", "source-revision.txt", "index.html", "ja/index.html"):
        if not (root / required).is_file():
            site.problem(f"missing {required}")
    if site.problems:
        return site
    for language in ("", "ja/"):
        if not list((root / language).glob("searchindex*.js")):
            site.problem(f"no search index in /{language}")

    html = sorted(p.relative_to(root).as_posix() for p in root.rglob("*.html"))
    for rel in html:
        text = (root / rel).read_text(encoding="utf-8")
        want = "ja" if rel.startswith("ja/") else "en"
        if site.page(rel).lang != want:
            site.problem(f"{rel}: lang is {site.page(rel).lang!r}, expected {want!r}")
        if LEFTOVER.search(text):
            site.problem(f"{rel}: contains an unexpanded mdBook directive or snippet anchor")
        for href in site.page(rel).links:
            site.check_link(rel, href)

    english = [p for p in html if "/" not in p and p not in NOT_CHAPTERS]
    japanese = [p[3:] for p in html if p.startswith("ja/") and p.count("/") == 1 and p[3:] not in NOT_CHAPTERS]
    if english != japanese:
        site.problem(f"chapters differ: en {english}, ja {japanese}")
    en_order = [p for p in chapter_order(site, "toc.html") if p in english]
    ja_order = [p[3:] for p in chapter_order(site, "ja/toc.html") if p[3:] in japanese]
    if en_order != ja_order or sorted(en_order) != english:
        site.problem(f"the tables of contents differ or miss chapters: en {en_order}, ja {ja_order}")
    for chapter in english:
        for rel, other, lang in ((chapter, "ja/" + chapter, "ja"), ("ja/" + chapter, chapter, "en")):
            if not (root / rel).is_file():
                continue
            links = [site.resolve(rel, href) for code, href in site.page(rel).alternates if code == lang]
            if (other, "") not in links:
                site.problem(f"{rel}: no hreflang={lang} link to {other}")
    if not site.problems:
        print(f"site: {len(html)} pages, {len(english)} chapters in en and ja, links and languages ok")
    return site


def main(argv):
    base = "/"
    if argv[:1] == ["--base-path"] and len(argv) >= 2:
        base, argv = argv[1], argv[2:]
    if len(argv) != 1 or not (base.startswith("/") and base.endswith("/")):
        print("usage: check_site.py [--base-path /PATH/] SITE_DIR", file=sys.stderr)
        return 2
    root = Path(argv[0])
    if not root.is_dir():
        print(f"check_site: {root} is not a directory", file=sys.stderr)
        return 1
    site = check(root, base)
    for problem in site.problems:
        print(f"check_site: {problem}", file=sys.stderr)
    return 1 if site.problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

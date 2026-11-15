#!/usr/bin/env python3
"""Saves the pages of `senu site` as static files (spec 0003, CI-1).

Usage: snapshot_site.py [--base-url https://host/path/] <new-or-empty-directory>

Starts `cargo run --quiet --locked -- site` on a free loopback port in its
own process group, crawls every internal link from / and /ja/, and fails
unless each one answers 200 with HTML. Pages are saved as index.html,
<slug>.html, ja/index.html and ja/<slug>.html, and every internal href
attribute of a real start tag becomes the relative path of the saved file,
while text, code, entities and comments stay byte for byte, so that an
escaped example such as `&lt;a href="/x"&gt;` is shown as served. The
stylesheet, /_senu/ui.css, is fetched as text/css and saved byte for byte
as _senu/ui.css (spec 0004, §6), and THIRD-PARTY-NOTICES.txt is copied
next to the pages. The directory works from
any plain static file server on localhost, or straight from disk. Finally,
every relative href must name a saved file and every #fragment an id in
the page it points to, the saved stylesheet must equal the served one and
carry both package notices, and the notices file must exist. The server is
stopped as Ctrl+C would. Standard library only.

With --base-url, the public address of the snapshot (spec 0006, §6), the
server gets it as SENU_SITE_URL, so every page names its canonical address,
and these files are written too: sitemap.xml with every page and its
translations, robots.txt, llms.txt (the project and every English page with
its description, for AI assistants), llms-full.txt (the English Markdown
sources in the order of the contents), each article's Markdown source next
to its page, 404.html and .nojekyll. They are then checked: every page has
exactly one canonical address, one description and one JSON-LD element
that parses, the sitemap lists exactly the saved pages, and every link of
llms.txt names a saved Markdown file. This script deploys nothing; the
`pages` workflow uploads the directory.
"""

import json

import html.parser
import posixpath
import re
import shutil
import signal
import sys
import urllib.error
import urllib.request
import xml.etree.ElementTree
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_tutorial import ROOT, CheckFailed, clean_env, start_server, stop  # noqa: E402

START = ("/", "/ja/")
STYLESHEET = "/_senu/ui.css"
NOTICES = "THIRD-PARTY-NOTICES.txt"
PACKAGE_NOTICES = (b"/*! tailwindcss v", b"/*! @tailwindcss/typography v")
HREF_ATTRIBUTE = re.compile(r'\shref="([^"]*)"')
BASE_URL = re.compile(r"https://[A-Za-z0-9][A-Za-z0-9.-]*(:[0-9]+)?/([A-Za-z0-9._~-]+/)*")
SITEMAP_NS = "http://www.sitemaps.org/schemas/sitemap/0.9"
XHTML_NS = "http://www.w3.org/1999/xhtml"
# The groups of articles by number, in the order of the contents, as
# `Section` in src/view.rs: the guide, the course, the notes, the appendices.
SECTIONS = (
    ("Library guide", range(10, 50)),
    ("Course: learn Rust with Senu", range(0, 10)),
    ("Notes", range(50, 90)),
    ("Appendices", range(90, 100)),
)
SUMMARY = (
    "Senu is an open-source Rust library (Apache-2.0). You declare a read-only function once, "
    "with its permission and parameters, and Senu serves it as an HTML page, a JSON API and an "
    "MCP tool for AI assistants, with the same authentication order, the same errors and one "
    "contract test suite for all three. It is built on Axum, Askama and the official rmcp SDK."
)


class Links(html.parser.HTMLParser):
    """The href and id attributes of real start tags, and for each href the
    position and text of its start tag. Also the page's metadata: its
    title, description, canonical address, language versions and JSON-LD."""

    def __init__(self, text):
        super().__init__()
        self.hrefs, self.ids, self.tags = [], set(), []
        self.title, self.descriptions, self.canonicals = "", [], []
        self.alternates, self.json_ld = [], []
        self._text = None
        self.feed(text)
        self.close()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name == "href":
                self.hrefs.append(value)
                self.tags.append((self.getpos(), self.get_starttag_text(), value))
            elif name == "id":
                self.ids.add(value)
        attrs = dict(attrs)
        if tag == "meta" and attrs.get("name") == "description":
            self.descriptions.append(attrs.get("content", ""))
        elif tag == "link" and attrs.get("rel") == "canonical":
            self.canonicals.append(attrs.get("href", ""))
        elif tag == "link" and attrs.get("rel") == "alternate" and "hreflang" in attrs:
            self.alternates.append((attrs["hreflang"], attrs.get("href", "")))
        elif tag == "title" or (tag == "script" and attrs.get("type") == "application/ld+json"):
            self._text = (tag, [])

    def handle_data(self, data):
        if self._text:
            self._text[1].append(data)

    def handle_endtag(self, tag):
        if self._text and self._text[0] == tag:
            text = "".join(self._text[1])
            if tag == "title":
                self.title = text
            else:
                self.json_ld.append(text)
            self._text = None


def rewrite(route, text):
    """`text` with the internal href of each real start tag made relative to
    the saved file of `route`; every other byte is kept."""
    # The parser counts lines by "\n" only, and columns in characters.
    line_starts = [0] + [match.end() for match in re.finditer("\n", text)]
    parts, done = [], 0
    for (line, column), tag_text, href in Links(text).tags:
        if not href.startswith("/") or href.startswith("//"):
            continue
        start = line_starts[line - 1] + column
        found = list(HREF_ATTRIBUTE.finditer(tag_text))
        if text[start : start + len(tag_text)] != tag_text or [m[1] for m in found] != [href]:
            raise CheckFailed(f"{route}: cannot rewrite the link in {tag_text}")
        target, mark, fragment = href.partition("#")
        parts += [text[done : start + found[0].start(1)], relative(route, target), mark, fragment]
        done = start + found[0].end(1)
    parts.append(text[done:])
    return "".join(parts)


def saved_as(route):
    """`/` -> index.html, `/x` -> x.html, `/ja/` -> ja/index.html, and the
    stylesheet keeps its name."""
    if route == STYLESHEET:
        return route[1:]
    if route.endswith("/"):
        return route[1:] + "index.html"
    return route[1:] + ".html"


def relative(page, route):
    base = posixpath.dirname(saved_as(page)) or "."
    return posixpath.relpath(saved_as(route), base)


def fetch(opener, base, route, referrer, want="text/html"):
    """The body of `route`, which must answer 200 with the type `want`."""
    try:
        with opener.open(base + route, timeout=30) as response:
            kind = response.headers.get("content-type", "")
            if response.status != 200 or not kind.startswith(want):
                raise CheckFailed(f"{route} answered {response.status} {kind}")
            return response.read()
    except urllib.error.HTTPError as err:
        raise CheckFailed(f"{referrer} links to {route}, which answers {err.code}") from None


def crawl(base):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    pages, queue = {}, [(route, "the start") for route in START]
    while queue:
        route, referrer = queue.pop(0)
        if route in pages:
            continue
        pages[route] = fetch(opener, base, route, referrer).decode("utf-8")
        for href in Links(pages[route]).hrefs:
            if href.startswith("/") and not href.startswith("//") and href != STYLESHEET:
                queue.append((href, route))
    return pages, fetch(opener, base, STYLESHEET, "the snapshot", want="text/css")


def save(out, pages, stylesheet):
    """Writes each page with its internal links made relative, the
    stylesheet as served, and a copy of the notices."""
    for route, text in pages.items():
        path = out / saved_as(route)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rewrite(route, text), encoding="utf-8")
    (out / "_senu").mkdir(exist_ok=True)
    (out / saved_as(STYLESHEET)).write_bytes(stylesheet)
    shutil.copyfile(ROOT / NOTICES, out / NOTICES)


def articles(content, locale):
    """The articles of `locale` under `content` as (section, slug, path),
    in the order of the contents."""
    found = []
    for path in sorted((content / locale).glob("[0-9][0-9]-*.md")):
        number, slug = int(path.name[:2]), path.name[3:-3]
        order = next(i for i, (_, numbers) in enumerate(SECTIONS) if number in numbers)
        found.append((order, number, slug, path))
    return [(SECTIONS[order][0], slug, path) for order, _, slug, path in sorted(found)]


def one_line(text):
    return " ".join(text.split())


def save_public(out, pages, base_url, content):
    """Writes the files that only a public snapshot has; see the module
    docstring. `pages` are the pages as served, with their metadata."""
    meta = {route: Links(text) for route, text in pages.items()}

    # The Markdown source of each article, next to its page.
    for locale, prefix in (("en", ""), ("ja", "ja/")):
        for _, slug, path in articles(content, locale):
            if f"/{prefix}{slug}" not in pages:
                raise CheckFailed(f"{path.name} has no page /{prefix}{slug}")
            shutil.copyfile(path, out / f"{prefix}{slug}.md")

    # sitemap.xml: every page with its language versions.
    xml.etree.ElementTree.register_namespace("", SITEMAP_NS)
    xml.etree.ElementTree.register_namespace("xhtml", XHTML_NS)
    urlset = xml.etree.ElementTree.Element(f"{{{SITEMAP_NS}}}urlset")
    for route in sorted(pages):
        if len(meta[route].canonicals) != 1:
            raise CheckFailed(f"{route} must have exactly one canonical address")
        url = xml.etree.ElementTree.SubElement(urlset, f"{{{SITEMAP_NS}}}url")
        xml.etree.ElementTree.SubElement(url, f"{{{SITEMAP_NS}}}loc").text = meta[route].canonicals[0]
        for lang, href in meta[route].alternates:
            xml.etree.ElementTree.SubElement(
                url, f"{{{XHTML_NS}}}link", rel="alternate", hreflang=lang, href=href
            )
    xml.etree.ElementTree.indent(urlset)
    sitemap = xml.etree.ElementTree.tostring(urlset, encoding="unicode", xml_declaration=True)
    (out / "sitemap.xml").write_text(sitemap + "\n", encoding="utf-8")

    (out / "robots.txt").write_text(
        f"User-agent: *\nAllow: /\n\nSitemap: {base_url}sitemap.xml\n", encoding="utf-8"
    )

    # llms.txt and llms-full.txt, from the English articles.
    lines = [
        "# Senu",
        "",
        f"> {SUMMARY}",
        "",
        "Operations are read-only and served by GET, parameters are strings, and MCP runs over "
        "stdio. Writes, MCP over HTTP, sessions and OAuth are not part of version 0.2. Senu is "
        "not audited security software. Each link below is the Markdown source of one page; "
        f"[llms-full.txt]({base_url}llms-full.txt) holds all of them in one file.",
        "",
        f"- Site: {base_url} (Japanese: {base_url}ja/)",
        "- Source: https://github.com/efg-technologies/senu",
        "- API reference: https://docs.rs/senu",
        "- Crate: https://crates.io/crates/senu",
    ]
    full = [f"# Senu: the complete guide and course\n\n> {SUMMARY}\n"]
    section = None
    for heading, slug, path in articles(content, "en"):
        page = meta[f"/{slug}"]
        if heading != section:
            lines += ["", f"## {heading}", ""]
            section = heading
        title = one_line(page.title).removesuffix(" · Senu")
        description = one_line(page.descriptions[0]) if page.descriptions else ""
        lines.append(f"- [{title}]({base_url}{slug}.md): {description}")
        source = path.read_text(encoding="utf-8").removeprefix("\ufeff").rstrip("\n")
        full.append(f"\n---\n\nSource: {base_url}{slug}.html ({heading})\n\n{source}\n")
    (out / "llms.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (out / "llms-full.txt").write_text("".join(full), encoding="utf-8")

    not_found = (
        '<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        '<meta name="robots" content="noindex">\n<title>Page not found · Senu</title>\n'
        f'<link rel="stylesheet" href="{base_url}_senu/ui.css">\n</head>\n'
        '<body class="prose mx-auto px-4 py-8 font-sans">\n<main>\n<h1>Page not found</h1>\n'
        f'<p>This address has no page. Start again from <a href="{base_url}">the Senu start page</a>'
        f' or <a href="{base_url}ja/">日本語のトップページ</a>.</p>\n</main>\n</body>\n</html>\n'
    )
    (out / "404.html").write_text(not_found, encoding="utf-8")
    (out / ".nojekyll").write_text("", encoding="utf-8")


def verify_public(out, base_url):
    """Checks the files of `save_public` against the saved pages; returns
    the number of addresses in the sitemap."""
    pages = {path: Links(path.read_text(encoding="utf-8")) for path in out.rglob("*.html")}
    pages.pop(out / "404.html")
    expected = set()
    for path, meta in pages.items():
        name = path.relative_to(out).as_posix()
        if len(meta.canonicals) != 1 or len(meta.descriptions) != 1 or len(meta.json_ld) != 1:
            raise CheckFailed(f"{name} needs one canonical, one description and one JSON-LD")
        address = base_url + name.removesuffix("index.html")
        if meta.canonicals[0] != address:
            raise CheckFailed(f"{name} names {meta.canonicals[0]}, not {address}")
        if not meta.descriptions[0].strip() or len(meta.descriptions[0]) > 160:
            raise CheckFailed(f"{name} needs a description of at most 160 characters")
        try:
            data = json.loads(meta.json_ld[0])
        except ValueError:
            raise CheckFailed(f"{name} has JSON-LD that does not parse") from None
        if data.get("@context") != "https://schema.org" or not data.get("@graph"):
            raise CheckFailed(f"{name} has JSON-LD without a schema.org graph")
        for _, href in meta.alternates:
            if not href.startswith(base_url):
                raise CheckFailed(f"{name} has an alternate outside {base_url}")
            expected.add(href)
        expected.add(address)
    addresses = {base_url + p.relative_to(out).as_posix().removesuffix("index.html") for p in pages}
    if expected != addresses:
        raise CheckFailed(f"alternates name pages that were not saved: {sorted(expected - addresses)}")
    root = xml.etree.ElementTree.parse(out / "sitemap.xml").getroot()
    listed = [loc.text for loc in root.iter(f"{{{SITEMAP_NS}}}loc")]
    if sorted(listed) != sorted(addresses):
        raise CheckFailed("sitemap.xml does not list exactly the saved pages")
    if f"Sitemap: {base_url}sitemap.xml" not in (out / "robots.txt").read_text(encoding="utf-8"):
        raise CheckFailed("robots.txt does not name the sitemap")
    llms = (out / "llms.txt").read_text(encoding="utf-8")
    links = re.findall(r"\]\((https://[^)]+\.md)\)", llms)
    if not llms.startswith("# Senu\n\n> ") or not links:
        raise CheckFailed("llms.txt needs a title, a summary and links")
    for link in links:
        if not link.startswith(base_url) or not (out / link[len(base_url):]).is_file():
            raise CheckFailed(f"llms.txt links to {link}, which was not saved")
    if len(links) != sum(1 for _ in out.glob("*.md")):
        raise CheckFailed("llms.txt does not list every English article")
    for name in ("llms-full.txt", "404.html", ".nojekyll"):
        if not (out / name).is_file():
            raise CheckFailed(f"{name} was not saved")
    return len(listed)


def verify(out, served):
    """The stylesheet equals the `served` bytes and carries both package
    notices, the notices file exists, every relative link names a saved
    file, and every fragment an id. Returns the number of pages."""
    css = out / saved_as(STYLESHEET)
    if not css.is_file():
        raise CheckFailed(f"the stylesheet {saved_as(STYLESHEET)} was not saved")
    if css.read_bytes() != served:
        raise CheckFailed("the saved stylesheet differs from the served one")
    for notice in PACKAGE_NOTICES:
        if notice not in served:
            raise CheckFailed(f"the stylesheet lacks the notice {notice.decode()}")
    if not (out / NOTICES).is_file():
        raise CheckFailed(f"the notices file {NOTICES} was not saved")
    files = {path: Links(path.read_text(encoding="utf-8")) for path in out.rglob("*.html")}
    files.pop(out / "404.html", None)
    files[css.resolve()] = Links("")
    for path, links in files.items():
        for href in links.hrefs:
            if href.startswith("https://"):
                continue
            if href.startswith("/"):
                raise CheckFailed(f"{path.relative_to(out)} still links to {href}")
            target, _, fragment = href.partition("#")
            target_path = (path.parent / target).resolve() if target else path
            if target_path not in files:
                raise CheckFailed(f"{path.relative_to(out)} links to missing {href}")
            if fragment and fragment not in files[target_path].ids:
                raise CheckFailed(f"{path.relative_to(out)} links to a missing fragment: {href}")
    return len(files) - 1


def main(argv):
    base_url = None
    if len(argv) == 3 and argv[0] == "--base-url":
        base_url, argv = argv[1], argv[2:]
    if len(argv) != 1 or argv[0].startswith("-"):
        usage = "usage: snapshot_site.py [--base-url https://host/path/] <new-or-empty-directory>"
        print(usage, file=sys.stderr)
        return 2
    out = Path(argv[0]).resolve()
    try:
        if base_url is not None and not BASE_URL.fullmatch(base_url):
            raise CheckFailed("--base-url must be an https:// address that ends in /")
        if out.exists() and (not out.is_dir() or any(out.iterdir())):
            raise CheckFailed(f"{out} must be a new or empty directory")
        env = clean_env()
        if base_url is not None:
            env["SENU_SITE_URL"] = base_url
        process, address, _, log = start_server("cargo run --quiet --locked -- site\n", ROOT, env)
        try:
            pages, stylesheet = crawl(f"http://{address}")
        finally:
            stop(process, first=signal.SIGINT)
            log.close()
        save(out, pages, stylesheet)
        count = verify(out.resolve(), stylesheet)
        extra = ""
        if base_url is not None:
            save_public(out, pages, base_url, ROOT / "content")
            listed = verify_public(out, base_url)
            extra = f", and the sitemap ({listed} addresses), llms.txt and the Markdown sources"
        print(f"ok: {count} pages, the stylesheet and {NOTICES} saved in {out}{extra}")
        return 0
    except CheckFailed as err:
        print(f"snapshot_site: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

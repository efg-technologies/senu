#!/usr/bin/env python3
"""Saves the pages of `senu site` as static files (spec 0003, CI-1).

Usage: snapshot_site.py <new-or-empty-directory>

Starts `cargo run --quiet --locked -- site` on a free loopback port in its
own process group, crawls every internal link from / and /ja/, and fails
unless each one answers 200 with HTML. Pages are saved as index.html,
<slug>.html, ja/index.html and ja/<slug>.html, and every internal href
attribute of a real start tag becomes the relative path of the saved file,
while text, code, entities and comments stay byte for byte, so that an
escaped example such as `&lt;a href="/x"&gt;` is shown as served. The
directory works from
any plain static file server on localhost, or straight from disk. Finally,
every relative href must name a saved file and every #fragment an id in
the page it points to. The server is stopped as Ctrl+C would. Nothing is
deployed anywhere. Standard library only.
"""

import html.parser
import posixpath
import re
import signal
import sys
import urllib.error
import urllib.request
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_tutorial import ROOT, CheckFailed, clean_env, start_server, stop  # noqa: E402

START = ("/", "/ja/")
HREF_ATTRIBUTE = re.compile(r'\shref="([^"]*)"')


class Links(html.parser.HTMLParser):
    """The href and id attributes of real start tags, and for each href the
    position and text of its start tag."""

    def __init__(self, text):
        super().__init__()
        self.hrefs, self.ids, self.tags = [], set(), []
        self.feed(text)
        self.close()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name == "href":
                self.hrefs.append(value)
                self.tags.append((self.getpos(), self.get_starttag_text(), value))
            elif name == "id":
                self.ids.add(value)


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
    """`/` -> index.html, `/x` -> x.html, `/ja/` -> ja/index.html."""
    if route.endswith("/"):
        return route[1:] + "index.html"
    return route[1:] + ".html"


def relative(page, route):
    base = posixpath.dirname(saved_as(page)) or "."
    return posixpath.relpath(saved_as(route), base)


def fetch(opener, base, route, referrer):
    try:
        with opener.open(base + route, timeout=30) as response:
            kind = response.headers.get("content-type", "")
            if response.status != 200 or not kind.startswith("text/html"):
                raise CheckFailed(f"{route} answered {response.status} {kind}")
            return response.read().decode("utf-8")
    except urllib.error.HTTPError as err:
        raise CheckFailed(f"{referrer} links to {route}, which answers {err.code}") from None


def crawl(base):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    pages, queue = {}, [(route, "the start") for route in START]
    while queue:
        route, referrer = queue.pop(0)
        if route in pages:
            continue
        pages[route] = fetch(opener, base, route, referrer)
        for href in Links(pages[route]).hrefs:
            if href.startswith("/") and not href.startswith("//"):
                queue.append((href, route))
    return pages


def save(out, pages):
    """Writes each page with its internal links made relative."""
    for route, text in pages.items():
        path = out / saved_as(route)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rewrite(route, text), encoding="utf-8")


def verify(out):
    """Every relative link names a saved file, and every fragment an id."""
    files = {path: Links(path.read_text(encoding="utf-8")) for path in out.rglob("*.html")}
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
    return len(files)


def main(argv):
    if len(argv) != 1:
        print("usage: snapshot_site.py <new-or-empty-directory>", file=sys.stderr)
        return 2
    out = Path(argv[0]).resolve()
    try:
        if out.exists() and (not out.is_dir() or any(out.iterdir())):
            raise CheckFailed(f"{out} must be a new or empty directory")
        process, address, _, log = start_server(
            "cargo run --quiet --locked -- site\n", ROOT, clean_env()
        )
        try:
            pages = crawl(f"http://{address}")
        finally:
            stop(process, first=signal.SIGINT)
            log.close()
        save(out, pages)
        print(f"ok: {verify(out.resolve())} pages saved in {out}")
        return 0
    except CheckFailed as err:
        print(f"snapshot_site: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

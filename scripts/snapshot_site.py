#!/usr/bin/env python3
"""Saves the pages of `senu site` as static files (spec 0003, CI-1).

Usage: snapshot_site.py <new-or-empty-directory>

Starts `cargo run --quiet --locked -- site` on a free loopback port in its
own process group, crawls every internal link from / and /ja/, and fails
unless each one answers 200 with HTML. Pages are saved as index.html,
<slug>.html, ja/index.html and ja/<slug>.html, and every internal href
becomes the relative path of the saved file, so the directory works from
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
INTERNAL_HREF = re.compile(r'href="(/[^"]*)"')


class Links(html.parser.HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.hrefs, self.ids = [], set()
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name == "href":
                self.hrefs.append(value)
            elif name == "id":
                self.ids.add(value)


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
        for route, text in pages.items():
            path = out / saved_as(route)
            path.parent.mkdir(parents=True, exist_ok=True)
            fixed = INTERNAL_HREF.sub(lambda m: f'href="{relative(route, m.group(1))}"', text)
            path.write_text(fixed, encoding="utf-8")
        print(f"ok: {verify(out.resolve())} pages saved in {out}")
        return 0
    except CheckFailed as err:
        print(f"snapshot_site: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

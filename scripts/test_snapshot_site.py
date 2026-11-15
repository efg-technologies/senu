#!/usr/bin/env python3
"""Regression tests for scripts/snapshot_site.py (spec 0003, CI-1).

They save fixed pages to a temporary directory without starting Senu, so
nothing is built and the repository is not changed.
`tests/tutorial_checker.rs` runs them as part of `cargo test`.

Usage: python3 scripts/test_snapshot_site.py
Standard library only.
"""

import json
import sys
import tempfile
import unittest
import xml.etree.ElementTree
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import snapshot_site  # noqa: E402

# What Senu serves for a code block that shows a link, next to a real link.
EXAMPLE = '<pre><code>&lt;li&gt;&lt;a href="/my-first-note"&gt;My first note&lt;/a&gt;\n</code></pre>\n'
COMMENT = '<!-- <a href="/my-first-note"> -->\n'
TEXT = 'Plain href="/my-first-note" text &amp; an entity.\n'
HEAD = '<head><link rel="stylesheet" href="/_senu/ui.css"></head>\n'
HOME = (
    '<!DOCTYPE html>\n<html lang="en">\n' + HEAD + '<body>\n'
    '<nav><a class="here" href="/" aria-current="page">Home</a>\n'
    '<a href="/my-first-note#more">My first note</a><a href="/ja/">日本語</a></nav>\n'
    + EXAMPLE + COMMENT + TEXT +
    '<p><a href="https://example.com/x">Outside</a></p>\n</body>\n</html>\n'
)
NOTE = '<!DOCTYPE html>\n<h2 id="more">More</h2>\n<a href="/">Home</a>\n'
JA = '<!DOCTYPE html>\n' + HEAD + '<a href="/my-first-note">英語</a>\n<a href="/ja/">ホーム</a>\n'
CSS = (b'/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */\n'
       b'/*! @tailwindcss/typography v0.5.20 | MIT License | x */\n.prose{color:red}')


class Save(unittest.TestCase):
    def save(self, pages, stylesheet=CSS):
        tmp = tempfile.TemporaryDirectory(prefix="snapshot-site-test-")
        self.addCleanup(tmp.cleanup)
        out = Path(tmp.name)
        snapshot_site.save(out, pages, stylesheet)
        return out

    def test_only_real_start_tag_links_become_relative_files(self):
        out = self.save({"/": HOME, "/my-first-note": NOTE, "/ja/": JA})
        home = (out / "index.html").read_text(encoding="utf-8")
        self.assertEqual(home, HOME
                         .replace('href="/_senu/ui.css"', 'href="_senu/ui.css"')
                         .replace('href="/" aria', 'href="index.html" aria')
                         .replace('href="/my-first-note#more"', 'href="my-first-note.html#more"')
                         .replace('href="/ja/"', 'href="ja/index.html"'))
        self.assertIn(EXAMPLE + COMMENT + TEXT, home)
        self.assertEqual((out / "ja/index.html").read_text(encoding="utf-8"),
                         JA.replace('"/my-first-note"', '"../my-first-note.html"')
                         .replace('"/_senu/ui.css"', '"../_senu/ui.css"')
                         .replace('"/ja/"', '"index.html"'))
        self.assertEqual((out / "_senu/ui.css").read_bytes(), CSS)
        self.assertIn("MIT License", (out / "THIRD-PARTY-NOTICES.txt").read_text(encoding="utf-8"))
        self.assertEqual(snapshot_site.verify(out, CSS), 3)

    def test_a_missing_fragment_or_page_is_refused(self):
        out = self.save({"/": HOME, "/my-first-note": NOTE.replace(' id="more"', "")})
        with self.assertRaisesRegex(snapshot_site.CheckFailed, "missing"):
            snapshot_site.verify(out, CSS)

    def test_a_missing_or_changed_stylesheet_or_notices_file_is_refused(self):
        pages = {"/": HOME, "/my-first-note": NOTE, "/ja/": JA}
        for name, problem in [("_senu/ui.css", "stylesheet"),
                              ("THIRD-PARTY-NOTICES.txt", "notices")]:
            out = self.save(pages)
            (out / name).unlink()
            with self.assertRaisesRegex(snapshot_site.CheckFailed, problem):
                snapshot_site.verify(out, CSS)
        out = self.save(pages, CSS + b" ")
        with self.assertRaisesRegex(snapshot_site.CheckFailed, "differs"):
            snapshot_site.verify(out, CSS)

    def test_a_stylesheet_without_a_package_notice_is_refused(self):
        pages = {"/": HOME, "/my-first-note": NOTE, "/ja/": JA}
        for notice in (b"/*! tailwindcss v", b"/*! @tailwindcss/typography v"):
            css = CSS.replace(notice, b"/* ")
            out = self.save(pages, css)
            with self.assertRaisesRegex(snapshot_site.CheckFailed, "notice"):
                snapshot_site.verify(out, css)


BASE = "https://example.org/senu/"


def public_page(lang, address, title, description, alternates):
    """A page as Senu serves it with SENU_SITE_URL: one canonical address,
    one description, its language versions and one JSON-LD element."""
    links = "".join(
        f'<link rel="alternate" hreflang="{code}" href="{href}">\n' for code, href in alternates
    )
    data = json.dumps({"@context": "https://schema.org", "@graph": [{"headline": title}]})
    return (
        f'<!DOCTYPE html>\n<html lang="{lang}">\n<head>\n<title>{title} · Senu</title>\n'
        f'<meta name="description" content="{description}">\n'
        f'<link rel="canonical" href="{address}">\n{links}'
        f'<link rel="icon" type="image/svg+xml" href="{BASE}icon.svg">\n'
        f'<script type="application/ld+json">{data}</script>\n'
        '<link rel="stylesheet" href="/_senu/ui.css">\n</head>\n<body><a href="/">Home</a></body>\n</html>\n'
    )


def public_pages():
    both = lambda en, ja: [("en", BASE + en), ("ja", BASE + ja), ("x-default", BASE + en)]
    return {
        "/": public_page("en", BASE, "Senu", "The start page.", both("", "ja/")),
        "/ja/": public_page("ja", BASE + "ja/", "Senu", "最初のページ。", both("", "ja/")),
        "/overview": public_page("en", BASE + "overview.html", "Overview", "What  Senu\nis.",
                                 both("overview.html", "ja/overview.html")),
        "/ja/overview": public_page("ja", BASE + "ja/overview.html", "概要", "Senu とは。",
                                    both("overview.html", "ja/overview.html")),
        "/start": public_page("en", BASE + "start.html", "Start", "The course begins.",
                              [("en", BASE + "start.html"), ("x-default", BASE + "start.html")]),
    }


class Public(unittest.TestCase):
    """The files of a public snapshot (spec 0006, §6)."""

    def save(self, pages):
        tmp = tempfile.TemporaryDirectory(prefix="snapshot-site-test-")
        self.addCleanup(tmp.cleanup)
        out, content = Path(tmp.name) / "out", Path(tmp.name) / "content"
        for name, text in (("en/01-start.md", "# Start\n\nThe course begins.\n"),
                           ("en/10-overview.md", "\ufeff# Overview\n\nWhat Senu is.\n"),
                           ("ja/10-overview.md", "# 概要\n\nSenu とは。\n")):
            (content / name).parent.mkdir(parents=True, exist_ok=True)
            (content / name).write_text(text, encoding="utf-8")
        out.mkdir()
        snapshot_site.save(out, pages, CSS)
        snapshot_site.save_public(out, pages, BASE, content)
        return out

    def test_the_sitemap_llms_files_and_sources_describe_exactly_the_saved_pages(self):
        out = self.save(public_pages())
        self.assertEqual(snapshot_site.verify(out, CSS), 5)
        self.assertEqual(snapshot_site.verify_public(out, BASE), 5)
        ns = {"s": snapshot_site.SITEMAP_NS, "x": snapshot_site.XHTML_NS}
        urls = xml.etree.ElementTree.parse(out / "sitemap.xml").getroot().findall("s:url", ns)
        self.assertEqual([url.find("s:loc", ns).text for url in urls],
                         [BASE, BASE + "ja/", BASE + "ja/overview.html",
                          BASE + "overview.html", BASE + "start.html"])
        langs = [link.get("hreflang") for link in urls[2].findall("x:link", ns)]
        self.assertEqual(langs, ["en", "ja", "x-default"])
        self.assertEqual((out / "robots.txt").read_text(encoding="utf-8"),
                         f"User-agent: *\nAllow: /\n\nSitemap: {BASE}sitemap.xml\n")
        llms = (out / "llms.txt").read_text(encoding="utf-8")
        # The guide comes before the course, and descriptions are one line.
        guide = f"## Library guide\n\n- [Overview]({BASE}overview.md): What Senu is.\n"
        course = f"## Course: learn Rust with Senu\n\n- [Start]({BASE}start.md): The course begins.\n"
        self.assertLess(llms.index(guide), llms.index(course))
        self.assertTrue(llms.startswith("# Senu\n\n> Senu is an open-source Rust library"))
        full = (out / "llms-full.txt").read_text(encoding="utf-8")
        self.assertLess(full.index("# Overview\n\nWhat Senu is."), full.index("# Start\n"))
        self.assertIn(f"Source: {BASE}overview.html (Library guide)", full)
        self.assertNotIn("\ufeff", full)
        self.assertEqual((out / "ja/overview.md").read_text(encoding="utf-8"), "# 概要\n\nSenu とは。\n")
        not_found = (out / "404.html").read_text(encoding="utf-8")
        self.assertIn('<meta name="robots" content="noindex">', not_found)
        self.assertIn(f'href="{BASE}"', not_found)
        self.assertTrue((out / ".nojekyll").is_file())
        icon = (out / "icon.svg").read_text(encoding="utf-8")
        xml.etree.ElementTree.fromstring(icon)
        self.assertIn(f'href="{BASE}icon.svg"', not_found)

    def test_an_article_without_a_page_is_refused(self):
        pages = public_pages()
        del pages["/start"]
        with self.assertRaisesRegex(snapshot_site.CheckFailed, "01-start.md has no page /start"):
            self.save(pages)

    def test_pages_without_exact_metadata_are_refused(self):
        broken = {
            "one canonical": lambda page: page.replace('<link rel="canonical"', '<link rel="x"'),
            "names": lambda page: page.replace(f'canonical" href="{BASE}', f'canonical" href="{BASE}x/'),
            "description": lambda page: page.replace('content="The start page."', 'content=" "'),
            "does not parse": lambda page: page.replace('{"@context"', '{@context'),
            "schema.org graph": lambda page: page.replace('"@graph"', '"graph"'),
            "does not name the icon": lambda page: page.replace("icon.svg", "favicon.ico"),
        }
        for message, change in broken.items():
            pages = public_pages()
            pages["/"] = change(pages["/"])
            self.assertNotEqual(pages["/"], public_pages()["/"], message)
            try:
                out = self.save(pages)
                snapshot_site.verify_public(out, BASE)
            except snapshot_site.CheckFailed as err:
                self.assertIn(message, str(err))
            else:
                self.fail(f"accepted a page that breaks: {message}")

    def test_files_that_disagree_with_the_pages_are_refused(self):
        def changed(name, old, new):
            out = self.save(public_pages())
            text = (out / name).read_text(encoding="utf-8")
            self.assertIn(old, text)
            (out / name).write_text(text.replace(old, new), encoding="utf-8")
            return out

        cases = (
            ("sitemap.xml", f"<loc>{BASE}start.html</loc>", f"<loc>{BASE}gone.html</loc>", "sitemap"),
            ("robots.txt", "Sitemap:", "Site-map:", "robots.txt"),
            ("llms.txt", f"{BASE}start.md", f"{BASE}gone.md", "llms.txt links"),
            ("llms.txt", f"- [Start]({BASE}start.md): The course begins.\n", "", "every English"),
        )
        for name, old, new, message in cases:
            with self.assertRaisesRegex(snapshot_site.CheckFailed, message):
                snapshot_site.verify_public(changed(name, old, new), BASE)
        out = self.save(public_pages())
        (out / "ja/overview.html").unlink()
        with self.assertRaisesRegex(snapshot_site.CheckFailed, "not saved"):
            snapshot_site.verify_public(out, BASE)

    def test_the_base_url_must_be_https_and_end_in_a_slash(self):
        for good in ("https://example.org/", BASE, "https://a.b-c.example:8443/x_y/z~1/"):
            self.assertTrue(snapshot_site.BASE_URL.fullmatch(good), good)
        for bad in ("http://example.org/", "https://example.org", "https://example.org/senu",
                    "https://example.org/?q/", "https://example.org/a b/", "https:///x/",
                    'https://example.org/"/', "https://example.org//"):
            self.assertFalse(snapshot_site.BASE_URL.fullmatch(bad), bad)


if __name__ == "__main__":
    unittest.main()

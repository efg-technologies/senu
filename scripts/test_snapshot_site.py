#!/usr/bin/env python3
"""Regression tests for scripts/snapshot_site.py (spec 0003, CI-1).

They save fixed pages to a temporary directory without starting Senu, so
nothing is built and the repository is not changed.
`tests/tutorial_checker.rs` runs them as part of `cargo test`.

Usage: python3 scripts/test_snapshot_site.py
Standard library only.
"""

import sys
import tempfile
import unittest
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


if __name__ == "__main__":
    unittest.main()

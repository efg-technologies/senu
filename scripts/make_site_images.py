#!/usr/bin/env python3
"""Draws the public site's images with headless Chromium (spec 0006, §6).

Usage: make_site_images.py [chromium-binary]

Writes assets/site/og.png (1200x630, the card that link previews show),
icon-512.png, icon-192.png and apple-touch-icon.png (180x180). The files are
committed; run this only to change them, and commit the result. The images
hold text and shapes only, drawn from the HTML below with system fonts, so
the output can differ slightly between machines. Standard library only.
"""

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "site"
FONT = "system-ui, -apple-system, 'Segoe UI', 'Noto Sans', sans-serif"
MARK = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="{size}" height="{size}">'
    '<path fill="none" stroke="#fff" stroke-width="7" stroke-linecap="round" '
    'd="M43 21c-3-5-9-7-14-6-6 1-9 6-8 10 2 9 23 5 23 16 0 5-5 9-12 9-6 0-10-3-12-7"/></svg>'
)
CARD = f"""<!DOCTYPE html><meta charset="utf-8"><style>
html,body{{margin:0}} body{{width:1200px;height:630px;background:#111827;color:#fff;
font-family:{FONT};display:flex;flex-direction:column;justify-content:space-between;
box-sizing:border-box;padding:64px 72px}}
.top{{display:flex;align-items:center;gap:20px;font-size:40px;font-weight:700}}
h1{{font-size:68px;line-height:1.12;margin:0;font-weight:800;letter-spacing:-1px}}
.doors{{display:flex;gap:18px}} .door{{border:2px solid #4b5563;border-radius:14px;
padding:14px 24px;font-size:30px;font-weight:600;color:#e5e7eb}}
.foot{{display:flex;justify-content:space-between;align-items:flex-end;font-size:28px;color:#9ca3af}}
code{{font-family:ui-monospace,Menlo,Consolas,monospace;color:#93c5fd}}
</style><body>
<div class="top">{MARK.format(size=64)}<span>Senu</span></div>
<h1>Declare a Rust function once.<br>Serve it three ways.</h1>
<div class="doors"><div class="door">Web page</div><div class="door">JSON API</div>
<div class="door">MCP tool for AI</div></div>
<div class="foot"><code>cargo add senu</code><span>senu.efg-technologies.com</span></div>
</body>"""
ICON = """<!DOCTYPE html><meta charset="utf-8"><style>html,body{{margin:0}}
body{{width:{size}px;height:{size}px;background:#111827;display:flex;align-items:center;
justify-content:center}}</style><body>{mark}</body>"""


def shoot(chromium, html, width, height, name):
    with tempfile.TemporaryDirectory() as tmp:
        page = Path(tmp) / "page.html"
        page.write_text(html, encoding="utf-8")
        subprocess.run(
            [chromium, "--headless=new", "--no-sandbox", "--hide-scrollbars",
             "--force-device-scale-factor=1", f"--window-size={width},{height}",
             f"--screenshot={OUT / name}", page.as_uri()],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120,
        )
    print(f"wrote assets/site/{name}")


def main(argv):
    chromium = argv[0] if argv else "chromium"
    OUT.mkdir(parents=True, exist_ok=True)
    shoot(chromium, CARD, 1200, 630, "og.png")
    # The mark fills 56% of the square, inside the safe zone of a maskable icon.
    for size, name in ((512, "icon-512.png"), (192, "icon-192.png"), (180, "apple-touch-icon.png")):
        shoot(chromium, ICON.format(size=size, mark=MARK.format(size=int(size * 0.56))), size, size, name)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

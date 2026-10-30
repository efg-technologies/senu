#!/usr/bin/env bash
# Builds the tutorial website from docs/tutorial with the pinned mdBook:
# English at the root and Japanese under ja/. Then checks it with
# scripts/check_site.py and copies it to the output directory.
#
# Usage: scripts/build_site.sh [--base-path PATH] [OUTPUT_DIR]
#
# OUTPUT_DIR defaults to target/site. It must be missing, empty, or a
# previous output of this script (marked by a .senu-site file); anything else
# is refused, so the script never deletes files it did not write. PATH is
# the URL path the site will be served from, "/" by default; it must start
# and end with "/". mdBook is $MDBOOK, or mdbook on PATH, and must be 0.5.4
# (scripts/install_mdbook.sh downloads it). The output also holds
# source-revision.txt, the commit it was built from, with "-dirty" when the
# working tree had changes.
set -euo pipefail

die() { echo "build_site: $*" >&2; exit 1; }
want="mdbook v0.5.4"
repo=$(cd "$(dirname "$0")/.." && pwd)
book=$repo/docs/tutorial
base=/
out=

while [ $# -gt 0 ]; do
  case $1 in
    --base-path) [ $# -ge 2 ] || die "--base-path needs a value"; base=$2; shift 2 ;;
    -*) die "unknown option $1 (usage: build_site.sh [--base-path PATH] [OUTPUT_DIR])" ;;
    *) [ -z "$out" ] || die "give at most one output directory"; out=$1; shift ;;
  esac
done
out=${out:-$repo/target/site}
case $base in
  /*/ | /) ;;
  *) die "--base-path must start and end with /" ;;
esac
case $base in
  *[!A-Za-z0-9/._~-]*) die "--base-path may only use letters, digits and / . _ ~ -" ;;
esac

if [ -e "$out" ] || [ -L "$out" ]; then
  [ -d "$out" ] && [ ! -L "$out" ] || die "$out exists and is not a directory"
  if [ -n "$(ls -A "$out")" ] && [ ! -f "$out/.senu-site" ]; then
    die "$out is not empty and is not an earlier build; choose another directory"
  fi
fi

for input in book.toml assets/senu.css src/SUMMARY.md src/ja/SUMMARY.md; do
  [ -f "$book/$input" ] || die "missing docs/tutorial/$input"
done
command -v python3 >/dev/null || die "python3 is required for the site check"
mdbook=${MDBOOK:-mdbook}
version=$("$mdbook" --version 2>/dev/null) ||
  die "cannot run '$mdbook'; set MDBOOK or run scripts/install_mdbook.sh"
[ "$version" = "$want" ] || die "need $want, but '$mdbook' is '$version'"

if revision=$(git -C "$repo" rev-parse --verify HEAD 2>/dev/null); then
  [ -z "$(git -C "$repo" status --porcelain)" ] || revision=$revision-dirty
else
  revision=unknown
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
MDBOOK_OUTPUT__HTML__SITE_URL=$base "$mdbook" build -d "$work/site" "$book"
MDBOOK_BOOK__SRC=src/ja MDBOOK_BOOK__LANGUAGE=ja MDBOOK_BOOK__TITLE="Senu チュートリアル" \
  MDBOOK_BOOK__DESCRIPTION="Senu を動かして変えてみる: 一つの Rust の処理を HTML、JSON、明示的に有効化する stdio の MCP で提供します。" \
  MDBOOK_OUTPUT__HTML__SITE_URL=${base}ja/ "$mdbook" build -d "$work/ja" "$book"
# The English build leaves an empty ja/ for the nested sources.
rmdir "$work/site/ja" 2>/dev/null || true
[ ! -e "$work/site/ja" ] || die "the English build wrote files under ja/"
mv "$work/ja" "$work/site/ja"
rm -f "$work/site/ja/.nojekyll"
touch "$work/site/.nojekyll"
printf '%s\n' "$revision" >"$work/site/source-revision.txt"
printf 'Built by scripts/build_site.sh; the next build may replace this directory.\n' \
  >"$work/site/.senu-site"
python3 "$repo/scripts/check_site.py" --base-path "$base" "$work/site"

mkdir -p "$out"
find "$out" -mindepth 1 -delete
cp -R "$work/site/." "$out/"
echo "build_site: wrote $out (base path $base, revision $revision)"

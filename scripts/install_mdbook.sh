#!/usr/bin/env bash
# Downloads the official mdBook 0.5.4 release for x86_64 Linux, checks its
# SHA-256 digest, and puts the `mdbook` binary in DIR (default
# target/tools/mdbook-0.5.4). Nothing is installed system-wide and no sudo is
# needed. Use it with: MDBOOK=DIR/mdbook scripts/build_site.sh
#
# Usage: scripts/install_mdbook.sh [DIR]
set -euo pipefail

die() { echo "install_mdbook: $*" >&2; exit 1; }
version=0.5.4
url=https://github.com/rust-lang/mdBook/releases/download/v$version/mdbook-v$version-x86_64-unknown-linux-gnu.tar.gz
sha256=3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7
repo=$(cd "$(dirname "$0")/.." && pwd)
dir=${1:-$repo/target/tools/mdbook-$version}

[ "$(uname -s)-$(uname -m)" = Linux-x86_64 ] || die "this script only fetches the x86_64 Linux build"
command -v sha256sum >/dev/null || die "sha256sum is required"
if [ -x "$dir/mdbook" ] && [ "$("$dir/mdbook" --version)" = "mdbook v$version" ]; then
  echo "install_mdbook: $dir/mdbook is already mdbook v$version"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL --proto '=https' --tlsv1.2 -o "$work/mdbook.tar.gz" "$url"
echo "$sha256  $work/mdbook.tar.gz" | sha256sum -c --quiet - || die "digest mismatch for $url"
mkdir "$work/unpacked"
tar -xzf "$work/mdbook.tar.gz" -C "$work/unpacked" --no-same-owner
[ "$(ls -A "$work/unpacked")" = mdbook ] && [ -f "$work/unpacked/mdbook" ] ||
  die "the archive does not hold exactly one mdbook file"
[ "$("$work/unpacked/mdbook" --version)" = "mdbook v$version" ] || die "the binary is not mdbook v$version"
mkdir -p "$dir"
install -m 0755 "$work/unpacked/mdbook" "$dir/mdbook"
echo "install_mdbook: installed $dir/mdbook"

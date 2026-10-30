<nav class="senu-lang" aria-label="Language"><a href="ja/setup.html" hreflang="ja" lang="ja">日本語</a></nav>

# Set up and run it

## Prerequisites

You need `git`, `bash`, `curl`, `python3` and the Rust toolchain manager
[rustup](https://rustup.rs/). The repository pins Rust 1.99.1 in
[`rust-toolchain.toml`](https://github.com/efg-technologies/senu/blob/main/rust-toolchain.toml),
and rustup uses that version inside the clone. If `cargo` reports that it is
not installed, run `rustup toolchain install 1.99.1`. CI runs the tutorial's
commands, except `git clone`, on GitHub-hosted Ubuntu; other systems have not
been tested.

## Clone and run the quickstart

Clone the repository and enter it. Every later command runs from this
directory.

```sh
{{#include ../commands/setup.sh:clone}}
```

Then run the quickstart:

```sh
{{#include ../commands/setup.sh:quickstart}}
```

[`scripts/quickstart.sh`](https://github.com/efg-technologies/senu/blob/main/scripts/quickstart.sh)
builds Senu, starts `senu serve` on a free loopback port with demo
credentials, calls the JSON API and the HTML page, then starts
`senu mcp --enable-mcp` and calls its tool. The first build downloads and
compiles the dependencies, so it takes a while; later runs are quick. You
should see this, with your own port number in place of `PORT`:

```text
{{#include ../commands/setup.out:quickstart}}
```

The script exits with status 0 only if every step worked. This is the whole
script:

```sh
{{#include ../../../scripts/quickstart.sh}}
```

## What to notice

All three calls search for `launch`, and all three return the same two
projects, `acme-001` and `acme-003`, in the same order. The catalogue also has
a project called "Globex Launch", but it belongs to the other tenant, so none
of the demo credentials can see it.

The name of `acme-003` is `<script>alert('x')</script> & "Launch" Notes`.
The JSON API and MCP return it as a plain string. The HTML page turns every
`<`, `>`, `&`, `'` and `"` into a character reference such as `&#60;`, so a
browser shows the text instead of running a script. Chapter 4 looks at this
more closely.

The last step does not just print the MCP reply.
[`scripts/check_mcp.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_mcp.py)
waits for each reply, checks that it answers the right request without an
error, and checks that the tool returned `acme-001` and `acme-003`.

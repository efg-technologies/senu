# Senu

[English](README.md) | [日本語](README.ja.md)

**One Rust operation, served as a web page, a JSON API and an MCP tool.**

Senu is an experimental Rust seed. A single pure function, `list_projects`,
is shared by three transports: an HTML page rendered with
[Askama](https://askama.rs/), a JSON API on [Axum](https://docs.rs/axum), and
an opt-in [MCP](https://modelcontextprotocol.io/) tool over stdio built with
the official [`rmcp`](https://rust.sdk.modelcontextprotocol.io/) SDK. Each
transport checks its own credential, picks a principal on the server side and
calls the same function, so all three return the same projects in the same
order. A test checks that.

## Status: experimental local demo

Senu is a runnable seed, not a product, and it is not audited security
software. It makes no performance or security claims.

- **Fixed demo data.** The catalogue is five projects in two tenants, built
  into the binary and held in memory. Nothing is stored or persisted, and
  there are no mutations.
- **`senu serve`** listens on a loopback address only. `/projects` uses HTTP
  Basic and `/api/projects` uses a Bearer token. There is no TLS: credentials
  travel in cleartext over loopback, where other local users can reach them.
  Basic auth here is not a production login. There are no sessions, login
  UI, OAuth, users or roles, rate limiting, request logging or audit trail.
- **`senu mcp --enable-mcp`** speaks MCP over stdio and is off unless that
  flag is given. `SENU_MCP_TOKEN` must be set before it starts. Under the MCP
  authorization spec, stdio servers take credentials from the environment, so
  this trusts the local process that launches Senu. It does not authenticate
  any remote party. There is no MCP over HTTP and no OAuth.
- **Mutations need more work.** Basic auth is an ambient browser credential,
  so any future write operation needs CSRF protection and a new spec.
- There are no macros, generators or scaffolding, and no published crate:
  run Senu from a clone of this repository.

## Quickstart

You need the Rust toolchain pinned in `rust-toolchain.toml` (for example
through rustup), `bash`, `curl` and `python3`. From a clone, run
`scripts/quickstart.sh`. This is the whole script:

```sh
#!/usr/bin/env bash
# Senu quickstart: build, call the JSON API and the HTML page, then call the
# MCP tool over stdio and check every reply. Run it from a clone of the
# repository; it needs cargo, curl and python3. The credentials are demo values
# for a loopback-only server; choose your own for anything else.
set -euo pipefail

command -v python3 >/dev/null || { echo "quickstart: python3 is required" >&2; exit 1; }
cargo build --locked --quiet
senu=./target/debug/senu
log=$(mktemp)

SENU_BIND=127.0.0.1:0 SENU_WEB_USER=demo SENU_WEB_PASSWORD=demo-password \
  SENU_API_TOKEN=demo-api-token "$senu" serve 2>"$log" &
server=$!
trap 'kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; rm -f "$log"' EXIT

url=
for _ in $(seq 100); do
  url=$(sed -n 's/^senu: listening on //p' "$log")
  [ -n "$url" ] && break
  sleep 0.1
done
[ -n "$url" ] || { cat "$log" >&2; exit 1; }

echo "== JSON API (Bearer): $url/api/projects?q=launch"
curl -fsS -H 'Authorization: Bearer demo-api-token' "$url/api/projects?q=launch"
echo
echo "== HTML page (Basic): $url/projects?q=launch"
curl -fsS -u demo:demo-password "$url/projects?q=launch" | grep 'data-project-id'

echo "== MCP over stdio: initialize, then tools/call list_projects {\"q\":\"launch\"}"
SENU_MCP_TOKEN=demo-mcp-token python3 scripts/check_mcp.py "$senu" mcp --enable-mcp
```

The MCP step runs `scripts/check_mcp.py`, a small client that uses only the
Python standard library. It waits for the `initialize` reply before calling
the tool, and it fails, killing the server, unless each reply carries the
request's id, no JSON-RPC error and no `isError`, and the call returns
acme-001 and acme-003. Any failed step makes the quickstart exit non-zero.

The HTML output shows Askama's escaping: the demo project named
`<script>alert('x')</script> & "Launch" Notes` is rendered as text.

## Commands

| Command | What it does |
|---|---|
| `senu serve` | HTML and JSON on `SENU_BIND` (default `127.0.0.1:3000`, loopback IP literals only, port 0 allowed) |
| `senu mcp --enable-mcp` | MCP over stdio until stdin closes; exits 0 on EOF |
| `senu --help` | usage |

`serve` needs `SENU_WEB_USER` (1–64 visible ASCII characters, no `:`),
`SENU_WEB_PASSWORD` and `SENU_API_TOKEN` (1–256 visible ASCII characters).
`mcp` needs `SENU_MCP_TOKEN`. All credentials must differ, and unknown
`SENU_*` variables are rejected. A configuration error exits 2 and names the
variable without printing its value; a bind failure exits 1. Tenants and
roles cannot be configured.

## Routes and the MCP tool

- `GET /projects?q=` (Basic): an HTML list with a search form.
- `GET /api/projects?q=` (Bearer): JSON `{"projects": [{"id", "name"}]}`.
- MCP tool `list_projects`: `readOnlyHint: true`, optional string `q`, no
  other fields. Results come back as `structuredContent` and as JSON text;
  operation errors are `isError` results with `{"error": code}`. An unknown
  tool gets JSON-RPC error `-32602` and a method that rmcp does not recognise
  gets `-32601`; both carry fixed messages that do not repeat the requested
  name.

`q` is trimmed, then matched case-insensitively against project names. After
trimming it may hold up to 80 Unicode scalar values and no control characters. Unknown fields are rejected.
Every other path returns 404 and every other method 405. Request bodies
(413) and targets over 2048 bytes (414) are refused, and credentials in the
query string are never read. Responses carry `Cache-Control: no-store` and
`X-Content-Type-Options: nosniff`, and errors never repeat input or secrets.

## Templates

Templates are Askama `.html` files with HTML escaping on. `askama.toml`
configures an ERB-like syntax:

```text
<%# comments %>
<%! for p in projects %>
<li data-project-id="<%= p.id %>"><%= p.name %></li>
<%! endfor %>
```

Blocks use `<%! %>`, expressions `<%= %>` and comments `<%# %>`. Plain `<%`
for blocks does not work, because Askama rejects a delimiter that is a prefix
of another. A test fails if a template or the configuration uses the `safe`
filter or turns escaping off.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -- --test-threads=2
scripts/quickstart.sh
```

The tests include `tests/quickstart_check.rs`, which runs the quickstart's MCP
checker against fake servers and needs `python3`. CI runs the same commands
on GitHub-hosted Ubuntu. Read
[the spec](docs/specs/0001-project-catalogue.md),
[the development log](docs/development.md) and
[the research notes](docs/research.md) for the details.

## License

Senu is licensed under [Apache-2.0](LICENSE).

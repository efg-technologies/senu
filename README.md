# Senu

[English](README.md) | [日本語](README.ja.md)

**Each Rust operation, served as a web page, a JSON API and an MCP tool.**

Senu is an experimental Rust seed. Each pure function, such as
`list_projects` or `get_project`, is declared once and shared by three
transports: an HTML page rendered with
[Askama](https://askama.rs/), a JSON API on [Axum](https://docs.rs/axum), and
an opt-in [MCP](https://modelcontextprotocol.io/) tool over stdio built with
the official [`rmcp`](https://rust.sdk.modelcontextprotocol.io/) SDK. Each
transport checks its own credential, picks a principal on the server side and
calls the same function, so all three return the same projects in the same
order. Tests check that. Senu also serves a small site of Markdown
articles, which holds a beginner course.

## Aims (MCS): minimal, consistent, spec-driven

- **Minimal:** application developers should own as little code,
  configuration and duplication as possible. Today a new article is one
  Markdown file, a change of look is classes in a template plus
  `npm run css`, and a new operation is a declaration plus one line per
  route and tool (see [Adding an operation](#adding-an-operation)).
- **Consistent:** one set of conventions for people and AI tools, kept by
  `rustfmt`, `clippy -D warnings` and, for the stylesheet, a CI drift gate.
  These check form; they do not prove that the business rules are right.
- **Spec-driven:** a spec in [docs/specs/](docs/specs/) comes first, then
  tests, then code; [the development log](docs/development.md) maps spec
  0004's acceptance criteria to their tests.

Rust is the foundation for its memory and type safety and its speed, but
the language alone does not make a program secure or fast. Newer tools,
such as Tailwind CSS here, are adopted where they serve these aims. Done:
operations declared once and exposed on three transports only through
explicit route and tool lists
([spec 0005](docs/specs/0005-app-developer-flow.md)), the article site and
the Tailwind UI (spec 0004). Not planned here: a library or starter kit
for other crates; adopters fork the seed.

## New to Rust? Start with the course

The beginner course is a small site that Senu serves on your own computer;
it is not published on the web. Install Rust and Git as its
[installation chapter](content/en/02-install.md) describes, then run:

```text
cd ~
git clone https://github.com/efg-technologies/senu.git
cd ~/senu
cargo run --quiet -- site
```

The first build takes several minutes. Then open http://127.0.0.1:3000/
(English) or http://127.0.0.1:3000/ja/ (Japanese). The course needs no
credentials; [Beginner course and `senu site`](#beginner-course-and-senu-site)
has the details. The rest of this README describes the transport demo.

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
- There are no generators or scaffolding, and no published crate:
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

## Beginner course and `senu site`

`senu site` serves a beginner course for people who know no Rust, written as
Markdown articles in `content/en` and `content/ja` and served by Senu itself
on a loopback address:

```text
cargo run --quiet -- site
```

Then open http://127.0.0.1:3000/ (English) or http://127.0.0.1:3000/ja/
(Japanese). The course starts with a visible page in the browser, has the
learner write notes, and then rebuild the reading-time function test first;
two optional appendices cover the catalogue, the API and MCP.

Any file named like `content/en/51-my-note.md` (two digits, a lowercase slug
and `.md`) becomes the page `/my-note` the next time `cargo run` or
`cargo build` runs: `build.rs` embeds the files, so the binary reads nothing
at run time and needs a rebuild after every change. The first line must be a
`# Title`. Raw HTML is shown as text, images are refused, and links must name
an existing page exactly or start with `https://`; a broken rule stops
startup with the file, the line and the reason. The content is trusted,
written by the repository's authors; this is not a system for untrusted
input. `senu site` takes only `SENU_BIND` and refuses every credential
variable; it has no `/projects`, `/api/*` or MCP routes.
[Spec 0003](docs/specs/0003-articles-and-beginner-tutorial.md) describes the
articles, the site and the course; it supersedes the mdBook tutorial of
spec 0002. The site is not published anywhere; CI keeps a static snapshot as
an artifact.

## Commands

| Command | What it does |
|---|---|
| `senu serve` | HTML and JSON on `SENU_BIND` (default `127.0.0.1:3000`, loopback IP literals only, port 0 allowed) |
| `senu site` | the Markdown articles as HTML on `SENU_BIND`, with no credentials |
| `senu mcp --enable-mcp` | MCP over stdio until stdin closes; exits 0 on EOF |
| `senu --help` | usage |

`serve` needs `SENU_WEB_USER` (1–64 visible ASCII characters, no `:`),
`SENU_WEB_PASSWORD` and `SENU_API_TOKEN` (1–256 visible ASCII characters).
`mcp` needs `SENU_MCP_TOKEN`. All credentials must differ, and unknown
`SENU_*` variables are rejected. A configuration error exits 2 and names the
variable without printing its value; a bind failure exits 1. Tenants and
roles cannot be configured.

## Routes and the MCP tools

- `GET /projects?q=` (Basic): an HTML list with a search form.
- `GET /api/projects?q=` (Bearer): JSON `{"projects": [{"id", "name"}]}`.
- `GET /project?id=` (Basic): one project's HTML page.
- `GET /api/project?id=` (Bearer): JSON `{"id", "name"}`.
- `GET /_senu/ui.css` (no credentials, also under `senu site`): the
  stylesheet, `text/css`, served from memory. `/_senu/` is reserved for
  Senu; every other path below it is 404.
- MCP tool `list_projects`: `readOnlyHint: true`, optional string `q`, no
  other fields. Results come back as `structuredContent` and as JSON text;
  operation errors are `isError` results with `{"error": code}`. An unknown
  tool gets JSON-RPC error `-32602` and a method that rmcp does not recognise
  gets `-32601`; both carry fixed messages that do not repeat the requested
  name.
- MCP tool `get_project`: `readOnlyHint: true`, required string `id`, no
  other fields, with the same results and errors.

`id` is required: 1 to 32 characters of `a-z`, `0-9` and `-`, matched
exactly; any other value is `invalid_id` (400). An id that is unknown or
belongs to another tenant is `not_found` (404) on every transport.
`q` is trimmed, then matched case-insensitively against project names. After
trimming it may hold up to 80 Unicode scalar values and no control characters. Unknown fields are rejected.
Every other path returns 404 and every other method 405. Request bodies
(413) and targets over 2048 bytes (414) are refused, and credentials in the
query string are never read. Responses carry `Cache-Control: no-store` and
`X-Content-Type-Options: nosniff`, and errors never repeat input or secrets.

## Adding an operation

Senu is a seed that you fork, not a library. The application is in
`src/demo/`; Senu's adapters are in `src/http.rs` and `src/mcp.rs`.

| The application owns | Senu owns |
|---|---|
| state, tenants and principals; `Permission` constants; each operation's function, argument struct, result, error codes and `Operation` declaration; views and templates; the route table and the tool list; business tests | Basic and Bearer checks, limits, the method check, the permission before the parameter envelope, error mapping, the CSP and headers, JSON Schemas from the parameters, route and tool validation at startup, and the contract tests |

1. Add `src/demo/<name>.rs` with a private function
   `fn(&S, &Principal, &Args) -> Result<T, OpError>` that decodes with
   `operation::decode` into a `#[serde(deny_unknown_fields)]` struct, and
   a `pub const OPERATION: Operation<S, T>` with its name, description,
   permission and string parameters. Errors of its own are
   `OpError::Invalid { code, phrase }` (400) or `OpError::NotFound`.
2. List it in `src/demo/mod.rs`: `.html(path, &OPERATION, view)` or
   `.api("/api/…", &OPERATION)` in `endpoints()`, and
   `mcp::tool(&OPERATION)` in `tools()`. An operation that is declared but
   not listed is not exposed.
3. Write its business tests next to it, add one succeeding sample to
   `demo_subject` in `tests/contract.rs`, which then runs CT-1 to CT-7
   over it, and list the routes and the tool in both READMEs, which
   `tests/docs_sync.rs` checks.

Limits: parameters are strings and paths are exact. Anything else goes
through `.custom(path, handler)`, which gets the size limits and response
headers but no authentication, method check or CSP; the handler does those
itself, and the contract tests do not cover it. `Operation::call` checks
the declared permission before the envelope and the envelope before the
function; any further policy inside the function runs after the envelope.
An invalid route table or tool list makes `senu serve` or `senu mcp` exit
with status 2 before it binds or reads stdin.

Migration (spec 0005, §9): routes, bodies, headers, schemas and errors are
unchanged. `Permission` was a two-variant enum and is now an open newtype.
`demo::LIST_PROJECTS` and `demo::VIEW_PROJECT` are the new names, and the
qualified `Permission::ListProjects` and `Permission::ViewProject` still work
as associated constants. An exhaustive `match` over the two old variants now
needs a `_` arm, and `use Permission::{ListProjects, ViewProject}` no longer
compiles, because associated constants cannot be imported: import
`demo::{LIST_PROJECTS, VIEW_PROJECT}` or write the qualified paths.
`OpError` and `HttpError` keep
`InvalidSearch` and `InvalidId` and gain `Invalid { code, phrase }`, so an
exhaustive `match` needs a new arm. `AppState` and `McpServer` take a state
type that defaults to `Catalogue`. `http::router`, `AppState::new`,
`with_principals`, `McpServer::new`, `with_principal`, `mcp::tools`,
`TOOL_NAME`, `GET_PROJECT_TOOL`, `operation::list_projects`,
`operation::get_project` and `Principal::demo_*` keep their paths.

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
filter or turns escaping off. The one exception is the article body, which
is `TrustedHtml` from the Markdown policy: the same test requires exactly
one `HtmlSafe` impl in `src/`, on that type. The `senu serve` pages extend
`serve_base.html`, which holds the head, the stylesheet link and the demo
banner.

## Look: Tailwind CSS and Typography

The pages use [Tailwind CSS](https://tailwindcss.com/) utility classes in
`templates/*.html` and the official Typography plugin's `prose` styles for
article text. Change the look by editing classes in a template, or theme
variables for every page in `assets/ui.css`, the only CSS source:

```css
@theme {
  --font-sans: "Trebuchet MS", "Noto Sans JP", sans-serif;
}
```

Tailwind writes rules only for the classes it finds in the templates, into
`assets/ui.generated.css`. That file is committed and built into the
binary, so Cargo alone builds and runs Senu; Node is needed only to change
the look. With Node 24 and its npm:

```sh
npm ci            # once: installs the exact versions in package-lock.json
npm run css       # after changing classes or the theme
cargo run -- site # or serve, as before
```

Commit the source together with `assets/ui.generated.css`. CI runs
`npm ci`, `npm run css` and `git diff --exit-code -- assets/ui.generated.css`,
and `cargo test` fails when a template class has no rule. Typography keeps
its own colours, so `@theme` does not change link colours inside articles;
a `prose-a:` class on `<main>` in `templates/site_base.html` does. The
licenses of both packages are in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt),
and the stylesheet keeps their notices.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -- --test-threads=2
scripts/quickstart.sh
python3 scripts/check_tutorial.py
python3 scripts/snapshot_site.py target/site
```

`tests/contract.rs` runs Senu's contract over every listed route and tool
of the demo and of a test-only second application. The tests also include
`tests/quickstart_check.rs`, which runs the quickstart's MCP
checker against fake servers, and `tests/tutorial_checker.rs`, which runs the
own tests of the tutorial checker and of the site snapshot; both need
`python3`. `check_tutorial.py` runs
the course in English and then in Japanese, each in a temporary copy of the
last commit, and leaves the working tree unchanged; commit your changes
first, because it refuses to run while tracked files differ from HEAD.
The course's optional last section runs `npm ci`, so `check_tutorial.py`
also needs Node 24, npm and access to the npm registry.
`snapshot_site.py` needs a new or empty directory; it starts `senu site`,
saves every page as a static file with relative links, the stylesheet and
the notices, and checks them. CI
runs the same commands on GitHub-hosted Ubuntu and uploads the snapshot as an
artifact; nothing is deployed. Read
[the specs](docs/specs/), [the development log](docs/development.md) and
[the research notes](docs/research.md) for the details.

## License

Senu is licensed under [Apache-2.0](LICENSE).

# Senu

[English](README.md) | [日本語](README.ja.md)

**Declare a Rust function once. Serve it as a web page, a JSON API and an MCP tool.**

[![CI](https://github.com/efg-technologies/senu/actions/workflows/ci.yml/badge.svg)](https://github.com/efg-technologies/senu/actions/workflows/ci.yml)
[Guide and course](https://senu.efg-technologies.com/) ·
[API reference](https://docs.rs/senu) ·
[Changelog](CHANGELOG.md) ·
[Security](SECURITY.md)

Senu is an open-source Rust library. You declare a read-only function once,
with its permission and parameters, and Senu serves it as an HTML page
rendered with [Askama](https://askama.rs/), a JSON API on
[Axum](https://docs.rs/axum) and an [MCP](https://modelcontextprotocol.io/)
tool for AI assistants built with the official
[`rmcp`](https://rust.sdk.modelcontextprotocol.io/) SDK. All three run the
same steps in the same order: size limits, the method, authentication, the
permission, the parameter envelope, your function, and one fixed error
mapping. The tests that prove it ship as a kit that you run over your own
routes.

```sh
cargo add senu --no-default-features
```

To build from the Git tag instead of crates.io:

```sh
cargo add senu --git https://github.com/efg-technologies/senu --tag v0.2.0 --no-default-features
```

## A complete application

This is [`examples/hello.rs`](examples/hello.rs) after its imports: one
operation behind a Bearer token, also listed as an MCP tool.

```rust
// 1. Your state. Here it is one string; in a real service it holds a
//    database pool.
struct Greeter {
    greeting: &'static str,
}

// 2. A permission, and a principal that holds it.
const GREET: Permission = Permission::new("greet");
const GUEST: Principal = Principal::new("guest", TenantId::from_static("main"), &[GREET]);

// 3. The function: state, caller and arguments in, a result or a safe
//    error out. It knows nothing about HTTP or MCP.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GreetingArgs {
    name: String,
}

#[derive(Serialize)]
struct Greeting {
    message: String,
}

fn greeting(greeter: &Greeter, _caller: &Principal, args: &Args) -> Result<Greeting, OpError> {
    let GreetingArgs { name } = decode(args)?;
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err(OpError::Invalid {
            code: "invalid_name",
            phrase: "Invalid name",
        });
    }
    Ok(Greeting {
        message: format!("{}, {name}!", greeter.greeting),
    })
}

// 4. The declaration. It exposes nothing by itself.
const GREETING: Operation<Greeter, Greeting> = Operation {
    name: "greeting",
    description: "Greets one person by name.",
    permission: GREET,
    params: &[Param::required("name", "Who to greet: 1 to 40 characters.")],
    run: Run::Sync(greeting),
};

// 5. Exposure: one JSON route and one MCP tool, both listed by hand.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = Secret::new(std::env::var("HELLO_TOKEN")?)?;
    let greeter = Greeter { greeting: "Hello" };

    if std::env::args().nth(1).as_deref() == Some("mcp") {
        // MCP over stdio: every call runs as GUEST.
        let server = McpServer::with_tools(greeter, GUEST, vec![mcp::tool(&GREETING)])?;
        return Ok(server.with_info("hello", "1.0.0").serve_stdio().await?);
    }

    // HTTP: `router` is an ordinary `axum::Router`.
    let state = AppState::builder(greeter)
        .api_auth(Bearer::new(token, GUEST))
        .build();
    let router = Endpoints::new()
        .api("/api/greeting", &GREETING)
        .into_router(state)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

```text
$ HELLO_TOKEN=choose-a-long-random-value cargo run --example hello &
$ curl -H 'Authorization: Bearer choose-a-long-random-value' 'http://127.0.0.1:3000/api/greeting?name=Ada'
{"message":"Hello, Ada!"}
```

[`examples/inventory.rs`](examples/inventory.rs) goes further: asynchronous
operations, an authenticator of its own that reads `X-Api-Key`, two
tenants, an HTML page, a stylesheet and MCP.
[`tests/example_inventory.rs`](tests/example_inventory.rs) runs Senu's
contract over it.

## What you get

- **One declaration, three transports.** `Operation` holds the name, the
  description, the permission, the string parameters and the function.
  `Endpoints` lists the routes and `mcp::tool` the tools. Declaring exposes
  nothing; only what you list is reachable.
- **One order of checks.** Authentication comes before the permission, and
  the permission before any validation, on HTML, JSON and MCP alike. A
  caller without the permission learns nothing about valid input.
- **Your authentication.** `auth::Authenticate` turns request headers into
  a server-side `Principal`. Bearer and Basic are built in; sessions, API
  keys in a database or a proxy header are an implementation of the trait.
  A route whose authenticator is missing is refused at startup.
- **Synchronous or asynchronous functions.** `Run::Sync` for work in
  memory, `Run::Async` for anything that waits.
- **Safe errors.** Every error is a fixed code and phrase, so none can
  repeat the input or a secret. `OpError::Internal` carries no detail.
- **The contract as a test kit.** `senu::contract::check` (feature
  `contract`) sends real requests through your routes and tools and checks
  seven rules, CT-1 to CT-7.
- **A plain `axum::Router`.** Bind it where you like and add tower layers
  for logging, timeouts and rate limits.

The [guide](https://senu.efg-technologies.com/overview.html) explains
each part: [operations](https://senu.efg-technologies.com/operations.html),
[authentication](https://senu.efg-technologies.com/authentication.html),
[MCP tools](https://senu.efg-technologies.com/mcp-tools.html),
[deployment](https://senu.efg-technologies.com/deployment.html),
[testing](https://senu.efg-technologies.com/testing.html) and
[questions and comparisons](https://senu.efg-technologies.com/faq.html).
AI assistants can read
[llms.txt](https://senu.efg-technologies.com/llms.txt).

## Status and limits

Version 0.2 is the first release meant for use as a library. Its API is
documented and its contract is tested, and it is honest about what it is
not:

- **Read-only.** Operations are served by `GET` and every MCP tool is
  marked read-only. There are no write operations, forms or CSRF tokens.
- **Strings and exact paths.** Parameters are strings, which an operation
  parses itself, and there are no path parameters.
- **MCP over stdio only.** A stdio server trusts the local process that
  starts it. There is no MCP over HTTP.
- **No sessions, login pages or OAuth.** They belong in your
  `Authenticate` implementation and your own routes.
- **No TLS, logging, timeouts or rate limits.** They belong in front of
  the router or in tower layers.
- **Not audited, pre-1.0.** Senu has had no independent security audit and
  makes no performance claim. A 0.x minor version may change the API;
  [CHANGELOG.md](CHANGELOG.md) lists every change.

[Spec 0006](docs/specs/0006-library-release-and-public-site.md) defines
this release.

## Features

| Feature | Default | What it adds |
|---|---|---|
| `demo` | yes | The catalogue demo, the `senu` binary and the article site with the course. A dependent turns it off with `default-features = false`. |
| `contract` | no | `senu::contract`, the test kit. Enable it under `[dev-dependencies]`. |

The rest of this README describes the demo, the course and development in
this repository.

## Aims (MCS): minimal, consistent, spec-driven

- **Minimal:** application developers should own as little code,
  configuration and duplication as possible. A new operation is a
  declaration plus one line per route and tool, a new article is one
  Markdown file, and a change of look is classes in a template plus
  `npm run css`.
- **Consistent:** one set of conventions for people and AI tools, kept by
  `rustfmt`, `clippy -D warnings`, the contract tests and, for the
  stylesheet, a CI drift gate. These check form and the adapters; they do
  not prove that your business rules are right.
- **Spec-driven:** a spec in [docs/specs/](docs/specs/) comes first, then
  tests, then code; [the development log](docs/development.md) records the
  checks that were actually run.

Rust is the foundation for its memory and type safety and its speed, but
the language alone does not make a program secure or fast.

## New to Rust? Start with the course

The beginner course is part of the site, online at
https://senu.efg-technologies.com/start.html (English) and
https://senu.efg-technologies.com/ja/start.html (Japanese). It has
you run the same site on your own computer. Install Rust and Git as its
[installation chapter](content/en/02-install.md) describes, then run:

```text
cd ~
git clone https://github.com/efg-technologies/senu.git
cd ~/senu
cargo run --quiet -- site
```

The first build takes several minutes. Then open http://127.0.0.1:3000/
(English) or http://127.0.0.1:3000/ja/ (Japanese). The course needs no
credentials; [The site and `senu site`](#the-site-and-senu-site) has the
details.

## The demo: `senu serve` and `senu mcp`

The `senu` binary is a local demo of the library, not a product.

- **Fixed demo data.** The catalogue is five projects in two tenants, built
  into the binary and held in memory. Nothing is stored, and there are no
  mutations.
- **`senu serve`** listens on a loopback address only. `/projects` uses HTTP
  Basic and `/api/projects` uses a Bearer token. There is no TLS, so the
  credentials travel in cleartext over loopback, where other local users can
  reach them.
- **`senu mcp --enable-mcp`** speaks MCP over stdio and is off unless that
  flag is given. `SENU_MCP_TOKEN` must be set before it starts. Under the MCP
  authorization spec, stdio servers take credentials from the environment, so
  this trusts the local process that launches Senu. It does not authenticate
  any remote party.

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

## The site and `senu site`

`senu site` serves the library guide and a beginner course for people who
know no Rust, written as Markdown articles in `content/en` and `content/ja`
and served by Senu itself on a loopback address:

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
spec 0002.

The public site is a static snapshot of the same pages.
`python3 scripts/snapshot_site.py --base-url https://senu.efg-technologies.com/ target/site`
starts `senu site` with `SENU_SITE_URL`, so that every page names its
canonical address, its translations and its structured data, and adds
`sitemap.xml`, `robots.txt`, `llms.txt`, `llms-full.txt`, each article's
Markdown source and `404.html`. The `pages` workflow deploys that directory
to GitHub Pages from `main`. Articles numbered 10 to 49 are the guide, 00
to 09 the course, 50 to 89 notes and 90 to 99 appendices.

## Commands

| Command | What it does |
|---|---|
| `senu serve` | HTML and JSON on `SENU_BIND` (default `127.0.0.1:3000`, loopback IP literals only, port 0 allowed) |
| `senu site` | the Markdown articles as HTML on `SENU_BIND`, with no credentials; `SENU_SITE_URL` (an `https://` address ending in `/`) adds canonical addresses and structured data |
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

In your own crate, follow [`examples/inventory.rs`](examples/inventory.rs)
and the [operations guide](https://senu.efg-technologies.com/operations.html).
In this repository the application is the demo in `src/demo/`, and Senu's
adapters are in `src/http.rs` and `src/mcp.rs`.

| The application owns | Senu owns |
|---|---|
| state, tenants and principals; the authenticators' credentials or its own `Authenticate`; `Permission` constants; each operation's function, argument struct, result, error codes and `Operation` declaration; views and templates; the route table and the tool list; business tests | the Basic and Bearer authenticators, limits, the method check, the permission before the parameter envelope, error mapping, the CSP and headers, JSON Schemas from the parameters, route and tool validation at startup, and the contract kit |

1. Add `src/demo/<name>.rs` with a private function
   `fn(&S, &Principal, &Args) -> Result<T, OpError>` that decodes with
   `operation::decode` into a `#[serde(deny_unknown_fields)]` struct, and
   a `pub const OPERATION: Operation<S, T>` with its name, description,
   permission, string parameters and `run: Run::Sync(function)`. Errors of
   its own are `OpError::Invalid { code, phrase }` (400) or
   `OpError::NotFound`.
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

Migration from 0.1 (spec 0006, §9): the demo's routes, bodies, headers,
schemas and error codes are unchanged. `run: f` becomes `run: Run::Sync(f)`.
`Operation::call` and `McpServer::call` are `async`. `Principal::tenant()`
returns `&TenantId`, which lives in `operation` and is no longer `Copy`;
`Project` is no longer `Copy` either. `OpError::InvalidSearch`,
`OpError::InvalidId` and their `HttpError` forms are associated constants
equal to `Invalid { code, phrase }`, available with the `demo` feature, so
they still work in expressions and patterns. The error enums are
`#[non_exhaustive]` and gained `Internal`, so a `match` needs a wildcard
arm. `AppState` and `McpServer` no longer default their state type to
`Catalogue`; write `McpServer<Catalogue>`. `AppState::new`,
`with_principals`, `http::router`, `McpServer::new`, `with_principal`,
`mcp::tools`, `TOOL_NAME`, `GET_PROJECT_TOOL`, `operation::list_projects`,
`operation::get_project` and `Principal::demo_*` keep their paths under
`demo`. `Permission::ListProjects` and `Permission::ViewProject` remain
associated constants, as since spec 0005.

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
filter or turns escaping off. The one exception is `TrustedHtml`, which
only `src/markdown.rs` can build: the article body from the Markdown
policy, and the JSON-LD element, whose `<`, `>` and `&` are `\u` escapes.
The same test requires exactly one `HtmlSafe` impl in `src/`, on that
type. The `senu serve` pages extend
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
cargo clippy --locked --no-default-features --lib -- -D warnings
cargo test --locked -- --test-threads=2
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
scripts/quickstart.sh
python3 scripts/check_tutorial.py
python3 scripts/snapshot_site.py target/site
```

`tests/contract.rs` runs the contract kit over every listed route and tool
of the demo and of a test-only second application, `tests/library.rs`
covers the library API, and `tests/example_inventory.rs` runs the kit over
`examples/inventory.rs`. The tests also include
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
runs the same commands on GitHub-hosted Ubuntu; the `pages` workflow builds
the public snapshot and, on `main`, deploys it. Read
[the specs](docs/specs/), [the development log](docs/development.md) and
[the research notes](docs/research.md) for the details.

## License

Senu is licensed under [Apache-2.0](LICENSE).

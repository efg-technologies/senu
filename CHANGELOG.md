# Changelog

All notable changes to Senu are recorded here. Senu follows
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version
(0.x) may change the API, and every such change is listed with its
migration.

## 0.3.0 (unreleased)

Write operations, typed arguments and session scaffolding
([spec 0007](docs/specs/0007-write-operations.md)), after the first
adopter's report. Routes, bodies, headers, schemas and codes of every read
are unchanged.

### Added

- **Write operations.** `Operation` has an `effect`: `Effect::Read`, or
  `Effect::Write { destructive, idempotent }` with the shorthands
  `Effect::CREATE`, `UPDATE` and `DELETE`. `Endpoints::api` serves a write
  by `POST` with a JSON object as its body, in a fixed order: limits, the
  method, the cross-site check, authentication, the permission, the content
  type, the body, the envelope, the function. Nothing of the body is parsed
  before the permission is known.
- **Typed parameters.** `ParamKind` (`String`, `Integer`, `Number`,
  `Boolean`) and `Param::integer`, `number` and `boolean`. The envelope and
  the MCP schema follow the kind. A read's parameters must be strings.
- **Cross-site protection.** Every write is checked with `Sec-Fetch-Site`,
  or `Origin` against `Host`, before authentication (403 `cross_origin`).
  `Endpoints::trusted_origin` admits further origins, and
  `auth::same_origin` is the same check for `custom` handlers.
- **Body limits.** 64 KiB per write by default, `Endpoints::max_body_bytes`
  and `Endpoints::body_limit`.
- **`OpError::Conflict { code, phrase }`** (409), and `HttpError::Conflict`,
  `CrossOrigin` and `UnsupportedMediaType`. `Operation::authorize` is the
  permission check alone.
- **MCP annotations by effect.** A write's tool has `readOnlyHint: false`
  and its declared `destructiveHint` and `idempotentHint`.
  `McpTool::effect` and `McpTool::params`.
- **Sessions.** `auth::cookie`, and `AppStateBuilder::html_login`, which
  sends a page request without a credential to a fixed login path with 303.
- **`Endpoints::custom_with_body`**: a route of the application's own that
  takes a body up to a limit, with Senu's response headers.
- **The contract for writes.** CT-8 to CT-12, and `Subject::auth`
  (`contract::OwnAuth`) to run every check through the application's own
  authenticators.
- **The `stdio` feature** for `McpServer::serve_stdio`. Without `demo` and
  `stdio` the core depends on neither `tokio/net` nor `axum/tokio`.
- `examples/items.rs`, and a guide page on writes in both languages.

### Changed (migration from 0.2)

- Every `Operation { .. }` needs `effect: Effect::Read` (or a write).
- A `Param { .. }` literal needs `kind: ParamKind::String`; the helpers
  `Param::required` and `Param::optional` are unchanged.
- `Authenticate::challenge` returns `Option<HeaderValue>` and defaults to
  `None`. Return `Some(..)` where you returned a value, or delete the method
  in a session authenticator.
- `contract::Subject` needs `auth: None`.
- `http::Declared` has an `effect` field.
- With `default-features = false`, `serve_stdio` needs
  `features = ["stdio"]`, and an application that calls `axum::serve` must
  enable Axum's `http1` and `tokio` features itself, as it must enable
  Tokio's `net`.
- A write's sample in the contract is a JSON object, not a query string.

### Not adopted

Path parameters, HTML form posts, nested arguments and MCP over HTTP; spec
0007, §5 gives the reasons.

## 0.2.0 (2026-11-15)

The first release meant for use as a library
([spec 0006](docs/specs/0006-library-release-and-public-site.md)). The
demo's routes, bodies, headers, schemas and error codes are unchanged.

### Added

- **A core without the demo.** `operation`, `auth`, `http` and `mcp` build
  with `default-features = false`. The catalogue demo, the `senu` binary
  and the article site moved behind the default feature `demo`.
- **Your own authentication.** `auth::Authenticate` turns request headers
  into a `Principal`. `auth::Basic` and `auth::Bearer` (with `.and` for
  more tokens) are built in, `auth::Secret::new` is public, and
  `AppState::builder(state).html_auth(a).api_auth(b).build()` attaches
  them. A route whose authenticator is missing is
  `RouteError::NoAuthenticator`.
- **Principals at run time.** `Principal::owned` and an owned
  `operation::TenantId` (`TenantId::new`, `TenantId::from_static`).
- **Asynchronous operations.** `Run::Async` beside `Run::Sync`;
  `Operation::admit` exposes the permission and envelope checks.
- **`OpError::Internal`** (500, `internal`), with no detail on any
  transport. `OpError` implements `Display` and `Error`.
- **`senu::contract`** (feature `contract`): CT-1 to CT-7 as a kit for an
  application's own tests.
- **Assets and the page CSP.** `Endpoints::asset`, `Endpoints::html_csp`,
  `Endpoints::csp` and `http::DEFAULT_HTML_CSP`.
- **MCP.** `McpServer::serve_stdio`, `McpServer::with_info` and
  `mcp::ServeError`.
- `Param::required` and `Param::optional`.
- `examples/hello.rs` and `examples/inventory.rs`.
- **The site.** A library guide in English and Japanese, sections in the
  contents, a description on every page, and with `SENU_SITE_URL` a
  canonical address, `hreflang` alternates, Open Graph tags and JSON-LD.
  `scripts/snapshot_site.py --base-url` also writes `sitemap.xml`,
  `robots.txt`, `llms.txt`, `llms-full.txt`, the Markdown sources and
  `404.html`, and the `pages` workflow deploys the snapshot.

### Changed (migration from 0.1)

- `run: f` in an `Operation` becomes `run: Run::Sync(f)`.
- `Operation::call` and `McpServer::call` are `async`.
- `Principal::tenant()` returns `&TenantId`; `TenantId` moved to
  `operation` (re-exported from `catalogue`) and is no longer `Copy`, and
  `catalogue::Project` is no longer `Copy`. `Principal::name()` returns
  `&str`.
- `OpError::InvalidSearch`, `OpError::InvalidId`, `HttpError::InvalidSearch`
  and `HttpError::InvalidId` are associated constants equal to
  `Invalid { code, phrase }`, available with `demo`. They still work in
  expressions and patterns, with the same codes and phrases on the wire.
- `OpError`, `HttpError`, `RouteError`, `ToolError` and `http::Kind` are
  `#[non_exhaustive]`; a `match` over them needs a wildcard arm.
- `AppState` and `McpServer` no longer default their state type to
  `Catalogue`.
- `AppState::new`, `AppState::with_principals`, `http::router`,
  `http::site_router`, `McpServer::new`, `McpServer::with_principal`,
  `mcp::tools`, `mcp::TOOL_NAME`, `mcp::GET_PROJECT_TOOL` and the demo
  re-exports in `operation` need the `demo` feature, which is on by
  default.
- `http::single_authorization` moved to `auth::single_authorization`.
- The site's start page describes the library; its `<h1>` and the site
  name changed, and the footer no longer calls Senu an experimental seed.

## 0.1.0

The seed: `list_projects` and `get_project` over a fixed catalogue, served
as HTML, JSON and MCP, with the article site and the beginner course
(specs 0001 to 0005). It was not published as a crate.

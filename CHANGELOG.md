# Changelog

All notable changes to Senu are recorded here. Senu follows
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version
(0.x) may change the API, and every such change is listed with its
migration.

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

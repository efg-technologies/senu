# Senu Spec 0006: The Library Release, the Public Site and Discoverability

Status: implemented, 2026-11-15 (evidence in `docs/development.md`), except the publication on crates.io in REL-1, which needs a registry token. It amends specs 0001, 0003 and 0005 where §9 says so, and it withdraws their non-goals "a library", "publishing" and "the site is not published". Evidence belongs in `docs/development.md`, CI runs and PRs, not in this file.

## 1. Goal and boundaries

- **Goal:** turn the seed into a crate that another crate can depend on for real services, publish its documentation as a public web site, and make both easy to find and to quote correctly for search engines and AI assistants.
- **What "usable in production" means here:** an external crate can declare operations over its own state, authenticate callers its own way, run asynchronous work, serve the result behind its own listener, and run Senu's contract over its own routes; the public API is documented and versioned; known limits are written down. It does not mean audited. Senu still makes no performance claim, and no claim about search ranking: §7 lists what is built and what is measured.
- **Non-goals of 0.2:** write operations (POST, forms, CSRF), MCP over HTTP, sessions, login pages and OAuth, typed or path parameters, pagination helpers, storage, generators, a workspace split, and a general Markdown site library. Each needs its own spec. Operations stay read-only and parameters stay strings.

## 2. Findings at fa1b96c

1. `Principal` holds `catalogue::TenantId`, so the core names the demo, and its name and permissions are `&'static`, so a principal cannot come from a database.
2. `AppState` takes `config::ServeConfig` (one Basic pair, one Bearer token, one principal each). An application cannot bring sessions, API keys per client or a proxy header.
3. `Operation::run` is a synchronous function pointer and the adapters call it on the async executor, so an operation cannot wait for a database without blocking a worker.
4. `OpError` has no failure for "the backend is down", and carries the demo's `InvalidSearch` and `InvalidId`.
5. HTML pages can only load `/_senu/ui.css`, whose rules cover Senu's own templates, and the CSP is fixed.
6. `tests/contract.rs` is generic but lives in Senu's test tree, so an adopter cannot run it.
7. `Cargo.toml` says `publish = false`; there is no changelog, security policy or docs.rs metadata; the crate-level documentation is four lines.
8. The site has no description, canonical URL, alternate-language links, structured data, sitemap, `robots.txt` or `llms.txt`, its only content is the course, and CI uploads the snapshot as an artifact without deploying it.

## 3. The crate

- **One package, three layers.** The core (`operation`, `auth`, `http`, `mcp`) builds with `--no-default-features` and names nothing from the demo. The feature `demo` (default) adds the catalogue application, the `senu` binary, the article site and their modules (`app`, `catalogue`, `cli`, `config`, `content`, `demo`, `discover`, `markdown`, `reading`, `site`, `view`), so the course's commands stay as they are. The feature `contract` adds `senu::contract` (§5). A dependent writes `senu = { version = "0.2", default-features = false }`. The binary has `required-features = ["demo"]`, and without `demo` the build script embeds nothing.
- **Principals.** `operation::TenantId` wraps a `Cow<'static, str>` (`TenantId::from_static`, `TenantId::new`); `catalogue::TenantId` re-exports it and keeps `ACME` and `GLOBEX`. `Principal::new` stays `const` over static data, `Principal::owned(name, tenant, permissions)` builds one at run time, and `tenant()` returns `&TenantId`. `Permission` is unchanged: permissions are constants of the application, and a stored role maps to them in application code.
- **Operations.** `Operation::run` becomes `Run<S, T>`: `Run::Sync(fn)` as before, or `Run::Async(fn)` returning `OpFuture<'a, T>` (a boxed `Send` future). `Operation::call` is `async` and keeps the order: permission, envelope, function. `Operation::admit` exposes the first two steps. `Param::required` and `Param::optional` are `const` helpers. A synchronous operation must not block; blocking work belongs in an asynchronous operation that uses `spawn_blocking` or an async client.
- **Errors.** `OpError` and `HttpError` gain `Internal` (500, `internal`, "Internal Server Error"; an `isError` tool result over MCP), lose the variants `InvalidSearch` and `InvalidId`, and become `#[non_exhaustive]`, as do `RouteError` and `ToolError`. Under `demo` the two old names remain as associated constants equal to `Invalid { code, phrase }` with the same codes and phrases, so every byte on the wire is unchanged.
- **Authentication.** `auth::Authenticate` is the one extension point: `authenticate(&HeaderMap) -> AuthFuture<'_>` yields `Option<Principal>`, and `challenge()` gives the `WWW-Authenticate` value of the 401. Only headers are passed, so a credential in the query string still cannot be read. `auth::Basic::new(user, password, principal)` and `auth::Bearer::new(token, principal)`, with `.and(token, principal)` for more tokens, are the built-in implementations; they accept exactly one `Authorization` header and compare in constant time for equal lengths, and `Bearer` compares every token without stopping early. `auth::Secret::new` is public and validates 1 to 256 visible ASCII characters.
- **HTTP.** `AppState::builder(state).html_auth(a).api_auth(b).build()` replaces the configuration-bound constructors, which stay under `demo` (`AppState::new`, `with_principals`). A table with an HTML or API route whose authenticator is missing is `RouteError::NoAuthenticator(path)`: it fails closed before binding. `Endpoints::asset(path, content_type, bytes)` serves a public, static, GET-only file, and `Endpoints::html_csp(policy)` replaces the page CSP; the default stays as it is. `into_router` returns a plain `axum::Router`, so the application chooses the listener, TLS termination, logging, rate limits and timeouts with ordinary tower layers. Senu itself still never listens anywhere but where the application binds, and `senu serve` stays loopback-only.
- **MCP.** `McpServer::call` is `async`. `McpServer::serve_stdio()` runs until stdin closes and returns `Ok(())` on EOF, so an application needs no direct `rmcp` dependency.
- **Hygiene.** `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]` with `cargo doc` under `-D warnings`, a crate-level guide with a compiled example, `examples/` that build in CI, `CHANGELOG.md`, `SECURITY.md`, docs.rs metadata (`all-features`), keywords and categories, and an `include` list. `rust-version` stays the pinned toolchain unless an older one is actually tested.

## 4. Security invariants

Every invariant of spec 0005, §6 holds. In addition: an operation route never runs without an authenticator; a principal comes only from an `Authenticate` implementation or from server code; the built-in authenticators never log or repeat a credential; `Internal` carries no detail; assets and the stylesheet hold no data and read no credential; a custom CSP is the application's responsibility and the contract checks the default only when it is in use.

## 5. The contract as a test kit

`senu::contract::Subject<S>` holds the application's `state`, `endpoints`, `tools`, a `principal` with every listed permission and one succeeding sample query per operation. `senu::contract::check(subject).await` builds routers with its own Basic and Bearer credentials, with the subject's principal and with a principal without permissions, and runs CT-1 to CT-7 of spec 0005 over every listed route and tool; a violation panics with the route or tool in the message. `tests/contract.rs` runs it over the demo and over the test-only `Stock` application, and an example shows it over an application outside the crate tree.

## 6. The public site

- **Content.** The site gains a second group of articles, the library guide (overview and quickstart, operations, authentication, MCP, deployment and limits, testing with the contract, comparison and FAQ). An article's number gives its section: 10 to 49 the guide, 00 to 09 the course, 50 to 89 notes, 90 to 99 appendices. The contents and the pager follow that order, and the index, which now introduces the library, lists each section under its heading. English is canonical and Japanese mirrors it, as before. Guide code is compiled: each article names an example file or a checked block.
- **Base URL.** `senu site` accepts `SENU_SITE_URL`, an absolute `https://` URL ending in `/`. Without it nothing changes. With it every page carries a canonical URL, `hreflang` alternates (`en`, `ja`, `x-default`), Open Graph and Twitter card tags and JSON-LD. Canonical addresses follow the snapshot's file names (`/install` is `install.html`, `/ja/` is `ja/`), because the public site is the snapshot. `senu serve` and `senu mcp` refuse the variable.
- **Description.** Every page has a `<meta name="description">`: the article's first paragraph as plain text, cut at a word or character boundary to at most 160 characters, and the index's intro.
- **Structured data.** JSON-LD is built by `serde_json` and emitted in one `<script type="application/ld+json">` whose `<`, `>` and `&` are `\u`-escaped. `TrustedHtml::json_ld` in `src/markdown.rs` builds the element, so `TrustedHtml` stays the crate's only `HtmlSafe` impl and only that module can construct one. The index describes a `SoftwareSourceCode`; an article is a `TechArticle` with a `BreadcrumbList`; the FAQ article also lists its questions as a `FAQPage`, taken from its headings.
- **Snapshot.** `snapshot_site.py --base-url URL` also writes `sitemap.xml` (every page with its alternates), `robots.txt` (allow all, with the sitemap), `llms.txt` (the project in one paragraph, then every English guide and course page with its description), `llms-full.txt` (the English Markdown sources in order), each article's Markdown source next to its page as `<slug>.md`, `404.html` and `icon.svg`, which every page names so that a browser does not ask the host for `/favicon.ico`. Each article links its Markdown source, and the index links `llms.txt`. Links stay relative, so the snapshot still works from disk.
- **Deployment.** A `pages` workflow builds the snapshot from `main` and deploys it to GitHub Pages; pull requests only build it. The canonical host is the Pages URL until a custom domain is configured in the workflow.

## 7. Discoverability: what is built and what is measured

Built: one clear sentence of positioning repeated verbatim in the crate description, both READMEs, the site index and `llms.txt`; question-shaped headings with a direct answer in the first sentence; comparison and FAQ pages that name the alternatives fairly; stable anchors; the metadata of §6; crates.io keywords and categories; the repository description, homepage and topics. Measured and recorded: validity of the sitemap and the JSON-LD (parsed in tests), that every page has exactly one canonical and one description, Lighthouse SEO and accessibility scores of the deployed pages, and the HTTP status of every sitemap URL after deployment. Not claimed: any ranking, traffic or citation by an assistant. Those can only be observed over weeks in Search Console and similar tools, which need the owner's account.

## 8. Tests

Existing tests keep passing with the renames of §9. New: core-only build (`cargo check --no-default-features`, clippy on it), authenticator unit tests (single header, constant-time path, many tokens, challenge), fail-closed route tables, an asynchronous operation through all three transports, `Internal` mapping, assets and CSP override, `serve_stdio` EOF, the contract kit over `Stock` with `Run::Async`, description and JSON-LD extraction (escaping included), `SENU_SITE_URL` validation, the snapshot's new files (parsed as XML and JSON), and `cargo package` listing only the intended files.

## 9. Compatibility and migration from 0.1

Routes, bodies, headers, schemas and error codes of the demo are unchanged. Source changes: `run: f` becomes `run: Run::Sync(f)`; `Operation::call` and `McpServer::call` are awaited; `Principal::tenant()` returns a reference and `TenantId` is no longer `Copy`; `OpError::InvalidSearch` and `InvalidId` are constants, not variants, and only under `demo`; matches over the error enums need a wildcard arm; `Project` is no longer `Copy`; `AppState` and `McpServer` no longer default their state type to `Catalogue`. `AppState::new`, `with_principals`, `http::router`, `McpServer::new`, `with_principal`, `mcp::tools`, `TOOL_NAME`, `GET_PROJECT_TOOL` and the `operation::{list_projects, get_project}` re-exports keep their paths under `demo`.

## 10. Acceptance criteria

- **LIB-1** The core builds, lints and documents without `demo`; an example outside `src/` declares an asynchronous operation with a custom authenticator and passes `contract::check`.
- **SEC-1** §4 holds; CT-1 to CT-7 pass for the demo, `Stock` and the example; the escaping guard allows exactly the two trusted types.
- **EQV-1** Every pre-existing test passes with only the §9 renames.
- **PKG-1** `cargo publish --dry-run` succeeds and the package holds only the listed files; `cargo doc --all-features` has no warning.
- **SITE-1** With a base URL, every snapshot page has one canonical, one description, alternates and valid JSON-LD; the sitemap lists exactly the pages; without a base URL no page names an absolute address of its own.
- **PUB-1** The site is deployed and every sitemap URL answers 200; the scores of §7 are recorded.
- **DOC-1** READMEs, AGENTS.md, the changelog and the guide describe the library, its limits and the migration; the course still passes in both languages.
- **REL-1** The version is 0.2.0, tagged, with release notes. Publishing to crates.io needs a registry token; if none is available the dry run and the exact command are recorded instead.

## 11. Delivery

1. `spec:` this file.
2. `core:` features, `TenantId`, owned principals, errors (EQV-1 in part).
3. `ops:` `Run`, async `call`, async MCP.
4. `auth:` `Authenticate`, the built-ins, `AppState::builder`, assets and CSP.
5. `contract:` the kit, the example, `serve_stdio`.
6. `pkg:` metadata, documentation, changelog, security policy.
7. `site:` descriptions, base URL, JSON-LD, snapshot files.
8. `content:` the guide in both languages.
9. `ci:` the Pages workflow and the release notes; then deployment and the measurements.

## 12. Risks and known limits

- A default-on `demo` feature means a dependent that forgets `default-features = false` compiles the demo modules. They add no dependency, and the guide's first command avoids them.
- Read-only operations with string parameters cover dashboards, catalogues and tools for assistants, not a whole application. The guide says so before the quickstart.
- The built-in authenticators hold static secrets. Sessions, rotation and lockout belong to the application's `Authenticate`.
- A public `axum::Router` and `askama::Error` tie Senu's major version to theirs.
- Pages cannot send response headers, so the public snapshot has no CSP; it holds no forms, scripts or credentials.

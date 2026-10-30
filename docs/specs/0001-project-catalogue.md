# Senu Spec 0001: Project Catalogue Vertical Slice

Status: implementation-ready, 2026-10-30, for the public repository `efg-technologies/senu` at `docs/specs/0001-project-catalogue.md`. Test evidence belongs in CI runs and PRs linked from Linear. It does not belong in this file.

Rust on Senu serves web pages, a JSON API, and MCP from one shared application logic. This experimental application/framework seed is not audited security software and makes no claims about performance or security. Senu has no macros, generators, or scaffolding, and the docs must not suggest otherwise.

## 1. Boundaries

- **In scope:** a read-only `list_projects` operation over a fixed in-memory catalogue, and one binary `senu` with two modes: `serve` (HTML and JSON on loopback) and opt-in `mcp` (stdio).
- **Out of scope:** sessions, OAuth, login UI, users/roles, ORM, persistence, mutations, a CSRF subsystem, TLS, MCP over HTTP, rate limiting, request logging, and performance targets. Any future mutation needs browser CSRF protection, because Basic auth is an ambient credential, and it needs a new spec.
- Both READMEs must state this status, the CSRF rule, and the demo limits. There is no TLS. Credentials travel in cleartext over loopback, where other local users can reach them. Basic auth here is not a production login.

## 2. Core

`list_projects(&Catalogue, &Principal, &serde_json::Map) -> Result<ProjectList, OpError>` is pure: it does no I/O and reads no clock or globals. It runs these steps in order:

1. If the principal lacks `Permission::ListProjects` (the only variant), return `Forbidden`.
2. Decode the arguments with `deny_unknown_fields`. The only accepted field is an optional string `q`. Anything else returns `InvalidRequest`.
3. Apply `q.trim()`. An empty result means no search. Any `char::is_control` character, or more than 80 `char`s, returns `InvalidSearch`. No Unicode normalization is applied.
4. Search only within `Catalogue::for_tenant(principal.tenant)`.
5. Keep projects whose lowercased name contains `q.to_lowercase()`, then sort by `id`.

`ProjectList { projects: [{ id, name }] }` exposes no tenant, totals, or hints. Only server code creates principals. There are three: `demo-web`, `demo-api`, and `demo-mcp`, each in tenant `acme` with `ListProjects`. Tests also use `globex` principals and principals with no permissions.

| id | tenant | name |
|---|---|---|
| acme-001 | acme | Apollo Launch |
| acme-002 | acme | Café Roadmap |
| acme-003 | acme | `<script>alert('x')</script> & "Launch" Notes` |
| globex-001 | globex | Globex Launch |
| globex-002 | globex | Zephyr Migration |

## 3. Binary and adapters

The CLI parser is a pure function. It accepts `senu serve`, `senu mcp --enable-mcp`, and `senu --help`. Anything else exits 2. A bare `senu mcp` explains that MCP is off by default. Configuration is validated before the server binds a port or reads stdin. A configuration failure exits 2 and names the variable without printing its value. A bind failure exits 1.

| Variable | Mode | Rule |
|---|---|---|
| `SENU_BIND` | serve, optional | default `127.0.0.1:3000`; must be a loopback IP literal; port 0 allowed |
| `SENU_WEB_USER` | serve | 1–64 VCHAR, no `:` |
| `SENU_WEB_PASSWORD`, `SENU_API_TOKEN` | serve | 1–256 VCHAR |
| `SENU_MCP_TOKEN` | mcp | 1–256 VCHAR |

VCHAR means bytes 0x21–0x7E. All credential values that are present must differ from one another. Unknown `SENU_*` names are rejected. Roles and tenants cannot be configured.

HTTP runs Axum 0.8 on a current-thread Tokio runtime. Stderr receives only `senu: listening on http://<addr>` and fatal errors.

- `GET /projects?q=` accepts Basic auth only. It returns an HTML list, a GET search form, and a "Local demo, not production" banner.
- `GET /api/projects?q=` accepts Bearer `SENU_API_TOKEN` only. It returns JSON `ProjectList`.
- Each request passes through limits, then routing, then authentication, then `list_projects`. Query pairs are parsed with `form_urlencoded` into the argument map, and repeated keys become arrays. Credentials in the query string are never read.
- Auth schemes match case-insensitively. Secrets are compared with `subtle::ConstantTimeEq`, and no timing guarantee is claimed. The 401 challenge is `Basic realm="senu", charset="UTF-8"` or `Bearer realm="senu"`.
- A path plus query longer than 2048 bytes returns 414. A `Content-Length` above 0 or any `Transfer-Encoding` returns 413. An unknown path returns 404, and any other method returns 405.
- Errors under `/api/*` are JSON `{"error": code}`, where `code` is one of `unauthorized`, `forbidden`, `invalid_request`, `invalid_search`, `not_found`, `method_not_allowed`, `uri_too_long`, `payload_too_large`, or `internal`. Other paths return fixed `text/plain` reason phrases. No error repeats the input, any credential, or Rust error text.
- Every response sets `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.
- HTML comes from Askama `.html` templates with escaping left on. The default syntax in `askama.toml` uses `<% %>` for blocks, `<%= %>` for expressions, and `<%# %>` for comments. Each item renders as `<li data-project-id="<%= p.id %>"><%= p.name %></li>`. A valid `q` appears only in `value="<%= q %>"`. No `safe` filter or escape override is allowed.

MCP uses the official `rmcp` crate with features `server` and `transport-io`, and a hand-written `ServerHandler`. There are no tool macros and no reflection. Under the MCP authorization spec, stdio servers take credentials from the environment. Senu validates `SENU_MCP_TOKEN` on its own, separate from the HTTP credentials, and binds `demo-mcp`. This does not authenticate any remote party. The server advertises only the tools capability. `tools/list` returns exactly one tool, `list_projects`, with `readOnlyHint: true` and an object input schema: optional string `q`, `maxLength` 80, `additionalProperties: false`. A successful call returns the `ProjectList` both as `structuredContent` and as JSON text. An operation error returns `isError: true` with `{"error": code}`. An unknown tool gets JSON-RPC error `-32602`. Stdout carries only JSON-RPC, and the process exits 0 when stdin reaches EOF.

Pin Axum 0.8.x, plus the current versions of Tokio, serde, serde_json, Askama, rmcp, base64, subtle, and form_urlencoded, each with minimal features. Commit `Cargo.lock`, pin the installed stable toolchain, and use edition 2024.

## 4. Acceptance criteria

- **OP-1** A principal with no permissions gets `Forbidden`, even when the request has unknown fields or an 81-character `q`.
- **OP-2** Unknown fields (`tenant`, `permission`, `token`) or a non-string `q` return `InvalidRequest`.
- **OP-3** A `q` of 80 multibyte scalars passes and 81 fails. `"a\u{7}b"` fails. `"  launch\n"` behaves like `launch`, and a whitespace-only `q` behaves like no search.
- **OP-4** For acme: no search returns 001, 002, 003; `launch` returns 001, 003; `CAFÉ` returns 002; `zephyr` returns nothing. For globex: `launch` returns globex-001.
- **OP-5** Search reads only the output of `for_tenant`. This is checked by code review plus OP-4.
- **CFG-1** Each missing, empty, malformed, duplicated, or unknown variable makes the process exit 2. The message names the variable, prints no value, and no listening line appears. Running `mcp` with only `SENU_API_TOKEN` set also exits 2.
- **CFG-2** `SENU_BIND` set to `0.0.0.0:3000` or `[::]:3000` exits 2. An occupied port exits 1.
- **CFG-3** These exit 2: `senu` with no arguments, `mcp` without `--enable-mcp`, `serve --enable-mcp`, and unknown arguments. `--help` exits 0. `GET /mcp` returns 404.
- **WEB-1** Missing, wrong, or cross-transport credentials return 401 with the matching challenge. Cross-transport cases include a Bearer or MCP token on `/projects`, Basic or the MCP token on `/api/projects`, and `?access_token=`.
- **WEB-2** On both routes: a principal with no permissions (injected in tests) gets 403; an 81-character `q` gets 400 `invalid_search`; `?tenant=globex` gets 400 `invalid_request`. Limits, 404, 405, and headers behave as §3 states.
- **WEB-3** The HTML contains no raw `<script>`, and the `acme-003` item text contains no raw `<`, `>` or `"` yet decodes back to the catalogue name. A search of `q=<b>"x"` is escaped inside `value`. A test fails if any template or configuration uses a `safe` filter or an escape override. (Corrected during implementation: Askama 0.16.1 escapes with numeric references such as `&#60;script&#62;`, so the earlier literal `&lt;script&gt;` check could only pass with the escape override this criterion forbids. Tests decode entities instead of matching one spelling.)
- **WEB-4** Sentinel secrets never appear in response bodies, headers, stdout, or stderr.
- **MCP-1** Over real stdio, the following all behave as specified, in order: `initialize` with rmcp's `ProtocolVersion::LATEST`, `notifications/initialized`, a `tools/list` returning one tool, a successful `tools/call` (`launch` returns acme-001 and acme-003), an `invalid_search` call, an `invalid_request` call (`tenant` field), and an unknown tool returning `-32602`. EOF then exits 0.
- **MCP-2** Every stdout line is a JSON-RPC 2.0 message. When `SENU_MCP_TOKEN` is missing, nothing is written to stdout.
- **MCP-3** Calling the in-process handler with a principal that has no permissions returns `isError` with `forbidden`.
- **X-1** For each `q` in {none, `launch`, `" CAFÉ "`, `zephyr`}, three outputs equal the direct `list_projects` output: the API `projects`, MCP `structuredContent.projects`, and the HTML items (ids in order, names unescaped).
- **DOC-1** `README.md`, `README.ja.md`, `LICENSE`, `docs/research.md`, and this spec exist. Every advertised command and route has a test.
- **DOC-2** Both READMEs embed `scripts/quickstart.sh` byte-for-byte, and a test checks this. CI runs the script and it succeeds.
- **DOC-3** `docs/research.md` records the following:
  - pinned versions and the sources inspected on 2026-10-30: https://docs.rs/axum/latest/axum/ (showing 0.8.9), https://askama.rs/en/stable/configuration.html, https://rust.sdk.modelcontextprotocol.io/, https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization, and https://doc.rust-lang.org/book/ch11-03-test-organization.html;
  - Loco commit `0fcc2a312d4b43a9972afdf969980d9ceb646cf2`, with the files reviewed (`src/controller/mod.rs`, `skills/loco/recipes/testing.md`) and its license (Apache-2.0, copyright Dotan Nahum and Elad Kaplan);
  - the concepts borrowed: explicit route composition, public-safe error mapping, and focused request tests, with no code copied;
  - that upstream notices must be kept if any code is adapted later, and that Loco does not endorse Senu.
- **CI-1** GitHub Actions run with `contents: read` and SHA-pinned actions. They run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked -- --test-threads=2` with `CARGO_BUILD_JOBS=2`, and the quickstart. No external services are used.

## 5. Test pyramid

There must be more unit tests than adapter tests, and more adapter tests than E2E tests.

- **Unit tests** live in `src/` and cover the core, the catalogue, the CLI parser, configuration, and auth parsing.
- **Adapter tests** live in `tests/adapter_*.rs`. They call the Axum `Router` in-process with `oneshot`, call the MCP handler directly, and run the parity check (X-1).
- **E2E tests** live in `tests/e2e_*.rs`, with at most six test functions. They spawn the real binary with a cleared environment and `127.0.0.1:0`. They send raw HTTP/1.1 over `TcpStream`, exchange newline-delimited JSON-RPC over stdio, and use 10 s timeouts. They cover API and HTML success, 401, 413, 414, WEB-4, and CFG-1/CFG-2 as a table-driven test.

## 6. Red-green commits

Each commit begins with its failing test, and the commit message notes that it was red first. The commit then adds the minimal code to pass and keeps CI green.

1. Scaffold: package, toolchain, lockfile, CI, LICENSE.
2. Domain types and catalogue invariants.
3. OP-1.
4. OP-2.
5. OP-3.
6. OP-4 and OP-5.
7. CLI parser.
8. Configuration.
9. API adapter.
10. Askama syntax spike.
11. HTML adapter.
12. Limits and headers.
13. `serve` wiring, with HTTP and startup E2E tests.
14. In-process MCP handler.
15. MCP stdio E2E test.
16. Parity (X-1).
17. READMEs, research notes, quickstart script, sync test, and CI job.

## 7. Decisions and risks

- **D1: license.** Senu is licensed under Apache-2.0. This is the ordinary project license chosen for this wholly new public repository, and the full text is in `LICENSE`.
- **R1:** Askama may reject `<%` because it is a prefix of `<%=`. If step 10 fails for that reason, record the error and the final syntax in `docs/research.md`. Then use `<%! %>` for blocks, keeping the expression syntax and escaping unchanged. If that also fails, stop and report.
- **R2:** If the pinned `rmcp` API differs from what §3 assumes, keep the behaviour §3 specifies and record the difference.

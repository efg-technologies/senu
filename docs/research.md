# Research notes

These notes record the versions Senu pins, the public sources reviewed for
[spec 0001](specs/0001-project-catalogue.md) on 2026-10-30, and where the
installed libraries differed from what the spec assumed. They contain no
copied source code.

## Pinned versions

Toolchain: Rust 1.99.1 (`rust-toolchain.toml`), edition 2024.

| Crate | Locked version | Features |
|---|---|---|
| axum | 0.8.9 | `http1`, `json`, `tokio` |
| tokio | 1.54.1 | `net`, `rt` (tests add `macros`) |
| serde | 1.0.229 | `derive` |
| serde_json | 1.0.154 | default |
| askama | 0.16.1 | `config`, `derive`, `std` |
| rmcp | 3.8.0 | `server`, `transport-io` |
| base64 | 0.23.1 | `std` |
| subtle | 2.6.1 | none |
| form_urlencoded | 1.2.2 | default |
| tower (tests only) | 0.5.3 | `util` |

`Cargo.lock` is committed and every command runs with `--locked`.

## Library and protocol sources

- Axum: https://docs.rs/axum/latest/axum/ (showing 0.8.9). Explicit routes,
  `Router::layer` middleware and `axum::serve` on a Tokio listener.
- Askama configuration: https://askama.rs/en/stable/configuration.html.
  Custom syntaxes in `askama.toml` and default HTML escaping.
- MCP Rust SDK: https://rust.sdk.modelcontextprotocol.io/. During
  implementation the installed `rmcp` 3.8.0 source was read for the
  `ServerHandler` trait, `ProtocolVersion`, `CallToolResult`, the stdio
  transport and the service shutdown path.
- MCP authorization: https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization.
  It covers HTTP transports; stdio implementations should take credentials
  from the environment instead. Senu therefore checks `SENU_MCP_TOKEN` at
  startup and trusts the local process that launched it. That is not remote
  authentication.
- Rust test organization: https://doc.rust-lang.org/book/ch11-03-test-organization.html.
  Unit tests in `src/`, integration tests in `tests/` with a shared
  `tests/support/mod.rs`.

## Corrections found while implementing

### Askama delimiters and escaping (spec risk R1, WEB-3)

- The spec first asked for `<% %>` blocks next to `<%= %>` expressions.
  Askama 0.16.1 refuses that at compile time: "an opening delimiter may not be
  the prefix of another delimiter. The block delimiter ("<%") clashes with the
  expression delimiter ("<%=")". Following R1, blocks use `<%! %>`.
  Expressions stay `<%= %>` and comments `<%# %>`.
- Askama 0.16.1 escapes with numeric character references (`&#60;`, `&#62;`,
  `&#34;`, `&#39;`, `&#38;`), not `&lt;`/`&gt;`. A literal `&lt;script&gt;`
  check could only pass by overriding the escaper, which WEB-3 forbids, so
  WEB-3 now asks that no raw `<`, `>` or `"` remain and that decoding the
  entities gives back the catalogue name.

### rmcp 3.8.0 (spec risk R2)

- `ProtocolVersion::LATEST` is `2026-07-28`. That revision replaced the
  `initialize` handshake with per-request `_meta`. A server that receives
  `initialize` naming it answers with
  `ProtocolVersion::LATEST_WITH_INITIALIZE` (`2025-11-25`), the newest revision
  that still has a handshake. The MCP-1 E2E test sends `initialize` with
  `LATEST` as the spec says and asserts the `2025-11-25` answer. The
  quickstart asks for `2025-11-25` directly.
- `ServerHandler::call_tool` returns `CallToolResponse` (complete,
  input-required or task). Senu always returns a complete result.
  `CallToolResult::structured` and `structured_error` put the same JSON in
  `structuredContent` and in one text block.
- `ServerHandler::get_info` returns `ServerConfig`, an alias of
  `InitializeResult`.
- After stdin reaches EOF, rmcp drains in-flight handler responses for up to
  5 seconds before closing. Senu's handler answers immediately, so a client
  that writes its requests and closes stdin still gets every reply. EOF before
  `initialize` ends initialization with `ConnectionClosed`, which Senu treats
  as a clean exit 0.
- rmcp offers tool macros. Senu does not use them; the one tool is declared
  by hand in `src/mcp.rs`.
- The tool schema's `maxLength: 80` counts code points of the untrimmed
  string, while the operation trims first. A client that enforces the schema
  may reject a padded search that the server would accept. The server's
  validation is the authority.

### HTTP review fixes

- HTML `maxlength` counts UTF-16 code units of the untrimmed value
  (https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#attr-fe-maxlength,
  https://infra.spec.whatwg.org/#string-length), unlike the operation's
  trimmed scalar count. The search input has no client-side length rule.
- `Content-Length` is `1*DIGIT`
  (https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6), and hyper keeps
  the raw spelling, so `00` and `000` are now treated as an empty body.

## Loco (concepts only)

[Loco](https://github.com/loco-rs/loco) was reviewed at commit
`0fcc2a312d4b43a9972afdf969980d9ceb646cf2`:

- [`src/controller/mod.rs`](https://github.com/loco-rs/loco/blob/0fcc2a312d4b43a9972afdf969980d9ceb646cf2/src/controller/mod.rs)
- [`src/controller/views/engine.rs`](https://github.com/loco-rs/loco/blob/0fcc2a312d4b43a9972afdf969980d9ceb646cf2/src/controller/views/engine.rs)
- [`skills/loco/recipes/testing.md`](https://github.com/loco-rs/loco/blob/0fcc2a312d4b43a9972afdf969980d9ceb646cf2/skills/loco/recipes/testing.md)

Loco is licensed under Apache-2.0, copyright Dotan Nahum and Elad Kaplan.

Senu borrows concepts only: explicit route composition, mapping internal
failures to a small set of public-safe errors, keeping template rendering
behind a narrow view function, and focused in-process request tests. No Loco
code was copied or adapted. If any Loco code is adapted later, its upstream
copyright and license notices must be kept with it. Loco does not endorse
Senu.

## Process and testing tools reviewed but not used

- GitHub Spec Kit: https://github.github.com/spec-kit/quickstart.html and
  https://github.com/github/spec-kit/blob/main/spec-driven.md. These informed
  the written-spec-first, acceptance-criteria-driven workflow. Spec Kit itself
  was not installed or run.
- Proptest: https://proptest-rs.github.io/. Not installed or used. Property
  tests may suit search validation later; this slice uses example-based
  tests over a fixed catalogue.
- cargo-nextest: https://nexte.st/. Not installed or used.

The first slice is small and deterministic, so standard `cargo test` covers
it. No speed or security claims are made for either choice.

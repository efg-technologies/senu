# Development log

This file records how the first Senu slice was built against
[spec 0001](specs/0001-project-catalogue.md), and the red-green evidence that
spec §6 asks for. Each entry lists the command, its exit status, and the test
counts or assertion messages that mattered. Commands run from the repository
root.

Local runs used a resource-bounded wrapper (a single build job with capped
memory and CPU) so the build would not disturb other work on the same machine.
CI runs the same `cargo` commands without that wrapper.

Spec §6 lists 17 red-green steps. The owner asked for a small number of
coherent commits, so the steps are grouped below. Every group that adds
behaviour was run red against stubs before the implementation was written,
and the output quoted here is the real output of those runs.

## Commit 1: scaffold (§6 step 1)

Package, pinned toolchain (Rust 1.99.1, edition 2024), lockfile, CI workflow
and the Apache-2.0 `LICENSE`. There is no behaviour yet, so there is no red
phase.

The resolver locked the current releases: axum 0.8.9, tokio 1.54.1,
serde 1.0.229, serde_json 1.0.154, askama 0.16.1, rmcp 3.8.0, base64 0.23.1,
subtle 2.6.1 and form_urlencoded 1.2.2, plus tower 0.5.3 for tests.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 0 tests |

## Commit 2: catalogue and `list_projects` (§6 steps 2–6)

Tests: `src/catalogue.rs` (catalogue invariants) and `src/operation.rs`
(OP-1 to OP-5). OP-5 is also enforced by construction: the search reads only
`Catalogue::for_tenant(principal.tenant())`.

Red: the stubs returned an empty catalogue, ignored the tenant, and made
`list_projects` return an empty list for every input.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --lib` (stubs) | 101 | 8 passed, 19 failed |
| `cargo test --locked --lib` (implementation) | 0 | 27 passed |

Representative red assertions:

- `op1_principal_without_permission_is_forbidden`: `left: Ok(ProjectList { projects: [] })`, expected `Err(Forbidden)`.
- `op3_control_characters_fail`: `q = "a\u{7}b"`, `left: Ok(ProjectList { projects: [] })`.
- `demo_catalogue_matches_the_spec_table`: `left: []`.

The eight tests that passed against the stubs compare two empty results (for
example "whitespace-only search equals no search") or check constants. They
pass for the right reason only once the catalogue is populated, which the
green run confirms.

## Commit 3: CLI parser, configuration and auth parsing (§6 steps 7–8)

Tests: `src/cli.rs` (CFG-3 argument forms), `src/config.rs` (CFG-1 and the
`SENU_BIND` half of CFG-2) and `src/auth.rs` (Basic and Bearer parsing,
constant-time comparison, redacted `Debug`). All are pure unit tests over
argument lists, environment pairs and header bytes.

Red: `parse_args` always returned `Help`, both config functions always
returned "SENU_BIND missing", and the header parser returned `None`.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --lib` (stubs) | 101 | 34 passed, 31 failed (auth 7, cli 7, config 17) |
| `cargo test --locked --lib` (implementation) | 0 | 65 passed |

Representative red assertions:

- `cli::accepts_the_three_documented_forms`: `left: Ok(Help)`, `right: Ok(Serve)`.
- `auth::parses_basic_credentials`: `left: None`, `right: Some(Basic(<redacted>))`.
- `config::valid_mcp_configuration_needs_only_the_mcp_token`: panicked on `expect("valid configuration")`.

Design notes:

- Every present `SENU_*` variable is validated in both modes, so a reused or
  malformed credential is never ignored. Unknown and repeated names are
  rejected. The mode then decides which variables are required.
- The distinctness check covers `SENU_WEB_USER`, `SENU_WEB_PASSWORD`,
  `SENU_API_TOKEN` and `SENU_MCP_TOKEN`.
- Bearer tokens may contain any visible ASCII byte, because configured tokens
  may. Header values over 1024 bytes are not parsed.

## Commit 4: JSON API, Askama syntax, HTML page, limits and headers (§6 steps 9–12)

Tests: `src/view.rs` (template rendering), `src/http.rs` (query decoding,
error mapping, body and header helpers) and `tests/adapter_http.rs`
(WEB-1, WEB-2, WEB-3 and the adapter half of WEB-4, called in-process with
`tower::ServiceExt::oneshot`).

### Step 10: Askama syntax spike (spec risk R1)

`askama.toml` first declared the spec's syntax, `<% %>` for blocks and
`<%= %>` for expressions. `cargo build --locked --lib` exited 101 with:

```text
error: an opening delimiter may not be the prefix of another delimiter. The block delimiter ("<%") clashes with the expression delimiter ("<%=")
```

Following R1, blocks moved to `<%! %>` while expressions (`<%= %>`) and
comments (`<%# %>`) stayed unchanged. The same build then exited 0.

### Red and green

Red: `router` returned 501 for every request, and the template was still the
spike (no banner, title or label).

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast --lib --test adapter_http` (stubs) | 101 | lib: 69 passed, 3 failed; adapter_http: 1 passed, 17 failed |
| same command (implementation) | 0 | lib: 81 passed; adapter_http: 18 passed |

Representative red assertions:

- `api_lists_projects_for_a_valid_bearer_token`: `left: 501`, `right: 200`.
- `templates_and_configuration_never_disable_escaping`: the spike template's
  comment contained the word "escapes", which the scan forbids. The comment was
  reworded; the scan stayed strict.
- `view::markup_in_project_names_is_escaped`: the rendered item was
  `&#60;script&#62;alert(&#39;x&#39;)&#60;/script&#62; &#38; &#34;Launch&#34; Notes`.
  Askama 0.16.1 escapes with numeric references, so the literal
  `&lt;script&gt;` check in WEB-3 could only pass with an escape override,
  which WEB-3 forbids. The spec wording of WEB-3 was corrected (same ID), and
  the tests now assert that no raw `<`, `>` or `"` remains and that decoding
  the entities returns the catalogue name.

The one adapter test that passed against the stub
(`responses_never_contain_credentials_or_input`) checks for the absence of
secrets, which an empty 501 response trivially satisfies; it is meaningful
only with the real router.

Design notes:

- Routes use `any(handler)` and each handler checks `GET` first, so the order
  is explicit: limits, then routing (404 or 405), then authentication, then
  `list_projects`. `HEAD` is answered with 405 as the spec says ("any other
  method"); every 405 carries `Allow: GET`.
- The limits and header middleware are attached with `Router::layer` after
  the routes and fallback, so they wrap every route and the 404 fallback.
- The HTML response also sends a restrictive `Content-Security-Policy`
  (no scripts, inline styles only). This is extra hardening, not a spec item.

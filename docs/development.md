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

## Commit 5: `serve` wiring and HTTP/startup E2E tests (§6 step 13)

Code: `src/app.rs` runs the parsed command with an explicit argument list and
environment, validates configuration before binding, and maps failures to
exit status 2 (arguments or configuration) or 1 (runtime, such as an
occupied port). `src/main.rs` only forwards `std::env` to it. MCP mode still
exits 1 with "not available yet" in this commit.

Tests: `tests/e2e_serve.rs` (three functions) with process helpers in
`tests/support/mod.rs`. They spawn the built binary with a cleared
environment and `127.0.0.1:0`, send raw HTTP/1.1 over `TcpStream`, and use
10 s timeouts; every child is killed and reaped on drop. Covered: API and
HTML success, 401 with the matching challenge, 413, 414, `GET /mcp` 404,
stderr holding only the listening line, sentinel secrets absent from every
response and stream (WEB-4), CFG-1/CFG-2/CFG-3 as one table-driven test, and
an occupied port exiting 1.

Evidence: no red run of these process tests was recorded before the wiring
was written, so this entry has green evidence only. The binary was
previously a stub that printed "not implemented yet" and exited 2.

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean (after `cargo fmt`) |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings, after replacing `[line.clone()]` with `std::slice::from_ref(&line)` (`clippy::cloned_ref_to_slice_refs`) |
| `cargo test --locked -- --test-threads=2` | 0 | lib 81, adapter_http 18, e2e_serve 3 passed |

## Commit 6: independent review fixes

An independent read-only review of the code through commit 5 reported two P2
findings. Both regression tests were added first and run red.

1. `templates/projects.html` set `maxlength="80"` on the search input.
   Browsers count that limit in UTF-16 code units of the untrimmed value
   ([HTML maxlength](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#attr-fe-maxlength),
   [Infra string length](https://infra.spec.whatwg.org/#string-length)),
   while `list_projects` counts Unicode scalars after trimming. The attribute
   was removed; the server stays the only length check.
2. `has_body` treated only the literal `Content-Length: 0` as empty, so an
   authenticated `GET` with `00` or `000` returned 413. `Content-Length` is
   `1*DIGIT` ([RFC 9110 §8.6](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6)),
   and hyper keeps the raw spelling. Any all-zero decimal value now means no
   body; positive, empty and malformed values still return 413.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast -- --test-threads=2` (tests only) | 101 | lib 81 passed, 2 failed; adapter_http 18 passed, 1 failed; e2e_serve 2 passed, 1 failed |
| same command (fix) | 0 | lib 83, adapter_http 19, e2e_serve 3 passed |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |

Red assertions:

- `http::tests::body_detection_accepts_any_decimal_spelling_of_zero`: `["00"]` was treated as a body.
- `view::tests::search_input_leaves_length_rules_to_the_server`: the input tag contained `maxlength`.
- `html_accepts_searches_a_browser_maxlength_would_block`: 80 emoji with surrounding spaces returned 200 and echoed the trimmed value, but the page still carried `maxlength`.
- `serve_answers_api_and_html_requests_over_a_real_listener`: `Content-Length: 00: {"error":"payload_too_large"}`, `left: 413`, `right: 200`.

## Commit 7: MCP handler and stdio E2E tests (§6 steps 14–15)

Code: `src/mcp.rs` declares one tool, `list_projects`, by hand: a
`rmcp::model::Tool` with `readOnlyHint: true` and an object input schema
(optional string `q`, `maxLength` 80 until commit 10, `additionalProperties: false`). A
hand-written `ServerHandler` answers `initialize` (tools capability only),
`tools/list` and `tools/call`; no tool macros or reflection are used. Calls go
through the same pure `list_projects` with the server-bound `demo-mcp`
principal. `src/app.rs` validates `SENU_MCP_TOKEN` before reading stdin, then
serves on `rmcp::transport::io::stdio()`. EOF, including EOF before
`initialize`, exits 0.

Tests: `tests/adapter_mcp.rs` (8 functions: the one declared tool and its
schema, tools-only capability, structured plus JSON text result, missing
arguments, MCP-3 forbidden principal before validation, invalid search and
unknown fields as `isError` results, tenant from the server principal, unknown
tool as `-32602`) and `tests/e2e_mcp.rs` (2 functions: the MCP-1 sequence over
real stdio with MCP-2 stdout checks, and EOF/missing-token startup).

Red: `McpServer::call` returned an internal error, `tools()` returned no tools,
`info()` returned the rmcp default, and MCP mode still exited 1.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast --test adapter_mcp --test adapter_parity --test e2e_mcp` (stubs) | 101 | adapter_mcp 0 passed, 8 failed; adapter_parity 0 passed, 1 failed; e2e_mcp 0 passed, 2 failed |
| same command (implementation) | 0 | adapter_mcp 8, adapter_parity 1, e2e_mcp 2 passed |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |

Representative red failures: `exactly_one_explicit_read_only_tool_is_declared`
failed on `tools.len()`; the stdio session test found stdout closed before the
`initialize` reply; the EOF test saw exit status 1 instead of 0.

Differences from the spec's assumptions (risk R2):

- rmcp 3.8.0 has `ProtocolVersion::LATEST` = `2026-07-28`, a revision that
  replaced the `initialize` handshake with per-request `_meta`. The E2E test
  still sends `initialize` with `LATEST`, as MCP-1 says, and rmcp's
  negotiation answers with `ProtocolVersion::LATEST_WITH_INITIALIZE`
  (`2025-11-25`), the newest revision that has a handshake. The test asserts
  that answer.
- `tools/call` returns `CallToolResponse` (complete, input-required or task);
  Senu always returns the complete variant.
- The schema's `maxLength` counted code points of the untrimmed string, while
  the operation trims first, so the schema rejected padded searches the
  server accepts. Commit 10 removed it (see below).

## Commit 8: three-transport parity (§6 step 16)

Test: `tests/adapter_parity.rs`, one table-driven function over the X-1
queries (none, `launch`, `" CAFÉ "`, `zephyr`). For each query it pins the
expected ids of the direct `list_projects` call, so equal-but-empty results
cannot pass by accident, then compares `(id, name)` pairs in order from the
JSON API, the MCP `structuredContent` and the HTML items after entity
decoding.

Red: the test was written together with the MCP tests and run against the
same stubs (see commit 7). It failed at the MCP call for the first query,
after the API and HTML comparisons for that query had passed. It passed once
the MCP handler was implemented; no parity-specific code was needed.

## Commit 9: READMEs, research notes, quickstart, sync test and CI job (§6 step 17)

Files: `README.md`, `README.ja.md`, `docs/research.md`,
`scripts/quickstart.sh`, `tests/docs_sync.rs`, and a `Quickstart` step at
the end of the CI job. Spec decision D1 is back to its original wording: the
Apache-2.0 license is provisional until the owner confirms it before merge.

The quickstart builds the binary, starts `senu serve` on `127.0.0.1:0`, reads
the listening line, calls the JSON API with Bearer and the HTML page with
Basic, then pipes `initialize`, `notifications/initialized` and one
`tools/call` into `senu mcp --enable-mcp`. The server is killed and reaped on
exit. Piping all requests and then closing stdin is reliable because rmcp
drains in-flight responses after EOF (see `docs/research.md`); 20 repeated
runs of that MCP pipeline against the built binary all returned both replies
and exited 0. Commit 12 replaced the pipe with a client that checks each
reply (see below).

`tests/docs_sync.rs` (DOC-2) was written after the READMEs, so it had no red
phase. As a check that it can fail, one line was appended to the script
temporarily: the test failed with "README.md must embed
scripts/quickstart.sh exactly once, byte for byte", and passed again once the
script was restored.

Final checks on this commit:

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 117 passed: lib 83, adapter_http 19, adapter_mcp 8, adapter_parity 1, docs_sync 1, e2e_mcp 2, e2e_serve 3 |
| `scripts/quickstart.sh` | 0 | API JSON, two escaped HTML items, MCP `tools/call` reply; run on the final script and code, before the README wording edits |

Test pyramid (spec §5): 83 unit tests, 28 adapter tests and 5 E2E test
functions (at most six allowed). `docs_sync` checks documentation and is not
part of the pyramid.

## Commit 10: MCP schema length rule (second independent review)

A second independent review of the completed slice reported three P2
findings. This commit fixes the first: the `q` property declared
`maxLength: 80`, which JSON Schema applies to the raw string, while
`list_projects` trims first. The real binary accepted 80 spaces followed by
`launch` (86 code points, 6 after trimming) and returned acme-001 and
acme-003, yet the declared schema rejected that input.

The schema now has no length keyword. The `q` description states the rule
(trimmed first, then at most 80 Unicode scalar values and no control
characters), and the operation remains the only check. The spec's §3 schema
wording and `docs/research.md` were updated; acceptance IDs are unchanged.

Tests, written first:

- `tests/adapter_mcp.rs`: the declaration test now requires no `maxLength`
  and a description that states the trimmed 80-scalar rule. The new
  `schema_admits_padded_searches_that_the_operation_accepts` checks the
  declared `q` schema with a small evaluator that fails on any keyword it does
  not understand, for 80 spaces plus `launch` and for 80 `é` padded with a
  space and a newline, then checks that each padded call returns the same
  result as the trimmed search.
- `tests/adapter_parity.rs`: X-1 gains the padded `launch` search, so the
  API, the HTML page and MCP are compared on it too.
- `tests/e2e_mcp.rs`: inside the existing stdio session, `tools/list` must
  declare no `maxLength`, and a padded `launch` call (id 7) must return the
  same result as `launch`.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast --test adapter_mcp --test adapter_parity --test e2e_mcp` (tests only) | 101 | adapter_mcp 7 passed, 2 failed; adapter_parity 1 passed; e2e_mcp 1 passed, 1 failed |
| same command (fix) | 0 | adapter_mcp 9, adapter_parity 1, e2e_mcp 2 passed |

Red assertions:

- `schema_admits_padded_searches_that_the_operation_accepts`: "the schema
  rejects" the 80-space `launch` search, "which the operation accepts".
- `exactly_one_explicit_read_only_tool_is_declared`: the `q` schema still had
  `maxLength`.
- `stdio_session_lists_and_calls_the_one_tool_then_exits_on_eof`: the
  `tools/list` reply still declared `maxLength`.

The new parity case passed before the fix: the server already accepted the
padded search, and only the declared schema disagreed.

## Commit 11: fixed method-not-found reply (second independent review)

The review's second finding: `McpServer` inherited rmcp's default
`ServerHandler::on_custom_request`, which answers an unrouted method with
`-32601` and the method name as the error message. After a proper handshake,
a request whose method was a configured credential sentinel came back on
stdout verbatim. The caller supplied that value, so this is not extraction of
a server secret, but it broke the rule that replies do not repeat input or
credentials.

`McpServer` now overrides `on_custom_request` with a fixed `-32601` "method
not found" and no `data`. rmcp still routes and answers every method it
knows, and the handshake is unchanged. The spec gains one §3 sentence and an
MCP-2 clause (same IDs); `docs/research.md` and both READMEs mention the
reply.

Test, written first: inside the existing stdio session of
`tests/e2e_mcp.rs`, request id 8 uses `SENU_MCP_TOKEN`'s sentinel as the
method and in its params. The reply must be `-32601` with the fixed message,
no `result`, no `data`, and no sentinel. The session now expects 8 replies.
The E2E test-function count stays at 5.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast --test e2e_mcp` (test only) | 101 | 1 passed, 1 failed: "the error repeats the method name or params" (the `-32601` code check before it passed) |
| `cargo test --locked --no-fail-fast --test e2e_mcp --test adapter_mcp` (fix) | 0 | e2e_mcp 2, adapter_mcp 9 passed |

## Commit 12: quickstart checks every MCP reply (second independent review)

The review's third finding: the quickstart piped three messages into
`senu mcp --enable-mcp` and printed the last stdout line with `tail -n 1`, so
a JSON-RPC error or an `isError` result still exited 0. Reproduced before the
fix with the same pipeline and only the tool name changed: it printed
`{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"unknown tool"}}`
and exited 0 under `pipefail`.

`scripts/check_mcp.py` is a small client that uses only the Python standard
library. It sends `initialize` and waits for the reply, which must report a
protocol version and the tools capability. Only then does it send
`notifications/initialized` and the `tools/call`, and wait for that reply.
Each reply must be the next stdout line, a JSON-RPC 2.0 response with the
request's id and a result rather than an error. The call must not have
`isError` set and must list exactly acme-001 and acme-003. After that, stdin
is closed, and the server must exit 0 without further stdout. Every wait is
bounded (10 s by default, `--timeout` for tests), and on any failure the
child is killed and reaped and the checker exits 1 with the reason on stderr,
printing no reply. The quickstart calls it as its last command under
`set -euo pipefail`, checks for `python3` first, and still kills and reaps the
HTTP server on exit. Both READMEs embed the new script byte for byte, list
`python3` as a prerequisite and describe the check. CI prints
`python3 --version` before the tests. The change is about validation: no
pipelining race was observed, and none is claimed.

Tests, written first: `tests/quickstart_check.rs` runs the checker against
a fake `sh` server that prints canned replies. One function checks a
well-formed session (exit 0, only the call reply on stdout). A table of 13
cases checks malformed JSON, a non-2.0 reply, an `initialize` error, a wrong
`initialize` id, a missing tools capability, EOF before each reply, a
`tools/call` JSON-RPC error, a wrong call id, `isError`, the wrong projects,
extra output and a non-zero exit. Each must exit 1, print nothing on stdout
and name its reason on stderr. A third function gives a silent server a 1 s
timeout and then checks that its process no longer exists. Like `docs_sync`,
these tests check a script and are not part of the test pyramid.

| Command | Exit | Result |
|---|---|---|
| `cargo test --locked --no-fail-fast --test quickstart_check` (tests only) | 101 | 0 passed, 3 failed: the checker did not exist yet (`python3: can't open file .../scripts/check_mcp.py`) |
| same command (checker) | 0 | 3 passed |

That red run shows only that the tests need the checker; the `tail -n 1`
reproduction above is the evidence for the original defect. Two more
mutation runs used a temporary copy of the checker whose call named an
unknown tool. Against the built binary it exited 1 with
"request 2 failed with a JSON-RPC error". A temporary copy of the quickstart
that used it also exited 1, with no `senu` process left. The unmodified
checker against the same binary printed the call reply and exited 0.

## Final checks after the second review fixes

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 121 passed: lib 83, adapter_http 19, adapter_mcp 9, adapter_parity 1, docs_sync 1, e2e_mcp 2, e2e_serve 3, quickstart_check 3 |
| `scripts/quickstart.sh` | 0 | API JSON, two escaped HTML items, and the checked MCP call reply; no `senu` process left |

Test pyramid (spec §5): 83 unit tests, 29 adapter tests and 5 E2E test
functions (at most six allowed). `docs_sync` and `quickstart_check` check
documentation and a script and are not part of the pyramid. CI has not run
these commits; publication is still waiting for the owner's license decision
(D1).

## Publication license

On 2026-10-30 the project owner selected Apache-2.0 and authorized publication.
The earlier entries record the provisional decision as it stood during
implementation. Both READMEs and spec decision D1 now state the selected
license. The `LICENSE` text is unchanged. The README language links point to
the English and Japanese versions. No application or quickstart code changed.

## Tutorial and documentation site (spec 0002)

Files: `docs/specs/0002-tutorial.md`, the mdBook sources in `docs/tutorial`
(English in `src/`, Japanese in `src/ja/`), shared command snippets and
expected output in `docs/tutorial/commands` and `docs/tutorial/exercise`,
`scripts/check_tutorial.py`, `scripts/build_site.sh`, `scripts/check_site.py`,
`scripts/install_mdbook.sh`, `tests/site_scripts.rs`, a new test in
`tests/docs_sync.rs`, two CI steps and a `site` CI job. No product source or
product test changed, and the catalogue still names `acme-001` "Apollo
Launch".

Every expected output in the tutorial was copied from a real run of its
command, and `check_tutorial.py` compares them on each run. Before writing the
exercise, every literal "Apollo" in the repository was listed. Two are
maintained contract fixtures in `tests/adapter_http.rs`: the expected JSON body
at line 132 and the search `apollo+launch` at line 301 (lowercase; there is no
mixed-case spelling). The exercise's third patch updates both. The fixtures in
`src/view.rs` and `tests/quickstart_check.rs` build their own data and stay
unchanged.

### Exercise red and green

`python3 scripts/check_tutorial.py exercise` ran every exercise step in a
temporary git copy that reused a copy of `target/debug`, and exited 0. The
red steps failed on test assertions; these are the matched lines it printed:

```text
ok: steps.sh:red (exit 101)
    test catalogue::tests::demo_catalogue_matches_the_spec_table ... FAILED
    assertion `left == right` failed
      left: [("acme-001", "acme", "Apollo Launch"), ("acme-002", "acme", "Café Roadmap"), ...
     right: [("acme-001", "acme", "Artemis Launch"), ("acme-002", "acme", "Café Roadmap"), ...
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 82 filtered out; finished in 0.00s
ok: steps.sh:suite-red (exit 101)
    failures:
        api_lists_projects_for_a_valid_bearer_token
        query_strings_are_percent_and_plus_decoded
    test result: FAILED. 17 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    error: test failed, to rerun pass `--test adapter_http`
```

The green steps passed: the single catalogue test (1 passed), the whole suite
after the fixture patch (every test binary `ok`, adapter_http 19 passed),
`adapter_parity` (1 passed) and the real quickstart, whose JSON, HTML and MCP
output all showed `acme-001` as "Artemis Launch" and `acme-003` unchanged. The
restore step left the copy clean on `main`, and the checker confirmed that the
real working tree had not changed.

Checks that the checks can fail:

- With the first patch temporarily edited to drop a comma, the red step
  failed to compile (`error[E0618]`), and the checker exited 1 with
  "expected line not found in order: test
  catalogue::tests::demo_catalogue_matches_the_spec_table ... FAILED". The
  patch was restored and `src/catalogue.rs` was unchanged.
- With one Japanese include in `web.md` temporarily pointing at another
  section, `tutorial_languages_share_chapters_and_snippets` failed with
  "web.md: the Japanese page must include the same snippets in the same
  order". The file was restored.
- `tests/site_scripts.rs` was written together with the site scripts, so it
  had no red run. It feeds `check_site.py` a passing fake site and 13 broken
  copies, each of which must fail with its own reason, and checks that
  `build_site.sh` refuses a foreign non-empty directory (whose file stays),
  a file, two bad base paths and mdBook 0.5.3.
- The first real site check failed on mdBook's own 404 page: its heading
  link `#document-not-found-404` resolved against `<base href="/">` to
  `index.html`. Links that are only a fragment are now checked against their
  own page.

### Final checks

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 124 passed: lib 83, adapter_http 19, adapter_mcp 9, adapter_parity 1, docs_sync 2, e2e_mcp 2, e2e_serve 3, quickstart_check 3, site_scripts 2 |
| `scripts/quickstart.sh` | 0 | API JSON, two escaped HTML items, checked MCP reply; no `senu` process left |
| `python3 scripts/check_tutorial.py commands` | 0 | 13 sections matched their expected output |
| `python3 scripts/check_tutorial.py exercise` | 0 | 10 steps as above; working tree unchanged |
| `scripts/install_mdbook.sh DIR` | 0 | official 0.5.4 tarball downloaded and its SHA-256 matched; a second run found the binary already installed |
| `MDBOOK=... scripts/build_site.sh` | 0 | 18 pages, 6 chapters in each language; links, languages and source links ok |
| `MDBOOK=... scripts/build_site.sh --base-path /senu/ DIR` | 0 | 404 pages use `<base href="/senu/">` and `/senu/ja/` |

A headless Chromium that was already installed loaded all 12 chapter pages at
390 and 1440 pixels wide: no page was wider than the viewport, no script
error was reported, the architecture figure had one column at 390 pixels and
three at 1440, and the text column was 630 pixels wide at 1440.

Not checked: how well mdBook's built-in search finds Japanese words, systems
other than Linux, and CI runs of these commits. The site has not been
published, so it has no public URL yet.

## Tutorial review fixes

An independent review of the tutorial checker reproduced three faults with
harmless fake commands: a timed-out command killed only its shell and left a
background grandchild running; the exercise copy took untracked files and the
target of a link that pointed out of the tree; and when the serve section's
`cargo build` failed, the old binary still started and the check passed. It
also asked for a clean-tree guard before the exercise and two more careful
claims in both languages. No product source or product test changed, and the
catalogue still names `acme-001` "Apollo Launch".

Files: `scripts/check_tutorial.py`, the new `scripts/test_check_tutorial.py`
(standard-library `unittest`) and `tests/tutorial_checker.rs`, which runs it
under `cargo test`; the `branch` section of `docs/tutorial/exercise/steps.sh`;
the exercise, web and introduction chapters in both languages; both READMEs
and TUT-2 and TUT-3 of spec 0002. CI is unchanged.

### Red and green

The six regression tests were written first and run against the previous
checker: `python3 scripts/test_check_tutorial.py` ran 6 tests with 6
failures. A timeout and a finished command each left their `sleep`
grandchild running; `start_server` returned success after a fake `cargo`
exited 9, because the fake old `senu` printed its listening line; the copy
listed and committed `untracked.txt` and `outside-link.txt`; a tracked edit
and a committed link to `../outside.txt` were not refused.

The fix runs every command in its own session and process group. Exits are
detected with `waitid(..., WNOWAIT)`, so the shell stays unreaped while its
group is sent SIGTERM and then SIGKILL; its id cannot be reused meanwhile, and
no other process is signalled (a bystander `sleep` in the tests survives). The
serve section runs with `bash -e`. The exercise copy is `git archive HEAD`
unpacked with `tarfile`'s `data` filter, after `git status --porcelain
--untracked-files=no` is empty. After the fix the same 6 tests passed in
1.4 s.

The first full exercise run after the fix failed in `suite-green`: inside the
copy, the tests' `/proc` reader raised `ProcessLookupError` for a process that
exited while it was read, and `start_server` left its log open on failure (a
`ResourceWarning`). Both were fixed; afterwards 15 consecutive runs of the
tests passed with no `ResourceWarning`. The tests also stop writing
`__pycache__`, which would leave the exercise copy unclean.

The exercise's `branch` section now runs `git status --short` and creates the
branch only if `git diff --quiet HEAD` succeeds. In a temporary clone with an
edit to `src/catalogue.rs` it printed ` M src/catalogue.rs`, exited 1,
created no branch and kept the edit; with only an untracked file it created
the branch and exited 0.

### Final checks after the review fixes

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --check` | 0 | clean |
| `cargo clippy --locked --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --locked -- --test-threads=2` | 0 | 125 passed, 0 failed: lib 83, adapter_http 19, adapter_mcp 9, adapter_parity 1, docs_sync 2, e2e_mcp 2, e2e_serve 3, quickstart_check 3, site_scripts 2, tutorial_checker 1 |
| `scripts/quickstart.sh` | 0 | API JSON, escaped HTML and checked MCP reply, still "Apollo Launch" |
| `python3 scripts/check_tutorial.py commands` | 0 | 13 sections matched their expected output |
| `python3 scripts/check_tutorial.py exercise` | 0 | branch 0, expect 0, red 101, rename 0, green 0, suite-red 101, fixtures 0, suite-green 0, parity 0, restore 0; working tree, HEAD and status unchanged |
| `MDBOOK=... scripts/build_site.sh` | 0 | 18 pages, 6 chapters in each language; links and languages ok |
| `MDBOOK=... scripts/build_site.sh --base-path /senu/ DIR` | 0 | same result under `/senu/` |

Not checked: systems other than Linux (the new tests read `/proc`), CI runs
of these commits, and a browser view of the changed pages. The site has not
been published, so it has no public URL.

## Coloured Cargo output in CI

The first CI run of `0f1f7ec` passed every check except the last one,
`python3 scripts/check_tutorial.py exercise`, which stopped at
``steps.sh:suite-red: expected line not found in order: error: test failed, to
rerun pass `--test adapter_http` ``. CI sets `CARGO_TERM_COLOR=always`, so Cargo
wrapped `error` in ANSI colour codes, and the checker compared that text with
the plain line in the `.out` file; locally, without forced colours, the step
passed. The same codes would also have hidden a coloured compiler error from
the check that a red step fails only because a test assertion failed, since
the line no longer started with `error`.

Files: `scripts/check_tutorial.py` and `scripts/test_check_tutorial.py`.
`normalize()` now removes ANSI control sequences before any comparison, in
both modes. CI, the tutorial and the product are unchanged.

### Red and green

Two regression tests run `check_exercise` on a temporary repository whose
only step, `suite-red`, prints coloured Cargo diagnostics and exits 101.
Against the previous checker, `python3 scripts/test_check_tutorial.py` ran 8
tests with 2 failures: the coloured line from CI raised the same "expected
line not found" error, and a coloured `error[E0425]` line printed before it
was not reported as a failure for another reason. After the fix the 8 tests
passed in three consecutive runs.

### Checks after the CI fix

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/test_check_tutorial.py` | 0 | 8 tests passed |
| `CARGO_TERM_COLOR=always python3 scripts/check_tutorial.py exercise` | 0 | branch 0, expect 0, red 101, rename 0, green 0, suite-red 101, fixtures 0, suite-green 0, parity 0, restore 0; working tree, HEAD and status unchanged |
| the same without `CARGO_TERM_COLOR` | 0 | the same steps and exits; working tree unchanged |
| `CARGO_TERM_COLOR=always python3 scripts/check_tutorial.py commands` | 0 | 13 sections matched their expected output |

Not checked: `cargo fmt` and `cargo clippy`, because no Rust file changed;
the test count of `suite-green`, which the checker does not print (it
requires every `test result:` line to be `ok`); the site build, because no
tutorial source changed; and CI runs of these commits.

## Spec 0003: embedded Markdown articles and the beginner course

Local Rust builds ran one at a time through a resource-bounded serial
wrapper (one CPU, 2 GiB); CI uses plain `cargo`. Each entry names the
command, its exit status and the decisive lines.

### Discovery and `build.rs` (CNT-1)

- Red: with `discover` stubbed to return an empty list,
  `cargo test --lib -q discover` exited 101: `6 failed; 3 passed`, for
  example ``assertion `left == right` failed  left: []``.
- Green: the same command exited 0: `9 passed`. The embedded table equals a
  fresh listing of `content/`; symlinks, a directory named `x.md`, a
  non-UTF-8 name, a 128 KiB + 1 file and invalid UTF-8 are refused, and
  `.x.md.swp` and `x.md~` are ignored.

### Reading time (`src/reading.rs`)

- Red: with the first attempt `words / WORDS_PER_MINUTE`,
  `cargo test --lib -q reading` exited 101: `2 passed; 3 failed`, with
  `left: 0 right: 1` for one word, `left: 1 right: 2` for 201 words and an
  off-by-one at `usize::MAX`.
- Green: quotient plus one when the remainder is not zero; the same command
  exited 0 with `5 passed`, including `usize::MAX` without overflow.

### Markdown and link policy (SAFE-1 to SAFE-3, SAFE-2)

- Red: with the event policy passing every event through and the link rule
  accepting any non-empty destination, `cargo test --lib -q markdown`
  exited 101: `7 passed; 9 failed` (images, a second H1, raw HTML, the
  info string, unsupported events and the destination table).
- Green: the same command exited 0 with `16 passed`. Raw HTML and code are
  escaped, the info string is gone, footnote, task-list, math,
  strikethrough, alert and definition syntax stays literal text with only
  allow-listed tags, events outside the list fail closed, and the 34 refused
  destinations report line 4 and the debug-quoted destination.

### Names, titles, navigation and page views (ART-1 to ART-3, SAFE-4)

- Red: with `Site::load` stubbed to return no pages,
  `cargo test --lib -q site::` exited 101: `1 passed; 12 failed`.
- Green after two test corrections (the label test had the two languages'
  link names swapped, and Askama writes `&#60;`, not `&lt;`): the same
  command exited 0 with `13 passed`.
- `templates_and_configuration_never_disable_escaping` keeps its template
  and `askama.toml` checks and now also requires exactly one `HtmlSafe` impl
  in `src/`, on `TrustedHtml` with a private field, and no `MaybeSafe`,
  `filters::Safe` or `safe` filter. It is a guard and passed on its first
  run (`cargo test -q --test adapter_http never_disable_escaping`: 1 passed);
  no red run is claimed for it.

### `senu site` router and wiring (ART-4, SITE-1 to SITE-3)

- Red: with the page handler answering 404 for everything, `site_config`
  ignoring credentials and startup skipping `Site::load`,
  `cargo test -q --lib site_` exited 101 (`1 failed`),
  `cargo test -q --test adapter_site` exited 101 (`1 passed; 4 failed`) and
  `cargo test -q --test e2e_site` exited 101 (`0 passed; 2 failed`).
- Green: `cargo test -q --lib` 127 passed; `--test adapter_site` 5 passed;
  `--test e2e_site` 2 passed; the existing `--test adapter_http` (19) and
  `--test e2e_serve` (3) still pass. `cargo clippy --locked --all-targets
  -- -D warnings` exited 0 after one elided lifetime in the new test.
- The E2E test starts the binary from a new empty directory with a cleared
  environment, checks four pages, 404 plain text for `/projects`,
  `/api/projects` and `/mcp`, 405 with `Allow: GET`, no `WWW-Authenticate`,
  identical bodies with `Authorization`, and that no response contains the
  repository path, the working directory or a sentinel.

### Tutorial checker (`scripts/check_tutorial.py`, EX-1, EX-2)

The checker now reads the fenced blocks of `content/<locale>/*.md` and runs
each language in its own copy of HEAD; it keeps the earlier process-group,
fail-fast-server, copy-only-the-commit and ANSI fixes, and drops Senu's own
build outputs from the seeded cache.

- Red (found by the first real run, then pinned by a test): the paragraph
  before a `file=` block was cleared by the blank line in front of the
  fence, so `content/en/04-first-note.md:20` was refused with "the paragraph
  before must name …". With that fix reverted,
  `python3 scripts/test_check_tutorial.py` exited 1: `Ran 16 tests`,
  `FAILED (failures=4, errors=2)`.
- Green: with the fix, the same command exited 0 (`Ran 16 tests`, `OK`) in
  three consecutive runs. The tests cover timeouts and grandchildren,
  Ctrl+C first for servers, a failed build never starting an old binary, an
  untracked note and stale `senu` artifacts never reaching the copy, refused
  dirty trees and links out of the tree, block parsing and file paths, ANSI
  and thread-id normalization, coloured compiler errors, and a changed
  `src/reading.rs`.

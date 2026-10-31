# Senu Spec 0005: The Application Developer Flow and Declared Operations

Status: implemented, 2026-10-31 (evidence in `docs/development.md`). Stage 2 of two; it starts only after spec 0004 is done. It amends spec 0001 where §9 says so. Evidence belongs in `docs/development.md`, CI runs and PRs, not in this file.

## 1. Goal and boundaries

- **Goal:** audit what an application developer writes today to add one small read-only function on all three transports (HTML, JSON and MCP), then move the single most repeated responsibility into Senu behind an explicit, safe convention, without hiding the application's business or policy semantics.
- **Measure:** the code, templates, metadata, tests, docs and commands that the application owns for equivalent working samples, before and after (§7). Senu's own size is a separate diagnostic and never a goal. Deleting tests is never counted as a saving.
- **Non-goals:** a general SDK or "all of Rails", generators or scaffolds, macros or a DSL, an ORM, a database, writes, logins, TLS, remote MCP, path parameters, pagination, new CLI modes, new configuration variables, plugins, splitting the crate into a workspace, publishing, and any automatic exposure of operations.

## 2. Audit of the current flow (1163895)

| Concern | Where it lives today | What an adopter can change |
|---|---|---|
| Start | Clone, `cargo run`, `scripts/quickstart.sh`; no scaffold | The whole seed (fork) |
| CLI and modes | `cli.rs` (`parse_args`, `HELP`); `app.rs` (`run`, private `serve`, `mcp`, `listen`) | Only by editing both |
| Configuration | `config.rs`: five fixed `SENU_*` names, pure validation; `Secret::new` is `pub(crate)` | Nothing per operation; this is already central |
| Data and tenants | `catalogue.rs`: constant `DEMO_PROJECTS`; `TenantId` is limited to `ACME` and `GLOBEX` | Only by editing |
| Principals and permissions | `operation.rs`: closed `Permission` enum (`ListProjects`); `Principal::demo_web`, `demo_api` and `demo_mcp` | `Principal::new` is public, but there are no new permissions |
| Operation | `operation.rs`: pure `list_projects` (authorizes first, decodes strictly, bounds, filters by tenant, sorts); `OpError` includes the operation-specific `InvalidSearch` | Business code, rightly owned by the application |
| HTTP | `http.rs`: one hand-written handler per route and surface, each repeating the method check, the credential check (private `single_authorization`), the public `query_arguments`, the transport principal, the error mapping (private `Surface::error`) and rendering with the CSP; private `limits` and `response_headers` layers | Only by editing `http.rs` |
| MCP | `mcp.rs`: `call` compares against `TOOL_NAME`; a hand-built JSON Schema; `tools()` returns one tool; startup in the private `app::mcp` | Only by editing `mcp.rs` |
| View | `view.rs`: a struct and a render function per page; `projects.html` is standalone (its own head) | One template per page |
| Tests | 177 test functions (127 unit, 50 integration) | — |

Findings:
1. Each new operation repeats the same transport wiring in two HTTP handlers, one MCP declaration with its dispatch and schema, and the matching transport tests (401, 405, unknown field, deny before validation, parity). The security ordering (limits, method, credential, principal, permission, decoding, operation, error mapping) is re-implemented in each handler, and parity depends on keeping three copies aligned.
2. Configuration, authentication parsing and the site are already central and are not repeated per operation.
3. An external crate cannot add an operation through the public API: `Permission` is closed, `Secret` cannot be built outside the crate, the layers and error helpers are private, `McpServer` and `app::run` hardcode `list_projects`, and routes added to `http::router`'s result would bypass its layers. The fair baseline is therefore the in-tree change that the seed invites today, reusing every existing helper, including crate-private ones. Copying private helpers into an external crate would inflate the baseline and is not done.

The chosen responsibility is finding 1: **declare an operation once, and expose it explicitly through shared transport adapters.**

## 3. Design

Application-owned code stays explicit: the data, the tenants, the principals per transport, the permissions, each operation's pure function, its argument struct, result type and error codes, its declaration, its view and template, the route table, the list of MCP tools, and the business tests. Senu owns the adapters, the validation and the contract tests.

- `Permission` becomes `Permission::new("list_projects")`, a `const` newtype that the application declares; `Permission::ListProjects` remains as a compatibility associated constant. `Principal` is unchanged.
- `Operation<S, T>` is a plain struct with `name`, `description`, `permission`, `params: &[Param]` and `run: fn(&S, &Principal, &Args) -> Result<T, OpError>`. `S` is the application's state and `T: Serialize` is its result type. `Operation::call` checks the permission before calling `run`, so denial always precedes input validation. After that check, it enforces the declared parameter envelope (known keys only, required parameters present, and present values strings and never null) before `run`. Duplicate parameter names and invalid operation/tool metadata are rejected at startup. `run` decodes with `operation::decode::<A>(args)` into the application's own `#[serde(deny_unknown_fields)]` struct, where an error becomes `invalid_request`. `Param` has a `name`, a `description` and `required`; parameters are strings, as query strings are, and an operation parses anything else itself.
- `OpError` keeps `Forbidden` and `InvalidRequest`, adds `NotFound` (404) and adds `Invalid { code, phrase }` while retaining `InvalidSearch` and its mapping for source compatibility, declared by the operation. Every public code, status and body stays byte-identical. `HttpError` follows.
- `http::Endpoints<S>` is an explicit route table: `.html(path, &OP, view)`, where `view` is `fn(&Args, &T) -> askama::Result<String>`; `.api(path, &OP)`; and `.custom(path, MethodRouter)` as the override hatch, which gets limits and headers but no automatic authentication. `.into_router(AppState<S>)` validates the table first and returns `RouteError` rather than letting Axum panic. It rejects duplicate paths, paths that are not exact (empty, missing `/`, or containing `{`, `}`, `*`, `?`, `#`, `%`, backslashes, control characters, old Axum colon/wildcard segment syntax, or a dot segment), API paths outside `/api/`, HTML paths under `/api/` and anything under `/_senu/`. It then adds `/_senu/ui.css`, the 404 fallback, the limits and the headers, so every route has the same layers. The shared handlers keep today's order: limits, method (405), exactly one credential (Basic for HTML, Bearer for API), the server-selected principal, `Operation::call`, then rendering with the existing error mapping and CSP.
- `templates/serve_base.html` (Senu-owned) carries the head, the stylesheet link and the demo banner. Application page templates extend it.
- `mcp::McpServer<S>::with_tools(state, principal, vec![mcp::tool(&OP), …])` returns `ToolError` for duplicate names. Each tool's JSON Schema is built from `params`: `type: string` with its description, `required` only when non-empty, and `additionalProperties: false`, with the read-only annotation. Only listed operations are tools. Startup is unchanged: the token is validated before stdin is read, and every call is bound to the stdio principal.
- Declaring an operation exposes nothing. A route or tool exists only where the application lists it, and an invalid table at startup exits with status 2.
- The application code moves to `src/demo/`: `mod.rs` with the permissions, principals, route table and tool list, and one file per operation. Today's public paths stay as re-exports (§9).

## 4. The second operation: `get_project`

It has its own semantics, and it does not specialize the adapters to one domain. Its argument is `{ "id": string }`, which is required; a missing `id`, `null`, a non-string value or an unknown field is `invalid_request`. A valid id has 1 to 32 bytes of `[a-z0-9-]`; anything else is `invalid_id` (400, "Invalid id"). It needs the new `view_project` permission, which the demo principals hold and test principals may lack; denial wins even over invalid input. It finds the project only within the principal's tenant. An id that is unknown or belongs to another tenant is `not_found`, identical on every transport, so a project's existence is never revealed. The result is `{ "id", "name" }` without the tenant. The routes are `/project?id=` (HTML, Basic) and `/api/project?id=` (JSON, Bearer), plus the MCP tool `get_project`. The HTML page escapes the markup fixture name of `acme-003`.

The contract tests also run a test-only state type that is not `Catalogue`, with one operation, which proves the adapters are generic.

## 5. Baseline first, then change

1. **Baseline (B):** before any core change, add `get_project` the current way: operation code, a `Permission` variant, two handlers, an MCP declaration and dispatch, a view and template, and tests (business cases plus HTTP and MCP auth, deny, invalid, not-found and parity cases) and README rows. All checks and CI pass. B is the working fair baseline.
2. **Change:** introduce the mechanism and migrate `list_projects` and `get_project` onto it. B's tests and every existing test must pass unchanged, apart from the type renames of §9. This proves the same meaning, results, schemas, statuses and authentication behaviour.
3. **Move:** generic transport cases of B that the contract tests now cover move into Senu-owned tests. Business assertions stay in the application and are counted.

## 6. Security invariants

These must keep holding: principals and tenants are selected only by the server, and tenants stay isolated; permission is denied before any input is validated; the same operation is called on HTML, JSON and MCP; limits, unknown-field, type and error behaviour stay as they are; autoescaping stays on, with `src/markdown.rs` as the only `HtmlSafe` impl; the Markdown URL and raw-HTML policy stays strict, and the site stays isolated without credentials; MCP stays an explicit opt-in on stdio, with the token checked at startup only; routes stay explicit and validated, with no filesystem paths at run time; errors repeat neither input nor credentials.

The permission-first guarantee is bounded. The permission that an operation declares is checked before the common parameter envelope and before decoding, and each public pure function (`list_projects`, `get_project`) still checks it before decoding when it is called directly; tests keep a direct call that is denied with malformed input. Any further domain policy inside `run` happens after the envelope, and Senu does not claim to check arbitrary policy before input validation.

## 7. Measured samples and counting rules

- **A1, the add-an-operation sample:** `get_project`. On the baseline side, B's `git diff --numstat`. On the after side, the operation's own file, its template, and every line added to `src/demo/mod.rs`, tests and docs. Both sides count non-blank lines, split into Rust, templates, metadata (declaration, params, error codes, route and tool lines), tests, docs and commands, plus the files touched and the commands run.
- **A2, `list_projects`:** a table of each of its responsibilities, showing which ones moved to Senu, which stayed in the application and why, with before and after lines.
- **A3, a second fixture:** the test-only state and operation of §4, listing every line its author writes.
- **Diagnostics, never goals:** the lines Senu gained, the test counts before and after, and confirmation that `Cargo.lock` and runtime dependencies are unchanged.
- Numbers in docs and PRs come only from these measurements. Until then, the expected outcome (no transport handler, schema or generic transport test written per operation in application code) is a target, not a result. No claim compares Senu with other frameworks.

## 8. Test pyramid

- **Unit:** each operation's business rules, which stay in the application; `Operation::call` ordering; schema building; and route and tool validation, with every rejected form of §3 returning an error and never panicking.
- **Contract (Senu-owned, run for every declared HTML/API route and tool and for the test-only fixture; custom routes retain application-owned authentication tests and receive shared limits, headers and path validation):**
  - **CT-1** Missing, wrong or duplicate credentials give 401 with the right challenge.
  - **CT-2** Methods other than GET give 405 with `Allow`; a body gives 413; an oversized target gives 414.
  - **CT-3** A principal without the operation's permission gets `forbidden` on all three transports, even with an unknown field or a malformed value.
  - **CT-4** An unknown field, a repeated query key, or an MCP `null` or non-string value gives `invalid_request` on all three.
  - **CT-5** Empty arguments give the same outcome on all three.
  - **CT-6** The MCP tool list matches the opted-in tools and their schemas; an operation that is declared but not listed is neither routed (404) nor callable (-32602).
  - **CT-7** HTML responses keep the CSP and the headers.
- **Application parity:** one line per operation with a meaningful input (`q=launch`, `id=acme-002`) asserts identical results across the three transports.
- **E2E (few):** the existing serve and MCP E2E tests each add one `get_project` call.
- **Unchanged:** all 177 existing tests, the course, the quickstart and the snapshot.

## 9. Compatibility and migration

The routes, bodies, headers, schemas and errors of `list_projects` are unchanged. The additions are the stage 1 stylesheet route, `get_project`'s routes and tool, and therefore a second entry in `tools/list`. Spec 0001's "one operation" becomes two, as listed here. Source-level changes, each covered by existing tests that compile against them:

- `demo::LIST_PROJECTS` is the preferred name; the existing `Permission::ListProjects` associated constant stays compatible.
- `OpError::InvalidSearch` and `HttpError::InvalidSearch` remain with their existing mappings. The new `Invalid { code, phrase }` and `NotFound` variants support application-specific errors.
- `AppState` and `McpServer` gain a type parameter that defaults to `Catalogue`.
- `http::router`, `AppState::new`, `with_principals`, `McpServer::new`, `with_principal`, `TOOL_NAME`, `operation::list_projects`, `Principal::demo_*`, `Catalogue` and `TenantId` stay at their paths.

## 10. Acceptance criteria

- **AUD-1** `docs/development.md` repeats §2 with file references checked against the code at the change's base.
- **BASE-1** Commit B builds and passes every check and CI before any core change.
- **EQV-1** After the change, B's tests and all existing tests pass with only the §9 renames. HTML, JSON and MCP results, schemas, statuses and authentication behaviour of both operations are identical.
- **DEC-1** Declared but unlisted operations are not exposed (CT-6). Route and tool collisions and invalid paths return errors, exit with status 2 at startup and never panic.
- **SEC-1** The invariants of §6 hold, CT-1 to CT-7 pass for both operations and the test-only fixture, and the template and escaping guards are unchanged.
- **GEN-1** The test-only state type compiles and passes the contract without any change to Senu code.
- **MEAS-1** A1 to A3 and the diagnostics are recorded with real numbers, and the moved responsibilities are explained.
- **DOC-1** Both READMEs and AGENTS.md describe the application-owned and Senu-owned boundary, how to add an operation, the override hatch and the migration notes. `docs_sync` keeps the route tables in step.
- **TST-1** At the final head, all of these pass: fmt, clippy, every Rust test (count recorded), the quickstart, the course in both languages, the snapshot and CI.

## 11. Delivery

Each commit passes all checks on its own and records its RED and GREEN evidence in `docs/development.md`.

1. `sample:` add `get_project` the current way (BASE-1, A1 baseline).
2. `ops:` declared permissions, `Operation` with permission first, and `OpError` with `Invalid` and `NotFound`; migrate both operations (EQV-1, in part).
3. `http:` `Endpoints`, `RouteError`, `serve_base.html` and the generic `AppState`; move both operations' routes (EQV-1, DEC-1).
4. `mcp:` `with_tools`, schemas from `params`, and the explicit tool list (EQV-1, DEC-1).
5. `test:` the contract tests, the test-only fixture, and the generic cases moved out of B (SEC-1, GEN-1).
6. `docs:` the audit, the boundary, the migration notes and the measurements (AUD-1, MEAS-1, DOC-1, TST-1).

## 12. Risks and known limits

- String-only parameters and exact paths are deliberate limits. Anything else goes through `.custom`, which carries its own authentication responsibility.
- A generic `S` with a `Catalogue` default keeps compatibility, but it still names the demo type inside Senu's adapters.
- Adopters still fork the seed. A runner for an external crate (CLI and configuration hooks) is a possible later spec, not this one.
- If the measurements show that the change saves the application less than expected, `docs/development.md` reports that as it is.

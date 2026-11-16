# Senu Spec 0007: Write Operations, Typed Arguments and Session Scaffolding

Status: draft, 2026-11-16. It answers the first adopter's report (Preship, on 0.2.0 at e3bbd2d) and withdraws the non-goals "write operations", "typed parameters" and "sessions" of spec 0006 as far as §3 says. It amends specs 0001, 0005 and 0006 where §9 says so. Evidence belongs in `docs/development.md`.

## 1. Goal and boundaries

- **Goal:** a declared operation may change state. It passes the same steps in the same order as a read, on the JSON API and as an MCP tool, with protection against cross-site requests, a body of typed scalars, fixed errors and contract tests that cover it.
- **Also:** what an application with cookie sessions had to write by hand: reading one cookie, sending people to a login page, routes of its own that take a body, and running the contract through its own authenticator.
- **Non-goals of 0.3:** HTML form posts through `.html`, nested or array arguments, file uploads, path parameters, MCP over HTTP (spec 0008), session storage, login pages, OAuth, a database layer, idempotency keys and optimistic locking.

## 2. Findings from the adopter's spike

1. A POST to any Senu route, `custom` ones included, is 413: the limits refuse every body.
2. Writes therefore lived in a second router merged beside Senu's, where the order of checks, the fixed errors, the response headers, the size limits and the contract do not apply. A forgotten permission check there fails open.
3. Arguments are strings only; a write needs numbers and booleans.
4. `OpError` has nothing for "not now" (409).
5. A session authenticator had to parse `Cookie` itself and invent a `WWW-Authenticate` value; pages wanted a redirect to a login page instead of 401.
6. The contract kit builds its own Basic and Bearer, so the application's authenticator is never exercised.
7. The core does not build for `wasm32-unknown-unknown`, because `tokio/net` and `axum/tokio` pull in mio.

## 3. Design

- **Effect.** `Operation` gains `effect: Effect`: `Effect::Read`, or `Effect::Write { destructive, idempotent }`. A read is served by `GET` with its arguments in the query, as before. A write is served by `POST` with a JSON object as its body, and never by `GET`.
- **Typed parameters.** `Param` gains `kind: ParamKind` (`String`, `Integer`, `Number`, `Boolean`); `Param::required(..)` and `Param::optional(..)` stay strings, and `.integer()`, `.number()` and `.boolean()` change the kind. The envelope checks each value against its kind; null, arrays and objects are refused; an integer is a JSON number without a fraction that fits `i64`. A read's parameters must all be strings, because a query string carries nothing else; `Operation::check` refuses any other declaration. `decode` is unchanged: the operation's own `deny_unknown_fields` struct now has `f64`, `i64` and `bool` fields.
- **The order for a write on HTTP.** (1) limits: the target, and a declared `Content-Length` above the route's limit is 413; (2) the method: anything but `POST` is 405 with `Allow: POST`; (3) the origin check below, 403 `cross_origin`; (4) authentication, 401; (5) the permission, 403; (6) the query must be empty, and `Content-Type` must be `application/json` (parameters allowed), else 415 `unsupported_media_type`; (7) the body is read up to the limit, 413 beyond it, and must be a JSON object, else `invalid_request`; (8) the envelope; (9) the function; (10) the result as JSON with 200, or the fixed error. Nothing of the body is read before step 7, so an unauthenticated or unpermitted caller costs no parsing and learns nothing about valid input. `Operation::authorize` is the permission check alone; `admit` remains both checks.
- **Cross-site requests.** Every write route, whatever its authenticator, applies the check that Go 1.25's `CrossOriginProtection` applies: with `Sec-Fetch-Site`, only `same-origin` and `none` pass; otherwise with `Origin`, its host and port must equal the request's `Host`; with neither header the request is not from a browser and passes. `Endpoints::trusted_origin("https://app.example.com")` admits further exact origins before those rules. The check uses headers that a browser sets and a page cannot forge; it does not protect against a non-browser client that holds a credential, which needs none.
- **Body limit.** 64 KiB per write route by default; `Endpoints::max_body_bytes(n)` changes it for the table (1 to 16 MiB). Reads and assets still refuse any body.
- **Where a write may be listed.** `.api(path, &OP)` and `mcp::tool(&OP)`. `.html` with a write is `RouteError::WriteNeedsApi`: a page that changes state posts to the JSON route from its own script or uses a `custom` route.
- **Errors.** `OpError::Conflict { code, phrase }` is 409: the request is well-formed but cannot be done now. `HttpError` gains `Conflict`, `CrossOrigin` (403 `cross_origin`) and `UnsupportedMediaType` (415). All stay fixed strings.
- **MCP.** A tool's annotations follow the effect: a read has `readOnlyHint: true`; a write has `readOnlyHint: false`, `destructiveHint` and `idempotentHint` as declared, and `openWorldHint: false` is not claimed. The schema types follow `ParamKind`. Over stdio every call still runs as the one principal chosen at startup, so give a principal that may write only to a process you would let write.
- **Custom routes with a body.** `Endpoints::custom_with_body(path, max_bytes, handler)` admits a body up to `max_bytes` (a larger `Content-Length` is 413, and Axum's extractors stop at the same limit). It gets the response headers and no authentication, method check, origin check or CSP, like `custom`. `auth::same_origin(&HeaderMap, &[&str])` is the origin check for such a handler to call.
- **Sessions.** `auth::cookie(headers, name)` returns the value of the cookie when it appears exactly once. `Authenticate::challenge` returns `Option<HeaderValue>`, `None` by default, so a session authenticator sends no invented scheme. `AppStateBuilder::html_login(path)` answers an unauthenticated page request with `303 See Other` to that fixed path instead of 401; the JSON API always answers 401.
- **The contract.** New checks for every listed write: **CT-8** `GET` and other methods are 405 with `Allow: POST`; **CT-9** a cross-site request (`Sec-Fetch-Site: cross-site`, or an `Origin` that is not the host) is 403 `cross_origin` even with good credentials; **CT-10** without credentials it is 401, and without the permission 403, each with a body that is not JSON, which proves the body was not read; **CT-11** a wrong `Content-Type` is 415, a body over the limit 413, a non-object, an unknown key and a value of the wrong kind `invalid_request`; **CT-12** the tool's annotations match the declared effect, its schema types match the parameter kinds, and the tool refuses the same malformed arguments; the sample succeeds as JSON. A write's sample is a JSON object as text. `Subject::auth` lets an application run all of CT-1 to CT-12 through its own authenticators: it supplies a function from a principal to an `AppState` and the request headers that authenticate on each surface. With it, an unauthenticated page request may be the login redirect, and the challenge is not compared. Read samples are compared across transports before any write sample runs; write samples run in the order of the table, on one state.
- **Features.** `stdio` (default) holds `McpServer::serve_stdio` and the dependencies that need sockets; `demo` implies it. Without `demo` and `stdio` the core uses neither `tokio/net` nor `axum/tokio`.

## 4. Security invariants

Those of specs 0005 and 0006 hold. In addition: a write never runs on `GET`; no byte of a body is parsed before authentication and the permission; a cross-site browser request never reaches authentication on a write route; a route's body limit holds for declared and for streamed lengths; errors stay fixed and repeat no input; a write is exposed only where listed, and its MCP annotations never call it read-only.

Not guaranteed: atomicity, idempotency or retries of the application's function; protection of `custom` routes, which own their checks; anything about a client that is not a browser and holds a valid credential.

## 5. Not adopted, and why

- **Path parameters:** exact paths keep the route table checkable at startup and the query form covers the need (`?id=P-15`).
- **HTML form posts:** they need a view for the result or a redirect policy, and an encoding of typed fields in forms. A page can post to the JSON route.
- **MCP over HTTP with a principal per request:** a second security boundary (remote callers, sessions, origin rules of the MCP specification). It gets spec 0008.
- **Nested arguments:** flat scalars keep the envelope and the schema exact. An operation that needs a structure takes a JSON string and parses it.

## 6. Tests

Every existing test passes with the renames of §9. New: kinds in the envelope and the schema; the write order on HTTP, step by step, with a function that counts its runs; the origin rules as a table; body limits by declared and by streamed length; `Conflict`; annotations; `custom_with_body`; `auth::cookie`; the login redirect; the contract over a write-capable example with its own cookie authenticator; mutations that each new check must catch.

## 7. The example

`examples/items.rs`: items per tenant with list, create, update and delete as declared operations, cookie sessions, a login redirect, a `custom_with_body` logout, JSON and MCP. `tests/example_items.rs` runs the contract through the example's own authenticator.

## 8. Delivery

1. `spec:` this file. 2. `ops:` `Effect`, `ParamKind`, `authorize`, `Conflict`. 3. `http:` write routes, the origin check, limits, `custom_with_body`. 4. `auth:` `cookie`, `challenge`, `html_login`. 5. `mcp:` annotations and schema types. 6. `contract:` CT-8 to CT-12 and `Subject::auth`. 7. `pkg:` the `stdio` feature. 8. `docs:` the example, the guide in both languages, READMEs, changelog.

## 9. Compatibility and migration from 0.2

Breaking, so the version is 0.3.0. Every `Operation { .. }` needs `effect: Effect::Read`; every `Param { .. }` literal needs `kind: ParamKind::String` (the helpers do not change). `Authenticate::challenge` returns `Option<HeaderValue>`. `contract::Subject` needs `auth: None`. `serve_stdio` needs the feature `stdio` when default features are off. `HttpError::MethodNotAllowed` no longer implies `Allow: GET` on a write route. Routes, bodies, headers, schemas and codes of every read are unchanged.

## 10. Acceptance criteria

- **WR-1** A declared write is reachable only by `POST` on a listed API route and as a listed tool, in the order of §3.
- **WR-2** CT-8 to CT-12 pass for the example and fail under matching mutations.
- **ARG-1** Typed parameters are enforced by the envelope and described by the schema.
- **SES-1** `auth::cookie`, the optional challenge and the login redirect work as §3 says.
- **CUS-1** `custom_with_body` takes a body up to its limit with Senu's headers.
- **EQV-1** Every pre-existing test passes with only the §9 changes.
- **PKG-1** The core builds without `stdio`; whether it builds for `wasm32-unknown-unknown` is recorded as measured.
- **DOC-1** The example, the guide in both languages, the READMEs, the changelog and the limits are in step; all checks of `AGENTS.md` pass.

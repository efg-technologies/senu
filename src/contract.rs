//! Senu's contract for declared operations, as a kit for your own tests.
//!
//! [`check`] takes your route table and tool list and sends real requests
//! through the same router and MCP server that production uses. It panics,
//! naming the route or tool, when one of these does not hold.
//!
//! For every listed read:
//!
//! - **CT-1** Missing, wrong or duplicated credentials are 401 with the
//!   challenge, before the permission and before any input is read.
//! - **CT-2** Other methods are 405, a body is 413 and a long target is 414,
//!   all before credentials.
//! - **CT-3** A principal without the permission gets 403, whatever the
//!   input.
//! - **CT-4** Unknown, repeated and non-string parameters are
//!   `invalid_request`.
//! - **CT-5** The same arguments give the same outcome as JSON, as a page
//!   and as a tool result, and each sample succeeds.
//! - **CT-6** Exactly the listed tools exist, each with a closed schema
//!   that matches its parameters; an unknown tool is an error that does not
//!   repeat its name.
//! - **CT-7** Pages and errors carry `Cache-Control: no-store`,
//!   `X-Content-Type-Options: nosniff` and, for pages, the table's CSP.
//!
//! For every listed write (spec 0007):
//!
//! - **CT-8** `GET` and every method but `POST` are 405 with `Allow: POST`.
//! - **CT-9** A cross-site browser request is 403 `cross_origin`, even with
//!   good credentials.
//! - **CT-10** Without credentials it is 401, and without the permission
//!   403, each with a body that is not JSON: the body was not read.
//! - **CT-11** A wrong `Content-Type` is 415, a body over the limit 413,
//!   and a non-object, a query string, an unknown key or a value of the
//!   wrong kind `invalid_request`.
//! - **CT-12** The tool's annotations match the declared effect and never
//!   say read-only, the tool refuses the same malformed arguments, and the
//!   sample succeeds as JSON and as a tool call.
//!
//! By default the kit brings its own Basic and Bearer credentials, because
//! the contract is about the adapters and your declarations. Set
//! [`Subject::auth`] to run the same checks through your own
//! authenticators. `custom` routes and assets are not covered: they are
//! yours.
//!
//! ```no_run
//! # use senu::http::Endpoints;
//! # use senu::mcp::McpTool;
//! # use senu::operation::{Principal, TenantId};
//! # struct Stock;
//! # fn state() -> Stock { Stock }
//! # fn endpoints() -> Endpoints<Stock> { Endpoints::new() }
//! # fn tools() -> Vec<McpTool<Stock>> { Vec::new() }
//! # const CLERK: Principal = Principal::new("clerk", TenantId::from_static("main"), &[]);
//! #[tokio::test]
//! async fn my_application_meets_senus_contract() {
//!     senu::contract::check(senu::contract::Subject {
//!         state,
//!         endpoints,
//!         tools,
//!         principal: CLERK,
//!         // A read's sample is a query; a write's is a JSON object.
//!         samples: &[("get_stock", "sku=SKU-1"), ("set_stock", r#"{"sku":"SKU-1","units":3}"#)],
//!         auth: None,
//!     })
//!     .await;
//! }
//! ```

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use base64::Engine as _;
use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use crate::auth::{Basic, Bearer, Secret};
use crate::http::{AppState, Declared, Endpoints, Kind, MAX_TARGET_BYTES, query_arguments};
use crate::mcp::{McpServer, McpTool};
use crate::operation::{Effect, Param, ParamKind, Principal, TenantId};

/// The kit's own credentials. They exist only inside the test process.
const WEB_USER: &str = "contract-user";
const WEB_PASSWORD: &str = "contract-web-password-5T";
const API_TOKEN: &str = "contract-api-token-8R";

/// Request headers, as (name, value) pairs.
pub type Headers = &'static [(&'static str, &'static str)];

/// The application's own authentication, for [`Subject::auth`].
pub struct OwnAuth<S> {
    /// Builds the state whose authenticators accept the headers below as
    /// the given principal. The kit calls it with the subject's principal
    /// and with a principal that holds no permission.
    pub app: fn(S, &Principal) -> AppState<S>,
    /// The headers that authenticate a page request, such as
    /// `&[("cookie", "sid=test-session")]`.
    pub html: Headers,
    /// The headers that authenticate an API request.
    pub api: Headers,
}

/// What the contract needs from an application.
pub struct Subject<S> {
    /// A fresh state.
    pub state: fn() -> S,
    /// The application's route table.
    pub endpoints: fn() -> Endpoints<S>,
    /// The application's tool list.
    pub tools: fn() -> Vec<McpTool<S>>,
    /// A principal that holds every listed operation's permission.
    pub principal: Principal,
    /// For each listed operation, its name and arguments that succeed
    /// against [`state`](Self::state): a query string for a read, such as
    /// `("get_stock", "sku=SKU-1")`, and a JSON object for a write, such as
    /// `("set_stock", r#"{"sku":"SKU-1","units":3}"#)`.
    pub samples: &'static [(&'static str, &'static str)],
    /// `None` uses the kit's Basic and Bearer credentials. `Some` runs the
    /// checks through the application's own authenticators.
    pub auth: Option<OwnAuth<S>>,
}

type Pairs = Vec<(String, String)>;

fn pairs(headers: Headers) -> Pairs {
    headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

impl<S: Send + Sync + 'static> Subject<S> {
    fn router(&self, principal: &Principal) -> Router {
        let state = match &self.auth {
            Some(own) => (own.app)((self.state)(), principal),
            None => {
                let secret =
                    |value: &str| Secret::new(value).expect("the kit's credentials are valid");
                AppState::builder((self.state)())
                    .html_auth(Basic::new(
                        secret(WEB_USER),
                        secret(WEB_PASSWORD),
                        principal.clone(),
                    ))
                    .api_auth(Bearer::new(secret(API_TOKEN), principal.clone()))
                    .build()
            }
        };
        match (self.endpoints)().into_router(state) {
            Ok(router) => router,
            Err(err) => panic!("the route table is invalid: {err}"),
        }
    }

    fn mcp(&self, principal: &Principal) -> McpServer<S> {
        match McpServer::with_tools((self.state)(), principal.clone(), (self.tools)()) {
            Ok(server) => server,
            Err(err) => panic!("the tool list is invalid: {err}"),
        }
    }

    /// The headers that authenticate on a route of `kind`.
    fn credential(&self, kind: Kind) -> Pairs {
        match (&self.auth, kind) {
            (Some(own), Kind::Html) => pairs(own.html),
            (Some(own), _) => pairs(own.api),
            (None, Kind::Html) => vec![("authorization".to_owned(), basic(WEB_USER, WEB_PASSWORD))],
            (None, _) => vec![("authorization".to_owned(), bearer(API_TOKEN))],
        }
    }

    /// Header sets that must not authenticate on a route of `kind`, and
    /// the challenge the 401 must carry when the kit knows it.
    fn refused(&self, kind: Kind) -> (Vec<Pairs>, Option<&'static str>) {
        let good = self.credential(kind);
        let auth = |value: String| vec![("authorization".to_owned(), value)];
        let mut attempts: Vec<Pairs> = vec![Vec::new()];
        if self.auth.is_some() {
            // Each credential header with a changed value.
            for i in 0..good.len() {
                let mut changed = good.clone();
                changed[i].1.push('x');
                attempts.push(changed);
            }
            return (attempts, None);
        }
        attempts.push([good.clone(), good].concat());
        let (wrong, challenge) = match kind {
            Kind::Html => (
                vec![
                    bearer(API_TOKEN),
                    basic(WEB_USER, API_TOKEN),
                    basic("someone", WEB_PASSWORD),
                ],
                "Basic realm=\"senu\", charset=\"UTF-8\"",
            ),
            _ => (
                vec![
                    basic(WEB_USER, WEB_PASSWORD),
                    bearer(WEB_PASSWORD),
                    format!("Token {API_TOKEN}"),
                ],
                "Bearer realm=\"senu\"",
            ),
        };
        attempts.extend(wrong.into_iter().map(auth));
        (attempts, Some(challenge))
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn header(&self, name: header::HeaderName) -> &str {
        self.headers
            .get(&name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
    }
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = match app.clone().oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .expect("a response body under 4 MiB");
    Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

fn request(method: &str, target: &str, headers: &[(String, String)]) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(target);
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder.body(Body::empty()).expect("a valid request")
}

/// A `POST` with a body and its content type.
fn post(
    target: &str,
    headers: &[(String, String)],
    content_type: &str,
    body: Vec<u8>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(target)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len());
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder.body(Body::from(body)).expect("a valid request")
}

fn basic(user: &str, password: &str) -> String {
    let raw = format!("{user}:{password}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    )
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// The error body of a declared route: `{"error": code}` under `/api/`,
/// plain text elsewhere.
fn assert_error(reply: &Reply, kind: Kind, status: StatusCode, code: &str, context: &str) {
    assert_eq!(reply.status, status, "{context}: {}", reply.body);
    if kind == Kind::Api {
        assert!(
            reply
                .header(header::CONTENT_TYPE)
                .starts_with("application/json"),
            "{context}: an API error must be JSON"
        );
        let body: Value = serde_json::from_str(&reply.body)
            .unwrap_or_else(|_| panic!("{context}: an API error must be JSON"));
        assert_eq!(body, json!({ "error": code }), "{context}");
    } else {
        assert!(
            reply.header(header::CONTENT_TYPE).starts_with("text/plain"),
            "{context}: an HTML-surface error must be plain text"
        );
    }
    assert_eq!(reply.header(header::CACHE_CONTROL), "no-store", "{context}");
    assert_eq!(
        reply.header(header::X_CONTENT_TYPE_OPTIONS),
        "nosniff",
        "{context}"
    );
}

async fn call<S>(server: &McpServer<S>, name: &str, args: Value) -> CallToolResult {
    let Value::Object(args) = args else {
        panic!("arguments must be an object")
    };
    let request = CallToolRequestParams::new(name.to_owned()).with_arguments(args);
    match server.call(&request).await {
        Ok(result) => result,
        Err(err) => panic!("tool {name}: expected a tool result, got {err:?}"),
    }
}

fn assert_tool_error(result: &CallToolResult, code: &str, context: &str) {
    assert_eq!(result.is_error, Some(true), "{context}");
    assert_eq!(
        result.structured_content,
        Some(json!({ "error": code })),
        "{context}"
    );
}

/// Queries that the envelope refuses: an unknown field and, for each
/// parameter, a repeated key.
fn refused_queries(params: &[&str]) -> Vec<String> {
    let mut queries = vec!["zz_unknown=1".to_owned()];
    queries.extend(params.iter().map(|name| format!("{name}=a&{name}=b")));
    queries
}

/// A JSON value that is not of `kind`.
fn wrong_values(kind: ParamKind) -> Vec<Value> {
    let mut values = vec![Value::Null, json!(["a"]), json!({ "a": 1 })];
    values.extend(match kind {
        ParamKind::String => vec![json!(7), json!(true)],
        ParamKind::Integer => vec![json!("7"), json!(1.5), json!(true)],
        ParamKind::Number => vec![json!("7"), json!(true)],
        _ => vec![json!("true"), json!(1)],
    });
    values
}

/// Arguments that the envelope refuses: an unknown field and, for each
/// parameter, values of another kind.
fn refused_arguments(params: &[Param]) -> Vec<Value> {
    let mut arguments = vec![json!({ "zz_unknown": "x" })];
    for param in params {
        for value in wrong_values(param.kind) {
            arguments.push(json!({ param.name: value }));
        }
    }
    arguments
}

/// Runs the contract over every HTML and API route and every tool that
/// `subject` lists: CT-1 to CT-7 for reads and CT-8 to CT-12 for writes.
///
/// # Panics
///
/// When the contract does not hold, with the check and the route or tool in
/// the message; also when the table or the list is invalid, or an operation
/// has no sample.
pub async fn check<S: Send + Sync + 'static>(subject: Subject<S>) {
    let nobody = Principal::new("nobody", TenantId::from_static("contract"), &[]);
    let app = subject.router(&subject.principal);
    let denied = subject.router(&nobody);
    let server = subject.mcp(&subject.principal);
    let denied_server = subject.mcp(&nobody);
    let endpoints = (subject.endpoints)();
    let (csp, body_limit) = (endpoints.csp(), endpoints.body_limit());
    let is_write = |route: &Declared| route.effect.is_some_and(Effect::is_write);
    let operations: Vec<Declared> = endpoints
        .declared()
        .into_iter()
        .filter(|route| matches!(route.kind, Kind::Html | Kind::Api))
        .collect();
    let routes: Vec<Declared> = operations
        .iter()
        .copied()
        .filter(|r| !is_write(r))
        .collect();
    let writes: Vec<Declared> = operations.iter().copied().filter(|r| is_write(r)).collect();
    let listed = (subject.tools)();
    let tools = server.tools();
    assert!(
        !operations.is_empty() || !tools.is_empty(),
        "the application lists no operation route and no tool"
    );
    let own_auth = subject.auth.is_some();
    // 401, or the login redirect that an application may choose for pages.
    let assert_unauthenticated =
        |reply: &Reply, kind: Kind, challenge: Option<&str>, context: &str| {
            if kind == Kind::Html && own_auth && reply.status == StatusCode::SEE_OTHER {
                assert!(reply.header(header::LOCATION).starts_with('/'), "{context}");
                return;
            }
            assert_error(
                reply,
                kind,
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                context,
            );
            if let Some(challenge) = challenge {
                assert_eq!(
                    reply.header(header::WWW_AUTHENTICATE),
                    challenge,
                    "{context}"
                );
            }
        };

    for route in &routes {
        let (path, kind) = (route.path, route.kind);
        let good = subject.credential(kind);
        let (attempts, challenge) = subject.refused(kind);

        // CT-1: missing, wrong or duplicate credentials, before permission
        // and input.
        for headers in &attempts {
            for (router, query) in [(&app, ""), (&denied, "?zz_unknown=1")] {
                let target = format!("{path}{query}");
                let reply = send(router, request("GET", &target, headers)).await;
                assert_unauthenticated(&reply, kind, challenge, &format!("CT-1 {target}"));
            }
        }

        // CT-2: other methods, bodies and long targets, before credentials.
        for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let reply = send(&app, request(method, path, &[])).await;
            assert_eq!(
                reply.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "CT-2 {method} {path}"
            );
            assert_eq!(reply.header(header::ALLOW), "GET", "CT-2 {method} {path}");
        }
        let with_body = Request::builder()
            .uri(path)
            .header(header::CONTENT_LENGTH, "1")
            .body(Body::from("x"))
            .expect("a valid request");
        let reply = send(&app, with_body).await;
        assert_error(
            &reply,
            kind,
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            &format!("CT-2 {path}"),
        );
        let long = format!("{path}?pad={}", "a".repeat(MAX_TARGET_BYTES));
        let reply = send(&app, request("GET", &long, &good)).await;
        assert_error(
            &reply,
            kind,
            StatusCode::URI_TOO_LONG,
            "uri_too_long",
            &format!("CT-2 {path}"),
        );

        // CT-3 and CT-4: the permission before the envelope.
        let names: Vec<&str> = route.params.iter().map(|param| param.name).collect();
        let refused = refused_queries(&names);
        for query in std::iter::once(String::new()).chain(refused.iter().cloned()) {
            let target = format!("{path}?{query}");
            let reply = send(&denied, request("GET", &target, &good)).await;
            assert_error(
                &reply,
                kind,
                StatusCode::FORBIDDEN,
                "forbidden",
                &format!("CT-3 {target}"),
            );
        }
        for query in &refused {
            let target = format!("{path}?{query}");
            let reply = send(&app, request("GET", &target, &good)).await;
            assert_error(
                &reply,
                kind,
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &format!("CT-4 {target}"),
            );
        }
    }

    // Every listed operation has a sample.
    let sample_of = |name: &str| {
        subject
            .samples
            .iter()
            .find(|(sample, _)| *sample == name)
            .map(|(_, sample)| *sample)
    };
    for route in &operations {
        let name = route.operation.unwrap_or("");
        assert!(
            sample_of(name).is_some(),
            "the operation {name} has no sample"
        );
    }
    for tool in &listed {
        assert!(
            sample_of(tool.name()).is_some(),
            "the tool {} has no sample",
            tool.name()
        );
    }
    for (name, _) in subject.samples {
        let known = operations.iter().any(|route| route.operation == Some(name))
            || listed.iter().any(|tool| tool.name() == *name);
        assert!(known, "the sample {name} names no listed operation");
    }

    // CT-5 and CT-7: empty arguments and the application's sample give the
    // same outcome on every transport that lists the read, and pages keep
    // their headers.
    for &(name, sample) in subject.samples {
        let path = |kind| {
            routes
                .iter()
                .find(|route| route.kind == kind && route.operation == Some(name))
                .map(|route| route.path)
        };
        let (api_path, html_path) = (path(Kind::Api), path(Kind::Html));
        let tool = listed.iter().find(|tool| tool.name() == name);
        if writes.iter().any(|route| route.operation == Some(name))
            || tool.is_some_and(|tool| tool.effect().is_write())
        {
            continue;
        }
        for query in ["", sample] {
            let context = format!("CT-5 {name}?{query}");
            // The outcome as (succeeded, JSON), from the API and from MCP.
            let mut outcome: Option<(bool, Value)> = None;
            if let Some(api_path) = api_path {
                let target = format!("{api_path}?{query}");
                let api = send(
                    &app,
                    request("GET", &target, &subject.credential(Kind::Api)),
                )
                .await;
                let body: Value = serde_json::from_str(&api.body)
                    .unwrap_or_else(|_| panic!("{context}: the API must answer JSON"));
                outcome = Some((api.status == StatusCode::OK, body));
            }
            if tool.is_some() {
                let args = Value::Object(query_arguments(Some(query)));
                let result = call(&server, name, args).await;
                let ok = result.is_error != Some(true);
                let body = result.structured_content.clone().unwrap_or(Value::Null);
                if let Some((api_ok, api_body)) = &outcome {
                    assert_eq!(ok, *api_ok, "{context}: MCP and the API disagree");
                    assert_eq!(&body, api_body, "{context}: MCP and the API disagree");
                }
                outcome = Some((ok, body));
            }
            if let Some(html_path) = html_path {
                let target = format!("{html_path}?{query}");
                let page = send(
                    &app,
                    request("GET", &target, &subject.credential(Kind::Html)),
                )
                .await;
                let ok = page.status == StatusCode::OK;
                if let Some((other_ok, _)) = &outcome {
                    assert_eq!(ok, *other_ok, "{context}: the page disagrees");
                } else {
                    outcome = Some((ok, Value::Null));
                }
                if ok {
                    assert!(
                        page.header(header::CONTENT_TYPE).starts_with("text/html"),
                        "CT-7 {target}"
                    );
                    assert_eq!(
                        page.header(header::CONTENT_SECURITY_POLICY),
                        csp,
                        "CT-7 {target}"
                    );
                    assert_eq!(
                        page.header(header::CACHE_CONTROL),
                        "no-store",
                        "CT-7 {target}"
                    );
                    assert_eq!(
                        page.header(header::X_CONTENT_TYPE_OPTIONS),
                        "nosniff",
                        "CT-7 {target}"
                    );
                }
            }
            if query == sample {
                let ok = outcome.is_some_and(|(ok, _)| ok);
                assert!(ok, "{context}: the sample must succeed");
            }
        }
    }

    // CT-8 to CT-11: every write route, after the reads were compared,
    // because the samples below change the state. They run in the order of
    // the table, on one state.
    for route in &writes {
        let (path, kind) = (route.path, route.kind);
        let name = route.operation.unwrap_or("");
        let good = subject.credential(kind);
        let sample = sample_of(name).unwrap_or("{}").as_bytes().to_vec();
        let json_type = "application/json";
        assert!(
            serde_json::from_slice::<Value>(&sample).is_ok_and(|value| value.is_object()),
            "the sample of the write {name} must be a JSON object"
        );

        // CT-8: only POST.
        for method in ["GET", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let reply = send(&app, request(method, path, &good)).await;
            assert_eq!(
                reply.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "CT-8 {method} {path}"
            );
            assert_eq!(reply.header(header::ALLOW), "POST", "CT-8 {method} {path}");
        }

        // CT-9: a browser on another site, with good credentials.
        let cross_site: [&[(&str, &str)]; 3] = [
            &[("sec-fetch-site", "cross-site")],
            &[("sec-fetch-site", "same-site")],
            &[
                ("origin", "https://attacker.example"),
                ("host", "app.example"),
            ],
        ];
        for extra in cross_site {
            let mut headers = good.clone();
            headers.extend(
                extra
                    .iter()
                    .map(|(n, v)| ((*n).to_owned(), (*v).to_owned())),
            );
            let reply = send(&app, post(path, &headers, json_type, sample.clone())).await;
            assert_error(
                &reply,
                kind,
                StatusCode::FORBIDDEN,
                "cross_origin",
                &format!("CT-9 {path} {extra:?}"),
            );
        }

        // CT-10: authentication and the permission before the body, which
        // is not JSON here, so reading it would give another error.
        let (attempts, challenge) = subject.refused(kind);
        for headers in &attempts {
            let reply = send(&app, post(path, headers, json_type, b"not json".to_vec())).await;
            assert_unauthenticated(&reply, kind, challenge, &format!("CT-10 {path}"));
        }
        for (content_type, body) in [(json_type, &b"not json"[..]), ("text/plain", &sample[..])] {
            let reply = send(&denied, post(path, &good, content_type, body.to_vec())).await;
            assert_error(
                &reply,
                kind,
                StatusCode::FORBIDDEN,
                "forbidden",
                &format!("CT-10 {path}"),
            );
        }

        // CT-11: the content type, the size and the shape of the body.
        for content_type in [
            "text/plain",
            "application/x-www-form-urlencoded",
            "application/jsonx",
        ] {
            let reply = send(&app, post(path, &good, content_type, sample.clone())).await;
            assert_error(
                &reply,
                kind,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                &format!("CT-11 {path} {content_type}"),
            );
        }
        let mut large = b"{\"zz_pad\":\"".to_vec();
        large.resize(body_limit + 1, b'a');
        let reply = send(&app, post(path, &good, json_type, large)).await;
        assert_error(
            &reply,
            kind,
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            &format!("CT-11 {path}"),
        );
        let long = format!("{path}?pad={}", "a".repeat(MAX_TARGET_BYTES));
        let reply = send(&app, post(&long, &good, json_type, sample.clone())).await;
        assert_error(
            &reply,
            kind,
            StatusCode::URI_TOO_LONG,
            "uri_too_long",
            &format!("CT-11 {path}"),
        );
        let mut malformed: Vec<(String, Vec<u8>)> = ["", "not json", "[]", "\"x\"", "7", "null"]
            .iter()
            .map(|body| (path.to_owned(), body.as_bytes().to_vec()))
            .collect();
        malformed.push((format!("{path}?zz_unknown=1"), sample.clone()));
        malformed.extend(
            refused_arguments(route.params)
                .iter()
                .map(|args| (path.to_owned(), args.to_string().into_bytes())),
        );
        for (target, body) in malformed {
            let context = format!("CT-11 {target} {}", String::from_utf8_lossy(&body));
            let reply = send(&app, post(&target, &good, json_type, body)).await;
            assert_error(
                &reply,
                kind,
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &context,
            );
        }

        // CT-12, first half: after every refusal above, the sample succeeds.
        let reply = send(
            &app,
            post(path, &good, "application/json; charset=utf-8", sample),
        )
        .await;
        assert_eq!(
            reply.status,
            StatusCode::OK,
            "CT-12 {path}: the sample must succeed: {}",
            reply.body
        );
        assert!(
            serde_json::from_str::<Value>(&reply.body).is_ok(),
            "CT-12 {path}: a write answers JSON"
        );
        assert_eq!(
            reply.header(header::CACHE_CONTROL),
            "no-store",
            "CT-12 {path}"
        );
    }

    // CT-6 and CT-12: only the listed tools, each annotated by its effect,
    // with a closed schema that matches its parameters; CT-3 and CT-4 over
    // MCP.
    let names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
    let listed_names: Vec<&str> = listed.iter().map(McpTool::name).collect();
    assert_eq!(names, listed_names, "CT-6 tools/list");
    for (tool, declared) in tools.iter().zip(&listed) {
        let wire = serde_json::to_value(tool).expect("a tool serializes");
        let annotations = &wire["annotations"];
        match declared.effect() {
            Effect::Read => assert_eq!(annotations["readOnlyHint"], true, "CT-6 {wire}"),
            Effect::Write {
                destructive,
                idempotent,
            } => {
                assert_eq!(annotations["readOnlyHint"], false, "CT-12 {wire}");
                assert_eq!(annotations["destructiveHint"], destructive, "CT-12 {wire}");
                assert_eq!(annotations["idempotentHint"], idempotent, "CT-12 {wire}");
            }
        }
        assert_eq!(
            wire["inputSchema"]["additionalProperties"], false,
            "CT-6 {wire}"
        );
        let mut expected: Vec<(String, &str)> = declared
            .params()
            .iter()
            .map(|param| (param.name.to_owned(), param.kind.json_type()))
            .collect();
        expected.sort();
        let schema: Vec<(String, &str)> = wire["inputSchema"]["properties"]
            .as_object()
            .map(|properties| {
                properties
                    .iter()
                    .map(|(key, value)| (key.clone(), value["type"].as_str().unwrap_or("")))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(schema, expected, "CT-6 {wire}");
        for route in operations
            .iter()
            .filter(|route| route.operation == Some(declared.name()))
        {
            assert_eq!(route.params, declared.params(), "CT-6 {}", declared.name());
            assert_eq!(
                route.effect,
                Some(declared.effect()),
                "CT-12 {}",
                declared.name()
            );
        }
        let refused = refused_arguments(declared.params());
        for args in std::iter::once(json!({})).chain(refused.iter().cloned()) {
            let context = format!("CT-3 {} {args}", tool.name);
            assert_tool_error(
                &call(&denied_server, &tool.name, args).await,
                "forbidden",
                &context,
            );
        }
        for args in refused {
            let context = format!("CT-4 {} {args}", tool.name);
            assert_tool_error(
                &call(&server, &tool.name, args).await,
                "invalid_request",
                &context,
            );
        }
        // CT-12, second half: the write's sample succeeds as a tool call.
        if declared.effect().is_write() {
            let sample = sample_of(declared.name()).unwrap_or("{}");
            let args: Value = serde_json::from_str(sample)
                .unwrap_or_else(|_| panic!("the sample of {} must be JSON", declared.name()));
            let result = call(&server, declared.name(), args).await;
            assert_ne!(
                result.is_error,
                Some(true),
                "CT-12 {}: the sample must succeed as a tool call",
                declared.name()
            );
        }
    }
    let unknown = CallToolRequestParams::new("zz_unknown_tool");
    match server.call(&unknown).await {
        Ok(_) => panic!("CT-6 an unknown tool returned a result"),
        Err(error) => {
            assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "CT-6");
            assert!(!error.message.contains("zz_unknown_tool"), "CT-6");
        }
    }
}

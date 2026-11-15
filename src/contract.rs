//! Senu's contract for declared operations, as a kit for your own tests.
//!
//! [`check`] takes your route table and tool list and sends real requests
//! through the same router and MCP server that production uses. It panics,
//! naming the route or tool, when one of these does not hold:
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
//! - **CT-6** Exactly the listed tools exist, each read-only with a closed
//!   schema that matches the route's parameters; an unknown tool is an
//!   error that does not repeat its name.
//! - **CT-7** Pages and errors carry `Cache-Control: no-store`,
//!   `X-Content-Type-Options: nosniff` and, for pages, the table's CSP.
//!
//! The kit brings its own Basic and Bearer credentials, because the
//! contract is about the adapters and your declarations, not about your
//! authenticator; test that one separately. `custom` routes and assets are
//! not covered: they are yours.
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
//!         samples: &[("get_stock", "sku=SKU-1")],
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
use crate::operation::{Principal, TenantId};

/// The kit's own credentials. They exist only inside the test process.
const WEB_USER: &str = "contract-user";
const WEB_PASSWORD: &str = "contract-web-password-5T";
const API_TOKEN: &str = "contract-api-token-8R";

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
    /// For each listed operation, its name and a query string that
    /// succeeds against [`state`](Self::state), such as
    /// `("get_stock", "sku=SKU-1")`.
    pub samples: &'static [(&'static str, &'static str)],
}

impl<S: Send + Sync + 'static> Subject<S> {
    fn router(&self, principal: &Principal) -> Router {
        let secret = |value: &str| Secret::new(value).expect("the kit's credentials are valid");
        let state = AppState::builder((self.state)())
            .html_auth(Basic::new(
                secret(WEB_USER),
                secret(WEB_PASSWORD),
                principal.clone(),
            ))
            .api_auth(Bearer::new(secret(API_TOKEN), principal.clone()))
            .build();
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

fn request(method: &str, target: &str, authorizations: &[&str]) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(target);
    for authorization in authorizations {
        builder = builder.header(header::AUTHORIZATION, *authorization);
    }
    builder.body(Body::empty()).expect("a valid request")
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

fn credential(kind: Kind) -> String {
    match kind {
        Kind::Html => basic(WEB_USER, WEB_PASSWORD),
        _ => bearer(API_TOKEN),
    }
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

/// MCP arguments that the envelope refuses: an unknown field and, for each
/// parameter, `null`, a number and an array.
fn refused_arguments(params: &[String]) -> Vec<Value> {
    let mut arguments = vec![json!({ "zz_unknown": "x" })];
    for name in params {
        for value in [Value::Null, json!(7), json!(["a"])] {
            arguments.push(json!({ name.as_str(): value }));
        }
    }
    arguments
}

/// Runs CT-1 to CT-7 over every HTML and API route and every tool that
/// `subject` lists.
///
/// # Panics
///
/// When the contract does not hold, with the route or tool in the message;
/// also when the table or the list is invalid, or an operation has no
/// sample.
pub async fn check<S: Send + Sync + 'static>(subject: Subject<S>) {
    let nobody = Principal::new("nobody", TenantId::from_static("contract"), &[]);
    let app = subject.router(&subject.principal);
    let denied = subject.router(&nobody);
    let server = subject.mcp(&subject.principal);
    let denied_server = subject.mcp(&nobody);
    let endpoints = (subject.endpoints)();
    let csp = endpoints.csp();
    let routes: Vec<Declared> = endpoints
        .declared()
        .into_iter()
        .filter(|route| matches!(route.kind, Kind::Html | Kind::Api))
        .collect();
    let tools = server.tools();
    assert!(
        !routes.is_empty() || !tools.is_empty(),
        "the application lists no operation route and no tool"
    );

    for route in &routes {
        let (path, kind) = (route.path, route.kind);
        let good = credential(kind);
        let (challenge, wrong) = match kind {
            Kind::Html => (
                "Basic realm=\"senu\", charset=\"UTF-8\"",
                vec![
                    bearer(API_TOKEN),
                    basic(WEB_USER, API_TOKEN),
                    basic("someone", WEB_PASSWORD),
                ],
            ),
            _ => (
                "Bearer realm=\"senu\"",
                vec![
                    basic(WEB_USER, WEB_PASSWORD),
                    bearer(WEB_PASSWORD),
                    format!("Token {API_TOKEN}"),
                ],
            ),
        };

        // CT-1: missing, wrong or duplicate credentials, before permission
        // and input.
        let mut attempts: Vec<Vec<&str>> = vec![vec![], vec![good.as_str(), good.as_str()]];
        attempts.extend(wrong.iter().map(|value| vec![value.as_str()]));
        for authorizations in attempts {
            for (router, query) in [(&app, ""), (&denied, "?zz_unknown=1")] {
                let target = format!("{path}{query}");
                let reply = send(router, request("GET", &target, &authorizations)).await;
                assert_error(
                    &reply,
                    kind,
                    StatusCode::UNAUTHORIZED,
                    "unauthorized",
                    &format!("CT-1 {target}"),
                );
                assert_eq!(
                    reply.header(header::WWW_AUTHENTICATE),
                    challenge,
                    "CT-1 {target}"
                );
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
        let reply = send(&app, request("GET", &long, &[&good])).await;
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
            let reply = send(&denied, request("GET", &target, &[&good])).await;
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
            let reply = send(&app, request("GET", &target, &[&good])).await;
            assert_error(
                &reply,
                kind,
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &format!("CT-4 {target}"),
            );
        }
    }

    // CT-5 and CT-7: empty arguments and the application's sample give the
    // same outcome on every transport that lists the operation, and pages
    // keep their headers.
    let sampled = |name: &str| subject.samples.iter().any(|(sample, _)| *sample == name);
    for route in &routes {
        let name = route.operation.unwrap_or("");
        assert!(sampled(name), "CT-5 the operation {name} has no sample");
    }
    for tool in &tools {
        assert!(
            sampled(&tool.name),
            "CT-5 the tool {} has no sample",
            tool.name
        );
    }
    for &(name, sample) in subject.samples {
        let path = |kind| {
            routes
                .iter()
                .find(|route| route.kind == kind && route.operation == Some(name))
                .map(|route| route.path)
        };
        let (api_path, html_path) = (path(Kind::Api), path(Kind::Html));
        let is_tool = tools.iter().any(|tool| tool.name == name);
        assert!(
            api_path.is_some() || html_path.is_some() || is_tool,
            "CT-5 the sample {name} names no listed operation"
        );
        for query in ["", sample] {
            let context = format!("CT-5 {name}?{query}");
            // The outcome as (succeeded, JSON), from the API and from MCP.
            let mut outcome: Option<(bool, Value)> = None;
            if let Some(api_path) = api_path {
                let target = format!("{api_path}?{query}");
                let api = send(&app, request("GET", &target, &[&bearer(API_TOKEN)])).await;
                let body: Value = serde_json::from_str(&api.body)
                    .unwrap_or_else(|_| panic!("{context}: the API must answer JSON"));
                outcome = Some((api.status == StatusCode::OK, body));
            }
            if is_tool {
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
                let page = send(&app, request("GET", &target, &[&credential(Kind::Html)])).await;
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

    // CT-6: only the listed tools, each read-only with a closed schema that
    // matches its routes' parameters; CT-3 and CT-4 over MCP.
    let listed: Vec<String> = (subject.tools)()
        .iter()
        .map(|tool| tool.name().to_owned())
        .collect();
    let names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
    assert_eq!(names, listed, "CT-6 tools/list");
    for tool in &tools {
        let wire = serde_json::to_value(tool).expect("a tool serializes");
        assert_eq!(wire["annotations"]["readOnlyHint"], true, "CT-6 {wire}");
        assert_eq!(
            wire["inputSchema"]["additionalProperties"], false,
            "CT-6 {wire}"
        );
        let schema: Vec<String> = wire["inputSchema"]["properties"]
            .as_object()
            .map(|properties| properties.keys().cloned().collect())
            .unwrap_or_default();
        for route in routes
            .iter()
            .filter(|route| route.operation == Some(&*tool.name))
        {
            let mut declared: Vec<&str> = route.params.iter().map(|param| param.name).collect();
            declared.sort_unstable();
            assert_eq!(schema, declared, "CT-6 {wire}");
        }
        let refused = refused_arguments(&schema);
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

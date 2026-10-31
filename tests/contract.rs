//! Senu's contract for declared operations (spec 0005, §8): the call order,
//! the parameter envelope and its schema, route and tool validation, and
//! CT-1 to CT-7 over every route and tool that the demo lists and over
//! `Stock`, a test-only application that shares nothing with the catalogue
//! but the principal type (GEN-1).

mod support;

use std::process::ExitCode;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::get;
use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode};
use senu::catalogue::{Catalogue, TenantId};
use senu::config::{ServeConfig, serve_config};
use senu::demo;
use senu::http::{
    AppState, Declared, Endpoints, Kind, MAX_TARGET_BYTES, RouteError, query_arguments,
};
use senu::mcp::{self, McpServer, McpTool, ToolError};
use senu::operation::{Args, OpError, Operation, Param, Permission, Principal, decode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use support::*;
use tower::ServiceExt as _;

// The test-only application. Its author writes only this section, down to
// `stock_tools`, and changes nothing in Senu.

struct Stock(&'static [(&'static str, u32)]);

fn stock() -> Stock {
    Stock(&[("SKU-1", 12), ("SKU-2", 0)])
}

const VIEW_STOCK: Permission = Permission::new("view_stock");

const CLERK: Principal = Principal::new("clerk", TenantId::ACME, &[VIEW_STOCK]);

#[derive(Debug, PartialEq, Serialize)]
struct Level {
    sku: String,
    units: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelArgs {
    sku: String,
}

const GET_STOCK: Operation<Stock, Level> = Operation {
    name: "get_stock",
    description: "Units in stock for one SKU.",
    permission: VIEW_STOCK,
    params: &[Param {
        name: "sku",
        description: "The SKU: 1 to 16 characters of A-Z, 0-9 and '-'.",
        required: true,
    }],
    run: get_stock,
};

/// Declared but listed nowhere, so it must stay unreachable (CT-6).
const STOCK_REPORT: Operation<Stock, Level> = Operation {
    name: "stock_report",
    description: "Not exposed.",
    permission: VIEW_STOCK,
    params: &[],
    run: unlisted,
};

fn get_stock(stock: &Stock, _principal: &Principal, args: &Args) -> Result<Level, OpError> {
    let LevelArgs { sku } = decode(args)?;
    let well_formed = (1..=16).contains(&sku.len())
        && sku
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
    if !well_formed {
        return Err(OpError::Invalid {
            code: "invalid_sku",
            phrase: "Invalid SKU",
        });
    }
    let (sku, units) = stock
        .0
        .iter()
        .find(|(known, _)| *known == sku)
        .ok_or(OpError::NotFound)?;
    Ok(Level {
        sku: (*sku).to_owned(),
        units: *units,
    })
}

fn unlisted(_: &Stock, _: &Principal, _: &Args) -> Result<Level, OpError> {
    unreachable!("an unlisted operation ran")
}

/// Only a number reaches the page, so there is nothing to escape.
fn stock_page(_args: &Args, level: &Level) -> Result<String, askama::Error> {
    Ok(format!(
        "<!DOCTYPE html>\n<title>Stock</title>\n<p>{} units</p>\n",
        level.units
    ))
}

fn stock_endpoints() -> Endpoints<Stock> {
    Endpoints::new()
        .html("/stock", &GET_STOCK, stock_page)
        .api("/api/stock", &GET_STOCK)
}

fn stock_tools() -> Vec<McpTool<Stock>> {
    vec![mcp::tool(&GET_STOCK)]
}

// What the contract needs from an application.

struct Subject<S> {
    state: fn() -> S,
    endpoints: fn() -> Endpoints<S>,
    tools: fn() -> Vec<McpTool<S>>,
    /// Holds every listed operation's permission.
    principal: Principal,
    /// The application's own samples: a query that succeeds, per operation.
    samples: &'static [(&'static str, &'static str)],
}

fn demo_subject() -> Subject<Catalogue> {
    Subject {
        state: Catalogue::demo,
        endpoints: demo::endpoints,
        tools: demo::tools,
        principal: Principal::demo_api(),
        samples: &[
            ("list_projects", "q=launch"),
            ("get_project", "id=acme-002"),
        ],
    }
}

fn stock_subject() -> Subject<Stock> {
    Subject {
        state: stock,
        endpoints: stock_endpoints,
        tools: stock_tools,
        principal: CLERK,
        samples: &[("get_stock", "sku=SKU-1")],
    }
}

const NOBODY: Principal = Principal::new("nobody", TenantId::ACME, &[]);

fn config() -> ServeConfig {
    serve_config(serve_env()).expect("valid test configuration")
}

impl<S: Send + Sync + 'static> Subject<S> {
    fn router(&self, principal: &Principal) -> Router {
        let state = AppState::with_principals(
            (self.state)(),
            &config(),
            principal.clone(),
            principal.clone(),
        );
        (self.endpoints)()
            .into_router(state)
            .expect("a valid table")
    }

    fn mcp(&self, principal: &Principal) -> McpServer<S> {
        McpServer::with_tools((self.state)(), principal.clone(), (self.tools)())
            .expect("a valid tool list")
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
            .map_or("", |value| value.to_str().unwrap())
    }
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

fn request(method: &str, target: &str, authorizations: &[&str]) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(target);
    for authorization in authorizations {
        builder = builder.header(header::AUTHORIZATION, *authorization);
    }
    builder.body(Body::empty()).unwrap()
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
            "{context}"
        );
        let body: Value = serde_json::from_str(&reply.body).expect("JSON error");
        assert_eq!(body, json!({ "error": code }), "{context}");
    } else {
        assert!(
            reply.header(header::CONTENT_TYPE).starts_with("text/plain"),
            "{context}"
        );
    }
    assert_eq!(reply.header(header::CACHE_CONTROL), "no-store", "{context}");
    assert_eq!(
        reply.header(header::X_CONTENT_TYPE_OPTIONS),
        "nosniff",
        "{context}"
    );
}

fn call<S>(server: &McpServer<S>, name: &str, args: Value) -> CallToolResult {
    let Value::Object(args) = args else {
        panic!("arguments must be an object")
    };
    let request = CallToolRequestParams::new(name.to_owned()).with_arguments(args);
    server.call(&request).expect("a tool result")
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
fn refused_queries(params: &[Param]) -> Vec<String> {
    let mut queries = vec!["zz_unknown=1".to_owned()];
    queries.extend(
        params
            .iter()
            .map(|param| format!("{0}=a&{0}=b", param.name)),
    );
    queries
}

/// MCP arguments that the envelope refuses: an unknown field and, for each
/// parameter, `null`, a number and an array.
fn refused_arguments(params: &[Param]) -> Vec<Value> {
    let mut arguments = vec![json!({ "zz_unknown": "x" })];
    for param in params {
        for value in [Value::Null, json!(7), json!(["a"])] {
            arguments.push(json!({ param.name: value }));
        }
    }
    arguments
}

/// CT-1 to CT-7 for every declared HTML and API route and every listed tool.
async fn check_contract<S: Send + Sync + 'static>(subject: Subject<S>) {
    let app = subject.router(&subject.principal);
    let denied = subject.router(&NOBODY);
    let server = subject.mcp(&subject.principal);
    let denied_server = subject.mcp(&NOBODY);
    let routes: Vec<Declared> = (subject.endpoints)()
        .declared()
        .into_iter()
        .filter(|route| route.kind != Kind::Custom)
        .collect();
    assert!(!routes.is_empty());

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
                    &target,
                );
                assert_eq!(
                    reply.header(header::WWW_AUTHENTICATE),
                    challenge,
                    "{target}"
                );
            }
        }

        // CT-2: other methods, bodies and long targets, before credentials.
        for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let reply = send(&app, request(method, path, &[])).await;
            assert_eq!(
                reply.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
            assert_eq!(reply.header(header::ALLOW), "GET", "{method} {path}");
        }
        let with_body = Request::builder()
            .uri(path)
            .header(header::CONTENT_LENGTH, "1")
            .body(Body::from("x"))
            .unwrap();
        let reply = send(&app, with_body).await;
        assert_error(
            &reply,
            kind,
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            path,
        );
        let long = format!("{path}?pad={}", "a".repeat(MAX_TARGET_BYTES));
        let reply = send(&app, request("GET", &long, &[&good])).await;
        assert_error(&reply, kind, StatusCode::URI_TOO_LONG, "uri_too_long", path);

        // CT-3 and CT-4: the permission before the envelope.
        let refused = refused_queries(route.params);
        for query in std::iter::once(String::new()).chain(refused.iter().cloned()) {
            let target = format!("{path}?{query}");
            let reply = send(&denied, request("GET", &target, &[&good])).await;
            assert_error(&reply, kind, StatusCode::FORBIDDEN, "forbidden", &target);
        }
        for query in &refused {
            let target = format!("{path}?{query}");
            let reply = send(&app, request("GET", &target, &[&good])).await;
            assert_error(
                &reply,
                kind,
                StatusCode::BAD_REQUEST,
                "invalid_request",
                &target,
            );
        }
    }

    // CT-5 and CT-7: empty arguments and the application's sample give the
    // same outcome on all three transports, and pages keep their headers.
    let tools = server.tools();
    for &(name, sample) in subject.samples {
        let path = |kind| {
            routes
                .iter()
                .find(|route| route.kind == kind && route.operation == Some(name))
                .map(|route| route.path)
        };
        let api_path = path(Kind::Api).expect("an API route");
        assert!(
            tools.iter().any(|tool| tool.name == name),
            "{name} is not a tool"
        );
        for query in ["", sample] {
            let target = format!("{api_path}?{query}");
            let api = send(&app, request("GET", &target, &[&bearer(API_TOKEN)])).await;
            let api_json: Value = serde_json::from_str(&api.body).expect("JSON");
            let args = Value::Object(query_arguments(Some(query)));
            let result = call(&server, name, args);
            assert_eq!(
                result.is_error,
                Some(api.status != StatusCode::OK),
                "{target}"
            );
            assert_eq!(result.structured_content, Some(api_json), "{target}");
            if query == sample {
                assert_eq!(api.status, StatusCode::OK, "{target}");
            }
            if let Some(html_path) = path(Kind::Html) {
                let target = format!("{html_path}?{query}");
                let page = send(&app, request("GET", &target, &[&credential(Kind::Html)])).await;
                assert_eq!(page.status, api.status, "{target}");
                if page.status == StatusCode::OK {
                    assert!(page.header(header::CONTENT_TYPE).starts_with("text/html"));
                    assert_eq!(
                        page.header(header::CONTENT_SECURITY_POLICY),
                        "default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
                    );
                    assert_eq!(page.header(header::CACHE_CONTROL), "no-store");
                    assert_eq!(page.header(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
                }
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
    assert_eq!(names, listed);
    for tool in &tools {
        let wire = serde_json::to_value(tool).unwrap();
        assert_eq!(wire["annotations"]["readOnlyHint"], true, "{wire}");
        assert_eq!(wire["inputSchema"]["additionalProperties"], false, "{wire}");
        for route in routes
            .iter()
            .filter(|route| route.operation == Some(&*tool.name))
        {
            let properties: Vec<&str> = route.params.iter().map(|param| param.name).collect();
            let schema: Vec<&str> = wire["inputSchema"]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            let mut sorted = properties.clone();
            sorted.sort();
            assert_eq!(schema, sorted, "{wire}");
        }
        let params: Vec<Param> = routes
            .iter()
            .find(|route| route.operation == Some(&*tool.name))
            .map_or(Vec::new(), |route| route.params.to_vec());
        let refused = refused_arguments(&params);
        for args in std::iter::once(json!({})).chain(refused.iter().cloned()) {
            let context = format!("{} {args}", tool.name);
            assert_tool_error(
                &call(&denied_server, &tool.name, args),
                "forbidden",
                &context,
            );
        }
        for args in refused {
            let context = format!("{} {args}", tool.name);
            assert_tool_error(
                &call(&server, &tool.name, args),
                "invalid_request",
                &context,
            );
        }
    }
    let unknown = CallToolRequestParams::new("zz_unknown_tool");
    let error = server.call(&unknown).expect_err("an unknown tool");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(!error.message.contains("zz_unknown_tool"));
}

#[tokio::test]
async fn the_demo_meets_the_contract() {
    check_contract(demo_subject()).await;
}

#[tokio::test]
async fn a_second_application_meets_the_contract_without_changes_to_senu() {
    check_contract(stock_subject()).await;
}

/// The fixture's own business rules, including an error of its own.
#[tokio::test]
async fn stock_levels_and_the_invalid_sku_error_on_all_three_transports() {
    let found = (GET_STOCK.run)(&stock(), &CLERK, &query_arguments(Some("sku=SKU-1")));
    let expected = Level {
        sku: "SKU-1".to_owned(),
        units: 12,
    };
    assert_eq!(found, Ok(expected));
    let invalid = OpError::Invalid {
        code: "invalid_sku",
        phrase: "Invalid SKU",
    };
    for (query, error) in [
        ("sku=sku-1", invalid),
        ("sku=", invalid),
        ("sku=SKU-9", OpError::NotFound),
    ] {
        let result = GET_STOCK.call(&stock(), &CLERK, &query_arguments(Some(query)));
        assert_eq!(result, Err(error), "{query}");
    }
    let wrong = json!({ "sku": 7 });
    assert_eq!(
        decode::<LevelArgs>(wrong.as_object().unwrap()).err(),
        Some(OpError::InvalidRequest)
    );

    let subject = stock_subject();
    let app = subject.router(&CLERK);
    let api = send(
        &app,
        request("GET", "/api/stock?sku=sku-1", &[&bearer(API_TOKEN)]),
    )
    .await;
    assert_error(
        &api,
        Kind::Api,
        StatusCode::BAD_REQUEST,
        "invalid_sku",
        "API",
    );
    let page = send(
        &app,
        request("GET", "/stock?sku=sku-1", &[&credential(Kind::Html)]),
    )
    .await;
    assert_error(
        &page,
        Kind::Html,
        StatusCode::BAD_REQUEST,
        "invalid_sku",
        "HTML",
    );
    assert_eq!(page.body, "Invalid SKU");
    let missing = send(
        &app,
        request("GET", "/api/stock?sku=SKU-9", &[&bearer(API_TOKEN)]),
    )
    .await;
    assert_error(
        &missing,
        Kind::Api,
        StatusCode::NOT_FOUND,
        "not_found",
        "API",
    );
    let result = call(&subject.mcp(&CLERK), "get_stock", json!({ "sku": "sku-1" }));
    assert_tool_error(&result, "invalid_sku", "MCP");
    let page = send(
        &app,
        request("GET", "/stock?sku=SKU-2", &[&credential(Kind::Html)]),
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("<p>0 units</p>"), "{}", page.body);
}

// Unit tests of the call order, the envelope and the schema.

const ECHO_PERMISSION: Permission = Permission::new("echo");

const ECHO_USER: Principal = Principal::new("echo-user", TenantId::ACME, &[ECHO_PERMISSION]);

const ECHO: Operation<(), Args> = Operation {
    name: "echo",
    description: "Returns its arguments.",
    permission: ECHO_PERMISSION,
    params: &[
        Param {
            name: "a",
            description: "Required.",
            required: true,
        },
        Param {
            name: "b",
            description: "Optional.",
            required: false,
        },
    ],
    run: echo,
};

/// The same declaration, but `run` must never be reached.
const EXPLODE: Operation<(), Args> = Operation {
    run: explode,
    ..ECHO
};

fn echo(_: &(), _: &Principal, args: &Args) -> Result<Args, OpError> {
    Ok(args.clone())
}

fn explode(_: &(), _: &Principal, _: &Args) -> Result<Args, OpError> {
    panic!("run was called")
}

fn envelope_cases() -> Vec<(Value, bool)> {
    vec![
        (json!({ "a": "x" }), true),
        (json!({ "a": "", "b": "" }), true),
        (json!({ "a": "x", "b": "y" }), true),
        (json!({}), false),
        (json!({ "b": "y" }), false),
        (json!({ "a": null }), false),
        (json!({ "a": 7 }), false),
        (json!({ "a": true }), false),
        (json!({ "a": ["x"] }), false),
        (json!({ "a": { "k": "v" } }), false),
        (json!({ "a": "x", "b": null }), false),
        (json!({ "a": "x", "c": "z" }), false),
        (json!({ "A": "x" }), false),
    ]
}

#[test]
fn call_checks_the_permission_then_the_envelope_and_only_then_runs() {
    const OTHER: Principal =
        Principal::new("other", TenantId::ACME, &[Permission::new("echo_other")]);
    for (args, admitted) in envelope_cases() {
        let args = args.as_object().unwrap().clone();
        for principal in [&NOBODY, &OTHER] {
            assert_eq!(
                EXPLODE.call(&(), principal, &args),
                Err(OpError::Forbidden),
                "{args:?}"
            );
        }
        let expected = if admitted {
            Ok(args.clone())
        } else {
            assert_eq!(
                EXPLODE.call(&(), &ECHO_USER, &args),
                Err(OpError::InvalidRequest)
            );
            Err(OpError::InvalidRequest)
        };
        assert_eq!(ECHO.call(&(), &ECHO_USER, &args), expected, "{args:?}");
    }
}

/// Whether `schema` admits `args`. Only the keywords Senu emits are
/// understood; any other keyword fails the test.
fn schema_admits(schema: &Value, args: &Value) -> bool {
    let schema = schema.as_object().unwrap();
    let args = args.as_object().unwrap();
    let properties = schema["properties"].as_object().unwrap();
    schema.iter().all(|(keyword, rule)| match keyword.as_str() {
        "type" => rule == "object",
        "properties" => {
            args.iter().all(|(key, value)| {
                properties.get(key).is_none_or(|property| {
                    property.as_object().unwrap().iter().all(|(keyword, rule)| {
                        match keyword.as_str() {
                            "type" => rule == "string" && value.is_string(),
                            "description" => true,
                            other => panic!("unexpected keyword {other}"),
                        }
                    })
                })
            })
        }
        "required" => rule
            .as_array()
            .unwrap()
            .iter()
            .all(|name| args.contains_key(name.as_str().unwrap())),
        "additionalProperties" => {
            rule == false && args.keys().all(|key| properties.contains_key(key))
        }
        other => panic!("unexpected keyword {other}"),
    })
}

#[test]
fn tool_schemas_are_built_from_the_declared_parameters_and_agree_with_the_envelope() {
    let server = McpServer::with_tools((), ECHO_USER, vec![mcp::tool(&ECHO)]).unwrap();
    let tool = serde_json::to_value(&server.tools()[0]).unwrap();
    assert_eq!(tool["name"], "echo");
    assert_eq!(tool["description"], "Returns its arguments.");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(
        tool["inputSchema"],
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "string", "description": "Required." },
                "b": { "type": "string", "description": "Optional." },
            },
            "required": ["a"],
            "additionalProperties": false,
        })
    );
    for (args, admitted) in envelope_cases() {
        assert_eq!(
            schema_admits(&tool["inputSchema"], &args),
            admitted,
            "{args}"
        );
        let result = call(&server, "echo", args.clone());
        assert_eq!(result.is_error, Some(!admitted), "{args}");
    }
    const NO_REQUIRED: Operation<(), Args> = Operation {
        params: &[Param {
            name: "b",
            description: "Optional.",
            required: false,
        }],
        ..ECHO
    };
    let server = McpServer::with_tools((), ECHO_USER, vec![mcp::tool(&NO_REQUIRED)]).unwrap();
    let tool = serde_json::to_value(&server.tools()[0]).unwrap();
    assert!(tool["inputSchema"].get("required").is_none(), "{tool}");
}

// Declarations are checked at startup, and an invalid one is an error.

fn table(endpoints: Endpoints<Stock>) -> Result<(), RouteError> {
    let state = AppState::with_principals(stock(), &config(), CLERK, CLERK);
    endpoints.into_router(state).map(|_| ())
}

fn ok_handler() -> axum::routing::MethodRouter<Arc<AppState<Stock>>> {
    get(|| async { "ok" })
}

#[test]
fn route_tables_are_checked_and_never_panic() {
    let leak = |path: String| -> &'static str { path.leak() };
    for path in [
        "",
        "stock",
        "/x/{id}",
        "/x/{*rest}",
        "/{id}",
        "/x}",
        "/x{",
        "/*",
        "/x/*rest",
        "/x?y=1",
        "/x#top",
        "/x%2Fy",
        "/./x",
        "/x/.",
        "/../x",
        "/x/..",
        "/x\\y",
        "/x\ny",
        "/x\u{7f}",
        "/x\u{0}",
        "/ x",
        "/é",
        "/:id",
        "/x/:id",
        "/x/:id/y",
    ] {
        let invalid = Err(RouteError::InvalidPath(path));
        assert_eq!(
            table(Endpoints::new().html(path, &GET_STOCK, stock_page)),
            invalid
        );
        assert_eq!(table(Endpoints::new().custom(path, ok_handler())), invalid);
        let api = leak(format!("/api{path}"));
        if !path.is_empty() && path.starts_with('/') {
            let invalid = Err(RouteError::InvalidPath(api));
            assert_eq!(table(Endpoints::new().api(api, &GET_STOCK)), invalid);
        }
        let message = RouteError::InvalidPath(path).to_string();
        assert!(!message.contains(['\n', '\u{0}', '\u{7f}']), "{message:?}");
    }
    for path in ["/_senu", "/_senu/", "/_senu/ui.css", "/_senu/x"] {
        assert_eq!(
            table(Endpoints::new().html(path, &GET_STOCK, stock_page)),
            Err(RouteError::Reserved(path))
        );
        assert_eq!(
            table(Endpoints::new().custom(path, ok_handler())),
            Err(RouteError::Reserved(path))
        );
    }
    for path in ["/stock", "/api", "/apiary/x"] {
        assert_eq!(
            table(Endpoints::new().api(path, &GET_STOCK)),
            Err(RouteError::WrongPrefix(path))
        );
    }
    for path in ["/api/stock", "/api"] {
        let endpoints = Endpoints::new().html(path, &GET_STOCK, stock_page);
        assert_eq!(table(endpoints), Err(RouteError::WrongPrefix(path)));
    }
    let twice = Endpoints::new().html("/stock", &GET_STOCK, stock_page);
    assert_eq!(
        table(twice.html("/stock", &GET_STOCK, stock_page)),
        Err(RouteError::Duplicate("/stock"))
    );
    let twice = Endpoints::new()
        .api("/api/stock", &GET_STOCK)
        .custom("/api/stock", ok_handler());
    assert_eq!(table(twice), Err(RouteError::Duplicate("/api/stock")));
    for path in ["/", "/apiary", "/stock/", "/a-b_c.d~e"] {
        assert_eq!(
            table(Endpoints::new().html(path, &GET_STOCK, stock_page)),
            Ok(()),
            "{path}"
        );
    }
    assert_eq!(
        table(stock_endpoints().custom("/healthz", ok_handler())),
        Ok(())
    );
}

#[test]
fn operation_metadata_is_checked_before_anything_is_exposed() {
    let long: &'static str = "a".repeat(65).leak();
    let param = |name, description| Param {
        name,
        description,
        required: true,
    };
    let duplicate = vec![param("sku", "One."), param("sku", "Two.")].leak();
    let variants: Vec<Operation<Stock, Level>> = vec![
        Operation {
            name: "",
            ..GET_STOCK
        },
        Operation {
            name: "Get_stock",
            ..GET_STOCK
        },
        Operation {
            name: "get-stock",
            ..GET_STOCK
        },
        Operation {
            name: "1_stock",
            ..GET_STOCK
        },
        Operation {
            name: long,
            ..GET_STOCK
        },
        Operation {
            description: " ",
            ..GET_STOCK
        },
        Operation {
            permission: Permission::new(""),
            ..GET_STOCK
        },
        Operation {
            permission: Permission::new("View stock"),
            ..GET_STOCK
        },
        Operation {
            params: vec![param("", "Empty.")].leak(),
            ..GET_STOCK
        },
        Operation {
            params: vec![param("Sku", "Upper case.")].leak(),
            ..GET_STOCK
        },
        Operation {
            params: vec![param("sku", "")].leak(),
            ..GET_STOCK
        },
        Operation {
            params: duplicate,
            ..GET_STOCK
        },
    ];
    for operation in variants {
        let operation: &'static Operation<Stock, Level> = Box::leak(Box::new(operation));
        let invalid = operation.check().expect_err("invalid metadata");
        assert_eq!(invalid.operation, operation.name);
        let endpoints = Endpoints::new().api("/api/stock", operation);
        assert_eq!(table(endpoints), Err(RouteError::Operation(invalid)));
        let tools = vec![mcp::tool(operation)];
        let refused = McpServer::with_tools(stock(), CLERK, tools).err();
        assert_eq!(refused, Some(ToolError::Operation(invalid)));
    }
    let longest = Operation {
        name: "a".repeat(64).leak(),
        ..GET_STOCK
    };
    assert_eq!(longest.check(), Ok(()));
    assert_eq!(GET_STOCK.check(), Ok(()));
    assert!(McpServer::with_tools(Catalogue::demo(), Principal::demo_mcp(), demo::tools()).is_ok());
}

#[test]
fn tool_lists_reject_duplicate_names() {
    let tools = vec![mcp::tool(&GET_STOCK), mcp::tool(&GET_STOCK)];
    let refused = McpServer::with_tools(stock(), CLERK, tools).err();
    assert_eq!(refused, Some(ToolError::Duplicate("get_stock")));
}

#[test]
fn invalid_tables_and_tool_lists_exit_with_status_2_at_startup() {
    let config = serve_config(env(&[
        ("SENU_BIND", "127.0.0.1:0"),
        ("SENU_WEB_USER", WEB_USER),
        ("SENU_WEB_PASSWORD", WEB_PASSWORD),
        ("SENU_API_TOKEN", API_TOKEN),
    ]))
    .unwrap();
    let state = AppState::with_principals(stock(), &config, CLERK, CLERK);
    let twice = Endpoints::new()
        .api("/api/stock", &GET_STOCK)
        .api("/api/stock", &GET_STOCK);
    assert_eq!(senu::app::serve(&config, state, twice), ExitCode::from(2));
    let tools = vec![mcp::tool(&GET_STOCK), mcp::tool(&GET_STOCK)];
    assert_eq!(senu::app::mcp(stock(), CLERK, tools), ExitCode::from(2));
}

// DEC-1 and CT-6: exposure is opt-in.

#[tokio::test]
async fn declared_but_unlisted_operations_are_neither_routed_nor_callable() {
    assert_eq!(STOCK_REPORT.check(), Ok(()));
    let subject = stock_subject();
    let declared = stock_endpoints().declared();
    assert!(
        declared
            .iter()
            .all(|route| route.operation == Some(GET_STOCK.name))
    );
    let app = subject.router(&CLERK);
    for (path, kind) in [
        ("/stock_report", Kind::Html),
        ("/api/stock_report", Kind::Api),
    ] {
        let reply = send(&app, request("GET", path, &[&credential(kind)])).await;
        assert_error(&reply, kind, StatusCode::NOT_FOUND, "not_found", path);
    }
    let server = subject.mcp(&CLERK);
    let names: Vec<String> = server
        .tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names, ["get_stock"]);
    let request = CallToolRequestParams::new(STOCK_REPORT.name);
    assert_eq!(
        server.call(&request).unwrap_err().code,
        ErrorCode::INVALID_PARAMS
    );

    let demo_tools: Vec<String> = demo::tools()
        .iter()
        .map(|tool| tool.name().to_owned())
        .collect();
    assert_eq!(demo_tools, ["list_projects", "get_project"]);
}

/// The override hatch: limits and headers, but authentication is the
/// handler's own job, unlike on a declared route.
#[tokio::test]
async fn custom_routes_get_the_layers_but_no_authentication() {
    let count = get(|State(app): State<Arc<AppState<Stock>>>| async move {
        app.state().0.len().to_string()
    });
    let endpoints = stock_endpoints()
        .custom("/healthz", ok_handler())
        .custom("/api/skus", count);
    assert_eq!(
        endpoints.declared().last(),
        Some(&Declared {
            path: "/api/skus",
            kind: Kind::Custom,
            operation: None,
            params: &[],
        })
    );
    let state = AppState::with_principals(stock(), &config(), CLERK, CLERK);
    let app = endpoints.into_router(state).unwrap();
    for (path, body) in [("/healthz", "ok"), ("/api/skus", "2")] {
        let reply = send(&app, request("GET", path, &[])).await;
        assert_eq!(reply.status, StatusCode::OK, "{path}");
        assert_eq!(reply.body, body);
        assert_eq!(reply.header(header::CACHE_CONTROL), "no-store");
        assert_eq!(reply.header(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert_eq!(reply.header(header::WWW_AUTHENTICATE), "");
        assert_eq!(reply.header(header::CONTENT_SECURITY_POLICY), "");
        let with_body = Request::builder()
            .uri(path)
            .header(header::CONTENT_LENGTH, "1")
            .body(Body::from("x"))
            .unwrap();
        assert_eq!(
            send(&app, with_body).await.status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        let long = format!("{path}?pad={}", "a".repeat(MAX_TARGET_BYTES));
        assert_eq!(
            send(&app, request("GET", &long, &[])).await.status,
            StatusCode::URI_TOO_LONG
        );
    }
    let reply = send(&app, request("GET", "/api/stock?sku=SKU-1", &[])).await;
    assert_error(
        &reply,
        Kind::Api,
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "declared",
    );
}

//! Adapter tests for the HTTP router, called in-process with `oneshot`.

mod support;

use axum::Router;
use axum::body::Body;
use axum::http::uri::PathAndQuery;
use axum::http::{HeaderMap, Request, StatusCode, Uri, header};
use senu::catalogue::{Catalogue, TenantId};
use senu::config::serve_config;
use senu::http::{AppState, MAX_TARGET_BYTES, router};
use senu::operation::Principal;
use serde_json::{Value, json};
use support::*;
use tower::ServiceExt as _;

const API: &str = "/api/projects";
const HTML: &str = "/projects";
const BASIC_CHALLENGE: &str = "Basic realm=\"senu\", charset=\"UTF-8\"";
const BEARER_CHALLENGE: &str = "Bearer realm=\"senu\"";

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("JSON body")
    }

    fn header(&self, name: header::HeaderName) -> &str {
        self.headers
            .get(&name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .to_str()
            .unwrap()
    }
}

fn app() -> Router {
    let config = serve_config(serve_env()).expect("valid test configuration");
    router(AppState::new(Catalogue::demo(), &config))
}

fn app_without_permissions() -> Router {
    let config = serve_config(serve_env()).expect("valid test configuration");
    let nobody = Principal::new("nobody", TenantId::ACME, &[]);
    router(AppState::with_principals(
        Catalogue::demo(),
        &config,
        nobody.clone(),
        nobody,
    ))
}

fn get(uri: &str, authorization: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder.body(Body::empty()).unwrap()
}

fn api_get(uri: &str) -> Request<Body> {
    get(uri, Some(&bearer(API_TOKEN)))
}

fn html_get(uri: &str) -> Request<Body> {
    get(uri, Some(&basic(WEB_USER, WEB_PASSWORD)))
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).expect("UTF-8 body"),
    }
}

fn assert_api_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert!(
        reply
            .header(header::CONTENT_TYPE)
            .starts_with("application/json")
    );
    assert_eq!(reply.json(), json!({ "error": code }));
}

fn assert_text_error(reply: &Reply, status: StatusCode, phrase: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert!(reply.header(header::CONTENT_TYPE).starts_with("text/plain"));
    assert_eq!(reply.body, phrase);
}

/// The item text exactly as served, before entity decoding.
fn raw_item_text<'a>(page: &'a str, id: &str) -> &'a str {
    let open = format!("<li data-project-id=\"{id}\">");
    let start = page.find(&open).expect("item") + open.len();
    let end = start + page[start..].find("</li>").expect("end of item");
    &page[start..end]
}

fn ids(list: &Value) -> Vec<&str> {
    list["projects"]
        .as_array()
        .expect("projects array")
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn api_lists_projects_for_a_valid_bearer_token() {
    let reply = send(&app(), api_get("/api/projects?q=launch")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(
        reply
            .header(header::CONTENT_TYPE)
            .starts_with("application/json")
    );
    assert_eq!(
        reply.json(),
        json!({"projects": [
            {"id": "acme-001", "name": "Apollo Launch"},
            {"id": "acme-003", "name": "<script>alert('x')</script> & \"Launch\" Notes"},
        ]})
    );
}

#[tokio::test]
async fn html_lists_projects_for_valid_basic_credentials() {
    let reply = send(&app(), html_get("/projects")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.header(header::CONTENT_TYPE).starts_with("text/html"));
    let item_ids: Vec<String> = html_items(&reply.body)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(item_ids, ["acme-001", "acme-002", "acme-003"]);
    assert!(reply.body.contains("Local demo, not production"));
    assert_eq!(
        reply.header(header::CONTENT_SECURITY_POLICY),
        "default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
    );
}

#[tokio::test]
async fn api_rejects_missing_wrong_and_cross_transport_credentials() {
    let app = app();
    let cases: Vec<(&str, Option<String>)> = vec![
        ("missing", None),
        ("wrong token", Some(bearer("not-the-token"))),
        ("web password as bearer", Some(bearer(WEB_PASSWORD))),
        ("mcp token as bearer", Some(bearer(MCP_TOKEN))),
        (
            "basic with web credentials",
            Some(basic(WEB_USER, WEB_PASSWORD)),
        ),
        ("basic with api token", Some(basic(WEB_USER, API_TOKEN))),
        ("basic with mcp token", Some(basic(WEB_USER, MCP_TOKEN))),
        ("unknown scheme", Some(format!("Token {API_TOKEN}"))),
    ];
    for (case, authorization) in cases {
        let reply = send(&app, get(API, authorization.as_deref())).await;
        assert_api_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
        assert_eq!(
            reply.header(header::WWW_AUTHENTICATE),
            BEARER_CHALLENGE,
            "{case}"
        );
    }

    let query_token = format!("/api/projects?access_token={API_TOKEN}");
    let reply = send(&app, get(&query_token, None)).await;
    assert_api_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");

    let twice = Request::builder()
        .uri(API)
        .header(header::AUTHORIZATION, bearer(API_TOKEN))
        .header(header::AUTHORIZATION, bearer(API_TOKEN))
        .body(Body::empty())
        .unwrap();
    assert_api_error(
        &send(&app, twice).await,
        StatusCode::UNAUTHORIZED,
        "unauthorized",
    );
}

#[tokio::test]
async fn html_rejects_missing_wrong_and_cross_transport_credentials() {
    let app = app();
    let cases: Vec<(&str, Option<String>)> = vec![
        ("missing", None),
        ("api token as bearer", Some(bearer(API_TOKEN))),
        ("web password as bearer", Some(bearer(WEB_PASSWORD))),
        ("mcp token as bearer", Some(bearer(MCP_TOKEN))),
        ("wrong password", Some(basic(WEB_USER, "not-the-password"))),
        ("wrong user", Some(basic("someone", WEB_PASSWORD))),
        ("api token as password", Some(basic(WEB_USER, API_TOKEN))),
        ("mcp token as password", Some(basic(WEB_USER, MCP_TOKEN))),
    ];
    for (case, authorization) in cases {
        let reply = send(&app, get(HTML, authorization.as_deref())).await;
        assert_text_error(&reply, StatusCode::UNAUTHORIZED, "Unauthorized");
        assert_eq!(
            reply.header(header::WWW_AUTHENTICATE),
            BASIC_CHALLENGE,
            "{case}"
        );
    }
    let query_password = format!("/projects?password={WEB_PASSWORD}&access_token={API_TOKEN}");
    let reply = send(&app, get(&query_password, None)).await;
    assert_text_error(&reply, StatusCode::UNAUTHORIZED, "Unauthorized");
}

#[tokio::test]
async fn auth_schemes_match_case_insensitively() {
    let app = app();
    let lower = format!("bearer {API_TOKEN}");
    assert_eq!(
        send(&app, get(API, Some(&lower))).await.status,
        StatusCode::OK
    );
    let upper = basic(WEB_USER, WEB_PASSWORD).replacen("Basic", "BASIC", 1);
    assert_eq!(
        send(&app, get(HTML, Some(&upper))).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn authentication_runs_before_any_validation() {
    let app = app();
    let long = "a".repeat(81);
    for query in [
        "?tenant=globex",
        &format!("?q={long}"),
        "?q=a&q=b",
        "?q=%07",
    ] {
        let reply = send(&app, get(&format!("{API}{query}"), None)).await;
        assert_api_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
        let reply = send(&app, get(&format!("{HTML}{query}"), None)).await;
        assert_text_error(&reply, StatusCode::UNAUTHORIZED, "Unauthorized");
    }
}

#[tokio::test]
async fn principal_without_permission_is_forbidden_on_both_routes() {
    let app = app_without_permissions();
    for query in ["", "?tenant=globex", &format!("?q={}", "a".repeat(81))] {
        let reply = send(&app, api_get(&format!("{API}{query}"))).await;
        assert_api_error(&reply, StatusCode::FORBIDDEN, "forbidden");
        let reply = send(&app, html_get(&format!("{HTML}{query}"))).await;
        assert_text_error(&reply, StatusCode::FORBIDDEN, "Forbidden");
    }
}

#[tokio::test]
async fn invalid_search_and_invalid_request_on_both_routes() {
    let app = app();
    let long = "a".repeat(81);
    let cases = [
        (format!("?q={long}"), "invalid_search", "Invalid search"),
        ("?q=a%07b".to_owned(), "invalid_search", "Invalid search"),
        (
            "?tenant=globex".to_owned(),
            "invalid_request",
            "Invalid request",
        ),
        ("?q=a&q=b".to_owned(), "invalid_request", "Invalid request"),
        (
            format!("?access_token={API_TOKEN}"),
            "invalid_request",
            "Invalid request",
        ),
    ];
    for (query, code, phrase) in cases {
        let reply = send(&app, api_get(&format!("{API}{query}"))).await;
        assert_api_error(&reply, StatusCode::BAD_REQUEST, code);
        let reply = send(&app, html_get(&format!("{HTML}{query}"))).await;
        assert_text_error(&reply, StatusCode::BAD_REQUEST, phrase);
    }
}

#[tokio::test]
async fn query_strings_are_percent_and_plus_decoded() {
    let app = app();
    let reply = send(&app, api_get("/api/projects?q=CAF%C3%89+")).await;
    assert_eq!(ids(&reply.json()), ["acme-002"]);
    let reply = send(&app, api_get("/api/projects?q=apollo+launch")).await;
    assert_eq!(ids(&reply.json()), ["acme-001"]);
    let reply = send(&app, api_get("/api/projects?q")).await;
    assert_eq!(ids(&reply.json()), ["acme-001", "acme-002", "acme-003"]);
}

#[tokio::test]
async fn unknown_paths_return_404_before_authentication() {
    let app = app();
    for path in ["/api/unknown", "/api", "/api/projects/acme-001"] {
        assert_api_error(
            &send(&app, get(path, None)).await,
            StatusCode::NOT_FOUND,
            "not_found",
        );
    }
    for path in ["/", "/mcp", "/projects/", "/unknown", "/PROJECTS"] {
        let reply = send(&app, get(path, None)).await;
        assert_text_error(&reply, StatusCode::NOT_FOUND, "Not Found");
    }
}

#[tokio::test]
async fn other_methods_return_405_before_authentication() {
    let app = app();
    for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
        for (path, json) in [(API, true), (HTML, false)] {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap();
            let reply = send(&app, request).await;
            assert_eq!(
                reply.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
            assert_eq!(reply.header(header::ALLOW), "GET");
            if method != "HEAD" {
                if json {
                    assert_api_error(&reply, StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
                } else {
                    assert_text_error(&reply, StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed");
                }
            }
        }
    }
}

#[tokio::test]
async fn request_targets_over_the_limit_return_414() {
    let app = app();
    for (path, json) in [(API, true), (HTML, false), ("/unknown", false)] {
        let prefix = format!("{path}?pad=");
        let at_limit = format!("{prefix}{}", "a".repeat(MAX_TARGET_BYTES - prefix.len()));
        let over = format!("{at_limit}a");
        assert_eq!(at_limit.len(), MAX_TARGET_BYTES);

        let reply = send(&app, get(&over, None)).await;
        if json {
            assert_api_error(&reply, StatusCode::URI_TOO_LONG, "uri_too_long");
        } else {
            assert_text_error(&reply, StatusCode::URI_TOO_LONG, "URI Too Long");
        }
        let reply = send(&app, get(&at_limit, None)).await;
        assert_ne!(reply.status, StatusCode::URI_TOO_LONG, "{path}");
    }
}

#[tokio::test]
async fn request_bodies_return_413_before_authentication() {
    let app = app();
    let with_length = Request::builder()
        .uri(API)
        .header(header::CONTENT_LENGTH, "1")
        .body(Body::from("x"))
        .unwrap();
    assert_api_error(
        &send(&app, with_length).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
    );

    let chunked = Request::builder()
        .uri(HTML)
        .header(header::TRANSFER_ENCODING, "chunked")
        .body(Body::empty())
        .unwrap();
    assert_text_error(
        &send(&app, chunked).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "Payload Too Large",
    );

    let empty = Request::builder()
        .uri(API)
        .header(header::AUTHORIZATION, bearer(API_TOKEN))
        .header(header::CONTENT_LENGTH, "0")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, empty).await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_stylesheet_needs_no_credentials_and_only_get() {
    let app = app();
    let css = committed_stylesheet();
    let wrong = basic(WEB_USER, API_TOKEN);
    for request in [
        get(STYLESHEET, None),
        get("/_senu/ui.css?q=%3Cb%3E", None),
        get(STYLESHEET, Some(&wrong)),
        api_get(STYLESHEET),
    ] {
        let reply = send(&app, request).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert!(reply.body == css, "not the committed stylesheet");
        assert_eq!(
            reply.header(header::CONTENT_TYPE),
            "text/css; charset=utf-8"
        );
        assert_eq!(reply.header(header::CACHE_CONTROL), "no-store");
        assert_eq!(reply.header(header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert!(!reply.headers.contains_key(header::WWW_AUTHENTICATE));
    }
    for method in ["HEAD", "POST", "PUT", "DELETE", "OPTIONS"] {
        let request = Request::builder()
            .method(method)
            .uri(STYLESHEET)
            .body(Body::empty())
            .unwrap();
        let reply = send(&app, request).await;
        assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED, "{method}");
        assert_eq!(reply.header(header::ALLOW), "GET", "{method}");
    }
    let with_body = Request::builder()
        .uri(STYLESHEET)
        .header(header::CONTENT_LENGTH, "1")
        .body(Body::from("x"))
        .unwrap();
    let reply = send(&app, with_body).await;
    assert_text_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large");
    let long = format!("{STYLESHEET}?{}", "a".repeat(MAX_TARGET_BYTES));
    let reply = send(&app, get(&long, None)).await;
    assert_text_error(&reply, StatusCode::URI_TOO_LONG, "URI Too Long");
    for target in NOT_THE_STYLESHEET {
        let request = Request::builder()
            .uri(Uri::from(PathAndQuery::from_static(target)))
            .body(Body::empty())
            .unwrap();
        let reply = send(&app, request).await;
        assert_text_error(&reply, StatusCode::NOT_FOUND, "Not Found");
    }
}

#[tokio::test]
async fn every_response_disables_caching_and_sniffing() {
    let app = app();
    let long = format!("{API}?q={}", "a".repeat(MAX_TARGET_BYTES));
    let requests = vec![
        api_get(API),
        html_get(HTML),
        get(API, None),
        get(HTML, None),
        api_get("/api/projects?tenant=x"),
        get("/nowhere", None),
        Request::builder()
            .method("POST")
            .uri(API)
            .body(Body::empty())
            .unwrap(),
        get(&long, None),
    ];
    for request in requests {
        let reply = send(&app, request).await;
        assert_eq!(
            reply.header(header::CACHE_CONTROL),
            "no-store",
            "{}",
            reply.status
        );
        assert_eq!(
            reply.header(header::X_CONTENT_TYPE_OPTIONS),
            "nosniff",
            "{}",
            reply.status
        );
    }
    let forbidden = send(&app_without_permissions(), api_get(API)).await;
    assert_eq!(forbidden.header(header::CACHE_CONTROL), "no-store");
}

#[tokio::test]
async fn html_escapes_catalogue_markup() {
    let reply = send(&app(), html_get("/projects?q=script")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(!reply.body.contains("<script>"), "{}", reply.body);
    let raw = raw_item_text(&reply.body, "acme-003");
    assert!(!raw.contains(['<', '>', '"']), "{raw}");
    assert_eq!(
        html_items(&reply.body),
        [(
            "acme-003".to_owned(),
            "<script>alert('x')</script> & \"Launch\" Notes".to_owned()
        )]
    );
}

#[tokio::test]
async fn html_accepts_searches_a_browser_maxlength_would_block() {
    // 80 emoji are 80 scalars (valid) but 160 UTF-16 code units, and the
    // surrounding spaces are trimmed by the operation before counting.
    let emoji = "\u{1F680}".repeat(80);
    let encoded: String =
        form_urlencoded::byte_serialize(format!("  {emoji}  ").as_bytes()).collect();
    let reply = send(&app(), html_get(&format!("/projects?q={encoded}"))).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(search_value(&reply.body), emoji);
    assert!(!reply.body.contains("maxlength"), "{}", reply.body);
}

#[tokio::test]
async fn html_escapes_the_search_inside_the_value_attribute() {
    let reply = send(&app(), html_get("/projects?q=%3Cb%3E%22x%22")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(!reply.body.contains("<b>"), "{}", reply.body);
    assert!(!reply.body.contains("<b>\"x\""), "{}", reply.body);
    assert_eq!(search_value(&reply.body), "<b>\"x\"");
}

#[test]
fn templates_and_configuration_never_disable_escaping() {
    let root = env!("CARGO_MANIFEST_DIR");
    let config = std::fs::read_to_string(format!("{root}/askama.toml")).unwrap();
    assert!(
        !config.contains("escape"),
        "askama.toml configures an escaper"
    );

    let mut templates = 0;
    for entry in std::fs::read_dir(format!("{root}/templates")).unwrap() {
        let path = entry.unwrap().path();
        assert_eq!(
            path.extension().and_then(|e| e.to_str()),
            Some("html"),
            "{path:?}"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        for forbidden in ["safe", "escape", "|e(", "| e("] {
            assert!(!text.contains(forbidden), "{path:?} contains {forbidden}");
        }
        templates += 1;
    }
    assert!(templates > 0);

    // Every Rust file under src/, including the application in src/demo/.
    let mut sources = Vec::new();
    let mut directories = vec![std::path::PathBuf::from(format!("{root}/src"))];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                sources.push(path);
            }
        }
    }
    assert!(sources.iter().any(|path| path.ends_with("demo/mod.rs")));

    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap();
        for line in text.lines().filter(|l| l.contains("#[template(")) {
            assert!(!line.contains("escape"), "{path:?}: {line}");
            assert!(
                !line.contains("source ="),
                "{path:?} uses an inline template: {line}"
            );
            assert!(
                !line.contains("ext ="),
                "{path:?} overrides the extension: {line}"
            );
        }
    }

    // Spec 0003, SAFE-4: the only unescaped insertion is `TrustedHtml`, the
    // Markdown policy's output. Exactly one `HtmlSafe` impl exists in src/,
    // on that type, and its field is private; nothing else opts out.
    let mut boundary = Vec::new();
    for path in &sources {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(path).unwrap();
        for line in text.lines().map(str::trim) {
            for forbidden in ["MaybeSafe", "filters::Safe", "|safe", "| safe"] {
                assert!(!line.contains(forbidden), "{name}: {line}");
            }
            if line.contains("HtmlSafe") && !line.starts_with("//") {
                boundary.push(format!("{name}: {line}"));
            }
        }
    }
    assert_eq!(
        boundary,
        ["markdown.rs: impl askama::filters::HtmlSafe for TrustedHtml {}"]
    );
    let markdown = std::fs::read_to_string(format!("{root}/src/markdown.rs")).unwrap();
    assert_eq!(
        markdown.matches("pub struct TrustedHtml(String);").count(),
        1
    );
    assert!(!markdown.contains("TrustedHtml(pub"));
}

// Spec 0005, §4: `get_project` on `/api/project` and `/project`.

const PROJECT_API: &str = "/api/project";
const PROJECT_HTML: &str = "/project";

#[tokio::test]
async fn project_routes_return_one_project_without_its_tenant() {
    let app = app();
    let reply = send(&app, api_get("/api/project?id=acme-002")).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        reply.json(),
        json!({"id": "acme-002", "name": "Café Roadmap"})
    );
    let reply = send(&app, html_get("/project?id=acme-003")).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert!(reply.header(header::CONTENT_TYPE).starts_with("text/html"));
    assert!(
        reply
            .header(header::CONTENT_SECURITY_POLICY)
            .starts_with("default-src 'none'")
    );
    assert!(!reply.body.contains("<script>"), "{}", reply.body);
    assert_eq!(
        html_project(&reply.body),
        (
            "acme-003".to_owned(),
            "<script>alert('x')</script> & \"Launch\" Notes".to_owned()
        )
    );
}

#[tokio::test]
async fn project_routes_check_method_and_credentials_before_input() {
    let app = app();
    for query in ["?id=acme-001", "?id=BAD", "?tenant=globex"] {
        let api = format!("{PROJECT_API}{query}");
        let html = format!("{PROJECT_HTML}{query}");
        for authorization in [None, Some(basic(WEB_USER, WEB_PASSWORD))] {
            let reply = send(&app, get(&api, authorization.as_deref())).await;
            assert_api_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
            assert_eq!(reply.header(header::WWW_AUTHENTICATE), BEARER_CHALLENGE);
        }
        for authorization in [None, Some(bearer(API_TOKEN))] {
            let reply = send(&app, get(&html, authorization.as_deref())).await;
            assert_text_error(&reply, StatusCode::UNAUTHORIZED, "Unauthorized");
            assert_eq!(reply.header(header::WWW_AUTHENTICATE), BASIC_CHALLENGE);
        }
        for (path, json) in [(&api, true), (&html, false)] {
            let request = Request::builder()
                .method("POST")
                .uri(path.as_str())
                .body(Body::empty())
                .unwrap();
            let reply = send(&app, request).await;
            assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED, "{path}");
            assert_eq!(reply.header(header::ALLOW), "GET");
            assert_eq!(
                json,
                reply
                    .header(header::CONTENT_TYPE)
                    .starts_with("application/json")
            );
        }
    }
}

#[tokio::test]
async fn project_routes_deny_a_principal_without_permission_before_validation() {
    let app = app_without_permissions();
    for query in [
        "?id=acme-001",
        "",
        "?id=BAD",
        "?id=a&id=b",
        "?tenant=globex",
    ] {
        let reply = send(&app, api_get(&format!("{PROJECT_API}{query}"))).await;
        assert_api_error(&reply, StatusCode::FORBIDDEN, "forbidden");
        let reply = send(&app, html_get(&format!("{PROJECT_HTML}{query}"))).await;
        assert_text_error(&reply, StatusCode::FORBIDDEN, "Forbidden");
    }
}

#[tokio::test]
async fn project_routes_reject_bad_ids_and_hide_other_tenants() {
    let app = app();
    let long = "a".repeat(33);
    let cases = [
        (
            "",
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid request",
        ),
        (
            "?id=a&id=b",
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid request",
        ),
        (
            "?id=acme-001&tenant=globex",
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid request",
        ),
        ("?id=", StatusCode::BAD_REQUEST, "invalid_id", "Invalid id"),
        (
            "?id=ACME-001",
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "Invalid id",
        ),
        (
            &format!("?id={long}"),
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "Invalid id",
        ),
        (
            "?id=acme-999",
            StatusCode::NOT_FOUND,
            "not_found",
            "Not Found",
        ),
        (
            "?id=globex-001",
            StatusCode::NOT_FOUND,
            "not_found",
            "Not Found",
        ),
    ];
    for (query, status, code, phrase) in cases {
        let reply = send(&app, api_get(&format!("{PROJECT_API}{query}"))).await;
        assert_api_error(&reply, status, code);
        let reply = send(&app, html_get(&format!("{PROJECT_HTML}{query}"))).await;
        assert_text_error(&reply, status, phrase);
    }
}

#[tokio::test]
async fn responses_never_contain_credentials_or_input() {
    let app = app();
    let marker = "zz-input-marker";
    let requests = vec![
        api_get(&format!("/api/projects?q={marker}")),
        get(
            &format!("/api/projects?{marker}=1"),
            Some(&bearer(API_TOKEN)),
        ),
        get(
            &format!("/projects?{marker}=1"),
            Some(&basic(WEB_USER, WEB_PASSWORD)),
        ),
        get(API, Some(&bearer(WEB_PASSWORD))),
        get(HTML, Some(&basic(WEB_USER, API_TOKEN))),
        get(&format!("/api/projects?access_token={API_TOKEN}"), None),
        get(&format!("/{marker}"), None),
        get(&format!("/api/{marker}"), None),
    ];
    for request in requests {
        let reply = send(&app, request).await;
        let mut seen = reply.body.clone();
        for (name, value) in &reply.headers {
            seen.push_str(&format!("{name}: {}\n", value.to_str().unwrap_or("")));
        }
        for secret in SECRETS {
            assert!(!seen.contains(secret), "{} leaked a secret", reply.status);
        }
        if reply.status != StatusCode::OK {
            assert!(
                !seen.contains(marker),
                "{} echoed input: {seen}",
                reply.status
            );
        }
    }
}

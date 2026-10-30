//! Adapter tests for `senu site` (spec 0003, §6, ART-4 and SITE-3): the
//! router is called in-process with `oneshot`, and the startup function is
//! given invalid content.

use std::net::TcpListener;
use std::process::ExitCode;

use axum::Router;
use axum::body::Body;
use axum::http::uri::PathAndQuery;
use axum::http::{HeaderMap, Request, StatusCode, Uri, header};
use senu::config::site_config;
use senu::http::site_router;
use senu::site::Site;
use tower::ServiceExt as _;

const FILES: [(&str, &str); 3] = [
    ("content/en/01-first-note.md", "# First note\n\nHello.\n"),
    (
        "content/ja/01-first-note.md",
        "# 最初のノート\n\nこんにちは。\n",
    ),
    ("content/en/51-x.md", "# X\n"),
];
const CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'";

fn app() -> Router {
    site_router(Site::load(&FILES).expect("valid fixture"))
}

async fn send(
    method: &str,
    target: &'static str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, String) {
    let mut builder = Request::builder()
        .method(method)
        .uri(Uri::from(PathAndQuery::from_static(target)));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, String::from_utf8(body.to_vec()).unwrap())
}

fn header(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).map(|value| value.to_str().unwrap())
}

#[tokio::test]
async fn pages_are_served_as_html_with_strict_headers() {
    for (target, h1) in [
        ("/", "<h1>Build a small notes site with Rust</h1>"),
        ("/first-note", "<h1>First note</h1>"),
        ("/ja/", "<h1>Rust で小さなノートサイトを作る</h1>"),
        ("/ja/first-note", "<h1>最初のノート</h1>"),
        ("/x", "<h1>X</h1>"),
    ] {
        let (status, headers, body) = send("GET", target, &[]).await;
        assert_eq!(status, StatusCode::OK, "{target}");
        assert!(body.contains(h1), "{target}: {body}");
        assert_eq!(
            header(&headers, header::CONTENT_TYPE),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(header(&headers, header::CONTENT_SECURITY_POLICY), Some(CSP));
        assert_eq!(header(&headers, header::CACHE_CONTROL), Some("no-store"));
        assert_eq!(
            header(&headers, header::X_CONTENT_TYPE_OPTIONS),
            Some("nosniff")
        );
        assert_eq!(header(&headers, header::WWW_AUTHENTICATE), None);
    }
}

#[tokio::test]
async fn only_exact_paths_are_pages() {
    for target in [
        "/First-note",
        "/first-note/",
        "/first%2Dnote",
        "/first%2dnote",
        "/ja",
        "//first-note",
        "/en/first-note",
        "/index.html",
        "/first-note.html",
        "/content/en/51-x.md",
        "/51-x",
        "/%2e%2e/first-note",
        "/ja/../first-note",
        "/./first-note",
        "/first-note%00",
        "/%5Cfirst-note",
        "/first-note;x",
        "/projects",
        "/api/projects",
        "/api",
        "/mcp",
    ] {
        let (status, headers, body) = send("GET", target, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{target}");
        assert_eq!(body, "Not Found", "{target}");
        assert_eq!(
            header(&headers, header::CONTENT_TYPE),
            Some("text/plain; charset=utf-8"),
            "{target}"
        );
        assert_eq!(header(&headers, header::WWW_AUTHENTICATE), None, "{target}");
    }
}

#[tokio::test]
async fn queries_and_credentials_do_not_change_a_page() {
    let (_, _, plain) = send("GET", "/first-note", &[]).await;
    for (target, headers) in [
        ("/first-note?x=1", &[][..]),
        ("/first-note?access_token=zz-token", &[]),
        ("/first-note", &[("authorization", "Bearer zz-token")]),
        ("/first-note", &[("authorization", "Basic ZGVtbzpkZW1v")]),
    ] {
        let (status, response_headers, body) = send("GET", target, headers).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, plain, "{target} {headers:?}");
        assert!(!body.contains("zz-token"));
        assert_eq!(header(&response_headers, header::WWW_AUTHENTICATE), None);
    }
    let (status, headers, _) = send(
        "GET",
        "/projects",
        &[("authorization", "Basic ZGVtbzpkZW1v")],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(header(&headers, header::WWW_AUTHENTICATE), None);
}

#[tokio::test]
async fn other_methods_and_oversized_requests_are_refused_as_plain_text() {
    for method in ["POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"] {
        let (status, headers, _) = send(method, "/first-note", &[]).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method}");
        assert_eq!(header(&headers, header::ALLOW), Some("GET"), "{method}");
    }
    let (status, _, body) = send("POST", "/missing", &[]).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::NOT_FOUND, "Not Found")
    );
    let (status, headers, body) = send("GET", "/first-note", &[("content-length", "5")]).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large")
    );
    assert_eq!(
        header(&headers, header::CONTENT_TYPE),
        Some("text/plain; charset=utf-8")
    );
    let long: &'static str = Box::leak(format!("/api/{}", "a".repeat(2048)).into_boxed_str());
    let (status, headers, body) = send("GET", long, &[]).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::URI_TOO_LONG, "URI Too Long")
    );
    assert_eq!(
        header(&headers, header::CONTENT_TYPE),
        Some("text/plain; charset=utf-8")
    );
}

#[test]
fn invalid_content_exits_2_before_binding() {
    // If startup tried to bind, the held port would make it exit 1.
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let bind = held.local_addr().unwrap().to_string();
    let config = site_config([("SENU_BIND".into(), bind.into())]).unwrap();
    let files = [
        ("content/en/02-b.md", "# B\n\n[x](javascript:alert(1))\n"),
        ("content/en/01-a.md", "No title.\n"),
    ];
    assert_eq!(senu::app::site(&config, &files), ExitCode::from(2));
    assert_eq!(senu::app::site(&config, &FILES), ExitCode::from(1));
}

//! X-1: the JSON API, the MCP tool and the HTML page return exactly what
//! `list_projects` returns, for the spec's queries and a padded search.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use rmcp::model::CallToolRequestParams;
use senu::catalogue::Catalogue;
use senu::config::serve_config;
use senu::http::{AppState, router};
use senu::mcp::{McpServer, TOOL_NAME};
use senu::operation::{Principal, list_projects};
use serde_json::{Map, Value};
use support::*;
use tower::ServiceExt as _;

type Items = Vec<(String, String)>;

async fn body(app: &axum::Router, uri: &str, authorization: &str) -> String {
    let request = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, authorization)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.expect("infallible");
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn json_items(list: &Value) -> Items {
    list["projects"]
        .as_array()
        .expect("projects array")
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap().to_owned(),
                p["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn api_mcp_and_html_match_the_operation_for_the_spec_queries() {
    let catalogue = Catalogue::demo();
    let config = serve_config(serve_env()).expect("valid test configuration");
    let app = router(AppState::new(Catalogue::demo(), &config));
    let mcp = McpServer::new(Catalogue::demo());

    // The padded search is 86 scalars long but 6 after trimming.
    let padded = format!("{}launch", " ".repeat(80));
    let cases: [(Option<&str>, &[&str]); 5] = [
        (None, &["acme-001", "acme-002", "acme-003"]),
        (Some("launch"), &["acme-001", "acme-003"]),
        (Some(" CAFÉ "), &["acme-002"]),
        (Some("zephyr"), &[]),
        (Some(&padded), &["acme-001", "acme-003"]),
    ];
    for (q, expected_ids) in cases {
        let mut args = Map::new();
        if let Some(q) = q {
            args.insert("q".to_owned(), Value::String(q.to_owned()));
        }
        let direct = list_projects(&catalogue, &Principal::demo_api(), &args).unwrap();
        let expected: Items = direct
            .projects
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        let ids: Vec<&str> = expected.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, expected_ids, "operation for {q:?}");

        let query = q
            .map(|q| {
                let encoded: String = form_urlencoded::byte_serialize(q.as_bytes()).collect();
                format!("?q={encoded}")
            })
            .unwrap_or_default();

        let api = body(&app, &format!("/api/projects{query}"), &bearer(API_TOKEN)).await;
        let api: Value = serde_json::from_str(&api).unwrap();
        assert_eq!(json_items(&api), expected, "API for {q:?}");

        let page = body(
            &app,
            &format!("/projects{query}"),
            &basic(WEB_USER, WEB_PASSWORD),
        )
        .await;
        assert_eq!(html_items(&page), expected, "HTML for {q:?}");

        let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(args.clone());
        let result = mcp.call(&request).expect("a tool result");
        assert_eq!(result.is_error, Some(false), "MCP for {q:?}");
        let structured = result.structured_content.expect("structuredContent");
        assert_eq!(json_items(&structured), expected, "MCP for {q:?}");
    }
}

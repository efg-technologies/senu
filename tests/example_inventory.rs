//! `examples/inventory.rs`, an application written only against Senu's
//! public library API (spec 0006, LIB-1): it meets the contract, its own
//! authenticator decides who calls, and tenants stay apart.

#[allow(dead_code)]
#[path = "../examples/inventory.rs"]
mod inventory;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use inventory::{ApiKeys, Inventory, NORTH_CLERK, VIEW_STOCK};
use rmcp::model::CallToolRequestParams;
use senu::auth::{Basic, Secret};
use senu::contract::{self, Subject};
use senu::http::AppState;
use senu::mcp::McpServer;
use senu::operation::{Principal, TenantId};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const NORTH_KEY: &str = "sentinel-north-key-4W";
const SOUTH_KEY: &str = "sentinel-south-key-6M";
const WEB_PASSWORD: &str = "sentinel-clerk-password-2H";

const SOUTH_CLERK: Principal =
    Principal::new("south-clerk", TenantId::from_static("south"), &[VIEW_STOCK]);

fn secret(value: &str) -> Secret {
    Secret::new(value).expect("a valid secret")
}

fn app() -> Router {
    let state = AppState::builder(Inventory::sample())
        .html_auth(Basic::new(
            secret("clerk"),
            secret(WEB_PASSWORD),
            NORTH_CLERK,
        ))
        .api_auth(ApiKeys::new(vec![
            (secret(NORTH_KEY), NORTH_CLERK),
            (secret(SOUTH_KEY), SOUTH_CLERK),
        ]))
        .build();
    inventory::endpoints()
        .into_router(state)
        .expect("a valid table")
}

async fn get(
    target: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::builder().uri(target);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .expect("infallible");
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn api(target: &str, key: &str) -> (StatusCode, Value) {
    let (status, _, body) = get(target, &[("x-api-key", key)]).await;
    (status, serde_json::from_str(&body).expect("JSON"))
}

#[tokio::test]
async fn the_example_meets_senus_contract() {
    contract::check(Subject {
        state: Inventory::sample,
        endpoints: inventory::endpoints,
        tools: inventory::tools,
        principal: NORTH_CLERK,
        samples: &[("get_stock", "sku=BOLT-8"), ("list_low_stock", "below=10")],
        auth: None,
    })
    .await;
}

#[tokio::test]
async fn the_custom_authenticator_selects_the_principal_and_its_tenant() {
    let (status, north) = api("/api/stock?sku=BOLT-8", NORTH_KEY).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        north,
        json!({ "sku": "BOLT-8", "name": "Bolt <M8>", "units": 240 })
    );
    let (status, south) = api("/api/stock?sku=BOLT-8", SOUTH_KEY).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(south["units"], 7);
    // Another tenant's item looks exactly like one that does not exist.
    let other = api("/api/stock?sku=NUT-8", SOUTH_KEY).await;
    let missing = api("/api/stock?sku=NOPE-1", SOUTH_KEY).await;
    assert_eq!(
        other,
        (StatusCode::NOT_FOUND, json!({ "error": "not_found" }))
    );
    assert_eq!(other, missing);
    // A tenant named in the query is an unknown field, not a way across.
    let (status, body) = api("/api/stock?sku=NUT-8&tenant=north", SOUTH_KEY).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({ "error": "invalid_request" }));
}

#[tokio::test]
async fn wrong_missing_and_misplaced_keys_get_the_authenticators_challenge() {
    let bearer = format!("Bearer {NORTH_KEY}");
    let attempts: [&[(&str, &str)]; 5] = [
        &[],
        &[("x-api-key", "sentinel-north-key-4X")],
        &[("x-api-key", NORTH_KEY), ("x-api-key", NORTH_KEY)],
        &[("authorization", &bearer)],
        &[("x-api-key", WEB_PASSWORD)],
    ];
    for headers in attempts {
        // The query is invalid, and the key sits in it: neither is read.
        let target = format!("/api/stock?zz=1&x-api-key={NORTH_KEY}");
        let (status, reply, body) = get(&target, headers).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{headers:?}");
        assert_eq!(body, r#"{"error":"unauthorized"}"#);
        assert_eq!(
            reply[header::WWW_AUTHENTICATE],
            "ApiKey realm=\"inventory\""
        );
        for secret in [NORTH_KEY, SOUTH_KEY, WEB_PASSWORD] {
            assert!(!body.contains(secret));
        }
    }
}

#[tokio::test]
async fn operations_parse_their_own_parameters_and_name_their_own_errors() {
    let (status, low) = api("/api/low-stock", NORTH_KEY).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(low["below"], 5);
    let skus: Vec<&str> = low["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["sku"].as_str().unwrap())
        .collect();
    assert_eq!(skus, ["NUT-8", "WASHER-8"]);
    for bad in [
        "below=-1",
        "below=1.5",
        "below=1234567",
        "below=",
        "below=ten",
    ] {
        let (status, body) = api(&format!("/api/low-stock?{bad}"), NORTH_KEY).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(body, json!({ "error": "invalid_below" }), "{bad}");
    }
    let (status, body) = api("/api/stock?sku=bolt-8", NORTH_KEY).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({ "error": "invalid_sku" }));
}

#[tokio::test]
async fn the_page_escapes_names_and_the_asset_needs_no_credentials() {
    let basic = {
        use base64::Engine as _;
        let raw = format!("clerk:{WEB_PASSWORD}");
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    };
    let (status, headers, page) = get("/low-stock?below=999", &[("authorization", &basic)]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert!(page.contains(r#"<li data-sku="BOLT-8">"#), "{page}");
    assert!(!page.contains("<M8>"), "{page}");
    assert!(page.contains("Bolt &#60;M8&#62;: 240"), "{page}");
    assert!(page.contains(r#"href="/assets/inventory.css""#));

    let (status, headers, css) = get("/assets/inventory.css", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/css; charset=utf-8");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(css.starts_with("body {"));
    let post = Request::builder()
        .method("POST")
        .uri("/assets/inventory.css")
        .body(Body::empty())
        .unwrap();
    let response = app().oneshot(post).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "GET");
}

#[tokio::test]
async fn the_same_operations_are_tools_bound_to_one_principal() {
    let server = McpServer::with_tools(Inventory::sample(), SOUTH_CLERK, inventory::tools())
        .expect("a valid list");
    let unnamed = serde_json::to_value(server.info()).unwrap();
    assert_eq!(unnamed["serverInfo"]["name"], "senu");
    let server = server.with_info("inventory", "1.0.0");
    let named = serde_json::to_value(server.info()).unwrap();
    assert_eq!(named["serverInfo"]["name"], "inventory");
    assert_eq!(named["serverInfo"]["version"], "1.0.0");
    let names: Vec<String> = server
        .tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names, ["get_stock", "list_low_stock"]);
    let args = json!({ "sku": "BOLT-8" }).as_object().unwrap().clone();
    let request = CallToolRequestParams::new("get_stock").with_arguments(args);
    let result = server.call(&request).await.expect("a tool result");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content,
        Some(json!({ "sku": "BOLT-8", "name": "Bolt M8 (south)", "units": 7 }))
    );
}

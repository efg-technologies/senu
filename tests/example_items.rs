//! `examples/items.rs`: declared writes behind a cookie session (spec 0007).
//! The contract runs through the example's own authenticator, and the
//! tests below pin what an adopter relies on: the order of the checks, the
//! cross-site rules, tenants, 409, the login redirect and custom bodies.

#[allow(dead_code)]
#[path = "../examples/items.rs"]
mod items;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use items::{EDIT_ITEMS, EDITOR, Store, VIEW_ITEMS};
use rmcp::model::CallToolRequestParams;
use senu::contract::{self, OwnAuth, Subject};
use senu::http::{AppState, Endpoints, RouteError};
use senu::mcp::McpServer;
use senu::operation::{Principal, TenantId};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const SESSION: &str = "sentinel-session-north-7Q";
const COOKIE: &str = "sid=sentinel-session-north-7Q";
const SOUTH: &str = "sid=sentinel-session-south-3K";
const VIEWER: &str = "sid=sentinel-session-viewer-9D";

const SOUTH_EDITOR: Principal = Principal::new(
    "editor@south.example",
    TenantId::from_static("south"),
    &[VIEW_ITEMS, EDIT_ITEMS],
);
const NORTH_VIEWER: Principal = Principal::new(
    "viewer@north.example",
    TenantId::from_static("north"),
    &[VIEW_ITEMS],
);

fn app() -> Router {
    let store = Store::sample();
    store.sign_in(SESSION, EDITOR);
    store.sign_in("sentinel-session-south-3K", SOUTH_EDITOR);
    store.sign_in("sentinel-session-viewer-9D", NORTH_VIEWER);
    items::endpoints()
        .into_router(items::state(store))
        .expect("a valid table")
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("not JSON: {}", self.body))
    }
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.expect("infallible");
    let (status, headers) = (response.status(), response.headers().clone());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

fn get(target: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::builder().uri(target);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::empty()).unwrap()
}

fn post(target: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(target)
        .header("content-type", "application/json")
        .header("content-length", body.len());
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::from(body.to_owned())).unwrap()
}

async fn titles(app: &Router, cookie: &str) -> Vec<String> {
    let reply = send(app, get("/api/items", &[("cookie", cookie)])).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    reply.json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["title"].as_str().unwrap().to_owned())
        .collect()
}

const NEW: &str = r#"{"title":"CSV import","lo":0,"hi":0.05}"#;

#[tokio::test]
async fn the_example_meets_the_contract_through_its_own_authenticator() {
    contract::check(Subject {
        state: Store::sample,
        endpoints: items::endpoints,
        tools: items::tools,
        principal: EDITOR,
        samples: &[
            ("list_items", ""),
            ("create_item", r#"{"title":"CSV import","lo":0,"hi":0.05}"#),
            ("rename_item", r#"{"id":"P-1","title":"Renamed"}"#),
            ("delete_item", r#"{"id":"P-1"}"#),
        ],
        auth: Some(OwnAuth {
            app: |store, principal| {
                store.sign_in("contract-session-5T", principal.clone());
                items::state(store)
            },
            html: &[("cookie", "sid=contract-session-5T")],
            api: &[("cookie", "sid=contract-session-5T")],
        }),
    })
    .await;
}

#[tokio::test]
async fn create_rename_and_delete_change_only_the_callers_project() {
    let app = app();
    let created = send(&app, post("/api/items/create", &[("cookie", COOKIE)], NEW)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(created.json(), json!({ "id": "P-3" }));
    assert_eq!(created.headers[header::CACHE_CONTROL], "no-store");
    assert!(
        titles(&app, COOKIE)
            .await
            .contains(&"CSV import".to_owned())
    );
    assert_eq!(titles(&app, SOUTH).await, ["Another project's item"]);

    // The other project has its own P-1; the caller's is renamed.
    let renamed = send(
        &app,
        post(
            "/api/items/rename",
            &[("cookie", COOKIE)],
            r#"{"id":"P-1","title":"  Free template  "}"#,
        ),
    )
    .await;
    assert_eq!(renamed.json()["title"], "Free template");
    assert_eq!(titles(&app, SOUTH).await, ["Another project's item"]);

    // An id of another project is not found, like one that is absent.
    for id in ["P-3", "P-99"] {
        let body = format!(r#"{{"id":"{id}","title":"x"}}"#);
        let reply = send(&app, post("/api/items/rename", &[("cookie", SOUTH)], &body)).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{id}");
        assert_eq!(reply.json(), json!({ "error": "not_found" }));
    }
    // A tenant in the body is an unknown field, not a way across.
    let across = r#"{"id":"P-3","title":"x","tenant":"north"}"#;
    let reply = send(
        &app,
        post("/api/items/rename", &[("cookie", SOUTH)], across),
    )
    .await;
    assert_eq!(reply.json(), json!({ "error": "invalid_request" }));

    let deleted = send(
        &app,
        post(
            "/api/items/delete",
            &[("cookie", COOKIE)],
            r#"{"id":"P-3"}"#,
        ),
    )
    .await;
    assert_eq!(deleted.json(), json!({ "deleted": true }));
    let again = send(
        &app,
        post(
            "/api/items/delete",
            &[("cookie", COOKIE)],
            r#"{"id":"P-3"}"#,
        ),
    )
    .await;
    assert_eq!(again.json(), json!({ "deleted": false }));
    // "Not now" is 409 with the operation's own code.
    let deployed = send(
        &app,
        post(
            "/api/items/delete",
            &[("cookie", COOKIE)],
            r#"{"id":"P-2"}"#,
        ),
    )
    .await;
    assert_eq!(deployed.status, StatusCode::CONFLICT);
    assert_eq!(deployed.json(), json!({ "error": "item_deployed" }));
    assert_eq!(titles(&app, COOKIE).await.len(), 2);
}

#[tokio::test]
async fn typed_arguments_and_the_operations_own_rules() {
    let app = app();
    let cases = [
        (r#"{"title":"x","lo":"0","hi":1}"#, "invalid_request"),
        (r#"{"title":"x","lo":0}"#, "invalid_request"),
        (r#"{"title":7,"lo":0,"hi":1}"#, "invalid_request"),
        (
            r#"{"title":"x","lo":0,"hi":1,"status":"deployed"}"#,
            "invalid_request",
        ),
        (r#"{"title":"x","lo":null,"hi":1}"#, "invalid_request"),
        (r#"{"title":"   ","lo":0,"hi":1}"#, "invalid_title"),
        (r#"{"title":"x","lo":2,"hi":1}"#, "invalid_range"),
    ];
    for (body, code) in cases {
        let reply = send(&app, post("/api/items/create", &[("cookie", COOKIE)], body)).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(reply.json(), json!({ "error": code }), "{body}");
    }
    assert_eq!(titles(&app, COOKIE).await.len(), 2, "nothing was created");
    // Integers are numbers too, and the title is trimmed.
    let reply = send(
        &app,
        post(
            "/api/items/create",
            &[("cookie", COOKIE)],
            r#"{"title":" x ","lo":-1,"hi":1.5}"#,
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
}

#[tokio::test]
async fn a_write_is_refused_step_by_step_before_it_can_change_anything() {
    let app = app();
    let before = titles(&app, COOKIE).await;
    let create = "/api/items/create";
    let step = |request: Request<Body>| send(&app, request);

    // The method, even with a session and a valid body in the query.
    let reply = step(get(
        &format!("{create}?title=x&lo=0&hi=1"),
        &[("cookie", COOKIE)],
    ))
    .await;
    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.headers[header::ALLOW], "POST");
    // Another site, before the session is even looked at.
    for extra in [
        ("sec-fetch-site", "cross-site"),
        ("sec-fetch-site", "same-site"),
        ("origin", "https://attacker.example"),
        ("origin", "null"),
    ] {
        for cookie in [COOKIE, "sid=unknown"] {
            let headers = [("cookie", cookie), ("host", "items.example"), extra];
            let reply = step(post(create, &headers, NEW)).await;
            assert_eq!(reply.status, StatusCode::FORBIDDEN, "{extra:?}");
            assert_eq!(
                reply.json(),
                json!({ "error": "cross_origin" }),
                "{extra:?}"
            );
        }
    }
    // No session: 401 without a scheme, whatever the body is.
    for headers in [
        &[][..],
        &[("cookie", "sid=unknown")],
        &[("cookie", "sid=a; sid=b")],
    ] {
        let reply = step(post(create, headers, "not json")).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        assert!(reply.headers.get(header::WWW_AUTHENTICATE).is_none());
    }
    // The session in the query string is never read.
    let reply = step(post(&format!("{create}?sid={SESSION}"), &[], NEW)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    // May read, may not write: 403 before the body is looked at.
    let reply = step(post(create, &[("cookie", VIEWER)], "not json")).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(reply.json(), json!({ "error": "forbidden" }));
    assert_eq!(titles(&app, VIEWER).await, before);
    // Then the content type, the size and the shape.
    let form = Request::builder()
        .method("POST")
        .uri(create)
        .header("cookie", COOKIE)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("title=x&lo=0&hi=1"))
        .unwrap();
    assert_eq!(step(form).await.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let big = format!(r#"{{"title":"{}","lo":0,"hi":1}}"#, "x".repeat(70_000));
    let reply = step(post(create, &[("cookie", COOKIE)], &big)).await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    // A streamed body without a declared length stops at the same limit.
    let streamed = Request::builder()
        .method("POST")
        .uri(create)
        .header("cookie", COOKIE)
        .header("content-type", "application/json")
        .header("transfer-encoding", "chunked")
        .body(Body::from(big))
        .unwrap();
    assert_eq!(step(streamed).await.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(titles(&app, COOKIE).await, before, "nothing ran");

    // The same host passes, as a browser on the application's own pages.
    for extra in [
        ("sec-fetch-site", "same-origin"),
        ("sec-fetch-site", "none"),
        ("origin", "https://items.example"),
    ] {
        let headers = [("cookie", COOKIE), ("host", "items.example"), extra];
        let reply = step(post(create, &headers, NEW)).await;
        assert_eq!(reply.status, StatusCode::OK, "{extra:?} {}", reply.body);
    }
}

#[tokio::test]
async fn trusted_origins_and_limits_are_part_of_the_table() {
    let store = Store::sample();
    store.sign_in(SESSION, EDITOR);
    let app = items::endpoints()
        .trusted_origin("https://app.example.com")
        .max_body_bytes(64)
        .into_router(items::state(store))
        .unwrap();
    let headers = [("cookie", COOKIE), ("host", "api.example.com")];
    let trusted = [
        headers[0],
        headers[1],
        ("origin", "https://app.example.com"),
    ];
    let other = [
        headers[0],
        headers[1],
        ("origin", "https://app.example.com.attacker.example"),
    ];
    let short = r#"{"title":"x","lo":0,"hi":1}"#;
    assert_eq!(
        send(&app, post("/api/items/create", &trusted, short))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, post("/api/items/create", &other, short))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let long = r#"{"title":"a title that makes this body longer than 64 bytes","lo":0,"hi":1}"#;
    assert_eq!(
        send(&app, post("/api/items/create", &headers, long))
            .await
            .status,
        StatusCode::PAYLOAD_TOO_LARGE
    );

    let table =
        |endpoints: Endpoints<Store>| endpoints.into_router(items::state(Store::sample())).err();
    for bytes in [0, 16 * 1024 * 1024 + 1] {
        assert_eq!(
            table(items::endpoints().max_body_bytes(bytes)),
            Some(RouteError::InvalidBodyLimit)
        );
    }
    for origin in [
        "app.example.com",
        "https://",
        "https://app.example.com/",
        "ftp://x",
        "https://a b",
    ] {
        assert_eq!(
            table(items::endpoints().trusted_origin(origin)),
            Some(RouteError::InvalidOrigin(origin)),
        );
    }
    fn page(_: &senu::operation::Args, _: &items::Created) -> Result<String, askama::Error> {
        Ok(String::new())
    }
    assert_eq!(
        table(Endpoints::new().html("/create", &items::CREATE_ITEM, page)),
        Some(RouteError::WriteNeedsApi("/create"))
    );
    for login in ["login", "//evil.example", "/login?next=x", "/a b"] {
        let state = AppState::builder(Store::sample())
            .api_auth(items::Sessions(Store::sample()))
            .html_login(login)
            .build();
        assert_eq!(
            items::endpoints().into_router(state).err(),
            Some(RouteError::InvalidLogin(login))
        );
    }
}

#[tokio::test]
async fn pages_go_to_the_login_page_and_custom_routes_take_a_bounded_body() {
    fn page(_: &senu::operation::Args, list: &items::ItemList) -> Result<String, askama::Error> {
        Ok(format!(
            "<!DOCTYPE html>\n<title>Items</title>\n<p>{}</p>\n",
            list.items.len()
        ))
    }
    let store = Store::sample();
    store.sign_in(SESSION, EDITOR);
    let app = items::endpoints()
        .html("/items", &items::LIST_ITEMS, page)
        .into_router(items::state(store))
        .unwrap();

    let anonymous = send(&app, get("/items", &[])).await;
    assert_eq!(anonymous.status, StatusCode::SEE_OTHER);
    assert_eq!(anonymous.headers[header::LOCATION], "/login");
    assert!(anonymous.headers.get(header::WWW_AUTHENTICATE).is_none());
    assert_eq!(anonymous.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        send(&app, get("/items", &[("cookie", COOKIE)]))
            .await
            .status,
        StatusCode::OK
    );
    // The API never redirects.
    assert_eq!(
        send(&app, get("/api/items", &[])).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(send(&app, get("/login", &[])).await.status, StatusCode::OK);

    let form = |headers: &[(&str, &str)], body: String| {
        let mut request = Request::builder()
            .method("POST")
            .uri("/logout")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("content-length", body.len());
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.body(Body::from(body)).unwrap()
    };
    // Over its limit: refused by Senu before the handler.
    let reply = send(&app, form(&[("cookie", COOKIE)], "x".repeat(1025))).await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    // From another site: refused by the handler's own check.
    let cross = [("cookie", COOKIE), ("sec-fetch-site", "cross-site")];
    assert_eq!(
        send(&app, form(&cross, "confirm=1".into())).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&app, get("/items", &[("cookie", COOKIE)]))
            .await
            .status,
        StatusCode::OK
    );
    let reply = send(&app, form(&[("cookie", COOKIE)], "confirm=1".into())).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(reply.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(
        send(&app, get("/items", &[("cookie", COOKIE)]))
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    // A plain custom route still takes no body.
    let login_post = Request::builder()
        .method("POST")
        .uri("/login")
        .header("content-length", "1")
        .body(Body::from("x"))
        .unwrap();
    assert_eq!(
        send(&app, login_post).await.status,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn the_writes_are_tools_that_never_claim_to_be_read_only() {
    let server = McpServer::with_tools(Store::sample(), EDITOR, items::tools()).unwrap();
    let wire = serde_json::to_value(server.tools()).unwrap();
    let hints = |name: &str| {
        let tool = wire
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        let a = &tool["annotations"];
        (
            a["readOnlyHint"].clone(),
            a["destructiveHint"].clone(),
            a["idempotentHint"].clone(),
        )
    };
    assert_eq!(hints("list_items"), (json!(true), Value::Null, Value::Null));
    assert_eq!(
        hints("create_item"),
        (json!(false), json!(false), json!(false))
    );
    assert_eq!(
        hints("rename_item"),
        (json!(false), json!(true), json!(true))
    );
    assert_eq!(
        hints("delete_item"),
        (json!(false), json!(true), json!(true))
    );
    let create = wire
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "create_item")
        .unwrap();
    assert_eq!(create["inputSchema"]["properties"]["lo"]["type"], "number");
    assert_eq!(
        create["inputSchema"]["properties"]["title"]["type"],
        "string"
    );
    assert_eq!(
        create["inputSchema"]["required"],
        json!(["title", "lo", "hi"])
    );

    let call = |name: &'static str, args: Value| {
        let request =
            CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone());
        let server = &server;
        async move { server.call(&request).await.expect("a tool result") }
    };
    let created = call(
        "create_item",
        json!({ "title": "From an assistant", "lo": 0, "hi": 0.2 }),
    )
    .await;
    assert_eq!(created.structured_content, Some(json!({ "id": "P-3" })));
    let refused = call("delete_item", json!({ "id": "P-2" })).await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(
        refused.structured_content,
        Some(json!({ "error": "item_deployed" }))
    );
    let wrong = call("create_item", json!({ "title": "x", "lo": "0", "hi": 1 })).await;
    assert_eq!(
        wrong.structured_content,
        Some(json!({ "error": "invalid_request" }))
    );

    // A principal that may only read cannot write through a tool either.
    let viewer = McpServer::with_tools(Store::sample(), NORTH_VIEWER, items::tools()).unwrap();
    let request = CallToolRequestParams::new("delete_item")
        .with_arguments(json!({ "id": "P-1" }).as_object().unwrap().clone());
    let denied = viewer.call(&request).await.unwrap();
    assert_eq!(
        denied.structured_content,
        Some(json!({ "error": "forbidden" }))
    );
}

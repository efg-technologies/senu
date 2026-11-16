//! The library API of spec 0006, §3 and §4: route tables fail closed
//! without an authenticator, `Internal` carries no detail on any transport,
//! an asynchronous authenticator supplies a run-time principal, and the
//! CSP and assets are checked with the table.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use rmcp::model::CallToolRequestParams;
use senu::auth::{AuthFuture, Authenticate, Bearer, Secret};
use senu::http::{AppState, DEFAULT_HTML_CSP, Endpoints, Kind, RouteError};
use senu::mcp::{self, McpServer};
use senu::operation::{
    Args, Effect, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const READ: Permission = Permission::new("read_notes");
const TOKEN: &str = "sentinel-library-token-1V";

/// The state counts how often an operation ran.
#[derive(Default)]
struct Notes {
    runs: AtomicUsize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WhoArgs {}

/// Returns what the operation was told about the caller.
async fn whoami(notes: &Notes, caller: &Principal, args: &Args) -> Result<Value, OpError> {
    let WhoArgs {} = decode(args)?;
    tokio::task::yield_now().await;
    notes.runs.fetch_add(1, Ordering::SeqCst);
    Ok(json!({ "name": caller.name(), "tenant": caller.tenant().as_str() }))
}

const WHOAMI: Operation<Notes, Value> = Operation {
    name: "whoami",
    description: "The caller as the server sees it.",
    permission: READ,
    params: &[],
    effect: Effect::Read,
    run: Run::Async(|notes, caller, args| Box::pin(whoami(notes, caller, args))),
};

fn broken(_: &Notes, _: &Principal, _: &Args) -> Result<Value, OpError> {
    // A real operation would log the cause here; none of it may leak.
    Err(OpError::Internal)
}

const BROKEN: Operation<Notes, Value> = Operation {
    name: "broken",
    description: "Always fails, as a backend that is down would.",
    permission: READ,
    params: &[Param::optional("q", "Ignored.")],
    effect: Effect::Read,
    run: Run::Sync(broken),
};

fn page(_args: &Args, value: &Value) -> Result<String, askama::Error> {
    Ok(format!(
        "<!DOCTYPE html>\n<title>Who</title>\n<p>{}</p>\n",
        value["tenant"]
    ))
}

/// Looks a session cookie up, as a database-backed authenticator would:
/// asynchronously, and with a principal built at run time.
struct Sessions;

impl Authenticate for Sessions {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
            tokio::task::yield_now().await;
            let user = cookie.strip_prefix("session=")?;
            let tenant = match user {
                "ann" => "team-a",
                "bob" => "team-b",
                _ => return None,
            };
            Some(Principal::owned(
                format!("user:{user}"),
                TenantId::new(tenant),
                vec![READ],
            ))
        })
    }
}

fn clerk() -> Principal {
    Principal::new("clerk", TenantId::from_static("main"), &[READ])
}

fn bearer() -> Bearer {
    Bearer::new(Secret::new(TOKEN).unwrap(), clerk())
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

async fn send(app: &Router, target: &str, headers: &[(header::HeaderName, &str)]) -> Reply {
    let mut request = Request::builder().uri(target);
    for (name, value) in headers {
        request = request.header(name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .expect("infallible");
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

#[test]
fn a_table_without_the_surfaces_authenticator_is_refused() {
    let table = |endpoints: Endpoints<Notes>, state: AppState<Notes>| {
        endpoints.into_router(state).map(|_| ())
    };
    let none = || AppState::builder(Notes::default()).build();
    let api_only = || {
        AppState::builder(Notes::default())
            .api_auth(bearer())
            .build()
    };
    let html_only = || {
        AppState::builder(Notes::default())
            .html_auth(Sessions)
            .build()
    };

    let api = || Endpoints::new().api("/api/whoami", &WHOAMI);
    let html = || Endpoints::new().html("/whoami", &WHOAMI, page);
    assert_eq!(
        table(api(), none()),
        Err(RouteError::NoAuthenticator("/api/whoami"))
    );
    assert_eq!(
        table(api(), html_only()),
        Err(RouteError::NoAuthenticator("/api/whoami"))
    );
    assert_eq!(
        table(html(), none()),
        Err(RouteError::NoAuthenticator("/whoami"))
    );
    assert_eq!(
        table(html(), api_only()),
        Err(RouteError::NoAuthenticator("/whoami"))
    );
    assert_eq!(table(api(), api_only()), Ok(()));
    assert_eq!(table(html(), html_only()), Ok(()));
    // Routes that never authenticate need no authenticator.
    let open = Endpoints::new()
        .asset("/app.css", "text/css", b"")
        .custom("/healthz", axum::routing::get(|| async { "ok" }));
    assert_eq!(table(open, none()), Ok(()));
    let message = RouteError::NoAuthenticator("/whoami").to_string();
    assert!(message.contains("html_auth or api_auth"), "{message}");
}

#[tokio::test]
async fn an_asynchronous_authenticator_supplies_a_run_time_principal() {
    let state = AppState::builder(Notes::default())
        .html_auth(Sessions)
        .api_auth(Sessions)
        .build();
    let app = Endpoints::new()
        .api("/api/whoami", &WHOAMI)
        .html("/whoami", &WHOAMI, page)
        .into_router(state)
        .unwrap();
    for (cookie, tenant) in [("session=ann", "team-a"), ("session=bob", "team-b")] {
        let reply = send(&app, "/api/whoami", &[(header::COOKIE, cookie)]).await;
        assert_eq!(reply.status, StatusCode::OK);
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(body["tenant"], tenant);
        assert!(body["name"].as_str().unwrap().starts_with("user:"));
        let page = send(&app, "/whoami", &[(header::COOKIE, cookie)]).await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(page.body.contains(tenant));
        assert_eq!(
            page.headers[header::CONTENT_SECURITY_POLICY],
            DEFAULT_HTML_CSP
        );
    }
    for headers in [
        vec![],
        vec![(header::COOKIE, "session=eve")],
        vec![(header::AUTHORIZATION, "Bearer session=ann")],
    ] {
        // The cookie in the query string is never read.
        let reply = send(&app, "/api/whoami?session=ann", &headers).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
        // A session authenticator has no scheme to announce.
        assert!(reply.headers.get(header::WWW_AUTHENTICATE).is_none());
        assert_eq!(reply.body, r#"{"error":"unauthorized"}"#);
    }
}

#[tokio::test]
async fn internal_failures_carry_no_detail_on_any_transport() {
    let state = AppState::builder(Notes::default())
        .html_auth(bearer())
        .api_auth(bearer())
        .build();
    let app = Endpoints::new()
        .api("/api/broken", &BROKEN)
        .html("/broken", &BROKEN, page)
        .into_router(state)
        .unwrap();
    let authorization = format!("Bearer {TOKEN}");
    let credentials = [(header::AUTHORIZATION, authorization.as_str())];
    let api = send(&app, "/api/broken?q=x", &credentials).await;
    assert_eq!(api.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(api.body, r#"{"error":"internal"}"#);
    let html = send(&app, "/broken?q=x", &credentials).await;
    assert_eq!(html.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(html.body, "Internal Server Error");
    assert!(html.headers.get(header::CONTENT_SECURITY_POLICY).is_none());
    for reply in [&api, &html] {
        assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(reply.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
    // The permission and the envelope still come first.
    let unknown = send(&app, "/api/broken?zz=1", &credentials).await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);

    let server = McpServer::with_tools(Notes::default(), clerk(), vec![mcp::tool(&BROKEN)])
        .expect("a valid list");
    let result = server
        .call(&CallToolRequestParams::new("broken"))
        .await
        .expect("a tool result");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content,
        Some(json!({ "error": "internal" }))
    );
}

#[tokio::test]
async fn the_operation_runs_once_and_only_after_every_check() {
    let state = AppState::builder(Notes::default())
        .api_auth(bearer())
        .build();
    let counter = |app: &AppState<Notes>| app.state().runs.load(Ordering::SeqCst);
    let count = axum::routing::get(
        move |axum::extract::State(app): axum::extract::State<senu::http::Shared<Notes>>| async move {
            counter(&app).to_string()
        },
    );
    let app = Endpoints::new()
        .api("/api/whoami", &WHOAMI)
        .custom("/runs", count)
        .into_router(state)
        .unwrap();
    let authorization = format!("Bearer {TOKEN}");
    let good = [(header::AUTHORIZATION, authorization.as_str())];
    for (target, headers, status) in [
        ("/api/whoami", &[][..], StatusCode::UNAUTHORIZED),
        ("/api/whoami?zz=1", &good[..], StatusCode::BAD_REQUEST),
        ("/api/nothing", &good[..], StatusCode::NOT_FOUND),
    ] {
        assert_eq!(send(&app, target, headers).await.status, status, "{target}");
    }
    assert_eq!(send(&app, "/runs", &[]).await.body, "0");
    assert_eq!(
        send(&app, "/api/whoami", &good).await.status,
        StatusCode::OK
    );
    assert_eq!(send(&app, "/runs", &[]).await.body, "1");
}

#[tokio::test]
async fn the_csp_and_assets_are_checked_with_the_table_and_then_served() {
    let state = || {
        AppState::builder(Notes::default())
            .html_auth(bearer())
            .build()
    };
    let html = || Endpoints::new().html("/whoami", &WHOAMI, page);
    for policy in [
        "",
        "   ",
        "default-src 'none'\nx",
        "default-src 'non\u{e9}'",
    ] {
        let refused = html().html_csp(policy).into_router(state()).err();
        assert_eq!(refused, Some(RouteError::InvalidCsp), "{policy:?}");
    }
    for content_type in ["", " ", "text/css\r\nx: y"] {
        let table = Endpoints::<Notes>::new().asset("/app.css", content_type, b"");
        let refused = table.into_router(state()).err();
        assert_eq!(refused, Some(RouteError::InvalidAsset("/app.css")));
    }
    let asset = |path| Endpoints::<Notes>::new().asset(path, "text/css", b"");
    assert_eq!(
        asset("/api/app.css").into_router(state()).err(),
        Some(RouteError::WrongPrefix("/api/app.css"))
    );
    assert_eq!(
        asset("/_senu/app.css").into_router(state()).err(),
        Some(RouteError::Reserved("/_senu/app.css"))
    );
    assert_eq!(
        asset("/app.css")
            .asset("/app.css", "text/css", b"")
            .into_router(state())
            .err(),
        Some(RouteError::Duplicate("/app.css"))
    );

    const POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'";
    let table = html().html_csp(POLICY).asset(
        "/logo.svg",
        "image/svg+xml",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    );
    assert_eq!(table.csp(), POLICY);
    assert_eq!(html().csp(), DEFAULT_HTML_CSP);
    let kinds: Vec<Kind> = table.declared().iter().map(|route| route.kind).collect();
    assert_eq!(kinds, [Kind::Html, Kind::Asset]);
    let app = table.into_router(state()).unwrap();
    let authorization = format!("Bearer {TOKEN}");
    let page = send(&app, "/whoami", &[(header::AUTHORIZATION, &authorization)]).await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(page.headers[header::CONTENT_SECURITY_POLICY], POLICY);
    let logo = send(&app, "/logo.svg", &[]).await;
    assert_eq!(logo.status, StatusCode::OK);
    assert_eq!(logo.headers[header::CONTENT_TYPE], "image/svg+xml");
    assert!(logo.body.starts_with("<svg"));
    // The built-in stylesheet is still there.
    let css = send(&app, "/_senu/ui.css", &[]).await;
    assert_eq!(css.status, StatusCode::OK);
}

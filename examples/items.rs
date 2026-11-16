//! Reads and writes on Senu: a small tracker of items per project.
//!
//! One read and three writes are declared once and served as a JSON API and
//! as MCP tools. People sign in with a session cookie, which the example's
//! own authenticator reads; a page request without one is sent to `/login`.
//! Every write passes Senu's cross-site check before authentication, and
//! nothing of its body is read before the permission is known.
//!
//! ```sh
//! ITEMS_SESSION=choose-a-long-random-value cargo run --example items
//! curl -b 'sid=choose-a-long-random-value' http://127.0.0.1:3000/api/items
//! curl -b 'sid=choose-a-long-random-value' -H 'Content-Type: application/json' \
//!   -d '{"title":"CSV import","lo":0,"hi":0.05}' http://127.0.0.1:3000/api/items/create
//! cargo run --example items -- mcp   # the tools over stdio
//! ```
//!
//! `tests/example_items.rs` runs Senu's contract over this file, through
//! this authenticator.

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use senu::auth::{self, AuthFuture, Authenticate};
use senu::http::{AppState, Endpoints, Shared};
use senu::mcp::{self, McpServer, McpTool};
use senu::operation::{
    Args, Effect, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
};
use serde::{Deserialize, Serialize};

// The state: items per tenant, and the sessions that are signed in.

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub lo: f64,
    pub hi: f64,
    pub status: String,
}

#[derive(Default)]
struct Inner {
    items: HashMap<TenantId, Vec<Item>>,
    next_id: u32,
    sessions: HashMap<String, Principal>,
}

#[derive(Clone, Default)]
pub struct Store(Arc<Mutex<Inner>>);

impl Store {
    pub fn sample() -> Self {
        let store = Self::default();
        {
            let mut inner = store.lock();
            let item = |id: &str, title: &str, status: &str| Item {
                id: id.to_owned(),
                title: title.to_owned(),
                lo: 0.0,
                hi: 0.5,
                status: status.to_owned(),
            };
            inner.items.insert(
                TenantId::from_static("north"),
                vec![
                    item("P-1", "Free invoice <template>", "idea"),
                    item("P-2", "Annual plan", "deployed"),
                ],
            );
            inner.items.insert(
                TenantId::from_static("south"),
                vec![item("P-1", "Another project's item", "idea")],
            );
            inner.next_id = 3;
        }
        store
    }

    /// Signs a session in. A real application does this after its login
    /// flow and keeps sessions in a database.
    pub fn sign_in(&self, session: &str, principal: Principal) {
        self.lock().sessions.insert(session.to_owned(), principal);
    }

    fn sign_out(&self, session: &str) {
        self.lock().sessions.remove(session);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock means a writer panicked; the data is still whole
        // because every write below is a single step.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// Permissions and principals. Reading and writing are separate permissions.

pub const VIEW_ITEMS: Permission = Permission::new("view_items");
pub const EDIT_ITEMS: Permission = Permission::new("edit_items");

pub const EDITOR: Principal = Principal::new(
    "editor@north.example",
    TenantId::from_static("north"),
    &[VIEW_ITEMS, EDIT_ITEMS],
);

// Authentication: a session cookie. `auth::cookie` returns the value only
// when the cookie appears exactly once.

pub struct Sessions(pub Store);

impl Authenticate for Sessions {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            let session = auth::cookie(headers, "sid")?;
            self.0.lock().sessions.get(session).cloned()
        })
    }
}

// The operations.

const INVALID_ID: OpError = OpError::Invalid {
    code: "invalid_id",
    phrase: "Invalid id",
};
const INVALID_TITLE: OpError = OpError::Invalid {
    code: "invalid_title",
    phrase: "Invalid title",
};
const INVALID_RANGE: OpError = OpError::Invalid {
    code: "invalid_range",
    phrase: "Invalid range",
};
const ITEM_DEPLOYED: OpError = OpError::Conflict {
    code: "item_deployed",
    phrase: "A deployed item cannot be deleted",
};

fn checked_id(id: &str) -> Result<&str, OpError> {
    let well_formed =
        (1..=16).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if well_formed { Ok(id) } else { Err(INVALID_ID) }
}

fn checked_title(title: &str) -> Result<&str, OpError> {
    let title = title.trim();
    let well_formed =
        (1..=120).contains(&title.chars().count()) && !title.chars().any(char::is_control);
    if well_formed {
        Ok(title)
    } else {
        Err(INVALID_TITLE)
    }
}

#[derive(Serialize)]
pub struct ItemList {
    pub items: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

fn list_items(store: &Store, caller: &Principal, args: &Args) -> Result<ItemList, OpError> {
    let NoArgs {} = decode(args)?;
    let items = store.lock().items.get(caller.tenant()).cloned();
    Ok(ItemList {
        items: items.unwrap_or_default(),
    })
}

pub const LIST_ITEMS: Operation<Store, ItemList> = Operation {
    name: "list_items",
    description: "The items of the caller's project.",
    permission: VIEW_ITEMS,
    params: &[],
    effect: Effect::Read,
    run: Run::Sync(list_items),
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    title: String,
    lo: f64,
    hi: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Created {
    pub id: String,
}

fn create_item(store: &Store, caller: &Principal, args: &Args) -> Result<Created, OpError> {
    let CreateArgs { title, lo, hi } = decode(args)?;
    let title = checked_title(&title)?.to_owned();
    if !(lo.is_finite() && hi.is_finite() && lo <= hi) {
        return Err(INVALID_RANGE);
    }
    let mut inner = store.lock();
    let id = format!("P-{}", inner.next_id);
    inner.next_id += 1;
    // The tenant comes from the caller, never from the arguments.
    inner
        .items
        .entry(caller.tenant().clone())
        .or_default()
        .push(Item {
            id: id.clone(),
            title,
            lo,
            hi,
            status: "idea".to_owned(),
        });
    Ok(Created { id })
}

pub const CREATE_ITEM: Operation<Store, Created> = Operation {
    name: "create_item",
    description: "Adds an item to the caller's project and returns its id.",
    permission: EDIT_ITEMS,
    params: &[
        Param::required("title", "The title: 1 to 120 characters."),
        Param::required("lo", "The lower bound of the expected effect.").number(),
        Param::required("hi", "The upper bound; not below the lower bound.").number(),
    ],
    effect: Effect::CREATE,
    run: Run::Sync(create_item),
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameArgs {
    id: String,
    title: String,
}

fn rename_item(store: &Store, caller: &Principal, args: &Args) -> Result<Item, OpError> {
    let RenameArgs { id, title } = decode(args)?;
    let (id, title) = (checked_id(&id)?, checked_title(&title)?);
    let mut inner = store.lock();
    // Another project's item is not found, exactly like one that is absent.
    let item = inner
        .items
        .get_mut(caller.tenant())
        .and_then(|items| items.iter_mut().find(|item| item.id == id))
        .ok_or(OpError::NotFound)?;
    item.title = title.to_owned();
    Ok(item.clone())
}

pub const RENAME_ITEM: Operation<Store, Item> = Operation {
    name: "rename_item",
    description: "Changes the title of one item of the caller's project.",
    permission: EDIT_ITEMS,
    params: &[
        Param::required("id", "The item's id, such as P-1."),
        Param::required("title", "The new title: 1 to 120 characters."),
    ],
    effect: Effect::UPDATE,
    run: Run::Sync(rename_item),
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteArgs {
    id: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Deleted {
    pub deleted: bool,
}

fn delete_item(store: &Store, caller: &Principal, args: &Args) -> Result<Deleted, OpError> {
    let DeleteArgs { id } = decode(args)?;
    let id = checked_id(&id)?;
    let mut inner = store.lock();
    let Some(items) = inner.items.get_mut(caller.tenant()) else {
        return Ok(Deleted { deleted: false });
    };
    let Some(at) = items.iter().position(|item| item.id == id) else {
        // Deleting what is not there is not an error, so a retry is safe.
        return Ok(Deleted { deleted: false });
    };
    if items[at].status == "deployed" {
        // Well-formed, but not possible now: 409 with its own code.
        return Err(ITEM_DEPLOYED);
    }
    items.remove(at);
    Ok(Deleted { deleted: true })
}

pub const DELETE_ITEM: Operation<Store, Deleted> = Operation {
    name: "delete_item",
    description: "Deletes one item of the caller's project, unless it is deployed.",
    permission: EDIT_ITEMS,
    params: &[Param::required("id", "The item's id, such as P-1.")],
    effect: Effect::DELETE,
    run: Run::Sync(delete_item),
};

// Routes of the application's own: a login page and a logout that takes a
// form body. Senu gives them its limits and response headers and nothing
// else, so the logout calls the cross-site check itself.

async fn login_page() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Sign in here. A real application starts its login flow on this page.\n",
    )
}

async fn logout(State(app): State<Shared<Store>>, headers: HeaderMap) -> impl IntoResponse {
    if !auth::same_origin(&headers, &[]) {
        return (StatusCode::FORBIDDEN, "Cross-origin request refused\n");
    }
    if let Some(session) = auth::cookie(&headers, "sid") {
        app.state().sign_out(session);
    }
    (StatusCode::OK, "Signed out\n")
}

// Exposure: only what is listed here is reachable. A read is served by
// GET, a write by POST with a JSON body.

pub fn endpoints() -> Endpoints<Store> {
    Endpoints::new()
        .api("/api/items", &LIST_ITEMS)
        .api("/api/items/create", &CREATE_ITEM)
        .api("/api/items/rename", &RENAME_ITEM)
        .api("/api/items/delete", &DELETE_ITEM)
        .custom("/login", get(login_page))
        .custom_with_body("/logout", 1024, post(logout))
}

pub fn tools() -> Vec<McpTool<Store>> {
    vec![
        mcp::tool(&LIST_ITEMS),
        mcp::tool(&CREATE_ITEM),
        mcp::tool(&RENAME_ITEM),
        mcp::tool(&DELETE_ITEM),
    ]
}

/// The state with the session authenticator on both surfaces. A page
/// request without a session goes to the login page; the API answers 401.
pub fn state(store: Store) -> AppState<Store> {
    AppState::builder(store.clone())
        .html_auth(Sessions(store.clone()))
        .api_auth(Sessions(store))
        .html_login("/login")
        .build()
}

// The process.

async fn serve() -> Result<(), String> {
    let session = std::env::var("ITEMS_SESSION").map_err(|_| "items: set ITEMS_SESSION")?;
    if session.len() < 16
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("items: ITEMS_SESSION must be 16 or more letters, digits or '-'".to_owned());
    }
    let store = Store::sample();
    store.sign_in(&session, EDITOR);
    let router = endpoints()
        .into_router(state(store))
        .map_err(|err| err.to_string())?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|err| format!("items: cannot listen: {}", err.kind()))?;
    eprintln!("items: listening on http://127.0.0.1:3000");
    axum::serve(listener, router)
        .await
        .map_err(|_| "items: the server stopped".to_owned())
}

async fn serve_mcp() -> Result<(), String> {
    // Over stdio every call runs as this one principal, which may write.
    // Start it only for a user you would let edit the project.
    McpServer::with_tools(Store::sample(), EDITOR, tools())
        .map_err(|err| err.to_string())?
        .with_info("items", "1.0.0")
        .serve_stdio()
        .await
        .map_err(|err| err.to_string())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let done = match std::env::args().nth(1).as_deref() {
        None => serve().await,
        Some("mcp") => serve_mcp().await,
        Some(_) => Err("usage: items [mcp]".to_owned()),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

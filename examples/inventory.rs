//! A small service on Senu's library API.
//!
//! It has two asynchronous operations over a shared store, API keys read
//! from an `X-Api-Key` header by its own authenticator, one HTML page behind
//! Basic, its own stylesheet, and the same operations as MCP tools. Each
//! caller sees only its own tenant's items.
//!
//! ```sh
//! INVENTORY_WEB_PASSWORD=choose-one INVENTORY_API_KEY=choose-another \
//!   cargo run --example inventory
//! curl -H 'X-Api-Key: choose-another' 'http://127.0.0.1:3000/api/stock?sku=BOLT-8'
//! cargo run --example inventory -- mcp   # the tools over stdio
//! ```
//!
//! `tests/example_inventory.rs` runs Senu's contract over this file. In your
//! own crate you need `senu` with `default-features = false`, `axum`,
//! `tokio`, `serde` and, for pages, `askama`.

use std::process::ExitCode;
use std::sync::Arc;

use askama::Template;
use axum::http::{HeaderMap, HeaderValue};
use senu::auth::{AuthFuture, Authenticate, Basic, Secret};
use senu::http::{AppState, Endpoints};
use senu::mcp::{self, McpServer, McpTool};
use senu::operation::{
    Args, Effect, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
};
use serde::{Deserialize, Serialize};

// The state: a store that is slow to read, as a file or a synchronous
// database client would be.

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Level {
    pub sku: String,
    pub name: String,
    pub units: u32,
}

struct Item {
    tenant: TenantId,
    level: Level,
}

pub struct Inventory(Arc<Vec<Item>>);

impl Inventory {
    pub fn sample() -> Self {
        let item = |tenant, sku: &str, name: &str, units| Item {
            tenant: TenantId::from_static(tenant),
            level: Level {
                sku: sku.to_owned(),
                name: name.to_owned(),
                units,
            },
        };
        Self(Arc::new(vec![
            item("north", "BOLT-8", "Bolt <M8>", 240),
            item("north", "NUT-8", "Nut M8", 3),
            item("north", "WASHER-8", "Washer M8", 0),
            item("south", "BOLT-8", "Bolt M8 (south)", 7),
        ]))
    }

    /// The caller's own items, read on the blocking pool. Callers of
    /// another tenant never see them. A failure of the pool is `internal`,
    /// with no detail.
    async fn levels(&self, caller: &Principal) -> Result<Vec<Level>, OpError> {
        let (items, tenant) = (Arc::clone(&self.0), caller.tenant().clone());
        let read = move || {
            items
                .iter()
                .filter(|item| item.tenant == tenant)
                .map(|item| item.level.clone())
                .collect()
        };
        tokio::task::spawn_blocking(read)
            .await
            .map_err(|_| OpError::Internal)
    }
}

// Permissions and principals.

pub const VIEW_STOCK: Permission = Permission::new("view_stock");

pub const NORTH_CLERK: Principal =
    Principal::new("north-clerk", TenantId::from_static("north"), &[VIEW_STOCK]);

// The operations. Each is declared once; nothing is exposed until it is
// listed in `endpoints` or `tools`.

const INVALID_SKU: OpError = OpError::Invalid {
    code: "invalid_sku",
    phrase: "Invalid SKU",
};

const INVALID_BELOW: OpError = OpError::Invalid {
    code: "invalid_below",
    phrase: "Invalid threshold",
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockArgs {
    sku: String,
}

async fn get_stock(
    inventory: &Inventory,
    caller: &Principal,
    args: &Args,
) -> Result<Level, OpError> {
    let StockArgs { sku } = decode(args)?;
    let well_formed = (1..=16).contains(&sku.len())
        && sku
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-');
    if !well_formed {
        return Err(INVALID_SKU);
    }
    let levels = inventory.levels(caller).await?;
    levels
        .into_iter()
        .find(|level| level.sku == sku)
        .ok_or(OpError::NotFound)
}

pub const GET_STOCK: Operation<Inventory, Level> = Operation {
    name: "get_stock",
    description: "Units in stock for one SKU of the caller's warehouse.",
    permission: VIEW_STOCK,
    params: &[Param::required(
        "sku",
        "The SKU: 1 to 16 characters of A-Z, 0-9 and '-'.",
    )],
    effect: Effect::Read,
    run: Run::Async(|inventory, caller, args| Box::pin(get_stock(inventory, caller, args))),
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LowStockArgs {
    below: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct LowStock {
    pub below: u32,
    pub items: Vec<Level>,
}

async fn list_low_stock(
    inventory: &Inventory,
    caller: &Principal,
    args: &Args,
) -> Result<LowStock, OpError> {
    let LowStockArgs { below } = decode(args)?;
    // Parameters are strings; the operation parses its own numbers.
    let below = match below.as_deref() {
        None => 5,
        Some(text) if text.len() <= 6 && text.bytes().all(|b| b.is_ascii_digit()) => {
            text.parse().map_err(|_| INVALID_BELOW)?
        }
        Some(_) => return Err(INVALID_BELOW),
    };
    let mut items = inventory.levels(caller).await?;
    items.retain(|level| level.units < below);
    items.sort_by(|a, b| a.sku.cmp(&b.sku));
    Ok(LowStock { below, items })
}

pub const LIST_LOW_STOCK: Operation<Inventory, LowStock> = Operation {
    name: "list_low_stock",
    description: "Items of the caller's warehouse with fewer units than a threshold, by SKU.",
    permission: VIEW_STOCK,
    params: &[Param::optional(
        "below",
        "The threshold: a whole number of at most 6 digits. Without it, 5.",
    )],
    effect: Effect::Read,
    run: Run::Async(|inventory, caller, args| Box::pin(list_low_stock(inventory, caller, args))),
};

// The page. Askama escapes every value, so `Bolt <M8>` is shown as text.
// `syntax = "default"` keeps this template independent of Senu's own
// `askama.toml`.

#[derive(Template)]
#[template(
    ext = "html",
    syntax = "default",
    source = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Low stock</title>
<link rel="stylesheet" href="/assets/inventory.css">
</head>
<body>
<h1>Fewer than {{ list.below }} units</h1>
<ul>
{% for item in list.items %}<li data-sku="{{ item.sku }}">{{ item.name }}: {{ item.units }}</li>
{% endfor %}</ul>
</body>
</html>
"#
)]
struct LowStockPage<'a> {
    list: &'a LowStock,
}

fn low_stock_page(_args: &Args, list: &LowStock) -> Result<String, askama::Error> {
    LowStockPage { list }.render()
}

const STYLESHEET: &[u8] = b"body { font-family: system-ui, sans-serif; margin: 2rem; }\n";

// Exposure: only what is listed here is reachable.

pub fn endpoints() -> Endpoints<Inventory> {
    Endpoints::new()
        .api("/api/stock", &GET_STOCK)
        .api("/api/low-stock", &LIST_LOW_STOCK)
        .html("/low-stock", &LIST_LOW_STOCK, low_stock_page)
        .asset(
            "/assets/inventory.css",
            "text/css; charset=utf-8",
            STYLESHEET,
        )
}

pub fn tools() -> Vec<McpTool<Inventory>> {
    vec![mcp::tool(&GET_STOCK), mcp::tool(&LIST_LOW_STOCK)]
}

// Authentication: one API key per client in an `X-Api-Key` header. A real
// service would look a hash of the key up in its database here.

pub struct ApiKeys(Vec<(Secret, Principal)>);

impl ApiKeys {
    pub fn new(keys: Vec<(Secret, Principal)>) -> Self {
        Self(keys)
    }
}

impl Authenticate for ApiKeys {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        Box::pin(async move {
            // Exactly one header, as for `Authorization`.
            let mut values = headers.get_all("x-api-key").iter();
            let (Some(presented), None) = (values.next(), values.next()) else {
                return None;
            };
            // Compare with every key, so the time does not show which one
            // matched.
            let mut found = None;
            for (key, principal) in &self.0 {
                if key.matches(presented.as_bytes()) && found.is_none() {
                    found = Some(principal.clone());
                }
            }
            found
        })
    }

    fn challenge(&self) -> Option<HeaderValue> {
        Some(HeaderValue::from_static("ApiKey realm=\"inventory\""))
    }
}

// The process: configuration from the environment, then one of two modes.

fn secret(variable: &str) -> Result<Secret, String> {
    let value = std::env::var(variable).map_err(|_| format!("inventory: set {variable}"))?;
    // The message names the variable, never the value.
    Secret::new(value).map_err(|err| format!("inventory: {variable}: {err}"))
}

async fn serve() -> Result<(), String> {
    let state = AppState::builder(Inventory::sample())
        .html_auth(Basic::new(
            Secret::new("clerk").map_err(|err| err.to_string())?,
            secret("INVENTORY_WEB_PASSWORD")?,
            NORTH_CLERK,
        ))
        .api_auth(ApiKeys::new(vec![(
            secret("INVENTORY_API_KEY")?,
            NORTH_CLERK,
        )]))
        .build();
    // A plain `axum::Router`: add your own tower layers for logging,
    // timeouts and rate limits here, and terminate TLS in front of it.
    let router = endpoints()
        .into_router(state)
        .map_err(|err| err.to_string())?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|err| format!("inventory: cannot listen: {}", err.kind()))?;
    eprintln!("inventory: listening on http://127.0.0.1:3000");
    axum::serve(listener, router)
        .await
        .map_err(|_| "inventory: the server stopped".to_owned())
}

async fn serve_mcp() -> Result<(), String> {
    // Stdio trusts the local process that starts the server, so every call
    // is bound to one principal chosen here.
    McpServer::with_tools(Inventory::sample(), NORTH_CLERK, tools())
        .map_err(|err| err.to_string())?
        .with_info("inventory", "1.0.0")
        .serve_stdio()
        .await
        .map_err(|err| err.to_string())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mode = std::env::args().nth(1);
    let done = match mode.as_deref() {
        None => serve().await,
        Some("mcp") => serve_mcp().await,
        Some(_) => Err("usage: inventory [mcp]".to_owned()),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

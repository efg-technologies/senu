# Declare an operation

An operation is a read-only function with a name, a description, a permission and string parameters. You declare it once as a constant and list it where it should be reachable.

## What is an operation made of?

The examples on this page come from `examples/inventory.rs`, a small stock service. Its first operation looks one item up:

```rust excerpt=examples/inventory.rs
pub const GET_STOCK: Operation<Inventory, Level> = Operation {
    name: "get_stock",
    description: "Units in stock for one SKU of the caller's warehouse.",
    permission: VIEW_STOCK,
    params: &[Param::required(
        "sku",
        "The SKU: 1 to 16 characters of A-Z, 0-9 and '-'.",
    )],
    run: Run::Async(|inventory, caller, args| Box::pin(get_stock(inventory, caller, args))),
};
```

- `name` is snake_case of at most 64 bytes. It becomes the MCP tool's name.
- `description` is one sentence for people and assistants. It becomes the tool's description, so say what the operation returns and for whom.
- `permission` is a constant of your application, such as `Permission::new("view_stock")`. It is checked before anything else.
- `params` are the only keys a caller may send. Each is a string with a description; `Param::required` must be present and `Param::optional` may be left out.
- `run` is the function: `Run::Sync` for pure work in memory, `Run::Async` for anything that waits.

Senu checks this metadata when the router or the tool list is built. A bad name, an empty description or a parameter declared twice is an error at startup, not a surprise in production.

## How do you write the function?

The function gets your state, the caller and the arguments, and returns a result that implements `Serialize`, or an `OpError`:

```rust excerpt=examples/inventory.rs
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
```

Three habits keep it safe:

1. Decode with `decode` into a struct marked `#[serde(deny_unknown_fields)]`. A missing or mistyped field is then `invalid_request`.
2. Read the tenant from the caller, never from the arguments. `caller.tenant()` was set by the server when it authenticated the request.
3. Answer `OpError::NotFound` both for what does not exist and for what belongs to another tenant, so nobody can probe for other tenants' data.

## What errors can an operation return?

| Error | HTTP | Code in JSON and MCP | Meaning |
|---|---|---|---|
| `OpError::Forbidden` | 403 | `forbidden` | The caller lacks the permission. Senu returns this itself before your function runs. |
| `OpError::InvalidRequest` | 400 | `invalid_request` | The arguments do not fit the declared parameters. |
| `OpError::Invalid { code, phrase }` | 400 | your `code` | Your own validation error, such as `invalid_sku`. |
| `OpError::NotFound` | 404 | `not_found` | Nothing matches for this caller. |
| `OpError::Internal` | 500 | `internal` | A backend failed. Log the cause yourself; the caller sees no detail. |

Every code and phrase is a fixed string, so an error can never repeat the input or a secret. Under `/api/` the body is `{"error": "<code>"}`; HTML routes answer with the phrase as plain text; an MCP tool returns a result with `isError` and the same JSON.

## How do asynchronous operations work?

Use `Run::Async` with a function that returns a boxed future. The closure in the declaration above, `|inventory, caller, args| Box::pin(get_stock(inventory, caller, args))`, is all the glue there is. Inside, await a database client, or move blocking work off the executor:

```rust excerpt=examples/inventory.rs
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
```

A `Run::Sync` function runs directly on the async executor. Keep it to work on data already in memory; if it can block, make it asynchronous.

## How do you expose an operation?

List it in an `Endpoints` table and in a tool list. Only what is listed is reachable:

```rust excerpt=examples/inventory.rs
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
```

- `.api(path, &OPERATION)` serves JSON. The path must be under `/api/`.
- `.html(path, &OPERATION, view)` serves a page. `view` is a function from the accepted arguments and the result to HTML; with Askama, every value is escaped by default.
- `.asset(path, content_type, bytes)` serves a public static file, such as your stylesheet.
- `.custom(path, handler)` is the escape hatch for anything else. It gets the size limits and response headers, but no authentication: that handler is yours.
- `.html_csp(policy)` replaces the pages' Content-Security-Policy. The default allows same-origin stylesheets and forms and nothing else: no scripts, no images, no frames.

`into_router(state)` checks the whole table and returns an `axum::Router`, or an error that names the path. Paths must be exact, unique and outside `/_senu/`, which is reserved for Senu's built-in stylesheet.

## What order do the checks run in?

Always the same: limits, method, authentication, permission, the parameter envelope, then your function. Permission before validation matters: a caller without the permission learns nothing about which inputs would have been valid. [The contract tests](/testing) verify this order on your own routes.

Next: [authenticate callers](/authentication).

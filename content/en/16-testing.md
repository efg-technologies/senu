# Test with the contract

Senu ships its own security tests as a kit. One call sends real requests through your routes and tools and fails when the rules that Senu promises do not hold for your application.

## What does the contract check?

| Check | What must hold |
|---|---|
| CT-1 | Missing, wrong or duplicated credentials are 401 with a challenge, before the permission and before any input is read. |
| CT-2 | Other methods are 405, a body is 413 and a long address is 414, all before credentials. |
| CT-3 | A principal without the permission gets 403, whatever the input. |
| CT-4 | Unknown, repeated and non-string parameters are `invalid_request`. |
| CT-5 | The same arguments give the same outcome as JSON, as a page and as a tool result, and each sample succeeds. |
| CT-6 | Exactly the listed tools exist, each with a closed schema that matches its parameters; a read is marked read-only. |
| CT-7 | Pages and errors carry `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` and, for pages, the CSP. |
| CT-8 | A write answers only `POST`; every other method is 405 with `Allow: POST`. |
| CT-9 | A cross-site browser request to a write is 403 `cross_origin`, even with good credentials. |
| CT-10 | A write without credentials is 401, and without the permission 403, each with a body that is not JSON: the body was not read. |
| CT-11 | For a write, a wrong `Content-Type` is 415, a body over the limit 413, and a non-object, a query string, an unknown key or a value of the wrong type `invalid_request`. |
| CT-12 | A write's tool is not marked read-only, its annotations match the declaration, and its sample succeeds as JSON and as a tool call. |

## How do you run it on your application?

Enable the feature for tests only:

```toml
[dev-dependencies]
senu = { version = "0.3", default-features = false, features = ["contract"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

Then give the kit your state, your route table, your tool list, a principal that holds every permission, and one sample per operation that succeeds: a query string for a read and a JSON object for a write. With `auth: None` the kit uses its own Basic and Bearer credentials:

```rust excerpt=tests/example_inventory.rs
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
```

In this repository the same test runs over `examples/inventory.rs`:

```sh check=contract
cargo test --quiet --test example_inventory
```

```text check=contract

running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

A violation panics with the check and the route in the message, such as `CT-3 /api/stock?zz_unknown=1`.

## How do you run it through your own authenticator?

Set `auth` to a function that builds your state for a given principal, and the request headers that authenticate. `examples/items.rs` uses cookie sessions, so its test signs a session in for the principal the kit passes and names the cookie:

```rust excerpt=tests/example_items.rs
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
```

The kit calls the function twice: with your principal, and with one that holds no permission. Write samples run in the order of the route table, on one state, so the delete above removes the item that the rename just renamed.

## What does the contract not cover?

- **Your authenticator's own rules.** With `Subject::auth` the contract runs through your authenticator, as the next section shows, but it only knows the headers you gave it. Test expiry, sign-out and rotation yourself.
- **Your business rules.** Whether `get_stock` returns the right number is yours to test, next to the function.
- **Tenant isolation.** Write a test in which one tenant's credential asks for another tenant's data and gets `not_found`.
- **`custom` routes and assets.** They are your handlers.

## How do you test an operation directly?

An operation is a plain function, so most tests need no HTTP at all. Call the function with a state, a principal and arguments. To include Senu's permission and envelope checks, call the declaration instead:

```rust
let args = senu::http::query_arguments(Some("sku=BOLT-8"));
let level = GET_STOCK.call(&Inventory::sample(), &NORTH_CLERK, &args).await?;
assert_eq!(level.units, 240);
```

## How do you know the tests can fail?

Break something on purpose once. In Senu's own development each contract check was seen to fail under a matching change: removing the fail-closed check for a missing authenticator, accepting any Bearer token, validating before the permission, dropping the page's CSP, and for writes skipping the cross-site check, checking the permission after the body, accepting any content type, accepting any method and marking a write read-only. A test that has never failed has not yet proved anything.

Next: [questions and comparisons](/faq).

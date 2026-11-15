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
| CT-6 | Exactly the listed tools exist, each read-only with a closed schema that matches the route's parameters. |
| CT-7 | Pages and errors carry `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` and, for pages, the CSP. |

## How do you run it on your application?

Enable the feature for tests only:

```toml
[dev-dependencies]
senu = { version = "0.2", default-features = false, features = ["contract"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

Then give the kit your state, your route table, your tool list, a principal that holds every permission, and one query per operation that succeeds:

```rust excerpt=tests/example_inventory.rs
#[tokio::test]
async fn the_example_meets_senus_contract() {
    contract::check(Subject {
        state: Inventory::sample,
        endpoints: inventory::endpoints,
        tools: inventory::tools,
        principal: NORTH_CLERK,
        samples: &[("get_stock", "sku=BOLT-8"), ("list_low_stock", "below=10")],
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

## What does the contract not cover?

- **Your authenticator.** The kit brings its own Basic and Bearer credentials, because it tests the adapters and your declarations. Test your authenticator directly: right, wrong, missing and duplicated credentials.
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

Break something on purpose once. In Senu's own development each contract check was seen to fail under a matching change: removing the fail-closed check for a missing authenticator, accepting any Bearer token, validating before the permission, and dropping the page's CSP. A test that has never failed has not yet proved anything.

Next: [questions and comparisons](/faq).

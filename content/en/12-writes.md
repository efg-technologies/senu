# Declare a write operation

A write is an operation that changes state. You declare it like a read, with `effect: Effect::CREATE`, `UPDATE` or `DELETE`, and Senu serves it by `POST` with a JSON body, behind a cross-site check, and as an MCP tool that is not marked read-only.

## How is a write declared?

The examples on this page come from `examples/items.rs`, a small tracker with cookie sessions. This write adds an item:

```rust excerpt=examples/items.rs
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
```

- `effect` says what the operation does to state. `Effect::Read` changes nothing. `Effect::CREATE` only adds; `Effect::UPDATE` and `Effect::DELETE` overwrite or remove, and repeating them is harmless. For anything else, write `Effect::Write { destructive, idempotent }` yourself.
- A write's parameters may be numbers and booleans as well as strings: `.number()`, `.integer()` and `.boolean()` change a parameter's kind. A read's parameters stay strings, because a query string carries nothing else; Senu refuses any other declaration at startup.
- The permission is usually a different one from the read's, so a principal can be allowed to look without being allowed to change.

The function decodes into its own struct, now with typed fields:

```rust excerpt=examples/items.rs
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
```

Senu has already checked that `title` is a string and `lo` and `hi` are numbers, that no other key was sent and that none is null. The rules that only your application knows, such as the length of a title, stay in the function and return your own error codes.

## How is a write served?

List it with `.api`, as you list a read. The path decides nothing; the declaration does:

```rust excerpt=examples/items.rs
pub fn endpoints() -> Endpoints<Store> {
    Endpoints::new()
        .api("/api/items", &LIST_ITEMS)
        .api("/api/items/create", &CREATE_ITEM)
        .api("/api/items/rename", &RENAME_ITEM)
        .api("/api/items/delete", &DELETE_ITEM)
        .custom("/login", get(login_page))
        .custom_with_body("/logout", 1024, post(logout))
}
```

A write route answers `POST` only and takes one JSON object as its body. From a terminal:

```sh manual
curl -b 'sid=choose-a-long-random-value' -H 'Content-Type: application/json' -d '{"title":"CSV import","lo":0,"hi":0.05}' http://127.0.0.1:3000/api/items/create
```

```text
{"id":"P-3"}
```

A success is always `200` with the function's result as JSON. A write cannot be listed with `.html`; a page that changes something posts to the JSON route from its own script, or uses a `custom` route.

## In what order is a write checked?

Always in this order, and each step answers before the next one runs:

1. Limits: a long address is 414, and a declared body length over the route's limit is 413.
2. The method: anything but `POST` is 405 with `Allow: POST`. A write never runs on `GET`.
3. The cross-site check below: 403 `cross_origin`.
4. Authentication: 401.
5. The permission: 403.
6. The query string must be empty, and `Content-Type` must be `application/json`: otherwise `invalid_request` or 415.
7. The body is read, up to the limit, and must be a JSON object: otherwise 413 or `invalid_request`.
8. The parameter envelope: unknown keys and values of the wrong kind are `invalid_request`.
9. Your function.

Nothing of the body is read before step 7. A caller without a credential or without the permission costs no parsing and learns nothing about which input would have been valid.

## How does Senu stop cross-site request forgery?

A browser attaches cookies and Basic credentials to requests that another site triggers. Senu therefore checks every write, whatever its authenticator, with headers that a browser sets and a page cannot forge:

1. If the request has `Sec-Fetch-Site`, only `same-origin` and `none` pass.
2. Otherwise, if it has `Origin`, the origin's host and port must equal the request's `Host`.
3. With neither header the request does not come from a browser, and passes.

These are the rules of Go's `CrossOriginProtection`. If your pages live on another origin than your API, name it with `Endpoints::trusted_origin("https://app.example.com")`. Senu sends no CORS headers, so a page on any other origin cannot read a response either.

The check protects a signed-in person's browser. It does nothing against a program that holds a valid credential, which needs no forgery to send a request.

## How large can a body be?

64 KiB by default. `Endpoints::max_body_bytes(n)` changes the limit for the table's writes, up to 16 MiB. The limit holds for the declared length and for what actually arrives. Reads and assets still refuse any body.

## How does a write say "not now"?

With `OpError::Conflict`, which is 409: the request is well-formed, but the state does not allow it.

```rust excerpt=examples/items.rs
const ITEM_DEPLOYED: OpError = OpError::Conflict {
    code: "item_deployed",
    phrase: "A deployed item cannot be deleted",
};
```

```rust excerpt=examples/items.rs
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
```

| Outcome | HTTP | Body |
|---|---|---|
| Done | 200 | the function's result as JSON |
| Malformed arguments | 400 | `{"error":"invalid_request"}` |
| Your own validation error | 400 | your code, such as `{"error":"invalid_title"}` |
| Nothing matches for this caller | 404 | `{"error":"not_found"}` |
| Not possible now | 409 | your code, such as `{"error":"item_deployed"}` |
| A backend failed | 500 | `{"error":"internal"}` |

Design a delete so that a second identical call succeeds without changing anything, as the example does. Then a client or an assistant that retries after a lost answer does no harm.

## What does a write look like as an MCP tool?

`mcp::tool(&CREATE_ITEM)` lists it, like a read. The annotations follow the declaration: `readOnlyHint` is `false`, and `destructiveHint` and `idempotentHint` are what the effect says, so a client can ask its user before it calls a tool that changes or removes something. The schema gives each parameter its type, such as `"lo": {"type": "number"}`.

Over stdio every call runs as the one principal chosen at startup. Give that principal a write permission only for a process you would let write.

## What does Senu not do for a write?

- It does not make your function atomic or idempotent. Use a transaction where your storage has one.
- It has no idempotency keys and no optimistic locking.
- Arguments are flat scalars. An operation that needs a list or a nested structure takes a JSON string and parses it.
- HTML form posts and file uploads are not declared operations; use a `custom` route.

Next: [authenticate callers](/authentication).

# What Senu is and when to use it

Senu is a Rust library that serves one declared function as an HTML page, a JSON API and an MCP tool, with the same checks in the same order on all three.

## What problem does Senu solve?

A service that people, programs and AI assistants all use needs three front doors: pages for people, JSON for programs and MCP tools for assistants. Written by hand, each door repeats the same steps: authenticate, authorize, validate, call the function, map the error. The copies drift apart, and the door that forgets a check becomes the way in.

With Senu you write the function once and declare what it needs. Senu's adapters then run the same steps for every route and tool, in one fixed order:

1. Size limits and the method: a long address is 414, a body where none is taken is 413, and a read answers only `GET`, a write only `POST`.
2. Authentication: your authenticator turns the request's headers into a server-side principal, or the answer is 401.
3. Permission: a principal without the operation's permission gets 403, whatever it sent.
4. The parameter envelope: unknown or repeated parameters and values of the wrong type are `invalid_request`.
5. Your function.
6. The result as JSON, as a page or as a tool result, or one fixed error that repeats neither the input nor a secret.

Declaring an operation exposes nothing. A route or a tool exists only where you list it, and Senu refuses at startup a table that has a route without an authenticator.

## What does a complete application look like?

This is `examples/hello.rs` from the repository, complete: one operation behind a Bearer token, also listed as an MCP tool.

```rust excerpt=examples/hello.rs
use senu::auth::{Bearer, Secret};
use senu::http::{AppState, Endpoints};
use senu::mcp::{self, McpServer};
use senu::operation::{
    Args, Effect, OpError, Operation, Param, Permission, Principal, Run, TenantId, decode,
};
use serde::{Deserialize, Serialize};

// 1. Your state. Here it is one string; in a real service it holds a
//    database pool.
struct Greeter {
    greeting: &'static str,
}

// 2. A permission, and a principal that holds it.
const GREET: Permission = Permission::new("greet");
const GUEST: Principal = Principal::new("guest", TenantId::from_static("main"), &[GREET]);

// 3. The function: state, caller and arguments in, a result or a safe
//    error out. It knows nothing about HTTP or MCP.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GreetingArgs {
    name: String,
}

#[derive(Serialize)]
struct Greeting {
    message: String,
}

fn greeting(greeter: &Greeter, _caller: &Principal, args: &Args) -> Result<Greeting, OpError> {
    let GreetingArgs { name } = decode(args)?;
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err(OpError::Invalid {
            code: "invalid_name",
            phrase: "Invalid name",
        });
    }
    Ok(Greeting {
        message: format!("{}, {name}!", greeter.greeting),
    })
}

// 4. The declaration. It exposes nothing by itself.
const GREETING: Operation<Greeter, Greeting> = Operation {
    name: "greeting",
    description: "Greets one person by name.",
    permission: GREET,
    params: &[Param::required("name", "Who to greet: 1 to 40 characters.")],
    effect: Effect::Read,
    run: Run::Sync(greeting),
};

// 5. Exposure: one JSON route and one MCP tool, both listed by hand.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = Secret::new(std::env::var("HELLO_TOKEN")?)?;
    let greeter = Greeter { greeting: "Hello" };

    if std::env::args().nth(1).as_deref() == Some("mcp") {
        // MCP over stdio: every call runs as GUEST.
        let server = McpServer::with_tools(greeter, GUEST, vec![mcp::tool(&GREETING)])?;
        return Ok(server.with_info("hello", "1.0.0").serve_stdio().await?);
    }

    // HTTP: `router` is an ordinary `axum::Router`.
    let state = AppState::builder(greeter)
        .api_auth(Bearer::new(token, GUEST))
        .build();
    let router = Endpoints::new()
        .api("/api/greeting", &GREETING)
        .into_router(state)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

Run it and call it:

```sh manual
HELLO_TOKEN=choose-a-long-random-value cargo run --example hello
```

```sh manual
curl -H 'Authorization: Bearer choose-a-long-random-value' 'http://127.0.0.1:3000/api/greeting?name=Ada'
```

```text
{"message":"Hello, Ada!"}
```

Without the token the answer is `401` with `{"error":"unauthorized"}`; with an empty name it is `400` with `{"error":"invalid_name"}`; with a misspelt parameter it is `400` with `{"error":"invalid_request"}`.

## How do you add Senu to a project?

```sh manual
cargo add senu --no-default-features
cargo add axum tokio serde --features tokio/macros,tokio/rt,tokio/net,serde/derive
```

To build from the Git tag instead of crates.io, use this in place of the first line:

```sh manual
cargo add senu --git https://github.com/efg-technologies/senu --tag v0.3.0 --no-default-features
```

`--no-default-features` leaves out the `demo` feature, which holds the sample catalogue, the `senu` binary and this site. Add `askama` when you render pages, and enable Senu's `contract` feature under `[dev-dependencies]` for [the contract tests](/testing).

## When should you use Senu, and when not?

Use Senu when the same data and the same actions must reach people, programs and assistants, and you want one place for authorization: internal dashboards, catalogues, status and reporting services, and tools that let an assistant look things up or make a change that a person could make.

Do not use Senu as your whole web framework. It has no forms, no session storage, no login pages and no database layer. A Senu router is an ordinary `axum::Router`, so the usual pattern is to merge it into an Axum application that does the rest.

## What are the limits of version 0.3?

- A read is served by `GET` and a write by `POST` with a JSON body. There are no HTML form posts and no file uploads.
- A read's parameters are strings. A write's may also be numbers and booleans, but not lists or nested objects.
- Paths are exact: there are no path parameters.
- MCP runs over stdio. MCP over HTTP is not included.
- The built-in authenticators hold static secrets. Sessions are yours to store; Senu reads the cookie and can send people to your login page. OAuth and key rotation are yours to write, behind one trait.
- Senu is not audited security software, and the API may change between 0.x versions.

## Where next?

- [Declare an operation](/operations): parameters, errors, asynchronous work and pages.
- [Declare a write operation](/writes): typed arguments, the cross-site check and 409.
- [Authenticate callers](/authentication): the built-in authenticators and your own.
- [Expose operations as MCP tools](/mcp-tools).
- [Deploy and operate](/deployment): what Senu does and what stays with you.
- [Test with the contract](/testing).
- [Questions and comparisons](/faq).
- New to Rust? [Start the course](/start).

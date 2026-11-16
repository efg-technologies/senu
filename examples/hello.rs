//! The smallest complete Senu application: one operation, served as JSON
//! behind a Bearer token and listed as an MCP tool.
//!
//! ```sh
//! HELLO_TOKEN=choose-a-long-random-value cargo run --example hello
//! curl -H 'Authorization: Bearer choose-a-long-random-value' \
//!   'http://127.0.0.1:3000/api/greeting?name=Ada'
//! HELLO_TOKEN=choose-a-long-random-value cargo run --example hello -- mcp
//! ```

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

//! Process entry point: arguments, configuration, then one of the modes.
//!
//! Stdout is reserved for `--help` and, in MCP mode, JSON-RPC. Stderr gets
//! the listening line and fatal errors only. Exit status 2 means bad
//! arguments or configuration, 1 means a runtime failure such as a port that
//! is already in use.

use std::ffi::OsString;
use std::process::ExitCode;

use crate::catalogue::Catalogue;
use crate::cli::{self, Command};
use crate::config::{self, ServeConfig, SiteConfig};
use crate::demo;
use crate::http::{self, AppState, Endpoints};
use crate::mcp::{McpServer, McpTool};
use crate::operation::Principal;
use crate::site::Site;

const USAGE_OR_CONFIG: u8 = 2;
const RUNTIME_FAILURE: u8 = 1;

/// Runs the binary with explicit arguments (without the program name) and
/// environment.
pub fn run<I>(args: &[OsString], vars: I) -> ExitCode
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let command = match cli::parse_args(args) {
        Ok(command) => command,
        Err(err) => return fail(USAGE_OR_CONFIG, &err),
    };
    match command {
        Command::Help => {
            print!("{}", cli::HELP);
            ExitCode::SUCCESS
        }
        Command::Serve => match config::serve_config(vars) {
            Ok(config) => {
                let state = AppState::new(Catalogue::demo(), &config);
                serve(&config, state, demo::endpoints())
            }
            Err(err) => fail(USAGE_OR_CONFIG, &err),
        },
        Command::Site => match config::site_config(vars) {
            Ok(config) => site(&config, crate::content::EMBEDDED),
            Err(err) => fail(USAGE_OR_CONFIG, &err),
        },
        Command::Mcp => match config::mcp_config(vars) {
            // The token is validated and never needed again: stdio trusts
            // the local process that set the environment.
            Ok(_config) => mcp(Catalogue::demo(), Principal::demo_mcp(), demo::tools()),
            Err(err) => fail(USAGE_OR_CONFIG, &err),
        },
    }
}

fn fail(code: u8, message: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(code)
}

/// A single-threaded runtime is plenty for a local demo and keeps the
/// process small.
fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

/// Runs `senu serve` with `endpoints`, the demo's table in production. The
/// table is checked before anything is bound, so an invalid one exits with
/// status 2 instead of panicking.
pub fn serve<S: Send + Sync + 'static>(
    config: &ServeConfig,
    state: AppState<S>,
    endpoints: Endpoints<S>,
) -> ExitCode {
    match endpoints.into_router(state) {
        Ok(app) => listen(config.bind, app),
        Err(err) => fail(USAGE_OR_CONFIG, &err),
    }
}

/// Runs `senu site` over `files`, the embedded articles in production. The
/// articles are checked before anything is bound, so bad content exits with
/// status 2. All names are checked first, then each article's content; errors
/// follow path order within each phase and are written to stderr.
pub fn site(config: &SiteConfig, files: &[(&str, &str)]) -> ExitCode {
    match Site::load(files) {
        Ok(site) => listen(config.bind, http::site_router(site)),
        Err(err) => fail(USAGE_OR_CONFIG, &err),
    }
}

fn listen(bind: std::net::SocketAddr, app: axum::Router) -> ExitCode {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(_) => return fail(RUNTIME_FAILURE, &"senu: could not start the runtime"),
    };
    runtime.block_on(async {
        let listener = match tokio::net::TcpListener::bind(bind).await {
            Ok(listener) => listener,
            Err(err) => {
                let message = format!("senu: cannot listen on {}: {}", bind, err.kind());
                return fail(RUNTIME_FAILURE, &message);
            }
        };
        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(_) => return fail(RUNTIME_FAILURE, &"senu: cannot read the listening address"),
        };
        eprintln!("senu: listening on http://{addr}");
        match axum::serve(listener, app).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => fail(
                RUNTIME_FAILURE,
                &"senu: the HTTP server stopped unexpectedly",
            ),
        }
    })
}

/// Serves `tools` over stdio until stdin reaches EOF, every call bound to
/// `principal`. The list is checked before stdin is read, so an invalid one
/// exits with status 2. Stdout carries only JSON-RPC; nothing is logged.
pub fn mcp<S: Send + Sync + 'static>(
    state: S,
    principal: Principal,
    tools: Vec<McpTool<S>>,
) -> ExitCode {
    use rmcp::ServiceExt as _;
    use rmcp::service::{QuitReason, ServerInitializeError};

    let server = match McpServer::with_tools(state, principal, tools) {
        Ok(server) => server,
        Err(err) => return fail(USAGE_OR_CONFIG, &err),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(_) => return fail(RUNTIME_FAILURE, &"senu: could not start the runtime"),
    };
    runtime.block_on(async {
        let running = match server.serve(rmcp::transport::io::stdio()).await {
            Ok(running) => running,
            // EOF before `initialize` is a clean shutdown, not an error.
            Err(ServerInitializeError::ConnectionClosed(_)) => return ExitCode::SUCCESS,
            Err(_) => return fail(RUNTIME_FAILURE, &"senu: MCP initialization failed"),
        };
        match running.waiting().await {
            Ok(QuitReason::Closed | QuitReason::Cancelled) => ExitCode::SUCCESS,
            _ => fail(
                RUNTIME_FAILURE,
                &"senu: the MCP server stopped unexpectedly",
            ),
        }
    })
}

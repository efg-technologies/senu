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
use crate::config::{self, ServeConfig};
use crate::http::{self, AppState};
use crate::mcp::McpServer;

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
            Ok(config) => serve(config),
            Err(err) => fail(USAGE_OR_CONFIG, &err),
        },
        Command::Mcp => match config::mcp_config(vars) {
            // The token is validated and never needed again: stdio trusts
            // the local process that set the environment.
            Ok(_config) => mcp(),
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

fn serve(config: ServeConfig) -> ExitCode {
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(_) => return fail(RUNTIME_FAILURE, &"senu: could not start the runtime"),
    };
    runtime.block_on(async {
        let listener = match tokio::net::TcpListener::bind(config.bind).await {
            Ok(listener) => listener,
            Err(err) => {
                let message = format!("senu: cannot listen on {}: {}", config.bind, err.kind());
                return fail(RUNTIME_FAILURE, &message);
            }
        };
        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(_) => return fail(RUNTIME_FAILURE, &"senu: cannot read the listening address"),
        };
        let app = http::router(AppState::new(Catalogue::demo(), &config));
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

/// Serves MCP over stdio until stdin reaches EOF. Stdout carries only
/// JSON-RPC; nothing is logged.
fn mcp() -> ExitCode {
    use rmcp::ServiceExt as _;
    use rmcp::service::{QuitReason, ServerInitializeError};

    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(_) => return fail(RUNTIME_FAILURE, &"senu: could not start the runtime"),
    };
    runtime.block_on(async {
        let server = McpServer::new(Catalogue::demo());
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

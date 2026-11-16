//! The MCP adapter: the application's explicit tool list over stdio.
//!
//! Under the MCP authorization spec a stdio server takes its credentials from
//! the environment. The application validates whatever it needs there
//! before it starts the server (the demo's `senu mcp --enable-mcp` checks
//! `SENU_MCP_TOKEN`) and then binds every call to one server-side
//! principal. That trusts the local process that launched the server and
//! its environment; it does not authenticate any remote party.
//!
//! Each tool is an operation that the application lists with [`tool`]; its
//! JSON Schema is built from the operation's parameters. Nothing is exposed
//! automatically, and no tool macros or reflection are used.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CustomRequest, CustomResult,
    ErrorCode, Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde::Serialize;
use serde_json::{Map, Value, json};

#[cfg(feature = "demo")]
use crate::catalogue::Catalogue;
#[cfg(feature = "demo")]
use crate::demo::{get_project, list_projects};
use crate::operation::{Args, Effect, InvalidDeclaration, OpError, Operation, Param, Principal};

/// The name of the `list_projects` tool.
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
pub const TOOL_NAME: &str = list_projects::OPERATION.name;

/// The name of the `get_project` tool.
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
pub const GET_PROJECT_TOOL: &str = get_project::OPERATION.name;

/// A tool's outcome: the operation's result already as JSON, so that one
/// list can hold operations with different result types.
type Outcome = Result<serde_json::Result<Value>, OpError>;

/// What a listed tool runs.
type Call<S> = Box<
    dyn for<'a> Fn(
            &'a S,
            &'a Principal,
            &'a Args,
        ) -> Pin<Box<dyn Future<Output = Outcome> + Send + 'a>>
        + Send
        + Sync,
>;

/// One listed tool: its schema and its operation.
pub struct McpTool<S> {
    name: &'static str,
    effect: Effect,
    params: &'static [Param],
    schema: Tool,
    invalid: Option<InvalidDeclaration>,
    call: Call<S>,
}

impl<S> McpTool<S> {
    /// The tool's name, which is its operation's.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The operation's effect, which decides the tool's annotations.
    pub fn effect(&self) -> Effect {
        self.effect
    }

    /// The operation's parameters, which the tool's schema describes.
    pub fn params(&self) -> &'static [Param] {
        self.params
    }
}

/// Lists `operation` as a tool of the same name. A read is annotated
/// `readOnlyHint: true`. A write is annotated `readOnlyHint: false` with
/// its declared `destructiveHint` and `idempotentHint`, so a client can ask
/// its user before it calls one.
pub fn tool<S, T>(operation: &'static Operation<S, T>) -> McpTool<S>
where
    S: Sync + 'static,
    T: Serialize + Send + 'static,
{
    McpTool {
        name: operation.name,
        effect: operation.effect,
        params: operation.params,
        schema: Tool::new(
            operation.name,
            operation.description,
            Arc::new(input_schema(operation.params)),
        )
        .with_annotations(match operation.effect {
            Effect::Read => ToolAnnotations::new().read_only(true),
            Effect::Write {
                destructive,
                idempotent,
            } => ToolAnnotations::new()
                .read_only(false)
                .destructive(destructive)
                .idempotent(idempotent),
        }),
        invalid: operation.check().err(),
        call: Box::new(move |state, principal, args| {
            Box::pin(async move {
                operation
                    .call(state, principal, args)
                    .await
                    .map(|result| serde_json::to_value(&result))
            })
        }),
    }
}

/// Every parameter is a described scalar of its kind; `required` appears
/// only when some are required, and no other property is allowed.
fn input_schema(params: &[Param]) -> Map<String, Value> {
    let properties: Map<String, Value> = params
        .iter()
        .map(|param| {
            let property = json!({
                "type": param.kind.json_type(),
                "description": param.description,
            });
            (param.name.to_owned(), property)
        })
        .collect();
    let required: Vec<&str> = params
        .iter()
        .filter(|param| param.required)
        .map(|param| param.name)
        .collect();
    let mut schema = Map::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_owned(), json!(required));
    }
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema
}

/// A tool list that Senu refuses. `senu mcp` prints it and exits with
/// status 2 before reading stdin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolError {
    /// The tool is listed twice.
    Duplicate(&'static str),
    /// The tool's operation has invalid metadata.
    Operation(InvalidDeclaration),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(name) => write!(f, "senu: tool {name:?} is listed twice"),
            Self::Operation(invalid) => write!(f, "senu: {invalid}"),
        }
    }
}

impl std::error::Error for ToolError {}

/// The MCP server state: the application's state, the principal bound at
/// startup and the listed tools.
pub struct McpServer<S> {
    state: S,
    principal: Principal,
    tools: Vec<McpTool<S>>,
    name: &'static str,
    version: &'static str,
}

/// Why [`McpServer::serve_stdio`] stopped with an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServeError {
    /// The `initialize` exchange failed.
    Initialize,
    /// The server stopped for a reason other than the client closing the
    /// connection.
    Stopped,
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Initialize => "senu: MCP initialization failed",
            Self::Stopped => "senu: the MCP server stopped unexpectedly",
        })
    }
}

impl std::error::Error for ServeError {}

#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
impl McpServer<Catalogue> {
    /// The demo server, bound to `demo-mcp`.
    pub fn new(catalogue: Catalogue) -> Self {
        Self::with_principal(catalogue, Principal::demo_mcp())
    }

    /// Binds another principal to the demo's tools. Tests use this to inject
    /// a principal without permissions or in another tenant. The demo's list
    /// is valid, which the tests check, so it is not checked again here.
    pub fn with_principal(catalogue: Catalogue, principal: Principal) -> Self {
        Self {
            state: catalogue,
            principal,
            tools: crate::demo::tools(),
            name: "senu",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

impl<S> McpServer<S> {
    /// Serves exactly `tools`, after checking that their names are unique
    /// and their operations' metadata is valid.
    pub fn with_tools(
        state: S,
        principal: Principal,
        tools: Vec<McpTool<S>>,
    ) -> Result<Self, ToolError> {
        for (i, tool) in tools.iter().enumerate() {
            if let Some(invalid) = tool.invalid {
                return Err(ToolError::Operation(invalid));
            }
            if tools[..i].iter().any(|other| other.name == tool.name) {
                return Err(ToolError::Duplicate(tool.name));
            }
        }
        Ok(Self {
            state,
            principal,
            tools,
            name: "senu",
            version: env!("CARGO_PKG_VERSION"),
        })
    }

    /// Names the server in `initialize`, for example with
    /// `env!("CARGO_PKG_NAME")` and `env!("CARGO_PKG_VERSION")` of the
    /// application. Without this the server is `senu` with Senu's version.
    pub fn with_info(mut self, name: &'static str, version: &'static str) -> Self {
        self.name = name;
        self.version = version;
        self
    }

    /// What `initialize` advertises: the tools capability only.
    pub fn info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(self.name, self.version))
    }

    /// What `tools/list` returns: the listed tools, in order.
    pub fn tools(&self) -> Vec<Tool> {
        self.tools.iter().map(|tool| tool.schema.clone()).collect()
    }

    /// Answers `tools/call`. An unknown tool is a JSON-RPC invalid-params
    /// error (-32602); an operation error is a tool result with `isError`.
    /// Neither repeats the caller's input.
    pub async fn call(&self, request: &CallToolRequestParams) -> Result<CallToolResult, ErrorData> {
        let Some(tool) = self.tools.iter().find(|tool| tool.name == request.name) else {
            return Err(ErrorData::invalid_params("unknown tool", None));
        };
        let no_arguments = Map::new();
        let args = request.arguments.as_ref().unwrap_or(&no_arguments);
        match (tool.call)(&self.state, &self.principal, args).await {
            Ok(value) => value
                .map(CallToolResult::structured)
                .map_err(|_| ErrorData::internal_error("internal", None)),
            Err(err) => Ok(CallToolResult::structured_error(
                json!({ "error": err.code() }),
            )),
        }
    }
}

impl<S: Send + Sync + 'static> McpServer<S> {
    /// Serves the tools over this process's stdin and stdout until the
    /// client closes stdin, which is `Ok(())`, also before `initialize`.
    /// Stdout then carries only JSON-RPC, so the application must not print
    /// to it. Call this on a Tokio runtime.
    pub async fn serve_stdio(self) -> Result<(), ServeError> {
        use rmcp::ServiceExt as _;
        use rmcp::service::{QuitReason, ServerInitializeError};

        let running = match self.serve(rmcp::transport::io::stdio()).await {
            Ok(running) => running,
            // EOF before `initialize` is a clean shutdown, not an error.
            Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(_) => return Err(ServeError::Initialize),
        };
        match running.waiting().await {
            Ok(QuitReason::Closed | QuitReason::Cancelled) => Ok(()),
            _ => Err(ServeError::Stopped),
        }
    }
}

/// The demo's tools, in this order.
#[cfg(feature = "demo")]
#[cfg_attr(docsrs, doc(cfg(feature = "demo")))]
pub fn tools() -> Vec<Tool> {
    McpServer::new(Catalogue::demo()).tools()
}

impl<S: Send + Sync + 'static> ServerHandler for McpServer<S> {
    fn get_info(&self) -> ServerConfig {
        self.info()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.call(&request).await.map(CallToolResponse::from)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools
            .iter()
            .find(|tool| tool.name == name)
            .map(|tool| tool.schema.clone())
    }

    /// Answers any method rmcp does not route. rmcp's default puts the
    /// requested method name in the error message; this reply is fixed and
    /// repeats neither the method nor its params.
    async fn on_custom_request(
        &self,
        _request: CustomRequest,
        _context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "method not found",
            None,
        ))
    }
}

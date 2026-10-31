//! The MCP adapter: two hand-written tools, `list_projects` and
//! `get_project`, over stdio.
//!
//! Under the MCP authorization spec a stdio server takes its credentials from
//! the environment. `senu mcp --enable-mcp` validates `SENU_MCP_TOKEN` (see
//! [`crate::config::mcp_config`]) before reading stdin and then binds every
//! call to the server-side `demo-mcp` principal. That trusts the local
//! process that launched Senu and its environment; it does not authenticate
//! any remote party.
//!
//! The tools are declared by hand below. Nothing is exposed automatically, and
//! no tool macros or reflection are used.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CustomRequest, CustomResult,
    ErrorCode, Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig, Tool, ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::{Map, Value, json};

use crate::catalogue::Catalogue;
use crate::operation::{MAX_ID_BYTES, MAX_SEARCH_CHARS, Principal, get_project, list_projects};

/// The name of the `list_projects` tool.
pub const TOOL_NAME: &str = "list_projects";

/// The name of the `get_project` tool.
pub const GET_PROJECT_TOOL: &str = "get_project";

/// The MCP server state: the catalogue and the principal bound at startup.
pub struct McpServer {
    catalogue: Catalogue,
    principal: Principal,
}

impl McpServer {
    /// The demo server, bound to `demo-mcp`.
    pub fn new(catalogue: Catalogue) -> Self {
        Self::with_principal(catalogue, Principal::demo_mcp())
    }

    /// Binds another principal. Tests use this to inject a principal without
    /// permissions or in another tenant.
    pub fn with_principal(catalogue: Catalogue, principal: Principal) -> Self {
        Self {
            catalogue,
            principal,
        }
    }

    /// What `initialize` advertises: the tools capability only.
    pub fn info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("senu", env!("CARGO_PKG_VERSION")))
    }

    /// Answers `tools/call`. An unknown tool is a JSON-RPC invalid-params
    /// error (-32602); an operation error is a tool result with `isError`.
    /// Neither repeats the caller's input.
    pub fn call(&self, request: &CallToolRequestParams) -> Result<CallToolResult, ErrorData> {
        let no_arguments = Map::new();
        let args = request.arguments.as_ref().unwrap_or(&no_arguments);
        let result = match request.name.as_ref() {
            TOOL_NAME => list_projects(&self.catalogue, &self.principal, args)
                .map(|list| serde_json::to_value(&list)),
            GET_PROJECT_TOOL => get_project(&self.catalogue, &self.principal, args)
                .map(|project| serde_json::to_value(&project)),
            _ => return Err(ErrorData::invalid_params("unknown tool", None)),
        };
        match result {
            Ok(value) => value
                .map(CallToolResult::structured)
                .map_err(|_| ErrorData::internal_error("internal", None)),
            Err(err) => Ok(CallToolResult::structured_error(
                json!({ "error": err.code() }),
            )),
        }
    }
}

/// The declared tools, in this order.
pub fn tools() -> Vec<Tool> {
    vec![list_projects_tool(), get_project_tool()]
}

/// The schema declares no `maxLength`: JSON Schema counts the raw string,
/// while the operation trims first, so a schema limit would reject padded
/// searches the server accepts. The description states the real rule.
fn list_projects_tool() -> Tool {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert(
        "properties".to_owned(),
        json!({
            "q": {
                "type": "string",
                "description": format!(
                    "Case-insensitive name search. Surrounding whitespace is trimmed first; \
                     the trimmed value may hold at most {MAX_SEARCH_CHARS} Unicode scalar values \
                     and no control characters. Empty means no search."
                )
            }
        }),
    );
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    Tool::new(
        TOOL_NAME,
        "List the demo projects visible to this server's principal, sorted by id.",
        Arc::new(schema),
    )
    .with_annotations(ToolAnnotations::new().read_only(true))
}

/// `id` is required and matched exactly, without trimming.
fn get_project_tool() -> Tool {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), json!("object"));
    schema.insert(
        "properties".to_owned(),
        json!({
            "id": {
                "type": "string",
                "description": format!(
                    "The project id: 1 to {MAX_ID_BYTES} characters of a-z, 0-9 and '-'."
                )
            }
        }),
    );
    schema.insert("required".to_owned(), json!(["id"]));
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    Tool::new(
        GET_PROJECT_TOOL,
        "Get one demo project visible to this server's principal by its id.",
        Arc::new(schema),
    )
    .with_annotations(ToolAnnotations::new().read_only(true))
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        self.info()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.call(&request).map(CallToolResponse::from)
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|tool| tool.name == name)
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

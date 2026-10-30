//! Adapter tests for the MCP handler, called in-process without a transport.

use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode};
use senu::catalogue::{Catalogue, TenantId};
use senu::mcp::{McpServer, TOOL_NAME, tools};
use senu::operation::Principal;
use serde_json::{Map, Value, json};

fn server() -> McpServer {
    McpServer::new(Catalogue::demo())
}

fn arguments(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("arguments must be an object, got {other}"),
    }
}

fn call(server: &McpServer, args: Value) -> CallToolResult {
    let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(arguments(args));
    server.call(&request).expect("a tool result")
}

/// The JSON text content, which must match `structuredContent`.
fn text_json(result: &CallToolResult) -> Value {
    let wire = serde_json::to_value(result).unwrap();
    let content = wire["content"].as_array().expect("content array");
    assert_eq!(content.len(), 1, "{wire}");
    assert_eq!(content[0]["type"], "text", "{wire}");
    serde_json::from_str(content[0]["text"].as_str().unwrap()).expect("JSON text")
}

fn assert_tool_error(result: &CallToolResult, code: &str) {
    assert_eq!(result.is_error, Some(true));
    let expected = json!({ "error": code });
    assert_eq!(result.structured_content.as_ref(), Some(&expected));
    assert_eq!(text_json(result), expected);
}

fn ids(value: &Value) -> Vec<&str> {
    value["projects"]
        .as_array()
        .expect("projects array")
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect()
}

#[test]
fn exactly_one_explicit_read_only_tool_is_declared() {
    let tools = tools();
    assert_eq!(tools.len(), 1);
    let tool = serde_json::to_value(&tools[0]).unwrap();
    assert_eq!(tool["name"], TOOL_NAME);
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    let schema = &tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    let properties = schema["properties"].as_object().expect("properties");
    assert_eq!(properties.keys().collect::<Vec<_>>(), ["q"]);
    assert_eq!(properties["q"]["type"], "string");
    assert_eq!(properties["q"]["maxLength"], 80);
    assert!(tool["inputSchema"].get("required").is_none());
}

#[test]
fn server_advertises_only_the_tools_capability() {
    let info = serde_json::to_value(server().info()).unwrap();
    let capabilities = info["capabilities"].as_object().expect("capabilities");
    assert_eq!(capabilities.keys().collect::<Vec<_>>(), ["tools"], "{info}");
    assert_eq!(info["serverInfo"]["name"], "senu");
}

#[test]
fn successful_call_returns_structured_content_and_the_same_json_text() {
    let result = call(&server(), json!({ "q": "launch" }));
    assert_eq!(result.is_error, Some(false));
    let structured = result
        .structured_content
        .clone()
        .expect("structuredContent");
    assert_eq!(ids(&structured), ["acme-001", "acme-003"]);
    assert_eq!(text_json(&result), structured);
    assert_eq!(
        structured.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["projects"]
    );
}

#[test]
fn missing_arguments_mean_no_search() {
    let request = CallToolRequestParams::new(TOOL_NAME);
    let result = server().call(&request).expect("a tool result");
    let structured = result.structured_content.expect("structuredContent");
    assert_eq!(ids(&structured), ["acme-001", "acme-002", "acme-003"]);
}

#[test]
fn principal_without_permission_is_forbidden_before_validation() {
    let nobody = McpServer::with_principal(
        Catalogue::demo(),
        Principal::new("nobody", TenantId::ACME, &[]),
    );
    assert_tool_error(&call(&nobody, json!({ "q": "launch" })), "forbidden");
    assert_tool_error(
        &call(&nobody, json!({ "tenant": "globex", "q": "a".repeat(81) })),
        "forbidden",
    );
}

#[test]
fn invalid_input_and_unknown_fields_are_tool_errors() {
    let server = server();
    assert_tool_error(
        &call(&server, json!({ "q": "a".repeat(81) })),
        "invalid_search",
    );
    assert_tool_error(&call(&server, json!({ "q": "a\u{7}b" })), "invalid_search");
    for args in [
        json!({ "tenant": "globex" }),
        json!({ "permission": "ListProjects" }),
        json!({ "token": "x" }),
        json!({ "q": 7 }),
        json!({ "q": ["launch"] }),
    ] {
        assert_tool_error(&call(&server, args), "invalid_request");
    }
}

#[test]
fn principal_tenant_comes_from_the_server_not_the_arguments() {
    let globex = McpServer::with_principal(
        Catalogue::demo(),
        Principal::new(
            "globex-mcp",
            TenantId::GLOBEX,
            &[senu::operation::Permission::ListProjects],
        ),
    );
    let structured = call(&globex, json!({ "q": "launch" }))
        .structured_content
        .unwrap();
    assert_eq!(ids(&structured), ["globex-001"]);
}

#[test]
fn unknown_tool_is_a_jsonrpc_invalid_params_error() {
    let request = CallToolRequestParams::new("delete_projects").with_arguments(Map::new());
    let error = server().call(&request).expect_err("unknown tool");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert_eq!(error.code.0, -32602);
    assert!(
        !error.message.contains("delete_projects"),
        "{}",
        error.message
    );
}

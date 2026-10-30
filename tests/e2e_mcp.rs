//! End-to-end tests of `senu mcp --enable-mcp` over real stdio, using the
//! real binary, a cleared environment, newline-delimited JSON-RPC and 10 s
//! timeouts.

mod support;

use rmcp::model::ProtocolVersion;
use serde_json::{Value, json};
use support::process::{self, Running};
use support::*;

fn mcp_vars() -> Vec<(&'static str, &'static str)> {
    vec![("SENU_MCP_TOKEN", MCP_TOKEN)]
}

fn send(server: &mut Running, message: Value) {
    server.write_line(&message.to_string());
}

/// Reads the next stdout line and checks it answers request `id`.
fn reply(server: &mut Running, id: u64) -> Value {
    let line = server.stdout_line();
    let message: Value = serde_json::from_str(&line).expect("a JSON-RPC line");
    assert_eq!(message["jsonrpc"], "2.0", "{line}");
    assert_eq!(message["id"], id, "{line}");
    message
}

fn call_tool(server: &mut Running, id: u64, name: &str, arguments: Value) -> Value {
    send(
        server,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }),
    );
    reply(server, id)
}

fn assert_tool_error(message: &Value, code: &str) {
    let result = &message["result"];
    assert_eq!(result["isError"], true, "{message}");
    assert_eq!(
        result["structuredContent"],
        json!({ "error": code }),
        "{message}"
    );
}

#[test]
fn stdio_session_lists_and_calls_the_one_tool_then_exits_on_eof() {
    let mut server = process::spawn(process::senu(&["mcp", "--enable-mcp"], &mcp_vars()));

    send(
        &mut server,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": ProtocolVersion::LATEST,
                "capabilities": {},
                "clientInfo": { "name": "senu-e2e", "version": "0.0.0" }
            }
        }),
    );
    let init = reply(&mut server, 1);
    // rmcp 3.8.0's LATEST revision has no `initialize` handshake, so the
    // server answers with the newest revision that still has one.
    assert_eq!(
        init["result"]["protocolVersion"],
        json!(ProtocolVersion::LATEST_WITH_INITIALIZE),
        "{init}"
    );
    let capabilities = init["result"]["capabilities"].as_object().unwrap();
    assert_eq!(capabilities.keys().collect::<Vec<_>>(), ["tools"], "{init}");
    send(
        &mut server,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );

    send(
        &mut server,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    );
    let list = reply(&mut server, 2);
    let tools = list["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 1, "{list}");
    assert_eq!(tools[0]["name"], "list_projects");
    assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);

    let found = call_tool(&mut server, 3, "list_projects", json!({ "q": "launch" }));
    assert_eq!(found["result"]["isError"], false, "{found}");
    let ids: Vec<&str> = found["result"]["structuredContent"]["projects"]
        .as_array()
        .expect("projects")
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["acme-001", "acme-003"]);

    let long = call_tool(
        &mut server,
        4,
        "list_projects",
        json!({ "q": "a".repeat(81) }),
    );
    assert_tool_error(&long, "invalid_search");
    let tenant = call_tool(
        &mut server,
        5,
        "list_projects",
        json!({ "tenant": "globex" }),
    );
    assert_tool_error(&tenant, "invalid_request");

    let unknown = call_tool(&mut server, 6, "delete_projects", json!({}));
    assert_eq!(unknown["error"]["code"], -32602, "{unknown}");
    assert!(unknown.get("result").is_none(), "{unknown}");

    server.close_stdin();
    let (status, stdout, stderr) = server.wait();
    assert_eq!(status.code(), Some(0), "{stderr:?}");
    assert_eq!(stdout.len(), 6, "one reply per request: {stdout:?}");
    for line in &stdout {
        let message: Value = serde_json::from_str(line).expect("only JSON-RPC on stdout");
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
    }
    assert!(stderr.is_empty(), "{stderr:?}");
    let transcript = format!("{}\n{}", stdout.join("\n"), stderr.join("\n"));
    for secret in SECRETS {
        assert!(!transcript.contains(secret), "output contains a secret");
    }
}

#[test]
fn stdio_exits_cleanly_on_eof_and_refuses_to_start_without_its_token() {
    let (status, stdout, stderr) =
        process::run(process::senu(&["mcp", "--enable-mcp"], &mcp_vars()));
    assert_eq!(status.code(), Some(0), "{stderr:?}");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(stderr.is_empty(), "{stderr:?}");

    let (status, stdout, stderr) = process::run(process::senu(&["mcp", "--enable-mcp"], &[]));
    assert_eq!(status.code(), Some(2), "{stderr:?}");
    assert!(
        stdout.is_empty(),
        "nothing on stdout without a token: {stdout:?}"
    );
    assert!(stderr.join("\n").contains("SENU_MCP_TOKEN"), "{stderr:?}");
}

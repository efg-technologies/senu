# Expose operations as MCP tools

An operation becomes an MCP tool when you list it with `mcp::tool`. AI assistants then call the same function, behind the same permission check, that your JSON API and pages use.

## What is MCP, and why serve it from the same function?

MCP, the Model Context Protocol, is how AI assistants such as Claude call tools in other programs. A tool has a name, a description and a JSON Schema for its arguments. If the tool is a second implementation next to your API, the two drift: one validates differently, or forgets the tenant check. In Senu the tool and the API route are the same `Operation`, so they cannot disagree, and [the contract tests](/testing) compare their answers.

## How do you list tools?

```rust excerpt=examples/inventory.rs
pub fn tools() -> Vec<McpTool<Inventory>> {
    vec![mcp::tool(&GET_STOCK), mcp::tool(&LIST_LOW_STOCK)]
}
```

Each tool takes its name and description from the operation. Its input schema is built from the declared parameters: every property is a string with its description, `required` lists the required ones, and `additionalProperties` is `false`. Every tool is marked `readOnlyHint: true`, because operations are read-only. Nothing is exposed automatically, and no macros or reflection are involved.

## How do you run the server?

```rust excerpt=examples/inventory.rs
async fn serve_mcp() -> Result<(), String> {
    // Stdio trusts the local process that starts the server, so every call
    // is bound to one principal chosen here.
    McpServer::with_tools(Inventory::sample(), NORTH_CLERK, tools())
        .map_err(|err| err.to_string())?
        .with_info("inventory", "1.0.0")
        .serve_stdio()
        .await
        .map_err(|err| err.to_string())
}
```

`serve_stdio` speaks MCP over the process's stdin and stdout until the client closes stdin. Stdout then carries only JSON-RPC, so log to stderr. `with_info` sets the name and version that `initialize` reports.

Every call runs as the one principal you pass in. Under the MCP specification, a stdio server takes its credentials from its environment: it trusts the local process that starts it, and it authenticates no remote party. Choose that principal with care, and give it only the permissions an assistant should have.

## How do you connect an assistant?

Most MCP clients read a JSON file that names the command to start. For the example above:

```json
{
  "mcpServers": {
    "inventory": {
      "command": "/absolute/path/to/inventory",
      "args": ["mcp"]
    }
  }
}
```

The client starts the program, sends `initialize`, lists the tools and calls them. A call and its answer look like this on the wire:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_low_stock","arguments":{"below":"4"}}}
```

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"below\":4,\"items\":[{\"name\":\"Nut M8\",\"sku\":\"NUT-8\",\"units\":3},{\"name\":\"Washer M8\",\"sku\":\"WASHER-8\",\"units\":0}]}"}],"structuredContent":{"below":4,"items":[{"name":"Nut M8","sku":"NUT-8","units":3},{"name":"Washer M8","sku":"WASHER-8","units":0}]},"isError":false}}
```

The result arrives twice: as `structuredContent` for clients that read JSON, and as the same JSON in a text block for those that do not.

## What do errors look like?

An operation's error is a normal tool result with `isError: true` and `{"error": "<code>"}`, with the same codes as the JSON API: `forbidden`, `invalid_request`, `not_found`, `internal` or your own. The assistant can read it and correct its call. An unknown tool name is a JSON-RPC error, `-32602`, with a fixed message that does not repeat the name.

## Is MCP over HTTP supported?

Not in version 0.2. Tools run over stdio, which suits assistants that start local programs. For a remote assistant, put the JSON API in front of it, or wait for the HTTP transport in a later version; it needs its own specification, because a remote caller must be authenticated on every request.

## What makes a good tool for an assistant?

- Write the `description` for a reader that cannot ask questions: what the tool returns, about what, and its limits.
- Describe each parameter's format in its `description`, such as "1 to 16 characters of A-Z, 0-9 and '-'".
- Return small, flat results with stable field names.
- Use your own error codes for mistakes the assistant can fix, such as `invalid_sku`.

Next: [deploy and operate](/deployment).

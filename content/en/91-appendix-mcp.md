# Appendix: Senu over MCP (optional)

MCP, the Model Context Protocol, lets AI tools call functions in other programs. Senu can offer its project list as one MCP tool, `list_projects`. It does so over **stdio**: the AI tool starts Senu itself, and the two talk through Senu's input and output instead of a network port. This mode is off unless you ask for it.

Stop any running Senu with Ctrl+C. Without the extra flag, Senu refuses:

```sh check=off exit=2
cargo run --quiet -- mcp
```

```text check=off
senu: MCP is off by default; start it with `senu mcp --enable-mcp`
```

The script `scripts/check_mcp.py` plays the part of an AI tool: it starts Senu in MCP mode, asks for the tool list, calls `list_projects` and prints the answer. It needs Python 3. MCP mode also needs a token before it starts; here it is a demo value:

```sh check=mcp
SENU_MCP_TOKEN=demo-mcp-token python3 scripts/check_mcp.py ./target/debug/senu mcp --enable-mcp
```

```text check=mcp
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"projects\":[{\"id\":\"acme-001\",\"name\":\"Artemis Launch\"},{\"id\":\"acme-003\",\"name\":\"<script>alert('x')</script> & \\\"Launch\\\" Notes\"}]}"}],"structuredContent":{"projects":[{"id":"acme-001","name":"Artemis Launch"},{"id":"acme-003","name":"<script>alert('x')</script> & \"Launch\" Notes"}]},"isError":false}}
```

`./target/debug/senu` is the program that `cargo run` built. If you renamed the project in the previous appendix, the answer shows Artemis Launch: the web page, the JSON and MCP all read the same list through the same function.

## Exercise

Run the same command without `SENU_MCP_TOKEN=demo-mcp-token`. What happens, and why is that useful?

**Solution.** Senu stops before it reads any input and says that `SENU_MCP_TOKEN` is not set, so a tool that was not given the token cannot start it by accident. The check script then reports that Senu exited.

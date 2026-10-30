<nav class="senu-lang" aria-label="Language"><a href="ja/mcp.html" hreflang="ja" lang="ja">日本語</a></nav>

# Opt-in MCP over stdio

The [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) lets a
client, such as an AI assistant, discover and call tools. Senu declares one
tool, `list_projects`, in
[`src/mcp.rs`](https://github.com/efg-technologies/senu/blob/main/src/mcp.rs),
using the official [`rmcp`](https://rust.sdk.modelcontextprotocol.io/) SDK.
The client starts `senu` as a child process and exchanges JSON-RPC messages,
one per line, over its standard input and output. No network port is
involved, and `senu serve` does not need to be running.

## Off unless you enable it

Running `senu mcp` without the flag refuses to start:

```sh
{{#include ../commands/mcp.sh:off}}
```

```text
{{#include ../commands/mcp.out:off}}
```

With `--enable-mcp`, Senu also needs `SENU_MCP_TOKEN`, a token separate from
the HTTP credentials. The MCP authorization rules have stdio servers take
credentials from the environment, so this trusts the local process that
starts Senu. It does not authenticate anyone remote; there is no MCP over
HTTP and no OAuth.

## Call the tool and check the reply

The quickstart's checker is a small client that uses only the Python standard
library. Run it on its own:

```sh
{{#include ../commands/mcp.sh:check}}
```

```text
{{#include ../commands/mcp.out:check}}
```

[`scripts/check_mcp.py`](https://github.com/efg-technologies/senu/blob/main/scripts/check_mcp.py)
sends `initialize` and waits for its reply. Only then does it send
`notifications/initialized` and a `tools/call` with `{"q": "launch"}`. Each
reply must carry its request's id and no error, the call must not be an
`isError` result, and it must list exactly `acme-001` and `acme-003`. Then it
closes stdin, and Senu must exit with status 0. On any failure the checker
stops Senu and exits with status 1.

The reply carries the same `ProjectList` twice: as `structuredContent` for
programs, and as JSON text in `content` for clients that only show text. It
lists the same projects, in the same order, as the HTML page and the API.

## What the tool declares

`tools/list` returns exactly one tool, with `readOnlyHint: true` and an input
schema that allows only an optional string `q`. An invalid search or an
unknown field comes back as an `isError` result with `{"error": code}`. An
unknown tool name gets JSON-RPC error `-32602`. The adapter tests in
[`tests/adapter_mcp.rs`](https://github.com/efg-technologies/senu/blob/main/tests/adapter_mcp.rs)
and the stdio test in
[`tests/e2e_mcp.rs`](https://github.com/efg-technologies/senu/blob/main/tests/e2e_mcp.rs)
cover these cases.

Other MCP clients that start stdio servers could run the same command with the
same environment variable, but this tutorial does not test any of them.

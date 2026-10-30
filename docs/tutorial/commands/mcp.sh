# ANCHOR: off
./target/debug/senu mcp; echo "exit status $?"
# ANCHOR_END: off
# ANCHOR: check
SENU_MCP_TOKEN=demo-mcp-token python3 scripts/check_mcp.py ./target/debug/senu mcp --enable-mcp
# ANCHOR_END: check

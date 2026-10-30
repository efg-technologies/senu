#!/usr/bin/env python3
"""The quickstart's MCP client: one stdio session, every reply checked.

Usage: check_mcp.py [--timeout SECONDS] COMMAND [ARG...]

Starts COMMAND and sends `initialize`, then waits for its reply. Only then
does it send `notifications/initialized` and a `tools/call` of
`list_projects` with q="launch", and wait for that reply. Each reply must be
the next stdout line: a JSON-RPC 2.0 response with the request's id and a
result, not an error. `initialize` must report a protocol version and the
tools capability. The call must not be an `isError` result and must list
exactly acme-001 and acme-003. Then stdin is closed, and COMMAND must exit 0
without writing anything else to stdout.

On success the call reply is printed. On any failure, or when a step takes
longer than the timeout (10 s by default), COMMAND is killed and reaped, the
reason goes to stderr and the exit status is 1. Standard library only.
"""

import json
import queue
import subprocess
import sys
import threading
import time

EXPECTED_IDS = ["acme-001", "acme-003"]
INITIALIZE = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2025-11-25",
        "capabilities": {},
        "clientInfo": {"name": "quickstart", "version": "0"},
    },
}
INITIALIZED = {"jsonrpc": "2.0", "method": "notifications/initialized"}
CALL = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {"name": "list_projects", "arguments": {"q": "launch"}},
}


class CheckFailed(Exception):
    pass


def pump(stream, lines):
    for line in stream:
        lines.put(line)
    lines.put(None)


def send(server, message):
    try:
        server.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
        server.stdin.flush()
    except OSError:
        raise CheckFailed("the server stopped reading its input") from None


def reply(lines, request_id, timeout):
    """The next stdout line, which must be the result for `request_id`."""
    try:
        line = lines.get(timeout=timeout)
    except queue.Empty:
        raise CheckFailed(f"no reply to request {request_id} within {timeout:g} s") from None
    if line is None:
        raise CheckFailed(f"stdout closed before the reply to request {request_id}")
    try:
        message = json.loads(line)
    except ValueError:
        raise CheckFailed(f"reply to request {request_id} is not valid JSON") from None
    if not isinstance(message, dict) or message.get("jsonrpc") != "2.0":
        raise CheckFailed(f"reply to request {request_id} is not JSON-RPC 2.0")
    got = message.get("id")
    if type(got) is not int or got != request_id:
        raise CheckFailed(f"expected the reply to request {request_id}, got id {got!r}")
    if "error" in message:
        raise CheckFailed(f"request {request_id} failed with a JSON-RPC error: {message['error']}")
    result = message.get("result")
    if not isinstance(result, dict):
        raise CheckFailed(f"reply to request {request_id} has no result object")
    return line.rstrip("\n"), result


def project_ids(result):
    try:
        return [project["id"] for project in result["structuredContent"]["projects"]]
    except (KeyError, TypeError):
        raise CheckFailed("the tools/call result has no structuredContent.projects") from None


def session(server, timeout):
    lines = queue.Queue()
    reader = threading.Thread(target=pump, args=(server.stdout, lines), daemon=True)
    reader.start()

    send(server, INITIALIZE)
    _, init = reply(lines, 1, timeout)
    capabilities = init.get("capabilities")
    if not isinstance(init.get("protocolVersion"), str) or not (
        isinstance(capabilities, dict) and "tools" in capabilities
    ):
        raise CheckFailed("initialize did not report a protocol version and the tools capability")

    send(server, INITIALIZED)
    send(server, CALL)
    line, result = reply(lines, 2, timeout)
    if result.get("isError", False) is not False:
        raise CheckFailed("the tools/call result has isError set")
    ids = project_ids(result)
    if ids != EXPECTED_IDS:
        raise CheckFailed(f"expected projects {EXPECTED_IDS}, got {ids}")

    try:
        server.stdin.close()
    except OSError:
        pass
    try:
        status = server.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        raise CheckFailed(f"the server did not exit within {timeout:g} s of EOF") from None
    if status != 0:
        raise CheckFailed(f"the server ended with exit status {status}")
    reader.join(timeout)
    if reader.is_alive():
        raise CheckFailed("stdout stayed open after the server exited")
    if lines.get_nowait() is not None:
        raise CheckFailed("unexpected output after the last reply")
    return line


def main(argv):
    timeout = 10.0
    if argv[:1] == ["--timeout"] and len(argv) >= 2:
        timeout = float(argv[1])
        argv = argv[2:]
    if not argv:
        print("usage: check_mcp.py [--timeout SECONDS] COMMAND [ARG...]", file=sys.stderr)
        return 2
    try:
        server = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except OSError as err:
        print(f"check_mcp: cannot start {argv[0]}: {err.strerror}", file=sys.stderr)
        return 1
    try:
        print(session(server, timeout))
        return 0
    except CheckFailed as err:
        print(f"check_mcp: {err}", file=sys.stderr)
        return 1
    finally:
        if server.poll() is None:
            server.kill()
            server.wait(timeout=10)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

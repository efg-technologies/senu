//! `scripts/check_mcp.py`, the quickstart's MCP client, against fake
//! servers: it must succeed only for a well-formed session and fail, without
//! printing a reply, for anything else. Like `docs_sync`, these tests check a
//! script rather than Senu, so they are not part of the test pyramid. They
//! need `python3`.

mod support;

use std::process::{Command, ExitStatus, Stdio};

use support::process;

/// A fake MCP server. For each request (a line containing `"id"`) it prints
/// the next canned reply, or exits on `close`. After the last reply it reads
/// to EOF and exits with `status`.
const FAKE: &str = r#"status=$1; shift
for reply in "$@"; do
  while IFS= read -r line; do
    case $line in *'"id"'*) break ;; esac
  done
  [ "$reply" = close ] && exit 0
  printf '%s\n' "$reply"
done
while IFS= read -r line; do :; done
exit "$status""#;

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"0"}}}"#;
const CALL: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{}"}],"structuredContent":{"projects":[{"id":"acme-001","name":"Apollo Launch"},{"id":"acme-003","name":"Notes"}]},"isError":false}}"#;

fn checker(args: &[&str]) -> Command {
    let mut command = Command::new("python3");
    command
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/check_mcp.py"))
        .args(args);
    command
}

/// Runs the checker against the fake server with these replies.
fn check(status: &str, replies: &[&str]) -> (ExitStatus, Vec<String>, Vec<String>) {
    let mut args = vec!["sh", "-c", FAKE, "fake", status];
    args.extend_from_slice(replies);
    process::run(checker(&args))
}

#[test]
fn a_well_formed_session_passes_and_prints_the_call_reply() {
    let (status, stdout, stderr) = check("0", &[INIT, CALL]);
    assert_eq!(status.code(), Some(0), "{stderr:?}");
    assert_eq!(stdout, [CALL]);
    assert!(stderr.is_empty(), "{stderr:?}");
}

#[test]
fn errors_wrong_ids_wrong_results_and_early_eof_fail() {
    let call_error = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"unknown tool"}}"#;
    let tool_error = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[],"structuredContent":{"error":"invalid_search"},"isError":true}}"#;
    let one_project = CALL.replace(r#",{"id":"acme-003","name":"Notes"}"#, "");
    let extra_line = format!("{CALL}\n{CALL}");
    let init_error = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"internal"}}"#;
    let init_wrong_id = INIT.replace(r#""id":1"#, r#""id":7"#);
    let init_no_tools =
        r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{}}}"#;
    let not_jsonrpc = INIT.replace(r#""jsonrpc":"2.0""#, r#""jsonrpc":"1.0""#);
    let call_wrong_id = CALL.replace(r#""id":2"#, r#""id":3"#);

    let cases: [(&str, &str, Vec<&str>, &str); 13] = [
        ("malformed", "0", vec!["not json"], "not valid JSON"),
        (
            "not JSON-RPC",
            "0",
            vec![not_jsonrpc.as_str()],
            "not JSON-RPC 2.0",
        ),
        ("initialize error", "0", vec![init_error], "JSON-RPC error"),
        (
            "initialize id",
            "0",
            vec![init_wrong_id.as_str()],
            "got id 7",
        ),
        ("no tools", "0", vec![init_no_tools], "tools capability"),
        ("EOF before initialize", "0", vec!["close"], "closed"),
        ("call error", "0", vec![INIT, call_error], "JSON-RPC error"),
        (
            "call id",
            "0",
            vec![INIT, call_wrong_id.as_str()],
            "got id 3",
        ),
        ("isError", "0", vec![INIT, tool_error], "isError"),
        (
            "wrong projects",
            "0",
            vec![INIT, one_project.as_str()],
            "expected projects",
        ),
        ("EOF before call", "0", vec![INIT, "close"], "closed"),
        (
            "extra output",
            "0",
            vec![INIT, extra_line.as_str()],
            "unexpected",
        ),
        ("exit status", "3", vec![INIT, CALL], "exit status 3"),
    ];
    for (name, exit, replies, reason) in cases {
        let (status, stdout, stderr) = check(exit, &replies);
        assert_eq!(status.code(), Some(1), "{name}: {stderr:?}");
        assert!(stdout.is_empty(), "{name}: printed {stdout:?}");
        let stderr = stderr.join("\n");
        assert!(stderr.starts_with("check_mcp: "), "{name}: {stderr}");
        assert!(stderr.contains(reason), "{name}: {stderr}");
    }
}

#[test]
fn a_silent_server_times_out_and_is_killed() {
    let pid_file = std::env::temp_dir().join(format!("senu-check-mcp-{}.pid", std::process::id()));
    let pid_path = pid_file.to_str().expect("UTF-8 temp path");
    let (status, stdout, stderr) = process::run(checker(&[
        "--timeout",
        "1",
        "sh",
        "-c",
        r#"echo $$ > "$1"; exec sleep 30"#,
        "fake",
        pid_path,
    ]));
    let pid = std::fs::read_to_string(&pid_file).expect("the fake wrote its pid");
    std::fs::remove_file(&pid_file).unwrap();
    assert_eq!(status.code(), Some(1), "{stderr:?}");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(stderr.join("\n").contains("no reply"), "{stderr:?}");

    // The checker killed and reaped the fake before exiting; a running or
    // unreaped process would still accept signal 0.
    let alive = Command::new("sh")
        .args(["-c", r#"kill -0 "$1""#, "sh", pid.trim()])
        .stderr(Stdio::null())
        .status()
        .expect("run kill -0");
    assert!(!alive.success(), "the fake server is still running");
}

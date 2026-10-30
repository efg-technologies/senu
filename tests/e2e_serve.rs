//! End-to-end tests of `senu serve` and startup validation, using the real
//! binary, a cleared environment, raw HTTP/1.1 over TCP and 10 s timeouts.

mod support;

use std::net::{SocketAddr, TcpListener};

use support::process::{self, get, http};
use support::*;

const LISTENING: &str = "senu: listening on http://";

fn serve_vars(bind: &str) -> Vec<(&str, &str)> {
    vec![
        ("SENU_BIND", bind),
        ("SENU_WEB_USER", WEB_USER),
        ("SENU_WEB_PASSWORD", WEB_PASSWORD),
        ("SENU_API_TOKEN", API_TOKEN),
    ]
}

fn assert_no_secrets(label: &str, text: &str) {
    for secret in SECRETS {
        assert!(
            !text.contains(secret),
            "{label} contains a configured secret"
        );
    }
}

#[test]
fn serve_answers_api_and_html_requests_over_a_real_listener() {
    let mut server = process::spawn(process::senu(&["serve"], &serve_vars("127.0.0.1:0")));
    let line = server.stderr_line();
    let addr: SocketAddr = line
        .strip_prefix(LISTENING)
        .unwrap_or_else(|| panic!("unexpected first stderr line: {line}"))
        .parse()
        .expect("listening address");
    assert!(addr.ip().is_loopback());
    assert_ne!(addr.port(), 0);

    let mut transcript = String::new();
    let mut exchange = |request: String| {
        let reply = http(addr, &request);
        transcript.push_str(&reply.head);
        transcript.push_str(&reply.body);
        reply
    };

    let api = exchange(get(
        "/api/projects?q=launch",
        &[format!("Authorization: {}", bearer(API_TOKEN))],
    ));
    assert_eq!(api.status, 200);
    let json: serde_json::Value = serde_json::from_str(&api.body).unwrap();
    assert_eq!(
        json["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["acme-001", "acme-003"]
    );
    assert_eq!(api.header("cache-control"), Some("no-store"));
    assert_eq!(api.header("x-content-type-options"), Some("nosniff"));

    let html = exchange(get(
        "/projects?q=launch",
        &[format!("Authorization: {}", basic(WEB_USER, WEB_PASSWORD))],
    ));
    assert_eq!(html.status, 200);
    assert!(
        html.header("content-type")
            .unwrap()
            .starts_with("text/html")
    );
    let ids: Vec<String> = html_items(&html.body)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, ["acme-001", "acme-003"]);
    assert!(!html.body.contains("<script>"));

    let unauthorized_api = exchange(get("/api/projects", &[]));
    assert_eq!(unauthorized_api.status, 401);
    assert_eq!(
        unauthorized_api.header("www-authenticate"),
        Some("Bearer realm=\"senu\"")
    );
    let unauthorized_html = exchange(get(
        "/projects",
        &[format!("Authorization: {}", bearer(API_TOKEN))],
    ));
    assert_eq!(unauthorized_html.status, 401);
    assert_eq!(
        unauthorized_html.header("www-authenticate"),
        Some("Basic realm=\"senu\", charset=\"UTF-8\"")
    );

    let with_body = "GET /api/projects HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 1\r\n\r\nx";
    let too_large = exchange(with_body.to_owned());
    assert_eq!(too_large.status, 413);
    assert_eq!(too_large.body, r#"{"error":"payload_too_large"}"#);

    let long_target = format!("/projects?q={}", "a".repeat(2100));
    let too_long = exchange(get(&long_target, &[]));
    assert_eq!(too_long.status, 414);
    assert_eq!(too_long.body, "URI Too Long");

    let mcp = exchange(get(
        "/mcp",
        &[format!("Authorization: {}", bearer(MCP_TOKEN))],
    ));
    assert_eq!(mcp.status, 404);

    let (stdout, stderr) = server.stop();
    assert!(stdout.is_empty(), "serve wrote to stdout: {stdout:?}");
    assert_eq!(
        stderr,
        std::slice::from_ref(&line),
        "stderr must hold only the listening line"
    );
    assert_no_secrets("HTTP responses", &transcript);
    assert_no_secrets("stderr", &line);
}

#[test]
fn startup_rejects_bad_arguments_and_configuration() {
    struct Case {
        args: &'static [&'static str],
        vars: Vec<(&'static str, &'static str)>,
        exit: i32,
        names: &'static [&'static str],
    }
    let serve_ok = || serve_vars("127.0.0.1:0");
    let replace = |name: &'static str, value: &'static str| {
        let mut vars: Vec<_> = serve_ok().into_iter().filter(|(k, _)| *k != name).collect();
        vars.push((name, value));
        vars
    };
    let remove = |name: &str| -> Vec<(&'static str, &'static str)> {
        serve_ok().into_iter().filter(|(k, _)| *k != name).collect()
    };
    let cases = vec![
        Case {
            args: &["serve"],
            vars: remove("SENU_WEB_USER"),
            exit: 2,
            names: &["SENU_WEB_USER"],
        },
        Case {
            args: &["serve"],
            vars: remove("SENU_API_TOKEN"),
            exit: 2,
            names: &["SENU_API_TOKEN"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_WEB_PASSWORD", ""),
            exit: 2,
            names: &["SENU_WEB_PASSWORD"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_API_TOKEN", "sentinel-api-token-3Z with space"),
            exit: 2,
            names: &["SENU_API_TOKEN"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_API_TOKEN", WEB_PASSWORD),
            exit: 2,
            names: &["SENU_API_TOKEN", "SENU_WEB_PASSWORD"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_ROLE", "admin"),
            exit: 2,
            names: &["SENU_ROLE"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_BIND", "0.0.0.0:3000"),
            exit: 2,
            names: &["SENU_BIND"],
        },
        Case {
            args: &["serve"],
            vars: replace("SENU_BIND", "[::]:3000"),
            exit: 2,
            names: &["SENU_BIND"],
        },
        Case {
            args: &["mcp", "--enable-mcp"],
            vars: vec![("SENU_API_TOKEN", API_TOKEN)],
            exit: 2,
            names: &["SENU_MCP_TOKEN"],
        },
        Case {
            args: &[],
            vars: serve_ok(),
            exit: 2,
            names: &["usage"],
        },
        Case {
            args: &["mcp"],
            vars: vec![("SENU_MCP_TOKEN", MCP_TOKEN)],
            exit: 2,
            names: &["off by default"],
        },
        Case {
            args: &["serve", "--enable-mcp"],
            vars: serve_ok(),
            exit: 2,
            names: &["usage"],
        },
        Case {
            args: &["serve", WEB_PASSWORD],
            vars: serve_ok(),
            exit: 2,
            names: &["usage"],
        },
    ];
    for case in cases {
        let label = format!(
            "{:?} with {:?}",
            case.args,
            case.vars.iter().map(|(k, _)| k).collect::<Vec<_>>()
        );
        let (status, stdout, stderr) = process::run(process::senu(case.args, &case.vars));
        assert_eq!(status.code(), Some(case.exit), "{label}: {stderr:?}");
        assert!(stdout.is_empty(), "{label}: stdout {stdout:?}");
        let stderr = stderr.join("\n");
        for name in case.names {
            assert!(
                stderr.contains(name),
                "{label}: stderr lacks {name}: {stderr}"
            );
        }
        assert!(!stderr.contains("listening"), "{label}: {stderr}");
        assert_no_secrets(&label, &stderr);
    }

    let (status, stdout, stderr) = process::run(process::senu(&["--help"], &[]));
    assert_eq!(status.code(), Some(0));
    assert!(stderr.is_empty());
    let help = stdout.join("\n");
    assert!(help.contains("senu mcp --enable-mcp"), "{help}");
}

#[test]
fn occupied_port_exits_with_status_1() {
    let holder = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = holder.local_addr().unwrap().to_string();
    let vars = serve_vars(&taken);
    let (status, stdout, stderr) = process::run(process::senu(&["serve"], &vars));
    assert_eq!(status.code(), Some(1), "{stderr:?}");
    assert!(stdout.is_empty());
    let stderr = stderr.join("\n");
    assert!(stderr.starts_with("senu: "), "{stderr}");
    assert!(!stderr.contains("listening on"), "{stderr}");
    assert_no_secrets("stderr", &stderr);
    drop(holder);
}

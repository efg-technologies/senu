//! End-to-end tests of `senu site` (spec 0003, SITE-1 and SITE-2) with the
//! real binary, a cleared environment, raw HTTP/1.1 and 10 s timeouts.

mod support;

use std::net::{SocketAddr, TcpListener};
use std::path::Path;

use support::process::{self, get, http};
use support::{STYLESHEET, committed_stylesheet};

const LISTENING: &str = "senu: listening on http://";
const CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'";
const SENTINEL: &str = "zz-sentinel-value-41";

/// The text of the first line of an article, without `# `.
fn title_of(rel: &str) -> String {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap();
    let first = text.trim_start_matches('\u{feff}').lines().next().unwrap();
    first
        .strip_prefix("# ")
        .expect("an ATX title")
        .trim()
        .to_owned()
}

#[test]
fn site_serves_the_embedded_articles_from_an_empty_directory() {
    let cwd = Path::new(env!("CARGO_TARGET_TMPDIR")).join("e2e-site-empty-cwd");
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).unwrap();
    let mut command = process::senu(&["site"], &[("SENU_BIND", "127.0.0.1:0")]);
    command.current_dir(&cwd);
    let mut server = process::spawn(command);
    let line = server.stderr_line();
    let addr: SocketAddr = line
        .strip_prefix(LISTENING)
        .unwrap_or_else(|| panic!("unexpected first stderr line: {line}"))
        .parse()
        .unwrap();
    assert!(addr.ip().is_loopback());

    let mut transcript = String::new();
    let mut fetch = |target: &str, headers: &[String]| {
        let reply = http(addr, &get(target, headers));
        transcript.push_str(&reply.head);
        transcript.push_str(&reply.body);
        assert_eq!(reply.header("www-authenticate"), None, "{target}");
        assert_eq!(reply.header("cache-control"), Some("no-store"), "{target}");
        assert_eq!(
            reply.header("x-content-type-options"),
            Some("nosniff"),
            "{target}"
        );
        reply
    };
    for (target, title) in [
        ("/", "Build a small notes site with Rust".to_owned()),
        ("/ja/", "Rust で小さなノートサイトを作る".to_owned()),
        ("/start", title_of("content/en/01-start.md")),
        ("/ja/start", title_of("content/ja/01-start.md")),
    ] {
        let page = fetch(target, &[]);
        assert_eq!(page.status, 200, "{target}");
        assert_eq!(
            page.header("content-type"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(page.header("content-security-policy"), Some(CSP));
        assert!(page.body.contains(&format!("<h1>{title}</h1>")), "{target}");
        let with_credentials = fetch(target, &[format!("Authorization: Bearer {SENTINEL}")]);
        assert_eq!(with_credentials.body, page.body, "{target}");
    }
    let stylesheet = fetch(STYLESHEET, &[]);
    assert_eq!(stylesheet.status, 200);
    assert_eq!(
        stylesheet.header("content-type"),
        Some("text/css; charset=utf-8")
    );
    assert!(
        stylesheet.body == committed_stylesheet(),
        "not the committed stylesheet"
    );
    for target in [
        "/projects",
        "/api/projects",
        "/mcp",
        "/missing",
        "//start",
        "/start/",
    ] {
        let reply = fetch(target, &[]);
        assert_eq!(reply.status, 404, "{target}");
        assert_eq!(
            reply.header("content-type"),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(reply.body, "Not Found");
    }
    let posted = http(
        addr,
        "POST /start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
    );
    assert_eq!((posted.status, posted.header("allow")), (405, Some("GET")));

    for leaked in [env!("CARGO_MANIFEST_DIR"), cwd.to_str().unwrap(), SENTINEL] {
        assert!(!transcript.contains(leaked), "a response contains {leaked}");
    }
    let (stdout, stderr) = server.stop();
    assert!(stdout.is_empty(), "{stdout:?}");
    assert_eq!(stderr.len(), 1, "{stderr:?}");
}

#[test]
fn site_startup_refuses_credentials_and_bad_configuration() {
    let refused = [
        ("SENU_WEB_USER", SENTINEL),
        ("SENU_WEB_PASSWORD", SENTINEL),
        ("SENU_API_TOKEN", SENTINEL),
        ("SENU_MCP_TOKEN", SENTINEL),
        ("SENU_X", SENTINEL),
        ("SENU_BIND", "0.0.0.0:0"),
        ("SENU_BIND", "192.0.2.1:3000"),
    ];
    for (name, value) in refused {
        let (status, stdout, stderr) = process::run(process::senu(&["site"], &[(name, value)]));
        assert_eq!(status.code(), Some(2), "{name}");
        assert!(stdout.is_empty(), "{name}: {stdout:?}");
        let stderr = stderr.join("\n");
        assert!(stderr.contains(name), "{name}: {stderr}");
        assert!(
            !stderr.contains(SENTINEL) && !stderr.contains(LISTENING),
            "{name}: {stderr}"
        );
    }

    let (status, _, stderr) = process::run(process::senu(&["site", "extra"], &[]));
    assert_eq!(status.code(), Some(2));
    assert!(
        stderr
            .join("\n")
            .contains("usage: senu serve | senu site | senu mcp --enable-mcp | senu --help")
    );

    let (status, stdout, _) = process::run(process::senu(&["--help"], &[]));
    assert_eq!(status.code(), Some(0));
    assert!(
        stdout
            .iter()
            .any(|line| line.trim_start().starts_with("senu site"))
    );

    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let bind = held.local_addr().unwrap().to_string();
    let (status, _, stderr) = process::run(process::senu(&["site"], &[("SENU_BIND", &bind)]));
    assert_eq!(status.code(), Some(1));
    assert_eq!(stderr.len(), 1, "{stderr:?}");
    assert!(
        stderr[0].starts_with(&format!("senu: cannot listen on {bind}: ")),
        "{stderr:?}"
    );
}

//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::ffi::OsString;

use base64::Engine as _;

pub const WEB_USER: &str = "demo";
pub const WEB_PASSWORD: &str = "sentinel-web-password-7Q";
pub const API_TOKEN: &str = "sentinel-api-token-3Z";
pub const MCP_TOKEN: &str = "sentinel-mcp-token-9K";

/// Every configured secret. None may ever appear in output.
pub const SECRETS: [&str; 3] = [WEB_PASSWORD, API_TOKEN, MCP_TOKEN];

/// The one stylesheet route of spec 0004, §4.
pub const STYLESHEET: &str = "/_senu/ui.css";

/// Near misses of `STYLESHEET`, all 404 on both routers: a trailing slash,
/// other case, percent-encoding, dot segments and other names.
pub const NOT_THE_STYLESHEET: [&str; 21] = [
    "/_senu/ui.css/",
    "/_senu/UI.css",
    "/_SENU/ui.css",
    "/_senu/ui.CSS",
    "/_senu/ui%2Ecss",
    "/_senu/ui%2ecss",
    "/%5Fsenu/ui.css",
    "/_senu%2Fui.css",
    "/_senu/./ui.css",
    "/_senu/../_senu/ui.css",
    "/x/../_senu/ui.css",
    "//_senu/ui.css",
    "/_senu/ui.css;x",
    "/_senu",
    "/_senu/",
    "/_senu/ui.generated.css",
    "/_senu/ui.js",
    "/_senu/x",
    "/assets/ui.css",
    "/assets/ui.generated.css",
    "/ui.css",
];

/// The committed stylesheet, which both routers serve byte for byte.
pub fn committed_stylesheet() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/ui.generated.css"
    ))
    .expect("assets/ui.generated.css")
}

pub fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect()
}

/// A valid `serve` environment using the sentinel credentials.
pub fn serve_env() -> Vec<(OsString, OsString)> {
    env(&[
        ("SENU_WEB_USER", WEB_USER),
        ("SENU_WEB_PASSWORD", WEB_PASSWORD),
        ("SENU_API_TOKEN", API_TOKEN),
    ])
}

pub fn basic(user: &str, password: &str) -> String {
    let raw = format!("{user}:{password}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    )
}

pub fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// Decodes HTML entities without assuming whether Askama emits named or
/// numeric references.
pub fn unescape_html(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let decoded = tail.find(';').and_then(|semi| {
            let entity = &tail[1..semi];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, semi + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The `(id, name)` pairs of the project list items, names unescaped.
pub fn html_items(page: &str) -> Vec<(String, String)> {
    const OPEN: &str = "<li data-project-id=\"";
    let mut items = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let id_end = after.find("\">").expect("end of id attribute");
        let name_start = id_end + 2;
        let name_end = name_start + after[name_start..].find("</li>").expect("end of item");
        items.push((
            unescape_html(&after[..id_end]),
            unescape_html(&after[name_start..name_end]),
        ));
        rest = &after[name_end..];
    }
    items
}

/// The unescaped `value` of the search input.
pub fn search_value(page: &str) -> String {
    let start = page.find("value=\"").expect("value attribute") + "value=\"".len();
    let end = start + page[start..].find('"').expect("closing quote");
    unescape_html(&page[start..end])
}

pub mod process {
    //! Real-process helpers: a cleared environment, piped stdio, 10 s
    //! timeouts, and children that are always killed and reaped.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    pub const TIMEOUT: Duration = Duration::from_secs(10);

    /// The real `senu` binary with an empty environment.
    pub fn senu(args: &[&str], vars: &[(&str, &str)]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_senu"));
        command.args(args).env_clear();
        for (name, value) in vars {
            command.env(name, value);
        }
        command
    }

    /// A running child whose stdout and stderr are read line by line on
    /// background threads. Dropping it kills and reaps the process.
    pub struct Running {
        child: Child,
        pub stdin: Option<ChildStdin>,
        stdout: Receiver<String>,
        stderr: Receiver<String>,
        readers: Vec<JoinHandle<()>>,
        stdout_seen: Vec<String>,
        stderr_seen: Vec<String>,
    }

    fn pump<R: Read + Send + 'static>(source: R) -> (Receiver<String>, JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel();
        let handle = thread::spawn(move || {
            for line in BufReader::new(source).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        (receiver, handle)
    }

    pub fn spawn(mut command: Command) -> Running {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn senu");
        let stdin = child.stdin.take();
        let (stdout, out_reader) = pump(child.stdout.take().unwrap());
        let (stderr, err_reader) = pump(child.stderr.take().unwrap());
        Running {
            child,
            stdin,
            stdout,
            stderr,
            readers: vec![out_reader, err_reader],
            stdout_seen: Vec::new(),
            stderr_seen: Vec::new(),
        }
    }

    impl Running {
        /// The next stdout line, or a panic after the timeout.
        pub fn stdout_line(&mut self) -> String {
            let line = self
                .stdout
                .recv_timeout(TIMEOUT)
                .expect("a stdout line within the timeout");
            self.stdout_seen.push(line.clone());
            line
        }

        /// The next stderr line, or a panic after the timeout.
        pub fn stderr_line(&mut self) -> String {
            let line = self
                .stderr
                .recv_timeout(TIMEOUT)
                .expect("a stderr line within the timeout");
            self.stderr_seen.push(line.clone());
            line
        }

        pub fn write_line(&mut self, line: &str) {
            let stdin = self.stdin.as_mut().expect("stdin is open");
            stdin.write_all(line.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
            stdin.flush().unwrap();
        }

        pub fn close_stdin(&mut self) {
            self.stdin.take();
        }

        /// Waits for exit within the timeout, then drains both streams.
        pub fn wait(mut self) -> (ExitStatus, Vec<String>, Vec<String>) {
            let deadline = Instant::now() + TIMEOUT;
            let status = loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    break status;
                }
                assert!(Instant::now() < deadline, "senu did not exit in time");
                thread::sleep(Duration::from_millis(20));
            };
            self.drain();
            (
                status,
                std::mem::take(&mut self.stdout_seen),
                std::mem::take(&mut self.stderr_seen),
            )
        }

        /// Kills the process, then drains both streams.
        pub fn stop(mut self) -> (Vec<String>, Vec<String>) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.drain();
            (
                std::mem::take(&mut self.stdout_seen),
                std::mem::take(&mut self.stderr_seen),
            )
        }

        fn drain(&mut self) {
            self.stdin.take();
            for reader in self.readers.drain(..) {
                reader.join().unwrap();
            }
            self.stdout_seen.extend(self.stdout.try_iter());
            self.stderr_seen.extend(self.stderr.try_iter());
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// Runs a command that should exit on its own, with empty stdin.
    pub fn run(command: Command) -> (ExitStatus, Vec<String>, Vec<String>) {
        let mut running = spawn(command);
        running.close_stdin();
        running.wait()
    }

    /// A raw HTTP/1.1 exchange over a real socket.
    pub struct HttpReply {
        pub status: u16,
        pub head: String,
        pub body: String,
    }

    impl HttpReply {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.head.lines().skip(1).find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then(|| value.trim())
            })
        }
    }

    pub fn http(addr: SocketAddr, request: &str) -> HttpReply {
        let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response");
        let raw = String::from_utf8(raw).expect("UTF-8 response");
        let (head, body) = raw.split_once("\r\n\r\n").expect("response head");
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status code");
        HttpReply {
            status,
            head: head.to_owned(),
            body: body.to_owned(),
        }
    }

    /// A GET request with `Connection: close` and optional extra headers.
    pub fn get(target: &str, headers: &[String]) -> String {
        let mut request =
            format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        for header in headers {
            request.push_str(header);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        request
    }
}

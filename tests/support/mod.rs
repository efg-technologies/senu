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

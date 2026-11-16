//! Authentication: the [`Authenticate`] trait, the built-in [`Basic`] and
//! [`Bearer`] authenticators, and `Authorization` header parsing.
//!
//! An authenticator turns a request's headers into a server-side
//! [`Principal`], or into nothing. It is the only place where a principal
//! is chosen, and it never sees the path or the query string.
//!
//! Secrets are compared with [`subtle::ConstantTimeEq`]. Senu makes no timing
//! guarantee beyond what that crate provides; in particular a length
//! mismatch is detected before the byte comparison.
//!
//! # Your own authenticator
//!
//! Sessions, API keys in a database or a header set by a trusted proxy are
//! each an implementation of the trait:
//!
//! ```
//! use axum::http::{HeaderMap, HeaderValue};
//! use senu::auth::{AuthFuture, Authenticate};
//! use senu::operation::{Permission, Principal, TenantId};
//!
//! const READ: Permission = Permission::new("read");
//!
//! struct ApiKeys; // would hold a database pool
//!
//! impl ApiKeys {
//!     async fn lookup(&self, key: &[u8]) -> Option<Principal> {
//!         // Look the key's hash up, then map the stored roles to constants.
//!         (key == b"k-123").then(|| Principal::owned("svc-reports", TenantId::new("t-7"), [READ]))
//!     }
//! }
//!
//! impl Authenticate for ApiKeys {
//!     fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
//!         Box::pin(async move {
//!             let key = senu::auth::bearer_token(headers)?;
//!             self.lookup(&key).await
//!         })
//!     }
//!
//!     fn challenge(&self) -> Option<HeaderValue> {
//!         Some(HeaderValue::from_static("Bearer realm=\"reports\""))
//!     }
//! }
//! ```

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use axum::http::{HeaderMap, HeaderValue, header};
use base64::Engine as _;
use subtle::ConstantTimeEq;

use crate::operation::Principal;

/// Longest `Authorization` header value that is parsed at all.
pub const MAX_AUTHORIZATION_BYTES: usize = 1024;

/// A configured credential. It never appears in `Debug` output and has no
/// `Display`, so it cannot leak through logs or error messages by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

/// Why a value cannot be a [`Secret`]. It never holds the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSecret;

impl fmt::Display for InvalidSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a secret must be 1 to 256 visible ASCII characters (0x21-0x7E)")
    }
}

impl std::error::Error for InvalidSecret {}

/// Longest [`Secret`], in bytes.
pub const MAX_SECRET_BYTES: usize = 256;

impl Secret {
    /// Accepts 1 to 256 visible ASCII characters, the values that can
    /// travel in an `Authorization` header unchanged.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidSecret> {
        let value = value.into();
        let valid = (1..=MAX_SECRET_BYTES).contains(&value.len()) && value.bytes().all(is_vchar);
        if valid {
            Ok(Self(value))
        } else {
            Err(InvalidSecret)
        }
    }

    /// Compares `presented` with this secret in constant time for equal
    /// lengths.
    pub fn matches(&self, presented: &[u8]) -> bool {
        self.0.as_bytes().ct_eq(presented).into()
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// Credentials presented in an `Authorization` header.
#[derive(Clone, PartialEq, Eq)]
pub enum Presented {
    /// `Basic`, decoded and split at the first colon.
    Basic {
        /// The bytes before the first colon.
        user: Vec<u8>,
        /// The bytes after it, which may hold more colons.
        password: Vec<u8>,
    },
    /// `Bearer` with its token.
    Bearer(Vec<u8>),
}

impl fmt::Debug for Presented {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic { .. } => f.write_str("Basic(<redacted>)"),
            Self::Bearer(_) => f.write_str("Bearer(<redacted>)"),
        }
    }
}

/// Parses one `Authorization` header value.
///
/// The scheme matches case-insensitively. Anything malformed, oversized or
/// using another scheme yields `None`.
pub fn parse_authorization(value: &[u8]) -> Option<Presented> {
    if value.len() > MAX_AUTHORIZATION_BYTES {
        return None;
    }
    let space = value.iter().position(|&b| b == b' ')?;
    let (scheme, rest) = value.split_at(space);
    let credentials = trim_leading_spaces(rest);
    if credentials.is_empty() || !credentials.iter().all(|&b| is_vchar(b)) {
        return None;
    }
    if scheme.eq_ignore_ascii_case(b"basic") {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(credentials)
            .ok()?;
        let colon = decoded.iter().position(|&b| b == b':')?;
        Some(Presented::Basic {
            user: decoded[..colon].to_vec(),
            password: decoded[colon + 1..].to_vec(),
        })
    } else if scheme.eq_ignore_ascii_case(b"bearer") {
        Some(Presented::Bearer(credentials.to_vec()))
    } else {
        None
    }
}

/// True when `header` carries Basic credentials equal to `user` and
/// `password`.
pub fn basic_matches(header: Option<&[u8]>, user: &Secret, password: &Secret) -> bool {
    match header.and_then(parse_authorization) {
        Some(Presented::Basic {
            user: presented_user,
            password: presented_password,
        }) => {
            // Evaluate both comparisons so a wrong user name and a wrong
            // password take the same path.
            let user_ok = user.as_bytes().ct_eq(&presented_user);
            let password_ok = password.as_bytes().ct_eq(&presented_password);
            (user_ok & password_ok).into()
        }
        _ => false,
    }
}

/// True when `header` carries a Bearer token equal to `token`.
pub fn bearer_matches(header: Option<&[u8]>, token: &Secret) -> bool {
    match header.and_then(parse_authorization) {
        Some(Presented::Bearer(presented)) => token.matches(&presented),
        _ => false,
    }
}

/// The future of [`Authenticate::authenticate`].
pub type AuthFuture<'a> = Pin<Box<dyn Future<Output = Option<Principal>> + Send + 'a>>;

/// Decides who is calling, from the request's headers alone.
///
/// `None` is answered with 401 and [`challenge`](Self::challenge). The path,
/// the query string and the body are never passed in, so a credential in
/// the URL cannot be read. Implementations must not log credentials, and
/// should take the same time for a wrong credential as for a right one
/// where they can.
pub trait Authenticate: Send + Sync + 'static {
    /// The principal for these headers, or `None`.
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a>;

    /// The `WWW-Authenticate` value sent with 401, for a scheme that has
    /// one. A session authenticator keeps the default, `None`.
    fn challenge(&self) -> Option<HeaderValue> {
        None
    }
}

/// The value of the cookie `name` when the request carries it exactly once,
/// for a session [`Authenticate`]. A name that appears twice is treated as
/// absent, so two layers cannot disagree about which value counts. The
/// value is returned as sent, without decoding.
pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut found = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| {
            let (key, value) = pair.trim_start_matches(' ').split_once('=')?;
            (key == name).then_some(value.trim_end_matches(' '))
        });
    match (found.next(), found.next()) {
        (Some(value), None) if !name.is_empty() => Some(value),
        _ => None,
    }
}

/// Whether `origin` has the form `scheme://host` or `scheme://host:port`,
/// in visible ASCII without a path.
pub(crate) fn is_origin(origin: &str) -> bool {
    let Some((scheme, host)) = origin.split_once("://") else {
        return false;
    };
    matches!(scheme, "https" | "http")
        && !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.:[]".contains(&b))
}

/// Whether a state-changing request may proceed as far as cross-site
/// request forgery is concerned (spec 0007, §3). The rules are those of Go
/// 1.25's `CrossOriginProtection`:
///
/// 1. An `Origin` equal to one of `trusted` passes.
/// 2. With `Sec-Fetch-Site`, only `same-origin` and `none` pass.
/// 3. Otherwise, with `Origin`, its host and port must equal `Host`.
/// 4. With neither header the request is not from a browser, and passes.
///
/// Senu applies this to every declared write. A `custom` handler that
/// changes state for a browser should call it too. It does nothing against
/// a non-browser client, which needs no forgery to send a request.
pub fn same_origin(headers: &HeaderMap, trusted: &[&str]) -> bool {
    same_origin_at(headers, None, trusted)
}

/// [`same_origin`] for a request whose host may be in the request target
/// instead of a `Host` header, as in HTTP/2.
pub(crate) fn same_origin_at(
    headers: &HeaderMap,
    authority: Option<&str>,
    trusted: &[&str],
) -> bool {
    let single = |name: header::HeaderName| {
        let mut values = headers.get_all(name).iter();
        match (values.next(), values.next()) {
            (None, _) => Ok(None),
            (Some(value), None) => value.to_str().map(Some).map_err(|_| ()),
            // Two values: refuse rather than guess which one counts.
            _ => Err(()),
        }
    };
    let (Ok(origin), Ok(site), Ok(host)) = (
        single(header::ORIGIN),
        single(header::HeaderName::from_static("sec-fetch-site")),
        single(header::HOST),
    ) else {
        return false;
    };
    if let Some(origin) = origin
        && trusted.contains(&origin)
    {
        return true;
    }
    match (site, origin) {
        (Some(site), _) => site == "same-origin" || site == "none",
        (None, None) => true,
        (None, Some(origin)) => match (origin.split_once("://"), host.or(authority)) {
            (Some((_, origin_host)), Some(host)) => origin_host.eq_ignore_ascii_case(host),
            _ => false,
        },
    }
}

/// The `Authorization` value when exactly one is present. Two headers are
/// treated as none, so a proxy and the server cannot disagree about which
/// one counts.
pub fn single_authorization(headers: &HeaderMap) -> Option<&[u8]> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => Some(value.as_bytes()),
        _ => None,
    }
}

/// The Bearer token of the single `Authorization` header, for an
/// [`Authenticate`] implementation that looks tokens up itself.
pub fn bearer_token(headers: &HeaderMap) -> Option<Vec<u8>> {
    match single_authorization(headers).and_then(parse_authorization) {
        Some(Presented::Bearer(token)) => Some(token),
        _ => None,
    }
}

/// HTTP Basic with one fixed user, password and principal.
///
/// Basic sends the password with every request, so use it only over TLS,
/// and prefer your own session-based [`Authenticate`] for people.
#[derive(Debug)]
pub struct Basic {
    user: Secret,
    password: Secret,
    principal: Principal,
}

impl Basic {
    /// Binds `principal` to one user name and password. A user name that
    /// holds `:` can never match, because Basic splits at the first colon.
    pub fn new(user: Secret, password: Secret, principal: Principal) -> Self {
        Self {
            user,
            password,
            principal,
        }
    }
}

impl Authenticate for Basic {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        let matches = basic_matches(single_authorization(headers), &self.user, &self.password);
        Box::pin(std::future::ready(matches.then(|| self.principal.clone())))
    }

    fn challenge(&self) -> Option<HeaderValue> {
        Some(HeaderValue::from_static(
            "Basic realm=\"senu\", charset=\"UTF-8\"",
        ))
    }
}

/// Bearer tokens, each bound to a principal.
#[derive(Debug)]
pub struct Bearer {
    tokens: Vec<(Secret, Principal)>,
}

impl Bearer {
    /// Binds `principal` to one token.
    pub fn new(token: Secret, principal: Principal) -> Self {
        Self {
            tokens: vec![(token, principal)],
        }
    }

    /// Adds another token with its own principal. If two entries hold the
    /// same token, the first one wins.
    pub fn and(mut self, token: Secret, principal: Principal) -> Self {
        self.tokens.push((token, principal));
        self
    }
}

impl Authenticate for Bearer {
    fn authenticate<'a>(&'a self, headers: &'a HeaderMap) -> AuthFuture<'a> {
        let presented = bearer_token(headers);
        // Compare with every token, so the time does not depend on which
        // one matched.
        let mut found = None;
        if let Some(presented) = presented {
            for (token, principal) in &self.tokens {
                if token.matches(&presented) && found.is_none() {
                    found = Some(principal.clone());
                }
            }
        }
        Box::pin(std::future::ready(found))
    }

    fn challenge(&self) -> Option<HeaderValue> {
        Some(HeaderValue::from_static("Bearer realm=\"senu\""))
    }
}

/// Visible ASCII, bytes 0x21 to 0x7E.
pub(crate) fn is_vchar(byte: u8) -> bool {
    (0x21..=0x7e).contains(&byte)
}

fn trim_leading_spaces(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| b != b' ').unwrap_or(bytes.len());
    &bytes[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(value: &str) -> Secret {
        Secret::new(value).expect("a valid secret")
    }

    fn basic_header(user: &str, password: &str) -> String {
        let raw = format!("{user}:{password}");
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }

    #[test]
    fn parses_basic_credentials() {
        assert_eq!(
            parse_authorization(basic_header("demo", "pw").as_bytes()),
            Some(Presented::Basic {
                user: b"demo".to_vec(),
                password: b"pw".to_vec(),
            })
        );
    }

    #[test]
    fn basic_password_may_contain_colons() {
        assert_eq!(
            parse_authorization(basic_header("demo", "a:b:c").as_bytes()),
            Some(Presented::Basic {
                user: b"demo".to_vec(),
                password: b"a:b:c".to_vec(),
            })
        );
    }

    #[test]
    fn parses_bearer_tokens_with_any_visible_ascii() {
        assert_eq!(
            parse_authorization(b"Bearer abc!\"#$%~"),
            Some(Presented::Bearer(b"abc!\"#$%~".to_vec()))
        );
    }

    #[test]
    fn schemes_match_case_insensitively() {
        for scheme in ["basic", "BASIC", "bAsIc"] {
            let header = basic_header("u", "p").replacen("Basic", scheme, 1);
            assert!(
                matches!(
                    parse_authorization(header.as_bytes()),
                    Some(Presented::Basic { .. })
                ),
                "{scheme}"
            );
        }
        for scheme in ["bearer", "BEARER", "BeArEr"] {
            let header = format!("{scheme} token");
            assert_eq!(
                parse_authorization(header.as_bytes()),
                Some(Presented::Bearer(b"token".to_vec())),
                "{scheme}"
            );
        }
    }

    #[test]
    fn several_spaces_after_the_scheme_are_allowed() {
        assert_eq!(
            parse_authorization(b"Bearer   token"),
            Some(Presented::Bearer(b"token".to_vec()))
        );
    }

    #[test]
    fn malformed_values_are_rejected() {
        let cases: &[&[u8]] = &[
            b"",
            b"Bearer",
            b"Bearer ",
            b"Basic",
            b"Basic ",
            b"Bearer a b",
            b"Bearer a\tb",
            b"Bearer\ttoken",
            b" Bearer token",
            b"Bearer token ",
            b"Bearer tok\x7fen",
            b"Bearer t\xc3\xa9",
            b"Token abc",
            b"Digest abc",
            b"Bearertoken",
            b"Basic !!!notbase64",
            b"Basic ZGVtbw==",
        ];
        for value in cases {
            assert_eq!(
                parse_authorization(value),
                None,
                "{:?}",
                String::from_utf8_lossy(value)
            );
        }
    }

    #[test]
    fn oversized_values_are_rejected() {
        let token = "a".repeat(MAX_AUTHORIZATION_BYTES);
        let header = format!("Bearer {token}");
        assert!(header.len() > MAX_AUTHORIZATION_BYTES);
        assert_eq!(parse_authorization(header.as_bytes()), None);
    }

    #[test]
    fn secret_matches_only_the_exact_bytes() {
        let s = secret("correct-horse");
        assert!(s.matches(b"correct-horse"));
        assert!(!s.matches(b"correct-hors"));
        assert!(!s.matches(b"correct-horse!"));
        assert!(!s.matches(b"CORRECT-HORSE"));
        assert!(!s.matches(b""));
    }

    #[test]
    fn basic_matches_requires_both_user_and_password() {
        let (user, password) = (secret("demo"), secret("web-pass"));
        let ok = basic_header("demo", "web-pass");
        assert!(basic_matches(Some(ok.as_bytes()), &user, &password));
        let wrong_user = basic_header("demo2", "web-pass");
        assert!(!basic_matches(
            Some(wrong_user.as_bytes()),
            &user,
            &password
        ));
        let wrong_password = basic_header("demo", "web-pas");
        assert!(!basic_matches(
            Some(wrong_password.as_bytes()),
            &user,
            &password
        ));
        assert!(!basic_matches(None, &user, &password));
    }

    #[test]
    fn basic_matches_rejects_bearer_credentials() {
        let (user, password) = (secret("demo"), secret("web-pass"));
        assert!(!basic_matches(Some(b"Bearer web-pass"), &user, &password));
    }

    #[test]
    fn bearer_matches_only_the_configured_token() {
        let token = secret("api-token");
        assert!(bearer_matches(Some(b"Bearer api-token"), &token));
        assert!(bearer_matches(Some(b"bearer api-token"), &token));
        assert!(!bearer_matches(Some(b"Bearer api-token2"), &token));
        assert!(!bearer_matches(None, &token));
        let basic = basic_header("demo", "api-token");
        assert!(!bearer_matches(Some(basic.as_bytes()), &token));
    }

    #[test]
    fn debug_output_never_contains_credentials() {
        let s = secret("sentinel-secret");
        assert!(!format!("{s:?}").contains("sentinel"));
        let presented = parse_authorization(b"Bearer sentinel-secret");
        assert!(!format!("{presented:?}").contains("sentinel"));
        let presented = parse_authorization(basic_header("u", "sentinel-secret").as_bytes());
        assert!(!format!("{presented:?}").contains("sentinel"));
    }

    // Spec 0006, §3: the public constructor and the authenticators.

    fn headers(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(
                header::AUTHORIZATION,
                HeaderValue::from_str(value).expect("a header value"),
            );
        }
        headers
    }

    fn principal(name: &'static str) -> Principal {
        Principal::new(name, crate::operation::TenantId::from_static("t"), &[])
    }

    /// The built-in authenticators are ready at once, so one poll is enough.
    fn now(future: AuthFuture<'_>) -> Option<Principal> {
        let mut future = future;
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(principal) => principal,
            std::task::Poll::Pending => panic!("a built-in authenticator was not ready"),
        }
    }

    #[test]
    fn secrets_are_1_to_256_visible_ascii_characters() {
        assert!(Secret::new("a").is_ok());
        assert!(Secret::new("a".repeat(256)).is_ok());
        assert!(Secret::new("!~").is_ok());
        for bad in [
            String::new(),
            "a".repeat(257),
            "a b".to_owned(),
            "tab\there".to_owned(),
            "line\n".to_owned(),
            "caf\u{e9}".to_owned(),
            "del\u{7f}".to_owned(),
        ] {
            assert_eq!(Secret::new(bad.clone()).err(), Some(InvalidSecret));
        }
        let message = InvalidSecret.to_string();
        assert!(message.contains("1 to 256"), "{message}");
    }

    #[test]
    fn only_a_single_authorization_header_is_used() {
        assert_eq!(single_authorization(&headers(&[])), None);
        let one = headers(&["Bearer a"]);
        assert_eq!(single_authorization(&one), Some(&b"Bearer a"[..]));
        assert_eq!(
            single_authorization(&headers(&["Bearer a", "Bearer a"])),
            None
        );
        assert_eq!(bearer_token(&one), Some(b"a".to_vec()));
        assert_eq!(bearer_token(&headers(&["Bearer a", "Bearer a"])), None);
        assert_eq!(bearer_token(&headers(&[&basic_header("u", "p")])), None);
    }

    #[test]
    fn basic_yields_its_principal_only_for_the_exact_pair() {
        let auth = Basic::new(secret("demo"), secret("web-pass"), principal("web"));
        let good = basic_header("demo", "web-pass");
        assert_eq!(
            now(auth.authenticate(&headers(&[&good]))),
            Some(principal("web"))
        );
        for wrong in [
            basic_header("demo", "web-pasS"),
            basic_header("Demo", "web-pass"),
            basic_header("demo", ""),
            "Bearer web-pass".to_owned(),
        ] {
            assert_eq!(now(auth.authenticate(&headers(&[&wrong]))), None, "{wrong}");
        }
        assert_eq!(now(auth.authenticate(&headers(&[]))), None);
        assert_eq!(now(auth.authenticate(&headers(&[&good, &good]))), None);
        assert_eq!(
            auth.challenge().unwrap(),
            "Basic realm=\"senu\", charset=\"UTF-8\""
        );
        let debug = format!("{auth:?}");
        assert!(
            !debug.contains("web-pass") && !debug.contains("demo"),
            "{debug}"
        );
    }

    #[test]
    fn bearer_maps_each_token_to_its_own_principal() {
        let auth = Bearer::new(secret("token-a"), principal("a"))
            .and(secret("token-b"), principal("b"))
            .and(secret("token-a"), principal("shadowed"));
        let call = |value: &str| now(auth.authenticate(&headers(&[value])));
        assert_eq!(call("Bearer token-a"), Some(principal("a")));
        assert_eq!(call("bearer token-b"), Some(principal("b")));
        for wrong in [
            "Bearer token-c",
            "Bearer token-",
            "Bearer token-aa",
            "Token token-a",
        ] {
            assert_eq!(call(wrong), None, "{wrong}");
        }
        assert_eq!(call(&basic_header("token-a", "token-a")), None);
        assert_eq!(now(auth.authenticate(&headers(&[]))), None);
        let twice = headers(&["Bearer token-a", "Bearer token-a"]);
        assert_eq!(now(auth.authenticate(&twice)), None);
        assert_eq!(auth.challenge().unwrap(), "Bearer realm=\"senu\"");
        assert!(!format!("{auth:?}").contains("token-a"));
    }

    // Spec 0007, §3: cookies and the cross-site check.

    fn with(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_str(value).expect("a header value"));
        }
        headers
    }

    #[test]
    fn a_cookie_counts_only_when_it_appears_exactly_once() {
        let one = with(&[("cookie", "theme=dark; sid=abc123; lang=ja")]);
        assert_eq!(cookie(&one, "sid"), Some("abc123"));
        assert_eq!(cookie(&one, "theme"), Some("dark"));
        assert_eq!(cookie(&one, "lang"), Some("ja"));
        assert_eq!(cookie(&one, "si"), None);
        assert_eq!(cookie(&one, "sid2"), None);
        assert_eq!(cookie(&one, ""), None);
        assert_eq!(cookie(&with(&[]), "sid"), None);
        // A value may hold '=' and is not decoded.
        let padded = with(&[("cookie", "sid=a=b%3D; x=1")]);
        assert_eq!(cookie(&padded, "sid"), Some("a=b%3D"));
        assert_eq!(cookie(&with(&[("cookie", "sid=")]), "sid"), Some(""));
        // Twice, in one header or in two, is treated as absent.
        assert_eq!(cookie(&with(&[("cookie", "sid=a; sid=b")]), "sid"), None);
        let two = with(&[("cookie", "sid=a"), ("cookie", "sid=a")]);
        assert_eq!(cookie(&two, "sid"), None);
        let split = with(&[("cookie", "x=1"), ("cookie", "sid=a")]);
        assert_eq!(cookie(&split, "sid"), Some("a"));
        // Names are case-sensitive, and a prefix is another cookie.
        assert_eq!(cookie(&with(&[("cookie", "SID=a; xsid=b")]), "sid"), None);
    }

    #[test]
    fn the_cross_site_check_follows_its_four_rules() {
        let trusted = ["https://app.example.com"];
        let cases: [(&[(&'static str, &str)], bool); 17] = [
            // 4. No browser header at all: a program, not a page.
            (&[], true),
            (&[("host", "api.example.com")], true),
            // 2. Sec-Fetch-Site decides when it is there.
            (&[("sec-fetch-site", "same-origin")], true),
            (&[("sec-fetch-site", "none")], true),
            (&[("sec-fetch-site", "same-site")], false),
            (&[("sec-fetch-site", "cross-site")], false),
            (&[("sec-fetch-site", "Same-Origin")], false),
            (
                &[
                    ("sec-fetch-site", "cross-site"),
                    ("origin", "https://api.example.com"),
                    ("host", "api.example.com"),
                ],
                false,
            ),
            // 3. Otherwise Origin against Host.
            (
                &[
                    ("origin", "https://api.example.com"),
                    ("host", "api.example.com"),
                ],
                true,
            ),
            (
                &[
                    ("origin", "https://API.example.com"),
                    ("host", "api.example.com"),
                ],
                true,
            ),
            (
                &[
                    ("origin", "https://api.example.com:8443"),
                    ("host", "api.example.com"),
                ],
                false,
            ),
            (
                &[
                    ("origin", "https://attacker.example"),
                    ("host", "api.example.com"),
                ],
                false,
            ),
            (&[("origin", "null"), ("host", "api.example.com")], false),
            (&[("origin", "https://api.example.com")], false),
            // 1. A trusted origin passes, whatever else is sent.
            (
                &[
                    ("origin", "https://app.example.com"),
                    ("host", "api.example.com"),
                ],
                true,
            ),
            (
                &[
                    ("origin", "https://app.example.com"),
                    ("sec-fetch-site", "cross-site"),
                ],
                true,
            ),
            (
                &[
                    ("origin", "https://app.example.com.attacker.example"),
                    ("host", "api.example.com"),
                ],
                false,
            ),
        ];
        for (pairs, expected) in cases {
            assert_eq!(same_origin(&with(pairs), &trusted), expected, "{pairs:?}");
        }
        // Two values of one header are refused rather than guessed at.
        let twice = with(&[
            ("origin", "https://app.example.com"),
            ("origin", "https://attacker.example"),
        ]);
        assert!(!same_origin(&twice, &trusted));
        let twice = with(&[
            ("sec-fetch-site", "same-origin"),
            ("sec-fetch-site", "cross-site"),
        ]);
        assert!(!same_origin(&twice, &[]));
        // Without a Host header, as in HTTP/2, the target's authority counts.
        let origin = with(&[("origin", "https://api.example.com")]);
        assert!(same_origin_at(&origin, Some("api.example.com"), &[]));
        assert!(!same_origin_at(&origin, Some("other.example.com"), &[]));
        let both = with(&[
            ("origin", "https://api.example.com"),
            ("host", "other.example.com"),
        ]);
        assert!(!same_origin_at(&both, Some("api.example.com"), &[]));
        for (origin, valid) in [
            ("https://app.example.com", true),
            ("http://localhost:3000", true),
            ("https://[::1]:8443", true),
            ("https://app.example.com/", false),
            ("app.example.com", false),
            ("https://", false),
            ("ftp://app.example.com", false),
            ("https://a b", false),
        ] {
            assert_eq!(is_origin(origin), valid, "{origin}");
        }
    }
}

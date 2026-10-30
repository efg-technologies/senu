//! Credentials and `Authorization` header parsing.
//!
//! Secrets are compared with [`subtle::ConstantTimeEq`]. Senu makes no timing
//! guarantee beyond what that crate provides; in particular a length
//! mismatch is detected before the byte comparison.

use std::fmt;

use base64::Engine as _;
use subtle::ConstantTimeEq;

/// Longest `Authorization` header value that is parsed at all.
pub const MAX_AUTHORIZATION_BYTES: usize = 1024;

/// A configured credential. It never appears in `Debug` output and has no
/// `Display`, so it cannot leak through logs or error messages by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Only configuration validation creates secrets.
    pub(crate) fn new(value: String) -> Self {
        Self(value)
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
    Basic { user: Vec<u8>, password: Vec<u8> },
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
        Secret::new(value.to_owned())
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
}

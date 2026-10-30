//! Environment configuration, validated before any port is bound or any
//! stdin is read.
//!
//! The functions here take the environment as an argument so they stay pure
//! and testable. Error messages name the variable but never print its value.

use std::ffi::OsString;
use std::fmt;
use std::net::SocketAddr;

use crate::auth::{Secret, is_vchar};

pub const BIND: &str = "SENU_BIND";
pub const WEB_USER: &str = "SENU_WEB_USER";
pub const WEB_PASSWORD: &str = "SENU_WEB_PASSWORD";
pub const API_TOKEN: &str = "SENU_API_TOKEN";
pub const MCP_TOKEN: &str = "SENU_MCP_TOKEN";

/// The address used when `SENU_BIND` is not set.
pub const DEFAULT_BIND: &str = "127.0.0.1:3000";

/// Configuration for `senu serve`.
#[derive(Debug)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    pub web_user: Secret,
    pub web_password: Secret,
    pub api_token: Secret,
}

/// Configuration for `senu mcp --enable-mcp`.
#[derive(Debug)]
pub struct McpConfig {
    pub token: Secret,
}

/// What is wrong with a variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Missing,
    Empty,
    Malformed,
    Unknown,
    Repeated,
    SameValueAs(&'static str),
}

/// A configuration failure. It names the variable and never holds its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub variable: String,
    pub problem: Problem,
}

impl ConfigError {
    fn new(variable: impl Into<String>, problem: Problem) -> Self {
        Self {
            variable: variable.into(),
            problem,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = &self.variable;
        match &self.problem {
            Problem::Missing => write!(f, "senu: {name} is not set"),
            Problem::Empty => write!(f, "senu: {name} is set but empty"),
            Problem::Malformed => write!(f, "senu: {name} is malformed: {}", rule(name)),
            Problem::Unknown => write!(
                f,
                "senu: {name} is an unknown variable; roles and tenants cannot be configured"
            ),
            Problem::Repeated => write!(f, "senu: {name} is set more than once"),
            Problem::SameValueAs(other) => {
                write!(f, "senu: {name} must not have the same value as {other}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn rule(variable: &str) -> &'static str {
    match variable {
        BIND => "expected a loopback IP address and port, such as 127.0.0.1:3000",
        WEB_USER => "expected 1 to 64 visible ASCII characters (0x21-0x7E) without ':'",
        _ => "expected 1 to 256 visible ASCII characters (0x21-0x7E)",
    }
}

/// Every variable Senu reads, in the order they are validated.
const KNOWN: [&str; 5] = [BIND, WEB_USER, WEB_PASSWORD, API_TOKEN, MCP_TOKEN];
const MAX_USER_BYTES: usize = 64;
const MAX_SECRET_BYTES: usize = 256;

/// The validated values, before the mode decides which are required.
struct Validated {
    bind: SocketAddr,
    web_user: Option<Secret>,
    web_password: Option<Secret>,
    api_token: Option<Secret>,
    mcp_token: Option<Secret>,
}

/// Validates the environment for `senu serve`.
pub fn serve_config<I>(vars: I) -> Result<ServeConfig, ConfigError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let values = validate(vars)?;
    Ok(ServeConfig {
        bind: values.bind,
        web_user: required(WEB_USER, values.web_user)?,
        web_password: required(WEB_PASSWORD, values.web_password)?,
        api_token: required(API_TOKEN, values.api_token)?,
    })
}

/// Validates the environment for `senu mcp --enable-mcp`.
pub fn mcp_config<I>(vars: I) -> Result<McpConfig, ConfigError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let values = validate(vars)?;
    Ok(McpConfig {
        token: required(MCP_TOKEN, values.mcp_token)?,
    })
}

fn required(name: &'static str, value: Option<Secret>) -> Result<Secret, ConfigError> {
    value.ok_or_else(|| ConfigError::new(name, Problem::Missing))
}

/// Checks every `SENU_*` variable that is present, whichever mode runs, so a
/// typo or a reused credential is never silently ignored.
fn validate<I>(vars: I) -> Result<Validated, ConfigError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let raw = collect(vars)?;
    let [bind, user, password, api, mcp] = raw;
    let validated = Validated {
        bind: bind_address(bind)?,
        web_user: credential(WEB_USER, user, MAX_USER_BYTES, true)?,
        web_password: credential(WEB_PASSWORD, password, MAX_SECRET_BYTES, false)?,
        api_token: credential(API_TOKEN, api, MAX_SECRET_BYTES, false)?,
        mcp_token: credential(MCP_TOKEN, mcp, MAX_SECRET_BYTES, false)?,
    };
    let credentials = [
        (WEB_USER, &validated.web_user),
        (WEB_PASSWORD, &validated.web_password),
        (API_TOKEN, &validated.api_token),
        (MCP_TOKEN, &validated.mcp_token),
    ];
    for (i, (name, value)) in credentials.iter().enumerate() {
        for (earlier, earlier_value) in &credentials[..i] {
            if let (Some(value), Some(earlier_value)) = (value, earlier_value)
                && value == earlier_value
            {
                return Err(ConfigError::new(*name, Problem::SameValueAs(earlier)));
            }
        }
    }
    Ok(validated)
}

/// Picks out the known variables and rejects unknown or repeated `SENU_*`
/// names. Other variables are ignored.
fn collect<I>(vars: I) -> Result<[Option<OsString>; 5], ConfigError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut values: [Option<OsString>; 5] = Default::default();
    let mut unknown = std::collections::BTreeSet::new();
    let mut repeated = None;
    for (name, value) in vars {
        let name = name.to_string_lossy();
        if !name.starts_with("SENU_") {
            continue;
        }
        match KNOWN.iter().position(|known| *known == name) {
            Some(index) => {
                if values[index].replace(value).is_some() {
                    repeated.get_or_insert(KNOWN[index]);
                }
            }
            None => {
                unknown.insert(printable_name(&name));
            }
        }
    }
    if let Some(name) = unknown.into_iter().next() {
        return Err(ConfigError::new(name, Problem::Unknown));
    }
    if let Some(name) = repeated {
        return Err(ConfigError::new(name, Problem::Repeated));
    }
    Ok(values)
}

/// Variable names are not secret, but they are still made safe to print.
fn printable_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_graphic() { c } else { '?' })
        .collect()
}

fn bind_address(raw: Option<OsString>) -> Result<SocketAddr, ConfigError> {
    let Some(raw) = raw else {
        return Ok(SocketAddr::from(([127, 0, 0, 1], 3000)));
    };
    if raw.is_empty() {
        return Err(ConfigError::new(BIND, Problem::Empty));
    }
    match raw.to_str().map(str::parse::<SocketAddr>) {
        Some(Ok(addr)) if addr.ip().is_loopback() => Ok(addr),
        _ => Err(ConfigError::new(BIND, Problem::Malformed)),
    }
}

fn credential(
    name: &'static str,
    raw: Option<OsString>,
    max_bytes: usize,
    forbid_colon: bool,
) -> Result<Option<Secret>, ConfigError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Err(ConfigError::new(name, Problem::Empty));
    }
    let malformed = || ConfigError::new(name, Problem::Malformed);
    let text = raw.into_string().map_err(|_| malformed())?;
    let valid = text.len() <= max_bytes
        && text.bytes().all(is_vchar)
        && !(forbid_colon && text.contains(':'));
    if valid {
        Ok(Some(Secret::new(text)))
    } else {
        Err(malformed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "demo";
    const PASSWORD: &str = "web-password-sentinel";
    const API: &str = "api-token-sentinel";
    const MCP: &str = "mcp-token-sentinel";

    fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    fn serve_env() -> Vec<(&'static str, &'static str)> {
        vec![
            (WEB_USER, USER),
            (WEB_PASSWORD, PASSWORD),
            (API_TOKEN, API),
            ("PATH", "/usr/bin"),
            ("HOME", "/nonexistent"),
        ]
    }

    fn with(
        base: &[(&'static str, &'static str)],
        name: &'static str,
        value: &'static str,
    ) -> Vec<(&'static str, &'static str)> {
        let mut pairs: Vec<_> = base.iter().copied().filter(|(k, _)| *k != name).collect();
        pairs.push((name, value));
        pairs
    }

    fn without(
        base: &[(&'static str, &'static str)],
        name: &str,
    ) -> Vec<(&'static str, &'static str)> {
        base.iter().copied().filter(|(k, _)| *k != name).collect()
    }

    fn serve_err(pairs: &[(&str, &str)]) -> ConfigError {
        serve_config(env(pairs)).expect_err("configuration should be rejected")
    }

    fn mcp_err(pairs: &[(&str, &str)]) -> ConfigError {
        mcp_config(env(pairs)).expect_err("configuration should be rejected")
    }

    fn assert_problem(err: &ConfigError, variable: &str, problem: Problem) {
        assert_eq!(err.variable, variable, "{err:?}");
        assert_eq!(err.problem, problem, "{err:?}");
    }

    #[test]
    fn valid_serve_configuration_uses_the_default_bind() {
        let config = serve_config(env(&serve_env())).expect("valid configuration");
        assert_eq!(config.bind, DEFAULT_BIND.parse().unwrap());
        assert!(config.web_user.matches(USER.as_bytes()));
        assert!(config.web_password.matches(PASSWORD.as_bytes()));
        assert!(config.api_token.matches(API.as_bytes()));
    }

    #[test]
    fn loopback_binds_are_accepted_including_port_zero() {
        for bind in ["127.0.0.1:0", "127.0.0.1:8080", "127.1.2.3:3000", "[::1]:0"] {
            let config = serve_config(env(&with(&serve_env(), BIND, bind))).expect(bind);
            assert_eq!(config.bind, bind.parse().unwrap());
        }
    }

    #[test]
    fn non_loopback_or_malformed_binds_are_rejected() {
        for bind in [
            "0.0.0.0:3000",
            "[::]:3000",
            "192.168.1.10:3000",
            "[::ffff:127.0.0.1]:3000",
            "localhost:3000",
            "127.0.0.1",
            "127.0.0.1:65536",
            "http://127.0.0.1:3000",
        ] {
            let err = serve_err(&with(&serve_env(), BIND, bind));
            assert_problem(&err, BIND, Problem::Malformed);
        }
        assert_problem(
            &serve_err(&with(&serve_env(), BIND, "")),
            BIND,
            Problem::Empty,
        );
    }

    #[test]
    fn each_serve_variable_is_required() {
        for name in [WEB_USER, WEB_PASSWORD, API_TOKEN] {
            assert_problem(
                &serve_err(&without(&serve_env(), name)),
                name,
                Problem::Missing,
            );
        }
    }

    #[test]
    fn each_serve_variable_must_be_non_empty() {
        for name in [WEB_USER, WEB_PASSWORD, API_TOKEN] {
            assert_problem(
                &serve_err(&with(&serve_env(), name, "")),
                name,
                Problem::Empty,
            );
        }
    }

    #[test]
    fn length_limits_are_inclusive() {
        let user64 = "u".repeat(64);
        let user65 = "u".repeat(65);
        let secret256 = "s".repeat(256);
        let secret257 = "s".repeat(257);
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };

        assert!(serve_config(env(&with(&serve_env(), WEB_USER, leak(user64)))).is_ok());
        assert_problem(
            &serve_err(&with(&serve_env(), WEB_USER, leak(user65))),
            WEB_USER,
            Problem::Malformed,
        );
        for name in [WEB_PASSWORD, API_TOKEN] {
            assert!(serve_config(env(&with(&serve_env(), name, leak(secret256.clone())))).is_ok());
            assert_problem(
                &serve_err(&with(&serve_env(), name, leak(secret257.clone()))),
                name,
                Problem::Malformed,
            );
        }
    }

    #[test]
    fn only_visible_ascii_is_accepted() {
        for value in [
            "has space",
            "tab\there",
            "new\nline",
            "del\u{7f}",
            "café",
            "\u{0}",
        ] {
            for name in [WEB_USER, WEB_PASSWORD, API_TOKEN] {
                assert_problem(
                    &serve_err(&with(&serve_env(), name, value)),
                    name,
                    Problem::Malformed,
                );
            }
        }
    }

    #[test]
    fn user_name_must_not_contain_a_colon_but_secrets_may() {
        assert_problem(
            &serve_err(&with(&serve_env(), WEB_USER, "de:mo")),
            WEB_USER,
            Problem::Malformed,
        );
        assert!(serve_config(env(&with(&serve_env(), WEB_PASSWORD, "pa:ss"))).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_values_are_malformed() {
        use std::os::unix::ffi::OsStringExt;
        let mut vars = env(&without(&serve_env(), API_TOKEN));
        vars.push((
            OsString::from(API_TOKEN),
            OsString::from_vec(vec![b'a', 0xff]),
        ));
        let err = serve_config(vars).unwrap_err();
        assert_problem(&err, API_TOKEN, Problem::Malformed);
    }

    #[test]
    fn unknown_senu_variables_are_rejected() {
        let err = serve_err(&with(&serve_env(), "SENU_ROLE", "admin"));
        assert_problem(&err, "SENU_ROLE", Problem::Unknown);
        let err = serve_err(&with(&serve_env(), "SENU_TENANT", "globex"));
        assert_problem(&err, "SENU_TENANT", Problem::Unknown);
        let err = mcp_err(&[(MCP_TOKEN, MCP), ("SENU_", "x")]);
        assert_problem(&err, "SENU_", Problem::Unknown);
    }

    #[test]
    fn repeated_variables_are_rejected() {
        let mut pairs = serve_env();
        pairs.push((API_TOKEN, API));
        assert_problem(&serve_err(&pairs), API_TOKEN, Problem::Repeated);
    }

    #[test]
    fn credential_values_must_differ() {
        let err = serve_err(&with(&serve_env(), API_TOKEN, PASSWORD));
        assert_eq!(err.problem, Problem::SameValueAs(WEB_PASSWORD));
        assert_eq!(err.variable, API_TOKEN);

        let err = serve_err(&with(&serve_env(), WEB_PASSWORD, USER));
        assert_eq!(err.problem, Problem::SameValueAs(WEB_USER));

        let mut pairs = serve_env();
        pairs.push((MCP_TOKEN, API));
        let err = serve_err(&pairs);
        assert_problem(&err, MCP_TOKEN, Problem::SameValueAs(API_TOKEN));
    }

    #[test]
    fn valid_mcp_configuration_needs_only_the_mcp_token() {
        let config = mcp_config(env(&[(MCP_TOKEN, MCP)])).expect("valid configuration");
        assert!(config.token.matches(MCP.as_bytes()));
    }

    #[test]
    fn mcp_does_not_fall_back_to_the_api_token() {
        assert_problem(&mcp_err(&[(API_TOKEN, API)]), MCP_TOKEN, Problem::Missing);
        assert_problem(&mcp_err(&[]), MCP_TOKEN, Problem::Missing);
    }

    #[test]
    fn mcp_token_is_validated_and_kept_distinct() {
        assert_problem(&mcp_err(&[(MCP_TOKEN, "")]), MCP_TOKEN, Problem::Empty);
        assert_problem(
            &mcp_err(&[(MCP_TOKEN, "two words")]),
            MCP_TOKEN,
            Problem::Malformed,
        );
        let err = mcp_err(&[(MCP_TOKEN, MCP), (WEB_PASSWORD, MCP)]);
        assert_problem(&err, MCP_TOKEN, Problem::SameValueAs(WEB_PASSWORD));
    }

    #[test]
    fn messages_name_the_variable_and_never_the_value() {
        let sentinel = "sentinel-value-0123456789";
        let leak: &'static str = sentinel;
        let failures = [
            serve_err(&with(
                &serve_env(),
                API_TOKEN,
                "sentinel-value-0123456789 x",
            )),
            serve_err(&with(&serve_env(), WEB_USER, "sentinel-value-0123456789:")),
            serve_err(&with(&serve_env(), BIND, leak)),
            serve_err(&[(WEB_USER, USER), (WEB_PASSWORD, leak), (API_TOKEN, leak)]),
            mcp_err(&[(MCP_TOKEN, &"sentinel-value-0123456789".repeat(20))]),
        ];
        for err in failures {
            let message = err.to_string();
            assert!(message.contains(&err.variable), "{message}");
            assert!(message.starts_with("senu: "), "{message}");
            assert!(!message.contains(sentinel), "{message}");
            assert!(!format!("{err:?}").contains(sentinel));
        }
    }

    #[test]
    fn messages_describe_the_problem() {
        let missing = serve_err(&without(&serve_env(), API_TOKEN)).to_string();
        assert!(missing.contains("not set"), "{missing}");
        let unknown = serve_err(&with(&serve_env(), "SENU_ROLE", "x")).to_string();
        assert!(unknown.contains("unknown"), "{unknown}");
        let bind = serve_err(&with(&serve_env(), BIND, "0.0.0.0:1")).to_string();
        assert!(bind.contains("loopback"), "{bind}");
        let same = serve_err(&with(&serve_env(), API_TOKEN, PASSWORD)).to_string();
        assert!(
            same.contains(WEB_PASSWORD) && same.contains(API_TOKEN),
            "{same}"
        );
    }

    #[test]
    fn debug_output_of_configs_redacts_secrets() {
        let serve = serve_config(env(&serve_env())).unwrap();
        let text = format!("{serve:?}");
        for secret in [PASSWORD, API] {
            assert!(!text.contains(secret), "{text}");
        }
        let mcp = mcp_config(env(&[(MCP_TOKEN, MCP)])).unwrap();
        assert!(!format!("{mcp:?}").contains(MCP));
    }
}

//! Command-line parsing. This is a pure function over the argument list.

use std::ffi::OsString;
use std::fmt;

/// What the binary was asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Site,
    Mcp,
    Help,
}

/// Why the arguments were rejected. Both variants exit with status 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliError {
    /// Anything that is not one of the accepted forms.
    Usage,
    /// `senu mcp` without `--enable-mcp`.
    McpNotEnabled,
}

impl fmt::Display for CliError {
    /// The message never repeats the arguments, which could hold a secret
    /// pasted by mistake.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(
                f,
                "senu: unrecognized arguments\n\
                 usage: senu serve | senu site | senu mcp --enable-mcp | senu --help"
            ),
            Self::McpNotEnabled => write!(
                f,
                "senu: MCP is off by default; start it with `senu mcp --enable-mcp`"
            ),
        }
    }
}

/// The text printed by `senu --help`.
pub const HELP: &str = "\
senu: experimental seed that serves one read-only project list as HTML, JSON and MCP,
and a small site of Markdown articles

Usage:
  senu serve              serve HTML and JSON on a loopback address
  senu site               serve the articles in content/ as HTML on a loopback address
  senu mcp --enable-mcp   serve MCP over stdio (off unless this flag is given)
  senu --help             show this help

Environment:
  SENU_BIND           serve, site: loopback address and port (default 127.0.0.1:3000)
  SENU_WEB_USER       serve: Basic user name for /projects
  SENU_WEB_PASSWORD   serve: Basic password for /projects
  SENU_API_TOKEN      serve: Bearer token for /api/projects
  SENU_MCP_TOKEN      mcp: token that must be set before the stdio server starts

This is a local demo, not production software. See README.md.
";

/// Parses the arguments after the program name.
pub fn parse_args(args: &[OsString]) -> Result<Command, CliError> {
    let mut words = Vec::with_capacity(args.len());
    for arg in args {
        words.push(arg.to_str().ok_or(CliError::Usage)?);
    }
    match words.as_slice() {
        ["serve"] => Ok(Command::Serve),
        ["site"] => Ok(Command::Site),
        ["mcp", "--enable-mcp"] => Ok(Command::Mcp),
        ["mcp"] => Err(CliError::McpNotEnabled),
        ["--help"] => Ok(Command::Help),
        _ => Err(CliError::Usage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, CliError> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_args(&args)
    }

    #[test]
    fn accepts_the_four_documented_forms() {
        assert_eq!(parse(&["serve"]), Ok(Command::Serve));
        assert_eq!(parse(&["site"]), Ok(Command::Site));
        assert_eq!(parse(&["mcp", "--enable-mcp"]), Ok(Command::Mcp));
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
    }

    #[test]
    fn no_arguments_is_a_usage_error() {
        assert_eq!(parse(&[]), Err(CliError::Usage));
    }

    #[test]
    fn bare_mcp_explains_that_mcp_is_off_by_default() {
        assert_eq!(parse(&["mcp"]), Err(CliError::McpNotEnabled));
        let message = CliError::McpNotEnabled.to_string();
        assert!(message.contains("off by default"));
        assert!(message.contains("senu mcp --enable-mcp"));
    }

    #[test]
    fn enable_mcp_is_rejected_outside_mcp() {
        assert_eq!(parse(&["serve", "--enable-mcp"]), Err(CliError::Usage));
        assert_eq!(parse(&["--enable-mcp"]), Err(CliError::Usage));
        assert_eq!(parse(&["--enable-mcp", "mcp"]), Err(CliError::Usage));
    }

    #[test]
    fn other_arguments_are_usage_errors() {
        let cases: &[&[&str]] = &[
            &["start"],
            &["help"],
            &["-h"],
            &["--version"],
            &["SERVE"],
            &[""],
            &["serve", "serve"],
            &["site", "extra"],
            &["site", "--enable-mcp"],
            &["SITE"],
            &["serve", "--help"],
            &["--help", "serve"],
            &["mcp", "--enable-mcp", "extra"],
            &["mcp", "--enable-mcp=true"],
            &["mcp", "--ENABLE-MCP"],
        ];
        for args in cases {
            assert_eq!(parse(args), Err(CliError::Usage), "args {args:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_arguments_are_usage_errors() {
        use std::os::unix::ffi::OsStringExt;
        let args = vec![OsString::from_vec(vec![0x73, 0xff])];
        assert_eq!(parse_args(&args), Err(CliError::Usage));
    }

    #[test]
    fn errors_do_not_repeat_the_arguments() {
        let sentinel = "sentinel-secret-in-argv";
        let err = parse(&[sentinel]).unwrap_err();
        assert!(!err.to_string().contains(sentinel));
        assert!(err.to_string().contains("usage: senu serve"));
    }

    #[test]
    fn help_mentions_every_command_and_variable() {
        for needle in [
            "senu serve",
            "senu site",
            "senu mcp --enable-mcp",
            "senu --help",
            "SENU_BIND",
            "SENU_WEB_USER",
            "SENU_WEB_PASSWORD",
            "SENU_API_TOKEN",
            "SENU_MCP_TOKEN",
            "not production",
        ] {
            assert!(HELP.contains(needle), "help lacks {needle}");
        }
    }
}

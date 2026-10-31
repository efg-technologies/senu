//! Declared operations: permissions, principals, errors and the call order.
//!
//! This module knows nothing about HTTP or MCP. Adapters authenticate a
//! caller, pick the matching server-side [`Principal`], and pass the decoded
//! arguments to [`Operation::call`] unchanged. The demo's operations live in
//! [`crate::demo`]; their earlier paths here are kept as re-exports.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::catalogue::TenantId;

pub use crate::demo::ProjectSummary;
pub use crate::demo::get_project::{MAX_ID_BYTES, get_project};
pub use crate::demo::list_projects::{MAX_SEARCH_CHARS, ProjectList, list_projects};

/// What a principal may do. The application declares each permission as a
/// constant, such as [`crate::demo::LIST_PROJECTS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permission(&'static str);

impl Permission {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub fn name(self) -> &'static str {
        self.0
    }
}

/// The authenticated caller, created only by server code.
///
/// Requests never carry a tenant or permissions; adapters map a verified
/// credential to one of these values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    name: &'static str,
    tenant: TenantId,
    permissions: &'static [Permission],
}

impl Principal {
    pub const fn new(
        name: &'static str,
        tenant: TenantId,
        permissions: &'static [Permission],
    ) -> Self {
        Self {
            name,
            tenant,
            permissions,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// A failure that is safe to show to the caller.
///
/// `Invalid` is an operation's own 400 error. `InvalidSearch` and
/// `InvalidId` are the demo's earlier variants, kept with their mappings
/// for compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpError {
    Forbidden,
    InvalidRequest,
    InvalidSearch,
    InvalidId,
    NotFound,
    /// A stable `code` for JSON and MCP and a `phrase` for HTML. Both are
    /// fixed strings, so neither can repeat the input.
    Invalid {
        code: &'static str,
        phrase: &'static str,
    },
}

impl OpError {
    /// The stable public error code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::InvalidRequest => "invalid_request",
            Self::InvalidSearch => "invalid_search",
            Self::InvalidId => "invalid_id",
            Self::NotFound => "not_found",
            Self::Invalid { code, .. } => code,
        }
    }
}

/// An operation's arguments: the decoded query string or MCP `arguments`.
pub type Args = Map<String, Value>;

/// One declared parameter. Parameters are strings, as query strings are; an
/// operation parses anything else itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
}

/// One operation of the application: its metadata and its function.
///
/// `S` is the application's state and `T` its result. Declaring an
/// operation exposes nothing; a route or a tool exists only where the
/// application lists it.
pub struct Operation<S, T> {
    pub name: &'static str,
    pub description: &'static str,
    pub permission: Permission,
    pub params: &'static [Param],
    /// The operation's function. [`Operation::call`] is the authorized entry
    /// point: it runs this only after the permission and the parameter
    /// envelope were checked, and every adapter goes through it. The field is
    /// public, so whoever holds the `Operation` can call `run` directly even
    /// when the function's name is private; that privacy is no authorization
    /// boundary. A direct call must check the permission itself, in the
    /// caller or in the function.
    pub run: fn(&S, &Principal, &Args) -> Result<T, OpError>,
}

impl<S, T> Operation<S, T> {
    /// Checks the declared permission, then the declared parameters (known
    /// keys only, required ones present, every value a string and never
    /// null), and only then calls `run`.
    ///
    /// Denial therefore precedes any input validation. Further policy inside
    /// `run` happens after the envelope.
    pub fn call(&self, state: &S, principal: &Principal, args: &Args) -> Result<T, OpError> {
        if !principal.has(self.permission) {
            return Err(OpError::Forbidden);
        }
        let declared = |key: &str| self.params.iter().any(|param| param.name == key);
        let known = args
            .iter()
            .all(|(key, value)| value.is_string() && declared(key));
        let complete = self
            .params
            .iter()
            .all(|param| !param.required || args.contains_key(param.name));
        if !(known && complete) {
            return Err(OpError::InvalidRequest);
        }
        (self.run)(state, principal, args)
    }

    /// Checks the metadata before anything is exposed: snake_case names of
    /// at most 64 bytes for the operation, its permission and each of its
    /// unique parameters, and a description for each.
    pub fn check(&self) -> Result<(), InvalidDeclaration> {
        let invalid = |reason| {
            Err(InvalidDeclaration {
                operation: self.name,
                reason,
            })
        };
        if !is_snake_case(self.name) {
            return invalid("the name is not snake_case of at most 64 bytes");
        }
        if !is_snake_case(self.permission.name()) {
            return invalid("the permission is not snake_case of at most 64 bytes");
        }
        if self.description.trim().is_empty() {
            return invalid("the description is empty");
        }
        for (i, param) in self.params.iter().enumerate() {
            if !is_snake_case(param.name) {
                return invalid("a parameter name is not snake_case of at most 64 bytes");
            }
            if param.description.trim().is_empty() {
                return invalid("a parameter has no description");
            }
            if self.params[..i]
                .iter()
                .any(|other| other.name == param.name)
            {
                return invalid("a parameter is declared twice");
            }
        }
        Ok(())
    }
}

/// An operation whose metadata Senu refuses at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDeclaration {
    pub operation: &'static str,
    pub reason: &'static str,
}

impl std::fmt::Display for InvalidDeclaration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation {:?}: {}", self.operation, self.reason)
    }
}

fn is_snake_case(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Decodes `args` into an operation's own argument struct, which should
/// use `#[serde(deny_unknown_fields)]`. Any mismatch is `invalid_request`.
pub fn decode<'a, A: Deserialize<'a>>(args: &'a Args) -> Result<A, OpError> {
    A::deserialize(args).map_err(|_| OpError::InvalidRequest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(OpError::Forbidden.code(), "forbidden");
        assert_eq!(OpError::InvalidRequest.code(), "invalid_request");
        assert_eq!(OpError::InvalidSearch.code(), "invalid_search");
    }
}

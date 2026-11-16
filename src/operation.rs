//! Declared operations: permissions, principals, errors and the call order.
//!
//! This module knows nothing about HTTP or MCP. Adapters authenticate a
//! caller, obtain the matching server-side [`Principal`], and pass the
//! decoded arguments to [`Operation::call`] unchanged.

use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;

use serde::Deserialize;
use serde_json::{Map, Value};

#[cfg(feature = "demo")]
pub use crate::demo::ProjectSummary;
#[cfg(feature = "demo")]
pub use crate::demo::get_project::{MAX_ID_BYTES, get_project};
#[cfg(feature = "demo")]
pub use crate::demo::list_projects::{MAX_SEARCH_CHARS, ProjectList, list_projects};

/// The tenant a principal belongs to.
///
/// A tenant never comes from a request: server code attaches it to the
/// [`Principal`], and operations read it from there to scope what they
/// return. An application with one tenant uses one constant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TenantId(Cow<'static, str>);

impl TenantId {
    /// A tenant known at compile time.
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// A tenant loaded at run time, for example with a user record.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    /// The identifier as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a principal may do. The application declares each permission as a
/// constant, such as `const VIEW_STOCK: Permission = Permission::new("view_stock");`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permission(&'static str);

impl Permission {
    /// Declares a permission. The name must be snake_case of at most 64
    /// bytes; [`Operation::check`] refuses anything else at startup.
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// The permission's name.
    pub fn name(self) -> &'static str {
        self.0
    }
}

/// The authenticated caller, created only by server code.
///
/// Requests never carry a tenant or permissions. An
/// [`Authenticate`](crate::auth::Authenticate) implementation maps a
/// verified credential to one of these values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    name: Cow<'static, str>,
    tenant: TenantId,
    permissions: Cow<'static, [Permission]>,
}

impl Principal {
    /// A principal known at compile time, such as a service account.
    pub const fn new(
        name: &'static str,
        tenant: TenantId,
        permissions: &'static [Permission],
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            tenant,
            permissions: Cow::Borrowed(permissions),
        }
    }

    /// A principal built at run time, for example from a user record whose
    /// roles the application has mapped to its permission constants.
    pub fn owned(
        name: impl Into<String>,
        tenant: TenantId,
        permissions: impl Into<Vec<Permission>>,
    ) -> Self {
        Self {
            name: Cow::Owned(name.into()),
            tenant,
            permissions: Cow::Owned(permissions.into()),
        }
    }

    /// A name for the application's own use, such as audit logging. Senu
    /// never sends it to the caller.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The tenant whose data this principal may see.
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    /// Whether the principal holds `permission`.
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// A failure that is safe to show to the caller.
///
/// Every variant maps to a fixed status, code and phrase, so an error can
/// never repeat the input or a secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpError {
    /// The principal lacks the operation's permission (403).
    Forbidden,
    /// The arguments do not fit the declared parameters (400).
    InvalidRequest,
    /// Nothing matches, or it belongs to another tenant (404).
    NotFound,
    /// The operation could not finish, for example because a backend
    /// failed (500). It carries no detail; log the cause before returning
    /// it.
    Internal,
    /// The request is well-formed but cannot be done now, such as deleting
    /// something that is in use (409). Like `Invalid`, it has a stable
    /// `code` and a `phrase`, both fixed strings.
    Conflict {
        /// The machine-readable code, such as `item_deployed`.
        code: &'static str,
        /// The plain-text body of the HTML surface.
        phrase: &'static str,
    },
    /// The operation's own 400 error: a stable `code` for JSON and MCP and
    /// a `phrase` for HTML. Both are fixed strings, so neither can repeat
    /// the input.
    Invalid {
        /// The machine-readable code, such as `invalid_sku`.
        code: &'static str,
        /// The plain-text body of the HTML surface, such as `Invalid SKU`.
        phrase: &'static str,
    },
}

impl OpError {
    /// The stable public error code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Internal => "internal",
            Self::Invalid { code, .. } | Self::Conflict { code, .. } => code,
        }
    }
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for OpError {}

/// An operation's arguments: the decoded query string or MCP `arguments`.
pub type Args = Map<String, Value>;

/// The JSON type of a parameter's value. A read takes strings only,
/// because a query string carries nothing else; a write's body may also
/// carry numbers and booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParamKind {
    /// A JSON string.
    String,
    /// A JSON number without a fraction that fits `i64`.
    Integer,
    /// Any JSON number.
    Number,
    /// `true` or `false`.
    Boolean,
}

impl ParamKind {
    /// The JSON Schema `type`.
    pub fn json_type(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    /// Whether `value` is of this kind. Null, arrays and objects never are.
    pub fn admits(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Integer => value.is_i64(),
            Self::Number => value.is_number(),
            Self::Boolean => value.is_boolean(),
        }
    }
}

/// One declared parameter: a named scalar that the caller may send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    /// The key in the query string, the JSON body and the tool's arguments.
    pub name: &'static str,
    /// What the value means, shown in the tool's JSON Schema.
    pub description: &'static str,
    /// Whether a call without it is `invalid_request`.
    pub required: bool,
    /// The value's JSON type.
    pub kind: ParamKind,
}

impl Param {
    /// A string parameter that every call must carry.
    pub const fn required(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            required: true,
            kind: ParamKind::String,
        }
    }

    /// A string parameter that a call may omit.
    pub const fn optional(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            required: false,
            kind: ParamKind::String,
        }
    }

    /// The same parameter as a whole number, for a write.
    pub const fn integer(mut self) -> Self {
        self.kind = ParamKind::Integer;
        self
    }

    /// The same parameter as a number, for a write.
    pub const fn number(mut self) -> Self {
        self.kind = ParamKind::Number;
        self
    }

    /// The same parameter as `true` or `false`, for a write.
    pub const fn boolean(mut self) -> Self {
        self.kind = ParamKind::Boolean;
        self
    }
}

/// Whether an operation changes state.
///
/// A read is served by `GET` and marked read-only for assistants. A write
/// is served by `POST` with a JSON body, behind the cross-site check, and
/// its two hints become the MCP tool's annotations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Changes nothing.
    Read,
    /// Changes state.
    Write {
        /// Whether it can remove or overwrite something, as an update or a
        /// delete does. A pure addition is not destructive.
        destructive: bool,
        /// Whether a second identical call changes nothing more.
        idempotent: bool,
    },
}

impl Effect {
    /// A write that only adds, such as a create.
    pub const CREATE: Self = Self::Write {
        destructive: false,
        idempotent: false,
    };
    /// A write that overwrites, such as an update: repeating it is harmless.
    pub const UPDATE: Self = Self::Write {
        destructive: true,
        idempotent: true,
    };
    /// A write that removes: repeating it is harmless.
    pub const DELETE: Self = Self::Write {
        destructive: true,
        idempotent: true,
    };

    /// Whether this is a write.
    pub const fn is_write(self) -> bool {
        matches!(self, Self::Write { .. })
    }
}

/// The future of an asynchronous operation.
pub type OpFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, OpError>> + Send + 'a>>;

/// An operation's function.
///
/// `Sync` is for pure work on data already in memory; it runs on the async
/// executor, so it must not block. `Async` is for anything that waits: a
/// database, another service, or blocking work moved to
/// `tokio::task::spawn_blocking`.
///
/// ```
/// # use senu::operation::{Args, OpError, OpFuture, Principal, Run};
/// struct Db;
/// async fn count(_db: &Db, _caller: &Principal, _args: &Args) -> Result<u64, OpError> {
///     Ok(3)
/// }
/// const RUN: Run<Db, u64> = Run::Async(|db, caller, args| Box::pin(count(db, caller, args)));
/// ```
pub enum Run<S, T> {
    /// A plain function.
    Sync(fn(&S, &Principal, &Args) -> Result<T, OpError>),
    /// A function that returns a boxed future borrowing its arguments.
    Async(for<'a> fn(&'a S, &'a Principal, &'a Args) -> OpFuture<'a, T>),
}

impl<S, T> Clone for Run<S, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, T> Copy for Run<S, T> {}

/// One operation of the application: its metadata and its function.
///
/// `S` is the application's state and `T` its result. Declaring an
/// operation exposes nothing; a route or a tool exists only where the
/// application lists it.
pub struct Operation<S, T> {
    /// The snake_case name: the MCP tool's name.
    pub name: &'static str,
    /// One sentence for people and assistants: the MCP tool's description.
    pub description: &'static str,
    /// What the caller must hold. It is checked before anything else.
    pub permission: Permission,
    /// The parameters. Any other key is refused.
    pub params: &'static [Param],
    /// Whether it changes state. A read's parameters must all be strings.
    pub effect: Effect,
    /// The operation's function. [`Operation::call`] is the authorized entry
    /// point: it runs this only after the permission and the parameter
    /// envelope were checked, and every adapter goes through it. The field is
    /// public, so whoever holds the `Operation` can call `run` directly even
    /// when the function's name is private; that privacy is no authorization
    /// boundary. A direct call must check the permission itself, in the
    /// caller or in the function.
    pub run: Run<S, T>,
}

impl<S, T> Operation<S, T> {
    /// Checks the declared permission, then the declared parameters (known
    /// keys only, required ones present, every value a string and never
    /// null), and only then calls `run`.
    ///
    /// Denial therefore precedes any input validation. Further policy inside
    /// `run` happens after the envelope.
    pub async fn call(&self, state: &S, principal: &Principal, args: &Args) -> Result<T, OpError> {
        self.admit(principal, args)?;
        match self.run {
            Run::Sync(run) => run(state, principal, args),
            Run::Async(run) => run(state, principal, args).await,
        }
    }

    /// The permission check alone. The HTTP adapter runs it before it reads
    /// a write's body.
    pub fn authorize(&self, principal: &Principal) -> Result<(), OpError> {
        if principal.has(self.permission) {
            Ok(())
        } else {
            Err(OpError::Forbidden)
        }
    }

    /// The checks that [`Operation::call`] makes before the function: the
    /// permission, then the parameter envelope (known keys only, required
    /// ones present, every value of its declared kind).
    pub fn admit(&self, principal: &Principal, args: &Args) -> Result<(), OpError> {
        self.authorize(principal)?;
        let kind = |key: &str| {
            self.params
                .iter()
                .find(|param| param.name == key)
                .map(|param| param.kind)
        };
        let known = args
            .iter()
            .all(|(key, value)| kind(key).is_some_and(|kind| kind.admits(value)));
        let complete = self
            .params
            .iter()
            .all(|param| !param.required || args.contains_key(param.name));
        if known && complete {
            Ok(())
        } else {
            Err(OpError::InvalidRequest)
        }
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
            if self.effect == Effect::Read && param.kind != ParamKind::String {
                return invalid("a read's parameters must be strings; only a write has a body");
            }
        }
        Ok(())
    }
}

/// An operation whose metadata Senu refuses at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDeclaration {
    /// The operation's name as declared.
    pub operation: &'static str,
    /// What is wrong with it.
    pub reason: &'static str,
}

impl std::fmt::Display for InvalidDeclaration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation {:?}: {}", self.operation, self.reason)
    }
}

impl std::error::Error for InvalidDeclaration {}

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
        assert_eq!(OpError::NotFound.code(), "not_found");
        assert_eq!(OpError::Internal.code(), "internal");
        assert_eq!(OpError::Internal.to_string(), "internal");
    }

    #[test]
    fn principals_are_static_or_owned_and_compare_by_value() {
        const READ: Permission = Permission::new("read");
        const STATIC: Principal = Principal::new("ann", TenantId::from_static("t1"), &[READ]);
        let owned = Principal::owned(String::from("ann"), TenantId::new("t1"), vec![READ]);
        assert_eq!(STATIC, owned);
        assert_eq!(owned.name(), "ann");
        assert_eq!(owned.tenant().as_str(), "t1");
        assert!(owned.has(READ));
        assert!(!owned.has(Permission::new("write")));
        assert_ne!(TenantId::new("t1"), TenantId::from_static("t2"));
    }

    #[test]
    fn param_helpers_set_required_and_the_kind() {
        assert!(Param::required("id", "The id.").required);
        assert!(!Param::optional("q", "A search.").required);
        assert_eq!(Param::required("id", "The id.").kind, ParamKind::String);
        assert_eq!(
            Param::optional("n", "A count.").integer().kind,
            ParamKind::Integer
        );
        assert_eq!(
            Param::required("x", "A ratio.").number().kind,
            ParamKind::Number
        );
        assert_eq!(
            Param::required("on", "A flag.").boolean().kind,
            ParamKind::Boolean
        );
    }

    #[test]
    fn kinds_admit_only_their_own_json_type() {
        use serde_json::json;
        let cases = [
            (json!("x"), [true, false, false, false]),
            (json!(""), [true, false, false, false]),
            (json!(3), [false, true, true, false]),
            (json!(-3), [false, true, true, false]),
            (json!(1.5), [false, false, true, false]),
            (json!(1e300), [false, false, true, false]),
            (json!(18446744073709551615_u64), [false, false, true, false]),
            (json!(true), [false, false, false, true]),
            (json!(null), [false; 4]),
            (json!([1]), [false; 4]),
            (json!({"a": 1}), [false; 4]),
        ];
        let kinds = [
            ParamKind::String,
            ParamKind::Integer,
            ParamKind::Number,
            ParamKind::Boolean,
        ];
        for (value, expected) in cases {
            for (kind, admitted) in kinds.iter().zip(expected) {
                assert_eq!(kind.admits(&value), admitted, "{kind:?} {value}");
            }
        }
        assert_eq!(ParamKind::Integer.json_type(), "integer");
        assert!(Effect::CREATE.is_write() && Effect::DELETE.is_write());
        assert!(!Effect::Read.is_write());
        assert_eq!(
            OpError::Conflict {
                code: "in_use",
                phrase: "In use"
            }
            .code(),
            "in_use"
        );
    }
}

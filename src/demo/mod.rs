//! The demo application: its permissions, principals, route table and MCP
//! tools, with one file per operation.
//!
//! Everything in this directory is application code. Senu's adapters in
//! [`crate::http`] and [`crate::mcp`] expose an operation only where
//! [`endpoints`] or [`tools`] lists it.

pub mod get_project;
pub mod list_projects;

use serde::{Deserialize, Serialize};

use crate::catalogue::{Catalogue, TenantId};
use crate::http::Endpoints;
use crate::http::HttpError;
use crate::mcp::{self, McpTool};
use crate::operation::{OpError, Permission, Principal};

/// May list the projects of the principal's tenant.
pub const LIST_PROJECTS: Permission = Permission::new("list_projects");

/// May view one project of the principal's tenant.
pub const VIEW_PROJECT: Permission = Permission::new("view_project");

/// What every demo principal may do.
const DEMO_PERMISSIONS: &[Permission] = &[LIST_PROJECTS, VIEW_PROJECT];

/// The earlier names of the demo's permissions, kept for compatibility.
#[allow(non_upper_case_globals)]
impl Permission {
    /// The earlier name of [`LIST_PROJECTS`].
    pub const ListProjects: Self = LIST_PROJECTS;
    /// The earlier name of [`VIEW_PROJECT`].
    pub const ViewProject: Self = VIEW_PROJECT;
}

/// The demo's own 400 errors under the names they had as variants in 0.1.
/// The codes and phrases on the wire are unchanged.
#[allow(non_upper_case_globals)]
impl OpError {
    /// `list_projects`: the search is too long or holds control characters.
    pub const InvalidSearch: Self = Self::Invalid {
        code: "invalid_search",
        phrase: "Invalid search",
    };
    /// `get_project`: the id is not 1 to 32 characters of `a-z`, `0-9`, `-`.
    pub const InvalidId: Self = Self::Invalid {
        code: "invalid_id",
        phrase: "Invalid id",
    };
}

/// The HTTP forms of the demo's own errors, as in 0.1.
#[allow(non_upper_case_globals)]
impl HttpError {
    /// 400 `invalid_search`.
    pub const InvalidSearch: Self = Self::Invalid {
        code: "invalid_search",
        phrase: "Invalid search",
    };
    /// 400 `invalid_id`.
    pub const InvalidId: Self = Self::Invalid {
        code: "invalid_id",
        phrase: "Invalid id",
    };
}

impl Principal {
    /// The browser principal bound to Basic authentication.
    pub const fn demo_web() -> Self {
        Self::new("demo-web", TenantId::ACME, DEMO_PERMISSIONS)
    }

    /// The API principal bound to Bearer authentication.
    pub const fn demo_api() -> Self {
        Self::new("demo-api", TenantId::ACME, DEMO_PERMISSIONS)
    }

    /// The MCP principal bound to the stdio token.
    pub const fn demo_mcp() -> Self {
        Self::new("demo-mcp", TenantId::ACME, DEMO_PERMISSIONS)
    }
}

/// One project as callers see it. It carries no tenant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    /// The project's id, such as `acme-001`.
    pub id: String,
    /// The project's name, as stored.
    pub name: String,
}

/// The demo's routes: HTML behind Basic, JSON behind Bearer.
pub fn endpoints() -> Endpoints<Catalogue> {
    Endpoints::new()
        .html("/projects", &list_projects::OPERATION, list_projects::page)
        .api("/api/projects", &list_projects::OPERATION)
        .html("/project", &get_project::OPERATION, get_project::page)
        .api("/api/project", &get_project::OPERATION)
}

/// The demo's MCP tools, in this order.
pub fn tools() -> Vec<McpTool<Catalogue>> {
    vec![
        mcp::tool(&list_projects::OPERATION),
        mcp::tool(&get_project::OPERATION),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Args;
    use serde_json::Value;

    pub(super) const NOBODY: Principal = Principal::new("nobody", TenantId::ACME, &[]);

    pub(super) fn args(value: Value) -> Args {
        match value {
            Value::Object(map) => map,
            other => panic!("test arguments must be an object, got {other}"),
        }
    }

    #[test]
    fn demo_principals_share_the_acme_tenant_and_permission() {
        for (principal, name) in [
            (Principal::demo_web(), "demo-web"),
            (Principal::demo_api(), "demo-api"),
            (Principal::demo_mcp(), "demo-mcp"),
        ] {
            assert_eq!(principal.name(), name);
            assert_eq!(principal.tenant(), &TenantId::ACME);
            assert!(principal.has(Permission::ListProjects));
        }
        assert!(!NOBODY.has(Permission::ListProjects));
    }
}

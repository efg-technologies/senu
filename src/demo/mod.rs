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
use crate::mcp::{self, McpTool};
use crate::operation::{Permission, Principal};

/// May list the projects of the principal's tenant.
pub const LIST_PROJECTS: Permission = Permission::new("list_projects");

/// May view one project of the principal's tenant.
pub const VIEW_PROJECT: Permission = Permission::new("view_project");

/// What every demo principal may do.
const DEMO_PERMISSIONS: &[Permission] = &[LIST_PROJECTS, VIEW_PROJECT];

/// The earlier names of the demo's permissions, kept for compatibility.
#[allow(non_upper_case_globals)]
impl Permission {
    pub const ListProjects: Self = LIST_PROJECTS;
    pub const ViewProject: Self = VIEW_PROJECT;
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
    pub id: String,
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
            assert_eq!(principal.tenant(), TenantId::ACME);
            assert!(principal.has(Permission::ListProjects));
        }
        assert!(!NOBODY.has(Permission::ListProjects));
    }
}

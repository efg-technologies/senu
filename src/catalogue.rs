//! The fixed in-memory project catalogue.
//!
//! Tenants and projects are compiled into the binary. Nothing here is read
//! from a request, the environment or a file.

/// A tenant identifier.
///
/// The only values are the associated constants, so a tenant can never come
/// from client input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TenantId(&'static str);

impl TenantId {
    pub const ACME: Self = Self("acme");
    pub const GLOBEX: Self = Self("globex");

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// One project in the catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: &'static str,
    pub tenant: TenantId,
    pub name: &'static str,
}

/// A read-only set of projects across tenants.
#[derive(Debug)]
pub struct Catalogue {
    projects: &'static [Project],
}

const DEMO_PROJECTS: &[Project] = &[
    Project {
        id: "acme-001",
        tenant: TenantId::ACME,
        name: "Apollo Launch",
    },
    Project {
        id: "acme-002",
        tenant: TenantId::ACME,
        name: "Café Roadmap",
    },
    Project {
        id: "acme-003",
        tenant: TenantId::ACME,
        name: "<script>alert('x')</script> & \"Launch\" Notes",
    },
    Project {
        id: "globex-001",
        tenant: TenantId::GLOBEX,
        name: "Globex Launch",
    },
    Project {
        id: "globex-002",
        tenant: TenantId::GLOBEX,
        name: "Zephyr Migration",
    },
];

impl Catalogue {
    /// The deterministic demo catalogue from spec 0001.
    pub fn demo() -> Self {
        Self {
            projects: DEMO_PROJECTS,
        }
    }

    /// The projects that belong to `tenant`, in catalogue order.
    pub fn for_tenant(&self, tenant: TenantId) -> impl Iterator<Item = &Project> + '_ {
        self.projects.iter().filter(move |p| p.tenant == tenant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(catalogue: &Catalogue, tenant: TenantId) -> Vec<&'static str> {
        catalogue.for_tenant(tenant).map(|p| p.id).collect()
    }

    #[test]
    fn demo_catalogue_matches_the_spec_table() {
        let all: Vec<(&str, &str, &str)> = [TenantId::ACME, TenantId::GLOBEX]
            .into_iter()
            .flat_map(|t| Catalogue::demo().for_tenant(t).copied().collect::<Vec<_>>())
            .map(|p| (p.id, p.tenant.as_str(), p.name))
            .collect();
        assert_eq!(
            all,
            vec![
                ("acme-001", "acme", "Apollo Launch"),
                ("acme-002", "acme", "Café Roadmap"),
                (
                    "acme-003",
                    "acme",
                    "<script>alert('x')</script> & \"Launch\" Notes"
                ),
                ("globex-001", "globex", "Globex Launch"),
                ("globex-002", "globex", "Zephyr Migration"),
            ]
        );
    }

    #[test]
    fn for_tenant_returns_only_that_tenant() {
        let catalogue = Catalogue::demo();
        assert_eq!(
            ids(&catalogue, TenantId::ACME),
            ["acme-001", "acme-002", "acme-003"]
        );
        assert_eq!(
            ids(&catalogue, TenantId::GLOBEX),
            ["globex-001", "globex-002"]
        );
        for tenant in [TenantId::ACME, TenantId::GLOBEX] {
            assert!(catalogue.for_tenant(tenant).all(|p| p.tenant == tenant));
        }
    }

    #[test]
    fn ids_are_unique_and_prefixed_by_their_tenant() {
        let mut seen = std::collections::HashSet::new();
        for project in DEMO_PROJECTS {
            assert!(seen.insert(project.id), "duplicate id {}", project.id);
            let prefix = format!("{}-", project.tenant.as_str());
            assert!(
                project.id.starts_with(&prefix),
                "{} lacks {prefix}",
                project.id
            );
            assert!(!project.name.is_empty());
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn tenant_ids_are_fixed_strings() {
        assert_eq!(TenantId::ACME.as_str(), "acme");
        assert_eq!(TenantId::GLOBEX.as_str(), "globex");
        assert_ne!(TenantId::ACME, TenantId::GLOBEX);
    }
}

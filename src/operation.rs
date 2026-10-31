//! The shared operations, `list_projects` and `get_project`.
//!
//! This module knows nothing about HTTP or MCP. Adapters authenticate a
//! caller, pick the matching server-side [`Principal`], and pass the decoded
//! arguments here unchanged.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::catalogue::{Catalogue, TenantId};

/// Longest accepted search, counted in Unicode scalar values after trimming.
pub const MAX_SEARCH_CHARS: usize = 80;

/// Longest accepted project id, in bytes; an id is ASCII `a-z`, `0-9` and `-`.
pub const MAX_ID_BYTES: usize = 32;

/// What a principal may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    ListProjects,
    ViewProject,
}

/// What every demo principal may do.
const DEMO_PERMISSIONS: &[Permission] = &[Permission::ListProjects, Permission::ViewProject];

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

/// One project as callers see it. It carries no tenant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
}

/// The result of `list_projects`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectList {
    pub projects: Vec<ProjectSummary>,
}

/// A failure that is safe to show to the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpError {
    Forbidden,
    InvalidRequest,
    InvalidSearch,
    InvalidId,
    NotFound,
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
        }
    }
}

/// Lists the principal's projects, optionally filtered by `q`.
///
/// This is pure: it performs no I/O and reads no clock or global state.
pub fn list_projects(
    catalogue: &Catalogue,
    principal: &Principal,
    args: &Map<String, Value>,
) -> Result<ProjectList, OpError> {
    // 1. Authorize before looking at the input, so a caller without the
    //    permission learns nothing about what would have been valid.
    if !principal.has(Permission::ListProjects) {
        return Err(OpError::Forbidden);
    }

    // 2. Decode strictly: only an optional string `q`.
    let args = ListProjectsArgs::deserialize(args).map_err(|_| OpError::InvalidRequest)?;

    // 3. Normalize and bound the search.
    let needle = match args.q.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(q) => Some(checked_search(q)?.to_lowercase()),
    };

    // 4. and 5. Search only inside the principal's tenant, then sort by id.
    let mut projects: Vec<ProjectSummary> = catalogue
        .for_tenant(principal.tenant())
        .filter(|p| {
            needle
                .as_deref()
                .is_none_or(|needle| p.name.to_lowercase().contains(needle))
        })
        .map(|p| ProjectSummary {
            id: p.id.to_owned(),
            name: p.name.to_owned(),
        })
        .collect();
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(ProjectList { projects })
}

/// Finds one of the principal's projects by its exact id.
///
/// This is pure, like `list_projects`. An unknown id and another tenant's id
/// give the same `NotFound`, so the existence of a project is never revealed.
pub fn get_project(
    catalogue: &Catalogue,
    principal: &Principal,
    args: &Map<String, Value>,
) -> Result<ProjectSummary, OpError> {
    // 1. Authorize before looking at the input.
    if !principal.has(Permission::ViewProject) {
        return Err(OpError::Forbidden);
    }

    // 2. Decode strictly: only a required string `id`.
    let args = GetProjectArgs::deserialize(args).map_err(|_| OpError::InvalidRequest)?;

    // 3. Check the form exactly, without trimming.
    if !is_valid_id(&args.id) {
        return Err(OpError::InvalidId);
    }

    // 4. Search only inside the principal's tenant.
    catalogue
        .for_tenant(principal.tenant())
        .find(|p| p.id == args.id)
        .map(|p| ProjectSummary {
            id: p.id.to_owned(),
            name: p.name.to_owned(),
        })
        .ok_or(OpError::NotFound)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetProjectArgs {
    id: String,
}

fn is_valid_id(id: &str) -> bool {
    (1..=MAX_ID_BYTES).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListProjectsArgs {
    #[serde(default, deserialize_with = "present_string")]
    q: Option<String>,
}

/// Accepts only a JSON string, so an explicit `null` is rejected rather than
/// read as "no search".
fn present_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

fn checked_search(q: &str) -> Result<&str, OpError> {
    if q.chars().any(char::is_control) || q.chars().count() > MAX_SEARCH_CHARS {
        Err(OpError::InvalidSearch)
    } else {
        Ok(q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOBODY: Principal = Principal::new("nobody", TenantId::ACME, &[]);
    const GLOBEX: Principal =
        Principal::new("globex-user", TenantId::GLOBEX, &[Permission::ListProjects]);

    fn args(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("test arguments must be an object, got {other}"),
        }
    }

    fn run(principal: &Principal, value: Value) -> Result<ProjectList, OpError> {
        list_projects(&Catalogue::demo(), principal, &args(value))
    }

    fn ids(principal: &Principal, value: Value) -> Vec<String> {
        run(principal, value)
            .expect("list_projects should succeed")
            .projects
            .into_iter()
            .map(|p| p.id)
            .collect()
    }

    fn acme_ids(value: Value) -> Vec<String> {
        ids(&Principal::demo_api(), value)
    }

    // OP-1: authorization runs before any validation.

    #[test]
    fn op1_principal_without_permission_is_forbidden() {
        assert_eq!(run(&NOBODY, json!({})), Err(OpError::Forbidden));
    }

    #[test]
    fn op1_forbidden_wins_over_unknown_fields() {
        let value = json!({"tenant": "globex", "permission": "all"});
        assert_eq!(run(&NOBODY, value), Err(OpError::Forbidden));
    }

    #[test]
    fn op1_forbidden_wins_over_an_overlong_search() {
        let value = json!({"q": "a".repeat(81)});
        assert_eq!(run(&NOBODY, value), Err(OpError::Forbidden));
    }

    #[test]
    fn op1_forbidden_wins_over_a_non_string_search() {
        assert_eq!(run(&NOBODY, json!({"q": 7})), Err(OpError::Forbidden));
    }

    // OP-2: only an optional string `q` is accepted.

    #[test]
    fn op2_tenant_permission_and_token_fields_are_rejected() {
        for field in ["tenant", "permission", "token"] {
            let value = json!({ field: "globex" });
            assert_eq!(
                run(&Principal::demo_api(), value),
                Err(OpError::InvalidRequest),
                "field {field}"
            );
        }
    }

    #[test]
    fn op2_unknown_field_is_rejected_even_next_to_a_valid_search() {
        let value = json!({"q": "launch", "tenant": "globex"});
        assert_eq!(
            run(&Principal::demo_api(), value),
            Err(OpError::InvalidRequest)
        );
    }

    #[test]
    fn op2_non_string_search_is_rejected() {
        for q in [
            json!(1),
            json!(true),
            json!(null),
            json!(["a"]),
            json!({"a": 1}),
        ] {
            assert_eq!(
                run(&Principal::demo_api(), json!({ "q": q.clone() })),
                Err(OpError::InvalidRequest),
                "q = {q}"
            );
        }
    }

    // OP-3: search length and control characters, after trimming.

    #[test]
    fn op3_eighty_multibyte_scalars_pass_and_eighty_one_fail() {
        assert_eq!(
            acme_ids(json!({"q": "界".repeat(80)})),
            Vec::<String>::new()
        );
        assert_eq!(
            run(&Principal::demo_api(), json!({"q": "界".repeat(81)})),
            Err(OpError::InvalidSearch)
        );
    }

    #[test]
    fn op3_length_is_counted_after_trimming() {
        let padded = format!("  {}  ", "é".repeat(80));
        assert!(run(&Principal::demo_api(), json!({ "q": padded })).is_ok());
    }

    #[test]
    fn op3_combining_marks_are_not_normalized() {
        // 41 base letters plus 41 combining accents is 82 scalars.
        let decomposed = "e\u{301}".repeat(41);
        assert_eq!(
            run(&Principal::demo_api(), json!({ "q": decomposed })),
            Err(OpError::InvalidSearch)
        );
    }

    #[test]
    fn op3_control_characters_fail() {
        for q in [
            "a\u{7}b",
            "a\tb",
            "a\nb",
            "\u{0}",
            "x\u{1b}[0m",
            "a\u{7f}",
            "a\u{85}b",
        ] {
            assert_eq!(
                run(&Principal::demo_api(), json!({ "q": q })),
                Err(OpError::InvalidSearch),
                "q = {q:?}"
            );
        }
    }

    #[test]
    fn op3_surrounding_whitespace_is_trimmed() {
        assert_eq!(
            acme_ids(json!({"q": "  launch\n"})),
            acme_ids(json!({"q": "launch"}))
        );
    }

    #[test]
    fn op3_whitespace_only_search_means_no_search() {
        assert_eq!(acme_ids(json!({"q": " \t\n "})), acme_ids(json!({})));
        assert_eq!(acme_ids(json!({"q": ""})), acme_ids(json!({})));
    }

    // OP-4 and OP-5: tenant-scoped, case-insensitive substring search.

    #[test]
    fn op4_acme_without_search_lists_all_acme_projects() {
        assert_eq!(acme_ids(json!({})), ["acme-001", "acme-002", "acme-003"]);
    }

    #[test]
    fn op4_acme_launch_matches_two_projects() {
        assert_eq!(acme_ids(json!({"q": "launch"})), ["acme-001", "acme-003"]);
        assert_eq!(acme_ids(json!({"q": "LAUNCH"})), ["acme-001", "acme-003"]);
    }

    #[test]
    fn op4_uppercase_accented_search_matches() {
        assert_eq!(acme_ids(json!({"q": "CAFÉ"})), ["acme-002"]);
    }

    #[test]
    fn op4_other_tenants_projects_are_invisible() {
        assert_eq!(acme_ids(json!({"q": "zephyr"})), Vec::<String>::new());
        assert_eq!(acme_ids(json!({"q": "globex"})), Vec::<String>::new());
    }

    #[test]
    fn op4_globex_launch_matches_only_globex() {
        assert_eq!(ids(&GLOBEX, json!({"q": "launch"})), ["globex-001"]);
        assert_eq!(ids(&GLOBEX, json!({})), ["globex-001", "globex-002"]);
    }

    #[test]
    fn op4_markup_in_names_is_returned_verbatim() {
        let list = run(&Principal::demo_api(), json!({"q": "script"})).unwrap();
        assert_eq!(
            list.projects,
            [ProjectSummary {
                id: "acme-003".into(),
                name: "<script>alert('x')</script> & \"Launch\" Notes".into(),
            }]
        );
    }

    #[test]
    fn op4_results_are_deterministic() {
        let first = run(&Principal::demo_api(), json!({"q": "a"}));
        let second = run(&Principal::demo_api(), json!({"q": "a"}));
        assert_eq!(first, second);
        let ids: Vec<_> = first.unwrap().projects.into_iter().map(|p| p.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    // Public shapes.

    #[test]
    fn project_list_serializes_without_tenant_or_totals() {
        let list = run(&Principal::demo_api(), json!({"q": "café"})).unwrap();
        assert_eq!(
            serde_json::to_value(list).unwrap(),
            json!({"projects": [{"id": "acme-002", "name": "Café Roadmap"}]})
        );
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(OpError::Forbidden.code(), "forbidden");
        assert_eq!(OpError::InvalidRequest.code(), "invalid_request");
        assert_eq!(OpError::InvalidSearch.code(), "invalid_search");
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

    // GET-1 to GET-5: `get_project` (spec 0005, §4).

    const LIST_ONLY: Principal =
        Principal::new("list-only", TenantId::ACME, &[Permission::ListProjects]);

    const GLOBEX_VIEWER: Principal = Principal::new(
        "globex-viewer",
        TenantId::GLOBEX,
        &[Permission::ViewProject],
    );

    fn get(principal: &Principal, value: Value) -> Result<ProjectSummary, OpError> {
        get_project(&Catalogue::demo(), principal, &args(value))
    }

    #[test]
    fn get1_forbidden_wins_over_any_input() {
        for value in [
            json!({"id": "acme-001"}),
            json!({}),
            json!({"id": null}),
            json!({"id": 7}),
            json!({"id": "ACME-001"}),
            json!({"id": "globex-001", "tenant": "globex"}),
        ] {
            assert_eq!(
                get(&LIST_ONLY, value.clone()),
                Err(OpError::Forbidden),
                "{value}"
            );
            assert_eq!(
                get(&NOBODY, value.clone()),
                Err(OpError::Forbidden),
                "{value}"
            );
        }
        for principal in [
            Principal::demo_web(),
            Principal::demo_api(),
            Principal::demo_mcp(),
        ] {
            assert!(
                principal.has(Permission::ViewProject),
                "{}",
                principal.name()
            );
        }
    }

    #[test]
    fn get2_id_is_one_required_string_field() {
        for value in [
            json!({}),
            json!({"id": null}),
            json!({"id": 7}),
            json!({"id": true}),
            json!({"id": ["acme-001"]}),
            json!({"id": "acme-001", "tenant": "globex"}),
            json!({"q": "launch"}),
        ] {
            assert_eq!(
                get(&Principal::demo_api(), value.clone()),
                Err(OpError::InvalidRequest),
                "{value}"
            );
        }
    }

    #[test]
    fn get3_ids_outside_the_form_are_invalid_ids() {
        let long = "a".repeat(MAX_ID_BYTES + 1);
        for id in [
            "",
            long.as_str(),
            "ACME-001",
            "acme_001",
            " acme-001",
            "acme-001\n",
            "acmé",
            "../acme-001",
            "acme-001%00",
        ] {
            assert_eq!(
                get(&Principal::demo_api(), json!({ "id": id })),
                Err(OpError::InvalidId),
                "id = {id:?}"
            );
        }
        // The longest well-formed id is checked against the catalogue.
        let longest = "a".repeat(MAX_ID_BYTES);
        assert_eq!(
            get(&Principal::demo_api(), json!({ "id": longest })),
            Err(OpError::NotFound)
        );
    }

    #[test]
    fn get4_finds_projects_only_inside_the_principals_tenant() {
        assert_eq!(
            get(&Principal::demo_api(), json!({"id": "acme-002"})),
            Ok(ProjectSummary {
                id: "acme-002".into(),
                name: "Café Roadmap".into(),
            })
        );
        assert_eq!(
            get(&GLOBEX_VIEWER, json!({"id": "globex-001"})).map(|p| p.name),
            Ok("Globex Launch".to_owned())
        );
        // Another tenant's project and a missing one are indistinguishable.
        for id in ["globex-001", "acme-999", "a"] {
            assert_eq!(
                get(&Principal::demo_api(), json!({ "id": id })),
                Err(OpError::NotFound),
                "id = {id}"
            );
        }
        assert_eq!(
            get(&GLOBEX_VIEWER, json!({"id": "acme-001"})),
            Err(OpError::NotFound)
        );
    }

    #[test]
    fn get5_result_and_error_codes_are_public_shapes() {
        let project = get(&Principal::demo_api(), json!({"id": "acme-003"})).unwrap();
        assert_eq!(
            serde_json::to_value(project).unwrap(),
            json!({"id": "acme-003", "name": "<script>alert('x')</script> & \"Launch\" Notes"})
        );
        assert_eq!(OpError::InvalidId.code(), "invalid_id");
        assert_eq!(OpError::NotFound.code(), "not_found");
    }
}

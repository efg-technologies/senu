//! `list_projects`: the principal's projects, optionally filtered by `q`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{LIST_PROJECTS, ProjectSummary};
use crate::catalogue::Catalogue;
use crate::operation::{Args, OpError, Operation, Param, Principal, decode};
use crate::view;

/// Longest accepted search, counted in Unicode scalar values after trimming.
pub const MAX_SEARCH_CHARS: usize = 80;

/// The declaration. JSON Schema's `maxLength` would count the raw string,
/// while the operation trims first, so the description states the real
/// rule instead.
pub const OPERATION: Operation<Catalogue, ProjectList> = Operation {
    name: "list_projects",
    description: "List the demo projects visible to this server's principal, sorted by id.",
    permission: LIST_PROJECTS,
    params: &[Param {
        name: "q",
        description: "Case-insensitive name search. Surrounding whitespace is trimmed first; \
                      the trimmed value may hold at most 80 Unicode scalar values \
                      and no control characters. Empty means no search.",
        required: false,
    }],
    run: list_projects,
};

const _: () = assert!(MAX_SEARCH_CHARS == 80, "update the description of q");

/// The result of `list_projects`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectList {
    pub projects: Vec<ProjectSummary>,
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
    if !principal.has(LIST_PROJECTS) {
        return Err(OpError::Forbidden);
    }

    // 2. Decode strictly: only an optional string `q`.
    let args: ListProjectsArgs = decode(args)?;

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

/// The HTML page. The operation accepted `args`, so `q` is absent or a
/// valid string.
pub fn page(args: &Args, list: &ProjectList) -> Result<String, askama::Error> {
    let q = args.get("q").and_then(Value::as_str).unwrap_or("").trim();
    view::projects_page(q, &list.projects)
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
    use crate::catalogue::TenantId;
    use crate::demo::tests::{NOBODY, args};
    use crate::operation::Permission;
    use serde_json::json;

    const GLOBEX: Principal =
        Principal::new("globex-user", TenantId::GLOBEX, &[Permission::ListProjects]);

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
}

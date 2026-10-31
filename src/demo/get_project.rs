//! `get_project`: one of the principal's projects by its exact id.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{ProjectSummary, VIEW_PROJECT};
use crate::catalogue::Catalogue;
use crate::operation::{Args, OpError, Operation, Param, Principal, decode};
use crate::view;

/// Longest accepted project id, in bytes; an id is ASCII `a-z`, `0-9` and `-`.
pub const MAX_ID_BYTES: usize = 32;

/// The declaration. `id` is required and matched exactly, without trimming.
pub const OPERATION: Operation<Catalogue, ProjectSummary> = Operation {
    name: "get_project",
    description: "Get one demo project visible to this server's principal by its id.",
    permission: VIEW_PROJECT,
    params: &[Param {
        name: "id",
        description: "The project id: 1 to 32 characters of a-z, 0-9 and '-'.",
        required: true,
    }],
    run: get_project,
};

const _: () = assert!(MAX_ID_BYTES == 32, "update the description of id");

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
    if !principal.has(VIEW_PROJECT) {
        return Err(OpError::Forbidden);
    }

    // 2. Decode strictly: only a required string `id`.
    let args: GetProjectArgs = decode(args)?;

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

/// The HTML page.
pub fn page(_args: &Args, project: &ProjectSummary) -> Result<String, askama::Error> {
    view::project_page(project)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::TenantId;
    use crate::demo::tests::{NOBODY, args};
    use crate::operation::Permission;
    use serde_json::json;

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

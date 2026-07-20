//! Transport-only types for the modern internal API.
//!
//! This crate is intentionally independent from Axum, SQLx, Redis and every
//! application service.  Rust handlers and generated frontend bindings share
//! these DTOs; infrastructure rows and form/domain models must not leak here.

pub mod commerce;
pub mod patch;
pub mod time;

use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi};

pub use commerce::{AdminPlanItem, CreatedId, PlanCreate, PlanPatch, SortIdsRequest};

/// OpenAPI 3.1 document generated from the Rust transport source of truth.
/// Endpoint families move into this document as their handlers stop returning
/// untyped `serde_json::Value`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "V2Board internal API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Generated contract for the modern, non-frozen internal API"
    ),
    components(schemas(
        AdminPlanItem,
        CreatedId,
        PlanCreate,
        PlanPatch,
        SortIdsRequest
    )),
    paths(
        commerce::admin_plans_list_contract,
        commerce::admin_plan_create_contract,
        commerce::admin_plan_patch_contract,
        commerce::admin_plan_delete_contract,
        commerce::admin_plans_sort_contract
    ),
    tags((name = "admin-plans", description = "Administrative plan management")),
    modifiers(&SecurityAddon)
)]
pub struct InternalApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::Value;
    use utoipa::OpenApi as _;

    use super::InternalApiDoc;

    #[test]
    fn admin_plan_response_marks_every_serialized_field_required() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let schema = &document["components"]["schemas"]["AdminPlanItem"];
        let property_names = schema["properties"]
            .as_object()
            .expect("AdminPlanItem properties")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let required_names = schema["required"]
            .as_array()
            .expect("AdminPlanItem required")
            .iter()
            .map(|value| value.as_str().expect("required property").to_owned())
            .collect::<BTreeSet<_>>();

        assert_eq!(required_names, property_names);
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
    }

    #[test]
    fn admin_plan_operation_set_includes_sort_and_no_legacy_upsert() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let paths = document["paths"].as_object().expect("paths");

        assert_eq!(
            paths["/api/v1/{secure_path}/plans/sort"]["post"]["operationId"],
            "adminPlansSort"
        );
        assert!(paths["/api/v1/{secure_path}/plans"].get("put").is_none());
        assert_eq!(
            paths["/api/v1/{secure_path}/plans/{id}"]["patch"]["responses"]["204"]["description"],
            "Plan updated"
        );
    }

    #[test]
    fn plan_patch_non_clearable_fields_are_optional_but_never_nullable() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let schema = &document["components"]["schemas"]["PlanPatch"];
        let required = schema["required"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();

        for field in [
            "name",
            "group_id",
            "transfer_enable",
            "show",
            "renew",
            "force_update",
        ] {
            assert!(!required.contains(field), "{field} must remain optional");
            let property_type = &schema["properties"][field]["type"];
            let types = match property_type {
                Value::String(value) => vec![value.as_str()],
                Value::Array(values) => values
                    .iter()
                    .map(|value| value.as_str().expect("schema type must be a string"))
                    .collect(),
                other => panic!("{field} must declare an OpenAPI type: {other}"),
            };
            assert!(
                !types.contains(&"null"),
                "{field} must reject explicit null"
            );
        }
    }
}

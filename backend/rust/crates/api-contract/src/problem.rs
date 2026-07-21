//! OpenAPI projection of the shared RFC 9457 problem registry.
//!
//! The registry rows live in the framework-free `v2board-problem-code`
//! crate. This module derives the OpenAPI string enum from that macro and
//! defines reusable response components for every status currently assigned
//! by the registry.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::{ToResponse, ToSchema};
use v2board_problem_code::Code;

macro_rules! define_openapi_problem_code {
    ($( $variant:ident => ($slug:literal, $status:ident, $detail:literal), )+) => {
        /// Stable, append-only machine discriminator from
        /// `docs/api-dialect.md` §3.4.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum ProblemCode {
            $(#[serde(rename = $slug)] $variant,)+
        }
    };
}

v2board_problem_code::problem_code_registry!(define_openapi_problem_code);

/// RFC 9457 body emitted by every modern internal API error.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProblemDetails {
    /// RFC 9457 problem type. The service currently uses the default URI.
    #[schema(example = "about:blank")]
    pub r#type: String,
    /// Generic English HTTP reason phrase for `status`.
    #[schema(example = "Bad Request")]
    pub title: String,
    /// Mirrors the HTTP response status.
    #[schema(minimum = 400, maximum = 599, example = 400)]
    pub status: u16,
    pub code: ProblemCode,
    /// Locale-aware presentation text; clients must discriminate on `code`.
    pub detail: String,
    /// Present only for `validation_failed`; first message is primary.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub errors: Option<BTreeMap<String, Vec<String>>>,
}

/// Tighten utoipa's structural projection with invariants sourced from the
/// shared registry. RFC 9457 permits extension members, so the base object
/// remains open; the discriminated branch fixes the only valid
/// code/status/title tuple for each stable code.
pub(crate) fn augment_problem_schema(document: &mut Value) {
    let schemas = document["components"]["schemas"]
        .as_object_mut()
        .expect("OpenAPI components.schemas");
    let mut base = schemas
        .remove("ProblemDetails")
        .expect("ProblemDetails component schema");
    base["properties"]["type"]["const"] = Value::String("about:blank".to_owned());
    // RFC 9457 explicitly permits extension members. Keep this exception
    // machine-readable so the general closed-DTO normalization cannot make
    // the problem base accidentally strict.
    base["additionalProperties"] = Value::Bool(true);

    let variants = Code::ALL
        .iter()
        .map(|code| {
            let mut properties = json!({
                "code": { "const": code.slug() },
                "status": { "type": "integer", "const": code.status() },
                "title": { "type": "string", "const": code.title() }
            });
            if *code != Code::ValidationFailed {
                properties["errors"] = Value::Bool(false);
            }
            json!({
                "type": "object",
                "additionalProperties": true,
                "required": ["code", "status", "title"],
                "properties": properties
            })
        })
        .collect::<Vec<_>>();
    schemas.insert(
        "ProblemDetails".to_owned(),
        json!({
            "allOf": [
                base,
                {
                    "oneOf": variants,
                    "discriminator": { "propertyName": "code" }
                }
            ]
        }),
    );
}

#[derive(ToResponse)]
#[response(
    description = "Bad request or business rejection",
    content_type = "application/problem+json"
)]
pub struct BadRequestProblem(pub ProblemDetails);

/// A 401 RFC 9457 response. The challenge is bare `Bearer` when credentials
/// were absent and includes `error=\"invalid_token\"` when they were rejected.
#[derive(ToResponse)]
#[response(
    description = "Missing, expired, or invalid authentication",
    content_type = "application/problem+json",
    headers((
        "WWW-Authenticate" = String,
        description = "RFC 6750 Bearer challenge"
    ))
)]
pub struct UnauthorizedProblem(pub ProblemDetails);

/// Honest operation-level fallback: an endpoint can return any registered
/// problem without pretending that every individual status is reachable.
/// `WWW-Authenticate` is emitted only when the concrete response is 401.
#[derive(ToResponse)]
#[response(
    description = "RFC 9457 problem response",
    content_type = "application/problem+json",
    headers((
        "WWW-Authenticate" = String,
        description = "Optional RFC 6750 Bearer challenge on 401 responses"
    ))
)]
pub struct DefaultProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Authenticated request is not authorized",
    content_type = "application/problem+json"
)]
pub struct ForbiddenProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Endpoint or resource was not found",
    content_type = "application/problem+json"
)]
pub struct NotFoundProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Concurrent or idempotency conflict",
    content_type = "application/problem+json"
)]
pub struct ConflictProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Request field validation failed",
    content_type = "application/problem+json"
)]
pub struct ValidationProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Request rate limit exceeded",
    content_type = "application/problem+json"
)]
pub struct RateLimitedProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Unexpected internal failure",
    content_type = "application/problem+json"
)]
pub struct InternalServerProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Upstream integration failed",
    content_type = "application/problem+json"
)]
pub struct BadGatewayProblem(pub ProblemDetails);

#[derive(ToResponse)]
#[response(
    description = "Transient service unavailability",
    content_type = "application/problem+json"
)]
pub struct ServiceUnavailableProblem(pub ProblemDetails);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::InternalApiDoc;
    use serde_json::Value;
    use utoipa::OpenApi as _;
    use v2board_problem_code::Code;

    fn augmented_document() -> Value {
        let mut document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        crate::operations::augment_openapi_document(&mut document);
        document
    }

    #[test]
    fn openapi_problem_code_enum_is_the_shared_registry() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let actual = document["components"]["schemas"]["ProblemCode"]["enum"]
            .as_array()
            .expect("ProblemCode enum")
            .iter()
            .map(|value| value.as_str().expect("problem code string"))
            .collect::<BTreeSet<_>>();
        let expected = Code::ALL
            .iter()
            .map(|code| code.slug())
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 101);
    }

    #[test]
    fn unauthorized_component_has_problem_media_type_and_bearer_challenge() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let response = &document["components"]["responses"]["UnauthorizedProblem"];
        assert_eq!(
            response["content"]["application/problem+json"]["schema"]["$ref"],
            "#/components/schemas/ProblemDetails"
        );
        assert_eq!(
            response["headers"]["WWW-Authenticate"]["schema"]["type"],
            "string"
        );
    }

    #[test]
    fn default_component_is_problem_media_with_an_optional_bearer_challenge() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let response = &document["components"]["responses"]["DefaultProblem"];
        assert_eq!(
            response["content"]["application/problem+json"]["schema"]["$ref"],
            "#/components/schemas/ProblemDetails"
        );
        assert_eq!(
            response["headers"]["WWW-Authenticate"]["schema"]["type"],
            "string"
        );
    }

    #[test]
    fn problem_details_pins_type_and_every_code_status_title_tuple() {
        let document = augmented_document();
        let schema = &document["components"]["schemas"]["ProblemDetails"];
        let all_of = schema["allOf"].as_array().expect("ProblemDetails allOf");
        assert_eq!(all_of.len(), 2);
        let base = &all_of[0];
        let discriminated = &all_of[1];
        let required = base["required"]
            .as_array()
            .expect("required fields")
            .iter()
            .map(|value| value.as_str().expect("required field name"))
            .collect::<BTreeSet<_>>();
        for name in ["type", "title", "status", "code", "detail"] {
            assert!(required.contains(name));
        }
        assert!(!required.contains("errors"));
        assert_eq!(base["properties"]["type"]["const"], "about:blank");
        assert_eq!(base["properties"]["errors"]["type"], "object");
        assert_eq!(base["additionalProperties"], Value::Bool(true));
        assert_eq!(discriminated["discriminator"]["propertyName"], "code");

        let variants = discriminated["oneOf"]
            .as_array()
            .expect("problem-code variants");
        assert_eq!(variants.len(), Code::ALL.len());
        for (variant, code) in variants.iter().zip(Code::ALL) {
            assert_eq!(variant["additionalProperties"], Value::Bool(true));
            assert_eq!(variant["properties"]["code"]["const"], code.slug());
            assert_eq!(variant["properties"]["status"]["const"], code.status());
            assert_eq!(variant["properties"]["title"]["const"], code.title());
            if *code == Code::ValidationFailed {
                assert!(variant["properties"].get("errors").is_none());
            } else {
                assert_eq!(variant["properties"]["errors"], Value::Bool(false));
            }
        }
    }
}

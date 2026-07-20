use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The only body-bearing success arm of `PATCH /{secure_path}/config`.
/// A fully activated write instead returns an empty 204 response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigActivationPending {
    pub activation: PendingActivation,
    #[schema(minimum = 1)]
    pub revision: u64,
}

/// Single-value discriminator kept as an enum so OpenAPI and generated
/// clients constrain the wire value instead of widening it to `string`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingActivation {
    Pending,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use utoipa::OpenApi as _;

    use crate::InternalApiDoc;

    #[test]
    fn pending_activation_schema_has_a_fixed_discriminator_and_positive_revision() {
        let document = serde_json::to_value(InternalApiDoc::openapi()).expect("OpenAPI JSON");
        let pending = &document["components"]["schemas"]["ConfigActivationPending"];
        assert_eq!(pending["additionalProperties"], Value::Bool(false));
        assert_eq!(pending["properties"]["revision"]["minimum"], 1);
        assert_eq!(
            document["components"]["schemas"]["PendingActivation"]["enum"],
            serde_json::json!(["pending"])
        );
    }
}

use crate::{ProvisionKind, ProvisionSpec, manifest::LegacyNodeActivationTransportSpec};

/// The single production capability decision for the destructive legacy
/// migration path. Keeping the unavailable reason in the value prevents the
/// report and CLI from drifting into independent Boolean gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionLegacyApplyCapability {
    Unavailable(ProductionLegacyApplyBlocker),
    Available,
}

impl ProductionLegacyApplyCapability {
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub const fn blocker(self) -> Option<ProductionLegacyApplyBlocker> {
        match self {
            Self::Unavailable(blocker) => Some(blocker),
            Self::Available => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionLegacyApplyBlocker {
    AwaitingBareMetalFaultMatrixAndSafetyAudit,
    WrongProvisionKind,
    SchemaV4OrV5ExecutionRequired,
    EmbeddedTargetNodeInventoryMustBeEmpty,
    NodeCutoverPolicyUnsupported,
}

impl ProductionLegacyApplyBlocker {
    pub const fn report_message(self) -> &'static str {
        match self {
            Self::AwaitingBareMetalFaultMatrixAndSafetyAudit => {
                "production one-shot apply is disabled until the real bare-metal crash/lost-ACK fault matrix and final safety audit pass"
            }
            Self::WrongProvisionKind => {
                "production one-shot apply is available only for legacy_reference_migration"
            }
            Self::SchemaV4OrV5ExecutionRequired => {
                "production one-shot apply requires a schema-v4 or schema-v5 legacy execution manifest"
            }
            Self::EmbeddedTargetNodeInventoryMustBeEmpty => {
                "production one-shot apply requires an empty embedded target node inventory"
            }
            Self::NodeCutoverPolicyUnsupported => {
                "production one-shot apply requires the schema-v4 empty-source policy or the schema-v5 discard-and-manual-rebuild policy"
            }
        }
    }
}

/// Global evidence gate. It deliberately remains unavailable in source until
/// a revision-bound bare-metal matrix artifact and its safety review are added
/// to the release gate; changing this value alone is not acceptable evidence.
pub const PRODUCTION_LEGACY_APPLY_CAPABILITY: ProductionLegacyApplyCapability =
    ProductionLegacyApplyCapability::Unavailable(
        ProductionLegacyApplyBlocker::AwaitingBareMetalFaultMatrixAndSafetyAudit,
    );

/// Return the complete production capability for one validated manifest.
///
/// The global evidence gate is only one part of admission: opening it must not
/// silently broaden the schema or node topology supported by the release.
pub fn production_legacy_apply_capability_for_spec(
    spec: &ProvisionSpec,
) -> ProductionLegacyApplyCapability {
    if spec.kind != ProvisionKind::LegacyReferenceMigration {
        return ProductionLegacyApplyCapability::Unavailable(
            ProductionLegacyApplyBlocker::WrongProvisionKind,
        );
    }
    let Some(execution) = spec.legacy_apply_execution() else {
        return ProductionLegacyApplyCapability::Unavailable(
            ProductionLegacyApplyBlocker::SchemaV4OrV5ExecutionRequired,
        );
    };
    if !execution.nodes.inventory.is_empty() {
        return ProductionLegacyApplyCapability::Unavailable(
            ProductionLegacyApplyBlocker::EmbeddedTargetNodeInventoryMustBeEmpty,
        );
    }
    if !supported_node_cutover_policy(spec.schema_version, &execution.nodes.activation_transport) {
        return ProductionLegacyApplyCapability::Unavailable(
            ProductionLegacyApplyBlocker::NodeCutoverPolicyUnsupported,
        );
    }
    PRODUCTION_LEGACY_APPLY_CAPABILITY
}

fn supported_node_cutover_policy(
    schema_version: u32,
    transport: &LegacyNodeActivationTransportSpec,
) -> bool {
    matches!(
        (schema_version, transport),
        (4, LegacyNodeActivationTransportSpec::NotRequiredNoNodes)
            | (
                5,
                LegacyNodeActivationTransportSpec::DiscardAndManualRebuild
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_capability_remains_fail_closed_with_a_typed_reason() {
        assert!(!PRODUCTION_LEGACY_APPLY_CAPABILITY.is_available());
        assert_eq!(
            PRODUCTION_LEGACY_APPLY_CAPABILITY.blocker(),
            Some(ProductionLegacyApplyBlocker::AwaitingBareMetalFaultMatrixAndSafetyAudit)
        );
    }

    #[test]
    fn v4_and_v5_admit_only_their_exact_node_cutover_policies() {
        assert!(supported_node_cutover_policy(
            4,
            &LegacyNodeActivationTransportSpec::NotRequiredNoNodes,
        ));
        assert!(!supported_node_cutover_policy(
            4,
            &LegacyNodeActivationTransportSpec::DiscardAndManualRebuild,
        ));
        assert!(supported_node_cutover_policy(
            5,
            &LegacyNodeActivationTransportSpec::DiscardAndManualRebuild,
        ));
        assert!(!supported_node_cutover_policy(
            5,
            &LegacyNodeActivationTransportSpec::NotRequiredNoNodes,
        ));
    }
}

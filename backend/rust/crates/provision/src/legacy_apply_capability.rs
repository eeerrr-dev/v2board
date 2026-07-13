use crate::{ProvisionKind, ProvisionSpec};

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
    SchemaV4ExecutionRequired,
    NonEmptyNodeInventoryUnsupported,
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
            Self::SchemaV4ExecutionRequired => {
                "production one-shot apply requires a schema-v4 legacy execution manifest"
            }
            Self::NonEmptyNodeInventoryUnsupported => {
                "production one-shot apply currently supports only an empty node inventory"
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
            ProductionLegacyApplyBlocker::SchemaV4ExecutionRequired,
        );
    };
    if !execution.nodes.inventory.is_empty() {
        return ProductionLegacyApplyCapability::Unavailable(
            ProductionLegacyApplyBlocker::NonEmptyNodeInventoryUnsupported,
        );
    }
    PRODUCTION_LEGACY_APPLY_CAPABILITY
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
}

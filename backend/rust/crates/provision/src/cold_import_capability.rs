use serde::Serialize;

use crate::{ProvisionKind, ProvisionSpec};

/// One fail-closed production admission value shared by validate, inspect and
/// apply. The archive-first contract is available for review, but no caller can
/// mutate a target until the cold importer and its operation-owned cleanup gate
/// are both proven.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionColdImportCapability {
    Unavailable(ProductionColdImportBlocker),
    Available,
}

impl ProductionColdImportCapability {
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub const fn blocker(self) -> Option<ProductionColdImportBlocker> {
        match self {
            Self::Unavailable(blocker) => Some(blocker),
            Self::Available => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionColdImportBlocker {
    ColdImporterAndOperationOwnedCleanupEvidenceIncomplete,
    WrongProvisionKind,
    ArchiveFirstV5Required,
}

impl ProductionColdImportBlocker {
    pub const fn report_message(self) -> &'static str {
        match self {
            Self::ColdImporterAndOperationOwnedCleanupEvidenceIncomplete => {
                "production apply is disabled until the archive-first importer, exact retained/discarded-row proof, and operation-owned pre-activation cleanup integration gate pass"
            }
            Self::WrongProvisionKind => {
                "production cold import is available only for legacy_reference_migration"
            }
            Self::ArchiveFirstV5Required => {
                "production cold import requires the unique archive-first schema_version 5 contract"
            }
        }
    }
}

pub const PRODUCTION_COLD_IMPORT_CAPABILITY: ProductionColdImportCapability =
    ProductionColdImportCapability::Unavailable(
        ProductionColdImportBlocker::ColdImporterAndOperationOwnedCleanupEvidenceIncomplete,
    );

pub fn production_cold_import_capability_for_spec(
    spec: &ProvisionSpec,
) -> ProductionColdImportCapability {
    if spec.kind != ProvisionKind::LegacyReferenceMigration {
        return ProductionColdImportCapability::Unavailable(
            ProductionColdImportBlocker::WrongProvisionKind,
        );
    }
    if spec.schema_version != 5 || spec.legacy_cold_import().is_none() {
        return ProductionColdImportCapability::Unavailable(
            ProductionColdImportBlocker::ArchiveFirstV5Required,
        );
    }
    PRODUCTION_COLD_IMPORT_CAPABILITY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_capability_is_a_single_typed_fail_closed_value() {
        assert!(!PRODUCTION_COLD_IMPORT_CAPABILITY.is_available());
        assert_eq!(
            PRODUCTION_COLD_IMPORT_CAPABILITY.blocker(),
            Some(
                ProductionColdImportBlocker::ColdImporterAndOperationOwnedCleanupEvidenceIncomplete
            )
        );
    }
}

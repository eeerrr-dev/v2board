mod apply_authorization;
pub mod apply_journal;
#[cfg(feature = "bare-metal-fault-matrix")]
pub mod bare_metal_fault_matrix;
mod inspect;
pub mod legacy_apply;
mod legacy_apply_capability;
pub mod legacy_backup;
pub mod legacy_clickhouse;
pub mod legacy_converter;
pub mod legacy_copy;
mod legacy_mysql;
pub mod lifecycle_ledger;
mod manifest;
pub mod native_activation;
mod native_legacy_source;
pub mod native_node_cutover;
pub mod postgres_runtime_grants;
pub mod production_legacy_apply;
pub mod target_activation;

pub use apply_authorization::{ApplyAuthorization, ApplyAuthorizationError};
pub use inspect::{
    ClickHouseInspection, DataInspection, DatabaseInspection, InspectionMode,
    LegacyJsonIdArrayColumnInspection, LegacyJsonIdArrayInspection, NativeUpgradeInspection,
    NextAction, PostgresInspection, PreflightVerdict, ProvisionPlan, ProvisionPlanError,
    SourceRedisInspection, TargetRedisInspection, build_inspection, build_plan,
};
pub use legacy_apply_capability::{
    PRODUCTION_LEGACY_APPLY_CAPABILITY, ProductionLegacyApplyCapability,
};
pub use manifest::{
    LEGACY_REFERENCE_COMMIT, NativeUpgradeImpactSpec, ProvisionKind, ProvisionSpec,
    ProvisionSpecError, SourceTransportSecurity, load_provision_spec,
};
pub use native_legacy_source::{
    LegacyUnitRole, PreAuthorizationDatastoreBindingInspection,
    PreAuthorizationLegacyUnitInspection, PreAuthorizationSourceControlInspection,
};

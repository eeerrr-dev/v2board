//! Deterministic import contract for the pinned source MySQL dump.
//!
//! The pre-release schema v1 imports one immutable dump into one empty target.
//! The mapping and loss policy below are the complete MySQL import contract.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use sha2::{Digest, Sha256, Sha384};

use crate::mysql_import_policy::{
    LegacyOrderBinding, LegacyOrderDisposition, LegacyOrderPolicyError, MYSQL_IMPORT_POLICY_MARKER,
    classify_legacy_order, is_legacy_stripe_payment_driver,
};

pub const MYSQL_IMPORT_SOURCE_PROFILE: &str =
    "wyx2685-v2board@7e77de9f4873b317157490529f7be7d6f8a62421";
pub const MYSQL_SOURCE_SCHEMA_SHA256: &str =
    "f2c1e14169a728325bb8073b8ffe1f31bb13c8913318fdb10710ae0a99a9e8cf";
pub const MYSQL_SOURCE_INSTALL_SQL_SHA256: &str =
    "04b04531037b9e0b6f2a6b02194a8f1bc102789af8ee7be963fd721d51bca8e2";
pub const MYSQL_IMPORT_SCHEMA_VERSION: u32 = 1;
pub const TARGET_POSTGRES_SCHEMA_ID: &str = "migrations-postgres/mysql-import-v1";
pub const MYSQL_IMPORT_REGISTRY_VERSION: u32 = 1;
pub const DEFAULT_BATCH_SIZE: u32 = 1_000;
pub const MAX_BATCH_SIZE: u32 = 100_000;
pub const POSTGRES_MAX_BIND_PARAMETERS: usize = 65_535;
/// Native identities are positive. The source preflight rejects every
/// non-positive business primary key before any target is created, so keyset
/// scans start at the exact lower bound of the accepted domain.
pub const INITIAL_SOURCE_ID_CURSOR: i64 = 0;

const TARGET_POSTGRES_MIGRATIONS: &[(i64, &str, &[u8])] = &[(
    1,
    "0001_initial.sql",
    include_bytes!("../../../migrations-postgres/0001_initial.sql"),
)];

const DECIMAL: ColumnRule = ColumnRule::ExactDecimal;
const JSON_ANY: ColumnRule = ColumnRule::Json(JsonShape::Any);
const JSON_ARRAY: ColumnRule = ColumnRule::Json(JsonShape::Array);
const ID_ARRAY_I32: ColumnRule = ColumnRule::PositiveIdArray {
    maximum: i32::MAX as u64,
    require_non_empty: false,
    output: JsonOutput::Json,
};
const ID_ARRAY_I64_AS_TEXT: ColumnRule = ColumnRule::PositiveIdArray {
    maximum: i64::MAX as u64,
    require_non_empty: false,
    output: JsonOutput::CanonicalText,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonShape {
    Any,
    Array,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonOutput {
    Json,
    CanonicalText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnRule {
    /// Parse and bind a base-10 fixed-point value without a binary float.
    ExactDecimal,
    /// Parse legacy JSON text and require the declared top-level shape.
    Json(JsonShape),
    /// Normalize positive decimal-string members to JSON integer numbers.
    PositiveIdArray {
        maximum: u64,
        require_non_empty: bool,
        output: JsonOutput,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransformColumn {
    pub source: &'static str,
    pub target: &'static str,
    pub rule: ColumnRule,
    /// Optional source/target tables whose `id` sets must contain every member.
    /// Reference validation is a pre-copy and final-verification obligation;
    /// row transformation alone cannot prove it. Both names are explicit
    /// because only the legacy source carries the `v2_*` prefix.
    pub source_referenced_table: Option<&'static str>,
    pub referenced_target_table: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddedValue {
    Null,
    I64(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddedColumn {
    pub target: &'static str,
    pub value: AddedValue,
    pub provenance: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeferredColumn {
    pub source: &'static str,
    pub target: &'static str,
    pub source_referenced_table: &'static str,
    pub referenced_target_table: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumedSourceColumn {
    pub source: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityWidth {
    I32,
    I64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableMapping {
    pub order: u16,
    pub source: &'static str,
    pub target: &'static str,
    pub identity_width: IdentityWidth,
    /// Same-name, same-value columns. These are intentionally enumerated;
    /// there is no implicit "copy every column" behavior.
    pub direct_columns: &'static [&'static str],
    pub transformed_columns: &'static [TransformColumn],
    pub added_columns: &'static [AddedColumn],
    pub deferred_columns: &'static [DeferredColumn],
    /// Source columns consumed by a derived target rather than the base row.
    pub consumed_source_columns: &'static [ConsumedSourceColumn],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedMappingKind {
    GiftcardRedemptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedMapping {
    pub order: u16,
    pub target: &'static str,
    pub kind: DerivedMappingKind,
    pub source_tables: &'static [&'static str],
    /// Complete target row written by the derived mapping. The live PostgreSQL
    /// schema gate prepares base inserts and verifies every column listed here.
    pub target_columns: &'static [&'static str],
    pub key_columns: &'static [&'static str],
    pub rule: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarReferenceRule {
    Required,
    Nullable,
    /// Legacy `v2_order.plan_id=0` denotes a deposit and intentionally has no
    /// plan row; every other value must resolve.
    ZeroMeansNoReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarReference {
    /// Legacy MySQL endpoint used by pre-copy validation.
    pub source_table: &'static str,
    /// Native PostgreSQL endpoint used by final verification.
    pub target_table: &'static str,
    pub column: &'static str,
    pub source_referenced_table: &'static str,
    pub target_referenced_table: &'static str,
    pub rule: ScalarReferenceRule,
}

const SERVER_GROUP: TableMapping = TableMapping {
    order: 10,
    source: "v2_server_group",
    target: "server_group",
    identity_width: IdentityWidth::I32,
    direct_columns: &["id", "name", "created_at", "updated_at"],
    transformed_columns: &[],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const PLAN: TableMapping = TableMapping {
    order: 20,
    source: "v2_plan",
    target: "plan",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "group_id",
        "transfer_enable",
        "device_limit",
        "name",
        "speed_limit",
        "show",
        "sort",
        "renew",
        "content",
        "month_price",
        "quarter_price",
        "half_year_price",
        "year_price",
        "two_year_price",
        "three_year_price",
        "onetime_price",
        "reset_price",
        "reset_traffic_method",
        "capacity_limit",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const PAYMENT: TableMapping = TableMapping {
    order: 30,
    source: "v2_payment",
    target: "payment_method",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "uuid",
        "payment",
        "name",
        "icon",
        "notify_domain",
        "handling_fee_fixed",
        "enable",
        "sort",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[
        TransformColumn {
            source: "config",
            target: "config",
            rule: JSON_ANY,
            source_referenced_table: None,
            referenced_target_table: None,
        },
        TransformColumn {
            source: "handling_fee_percent",
            target: "handling_fee_percent",
            rule: DECIMAL,
            source_referenced_table: None,
            referenced_target_table: None,
        },
    ],
    added_columns: &[AddedColumn {
        target: "archived_at",
        value: AddedValue::Null,
        provenance: "legacy payment rows are active history; native archival did not exist",
    }],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const COUPON: TableMapping = TableMapping {
    order: 40,
    source: "v2_coupon",
    target: "coupon",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "code",
        "name",
        "type",
        "value",
        "show",
        "limit_use",
        "limit_use_with_user",
        "started_at",
        "ended_at",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[
        TransformColumn {
            source: "limit_plan_ids",
            target: "limit_plan_ids",
            rule: ID_ARRAY_I32,
            source_referenced_table: Some("v2_plan"),
            referenced_target_table: Some("plan"),
        },
        TransformColumn {
            source: "limit_period",
            target: "limit_period",
            rule: JSON_ARRAY,
            source_referenced_table: None,
            referenced_target_table: None,
        },
    ],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const USER: TableMapping = TableMapping {
    order: 50,
    source: "v2_user",
    target: "users",
    identity_width: IdentityWidth::I64,
    direct_columns: &[
        "id",
        "telegram_id",
        "email",
        "password",
        "password_algo",
        "password_salt",
        "balance",
        "discount",
        "commission_type",
        "commission_rate",
        "commission_balance",
        "t",
        "u",
        "d",
        "transfer_enable",
        "device_limit",
        "banned",
        "is_admin",
        "last_login_at",
        "is_staff",
        "last_login_ip",
        "uuid",
        "group_id",
        "plan_id",
        "speed_limit",
        "auto_renewal",
        "remind_expire",
        "remind_traffic",
        "token",
        "expired_at",
        "remarks",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[],
    added_columns: &[
        AddedColumn {
            target: "session_epoch",
            value: AddedValue::I64(0),
            provenance: "legacy sessions are discarded and the frozen legacy mapping initializes epoch zero",
        },
        AddedColumn {
            target: "traffic_epoch",
            value: AddedValue::I64(0),
            provenance: "the frozen legacy mapping initializes traffic epoch zero",
        },
        AddedColumn {
            target: "scheduled_traffic_reset_key",
            value: AddedValue::Null,
            provenance: "legacy reset scheduling has no durable per-user key",
        },
    ],
    deferred_columns: &[DeferredColumn {
        source: "invite_user_id",
        target: "invite_user_id",
        source_referenced_table: "v2_user",
        referenced_target_table: "users",
        reason: "self references are patched after every user id exists; NULL and cycles remain exact",
    }],
    consumed_source_columns: &[],
};

const ORDER: TableMapping = TableMapping {
    order: 60,
    source: "v2_order",
    target: "orders",
    identity_width: IdentityWidth::I64,
    direct_columns: &[
        "id",
        "invite_user_id",
        "user_id",
        "plan_id",
        "coupon_id",
        "payment_id",
        "type",
        "period",
        "trade_no",
        "callback_no",
        "total_amount",
        "handling_amount",
        "discount_amount",
        "surplus_amount",
        "refund_amount",
        "balance_amount",
        "status",
        "commission_status",
        "commission_balance",
        "actual_commission_balance",
        "paid_at",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[TransformColumn {
        source: "surplus_order_ids",
        target: "surplus_order_ids",
        rule: ID_ARRAY_I64_AS_TEXT,
        source_referenced_table: Some("v2_order"),
        referenced_target_table: Some("orders"),
    }],
    added_columns: &[AddedColumn {
        target: "callback_no_hash",
        value: AddedValue::Null,
        provenance: "legacy orders predate native callback hash capture",
    }],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const COMMISSION_LOG: TableMapping = TableMapping {
    order: 70,
    source: "v2_commission_log",
    target: "commission_log",
    identity_width: IdentityWidth::I64,
    direct_columns: &[
        "id",
        "invite_user_id",
        "user_id",
        "trade_no",
        "order_amount",
        "get_amount",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const INVITE_CODE: TableMapping = TableMapping {
    order: 80,
    source: "v2_invite_code",
    target: "invite_code",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "user_id",
        "code",
        "status",
        "pv",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const GIFTCARD: TableMapping = TableMapping {
    order: 90,
    source: "v2_giftcard",
    target: "gift_card",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "code",
        "name",
        "type",
        "value",
        "plan_id",
        "limit_use",
        "started_at",
        "ended_at",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[ConsumedSourceColumn {
        source: "used_user_ids",
        reason: "expanded by the gift_card_redemption target mapping",
    }],
};

macro_rules! direct_table {
    ($name:ident, $order:literal, $source:literal, $target:literal, $width:ident, [$($column:literal),+ $(,)?]) => {
        const $name: TableMapping = TableMapping {
            order: $order,
            source: $source,
            target: $target,
            identity_width: IdentityWidth::$width,
            direct_columns: &[$($column),+],
            transformed_columns: &[],
            added_columns: &[],
            deferred_columns: &[],
            consumed_source_columns: &[],
        };
    };
}

direct_table!(
    KNOWLEDGE,
    100,
    "v2_knowledge",
    "knowledge",
    I32,
    [
        "id",
        "language",
        "category",
        "title",
        "body",
        "sort",
        "show",
        "created_at",
        "updated_at"
    ]
);

const NOTICE: TableMapping = TableMapping {
    order: 110,
    source: "v2_notice",
    target: "notice",
    identity_width: IdentityWidth::I32,
    direct_columns: &[
        "id",
        "title",
        "content",
        "show",
        "img_url",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[TransformColumn {
        source: "tags",
        target: "tags",
        rule: JSON_ARRAY,
        source_referenced_table: None,
        referenced_target_table: None,
    }],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

direct_table!(
    TICKET,
    120,
    "v2_ticket",
    "ticket",
    I64,
    [
        "id",
        "user_id",
        "subject",
        "level",
        "status",
        "reply_status",
        "created_at",
        "updated_at"
    ]
);

direct_table!(
    TICKET_MESSAGE,
    130,
    "v2_ticket_message",
    "ticket_message",
    I64,
    [
        "id",
        "user_id",
        "ticket_id",
        "message",
        "created_at",
        "updated_at"
    ]
);

direct_table!(
    STAT,
    140,
    "v2_stat",
    "stat",
    I64,
    [
        "id",
        "record_at",
        "record_type",
        "order_count",
        "order_total",
        "commission_count",
        "commission_total",
        "paid_count",
        "paid_total",
        "register_count",
        "invite_count",
        "transfer_used_total",
        "created_at",
        "updated_at"
    ]
);

pub const TABLE_MAPPINGS: &[TableMapping] = &[
    SERVER_GROUP,
    PLAN,
    PAYMENT,
    COUPON,
    USER,
    ORDER,
    COMMISSION_LOG,
    INVITE_CODE,
    GIFTCARD,
    KNOWLEDGE,
    NOTICE,
    TICKET,
    TICKET_MESSAGE,
    STAT,
];

pub const DERIVED_MAPPINGS: &[DerivedMapping] = &[DerivedMapping {
    order: 150,
    target: "gift_card_redemption",
    kind: DerivedMappingKind::GiftcardRedemptions,
    source_tables: &["v2_giftcard", "v2_user"],
    target_columns: &[
        "giftcard_id",
        "user_id",
        "created_at",
        "created_at_provenance",
    ],
    key_columns: &["giftcard_id", "user_id"],
    rule: "expand distinct used_user_ids; every id must exist; created_at=0 and created_at_provenance=legacy_unknown",
}];

/// Whole tables intentionally omitted from the target.
/// Legacy sources `v2_user`, `v2_payment`, `v2_server_group`, and `v2_stat`
/// remain in the row-mapping inventory. The adapter applies the fixed Stripe
/// row policy within source `v2_payment`/`v2_order`; non-Stripe payment rows and
/// their original `enable` values remain exact in unprefixed native targets.
pub const DISCARDED_SOURCE_TABLES: &[&str] = &[
    "failed_jobs",
    "v2_log",
    "v2_mail_log",
    "v2_stat_server",
    "v2_stat_user",
    "v2_server_route",
    "v2_server_shadowsocks",
    "v2_server_vmess",
    "v2_server_trojan",
    "v2_server_tuic",
    "v2_server_hysteria",
    "v2_server_vless",
    "v2_server_anytls",
    "v2_server_v2node",
];

/// Native PostgreSQL tables whose legacy contents are fixed losses. These
/// names are deliberately independent from `DISCARDED_SOURCE_TABLES`: legacy
/// MySQL keeps its `v2_*` names while the first native schema is unprefixed.
pub const DISCARDED_TARGET_TABLES: &[&str] = &[
    "system_log",
    "mail_log",
    "server_traffic",
    "user_traffic",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
    "server_credential",
];

pub fn copied_table_mappings() -> impl Iterator<Item = &'static TableMapping> {
    TABLE_MAPPINGS.iter()
}

/// Native PostgreSQL tables that must be proven empty after conversion.
/// `failed_jobs` has no native target table, while `server_credential` is a
/// native-only derived table that must also start empty.
pub fn discarded_target_tables() -> impl Iterator<Item = &'static str> {
    DISCARDED_TARGET_TABLES.iter().copied()
}

pub fn built_derived_mappings() -> impl Iterator<Item = &'static DerivedMapping> {
    DERIVED_MAPPINGS.iter()
}

/// Scalar relationships that must be proven before copy and again against the
/// target. Historical ids intentionally lacking a PostgreSQL FK are
/// value-verified but are not silently re-parented.
pub const SCALAR_REFERENCES: &[ScalarReference] = &[
    ScalarReference {
        source_table: "v2_plan",
        target_table: "plan",
        column: "group_id",
        source_referenced_table: "v2_server_group",
        target_referenced_table: "server_group",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        source_table: "v2_user",
        target_table: "users",
        column: "invite_user_id",
        source_referenced_table: "v2_user",
        target_referenced_table: "users",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        source_table: "v2_user",
        target_table: "users",
        column: "group_id",
        source_referenced_table: "v2_server_group",
        target_referenced_table: "server_group",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        source_table: "v2_user",
        target_table: "users",
        column: "plan_id",
        source_referenced_table: "v2_plan",
        target_referenced_table: "plan",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        source_table: "v2_order",
        target_table: "orders",
        column: "user_id",
        source_referenced_table: "v2_user",
        target_referenced_table: "users",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        source_table: "v2_order",
        target_table: "orders",
        column: "plan_id",
        source_referenced_table: "v2_plan",
        target_referenced_table: "plan",
        rule: ScalarReferenceRule::ZeroMeansNoReference,
    },
    // Validate source orders against the complete payment id set. After the
    // Stripe policy runs, every retained non-null id names a retained payment.
    ScalarReference {
        source_table: "v2_order",
        target_table: "orders",
        column: "payment_id",
        source_referenced_table: "v2_payment",
        target_referenced_table: "payment_method",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        source_table: "v2_invite_code",
        target_table: "invite_code",
        column: "user_id",
        source_referenced_table: "v2_user",
        target_referenced_table: "users",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        source_table: "v2_giftcard",
        target_table: "gift_card",
        column: "plan_id",
        source_referenced_table: "v2_plan",
        target_referenced_table: "plan",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        source_table: "v2_ticket",
        target_table: "ticket",
        column: "user_id",
        source_referenced_table: "v2_user",
        target_referenced_table: "users",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        source_table: "v2_ticket_message",
        target_table: "ticket_message",
        column: "ticket_id",
        source_referenced_table: "v2_ticket",
        target_referenced_table: "ticket",
        rule: ScalarReferenceRule::Required,
    },
];

/// Columns intentionally omitted from inserts because PostgreSQL derives them
/// from preserved legacy values. Final verification must still compare their
/// evaluated meaning.
pub const TARGET_GENERATED_COLUMNS: &[(&str, &[&str])] = &[("orders", &["referenced_plan_id"])];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum SourceValue {
    Null,
    I64(i64),
    U64(u64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CanonicalValue {
    Null,
    I64(i64),
    U64(u64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
    Json(Value),
}

pub type SourceRow = BTreeMap<String, SourceValue>;
pub type CanonicalRow = BTreeMap<String, CanonicalValue>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegacyGiftcardRedemptionRow {
    pub giftcard_id: i32,
    pub user_id: i64,
    pub created_at: i64,
    pub created_at_provenance: String,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ConverterError {
    #[error("converter registry is invalid: {0}")]
    Registry(String),
    #[error("unknown mapping table {0}")]
    UnknownTable(String),
    #[error("source row for {table} is missing column {column}")]
    MissingColumn { table: String, column: String },
    #[error("source row for {table} contains unconsumed column {column}")]
    UnconsumedColumn { table: String, column: String },
    #[error("{table}.{column} has incompatible source type {actual}")]
    TypeMismatch {
        table: String,
        column: String,
        actual: &'static str,
    },
    #[error("{table}.{column} contains invalid JSON: {message}")]
    InvalidJson {
        table: String,
        column: String,
        message: String,
    },
    #[error("{table}.{column} contains an invalid exact decimal")]
    InvalidDecimal { table: String, column: String },
    #[error("{table}.{column} contains an invalid positive id at member {index}")]
    InvalidIdArray {
        table: String,
        column: String,
        index: usize,
    },
    #[error("batch ids for {table} are not strictly increasing after cursor {after_id}")]
    NonMonotonicBatch { table: String, after_id: i64 },
    #[error("batch size {0} is outside 1..={MAX_BATCH_SIZE}")]
    InvalidBatchSize(u32),
    #[error("target batch has {parameters} bind parameters, above PostgreSQL's supported limit")]
    TargetBatchParameterLimit { parameters: usize },
    #[error(transparent)]
    OrderPolicy(#[from] LegacyOrderPolicyError),
}

pub fn mapping_for_source(table: &str) -> Option<&'static TableMapping> {
    TABLE_MAPPINGS
        .iter()
        .find(|mapping| mapping.source == table)
}

pub fn audit_registry() -> Result<(), ConverterError> {
    let expected_source_tables = [
        "v2_commission_log",
        "v2_coupon",
        "v2_giftcard",
        "v2_invite_code",
        "v2_knowledge",
        "v2_notice",
        "v2_order",
        "v2_payment",
        "v2_plan",
        "v2_server_group",
        "v2_stat",
        "v2_ticket",
        "v2_ticket_message",
        "v2_user",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let expected_target_tables = [
        "commission_log",
        "coupon",
        "gift_card",
        "invite_code",
        "knowledge",
        "notice",
        "orders",
        "payment_method",
        "plan",
        "server_group",
        "stat",
        "ticket",
        "ticket_message",
        "users",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    let mut seen_orders = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut seen_targets = BTreeSet::new();
    for mapping in TABLE_MAPPINGS {
        validate_identifier(mapping.source)?;
        validate_identifier(mapping.target)?;
        if !seen_orders.insert(mapping.order) {
            return registry_error(format!("duplicate table order {}", mapping.order));
        }
        if !seen_sources.insert(mapping.source) {
            return registry_error(format!("duplicate source table {}", mapping.source));
        }
        if !seen_targets.insert(mapping.target) {
            return registry_error(format!("duplicate target table {}", mapping.target));
        }

        let mut source_columns = BTreeSet::new();
        let mut target_columns = BTreeSet::new();
        for column in mapping.direct_columns {
            validate_identifier(column)?;
            if !source_columns.insert(*column) || !target_columns.insert(*column) {
                return registry_error(format!(
                    "duplicate direct column {}.{}",
                    mapping.source, column
                ));
            }
        }
        for column in mapping.transformed_columns {
            validate_identifier(column.source)?;
            validate_identifier(column.target)?;
            if !source_columns.insert(column.source) || !target_columns.insert(column.target) {
                return registry_error(format!(
                    "duplicate transformed column {}.{}",
                    mapping.source, column.source
                ));
            }
        }
        for column in mapping.added_columns {
            validate_identifier(column.target)?;
            if !target_columns.insert(column.target) {
                return registry_error(format!(
                    "duplicate added target column {}.{}",
                    mapping.target, column.target
                ));
            }
            if column.provenance.trim().is_empty() {
                return registry_error(format!(
                    "added target column {}.{} lacks provenance",
                    mapping.target, column.target
                ));
            }
        }
        for column in mapping.deferred_columns {
            validate_identifier(column.source)?;
            validate_identifier(column.target)?;
            validate_identifier(column.source_referenced_table)?;
            validate_identifier(column.referenced_target_table)?;
            if !source_columns.insert(column.source) || !target_columns.insert(column.target) {
                return registry_error(format!(
                    "duplicate deferred column {}.{}",
                    mapping.source, column.source
                ));
            }
        }
        for column in mapping.consumed_source_columns {
            validate_identifier(column.source)?;
            if !source_columns.insert(column.source) {
                return registry_error(format!(
                    "duplicate consumed source column {}.{}",
                    mapping.source, column.source
                ));
            }
        }
        if !source_columns.contains("id") || !target_columns.contains("id") {
            return registry_error(format!(
                "base mapping {} must preserve its id",
                mapping.source
            ));
        }
        if !mapping.direct_columns.contains(&"created_at")
            || !mapping.direct_columns.contains(&"updated_at")
        {
            return registry_error(format!(
                "base mapping {} must preserve both timestamps",
                mapping.source
            ));
        }
    }

    if seen_sources != expected_source_tables {
        return registry_error("source table registry differs from the pinned core inventory");
    }
    if seen_targets != expected_target_tables {
        return registry_error(
            "target table registry differs from the unprefixed native inventory",
        );
    }
    if !TABLE_MAPPINGS
        .windows(2)
        .all(|pair| pair[0].order < pair[1].order)
    {
        return registry_error("table mappings are not stored in strict execution order");
    }
    for mapping in TABLE_MAPPINGS {
        for column in mapping.transformed_columns {
            match (
                column.source_referenced_table,
                column.referenced_target_table,
            ) {
                (None, None) => {}
                (Some(source_table), Some(target_table))
                    if TABLE_MAPPINGS.iter().any(|candidate| {
                        candidate.source == source_table && candidate.target == target_table
                    }) => {}
                _ => {
                    return registry_error(format!(
                        "{}.{} has mismatched source/target reference metadata",
                        mapping.source, column.source
                    ));
                }
            }
        }
        for column in mapping.deferred_columns {
            if !TABLE_MAPPINGS.iter().any(|candidate| {
                candidate.source == column.source_referenced_table
                    && candidate.target == column.referenced_target_table
            }) {
                return registry_error(format!(
                    "{}.{} has mismatched deferred source/target reference metadata",
                    mapping.source, column.source
                ));
            }
        }
    }
    if DERIVED_MAPPINGS
        .iter()
        .any(|mapping| !seen_orders.insert(mapping.order))
    {
        return registry_error("derived mapping order collides with a base mapping");
    }
    if !DERIVED_MAPPINGS
        .windows(2)
        .all(|pair| pair[0].order < pair[1].order)
    {
        return registry_error("derived mappings are not stored in strict execution order");
    }
    for mapping in DERIVED_MAPPINGS {
        validate_identifier(mapping.target)?;
        if mapping.source_tables.is_empty()
            || mapping.target_columns.is_empty()
            || mapping.key_columns.is_empty()
        {
            return registry_error(format!(
                "derived mapping {} has no source, target columns, or key",
                mapping.target
            ));
        }
        for source in mapping.source_tables {
            validate_identifier(source)?;
            if !seen_sources.contains(source) {
                return registry_error(format!(
                    "derived mapping {} has unknown source {source}",
                    mapping.target
                ));
            }
        }
        let mut target_columns = BTreeSet::new();
        for column in mapping.target_columns {
            validate_identifier(column)?;
            if !target_columns.insert(*column) {
                return registry_error(format!(
                    "derived mapping {} repeats target column {column}",
                    mapping.target
                ));
            }
        }
        for column in mapping.key_columns {
            validate_identifier(column)?;
            if !target_columns.contains(column) {
                return registry_error(format!(
                    "derived mapping {} key column {column} is not a target column",
                    mapping.target
                ));
            }
        }
    }
    for reference in SCALAR_REFERENCES {
        validate_identifier(reference.source_table)?;
        validate_identifier(reference.target_table)?;
        validate_identifier(reference.column)?;
        validate_identifier(reference.source_referenced_table)?;
        validate_identifier(reference.target_referenced_table)?;
        if !seen_sources.contains(reference.source_table)
            || !seen_sources.contains(reference.source_referenced_table)
            || !seen_targets.contains(reference.target_table)
            || !seen_targets.contains(reference.target_referenced_table)
            || !TABLE_MAPPINGS.iter().any(|mapping| {
                mapping.source == reference.source_table && mapping.target == reference.target_table
            })
            || !TABLE_MAPPINGS.iter().any(|mapping| {
                mapping.source == reference.source_referenced_table
                    && mapping.target == reference.target_referenced_table
            })
        {
            return registry_error(format!(
                "scalar reference {} -> {}.{} names an unregistered source or target table",
                reference.source_table, reference.target_table, reference.column
            ));
        }
    }
    let discarded_sources = DISCARDED_SOURCE_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if discarded_sources.len() != DISCARDED_SOURCE_TABLES.len()
        || !discarded_sources.is_disjoint(&seen_sources)
    {
        return registry_error("schema-v1 discard inventory is not the audited policy");
    }
    for table in DISCARDED_SOURCE_TABLES {
        validate_identifier(table)?;
    }
    let discarded_targets = DISCARDED_TARGET_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if discarded_targets.len() != DISCARDED_TARGET_TABLES.len()
        || !discarded_targets.is_disjoint(&seen_targets)
    {
        return registry_error("schema-v1 discarded target inventory is not unique and disjoint");
    }
    for table in DISCARDED_TARGET_TABLES {
        validate_identifier(table)?;
    }
    if ["v2_user", "v2_payment", "v2_server_group", "v2_stat"]
        .iter()
        .any(|table| discarded_sources.contains(table))
    {
        return registry_error("schema-v1 discards protected durable business data");
    }
    Ok(())
}

fn validate_identifier(identifier: &str) -> Result<(), ConverterError> {
    let valid = !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        registry_error(format!("unsafe SQL identifier {identifier:?}"))
    }
}

fn registry_error<T>(message: impl Into<String>) -> Result<T, ConverterError> {
    Err(ConverterError::Registry(message.into()))
}

/// Domain-separated identity of the current pre-release PostgreSQL schema.
/// Each entry binds its version, filename, and SQLx-style SHA-384 checksum.
pub fn target_postgres_schema_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.postgres-schema.v1\0");
    for (version, name, sql) in TARGET_POSTGRES_MIGRATIONS {
        digest.update(version.to_be_bytes());
        digest_field(&mut digest, name.as_bytes());
        digest_field(&mut digest, &Sha384::digest(sql));
    }
    hex::encode(digest.finalize())
}

pub fn registry_sha256() -> Result<String, ConverterError> {
    audit_registry()?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-mysql-import-registry-v1\0");
    digest.update(MYSQL_IMPORT_REGISTRY_VERSION.to_be_bytes());
    digest_field(&mut digest, MYSQL_IMPORT_SOURCE_PROFILE.as_bytes());
    digest_field(&mut digest, MYSQL_SOURCE_SCHEMA_SHA256.as_bytes());
    digest_field(&mut digest, MYSQL_SOURCE_INSTALL_SQL_SHA256.as_bytes());
    digest_field(&mut digest, TARGET_POSTGRES_SCHEMA_ID.as_bytes());
    digest_field(&mut digest, target_postgres_schema_sha256().as_bytes());
    for mapping in TABLE_MAPPINGS {
        digest_field(&mut digest, mapping.order.to_string().as_bytes());
        digest_field(&mut digest, mapping.source.as_bytes());
        digest_field(&mut digest, mapping.target.as_bytes());
        digest_field(
            &mut digest,
            format!("{:?}", mapping.identity_width).as_bytes(),
        );
        for column in mapping.direct_columns {
            digest_field(&mut digest, b"direct");
            digest_field(&mut digest, column.as_bytes());
        }
        for column in mapping.transformed_columns {
            digest_field(&mut digest, b"transform");
            digest_field(&mut digest, column.source.as_bytes());
            digest_field(&mut digest, column.target.as_bytes());
            digest_field(&mut digest, format!("{:?}", column.rule).as_bytes());
            digest_field(
                &mut digest,
                column.source_referenced_table.unwrap_or("").as_bytes(),
            );
            digest_field(
                &mut digest,
                column.referenced_target_table.unwrap_or("").as_bytes(),
            );
        }
        for column in mapping.added_columns {
            digest_field(&mut digest, b"added");
            digest_field(&mut digest, column.target.as_bytes());
            digest_field(&mut digest, format!("{:?}", column.value).as_bytes());
            digest_field(&mut digest, column.provenance.as_bytes());
        }
        for column in mapping.deferred_columns {
            digest_field(&mut digest, b"deferred");
            digest_field(&mut digest, column.source.as_bytes());
            digest_field(&mut digest, column.target.as_bytes());
            digest_field(&mut digest, column.source_referenced_table.as_bytes());
            digest_field(&mut digest, column.referenced_target_table.as_bytes());
            digest_field(&mut digest, column.reason.as_bytes());
        }
        for column in mapping.consumed_source_columns {
            digest_field(&mut digest, b"consumed");
            digest_field(&mut digest, column.source.as_bytes());
            digest_field(&mut digest, column.reason.as_bytes());
        }
    }
    for mapping in DERIVED_MAPPINGS {
        digest_field(&mut digest, mapping.order.to_string().as_bytes());
        digest_field(&mut digest, mapping.target.as_bytes());
        digest_field(&mut digest, format!("{:?}", mapping.kind).as_bytes());
        for table in mapping.source_tables {
            digest_field(&mut digest, table.as_bytes());
        }
        for column in mapping.target_columns {
            digest_field(&mut digest, b"target-column");
            digest_field(&mut digest, column.as_bytes());
        }
        for column in mapping.key_columns {
            digest_field(&mut digest, b"key-column");
            digest_field(&mut digest, column.as_bytes());
        }
        digest_field(&mut digest, mapping.rule.as_bytes());
    }
    digest_field(&mut digest, b"mysql-import-schema-v1");
    digest_field(&mut digest, MYSQL_IMPORT_POLICY_MARKER.as_bytes());
    for table in DISCARDED_SOURCE_TABLES {
        digest_field(&mut digest, b"discard-source-table");
        digest_field(&mut digest, table.as_bytes());
    }
    for table in DISCARDED_TARGET_TABLES {
        digest_field(&mut digest, b"empty-target-table");
        digest_field(&mut digest, table.as_bytes());
    }
    for reference in SCALAR_REFERENCES {
        digest_field(&mut digest, b"scalar-reference");
        digest_field(&mut digest, reference.source_table.as_bytes());
        digest_field(&mut digest, reference.target_table.as_bytes());
        digest_field(&mut digest, reference.column.as_bytes());
        digest_field(&mut digest, reference.source_referenced_table.as_bytes());
        digest_field(&mut digest, reference.target_referenced_table.as_bytes());
        digest_field(&mut digest, format!("{:?}", reference.rule).as_bytes());
    }
    for (table, columns) in TARGET_GENERATED_COLUMNS {
        digest_field(&mut digest, b"generated-target-columns");
        digest_field(&mut digest, table.as_bytes());
        for column in *columns {
            digest_field(&mut digest, column.as_bytes());
        }
    }
    digest_field(
        &mut digest,
        b"preserve-mysql-persisted-v2-user-u-d-and-permanent-token;never-fold-legacy-redis",
    );
    Ok(hex::encode(digest.finalize()))
}

fn transform_row(
    mapping: &TableMapping,
    source: &SourceRow,
) -> Result<CanonicalRow, ConverterError> {
    let expected = source_column_names(mapping);
    for column in source.keys() {
        if !expected.contains(column.as_str()) {
            return Err(ConverterError::UnconsumedColumn {
                table: mapping.source.to_string(),
                column: column.clone(),
            });
        }
    }
    let mut target = BTreeMap::new();
    for column in mapping.direct_columns {
        let value = required_source_value(mapping, source, column)?;
        target.insert((*column).to_string(), direct_value(value));
    }
    for column in mapping.transformed_columns {
        let value = required_source_value(mapping, source, column.source)?;
        let value = apply_rule(mapping, column, value)?;
        target.insert(column.target.to_string(), value);
    }
    for column in mapping.added_columns {
        let value = match column.value {
            AddedValue::Null => CanonicalValue::Null,
            AddedValue::I64(value) => CanonicalValue::I64(value),
        };
        target.insert(column.target.to_string(), value);
    }
    // Deferred and consumed columns must still be present in the source row so
    // a partial SELECT cannot accidentally look complete.
    for column in mapping.deferred_columns {
        required_source_value(mapping, source, column.source)?;
    }
    for column in mapping.consumed_source_columns {
        required_source_value(mapping, source, column.source)?;
    }
    Ok(target)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MysqlImportRowDisposition {
    Discard,
    Retain(CanonicalRow),
}

/// The only public base-row conversion entry point. Orders are checked against
/// the complete source payment index before the fixed Stripe loss policy is
/// applied: Stripe payment rows and status 0/1 Stripe orders are omitted, while
/// status 2/3/4 Stripe history is detached from provider fields.
pub fn transform_mysql_import_row(
    mapping: &TableMapping,
    source: &SourceRow,
    known_payment_ids: &BTreeSet<i32>,
    stripe_payment_ids: &BTreeSet<i32>,
) -> Result<MysqlImportRowDisposition, ConverterError> {
    match mapping.source {
        "v2_payment" => {
            let driver = required_source_value(mapping, source, "payment")?;
            let SourceValue::Text(driver) = driver else {
                return Err(ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: "payment".to_string(),
                    actual: source_kind(driver),
                });
            };
            if is_legacy_stripe_payment_driver(driver) {
                Ok(MysqlImportRowDisposition::Discard)
            } else {
                transform_row(mapping, source).map(MysqlImportRowDisposition::Retain)
            }
        }
        "v2_order" => {
            let status = source_i16(mapping, source, "status")?;
            let payment_id = source_optional_i32(mapping, source, "payment_id")?;
            let callback_no = source_optional_text(mapping, source, "callback_no")?;
            match classify_legacy_order(
                LegacyOrderBinding {
                    status,
                    payment_id,
                    callback_no,
                },
                known_payment_ids,
                stripe_payment_ids,
            )? {
                LegacyOrderDisposition::DiscardUnfinishedStripe => {
                    Ok(MysqlImportRowDisposition::Discard)
                }
                LegacyOrderDisposition::RetainUnchanged(_) => {
                    transform_row(mapping, source).map(MysqlImportRowDisposition::Retain)
                }
                LegacyOrderDisposition::RetainScrubbedStripe(_) => {
                    let mut row = transform_row(mapping, source)?;
                    row.insert("payment_id".to_string(), CanonicalValue::Null);
                    row.insert("callback_no".to_string(), CanonicalValue::Null);
                    row.insert("callback_no_hash".to_string(), CanonicalValue::Null);
                    Ok(MysqlImportRowDisposition::Retain(row))
                }
            }
        }
        _ => transform_row(mapping, source).map(MysqlImportRowDisposition::Retain),
    }
}

fn source_i16(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<i16, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    let converted = match value {
        SourceValue::I64(value) => i16::try_from(*value).ok(),
        SourceValue::U64(value) => i16::try_from(*value).ok(),
        _ => None,
    };
    converted.ok_or_else(|| ConverterError::TypeMismatch {
        table: mapping.source.to_string(),
        column: column.to_string(),
        actual: source_kind(value),
    })
}

fn source_optional_i32(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<Option<i32>, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    match value {
        SourceValue::Null => Ok(None),
        SourceValue::I64(value) => {
            i32::try_from(*value)
                .map(Some)
                .map_err(|_| ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: column.to_string(),
                    actual: "i64",
                })
        }
        SourceValue::U64(value) => {
            i32::try_from(*value)
                .map(Some)
                .map_err(|_| ConverterError::TypeMismatch {
                    table: mapping.source.to_string(),
                    column: column.to_string(),
                    actual: "u64",
                })
        }
        other => Err(ConverterError::TypeMismatch {
            table: mapping.source.to_string(),
            column: column.to_string(),
            actual: source_kind(other),
        }),
    }
}

fn source_optional_text(
    mapping: &TableMapping,
    source: &SourceRow,
    column: &str,
) -> Result<Option<String>, ConverterError> {
    let value = required_source_value(mapping, source, column)?;
    match value {
        SourceValue::Null => Ok(None),
        SourceValue::Text(value) => Ok(Some(value.clone())),
        other => Err(ConverterError::TypeMismatch {
            table: mapping.source.to_string(),
            column: column.to_string(),
            actual: source_kind(other),
        }),
    }
}

fn source_column_names(mapping: &TableMapping) -> BTreeSet<&str> {
    source_columns_in_order(mapping).into_iter().collect()
}

fn source_columns_in_order(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.source),
        )
        .chain(mapping.deferred_columns.iter().map(|column| column.source))
        .chain(
            mapping
                .consumed_source_columns
                .iter()
                .map(|column| column.source),
        )
        .collect()
}

fn required_source_value<'a>(
    mapping: &TableMapping,
    source: &'a SourceRow,
    column: &str,
) -> Result<&'a SourceValue, ConverterError> {
    source
        .get(column)
        .ok_or_else(|| ConverterError::MissingColumn {
            table: mapping.source.to_string(),
            column: column.to_string(),
        })
}

fn direct_value(value: &SourceValue) -> CanonicalValue {
    match value {
        SourceValue::Null => CanonicalValue::Null,
        SourceValue::I64(value) => CanonicalValue::I64(*value),
        SourceValue::U64(value) => CanonicalValue::U64(*value),
        SourceValue::Decimal(value) => CanonicalValue::Decimal(value.clone()),
        SourceValue::Text(value) => CanonicalValue::Text(value.clone()),
        SourceValue::Bytes(value) => CanonicalValue::Bytes(value.clone()),
    }
}

fn apply_rule(
    mapping: &TableMapping,
    column: &TransformColumn,
    source: &SourceValue,
) -> Result<CanonicalValue, ConverterError> {
    if matches!(source, SourceValue::Null) {
        return Ok(CanonicalValue::Null);
    }
    match column.rule {
        ColumnRule::ExactDecimal => {
            let text = match source {
                SourceValue::Decimal(value) | SourceValue::Text(value) => value,
                other => return type_mismatch(mapping, column, source_kind(other)),
            };
            let normalized =
                normalize_decimal(text).ok_or_else(|| ConverterError::InvalidDecimal {
                    table: mapping.source.to_string(),
                    column: column.source.to_string(),
                })?;
            Ok(CanonicalValue::Decimal(normalized))
        }
        ColumnRule::Json(shape) => {
            let text = source_text(mapping, column, source)?;
            let value = parse_json(mapping, column, text)?;
            if !json_shape_matches(&value, shape) {
                return Err(ConverterError::InvalidJson {
                    table: mapping.source.to_string(),
                    column: column.source.to_string(),
                    message: format!("top-level value is not {shape:?}"),
                });
            }
            Ok(CanonicalValue::Json(value))
        }
        ColumnRule::PositiveIdArray {
            maximum,
            require_non_empty,
            output,
        } => {
            let text = source_text(mapping, column, source)?;
            let value = normalize_id_array(mapping, column, text, maximum, require_non_empty)?;
            match output {
                JsonOutput::Json => Ok(CanonicalValue::Json(value)),
                JsonOutput::CanonicalText => Ok(CanonicalValue::Text(value.to_string())),
            }
        }
    }
}

fn source_text<'a>(
    mapping: &TableMapping,
    column: &TransformColumn,
    source: &'a SourceValue,
) -> Result<&'a str, ConverterError> {
    match source {
        SourceValue::Text(value) => Ok(value),
        other => type_mismatch(mapping, column, source_kind(other)),
    }
}

fn type_mismatch<T>(
    mapping: &TableMapping,
    column: &TransformColumn,
    actual: &'static str,
) -> Result<T, ConverterError> {
    Err(ConverterError::TypeMismatch {
        table: mapping.source.to_string(),
        column: column.source.to_string(),
        actual,
    })
}

fn source_kind(value: &SourceValue) -> &'static str {
    match value {
        SourceValue::Null => "null",
        SourceValue::I64(_) => "i64",
        SourceValue::U64(_) => "u64",
        SourceValue::Decimal(_) => "decimal",
        SourceValue::Text(_) => "text",
        SourceValue::Bytes(_) => "bytes",
    }
}

fn parse_json(
    mapping: &TableMapping,
    column: &TransformColumn,
    text: &str,
) -> Result<Value, ConverterError> {
    serde_json::from_str(text).map_err(|error| ConverterError::InvalidJson {
        table: mapping.source.to_string(),
        column: column.source.to_string(),
        message: error.to_string(),
    })
}

fn json_shape_matches(value: &Value, shape: JsonShape) -> bool {
    match shape {
        JsonShape::Any => true,
        JsonShape::Array => value.is_array(),
    }
}

fn normalize_id_array(
    mapping: &TableMapping,
    column: &TransformColumn,
    text: &str,
    maximum: u64,
    require_non_empty: bool,
) -> Result<Value, ConverterError> {
    let value = parse_json(mapping, column, text)?;
    let members = value
        .as_array()
        .ok_or_else(|| ConverterError::InvalidJson {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            message: "top-level value is not an array".to_string(),
        })?;
    if require_non_empty && members.is_empty() {
        return Err(ConverterError::InvalidJson {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            message: "array must not be empty".to_string(),
        });
    }
    let mut normalized = Vec::with_capacity(members.len());
    for (index, member) in members.iter().enumerate() {
        let id = match member {
            Value::Number(number) => number.as_u64(),
            Value::String(text) if canonical_positive_decimal(text) => text.parse::<u64>().ok(),
            _ => None,
        }
        .filter(|id| *id > 0 && *id <= maximum)
        .ok_or_else(|| ConverterError::InvalidIdArray {
            table: mapping.source.to_string(),
            column: column.source.to_string(),
            index,
        })?;
        normalized.push(Value::Number(Number::from(id)));
    }
    Ok(Value::Array(normalized))
}

fn canonical_positive_decimal(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && (b'1'..=b'9').contains(&bytes[0])
        && bytes[1..].iter().all(u8::is_ascii_digit)
}

fn normalize_decimal(value: &str) -> Option<String> {
    if value.is_empty()
        || value.starts_with('+')
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    if unsigned.is_empty() || unsigned.starts_with('+') {
        return None;
    }
    let split = unsigned.split_once('.');
    let (integer, fraction) = split.unwrap_or((unsigned, ""));
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || unsigned.matches('.').count() > 1
        || split.is_some_and(|_| fraction.is_empty())
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.trim_end_matches('0');
    let is_zero = integer == "0" && fraction.is_empty();
    let sign = if negative && !is_zero { "-" } else { "" };
    if fraction.is_empty() {
        Some(format!("{sign}{integer}"))
    } else {
        Some(format!("{sign}{integer}.{fraction}"))
    }
}

pub fn source_batch_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    let columns = source_columns_in_order(mapping)
        .into_iter()
        .map(mysql_identifier)
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "SELECT {columns} FROM {} WHERE `id` > ? ORDER BY `id` ASC LIMIT ?",
        mysql_identifier(mapping.source)
    ))
}

pub fn target_insert_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    target_batch_insert_sql(mapping, 1)
}

pub fn maximum_rows_per_target_insert(mapping: &TableMapping) -> Result<usize, ConverterError> {
    let column_count = target_insert_columns(mapping).len();
    if column_count == 0 {
        return registry_error(format!(
            "target table {} has no insert columns",
            mapping.target
        ));
    }
    Ok(POSTGRES_MAX_BIND_PARAMETERS / column_count)
}

pub fn target_batch_insert_sql(
    mapping: &TableMapping,
    row_count: usize,
) -> Result<String, ConverterError> {
    audit_registry()?;
    if row_count == 0 || row_count > MAX_BATCH_SIZE as usize {
        return Err(ConverterError::InvalidBatchSize(
            u32::try_from(row_count).unwrap_or(u32::MAX),
        ));
    }
    let columns = target_insert_columns(mapping);
    let maximum_rows = maximum_rows_per_target_insert(mapping)?;
    let parameter_count =
        columns
            .len()
            .checked_mul(row_count)
            .ok_or(ConverterError::TargetBatchParameterLimit {
                parameters: usize::MAX,
            })?;
    if row_count > maximum_rows || parameter_count > POSTGRES_MAX_BIND_PARAMETERS {
        return Err(ConverterError::TargetBatchParameterLimit {
            parameters: parameter_count,
        });
    }
    let quoted = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let parameters = (0..row_count)
        .map(|row| {
            let first = row * columns.len() + 1;
            let values = (first..first + columns.len())
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({values})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "INSERT INTO {} ({quoted}) VALUES {parameters}",
        postgres_identifier(mapping.target)
    ))
}

pub fn target_compare_row_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    let columns = target_insert_columns(mapping)
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "SELECT {columns} FROM {} WHERE {} = $1",
        postgres_identifier(mapping.target),
        postgres_identifier("id")
    ))
}

pub fn deferred_user_inviter_sql() -> &'static str {
    "UPDATE \"users\" SET \"invite_user_id\" = $2 WHERE \"id\" = $1 AND \"invite_user_id\" IS NOT DISTINCT FROM $3"
}

pub fn sequence_reset_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    Ok(format!(
        "SELECT setval(pg_get_serial_sequence('{table}', 'id'), GREATEST(COALESCE(MAX(id), 1), 1), MAX(id) IS NOT NULL AND MAX(id) >= 1) FROM {quoted}",
        table = mapping.target,
        quoted = postgres_identifier(mapping.target),
    ))
}

fn target_insert_columns(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.target),
        )
        .chain(mapping.added_columns.iter().map(|column| column.target))
        .collect()
}

fn mysql_identifier(identifier: &str) -> String {
    format!("`{identifier}`")
}

fn postgres_identifier(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

pub fn validate_batch_ids(
    mapping: &TableMapping,
    after_id: i64,
    rows: &[SourceRow],
    batch_size: u32,
) -> Result<i64, ConverterError> {
    if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
        return Err(ConverterError::InvalidBatchSize(batch_size));
    }
    if rows.len() > batch_size as usize {
        return Err(ConverterError::InvalidBatchSize(batch_size));
    }
    let mut previous = after_id;
    for row in rows {
        let id = match row.get("id") {
            Some(SourceValue::I64(id)) => *id,
            Some(SourceValue::U64(id)) => {
                i64::try_from(*id).map_err(|_| ConverterError::NonMonotonicBatch {
                    table: mapping.target.to_string(),
                    after_id,
                })?
            }
            _ => {
                return Err(ConverterError::NonMonotonicBatch {
                    table: mapping.target.to_string(),
                    after_id,
                });
            }
        };
        if id <= previous {
            return Err(ConverterError::NonMonotonicBatch {
                table: mapping.target.to_string(),
                after_id,
            });
        }
        previous = id;
    }
    Ok(previous)
}

pub fn canonical_rows_sha256(
    mapping: &TableMapping,
    rows: &[CanonicalRow],
) -> Result<String, ConverterError> {
    audit_registry()?;
    let columns = target_insert_columns(mapping);
    let mut digest = Sha256::new();
    digest.update(b"v2board-mysql-import-canonical-rows-v1\0");
    digest_field(&mut digest, mapping.target.as_bytes());
    for row in rows {
        for column in &columns {
            digest_field(&mut digest, column.as_bytes());
            let value = row
                .get(*column)
                .ok_or_else(|| ConverterError::MissingColumn {
                    table: mapping.target.to_string(),
                    column: (*column).to_string(),
                })?;
            digest_canonical_value(&mut digest, value);
        }
    }
    Ok(hex::encode(digest.finalize()))
}

/// Expands legacy set-valued redemption ids without inventing a historical
/// timestamp. The PostgreSQL baseline explicitly reserves `(0,
/// "legacy_unknown")` for this representation. Output is sorted and deduped;
/// malformed or missing users fail closed.
pub fn expand_giftcard_redemptions(
    giftcard_id: i32,
    used_user_ids: &SourceValue,
) -> Result<Vec<LegacyGiftcardRedemptionRow>, ConverterError> {
    if matches!(used_user_ids, SourceValue::Null) {
        return Ok(Vec::new());
    }
    let mapping = mapping_for_source("v2_giftcard")
        .ok_or_else(|| ConverterError::UnknownTable("v2_giftcard".to_string()))?;
    let column = TransformColumn {
        source: "used_user_ids",
        target: "user_id",
        rule: ColumnRule::PositiveIdArray {
            maximum: i64::MAX as u64,
            require_non_empty: false,
            output: JsonOutput::Json,
        },
        source_referenced_table: Some("v2_user"),
        referenced_target_table: Some("users"),
    };
    let text = source_text(mapping, &column, used_user_ids)?;
    let normalized = normalize_id_array(mapping, &column, text, i64::MAX as u64, false)?;
    let members = normalized.as_array().ok_or_else(|| {
        ConverterError::Registry("id-array normalizer returned a non-array".into())
    })?;
    let ids = members
        .iter()
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                ConverterError::Registry("id-array normalizer returned a non-u64".into())
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut rows = Vec::with_capacity(ids.len());
    for user_id in &ids {
        rows.push(LegacyGiftcardRedemptionRow {
            giftcard_id,
            user_id: i64::try_from(*user_id).map_err(|_| {
                ConverterError::Registry("id-array normalizer exceeded i64::MAX".into())
            })?,
            created_at: 0,
            created_at_provenance: "legacy_unknown".to_string(),
        });
    }
    Ok(rows)
}

fn digest_canonical_value(digest: &mut Sha256, value: &CanonicalValue) {
    match value {
        CanonicalValue::Null => digest_field(digest, b"null"),
        CanonicalValue::I64(value) => digest_field(digest, format!("i:{value}").as_bytes()),
        CanonicalValue::U64(value) => digest_field(digest, format!("u:{value}").as_bytes()),
        CanonicalValue::Decimal(value) => digest_field(digest, format!("d:{value}").as_bytes()),
        CanonicalValue::Text(value) => {
            digest_field(digest, b"text");
            digest_field(digest, value.as_bytes());
        }
        CanonicalValue::Bytes(value) => {
            digest_field(digest, b"bytes");
            digest_field(digest, value);
        }
        CanonicalValue::Json(value) => {
            digest_field(digest, b"json");
            digest_field(digest, value.to_string().as_bytes());
        }
    }
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete_and_stable() {
        audit_registry().expect("registry");
        assert_eq!(TABLE_MAPPINGS.len(), 14);
        assert_eq!(registry_sha256().expect("hash").len(), 64);
        assert!(SCALAR_REFERENCES.contains(&ScalarReference {
            source_table: "v2_order",
            target_table: "orders",
            column: "payment_id",
            source_referenced_table: "v2_payment",
            target_referenced_table: "payment_method",
            rule: ScalarReferenceRule::Nullable,
        }));
    }

    #[test]
    fn public_row_conversion_enforces_the_stripe_policy() {
        let payment_ids = BTreeSet::from([7, 99]);
        let stripe_ids = BTreeSet::from([7]);

        let mut stripe_payment = complete_source_row(&PAYMENT);
        stripe_payment.insert(
            "payment".to_string(),
            SourceValue::Text("StripeCredit".to_string()),
        );
        assert_eq!(
            transform_mysql_import_row(&PAYMENT, &stripe_payment, &payment_ids, &stripe_ids)
                .unwrap(),
            MysqlImportRowDisposition::Discard
        );

        let mut manual_payment = stripe_payment.clone();
        manual_payment.insert("payment".to_string(), SourceValue::Text("EPay".to_string()));
        assert!(matches!(
            transform_mysql_import_row(&PAYMENT, &manual_payment, &payment_ids, &stripe_ids)
                .unwrap(),
            MysqlImportRowDisposition::Retain(_)
        ));

        let mut order = complete_source_row(&ORDER);
        order.insert("payment_id".to_string(), SourceValue::I64(7));
        order.insert(
            "callback_no".to_string(),
            SourceValue::Text("pi_legacy".to_string()),
        );
        for status in [0, 1] {
            order.insert("status".to_string(), SourceValue::I64(status));
            assert_eq!(
                transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids).unwrap(),
                MysqlImportRowDisposition::Discard
            );
        }
        for status in [2, 3, 4] {
            order.insert("status".to_string(), SourceValue::I64(status));
            let MysqlImportRowDisposition::Retain(row) =
                transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids).unwrap()
            else {
                panic!("terminal Stripe history must be retained");
            };
            assert_eq!(row.get("payment_id"), Some(&CanonicalValue::Null));
            assert_eq!(row.get("callback_no"), Some(&CanonicalValue::Null));
            assert_eq!(row.get("callback_no_hash"), Some(&CanonicalValue::Null));
        }
        order.insert("status".to_string(), SourceValue::I64(5));
        assert!(matches!(
            transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids),
            Err(ConverterError::OrderPolicy(
                LegacyOrderPolicyError::UnsupportedStripeStatus(5)
            ))
        ));

        order.insert("status".to_string(), SourceValue::I64(3));
        order.insert("payment_id".to_string(), SourceValue::I64(404));
        assert_eq!(
            transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids),
            Err(ConverterError::OrderPolicy(
                LegacyOrderPolicyError::UnknownPaymentId(404)
            ))
        );
    }

    fn complete_source_row(mapping: &TableMapping) -> SourceRow {
        let mut row = SourceRow::new();
        for column in mapping.direct_columns {
            row.insert((*column).to_string(), SourceValue::I64(1));
        }
        for column in mapping.transformed_columns {
            let value = match column.rule {
                ColumnRule::ExactDecimal => SourceValue::Text("1".to_string()),
                ColumnRule::Json(JsonShape::Any) => SourceValue::Text("{}".to_string()),
                ColumnRule::Json(JsonShape::Array) => SourceValue::Text("[]".to_string()),
                ColumnRule::PositiveIdArray {
                    require_non_empty, ..
                } => SourceValue::Text(if require_non_empty { "[1]" } else { "[]" }.to_string()),
            };
            row.insert(column.source.to_string(), value);
        }
        for column in mapping.deferred_columns {
            row.insert(column.source.to_string(), SourceValue::Null);
        }
        for column in mapping.consumed_source_columns {
            row.insert(column.source.to_string(), SourceValue::Null);
        }
        row
    }

    #[test]
    fn schema_v1_discards_only_audited_operational_history() {
        assert_eq!(MYSQL_IMPORT_SCHEMA_VERSION, 1);
        assert_eq!(
            copied_table_mappings()
                .map(|mapping| mapping.source)
                .collect::<BTreeSet<_>>(),
            TABLE_MAPPINGS
                .iter()
                .map(|mapping| mapping.source)
                .collect()
        );
        assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_server_group"));
        assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_stat"));
        assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_user"));
        assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_payment"));
        assert_eq!(
            TARGET_GENERATED_COLUMNS,
            &[("orders", &["referenced_plan_id"] as &[&str])]
        );
        assert!(
            TABLE_MAPPINGS
                .iter()
                .all(|mapping| !mapping.target.starts_with("v2_"))
        );
        assert!(
            DERIVED_MAPPINGS
                .iter()
                .all(|mapping| !mapping.target.starts_with("v2_"))
        );
        assert!(
            DISCARDED_TARGET_TABLES
                .iter()
                .all(|table| !table.starts_with("v2_"))
        );
        assert_eq!(
            DISCARDED_SOURCE_TABLES,
            [
                "failed_jobs",
                "v2_log",
                "v2_mail_log",
                "v2_stat_server",
                "v2_stat_user",
                "v2_server_route",
                "v2_server_shadowsocks",
                "v2_server_vmess",
                "v2_server_trojan",
                "v2_server_tuic",
                "v2_server_hysteria",
                "v2_server_vless",
                "v2_server_anytls",
                "v2_server_v2node",
            ]
        );
        assert_eq!(discarded_target_tables().count(), 14);
        assert_eq!(
            discarded_target_tables().collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "system_log",
                "mail_log",
                "server_anytls",
                "server_credential",
                "server_hysteria",
                "server_route",
                "server_shadowsocks",
                "server_trojan",
                "server_tuic",
                "server_v2node",
                "server_vless",
                "server_vmess",
                "server_traffic",
                "user_traffic",
            ])
        );
        for table in DISCARDED_SOURCE_TABLES {
            assert!(mapping_for_source(table).is_none());
        }
        assert_eq!(built_derived_mappings().count(), 1);
    }

    #[test]
    fn permanent_user_credentials_and_counters_are_direct() {
        let user = mapping_for_source("v2_user").expect("user mapping");
        for column in [
            "id",
            "email",
            "password",
            "password_algo",
            "password_salt",
            "uuid",
            "token",
            "balance",
            "commission_balance",
            "t",
            "u",
            "d",
            "transfer_enable",
        ] {
            assert!(user.direct_columns.contains(&column), "missing {column}");
        }
        assert_eq!(user.deferred_columns[0].source, "invite_user_id");
    }

    #[test]
    fn positive_id_arrays_normalize_only_canonical_decimal_strings() {
        let mapping = mapping_for_source("v2_coupon").expect("coupon");
        let column = &mapping.transformed_columns[0];
        let value = normalize_id_array(mapping, column, r#"[1,"2",3]"#, i32::MAX as u64, false)
            .expect("valid ids");
        assert_eq!(value, serde_json::json!([1, 2, 3]));

        for invalid in [r#"[0]"#, r#"["01"]"#, r#"[" 1"]"#, r#"[1.0]"#, r#"null"#] {
            assert!(
                normalize_id_array(mapping, column, invalid, i32::MAX as u64, false).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn decimal_normalization_never_uses_float() {
        assert_eq!(normalize_decimal("001.2300"), Some("1.23".to_string()));
        assert_eq!(normalize_decimal("-0.00"), Some("0".to_string()));
        assert_eq!(
            normalize_decimal("9007199254740993.25"),
            Some("9007199254740993.25".to_string())
        );
        assert_eq!(normalize_decimal("1e3"), None);
        assert_eq!(normalize_decimal(" 1"), None);
    }

    #[test]
    fn sql_is_keyset_and_requires_an_empty_target() {
        let user = mapping_for_source("v2_user").expect("user");
        let source_sql = source_batch_sql(user).expect("source sql");
        assert!(source_sql.contains("WHERE `id` > ? ORDER BY `id` ASC LIMIT ?"));
        assert!(!source_sql.contains("OFFSET"));
        assert_eq!(INITIAL_SOURCE_ID_CURSOR, 0);

        let target_sql = target_insert_sql(user).expect("target sql");
        assert!(source_sql.contains("FROM `v2_user`"));
        assert!(target_sql.starts_with("INSERT INTO \"users\""));
        assert!(target_sql.contains(" VALUES ("));
        assert!(!target_sql.contains("invite_user_id"));

        let target_batch = target_batch_insert_sql(user, 3).expect("target batch");
        assert!(target_batch.contains("), ($"));
        assert!(
            maximum_rows_per_target_insert(user).expect("target batch limit")
                >= DEFAULT_BATCH_SIZE as usize
        );

        let reset = sequence_reset_sql(user).expect("sequence reset");
        assert!(reset.contains("GREATEST(COALESCE(MAX(id), 1), 1)"));
    }

    #[test]
    fn giftcard_redemptions_have_explicit_unknown_time_provenance() {
        let rows = expand_giftcard_redemptions(3, &SourceValue::Text(r#"[9,"7",9]"#.to_string()))
            .expect("redemptions");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user_id, 7);
        assert_eq!(rows[0].created_at, 0);
        assert_eq!(rows[0].created_at_provenance, "legacy_unknown");
    }
}

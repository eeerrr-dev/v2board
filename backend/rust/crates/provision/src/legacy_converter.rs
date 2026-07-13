//! Deterministic, one-shot conversion core for the pinned legacy database.
//!
//! This module deliberately contains no CDC, dual-write, shadow-read, or
//! gradual-cutover path. A caller must bind it to one fenced, repeatable-read
//! source snapshot and to a durable operation journal before the target is
//! mutated. The SQL adapters and lifecycle orchestration live outside this
//! module; the registry and state checks here are the auditable conversion
//! contract shared by those adapters.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use sha2::{Digest, Sha256, Sha384};

pub const LEGACY_CONVERTER_PROFILE: &str =
    "wyx2685-v2board@7e77de9f4873b317157490529f7be7d6f8a62421";
pub const LEGACY_SEMANTIC_SCHEMA_SHA256: &str =
    "4b5eaec681531751c79b48188e5a1c665df4f660dffbb88d6853cea6cf04801e";
pub const LEGACY_INSTALL_SQL_SHA256: &str =
    "04b04531037b9e0b6f2a6b02194a8f1bc102789af8ee7be963fd721d51bca8e2";
pub const TARGET_POSTGRES_LINEAGE: &str = "migrations-postgres/v2";
pub const TARGET_POSTGRES_LINEAGE_SHA256: &str =
    "2b3ac5c043a36438ae9e9547635ed463176e30c59b41b97a44b99d847434effd";
pub const CONVERTER_REGISTRY_VERSION: u32 = 2;
pub const DEFAULT_BATCH_SIZE: u32 = 1_000;
pub const MAX_BATCH_SIZE: u32 = 100_000;
pub const POSTGRES_MAX_BIND_PARAMETERS: usize = 65_535;
/// The first keyset query starts below every signed legacy identity. Do not
/// use zero: `NO_AUTO_VALUE_ON_ZERO` allowed explicitly inserted zero ids.
pub const INITIAL_SOURCE_ID_CURSOR: i64 = i64::MIN;

const TARGET_POSTGRES_MIGRATIONS: &[(i64, &str, &[u8])] = &[
    (
        1,
        "0001_initial.sql",
        include_bytes!("../../../migrations-postgres/0001_initial.sql"),
    ),
    (
        2,
        "0002_legacy_lifecycle_and_analytics_admission.sql",
        include_bytes!(
            "../../../migrations-postgres/0002_legacy_lifecycle_and_analytics_admission.sql"
        ),
    ),
];

const DECIMAL: ColumnRule = ColumnRule::ExactDecimal;
const JSON_ANY: ColumnRule = ColumnRule::Json(JsonShape::Any);
const JSON_ARRAY: ColumnRule = ColumnRule::Json(JsonShape::Array);
const JSON_STRING: ColumnRule = ColumnRule::TextAsJsonString;
const ID_ARRAY_I32: ColumnRule = ColumnRule::PositiveIdArray {
    maximum: i32::MAX as u64,
    require_non_empty: false,
    output: JsonOutput::Json,
};
const NON_EMPTY_ID_ARRAY_I32: ColumnRule = ColumnRule::PositiveIdArray {
    maximum: i32::MAX as u64,
    require_non_empty: true,
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
    Object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonOutput {
    Json,
    CanonicalText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnRule {
    /// Preserve the typed value exactly. The SQL adapter is responsible for
    /// lossless width conversion and must never route integers through f64.
    Direct,
    /// Parse and bind a base-10 fixed-point value without a binary float.
    ExactDecimal,
    /// Parse legacy JSON text and require the declared top-level shape.
    Json(JsonShape),
    /// Preserve legacy text as a JSON string (not as parsed JSON).
    TextAsJsonString,
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
    /// An optional target table whose `id` set must contain every member.
    /// Reference validation is a pre-copy and final-verification obligation;
    /// row transformation alone cannot prove it.
    pub referenced_table: Option<&'static str>,
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
    pub referenced_table: &'static str,
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
    NodeCredentials,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedMapping {
    pub order: u16,
    pub target: &'static str,
    pub kind: DerivedMappingKind,
    pub source_tables: &'static [&'static str],
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
    pub table: &'static str,
    pub column: &'static str,
    pub referenced_table: &'static str,
    pub rule: ScalarReferenceRule,
}

const SERVER_GROUP: TableMapping = TableMapping {
    order: 10,
    source: "v2_server_group",
    target: "v2_server_group",
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
    target: "v2_plan",
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
    target: "v2_payment",
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
            referenced_table: None,
        },
        TransformColumn {
            source: "handling_fee_percent",
            target: "handling_fee_percent",
            rule: DECIMAL,
            referenced_table: None,
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
    target: "v2_coupon",
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
            referenced_table: Some("v2_plan"),
        },
        TransformColumn {
            source: "limit_period",
            target: "limit_period",
            rule: JSON_ARRAY,
            referenced_table: None,
        },
    ],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const USER: TableMapping = TableMapping {
    order: 50,
    source: "v2_user",
    target: "v2_user",
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
        referenced_table: "v2_user",
        reason: "self references are patched after every user id exists; NULL and cycles remain exact",
    }],
    consumed_source_columns: &[],
};

const ORDER: TableMapping = TableMapping {
    order: 60,
    source: "v2_order",
    target: "v2_order",
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
        referenced_table: Some("v2_order"),
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
    target: "v2_commission_log",
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
    target: "v2_invite_code",
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
    target: "v2_giftcard",
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
        reason: "expanded by the v2_giftcard_redemption derived mapping",
    }],
};

macro_rules! direct_table {
    ($name:ident, $order:literal, $table:literal, $width:ident, [$($column:literal),+ $(,)?]) => {
        const $name: TableMapping = TableMapping {
            order: $order,
            source: $table,
            target: $table,
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
    target: "v2_notice",
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
        referenced_table: None,
    }],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

direct_table!(
    TICKET,
    120,
    "v2_ticket",
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
    LOG,
    140,
    "v2_log",
    I64,
    [
        "id",
        "title",
        "level",
        "host",
        "uri",
        "method",
        "data",
        "ip",
        "context",
        "created_at",
        "updated_at"
    ]
);

direct_table!(
    MAIL_LOG,
    150,
    "v2_mail_log",
    I64,
    [
        "id",
        "email",
        "subject",
        "template_name",
        "error",
        "created_at",
        "updated_at"
    ]
);

direct_table!(
    STAT,
    160,
    "v2_stat",
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

direct_table!(
    STAT_SERVER,
    170,
    "v2_stat_server",
    I64,
    [
        "id",
        "server_id",
        "server_type",
        "u",
        "d",
        "record_type",
        "record_at",
        "created_at",
        "updated_at"
    ]
);

const STAT_USER: TableMapping = TableMapping {
    order: 180,
    source: "v2_stat_user",
    target: "v2_stat_user",
    identity_width: IdentityWidth::I64,
    direct_columns: &[
        "id",
        "user_id",
        "u",
        "d",
        "record_type",
        "record_at",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[TransformColumn {
        source: "server_rate",
        target: "server_rate",
        rule: DECIMAL,
        referenced_table: None,
    }],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

const SERVER_ROUTE: TableMapping = TableMapping {
    order: 190,
    source: "v2_server_route",
    target: "v2_server_route",
    identity_width: IdentityWidth::I32,
    direct_columns: &["id", "remarks", "action", "created_at", "updated_at"],
    transformed_columns: &[
        TransformColumn {
            source: "match",
            target: "match",
            rule: JSON_ARRAY,
            referenced_table: None,
        },
        TransformColumn {
            source: "action_value",
            target: "action_value",
            rule: JSON_STRING,
            referenced_table: None,
        },
    ],
    added_columns: &[],
    deferred_columns: &[],
    consumed_source_columns: &[],
};

macro_rules! node_table {
    (
        $name:ident, $order:literal, $table:literal,
        direct [$($direct:literal),+ $(,)?],
        json [$($json:literal),* $(,)?]
    ) => {
        const $name: TableMapping = TableMapping {
            order: $order,
            source: $table,
            target: $table,
            identity_width: IdentityWidth::I32,
            direct_columns: &[$($direct),+],
            transformed_columns: &[
                TransformColumn {
                    source: "group_id",
                    target: "group_id",
                    rule: NON_EMPTY_ID_ARRAY_I32,
                    referenced_table: Some("v2_server_group"),
                },
                TransformColumn {
                    source: "route_id",
                    target: "route_id",
                    rule: ID_ARRAY_I32,
                    // Legacy deliberately tolerated deleted/missing optional
                    // routes. Preserve the ids; do not invent or drop them.
                    referenced_table: None,
                },
                TransformColumn {
                    source: "tags",
                    target: "tags",
                    rule: JSON_ARRAY,
                    referenced_table: None,
                },
                $(TransformColumn {
                    source: $json,
                    target: $json,
                    rule: JSON_ANY,
                    referenced_table: None,
                },)*
            ],
            added_columns: &[],
            deferred_columns: &[],
            consumed_source_columns: &[],
        };
    };
}

node_table!(
    SERVER_SHADOWSOCKS,
    200,
    "v2_server_shadowsocks",
    direct [
        "id", "parent_id", "name", "rate", "host", "port", "server_port", "cipher", "obfs",
        "show", "sort", "created_at", "updated_at"
    ],
    json ["obfs_settings"]
);

node_table!(
    SERVER_VMESS,
    210,
    "v2_server_vmess",
    direct [
        "id", "name", "parent_id", "host", "port", "server_port", "tls", "rate", "network",
        "show", "sort", "created_at", "updated_at"
    ],
    json [
        "rules",
        "networkSettings",
        "tlsSettings",
        "ruleSettings",
        "dnsSettings"
    ]
);

node_table!(
    SERVER_TROJAN,
    220,
    "v2_server_trojan",
    direct [
        "id", "parent_id", "name", "rate", "host", "port", "server_port", "network",
        "allow_insecure", "server_name", "show", "sort", "created_at", "updated_at"
    ],
    json ["network_settings"]
);

node_table!(
    SERVER_TUIC,
    230,
    "v2_server_tuic",
    direct [
        "id", "name", "parent_id", "host", "port", "server_port", "rate", "show", "sort",
        "server_name", "insecure", "disable_sni", "udp_relay_mode", "zero_rtt_handshake",
        "congestion_control", "created_at", "updated_at"
    ],
    json []
);

node_table!(
    SERVER_HYSTERIA,
    240,
    "v2_server_hysteria",
    direct [
        "id", "version", "name", "parent_id", "host", "port", "server_port", "rate", "show",
        "sort", "up_mbps", "down_mbps", "obfs", "obfs_password", "server_name", "insecure",
        "created_at", "updated_at"
    ],
    json []
);

node_table!(
    SERVER_VLESS,
    250,
    "v2_server_vless",
    direct [
        "id", "name", "parent_id", "host", "port", "server_port", "tls", "flow", "network",
        "encryption", "rate", "show", "sort", "created_at", "updated_at"
    ],
    json ["tls_settings", "network_settings", "encryption_settings"]
);

node_table!(
    SERVER_ANYTLS,
    260,
    "v2_server_anytls",
    direct [
        "id", "name", "parent_id", "host", "port", "server_port", "rate", "show", "sort",
        "server_name", "insecure", "created_at", "updated_at"
    ],
    json ["padding_scheme"]
);

node_table!(
    SERVER_V2NODE,
    270,
    "v2_server_v2node",
    direct [
        "id", "name", "parent_id", "host", "listen_ip", "port", "server_port", "rate", "show",
        "sort", "protocol", "tls", "flow", "network", "encryption", "disable_sni",
        "udp_relay_mode", "zero_rtt_handshake", "congestion_control", "cipher", "up_mbps",
        "down_mbps", "obfs", "obfs_password", "created_at", "updated_at"
    ],
    json [
        "tls_settings",
        "network_settings",
        "encryption_settings",
        "padding_scheme"
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
    LOG,
    MAIL_LOG,
    STAT,
    STAT_SERVER,
    STAT_USER,
    SERVER_ROUTE,
    SERVER_SHADOWSOCKS,
    SERVER_VMESS,
    SERVER_TROJAN,
    SERVER_TUIC,
    SERVER_HYSTERIA,
    SERVER_VLESS,
    SERVER_ANYTLS,
    SERVER_V2NODE,
];

pub const DERIVED_MAPPINGS: &[DerivedMapping] = &[
    DerivedMapping {
        order: 280,
        target: "v2_giftcard_redemption",
        kind: DerivedMappingKind::GiftcardRedemptions,
        source_tables: &["v2_giftcard", "v2_user"],
        key_columns: &["giftcard_id", "user_id"],
        rule: "expand distinct used_user_ids; every id must exist; created_at=0 and created_at_provenance=legacy_unknown",
    },
    DerivedMapping {
        order: 290,
        target: "v2_server_credential",
        kind: DerivedMappingKind::NodeCredentials,
        source_tables: &[
            "v2_server_shadowsocks",
            "v2_server_vmess",
            "v2_server_trojan",
            "v2_server_tuic",
            "v2_server_hysteria",
            "v2_server_vless",
            "v2_server_anytls",
            "v2_server_v2node",
        ],
        key_columns: &["node_type", "node_id"],
        rule: "one row per source node; credential_epoch=0; updated_at=source node updated_at",
    },
];

pub const NODE_CREDENTIAL_SOURCES: &[(&str, &str)] = &[
    ("shadowsocks", "v2_server_shadowsocks"),
    ("vmess", "v2_server_vmess"),
    ("trojan", "v2_server_trojan"),
    ("tuic", "v2_server_tuic"),
    ("hysteria", "v2_server_hysteria"),
    ("vless", "v2_server_vless"),
    ("anytls", "v2_server_anytls"),
    ("v2node", "v2_server_v2node"),
];

/// Scalar relationships that must be proven before copy and again against the
/// target. Historical ids intentionally lacking a PostgreSQL FK (for example
/// commission/stat rows and optional deleted route ids) are value-verified but
/// are not silently re-parented.
pub const SCALAR_REFERENCES: &[ScalarReference] = &[
    ScalarReference {
        table: "v2_plan",
        column: "group_id",
        referenced_table: "v2_server_group",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        table: "v2_user",
        column: "invite_user_id",
        referenced_table: "v2_user",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        table: "v2_user",
        column: "group_id",
        referenced_table: "v2_server_group",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        table: "v2_user",
        column: "plan_id",
        referenced_table: "v2_plan",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        table: "v2_order",
        column: "user_id",
        referenced_table: "v2_user",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        table: "v2_order",
        column: "plan_id",
        referenced_table: "v2_plan",
        rule: ScalarReferenceRule::ZeroMeansNoReference,
    },
    ScalarReference {
        table: "v2_invite_code",
        column: "user_id",
        referenced_table: "v2_user",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        table: "v2_giftcard",
        column: "plan_id",
        referenced_table: "v2_plan",
        rule: ScalarReferenceRule::Nullable,
    },
    ScalarReference {
        table: "v2_ticket",
        column: "user_id",
        referenced_table: "v2_user",
        rule: ScalarReferenceRule::Required,
    },
    ScalarReference {
        table: "v2_ticket_message",
        column: "ticket_id",
        referenced_table: "v2_ticket",
        rule: ScalarReferenceRule::Required,
    },
];

/// Native-only tables that a legacy bulk conversion must leave empty. The
/// lifecycle bootstrap owns `v2_system_installation`; the other tables start
/// empty because no corresponding durable legacy fact exists.
pub const TARGET_ONLY_TABLES: &[&str] = &[
    "v2_system_installation",
    "v2_lifecycle_operation",
    "v2_lifecycle_event",
    "v2_lifecycle_activation_commit",
    "v2_legacy_copy_checkpoint",
    "v2_legacy_traffic_fold_item",
    "v2_legacy_traffic_fold",
    "v2_payment_reconciliation",
    "v2_mail_outbox_batch",
    "v2_mail_outbox",
    "v2_analytics_admission_policy",
    "v2_analytics_admission_state",
    "v2_analytics_delivery_batch",
    "v2_analytics_outbox",
    "v2_server_traffic_report",
    "v2_server_traffic_report_item",
];

/// Columns intentionally omitted from inserts because PostgreSQL derives them
/// from preserved legacy values. Final verification must still compare their
/// evaluated meaning.
pub const TARGET_GENERATED_COLUMNS: &[(&str, &[&str])] = &[
    ("v2_order", &["referenced_plan_id", "unfinished_user_id"]),
    ("v2_ticket", &["open_user_id"]),
];

pub const DRAINED_SOURCE_TABLES: &[&str] = &["failed_jobs"];

#[derive(Clone, Debug, Eq, PartialEq)]
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
    #[error("batch ids for {table} are not strictly increasing after checkpoint {after_id}")]
    NonMonotonicBatch { table: String, after_id: i64 },
    #[error("batch size {0} is outside 1..={MAX_BATCH_SIZE}")]
    InvalidBatchSize(u32),
    #[error("target batch has {parameters} bind parameters, above PostgreSQL's supported limit")]
    TargetBatchParameterLimit { parameters: usize },
    #[error("source schema hash does not match the pinned profile")]
    SourceSchemaMismatch,
    #[error("source snapshot fingerprint must be lowercase SHA-256")]
    InvalidSnapshotFingerprint,
    #[error("checkpoint binding does not match this conversion run")]
    CheckpointBindingMismatch,
    #[error("checkpoint phase transition is not monotonic")]
    CheckpointRegression,
    #[error("target row conflicts with the expected canonical row")]
    RetryConflict,
    #[error("{table}.{column} references missing {referenced_table} id {id}")]
    MissingIdReference {
        table: String,
        column: String,
        referenced_table: String,
        id: u64,
    },
}

pub fn mapping_for_source(table: &str) -> Option<&'static TableMapping> {
    TABLE_MAPPINGS
        .iter()
        .find(|mapping| mapping.source == table)
}

pub fn audit_registry() -> Result<(), ConverterError> {
    if target_postgres_lineage_sha256() != TARGET_POSTGRES_LINEAGE_SHA256 {
        return registry_error("target PostgreSQL migration lineage digest changed");
    }
    let expected_source_tables = [
        "v2_commission_log",
        "v2_coupon",
        "v2_giftcard",
        "v2_invite_code",
        "v2_knowledge",
        "v2_log",
        "v2_mail_log",
        "v2_notice",
        "v2_order",
        "v2_payment",
        "v2_plan",
        "v2_server_anytls",
        "v2_server_group",
        "v2_server_hysteria",
        "v2_server_route",
        "v2_server_shadowsocks",
        "v2_server_trojan",
        "v2_server_tuic",
        "v2_server_v2node",
        "v2_server_vless",
        "v2_server_vmess",
        "v2_stat",
        "v2_stat_server",
        "v2_stat_user",
        "v2_ticket",
        "v2_ticket_message",
        "v2_user",
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
            validate_identifier(column.referenced_table)?;
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
    if !TABLE_MAPPINGS
        .windows(2)
        .all(|pair| pair[0].order < pair[1].order)
    {
        return registry_error("table mappings are not stored in strict execution order");
    }
    for mapping in TABLE_MAPPINGS {
        for column in mapping.transformed_columns {
            if column
                .referenced_table
                .is_some_and(|table| !seen_targets.contains(table))
            {
                return registry_error(format!(
                    "{}.{} references an unregistered target table",
                    mapping.source, column.source
                ));
            }
        }
        for column in mapping.deferred_columns {
            if !seen_targets.contains(column.referenced_table) {
                return registry_error(format!(
                    "{}.{} defers to an unregistered target table",
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
        if mapping.source_tables.is_empty() || mapping.key_columns.is_empty() {
            return registry_error(format!(
                "derived mapping {} has no source or key",
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
        for column in mapping.key_columns {
            validate_identifier(column)?;
        }
    }
    for reference in SCALAR_REFERENCES {
        validate_identifier(reference.table)?;
        validate_identifier(reference.column)?;
        validate_identifier(reference.referenced_table)?;
        if !seen_targets.contains(reference.table)
            || !seen_targets.contains(reference.referenced_table)
        {
            return registry_error(format!(
                "scalar reference {}.{} names an unregistered table",
                reference.table, reference.column
            ));
        }
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

/// Domain-separated identity of the complete ordered PostgreSQL migration
/// lineage. Each entry binds its version, immutable filename, and SQLx-style
/// SHA-384 content checksum; adding a migration necessarily changes this
/// digest without rewriting a historical migration.
pub fn target_postgres_lineage_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.postgres-target-lineage.v1\0");
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
    digest.update(b"v2board-legacy-converter-registry-v1\0");
    digest.update(CONVERTER_REGISTRY_VERSION.to_be_bytes());
    digest_field(&mut digest, LEGACY_CONVERTER_PROFILE.as_bytes());
    digest_field(&mut digest, LEGACY_SEMANTIC_SCHEMA_SHA256.as_bytes());
    digest_field(&mut digest, LEGACY_INSTALL_SQL_SHA256.as_bytes());
    digest_field(&mut digest, TARGET_POSTGRES_LINEAGE.as_bytes());
    digest_field(&mut digest, TARGET_POSTGRES_LINEAGE_SHA256.as_bytes());
    for mapping in TABLE_MAPPINGS {
        digest_field(&mut digest, mapping.order.to_string().as_bytes());
        digest_field(&mut digest, mapping.source.as_bytes());
        digest_field(&mut digest, mapping.target.as_bytes());
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
                column.referenced_table.unwrap_or("").as_bytes(),
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
            digest_field(&mut digest, column.referenced_table.as_bytes());
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
        for column in mapping.key_columns {
            digest_field(&mut digest, column.as_bytes());
        }
        digest_field(&mut digest, mapping.rule.as_bytes());
    }
    Ok(hex::encode(digest.finalize()))
}

pub fn transform_row(
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
        ColumnRule::Direct => Ok(direct_value(source)),
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
        ColumnRule::TextAsJsonString => {
            let text = source_text(mapping, column, source)?;
            Ok(CanonicalValue::Json(Value::String(text.to_string())))
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
        JsonShape::Object => value.is_object(),
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

pub fn target_insert_if_absent_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    target_batch_insert_if_absent_sql(mapping, 1)
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

pub fn target_batch_insert_if_absent_sql(
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
        "INSERT INTO {} ({quoted}) VALUES {parameters} ON CONFLICT ({}) DO NOTHING",
        postgres_identifier(mapping.target),
        postgres_identifier("id")
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
    "UPDATE \"v2_user\" SET \"invite_user_id\" = $2 WHERE \"id\" = $1 AND \"invite_user_id\" IS NOT DISTINCT FROM $3"
}

/// Builds the deterministic target-side derivation for native scoped node
/// credential epochs. This runs only after every node table has been copied
/// and value-verified. A retry must compare all four values for conflicts.
pub fn node_credential_rows_sql() -> Result<String, ConverterError> {
    audit_registry()?;
    let rows = NODE_CREDENTIAL_SOURCES
        .iter()
        .map(|(node_type, table)| {
            format!(
                "SELECT '{node_type}'::text AS node_type, id AS node_id, 0::bigint AS credential_epoch, updated_at FROM {}",
                postgres_identifier(table)
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    Ok(format!(
        "INSERT INTO \"v2_server_credential\" (node_type, node_id, credential_epoch, updated_at) {rows} ON CONFLICT (node_type, node_id) DO NOTHING"
    ))
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversionRunBinding {
    pub operation_id: String,
    pub target_installation_id: String,
    pub source_snapshot_sha256: String,
    pub source_schema_sha256: String,
    pub registry_sha256: String,
}

impl ConversionRunBinding {
    pub fn validate(&self) -> Result<(), ConverterError> {
        let operation_id = uuid::Uuid::parse_str(&self.operation_id)
            .map_err(|_| ConverterError::CheckpointBindingMismatch)?;
        let installation_id = uuid::Uuid::parse_str(&self.target_installation_id)
            .map_err(|_| ConverterError::CheckpointBindingMismatch)?;
        if operation_id.is_nil() || installation_id.is_nil() {
            return Err(ConverterError::CheckpointBindingMismatch);
        }
        if self.source_schema_sha256 != LEGACY_SEMANTIC_SCHEMA_SHA256 {
            return Err(ConverterError::SourceSchemaMismatch);
        }
        if !is_lower_sha256(&self.source_snapshot_sha256) {
            return Err(ConverterError::InvalidSnapshotFingerprint);
        }
        if self.registry_sha256 != registry_sha256()? {
            return Err(ConverterError::CheckpointBindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionPhase {
    CopyBaseTables,
    ApplyDeferredReferences,
    BuildDerivedRows,
    ResetSequences,
    FoldFrozenTraffic,
    VerifyAllValues,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStep {
    CopyBaseTable(&'static TableMapping),
    ApplyDeferredReferences(&'static TableMapping),
    BuildDerivedRows(&'static DerivedMapping),
    ResetSequence(&'static TableMapping),
    FoldFrozenTraffic,
    VerifyBaseTable(&'static TableMapping),
    VerifyDerivedTable(&'static DerivedMapping),
    Complete,
}

/// Returns the only supported converter order. Copy steps use keyset batches;
/// every target batch is inserted-or-compared before its durable checkpoint is
/// advanced. Deferred self references and derived tables therefore never race
/// an absent base id, and sequences move only after all explicit ids exist.
pub fn execution_steps() -> Result<Vec<ExecutionStep>, ConverterError> {
    audit_registry()?;
    let mut steps = Vec::new();
    steps.extend(TABLE_MAPPINGS.iter().map(ExecutionStep::CopyBaseTable));
    steps.extend(
        TABLE_MAPPINGS
            .iter()
            .filter(|mapping| !mapping.deferred_columns.is_empty())
            .map(ExecutionStep::ApplyDeferredReferences),
    );
    steps.extend(DERIVED_MAPPINGS.iter().map(ExecutionStep::BuildDerivedRows));
    steps.extend(TABLE_MAPPINGS.iter().map(ExecutionStep::ResetSequence));
    steps.push(ExecutionStep::FoldFrozenTraffic);
    steps.extend(TABLE_MAPPINGS.iter().map(ExecutionStep::VerifyBaseTable));
    steps.extend(
        DERIVED_MAPPINGS
            .iter()
            .map(ExecutionStep::VerifyDerivedTable),
    );
    steps.push(ExecutionStep::Complete);
    Ok(steps)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversionCheckpoint {
    pub binding: ConversionRunBinding,
    pub phase: ConversionPhase,
    pub table_order: u16,
    pub table: String,
    pub last_source_id: i64,
    pub source_rows_seen: u64,
    pub target_rows_verified: u64,
    pub rolling_sha256: String,
}

impl ConversionCheckpoint {
    pub fn validate_resume(
        &self,
        expected: &ConversionRunBinding,
        previous: Option<&Self>,
    ) -> Result<(), ConverterError> {
        expected.validate()?;
        if &self.binding != expected || !is_lower_sha256(&self.rolling_sha256) {
            return Err(ConverterError::CheckpointBindingMismatch);
        }
        if let Some(previous) = previous {
            let regressed = self.phase < previous.phase
                || (self.phase == previous.phase && self.table_order < previous.table_order)
                || (self.phase == previous.phase
                    && self.table_order == previous.table_order
                    && self.last_source_id < previous.last_source_id)
                || self.source_rows_seen < previous.source_rows_seen
                || self.target_rows_verified < previous.target_rows_verified;
            if regressed {
                return Err(ConverterError::CheckpointRegression);
            }
        }
        Ok(())
    }
}

pub fn validate_batch_ids(
    mapping: &TableMapping,
    after_id: i64,
    rows: &[CanonicalRow],
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
            Some(CanonicalValue::I64(id)) => *id,
            Some(CanonicalValue::U64(id)) => {
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
    digest.update(b"v2board-legacy-converter-canonical-rows-v1\0");
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

pub fn retry_accepts_existing(
    expected: &CanonicalRow,
    existing: &CanonicalRow,
) -> Result<(), ConverterError> {
    if expected == existing {
        Ok(())
    } else {
        Err(ConverterError::RetryConflict)
    }
}

/// Expands legacy set-valued redemption ids without inventing a historical
/// timestamp. The PostgreSQL baseline explicitly reserves `(0,
/// "legacy_unknown")` for this representation. Output is sorted and deduped
/// for deterministic retry, while malformed or missing users fail closed.
pub fn expand_giftcard_redemptions(
    giftcard_id: i32,
    used_user_ids: &SourceValue,
    known_user_ids: &BTreeSet<u64>,
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
        referenced_table: Some("v2_user"),
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
        if !known_user_ids.contains(user_id) {
            return Err(ConverterError::MissingIdReference {
                table: "v2_giftcard".to_string(),
                column: "used_user_ids".to_string(),
                referenced_table: "v2_user".to_string(),
                id: *user_id,
            });
        }
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

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete_and_stable() {
        audit_registry().expect("registry");
        assert_eq!(TABLE_MAPPINGS.len(), 27);
        assert_eq!(registry_sha256().expect("hash").len(), 64);
        let steps = execution_steps().expect("steps");
        assert_eq!(
            steps
                .iter()
                .filter(|step| matches!(step, ExecutionStep::CopyBaseTable(_)))
                .count(),
            27
        );
        assert_eq!(steps.last(), Some(&ExecutionStep::Complete));
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
    fn retry_requires_full_canonical_equality() {
        let mut expected = CanonicalRow::new();
        expected.insert("id".to_string(), CanonicalValue::I64(7));
        expected.insert(
            "token".to_string(),
            CanonicalValue::Text("permanent".to_string()),
        );
        assert!(retry_accepts_existing(&expected, &expected).is_ok());

        let mut conflict = expected.clone();
        conflict.insert(
            "token".to_string(),
            CanonicalValue::Text("changed".to_string()),
        );
        assert_eq!(
            retry_accepts_existing(&expected, &conflict),
            Err(ConverterError::RetryConflict)
        );
    }

    #[test]
    fn sql_is_keyset_and_conflict_is_not_an_update() {
        let user = mapping_for_source("v2_user").expect("user");
        let source_sql = source_batch_sql(user).expect("source sql");
        assert!(source_sql.contains("WHERE `id` > ? ORDER BY `id` ASC LIMIT ?"));
        assert!(!source_sql.contains("OFFSET"));
        assert_eq!(INITIAL_SOURCE_ID_CURSOR, i64::MIN);

        let target_sql = target_insert_if_absent_sql(user).expect("target sql");
        assert!(target_sql.ends_with("ON CONFLICT (\"id\") DO NOTHING"));
        assert!(!target_sql.contains("DO UPDATE"));
        assert!(!target_sql.contains("invite_user_id"));

        let target_batch = target_batch_insert_if_absent_sql(user, 3).expect("target batch");
        assert!(target_batch.contains("), ($"));
        assert!(
            maximum_rows_per_target_insert(user).expect("target batch limit")
                >= DEFAULT_BATCH_SIZE as usize
        );

        let credentials = node_credential_rows_sql().expect("credential SQL");
        assert_eq!(credentials.matches("SELECT '").count(), 8);
        assert!(credentials.ends_with("ON CONFLICT (node_type, node_id) DO NOTHING"));

        let reset = sequence_reset_sql(user).expect("sequence reset");
        assert!(reset.contains("GREATEST(COALESCE(MAX(id), 1), 1)"));
    }

    #[test]
    fn giftcard_redemptions_have_explicit_unknown_time_provenance() {
        let users = [7_u64, 9].into_iter().collect::<BTreeSet<_>>();
        let rows =
            expand_giftcard_redemptions(3, &SourceValue::Text(r#"[9,"7",9]"#.to_string()), &users)
                .expect("redemptions");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user_id, 7);
        assert_eq!(rows[0].created_at, 0);
        assert_eq!(rows[0].created_at_provenance, "legacy_unknown");

        assert!(
            expand_giftcard_redemptions(3, &SourceValue::Text("[10]".to_string()), &users,)
                .is_err()
        );
    }

    #[test]
    fn checkpoint_is_bound_and_monotonic() {
        let binding = ConversionRunBinding {
            operation_id: uuid::Uuid::from_u128(1).to_string(),
            target_installation_id: uuid::Uuid::from_u128(2).to_string(),
            source_snapshot_sha256: "a".repeat(64),
            source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
            registry_sha256: registry_sha256().expect("registry hash"),
        };
        let first = ConversionCheckpoint {
            binding: binding.clone(),
            phase: ConversionPhase::CopyBaseTables,
            table_order: 10,
            table: "v2_server_group".to_string(),
            last_source_id: 4,
            source_rows_seen: 4,
            target_rows_verified: 4,
            rolling_sha256: "b".repeat(64),
        };
        first.validate_resume(&binding, None).expect("first");
        let mut regressed = first.clone();
        regressed.last_source_id = 3;
        assert_eq!(
            regressed.validate_resume(&binding, Some(&first)),
            Err(ConverterError::CheckpointRegression)
        );
    }
}

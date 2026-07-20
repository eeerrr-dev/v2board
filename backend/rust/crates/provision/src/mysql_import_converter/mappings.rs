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
    /// Convert a legacy integer flag into a native PostgreSQL boolean. Only
    /// the exact source values 0 and 1 are accepted.
    Boolean01,
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
    /// Reference membership cannot be proven during row transformation; it is
    /// verified against the completed target after COPY. Both names are
    /// explicit because only the legacy source carries the `v2_*` prefix.
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
    /// Source columns consumed by a derived target rather than the base row.
    pub consumed_source_columns: &'static [ConsumedSourceColumn],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedMappingKind {
    PlanPrices,
    GiftcardRedemptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedMapping {
    pub order: u16,
    pub target: &'static str,
    pub kind: DerivedMappingKind,
    pub source_tables: &'static [&'static str],
    /// Complete target row written by the derived COPY stream. The live
    /// PostgreSQL schema gate verifies every column listed here.
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
        "sort",
        "content",
        "reset_traffic_method",
        "capacity_limit",
        "created_at",
        "updated_at",
    ],
    transformed_columns: &[
        TransformColumn {
            source: "show",
            target: "show",
            rule: ColumnRule::Boolean01,
            source_referenced_table: None,
            referenced_target_table: None,
        },
        TransformColumn {
            source: "renew",
            target: "renew",
            rule: ColumnRule::Boolean01,
            source_referenced_table: None,
            referenced_target_table: None,
        },
    ],
    added_columns: &[],
    consumed_source_columns: &[
        ConsumedSourceColumn {
            source: "month_price",
            reason: "normalized into plan_price(period=month)",
        },
        ConsumedSourceColumn {
            source: "quarter_price",
            reason: "normalized into plan_price(period=quarter)",
        },
        ConsumedSourceColumn {
            source: "half_year_price",
            reason: "normalized into plan_price(period=half_year)",
        },
        ConsumedSourceColumn {
            source: "year_price",
            reason: "normalized into plan_price(period=year)",
        },
        ConsumedSourceColumn {
            source: "two_year_price",
            reason: "normalized into plan_price(period=two_year)",
        },
        ConsumedSourceColumn {
            source: "three_year_price",
            reason: "normalized into plan_price(period=three_year)",
        },
        ConsumedSourceColumn {
            source: "onetime_price",
            reason: "normalized into plan_price(period=one_time)",
        },
        ConsumedSourceColumn {
            source: "reset_price",
            reason: "normalized into plan_price(period=reset)",
        },
    ],
};

pub(super) const PAYMENT: TableMapping = TableMapping {
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
    consumed_source_columns: &[],
};

const USER: TableMapping = TableMapping {
    order: 50,
    source: "v2_user",
    target: "users",
    identity_width: IdentityWidth::I64,
    direct_columns: &[
        "id",
        "invite_user_id",
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
    consumed_source_columns: &[],
};

pub(super) const ORDER: TableMapping = TableMapping {
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

pub const DERIVED_MAPPINGS: &[DerivedMapping] = &[
    DerivedMapping {
        order: 21,
        target: "plan_price",
        kind: DerivedMappingKind::PlanPrices,
        source_tables: &["v2_plan"],
        target_columns: &["plan_id", "period", "amount_minor"],
        key_columns: &["plan_id", "period"],
        rule: "expand each non-null legacy period price into one native plan_price row",
    },
    DerivedMapping {
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
    },
];

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
    // Older upgraded installations can retain this now-unused table. Its
    // presence is audited, but neither its schema nor its rows are scanned.
    "v2_tutorial",
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

/// Columns intentionally omitted from COPY because PostgreSQL derives them
/// from preserved legacy values. Final verification must still compare their
/// evaluated meaning.
pub const TARGET_GENERATED_COLUMNS: &[(&str, &[&str])] = &[("orders", &["referenced_plan_id"])];

pub fn mapping_for_source(table: &str) -> Option<&'static TableMapping> {
    TABLE_MAPPINGS
        .iter()
        .find(|mapping| mapping.source == table)
}

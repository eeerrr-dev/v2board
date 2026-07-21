//! Canonical registry for every modern internal HTTP operation.
//!
//! Each path is stored once, relative to its surface. Axum derives its mount
//! path from [`InternalOperation::runtime_path`], while documentation and
//! audit tooling derive the public template from
//! [`InternalOperation::documented_path`]. This keeps the live admin prefix
//! dynamic without duplicating its 89 route literals.

use std::{borrow::Cow, collections::BTreeMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use utoipa::{OpenApi as _, ToSchema};

/// One selectable server-owned mail template identifier.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct EmailTemplateName(pub String);

/// One administrator-visible knowledge category name. The user endpoint has
/// an object projection; the administrator endpoint deliberately returns bare
/// strings and therefore has a distinct contract.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct AdminKnowledgeCategoryName(pub String);

/// Named map root for the provider-owned payment form vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct PaymentProviderFormResponse(
    pub BTreeMap<String, crate::admin_business::PaymentProviderFormField>,
);

macro_rules! page_schema {
    ($name:ident, $item:path) => {
        #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub items: Vec<$item>,
            pub total: i64,
        }
    };
}

page_schema!(UserNoticePage, crate::content::NoticeView);
page_schema!(UserCommissionPage, crate::user_activity::CommissionView);
page_schema!(SystemLogPage, crate::admin_platform::SystemLogView);
page_schema!(AuditLogPage, crate::admin_platform::AuditLogView);
page_schema!(AdminCouponPage, crate::admin_codes::AdminCouponItem);
page_schema!(AdminGiftcardPage, crate::admin_codes::AdminGiftcardItem);
page_schema!(AdminOrderPage, crate::admin_business::AdminOrderListItem);
page_schema!(
    AdminPaymentReconciliationPage,
    crate::admin_business::AdminPaymentReconciliationListItem
);
page_schema!(AdminTicketPage, crate::admin_business::AdminTicketItem);
page_schema!(AdminUserPage, crate::admin_business::AdminUserListItem);
page_schema!(
    AdminUserTrafficPage,
    crate::admin_platform::AdminUserTrafficView
);

/// Component-only catalog for every root schema referenced by the canonical
/// operation inventory. Nested DTO dependencies are recursively registered by
/// `utoipa`; the explicit root list makes an omitted operation contract fail
/// closed in `schema_value` instead of degrading to an open JSON object.
#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(
    crate::auth::PublicConfig,
    crate::auth::InviteViewRequest,
    crate::auth::LoginRequest,
    crate::auth::RegisterRequest,
    crate::auth::AuthData,
    crate::auth::TokenLoginRequest,
    crate::auth::PasswordResetRequest,
    crate::auth::EmailCodeRequest,
    crate::auth::StepUpRequest,
    crate::auth::StepUpGrant,
    crate::auth::QuickLoginUrlRequest,
    crate::auth::QuickLoginUrl,
    crate::auth::SessionState,
    crate::user::UserProfile,
    crate::user::UserProfilePatch,
    crate::user::PasswordUpdateRequest,
    crate::user::UserStats,
    crate::user::UserSession,
    crate::user::UserConfig,
    crate::user::UserPlan,
    crate::user::Subscription,
    crate::user::ResetSubscriptionToken,
    crate::user::TelegramBot,
    crate::user::CommissionTransferRequest,
    crate::user::GiftCardRedemptionRequest,
    crate::user::GiftCardRedemption,
    crate::user_activity::UserServerView,
    crate::user_activity::TrafficLogView,
    crate::user_activity::InviteView,
    crate::user_activity::CommissionView,
    crate::user_activity::UserTicketView,
    crate::user_activity::UserTicketDetailView,
    crate::user_activity::UserTicketCreateRequest,
    crate::user_activity::UserTicketReplyRequest,
    crate::user_activity::WithdrawalTicketCreateRequest,
    crate::user_commerce::UserOrder,
    crate::user_commerce::PaymentMethod,
    crate::user_commerce::Coupon,
    crate::user_commerce::CreateOrderRequest,
    crate::user_commerce::OrderStatus,
    crate::user_commerce::CheckoutRequest,
    crate::user_commerce::StripeIntentRequest,
    crate::user_commerce::CheckoutOutcome,
    crate::user_commerce::StripePaymentIntent,
    crate::user_commerce::CouponCheckRequest,
    crate::admin_platform::MfaCodeRequest,
    crate::admin_platform::MfaStatusView,
    crate::admin_platform::TotpProvisioningView,
    crate::admin_platform::AdminConfigView,
    crate::admin_platform::AdminConfigPatchRequest,
    crate::admin_platform::TelegramWebhookRequest,
    crate::admin_platform::TestMailResult,
    crate::admin_platform::SystemStatusView,
    crate::admin_platform::QueueStatsView,
    crate::admin_platform::QueueWorkloadItem,
    crate::admin_platform::QueueMasterView,
    crate::admin_platform::SystemLogView,
    crate::admin_platform::AuditLogView,
    crate::admin_platform::AdminStatSummaryView,
    crate::admin_platform::ServerRankView,
    crate::admin_platform::UserRankView,
    crate::admin_platform::StatSeriesPointView,
    crate::admin_platform::AdminUserTrafficView,
    crate::commerce::AdminPlanItem,
    crate::commerce::CreatedId,
    crate::commerce::PlanCreate,
    crate::commerce::PlanPatch,
    crate::commerce::SortIdsRequest,
    crate::admin_business::AdminPaymentItem,
    crate::admin_business::PaymentProviderCode,
    crate::admin_business::AdminPaymentCreateRequest,
    crate::admin_business::AdminPaymentPatchRequest,
    crate::admin_business::PaymentProviderFormField,
    crate::admin_business::AdminOrderListItem,
    crate::admin_business::AdminOrderDetail,
    crate::admin_business::AdminOrderPatchRequest,
    crate::admin_business::AdminOrderCreateRequest,
    crate::admin_business::ReconciliationResolveRequest,
    crate::admin_business::AdminPaymentReconciliationListItem,
    crate::admin_business::AdminTicketItem,
    crate::admin_business::AdminTicketDetail,
    crate::admin_business::TicketReplyRequest,
    crate::admin_business::AdminUserListItem,
    crate::admin_business::AdminUserDetail,
    crate::admin_business::StaffUserDetail,
    crate::admin_business::AdminUserGenerateRequest,
    crate::admin_business::AdminUserPatchRequest,
    crate::admin_business::StaffUserPatchRequest,
    crate::admin_business::AdminUserFilterRequest,
    crate::admin_business::AdminUserMailRequest,
    crate::admin_business::AdminSetInviterRequest,
    crate::content::NoticeView,
    crate::content::NoticeCreateRequest,
    crate::content::NoticePatchRequest,
    crate::content::KnowledgeGroups,
    crate::content::KnowledgeSummaryView,
    crate::content::KnowledgeDetailView,
    crate::content::KnowledgeCategoryView,
    crate::content::KnowledgeCreateRequest,
    crate::content::KnowledgePatchRequest,
    crate::content::KnowledgeSortRequest,
    crate::admin_codes::AdminCouponItem,
    crate::admin_codes::CouponGenerateRequest,
    crate::admin_codes::CouponPatchRequest,
    crate::admin_codes::AdminGiftcardItem,
    crate::admin_codes::GiftcardGenerateRequest,
    crate::admin_codes::GiftcardPatchRequest,
    crate::admin_servers::NodeSortRequest,
    crate::admin_servers::ServerNodeView,
    crate::admin_servers::ServerGroupView,
    crate::admin_servers::ServerGroupWriteRequest,
    crate::admin_servers::ServerRouteView,
    crate::admin_servers::ServerRouteCreateRequest,
    crate::admin_servers::ServerRoutePatchRequest,
    crate::admin_servers::ServerWriteRequest,
    crate::common::CreatedInt32Id,
    crate::common::CreatedInt64Id,
    crate::common::CreatedTradeNo,
    crate::configuration::ConfigActivationPending,
    EmailTemplateName,
    AdminKnowledgeCategoryName,
    PaymentProviderFormResponse,
    UserNoticePage,
    UserCommissionPage,
    SystemLogPage,
    AuditLogPage,
    AdminCouponPage,
    AdminGiftcardPage,
    AdminOrderPage,
    AdminPaymentReconciliationPage,
    AdminTicketPage,
    AdminUserPage,
    AdminUserTrafficPage
)))]
struct OperationSchemaCatalog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperationSurface {
    Public,
    Auth,
    User,
    Admin,
    Staff,
}

impl OperationSurface {
    pub const fn documented_prefix(self) -> &'static str {
        match self {
            Self::Public => "/api/v1/public",
            Self::Auth => "/api/v1/auth",
            Self::User => "/api/v1/user",
            Self::Admin => "/api/v1/{secure_path}",
            Self::Staff => "/api/v1/staff",
        }
    }

    pub const fn uses_relative_runtime_router(self) -> bool {
        matches!(self, Self::Admin | Self::Staff)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuccessRepresentation {
    pub content_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuccessResponse {
    pub status: u16,
    pub representations: &'static [SuccessRepresentation],
    pub headers: &'static [&'static str],
}

impl SuccessResponse {
    pub const fn json(status: u16, representations: &'static [SuccessRepresentation]) -> Self {
        Self {
            status,
            representations,
            headers: &[],
        }
    }

    pub const fn empty(status: u16) -> Self {
        Self {
            status,
            representations: &[],
            headers: &[],
        }
    }

    pub const fn redirect(status: u16) -> Self {
        Self {
            status,
            representations: &[],
            headers: &["Location"],
        }
    }

    pub const fn csv(status: u16) -> Self {
        Self {
            status,
            representations: &[SuccessRepresentation {
                content_type: "text/csv; charset=utf-8",
            }],
            headers: &["Content-Disposition"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterLocation {
    Query,
    Header,
}

impl ParameterLocation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Header => "header",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSchema {
    String {
        values: &'static [&'static str],
        min_length: Option<usize>,
        /// Wire-byte bound. OpenAPI `maxLength` counts Unicode code points,
        /// so this is emitted as `x-v2board-max-bytes` instead.
        max_length: Option<usize>,
        default: Option<&'static str>,
    },
    Integer {
        format: &'static str,
        minimum: Option<i64>,
        maximum: Option<i64>,
        default: Option<i64>,
    },
    Boolean {
        default: Option<bool>,
    },
    IntegerArray {
        format: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationParameter {
    pub name: &'static str,
    pub location: ParameterLocation,
    pub required: bool,
    pub description: &'static str,
    pub schema: ParameterSchema,
    pub style: Option<&'static str>,
    pub explode: Option<bool>,
}

impl OperationParameter {
    const fn query(
        name: &'static str,
        required: bool,
        description: &'static str,
        schema: ParameterSchema,
    ) -> Self {
        Self {
            name,
            location: ParameterLocation::Query,
            required,
            description,
            schema,
            style: None,
            explode: None,
        }
    }

    const fn repeatable_query(
        name: &'static str,
        description: &'static str,
        schema: ParameterSchema,
    ) -> Self {
        Self {
            name,
            location: ParameterLocation::Query,
            required: false,
            description,
            schema,
            style: Some("form"),
            explode: Some(true),
        }
    }

    const fn header(
        name: &'static str,
        description: &'static str,
        schema: ParameterSchema,
    ) -> Self {
        Self {
            name,
            location: ParameterLocation::Header,
            required: false,
            description,
            schema,
            style: Some("simple"),
            explode: Some(false),
        }
    }
}

const JSON: &[SuccessRepresentation] = &[SuccessRepresentation {
    content_type: "application/json",
}];

const OK_JSON: &[SuccessResponse] = &[SuccessResponse::json(200, JSON)];
const CREATED_JSON: &[SuccessResponse] = &[SuccessResponse::json(201, JSON)];
const NO_CONTENT: &[SuccessResponse] = &[SuccessResponse::empty(204)];
const FOUND_LOCATION: &[SuccessResponse] = &[SuccessResponse::redirect(302)];
const CONFIG_PATCH: &[SuccessResponse] = &[
    SuccessResponse::json(202, JSON),
    SuccessResponse::empty(204),
];
const GENERATED_JSON_OR_CSV: &[SuccessResponse] =
    &[SuccessResponse::json(201, JSON), SuccessResponse::csv(200)];
const CSV_DOWNLOAD: &[SuccessResponse] = &[SuccessResponse::csv(200)];
const PLAN_LIST: &[SuccessResponse] = OK_JSON;
const PLAN_CREATE: &[SuccessResponse] = CREATED_JSON;

const PAGE: OperationParameter = OperationParameter::query(
    "page",
    false,
    "One-based page number; values below 1 are rejected with validation_failed.",
    ParameterSchema::Integer {
        format: "int64",
        minimum: Some(1),
        maximum: None,
        default: Some(1),
    },
);
const PER_PAGE_10: OperationParameter = OperationParameter::query(
    "per_page",
    false,
    "Page size; the endpoint default is 10 and the shared hard limit is 100.",
    ParameterSchema::Integer {
        format: "int64",
        minimum: Some(1),
        maximum: Some(100),
        default: Some(10),
    },
);
const PER_PAGE_5: OperationParameter = OperationParameter::query(
    "per_page",
    false,
    "Page size; notices default to 5 and the shared hard limit is 100.",
    ParameterSchema::Integer {
        format: "int64",
        minimum: Some(1),
        maximum: Some(100),
        default: Some(5),
    },
);
const FILTER: OperationParameter = OperationParameter::query(
    "filter",
    false,
    "One URL-encoded JSON array of AND-combined {field,op,value} clauses; operators are eq, neq, like, gt, gte, lt, lte, and in, resolved against the endpoint field whitelist.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: None,
        default: None,
    },
);
const SORT_DIR: OperationParameter = OperationParameter::query(
    "sort_dir",
    false,
    "Sort direction; invalid values are rejected rather than silently defaulted.",
    ParameterSchema::String {
        values: &["asc", "desc"],
        min_length: None,
        max_length: None,
        default: Some("desc"),
    },
);

const AUTH_QUICK_LOGIN_QUERY: &[OperationParameter] = &[
    OperationParameter::query(
        "token",
        true,
        "Non-empty temporary login token; the 256-byte limit is enforced by the handler.",
        ParameterSchema::String {
            values: &[],
            min_length: Some(1),
            max_length: Some(256),
            default: None,
        },
    ),
    OperationParameter::query(
        "redirect",
        false,
        "Post-login route carried into the browser redirect; blank values become dashboard and the handler enforces a 2048-byte limit.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: Some(2048),
            default: None,
        },
    ),
];
const USER_ORDERS_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "status",
    false,
    "Exact order status filter; no narrower business enum is imposed by this read endpoint.",
    ParameterSchema::Integer {
        format: "int32",
        minimum: Some(i16::MIN as i64),
        maximum: Some(i16::MAX as i64),
        default: None,
    },
)];
const KNOWLEDGE_LIST_QUERY: &[OperationParameter] = &[
    OperationParameter::query(
        "language",
        false,
        "Exact knowledge locale; only omission selects the zh-CN default.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: Some("zh-CN"),
        },
    ),
    OperationParameter::query(
        "keyword",
        false,
        "Case-insensitive title/body search; an empty or whitespace-only value is ignored.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: None,
        },
    ),
];
const KNOWLEDGE_CATEGORIES_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "language",
    false,
    "Exact knowledge locale; only omission selects the zh-CN default.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: None,
        default: Some("zh-CN"),
    },
)];
const NOTICES_QUERY: &[OperationParameter] = &[PAGE, PER_PAGE_5];
const DEFAULT_PAGE_QUERY: &[OperationParameter] = &[PAGE, PER_PAGE_10];
const CONFIG_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "group",
    false,
    "Known group names select one keyed group; an absent or unknown value returns the full configuration view.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: None,
        default: None,
    },
)];
const SYSTEM_LOGS_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    FILTER,
    OperationParameter::query(
        "sort_by",
        false,
        "Sortable fields for system logs.",
        ParameterSchema::String {
            values: &["created_at", "level"],
            min_length: None,
            max_length: None,
            default: Some("created_at"),
        },
    ),
    SORT_DIR,
];
const AUDIT_LOGS_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    FILTER,
    OperationParameter::query(
        "sort_by",
        false,
        "Audit logs may only be sorted by created_at.",
        ParameterSchema::String {
            values: &["created_at"],
            min_length: None,
            max_length: None,
            default: Some("created_at"),
        },
    ),
    SORT_DIR,
];
const CONTENT_LIST_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    OperationParameter::query(
        "sort_by",
        false,
        "Coupon and gift-card lists may only be sorted by created_at.",
        ParameterSchema::String {
            values: &["created_at"],
            min_length: None,
            max_length: None,
            default: Some("created_at"),
        },
    ),
    SORT_DIR,
];
const STATS_WINDOW_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "window",
    true,
    "Required rank window selector.",
    ParameterSchema::String {
        values: &["today", "previous"],
        min_length: None,
        max_length: None,
        default: None,
    },
)];
const STATS_USER_TRAFFIC_QUERY: &[OperationParameter] = &[
    OperationParameter::query(
        "user_id",
        true,
        "Required user identifier; the handler does not impose an additional positivity check.",
        ParameterSchema::Integer {
            format: "int64",
            minimum: None,
            maximum: None,
            default: None,
        },
    ),
    PAGE,
    PER_PAGE_10,
];
const STATS_RECORDS_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "type",
    false,
    "Daily or monthly statistics bucket.",
    ParameterSchema::String {
        values: &["d", "m"],
        min_length: None,
        max_length: None,
        default: Some("d"),
    },
)];
const PAYMENT_FORM_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "payment_id",
    false,
    "Existing active payment method used to seed the provider form with redacted values.",
    ParameterSchema::Integer {
        format: "int64",
        minimum: None,
        maximum: None,
        default: None,
    },
)];
const ORDERS_LIST_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    FILTER,
    OperationParameter::query(
        "sort_by",
        false,
        "Whitelisted order field; the full field vocabulary is described by docs/api-dialect.md section 7.",
        ParameterSchema::String {
            values: &[
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
            min_length: None,
            max_length: None,
            default: Some("created_at"),
        },
    ),
    SORT_DIR,
    OperationParameter::query(
        "commission_only",
        false,
        "When true, scope the list to commission-bearing orders.",
        ParameterSchema::Boolean {
            default: Some(false),
        },
    ),
];
const RECONCILIATIONS_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    OperationParameter::query(
        "resolved",
        false,
        "Resolution state: open aliases select unresolved rows, closed aliases select resolved rows, and all selects both.",
        ParameterSchema::String {
            values: &["0", "unresolved", "open", "1", "resolved", "closed", "all"],
            min_length: None,
            max_length: None,
            default: Some("unresolved"),
        },
    ),
    OperationParameter::query(
        "payment_id",
        false,
        "Exact payment-method identifier; values must fit the target signed 32-bit column.",
        ParameterSchema::Integer {
            format: "int64",
            minimum: Some(i32::MIN as i64),
            maximum: Some(i32::MAX as i64),
            default: None,
        },
    ),
    OperationParameter::query(
        "reason",
        false,
        "Exact reconciliation reason.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: None,
        },
    ),
    OperationParameter::query(
        "trade_no",
        false,
        "Exact trade number, hashed server-side before matching.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: None,
        },
    ),
    OperationParameter::query(
        "callback_no",
        false,
        "Exact provider callback number, hashed server-side before matching.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: None,
        },
    ),
];
const ADMIN_TICKETS_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    OperationParameter::query(
        "status",
        false,
        "Exact ticket status; the list endpoint does not impose a narrower enum.",
        ParameterSchema::Integer {
            format: "int64",
            minimum: None,
            maximum: None,
            default: None,
        },
    ),
    OperationParameter::repeatable_query(
        "reply_status",
        "Repeat this key once per reply status; JSON-stringified and comma-separated arrays are not accepted.",
        ParameterSchema::IntegerArray { format: "int64" },
    ),
    OperationParameter::query(
        "email",
        false,
        "A non-empty known email scopes to that user; an absent, empty, or unknown email leaves the list unscoped.",
        ParameterSchema::String {
            values: &[],
            min_length: None,
            max_length: None,
            default: None,
        },
    ),
];
const STAFF_TICKETS_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    OperationParameter::query(
        "status",
        false,
        "Exact ticket status; staff has no reply_status or email filter.",
        ParameterSchema::Integer {
            format: "int64",
            minimum: None,
            maximum: None,
            default: None,
        },
    ),
];
const USERS_LIST_QUERY: &[OperationParameter] = &[
    PAGE,
    PER_PAGE_10,
    FILTER,
    OperationParameter::query(
        "sort_by",
        false,
        "Whitelisted user field, including the computed total_used field.",
        ParameterSchema::String {
            values: &[
                "id",
                "email",
                "telegram_id",
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
                "is_staff",
                "last_login_at",
                "uuid",
                "group_id",
                "plan_id",
                "speed_limit",
                "token",
                "expired_at",
                "remarks",
                "invite_user_id",
                "created_at",
                "updated_at",
                "total_used",
            ],
            min_length: None,
            max_length: None,
            default: Some("created_at"),
        },
    ),
    SORT_DIR,
];
const SERVER_GROUPS_QUERY: &[OperationParameter] = &[OperationParameter::query(
    "group_id",
    false,
    "Exact server-group identifier; a miss returns an empty array.",
    ParameterSchema::Integer {
        format: "int64",
        minimum: None,
        maximum: None,
        default: None,
    },
)];

const ACCEPT_LANGUAGE_HEADER: OperationParameter = OperationParameter::header(
    "Accept-Language",
    "Standard weighted language-range list. Exact and primary-subtag matches resolve against enabled locales; malformed, wildcard, q=0, and unknown candidates fall back to zh-CN.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: None,
        default: None,
    },
);
const USER_AGENT_HEADER: OperationParameter = OperationParameter::header(
    "User-Agent",
    "Optional session metadata. Invalid UTF-8 is treated as absent; values longer than 512 bytes are truncated at a UTF-8 boundary.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: None,
        default: None,
    },
);
const IDEMPOTENCY_KEY_HEADER: OperationParameter = OperationParameter::header(
    "Idempotency-Key",
    "Optional ASCII bulk-mail idempotency key. It is trimmed, may be at most 512 bytes, and omission or an empty value generates a server UUID.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: Some(512),
        default: None,
    },
);
const STEP_UP_HEADER: OperationParameter = OperationParameter::header(
    "X-V2Board-Step-Up",
    "Conditionally required when privileged step-up is enabled and the current password-authentication window has expired. A supplied non-empty token must be at most 256 bytes, live, and bound to the current user and session.",
    ParameterSchema::String {
        values: &[],
        min_length: None,
        max_length: Some(256),
        default: None,
    },
);
const COMMON_HEADERS: &[OperationParameter] = &[ACCEPT_LANGUAGE_HEADER];
const USER_AGENT_HEADERS: &[OperationParameter] = &[ACCEPT_LANGUAGE_HEADER, USER_AGENT_HEADER];
const STEP_UP_HEADERS: &[OperationParameter] = &[ACCEPT_LANGUAGE_HEADER, STEP_UP_HEADER];
const MAIL_HEADERS: &[OperationParameter] = &[
    ACCEPT_LANGUAGE_HEADER,
    IDEMPOTENCY_KEY_HEADER,
    STEP_UP_HEADER,
];
const NO_PARAMETERS: &[OperationParameter] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalOperation {
    pub id: &'static str,
    /// Compatibility ids represented by this HTTP operation. Six PATCH
    /// operations intentionally cover both update and toggle semantics.
    pub logical_ids: &'static [&'static str],
    pub surface: OperationSurface,
    pub method: HttpMethod,
    /// Canonical path relative to `surface`; always begins with `/`.
    pub path: &'static str,
    pub documented: bool,
    pub legacy_mapped: bool,
    pub spec_section: &'static str,
    pub successes: &'static [SuccessResponse],
}

impl InternalOperation {
    /// Public/auth/user routes are mounted at their fixed full path. Admin and
    /// staff operations are mounted into prefix-relative subrouters.
    pub fn runtime_path(self) -> Cow<'static, str> {
        if self.surface.uses_relative_runtime_router() {
            Cow::Borrowed(self.path)
        } else {
            Cow::Owned(format!("{}{}", self.surface.documented_prefix(), self.path))
        }
    }

    /// Public path template used by OpenAPI and route audits. No boot-time
    /// admin path value enters the registry.
    pub fn documented_path(self) -> String {
        format!("{}{}", self.surface.documented_prefix(), self.path)
    }

    /// Stable generated-client key. Five plan operation ids predate the
    /// deterministic naming rule and retain their exact client-visible names;
    /// every other entry derives camelCase from the canonical dotted id.
    pub fn openapi_operation_id(self) -> String {
        match self.id {
            "admin.plans.list" => "adminPlansList".to_owned(),
            "admin.plans.create" => "adminPlanCreate".to_owned(),
            "admin.plans.update" => "adminPlanPatch".to_owned(),
            "admin.plans.delete" => "adminPlanDelete".to_owned(),
            "admin.plans.sort" => "adminPlansSort".to_owned(),
            id => camel_case_operation_id(id),
        }
    }

    /// Operation-specific query contract. The remaining registry operations
    /// deliberately return an empty slice, so a new Axum `Query` extractor
    /// must update this match and the exact-set tests in the same change.
    pub fn query_parameters(self) -> &'static [OperationParameter] {
        match self.id {
            "auth.quick-login" => AUTH_QUICK_LOGIN_QUERY,
            "user.orders.list" => USER_ORDERS_QUERY,
            "user.knowledge.list" => KNOWLEDGE_LIST_QUERY,
            "user.knowledge-categories.list" => KNOWLEDGE_CATEGORIES_QUERY,
            "user.notices.list" => NOTICES_QUERY,
            "user.commissions.list" => DEFAULT_PAGE_QUERY,
            "admin.config.get" => CONFIG_QUERY,
            "admin.system.logs" => SYSTEM_LOGS_QUERY,
            "admin.system.audit-logs.list" => AUDIT_LOGS_QUERY,
            "admin.coupons.list" | "admin.gift-cards.list" => CONTENT_LIST_QUERY,
            "admin.stats.server-rank" | "admin.stats.user-rank" => STATS_WINDOW_QUERY,
            "admin.stats.user-traffic" => STATS_USER_TRAFFIC_QUERY,
            "admin.stats.records" => STATS_RECORDS_QUERY,
            "admin.payment-providers.form" => PAYMENT_FORM_QUERY,
            "admin.orders.list" => ORDERS_LIST_QUERY,
            "admin.payment-reconciliations.list" => RECONCILIATIONS_QUERY,
            "admin.tickets.list" => ADMIN_TICKETS_QUERY,
            "admin.users.list" => USERS_LIST_QUERY,
            "admin.server-groups.list" => SERVER_GROUPS_QUERY,
            "staff.tickets.list" => STAFF_TICKETS_QUERY,
            _ => NO_PARAMETERS,
        }
    }

    /// Headers consumed by an individual handler or its surface guard. Step-up
    /// is derived from the structural routing policy rather than maintained as
    /// a second 67-id allow-list: every admin/staff mutation plus the two
    /// credential-bearing reads receives the optional conditional header.
    pub fn header_parameters(self) -> &'static [OperationParameter] {
        if matches!(self.id, "auth.register" | "auth.login" | "auth.token-login") {
            return USER_AGENT_HEADERS;
        }
        if matches!(self.id, "admin.users.mail" | "staff.users.mail") {
            return MAIL_HEADERS;
        }
        if (matches!(
            self.surface,
            OperationSurface::Admin | OperationSurface::Staff
        ) && !matches!(self.method, HttpMethod::Get))
            || matches!(
                self.id,
                "admin.nodes.list" | "admin.payment-reconciliations.list"
            )
        {
            return STEP_UP_HEADERS;
        }
        COMMON_HEADERS
    }

    /// Exact named JSON request root, or no request body. Every mutating route
    /// is classified explicitly; adding a POST/PATCH/PUT without selecting a
    /// DTO is a hard failure instead of an open-object fallback.
    pub fn request_body_schema(self) -> Option<&'static str> {
        match self.id {
            "public.invite-views.create" => Some("InviteViewRequest"),
            "auth.register" => Some("RegisterRequest"),
            "auth.login" => Some("LoginRequest"),
            "auth.token-login" => Some("TokenLoginRequest"),
            "auth.password-reset" => Some("PasswordResetRequest"),
            "auth.step-up" => Some("StepUpRequest"),
            "auth.quick-login-url" => Some("QuickLoginUrlRequest"),
            "auth.email-codes" => Some("EmailCodeRequest"),
            "user.profile.update" => Some("UserProfilePatch"),
            "user.password.update" => Some("PasswordUpdateRequest"),
            "user.commission-transfers.create" => Some("CommissionTransferRequest"),
            "user.gift-card-redemptions.create" => Some("GiftCardRedemptionRequest"),
            "user.orders.create" => Some("CreateOrderRequest"),
            "user.orders.checkout" => Some("CheckoutRequest"),
            "user.orders.stripe-intent" => Some("StripeIntentRequest"),
            "user.coupons.check" => Some("CouponCheckRequest"),
            "user.tickets.create" => Some("UserTicketCreateRequest"),
            "user.tickets.replies.create" => Some("UserTicketReplyRequest"),
            "user.withdrawal-tickets.create" => Some("WithdrawalTicketCreateRequest"),
            "admin.account.mfa.totp.confirm"
            | "admin.account.mfa.totp.disable"
            | "staff.account.mfa.totp.confirm"
            | "staff.account.mfa.totp.disable" => Some("MfaCodeRequest"),
            "admin.config.update" => Some("AdminConfigPatchRequest"),
            "admin.telegram-webhook.set" => Some("TelegramWebhookRequest"),
            "admin.plans.create" => Some("PlanCreate"),
            "admin.plans.update" => Some("PlanPatch"),
            "admin.plans.sort" | "admin.payments.sort" => Some("SortIdsRequest"),
            "admin.payments.create" => Some("AdminPaymentCreateRequest"),
            "admin.payments.update" => Some("AdminPaymentPatchRequest"),
            "admin.notices.create" | "staff.notices.create" => Some("NoticeCreateRequest"),
            "admin.notices.update" | "staff.notices.update" => Some("NoticePatchRequest"),
            "admin.knowledge.create" => Some("KnowledgeCreateRequest"),
            "admin.knowledge.update" => Some("KnowledgePatchRequest"),
            "admin.knowledge.sort" => Some("KnowledgeSortRequest"),
            "admin.coupons.create" => Some("CouponGenerateRequest"),
            "admin.coupons.update" => Some("CouponPatchRequest"),
            "admin.gift-cards.create" => Some("GiftcardGenerateRequest"),
            "admin.gift-cards.update" => Some("GiftcardPatchRequest"),
            "admin.payment-reconciliations.resolve" => Some("ReconciliationResolveRequest"),
            "admin.orders.update" => Some("AdminOrderPatchRequest"),
            "admin.orders.create" => Some("AdminOrderCreateRequest"),
            "admin.tickets.replies.create" | "staff.tickets.replies.create" => {
                Some("TicketReplyRequest")
            }
            "admin.users.update" => Some("AdminUserPatchRequest"),
            "staff.users.update" => Some("StaffUserPatchRequest"),
            "admin.users.set-inviter" => Some("AdminSetInviterRequest"),
            "admin.users.create" => Some("AdminUserGenerateRequest"),
            "admin.users.export"
            | "admin.users.ban"
            | "admin.users.bulk-delete"
            | "staff.users.ban" => Some("AdminUserFilterRequest"),
            "admin.users.mail" | "staff.users.mail" => Some("AdminUserMailRequest"),
            "admin.nodes.sort" => Some("NodeSortRequest"),
            "admin.server-groups.create" | "admin.server-groups.update" => {
                Some("ServerGroupWriteRequest")
            }
            "admin.server-routes.create" => Some("ServerRouteCreateRequest"),
            "admin.server-routes.update" => Some("ServerRoutePatchRequest"),
            "admin.servers.create" | "admin.servers.update" => Some("ServerWriteRequest"),
            "user.subscription.new-period"
            | "user.subscription.reset-token"
            | "user.orders.cancel"
            | "user.invite-codes.create"
            | "user.tickets.close"
            | "admin.account.mfa.totp.setup"
            | "admin.test-mail.send"
            | "admin.orders.mark-paid"
            | "admin.orders.cancel"
            | "admin.tickets.close"
            | "admin.users.reset-secret"
            | "admin.servers.copy"
            | "staff.account.mfa.totp.setup"
            | "staff.tickets.close" => None,
            _ if matches!(self.method, HttpMethod::Get | HttpMethod::Delete) => None,
            _ => panic!(
                "unclassified JSON request body for {} {}",
                self.method.as_str(),
                self.id
            ),
        }
    }

    /// Named schema for one successful JSON representation. This is called
    /// only for an `application/json` success arm; non-JSON/empty successes
    /// never acquire a phantom body.
    pub fn json_success_schema(self, status: u16) -> &'static str {
        match (self.id, status) {
            ("public.config", 200) => "PublicConfig",
            ("auth.register", 201) | ("auth.login" | "auth.token-login", 200) => "AuthData",
            ("auth.step-up", 200) => "StepUpGrant",
            ("auth.quick-login-url", 200) => "QuickLoginUrl",
            ("auth.session.get", 200) => "SessionState",
            ("user.profile.get", 200) => "UserProfile",
            ("user.stats.get", 200) => "UserStats",
            ("user.sessions.list", 200) => "UserSession[]",
            ("user.gift-card-redemptions.create", 200) => "GiftCardRedemption",
            ("user.telegram-bot.get", 200) => "TelegramBot",
            ("user.config.get", 200) => "UserConfig",
            ("user.subscription.get", 200) => "Subscription",
            ("user.subscription.reset-token", 200) => "ResetSubscriptionToken",
            ("user.servers.list", 200) => "UserServerView[]",
            ("user.traffic-logs.list", 200) => "TrafficLogView[]",
            ("user.plans.get", 200) => "UserPlan",
            ("user.plans.list", 200) => "UserPlan[]",
            ("user.orders.create", 201) => "CreatedTradeNo",
            ("user.orders.list", 200) => "UserOrder[]",
            ("user.orders.get", 200) => "UserOrder",
            ("user.orders.status", 200) => "OrderStatus",
            ("user.orders.checkout", 200) => "CheckoutOutcome",
            ("user.orders.stripe-intent", 200) => "StripePaymentIntent",
            ("user.payment-methods.list", 200) => "PaymentMethod[]",
            ("user.coupons.check", 200) => "Coupon",
            ("user.invite.get", 200) => "InviteView",
            ("user.commissions.list", 200) => "UserCommissionPage",
            ("user.tickets.get", 200) => "UserTicketDetailView",
            ("user.tickets.list", 200) => "UserTicketView[]",
            ("user.tickets.create" | "user.withdrawal-tickets.create", 201) => "CreatedInt64Id",
            ("user.knowledge.get", 200) => "KnowledgeDetailView",
            ("user.knowledge.list", 200) => "KnowledgeGroups",
            ("user.knowledge-categories.list", 200) => "KnowledgeCategoryView[]",
            ("user.notices.list", 200) => "UserNoticePage",
            ("admin.account.mfa.get" | "staff.account.mfa.get", 200) => "MfaStatusView",
            ("admin.account.mfa.totp.setup" | "staff.account.mfa.totp.setup", 200) => {
                "TotpProvisioningView"
            }
            ("admin.config.get", 200) => "AdminConfigView",
            ("admin.config.update", 202) => "ConfigActivationPending",
            ("admin.email-templates.list", 200) => "EmailTemplateName[]",
            ("admin.payment-providers.list", 200) => "PaymentProviderCode[]",
            ("admin.test-mail.send", 200) => "TestMailResult",
            ("admin.system.status", 200) => "SystemStatusView",
            ("admin.system.queue-stats", 200) => "QueueStatsView",
            ("admin.system.queue-workload", 200) => "QueueWorkloadItem[]",
            ("admin.system.queue-masters", 200) => "QueueMasterView[]",
            ("admin.system.logs", 200) => "SystemLogPage",
            ("admin.system.audit-logs.list", 200) => "AuditLogPage",
            ("admin.plans.list" | "staff.plans.list", 200) => "AdminPlanItem[]",
            ("admin.plans.create", 201) => "CreatedId",
            ("admin.payments.list", 200) => "AdminPaymentItem[]",
            ("admin.payment-providers.form", 200) => "PaymentProviderFormResponse",
            ("admin.payments.create", 201) => "CreatedInt32Id",
            ("admin.notices.list" | "staff.notices.list", 200) => "NoticeView[]",
            ("admin.notices.create" | "staff.notices.create", 201) => "CreatedInt32Id",
            ("admin.knowledge.list", 200) => "KnowledgeSummaryView[]",
            ("admin.knowledge.get", 200) => "KnowledgeDetailView",
            ("admin.knowledge-categories.list", 200) => "AdminKnowledgeCategoryName[]",
            ("admin.knowledge.create", 201) => "CreatedInt32Id",
            ("admin.coupons.list", 200) => "AdminCouponPage",
            ("admin.coupons.create", 201) => "CreatedInt32Id",
            ("admin.gift-cards.list", 200) => "AdminGiftcardPage",
            ("admin.gift-cards.create", 201) => "CreatedInt32Id",
            ("admin.orders.list", 200) => "AdminOrderPage",
            ("admin.orders.get", 200) => "AdminOrderDetail",
            ("admin.orders.create", 201) => "CreatedTradeNo",
            ("admin.payment-reconciliations.list", 200) => "AdminPaymentReconciliationPage",
            ("admin.tickets.list" | "staff.tickets.list", 200) => "AdminTicketPage",
            ("admin.tickets.get" | "staff.tickets.get", 200) => "AdminTicketDetail",
            ("admin.users.list", 200) => "AdminUserPage",
            ("admin.users.get", 200) => "AdminUserDetail",
            ("staff.users.get", 200) => "StaffUserDetail",
            ("admin.users.create", 201) => "CreatedInt64Id",
            ("admin.nodes.list", 200) => "ServerNodeView[]",
            ("admin.server-groups.list", 200) => "ServerGroupView[]",
            ("admin.server-groups.create", 201) => "CreatedInt32Id",
            ("admin.server-routes.list", 200) => "ServerRouteView[]",
            ("admin.server-routes.create", 201) => "CreatedInt32Id",
            ("admin.servers.create" | "admin.servers.copy", 201) => "CreatedInt32Id",
            ("admin.stats.summary", 200) => "AdminStatSummaryView",
            ("admin.stats.server-rank", 200) => "ServerRankView[]",
            ("admin.stats.user-rank", 200) => "UserRankView[]",
            ("admin.stats.orders" | "admin.stats.records", 200) => "StatSeriesPointView[]",
            ("admin.stats.user-traffic", 200) => "AdminUserTrafficPage",
            _ => panic!(
                "unclassified JSON success schema for {} status {}",
                self.id, status
            ),
        }
    }
}

fn camel_case_operation_id(id: &str) -> String {
    let mut output = String::with_capacity(id.len());
    let mut uppercase_next = false;
    for character in id.chars() {
        if character.is_ascii_alphanumeric() {
            if uppercase_next {
                output.push(character.to_ascii_uppercase());
                uppercase_next = false;
            } else {
                output.push(character);
            }
        } else {
            uppercase_next = true;
        }
    }
    output
}

macro_rules! operation {
    ($id:literal, [$($logical_id:literal),+ $(,)?], $surface:ident, $method:ident, $path:literal, $section:literal, $successes:ident) => {
        InternalOperation {
            id: $id,
            logical_ids: &[$($logical_id),+],
            surface: OperationSurface::$surface,
            method: HttpMethod::$method,
            path: $path,
            documented: true,
            legacy_mapped: true,
            spec_section: $section,
            successes: $successes,
        }
    };
}

macro_rules! native_operation {
    ($id:literal, $surface:ident, $method:ident, $path:literal, $section:literal, $successes:ident) => {
        InternalOperation {
            id: $id,
            logical_ids: &[$id],
            surface: OperationSurface::$surface,
            method: HttpMethod::$method,
            path: $path,
            documented: true,
            legacy_mapped: false,
            spec_section: $section,
            successes: $successes,
        }
    };
}

/// Exactly 158 unique modern internal `(surface, method, path)` operations.
/// The compatibility map's 155 logical ids collapse to 149 unique routes;
/// nine documented native routes bring the runtime registry to 158 routes and
/// 164 logical ids.
pub const INTERNAL_OPERATIONS: &[InternalOperation] = &[
    operation!(
        "public.config",
        ["public.config"],
        Public,
        Get,
        "/config",
        "§5.1",
        OK_JSON
    ),
    operation!(
        "public.invite-views.create",
        ["public.invite-views.create"],
        Public,
        Post,
        "/invite-views",
        "§5.1",
        NO_CONTENT
    ),
    operation!(
        "auth.register",
        ["auth.register"],
        Auth,
        Post,
        "/register",
        "§5.2",
        CREATED_JSON
    ),
    operation!(
        "auth.login",
        ["auth.login"],
        Auth,
        Post,
        "/login",
        "§5.2",
        OK_JSON
    ),
    operation!(
        "auth.quick-login",
        ["auth.quick-login"],
        Auth,
        Get,
        "/quick-login",
        "§5.2",
        FOUND_LOCATION
    ),
    operation!(
        "auth.token-login",
        ["auth.token-login"],
        Auth,
        Post,
        "/token-login",
        "§5.2",
        OK_JSON
    ),
    operation!(
        "auth.password-reset",
        ["auth.password-reset"],
        Auth,
        Post,
        "/password-reset",
        "§5.2",
        NO_CONTENT
    ),
    operation!(
        "auth.step-up",
        ["auth.step-up"],
        Auth,
        Post,
        "/step-up",
        "§5.2",
        OK_JSON
    ),
    operation!(
        "auth.quick-login-url",
        ["auth.quick-login-url"],
        Auth,
        Post,
        "/quick-login-url",
        "§5.2",
        OK_JSON
    ),
    operation!(
        "auth.email-codes",
        ["auth.email-codes"],
        Auth,
        Post,
        "/email-codes",
        "§5.2",
        NO_CONTENT
    ),
    operation!(
        "auth.session.get",
        ["auth.session.get"],
        Auth,
        Get,
        "/session",
        "§5.2",
        OK_JSON
    ),
    operation!(
        "auth.session.delete",
        ["auth.session.delete"],
        Auth,
        Delete,
        "/session",
        "§5.2",
        NO_CONTENT
    ),
    operation!(
        "user.profile.get",
        ["user.profile.get"],
        User,
        Get,
        "/profile",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.profile.update",
        ["user.profile.update"],
        User,
        Patch,
        "/profile",
        "§5.3",
        NO_CONTENT
    ),
    operation!(
        "user.password.update",
        ["user.password.update"],
        User,
        Put,
        "/password",
        "§5.3",
        NO_CONTENT
    ),
    operation!(
        "user.stats.get",
        ["user.stats.get"],
        User,
        Get,
        "/stats",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.sessions.list",
        ["user.sessions.list"],
        User,
        Get,
        "/sessions",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.sessions.delete",
        ["user.sessions.delete"],
        User,
        Delete,
        "/sessions/{session_id}",
        "§5.3",
        NO_CONTENT
    ),
    operation!(
        "user.commission-transfers.create",
        ["user.commission-transfers.create"],
        User,
        Post,
        "/commission-transfers",
        "§5.3",
        NO_CONTENT
    ),
    operation!(
        "user.gift-card-redemptions.create",
        ["user.gift-card-redemptions.create"],
        User,
        Post,
        "/gift-card-redemptions",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.telegram-binding.delete",
        ["user.telegram-binding.delete"],
        User,
        Delete,
        "/telegram-binding",
        "§5.3",
        NO_CONTENT
    ),
    operation!(
        "user.telegram-bot.get",
        ["user.telegram-bot.get"],
        User,
        Get,
        "/telegram-bot",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.config.get",
        ["user.config.get"],
        User,
        Get,
        "/config",
        "§5.3",
        OK_JSON
    ),
    operation!(
        "user.subscription.get",
        ["user.subscription.get"],
        User,
        Get,
        "/subscription",
        "§5.4",
        OK_JSON
    ),
    operation!(
        "user.subscription.new-period",
        ["user.subscription.new-period"],
        User,
        Post,
        "/subscription/new-period",
        "§5.4",
        NO_CONTENT
    ),
    operation!(
        "user.subscription.reset-token",
        ["user.subscription.reset-token"],
        User,
        Post,
        "/subscription/reset-token",
        "§5.4",
        OK_JSON
    ),
    operation!(
        "user.servers.list",
        ["user.servers.list"],
        User,
        Get,
        "/servers",
        "§5.4",
        OK_JSON
    ),
    operation!(
        "user.traffic-logs.list",
        ["user.traffic-logs.list"],
        User,
        Get,
        "/traffic-logs",
        "§5.4",
        OK_JSON
    ),
    operation!(
        "user.plans.get",
        ["user.plans.get"],
        User,
        Get,
        "/plans/{id}",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.plans.list",
        ["user.plans.list"],
        User,
        Get,
        "/plans",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.orders.create",
        ["user.orders.create"],
        User,
        Post,
        "/orders",
        "§5.5",
        CREATED_JSON
    ),
    operation!(
        "user.orders.list",
        ["user.orders.list"],
        User,
        Get,
        "/orders",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.orders.get",
        ["user.orders.get"],
        User,
        Get,
        "/orders/{trade_no}",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.orders.status",
        ["user.orders.status"],
        User,
        Get,
        "/orders/{trade_no}/status",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.orders.cancel",
        ["user.orders.cancel"],
        User,
        Post,
        "/orders/{trade_no}/cancel",
        "§5.5",
        NO_CONTENT
    ),
    operation!(
        "user.orders.checkout",
        ["user.orders.checkout"],
        User,
        Post,
        "/orders/{trade_no}/checkout",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.orders.stripe-intent",
        ["user.orders.stripe-intent"],
        User,
        Post,
        "/orders/{trade_no}/stripe-intent",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.payment-methods.list",
        ["user.payment-methods.list"],
        User,
        Get,
        "/payment-methods",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.coupons.check",
        ["user.coupons.check"],
        User,
        Post,
        "/coupons/check",
        "§5.5",
        OK_JSON
    ),
    operation!(
        "user.invite-codes.create",
        ["user.invite-codes.create"],
        User,
        Post,
        "/invite-codes",
        "§5.6",
        NO_CONTENT
    ),
    operation!(
        "user.invite.get",
        ["user.invite.get"],
        User,
        Get,
        "/invite",
        "§5.6",
        OK_JSON
    ),
    operation!(
        "user.commissions.list",
        ["user.commissions.list"],
        User,
        Get,
        "/commissions",
        "§5.6",
        OK_JSON
    ),
    operation!(
        "user.tickets.get",
        ["user.tickets.get"],
        User,
        Get,
        "/tickets/{id}",
        "§5.7",
        OK_JSON
    ),
    operation!(
        "user.tickets.list",
        ["user.tickets.list"],
        User,
        Get,
        "/tickets",
        "§5.7",
        OK_JSON
    ),
    operation!(
        "user.tickets.create",
        ["user.tickets.create"],
        User,
        Post,
        "/tickets",
        "§5.7",
        CREATED_JSON
    ),
    operation!(
        "user.tickets.replies.create",
        ["user.tickets.replies.create"],
        User,
        Post,
        "/tickets/{id}/replies",
        "§5.7",
        NO_CONTENT
    ),
    operation!(
        "user.tickets.close",
        ["user.tickets.close"],
        User,
        Post,
        "/tickets/{id}/close",
        "§5.7",
        NO_CONTENT
    ),
    operation!(
        "user.withdrawal-tickets.create",
        ["user.withdrawal-tickets.create"],
        User,
        Post,
        "/withdrawal-tickets",
        "§5.7",
        CREATED_JSON
    ),
    operation!(
        "user.knowledge.get",
        ["user.knowledge.get"],
        User,
        Get,
        "/knowledge/{id}",
        "§5.8",
        OK_JSON
    ),
    operation!(
        "user.knowledge.list",
        ["user.knowledge.list"],
        User,
        Get,
        "/knowledge",
        "§5.8",
        OK_JSON
    ),
    operation!(
        "user.knowledge-categories.list",
        ["user.knowledge-categories.list"],
        User,
        Get,
        "/knowledge-categories",
        "§5.8",
        OK_JSON
    ),
    operation!(
        "user.notices.list",
        ["user.notices.list"],
        User,
        Get,
        "/notices",
        "§5.8",
        OK_JSON
    ),
    native_operation!(
        "admin.account.mfa.get",
        Admin,
        Get,
        "/account/mfa",
        "§6.10",
        OK_JSON
    ),
    native_operation!(
        "admin.account.mfa.totp.setup",
        Admin,
        Post,
        "/account/mfa/totp",
        "§6.10",
        OK_JSON
    ),
    native_operation!(
        "admin.account.mfa.totp.confirm",
        Admin,
        Post,
        "/account/mfa/totp/confirm",
        "§6.10",
        NO_CONTENT
    ),
    native_operation!(
        "admin.account.mfa.totp.disable",
        Admin,
        Post,
        "/account/mfa/totp/disable",
        "§6.10",
        NO_CONTENT
    ),
    operation!(
        "admin.config.get",
        ["admin.config.get"],
        Admin,
        Get,
        "/config",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.config.update",
        ["admin.config.update"],
        Admin,
        Patch,
        "/config",
        "§6.1",
        CONFIG_PATCH
    ),
    operation!(
        "admin.email-templates.list",
        ["admin.email-templates.list"],
        Admin,
        Get,
        "/email-templates",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.telegram-webhook.set",
        ["admin.telegram-webhook.set"],
        Admin,
        Post,
        "/telegram-webhook",
        "§6.1",
        NO_CONTENT
    ),
    operation!(
        "admin.test-mail.send",
        ["admin.test-mail.send"],
        Admin,
        Post,
        "/test-mail",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.system.status",
        ["admin.system.status"],
        Admin,
        Get,
        "/system/status",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.system.queue-stats",
        ["admin.system.queue-stats"],
        Admin,
        Get,
        "/system/queue-stats",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.system.queue-workload",
        ["admin.system.queue-workload"],
        Admin,
        Get,
        "/system/queue-workload",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.system.queue-masters",
        ["admin.system.queue-masters"],
        Admin,
        Get,
        "/system/queue-masters",
        "§6.1",
        OK_JSON
    ),
    operation!(
        "admin.system.logs",
        ["admin.system.logs"],
        Admin,
        Get,
        "/system/logs",
        "§6.1",
        OK_JSON
    ),
    native_operation!(
        "admin.system.audit-logs.list",
        Admin,
        Get,
        "/system/audit-logs",
        "§6.11",
        OK_JSON
    ),
    operation!(
        "admin.plans.list",
        ["admin.plans.list"],
        Admin,
        Get,
        "/plans",
        "§6.2",
        PLAN_LIST
    ),
    operation!(
        "admin.plans.create",
        ["admin.plans.create"],
        Admin,
        Post,
        "/plans",
        "§6.2",
        PLAN_CREATE
    ),
    operation!(
        "admin.plans.update",
        ["admin.plans.update", "admin.plans.toggle"],
        Admin,
        Patch,
        "/plans/{id}",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.plans.delete",
        ["admin.plans.delete"],
        Admin,
        Delete,
        "/plans/{id}",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.plans.sort",
        ["admin.plans.sort"],
        Admin,
        Post,
        "/plans/sort",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.payments.list",
        ["admin.payments.list"],
        Admin,
        Get,
        "/payments",
        "§6.2",
        OK_JSON
    ),
    operation!(
        "admin.payment-providers.list",
        ["admin.payment-providers.list"],
        Admin,
        Get,
        "/payment-providers",
        "§6.2",
        OK_JSON
    ),
    operation!(
        "admin.payment-providers.form",
        ["admin.payment-providers.form"],
        Admin,
        Get,
        "/payment-providers/{code}/form",
        "§6.2",
        OK_JSON
    ),
    operation!(
        "admin.payments.create",
        ["admin.payments.create"],
        Admin,
        Post,
        "/payments",
        "§6.2",
        CREATED_JSON
    ),
    operation!(
        "admin.payments.update",
        ["admin.payments.update", "admin.payments.toggle"],
        Admin,
        Patch,
        "/payments/{id}",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.payments.delete",
        ["admin.payments.delete"],
        Admin,
        Delete,
        "/payments/{id}",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.payments.sort",
        ["admin.payments.sort"],
        Admin,
        Post,
        "/payments/sort",
        "§6.2",
        NO_CONTENT
    ),
    operation!(
        "admin.notices.list",
        ["admin.notices.list"],
        Admin,
        Get,
        "/notices",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.notices.create",
        ["admin.notices.create"],
        Admin,
        Post,
        "/notices",
        "§6.3",
        CREATED_JSON
    ),
    operation!(
        "admin.notices.update",
        ["admin.notices.update", "admin.notices.toggle"],
        Admin,
        Patch,
        "/notices/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.notices.delete",
        ["admin.notices.delete"],
        Admin,
        Delete,
        "/notices/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.knowledge.list",
        ["admin.knowledge.list"],
        Admin,
        Get,
        "/knowledge",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.knowledge.get",
        ["admin.knowledge.get"],
        Admin,
        Get,
        "/knowledge/{id}",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.knowledge-categories.list",
        ["admin.knowledge-categories.list"],
        Admin,
        Get,
        "/knowledge-categories",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.knowledge.create",
        ["admin.knowledge.create"],
        Admin,
        Post,
        "/knowledge",
        "§6.3",
        CREATED_JSON
    ),
    operation!(
        "admin.knowledge.update",
        ["admin.knowledge.update", "admin.knowledge.toggle"],
        Admin,
        Patch,
        "/knowledge/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.knowledge.delete",
        ["admin.knowledge.delete"],
        Admin,
        Delete,
        "/knowledge/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.knowledge.sort",
        ["admin.knowledge.sort"],
        Admin,
        Post,
        "/knowledge/sort",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.coupons.list",
        ["admin.coupons.list"],
        Admin,
        Get,
        "/coupons",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.coupons.create",
        ["admin.coupons.create"],
        Admin,
        Post,
        "/coupons",
        "§6.3",
        GENERATED_JSON_OR_CSV
    ),
    operation!(
        "admin.coupons.update",
        ["admin.coupons.update", "admin.coupons.toggle"],
        Admin,
        Patch,
        "/coupons/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.coupons.delete",
        ["admin.coupons.delete"],
        Admin,
        Delete,
        "/coupons/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.gift-cards.list",
        ["admin.gift-cards.list"],
        Admin,
        Get,
        "/gift-cards",
        "§6.3",
        OK_JSON
    ),
    operation!(
        "admin.gift-cards.create",
        ["admin.gift-cards.create"],
        Admin,
        Post,
        "/gift-cards",
        "§6.3",
        GENERATED_JSON_OR_CSV
    ),
    operation!(
        "admin.gift-cards.update",
        ["admin.gift-cards.update"],
        Admin,
        Patch,
        "/gift-cards/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.gift-cards.delete",
        ["admin.gift-cards.delete"],
        Admin,
        Delete,
        "/gift-cards/{id}",
        "§6.3",
        NO_CONTENT
    ),
    operation!(
        "admin.orders.list",
        ["admin.orders.list"],
        Admin,
        Get,
        "/orders",
        "§6.4",
        OK_JSON
    ),
    operation!(
        "admin.orders.get",
        ["admin.orders.get"],
        Admin,
        Get,
        "/orders/{trade_no}",
        "§6.4",
        OK_JSON
    ),
    operation!(
        "admin.payment-reconciliations.resolve",
        ["admin.payment-reconciliations.resolve"],
        Admin,
        Post,
        "/payment-reconciliations/{id}/resolve",
        "§6.4",
        NO_CONTENT
    ),
    operation!(
        "admin.orders.update",
        ["admin.orders.update"],
        Admin,
        Patch,
        "/orders/{trade_no}",
        "§6.4",
        NO_CONTENT
    ),
    operation!(
        "admin.orders.mark-paid",
        ["admin.orders.mark-paid"],
        Admin,
        Post,
        "/orders/{trade_no}/mark-paid",
        "§6.4",
        NO_CONTENT
    ),
    operation!(
        "admin.orders.cancel",
        ["admin.orders.cancel"],
        Admin,
        Post,
        "/orders/{trade_no}/cancel",
        "§6.4",
        NO_CONTENT
    ),
    operation!(
        "admin.orders.create",
        ["admin.orders.create"],
        Admin,
        Post,
        "/orders",
        "§6.4",
        CREATED_JSON
    ),
    operation!(
        "admin.payment-reconciliations.list",
        ["admin.payment-reconciliations.list"],
        Admin,
        Get,
        "/payment-reconciliations",
        "§6.4",
        OK_JSON
    ),
    operation!(
        "admin.tickets.list",
        ["admin.tickets.list"],
        Admin,
        Get,
        "/tickets",
        "§6.5",
        OK_JSON
    ),
    operation!(
        "admin.tickets.get",
        ["admin.tickets.get"],
        Admin,
        Get,
        "/tickets/{id}",
        "§6.5",
        OK_JSON
    ),
    operation!(
        "admin.tickets.replies.create",
        ["admin.tickets.replies.create"],
        Admin,
        Post,
        "/tickets/{id}/replies",
        "§6.5",
        NO_CONTENT
    ),
    operation!(
        "admin.tickets.close",
        ["admin.tickets.close"],
        Admin,
        Post,
        "/tickets/{id}/close",
        "§6.5",
        NO_CONTENT
    ),
    operation!(
        "admin.users.list",
        ["admin.users.list"],
        Admin,
        Get,
        "/users",
        "§6.6",
        OK_JSON
    ),
    operation!(
        "admin.users.get",
        ["admin.users.get"],
        Admin,
        Get,
        "/users/{id}",
        "§6.6",
        OK_JSON
    ),
    operation!(
        "admin.users.update",
        ["admin.users.update"],
        Admin,
        Patch,
        "/users/{id}",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.set-inviter",
        ["admin.users.set-inviter"],
        Admin,
        Post,
        "/users/{id}/set-inviter",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.create",
        ["admin.users.create"],
        Admin,
        Post,
        "/users",
        "§6.6",
        GENERATED_JSON_OR_CSV
    ),
    operation!(
        "admin.users.export",
        ["admin.users.export"],
        Admin,
        Post,
        "/users/export",
        "§6.6",
        CSV_DOWNLOAD
    ),
    operation!(
        "admin.users.mail",
        ["admin.users.mail"],
        Admin,
        Post,
        "/users/mail",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.ban",
        ["admin.users.ban"],
        Admin,
        Post,
        "/users/ban",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.reset-secret",
        ["admin.users.reset-secret"],
        Admin,
        Post,
        "/users/{id}/reset-secret",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.delete",
        ["admin.users.delete"],
        Admin,
        Delete,
        "/users/{id}",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.users.bulk-delete",
        ["admin.users.bulk-delete"],
        Admin,
        Post,
        "/users/bulk-delete",
        "§6.6",
        NO_CONTENT
    ),
    operation!(
        "admin.nodes.list",
        ["admin.nodes.list"],
        Admin,
        Get,
        "/nodes",
        "§6.7",
        OK_JSON
    ),
    operation!(
        "admin.nodes.sort",
        ["admin.nodes.sort"],
        Admin,
        Post,
        "/nodes/sort",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.server-groups.list",
        ["admin.server-groups.list"],
        Admin,
        Get,
        "/server-groups",
        "§6.7",
        OK_JSON
    ),
    operation!(
        "admin.server-groups.create",
        ["admin.server-groups.create"],
        Admin,
        Post,
        "/server-groups",
        "§6.7",
        CREATED_JSON
    ),
    operation!(
        "admin.server-groups.update",
        ["admin.server-groups.update"],
        Admin,
        Patch,
        "/server-groups/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.server-groups.delete",
        ["admin.server-groups.delete"],
        Admin,
        Delete,
        "/server-groups/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.server-routes.list",
        ["admin.server-routes.list"],
        Admin,
        Get,
        "/server-routes",
        "§6.7",
        OK_JSON
    ),
    operation!(
        "admin.server-routes.create",
        ["admin.server-routes.create"],
        Admin,
        Post,
        "/server-routes",
        "§6.7",
        CREATED_JSON
    ),
    operation!(
        "admin.server-routes.update",
        ["admin.server-routes.update"],
        Admin,
        Patch,
        "/server-routes/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.server-routes.delete",
        ["admin.server-routes.delete"],
        Admin,
        Delete,
        "/server-routes/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.servers.create",
        ["admin.servers.create"],
        Admin,
        Post,
        "/servers/{type}",
        "§6.7",
        CREATED_JSON
    ),
    operation!(
        "admin.servers.update",
        ["admin.servers.update", "admin.servers.toggle"],
        Admin,
        Patch,
        "/servers/{type}/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.servers.delete",
        ["admin.servers.delete"],
        Admin,
        Delete,
        "/servers/{type}/{id}",
        "§6.7",
        NO_CONTENT
    ),
    operation!(
        "admin.servers.copy",
        ["admin.servers.copy"],
        Admin,
        Post,
        "/servers/{type}/{id}/copy",
        "§6.7",
        CREATED_JSON
    ),
    operation!(
        "admin.stats.summary",
        ["admin.stats.summary"],
        Admin,
        Get,
        "/stats/summary",
        "§6.8",
        OK_JSON
    ),
    operation!(
        "admin.stats.server-rank",
        ["admin.stats.server-rank"],
        Admin,
        Get,
        "/stats/server-rank",
        "§6.8",
        OK_JSON
    ),
    operation!(
        "admin.stats.user-rank",
        ["admin.stats.user-rank"],
        Admin,
        Get,
        "/stats/user-rank",
        "§6.8",
        OK_JSON
    ),
    operation!(
        "admin.stats.orders",
        ["admin.stats.orders"],
        Admin,
        Get,
        "/stats/orders",
        "§6.8",
        OK_JSON
    ),
    operation!(
        "admin.stats.user-traffic",
        ["admin.stats.user-traffic"],
        Admin,
        Get,
        "/stats/user-traffic",
        "§6.8",
        OK_JSON
    ),
    operation!(
        "admin.stats.records",
        ["admin.stats.records"],
        Admin,
        Get,
        "/stats/records",
        "§6.8",
        OK_JSON
    ),
    native_operation!(
        "staff.account.mfa.get",
        Staff,
        Get,
        "/account/mfa",
        "§6.10",
        OK_JSON
    ),
    native_operation!(
        "staff.account.mfa.totp.setup",
        Staff,
        Post,
        "/account/mfa/totp",
        "§6.10",
        OK_JSON
    ),
    native_operation!(
        "staff.account.mfa.totp.confirm",
        Staff,
        Post,
        "/account/mfa/totp/confirm",
        "§6.10",
        NO_CONTENT
    ),
    native_operation!(
        "staff.account.mfa.totp.disable",
        Staff,
        Post,
        "/account/mfa/totp/disable",
        "§6.10",
        NO_CONTENT
    ),
    operation!(
        "staff.tickets.list",
        ["staff.tickets.list"],
        Staff,
        Get,
        "/tickets",
        "§6.9",
        OK_JSON
    ),
    operation!(
        "staff.tickets.get",
        ["staff.tickets.get"],
        Staff,
        Get,
        "/tickets/{id}",
        "§6.9",
        OK_JSON
    ),
    operation!(
        "staff.tickets.replies.create",
        ["staff.tickets.replies.create"],
        Staff,
        Post,
        "/tickets/{id}/replies",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.tickets.close",
        ["staff.tickets.close"],
        Staff,
        Post,
        "/tickets/{id}/close",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.users.get",
        ["staff.users.get"],
        Staff,
        Get,
        "/users/{id}",
        "§6.9",
        OK_JSON
    ),
    operation!(
        "staff.users.update",
        ["staff.users.update"],
        Staff,
        Patch,
        "/users/{id}",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.users.mail",
        ["staff.users.mail"],
        Staff,
        Post,
        "/users/mail",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.users.ban",
        ["staff.users.ban"],
        Staff,
        Post,
        "/users/ban",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.plans.list",
        ["staff.plans.list"],
        Staff,
        Get,
        "/plans",
        "§6.9",
        PLAN_LIST
    ),
    operation!(
        "staff.notices.list",
        ["staff.notices.list"],
        Staff,
        Get,
        "/notices",
        "§6.9",
        OK_JSON
    ),
    operation!(
        "staff.notices.create",
        ["staff.notices.create"],
        Staff,
        Post,
        "/notices",
        "§6.9",
        CREATED_JSON
    ),
    operation!(
        "staff.notices.update",
        ["staff.notices.update"],
        Staff,
        Patch,
        "/notices/{id}",
        "§6.9",
        NO_CONTENT
    ),
    operation!(
        "staff.notices.delete",
        ["staff.notices.delete"],
        Staff,
        Delete,
        "/notices/{id}",
        "§6.9",
        NO_CONTENT
    ),
];

pub fn operation(id: &str) -> Option<&'static InternalOperation> {
    INTERNAL_OPERATIONS
        .iter()
        .find(|operation| operation.id == id)
}

/// Add every registry operation to the generated OpenAPI JSON document.
/// Method, path, transport metadata, typed request bodies, typed success
/// responses, and RFC 9457 failures all come from the same operation entry;
/// there is no second documentation route declaration.
pub fn augment_openapi_document(document: &mut Value) {
    crate::problem::augment_problem_schema(document);
    let catalog = serde_json::to_value(OperationSchemaCatalog::openapi())
        .expect("operation schema catalog must serialize");
    let catalog_schemas = catalog["components"]["schemas"]
        .as_object()
        .expect("operation schema catalog components.schemas")
        .clone();
    let component_schemas = document["components"]["schemas"]
        .as_object_mut()
        .expect("OpenAPI components.schemas");
    for (name, schema) in catalog_schemas {
        if let Some(existing) = component_schemas.get(&name) {
            assert_eq!(
                existing, &schema,
                "conflicting OpenAPI component schema {name}"
            );
        } else {
            component_schemas.insert(name, schema);
        }
    }
    normalize_closed_component_schemas(component_schemas);
    let component_names = component_schemas
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let paths = document["paths"]
        .as_object_mut()
        .expect("OpenAPI paths must be an object");

    for operation in INTERNAL_OPERATIONS {
        let documented_path = operation.documented_path();
        let method = operation.method.as_str().to_ascii_lowercase();
        let path_item = paths
            .entry(documented_path.clone())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("OpenAPI path item must be an object");

        assert!(
            !path_item.contains_key(&method),
            "duplicate OpenAPI declaration for {} {}",
            operation.method.as_str(),
            documented_path
        );

        let mut generated = Map::new();
        generated.insert(
            "operationId".to_owned(),
            Value::String(operation.openapi_operation_id()),
        );
        generated.insert("tags".to_owned(), json!([surface_tag(operation.surface)]));
        let mut parameters = path_parameter_names(&documented_path)
            .into_iter()
            .map(|name| {
                json!({
                    "name": name,
                    "in": "path",
                    "required": true,
                    "schema": path_parameter_schema(name)
                })
            })
            .collect::<Vec<_>>();
        parameters.extend(
            operation
                .query_parameters()
                .iter()
                .chain(operation.header_parameters())
                .map(openapi_operation_parameter),
        );
        if !parameters.is_empty() {
            generated.insert("parameters".to_owned(), Value::Array(parameters));
        }
        if let Some(schema_name) = operation.request_body_schema() {
            generated.insert(
                "requestBody".to_owned(),
                json!({
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": schema_value(schema_name, &component_names)
                        }
                    }
                }),
            );
        }
        generated.insert(
            "responses".to_owned(),
            generated_responses(operation, &component_names),
        );
        match operation_security(operation) {
            OperationSecurity::None => {}
            OperationSecurity::RequiredBearer => {
                generated.insert("security".to_owned(), json!([{ "bearer_auth": [] }]));
            }
            OperationSecurity::OptionalBearer => {
                generated.insert("security".to_owned(), json!([{ "bearer_auth": [] }, {}]));
            }
        }
        add_registry_extensions(&mut generated, operation);
        path_item.insert(method, Value::Object(generated));
    }
}

/// Serde's `deny_unknown_fields` is not projected consistently by utoipa for
/// flattened/allOf fragments and inline tagged-enum variants. Close every
/// ordinary object recursively at the generated-document boundary. Deliberate
/// map/extension islands already carry `additionalProperties` and are left
/// intact; RFC 9457's open `ProblemDetails` is handled explicitly by its own
/// augmenter.
fn normalize_closed_component_schemas(schemas: &mut Map<String, Value>) {
    let catalog = schemas.clone();
    for (name, schema) in schemas {
        if name != "ProblemDetails" {
            normalize_structural_schema(schema, &catalog, 0);
        }
    }
}

fn normalize_structural_schema(schema: &mut Value, catalog: &Map<String, Value>, depth: usize) {
    assert!(
        depth < 32,
        "OpenAPI structural schema recursion is too deep"
    );
    if let Some(flattened) = flatten_structural_all_of(schema, catalog, depth) {
        *schema = flattened;
    }

    match schema {
        Value::Array(items) => {
            for item in items {
                normalize_structural_schema(item, catalog, depth + 1);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                normalize_structural_schema(value, catalog, depth + 1);
            }
            let object_type = object.get("type").is_some_and(|kind| {
                kind == "object"
                    || kind
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "object"))
            });
            if (object_type || object.contains_key("properties"))
                && !object.contains_key("additionalProperties")
            {
                object.insert("additionalProperties".to_owned(), Value::Bool(false));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn flatten_structural_all_of(
    schema: &Value,
    catalog: &Map<String, Value>,
    depth: usize,
) -> Option<Value> {
    let original = schema.as_object()?;
    let members = original.get("allOf")?.as_array()?;
    let mut output = original.clone();
    output.remove("allOf");
    let mut properties = output
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut required = output
        .remove("required")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<std::collections::BTreeSet<_>>();

    for member in members {
        let mut member = resolve_schema_reference(member, catalog)?;
        normalize_structural_schema(&mut member, catalog, depth + 1);
        let member = member.as_object()?;
        let is_object = member.get("type").is_some_and(|kind| kind == "object")
            || member.contains_key("properties");
        if !is_object
            || member.contains_key("oneOf")
            || member.contains_key("anyOf")
            || member.contains_key("not")
        {
            return None;
        }
        if let Some(member_properties) = member.get("properties").and_then(Value::as_object) {
            for (name, property) in member_properties {
                if let Some(existing) = properties.insert(name.clone(), property.clone()) {
                    assert_eq!(
                        &existing, property,
                        "conflicting property {name} while flattening OpenAPI allOf"
                    );
                }
            }
        }
        if let Some(member_required) = member.get("required").and_then(Value::as_array) {
            required.extend(member_required.iter().map(|value| {
                value
                    .as_str()
                    .expect("required item must be a string")
                    .to_owned()
            }));
        }
    }

    output.insert("type".to_owned(), Value::String("object".to_owned()));
    output.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        output.insert(
            "required".to_owned(),
            Value::Array(required.into_iter().map(Value::String).collect()),
        );
    }
    output.insert("additionalProperties".to_owned(), Value::Bool(false));
    Some(Value::Object(output))
}

fn resolve_schema_reference(schema: &Value, catalog: &Map<String, Value>) -> Option<Value> {
    let object = schema.as_object()?;
    let Some(reference) = object.get("$ref").and_then(Value::as_str) else {
        return Some(schema.clone());
    };
    let name = reference.strip_prefix("#/components/schemas/")?;
    catalog.get(name).cloned()
}

fn add_registry_extensions(target: &mut Map<String, Value>, operation: &InternalOperation) {
    target.insert(
        "x-v2board-operation-id".to_owned(),
        Value::String(operation.id.to_owned()),
    );
    target.insert(
        "x-v2board-logical-operation-ids".to_owned(),
        json!(operation.logical_ids),
    );
    target.insert(
        "x-v2board-spec-section".to_owned(),
        Value::String(operation.spec_section.to_owned()),
    );
}

fn generated_responses(
    operation: &InternalOperation,
    component_names: &std::collections::BTreeSet<String>,
) -> Value {
    let mut responses = Map::new();
    for response in operation.successes {
        let mut value = Map::new();
        value.insert(
            "description".to_owned(),
            Value::String(success_description(operation, response.status).to_owned()),
        );
        if !response.representations.is_empty() {
            let content = response
                .representations
                .iter()
                .map(|representation| {
                    let schema_name = match representation.content_type {
                        "application/json" => operation.json_success_schema(response.status),
                        "text/csv; charset=utf-8" => "string",
                        other => panic!(
                            "unclassified success representation {other} on {}",
                            operation.id
                        ),
                    };
                    (
                        representation.content_type.to_owned(),
                        json!({
                            "schema": schema_value(schema_name, component_names)
                        }),
                    )
                })
                .collect::<Map<_, _>>();
            value.insert("content".to_owned(), Value::Object(content));
        }
        if !response.headers.is_empty() {
            let headers = response
                .headers
                .iter()
                .map(|name| {
                    (
                        (*name).to_owned(),
                        json!({
                            "description": format!("{name} response header"),
                            "schema": { "type": "string" }
                        }),
                    )
                })
                .collect::<Map<_, _>>();
            value.insert("headers".to_owned(), Value::Object(headers));
        }
        responses.insert(response.status.to_string(), Value::Object(value));
    }
    responses.insert(
        "default".to_owned(),
        json!({ "$ref": "#/components/responses/DefaultProblem" }),
    );
    Value::Object(responses)
}

fn schema_value(name: &str, component_names: &std::collections::BTreeSet<String>) -> Value {
    match name {
        "string" => json!({ "type": "string" }),
        name if name.ends_with("[]") => {
            let item = name.trim_end_matches("[]");
            assert!(
                component_names.contains(item),
                "unregistered OpenAPI array item schema {item}"
            );
            json!({
                "type": "array",
                "items": { "$ref": format!("#/components/schemas/{item}") }
            })
        }
        name if component_names.contains(name) => {
            json!({ "$ref": format!("#/components/schemas/{name}") })
        }
        name => panic!("unregistered OpenAPI component schema {name}"),
    }
}

fn path_parameter_schema(name: &str) -> Value {
    match name {
        "id" => json!({ "type": "integer", "format": "int64", "minimum": 1 }),
        "secure_path" | "session_id" | "trade_no" | "code" | "type" => {
            json!({ "type": "string", "minLength": 1 })
        }
        other => panic!("unclassified internal API path parameter {other}"),
    }
}

fn openapi_operation_parameter(parameter: &OperationParameter) -> Value {
    let mut value = Map::new();
    value.insert("name".to_owned(), Value::String(parameter.name.to_owned()));
    value.insert(
        "in".to_owned(),
        Value::String(parameter.location.as_str().to_owned()),
    );
    value.insert("required".to_owned(), Value::Bool(parameter.required));
    value.insert(
        "description".to_owned(),
        Value::String(parameter.description.to_owned()),
    );
    value.insert(
        "schema".to_owned(),
        parameter_schema_value(parameter.schema),
    );
    if let Some(style) = parameter.style {
        value.insert("style".to_owned(), Value::String(style.to_owned()));
    }
    if let Some(explode) = parameter.explode {
        value.insert("explode".to_owned(), Value::Bool(explode));
    }
    Value::Object(value)
}

fn parameter_schema_value(schema: ParameterSchema) -> Value {
    match schema {
        ParameterSchema::String {
            values,
            min_length,
            max_length,
            default,
        } => {
            let mut schema =
                Map::from_iter([("type".to_owned(), Value::String("string".to_owned()))]);
            if !values.is_empty() {
                schema.insert("enum".to_owned(), json!(values));
            }
            if let Some(min_length) = min_length {
                schema.insert("minLength".to_owned(), json!(min_length));
            }
            if let Some(max_length) = max_length {
                schema.insert("x-v2board-max-bytes".to_owned(), json!(max_length));
            }
            if let Some(default) = default {
                schema.insert("default".to_owned(), Value::String(default.to_owned()));
            }
            Value::Object(schema)
        }
        ParameterSchema::Integer {
            format,
            minimum,
            maximum,
            default,
        } => {
            let mut schema = Map::from_iter([
                ("type".to_owned(), Value::String("integer".to_owned())),
                ("format".to_owned(), Value::String(format.to_owned())),
            ]);
            if let Some(minimum) = minimum {
                schema.insert("minimum".to_owned(), json!(minimum));
            }
            if let Some(maximum) = maximum {
                schema.insert("maximum".to_owned(), json!(maximum));
            }
            if let Some(default) = default {
                schema.insert("default".to_owned(), json!(default));
            }
            Value::Object(schema)
        }
        ParameterSchema::Boolean { default } => {
            let mut schema =
                Map::from_iter([("type".to_owned(), Value::String("boolean".to_owned()))]);
            if let Some(default) = default {
                schema.insert("default".to_owned(), Value::Bool(default));
            }
            Value::Object(schema)
        }
        ParameterSchema::IntegerArray { format } => json!({
            "type": "array",
            "items": {
                "type": "integer",
                "format": format,
            },
        }),
    }
}

fn success_description(operation: &InternalOperation, status: u16) -> &'static str {
    match (operation.id, status) {
        ("admin.plans.list", 200) => "All plans",
        ("admin.plans.create", 201) => "Created plan",
        ("admin.plans.update", 204) => "Plan updated",
        ("admin.plans.delete", 204) => "Plan deleted",
        ("admin.plans.sort", 204) => "Plans reordered",
        (_, 204) => "No content",
        (_, 302) => "Redirect",
        _ => "Success",
    }
}

fn path_parameter_names(path: &str) -> Vec<&str> {
    path.split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .collect()
}

fn surface_tag(surface: OperationSurface) -> &'static str {
    match surface {
        OperationSurface::Public => "public",
        OperationSurface::Auth => "auth",
        OperationSurface::User => "user",
        OperationSurface::Admin => "admin",
        OperationSurface::Staff => "staff",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationSecurity {
    None,
    OptionalBearer,
    RequiredBearer,
}

fn operation_security(operation: &InternalOperation) -> OperationSecurity {
    if matches!(operation.id, "auth.session.get" | "auth.session.delete") {
        return OperationSecurity::OptionalBearer;
    }
    if matches!(
        operation.surface,
        OperationSurface::User | OperationSurface::Admin | OperationSurface::Staff
    ) || matches!(operation.id, "auth.step-up" | "auth.quick-login-url")
    {
        OperationSecurity::RequiredBearer
    } else {
        OperationSecurity::None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    #[test]
    fn registry_is_exact_unique_and_fully_documented() {
        assert_eq!(INTERNAL_OPERATIONS.len(), 158);
        let mut ids = BTreeSet::new();
        let mut logical_ids = BTreeSet::new();
        let mut routes = BTreeSet::new();
        let mut surfaces = BTreeMap::new();
        for operation in INTERNAL_OPERATIONS {
            assert!(ids.insert(operation.id), "duplicate id {}", operation.id);
            assert!(
                routes.insert((operation.surface, operation.method, operation.path)),
                "duplicate route {} {}",
                operation.method.as_str(),
                operation.documented_path()
            );
            assert!(operation.path.starts_with('/'));
            assert!(!operation.path.contains("{secure_path}"));
            assert!(operation.documented);
            assert!(operation.spec_section.starts_with('§'));
            assert!(!operation.successes.is_empty());
            for response in operation.successes {
                assert!((200..400).contains(&response.status));
            }
            for logical_id in operation.logical_ids {
                assert!(
                    logical_ids.insert(*logical_id),
                    "duplicate logical id {logical_id}"
                );
            }
            *surfaces.entry(operation.surface).or_insert(0_usize) += 1;
        }
        assert_eq!(logical_ids.len(), 164);
        assert_eq!(surfaces[&OperationSurface::Public], 2);
        assert_eq!(surfaces[&OperationSurface::Auth], 10);
        assert_eq!(surfaces[&OperationSurface::User], 40);
        assert_eq!(surfaces[&OperationSurface::Admin], 89);
        assert_eq!(surfaces[&OperationSurface::Staff], 17);
        assert_eq!(
            INTERNAL_OPERATIONS
                .iter()
                .filter(|operation| !operation.legacy_mapped)
                .count(),
            9
        );
    }

    #[test]
    fn dynamic_surfaces_derive_runtime_and_documented_paths() {
        let admin = operation("admin.plans.list").expect("admin plans operation");
        assert_eq!(admin.runtime_path(), "/plans");
        assert_eq!(admin.documented_path(), "/api/v1/{secure_path}/plans");
        let user = operation("user.plans.list").expect("user plans operation");
        assert_eq!(user.runtime_path(), "/api/v1/user/plans");
        assert_eq!(user.documented_path(), "/api/v1/user/plans");
    }

    #[test]
    fn exceptional_success_sets_are_explicit() {
        let quick = operation("auth.quick-login").unwrap();
        assert_eq!(quick.successes, FOUND_LOCATION);
        let config = operation("admin.config.update").unwrap();
        assert_eq!(config.successes, CONFIG_PATCH);
        for id in [
            "admin.coupons.create",
            "admin.gift-cards.create",
            "admin.users.create",
        ] {
            assert_eq!(
                operation(id).unwrap().successes,
                GENERATED_JSON_OR_CSV,
                "{id}"
            );
        }
        assert_eq!(
            operation("admin.users.export").unwrap().successes,
            CSV_DOWNLOAD
        );
    }

    #[test]
    fn augmented_openapi_contains_the_exact_registry_operation_set() {
        let mut document =
            serde_json::to_value(crate::InternalApiDoc::openapi()).expect("base OpenAPI");
        augment_openapi_document(&mut document);
        let mut operation_ids = BTreeSet::new();
        let mut count = 0_usize;
        for path_item in document["paths"].as_object().expect("paths").values() {
            for method in ["get", "post", "put", "patch", "delete"] {
                if let Some(operation) = path_item.get(method) {
                    count += 1;
                    assert!(
                        operation_ids.insert(
                            operation["operationId"]
                                .as_str()
                                .expect("operationId")
                                .to_owned()
                        ),
                        "duplicate OpenAPI operationId"
                    );
                }
            }
        }
        assert_eq!(count, 158);
        assert_eq!(operation_ids.len(), 158);
        for operation in INTERNAL_OPERATIONS {
            let wire = &document["paths"][operation.documented_path()]
                [operation.method.as_str().to_ascii_lowercase()];
            assert_eq!(
                wire["x-v2board-operation-id"],
                Value::String(operation.id.to_owned())
            );
            for name in path_parameter_names(&operation.documented_path()) {
                assert!(wire["parameters"].as_array().is_some_and(|parameters| {
                    parameters
                        .iter()
                        .any(|parameter| parameter["name"] == name && parameter["required"] == true)
                }));
            }
            if operation.request_body_schema().is_some() {
                assert_eq!(wire["requestBody"]["required"], true);
                assert!(wire["requestBody"]["content"]["application/json"].is_object());
            }
            assert_eq!(
                wire["responses"]["default"]["$ref"],
                "#/components/responses/DefaultProblem"
            );
            assert!(
                wire["responses"]
                    .as_object()
                    .expect("operation responses")
                    .keys()
                    .filter_map(|status| status.parse::<u16>().ok())
                    .all(|status| status < 400)
            );
        }
    }

    #[test]
    fn generated_structural_objects_are_flattened_and_closed() {
        let document = augmented_document();
        let schemas = document["components"]["schemas"]
            .as_object()
            .expect("component schemas");

        for name in [
            "AdminOrderDetail",
            "AdminOrderListItem",
            "AdminPaymentReconciliationListItem",
            "AdminTicketDetail",
            "AdminUserDetail",
            "AdminUserListItem",
            "StaffUserDetail",
        ] {
            let schema = &schemas[name];
            assert!(schema.get("allOf").is_none(), "{name} must be flattened");
            assert_eq!(schema["type"], "object", "{name}");
            assert_eq!(schema["additionalProperties"], false, "{name}");
        }
        assert!(
            schemas["AdminOrderDetail"]["properties"]
                .get("trade_no")
                .is_some()
        );
        assert!(
            schemas["AdminOrderDetail"]["properties"]
                .get("commission_log")
                .is_some()
        );

        for name in ["CreateOrderRequest", "ServerNodeView"] {
            for variant in schemas[name]["oneOf"]
                .as_array()
                .unwrap_or_else(|| panic!("{name} oneOf"))
            {
                assert!(variant.get("allOf").is_none(), "{name} variant");
                assert_eq!(variant["type"], "object", "{name} variant");
                assert_eq!(variant["additionalProperties"], false, "{name} variant");
            }
        }

        fn assert_object_policy(schema: &Value, path: &str) {
            match schema {
                Value::Array(items) => {
                    for (index, item) in items.iter().enumerate() {
                        assert_object_policy(item, &format!("{path}/{index}"));
                    }
                }
                Value::Object(object) => {
                    let structural_object = object.get("type").is_some_and(|kind| {
                        kind == "object"
                            || kind
                                .as_array()
                                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "object"))
                    }) || object.contains_key("properties");
                    if structural_object {
                        assert!(
                            object.contains_key("additionalProperties"),
                            "object schema lacks explicit unknown-field policy at {path}: {schema}"
                        );
                    }
                    for (name, child) in object {
                        assert_object_policy(child, &format!("{path}/{name}"));
                    }
                }
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }

        for (name, schema) in schemas {
            if name != "ProblemDetails" {
                assert_object_policy(schema, name);
            }
        }
        assert!(
            schemas["AdminPaymentPatchRequest"]["properties"]
                .get("config")
                .is_none(),
            "provider configuration is immutable and must not leak into PATCH"
        );
    }

    fn assert_named_json_root(schema: &Value, context: &str) {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            assert_ne!(
                reference, "#/components/schemas/JsonValue",
                "{context} uses the forbidden JsonValue root"
            );
            assert!(
                reference.starts_with("#/components/schemas/"),
                "{context} has a non-component root ref {reference}"
            );
            return;
        }
        if schema.get("type") == Some(&Value::String("array".to_owned())) {
            let item_reference = schema["items"]["$ref"]
                .as_str()
                .unwrap_or_else(|| panic!("{context} array items must use a named schema"));
            assert_ne!(
                item_reference, "#/components/schemas/JsonValue",
                "{context} uses the forbidden JsonValue item root"
            );
            assert!(item_reference.starts_with("#/components/schemas/"));
            return;
        }
        panic!("{context} JSON root is not a named DTO: {schema}");
    }

    #[test]
    fn every_json_request_and_success_has_a_named_non_json_value_root() {
        let document = augmented_document();
        let mut json_request_count = 0_usize;
        let mut json_success_count = 0_usize;
        for operation in INTERNAL_OPERATIONS {
            let wire = wire_operation(&document, operation.id);
            match operation.request_body_schema() {
                Some(_) => {
                    json_request_count += 1;
                    assert_named_json_root(
                        &wire["requestBody"]["content"]["application/json"]["schema"],
                        &format!("{} request", operation.id),
                    );
                }
                None => assert!(
                    wire.get("requestBody").is_none(),
                    "{} must not declare a request body",
                    operation.id
                ),
            }
            for response in operation.successes {
                let response_wire = &wire["responses"][response.status.to_string()];
                for representation in response.representations {
                    let schema = &response_wire["content"][representation.content_type]["schema"];
                    if representation.content_type == "application/json" {
                        json_success_count += 1;
                        assert_named_json_root(
                            schema,
                            &format!("{} {} response", operation.id, response.status),
                        );
                    } else if representation.content_type == "text/csv; charset=utf-8" {
                        assert_eq!(schema, &json!({ "type": "string" }));
                    } else {
                        panic!(
                            "{} has unclassified success content type {}",
                            operation.id, representation.content_type
                        );
                    }
                }
            }
        }
        assert_eq!(json_request_count, 64);
        assert_eq!(json_success_count, 95);
    }

    fn assert_local_refs_resolve(document: &Value, value: &Value, location: &str) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    let pointer = reference
                        .strip_prefix('#')
                        .unwrap_or_else(|| panic!("external ref at {location}: {reference}"));
                    assert!(
                        document.pointer(pointer).is_some(),
                        "unresolved ref at {location}: {reference}"
                    );
                }
                for (key, child) in object {
                    assert_local_refs_resolve(document, child, &format!("{location}/{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    assert_local_refs_resolve(document, child, &format!("{location}/{index}"));
                }
            }
            _ => {}
        }
    }

    #[test]
    fn every_generated_openapi_reference_resolves() {
        let document = augmented_document();
        assert_local_refs_resolve(&document, &document, "#");
    }

    #[test]
    fn openapi_distinguishes_required_optional_and_absent_bearer_auth() {
        let mut document =
            serde_json::to_value(crate::InternalApiDoc::openapi()).expect("base OpenAPI");
        augment_openapi_document(&mut document);

        for path in ["/api/v1/auth/session"] {
            for method in ["get", "delete"] {
                assert_eq!(
                    document["paths"][path][method]["security"],
                    json!([{ "bearer_auth": [] }, {}])
                );
            }
        }
        assert_eq!(
            document["paths"]["/api/v1/auth/step-up"]["post"]["security"],
            json!([{ "bearer_auth": [] }])
        );
        assert_eq!(
            document["paths"]["/api/v1/user/profile"]["get"]["security"],
            json!([{ "bearer_auth": [] }])
        );
        assert!(
            document["paths"]["/api/v1/public/config"]["get"]
                .get("security")
                .is_none()
        );
    }

    fn augmented_document() -> Value {
        let mut document =
            serde_json::to_value(crate::InternalApiDoc::openapi()).expect("base OpenAPI");
        augment_openapi_document(&mut document);
        document
    }

    fn wire_operation<'a>(document: &'a Value, id: &str) -> &'a Value {
        let operation = operation(id).expect("registered operation");
        &document["paths"][operation.documented_path()]
            [operation.method.as_str().to_ascii_lowercase()]
    }

    fn wire_parameter<'a>(operation: &'a Value, location: &str, name: &str) -> &'a Value {
        operation["parameters"]
            .as_array()
            .expect("operation parameters")
            .iter()
            .find(|parameter| parameter["in"] == location && parameter["name"] == name)
            .unwrap_or_else(|| panic!("missing {location} parameter {name}"))
    }

    #[test]
    fn query_contract_is_exactly_the_twenty_two_extractor_operations() {
        let expected = BTreeSet::from([
            "admin.config.get",
            "admin.coupons.list",
            "admin.gift-cards.list",
            "admin.orders.list",
            "admin.payment-providers.form",
            "admin.payment-reconciliations.list",
            "admin.server-groups.list",
            "admin.stats.records",
            "admin.stats.server-rank",
            "admin.stats.user-rank",
            "admin.stats.user-traffic",
            "admin.system.audit-logs.list",
            "admin.system.logs",
            "admin.tickets.list",
            "admin.users.list",
            "auth.quick-login",
            "staff.tickets.list",
            "user.commissions.list",
            "user.knowledge-categories.list",
            "user.knowledge.list",
            "user.notices.list",
            "user.orders.list",
        ]);
        let actual = INTERNAL_OPERATIONS
            .iter()
            .filter(|operation| !operation.query_parameters().is_empty())
            .map(|operation| operation.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);

        for operation in INTERNAL_OPERATIONS {
            let mut names = BTreeSet::new();
            for parameter in operation.query_parameters() {
                assert_eq!(parameter.location, ParameterLocation::Query);
                assert!(
                    names.insert(parameter.name),
                    "duplicate query parameter {} on {}",
                    parameter.name,
                    operation.id
                );
            }
        }
    }

    #[test]
    fn openapi_pins_query_required_enums_pagination_and_repeat_encoding() {
        let document = augmented_document();
        let quick = wire_operation(&document, "auth.quick-login");
        let token = wire_parameter(quick, "query", "token");
        assert_eq!(token["required"], true);
        assert_eq!(token["schema"]["minLength"], 1);
        assert_eq!(token["schema"]["x-v2board-max-bytes"], 256);

        let notices = wire_operation(&document, "user.notices.list");
        let page = wire_parameter(notices, "query", "page");
        assert_eq!(page["schema"]["minimum"], 1);
        assert_eq!(page["schema"]["default"], 1);
        let per_page = wire_parameter(notices, "query", "per_page");
        assert_eq!(per_page["schema"]["minimum"], 1);
        assert_eq!(per_page["schema"]["maximum"], 100);
        assert_eq!(per_page["schema"]["default"], 5);

        for id in ["admin.stats.server-rank", "admin.stats.user-rank"] {
            let window = wire_parameter(wire_operation(&document, id), "query", "window");
            assert_eq!(window["required"], true);
            assert_eq!(window["schema"]["enum"], json!(["today", "previous"]));
        }
        assert_eq!(
            wire_parameter(
                wire_operation(&document, "admin.stats.user-traffic"),
                "query",
                "user_id"
            )["required"],
            true
        );
        assert_eq!(
            wire_parameter(
                wire_operation(&document, "admin.stats.records"),
                "query",
                "type"
            )["schema"],
            json!({ "type": "string", "enum": ["d", "m"], "default": "d" })
        );

        let reply_status = wire_parameter(
            wire_operation(&document, "admin.tickets.list"),
            "query",
            "reply_status",
        );
        assert_eq!(reply_status["required"], false);
        assert_eq!(reply_status["style"], "form");
        assert_eq!(reply_status["explode"], true);
        assert_eq!(reply_status["schema"]["type"], "array");
        assert_eq!(reply_status["schema"]["items"]["format"], "int64");

        let staff_names = wire_operation(&document, "staff.tickets.list")["parameters"]
            .as_array()
            .expect("staff ticket parameters")
            .iter()
            .filter(|parameter| parameter["in"] == "query")
            .map(|parameter| parameter["name"].as_str().expect("parameter name"))
            .collect::<BTreeSet<_>>();
        assert_eq!(staff_names, BTreeSet::from(["page", "per_page", "status"]));
    }

    #[test]
    fn operation_specific_headers_follow_the_runtime_guards() {
        let mut user_agent = BTreeSet::new();
        let mut idempotency = BTreeSet::new();
        let mut step_up = BTreeSet::new();
        for operation in INTERNAL_OPERATIONS {
            let headers = operation.header_parameters();
            let names = headers
                .iter()
                .map(|parameter| {
                    assert_eq!(parameter.location, ParameterLocation::Header);
                    assert!(!parameter.required);
                    parameter.name
                })
                .collect::<BTreeSet<_>>();
            assert!(
                names.contains("Accept-Language"),
                "{} has no common locale header",
                operation.id
            );
            if names.contains("User-Agent") {
                user_agent.insert(operation.id);
            }
            if names.contains("Idempotency-Key") {
                idempotency.insert(operation.id);
            }
            if names.contains("X-V2Board-Step-Up") {
                step_up.insert(operation.id);
            }
            let expects_step_up = (matches!(
                operation.surface,
                OperationSurface::Admin | OperationSurface::Staff
            ) && operation.method != HttpMethod::Get)
                || matches!(
                    operation.id,
                    "admin.nodes.list" | "admin.payment-reconciliations.list"
                );
            assert_eq!(
                names.contains("X-V2Board-Step-Up"),
                expects_step_up,
                "{}",
                operation.id
            );
        }
        assert_eq!(
            user_agent,
            BTreeSet::from(["auth.login", "auth.register", "auth.token-login"])
        );
        assert_eq!(
            idempotency,
            BTreeSet::from(["admin.users.mail", "staff.users.mail"])
        );
        assert_eq!(step_up.len(), 67);

        let document = augmented_document();
        for operation in INTERNAL_OPERATIONS {
            let language = wire_parameter(
                wire_operation(&document, operation.id),
                "header",
                "Accept-Language",
            );
            assert_eq!(language["required"], false, "{}", operation.id);
        }
        let mail = wire_operation(&document, "admin.users.mail");
        assert_eq!(
            wire_parameter(mail, "header", "Idempotency-Key")["schema"]["x-v2board-max-bytes"],
            512
        );
        let step = wire_parameter(mail, "header", "X-V2Board-Step-Up");
        assert_eq!(step["required"], false);
        assert_eq!(step["schema"]["x-v2board-max-bytes"], 256);
        assert!(
            step["description"]
                .as_str()
                .expect("step-up description")
                .contains("Conditionally required")
        );
    }
}

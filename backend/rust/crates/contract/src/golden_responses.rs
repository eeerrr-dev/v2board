//! Golden-response contract lane for the admin API surface.
//!
//! `golden-responses` regenerates or verifies the `admin.*` JSON fixtures in
//! `frontend/packages/api-client/goldens`. Each fixture is a full response
//! body produced by the real application services and remaining admin service
//! against a disposable, freshly migrated PostgreSQL database seeded with
//! fixed timestamps, identifiers, and codes — since W14 every family
//! serializes the bare modern dialect-v2 body (no legacy envelope remains).
//! The api-client vitest suite parses
//! every fixture with its zod contract schema, closing the Rust→zod edge
//! that previously drifted silently. The `guest.*`/`passport.*`/`user.*`
//! fixtures for pure serde response structs are owned by the `v2board-api`
//! `golden_wire` test.
//!
//! Default mode verifies the checked-in fixtures byte-for-byte; set
//! `UPDATE_GOLDENS=1` (wired through `make contract-goldens`) to rewrite
//! them.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_api_contract::{
    AdminPlanItem, Page,
    admin_business::{
        AdminPaymentItem, AdminTicketDetail, AdminTicketItem, AdminTicketMessageItem,
    },
    admin_codes::{AdminCouponItem, AdminGiftcardItem},
    admin_platform::{AdminUserTrafficView, StatSeriesPointView},
    content::{KnowledgeDetailView, KnowledgeSummaryView, NoticeView},
    time::Rfc3339Timestamp,
};
use v2board_application::{
    admin_order::{AdminOrderQuery, OrderSort},
    admin_user::{AdminUserListRequest, UserSort},
    content::{ContentService, KnowledgeArticle, KnowledgeSummary, Notice},
    payment::{PaymentMethod, PaymentService},
    plan::{Plan, PlanService},
    promotion::{
        AdminCoupon as ApplicationAdminCoupon, GiftCard as ApplicationGiftCard, PromotionService,
    },
    reconciliation::ReconciliationService,
    statistics::{StatisticsBucket, StatisticsService},
    ticket::{
        OperatorTicketListQuery, OperatorTicketOrder, Ticket, TicketDetail, TicketMessage,
        TicketRepository,
    },
};
use v2board_compat::Pagination;
use v2board_config::AppConfig;
use v2board_db::{
    admin_payment::PostgresPaymentRepository, content::PostgresContentRepository,
    coupon::PostgresPromotionRepository, plan::PostgresPlanRepository,
    reconciliation::PostgresReconciliationRepository, statistics::PostgresStatisticsRepository,
    ticket::PostgresTicketRepository,
};
use v2board_domain_model::{MoneyMinor, PlanPricePeriod};
use v2board_payment_adapters::{EncryptedPaymentSecurity, Sha256ReconciliationIdentityHasher};

use crate::admin_user_projection::{
    admin_user_detail, admin_user_list_item, contract_admin_user_service,
};
use crate::order_projection::{admin_order_detail, admin_order_list, contract_order_service};
use crate::production_invariants::{
    DEFAULT_INTEGRATION_REDIS_URL, DEFAULT_ROOT_DATABASE_URL, GeneratedDatabaseName, MIGRATOR,
    create_database, database_url_for, drop_database, env_or, flush_redis, integration_config,
};
use crate::reconciliation_projection::admin_reconciliation;
use crate::server_projection::{
    contract_server_service, server_group_view, server_node_view, server_route_view,
};
use crate::statistics_calendar::ContractStatisticsCalendar;

/// 2023-11-14T22:13:20Z. Every seeded row pins its timestamps near this value
/// so regeneration is byte-stable regardless of when it runs.
const GOLDEN_TIME: i64 = 1_700_000_000;
/// The one immutable installation identity the disposable database carries.
const GOLDEN_INSTALLATION_ID: &str = "00000000-0000-4000-8000-00000000000a";
const DEFAULT_GOLDENS_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../frontend/packages/api-client/goldens"
);
/// This lane owns exactly the `admin.` fixtures inside the goldens directory.
const ADMIN_GOLDEN_PREFIX: &str = "admin.";

fn admin_ticket_item(ticket: Ticket) -> AdminTicketItem {
    AdminTicketItem {
        id: ticket.id,
        user_id: ticket.user_id,
        subject: ticket.subject,
        level: ticket.level.code(),
        status: ticket.status.code(),
        reply_status: ticket.reply_status.code(),
        last_reply_user_id: ticket.last_reply_user_id,
        created_at: Rfc3339Timestamp::from_epoch_seconds(ticket.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(ticket.updated_at),
    }
}

fn admin_coupon_document(view: ApplicationAdminCoupon) -> AdminCouponItem {
    AdminCouponItem {
        id: view.id,
        code: view.code,
        name: view.name,
        coupon_type: view.kind_code,
        value: view.value,
        show: view.visible,
        limit_use: view.remaining_uses,
        limit_use_with_user: view.per_user_limit,
        limit_plan_ids: view.plan_ids,
        limit_period: view.periods,
        started_at: Rfc3339Timestamp::from_epoch_seconds(view.starts_at),
        ended_at: Rfc3339Timestamp::from_epoch_seconds(view.ends_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn admin_giftcard_document(view: ApplicationGiftCard) -> AdminGiftcardItem {
    AdminGiftcardItem {
        id: view.id,
        code: view.code,
        name: view.name,
        card_type: view.kind_code,
        value: view.value,
        plan_id: view.plan_id,
        limit_use: view.remaining_uses,
        used_user_ids: view.redeemed_user_ids,
        started_at: Rfc3339Timestamp::from_epoch_seconds(view.starts_at),
        ended_at: Rfc3339Timestamp::from_epoch_seconds(view.ends_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn admin_ticket_message(message: TicketMessage) -> AdminTicketMessageItem {
    AdminTicketMessageItem {
        id: message.id,
        user_id: message.user_id,
        ticket_id: message.ticket_id,
        message: message.message,
        is_me: message.is_me,
        created_at: Rfc3339Timestamp::from_epoch_seconds(message.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(message.updated_at),
    }
}

fn admin_ticket_detail(detail: TicketDetail) -> AdminTicketDetail {
    AdminTicketDetail {
        ticket: admin_ticket_item(detail.ticket),
        message: detail
            .messages
            .into_iter()
            .map(admin_ticket_message)
            .collect(),
    }
}

fn admin_payment_document(payment: PaymentMethod) -> Result<AdminPaymentItem> {
    let handling_fee_percent = payment
        .handling_fee_percent
        .map(|value| {
            value
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .context("stored payment fee is not a finite number")
        })
        .transpose()?;
    Ok(AdminPaymentItem {
        id: payment.id,
        name: payment.name,
        payment: payment.provider,
        icon: payment.icon,
        handling_fee_fixed: payment.handling_fee_fixed,
        handling_fee_percent,
        uuid: payment.uuid,
        config: payment.config,
        notify_domain: payment.notify_domain,
        notify_url: payment.notify_url,
        enable: payment.enable,
        sort: payment.sort,
        created_at: Rfc3339Timestamp::from_epoch_seconds(payment.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(payment.updated_at),
        legacy_md5_signature: payment.legacy_md5_signature,
        security_warning: payment.security_warning,
    })
}

fn admin_plan_document(view: Plan) -> AdminPlanItem {
    AdminPlanItem {
        id: view.id,
        group_id: view.group_id,
        transfer_enable: view.transfer_enable,
        device_limit: view.device_limit,
        name: view.name,
        speed_limit: view.speed_limit,
        show: view.show,
        sort: view.sort,
        renew: view.renew,
        content: view.content,
        month_price: view.prices.get(PlanPricePeriod::Month).map(MoneyMinor::get),
        quarter_price: view
            .prices
            .get(PlanPricePeriod::Quarter)
            .map(MoneyMinor::get),
        half_year_price: view
            .prices
            .get(PlanPricePeriod::HalfYear)
            .map(MoneyMinor::get),
        year_price: view.prices.get(PlanPricePeriod::Year).map(MoneyMinor::get),
        two_year_price: view
            .prices
            .get(PlanPricePeriod::TwoYear)
            .map(MoneyMinor::get),
        three_year_price: view
            .prices
            .get(PlanPricePeriod::ThreeYear)
            .map(MoneyMinor::get),
        onetime_price: view
            .prices
            .get(PlanPricePeriod::OneTime)
            .map(MoneyMinor::get),
        reset_price: view.prices.get(PlanPricePeriod::Reset).map(MoneyMinor::get),
        reset_traffic_method: view.reset_traffic_method,
        capacity_limit: view.capacity_limit,
        count: view.count,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn notice_document(view: Notice) -> NoticeView {
    NoticeView {
        id: view.id,
        title: view.title,
        content: view.content,
        show: view.visibility.is_visible(),
        img_url: view.img_url,
        tags: view.tags,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn knowledge_summary_document(view: KnowledgeSummary) -> KnowledgeSummaryView {
    KnowledgeSummaryView {
        id: view.id,
        category: view.category,
        title: view.title,
        sort: view.sort,
        show: view.visibility.is_visible(),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn knowledge_detail_document(view: KnowledgeArticle) -> KnowledgeDetailView {
    KnowledgeDetailView {
        id: view.id,
        language: view.language,
        category: view.category,
        title: view.title,
        body: view.body,
        sort: view.sort,
        show: view.visibility.is_visible(),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

pub async fn run() -> Result<()> {
    let update = std::env::var("UPDATE_GOLDENS").is_ok_and(|value| value == "1");
    let goldens_dir = PathBuf::from(env_or("V2BOARD_GOLDENS_DIR", DEFAULT_GOLDENS_DIR));
    ensure!(
        goldens_dir.is_dir(),
        "golden fixtures directory {} is unavailable; run through `make contract-goldens` or \
         `make rust-integration` so frontend/packages/api-client/goldens is mounted",
        goldens_dir.display()
    );

    let root_database_url = env_or(
        "RUST_INTEGRATION_DATABASE_ROOT_URL",
        DEFAULT_ROOT_DATABASE_URL,
    );
    let redis_url = env_or("RUST_INTEGRATION_REDIS_URL", DEFAULT_INTEGRATION_REDIS_URL);

    let database_name = GeneratedDatabaseName::new("goldens")?;
    let database_url = database_url_for(&root_database_url, &database_name)?;
    let root = PgPoolOptions::new()
        .max_connections(2)
        .connect(&root_database_url)
        .await
        .context("connect to the disposable-database administrator")?;
    create_database(&root, &database_name).await?;

    let generated = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("connect to the disposable goldens database")
    {
        Ok(pool) => {
            let result = generate_documents(&pool, &redis_url).await;
            pool.close().await;
            result
        }
        Err(error) => Err(error),
    };

    let drop_result = drop_database(&root, &database_name).await;
    root.close().await;
    let documents = match (generated, drop_result) {
        (Err(error), Err(cleanup)) => {
            return Err(error.context(format!(
                "also failed to drop disposable database {}: {cleanup:#}",
                database_name.as_str()
            )));
        }
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(cleanup)) => return Err(cleanup),
        (Ok(documents), Ok(())) => documents,
    };

    if update {
        write_documents(&goldens_dir, &documents)?;
        println!(
            "Rewrote {} admin golden response fixtures in {}.",
            documents.len(),
            goldens_dir.display()
        );
    } else {
        verify_documents(&goldens_dir, &documents)?;
        println!(
            "All {} admin golden response fixtures match the live serialization.",
            documents.len()
        );
    }
    Ok(())
}

async fn generate_documents(pool: &PgPool, redis_url: &str) -> Result<Vec<(String, String)>> {
    let redis = redis::Client::open(redis_url)?;
    flush_redis(&redis).await?;

    MIGRATOR
        .run(pool)
        .await
        .context("apply every embedded migration to the disposable goldens database")?;
    seed_fixture_rows(pool).await?;

    let config = golden_config(pool, redis_url)?;
    let payments = PaymentService::new(
        PostgresPaymentRepository::new(pool.clone()),
        EncryptedPaymentSecurity::new(config.app_key.clone()),
        config.app_url.clone(),
    );
    let reconciliations = ReconciliationService::new(
        PostgresReconciliationRepository::new(pool.clone()),
        Sha256ReconciliationIdentityHasher,
    );
    let orders = contract_order_service(pool.clone());
    let admin = contract_admin_user_service(pool.clone(), Arc::new(config));
    let servers = contract_server_service(pool.clone());
    let content = ContentService::new(PostgresContentRepository::new(pool.clone()));
    let plans = PlanService::new(PostgresPlanRepository::new(pool.clone()));
    let promotions = PromotionService::new(PostgresPromotionRepository::new(pool.clone()));
    let statistics = StatisticsService::new(
        PostgresStatisticsRepository::new(pool.clone()),
        ContractStatisticsCalendar,
    );

    let mut documents = Vec::new();
    // The W11 admin commerce family (docs/api-dialect.md §6.2/§6.4) serializes
    // modern dialect-v2 bodies straight from the typed domain methods: bare
    // arrays for the deliberately unpaginated plan/payment/provider lists,
    // `{items,total}` for the paginated order and reconciliation lists, and a
    // bare object for the `trade_no`-addressed order detail.
    let commerce_page = Pagination {
        page: 1,
        per_page: 10,
    };
    documents.push((
        "admin.plans.json".to_string(),
        pretty_document(
            &plans
                .plans()
                .await?
                .into_iter()
                .map(admin_plan_document)
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.push((
        "admin.payments.json".to_string(),
        pretty_document(
            &payments
                .payments()
                .await?
                .into_iter()
                .map(admin_payment_document)
                .collect::<Result<Vec<_>>>()?,
        )?,
    ));
    documents.push((
        "admin.payment-providers.json".to_string(),
        pretty_document(&payments.provider_codes())?,
    ));
    let order_page = orders
        .orders(AdminOrderQuery {
            predicates: Vec::new(),
            sort: OrderSort::default(),
            commission_only: false,
            limit: commerce_page.limit(),
            offset: commerce_page.offset(),
        })
        .await?;
    documents.push((
        "admin.orders.json".to_string(),
        pretty_document(&Page {
            items: order_page.items.into_iter().map(admin_order_list).collect(),
            total: order_page.total,
        })?,
    ));
    documents.push((
        "admin.order.detail.json".to_string(),
        pretty_document(&admin_order_detail(
            orders.order("golden-trade-plan-00000000000001").await?,
        ))?,
    ));
    let reconciliation_page = reconciliations
        .reconciliations(
            commerce_page.limit(),
            commerce_page.offset(),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;
    documents.push((
        "admin.payment-reconciliations.json".to_string(),
        pretty_document(&Page {
            items: reconciliation_page
                .items
                .into_iter()
                .map(admin_reconciliation)
                .collect(),
            total: reconciliation_page.total,
        })?,
    ));

    // The W12 admin users family (docs/api-dialect.md §6.6) serializes the
    // modern dialect-v2 body straight from the typed domain methods: the
    // `{items,total}` list plus the two bare detail projections (member id 2
    // carries an inviter, id 1 does not). The projection drops `t` and crosses
    // every epoch as an RFC 3339 string (§4.5).
    let page = admin
        .users(AdminUserListRequest {
            limit: 10,
            offset: 0,
            filters: Vec::new(),
            sort: UserSort::default(),
        })
        .await?;
    documents.push((
        "admin.users.json".to_string(),
        pretty_document(&Page {
            items: page.items.into_iter().map(admin_user_list_item).collect(),
            total: page.total,
        })?,
    ));
    documents.push((
        "admin.user.detail.json".to_string(),
        pretty_document(&admin_user_detail(admin.user_detail(2).await?))?,
    ));
    documents.push((
        "admin.user.detail.no-inviter.json".to_string(),
        pretty_document(&admin_user_detail(admin.user_detail(1).await?))?,
    ));

    // The W10 content family (docs/api-dialect.md §6.3) serializes modern
    // dialect-v2 bodies straight from typed application methods: bare arrays
    // for the deliberately unpaginated notice/knowledge lists, `{items,total}`
    // for the paginated coupon/gift-card lists, bare objects for details.
    let content_page = Pagination {
        page: 1,
        per_page: 10,
    };
    documents.push((
        "admin.notices.json".to_string(),
        pretty_document(
            &content
                .notices()
                .await?
                .into_iter()
                .map(notice_document)
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.push((
        "admin.knowledge.json".to_string(),
        pretty_document(
            &content
                .knowledge()
                .await?
                .into_iter()
                .map(knowledge_summary_document)
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.push((
        "admin.knowledge.detail.json".to_string(),
        pretty_document(&knowledge_detail_document(
            content.knowledge_detail(1).await?,
        ))?,
    ));
    documents.push((
        "admin.knowledge-categories.json".to_string(),
        pretty_document(&content.knowledge_categories().await?)?,
    ));
    let page = promotions
        .coupons(content_page.limit(), content_page.offset(), None, None)
        .await?;
    documents.push((
        "admin.coupons.json".to_string(),
        pretty_document(&Page {
            items: page.items.into_iter().map(admin_coupon_document).collect(),
            total: page.total,
        })?,
    ));
    let page = promotions
        .gift_cards(content_page.limit(), content_page.offset(), None, None)
        .await?;
    documents.push((
        "admin.gift-cards.json".to_string(),
        pretty_document(&Page {
            items: page
                .items
                .into_iter()
                .map(admin_giftcard_document)
                .collect(),
            total: page.total,
        })?,
    ));

    // The W13 admin servers family (docs/api-dialect.md §6.7) serializes
    // modern dialect-v2 bodies straight from the typed domain methods: bare
    // arrays for the node, group, and route lists. The node rows pin the
    // dialect-v2 projection (`show` bool, `rate` number, RFC 3339 timestamps)
    // with the vmess camelCase settings keys kept as-is (R22).
    documents.push((
        "admin.nodes.json".to_string(),
        pretty_document(
            &servers
                .nodes(GOLDEN_TIME)
                .await?
                .into_iter()
                .map(server_node_view)
                .collect::<Result<Vec<_>>>()?,
        )?,
    ));
    documents.push((
        "admin.server-groups.json".to_string(),
        pretty_document(
            &servers
                .groups(None)
                .await?
                .into_iter()
                .map(server_group_view)
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.push((
        "admin.server-routes.json".to_string(),
        pretty_document(
            &servers
                .routes()
                .await?
                .into_iter()
                .map(server_route_view)
                .collect::<Vec<_>>(),
        )?,
    ));

    // The W14 admin tickets + stats families (docs/api-dialect.md §6.5/§6.8)
    // serialize modern dialect-v2 bodies straight from the typed domain
    // methods: `{items,total}` for the ticket list and per-user traffic page,
    // a bare ticket detail with its `message[]` thread, and bare
    // `{series,date,value}` arrays with snake_case series slugs and
    // integer-cent money. `local_month_day` uses the fixed +08:00 app
    // timezone, so the seeded `stat` rows render byte-stable dates.
    let ticket_repository = PostgresTicketRepository::new(pool.clone());
    let ticket_page = ticket_repository
        .list_for_operator(OperatorTicketListQuery {
            limit: 10,
            offset: 0,
            status: None,
            reply_statuses: Vec::new(),
            email: None,
            order: OperatorTicketOrder::UpdatedAt,
        })
        .await?;
    documents.push((
        "admin.tickets.json".to_string(),
        pretty_document(&Page {
            items: ticket_page
                .items
                .into_iter()
                .map(admin_ticket_item)
                .collect(),
            total: ticket_page.total,
        })?,
    ));
    documents.push((
        "admin.ticket.detail.json".to_string(),
        pretty_document(&admin_ticket_detail(
            ticket_repository
                .find_for_operator(1)
                .await?
                .context("seeded golden ticket missing")?,
        ))?,
    ));
    let pagination = Pagination {
        page: 1,
        per_page: 10,
    };
    let (items, total) = statistics
        .user_traffic(2, pagination.limit(), pagination.offset())
        .await?;
    let items = items
        .into_iter()
        .map(|item| AdminUserTrafficView {
            record_at: Rfc3339Timestamp::from_epoch_seconds(item.recorded_at),
            u: item.upload,
            d: item.download,
            server_rate: item.server_rate,
        })
        .collect();
    documents.push((
        "admin.stats.user-traffic.json".to_string(),
        pretty_document(&Page { items, total })?,
    ));
    documents.push((
        "admin.stats.orders.json".to_string(),
        pretty_document(
            &statistics
                .series(StatisticsBucket::Daily)
                .await?
                .into_iter()
                .map(|point| StatSeriesPointView {
                    series: point.series.to_string(),
                    date: point.date,
                    value: point.value,
                })
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.push((
        "admin.stats.records.json".to_string(),
        pretty_document(
            &statistics
                .series(StatisticsBucket::Monthly)
                .await?
                .into_iter()
                .map(|point| StatSeriesPointView {
                    series: point.series.to_string(),
                    date: point.date,
                    value: point.value,
                })
                .collect::<Vec<_>>(),
        )?,
    ));
    documents.sort_by(|left, right| left.0.cmp(&right.0));

    flush_redis(&redis).await?;
    Ok(documents)
}

fn pretty_document<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

/// Deterministic service configuration: identical to the invariants gate's
/// testing config, with every field that reaches a pinned response body set
/// to a fixed literal instead of an environment-derived value.
fn golden_config(pool: &PgPool, redis_url: &str) -> Result<AppConfig> {
    let mut config = integration_config(pool, redis_url)?;
    config.app_name = "V2Board".to_string();
    config.app_url = Some("https://golden.v2board.test".to_string());
    config.subscribe_url = None;
    config.subscribe_path = String::new();
    config.show_subscribe_method = 0;
    config.server_api_url = None;
    config.server_token = None;
    Ok(config)
}

async fn seed_fixture_rows(pool: &PgPool) -> Result<()> {
    // The immutable installation identity every service constructor reads.
    sqlx::query(
        "INSERT INTO system_installation (singleton, installation_id, created_at) \
         VALUES (1, $1::uuid, $2)",
    )
    .bind(GOLDEN_INSTALLATION_ID)
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Server group (id 1): the FK anchor for the plan and the node below.
    sqlx::query(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ('Golden group', $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // One visible plan (id 1) referenced by the member and the plan order.
    sqlx::query(
        "INSERT INTO plan (group_id, transfer_enable, device_limit, name, speed_limit, \"show\", \
         sort, renew, content, reset_traffic_method, \
         capacity_limit, created_at, updated_at) \
         VALUES (1, 100, 3, 'Golden Plan', NULL, TRUE, 1, TRUE, \
         'golden plan content', 0, 50, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO plan_price (plan_id, period, amount_minor) VALUES \
         (1, 'month', 1000), (1, 'quarter', 2700), (1, 'year', 9600), \
         (1, 'one_time', 15000), (1, 'reset', 300)",
    )
    .execute(pool)
    .await?;

    // Users: id 1 is the inviter-less inviter, id 2 the invited member every
    // per-user endpoint targets.
    sqlx::query(
        "INSERT INTO users (email, password, uuid, token, u, d, transfer_enable, balance, \
         commission_balance, created_at, updated_at) \
         VALUES ('golden-inviter@example.test', 'golden-password-hash', \
         '00000000-0000-4000-8000-000000000001', 'goldeninvitertoken00000000000001', \
         0, 0, 0, 0, 0, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO users (email, password, uuid, token, invite_user_id, u, d, \
         transfer_enable, balance, commission_balance, plan_id, expired_at, created_at, updated_at) \
         VALUES ('golden-member@example.test', 'golden-password-hash', \
         '00000000-0000-4000-8000-000000000002', 'goldenmembertoken000000000000002', \
         1, 1073741824, 2147483648, 107374182400, 1000, 500, 1, $1, $2, $2)",
    )
    .bind(GOLDEN_TIME + 86_400 * 30)
    .bind(GOLDEN_TIME + 5)
    .execute(pool)
    .await?;

    // Per-user traffic history for stat/getStatUser.
    sqlx::query(
        "INSERT INTO user_traffic (user_id, server_rate, u, d, record_type, record_at, \
         created_at, updated_at) VALUES (2, 1.00, 100, 200, 'd', $1, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Aggregated stat periods for the W14 §6.8 series fixtures: two daily
    // rows (stats/orders reads the newest 31 `d` periods) and one monthly row
    // (stats/records `?type=m`). Fixed epochs render byte-stable dates
    // through the +08:00 app timezone.
    sqlx::query(
        "INSERT INTO stat (record_at, record_type, order_count, order_total, commission_count, \
         commission_total, paid_count, paid_total, register_count, invite_count, \
         transfer_used_total, created_at, updated_at) VALUES \
         ($1, 'd', 3, 3000, 1, 100, 2, 1500, 4, 1, '1073741824', $1, $1), \
         ($2, 'd', 5, 6000, 2, 300, 3, 2500, 6, 2, '2147483648', $2, $2), \
         ($3, 'm', 30, 60000, 6, 900, 20, 45000, 40, 8, '32212254720', $3, $3)",
    )
    .bind(GOLDEN_TIME - 86_400)
    .bind(GOLDEN_TIME)
    .bind(GOLDEN_TIME - 86_400 * 30)
    .execute(pool)
    .await?;

    // Knowledge article (id 1) for the list, detail, and category projections.
    sqlx::query(
        "INSERT INTO knowledge (language, category, title, body, sort, \"show\", created_at, updated_at) \
         VALUES ('en-US', 'Golden Category', 'Golden article', 'golden knowledge body', 1, 1, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Ticket (id 1) with a single member message.
    sqlx::query(
        "INSERT INTO ticket (user_id, subject, level, status, reply_status, created_at, updated_at) \
         VALUES (2, 'Golden ticket subject', 1, 0, 0, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO ticket_message (user_id, ticket_id, message, created_at, updated_at) \
         VALUES (2, 1, 'golden ticket message', $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Coupons: a minimal row and one exercising the JSONB limit arrays.
    sqlx::query(
        "INSERT INTO coupon (code, name, type, value, \"show\", started_at, ended_at, created_at, updated_at) \
         VALUES ('GOLDENCOUPONA', 'Golden coupon plain', 1, 100, 1, $1, $2, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .bind(GOLDEN_TIME + 3_600)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO coupon (code, name, type, value, \"show\", limit_use, limit_use_with_user, \
         limit_plan_ids, limit_period, started_at, ended_at, created_at, updated_at) \
         VALUES ('GOLDENCOUPONB', 'Golden coupon limited', 2, 20, 1, 10, 1, \
         '[1]'::jsonb, '[\"month_price\"]'::jsonb, $1, $2, $3, $3)",
    )
    .bind(GOLDEN_TIME)
    .bind(GOLDEN_TIME + 3_600)
    .bind(GOLDEN_TIME + 10)
    .execute(pool)
    .await?;

    // Gift card.
    sqlx::query(
        "INSERT INTO gift_card (code, name, type, value, started_at, ended_at, created_at, updated_at) \
         VALUES ('GOLDENGIFT0001', 'Golden gift card', 1, 100, $1, $2, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .bind(GOLDEN_TIME + 3_600)
    .execute(pool)
    .await?;

    // Orders: a pending plan order (id 1, with a commission log) and a
    // deposit order (id 2). Distinct created_at keeps list ordering stable.
    sqlx::query(
        "INSERT INTO orders (user_id, plan_id, invite_user_id, type, period, trade_no, \
         total_amount, status, commission_status, commission_balance, created_at, updated_at) \
         VALUES (2, 1, 1, 1, 'month_price', 'golden-trade-plan-00000000000001', \
         1000, 0, 0, 100, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO orders (user_id, plan_id, type, period, trade_no, total_amount, status, \
         commission_status, commission_balance, paid_at, created_at, updated_at) \
         VALUES (2, 0, 1, 'deposit', 'golden-trade-deposit-00000000002', 500, 3, 0, 0, $1, $1, $1)",
    )
    .bind(GOLDEN_TIME + 10)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO commission_log (invite_user_id, user_id, trade_no, order_amount, get_amount, \
         created_at, updated_at) \
         VALUES (1, 2, 'golden-trade-plan-00000000000001', 1000, 100, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Payment methods: the enabled EPay row pins handling_fee_percent as a
    // JSON number (vs the user endpoint's decimal string), the second row
    // pins the NULL percent and notify_domain branches.
    sqlx::query(
        "INSERT INTO payment_method (uuid, payment, name, icon, config, handling_fee_fixed, \
         handling_fee_percent, enable, sort, created_at, updated_at) \
         VALUES ('goldenepayuuid000000000000000001', 'EPay', 'Golden EPay', NULL, \
         $1, 20, 0.50, 1, 1, $2, $2)",
    )
    .bind(
        crate::production_invariants::encrypt_payment_fixture_config(
            "EPay",
            "goldenepayuuid000000000000000001",
            &json!({ "key": "golden-epay-key", "pid": "1000", "url": "https://epay.golden.test" }),
        )?,
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO payment_method (uuid, payment, name, icon, config, notify_domain, \
         handling_fee_fixed, handling_fee_percent, enable, sort, created_at, updated_at) \
         VALUES ('goldenepayuuid000000000000000002', 'EPay', 'Golden EPay disabled', NULL, \
         $1, 'https://notify.golden.test', NULL, NULL, 0, 2, $2, $2)",
    )
    .bind(
        crate::production_invariants::encrypt_payment_fixture_config(
            "EPay",
            "goldenepayuuid000000000000000002",
            &json!({ "key": "golden-epay-key-2", "pid": "2000", "url": "https://epay2.golden.test" }),
        )?,
    )
    .bind(GOLDEN_TIME + 10)
    .execute(pool)
    .await?;

    // One open payment reconciliation bound to the plan order, so the W11
    // `GET payment-reconciliations` fixture pins a non-empty ledger with the
    // server-side `trade_no`/`callback_no` hashes.
    sqlx::query(
        "INSERT INTO payment_reconciliation (payment_id, provider, trade_no, trade_no_hash, \
         callback_no, callback_no_hash, reason, order_status, expected_amount, settled_amount, \
         occurrence_count, first_seen_at, last_seen_at, resolved_at, resolution) \
         VALUES (1, 'EPay', 'golden-trade-plan-00000000000001', \
         sha256('golden-trade-plan-00000000000001'::bytea), 'golden-callback-00000000000001', \
         sha256('golden-callback-00000000000001'::bytea), 'amount_mismatch', 0, 1000, 900, \
         1, $1, $1, NULL, NULL)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Notice with the tag array shape the dashboard popup contract reads.
    sqlx::query(
        "INSERT INTO notice (title, content, \"show\", img_url, tags, created_at, updated_at) \
         VALUES ('Golden notice', 'golden notice content', 1, NULL, \
         '[\"golden\",\"popup\"]'::jsonb, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    // Server route and one shadowsocks node in the golden group.
    sqlx::query(
        "INSERT INTO server_route (remarks, match, action, action_value, created_at, updated_at) \
         VALUES ('Golden route', '[\"*\"]'::jsonb, 'block', NULL, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO server_shadowsocks (group_id, name, rate, host, port, server_port, cipher, \
         \"show\", sort, created_at, updated_at) \
         VALUES ('[1]'::jsonb, 'Golden shadowsocks node', '1', 'ss.golden.test', '443', 443, \
         'aes-128-gcm', 1, 1, $1, $1)",
    )
    .bind(GOLDEN_TIME)
    .execute(pool)
    .await?;

    Ok(())
}

fn write_documents(goldens_dir: &Path, documents: &[(String, String)]) -> Result<()> {
    for (file_name, body) in documents {
        std::fs::write(goldens_dir.join(file_name), body)
            .with_context(|| format!("write golden fixture {file_name}"))?;
    }
    for stale in stale_admin_files(goldens_dir, documents)? {
        std::fs::remove_file(goldens_dir.join(&stale))
            .with_context(|| format!("remove stale golden fixture {stale}"))?;
    }
    Ok(())
}

fn verify_documents(goldens_dir: &Path, documents: &[(String, String)]) -> Result<()> {
    let mut failures = Vec::new();
    for (file_name, expected) in documents {
        let path = goldens_dir.join(file_name);
        match std::fs::read_to_string(&path) {
            Ok(actual) if &actual == expected => {}
            Ok(_) => failures.push(format!("{file_name}: content drifted")),
            Err(_) => failures.push(format!("{file_name}: fixture is missing")),
        }
    }
    for stale in stale_admin_files(goldens_dir, documents)? {
        failures.push(format!("{stale}: fixture has no generating admin endpoint"));
    }
    ensure!(
        failures.is_empty(),
        "admin golden response fixtures drifted from the live serialization; \
         regenerate with `make contract-goldens` and review the diff:\n  {}",
        failures.join("\n  ")
    );
    Ok(())
}

fn stale_admin_files(goldens_dir: &Path, documents: &[(String, String)]) -> Result<Vec<String>> {
    let expected: std::collections::BTreeSet<&str> =
        documents.iter().map(|(name, _)| name.as_str()).collect();
    let mut stale = Vec::new();
    for entry in std::fs::read_dir(goldens_dir).context("list the goldens directory")? {
        let file_name = entry?.file_name();
        let Some(name) = file_name.to_str() else {
            bail!("golden fixture with a non-UTF-8 file name");
        };
        if name.starts_with(ADMIN_GOLDEN_PREFIX) && !expected.contains(name) {
            stale.push(name.to_string());
        }
    }
    stale.sort();
    Ok(stale)
}

use sqlx::PgPool;
use v2board_api_contract::{
    admin_business::{
        AdminCommissionLogItem, AdminOrderDetail, AdminOrderFields, AdminOrderListItem,
        AdminPaymentReconciliationItem,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::admin_order::{
    AdminCommissionLog, AdminOrder, AdminOrderDetail as ApplicationOrderDetail,
    AdminOrderLifecycle, AdminOrderLifecycleError, AdminOrderListItem as ApplicationOrderListItem,
    AdminOrderReconciliation, AdminOrderService, AssignOrderPolicy, OrderNumberGenerator,
};
use v2board_db::admin_order::PostgresAdminOrderRepository;

pub(crate) fn contract_order_service(
    pool: PgPool,
) -> AdminOrderService<
    PostgresAdminOrderRepository,
    ReadOnlyOrderLifecycle,
    UnusedOrderNumberGenerator,
> {
    AdminOrderService::new(
        PostgresAdminOrderRepository::new(pool),
        ReadOnlyOrderLifecycle,
        UnusedOrderNumberGenerator,
        AssignOrderPolicy {
            default_commission_rate: 0,
            commission_first_time_enable: false,
        },
    )
}

pub(crate) fn admin_order_fields(view: AdminOrder) -> AdminOrderFields {
    AdminOrderFields {
        id: view.id,
        invite_user_id: view.invite_user_id,
        user_id: view.user_id,
        plan_id: view.plan_id,
        coupon_id: view.coupon_id,
        r#type: view.kind,
        period: view.period,
        trade_no: view.trade_no,
        callback_no: view.callback_no,
        total_amount: view.total_amount,
        handling_amount: view.handling_amount,
        discount_amount: view.discount_amount,
        surplus_amount: view.surplus_amount,
        refund_amount: view.refund_amount,
        balance_amount: view.balance_amount,
        surplus_order_ids: view.surplus_order_ids,
        status: view.status,
        commission_status: view.commission_status,
        commission_balance: view.commission_balance,
        actual_commission_balance: view.actual_commission_balance,
        payment_id: view.payment_id,
        paid_at: view.paid_at.map(Rfc3339Timestamp::from_epoch_seconds),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

pub(crate) fn admin_order_list(view: ApplicationOrderListItem) -> AdminOrderListItem {
    AdminOrderListItem {
        order: admin_order_fields(view.order),
        email: view.email,
        plan_name: view.plan_name,
        payment_reconciliation_open_count: view.open_reconciliation_count,
    }
}

fn commission_log(view: AdminCommissionLog) -> AdminCommissionLogItem {
    AdminCommissionLogItem {
        id: view.id,
        invite_user_id: view.invite_user_id,
        user_id: view.user_id,
        trade_no: view.trade_no,
        order_amount: view.order_amount,
        get_amount: view.get_amount,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn reconciliation(view: AdminOrderReconciliation) -> AdminPaymentReconciliationItem {
    AdminPaymentReconciliationItem {
        id: view.id,
        payment_id: view.payment_id,
        provider: view.provider,
        trade_no: view.trade_no,
        trade_no_hash: view.trade_no_hash,
        callback_no: view.callback_no,
        callback_no_hash: view.callback_no_hash,
        reason: view.reason,
        order_status: view.order_status,
        expected_amount: view.expected_amount,
        settled_amount: view.settled_amount,
        occurrence_count: view.occurrence_count,
        first_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.first_seen_at),
        last_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.last_seen_at),
        resolved_at: view.resolved_at.map(Rfc3339Timestamp::from_epoch_seconds),
        resolution: view.resolution,
    }
}

pub(crate) fn admin_order_detail(view: ApplicationOrderDetail) -> AdminOrderDetail {
    AdminOrderDetail {
        order: admin_order_fields(view.order),
        commission_log: view
            .commission_log
            .into_iter()
            .map(commission_log)
            .collect(),
        payment_reconciliations: view
            .payment_reconciliations
            .into_iter()
            .map(reconciliation)
            .collect(),
        surplus_orders: view
            .surplus_orders
            .map(|orders| orders.into_iter().map(admin_order_fields).collect()),
    }
}

pub(crate) struct ReadOnlyOrderLifecycle;

impl AdminOrderLifecycle for ReadOnlyOrderLifecycle {
    async fn mark_paid(&self, _trade_no: &str) -> Result<(), AdminOrderLifecycleError> {
        Err(AdminOrderLifecycleError::new(
            "contract read service",
            "write lifecycle is unavailable",
        ))
    }

    async fn cancel_payment_binding(
        &self,
        _payment_id: Option<i32>,
        _callback_no: Option<&str>,
    ) -> Result<bool, AdminOrderLifecycleError> {
        Err(AdminOrderLifecycleError::new(
            "contract read service",
            "write lifecycle is unavailable",
        ))
    }
}

pub(crate) struct UnusedOrderNumberGenerator;

impl OrderNumberGenerator for UnusedOrderNumberGenerator {
    fn generate(&self) -> String {
        panic!("contract read service cannot generate orders")
    }
}

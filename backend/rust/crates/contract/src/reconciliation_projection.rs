use v2board_api_contract::{
    admin_business::{AdminPaymentReconciliationItem, AdminPaymentReconciliationListItem},
    time::Rfc3339Timestamp,
};
use v2board_application::reconciliation::PaymentReconciliation;

pub(crate) fn admin_reconciliation(
    view: PaymentReconciliation,
) -> AdminPaymentReconciliationListItem {
    AdminPaymentReconciliationListItem {
        reconciliation: AdminPaymentReconciliationItem {
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
        },
        payment_name: view.payment_name,
        payment_archived_at: view
            .payment_archived_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
    }
}

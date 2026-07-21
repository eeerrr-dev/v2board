use std::sync::Arc;

use v2board_application::admin_order::{
    AdminOrderLifecycle, AdminOrderLifecycleError, AdminOrderLifecycleFailure, OrderNumberGenerator,
};
use v2board_application::order::{OrderError, OrderFailure};
use v2board_config::AppConfig;
use v2board_db::DbPool;
use v2board_order_adapters::{
    RuntimeOrderService, TimestampOrderNumberGenerator, runtime_order_service,
};

pub(crate) struct RuntimeAdminOrderLifecycle {
    service: RuntimeOrderService,
}

impl RuntimeAdminOrderLifecycle {
    pub(crate) fn new(db: DbPool, config: Arc<AppConfig>) -> Self {
        Self {
            service: runtime_order_service(db, config),
        }
    }
}

impl AdminOrderLifecycle for RuntimeAdminOrderLifecycle {
    async fn mark_paid(&self, trade_no: &str) -> Result<(), AdminOrderLifecycleError> {
        self.service
            .paid_manually(trade_no)
            .await
            .map_err(|error| lifecycle_error("mark paid", error))
    }

    async fn cancel_payment_binding(
        &self,
        payment_id: Option<i32>,
        callback_no: Option<&str>,
    ) -> Result<bool, AdminOrderLifecycleError> {
        self.service
            .cancel_payment_binding(payment_id, callback_no)
            .await
            .map_err(|error| lifecycle_error("cancel provider payment", error))
    }
}

fn lifecycle_error(operation: &'static str, error: OrderError) -> AdminOrderLifecycleError {
    let failure = match error.failure() {
        Some(OrderFailure::OrderNotFound) => AdminOrderLifecycleFailure::NotFound,
        Some(OrderFailure::OrderNotPending) => AdminOrderLifecycleFailure::NotPending,
        Some(OrderFailure::OrderUpdateFailed) => AdminOrderLifecycleFailure::UpdateFailed,
        _ => AdminOrderLifecycleFailure::Other,
    };
    AdminOrderLifecycleError::classified(failure, operation, error)
}

pub(crate) struct RuntimeOrderNumberGenerator;

impl OrderNumberGenerator for RuntimeOrderNumberGenerator {
    fn generate(&self) -> String {
        TimestampOrderNumberGenerator::generate()
    }
}

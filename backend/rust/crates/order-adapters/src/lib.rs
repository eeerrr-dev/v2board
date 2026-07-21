use std::sync::Arc;

use chrono::{FixedOffset, Months, TimeZone, Utc};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use uuid::Uuid;
use v2board_application::order::{
    CreateOrderPolicy, FulfillmentPolicy, OrderClock, OrderNumberGenerator, OrderPolicy,
    OrderService,
};
use v2board_application::order_jobs::{
    CommissionRun, CommissionService, RenewalCalendar, RenewalRun, RenewalService,
};
use v2board_config::{AppConfig, app_timezone};
use v2board_db::{DbPool, PostgresOrderJobsRepository, PostgresOrderRepository};
use v2board_payment_adapters::RuntimePaymentGateway;

pub type RuntimeOrderService = OrderService<
    PostgresOrderRepository<RuntimePaymentGateway>,
    RuntimePaymentGateway,
    SystemOrderClock,
    TimestampOrderNumberGenerator,
    ConfiguredOrderPolicy,
>;

pub fn runtime_order_service(db: DbPool, config: Arc<AppConfig>) -> RuntimeOrderService {
    let gateway = RuntimePaymentGateway::new(config.clone());
    OrderService::new(
        PostgresOrderRepository::new(db, gateway.clone()),
        gateway,
        SystemOrderClock,
        TimestampOrderNumberGenerator,
        ConfiguredOrderPolicy::new(config),
    )
}

pub type RuntimeCommissionService = CommissionService<PostgresOrderJobsRepository>;

pub fn runtime_commission_service(db: DbPool) -> RuntimeCommissionService {
    CommissionService::new(PostgresOrderJobsRepository::new(db))
}

pub type RuntimeRenewalService = RenewalService<
    PostgresOrderJobsRepository,
    AppTimezoneRenewalCalendar,
    TimestampOrderNumberGenerator,
>;

pub fn runtime_renewal_service(db: DbPool) -> RuntimeRenewalService {
    RenewalService::new(
        PostgresOrderJobsRepository::new(db),
        AppTimezoneRenewalCalendar,
        TimestampOrderNumberGenerator,
    )
}

pub fn commission_run(config: &AppConfig, now: i64) -> CommissionRun {
    const AUTO_CHECK_DELAY_SECONDS: i64 = 3 * 86_400;
    CommissionRun {
        now,
        auto_check_cutoff: config
            .commission_auto_check_enable
            .then_some(now.saturating_sub(AUTO_CHECK_DELAY_SECONDS)),
        auto_check_batch_size: 1_000,
        auto_check_max_batches: 20,
        max_payouts: 10_000,
        shares: commission_shares(config),
        credit_account_balance: config.withdraw_close_enable,
    }
}

pub const fn renewal_run(now: i64) -> RenewalRun {
    RenewalRun {
        now,
        renewal_before: now.saturating_add(2 * 86_400),
        candidate_page_size: 250,
    }
}

fn commission_shares(config: &AppConfig) -> Vec<i32> {
    if !config.commission_distribution_enable {
        return vec![100];
    }
    vec![
        parse_share(config.commission_distribution_l1.as_deref()),
        parse_share(config.commission_distribution_l2.as_deref()),
        parse_share(config.commission_distribution_l3.as_deref()),
    ]
}

fn parse_share(value: Option<&str>) -> i32 {
    value
        .map(str::trim)
        .and_then(|value| value.parse::<Decimal>().ok())
        .and_then(|value| value.trunc().to_i32())
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AppTimezoneRenewalCalendar;

impl RenewalCalendar for AppTimezoneRenewalCalendar {
    fn add_months(&self, timestamp: i64, months: u32) -> Option<i64> {
        app_timezone()
            .timestamp_opt(timestamp, 0)
            .single()?
            .checked_add_months(Months::new(months))
            .map(|date| date.timestamp())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemOrderClock;

impl OrderClock for SystemOrderClock {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TimestampOrderNumberGenerator;

impl TimestampOrderNumberGenerator {
    pub fn generate() -> String {
        let timezone =
            FixedOffset::east_opt(8 * 3_600).expect("the pinned application offset is valid");
        let now = Utc::now().with_timezone(&timezone);
        let bytes = *Uuid::new_v4().as_bytes();
        let random =
            10_000 + (u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 90_000);
        format!(
            "{}{:06}{}",
            now.format("%Y%m%d%H%M%S"),
            now.timestamp_subsec_micros(),
            random
        )
    }
}

impl OrderNumberGenerator for TimestampOrderNumberGenerator {
    fn generate(&self) -> String {
        Self::generate()
    }
}

#[derive(Clone)]
pub struct ConfiguredOrderPolicy {
    config: Arc<AppConfig>,
}

impl ConfiguredOrderPolicy {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

impl OrderPolicy for ConfiguredOrderPolicy {
    fn create_policy(&self) -> CreateOrderPolicy {
        CreateOrderPolicy {
            plan_change_enabled: self.config.plan_change_enable,
            surplus_enabled: self.config.surplus_enable,
            commission_first_time_enabled: self.config.commission_first_time_enable,
            default_commission_rate: self.config.invite_commission,
        }
    }

    fn fulfillment_policy(&self, total_amount: i32) -> FulfillmentPolicy {
        FulfillmentPolicy {
            deposit_bonus: self.config.deposit_bonus(total_amount),
            new_order_event_id: self.config.new_order_event_id,
            renewal_order_event_id: self.config.renew_order_event_id,
            change_order_event_id: self.config.change_order_event_id,
        }
    }

    fn try_out_plan_id(&self) -> i32 {
        self.config.try_out_plan_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_order_numbers_have_the_legacy_sortable_shape() {
        let value = TimestampOrderNumberGenerator::generate();
        assert_eq!(value.len(), 25);
        assert!(value.bytes().all(|byte| byte.is_ascii_digit()));
    }

    #[test]
    fn renewal_window_uses_saturating_timestamp_arithmetic() {
        assert_eq!(renewal_run(i64::MAX).renewal_before, i64::MAX);
    }
}

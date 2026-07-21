use std::collections::BTreeSet;

use crate::MoneyMinor;

pub const PLAN_FORCE_UPDATE_MAX_USERS: usize = 10_000;
const GIB_BYTES: i64 = 1_073_741_824;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanInputViolation {
    EmptyName,
    TransferEnableOutOfRange,
    DeviceLimitOutOfRange,
    SpeedLimitOutOfRange,
    CapacityLimitOutOfRange,
    InvalidResetTrafficMethod,
    SortIdOutOfRange,
    DuplicateSortId,
}

pub fn validate_plan_name(name: &str) -> Result<(), PlanInputViolation> {
    if name.trim().is_empty() {
        Err(PlanInputViolation::EmptyName)
    } else {
        Ok(())
    }
}

fn validate_nonnegative_i32(
    value: i64,
    violation: PlanInputViolation,
) -> Result<(), PlanInputViolation> {
    if (0..=i64::from(i32::MAX)).contains(&value) {
        Ok(())
    } else {
        Err(violation)
    }
}

pub fn validate_plan_transfer_enable(value: i64) -> Result<(), PlanInputViolation> {
    validate_nonnegative_i32(value, PlanInputViolation::TransferEnableOutOfRange)
}

pub fn validate_plan_device_limit(value: Option<i64>) -> Result<(), PlanInputViolation> {
    value.map_or(Ok(()), |value| {
        validate_nonnegative_i32(value, PlanInputViolation::DeviceLimitOutOfRange)
    })
}

pub fn validate_plan_speed_limit(value: Option<i64>) -> Result<(), PlanInputViolation> {
    value.map_or(Ok(()), |value| {
        validate_nonnegative_i32(value, PlanInputViolation::SpeedLimitOutOfRange)
    })
}

pub fn validate_plan_capacity_limit(value: Option<i64>) -> Result<(), PlanInputViolation> {
    value.map_or(Ok(()), |value| {
        validate_nonnegative_i32(value, PlanInputViolation::CapacityLimitOutOfRange)
    })
}

pub fn validate_plan_reset_traffic_method(value: Option<i64>) -> Result<(), PlanInputViolation> {
    if value.is_none_or(|value| (0..=4).contains(&value)) {
        Ok(())
    } else {
        Err(PlanInputViolation::InvalidResetTrafficMethod)
    }
}

pub fn plan_transfer_bytes(gib: i64) -> Result<i64, PlanInputViolation> {
    validate_plan_transfer_enable(gib)?;
    gib.checked_mul(GIB_BYTES)
        .ok_or(PlanInputViolation::TransferEnableOutOfRange)
}

pub fn normalize_plan_sort_ids(ids: &[i64]) -> Result<Vec<i32>, PlanInputViolation> {
    let mut unique = BTreeSet::new();
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let id = i32::try_from(*id)
            .ok()
            .filter(|id| *id > 0)
            .ok_or(PlanInputViolation::SortIdOutOfRange)?;
        if !unique.insert(id) {
            return Err(PlanInputViolation::DuplicateSortId);
        }
        normalized.push(id);
    }
    Ok(normalized)
}

/// A purchasable or account-reset period for a subscription plan.
///
/// The enum is the canonical business vocabulary. Legacy wire/source names and
/// native database labels are adapter representations and deliberately do not
/// live in this type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlanPricePeriod {
    Month,
    Quarter,
    HalfYear,
    Year,
    TwoYear,
    ThreeYear,
    OneTime,
    Reset,
}

impl PlanPricePeriod {
    pub const ALL: [Self; 8] = [
        Self::Month,
        Self::Quarter,
        Self::HalfYear,
        Self::Year,
        Self::TwoYear,
        Self::ThreeYear,
        Self::OneTime,
        Self::Reset,
    ];

    pub const fn recurring_months(self) -> Option<u32> {
        match self {
            Self::Month => Some(1),
            Self::Quarter => Some(3),
            Self::HalfYear => Some(6),
            Self::Year => Some(12),
            Self::TwoYear => Some(24),
            Self::ThreeYear => Some(36),
            Self::OneTime | Self::Reset => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Month => 0,
            Self::Quarter => 1,
            Self::HalfYear => 2,
            Self::Year => 3,
            Self::TwoYear => 4,
            Self::ThreeYear => 5,
            Self::OneTime => 6,
            Self::Reset => 7,
        }
    }
}

/// The complete native price collection for a plan.
///
/// The fixed-size representation makes the eight supported business periods
/// exhaustive without exposing legacy column names to application code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanPrices([Option<MoneyMinor>; PlanPricePeriod::ALL.len()]);

impl PlanPrices {
    pub const EMPTY: Self = Self([None; PlanPricePeriod::ALL.len()]);

    pub const fn get(&self, period: PlanPricePeriod) -> Option<MoneyMinor> {
        self.0[period.index()]
    }

    pub fn set(&mut self, period: PlanPricePeriod, amount: Option<MoneyMinor>) {
        self.0[period.index()] = amount;
    }

    pub fn iter(&self) -> impl Iterator<Item = (PlanPricePeriod, Option<MoneyMinor>)> + '_ {
        PlanPricePeriod::ALL
            .into_iter()
            .map(|period| (period, self.get(period)))
    }
}

impl Default for PlanPrices {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// A single PATCH operation over a price entry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlanPriceUpdate {
    #[default]
    Retain,
    Clear,
    Set(MoneyMinor),
}

/// PATCH operations for every supported business period.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanPriceUpdates([PlanPriceUpdate; PlanPricePeriod::ALL.len()]);

impl PlanPriceUpdates {
    pub const RETAIN_ALL: Self = Self([PlanPriceUpdate::Retain; PlanPricePeriod::ALL.len()]);

    pub const fn get(&self, period: PlanPricePeriod) -> PlanPriceUpdate {
        self.0[period.index()]
    }

    pub fn set(&mut self, period: PlanPricePeriod, update: PlanPriceUpdate) {
        self.0[period.index()] = update;
    }

    pub fn iter(&self) -> impl Iterator<Item = (PlanPricePeriod, PlanPriceUpdate)> + '_ {
        PlanPricePeriod::ALL
            .into_iter()
            .map(|period| (period, self.get(period)))
    }
}

impl Default for PlanPriceUpdates {
    fn default() -> Self {
        Self::RETAIN_ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_business_period_is_unique() {
        assert_eq!(
            PlanPricePeriod::ALL
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            8
        );
    }

    #[test]
    fn only_recurring_periods_have_month_durations() {
        assert_eq!(PlanPricePeriod::Month.recurring_months(), Some(1));
        assert_eq!(PlanPricePeriod::ThreeYear.recurring_months(), Some(36));
        assert_eq!(PlanPricePeriod::OneTime.recurring_months(), None);
        assert_eq!(PlanPricePeriod::Reset.recurring_months(), None);
    }

    #[test]
    fn price_collections_are_period_keyed_and_patch_safe() {
        let mut prices = PlanPrices::default();
        prices.set(PlanPricePeriod::Month, Some(MoneyMinor::from_i32(1_999)));
        assert_eq!(prices.get(PlanPricePeriod::Month).unwrap().get(), 1_999);
        assert_eq!(prices.get(PlanPricePeriod::Year), None);
        assert_eq!(prices.iter().count(), PlanPricePeriod::ALL.len());

        let mut updates = PlanPriceUpdates::default();
        updates.set(PlanPricePeriod::Month, PlanPriceUpdate::Clear);
        updates.set(
            PlanPricePeriod::Year,
            PlanPriceUpdate::Set(MoneyMinor::from_i32(9_999)),
        );
        assert_eq!(updates.get(PlanPricePeriod::Month), PlanPriceUpdate::Clear);
        assert!(matches!(
            updates.get(PlanPricePeriod::Year),
            PlanPriceUpdate::Set(amount) if amount.get() == 9_999
        ));
        assert_eq!(
            updates.get(PlanPricePeriod::Quarter),
            PlanPriceUpdate::Retain
        );
    }

    #[test]
    fn plan_limits_and_reset_policy_are_pure_domain_rules() {
        assert_eq!(validate_plan_name("  "), Err(PlanInputViolation::EmptyName));
        assert_eq!(
            validate_plan_transfer_enable(-1),
            Err(PlanInputViolation::TransferEnableOutOfRange)
        );
        assert_eq!(plan_transfer_bytes(2), Ok(2_147_483_648));
        assert_eq!(
            validate_plan_device_limit(Some(i64::from(i32::MAX) + 1)),
            Err(PlanInputViolation::DeviceLimitOutOfRange)
        );
        assert_eq!(validate_plan_reset_traffic_method(Some(4)), Ok(()));
        assert_eq!(
            validate_plan_reset_traffic_method(Some(5)),
            Err(PlanInputViolation::InvalidResetTrafficMethod)
        );
    }

    #[test]
    fn exact_sort_ids_are_positive_unique_i32_values() {
        assert_eq!(normalize_plan_sort_ids(&[]), Ok(Vec::new()));
        assert_eq!(normalize_plan_sort_ids(&[3, 1, 2]), Ok(vec![3, 1, 2]));
        assert_eq!(
            normalize_plan_sort_ids(&[1, 1]),
            Err(PlanInputViolation::DuplicateSortId)
        );
        for invalid in [0, -1, i64::from(i32::MAX) + 1] {
            assert_eq!(
                normalize_plan_sort_ids(&[invalid]),
                Err(PlanInputViolation::SortIdOutOfRange)
            );
        }
    }
}

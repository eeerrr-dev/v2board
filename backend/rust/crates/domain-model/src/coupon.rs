//! Pure coupon identity, applicability, and order-facing discount policy.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CouponKind {
    Amount,
    Percentage,
}

impl CouponKind {
    pub const fn code(self) -> i16 {
        match self {
            Self::Amount => 1,
            Self::Percentage => 2,
        }
    }
}

impl TryFrom<i16> for CouponKind {
    type Error = CouponRuleViolation;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Amount),
            2 => Ok(Self::Percentage),
            _ => Err(CouponRuleViolation::InvalidDiscount),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coupon {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub kind_code: i16,
    pub value: i32,
    pub visible: bool,
    pub remaining_uses: Option<i32>,
    pub per_user_limit: Option<i32>,
    pub plan_ids: Option<Vec<i32>>,
    pub periods: Option<Vec<String>>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CouponUseContext<'a> {
    pub plan_id: Option<i32>,
    pub period: Option<&'a str>,
    pub user_use_count: i64,
    pub now: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CouponRuleViolation {
    InvalidDiscount,
    Hidden,
    Unavailable,
    NotStarted,
    Expired,
    PlanNotApplicable,
    PeriodNotApplicable,
    UserLimitExceeded(i32),
}

/// Validates one coupon against the same policy used by the read-shaped check
/// endpoint and the locked order-consumption path. The caller decides whether
/// plan/period context is available; an absent context intentionally leaves
/// that restriction unchecked, matching the public coupon-check contract.
pub fn validate_coupon(
    coupon: &Coupon,
    context: CouponUseContext<'_>,
) -> Result<CouponKind, CouponRuleViolation> {
    let kind = CouponKind::try_from(coupon.kind_code)?;
    let valid_value = match kind {
        CouponKind::Amount => coupon.value >= 0,
        CouponKind::Percentage => (0..=100).contains(&coupon.value),
    };
    if !valid_value {
        return Err(CouponRuleViolation::InvalidDiscount);
    }
    if !coupon.visible {
        return Err(CouponRuleViolation::Hidden);
    }
    if coupon
        .remaining_uses
        .is_some_and(|remaining| remaining <= 0)
    {
        return Err(CouponRuleViolation::Unavailable);
    }
    if context.now < coupon.starts_at {
        return Err(CouponRuleViolation::NotStarted);
    }
    if context.now > coupon.ends_at {
        return Err(CouponRuleViolation::Expired);
    }
    if let (Some(plan_id), Some(allowed)) = (context.plan_id, coupon.plan_ids.as_ref())
        && !allowed.contains(&plan_id)
    {
        return Err(CouponRuleViolation::PlanNotApplicable);
    }
    if let (Some(period), Some(allowed)) = (context.period, coupon.periods.as_ref())
        && !allowed.iter().any(|candidate| candidate == period)
    {
        return Err(CouponRuleViolation::PeriodNotApplicable);
    }
    if let Some(limit) = coupon.per_user_limit
        && context.user_use_count >= i64::from(limit)
    {
        return Err(CouponRuleViolation::UserLimitExceeded(limit));
    }
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coupon() -> Coupon {
        Coupon {
            id: 1,
            code: "SAVE".to_string(),
            name: "Save".to_string(),
            kind_code: 2,
            value: 25,
            visible: true,
            remaining_uses: Some(3),
            per_user_limit: Some(2),
            plan_ids: Some(vec![7]),
            periods: Some(vec!["month_price".to_string()]),
            starts_at: 10,
            ends_at: 20,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn context<'a>() -> CouponUseContext<'a> {
        CouponUseContext {
            plan_id: Some(7),
            period: Some("month_price"),
            user_use_count: 1,
            now: 15,
        }
    }

    #[test]
    fn valid_coupon_returns_its_typed_discount_kind() {
        assert_eq!(
            validate_coupon(&coupon(), context()),
            Ok(CouponKind::Percentage)
        );
    }

    #[test]
    fn invalid_legacy_discounts_fail_closed_before_applicability() {
        for (kind_code, value) in [(1, -1), (2, -1), (2, 101), (9, 10)] {
            let mut coupon = coupon();
            coupon.kind_code = kind_code;
            coupon.value = value;
            assert_eq!(
                validate_coupon(&coupon, context()),
                Err(CouponRuleViolation::InvalidDiscount)
            );
        }
    }

    #[test]
    fn every_applicability_boundary_is_explicit() {
        let mut candidate = coupon();
        candidate.visible = false;
        assert_eq!(
            validate_coupon(&candidate, context()),
            Err(CouponRuleViolation::Hidden)
        );

        let mut candidate = coupon();
        candidate.remaining_uses = Some(0);
        assert_eq!(
            validate_coupon(&candidate, context()),
            Err(CouponRuleViolation::Unavailable)
        );

        let mut before = context();
        before.now = 9;
        assert_eq!(
            validate_coupon(&coupon(), before),
            Err(CouponRuleViolation::NotStarted)
        );
        let mut after = context();
        after.now = 21;
        assert_eq!(
            validate_coupon(&coupon(), after),
            Err(CouponRuleViolation::Expired)
        );

        let mut other_plan = context();
        other_plan.plan_id = Some(8);
        assert_eq!(
            validate_coupon(&coupon(), other_plan),
            Err(CouponRuleViolation::PlanNotApplicable)
        );
        let mut other_period = context();
        other_period.period = Some("year_price");
        assert_eq!(
            validate_coupon(&coupon(), other_period),
            Err(CouponRuleViolation::PeriodNotApplicable)
        );
        let mut exhausted_user = context();
        exhausted_user.user_use_count = 2;
        assert_eq!(
            validate_coupon(&coupon(), exhausted_user),
            Err(CouponRuleViolation::UserLimitExceeded(2))
        );
    }

    #[test]
    fn absent_check_context_does_not_invent_plan_or_period_rejections() {
        let mut context = context();
        context.plan_id = None;
        context.period = None;
        assert_eq!(
            validate_coupon(&coupon(), context),
            Ok(CouponKind::Percentage)
        );
    }
}

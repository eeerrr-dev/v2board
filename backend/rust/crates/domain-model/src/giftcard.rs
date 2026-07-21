//! Pure gift-card redemption rules.

const GIB_BYTES: i64 = 1_073_741_824;
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GiftCardKind {
    Balance,
    Duration,
    Traffic,
    ResetTraffic,
    Plan,
}

impl GiftCardKind {
    pub const fn code(self) -> i16 {
        match self {
            Self::Balance => 1,
            Self::Duration => 2,
            Self::Traffic => 3,
            Self::ResetTraffic => 4,
            Self::Plan => 5,
        }
    }
}

impl TryFrom<i16> for GiftCardKind {
    type Error = GiftCardRuleViolation;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Balance),
            2 => Ok(Self::Duration),
            3 => Ok(Self::Traffic),
            4 => Ok(Self::ResetTraffic),
            5 => Ok(Self::Plan),
            _ => Err(GiftCardRuleViolation::UnknownType),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardSnapshot {
    pub id: i32,
    pub kind_code: i16,
    pub value: Option<i32>,
    pub plan_id: Option<i32>,
    pub remaining_uses: Option<i32>,
    pub starts_at: i64,
    pub ends_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardUserSnapshot {
    pub id: i64,
    pub balance: i32,
    pub expires_at: Option<i64>,
    pub transfer_enable: i64,
    pub traffic_epoch: i64,
    pub uploaded: i64,
    pub downloaded: i64,
    pub plan_id: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardPlanSnapshot {
    pub id: i32,
    pub group_id: i32,
    pub transfer_gib: i64,
    pub device_limit: Option<i32>,
    pub capacity_limit: Option<i32>,
    pub capacity_used: i64,
    pub has_existing_reservation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanBindingMutation {
    pub group_id: i32,
    /// A plan card assigns this even when it is `None`.
    pub device_limit: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardRedemptionMutation {
    pub giftcard_id: i32,
    pub user_id: i64,
    pub kind: GiftCardKind,
    pub value: Option<i32>,
    pub balance: i32,
    pub expires_at: Option<i64>,
    pub transfer_enable: i64,
    pub traffic_epoch: i64,
    pub uploaded: i64,
    pub downloaded: i64,
    pub plan_id: Option<i32>,
    /// `None` retains both group and device limit. `Some` applies the plan's
    /// group and assigns its nullable device limit exactly.
    pub plan_binding: Option<PlanBindingMutation>,
    pub remaining_uses: Option<i32>,
    pub redeemed_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GiftCardRuleViolation {
    NotYetValid,
    Expired,
    UsageLimitReached,
    AlreadyRedeemed,
    NegativeValue,
    NotSuitable,
    UnknownType,
    TrafficNegative,
    DurationNegative,
    BalanceOutOfRange,
    TrafficOutOfRange,
    DurationOutOfRange,
    TrafficEpochOutOfRange,
    PlanUnavailable,
    PlanSoldOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedGiftCardRedemption {
    giftcard: GiftCardSnapshot,
    user: GiftCardUserSnapshot,
    kind: GiftCardKind,
}

impl PreparedGiftCardRedemption {
    pub const fn required_plan_id(&self) -> Option<i32> {
        if matches!(self.kind, GiftCardKind::Plan) {
            self.giftcard.plan_id
        } else {
            None
        }
    }

    pub fn apply(
        self,
        plan: Option<GiftCardPlanSnapshot>,
        now: i64,
    ) -> Result<GiftCardRedemptionMutation, GiftCardRuleViolation> {
        let value = self.giftcard.value.unwrap_or_default();
        let mut mutation = GiftCardRedemptionMutation {
            giftcard_id: self.giftcard.id,
            user_id: self.user.id,
            kind: self.kind,
            value: self.giftcard.value,
            balance: self.user.balance,
            expires_at: self.user.expires_at,
            transfer_enable: self.user.transfer_enable,
            traffic_epoch: self.user.traffic_epoch,
            uploaded: self.user.uploaded,
            downloaded: self.user.downloaded,
            plan_id: self.user.plan_id,
            plan_binding: None,
            remaining_uses: self.giftcard.remaining_uses.map(|remaining| remaining - 1),
            redeemed_at: now,
        };

        match self.kind {
            GiftCardKind::Balance => {
                mutation.balance = checked_add_cents(mutation.balance, value)?;
            }
            GiftCardKind::Duration => {
                let expires_at = mutation
                    .expires_at
                    .ok_or(GiftCardRuleViolation::NotSuitable)?;
                mutation.expires_at = Some(checked_add_giftcard_days(expires_at.max(now), value)?);
            }
            GiftCardKind::Traffic => {
                let bytes = checked_gib_bytes(i64::from(value))?;
                mutation.transfer_enable = mutation
                    .transfer_enable
                    .checked_add(bytes)
                    .ok_or(GiftCardRuleViolation::TrafficOutOfRange)?;
            }
            GiftCardKind::ResetTraffic => reset_traffic(&mut mutation)?,
            GiftCardKind::Plan => {
                let plan = plan.ok_or(GiftCardRuleViolation::PlanUnavailable)?;
                if Some(plan.id) != self.giftcard.plan_id {
                    return Err(GiftCardRuleViolation::PlanUnavailable);
                }
                if let Some(limit) = plan.capacity_limit
                    && !giftcard_plan_has_capacity(
                        limit,
                        plan.capacity_used,
                        plan.has_existing_reservation,
                    )
                {
                    return Err(GiftCardRuleViolation::PlanSoldOut);
                }
                mutation.plan_id = Some(plan.id);
                mutation.plan_binding = Some(PlanBindingMutation {
                    group_id: plan.group_id,
                    device_limit: plan.device_limit,
                });
                mutation.transfer_enable = checked_gib_bytes(plan.transfer_gib)?;
                reset_traffic(&mut mutation)?;
                mutation.expires_at = if value == 0 {
                    None
                } else {
                    Some(checked_add_giftcard_days(now, value)?)
                };
            }
        }
        Ok(mutation)
    }
}

pub fn prepare_gift_card_redemption(
    giftcard: GiftCardSnapshot,
    user: GiftCardUserSnapshot,
    already_redeemed: bool,
    now: i64,
) -> Result<PreparedGiftCardRedemption, GiftCardRuleViolation> {
    validate_gift_card_window_and_limit(giftcard, now)?;
    if already_redeemed {
        return Err(GiftCardRuleViolation::AlreadyRedeemed);
    }
    let kind = GiftCardKind::try_from(giftcard.kind_code)?;
    let value = giftcard.value.unwrap_or_default();
    if matches!(
        kind,
        GiftCardKind::Balance | GiftCardKind::Duration | GiftCardKind::Traffic | GiftCardKind::Plan
    ) && value < 0
    {
        return Err(GiftCardRuleViolation::NegativeValue);
    }
    if matches!(kind, GiftCardKind::Plan) {
        let can_apply =
            user.plan_id.is_none() || user.expires_at.is_some_and(|expires_at| expires_at < now);
        if !can_apply {
            return Err(GiftCardRuleViolation::NotSuitable);
        }
        if giftcard.plan_id.is_none() {
            return Err(GiftCardRuleViolation::PlanUnavailable);
        }
    }
    Ok(PreparedGiftCardRedemption {
        giftcard,
        user,
        kind,
    })
}

pub fn validate_gift_card_window_and_limit(
    giftcard: GiftCardSnapshot,
    now: i64,
) -> Result<(), GiftCardRuleViolation> {
    if giftcard.starts_at != 0 && now < giftcard.starts_at {
        return Err(GiftCardRuleViolation::NotYetValid);
    }
    if giftcard.ends_at != 0 && now > giftcard.ends_at {
        return Err(GiftCardRuleViolation::Expired);
    }
    if giftcard.remaining_uses.is_some_and(|limit| limit <= 0) {
        return Err(GiftCardRuleViolation::UsageLimitReached);
    }
    Ok(())
}

fn reset_traffic(mutation: &mut GiftCardRedemptionMutation) -> Result<(), GiftCardRuleViolation> {
    mutation.traffic_epoch = mutation
        .traffic_epoch
        .checked_add(1)
        .ok_or(GiftCardRuleViolation::TrafficEpochOutOfRange)?;
    mutation.uploaded = 0;
    mutation.downloaded = 0;
    Ok(())
}

pub fn checked_add_cents(left: i32, right: i32) -> Result<i32, GiftCardRuleViolation> {
    left.checked_add(right)
        .ok_or(GiftCardRuleViolation::BalanceOutOfRange)
}

pub fn checked_gib_bytes(gib: i64) -> Result<i64, GiftCardRuleViolation> {
    if gib < 0 {
        return Err(GiftCardRuleViolation::TrafficNegative);
    }
    gib.checked_mul(GIB_BYTES)
        .ok_or(GiftCardRuleViolation::TrafficOutOfRange)
}

pub fn checked_add_giftcard_days(base: i64, days: i32) -> Result<i64, GiftCardRuleViolation> {
    if days < 0 {
        return Err(GiftCardRuleViolation::DurationNegative);
    }
    let seconds = i64::from(days)
        .checked_mul(SECONDS_PER_DAY)
        .ok_or(GiftCardRuleViolation::DurationOutOfRange)?;
    base.checked_add(seconds)
        .ok_or(GiftCardRuleViolation::DurationOutOfRange)
}

pub fn giftcard_plan_has_capacity(
    capacity_limit: i32,
    capacity_used: i64,
    has_existing_reservation: bool,
) -> bool {
    has_existing_reservation || capacity_used < i64::from(capacity_limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind_code: i16, value: Option<i32>) -> GiftCardSnapshot {
        GiftCardSnapshot {
            id: 1,
            kind_code,
            value,
            plan_id: None,
            remaining_uses: Some(1),
            starts_at: 0,
            ends_at: 0,
        }
    }

    fn user() -> GiftCardUserSnapshot {
        GiftCardUserSnapshot {
            id: 7,
            balance: 100,
            expires_at: Some(500),
            transfer_enable: 10,
            traffic_epoch: 2,
            uploaded: 3,
            downloaded: 4,
            plan_id: None,
        }
    }

    #[test]
    fn units_reject_negative_values_and_integer_overflow() {
        assert_eq!(checked_gib_bytes(2).unwrap(), 2_147_483_648);
        assert!(checked_gib_bytes(-1).is_err());
        assert!(checked_gib_bytes(i64::MAX).is_err());
        assert_eq!(checked_add_giftcard_days(1_000, 2).unwrap(), 173_800);
        assert!(checked_add_giftcard_days(1_000, -1).is_err());
        assert!(checked_add_giftcard_days(i64::MAX, 1).is_err());
    }

    #[test]
    fn plan_capacity_allows_materializing_an_existing_reservation() {
        assert!(giftcard_plan_has_capacity(2, 1, false));
        assert!(!giftcard_plan_has_capacity(2, 2, false));
        assert!(giftcard_plan_has_capacity(2, 2, true));
        assert!(!giftcard_plan_has_capacity(-1, 0, false));
    }

    #[test]
    fn validation_precedence_matches_the_transactional_contract() {
        let mut invalid = card(1, Some(-1));
        invalid.starts_at = 200;
        assert_eq!(
            prepare_gift_card_redemption(invalid, user(), false, 100),
            Err(GiftCardRuleViolation::NotYetValid)
        );
        invalid.starts_at = 0;
        invalid.remaining_uses = Some(0);
        assert_eq!(
            prepare_gift_card_redemption(invalid, user(), true, 100),
            Err(GiftCardRuleViolation::UsageLimitReached)
        );
    }

    #[test]
    fn plan_card_assigns_nullable_device_limit_and_resets_traffic() {
        let mut plan_card = card(5, Some(30));
        plan_card.plan_id = Some(9);
        let mutation = prepare_gift_card_redemption(plan_card, user(), false, 1_000)
            .unwrap()
            .apply(
                Some(GiftCardPlanSnapshot {
                    id: 9,
                    group_id: 3,
                    transfer_gib: 2,
                    device_limit: None,
                    capacity_limit: Some(1),
                    capacity_used: 0,
                    has_existing_reservation: false,
                }),
                1_000,
            )
            .unwrap();
        assert_eq!(mutation.plan_id, Some(9));
        assert_eq!(
            mutation.plan_binding,
            Some(PlanBindingMutation {
                group_id: 3,
                device_limit: None
            })
        );
        assert_eq!(mutation.transfer_enable, 2_147_483_648);
        assert_eq!((mutation.uploaded, mutation.downloaded), (0, 0));
        assert_eq!(mutation.expires_at, Some(2_593_000));
    }
}

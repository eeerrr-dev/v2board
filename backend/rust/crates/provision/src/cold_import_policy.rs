//! Pure, versioned row-disposition policy for the destructive legacy cold import.
//!
//! This module deliberately performs no database or provider I/O. It defines the
//! mandatory policy contract that a future importer must apply to the frozen
//! MySQL rows during conversion and verification.

use std::collections::BTreeSet;

/// Stable policy version bound into manifests and converter registries.
pub const COLD_IMPORT_POLICY_VERSION: &str = "1";

/// Domain-separated description of every destructive Stripe decision in version 1.
pub const COLD_IMPORT_POLICY_MARKER: &str = "v2board.cold-import.v1:discard-stripe-payments;discard-unfinished-stripe-orders;scrub-retained-stripe-order-bindings";

/// Returns whether a legacy payment driver belongs to the discarded Stripe family.
///
/// Legacy drivers are class-like names such as `StripeCredit` and
/// `StripeCheckout`. Classification is deliberately independent of the payment
/// row's `enable` value: disabled and historical Stripe configurations are also
/// discarded.
pub fn is_legacy_stripe_payment_driver(driver: &str) -> bool {
    driver
        .as_bytes()
        .get(..b"stripe".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"stripe"))
}

/// The only order fields inspected or changed by the cold-import Stripe policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyOrderBinding {
    pub status: i16,
    pub payment_id: Option<i32>,
    pub callback_no: Option<String>,
}

/// Deterministic target disposition for one legacy order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyOrderDisposition {
    /// The order is unrelated to a discarded Stripe payment and remains exact.
    RetainUnchanged(LegacyOrderBinding),
    /// Status 0/1 Stripe orders are unfinished provider state and do not migrate.
    DiscardUnfinishedStripe,
    /// A completed/cancelled/offset Stripe order remains as history without a
    /// live payment or provider callback binding.
    RetainScrubbedStripe(LegacyOrderBinding),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LegacyOrderPolicyError {
    #[error("Stripe order has unsupported legacy status {0}")]
    UnsupportedStripeStatus(i16),
}

/// Applies the versioned cold-import policy to one legacy order binding.
pub fn classify_legacy_order(
    mut order: LegacyOrderBinding,
    stripe_payment_ids: &BTreeSet<i32>,
) -> Result<LegacyOrderDisposition, LegacyOrderPolicyError> {
    let is_stripe = order
        .payment_id
        .is_some_and(|payment_id| stripe_payment_ids.contains(&payment_id));
    if !is_stripe {
        return Ok(LegacyOrderDisposition::RetainUnchanged(order));
    }
    match order.status {
        0 | 1 => Ok(LegacyOrderDisposition::DiscardUnfinishedStripe),
        2..=4 => {
            order.payment_id = None;
            order.callback_no = None;
            Ok(LegacyOrderDisposition::RetainScrubbedStripe(order))
        }
        status => Err(LegacyOrderPolicyError::UnsupportedStripeStatus(status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripe_ids() -> BTreeSet<i32> {
        BTreeSet::from([7, 11])
    }

    fn order(
        status: i16,
        payment_id: Option<i32>,
        callback_no: Option<&str>,
    ) -> LegacyOrderBinding {
        LegacyOrderBinding {
            status,
            payment_id,
            callback_no: callback_no.map(str::to_owned),
        }
    }

    #[test]
    fn stripe_driver_prefix_is_ascii_case_insensitive() {
        for driver in ["Stripe", "StripeCredit", "stripecheckout", "STRIPEFoo"] {
            assert!(
                is_legacy_stripe_payment_driver(driver),
                "{driver} must be discarded"
            );
        }
        for driver in ["manual", "EPay", "NotStripe"] {
            assert!(
                !is_legacy_stripe_payment_driver(driver),
                "{driver} must be retained"
            );
        }
    }

    #[test]
    fn stripe_driver_classification_is_independent_of_enable() {
        for (driver, _enable) in [("StripeCredit", 0_i16), ("StripeCredit", 1_i16)] {
            assert!(is_legacy_stripe_payment_driver(driver));
        }
        for (driver, _enable) in [("manual", 0_i16), ("manual", 1_i16)] {
            assert!(!is_legacy_stripe_payment_driver(driver));
        }
    }

    #[test]
    fn unfinished_stripe_orders_are_discarded_for_status_zero_and_one() {
        for status in [0, 1] {
            assert_eq!(
                classify_legacy_order(order(status, Some(7), Some("pi_unfinished")), &stripe_ids(),)
                    .unwrap(),
                LegacyOrderDisposition::DiscardUnfinishedStripe
            );
        }
    }

    #[test]
    fn finished_stripe_orders_are_retained_with_bindings_scrubbed() {
        for status in [2, 3, 4] {
            assert_eq!(
                classify_legacy_order(
                    order(status, Some(11), Some("pi_historical")),
                    &stripe_ids(),
                )
                .unwrap(),
                LegacyOrderDisposition::RetainScrubbedStripe(order(status, None, None))
            );
        }
    }

    #[test]
    fn an_absent_payment_id_is_retained_unchanged() {
        for status in 0..=4 {
            let input = order(status, None, Some("unbound-callback"));
            assert_eq!(
                classify_legacy_order(input.clone(), &stripe_ids()).unwrap(),
                LegacyOrderDisposition::RetainUnchanged(input)
            );
        }
    }

    #[test]
    fn an_unknown_payment_id_is_retained_unchanged_for_every_status() {
        for status in 0..=4 {
            let input = order(status, Some(99), Some("non-stripe-callback"));
            assert_eq!(
                classify_legacy_order(input.clone(), &stripe_ids()).unwrap(),
                LegacyOrderDisposition::RetainUnchanged(input)
            );
        }
    }

    #[test]
    fn retained_non_stripe_orders_preserve_none_callbacks() {
        let input = order(3, Some(99), None);
        assert_eq!(
            classify_legacy_order(input.clone(), &stripe_ids()).unwrap(),
            LegacyOrderDisposition::RetainUnchanged(input)
        );
    }

    #[test]
    fn unknown_stripe_statuses_fail_closed() {
        for status in [-1, 5, i16::MAX] {
            assert_eq!(
                classify_legacy_order(order(status, Some(7), Some("pi_unknown")), &stripe_ids()),
                Err(LegacyOrderPolicyError::UnsupportedStripeStatus(status))
            );
        }
    }

    #[test]
    fn policy_identity_is_stable_and_domain_separated() {
        assert_eq!(COLD_IMPORT_POLICY_VERSION, "1");
        assert_eq!(
            COLD_IMPORT_POLICY_MARKER,
            "v2board.cold-import.v1:discard-stripe-payments;discard-unfinished-stripe-orders;scrub-retained-stripe-order-bindings"
        );
    }
}

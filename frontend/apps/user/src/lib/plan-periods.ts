import type { ParseKeys } from 'i18next';
import type { PlanPeriod } from '@v2board/types';

// Canonical plan-period domain knowledge for the commerce surfaces. The pages
// still carry private copies of these tables (checkout's PERIOD_LABELS/PERIODS,
// orders' PERIOD_LABEL, order detail's PERIOD_LABEL_KEY, plans' PERIOD_PRICES);
// plan-periods.test.ts pins that every copy is derivable from these exports so
// the pages can be rebuilt on top of them.
//
// Deliberately NOT shared here: checkout's getDefaultPeriod (includes
// reset_price and iterates server JSON key order — it feeds the Tier-1
// save-order payload) and plans' getUnitPriceTag (excludes reset_price, Tier-2
// display tag). They differ by design; do not unify them into one picker.

/** Every backend plan-period key, in the legacy display order. */
export const PLAN_PERIOD_ORDER: readonly PlanPeriod[] = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
];

export type PurchasablePlanPeriod = Exclude<PlanPeriod, 'reset_price'>;

/** Periods selectable at checkout and priced on plan cards — everything but reset_price. */
export const PURCHASABLE_PLAN_PERIODS: readonly PurchasablePlanPeriod[] =
  PLAN_PERIOD_ORDER.filter(
    (period): period is PurchasablePlanPeriod => period !== 'reset_price',
  );

/** PlanPeriod -> i18n label key, matching the legacy translations. */
export const PLAN_PERIOD_LABELS: Record<PlanPeriod, ParseKeys> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

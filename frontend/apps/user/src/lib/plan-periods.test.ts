import { describe, expect, it } from 'vitest';
import type { ParseKeys } from 'i18next';
import type { PlanPeriod } from '@v2board/types';
import {
  PLAN_PERIOD_LABELS,
  PLAN_PERIOD_ORDER,
  PURCHASABLE_PLAN_PERIODS,
  type PurchasablePlanPeriod,
} from './plan-periods';

// Frozen copies of the page-local literals that the shared exports must be able
// to rebuild byte-for-byte. Sources: plans/checkout.tsx (PERIOD_LABELS,
// PERIODS), orders/index.tsx (PERIOD_LABEL), orders/detail.tsx
// (PERIOD_LABEL_KEY), plans/index.tsx (PERIOD_PRICES). When those pages adopt
// lib/plan-periods, these frozen copies keep pinning the derived shapes.

const PAGE_PERIOD_LABEL_MAP: Record<PlanPeriod, ParseKeys> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

const CHECKOUT_PERIODS: {
  key: PurchasablePlanPeriod;
  period: PurchasablePlanPeriod;
  labelKey: ParseKeys;
}[] = [
  { key: 'month_price', period: 'month_price', labelKey: 'plan.monthly' },
  { key: 'quarter_price', period: 'quarter_price', labelKey: 'plan.quarterly' },
  { key: 'half_year_price', period: 'half_year_price', labelKey: 'plan.half_year' },
  { key: 'year_price', period: 'year_price', labelKey: 'plan.yearly' },
  { key: 'two_year_price', period: 'two_year_price', labelKey: 'plan.two_year' },
  { key: 'three_year_price', period: 'three_year_price', labelKey: 'plan.three_year' },
  { key: 'onetime_price', period: 'onetime_price', labelKey: 'plan.onetime' },
];

const PLANS_PERIOD_PRICES: { key: PurchasablePlanPeriod; labelKey: ParseKeys }[] = [
  { key: 'month_price', labelKey: 'plan.monthly' },
  { key: 'quarter_price', labelKey: 'plan.quarterly' },
  { key: 'half_year_price', labelKey: 'plan.half_year' },
  { key: 'year_price', labelKey: 'plan.yearly' },
  { key: 'two_year_price', labelKey: 'plan.two_year' },
  { key: 'three_year_price', labelKey: 'plan.three_year' },
  { key: 'onetime_price', labelKey: 'plan.onetime' },
];

describe('plan-periods canonical tables', () => {
  it('matches the shared 8-entry label map used by checkout, orders, and order detail', () => {
    // checkout.tsx PERIOD_LABELS, orders/index.tsx PERIOD_LABEL, and
    // orders/detail.tsx PERIOD_LABEL_KEY are the identical literal; key
    // insertion order included.
    expect(JSON.stringify(PLAN_PERIOD_LABELS)).toBe(JSON.stringify(PAGE_PERIOD_LABEL_MAP));
  });

  it('orders every plan period exactly once, in label-map key order', () => {
    expect([...PLAN_PERIOD_ORDER]).toEqual(Object.keys(PAGE_PERIOD_LABEL_MAP));
    expect(new Set(PLAN_PERIOD_ORDER).size).toBe(PLAN_PERIOD_ORDER.length);
  });

  it('derives the purchasable list by excluding reset_price without reordering', () => {
    expect([...PURCHASABLE_PLAN_PERIODS]).toEqual(
      PLAN_PERIOD_ORDER.filter((period) => period !== 'reset_price'),
    );
    expect(PURCHASABLE_PLAN_PERIODS).not.toContain('reset_price');
  });

  it("rebuilds checkout's PERIODS byte-for-byte", () => {
    const derived = PURCHASABLE_PLAN_PERIODS.map((key) => ({
      key,
      period: key,
      labelKey: PLAN_PERIOD_LABELS[key],
    }));
    expect(derived).toEqual(CHECKOUT_PERIODS);
    expect(JSON.stringify(derived)).toBe(JSON.stringify(CHECKOUT_PERIODS));
  });

  it("rebuilds plans' PERIOD_PRICES byte-for-byte", () => {
    const derived = PURCHASABLE_PLAN_PERIODS.map((key) => ({
      key,
      labelKey: PLAN_PERIOD_LABELS[key],
    }));
    expect(derived).toEqual(PLANS_PERIOD_PRICES);
    expect(JSON.stringify(derived)).toBe(JSON.stringify(PLANS_PERIOD_PRICES));
  });
});

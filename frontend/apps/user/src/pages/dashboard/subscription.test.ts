import { describe, expect, it } from 'vitest';
import { deriveDashboardSubscription } from './dashboard-subscription';

const FUTURE = '2286-11-20T17:46:39Z';
type Subscribe = NonNullable<Parameters<typeof deriveDashboardSubscription>[0]>;
type Plan = NonNullable<Subscribe['plan']>;

function makeSub(overrides: { u?: number; d?: number } = {}): Subscribe {
  const plan: Plan = {
    id: 1,
    group_id: 1,
    transfer_enable: 100_000,
    device_limit: null,
    speed_limit: null,
    reset_traffic_method: null,
    name: 'Plan',
    sort: null,
    reset_price: 100,
    renew: true,
    show: true,
    content: null,
    month_price: 1_000,
    quarter_price: null,
    half_year_price: null,
    year_price: null,
    two_year_price: null,
    three_year_price: null,
    onetime_price: null,
    capacity_limit: null,
    created_at: '1970-01-01T00:00:00Z',
    updated_at: '1970-01-01T00:00:00Z',
  };
  return {
    plan_id: 1,
    token: 't',
    expired_at: FUTURE,
    u: 0,
    d: 0,
    transfer_enable: 100_000,
    device_limit: null,
    email: 'a@b.c',
    uuid: 'u',
    plan,
    alive_ip: 0,
    subscribe_url: 'https://example.com/sub',
    reset_day: null,
    allow_new_period: true,
    ...overrides,
  };
}

describe('deriveDashboardSubscription traffic boundary', () => {
  it('does not offer a new period until the quota is truly spent (99.996% rounds to 100.00)', () => {
    // 99.996% used: the 2-decimal display value rounds up to 100.00, but the
    // real quota is not yet exhausted.
    const vm = deriveDashboardSubscription(makeSub({ u: 99_996, d: 0 }));

    expect(vm.usedPctRounded).toBe(100);
    expect(vm.canNewPeriod).toBe(false);
    expect(vm.shouldShowTrafficAlert).toBe(true);
    expect(vm.resetAvailable).toBe(true);
  });

  it('offers a new period once the quota is exactly exhausted', () => {
    const vm = deriveDashboardSubscription(makeSub({ u: 100_000, d: 0 }));

    expect(vm.canNewPeriod).toBe(true);
    expect(vm.shouldShowTrafficAlert).toBe(false);
  });

  it('keeps reset unavailable just below 80% (79.996% rounds to 80.00)', () => {
    const vm = deriveDashboardSubscription(makeSub({ u: 79_996, d: 0 }));

    expect(vm.usedPctRounded).toBe(80);
    expect(vm.resetAvailable).toBe(false);
    expect(vm.shouldShowTrafficAlert).toBe(false);
  });
});

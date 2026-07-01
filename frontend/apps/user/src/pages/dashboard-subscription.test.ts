import { describe, expect, it } from 'vitest';
import type { Plan, SubscribeInfo } from '@v2board/types';
import { useDashboardSubscription } from './dashboard-subscription';

const FUTURE = 9_999_999_999;

function makeSub(overrides: Partial<SubscribeInfo> = {}): SubscribeInfo {
  const plan = {
    id: 1,
    reset_price: 100,
    renew: 1,
    show: 1,
  } as Plan;
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
    allow_new_period: 1,
    ...overrides,
  };
}

describe('useDashboardSubscription traffic boundary', () => {
  it('does not offer a new period until the quota is truly spent (99.996% rounds to 100.00)', () => {
    // 99.996% used: the 2-decimal display value rounds up to 100.00, but the
    // real quota is not yet exhausted.
    const vm = useDashboardSubscription(makeSub({ u: 99_996, d: 0 }));

    expect(vm.usedPctRounded).toBe(100);
    expect(vm.canNewPeriod).toBe(false);
    expect(vm.shouldShowTrafficAlert).toBe(true);
    expect(vm.resetAvailable).toBe(true);
  });

  it('offers a new period once the quota is exactly exhausted', () => {
    const vm = useDashboardSubscription(makeSub({ u: 100_000, d: 0 }));

    expect(vm.canNewPeriod).toBe(true);
    expect(vm.shouldShowTrafficAlert).toBe(false);
  });

  it('keeps reset unavailable just below 80% (79.996% rounds to 80.00)', () => {
    const vm = useDashboardSubscription(makeSub({ u: 79_996, d: 0 }));

    expect(vm.usedPctRounded).toBe(80);
    expect(vm.resetAvailable).toBe(false);
    expect(vm.shouldShowTrafficAlert).toBe(false);
  });
});

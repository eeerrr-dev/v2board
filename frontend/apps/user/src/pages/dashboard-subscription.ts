import type { useSubscribe } from '@/lib/queries';

type Subscribe = ReturnType<typeof useSubscribe>['data'];
type TrafficTone = 'danger' | 'warning' | 'success';

export interface DashboardSubscriptionViewModel {
  used: number;
  usedPct: number;
  usedPctRounded: number;
  usedPctClamped: number;
  trafficTone: TrafficTone;
  daysLeft: string;
  expired: boolean;
  canRenew: boolean;
  resetAvailable: boolean;
  shouldShowTrafficAlert: boolean;
  trafficAlertResetAvailable: boolean;
  canNewPeriod: boolean;
}

export function useDashboardSubscription(sub: Subscribe): DashboardSubscriptionViewModel {
  const hasPlan = Boolean(sub?.plan_id);
  const used = sub ? sub.u + sub.d : 0;
  const usedPct = sub?.transfer_enable ? (used / sub.transfer_enable) * 100 : 0;
  const usedPctRounded = Math.round(usedPct * 100) / 100;
  const usedPctClamped = Math.max(0, Math.min(100, usedPct));
  const trafficTone = getTrafficTone(usedPctRounded);
  const daysLeft = getSubscriptionDaysLeft(sub?.expired_at);
  const expired = isSubscriptionExpired(sub?.expired_at ?? null);
  const canRenew = isSubscriptionRenewable(sub);
  // Gate the reset/new-period actions on the true usage, not the 2-decimal
  // display value: rounding 99.996% up to 100.00 would otherwise offer a new
  // period (and drop the low-traffic alert) before the quota is really spent.
  const resetAvailable = Boolean(hasPlan && sub?.plan?.reset_price && usedPct >= 80 && !expired);
  const shouldShowTrafficAlert = Boolean(usedPct >= 80 && usedPct < 100 && !expired);
  const trafficAlertResetAvailable = Boolean(sub?.plan?.reset_price);
  const canNewPeriod = Boolean(hasPlan && sub?.allow_new_period && usedPct >= 100 && !expired);

  return {
    used,
    usedPct,
    usedPctRounded,
    usedPctClamped,
    trafficTone,
    daysLeft,
    expired,
    canRenew,
    resetAvailable,
    shouldShowTrafficAlert,
    trafficAlertResetAvailable,
    canNewPeriod,
  };
}

export function isSubscriptionExpired(expiredAt: number | null | undefined) {
  return expiredAt !== null && expiredAt !== undefined && expiredAt < Date.now() / 1000;
}

export function getTrafficTone(usedPctRounded: number): TrafficTone {
  if (usedPctRounded >= 100) return 'danger';
  if (usedPctRounded >= 80) return 'warning';
  return 'success';
}

export function getSubscriptionDaysLeft(timestamp: number | string | null | undefined) {
  return ((Number(timestamp) - Math.floor(Date.now() / 1000)) / 86400).toFixed(0);
}

export function isSubscriptionRenewable(subscribe: Subscribe) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(subscribe.plan.show || !isSubscriptionExpired(subscribe.expired_at));
}

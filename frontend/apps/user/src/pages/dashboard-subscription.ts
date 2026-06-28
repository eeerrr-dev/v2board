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
  const daysLeft = legacyDaysUntil(sub?.expired_at);
  const expired = isLegacyExpired(sub?.expired_at ?? null);
  const canRenew = isLegacyRenewable(sub);
  const resetAvailable = Boolean(
    hasPlan && sub?.plan?.reset_price && usedPctRounded >= 80 && !expired,
  );
  const shouldShowTrafficAlert = Boolean(usedPctRounded >= 80 && usedPctRounded < 100 && !expired);
  const trafficAlertResetAvailable = Boolean(sub?.plan?.reset_price);
  const canNewPeriod = Boolean(
    hasPlan && sub?.allow_new_period && usedPctRounded >= 100 && !expired,
  );

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

export function isLegacyExpired(expiredAt: number | null | undefined) {
  return expiredAt !== null && expiredAt !== undefined && expiredAt < Date.now() / 1000;
}

export function getTrafficTone(usedPctRounded: number): TrafficTone {
  if (usedPctRounded >= 100) return 'danger';
  if (usedPctRounded >= 80) return 'warning';
  return 'success';
}

export function legacyDaysUntil(timestamp: number | string | null | undefined) {
  return ((Number(timestamp) - Math.floor(Date.now() / 1000)) / 86400).toFixed(0);
}

export function isLegacyRenewable(subscribe: Subscribe) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(subscribe.plan.show || !isLegacyExpired(subscribe.expired_at));
}

import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import {
  type SaveOrderInput,
  useCancelOrderMutation,
  useCheckCouponMutation,
  useCommConfig,
  useOrders,
  usePlan,
  useSaveOrderMutation,
  useSubscribe,
  useUserInfo,
} from '@/lib/queries';
import type { Coupon, Plan, PlanPeriod } from '@v2board/types';
import { PLAN_PERIOD_LABELS, PURCHASABLE_PLAN_PERIODS } from '@/lib/plan-periods';
import { isSubscriptionExpired } from '@/pages/dashboard-subscription';
import { PlanContent } from '@/components/plan-content';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { Input } from '@/components/ui/input';
import { PageShell } from '@/components/ui/page';
import { RadioGroup, RadioGroupIndicator, RadioGroupItem } from '@/components/ui/radio-group';
import { Spinner } from '@/components/ui/spinner';

// Derived from the canonical lib/plan-periods tables; plan-periods.test.ts pins
// that this derivation matches the backend order-amount contract exactly.
const PERIODS = PURCHASABLE_PLAN_PERIODS.map((key) => ({
  key,
  period: key,
  labelKey: PLAN_PERIOD_LABELS[key],
}));

export default function PlanCheckoutPage() {
  const { plan_id } = useParams();
  const planId = plan_id;
  const { t } = useTranslation();
  const navigate = useNavigate();
  const planQuery = usePlan(planId);
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const orders = useOrders();
  const { data: info } = useUserInfo({ refetchOnMount: false });
  const { data: subscribe } = useSubscribe({ enabled: false });
  const cancelOrder = useCancelOrderMutation();
  const checkCoupon = useCheckCouponMutation();
  const saveOrderMutation = useSaveOrderMutation();
  const [period, setPeriod] = useState<PlanPeriod | undefined>();
  const [couponCode, setCouponCode] = useState('');
  const [appliedCoupon, setAppliedCoupon] = useState<Coupon | null>(null);
  const ordersStateRef = useRef({ data: orders.data, isSuccess: orders.isSuccess });

  // Confirm dialogs outlive the render that opened them. Keep their eventual
  // actions connected to the latest query state instead of a stale success
  // snapshot if the orders request fails or is reset while the dialog is open.
  useEffect(() => {
    ordersStateRef.current = { data: orders.data, isSuccess: orders.isSuccess };
  }, [orders.data, orders.isSuccess]);

  const symbol = comm?.currency_symbol;
  const currency = comm?.currency;

  // React Compiler memoizes this derivation; no manual useMemo needed.
  const planData = planQuery.data;
  const periods = planData
    ? PERIODS.filter((item) => isValidCheckoutPeriod(planData, item.key))
    : [];

  const onApplyCoupon = () => {
    checkCoupon.mutate(
      {
        code: couponCode,
        planId: planId as string,
      },
      {
        onSuccess: setAppliedCoupon,
        onError: () => {
          // A failed re-verify must not leave a previously applied coupon in place:
          // saveOrder sends appliedCoupon.code, so a stale discount would otherwise
          // be shown in the total and submitted under a now-invalid code. The shared
          // MutationCache remains the single owner of the error notification.
          setAppliedCoupon(null);
        },
      },
    );
  };

  const getUnfinishedOrderCheck = () => {
    const currentOrders = ordersStateRef.current;
    const firstOrder = currentOrders.data?.[0];
    const unfinishedOrder =
      firstOrder && (firstOrder.status === 0 || firstOrder.status === 1) ? firstOrder : undefined;
    return { isSuccess: currentOrders.isSuccess, unfinishedOrder };
  };

  const saveOrder = (cancelledUnfinishedTradeNo?: string) => {
    // Fail closed in the action itself as well as in the disabled UI. An
    // absent orders payload can mean "not fetched" or "fetch failed"; only a
    // successful query proves that there is no order to cancel first.
    const ordersCheck = getUnfinishedOrderCheck();
    if (!ordersCheck.isSuccess || !planQuery.data) return;
    const { unfinishedOrder } = ordersCheck;
    if (unfinishedOrder && unfinishedOrder.trade_no !== cancelledUnfinishedTradeNo) {
      return;
    }
    const currentPeriod = period ?? getDefaultPeriod(planQuery.data);
    if (!isValidCheckoutPeriod(planQuery.data, currentPeriod) || hasInvalidCoupon(appliedCoupon)) {
      return;
    }
    const payload: SaveOrderInput = {
      plan_id: planQuery.data.id,
      period: currentPeriod,
    };
    if (appliedCoupon?.name) payload.coupon_code = appliedCoupon.code;

    saveOrderMutation.mutate(payload, {
      onSuccess: (tradeNo) => void navigate(`/order/${tradeNo}`),
    });
  };

  const continueAfterUnfinishedOrderCheck = async () => {
    const ordersCheck = getUnfinishedOrderCheck();
    if (!ordersCheck.isSuccess) return;
    const { unfinishedOrder } = ordersCheck;
    if (unfinishedOrder) {
      void confirmDialog({
        title: t(($) => $.common.attention),
        description: t(($) => $.plan.unfinished_order_confirm),
        confirmText: t(($) => $.plan.confirm_cancel_previous),
        cancelText: t(($) => $.plan.return_orders),
        confirmButtonProps: { loading: cancelOrder.isPending },
        onConfirm: async () => {
          await cancelOrder.mutateAsync(unfinishedOrder.trade_no);
          saveOrder(unfinishedOrder.trade_no);
        },
        onCancel: () => navigate('/order'),
      });
      return;
    }
    saveOrder();
  };

  const onSubmit = () => {
    // This guard is intentionally duplicated in saveOrder: the button's
    // disabled attribute is presentation, not a security/consistency boundary.
    if (!getUnfinishedOrderCheck().isSuccess) return;
    const plan = planQuery.data;
    if (!plan) return;
    const currentPeriod = period ?? getDefaultPeriod(plan);
    if (!isValidCheckoutPeriod(plan, currentPeriod) || hasInvalidCoupon(appliedCoupon)) return;
    if (
      info?.plan_id &&
      info.plan_id !== plan.id &&
      !isSubscriptionExpired(subscribe?.expired_at)
    ) {
      void confirmDialog({
        title: t(($) => $.common.attention),
        description: t(($) => $.plan.change_warning),
        onConfirm: continueAfterUnfinishedOrderCheck,
      });
      return;
    }
    void continueAfterUnfinishedOrderCheck();
  };

  // Full-page spinner only for the initial load: cached plan data keeps
  // rendering while the mount refetch runs in the background.
  if (planQuery.isPending) {
    return (
      <div className="flex min-h-44 items-center justify-center" role="status">
        <Spinner className="size-5" />
      </div>
    );
  }

  // A failed plan fetch must not fall through to a cashier with no product data.
  // Surface the error with a retry instead (failure presentation is Tier-2 on
  // this redesigned surface).
  if (planQuery.isError || !planQuery.data) {
    return <ErrorState data-testid="checkout-error" onRetry={() => void planQuery.refetch()} />;
  }

  const plan = planQuery.data;
  const selectedPeriod = period ?? getDefaultPeriod(plan);
  const selectedPrice = selectedPeriod ? plan[selectedPeriod] : undefined;
  const validPeriod = typeof selectedPrice === 'number' && Number.isFinite(selectedPrice);
  const basePrice = validPeriod ? selectedPrice : 0;
  const periodLabel = selectedPeriod ? t(PLAN_PERIOD_LABELS[selectedPeriod]) : '';
  const invalidCoupon = hasInvalidCoupon(appliedCoupon);
  let discount = 0;
  if (appliedCoupon?.name && !invalidCoupon) {
    if (appliedCoupon.type === 1) discount = Number(appliedCoupon.value.toFixed(2));
    if (appliedCoupon.type === 2) {
      discount = Number((basePrice * (appliedCoupon.value / 100)).toFixed(2));
    }
  }
  const totalAmount = Math.max(0, basePrice - discount);
  const validationError = !validPeriod
    ? t(($) => $.errors['This payment period cannot be purchased, please choose another period'])
    : invalidCoupon
      ? t(($) => $.errors['Coupon failed'])
      : null;
  const canRenew = Boolean(plan.renew || info?.plan_id !== plan.id);

  if (!canRenew) {
    return (
      <Card data-testid="plan-non-renewable">
        <CardContent className="flex min-h-64 flex-col items-center justify-center gap-4 text-center">
          <div className="space-y-2">
            <h2 className="text-lg leading-7 font-semibold text-card-foreground">
              {t(($) => $.plan.cannot_renew_current)}
            </h2>
            <p className="text-sm text-muted-foreground">{t(($) => $.plan.select_other)}</p>
          </div>
          <Button asChild>
            <Link to="/plan">{t(($) => $.plan.select_other)}</Link>
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <PageShell
      id="cashier"
      data-testid="checkout-page"
      className="grid max-w-6xl gap-6 @3xl/main:grid-cols-[minmax(0,1fr)_24rem]"
    >
      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-xl leading-7">{plan.name}</CardTitle>
          </CardHeader>
          <CardContent>
            <PlanContent content={plan.content} htmlClassName="custom-html-style" />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base leading-6">{t(($) => $.plan.select_period)}</CardTitle>
          </CardHeader>
          <CardContent className="p-6 pt-0">
            <RadioGroup
              value={selectedPeriod}
              onValueChange={(nextPeriod) => setPeriod(nextPeriod as PlanPeriod)}
            >
              {periods.map((item) => {
                const price = plan[item.period];
                if (typeof price !== 'number' || !Number.isFinite(price)) return null;
                return (
                  <RadioGroupItem
                    key={item.period}
                    value={item.period}
                    data-testid="checkout-period-option"
                  >
                    <div className="flex items-center gap-3">
                      <RadioGroupIndicator data-testid="checkout-period-radio" />
                      {t(item.labelKey)}
                    </div>
                    <span className="font-medium">
                      {symbol}
                      {(price / 100).toFixed(2)}
                    </span>
                  </RadioGroupItem>
                );
              })}
            </RadioGroup>
          </CardContent>
        </Card>
      </div>

      <aside data-testid="checkout-side" className="space-y-4">
        <Card>
          <CardContent className="flex flex-col gap-2 p-4 sm:flex-row">
            <Input
              type="text"
              data-testid="coupon-input"
              value={couponCode}
              placeholder={t(($) => $.plan.coupon_question)}
              onChange={(event) => setCouponCode(event.target.value)}
            />
            <Button loading={checkCoupon.isPending} onClick={onApplyCoupon} type="button">
              {t(($) => $.plan.verify)}
            </Button>
          </CardContent>
        </Card>

        <Card data-testid="checkout-summary">
          <CardHeader>
            <CardTitle className="text-base leading-6">{t(($) => $.plan.order_total)}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-start justify-between gap-4 border-b border-border pb-4 text-sm">
              <div>
                {plan.name} x {periodLabel}
              </div>
              <div className="text-right font-medium">
                {symbol}
                {(basePrice / 100).toFixed(2)}
              </div>
            </div>
            {appliedCoupon?.name ? (
              <div className="space-y-2 border-b border-border pb-4 text-sm">
                <div className="text-muted-foreground">{t(($) => $.plan.discount)}</div>
                <div className="flex items-center justify-between gap-4">
                  <div>{appliedCoupon.name}</div>
                  <div className="text-right font-medium">
                    -{symbol}
                    {(discount / 100).toFixed(2)}
                  </div>
                </div>
              </div>
            ) : null}
            <div className="text-sm text-muted-foreground">{t(($) => $.plan.grand_total)}</div>
            <div className="text-3xl font-semibold tracking-normal text-card-foreground">
              {symbol} {(totalAmount / 100).toFixed(2)} {currency}
            </div>
            {info?.plan_id &&
            info.plan_id !== plan.id &&
            !isSubscriptionExpired(subscribe?.expired_at) ? (
              <Alert>
                <AlertDescription>{t(($) => $.plan.change_warning)}</AlertDescription>
              </Alert>
            ) : null}
            {validationError ? (
              <Alert variant="destructive" data-testid="checkout-validation-error">
                <AlertDescription>{validationError}</AlertDescription>
              </Alert>
            ) : null}
            {orders.isPending ? (
              <div
                className="flex items-center gap-2 text-sm text-muted-foreground"
                role="status"
                data-testid="unfinished-orders-loading"
              >
                <Spinner className="size-4" />
                <span>{t(($) => $.common.loading)}</span>
              </div>
            ) : null}
            {orders.isError ? (
              <ErrorState
                data-testid="unfinished-orders-error"
                onRetry={() => void orders.refetch()}
              />
            ) : null}
            <Button
              type="button"
              block
              data-testid="commerce-submit"
              loading={saveOrderMutation.isPending}
              disabled={validationError !== null || !orders.isSuccess}
              onClick={onSubmit}
            >
              {t(($) => $.plan.place_order)}
            </Button>
          </CardContent>
        </Card>
      </aside>
    </PageShell>
  );
}

// Deliberately local and unshared (see lib/plan-periods.ts): includes
// reset_price and iterates server JSON key order — it feeds the Tier-1
// save-order payload.
function getDefaultPeriod(plan: Plan): PlanPeriod | undefined {
  let period: PlanPeriod | undefined;
  for (const key of Object.keys(plan).reverse()) {
    if (key in PLAN_PERIOD_LABELS && isValidCheckoutPeriod(plan, key as PlanPeriod)) {
      period = key as PlanPeriod;
    }
  }
  return period;
}

function isValidCheckoutPeriod(plan: Plan, period: PlanPeriod | undefined): period is PlanPeriod {
  if (!period) return false;
  const price = plan[period];
  return typeof price === 'number' && Number.isFinite(price);
}

function hasInvalidCoupon(coupon: Coupon | null): boolean {
  if (!coupon?.name) return false;
  return (coupon.type !== 1 && coupon.type !== 2) || !Number.isFinite(coupon.value);
}

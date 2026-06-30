import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import {
  type SaveOrderPayload,
  useCancelOrderMutation,
  useCheckCouponMutation,
  useCommConfig,
  useOrders,
  usePlan,
  useSaveOrderMutation,
  useSubscribe,
  useUserInfo,
} from '@/lib/queries';
import type { ParseKeys } from 'i18next';
import type { Coupon, Plan, PlanPeriod } from '@v2board/types';
import { PlanContent } from '@/components/plan-content';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { PageShell } from '@/components/ui/page';
import { RadioGroup, RadioGroupIndicator, RadioGroupItem } from '@/components/ui/radio-group';
import { Spinner } from '@/components/ui/spinner';

const PERIOD_LABELS: Record<PlanPeriod, ParseKeys> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

type PurchasablePlanPeriod = Exclude<PlanPeriod, 'reset_price'>;

const PERIODS: { key: PurchasablePlanPeriod; period: PurchasablePlanPeriod; labelKey: ParseKeys }[] = [
  { key: 'month_price', period: 'month_price', labelKey: 'plan.monthly' },
  { key: 'quarter_price', period: 'quarter_price', labelKey: 'plan.quarterly' },
  { key: 'half_year_price', period: 'half_year_price', labelKey: 'plan.half_year' },
  { key: 'year_price', period: 'year_price', labelKey: 'plan.yearly' },
  { key: 'two_year_price', period: 'two_year_price', labelKey: 'plan.two_year' },
  { key: 'three_year_price', period: 'three_year_price', labelKey: 'plan.three_year' },
  { key: 'onetime_price', period: 'onetime_price', labelKey: 'plan.onetime' },
];

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

  const symbol = comm?.currency_symbol;
  const currency = comm?.currency;

  const periods = useMemo(() => {
    if (!planQuery.data) return [];
    return PERIODS.filter(
      (p) =>
        planQuery.data && planQuery.data[p.key] !== null,
    );
  }, [planQuery.data]);

  useEffect(() => {
    if (planQuery.error) navigate('/plan');
  }, [navigate, planQuery.error]);

  const onApplyCoupon = async () => {
    try {
      const checked = await checkCoupon.mutateAsync({
        code: couponCode,
        planId: planId as string,
      });
      setAppliedCoupon(checked);
    } catch {}
  };

  const saveOrder = async () => {
    if (!planQuery.data) return;
    const currentPeriod = period ?? getDefaultPeriod(planQuery.data);
    const payload: SaveOrderPayload = {
      plan_id: planQuery.data.id,
      period: currentPeriod,
    };
    if (appliedCoupon?.name) payload.coupon_code = appliedCoupon.code;

    try {
      const tradeNo = await saveOrderMutation.mutateAsync(payload);
      navigate(`/order/${tradeNo}`);
    } catch {}
  };

  const onSubmit = async () => {
    const plan = planQuery.data;
    if (!plan) return;
    if (
      info?.plan_id &&
      info.plan_id !== plan.id &&
      !isLegacyExpired(subscribe?.expired_at)
    ) {
      void confirmDialog({
        title: t('common.attention'),
        description: t('plan.change_warning'),
        onConfirm: saveOrder,
      });
      return;
    }

    const firstOrder = orders.data?.[0];
    const unfinishedOrder =
      firstOrder && (firstOrder.status === 0 || firstOrder.status === 1) ? firstOrder : undefined;
    if (unfinishedOrder) {
      void confirmDialog({
        title: t('common.attention'),
        description: t('plan.unfinished_order_confirm'),
        confirmText: t('plan.confirm_cancel_previous'),
        cancelText: t('plan.return_orders'),
        confirmButtonProps: { loading: cancelOrder.isPending },
        onConfirm: async () => {
          await cancelOrder.mutateAsync(unfinishedOrder.trade_no);
          await saveOrder();
        },
        onCancel: () => navigate('/order'),
      });
      return;
    }
    await saveOrder();
  };

  if (planQuery.isFetching || planQuery.error || !planQuery.data) {
    return (
      <div className="flex min-h-44 items-center justify-center" role="status">
        <Spinner className="size-5" />
      </div>
    );
  }

  const plan = planQuery.data;
  const selectedPeriod = period ?? getDefaultPeriod(plan);
  // Faithful to the original render: the base row is `(plan[selectPeriod]/100).toFixed(2)`
  // and getTotalAmount() reads `plan[selectPeriod]` directly. With no period selectable
  // (all prices null) selectPeriod has no value, so plan[selectPeriod] is undefined and
  // both the base row and grand total render "NaN" rather than "0.00".
  const basePrice = (plan as unknown as Record<string, number | null>)[
    selectedPeriod as PlanPeriod
  ] as number;
  const periodLabel = selectedPeriod ? t(PERIOD_LABELS[selectedPeriod]) : '';
  // Faithful to the original couponProcess(amount, type, value): case 1 → value,
  // case 2 → amount*(value/100), no default → undefined → the discount/total render
  // "NaN" for any unknown coupon type.
  const discount = appliedCoupon?.name
    ? appliedCoupon.type === 1
      ? Number(appliedCoupon.value.toFixed(2))
      : appliedCoupon.type === 2
        ? Number((basePrice * (appliedCoupon.value / 100)).toFixed(2))
        : NaN
    : 0;
  const totalAmount = Math.max(0, basePrice - discount);
  const canRenew = Boolean(plan.renew || info?.plan_id !== plan.id);

  if (!canRenew) {
    return (
      <Card data-testid="plan-non-renewable">
        <CardContent className="flex min-h-64 flex-col items-center justify-center gap-4 text-center">
          <div className="space-y-2">
            <h2 className="text-lg font-semibold leading-7 text-card-foreground">{t('plan.cannot_renew_current')}</h2>
            <p className="text-sm text-muted-foreground">{t('plan.select_other')}</p>
          </div>
          <Button type="button" onClick={() => navigate('/plan')}>
            {t('plan.select_other')}
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <PageShell id="cashier" data-testid="checkout-page" className="grid max-w-6xl gap-6 lg:grid-cols-[minmax(0,1fr)_24rem]">
      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-xl leading-7">{plan.name}</CardTitle>
          </CardHeader>
          <CardContent>
            <PlanContent
              content={plan.content}
              className="plan-content"
              htmlClassName="custom-html-style plan-content"
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base leading-6">{t('plan.select_period')}</CardTitle>
          </CardHeader>
          <CardContent className="p-6 pt-0">
            <RadioGroup
              value={selectedPeriod}
              onValueChange={(nextPeriod) => setPeriod(nextPeriod as PlanPeriod)}
            >
              {periods.map((item) => {
                const price = plan[item.period];
                if (price === null) return null;
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
                    <span className="price font-medium">
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
              placeholder={t('plan.coupon_question')}
              onChange={(event) => setCouponCode(event.target.value)}
            />
            <Button loading={checkCoupon.isPending} onClick={onApplyCoupon} type="button">
              {t('plan.verify')}
            </Button>
          </CardContent>
        </Card>

        <Card data-testid="checkout-summary">
          <CardHeader>
            <CardTitle className="text-base leading-6">{t('plan.order_total')}</CardTitle>
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
                <div className="text-muted-foreground">{t('plan.discount')}</div>
                <div className="flex items-center justify-between gap-4">
                  <div>{appliedCoupon.name}</div>
                  <div className="text-right font-medium">
                    -{symbol}
                    {(discount / 100).toFixed(2)}
                  </div>
                </div>
              </div>
            ) : null}
            <div className="text-sm text-muted-foreground">{t('plan.grand_total')}</div>
            <div className="text-3xl font-semibold tracking-normal text-card-foreground">
              {symbol} {(totalAmount / 100).toFixed(2)} {currency}
            </div>
            {info?.plan_id && info.plan_id !== plan.id && !isLegacyExpired(subscribe?.expired_at) ? (
              <Alert>
                <AlertDescription>{t('plan.change_warning')}</AlertDescription>
              </Alert>
            ) : null}
            <Button
              type="button"
              block
              data-testid="commerce-submit"
              loading={saveOrderMutation.isPending}
              onClick={onSubmit}
            >
              {t('plan.place_order')}
            </Button>
          </CardContent>
        </Card>
      </aside>
    </PageShell>
  );
}

function getDefaultPeriod(plan: Plan): PlanPeriod | undefined {
  let period: PlanPeriod | undefined;
  for (const key of Object.keys(plan).reverse()) {
    if (key in PERIOD_LABELS && plan[key as PlanPeriod] !== null) {
      period = key as PlanPeriod;
    }
  }
  return period;
}

function isLegacyExpired(expiredAt: number | null | undefined) {
  return expiredAt !== null && Number(expiredAt) < Date.now() / 1000;
}

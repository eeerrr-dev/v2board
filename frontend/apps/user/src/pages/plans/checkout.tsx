import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { user } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import {
  userKeys,
  useCancelOrderMutation,
  useCommConfig,
  useOrders,
  usePlan,
  useSubscribe,
  useUserInfo,
} from '@/lib/queries';
import type { Coupon, Plan, PlanPeriod } from '@v2board/types';
import { PlanContent } from '@/components/plan-content';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { ExclamationCircleIcon } from '@/components/ant-icon';
import { AntBtn } from '@/components/ant-btn';

const PERIOD_LABELS: Record<PlanPeriod, string> = {
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

const PERIODS: { key: PurchasablePlanPeriod; period: PurchasablePlanPeriod; labelKey: string }[] = [
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
  const queryClient = useQueryClient();
  const planQuery = usePlan(planId);
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const orders = useOrders();
  const { data: info } = useUserInfo({ refetchOnMount: false });
  const { data: subscribe } = useSubscribe({ enabled: false });
  const cancelOrder = useCancelOrderMutation();
  const [period, setPeriod] = useState<PlanPeriod | undefined>();
  const [appliedCoupon, setAppliedCoupon] = useState<Coupon | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const couponRef = useRef<HTMLInputElement>(null);

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
    if (period || !planQuery.data) return;
    setPeriod(getDefaultPeriod(planQuery.data));
  }, [period, planQuery.data]);

  useEffect(() => {
    if (planQuery.error) navigate('/plan');
  }, [navigate, planQuery.error]);

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: userKeys.plans });
      queryClient.removeQueries({ queryKey: userKeys.plan(planId as string) });
    },
    [planId, queryClient],
  );

  const onApplyCoupon = async () => {
    try {
      const checked = await user.checkCoupon(
        apiClient,
        couponRef.current!.value,
        planId as string,
      );
      setAppliedCoupon(checked);
    } catch {}
  };

  const saveOrder = async () => {
    if (!planQuery.data) return;
    const currentPeriod = period ?? getDefaultPeriod(planQuery.data);
    setSubmitting(true);
    try {
      const tradeNo = await user.saveOrder(apiClient, {
        plan_id: planQuery.data.id,
        period: currentPeriod,
        coupon_code: appliedCoupon?.name ? appliedCoupon.code : undefined,
      });
      navigate(`/order/${tradeNo}`);
    } catch {
    } finally {
      setSubmitting(false);
    }
  };

  const onSubmit = async () => {
    const plan = planQuery.data;
    if (!plan) return;
    if (
      info?.plan_id &&
      info.plan_id !== plan.id &&
      !isLegacyExpired(subscribe?.expired_at)
    ) {
      void legacyConfirm({
        title: t('common.attention'),
        content: t('plan.change_warning'),
        onOk: () => {
          void saveOrder();
        },
      });
      return;
    }

    const firstOrder = orders.data?.[0];
    const unfinishedOrder =
      firstOrder && (firstOrder.status === 0 || firstOrder.status === 1) ? firstOrder : undefined;
    if (unfinishedOrder) {
      void legacyConfirm({
        title: t('common.attention'),
        content: t('plan.unfinished_order_confirm'),
        okText: t('plan.confirm_cancel_previous'),
        cancelText: t('plan.return_orders'),
        okButtonProps: { loading: cancelOrder.isPending },
        onOk: () => {
          void cancelOrder
            .mutateAsync(unfinishedOrder.trade_no)
            .then(() => {
              // Legacy order/cancel owns the list refresh, then dispatches `details`
              // (plural), which has no effect; this callback only continues ordering.
              void saveOrder();
            })
            .catch(() => {});
        },
        onCancel: () => navigate('/order'),
      });
      return;
    }
    await saveOrder();
  };

  if (planQuery.isFetching) {
    return (
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
      </div>
    );
  }
  if (planQuery.error || !planQuery.data) {
    return (
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
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
      <div className="row">
        <div className="col-12">
          <div className="block block-rounded">
            <div className="block-content">
              <div className="ant-result ant-result-info">
                <div className="ant-result-icon">
                  <ExclamationCircleIcon />
                </div>
                <div className="ant-result-title">{t('plan.cannot_renew_current')}</div>
                <div className="ant-result-subtitle">
                  {/* antd v3 Button: classNames('ant-btn', className, {'ant-btn-primary':type}) —
                      the passed className precedes the type modifier. */}
                  <AntBtn
                    type="button"
                    className="ant-btn mt-3 ant-btn-primary"
                    onClick={() => navigate('/plan')}
                  >
                    {t('plan.select_other')}
                  </AntBtn>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="row" id="cashier">
      <div className="col-md-8 col-sm-12">
        <div className="block block-link-pop block-rounded py-3" style={{ backgroundColor: '#fff' }}>
          <h4 className="mb-0 px-3">{plan.name}</h4>
          <PlanContent
            content={plan.content}
            className="v2board-plan-content px-3"
            htmlClassName="v2board-plan-content"
            guardNull
          />
        </div>

        <div className="block block-rounded js-appear-enabled">
          <div className="block-header block-header-default">
            <h3 className="block-title">{t('plan.select_period')}</h3>
            <div className="block-options" />
          </div>
          <div className="block-content p-0">
            {periods.map((item) => {
              const price = plan[item.period];
              if (price === null) return null;
              return (
                <div
                  onClick={() => setPeriod(item.period)}
                  className={`v2board-select ${selectedPeriod === item.period ? 'active border-primary' : 'false'}`}
                >
                  <div style={{ flex: 1 }}>
                    {/* antd v3 Radio: classNames(className, {'ant-radio-wrapper':true,
                        'ant-radio-wrapper-checked':checked}) — the passed className leads. */}
                    <label
                      className={`v2board-select-radio ant-radio-wrapper${selectedPeriod === item.period ? ' ant-radio-wrapper-checked' : ''}`}
                    >
                      <span className={`ant-radio${selectedPeriod === item.period ? ' ant-radio-checked' : ''}`}>
                        {/* The visible checked dot is driven by the ant-radio-checked class
                            above; the (opacity:0) input binds to the post-mount `period`
                            state — never the render-time default — so it adopts checked via
                            an update rather than at mount. The original (React 16 antd Radio)
                            sets selectPeriod atomically with the plan yet never reflects
                            `checked` to the attribute; React 19 reflects a mount-time checked,
                            so deferring keeps the DOM attribute-free, matching the original. */}
                        <input
                          type="radio"
                          className="ant-radio-input"
                          checked={period === item.period}
                          onChange={() => {}}
                        />
                        <span className="ant-radio-inner" />
                      </span>
                    </label>
                    {t(item.labelKey)}
                  </div>
                  <div style={{ flex: 1, textAlign: 'right' }}>
                    <span className="price">
                      {symbol}
                      {(price / 100).toFixed(2)}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      <div className="col-md-4 col-sm-12">
        <div
          // Original class string has a DOUBLE space after `block-rounded` (umi.js).
          className="block block-link-pop block-rounded  px-3 py-3 mb-2 text-light"
          style={{ background: '#35383D' }}
        >
          <input
            type="text"
            className="form-control v2board-input-coupon p-0"
            ref={couponRef}
            placeholder={t('plan.coupon_question')}
          />
          <button
            onClick={onApplyCoupon}
            type="button"
            className="btn btn-primary"
            style={{ position: 'absolute', right: 30, top: 17 }}
          >
            <i className="fa fa-fw fa-ticket-alt mr-2" />
            {t('plan.verify')}
          </button>
        </div>

        <div
          // Original class string has a DOUBLE space after `block-rounded` (umi.js).
          className="block block-link-pop block-rounded  px-3 py-3 text-light"
          style={{ background: '#35383D' }}
        >
          <h5 className="text-light mb-3">{t('plan.order_total')}</h5>
          <div className="row no-gutters pb-3" style={{ borderBottom: '1px solid #646669' }}>
            <div className="col-8">
              {plan.name} x {periodLabel}
            </div>
            <div className="col-4 text-right">
              {symbol}
              {(basePrice / 100).toFixed(2)}
            </div>
          </div>
          {appliedCoupon?.name ? (
            <div>
              <div className="pt-3" style={{ color: '#646669' }}>
                {t('plan.discount')}
              </div>
              <div className="row no-gutters py-3" style={{ borderBottom: '1px solid #646669' }}>
                <div className="col-8">{appliedCoupon.name}</div>
                <div className="col-4 text-right">
                  -{symbol}
                  {(discount / 100).toFixed(2)}
                </div>
              </div>
            </div>
          ) : null}
          <div className="pt-3" style={{ color: '#646669' }}>
            {t('plan.grand_total')}
          </div>
          <h1 className="text-light mt-3 mb-3">
            {symbol} {(totalAmount / 100).toFixed(2)} {currency}
          </h1>
          <button
            type="button"
            className="btn btn-block btn-primary"
            disabled={submitting}
            onClick={onSubmit}
          >
            {submitting ? (
              <LegacyLoadingIcon />
            ) : (
              <span>
                <i className="far fa-check-circle" /> {t('plan.place_order')}
              </span>
            )}
          </button>
        </div>
      </div>
    </div>
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

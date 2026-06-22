import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import QRCode from 'qrcode.react';
import { user } from '@v2board/api-client';
import type { Order, PaymentMethod } from '@v2board/types';
import { apiClient } from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import {
  userKeys,
  useCommConfig,
  useOrder,
  usePaymentMethods,
  useCancelOrderMutation,
  useUserInfo,
} from '@/lib/queries';
import { legacyConfirm } from '@/components/legacy-confirm';
import { StripeCardForm } from '@/components/stripe-card-form';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { CheckCircleIcon, InfoCircleIcon, WarningIcon } from '@/components/ant-icon';
import { toast } from '@/lib/legacy-toast';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { formatLegacyDateTime } from '@v2board/config/format';

const PERIOD_LABEL_KEY: Record<string, string> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

export default function OrderDetailPage() {
  const { t } = useTranslation();
  const { trade_no } = useParams();
  const tradeNo = trade_no;
  const queryClient = useQueryClient();
  const orderQuery = useOrder(tradeNo);
  const paymentsQuery = usePaymentMethods({ enabled: Boolean(orderQuery.data) });
  // Old componentDidMount dispatches order/detail, then user/getUserInfo, then comm/config.
  useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const cancel = useCancelOrderMutation();
  const [methodId, setMethodId] = useState<number | undefined>();
  const [qrcodeVisible, setQrcodeVisible] = useState(false);
  const [payUrl, setPayUrl] = useState<string | undefined>();
  const [paying, setPaying] = useState(false);
  const [stripePk, setStripePk] = useState<string | null>(null);
  const [stripeToken, setStripeToken] = useState<{ id: string } | null>(null);
  const [preHandlingAmount, setPreHandlingAmount] = useState<number | undefined>();
  const symbol = comm?.currency_symbol;
  const currency = comm?.currency;
  const paymentMethods = orderQuery.data ? paymentsQuery.data : undefined;
  const hasLoadedOrder = Boolean(orderQuery.data);
  const loading = useLegacyFetchLoading(orderQuery.isFetching);

  // The original calls check() once from the order/detail fetch callback, regardless of the
  // loaded status: it polls /user/order/check every 3s while pending, and on a non-pending
  // result clears the timer, hides the QR modal and refetches the detail. A ref keeps this to
  // one start per trade_no so the refetch (which re-runs this effect) cannot restart the poll.
  const checkedRef = useRef<string | null>(null);
  const previousOrderStatusRef = useRef<{ tradeNo?: string; status?: number }>({});
  useEffect(() => {
    if (!tradeNo || !hasLoadedOrder) return;
    if (checkedRef.current === tradeNo) return;
    checkedRef.current = tradeNo;
    let cancelled = false;
    let timer = 0;
    const check = () => {
      timer = window.setTimeout(() => {
        user
          .checkOrder(apiClient, tradeNo)
          .then((status) => {
            if (cancelled) return;
            if (status !== 0) {
              setQrcodeVisible(false);
              // The original poll success only hides the QR modal; it leaves
              // payUrl in state. Manual modal cancel is the path that clears it.
              orderQuery.refetch();
            } else {
              check();
            }
          })
          .catch(() => {});
      }, 3000);
    };
    check();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [tradeNo, hasLoadedOrder]);

  useEffect(() => {
    const first = paymentMethods?.[0];
    if (methodId !== undefined || !first || !orderQuery.data) return;
    setMethodId(first.id);
    setPreHandlingAmount(calculatePreHandlingAmount(orderQuery.data, first));
  }, [methodId, orderQuery.data, paymentMethods]);

  useEffect(() => {
    const status = orderQuery.data?.status;
    const previous =
      previousOrderStatusRef.current.tradeNo === tradeNo
        ? previousOrderStatusRef.current.status
        : undefined;

    if (status !== 0) {
      setQrcodeVisible(false);
    }
    if (previous === 0 && status !== 0) {
      // The bundled poll success refetches order/detail without re-running
      // getPaymentMethod/changePaymentMethod, so the locally injected
      // pre_handling_amount disappears unless the fresh detail includes it.
      setPreHandlingAmount(undefined);
    }

    previousOrderStatusRef.current = { tradeNo, status };
  }, [orderQuery.data?.status, tradeNo]);

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: ['user', 'orders'] });
      if (tradeNo) queryClient.removeQueries({ queryKey: userKeys.orderDetail(tradeNo) });
      queryClient.removeQueries({ queryKey: userKeys.payments });
    },
    [queryClient, tradeNo],
  );

  const effectiveMethodId = methodId ?? paymentMethods?.[0]?.id;
  const selectedPayment = paymentMethods?.find((p) => p.id === effectiveMethodId);
  const isStripePayment = selectedPayment?.payment === 'StripeCredit';

  useEffect(() => {
    // The original only fetches the Stripe public key the first time a Stripe method is
    // selected (it guards on the existing key) and never resets it or the card token when
    // switching methods. So once a pk/token is captured it persists across method changes
    // — match that by fetching once and never clearing either piece of state.
    if (!isStripePayment || effectiveMethodId === undefined || stripePk) return;
    let cancelled = false;
    user
      .getStripePublicKey(apiClient, effectiveMethodId)
      .then((pk) => {
        if (!cancelled) setStripePk(pk);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [effectiveMethodId, isStripePayment, stripePk]);

  const handleStripeToken = useCallback((token: { id: string } | null) => {
    setStripeToken(token);
  }, []);

  if (loading) {
    return (
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
      </div>
    );
  }
  const order = (orderQuery.data ?? { plan: {} }) as Order;
  const isPending = order.status === 0;
  const isDeposit = order.plan?.id == 0;
  const periodLabelKey = order.period ? PERIOD_LABEL_KEY[order.period] : undefined;
  const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
  const legacyPreHandlingAmount =
    preHandlingAmount ??
    order.pre_handling_amount ??
    (methodId === undefined ? calculatePreHandlingAmount(order, selectedPayment) : 0);
  const grandTotal = order.total_amount + (legacyPreHandlingAmount || 0);

  const onPay = async () => {
    if (!tradeNo) return;
    if (isStripePayment && !stripeToken) {
      toast.error(t('order.credit_card_check'));
      return;
    }
    setPaying(true);
    let keepLegacyLoading = false;
    try {
      const result = await user.checkoutOrder(apiClient, {
        trade_no: tradeNo,
        method: effectiveMethodId as number,
        token: isStripePayment ? stripeToken?.id : undefined,
      });
      if (isStripePayment) {
        toast.loading('请稍等，我们正在验证该笔支付', { duration: 5000 });
        return;
      }
      if (result.type === 0) {
        setQrcodeVisible(true);
        setPayUrl(typeof result.data === 'string' ? result.data : undefined);
      } else if (result.type === 1 && typeof result.data === 'string') {
        window.location.href = result.data;
        toast.info('正在前往收银台');
      }
    } catch (error) {
      if (isLegacyCheckoutNetworkError(error)) {
        keepLegacyLoading = true;
      }
    } finally {
      if (!keepLegacyLoading) setPaying(false);
    }
  };

  const transferEnable =
    order.plan && 'transfer_enable' in order.plan && order.plan.transfer_enable != null
      ? order.plan.transfer_enable
      : null;

  const handleCancel = () => {
    const cancelTradeNo = order.trade_no;
    if (!cancelTradeNo) return;
    void legacyConfirm({
      title: t('common.attention'),
      content: t('order.cancel_confirm'),
      okText: t('order.cancel'),
      okButtonProps: { loading: cancel.isPending },
      onOk: () => {
        // Legacy order/cancel dispatches `fetch`, then `details` (plural). The
        // mutation starts the list refresh; the model has no `details` effect, so
        // the detail view is not refreshed here.
        void cancel.mutateAsync(cancelTradeNo).catch(() => {});
      },
    });
  };

  return (
    <>
      <div className="row" id="cashier">
        <div className={isPending ? 'col-md-8 col-sm-12' : 'col-12'}>
          {!isPending && <OrderResult status={order.status} />}

          <LegacyBlock title={t('order.product_info')} tradeTitle>
            <div className="v2board-order-info">
              {isDeposit ? (
                <InfoRow label={t('order.product_name')}>充值</InfoRow>
              ) : null}
              {!isDeposit && (
                <InfoRow label={t('order.product_traffic')}>
                  {transferEnable}
                  {' GB'}
                </InfoRow>
              )}
            </div>
          </LegacyBlock>
          <LegacyBlock
            title={t('order.info')}
            tradeTitle
            options={
              isPending ? (
                <button
                  disabled={cancel.isPending}
                  type="button"
                  className="btn btn-primary btn-sm btn-danger btn-rounded px-3"
                  onClick={handleCancel}
                >
                  {cancel.isPending && (
                    <div>
                      <LegacyLoadingIcon />
                    </div>
                  )}
                  {' '}
                  {t('order.cancel')}
                </button>
              ) : null
            }
          >
            <div className="v2board-order-info">
              <InfoRow label={t('order.trade_no')}>{order.trade_no}</InfoRow>
              {order.discount_amount ? (
                <InfoRow label={t('order.discount_amount')}>
                  {amountText(order.discount_amount)}
                </InfoRow>
              ) : null}
              {order.surplus_amount ? (
                <InfoRow label={t('order.surplus_used')}>
                  {amountText(order.surplus_amount)}
                </InfoRow>
              ) : null}
              {order.refund_amount ? (
                <InfoRow label={t('order.refund_amount')}>{amountText(order.refund_amount)}</InfoRow>
              ) : null}
              {order.balance_amount ? (
                <InfoRow label={t('order.balance_used')}>{amountText(order.balance_amount)}</InfoRow>
              ) : null}
              {legacyPreHandlingAmount ? (
                <InfoRow label={t('order.handling_fee')}>
                  {amountText(legacyPreHandlingAmount)}
                </InfoRow>
              ) : null}
              <InfoRow label={t('order.created_at')}>
                {formatLegacyDateTime(order.created_at)}
              </InfoRow>
            </div>
          </LegacyBlock>

          {isPending && (
            <>
              <div className="block block-rounded js-appear-enabled">
                <div className="block-header block-header-default">
                  <h3 className="block-title">{t('order.payment_method')}</h3>
                  <div className="block-options" />
                </div>
                <div className="block-content p-0">
                  {paymentMethods?.map((method) => (
                    <div
                      key={method.id}
                      className={`v2board-select ${effectiveMethodId === method.id ? 'active border-primary' : 'false'}`}
                      onClick={() => {
                        setMethodId(method.id);
                        setPreHandlingAmount(calculatePreHandlingAmount(order, method));
                      }}
                    >
                      <div style={{ flex: 1, paddingTop: 4 }}>
                        {/* antd v3 Radio: classNames(className, {'ant-radio-wrapper':true,
                            'ant-radio-wrapper-checked':checked}) — the passed className leads. */}
                        <label
                          className={`v2board-select-radio ant-radio-wrapper${effectiveMethodId === method.id ? ' ant-radio-wrapper-checked' : ''}`}
                        >
                          <span className={`ant-radio${effectiveMethodId === method.id ? ' ant-radio-checked' : ''}`}>
                            <input
                              type="radio"
                              className="ant-radio-input"
                              checked={effectiveMethodId === method.id}
                              onChange={() => {}}
                            />
                            <span className="ant-radio-inner" />
                          </span>
                        </label>
                        {method.name}
                      </div>
                      {method.icon && (
                        <div style={{ flex: 1, textAlign: 'right' }}>
                          <img height={30} src={method.icon} />
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>

              {isStripePayment && stripePk && (
                <>
                  <h3 className="font-w300 mt-5 mb-3">{t('order.credit_card_title')}</h3>
                  <StripeCardForm key={stripePk} publicKey={stripePk} onToken={handleStripeToken} />
                  <div style={{ fontSize: 12 }} className="mt-3 mb-5">
                    <i
                      className="fa fa-user-shield"
                      style={{ marginRight: 5, color: '#7cb305' }}
                    />
                    {t('order.credit_card_security')}
                  </div>
                </>
              )}
            </>
          )}
        </div>

        {isPending && (
          <div className="col-md-4 col-sm-12">
            <div
              // Original class string has a DOUBLE space after `block-rounded` (umi.js).
              className="block block-link-pop block-rounded  px-3 py-3 text-light"
              style={{ background: '#35383D' }}
            >
              <h5 className="text-light mb-3">{t('order.total')}</h5>

              {isDeposit ? (
                <div>
                  <div className="pt-3">
                    {t('order.deposit_bonus')}
                    <div className="text-right">{moneyText(order.bounus, symbol)}</div>
                  </div>
                </div>
              ) : null}

              {isDeposit ? (
                <div>
                  <div className="pt-3">
                    {t('order.deposit_received')}
                    <div className="text-right">{moneyText(order.get_amount, symbol)}</div>
                  </div>
                  <div
                    className="row no-gutters py-3"
                    style={{ borderBottom: '1px solid #646669' }}
                  />
                </div>
              ) : null}

              {!isDeposit && (
                <div
                  className="row no-gutters pb-3"
                  style={{ borderBottom: '1px solid #646669' }}
                >
                  <div className="col-8">
                    {/* Original renders `name, " x ", periodText` — the " x " is always
                        present even when the period label resolves to empty. */}
                    {order.plan?.name} x {periodLabel}
                  </div>
                  <div className="col-4 text-right">
                    {moneyText(
                      (order.plan as Record<string, number | null> | undefined)?.[
                        order.period as string
                      ],
                      symbol,
                    )}
                  </div>
                </div>
              )}

              {order.discount_amount ? (
                <AmountBlock label={t('order.discount')}>
                  {moneyText(order.discount_amount, symbol)}
                </AmountBlock>
              ) : null}
              {order.surplus_amount ? (
                <AmountBlock label={t('order.surplus')}>
                  {moneyText(order.surplus_amount, symbol)}
                </AmountBlock>
              ) : null}
              {order.refund_amount ? (
                <AmountBlock label={t('order.refund')}>
                  - {moneyText(order.refund_amount, symbol)}
                </AmountBlock>
              ) : null}
              {legacyPreHandlingAmount ? (
                <AmountBlock label={t('order.handling_fee')}>
                  + {(legacyPreHandlingAmount / 100).toFixed(2)}
                </AmountBlock>
              ) : null}

              <div className="pt-3" style={{ color: '#646669' }}>
                {t('order.grand_total')}
              </div>
              <h1 className="text-light mt-3 mb-3">
                {symbol} {(grandTotal / 100).toFixed(2)} {currency}
              </h1>
              <button
                type="button"
                className="btn btn-block btn-primary"
                disabled={paying || (isStripePayment && !stripeToken)}
                onClick={onPay}
              >
                {paying ? (
                  <LegacyLoadingIcon />
                ) : (
                  <span>
                    <i className="far fa-check-circle" /> {t('order.checkout')}
                  </span>
                )}
              </button>
            </div>
          </div>
        )}
      </div>

      <Dialog
        open={qrcodeVisible}
        onOpenChange={(open) => {
          if (!open) {
            setQrcodeVisible(false);
            setPayUrl(undefined);
          }
        }}
      >
        <DialogContent
          className="v2board-payment-qrcode"
          closable={false}
          maskClosable
          width={300}
          centered
          footer={<div style={{ textAlign: 'center' }}>{t('order.waiting_pay')}</div>}
        >
          {payUrl && (
            <QRCode
              value={payUrl}
              renderAs="svg"
              size="250"
            />
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}

function LegacyBlock({
  title,
  tradeTitle = false,
  options,
  children,
}: {
  title: string;
  tradeTitle?: boolean;
  options?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="block block-rounded">
      <div className="block-header block-header-default">
        <h3 className={`block-title${tradeTitle ? ' v2board-trade-no' : ''}`}>{title}</h3>
        {options ? <div className="block-options">{options}</div> : null}
      </div>
      <div className="block-content pb-4">{children}</div>
    </div>
  );
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <span>{label}：</span>
      <span>{children}</span>
    </div>
  );
}

function AmountBlock({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <div className="pt-3" style={{ color: '#646669' }}>
        {label}
      </div>
      <div className="row no-gutters py-3" style={{ borderBottom: '1px solid #646669' }}>
        <div className="col-8" />
        <div className="col-4 text-right">{children}</div>
      </div>
    </div>
  );
}

function amountText(cents: number) {
  return (cents / 100).toFixed(2);
}

function moneyText(cents: number | null | undefined, symbol?: string | null) {
  return (
    <>
      {symbol}
      {((cents as number) / 100).toFixed(2)}
    </>
  );
}

function calculatePreHandlingAmount(order: Order, method?: PaymentMethod) {
  return order.total_amount > 0 && (method?.handling_fee_fixed || method?.handling_fee_percent)
    ? order.total_amount * ((method.handling_fee_percent as number) / 100) +
        (method.handling_fee_fixed as number)
    : 0;
}

function isLegacyCheckoutNetworkError(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'status' in error &&
    (error as { status?: unknown }).status === 0
  );
}

function OrderResult({ status }: { status?: number }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const result =
    status === 1
      ? {
          icon: <InfoCircleIcon />,
          status: 'info',
          title: t('order.processing_title'),
          subtitle: t('order.processing'),
        }
      : status === 2
        ? {
            icon: <WarningIcon />,
            status: 'warning',
            title: t('common.cancelled'),
            subtitle: t('order.cancel_timeout'),
          }
        : status === 3 || status === 4
          ? {
              icon: <CheckCircleIcon />,
              status: 'success',
              title: t('common.completed'),
              subtitle: t('order.success'),
            }
          : {
              icon: <InfoCircleIcon />,
              status: 'info',
              title: '',
              subtitle: '',
            };

  return (
    <div className="block block-rounded">
      <div className="block-content pt-0">
        <div className={`ant-result ant-result-${result.status} py-4`}>
          <div className="ant-result-icon">
            {result.icon}
          </div>
          <div className="ant-result-title">{result.title}</div>
          {result.subtitle ? (
            <div className="ant-result-subtitle">{result.subtitle}</div>
          ) : null}
          {(status === 3 || status === 4) && (
            <div className="ant-result-extra">
              <button
                type="button"
                onClick={() => navigate('/knowledge')}
                className="btn btn-primary btn-sm btn-danger btn-rounded px-3"
              >
                <i className="nav-main-link-icon si si-book-open mr-1" />
                {t('order.view_tutorial')}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

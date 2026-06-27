import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import QRCode from 'qrcode.react';
import { user } from '@v2board/api-client';
import type { Order, PaymentMethod } from '@v2board/types';
import { BookOpen, CheckCircle2, Info, TriangleAlert } from 'lucide-react';
import { apiClient } from '@/lib/api';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
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
import { toast } from '@/lib/legacy-toast';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { formatUserLegacyDateTime } from '@/lib/legacy-date';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';

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
      <div className="flex min-h-44 items-center justify-center" role="status">
        <Spinner className="size-5" />
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
      <PageShell
        id="cashier"
        className={cn('grid max-w-6xl gap-6', isPending && 'lg:grid-cols-[minmax(0,1fr)_24rem]')}
      >
        <div className="space-y-6">
          {!isPending && <OrderResult status={order.status} />}

          <OrderInfoCard title={t('order.product_info')} tradeTitle>
            <div data-testid="order-info">
              {isDeposit ? (
                <InfoRow label={t('order.product_name')}>充值</InfoRow>
              ) : (
                <>
                  <InfoRow label={t('order.product_name')}>{order.plan?.name}</InfoRow>
                  <InfoRow label={t('order.product_period')}>{periodLabel}</InfoRow>
                  <InfoRow label={t('order.product_traffic')}>
                    {transferEnable}
                    {' GB'}
                  </InfoRow>
                </>
              )}
            </div>
          </OrderInfoCard>
          <OrderInfoCard
            title={t('order.info')}
            tradeTitle
            options={
              isPending ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  loading={cancel.isPending}
                  onClick={handleCancel}
                >
                  {t('order.cancel')}
                </Button>
              ) : null
            }
          >
            <div data-testid="order-info">
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
                {formatUserLegacyDateTime(order.created_at)}
              </InfoRow>
            </div>
          </OrderInfoCard>

          {isPending && (
            <>
              <Card>
                <CardHeader>
                  <CardTitle className="text-base leading-6">{t('order.payment_method')}</CardTitle>
                </CardHeader>
                <CardContent className="grid gap-3 p-6 pt-0">
                  {paymentMethods?.map((method) => (
                    <button
                      type="button"
                      key={method.id}
                      className={cn(
                        'flex min-h-12 items-center justify-between rounded-lg border border-border bg-background px-4 py-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
                        effectiveMethodId === method.id && 'border-primary bg-accent text-accent-foreground',
                      )}
                      data-testid="payment-option"
                      data-state={effectiveMethodId === method.id ? 'checked' : 'unchecked'}
                      onClick={() => {
                        setMethodId(method.id);
                        setPreHandlingAmount(calculatePreHandlingAmount(order, method));
                      }}
                    >
                      <div className="flex items-center gap-3">
                        <span
                          className={cn(
                            'flex size-4 items-center justify-center rounded-full border border-input',
                            effectiveMethodId === method.id && 'border-primary',
                          )}
                          data-testid="payment-option-radio"
                        >
                          <span
                            className={cn(
                              'size-2 rounded-full bg-primary opacity-0',
                              effectiveMethodId === method.id && 'opacity-100',
                            )}
                          />
                        </span>
                        {method.name}
                      </div>
                      {method.icon && (
                        <img className="h-7 w-auto" src={method.icon} alt="" />
                      )}
                    </button>
                  ))}
                </CardContent>
              </Card>

              {isStripePayment && stripePk && (
                <>
                  <h3 className="text-base font-semibold leading-6">{t('order.credit_card_title')}</h3>
                  <StripeCardForm key={stripePk} publicKey={stripePk} onToken={handleStripeToken} />
                  <div className="mt-3 mb-5 text-sm text-muted-foreground">
                    {t('order.credit_card_security')}
                  </div>
                </>
              )}
            </>
          )}
        </div>

        {isPending && (
          <aside data-testid="order-side">
            <Card data-testid="order-summary">
              <CardHeader>
                <CardTitle className="text-base leading-6">{t('order.total')}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {isDeposit ? (
                  <div className="space-y-2 text-sm">
                    <div className="flex items-center justify-between gap-4">
                      {t('order.deposit_bonus')}
                      <div className="text-right font-medium">{moneyText(order.bounus, symbol)}</div>
                    </div>
                  </div>
                ) : null}

                {isDeposit ? (
                  <div className="border-b border-border pb-4 text-sm">
                    <div className="flex items-center justify-between gap-4">
                      {t('order.deposit_received')}
                      <div className="text-right font-medium">{moneyText(order.get_amount, symbol)}</div>
                    </div>
                  </div>
                ) : null}

                {!isDeposit && (
                  <div className="flex items-start justify-between gap-4 border-b border-border pb-4 text-sm">
                    <div>
                      {order.plan?.name} x {periodLabel}
                    </div>
                    <div className="text-right font-medium">
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

                <div className="pt-2 text-sm text-muted-foreground">
                  {t('order.grand_total')}
                </div>
                <h1 className="text-3xl font-semibold tracking-normal">
                  {symbol} {(grandTotal / 100).toFixed(2)} {currency}
                </h1>
                <Button
                  type="button"
                  block
                  data-testid="commerce-submit"
                  loading={paying}
                  disabled={isStripePayment && !stripeToken}
                  onClick={onPay}
                >
                  {t('order.checkout')}
                </Button>
              </CardContent>
            </Card>
          </aside>
        )}
      </PageShell>

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
          className="w-[min(calc(100vw-2rem),20rem)]"
          data-testid="payment-qrcode"
          showCloseButton={false}
        >
          <DialogHeader className="sr-only">
            <DialogTitle>{t('order.checkout')}</DialogTitle>
            <DialogDescription>{t('order.waiting_pay')}</DialogDescription>
          </DialogHeader>
          {payUrl && (
            <div className="flex justify-center">
              <QRCode
                value={payUrl}
                renderAs="svg"
                size="250"
              />
            </div>
          )}
          <DialogFooter className="justify-center sm:justify-center">
            <p
              className="text-center text-sm text-muted-foreground"
              data-testid="payment-qrcode-status"
            >
              {t('order.waiting_pay')}
            </p>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function OrderInfoCard({
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
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-4">
        <CardTitle
          className={cn('text-base leading-6', tradeTitle && 'truncate')}
          data-testid="order-info-title"
        >
          {title}
        </CardTitle>
        {options ? <div>{options}</div> : null}
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  );
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 border-b border-border py-3 text-sm last:border-b-0 sm:flex-row sm:items-center sm:justify-between">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{children}</span>
    </div>
  );
}

function AmountBlock({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-2 border-b border-border pb-4 text-sm">
      <div className="text-muted-foreground">{label}</div>
      <div className="text-right font-medium">{children}</div>
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
          icon: <Info className="size-8" />,
          status: 'info',
          title: t('order.processing_title'),
          subtitle: t('order.processing'),
        }
      : status === 2
        ? {
            icon: <TriangleAlert className="size-8" />,
            status: 'warning',
            title: t('common.cancelled'),
            subtitle: t('order.cancel_timeout'),
          }
        : status === 3 || status === 4
          ? {
              icon: <CheckCircle2 className="size-8" />,
              status: 'success',
              title: t('common.completed'),
              subtitle: t('order.success'),
            }
          : {
              icon: <Info className="size-8" />,
              status: 'info',
              title: '',
              subtitle: '',
            };

  return (
    <Card data-result-status={result.status} data-testid="order-result">
      <CardContent className="flex flex-col items-center gap-4 py-8 text-center">
        <div
          className={cn(
            'rounded-full border p-3',
            result.status === 'success' && 'border-green-200 bg-green-50 text-green-700',
            result.status === 'warning' && 'border-yellow-200 bg-yellow-50 text-yellow-700',
            result.status === 'info' && 'border-blue-200 bg-blue-50 text-blue-700',
          )}
        >
          {result.icon}
        </div>
        <div className="space-y-1">
          <div className="text-lg font-semibold leading-7">{result.title}</div>
          {result.subtitle ? (
            <div className="text-sm text-muted-foreground">{result.subtitle}</div>
          ) : null}
        </div>
        {(status === 3 || status === 4) && (
          <Button type="button" onClick={() => navigate('/knowledge')}>
            <BookOpen className="size-4" />
            {t('order.view_tutorial')}
          </Button>
        )}
      </CardContent>
    </Card>
  );
}

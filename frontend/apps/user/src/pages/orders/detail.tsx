import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { QRCodeSVG } from 'qrcode.react';
import type { Order, PaymentMethod } from '@v2board/types';
import { BookOpen, CheckCircle2, Info, TriangleAlert } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import {
  useCheckoutOrderMutation,
  useCommConfig,
  useOrder,
  useOrderStatus,
  usePaymentMethods,
  useCancelOrderMutation,
  useStripePublicKey,
  useUserInfo,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { StripeCardForm } from '@/components/stripe-card-form';
import { toast } from '@/lib/toast';
import { formatUserLegacyDateTime } from '@/lib/legacy-date';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { RadioGroup, RadioGroupIndicator, RadioGroupItem } from '@/components/ui/radio-group';
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
  const orderQuery = useOrder(tradeNo);
  const paymentsQuery = usePaymentMethods({ enabled: Boolean(orderQuery.data) });
  // Old componentDidMount dispatches order/detail, then user/getUserInfo, then comm/config.
  useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const cancel = useCancelOrderMutation();
  const checkout = useCheckoutOrderMutation();
  const { mutateAsync: checkoutOrder } = checkout;
  const [methodId, setMethodId] = useState<number | undefined>();
  const [qrcodeVisible, setQrcodeVisible] = useState(false);
  const [payUrl, setPayUrl] = useState<string | undefined>();
  const [pollOrderStatus, setPollOrderStatus] = useState(false);
  const [stripeToken, setStripeToken] = useState<{ id: string } | null>(null);
  const orderStatusQuery = useOrderStatus(tradeNo, {
    enabled: pollOrderStatus,
    refetchInterval: pollOrderStatus ? 3000 : false,
  });
  const symbol = comm?.currency_symbol;
  const currency = comm?.currency;
  const paymentMethods = orderQuery.data ? paymentsQuery.data : undefined;
  const hasLoadedOrder = Boolean(orderQuery.data);
  const loading = orderQuery.isFetching;

  // The original waits 3s before starting /user/order/check and only starts once per trade_no.
  // After that first delay, TanStack Query owns the 3s refetch cadence.
  const checkedRef = useRef<string | null>(null);
  useEffect(() => {
    if (!tradeNo || !hasLoadedOrder) return;
    if (checkedRef.current === tradeNo) return;
    checkedRef.current = tradeNo;
    const timer = window.setTimeout(() => setPollOrderStatus(true), 3000);
    return () => {
      window.clearTimeout(timer);
      setPollOrderStatus(false);
    };
  }, [tradeNo, hasLoadedOrder]);

  useEffect(() => {
    if (orderStatusQuery.isError) setPollOrderStatus(false);
  }, [orderStatusQuery.isError]);

  useEffect(() => {
    const status = orderStatusQuery.data;
    if (status === undefined || status === 0) return;
    setPollOrderStatus(false);
    setQrcodeVisible(false);
    // The original poll success only hides the QR modal; it leaves payUrl in state.
    // Manual modal cancel is the path that clears it.
    orderQuery.refetch();
  }, [orderQuery.refetch, orderStatusQuery.data]);

  useEffect(() => {
    // The bundled poll success only hides the QR modal once the order leaves the
    // pending (status 0) state.
    if (orderQuery.data?.status !== 0) setQrcodeVisible(false);
  }, [orderQuery.data?.status]);

  const effectiveMethodId = methodId ?? paymentMethods?.[0]?.id;
  const selectedPayment = paymentMethods?.find((p) => p.id === effectiveMethodId);
  const isStripePayment = selectedPayment?.payment === 'StripeCredit';

  // The original only fetches the Stripe public key once a Stripe method is selected and
  // never refetches it, so cache it forever behind the selected method.
  const stripeQuery = useStripePublicKey(
    effectiveMethodId === undefined ? undefined : String(effectiveMethodId),
    { enabled: isStripePayment },
  );
  const stripePk = stripeQuery.data ?? null;

  // pre_handling_amount from the server wins; otherwise derive the fee from the selected
  // method. The bundled poll-success refetch replaces the order detail without re-running
  // getPaymentMethod, so a paid (non-pending) order has no locally injected fee.
  const effectivePreHandlingAmount = useMemo(() => {
    const currentOrder = orderQuery.data;
    if (!currentOrder) return 0;
    return (
      currentOrder.pre_handling_amount ??
      (currentOrder.status === 0 ? calculatePreHandlingAmount(currentOrder, selectedPayment) : 0)
    );
  }, [orderQuery.data, selectedPayment]);

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
  const grandTotal = order.total_amount + (effectivePreHandlingAmount || 0);

  const onPay = async () => {
    if (!tradeNo) return;
    if (isStripePayment && !stripeToken) {
      toast.error(t('order.credit_card_check'));
      return;
    }
    try {
      const result = await checkoutOrder({
        trade_no: tradeNo,
        method: effectiveMethodId as number,
        token: isStripePayment ? stripeToken?.id : undefined,
      });
      if (isStripePayment) {
        toast.loading(t('order.stripe_verifying'), { duration: 5000 });
        return;
      }
      if (result.type === 0) {
        setQrcodeVisible(true);
        setPayUrl(typeof result.data === 'string' ? result.data : undefined);
      } else if (result.type === 1 && typeof result.data === 'string') {
        window.location.href = result.data;
        toast.info(t('order.redirecting_checkout'));
      }
    } catch {
      // The mutation tracks its own error/pending state; swallow here to keep
      // the checkout button restored after a failed /payment request.
    }
  };

  const transferEnable =
    order.plan && 'transfer_enable' in order.plan && order.plan.transfer_enable != null
      ? order.plan.transfer_enable
      : null;

  const handleCancel = () => {
    const cancelTradeNo = order.trade_no;
    if (!cancelTradeNo) return;
    void confirmDialog({
      title: t('common.attention'),
      description: t('order.cancel_confirm'),
      confirmText: t('order.cancel'),
      confirmButtonProps: { loading: cancel.isPending },
      onConfirm: () => cancel.mutateAsync(cancelTradeNo),
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
                <InfoRow label={t('order.product_name')}>{t('order.deposit')}</InfoRow>
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
              {effectivePreHandlingAmount ? (
                <InfoRow label={t('order.handling_fee')}>
                  {amountText(effectivePreHandlingAmount)}
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
                <CardContent className="p-6 pt-0">
                  <RadioGroup
                    value={effectiveMethodId === undefined ? undefined : String(effectiveMethodId)}
                    onValueChange={(nextMethodId) => {
                      setMethodId(Number(nextMethodId));
                    }}
                  >
                    {paymentMethods?.map((method) => (
                      <RadioGroupItem
                        key={method.id}
                        value={String(method.id)}
                        data-testid="payment-option"
                      >
                        <div className="flex items-center gap-3">
                          <RadioGroupIndicator data-testid="payment-option-radio" />
                          {method.name}
                        </div>
                        {method.icon && (
                          <img className="h-7 w-auto" src={method.icon} alt="" />
                        )}
                      </RadioGroupItem>
                    ))}
                  </RadioGroup>
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
                {effectivePreHandlingAmount ? (
                  <AmountBlock label={t('order.handling_fee')}>
                    + {(effectivePreHandlingAmount / 100).toFixed(2)}
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
                  loading={checkout.isPending}
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
              <QRCodeSVG
                value={payUrl}
                size={250}
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

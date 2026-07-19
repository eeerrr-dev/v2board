import type { ReactNode } from 'react';
import type { SelectorParam } from 'i18next';
import { Link, useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { QRCodeSVG } from 'qrcode.react';
import { BookOpen, CheckCircle2, Info, TriangleAlert } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { StripePaymentForm } from '@/components/stripe-payment-form';
import { formatBackendDateTime } from '@v2board/config/format';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { EmptyState, PageShell } from '@/components/ui/page';
import { RadioGroup, RadioGroupIndicator, RadioGroupItem } from '@/components/ui/radio-group';
import { LoadingState, SkeletonLines } from '@/components/ui/loading-state';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/cn';
import { useOrderCheckoutController } from './use-order-checkout-controller';

const PERIOD_LABEL_KEY: Record<string, SelectorParam> = {
  month_price: ($) => $.plan.monthly,
  quarter_price: ($) => $.plan.quarterly,
  half_year_price: ($) => $.plan.half_year,
  year_price: ($) => $.plan.yearly,
  two_year_price: ($) => $.plan.two_year,
  three_year_price: ($) => $.plan.three_year,
  onetime_price: ($) => $.plan.onetime,
  reset_price: ($) => $.plan.reset,
};

export default function OrderDetailPage() {
  const { t } = useTranslation();
  const { trade_no } = useParams();
  const {
    order,
    isLoading,
    orderError,
    retryOrder,
    isPending,
    paymentMethods,
    paymentMethodsState,
    effectiveMethodId,
    canCheckout,
    selectMethod,
    isStripePayment,
    stripePaymentIntent,
    stripePreparation,
    stripePaymentRef,
    paymentComplete,
    setPaymentComplete,
    onPay,
    isCheckoutPending,
    qrcode,
    cancel,
    fee,
    currencySymbol: symbol,
    currency,
  } = useOrderCheckoutController(trade_no);

  if (isLoading) {
    return (
      <LoadingState className="min-h-44 py-6">
        <SkeletonLines lines={4} />
      </LoadingState>
    );
  }

  if (!order) {
    return (
      <PageShell className="max-w-3xl">
        <ErrorState
          data-testid="order-detail-error"
          message={orderError ?? undefined}
          onRetry={retryOrder}
        />
      </PageShell>
    );
  }

  const isDeposit = order.plan?.id == 0;
  const periodLabelKey = order.period ? PERIOD_LABEL_KEY[order.period] : undefined;
  const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
  const grandTotal = order.total_amount + (fee || 0);

  const transferEnable =
    order.plan && 'transfer_enable' in order.plan && order.plan.transfer_enable != null
      ? order.plan.transfer_enable
      : null;

  return (
    <>
      <PageShell
        id="cashier"
        className={cn(
          'grid max-w-6xl gap-6',
          isPending && '@3xl/main:grid-cols-[minmax(0,1fr)_24rem]',
        )}
      >
        <div className="space-y-6">
          {!isPending && <OrderResult status={order.status} />}

          <OrderInfoCard title={t(($) => $.order.product_info)} tradeTitle>
            <div data-testid="order-info">
              {isDeposit ? (
                <InfoRow label={t(($) => $.order.product_name)}>
                  {t(($) => $.order.deposit)}
                </InfoRow>
              ) : (
                <>
                  <InfoRow label={t(($) => $.order.product_name)}>{order.plan?.name}</InfoRow>
                  <InfoRow label={t(($) => $.order.product_period)}>{periodLabel}</InfoRow>
                  <InfoRow label={t(($) => $.order.product_traffic)}>
                    {transferEnable}
                    {' GB'}
                  </InfoRow>
                </>
              )}
            </div>
          </OrderInfoCard>
          <OrderInfoCard
            title={t(($) => $.order.info)}
            tradeTitle
            options={
              isPending ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  loading={cancel.isPending}
                  disabled={isCheckoutPending}
                  onClick={cancel.run}
                >
                  {t(($) => $.order.cancel)}
                </Button>
              ) : null
            }
          >
            <div data-testid="order-info">
              <InfoRow label={t(($) => $.order.trade_no)}>{order.trade_no}</InfoRow>
              {order.discount_amount ? (
                <InfoRow label={t(($) => $.order.discount_amount)}>
                  {amountText(order.discount_amount)}
                </InfoRow>
              ) : null}
              {order.surplus_amount ? (
                <InfoRow label={t(($) => $.order.surplus_used)}>
                  {amountText(order.surplus_amount)}
                </InfoRow>
              ) : null}
              {order.refund_amount ? (
                <InfoRow label={t(($) => $.order.refund_amount)}>
                  {amountText(order.refund_amount)}
                </InfoRow>
              ) : null}
              {order.balance_amount ? (
                <InfoRow label={t(($) => $.order.balance_used)}>
                  {amountText(order.balance_amount)}
                </InfoRow>
              ) : null}
              {fee ? (
                <InfoRow label={t(($) => $.order.handling_fee)}>{amountText(fee)}</InfoRow>
              ) : null}
              <InfoRow label={t(($) => $.order.created_at)}>
                {formatBackendDateTime(order.created_at)}
              </InfoRow>
            </div>
          </OrderInfoCard>

          {isPending && (
            <>
              <Card>
                <CardHeader>
                  <CardTitle className="text-base leading-6">
                    {t(($) => $.order.payment_method)}
                  </CardTitle>
                </CardHeader>
                <CardContent className="p-6 pt-0">
                  {paymentMethodsState.isPending ? (
                    <LoadingState className="min-h-24" data-testid="payment-methods-loading">
                      <div className="grid gap-3" aria-hidden>
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                      </div>
                    </LoadingState>
                  ) : paymentMethodsState.error ? (
                    <ErrorState
                      data-testid="payment-methods-error"
                      message={paymentMethodsState.error}
                      onRetry={paymentMethodsState.retry}
                    />
                  ) : paymentMethodsState.isEmpty ? (
                    <EmptyState
                      className="min-h-24"
                      data-testid="payment-methods-empty"
                      title={t(($) => $.common.empty)}
                    />
                  ) : (
                    <RadioGroup
                      disabled={isCheckoutPending}
                      value={
                        effectiveMethodId === undefined ? undefined : String(effectiveMethodId)
                      }
                      onValueChange={(nextMethodId) => {
                        selectMethod(Number(nextMethodId));
                      }}
                    >
                      {paymentMethods.map((method) => (
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
                            <img
                              className="h-7 w-auto"
                              src={method.icon}
                              alt=""
                              loading="lazy"
                              decoding="async"
                            />
                          )}
                        </RadioGroupItem>
                      ))}
                    </RadioGroup>
                  )}
                </CardContent>
              </Card>

              {isStripePayment && stripePreparation.isPending && (
                <LoadingState className="min-h-24">
                  <Skeleton className="h-24 w-full" aria-hidden />
                </LoadingState>
              )}

              {isStripePayment && stripePreparation.error && (
                <Alert variant="destructive">
                  <TriangleAlert />
                  <AlertDescription>
                    <span>{stripePreparation.error}</span>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={stripePreparation.retry}
                    >
                      {t(($) => $.common.retry)}
                    </Button>
                  </AlertDescription>
                </Alert>
              )}

              {isStripePayment && stripePaymentIntent && (
                <>
                  <h2 className="text-base leading-6 font-semibold text-foreground">
                    {t(($) => $.order.credit_card_title)}
                  </h2>
                  <StripePaymentForm
                    key={stripePaymentIntent.client_secret}
                    publicKey={stripePaymentIntent.public_key}
                    clientSecret={stripePaymentIntent.client_secret}
                    returnUrl={window.location.href}
                    ref={stripePaymentRef}
                    onCompleteChange={setPaymentComplete}
                  />
                  <div className="mt-3 mb-5 text-sm text-muted-foreground">
                    {t(($) => $.order.credit_card_security)}
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
                <CardTitle className="text-base leading-6">{t(($) => $.order.total)}</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {isDeposit ? (
                  <div className="space-y-2 text-sm">
                    <div className="flex items-center justify-between gap-4">
                      {t(($) => $.order.deposit_bonus)}
                      <div className="text-right font-medium">
                        {moneyText(order.bounus, symbol)}
                      </div>
                    </div>
                  </div>
                ) : null}

                {isDeposit ? (
                  <div className="border-b border-border pb-4 text-sm">
                    <div className="flex items-center justify-between gap-4">
                      {t(($) => $.order.deposit_received)}
                      <div className="text-right font-medium">
                        {moneyText(order.get_amount, symbol)}
                      </div>
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
                  <AmountBlock label={t(($) => $.order.discount)}>
                    {moneyText(order.discount_amount, symbol)}
                  </AmountBlock>
                ) : null}
                {order.surplus_amount ? (
                  <AmountBlock label={t(($) => $.order.surplus)}>
                    {moneyText(order.surplus_amount, symbol)}
                  </AmountBlock>
                ) : null}
                {order.refund_amount ? (
                  <AmountBlock label={t(($) => $.order.refund)}>
                    - {moneyText(order.refund_amount, symbol)}
                  </AmountBlock>
                ) : null}
                {fee ? (
                  <AmountBlock label={t(($) => $.order.handling_fee)}>
                    + {(fee / 100).toFixed(2)}
                  </AmountBlock>
                ) : null}

                <div className="pt-2 text-sm text-muted-foreground">
                  {t(($) => $.order.grand_total)}
                </div>
                <div className="text-3xl font-semibold tracking-normal text-card-foreground">
                  {symbol} {(grandTotal / 100).toFixed(2)} {currency}
                </div>
                <Button
                  type="button"
                  block
                  data-testid="commerce-submit"
                  loading={isCheckoutPending}
                  disabled={
                    !canCheckout || (isStripePayment && (!stripePaymentIntent || !paymentComplete))
                  }
                  onClick={onPay}
                >
                  {t(($) => $.order.checkout)}
                </Button>
              </CardContent>
            </Card>
          </aside>
        )}
      </PageShell>

      <Dialog
        open={qrcode.visible}
        onOpenChange={(open) => {
          if (!open) qrcode.close();
        }}
      >
        <DialogContent
          className="w-[min(calc(100vw-2rem),20rem)]"
          data-testid="payment-qrcode"
          showCloseButton={false}
        >
          <DialogHeader className="sr-only">
            <DialogTitle>{t(($) => $.order.checkout)}</DialogTitle>
            <DialogDescription>{t(($) => $.order.waiting_pay)}</DialogDescription>
          </DialogHeader>
          {qrcode.payUrl && (
            <div
              className="flex justify-center"
              role="img"
              aria-label={t(($) => $.common.scan_qrcode)}
            >
              <QRCodeSVG value={qrcode.payUrl} size={250} />
            </div>
          )}
          <DialogFooter className="justify-center sm:justify-center">
            <p
              className="text-center text-sm text-muted-foreground"
              data-testid="payment-qrcode-status"
            >
              {t(($) => $.order.waiting_pay)}
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

function OrderResult({ status }: { status?: number }) {
  const { t } = useTranslation();
  const result =
    status === 1
      ? {
          icon: <Info className="size-8" />,
          status: 'info',
          title: t(($) => $.order.processing_title),
          subtitle: t(($) => $.order.processing),
        }
      : status === 2
        ? {
            icon: <TriangleAlert className="size-8" />,
            status: 'warning',
            title: t(($) => $.common.cancelled),
            subtitle: t(($) => $.order.cancel_timeout),
          }
        : status === 3 || status === 4
          ? {
              icon: <CheckCircle2 className="size-8" />,
              status: 'success',
              title: t(($) => $.common.completed),
              subtitle: t(($) => $.order.success),
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
            result.status === 'success' && 'border-success/30 bg-success/10 text-success',
            result.status === 'warning' && 'border-warning/30 bg-warning/10 text-warning',
            result.status === 'info' && 'border-info/30 bg-info/10 text-info',
          )}
        >
          {result.icon}
        </div>
        <div className="space-y-1">
          <div className="text-lg leading-7 font-semibold">{result.title}</div>
          {result.subtitle ? (
            <div className="text-sm text-muted-foreground">{result.subtitle}</div>
          ) : null}
        </div>
        {(status === 3 || status === 4) && (
          <Button asChild>
            <Link to="/knowledge">
              <BookOpen className="size-4" />
              {t(($) => $.order.view_tutorial)}
            </Link>
          </Button>
        )}
      </CardContent>
    </Card>
  );
}

import { useCallback, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { QRCodeCanvas } from '@rc-component/qrcode';
import { user } from '@v2board/api-client';
import type { Order } from '@v2board/types';
import { apiClient } from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { useCommConfig, useOrder, usePaymentMethods, useCancelOrderMutation } from '@/lib/queries';
import { formatDateTime } from '@v2board/config/format';
import { legacyConfirm } from '@/components/legacy-confirm';
import { StripeCardForm } from '@/components/stripe-card-form';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { toast } from '@/lib/legacy-toast';

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
  const { tradeNo } = useParams();
  const orderQuery = useOrder(tradeNo);
  const paymentsQuery = usePaymentMethods({ enabled: Boolean(orderQuery.data) });
  const { data: comm } = useCommConfig();
  const cancel = useCancelOrderMutation();
  const [methodId, setMethodId] = useState<number | undefined>();
  const [payUrl, setPayUrl] = useState<string | null>(null);
  const [paying, setPaying] = useState(false);
  const [stripePk, setStripePk] = useState<string | null>(null);
  const [stripeToken, setStripeToken] = useState<{ id: string } | null>(null);
  const symbol = comm?.currency_symbol;
  const currency = comm?.currency;
  const paymentMethods = orderQuery.data ? paymentsQuery.data : undefined;

  useEffect(() => {
    if (!tradeNo) return;
    if (orderQuery.data?.status !== 0) return;
    const id = window.setInterval(() => {
      user
        .checkOrder(apiClient, tradeNo)
        .then((status) => {
          if (status !== 0) {
            orderQuery.refetch();
            window.clearInterval(id);
          }
        })
        .catch(() => {});
    }, 3000);
    return () => window.clearInterval(id);
  }, [tradeNo, orderQuery]);

  useEffect(() => {
    const first = paymentMethods?.[0];
    if (methodId !== undefined || !first) return;
    setMethodId(first.id);
  }, [methodId, paymentMethods]);

  useEffect(() => {
    if (orderQuery.data?.status !== 0) setPayUrl(null);
  }, [orderQuery.data?.status, tradeNo]);

  const selectedPayment = paymentMethods?.find((p) => p.id === methodId);
  const isStripePayment = selectedPayment?.payment === 'StripeCredit';

  useEffect(() => {
    setStripeToken(null);
    if (methodId === undefined || !isStripePayment) {
      setStripePk(null);
      return;
    }
    let cancelled = false;
    user
      .getStripePublicKey(apiClient, methodId)
      .then((pk) => {
        if (!cancelled) setStripePk(pk);
      })
      .catch(() => {
        if (!cancelled) setStripePk(null);
      });
    return () => {
      cancelled = true;
    };
  }, [isStripePayment, methodId]);

  const handleStripeToken = useCallback((token: { id: string } | null) => {
    setStripeToken(token);
  }, []);

  if (orderQuery.isFetching) {
    return (
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
      </div>
    );
  }
  if (orderQuery.error || !orderQuery.data) {
    return (
      <div className="spinner-grow text-primary" role="status">
        <span className="sr-only">Loading...</span>
      </div>
    );
  }
  const order = orderQuery.data;
  const isPending = order.status === 0;
  const isDeposit = order.period === 'deposit' || order.plan?.id === 0;
  const periodLabel = order.period ? t(PERIOD_LABEL_KEY[order.period] ?? '') : '';
  const handlingFee =
    isPending &&
    selectedPayment &&
    order.total_amount > 0 &&
    ((selectedPayment.handling_fee_fixed ?? 0) || (selectedPayment.handling_fee_percent ?? 0))
      ? (() => {
          const percent = selectedPayment.handling_fee_percent ?? 0;
          const fixed = selectedPayment.handling_fee_fixed ?? 0;
          return order.total_amount * (percent / 100) + fixed;
        })()
      : 0;
  const grandTotal = order.total_amount + handlingFee;

  const onPay = async () => {
    if (!tradeNo) return;
    if (isStripePayment && !stripeToken) {
      toast.error(t('order.credit_card_check'));
      return;
    }
    setPaying(true);
    try {
      const result = await user.checkoutOrder(apiClient, {
        trade_no: tradeNo,
        method: methodId as number,
        token: isStripePayment ? stripeToken?.id : undefined,
      });
      if (isStripePayment) {
        toast.loading(t('order.stripe_verifying'), { duration: 5000 });
        return;
      }
      if (result.type === 0 && typeof result.data === 'string') {
        setPayUrl(result.data);
      } else if (result.type === 1 && typeof result.data === 'string') {
        window.location.href = result.data;
        toast.info('正在前往收银台');
      }
    } catch {
    } finally {
      setPaying(false);
    }
  };

  const planName = isDeposit ? t('order.deposit') : (order.plan?.name ?? '');
  const transferEnable =
    order.plan && 'transfer_enable' in order.plan && order.plan.transfer_enable != null
      ? order.plan.transfer_enable
      : null;

  const handleCancel = async () => {
    if (!tradeNo) return;
    const ok = await legacyConfirm({
      title: t('common.attention'),
      content: t('order.cancel_confirm'),
      okText: t('order.cancel'),
    });
    if (!ok) return;
    try {
      await cancel.mutateAsync(tradeNo);
    } catch {}
  };

  return (
    <>
      <div className="row" id="cashier">
        <div className={isPending ? 'col-md-8 col-sm-12' : 'col-12'}>
          {!isPending && <OrderResult status={order.status} />}

          <LegacyBlock title={t('order.product_info')} tradeTitle>
            <div className="v2board-order-info">
              <InfoRow label={t('order.product_name')}>{planName}</InfoRow>
              {!isDeposit && (
                <>
                  <InfoRow label={t('order.product_period')}>{periodLabel}</InfoRow>
                  <InfoRow label={t('order.product_traffic')}>
                    {transferEnable == null ? '' : `${transferEnable} GB`}
                  </InfoRow>
                </>
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
                  {cancel.isPending && <LegacyLoadingIcon className="mr-1" />}
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
              {handlingFee ? (
                <InfoRow label={t('order.handling_fee')}>{amountText(handlingFee)}</InfoRow>
              ) : null}
              <InfoRow label={t('order.created_at')}>{formatDateTime(order.created_at)}</InfoRow>
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
                      className={`v2board-select ${methodId === method.id ? 'active border-primary' : ''}`}
                      onClick={() => setMethodId(method.id)}
                    >
                      <div style={{ flex: 1, paddingTop: 4 }}>
                        <span className="v2board-select-radio">
                          <span className="ant-radio">
                            <span
                              className={`ant-radio-inner${methodId === method.id ? ' ant-radio-inner-checked' : ''}`}
                            />
                          </span>
                        </span>
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
                  <StripeCardForm publicKey={stripePk} onToken={handleStripeToken} />
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
              className="block block-link-pop block-rounded px-3 py-3 text-light"
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
                    {order.plan?.name} {periodLabel ? `x ${periodLabel}` : ''}
                  </div>
                  <div className="col-4 text-right">
                    {moneyText(getPlanPeriodPrice(order), symbol)}
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
              {handlingFee ? (
                <AmountBlock label={t('order.handling_fee')}>
                  + {(handlingFee / 100).toFixed(2)}
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

      <Dialog open={Boolean(payUrl)} onOpenChange={(open) => !open && setPayUrl(null)}>
        <DialogContent
          className="v2board-payment-qrcode v2board-qrcode-dialog"
          showClose={false}
          centered
        >
          <div className="ant-modal-body">
            {payUrl && <QRCodeCanvas value={payUrl} size={250} />}
          </div>
          <div className="ant-modal-footer">
            <div style={{ textAlign: 'center' }}>{t('order.waiting_pay')}</div>
          </div>
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

function getPlanPeriodPrice(order: Order) {
  const plan = order.plan as Record<string, unknown> | undefined;
  const raw = order.period ? plan?.[order.period] : null;
  return typeof raw === 'number' ? raw : order.total_amount;
}

function OrderResult({ status }: { status: number }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const result =
    status === 1
      ? {
          icon: 'anticon anticon-info-circle',
          status: 'info',
          title: t('order.processing_title'),
          subtitle: t('order.processing'),
        }
      : status === 2
        ? {
            icon: 'anticon anticon-warning',
            status: 'warning',
            title: t('common.cancelled'),
            subtitle: t('order.cancel_timeout'),
          }
        : {
            icon: 'anticon anticon-check-circle',
            status: 'success',
            title: t('common.completed'),
            subtitle: t('order.success'),
          };

  return (
    <div className="block block-rounded">
      <div className="block-content pt-0">
        <div className={`ant-result ant-result-${result.status} py-4`}>
          <div className="ant-result-icon">
            <i className={result.icon} />
          </div>
          <div className="ant-result-title">{result.title}</div>
          <div className="ant-result-subtitle">{result.subtitle}</div>
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

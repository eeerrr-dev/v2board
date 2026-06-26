import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useOrders, useCancelOrderMutation } from '@/lib/queries';
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import { legacyConfirm } from '@/components/legacy-confirm';
import { legacyHref } from '@/lib/legacy-href';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';

const STATUS_LABEL: Record<number, { key: string; status: string }> = {
  0: { key: 'order.status_unpaid', status: 'error' },
  1: { key: 'order.status_processing', status: 'processing' },
  2: { key: 'order.status_cancelled', status: 'default' },
  3: { key: 'order.status_completed', status: 'success' },
  4: { key: 'order.status_credit', status: 'default' },
};

const PERIOD_LABEL: Record<string, string> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

export default function OrdersPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const ordersQuery = useOrders();
  const { data, isFetching } = ordersQuery;
  const loading = useLegacyFetchLoading(isFetching, ordersQuery.error);
  const cancel = useCancelOrderMutation();
  const orders = data ?? [];

  const onCancelOrder = (tradeNo: string) => {
    void legacyConfirm({
      title: t('common.attention'),
      content: t('order.cancel_confirm'),
      okText: t('order.cancel'),
      okButtonProps: { loading: cancel.isPending },
      onOk: () => {
        void cancel.mutateAsync(tradeNo).catch(() => {});
      },
    });
  };

  return (
    <Card className="v2board-orders-card">
      <CardContent className="p-0">
        {loading ? (
          <div className="flex items-center gap-2 border-b border-border px-4 py-3 text-sm text-muted-foreground">
            <Spinner className="size-4" />
            <span>{t('common.loading')}</span>
          </div>
        ) : null}

        {orders.length === 0 ? (
          <div className="v2board-orders-empty flex min-h-44 items-center justify-center px-6 text-sm text-muted-foreground">
            {t('order.no_orders')}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="v2board-orders-table w-full min-w-[760px] text-sm">
              <thead className="border-b border-border bg-muted/50 text-muted-foreground">
                <tr>
                  <th className="px-4 py-3 text-left font-medium">
                    <span className="ant-table-column-title">{t('order.trade_no_col')}</span>
                  </th>
                  <th className="px-4 py-3 text-left font-medium">
                    <span className="ant-table-column-title">{t('order.period')}</span>
                  </th>
                  <th className="px-4 py-3 text-right font-medium">
                    <span className="ant-table-column-title">{t('order.amount')}</span>
                  </th>
                  <th className="px-4 py-3 text-left font-medium">
                    <span className="ant-table-column-title">{t('order.status')}</span>
                  </th>
                  <th className="px-4 py-3 text-left font-medium">
                    <span className="ant-table-column-title">{t('order.created_at')}</span>
                  </th>
                  <th className="px-4 py-3 text-right font-medium">
                    <span className="ant-table-column-title">{t('order.action_col')}</span>
                  </th>
                </tr>
              </thead>
              <tbody className="ant-table-tbody divide-y divide-border">
                {orders.map((order) => {
                  const status = STATUS_LABEL[order.status];
                  const periodLabelKey = order.period ? PERIOD_LABEL[order.period] : undefined;
                  const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
                  return (
                    <tr key={order.trade_no} className="transition-colors hover:bg-muted/50">
                      <td className="px-4 py-4">
                        <a
                          ref={legacyHref()}
                          className="font-medium text-foreground underline-offset-4 hover:underline"
                          onClick={() => navigate(`/order/${order.trade_no}`)}
                        >
                          {order.trade_no}
                        </a>
                      </td>
                      <td className="px-4 py-4">
                        <span className="rounded-md border border-border bg-secondary px-2 py-1 text-xs font-medium text-secondary-foreground">
                          {periodLabel}
                        </span>
                      </td>
                      <td className="px-4 py-4 text-right font-medium">
                        {(order.total_amount / 100).toFixed(2)}
                      </td>
                      <td className="px-4 py-4">
                        <StatusPill status={status?.status}>{status ? t(status.key) : ''}</StatusPill>
                      </td>
                      <td className="px-4 py-4 text-muted-foreground">
                        {formatUserLegacyDateMinuteSlash(order.created_at)}
                      </td>
                      <td className="px-4 py-4 text-right">
                        <div className="flex justify-end gap-2">
                          <Button asChild variant="ghost" size="sm">
                            <a
                              ref={legacyHref()}
                              onClick={() => navigate(`/order/${order.trade_no}`)}
                            >
                              {t('order.return')}
                            </a>
                          </Button>
                          <Button asChild variant="ghost" size="sm" disabled={order.status !== 0}>
                            <a
                              ref={legacyHref()}
                              aria-disabled={order.status !== 0}
                              onClick={(event) => {
                                if (order.status !== 0) {
                                  event.preventDefault();
                                  return;
                                }
                                void onCancelOrder(order.trade_no);
                              }}
                            >
                              {t('common.cancel')}
                            </a>
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function StatusPill({ status, children }: { status?: string; children: string }) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-md border px-2 py-1 text-xs font-medium',
        status === 'success' && 'border-green-200 bg-green-50 text-green-700',
        status === 'processing' && 'border-blue-200 bg-blue-50 text-blue-700',
        status === 'error' && 'border-destructive/20 bg-destructive/10 text-destructive',
        (!status || status === 'default') && 'border-border bg-secondary text-secondary-foreground',
      )}
    >
      {children}
    </span>
  );
}

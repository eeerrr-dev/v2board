import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useOrders, useCancelOrderMutation } from '@/lib/queries';
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import { legacyConfirm } from '@/components/legacy-confirm';
import { legacyHref } from '@/lib/legacy-href';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import {
  DataTable,
  TableCell,
  TableRow,
} from '@/components/ui/table';

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
    <PageShell data-testid="orders-page">
      <Card className="overflow-hidden py-0" data-testid="orders-card">
        <CardContent className="p-0">
          {loading ? (
            <div className="flex items-center gap-2 border-b border-border px-4 py-3 text-sm text-muted-foreground">
              <Spinner className="size-4" />
              <span>{t('common.loading')}</span>
            </div>
          ) : null}

          {orders.length === 0 ? (
            <div
              className="flex min-h-44 items-center justify-center px-6 text-sm text-muted-foreground"
              data-testid="orders-empty"
            >
              {t('order.no_orders')}
            </div>
          ) : (
            <DataTable
              className="min-w-[760px]"
              data-testid="orders-table"
              headers={[
                { content: t('order.trade_no_col') },
                { content: t('order.period') },
                { align: 'right', content: t('order.amount') },
                { content: t('order.status') },
                { content: t('order.created_at') },
                { align: 'right', content: t('order.action_col') },
              ]}
            >
              {orders.map((order) => {
                const status = STATUS_LABEL[order.status];
                const periodLabelKey = order.period ? PERIOD_LABEL[order.period] : undefined;
                const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
                return (
                  <TableRow key={order.trade_no}>
                    <TableCell>
                      <a
                        ref={legacyHref()}
                        className="font-medium text-foreground underline-offset-4 hover:underline"
                        onClick={() => navigate(`/order/${order.trade_no}`)}
                      >
                        {order.trade_no}
                      </a>
                    </TableCell>
                    <TableCell>
                      <StatusBadge>{periodLabel}</StatusBadge>
                    </TableCell>
                    <TableCell className="text-right font-medium">
                      {(order.total_amount / 100).toFixed(2)}
                    </TableCell>
                    <TableCell>
                      <StatusPill status={status?.status}>{status ? t(status.key) : ''}</StatusPill>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatUserLegacyDateMinuteSlash(order.created_at)}
                    </TableCell>
                    <TableCell className="text-right">
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
                    </TableCell>
                  </TableRow>
                );
              })}
            </DataTable>
          )}
        </CardContent>
      </Card>
    </PageShell>
  );
}

function StatusPill({ status, children }: { status?: string; children: string }) {
  const tone: StatusTone =
    status === 'success'
      ? 'success'
      : status === 'processing'
        ? 'info'
        : status === 'error'
          ? 'destructive'
          : 'default';
  return (
    <StatusBadge tone={tone} showDot>
      {children}
    </StatusBadge>
  );
}

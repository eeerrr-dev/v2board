import type { SelectorParam } from 'i18next';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router';
import { useOrders, useCancelOrderMutation } from '@/lib/queries';
import { PLAN_PERIOD_LABELS } from '@/lib/plan-periods';
import { formatBackendDateMinuteSlash, formatCentsPlain } from '@v2board/config/format';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';

const STATUS_LABEL: Record<number, { key: SelectorParam; status: string }> = {
  0: { key: ($) => $.order.status_unpaid, status: 'error' },
  1: { key: ($) => $.order.status_processing, status: 'processing' },
  2: { key: ($) => $.order.status_cancelled, status: 'default' },
  3: { key: ($) => $.order.status_completed, status: 'success' },
  4: { key: ($) => $.order.status_credit, status: 'default' },
};

// Rebuilt from the canonical lib/plan-periods table (plan-periods.test.ts pins
// the derivation). Typed as Record<string, …> so an order's `period` union
// member outside the plan-period keys (e.g. 'deposit') indexes safely.
const PERIOD_LABEL: Record<string, SelectorParam> = PLAN_PERIOD_LABELS;

export default function OrdersPage() {
  const { t } = useTranslation();
  const ordersQuery = useOrders();
  const { data, isFetching, isError, refetch } = ordersQuery;
  const loading = isFetching;
  const cancel = useCancelOrderMutation();
  const orders = data ?? [];
  const orderColumns = [
    {
      header: t(($) => $.order.trade_no_col),
      cell: ({ row }) => (
        <Link
          to={`/order/${row.original.trade_no}`}
          className="text-left font-medium text-foreground underline-offset-4 hover:underline focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
        >
          {row.original.trade_no}
        </Link>
      ),
    },
    {
      header: t(($) => $.order.period),
      cell: ({ row }) => {
        const periodLabelKey = row.original.period ? PERIOD_LABEL[row.original.period] : undefined;
        const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
        return <StatusBadge>{periodLabel}</StatusBadge>;
      },
    },
    {
      accessorKey: 'total_amount',
      sortingFn: 'basic',
      meta: { align: 'right', className: 'font-medium' },
      header: t(($) => $.order.amount),
      cell: ({ row }) => formatCentsPlain(row.original.total_amount),
    },
    {
      header: t(($) => $.order.status),
      cell: ({ row }) => {
        const status = STATUS_LABEL[row.original.status];
        return <StatusPill status={status?.status}>{status ? t(status.key) : ''}</StatusPill>;
      },
    },
    {
      accessorKey: 'created_at',
      sortingFn: 'basic',
      meta: { className: 'text-muted-foreground' },
      header: t(($) => $.order.created_at),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
    },
    {
      meta: { align: 'right' },
      header: t(($) => $.order.action_col),
      cell: ({ row }) => (
        <div className="flex justify-end gap-2">
          <Button asChild variant="ghost" size="sm">
            <Link to={`/order/${row.original.trade_no}`}>{t(($) => $.order.return)}</Link>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={row.original.status !== 0}
            onClick={() => void onCancelOrder(row.original.trade_no)}
          >
            {t(($) => $.common.cancel)}
          </Button>
        </div>
      ),
    },
  ] satisfies DataTableColumn<(typeof orders)[number]>[];

  const onCancelOrder = (tradeNo: string) => {
    void confirmDialog({
      title: t(($) => $.common.attention),
      description: t(($) => $.order.cancel_confirm),
      confirmText: t(($) => $.order.cancel),
      confirmButtonProps: { loading: cancel.isPending },
      onConfirm: () => cancel.mutateAsync(tradeNo),
    });
  };

  return (
    <PageShell data-testid="orders-page">
      <Card className="overflow-hidden py-0" data-testid="orders-card">
        <CardContent className="p-0">
          {loading ? (
            <LoadingState className="border-b border-border px-4 py-3">
              <SkeletonRows rows={3} />
            </LoadingState>
          ) : null}

          {isError ? (
            // A failed fetch must not fall through to the empty state below —
            // that would wrongly tell the user they have no orders. Surface the
            // error with a retry instead.
            <div className="p-4">
              <ErrorState onRetry={() => void refetch()} data-testid="orders-error" />
            </div>
          ) : orders.length === 0 ? (
            <div
              className="flex min-h-44 items-center justify-center px-6 text-sm text-muted-foreground"
              data-testid="orders-empty"
            >
              {t(($) => $.order.no_orders)}
            </div>
          ) : (
            <DataTable
              className="min-w-[760px]"
              columns={orderColumns}
              data={orders}
              data-testid="orders-table"
              getRowKey={(order) => order.trade_no}
              virtualizer={{ enabled: orders.length > VIRTUALIZE_MIN_ROWS }}
            />
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

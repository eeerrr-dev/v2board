import { useTranslation } from 'react-i18next';
import { formatBackendDateSlash, formatBytes } from '@v2board/config/format';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { HeaderTooltip } from '@/components/ui/header-tooltip';
import { PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useTrafficLog } from '@/lib/queries';
import { useEmptyDescription } from '@/lib/use-empty-description';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';

export default function TrafficPage() {
  const { t } = useTranslation();
  const trafficQuery = useTrafficLog();
  // Key the loading banner on isPending (no data yet), not isFetching, so
  // cached rows keep rendering quietly during background refetches.
  const { data, isPending, isError, refetch } = trafficQuery;
  const rows = data ?? [];
  const { bodyRef, onScroll, scrollPosition } = useTableScrollPosition(rows.length);
  const emptyDescription = useEmptyDescription();
  const trafficColumns = [
    {
      accessorKey: 'record_at',
      // RFC 3339 UTC strings sort lexicographically in chronological order;
      // keep the newest-first first click the numeric column used to get.
      sortingFn: 'basic',
      sortDescFirst: true,
      header: t(($) => $.traffic.record_at),
      cell: ({ row }) => formatBackendDateSlash(row.original.record_at),
    },
    {
      meta: { align: 'right' },
      header: t(($) => $.traffic.actual_upload),
      cell: ({ row }) => formatBytes(row.original.u),
    },
    {
      meta: { align: 'right' },
      header: t(($) => $.traffic.actual_download),
      cell: ({ row }) => formatBytes(row.original.d),
    },
    {
      meta: { align: 'center' },
      header: t(($) => $.traffic.deduct_rate),
      cell: ({ row }) => {
        const rate = row.original.server_rate;
        return <StatusBadge>{rate ? `${rate.toFixed(2)} x` : '-'}</StatusBadge>;
      },
    },
    {
      id: 'total-charged',
      meta: { align: 'right', className: 'font-medium' },
      header: () => (
        <HeaderTooltip
          className="justify-end"
          placement="topRight"
          title={t(($) => $.traffic.total_formula)}
        >
          {t(($) => $.traffic.total_charged)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => {
        // Charged math (u+d)*server_rate — server_rate is a JSON number on
        // the modern wire (docs/api-dialect.md §5.4, W6).
        const charged = (row.original.u + row.original.d) * row.original.server_rate;
        return formatBytes(charged);
      },
    },
  ] satisfies DataTableColumn<(typeof rows)[number]>[];

  return (
    <TooltipProvider delayDuration={100}>
      <PageShell data-testid="traffic-page">
        <Card className="overflow-hidden py-0" data-testid="traffic-card">
          <CardContent className="p-0">
            <div className="border-b border-border bg-card p-4">
              <Alert className="bg-muted/40" data-testid="traffic-notice">
                <AlertDescription>{t(($) => $.traffic.notice)}</AlertDescription>
              </Alert>
            </div>

            {isPending ? (
              <LoadingState className="border-b border-border px-4 py-3">
                <SkeletonRows rows={3} />
              </LoadingState>
            ) : null}

            {isError ? (
              // A failed fetch must not render as an empty traffic table (which
              // reads as "no usage"); show the error with a retry instead.
              <div className="p-4">
                <ErrorState onRetry={() => void refetch()} data-testid="traffic-error" />
              </div>
            ) : (
              <DataTable
                className="min-w-[800px]"
                columns={trafficColumns}
                data={rows}
                data-table-kind="service"
                data-testid="traffic-table"
                empty={data !== undefined && rows.length === 0 ? emptyDescription : undefined}
                emptyClassName="py-16"
                emptyTestId="traffic-empty"
                scrollRef={bodyRef}
                scrollProps={{
                  tabIndex: 0,
                  role: 'region',
                  'aria-label': t(($) => $.nav.traffic),
                  'data-scroll-position': scrollPosition,
                  'data-testid': 'service-table-scroll',
                  onScroll,
                }}
                virtualizer={{ enabled: rows.length > VIRTUALIZE_MIN_ROWS }}
              />
            )}
          </CardContent>
        </Card>
      </PageShell>
    </TooltipProvider>
  );
}

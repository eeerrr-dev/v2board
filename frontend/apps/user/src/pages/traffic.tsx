import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { CircleHelp } from 'lucide-react';
import { formatBytes } from '@v2board/config/format';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { useTrafficLog } from '@/lib/queries';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { formatUserLegacyDateSlash } from '@/lib/legacy-date';

export default function TrafficPage() {
  const { t, i18n } = useTranslation();
  const trafficQuery = useTrafficLog();
  const { data, isFetching } = trafficQuery;
  const loading = useLegacyFetchLoading(isFetching, trafficQuery.error);
  const rows = data ?? [];
  const { bodyRef, onScroll, scrollPosition } = useTableScrollPosition(rows.length);
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;
  const trafficColumns = [
    {
      header: t('traffic.record_at'),
      cell: ({ row }) =>
        row.original.record_at ? formatUserLegacyDateSlash(row.original.record_at) : '-',
    },
    {
      meta: { align: 'right' },
      header: t('traffic.actual_upload'),
      cell: ({ row }) => {
        const upload = parseInt(String(row.original.u));
        return row.original.server_rate ? formatBytes(upload) : 0;
      },
    },
    {
      meta: { align: 'right' },
      header: t('traffic.actual_download'),
      cell: ({ row }) => {
        const download = parseInt(String(row.original.d));
        return row.original.server_rate ? formatBytes(download) : 0;
      },
    },
    {
      meta: { align: 'center' },
      header: t('traffic.deduct_rate'),
      cell: ({ row }) => {
        const rate = Number.parseFloat(row.original.server_rate);
        return <StatusBadge>{rate ? `${rate.toFixed(2)} x` : '-'}</StatusBadge>;
      },
    },
    {
      id: 'total-charged',
      meta: { align: 'right', className: 'font-medium' },
      header: () => (
        <HeaderTooltip title={t('traffic.total_formula')} placement="topRight">
          {t('traffic.total_charged')}
        </HeaderTooltip>
      ),
      cell: ({ row }) => {
        const upload = parseInt(String(row.original.u));
        const download = parseInt(String(row.original.d));
        const charged = (upload + download) * (row.original.server_rate as unknown as number);
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
                <AlertDescription>{t('traffic.notice')}</AlertDescription>
              </Alert>
            </div>

            {loading ? (
              <div
                className="flex items-center gap-2 border-b border-border px-4 py-3 text-sm text-muted-foreground"
                role="status"
              >
                <Spinner className="size-4" />
                <span>Loading...</span>
              </div>
            ) : null}

            <DataTable
              className="min-w-[800px]"
              columns={trafficColumns}
              data={rows}
              data-table-kind="service"
              data-testid="traffic-table"
              empty={!rows.length ? emptyDescription : undefined}
              emptyClassName="py-16"
              emptyTestId="traffic-empty"
              scrollRef={bodyRef}
              scrollProps={{
                tabIndex: -1,
                'data-scroll-position': scrollPosition,
                'data-testid': 'service-table-scroll',
                onScroll,
              }}
            />
          </CardContent>
        </Card>
      </PageShell>
    </TooltipProvider>
  );
}

function HeaderTooltip({
  children,
  placement = 'top',
  title,
}: {
  children: ReactNode;
  placement?: 'top' | 'topRight';
  title: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="v2board-service-tooltip-trigger inline-flex cursor-help items-center justify-end gap-1">
          {children}
          <CircleHelp className="size-3.5" />
        </span>
      </TooltipTrigger>
      <TooltipContent placement={placement}>{title}</TooltipContent>
    </Tooltip>
  );
}

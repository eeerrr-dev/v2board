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
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from '@/components/ui/table';
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

            <TableScroll
              ref={bodyRef}
              tabIndex={-1}
              data-scroll-position={scrollPosition}
              data-testid="service-table-scroll"
              onScroll={onScroll}
            >
              <Table className="min-w-[800px]" data-table-kind="service" data-testid="traffic-table">
                <TableHeader>
                  <tr>
                    <TableHead>{t('traffic.record_at')}</TableHead>
                    <TableHead className="text-right">{t('traffic.actual_upload')}</TableHead>
                    <TableHead className="text-right">
                      {t('traffic.actual_download')}
                    </TableHead>
                    <TableHead className="text-center">{t('traffic.deduct_rate')}</TableHead>
                    <TableHead className="text-right">
                      <HeaderTooltip title={t('traffic.total_formula')} placement="topRight">
                        {t('traffic.total_charged')}
                      </HeaderTooltip>
                    </TableHead>
                  </tr>
                </TableHeader>
                <TableBody>
                  {rows.length ? (
                    rows.map((row, index) => {
                      const rate = Number.parseFloat(row.server_rate);
                      const upload = parseInt(String(row.u));
                      const download = parseInt(String(row.d));
                      const charged = (upload + download) * (row.server_rate as unknown as number);
                      return (
                        <TableRow data-row-key={index} key={index}>
                          <TableCell>
                            {row.record_at ? formatUserLegacyDateSlash(row.record_at) : '-'}
                          </TableCell>
                          <TableCell className="text-right">
                            {row.server_rate ? formatBytes(upload) : 0}
                          </TableCell>
                          <TableCell className="text-right">
                            {row.server_rate ? formatBytes(download) : 0}
                          </TableCell>
                          <TableCell className="text-center">
                            <StatusBadge>{rate ? `${rate.toFixed(2)} x` : '-'}</StatusBadge>
                          </TableCell>
                          <TableCell className="text-right font-medium">
                            {formatBytes(charged)}
                          </TableCell>
                        </TableRow>
                      );
                    })
                  ) : (
                    <TableEmpty className="py-16" colSpan={5} data-testid="traffic-empty">
                      {emptyDescription}
                    </TableEmpty>
                  )}
                </TableBody>
              </Table>
            </TableScroll>
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

import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { CircleHelp } from 'lucide-react';
import { formatBytes } from '@v2board/config/format';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
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
import { cn } from '@/lib/cn';
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
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(rows.length);
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;

  return (
    <TooltipProvider delayDuration={100}>
      <Card className="v2board-traffic-card overflow-hidden">
        <CardContent className="p-0">
          <div className="border-b border-border bg-card p-4">
            <Alert className="v2board-traffic-notice bg-muted/40">
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
            className={cn('v2board-service-table-scroll', scrollPositionClassName)}
            onScroll={onScroll}
          >
            <Table className="v2board-service-table v2board-traffic-table min-w-[800px]">
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
                          <span className="inline-flex min-w-16 items-center justify-center rounded-md border border-border bg-secondary px-2 py-1 text-xs font-medium text-secondary-foreground">
                            {rate ? `${rate.toFixed(2)} x` : '-'}
                          </span>
                        </TableCell>
                        <TableCell className="text-right font-medium">
                          {formatBytes(charged)}
                        </TableCell>
                      </TableRow>
                    );
                  })
                ) : (
                  <TableEmpty className="py-16" colSpan={5} rowClassName="v2board-traffic-empty">
                    {emptyDescription}
                  </TableEmpty>
                )}
              </TableBody>
            </Table>
          </TableScroll>
        </CardContent>
      </Card>
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

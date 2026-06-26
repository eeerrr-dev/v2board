import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { CircleHelp } from 'lucide-react';
import { formatBytes } from '@v2board/config/format';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Card, CardContent } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
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

          <div
            ref={bodyRef}
            tabIndex={-1}
            className={cn('v2board-service-table-scroll overflow-x-auto', scrollPositionClassName)}
            onScroll={onScroll}
          >
            <table className="v2board-service-table v2board-traffic-table w-full min-w-[800px] text-sm">
              <thead className="border-b border-border bg-muted/50 text-muted-foreground">
                <tr>
                  <th className="px-4 py-3 text-left font-medium">{t('traffic.record_at')}</th>
                  <th className="px-4 py-3 text-right font-medium">{t('traffic.actual_upload')}</th>
                  <th className="px-4 py-3 text-right font-medium">
                    {t('traffic.actual_download')}
                  </th>
                  <th className="px-4 py-3 text-center font-medium">{t('traffic.deduct_rate')}</th>
                  <th className="px-4 py-3 text-right font-medium">
                    <HeaderTooltip title={t('traffic.total_formula')} placement="topRight">
                      {t('traffic.total_charged')}
                    </HeaderTooltip>
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {rows.length ? (
                  rows.map((row, index) => {
                    const rate = Number.parseFloat(row.server_rate);
                    const upload = parseInt(String(row.u));
                    const download = parseInt(String(row.d));
                    const charged = (upload + download) * (row.server_rate as unknown as number);
                    return (
                      <tr
                        className="transition-colors hover:bg-muted/50"
                        data-row-key={index}
                        key={index}
                      >
                        <td className="px-4 py-4">
                          {row.record_at ? formatUserLegacyDateSlash(row.record_at) : '-'}
                        </td>
                        <td className="px-4 py-4 text-right">
                          {row.server_rate ? formatBytes(upload) : 0}
                        </td>
                        <td className="px-4 py-4 text-right">
                          {row.server_rate ? formatBytes(download) : 0}
                        </td>
                        <td className="px-4 py-4 text-center">
                          <span className="inline-flex min-w-16 items-center justify-center rounded-md border border-border bg-secondary px-2 py-1 text-xs font-medium text-secondary-foreground">
                            {rate ? `${rate.toFixed(2)} x` : '-'}
                          </span>
                        </td>
                        <td className="px-4 py-4 text-right font-medium">{formatBytes(charged)}</td>
                      </tr>
                    );
                  })
                ) : (
                  <tr className="v2board-traffic-empty">
                    <td
                      className="px-4 py-16 text-center text-sm text-muted-foreground"
                      colSpan={5}
                    >
                      {emptyDescription}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
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

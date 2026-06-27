import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { CircleHelp, Server } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Spinner } from '@/components/ui/spinner';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/cn';
import { useServers, useSubscribe } from '@/lib/queries';
import { legacyHref } from '@/lib/legacy-href';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

export default function NodePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  // Old componentDidMount dispatches user/getSubscribe before server/fetch.
  const subscribe = useSubscribe({ refetchOnMount: 'always' });
  const serversQuery = useServers({ refetchOnMount: 'always' });
  const { data, isFetching } = serversQuery;
  const loading = useLegacyFetchLoading(isFetching, serversQuery.error);
  const servers = data ?? [];
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(servers.length, {
    syncOnMount: false,
    syncOnResize: false,
  });

  const to = subscribe.data?.plan_id ? `/plan/${subscribe.data.plan_id}` : '/plan';

  if (loading) {
    return (
      <div
        className="v2board-node-loading flex min-h-44 items-center justify-center rounded-xl border border-border bg-card text-card-foreground shadow-sm"
        role="status"
      >
        <Spinner className="size-5 text-muted-foreground" />
        <span className="sr-only">Loading...</span>
      </div>
    );
  }

  if (servers.length === 0) {
    return (
      <Alert className="v2board-node-empty bg-card">
        <Server className="size-4" />
        <AlertDescription>
          <span className="flex flex-wrap items-center gap-1">
            <span>{t('node.no_available')}</span>
            <Button asChild variant="link" className="h-auto p-0 text-sm">
              <a
                className="v2board-node-empty-action"
                ref={legacyHref()}
                onClick={() => navigate(to)}
              >
                {subscribe.data?.plan_id ? t('node.renew') : t('node.subscribe')}
              </a>
            </Button>
          </span>
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <TooltipProvider delayDuration={100}>
      <Card className="v2board-node-card overflow-hidden">
        <CardContent className="p-0">
          <TableScroll
            ref={bodyRef}
            className={cn('v2board-service-table-scroll', scrollPositionClassName)}
            tabIndex={-1}
            onScroll={onScroll}
          >
            <Table className="v2board-service-table v2board-node-table min-w-[900px]">
              <TableHeader>
                <tr>
                  <TableHead>
                    <span className="v2board-table-column-title">{t('node.simple_name')}</span>
                  </TableHead>
                  <TableHead className="text-center">
                    <HeaderTooltip title={t('node.status_tip')}>{t('node.status')}</HeaderTooltip>
                  </TableHead>
                  <TableHead className="text-center">
                    <HeaderTooltip title={t('node.rate_tip')}>{t('node.rate')}</HeaderTooltip>
                  </TableHead>
                  <TableHead>
                    <span className="v2board-table-column-title">{t('node.tags')}</span>
                  </TableHead>
                </tr>
              </TableHeader>
              <TableBody>
                {servers.map((server, index) => {
                  const online = Boolean(parseInt(String(server.is_online)));
                  return (
                    <TableRow data-row-key={index} key={index}>
                      <TableCell className="font-medium text-foreground">{server.name}</TableCell>
                      <TableCell className="text-center">
                        <span
                          className={cn(
                            'inline-flex size-2.5 rounded-full',
                            online ? 'bg-emerald-500' : 'bg-destructive',
                          )}
                          aria-label={online ? 'online' : 'offline'}
                        />
                      </TableCell>
                      <TableCell className="text-center">
                        <span className="inline-flex min-w-16 items-center justify-center rounded-md border border-border bg-secondary px-2 py-1 text-xs font-medium text-secondary-foreground">
                          {String(server.rate)} x
                        </span>
                      </TableCell>
                      <TableCell>
                        {server.tags?.length ? (
                          <div className="flex flex-wrap gap-1.5">
                            {server.tags.map((tag) => (
                              <span
                                className="inline-flex rounded-md border border-border bg-background px-2 py-1 text-xs text-muted-foreground"
                                key={tag}
                              >
                                {tag}
                              </span>
                            ))}
                          </div>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableScroll>
        </CardContent>
      </Card>
    </TooltipProvider>
  );
}

function HeaderTooltip({ children, title }: { children: ReactNode; title: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="v2board-service-tooltip-trigger inline-flex cursor-help items-center justify-center gap-1">
          {children}
          <CircleHelp className="size-3.5" />
        </span>
      </TooltipTrigger>
      <TooltipContent>{title}</TooltipContent>
    </Tooltip>
  );
}

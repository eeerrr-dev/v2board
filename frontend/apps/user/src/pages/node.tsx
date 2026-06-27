import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { CircleHelp, Server } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import {
  DataTable,
  TableCell,
  TableRow,
} from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
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
  const { bodyRef, onScroll, scrollPosition } = useTableScrollPosition(servers.length, {
    syncOnMount: false,
    syncOnResize: false,
  });

  const to = subscribe.data?.plan_id ? `/plan/${subscribe.data.plan_id}` : '/plan';

  if (loading) {
    return (
      <PageShell data-testid="node-page">
        <div
          className="flex min-h-44 items-center justify-center rounded-xl border border-border bg-card text-card-foreground shadow-sm"
          data-testid="node-loading"
          role="status"
        >
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">Loading...</span>
        </div>
      </PageShell>
    );
  }

  if (servers.length === 0) {
    return (
      <PageShell data-testid="node-page">
        <Alert className="bg-card" data-testid="node-empty">
          <Server className="size-4" />
          <AlertDescription>
            <span className="flex flex-wrap items-center gap-1">
              <span>{t('node.no_available')}</span>
              <Button asChild variant="link" className="h-auto p-0 text-sm">
                <a
                  data-testid="node-empty-action"
                  ref={legacyHref()}
                  onClick={() => navigate(to)}
                >
                  {subscribe.data?.plan_id ? t('node.renew') : t('node.subscribe')}
                </a>
              </Button>
            </span>
          </AlertDescription>
        </Alert>
      </PageShell>
    );
  }

  return (
    <TooltipProvider delayDuration={100}>
      <PageShell data-testid="node-page">
        <Card className="overflow-hidden py-0" data-testid="node-card">
          <CardContent className="p-0">
            <DataTable
              className="min-w-[900px]"
              data-table-kind="service"
              data-testid="node-table"
              headers={[
                { content: <span>{t('node.simple_name')}</span> },
                {
                  align: 'center',
                  content: <HeaderTooltip title={t('node.status_tip')}>{t('node.status')}</HeaderTooltip>,
                },
                {
                  align: 'center',
                  content: <HeaderTooltip title={t('node.rate_tip')}>{t('node.rate')}</HeaderTooltip>,
                },
                { content: <span>{t('node.tags')}</span> },
              ]}
              scrollRef={bodyRef}
              scrollProps={{
                'data-scroll-position': scrollPosition,
                'data-testid': 'service-table-scroll',
                tabIndex: -1,
                onScroll,
              }}
            >
              {servers.map((server, index) => {
                const online = Boolean(parseInt(String(server.is_online)));
                return (
                  <TableRow data-row-key={index} key={index}>
                    <TableCell className="font-medium text-foreground">{server.name}</TableCell>
                    <TableCell className="text-center">
                      <StatusBadge
                        tone={online ? 'success' : 'destructive'}
                        showDot
                        aria-label={online ? 'online' : 'offline'}
                      >
                        {online ? t('node.online') : t('node.offline')}
                      </StatusBadge>
                    </TableCell>
                    <TableCell className="text-center">
                      <StatusBadge>{String(server.rate)} x</StatusBadge>
                    </TableCell>
                    <TableCell>
                      {server.tags?.length ? (
                        <div className="flex flex-wrap gap-1.5">
                          {server.tags.map((tag) => (
                            <StatusBadge className="bg-background text-muted-foreground" key={tag}>
                              {tag}
                            </StatusBadge>
                          ))}
                        </div>
                      ) : (
                        <span className="text-muted-foreground">-</span>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </DataTable>
          </CardContent>
        </Card>
      </PageShell>
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

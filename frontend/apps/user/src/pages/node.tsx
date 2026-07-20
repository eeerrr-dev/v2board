import { useTranslation } from 'react-i18next';
import { Link } from 'react-router';
import { Server } from 'lucide-react';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import { ErrorState } from '@v2board/ui/error-state';
import { HeaderTooltip } from '@v2board/ui/header-tooltip';
import { PageShell } from '@v2board/ui/page';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { StatusBadge } from '@v2board/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@v2board/ui/table';
import { TooltipProvider } from '@v2board/ui/tooltip';
import { useServers, useSubscribe } from '@/lib/queries';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';

export default function NodePage() {
  const { t } = useTranslation();
  // §4.6 (docs/api-dialect.md): subscription and servers fetch in parallel,
  // but rendering stays subscription-gated — the node list never renders
  // before subscription state is known, because subscription state owns the
  // empty-state route and a failed subscription request cannot mean "no plan".
  const subscribe = useSubscribe({ refetchOnMount: 'always' });
  const serversQuery = useServers({ refetchOnMount: 'always' });
  // Key the full-page spinner on isPending (no data yet), not isFetching, so
  // cached rows keep rendering while the refetchOnMount('always') background
  // refetch runs instead of blanking the page on every revisit.
  const { data, isPending, isError, refetch } = serversQuery;
  const servers = data ?? [];
  const { bodyRef, onScroll, scrollPosition } = useTableScrollPosition(servers.length, {
    syncOnMount: false,
    syncOnResize: false,
  });
  const serviceColumns = [
    {
      id: 'name',
      accessorKey: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.node.simple_name)}</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'status',
      meta: { align: 'center' },
      header: () => (
        <HeaderTooltip className="justify-center" title={t(($) => $.node.status_tip)}>
          {t(($) => $.node.status)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => {
        const online = row.original.is_online;
        return (
          <StatusBadge
            tone={online ? 'success' : 'destructive'}
            showDot
            aria-label={online ? 'online' : 'offline'}
          >
            {online ? t(($) => $.node.online) : t(($) => $.node.offline)}
          </StatusBadge>
        );
      },
    },
    {
      id: 'rate',
      meta: { align: 'center' },
      header: () => (
        <HeaderTooltip className="justify-center" title={t(($) => $.node.rate_tip)}>
          {t(($) => $.node.rate)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => <StatusBadge>{row.original.rate} x</StatusBadge>,
    },
    {
      id: 'tags',
      header: () => <span>{t(($) => $.node.tags)}</span>,
      cell: ({ row }) =>
        row.original.tags?.length ? (
          <div className="flex flex-wrap gap-1.5">
            {row.original.tags.map((tag) => (
              <StatusBadge className="bg-background text-muted-foreground" key={tag}>
                {tag}
              </StatusBadge>
            ))}
          </div>
        ) : (
          <span className="text-muted-foreground">-</span>
        ),
    },
  ] satisfies DataTableColumn<(typeof servers)[number]>[];

  const to = subscribe.data?.plan_id ? `/plan/${subscribe.data.plan_id}` : '/plan';

  if (subscribe.isPending || (subscribe.isSuccess && isPending)) {
    return (
      <PageShell data-testid="node-page">
        <LoadingState
          className="min-h-44 rounded-xl border border-border bg-card p-6 text-card-foreground shadow-sm"
          data-testid="node-loading"
        >
          <SkeletonRows rows={3} />
        </LoadingState>
      </PageShell>
    );
  }

  if (subscribe.isError) {
    return (
      <PageShell data-testid="node-page">
        <ErrorState onRetry={() => void subscribe.refetch()} data-testid="node-subscribe-error" />
      </PageShell>
    );
  }

  // A failed fetch must not fall through to the empty state below — that would
  // wrongly tell a paying user their subscription has no nodes. Surface the
  // error with a retry instead.
  if (isError) {
    return (
      <PageShell data-testid="node-page">
        <ErrorState onRetry={() => void refetch()} data-testid="node-error" />
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
              <span>{t(($) => $.node.no_available)}</span>
              <Button
                asChild
                data-testid="node-empty-action"
                variant="link"
                className="h-auto p-0 text-sm"
              >
                <Link to={to}>
                  {subscribe.data?.plan_id ? t(($) => $.node.renew) : t(($) => $.node.subscribe)}
                </Link>
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
              columns={serviceColumns}
              data={servers}
              data-table-kind="service"
              data-testid="node-table"
              scrollRef={bodyRef}
              scrollProps={{
                'data-scroll-position': scrollPosition,
                'data-testid': 'service-table-scroll',
                tabIndex: 0,
                role: 'region',
                'aria-label': t(($) => $.nav.node),
                onScroll,
              }}
              virtualizer={{ enabled: servers.length > VIRTUALIZE_MIN_ROWS }}
            />
          </CardContent>
        </Card>
      </PageShell>
    </TooltipProvider>
  );
}

import { type ReactNode } from 'react';
import type { SelectorParam } from 'i18next';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, XCircle } from 'lucide-react';
import { useQueueStats, useQueueWorkload } from '@/lib/queries';
import { Card, CardContent } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge } from '@/components/ui/status-badge';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { ErrorState } from '@/components/ui/error-state';

type QueueWorkloadRow = { name: string; processes: number; length: number; wait: number };

// Keyed by the wire queue `name` values; labels resolve through t() at render.
const QUEUE_NAME_KEYS: Record<string, SelectorParam> = {
  order_handle: ($) => $.admin.system.queue_order_handle,
  send_email: ($) => $.admin.system.queue_send_email,
  send_email_mass: ($) => $.admin.system.queue_send_email_mass,
  send_telegram: ($) => $.admin.system.queue_send_telegram,
  stat: ($) => $.admin.system.queue_stat,
  traffic_fetch: ($) => $.admin.system.queue_traffic_fetch,
};

function StatCard({ label, value }: { label: string; value: ReactNode }) {
  return (
    <Card>
      <CardContent className="space-y-3">
        <div className="text-sm text-muted-foreground">{label}</div>
        <div className="text-3xl font-semibold tracking-tight text-foreground">{value}</div>
      </CardContent>
    </Card>
  );
}

export default function SystemPage() {
  const { t } = useTranslation();
  const queueStats = useQueueStats();
  const queueWorkload = useQueueWorkload();
  const stats = queueStats.data;
  const workload = queueWorkload.data?.filter((item) => item.name !== 'default');

  const workloadColumns: DataTableColumn<QueueWorkloadRow>[] = [
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.system.queue_name)}</span>,
      cell: ({ row }) => {
        const labelKey = QUEUE_NAME_KEYS[row.original.name];
        return labelKey ? t(labelKey) : row.original.name;
      },
    },
    {
      id: 'processes',
      header: () => <span>{t(($) => $.admin.system.processes)}</span>,
      cell: ({ row }) => row.original.processes,
    },
    {
      id: 'length',
      header: () => <span>{t(($) => $.admin.system.length)}</span>,
      cell: ({ row }) => row.original.length,
    },
    {
      id: 'wait',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.admin.system.wait)}</span>,
      cell: ({ row }) => `${row.original.wait}s`,
    },
  ];

  return (
    <PageShell data-testid="queue-page">
      <section className="space-y-3">
        <h2 className="text-base font-semibold text-foreground">
          {t(($) => $.admin.system.overview)}
        </h2>
        {queueStats.isError ? (
          <ErrorState
            message={t(($) => $.admin.system.stats_error)}
            onRetry={() => void queueStats.refetch()}
          />
        ) : stats ? (
          <div className="grid gap-4 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
            <StatCard
              label={t(($) => $.admin.system.jobs_per_minute)}
              value={stats.jobs_per_minute || '0'}
            />
            <StatCard
              label={t(($) => $.admin.system.recent_jobs)}
              value={stats.recent_jobs || '0'}
            />
            <StatCard
              label={t(($) => $.admin.system.failed_jobs)}
              value={stats.failed_jobs || '0'}
            />
            <StatCard
              label={t(($) => $.admin.system.status)}
              value={
                <StatusBadge tone={stats.status ? 'success' : 'destructive'} className="gap-1.5">
                  {stats.status ? (
                    <CheckCircle2 className="size-4" />
                  ) : (
                    <XCircle className="size-4" />
                  )}
                  {stats.status
                    ? t(($) => $.admin.system.status_running)
                    : t(($) => $.admin.system.status_stopped)}
                </StatusBadge>
              }
            />
          </div>
        ) : (
          <LoadingPanel />
        )}
      </section>

      <section className="space-y-3">
        <h2 className="text-base font-semibold text-foreground">
          {t(($) => $.admin.system.workload_title)}
        </h2>
        {queueWorkload.isError ? (
          <ErrorState
            message={t(($) => $.admin.system.workload_error)}
            onRetry={() => void queueWorkload.refetch()}
          />
        ) : workload ? (
          <Card className="overflow-hidden py-0">
            <CardContent className="p-0">
              <DataTable
                columns={workloadColumns}
                data={workload}
                getRowKey={(row) => row.name}
                data-testid="queue-workload-table"
                empty={workload.length === 0 ? t(($) => $.admin.system.workload_empty) : undefined}
                emptyTestId="queue-workload-empty"
              />
            </CardContent>
          </Card>
        ) : (
          <LoadingPanel />
        )}
      </section>
    </PageShell>
  );
}

function LoadingPanel() {
  return (
    <LoadingState className="min-h-32 rounded-xl border border-border bg-card p-4">
      <SkeletonRows rows={2} />
    </LoadingState>
  );
}

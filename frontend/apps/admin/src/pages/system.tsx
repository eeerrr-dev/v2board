import { type ReactNode } from 'react';
import { CheckCircle2, XCircle } from 'lucide-react';
import { useQueueStats, useQueueWorkload } from '@/lib/queries';
import { Card, CardContent } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge } from '@/components/ui/status-badge';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { ErrorState } from '@/components/ui/error-state';

type QueueWorkloadRow = { name: string; processes: number; length: number; wait: number };

const QUEUE_NAMES: Record<string, string> = {
  order_handle: '订单队列',
  send_email: '邮件队列',
  send_email_mass: '邮件群发队列',
  send_telegram: 'Telegram消息队列',
  stat: '统计队列',
  traffic_fetch: '流量消费队列',
};

const workloadColumns: DataTableColumn<QueueWorkloadRow>[] = [
  {
    id: 'name',
    meta: { className: 'font-medium text-foreground' },
    header: () => <span>队列名称</span>,
    cell: ({ row }) => QUEUE_NAMES[row.original.name] ?? row.original.name,
  },
  {
    id: 'processes',
    header: () => <span>作业量</span>,
    cell: ({ row }) => row.original.processes,
  },
  {
    id: 'length',
    header: () => <span>任务量</span>,
    cell: ({ row }) => row.original.length,
  },
  {
    id: 'wait',
    meta: { align: 'right' },
    header: () => <span>占用时间</span>,
    cell: ({ row }) => `${row.original.wait}s`,
  },
];

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
  const queueStats = useQueueStats();
  const queueWorkload = useQueueWorkload();
  const stats = queueStats.data;
  const workload = queueWorkload.data?.filter((item) => item.name !== 'default');

  return (
    <PageShell data-testid="queue-page">
      <section className="space-y-3">
        <h2 className="text-base font-semibold text-foreground">总览</h2>
        {queueStats.isError ? (
          <ErrorState message="队列状态加载失败" onRetry={() => void queueStats.refetch()} />
        ) : stats ? (
          <div className="grid gap-4 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
            <StatCard label="当前作业量" value={stats.jobs_per_minute || '0'} />
            <StatCard label="近一小时处理量" value={stats.recent_jobs || '0'} />
            <StatCard label="7日内报错数量" value={stats.failed_jobs || '0'} />
            <StatCard
              label="状态"
              value={
                <StatusBadge tone={stats.status ? 'success' : 'destructive'} className="gap-1.5">
                  {stats.status ? (
                    <CheckCircle2 className="size-4" />
                  ) : (
                    <XCircle className="size-4" />
                  )}
                  {stats.status ? '运行中' : '未启动'}
                </StatusBadge>
              }
            />
          </div>
        ) : (
          <LoadingPanel />
        )}
      </section>

      <section className="space-y-3">
        <h2 className="text-base font-semibold text-foreground">当前作业详情</h2>
        {queueWorkload.isError ? (
          <ErrorState message="作业详情加载失败" onRetry={() => void queueWorkload.refetch()} />
        ) : workload ? (
          <Card className="overflow-hidden py-0">
            <CardContent className="p-0">
              <DataTable
                columns={workloadColumns}
                data={workload}
                getRowKey={(row) => row.name}
                data-testid="queue-workload-table"
                empty={workload.length === 0 ? '暂无作业' : undefined}
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

import { useState } from 'react';
import type { admin } from '@v2board/api-client';
import { formatBackendDateTime } from '@v2board/config/format';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { PageHeader, PageShell } from '@/components/ui/page';
import { PaginationControl } from '@/components/ui/pagination';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { useAuditLogs } from '@/lib/queries';

const PAGINATION_LABELS = {
  itemsPerPage: '条/页',
  nextPage: '下一页',
  nextWindow: '向后 5 页',
  previousPage: '上一页',
  previousWindow: '向前 5 页',
};

const SURFACE_TEXT: Record<string, string> = {
  admin: '管理员',
  staff: '员工',
};

// Radix Select items cannot carry an empty value, so the "all" choice uses a
// sentinel that simply omits the §7 filter clause.
const ALL_SURFACES = 'all';

interface QueryState {
  current: number;
  pageSize: number;
  surface: string;
}

/**
 * The §6.11 operator audit trail (`GET system/audit-logs`): a read-only,
 * newest-first view of every recorded admin/staff mutation. The table is
 * append-only on the backend — this page never mutates anything.
 */
export default function AuditPage() {
  const [query, setQuery] = useState<QueryState>({
    current: 1,
    pageSize: 20,
    surface: ALL_SURFACES,
  });
  const logs = useAuditLogs({
    page: query.current,
    per_page: query.pageSize,
    filter:
      query.surface === ALL_SURFACES
        ? undefined
        : [{ field: 'surface', op: 'eq', value: query.surface }],
  });

  const data = logs.data?.items ?? [];
  const total = logs.data?.total ?? 0;

  const columns: DataTableColumn<admin.AdminAuditLogRecord>[] = [
    {
      id: 'created_at',
      meta: { className: 'whitespace-nowrap tabular-nums' },
      header: () => <span>时间</span>,
      cell: ({ row }) => formatBackendDateTime(row.original.created_at),
    },
    {
      id: 'actor_email',
      meta: { className: 'text-foreground' },
      header: () => <span>操作者</span>,
      cell: ({ row }) => row.original.actor_email,
    },
    {
      id: 'surface',
      meta: { align: 'center' },
      header: () => <span>界面</span>,
      cell: ({ row }) => (
        <Badge variant={row.original.surface === 'admin' ? 'default' : 'secondary'}>
          {SURFACE_TEXT[row.original.surface] ?? row.original.surface}
        </Badge>
      ),
    },
    {
      id: 'method',
      meta: { align: 'center', className: 'font-mono text-xs' },
      header: () => <span>方法</span>,
      cell: ({ row }) => row.original.method,
    },
    {
      id: 'path',
      meta: { className: 'font-mono text-xs break-all' },
      header: () => <span>路径</span>,
      cell: ({ row }) => row.original.path,
    },
    {
      id: 'status_code',
      meta: { align: 'center', className: 'tabular-nums' },
      header: () => <span>状态</span>,
      cell: ({ row }) => (
        <Badge variant={row.original.status_code < 400 ? 'secondary' : 'destructive'}>
          {row.original.status_code}
        </Badge>
      ),
    },
    {
      id: 'client_ip',
      meta: { className: 'font-mono text-xs' },
      header: () => <span>来源 IP</span>,
      cell: ({ row }) => row.original.client_ip ?? '-',
    },
    {
      id: 'request_id',
      meta: { className: 'max-w-40 truncate font-mono text-xs' },
      header: () => <span>请求 ID</span>,
      cell: ({ row }) => row.original.request_id ?? '-',
    },
  ];

  return (
    <PageShell data-testid="audit-page">
      <PageHeader
        title="审计日志"
        description="每一次管理员/员工修改操作的只读追加记录；请求体不会被记录。"
      />

      <Card>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={query.surface}
              onValueChange={(surface) => setQuery((state) => ({ ...state, current: 1, surface }))}
            >
              <SelectTrigger
                className="w-40"
                aria-label="界面筛选"
                data-testid="audit-surface-filter"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL_SURFACES}>全部界面</SelectItem>
                <SelectItem value="admin">管理员</SelectItem>
                <SelectItem value="staff">员工</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {logs.isError ? (
            <ErrorState
              message="审计日志加载失败"
              onRetry={() => void logs.refetch()}
              data-testid="audit-error"
            />
          ) : (
            <>
              <DataTable
                columns={columns}
                data={data}
                getRowKey={(row) => row.id}
                className="min-w-[960px]"
                data-testid="audit-table"
                empty={
                  !logs.isError && logs.data !== undefined && data.length === 0
                    ? '暂无审计记录'
                    : undefined
                }
                emptyTestId="audit-empty"
              />

              {total > 0 ? (
                <PaginationControl
                  current={query.current}
                  pageSize={query.pageSize}
                  total={total}
                  labels={PAGINATION_LABELS}
                  onChange={(page, pageSize) =>
                    setQuery((state) => ({ ...state, current: page, pageSize }))
                  }
                  testIds={{ page: 'audit-page-control', pageSize: 'audit-page-size' }}
                />
              ) : null}
            </>
          )}

          {logs.isPending ? (
            <div className="flex justify-center py-6" role="status">
              <Spinner className="size-5 text-muted-foreground" />
              <span className="sr-only">加载中</span>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </PageShell>
  );
}

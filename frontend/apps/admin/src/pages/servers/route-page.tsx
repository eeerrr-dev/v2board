import { Pencil, Plus, Trash2 } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { ErrorState } from '@/components/ui/error-state';
import { PageHeader, PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import {
  useDropServerRouteMutation,
  useSaveServerRouteMutation,
  useServerRoutes,
} from '@/lib/queries';
import { ROUTE_ACTION_TEXT, getRouteMatchLabel } from './domain';
import { ServerRouteDialog } from './route-dialog';
import { splitServerRouteMatches, type ServerRouteFormValues } from './form-schema';

export function ServerRoutePage() {
  const routes = useServerRoutes();
  const save = useSaveServerRouteMutation();
  const drop = useDropServerRouteMutation();
  const data = routes.data ?? [];

  const saveRoute = (route: ServerRouteFormValues, onSuccess: () => void) => {
    const payload = {
      remarks: route.remarks,
      match: route.action === 'default_out' ? [] : splitServerRouteMatches(route.match),
      action: route.action,
      action_value: route.action_value,
      ...(route.id === undefined ? {} : { id: route.id }),
    };
    save.mutate(payload, { onSuccess });
  };

  const removeRoute = async (record: admin.ServerRoute) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该路由吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(record.id);
  };

  const columns: DataTableColumn<admin.ServerRoute>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'remarks',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>备注</span>,
      cell: ({ row }) => row.original.remarks,
    },
    {
      id: 'match',
      header: () => <span>匹配数量</span>,
      cell: ({ row }) => getRouteMatchLabel(row.original.match),
    },
    {
      id: 'action',
      header: () => <span>动作</span>,
      cell: ({ row }) => ROUTE_ACTION_TEXT[row.original.action],
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerRouteDialog route={row.original} pending={save.isPending} onSave={saveRoute}>
            <Button variant="ghost" size="sm" data-testid={`server-route-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </ServerRouteDialog>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeRoute(row.original)}
            data-testid={`server-route-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-route-page">
      {routes.isError ? (
        <ErrorState message="路由列表加载失败" onRetry={() => void routes.refetch()} />
      ) : null}
      <PageHeader
        title="路由管理"
        actions={
          <ServerRouteDialog pending={save.isPending} onSave={saveRoute}>
            <Button data-testid="server-route-create">
              <Plus className="size-4" />
              添加路由
            </Button>
          </ServerRouteDialog>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[720px]"
            data-testid="server-routes-table"
            empty={
              routes.isSuccess && routes.data !== undefined && data.length === 0
                ? '暂无路由'
                : undefined
            }
            emptyTestId="server-routes-empty"
          />
        </CardContent>
      </Card>

      {routes.isFetching ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}

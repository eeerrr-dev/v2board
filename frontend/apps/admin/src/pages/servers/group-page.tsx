import { Database, Pencil, Plus, Trash2, User } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { ErrorState } from '@/components/ui/error-state';
import { PageHeader, PageShell } from '@/components/ui/page';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import {
  useDropServerGroupMutation,
  useSaveServerGroupMutation,
  useServerGroups,
} from '@/lib/queries';
import { ServerGroupDialog } from './group-dialog';
import type { ServerGroupFormValues } from './form-schema';

export function ServerGroupPage() {
  const groups = useServerGroups();
  const save = useSaveServerGroupMutation();
  const drop = useDropServerGroupMutation();
  const data = groups.data ?? [];

  const saveGroup = (payload: ServerGroupFormValues, onSuccess: () => void) => {
    save.mutate(payload, { onSuccess });
  };

  const removeGroup = async (record: admin.ServerGroup) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该权限组吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(record.id);
  };

  const columns: DataTableColumn<admin.ServerGroup>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>组ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>组名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'user_count',
      header: () => <span>用户数量</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <User className="size-4 text-muted-foreground" /> {row.original.user_count}
        </span>
      ),
    },
    {
      id: 'server_count',
      header: () => <span>节点数量</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <Database className="size-4 text-muted-foreground" /> {row.original.server_count}
        </span>
      ),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerGroupDialog record={row.original} pending={save.isPending} onSave={saveGroup}>
            <Button variant="ghost" size="sm" data-testid={`server-group-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </ServerGroupDialog>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeGroup(row.original)}
            data-testid={`server-group-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-group-page">
      {groups.isError ? (
        <ErrorState message="权限组加载失败" onRetry={() => void groups.refetch()} />
      ) : null}
      <PageHeader
        title="权限组管理"
        actions={
          <ServerGroupDialog pending={save.isPending} onSave={saveGroup}>
            <Button data-testid="server-group-create">
              <Plus className="size-4" />
              添加权限组
            </Button>
          </ServerGroupDialog>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[720px]"
            data-testid="server-groups-table"
            empty={
              groups.isSuccess && groups.data !== undefined && data.length === 0
                ? '暂无权限组'
                : undefined
            }
            emptyTestId="server-groups-empty"
          />
        </CardContent>
      </Card>

      {groups.isFetching ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}

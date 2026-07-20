import { useTranslation } from 'react-i18next';
import { Database, Pencil, Plus, Trash2, User } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import { ErrorState } from '@v2board/ui/error-state';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import {
  useDropServerGroupMutation,
  useSaveServerGroupMutation,
  useServerGroups,
} from '@/lib/queries';
import { ServerGroupDialog } from './group-dialog';
import type { ServerGroupFormValues } from './form-schema';

export function ServerGroupPage() {
  const { t } = useTranslation();
  const groups = useServerGroups();
  const save = useSaveServerGroupMutation();
  const drop = useDropServerGroupMutation();
  const data = groups.data ?? [];

  const saveGroup = (payload: ServerGroupFormValues, onSuccess: () => void) => {
    save.mutate(payload, { onSuccess });
  };

  const removeGroup = async (record: admin.ServerGroup) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.servers.warning),
      description: t(($) => $.admin.servers.confirm_delete_group),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    drop.mutate(record.id);
  };

  const columns: DataTableColumn<admin.ServerGroup>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.servers.col_group_id)}</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.servers.group_name)}</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'user_count',
      header: () => <span>{t(($) => $.admin.servers.user_count)}</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <User className="size-4 text-muted-foreground" /> {row.original.user_count}
        </span>
      ),
    },
    {
      id: 'server_count',
      header: () => <span>{t(($) => $.admin.servers.node_count)}</span>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <Database className="size-4 text-muted-foreground" /> {row.original.server_count}
        </span>
      ),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerGroupDialog record={row.original} pending={save.isPending} onSave={saveGroup}>
            <Button variant="ghost" size="sm" data-testid={`server-group-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              {t(($) => $.common.edit)}
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
            {t(($) => $.common.delete)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-group-page">
      {groups.isError ? (
        <ErrorState
          message={t(($) => $.admin.servers.groups_load_failed)}
          onRetry={() => void groups.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.servers.group_title)}
        actions={
          <ServerGroupDialog pending={save.isPending} onSave={saveGroup}>
            <Button data-testid="server-group-create">
              <Plus className="size-4" />
              {t(($) => $.admin.servers.add_group)}
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
                ? t(($) => $.admin.servers.no_groups)
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

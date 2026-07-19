import { useTranslation } from 'react-i18next';
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
import { getRouteActionText, getRouteMatchLabel } from './domain';
import { ServerRouteDialog } from './route-dialog';
import { splitServerRouteMatches, type ServerRouteFormValues } from './form-schema';

export function ServerRoutePage() {
  const { t } = useTranslation();
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
      title: t(($) => $.admin.servers.warning),
      description: t(($) => $.admin.servers.confirm_delete_route),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    drop.mutate(record.id);
  };

  const routeActionText = getRouteActionText(t);
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
      header: () => <span>{t(($) => $.admin.servers.remarks)}</span>,
      cell: ({ row }) => row.original.remarks,
    },
    {
      id: 'match',
      header: () => <span>{t(($) => $.admin.servers.match_count)}</span>,
      cell: ({ row }) => getRouteMatchLabel(t, row.original.match),
    },
    {
      id: 'action',
      header: () => <span>{t(($) => $.admin.servers.action)}</span>,
      cell: ({ row }) => routeActionText[row.original.action],
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <ServerRouteDialog route={row.original} pending={save.isPending} onSave={saveRoute}>
            <Button variant="ghost" size="sm" data-testid={`server-route-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              {t(($) => $.common.edit)}
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
            {t(($) => $.common.delete)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="server-route-page">
      {routes.isError ? (
        <ErrorState
          message={t(($) => $.admin.servers.routes_load_failed)}
          onRetry={() => void routes.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.servers.route_title)}
        actions={
          <ServerRouteDialog pending={save.isPending} onSave={saveRoute}>
            <Button data-testid="server-route-create">
              <Plus className="size-4" />
              {t(($) => $.admin.servers.add_route)}
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
                ? t(($) => $.admin.servers.no_routes)
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

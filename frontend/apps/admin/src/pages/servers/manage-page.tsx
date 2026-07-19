import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useBeforeUnload, useBlocker } from 'react-router';
import {
  ArrowDown,
  ArrowUp,
  ChevronDown,
  Copy,
  ListFilter,
  Pencil,
  Plus,
  Trash2,
  User,
} from 'lucide-react';
import type { admin } from '@v2board/api-client';
import { copyText } from '@v2board/config/clipboard';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ErrorState } from '@/components/ui/error-state';
import { HeaderTooltip } from '@/components/ui/header-tooltip';
import { Input } from '@/components/ui/input';
import { PageHeader, PageShell } from '@/components/ui/page';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { Switch } from '@/components/ui/switch';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { cn } from '@/lib/cn';
import {
  useCopyServerMutation,
  useDropServerMutation,
  useServerGroups,
  useServerNodes,
  useServerRoutes,
  useSortServerNodesMutation,
  useUpdateServerMutation,
} from '@/lib/queries';
import { toast } from '@/lib/toast';
import {
  NODE_TYPE_FILTERS,
  SERVER_TYPES,
  SERVER_TYPE_LABELS,
  applyServerNodeColumnControls,
  createServerSortPayload,
  moveServerNodeByDragIndexes,
  type NodeFilterItem,
} from './domain';
import { AvailabilityDot, ServerTypeTag } from './form-controls';
import { NodeEditor } from './node-editor';

function ServerSortNavigationGuard({ when }: { when: boolean }) {
  const { t } = useTranslation();
  const blocker = useBlocker(when);
  // Deliberate useCallback: useBeforeUnload re-subscribes the capture-phase
  // window listener whenever this identity changes.
  const handleBeforeUnload = useCallback(
    (event: BeforeUnloadEvent) => {
      if (!when) return;
      event.preventDefault();
      // Browsers still require returnValue alongside preventDefault to show the
      // native confirmation for a hard reload or tab close.
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      event.returnValue = '';
    },
    [when],
  );
  useBeforeUnload(handleBeforeUnload, { capture: true });

  const stay = () => {
    if (blocker.state === 'blocked') blocker.reset();
  };
  const leave = () => {
    if (blocker.state === 'blocked') blocker.proceed();
  };

  return (
    <AlertDialog
      open={blocker.state === 'blocked'}
      onOpenChange={(open) => {
        if (!open) stay();
      }}
    >
      <AlertDialogContent data-testid="server-sort-leave-dialog" className="sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle>{t(($) => $.admin.servers.sort_leave_prompt)}</AlertDialogTitle>
          <AlertDialogDescription>
            {t(($) => $.admin.servers.sort_leave_description)}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel asChild>
            <Button type="button" variant="outline" data-testid="server-sort-stay">
              {t(($) => $.common.cancel)}
            </Button>
          </AlertDialogCancel>
          <AlertDialogAction asChild>
            <Button
              type="button"
              onClick={(event) => {
                event.preventDefault();
                leave();
              }}
              data-testid="server-sort-leave"
            >
              {t(($) => $.common.confirm)}
            </Button>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

function NodeFilterMenu({
  items,
  value,
  active,
  onApply,
}: {
  items: NodeFilterItem[];
  value: string[];
  active: boolean;
  onApply: (next: string[]) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<string[]>(value);
  const toggle = (target: string) =>
    setPending((prev) =>
      prev.includes(target) ? prev.filter((item) => item !== target) : [...prev, target],
    );
  return (
    <DropdownMenu
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) setPending(value);
        setOpen(nextOpen);
      }}
    >
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t(($) => $.admin.servers.filter)}
          className={cn(
            'ml-1 inline-flex size-6 items-center justify-center rounded-sm transition-colors outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50',
            active ? 'text-primary' : 'text-muted-foreground',
          )}
        >
          <ListFilter className="size-3.5" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-40">
        <div className="max-h-64 overflow-y-auto py-1">
          {items.map((item) => (
            <label
              key={item.value}
              className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
            >
              <Checkbox
                checked={pending.includes(item.value)}
                onCheckedChange={() => toggle(item.value)}
              />
              <span>{item.text}</span>
            </label>
          ))}
        </div>
        <DropdownMenuSeparator />
        <div className="flex items-center justify-between px-2 py-1">
          <button
            type="button"
            className="text-sm text-primary"
            onClick={() => {
              onApply(pending);
              setOpen(false);
            }}
          >
            {t(($) => $.common.confirm)}
          </button>
          <button
            type="button"
            className="text-sm text-muted-foreground"
            onClick={() => {
              setPending([]);
              onApply([]);
              setOpen(false);
            }}
          >
            {t(($) => $.admin.servers.reset)}
          </button>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function OnlineSortHeader({
  sort,
  onCycle,
}: {
  sort: '' | 'ascend' | 'descend';
  onCycle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1.5 rounded-sm transition-colors outline-none select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={onCycle}
    >
      <HeaderTooltip title={t(($) => $.admin.servers.online_count_tip)}>
        {t(($) => $.admin.servers.online_count)}
      </HeaderTooltip>
      {sort === 'ascend' ? (
        <ArrowUp className="size-3.5" />
      ) : sort === 'descend' ? (
        <ArrowDown className="size-3.5" />
      ) : (
        <ArrowUp className="size-3.5 opacity-40" />
      )}
    </button>
  );
}

export function ServerManagePage() {
  const { t } = useTranslation();
  const nodes = useServerNodes();
  const groups = useServerGroups();
  const routes = useServerRoutes();
  const update = useUpdateServerMutation();
  const drop = useDropServerMutation();
  const copy = useCopyServerMutation();
  const sort = useSortServerNodesMutation();
  const [searchKey, setSearchKey] = useState<string | undefined>();
  const [sortMode, setSortMode] = useState(false);
  const [onlineSort, setOnlineSort] = useState<'' | 'ascend' | 'descend'>('');
  const [typeFilter, setTypeFilter] = useState<string[]>([]);
  const [groupFilter, setGroupFilter] = useState<string[]>([]);
  const [orderedNodesOverride, setOrderedNodesOverride] = useState<admin.ServerNode[] | null>(null);
  const [sortingLoading, setSortingLoading] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(10);
  const [editing, setEditing] = useState<{
    type: admin.ServerTypeName;
    record?: admin.ServerNode;
    key: number;
  } | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const editorDependenciesReady =
    groups.isSuccess && groups.data !== undefined && routes.isSuccess && routes.data !== undefined;

  const orderedNodes = orderedNodesOverride ?? nodes.data ?? [];

  const searchedNodes =
    searchKey && orderedNodes
      ? orderedNodes.filter((node) => JSON.stringify(node).includes(searchKey))
      : orderedNodes;
  // Column controls are hidden in sort mode; reorder operates on the raw list,
  // so filtering and online-count sorting only apply while browsing.
  const filteredNodes = sortMode
    ? searchedNodes
    : applyServerNodeColumnControls(searchedNodes, { typeFilter, groupFilter, onlineSort });

  const pageCount = Math.max(1, Math.ceil(filteredNodes.length / pageSize));
  const activePage = Math.min(currentPage, pageCount);
  const visibleNodes = sortMode
    ? filteredNodes
    : filteredNodes.slice((activePage - 1) * pageSize, activePage * pageSize);

  const groupName = (ids: admin.ServerNode['group_id']) =>
    ids.map((id) => groups.data?.find((group) => group.id === Number(id))?.name).filter(Boolean);

  const openEditor = (type: admin.ServerTypeName, record?: admin.ServerNode) => {
    if (!editorDependenciesReady) return;
    setEditing((current) => ({ type, record, key: (current?.key ?? 0) + 1 }));
    setDrawerOpen(true);
  };

  const toggleNodeShow = (row: admin.ServerNode) => {
    update.mutate({ type: row.type, id: row.id, show: !row.show });
  };

  const copyNode = (row: admin.ServerNode) => {
    copy.mutate({ type: row.type, id: row.id });
  };

  const removeNode = async (row: admin.ServerNode) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.servers.warning),
      description: t(($) => $.admin.servers.confirm_delete_node),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    drop.mutate({ type: row.type, id: row.id });
  };

  const copyHost = async (host: string) => {
    if (await copyText(host)) toast.success(t(($) => $.admin.servers.copy_success));
    else toast.error(t(($) => $.admin.servers.copy_fail));
  };

  const moveNode = (id: number, direction: -1 | 1) => {
    const list = orderedNodes;
    const index = list.findIndex((node) => node.id === id);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= list.length) return;
    setOrderedNodesOverride(moveServerNodeByDragIndexes(list, index, target));
  };

  const cycleOnlineSort = () => {
    setCurrentPage(1);
    setOnlineSort((current) => (current === '' ? 'ascend' : current === 'ascend' ? 'descend' : ''));
  };

  const applyTypeFilter = (next: string[]) => {
    setCurrentPage(1);
    setTypeFilter(next);
  };

  const applyGroupFilter = (next: string[]) => {
    setCurrentPage(1);
    setGroupFilter(next);
  };

  const saveSort = () => {
    if (!sortMode) {
      setSortMode(true);
      return;
    }
    setSortingLoading(true);
    sort.mutate(createServerSortPayload(orderedNodes), {
      onSuccess: () => {
        setOrderedNodesOverride(null);
        setSortMode(false);
      },
      onSettled: () => {
        setSortingLoading(false);
      },
    });
  };

  const changePage = (page: number, nextSize: number) => {
    setCurrentPage(page);
    if (nextSize !== pageSize) {
      setPageSize(nextSize);
    }
  };

  const idColumn: DataTableColumn<admin.ServerNode> = {
    id: 'node_id',
    header: () => (
      <span className="inline-flex items-center">
        {t(($) => $.admin.servers.col_node_id)}
        <NodeFilterMenu
          items={NODE_TYPE_FILTERS}
          value={typeFilter}
          active={typeFilter.length > 0}
          onApply={applyTypeFilter}
        />
      </span>
    ),
    cell: ({ row }) => (
      <ServerTypeTag type={row.original.type}>
        {row.original.parent_id
          ? `${row.original.id} => ${row.original.parent_id}`
          : row.original.id}
      </ServerTypeTag>
    ),
  };

  const sortColumns: DataTableColumn<admin.ServerNode>[] = [
    {
      id: 'sort',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.common.sort)}</span>,
      cell: ({ row }) => {
        const index = orderedNodes.findIndex((node) => node.id === row.original.id);
        return (
          <div className="flex items-center justify-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index <= 0}
              onClick={() => moveNode(row.original.id, -1)}
              aria-label={t(($) => $.admin.servers.move_up)}
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedNodes.length - 1}
              onClick={() => moveNode(row.original.id, 1)}
              aria-label={t(($) => $.admin.servers.move_down)}
            >
              <ArrowDown className="size-4" />
            </Button>
          </div>
        );
      },
    },
    idColumn,
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.servers.col_node)}</span>,
      cell: ({ row }) => row.original.name,
    },
  ];

  const browseColumns: DataTableColumn<admin.ServerNode>[] = [
    idColumn,
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.servers.col_show)}</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          onCheckedChange={() => toggleNodeShow(row.original)}
          aria-label={t(($) => $.admin.servers.toggle_show_aria, { name: row.original.name })}
        />
      ),
    },
    {
      id: 'node',
      meta: { className: 'font-medium text-foreground' },
      header: () => (
        <HeaderTooltip title={t(($) => $.admin.servers.node_name)}>
          {t(($) => $.admin.servers.col_node)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-2">
          <AvailabilityDot status={row.original.available_status} />
          {row.original.name}
        </span>
      ),
    },
    {
      id: 'host',
      header: () => <span>{t(($) => $.admin.servers.col_address)}</span>,
      cell: ({ row }) => (
        <button
          type="button"
          className="cursor-pointer text-left tabular-nums"
          onClick={() => void copyHost(row.original.host)}
        >
          {row.original.host}:{row.original.port}
        </button>
      ),
    },
    {
      id: 'online',
      header: () => <OnlineSortHeader sort={onlineSort} onCycle={cycleOnlineSort} />,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-1.5 tabular-nums">
          <User className="size-4 text-muted-foreground" /> {row.original.online || 0}
        </span>
      ),
    },
    {
      id: 'rate',
      meta: { align: 'center' },
      header: () => (
        <HeaderTooltip title={t(($) => $.admin.servers.rate_tip)} className="justify-center">
          {t(($) => $.admin.servers.rate)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => (
        <Badge variant="secondary" className="min-w-14 justify-center tabular-nums">
          {row.original.rate} x
        </Badge>
      ),
    },
    {
      id: 'group',
      header: () => (
        <span className="inline-flex items-center">
          {t(($) => $.admin.servers.group)}
          <NodeFilterMenu
            items={(groups.data ?? []).map((group) => ({
              text: group.name,
              value: String(group.id),
            }))}
            value={groupFilter}
            active={groupFilter.length > 0}
            onApply={applyGroupFilter}
          />
        </span>
      ),
      cell: ({ row }) => (
        <div className="flex flex-wrap gap-1">
          {groupName(row.original.group_id).map((name) => (
            <Badge key={name} variant="secondary">
              {name}
            </Badge>
          ))}
        </div>
      ),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" data-testid={`node-actions-${row.original.id}`}>
              {t(($) => $.common.operation)}
              <ChevronDown className="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              disabled={!editorDependenciesReady}
              onClick={() => openEditor(row.original.type, row.original)}
              data-testid={`node-edit-${row.original.id}`}
            >
              <Pencil className="size-4" />
              {t(($) => $.common.edit)}
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => copyNode(row.original)}
              data-testid={`node-copy-${row.original.id}`}
            >
              <Copy className="size-4" />
              {t(($) => $.common.copy)}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onClick={() => void removeNode(row.original)}
              data-testid={`node-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              {t(($) => $.common.delete)}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ];

  const columns = sortMode ? sortColumns : browseColumns;
  const emptyText =
    nodes.isSuccess && nodes.data !== undefined && filteredNodes.length === 0
      ? t(($) => $.admin.servers.no_nodes)
      : undefined;

  return (
    <PageShell data-testid="server-manage-page">
      {nodes.isError ? (
        <ErrorState
          message={t(($) => $.admin.servers.nodes_load_failed)}
          onRetry={() => void nodes.refetch()}
        />
      ) : null}
      {groups.isError ? (
        <ErrorState
          message={t(($) => $.admin.servers.groups_load_failed_blocking)}
          onRetry={() => void groups.refetch()}
        />
      ) : null}
      {routes.isError ? (
        <ErrorState
          message={t(($) => $.admin.servers.routes_load_failed_blocking)}
          onRetry={() => void routes.refetch()}
        />
      ) : null}
      <ServerSortNavigationGuard when={sortMode} />
      <PageHeader
        title={t(($) => $.admin.servers.manage_title)}
        actions={
          <>
            {editorDependenciesReady ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button data-testid="node-add">
                    <Plus className="size-4" />
                    {t(($) => $.admin.servers.add_node)}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  {SERVER_TYPES.map((type) => (
                    <DropdownMenuItem
                      key={type}
                      onClick={() => openEditor(type)}
                      data-testid={`node-add-${type}`}
                    >
                      <ServerTypeTag type={type}>{SERVER_TYPE_LABELS[type]}</ServerTypeTag>
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button disabled data-testid="node-add">
                <Plus className="size-4" />
                {t(($) => $.admin.servers.add_node)}
              </Button>
            )}
            <Button
              variant={sortMode ? 'default' : 'outline'}
              onClick={saveSort}
              data-testid="node-sort-toggle"
            >
              {sortMode ? t(($) => $.admin.servers.save_sort) : t(($) => $.admin.servers.edit_sort)}
            </Button>
          </>
        }
      />

      <div className="w-full sm:max-w-xs">
        <Input
          aria-label={t(($) => $.admin.servers.search_nodes_aria)}
          placeholder={t(($) => $.admin.servers.search_placeholder)}
          onChange={(event) => {
            setSearchKey(event.target.value);
            setCurrentPage(1);
          }}
          data-testid="node-search"
        />
      </div>

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <DataTable
              columns={columns}
              data={visibleNodes}
              getRowKey={(row) => row.id}
              className="min-w-[1080px]"
              data-testid="server-nodes-table"
              empty={emptyText}
              emptyTestId="server-nodes-empty"
              virtualizer={{ enabled: visibleNodes.length > VIRTUALIZE_MIN_ROWS }}
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {!sortMode && filteredNodes.length > 0 ? (
        <ServerPagination
          current={activePage}
          pageSize={pageSize}
          total={filteredNodes.length}
          onChange={changePage}
        />
      ) : null}

      {nodes.isFetching || sortingLoading ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}

      <NodeEditor
        key={editing?.key ?? 0}
        open={drawerOpen && editorDependenciesReady}
        type={editing?.type ?? 'v2node'}
        record={editing?.record}
        nodes={nodes.data ?? []}
        groups={groups.data ?? []}
        routes={routes.data ?? []}
        dependenciesReady={editorDependenciesReady}
        onClose={() => setDrawerOpen(false)}
      />
    </PageShell>
  );
}

const SERVER_PAGE_SIZE_OPTIONS = [10, 50, 100, 500];

function ServerPagination({
  current,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  pageSize: number;
  total: number;
  onChange: (page: number, pageSize: number) => void;
}) {
  const { t } = useTranslation();
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  return (
    <div className="flex flex-wrap items-center justify-end gap-3">
      <span className="text-sm text-muted-foreground">
        {t(($) => $.admin.servers.total_items, { total })}
      </span>
      <Select value={String(pageSize)} onValueChange={(value) => onChange(1, Number(value))}>
        <SelectTrigger
          className="h-9 w-28"
          data-testid="node-page-size"
          aria-label={t(($) => $.admin.servers.page_size_aria)}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SERVER_PAGE_SIZE_OPTIONS.map((size) => (
            <SelectItem key={size} value={String(size)}>
              {size} {t(($) => $.common.items_per_page)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <div className="flex items-center gap-1">
        <Button
          variant="outline"
          size="sm"
          disabled={current <= 1}
          onClick={() => onChange(current - 1, pageSize)}
        >
          {t(($) => $.common.prev_page)}
        </Button>
        <span className="px-2 text-sm tabular-nums" data-testid="node-page">
          {current} / {pageCount}
        </span>
        <Button
          variant="outline"
          size="sm"
          disabled={current >= pageCount}
          onClick={() => onChange(current + 1, pageSize)}
        >
          {t(($) => $.common.next_page)}
        </Button>
      </div>
    </div>
  );
}

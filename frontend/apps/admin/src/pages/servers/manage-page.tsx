import { useCallback, useState } from 'react';
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
import { Spinner } from '@/components/ui/spinner';
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
  SERVER_SORT_LEAVE_PROMPT,
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
          <AlertDialogTitle>{SERVER_SORT_LEAVE_PROMPT}</AlertDialogTitle>
          <AlertDialogDescription>离开后，本次排序调整将不会保存。</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel asChild>
            <Button type="button" variant="outline" data-testid="server-sort-stay">
              取消
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
              确定
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
          aria-label="筛选"
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
            确定
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
            重置
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
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1.5 rounded-sm transition-colors outline-none select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={onCycle}
    >
      <HeaderTooltip title="在线人数">人数</HeaderTooltip>
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
      title: '警告',
      description: '确定要删除该节点吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate({ type: row.type, id: row.id });
  };

  const copyHost = async (host: string) => {
    if (await copyText(host)) toast.success('复制成功');
    else toast.error('复制失败');
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
        节点ID
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
      header: () => <span>排序</span>,
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
              aria-label="上移"
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedNodes.length - 1}
              onClick={() => moveNode(row.original.id, 1)}
              aria-label="下移"
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
      header: () => <span>节点</span>,
      cell: ({ row }) => row.original.name,
    },
  ];

  const browseColumns: DataTableColumn<admin.ServerNode>[] = [
    idColumn,
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>显隐</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          onCheckedChange={() => toggleNodeShow(row.original)}
          aria-label={`切换「${row.original.name}」显隐`}
        />
      ),
    },
    {
      id: 'node',
      meta: { className: 'font-medium text-foreground' },
      header: () => <HeaderTooltip title="节点名称">节点</HeaderTooltip>,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-2">
          <AvailabilityDot status={row.original.available_status} />
          {row.original.name}
        </span>
      ),
    },
    {
      id: 'host',
      header: () => <span>地址</span>,
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
        <HeaderTooltip title="流量倍率" className="justify-center">
          倍率
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
          权限组
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
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" data-testid={`node-actions-${row.original.id}`}>
              操作
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
              编辑
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => copyNode(row.original)}
              data-testid={`node-copy-${row.original.id}`}
            >
              <Copy className="size-4" />
              复制
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onClick={() => void removeNode(row.original)}
              data-testid={`node-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              删除
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ];

  const columns = sortMode ? sortColumns : browseColumns;
  const emptyText =
    nodes.isSuccess && nodes.data !== undefined && filteredNodes.length === 0
      ? '暂无节点'
      : undefined;

  return (
    <PageShell data-testid="server-manage-page">
      {nodes.isError ? (
        <ErrorState message="节点列表加载失败" onRetry={() => void nodes.refetch()} />
      ) : null}
      {groups.isError ? (
        <ErrorState message="权限组加载失败，无法编辑节点" onRetry={() => void groups.refetch()} />
      ) : null}
      {routes.isError ? (
        <ErrorState
          message="路由列表加载失败，无法编辑节点"
          onRetry={() => void routes.refetch()}
        />
      ) : null}
      <ServerSortNavigationGuard when={sortMode} />
      <PageHeader
        title="节点管理"
        actions={
          <>
            {editorDependenciesReady ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button data-testid="node-add">
                    <Plus className="size-4" />
                    添加节点
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
                添加节点
              </Button>
            )}
            <Button
              variant={sortMode ? 'default' : 'outline'}
              onClick={saveSort}
              data-testid="node-sort-toggle"
            >
              {sortMode ? '保存排序' : '编辑排序'}
            </Button>
          </>
        }
      />

      <div className="w-full sm:max-w-xs">
        <Input
          aria-label="搜索节点"
          placeholder="输入任意关键字搜索"
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
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
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
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  return (
    <div className="flex flex-wrap items-center justify-end gap-3">
      <span className="text-sm text-muted-foreground">共 {total} 条</span>
      <Select value={String(pageSize)} onValueChange={(value) => onChange(1, Number(value))}>
        <SelectTrigger className="h-9 w-28" data-testid="node-page-size">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SERVER_PAGE_SIZE_OPTIONS.map((size) => (
            <SelectItem key={size} value={String(size)}>
              {size} 条/页
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
          上一页
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
          下一页
        </Button>
      </div>
    </div>
  );
}

import { useEffect, useMemo, useState, type ComponentProps } from 'react';
import dayjs from 'dayjs';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useFieldArray, useForm, useFormState, useWatch } from 'react-hook-form';
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Ban,
  ChevronDown,
  ChevronsUpDown,
  Copy,
  FileSpreadsheet,
  ListFilter,
  Mail,
  Pencil,
  Plus,
  ReceiptText,
  RefreshCw,
  SlidersHorizontal,
  Trash2,
  UserPlus,
  UsersRound,
  X,
} from 'lucide-react';
import type { admin, AdminFilter } from '@v2board/api-client';
import type { AdminUserRow } from '@v2board/types';
import { copyText } from '@v2board/config/clipboard';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import { takeStoredAdminFilters } from '@/lib/stored-admin-filters';
import {
  useAdminPlans,
  useAdminUsers,
  useAssignOrderMutation,
  useBanUsersMutation,
  useDeleteAllUsersMutation,
  useDeleteUserMutation,
  useDumpUsersCsvMutation,
  useGenerateUserMutation,
  useResetUserSecretMutation,
  useSendMailToUsersMutation,
  useServerGroups,
} from '@/lib/queries';
import { UserManageDrawer } from '@/components/user-manage-drawer';
import { UserTrafficModal } from '@/components/user-traffic-modal';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Field, FieldError, FieldLabel, FieldLegend, FieldSet } from '@/components/ui/field';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { PaginationControl } from '@/components/ui/pagination';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { cn } from '@/lib/cn';
import {
  assignOrderSchema,
  generateUserSchema,
  sendMailSchema,
  userFilterSchema,
  type AssignOrderValues,
  type GenerateUserValues,
  type SendMailValues,
  type UserFilterValues,
} from './user-action-form-schema';

interface QueryState {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
}

interface PlanOption {
  label: string;
  value: number;
}

type GenerateUserPayload = Parameters<typeof admin.generateUser>[1];

interface FilterField {
  key: string;
  title: string;
  condition: string[];
  type?: 'text' | 'select' | 'date';
  options?: { label: string; value: string | number }[];
}

const PLAN_NONE = 'null';

const PAGE_SIZE_OPTIONS = [10, 50, 100, 150];

const PAGINATION_LABELS = {
  itemsPerPage: '条/页',
  nextPage: '下一页',
  nextWindow: '向后 5 页',
  previousPage: '上一页',
  previousWindow: '向前 5 页',
};

const PERIOD_TEXT: Record<string, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '流量重置包',
};

const PERIOD_OPTIONS = Object.keys(PERIOD_TEXT).map((period) => ({
  value: period,
  label: PERIOD_TEXT[period] ?? period,
}));

// Cross-page Tier-1 contract: the order manager (and the dashboard) seed an
// AdminFilter[] into sessionStorage then navigate to /user. Read it on mount,
// apply it as the initial filter, and clear it.
function readStoredUserFilter(): AdminFilter[] {
  return takeStoredAdminFilters('v2board-admin-user-filter');
}

function downloadText(name: string, buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = name;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

function downloadGeneratedUserCsv(buffer: unknown) {
  downloadText(`USER ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`, buffer);
}

function requestErrorMessage(error: unknown) {
  return error instanceof Error && error.message ? error.message : '请求失败';
}

function planSelectItems(plans: PlanOption[], includeEmpty = false) {
  return [
    ...(includeEmpty ? [{ value: PLAN_NONE, label: '无' }] : []),
    ...plans.map((plan) => ({ value: String(plan.value), label: plan.label })),
  ];
}

export default function UsersPage() {
  const navigate = useNavigate();
  const [currentUnixTime, setCurrentUnixTime] = useState(() => Date.now() / 1000);
  const [query, setQuery] = useState<QueryState>(() => ({
    current: 1,
    pageSize: 10,
    filter: readStoredUserFilter(),
  }));
  const users = useAdminUsers(query);
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const remove = useDeleteUserMutation();
  const resetSecret = useResetUserSecretMutation();
  const generate = useGenerateUserMutation();
  const dumpCsv = useDumpUsersCsvMutation();
  const sendMail = useSendMailToUsersMutation();
  const banUsers = useBanUsersMutation();
  const deleteAll = useDeleteAllUsersMutation();
  const planData = plans.data;
  const groupData = groups.data;
  const plansReady = !plans.isError && planData !== undefined;
  const groupsReady = !groups.isError && groupData !== undefined;

  const [editing, setEditing] = useState<AdminUserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [mailOpen, setMailOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [assigning, setAssigning] = useState<AdminUserRow | null>(null);
  const [trafficUser, setTrafficUser] = useState<AdminUserRow | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setCurrentUnixTime(Date.now() / 1000), 60_000);
    return () => window.clearInterval(timer);
  }, []);

  const planOptions: PlanOption[] = plansReady
    ? planData.map((plan) => ({ label: plan.name, value: plan.id }))
    : [];

  const groupMap = new Map<number, string>();
  if (groupsReady) {
    for (const group of groupData) groupMap.set(group.id, group.name);
  }

  const filterFields = useMemo<FilterField[]>(
    () => [
      { key: 'email', title: '邮箱', condition: ['模糊'] },
      { key: 'id', title: '用户ID', condition: ['=', '>=', '>', '<', '<='] },
      ...(plansReady
        ? [
            {
              key: 'plan_id',
              title: '订阅',
              condition: ['='],
              type: 'select' as const,
              options: [
                { label: '无订阅', value: PLAN_NONE },
                ...planOptions.map((plan) => ({ label: plan.label, value: plan.value })),
              ],
            },
          ]
        : []),
      { key: 'transfer_enable', title: '流量', condition: ['>=', '>', '<', '<='] },
      { key: 'd', title: '下行', condition: ['>=', '>', '<', '<='] },
      { key: 'expired_at', title: '到期时间', condition: ['>=', '>', '<', '<='], type: 'date' },
      { key: 'uuid', title: 'UUID', condition: ['='] },
      { key: 'token', title: 'TOKEN', condition: ['='] },
      {
        key: 'banned',
        title: '账号状态',
        condition: ['='],
        type: 'select',
        options: [
          { label: '正常', value: 0 },
          { label: '封禁', value: 1 },
        ],
      },
      { key: 'invite_by_email', title: '邀请人邮箱', condition: ['模糊'] },
      { key: 'invite_user_id', title: '邀请人ID', condition: ['='] },
      { key: 'remarks', title: '备注', condition: ['模糊'] },
      {
        key: 'is_admin',
        title: '管理员',
        condition: ['='],
        type: 'select',
        options: [
          { label: '是', value: 1 },
          { label: '否', value: 0 },
        ],
      },
    ],
    [planOptions, plansReady],
  );

  const data = users.data?.data ?? [];
  const total = users.data?.total ?? 0;

  const setFilter = (filter: AdminFilter[]) =>
    setQuery((state) => ({ ...state, current: 1, filter }));

  const sortBy = (key: string) =>
    setQuery((state) => ({
      ...state,
      current: 1,
      sort: key,
      sort_type: state.sort === key && state.sort_type === 'ASC' ? 'DESC' : 'ASC',
    }));

  const jumpOrderFilter = (key: string, condition: string, value: string | number) => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    navigate('/order');
  };

  const resetUserSecret = async (row: AdminUserRow) => {
    const confirmed = await confirmDialog({
      title: '重置安全信息',
      description: `确定要重置${row.email}的安全信息吗？`,
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    resetSecret.mutate(row.id, {
      onSuccess: () => {
        toast.success('重置成功');
      },
    });
  };

  const deleteUser = async (row: AdminUserRow) => {
    const confirmed = await confirmDialog({
      title: '删除用户',
      description: `确定要删除${row.email}的用户信息吗？`,
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    remove.mutate(row.id, {
      onSuccess: () => {
        toast.success('删除成功');
      },
    });
  };

  const copySubscribeUrl = async (row: AdminUserRow) => {
    if (await copyText(row.subscribe_url)) toast.success('复制成功');
    else toast.error('复制失败');
  };

  const runUserAction = (key: string, row: AdminUserRow) => {
    if (key === 'edit') setEditing(row);
    if (key === 'assign' && plansReady) setAssigning(row);
    if (key === 'copy') void copySubscribeUrl(row);
    if (key === 'reset') void resetUserSecret(row);
    if (key === 'orders') jumpOrderFilter('user_id', '=', row.id);
    if (key === 'invite') setFilter([{ key: 'invite_user_id', condition: '=', value: row.id }]);
    if (key === 'traffic') setTrafficUser(row);
    if (key === 'delete') void deleteUser(row);
  };

  const exportCsv = () => {
    const toastId = toast.loading('导出中');
    dumpCsv.mutate(query.filter, {
      onSuccess: (response) => {
        downloadText(`${formatDateTime(Date.now() / 1000)}.csv`, response.buffer);
      },
      onSettled: () => {
        toast.dismiss(toastId);
      },
    });
  };

  const bulkBan = async () => {
    const confirmed = await confirmDialog({
      title: '提醒',
      description: '确定要进行封禁吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    banUsers.mutate(query.filter);
  };

  const bulkDelete = async () => {
    const confirmed = await confirmDialog({
      title: '提醒',
      description: '确定要进行删除吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    deleteAll.mutate(query.filter);
  };

  const sortHeader = (label: string, key: string) => {
    function SortHeader() {
      const active = query.sort === key;
      const Icon = active ? (query.sort_type === 'ASC' ? ArrowUp : ArrowDown) : ChevronsUpDown;
      return (
        <button
          type="button"
          className="inline-flex items-center gap-1.5 rounded-sm outline-none transition-colors select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onClick={() => sortBy(key)}
        >
          {label}
          <Icon className={cn('size-3.5', !active && 'opacity-50')} aria-hidden="true" />
        </button>
      );
    }
    return SortHeader;
  };

  const renderEmail = (row: AdminUserRow) => {
    const onlineAt = (row as AdminUserRow & { t?: number | null }).t;
    const online = !(currentUnixTime - 600 > Number(onlineAt));
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="inline-flex items-center gap-2">
            <span
              className={cn(
                'size-2 shrink-0 rounded-full',
                online ? 'bg-success' : 'bg-muted-foreground',
              )}
            />
            {row.email}
          </span>
        </TooltipTrigger>
        <TooltipContent>
          {onlineAt ? `最后在线${formatDateTime(Number(onlineAt))}` : '从未在线'}
        </TooltipContent>
      </Tooltip>
    );
  };

  const renderDeviceLimit = (row: AdminUserRow) => {
    const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;
    const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';
    const text = `${deviceCount} / ${deviceLimit}`;
    return row.ips ? (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="tabular-nums">{text}</span>
        </TooltipTrigger>
        <TooltipContent>{row.ips}</TooltipContent>
      </Tooltip>
    ) : (
      <span className="tabular-nums">{text}</span>
    );
  };

  const renderExpiredAt = (value: number | null) => {
    const expired = value !== null && value < currentUnixTime;
    return (
      <StatusBadge tone={expired ? 'destructive' : 'success'}>
        {value ? formatDateMinuteSlash(value) : value === null ? '长期有效' : '-'}
      </StatusBadge>
    );
  };

  const columns: DataTableColumn<AdminUserRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: sortHeader('ID', 'id'),
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'email',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>邮箱</span>,
      cell: ({ row }) => renderEmail(row.original),
    },
    {
      id: 'banned',
      header: sortHeader('状态', 'banned'),
      cell: ({ row }) => (
        <StatusBadge tone={row.original.banned ? 'destructive' : 'success'}>
          {row.original.banned ? '封禁' : '正常'}
        </StatusBadge>
      ),
    },
    {
      id: 'plan_id',
      header: sortHeader('订阅', 'plan_id'),
      cell: ({ row }) => row.original.plan_name || '-',
    },
    {
      id: 'group_id',
      header: sortHeader('权限组', 'group_id'),
      cell: ({ row }) =>
        row.original.group_id != null ? (groupMap.get(row.original.group_id) ?? '-') : '-',
    },
    {
      id: 'total_used',
      meta: { align: 'right' },
      header: sortHeader('已用(G)', 'total_used'),
      cell: ({ row }) => {
        const over =
          parseFloat(String(row.original.total_used)) >
          parseFloat(String(row.original.transfer_enable));
        return (
          <StatusBadge tone={over ? 'destructive' : 'success'}>
            {row.original.total_used}
          </StatusBadge>
        );
      },
    },
    {
      id: 'transfer_enable',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader('流量(G)', 'transfer_enable'),
      cell: ({ row }) => row.original.transfer_enable,
    },
    {
      id: 'device',
      meta: { align: 'right' },
      header: sortHeader('设备数', 'updated_at'),
      cell: ({ row }) => renderDeviceLimit(row.original),
    },
    {
      id: 'expired_at',
      header: sortHeader('到期时间', 'expired_at'),
      cell: ({ row }) => renderExpiredAt(row.original.expired_at),
    },
    {
      id: 'balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader('余额', 'balance'),
      cell: ({ row }) => row.original.balance,
    },
    {
      id: 'commission_balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader('佣金', 'commission_balance'),
      cell: ({ row }) => row.original.commission_balance,
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: sortHeader('加入时间', 'created_at'),
      cell: ({ row }) => formatDateMinuteSlash(row.original.created_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <UserRowActions row={row.original} onAction={runUserAction} assignDisabled={!plansReady} />
      ),
    },
  ];

  return (
    <PageShell data-testid="users-page">
      {users.isError ? (
        <ErrorState message="用户列表加载失败" onRetry={() => void users.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      {groups.isError ? (
        <ErrorState message="权限组加载失败" onRetry={() => void groups.refetch()} />
      ) : null}
      <PageHeader
        title="用户管理"
        actions={
          <Button
            onClick={() => setCreating(true)}
            disabled={!plansReady}
            data-testid="user-create"
          >
            <UserPlus className="size-4" />
            创建用户
          </Button>
        }
      />

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  variant={query.filter.length ? 'default' : 'outline'}
                  size="sm"
                  onClick={() => setFilterOpen(true)}
                  data-testid="user-filter-open"
                >
                  <ListFilter className="size-4" />
                  过滤器
                  {query.filter.length ? (
                    <span className="ml-1 inline-flex size-5 items-center justify-center rounded-full bg-primary-foreground text-xs text-primary">
                      {query.filter.length}
                    </span>
                  ) : null}
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" data-testid="user-bulk-actions">
                      <SlidersHorizontal className="size-4" />
                      操作
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuItem onClick={exportCsv} data-testid="user-export-csv">
                      <FileSpreadsheet className="size-4" />
                      导出CSV
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => setMailOpen(true)}
                      data-testid="user-send-mail"
                    >
                      <Mail className="size-4" />
                      发送邮件
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      disabled={!query.filter.length}
                      onClick={() => void bulkBan()}
                      data-testid="user-bulk-ban"
                    >
                      <Ban className="size-4" />
                      批量封禁
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      variant="destructive"
                      disabled={!query.filter.length}
                      onClick={() => void bulkDelete()}
                      data-testid="user-bulk-delete"
                    >
                      <Trash2 className="size-4" />
                      批量删除
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
                {query.filter.length ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setFilter([])}
                    data-testid="user-filter-reset"
                  >
                    清除筛选
                  </Button>
                ) : null}
              </div>
              <p className="text-xs text-muted-foreground">
                Tips：可以使用过滤器过滤后再使用操作对过滤的用户进行操作。
              </p>
            </div>

            <DataTable
              columns={columns}
              data={data}
              getRowKey={(row) => row.id}
              className="min-w-[1280px]"
              data-testid="users-table"
              empty={
                !users.isError && users.data !== undefined && data.length === 0
                  ? '暂无用户'
                  : undefined
              }
              emptyTestId="users-empty"
              virtualizer={{ enabled: data.length > VIRTUALIZE_MIN_ROWS }}
            />

            {total > 0 ? (
              <PaginationControl
                current={query.current}
                pageSize={query.pageSize}
                total={total}
                pageSizeOptions={PAGE_SIZE_OPTIONS}
                labels={PAGINATION_LABELS}
                onChange={(page, pageSize) =>
                  setQuery((state) => ({ ...state, current: page, pageSize }))
                }
                testIds={{ page: 'user-page', pageSize: 'user-page-size' }}
              />
            ) : null}
          </CardContent>
        </Card>
      </TooltipProvider>

      <UserFilterSheet
        open={filterOpen}
        onOpenChange={setFilterOpen}
        fields={filterFields}
        value={query.filter}
        onApply={setFilter}
      />

      <UserManageDrawer
        userId={editing?.id}
        open={editing != null}
        onClose={() => setEditing(null)}
      />

      <GenerateUserModal
        open={creating && plansReady}
        plans={planOptions}
        loading={generate.isPending}
        onClose={() => setCreating(false)}
        onSubmit={async (values) => {
          if (!plansReady) return;
          const response = await generate.mutateAsync(values);
          if (values.generate_count) downloadGeneratedUserCsv(response.buffer);
          setCreating(false);
        }}
      />

      <SendMailModal
        open={mailOpen}
        filter={query.filter}
        loading={sendMail.isPending}
        onClose={() => setMailOpen(false)}
        onSubmit={async (values) => {
          await sendMail.mutateAsync({ filter: query.filter, ...values });
          toast.success('已加入队列执行');
          setMailOpen(false);
        }}
      />

      <AssignOrderModal
        user={plansReady ? assigning : null}
        plans={planOptions}
        onClose={() => setAssigning(null)}
      />

      <UserTrafficModal
        userId={trafficUser?.id}
        open={trafficUser != null}
        onClose={() => setTrafficUser(null)}
      />

      {users.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

function UserRowActions({
  row,
  onAction,
  assignDisabled,
}: {
  row: AdminUserRow;
  onAction: (key: string, row: AdminUserRow) => void;
  assignDisabled: boolean;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" data-testid={`user-actions-${row.id}`}>
          操作
          <ChevronDown className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => onAction('edit', row)} data-testid={`user-edit-${row.id}`}>
          <Pencil className="size-4" />
          编辑
        </DropdownMenuItem>
        <DropdownMenuItem disabled={assignDisabled} onClick={() => onAction('assign', row)}>
          <Plus className="size-4" />
          分配订单
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('copy', row)} data-testid={`user-copy-${row.id}`}>
          <Copy className="size-4" />
          复制订阅URL
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('reset', row)}>
          <RefreshCw className="size-4" />
          重置UUID及订阅URL
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('orders', row)}>
          <ReceiptText className="size-4" />
          TA的订单
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('invite', row)}>
          <UsersRound className="size-4" />
          TA的邀请
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onAction('traffic', row)}>
          <Activity className="size-4" />
          TA的流量记录
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onClick={() => onAction('delete', row)}
          data-testid={`user-delete-${row.id}`}
        >
          <Trash2 className="size-4" />
          删除用户
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AmountInput({ suffix, ...props }: ComponentProps<typeof Input> & { suffix: string }) {
  return (
    <div className="relative">
      <Input className="pr-8" {...props} />
      <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
        {suffix}
      </span>
    </div>
  );
}

function UserFilterSheet({
  open,
  onOpenChange,
  fields,
  value,
  onApply,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fields: FilterField[];
  value: AdminFilter[];
  onApply: (filter: AdminFilter[]) => void;
}) {
  const form = useForm<UserFilterValues>({
    resolver: zodResolver(userFilterSchema),
    defaultValues: { rows: value },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const {
    fields: filterRows,
    append,
    remove,
  } = useFieldArray({
    control: form.control,
    name: 'rows',
  });
  const rows = useWatch({ control: form.control, name: 'rows' }) ?? [];

  useEffect(() => {
    if (open) form.reset({ rows: value });
  }, [form, open, value]);

  const fieldOf = (key: string) => fields.find((field) => field.key === key) ?? fields[0]!;

  const addRow = () => {
    const field = fields[0]!;
    append({ key: field.key, condition: field.condition[0]!, value: '' });
  };

  const changeField = (index: number, key: string) => {
    const field = fieldOf(key);
    form.setValue(`rows.${index}.key`, key, { shouldDirty: true });
    form.setValue(`rows.${index}.condition`, field.condition[0]!, { shouldDirty: true });
    form.setValue(`rows.${index}.value`, '', { shouldDirty: true, shouldValidate: true });
  };

  const apply = form.handleSubmit(({ rows: nextRows }) => {
    onApply(nextRows);
    onOpenChange(false);
  });

  const reset = () => {
    form.reset({ rows: [] });
    onApply([]);
    onOpenChange(false);
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="flex w-full flex-col gap-0 overflow-hidden p-0 sm:max-w-md"
        data-testid="user-filter-sheet"
      >
        <SheetHeader className="border-b border-border px-6 py-4">
          <SheetTitle>过滤器</SheetTitle>
          <SheetDescription>组合字段条件以筛选用户列表。</SheetDescription>
        </SheetHeader>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-4">
          {filterRows.length === 0 ? (
            <p className="text-sm text-muted-foreground">点击下方按钮添加过滤条件。</p>
          ) : null}
          {filterRows.map((filterRow, index) => {
            const row = rows[index] ?? filterRow;
            const field = fieldOf(row.key);
            const valueError = formErrors.rows?.[index]?.value;
            return (
              <div key={filterRow.id} className="space-y-2 rounded-md border border-border p-3">
                <div className="flex items-center gap-2">
                  <Select value={row.key} onValueChange={(key) => changeField(index, key)}>
                    <SelectTrigger
                      className="flex-1"
                      aria-label={`筛选字段 ${index + 1}`}
                      data-testid={`user-filter-field-${index}`}
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {fields.map((item) => (
                        <SelectItem key={item.key} value={item.key}>
                          {item.title}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Controller
                    control={form.control}
                    name={`rows.${index}.condition`}
                    render={({ field: conditionField }) => (
                      <Select value={conditionField.value} onValueChange={conditionField.onChange}>
                        <SelectTrigger
                          className="w-24"
                          aria-label={`筛选条件 ${index + 1}`}
                          data-testid={`user-filter-condition-${index}`}
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {field.condition.map((condition) => (
                            <SelectItem key={condition} value={condition}>
                              {condition}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-9 shrink-0 text-muted-foreground"
                    aria-label="删除条件"
                    onClick={() => remove(index)}
                    data-testid={`user-filter-remove-${index}`}
                  >
                    <X className="size-4" />
                  </Button>
                </div>
                <Field data-invalid={Boolean(valueError)}>
                  <Controller
                    control={form.control}
                    name={`rows.${index}.value`}
                    render={({ field: valueField }) => (
                      <FilterValueInput
                        index={index}
                        field={field}
                        value={valueField.value}
                        onChange={valueField.onChange}
                      />
                    )}
                  />
                  <FieldError errors={[valueError]} />
                </Field>
              </div>
            );
          })}

          <Button type="button" variant="outline" onClick={addRow} data-testid="user-filter-add">
            <Plus className="size-4" />
            添加条件
          </Button>
        </div>

        <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
          <Button
            type="button"
            variant="outline"
            onClick={reset}
            data-testid="user-filter-reset-all"
          >
            重置
          </Button>
          <Button type="button" onClick={() => void apply()} data-testid="user-filter-apply">
            确定
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function FilterValueInput({
  index,
  field,
  value,
  onChange,
}: {
  index: number;
  field: FilterField;
  value: AdminFilter['value'];
  onChange: (value: AdminFilter['value']) => void;
}) {
  if (field.type === 'select') {
    const options = field.options ?? [];
    const current =
      value == null ? undefined : options.find((option) => String(option.value) === String(value));
    return (
      <Select
        value={current ? String(current.value) : undefined}
        onValueChange={(next) => {
          const option = options.find((item) => String(item.value) === next);
          onChange(option ? option.value : next);
        }}
      >
        <SelectTrigger
          className="w-full"
          aria-label={`筛选值 ${index + 1}`}
          data-testid={`user-filter-value-${index}`}
        >
          <SelectValue placeholder="请选择" />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={String(option.value)} value={String(option.value)}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  if (field.type === 'date') {
    return (
      <Input
        type="datetime-local"
        aria-label={`筛选值 ${index + 1}`}
        value={value ? dayjs(1000 * Number(value)).format('YYYY-MM-DDTHH:mm') : ''}
        onChange={(event) =>
          onChange(event.target.value ? String(dayjs(event.target.value).unix()) : '')
        }
        data-testid={`user-filter-value-${index}`}
      />
    );
  }

  return (
    <Input
      placeholder="欲检索内容"
      aria-label={`筛选值 ${index + 1}`}
      value={value == null ? '' : String(value)}
      onChange={(event) => onChange(event.target.value)}
      data-testid={`user-filter-value-${index}`}
    />
  );
}

function GenerateUserModal({
  open,
  plans,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: PlanOption[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: GenerateUserPayload) => Promise<void>;
}) {
  const form = useForm<GenerateUserValues>({
    resolver: zodResolver(generateUserSchema),
    defaultValues: {
      email_prefix: '',
      email_suffix: '',
      password: '',
      plan_id: null,
      expired_at: null,
      generate_count: '',
    },
  });
  // Read form state through the useFormState subscription instead of the
  // mutable form.formState proxy: the React Compiler caches proxy reads, which
  // drops react-hook-form's render-time access tracking and freezes error UI.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });
  const emailPrefix = useWatch({ control: form.control, name: 'email_prefix' });
  const generateCount = useWatch({ control: form.control, name: 'generate_count' });

  useEffect(() => {
    if (!open) form.reset();
  }, [form, open]);

  const close = () => {
    form.reset();
    onClose();
  };

  const planItems = planSelectItems(plans, true);
  const submit = form.handleSubmit(async (values) => {
    form.clearErrors('root.serverError');
    const emailPrefix = values.email_prefix.trim();
    const generateCount = values.generate_count.trim();
    const payload: GenerateUserPayload = {
      email_suffix: values.email_suffix.trim(),
      ...(emailPrefix ? { email_prefix: emailPrefix } : {}),
      ...(generateCount ? { generate_count: generateCount } : {}),
      ...(values.password ? { password: values.password } : {}),
      ...(values.plan_id != null ? { plan_id: values.plan_id } : {}),
      ...(values.expired_at ? { expired_at: values.expired_at } : {}),
    };
    try {
      await onSubmit(payload);
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-generate-dialog">
        <DialogHeader>
          <DialogTitle>创建用户</DialogTitle>
          <DialogDescription>批量创建用户并设置初始订阅与到期时间。</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={submit} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <FieldSet
            data-invalid={Boolean(
              formErrors.email_prefix || formErrors.email_suffix,
            )}
          >
            <FieldLegend variant="label">邮箱</FieldLegend>
            <div className="flex items-center gap-2">
              {!generateCount ? (
                <Controller
                  control={form.control}
                  name="email_prefix"
                  render={({ field }) => (
                    <Input
                      {...field}
                      placeholder="账号（批量生成请留空）"
                      onChange={(event) => {
                        field.onChange(event);
                        if (event.target.value) {
                          form.setValue('generate_count', '', { shouldValidate: true });
                        }
                      }}
                      data-testid="generate-email-prefix"
                      aria-invalid={Boolean(formErrors.email_prefix)}
                    />
                  )}
                />
              ) : null}
              <span className="text-muted-foreground">@</span>
              <Controller
                control={form.control}
                name="email_suffix"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    placeholder="域"
                    data-testid="generate-email-suffix"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
            </div>
            <FieldError
              errors={[formErrors.email_prefix, formErrors.email_suffix]}
            />
          </FieldSet>
          <Field>
            <FieldLabel htmlFor="generate-password">密码</FieldLabel>
            <Controller
              control={form.control}
              name="password"
              render={({ field }) => (
                <Input {...field} id="generate-password" placeholder="留空则密码与邮箱相同" />
              )}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="generate-expired">到期时间</FieldLabel>
            <Controller
              control={form.control}
              name="expired_at"
              render={({ field }) => (
                <Input
                  id="generate-expired"
                  type="date"
                  placeholder="请选择用户到期日期，为空则不限制到期时间"
                  value={field.value ? dayjs(1000 * Number(field.value)).format('YYYY-MM-DD') : ''}
                  onChange={(event) =>
                    field.onChange(
                      event.target.value ? String(dayjs(event.target.value).unix()) : null,
                    )
                  }
                  data-testid="generate-expired"
                />
              )}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="generate-plan">订阅计划</FieldLabel>
            <Controller
              control={form.control}
              name="plan_id"
              render={({ field }) => (
                <Select
                  value={field.value != null ? String(field.value) : PLAN_NONE}
                  onValueChange={(value) =>
                    field.onChange(value === PLAN_NONE ? null : Number(value))
                  }
                >
                  <SelectTrigger id="generate-plan" className="w-full">
                    <SelectValue placeholder="请选择用户订阅计划" />
                  </SelectTrigger>
                  <SelectContent>
                    {planItems.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
          </Field>
          {!emailPrefix ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="generate-count">生成数量</FieldLabel>
              <Controller
                control={form.control}
                name="generate_count"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    id="generate-count"
                    placeholder="如果为批量生成请输入生成数量"
                    onChange={(event) => {
                      field.onChange(event);
                      if (event.target.value) {
                        form.setValue('email_prefix', '', { shouldValidate: true });
                      }
                    }}
                    data-testid="generate-count"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={loading || isSubmitting}
              loading={loading || isSubmitting}
              data-testid="generate-submit"
            >
              生成
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function SendMailModal({
  open,
  filter,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  filter: AdminFilter[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: SendMailValues) => Promise<void>;
}) {
  const form = useForm<SendMailValues>({
    resolver: zodResolver(sendMailSchema),
    defaultValues: { subject: '', content: '' },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  useEffect(() => {
    if (!open) form.reset();
  }, [form, open]);

  const close = () => {
    form.reset();
    onClose();
  };
  const submit = form.handleSubmit(async (values) => {
    form.clearErrors('root.serverError');
    try {
      await onSubmit(values);
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-send-mail-dialog">
        <DialogHeader>
          <DialogTitle>发送邮件</DialogTitle>
          <DialogDescription>向当前筛选范围内的用户发送邮件。</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={submit} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field>
            <FieldLabel htmlFor="send-mail-recipient">收件人</FieldLabel>
            <Input
              id="send-mail-recipient"
              disabled
              value={filter.length ? '过滤用户' : '全部用户'}
            />
          </Field>
          <Field data-invalid={Boolean(formErrors.subject)}>
            <FieldLabel htmlFor="send-mail-subject">主题</FieldLabel>
            <Controller
              control={form.control}
              name="subject"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="send-mail-subject"
                  placeholder="请输入邮件主题"
                  data-testid="send-mail-subject"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.subject]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.content)}>
            <FieldLabel htmlFor="send-mail-content">发送内容</FieldLabel>
            <Controller
              control={form.control}
              name="content"
              render={({ field, fieldState }) => (
                <Textarea
                  {...field}
                  id="send-mail-content"
                  rows={12}
                  placeholder="请输入邮件内容"
                  data-testid="send-mail-content"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.content]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={loading || isSubmitting}
              loading={loading || isSubmitting}
              data-testid="send-mail-submit"
            >
              确定
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function AssignOrderModal({
  user,
  plans,
  onClose,
}: {
  user: AdminUserRow | null;
  plans: PlanOption[];
  onClose: () => void;
}) {
  const assign = useAssignOrderMutation();
  const form = useForm<AssignOrderValues>({
    resolver: zodResolver(assignOrderSchema),
    defaultValues: {
      email: user?.email ?? '',
      plan_id: undefined,
      period: undefined,
      total_amount: '',
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  useEffect(() => {
    form.reset({
      email: user?.email ?? '',
      plan_id: undefined,
      period: undefined,
      total_amount: '',
    });
  }, [form, user]);

  const close = () => {
    form.reset();
    onClose();
  };

  const doAssign = form.handleSubmit(async (values) => {
    // total_amount stays the raw entered value; the api-client applies the ×100
    // cents conversion. Preserving the raw payload here is the contract.
    form.clearErrors('root.serverError');
    try {
      await assign.mutateAsync(values);
      close();
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={Boolean(user)} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-assign-dialog">
        <DialogHeader>
          <DialogTitle>订单分配</DialogTitle>
          <DialogDescription>为当前用户创建并分配订阅订单。</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={doAssign} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field data-invalid={Boolean(formErrors.email)}>
            <FieldLabel htmlFor="assign-email">用户邮箱</FieldLabel>
            <Controller
              control={form.control}
              name="email"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="assign-email"
                  type="email"
                  placeholder="请输入用户邮箱"
                  data-testid="assign-email"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.email]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.plan_id)}>
            <FieldLabel htmlFor="user-assign-plan">请选择订阅</FieldLabel>
            <Controller
              control={form.control}
              name="plan_id"
              render={({ field }) => (
                <Select
                  value={field.value != null ? String(field.value) : undefined}
                  onValueChange={(value) => field.onChange(Number(value))}
                >
                  <SelectTrigger
                    id="user-assign-plan"
                    className="w-full"
                    aria-invalid={Boolean(formErrors.plan_id)}
                  >
                    <SelectValue placeholder="请选择订阅" />
                  </SelectTrigger>
                  <SelectContent>
                    {plans.map((plan) => (
                      <SelectItem key={plan.value} value={String(plan.value)}>
                        {plan.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
            <FieldError errors={[formErrors.plan_id]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.period)}>
            <FieldLabel htmlFor="user-assign-period">请选择周期</FieldLabel>
            <Controller
              control={form.control}
              name="period"
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange}>
                  <SelectTrigger
                    id="user-assign-period"
                    className="w-full"
                    aria-invalid={Boolean(formErrors.period)}
                  >
                    <SelectValue placeholder="请选择周期" />
                  </SelectTrigger>
                  <SelectContent>
                    {PERIOD_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
            <FieldError errors={[formErrors.period]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.total_amount)}>
            <FieldLabel htmlFor="assign-amount">支付金额</FieldLabel>
            <Controller
              control={form.control}
              name="total_amount"
              render={({ field, fieldState }) => (
                <AmountInput
                  {...field}
                  id="assign-amount"
                  suffix="¥"
                  placeholder="请输入需要支付的金额"
                  data-testid="assign-amount"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.total_amount]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={assign.isPending || isSubmitting}
              loading={assign.isPending || isSubmitting}
              data-testid="assign-submit"
            >
              确定
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

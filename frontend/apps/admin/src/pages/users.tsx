import { useEffect, useMemo, useState, type ComponentProps, type ReactNode } from 'react';
import dayjs from 'dayjs';
import { useNavigate } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
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
import type { AdminFilter } from '@v2board/api-client';
import type { AdminUserRow, PlanPeriod } from '@v2board/types';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
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
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageHeader, PageShell } from '@/components/ui/page';
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
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { Textarea } from '@/components/ui/textarea';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { cn } from '@/lib/cn';

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

interface GenerateUserSubmit {
  email_prefix?: string;
  email_suffix?: string;
  password?: string;
  plan_id?: number | null;
  expired_at?: string | null;
  generate_count?: string;
}

interface SendMailSubmit {
  subject?: string;
  content?: string;
}

interface AssignOrderSubmit {
  email?: string;
  plan_id?: number;
  period?: PlanPeriod;
  total_amount?: string;
}

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

function assignOrderSubmit(email?: string): AssignOrderSubmit {
  return {
    email: email || undefined,
    plan_id: undefined,
    period: undefined,
    total_amount: undefined,
  };
}

// Cross-page Tier-1 contract: the order manager (and the dashboard) seed an
// AdminFilter[] into sessionStorage then navigate to /user. Read it on mount,
// apply it as the initial filter, and clear it.
function readStoredUserFilter(): AdminFilter[] {
  if (typeof window === 'undefined') return [];
  const stored = window.sessionStorage.getItem('v2board-admin-user-filter');
  if (!stored) return [];
  window.sessionStorage.removeItem('v2board-admin-user-filter');
  try {
    const parsed = JSON.parse(stored) as AdminFilter[] | { filter?: AdminFilter[] };
    if (Array.isArray(parsed)) return parsed;
    return Array.isArray(parsed.filter) ? parsed.filter : [];
  } catch {
    return [];
  }
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

function planSelectItems(plans: PlanOption[], includeEmpty = false) {
  return [
    ...(includeEmpty ? [{ value: PLAN_NONE, label: '无' }] : []),
    ...plans.map((plan) => ({ value: String(plan.value), label: plan.label })),
  ];
}

export default function UsersPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
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

  const [editing, setEditing] = useState<AdminUserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [mailOpen, setMailOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [assigning, setAssigning] = useState<AdminUserRow | null>(null);
  const [trafficUser, setTrafficUser] = useState<AdminUserRow | null>(null);

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: ['admin', 'users'] });
      queryClient.removeQueries({ queryKey: ['admin', 'user'] });
    },
    [queryClient],
  );

  const planOptions = useMemo<PlanOption[]>(
    () => plans.data?.map((plan) => ({ label: plan.name, value: plan.id })) ?? [],
    [plans.data],
  );

  const groupMap = useMemo(() => {
    const map = new Map<number, string>();
    for (const group of groups.data ?? []) map.set(group.id, group.name);
    return map;
  }, [groups.data]);

  const filterFields = useMemo<FilterField[]>(
    () => [
      { key: 'email', title: '邮箱', condition: ['模糊'] },
      { key: 'id', title: '用户ID', condition: ['=', '>=', '>', '<', '<='] },
      {
        key: 'plan_id',
        title: '订阅',
        condition: ['='],
        type: 'select',
        options: [
          { label: '无订阅', value: PLAN_NONE },
          ...planOptions.map((plan) => ({ label: plan.label, value: plan.value })),
        ],
      },
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
    [planOptions],
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
      JSON.stringify({ filter: [{ key, condition, value }], total }),
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
    resetSecret
      .mutateAsync(row.id)
      .then(() => {
        toast.success('重置成功');
        void users.refetch();
      })
      .catch(() => undefined);
  };

  const deleteUser = async (row: AdminUserRow) => {
    const confirmed = await confirmDialog({
      title: '删除用户',
      description: `确定要删除${row.email}的用户信息吗？`,
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    remove
      .mutateAsync(row.id)
      .then(() => {
        toast.success('删除成功');
        void users.refetch();
      })
      .catch(() => undefined);
  };

  const copySubscribeUrl = (row: AdminUserRow) => {
    void navigator.clipboard?.writeText(row.subscribe_url);
  };

  const runUserAction = (key: string, row: AdminUserRow) => {
    if (key === 'edit') setEditing(row);
    if (key === 'assign') setAssigning(row);
    if (key === 'copy') copySubscribeUrl(row);
    if (key === 'reset') void resetUserSecret(row);
    if (key === 'orders') jumpOrderFilter('user_id', '=', row.id);
    if (key === 'invite') setFilter([{ key: 'invite_user_id', condition: '=', value: row.id }]);
    if (key === 'traffic') setTrafficUser(row);
    if (key === 'delete') void deleteUser(row);
  };

  const exportCsv = () => {
    const toastId = toast.loading('导出中');
    dumpCsv
      .mutateAsync(query.filter)
      .then((response) => {
        toast.dismiss(toastId);
        downloadText(`${formatDateTime(Date.now() / 1000)}.csv`, response.buffer);
      })
      .catch(() => {
        toast.dismiss(toastId);
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
    banUsers
      .mutateAsync(query.filter)
      .then(() => void users.refetch())
      .catch(() => undefined);
  };

  const bulkDelete = async () => {
    const confirmed = await confirmDialog({
      title: '提醒',
      description: '确定要进行删除吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    deleteAll
      .mutateAsync(query.filter)
      .then(() => void users.refetch())
      .catch(() => undefined);
  };

  const sortHeader = (label: string, key: string) => () => {
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
  };

  const renderEmail = (row: AdminUserRow) => {
    const onlineAt = (row as AdminUserRow & { t?: number | null }).t;
    const online = !(Date.now() / 1000 - 600 > Number(onlineAt));
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="inline-flex items-center gap-2">
            <span
              className={cn('size-2 shrink-0 rounded-full', online ? 'bg-success' : 'bg-muted-foreground')}
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
    const expired = value !== null && value < Date.now() / 1000;
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
          <StatusBadge tone={over ? 'destructive' : 'success'}>{row.original.total_used}</StatusBadge>
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
      cell: ({ row }) => <UserRowActions row={row.original} onAction={runUserAction} />,
    },
  ];

  return (
    <PageShell data-testid="users-page">
      <PageHeader
        title="用户管理"
        actions={
          <Button onClick={() => setCreating(true)} data-testid="user-create">
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
                    <DropdownMenuItem onClick={() => setMailOpen(true)} data-testid="user-send-mail">
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
              empty={data.length === 0 ? '暂无用户' : undefined}
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
        onSaved={() => users.refetch()}
      />

      <GenerateUserModal
        open={creating}
        plans={planOptions}
        loading={generate.isPending}
        onClose={() => setCreating(false)}
        onSubmit={(values) =>
          generate
            .mutateAsync(values as Parameters<typeof generate.mutateAsync>[0])
            .then((response) => {
              if (values.generate_count) downloadGeneratedUserCsv(response.buffer);
              return users.refetch();
            })
            .then(() => {
              setCreating(false);
            })
            .catch(() => undefined)
        }
      />

      <SendMailModal
        open={mailOpen}
        filter={query.filter}
        loading={sendMail.isPending}
        onClose={() => setMailOpen(false)}
        onSubmit={(values) =>
          sendMail
            .mutateAsync({ filter: query.filter, ...values })
            .then(() => {
              toast.success('已加入队列执行');
              setMailOpen(false);
            })
            .catch(() => undefined)
        }
      />

      <AssignOrderModal user={assigning} plans={planOptions} onClose={() => setAssigning(null)} />

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
}: {
  row: AdminUserRow;
  onAction: (key: string, row: AdminUserRow) => void;
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
        <DropdownMenuItem onClick={() => onAction('assign', row)}>
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

function FieldRow({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function AmountInput({
  suffix,
  ...props
}: ComponentProps<typeof Input> & { suffix: string }) {
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
  const [rows, setRows] = useState<AdminFilter[]>(value);

  useEffect(() => {
    if (open) setRows(value);
  }, [open, value]);

  const fieldOf = (key: string) => fields.find((field) => field.key === key) ?? fields[0]!;

  const addRow = () => {
    const field = fields[0]!;
    setRows((current) => [...current, { key: field.key, condition: field.condition[0]!, value: '' }]);
  };

  const removeRow = (index: number) =>
    setRows((current) => current.filter((_, itemIndex) => itemIndex !== index));

  const update = (index: number, patch: Partial<AdminFilter>) =>
    setRows((current) => current.map((row, itemIndex) => (itemIndex === index ? { ...row, ...patch } : row)));

  const changeField = (index: number, key: string) => {
    const field = fieldOf(key);
    update(index, { key, condition: field.condition[0]!, value: '' });
  };

  const apply = () => {
    onApply(rows.filter((row) => row.value !== '' && row.value != null));
    onOpenChange(false);
  };

  const reset = () => {
    setRows([]);
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
        </SheetHeader>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-4">
          {rows.length === 0 ? (
            <p className="text-sm text-muted-foreground">点击下方按钮添加过滤条件。</p>
          ) : null}
          {rows.map((row, index) => {
            const field = fieldOf(row.key);
            return (
              <div key={index} className="space-y-2 rounded-md border border-border p-3">
                <div className="flex items-center gap-2">
                  <Select value={row.key} onValueChange={(key) => changeField(index, key)}>
                    <SelectTrigger className="flex-1" data-testid={`user-filter-field-${index}`}>
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
                  <Select
                    value={row.condition}
                    onValueChange={(condition) => update(index, { condition })}
                  >
                    <SelectTrigger className="w-24" data-testid={`user-filter-condition-${index}`}>
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
                  <Button
                    variant="ghost"
                    size="icon"
                    className="size-9 shrink-0 text-muted-foreground"
                    aria-label="删除条件"
                    onClick={() => removeRow(index)}
                    data-testid={`user-filter-remove-${index}`}
                  >
                    <X className="size-4" />
                  </Button>
                </div>
                <FilterValueInput
                  index={index}
                  field={field}
                  value={row.value}
                  onChange={(next) => update(index, { value: next })}
                />
              </div>
            );
          })}

          <Button variant="outline" onClick={addRow} data-testid="user-filter-add">
            <Plus className="size-4" />
            添加条件
          </Button>
        </div>

        <SheetFooter className="flex-row justify-end gap-2 border-t border-border px-6 py-4">
          <Button variant="outline" onClick={reset} data-testid="user-filter-reset-all">
            重置
          </Button>
          <Button onClick={apply} data-testid="user-filter-apply">
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
    const current = value == null ? undefined : options.find((option) => String(option.value) === String(value));
    return (
      <Select
        value={current ? String(current.value) : undefined}
        onValueChange={(next) => {
          const option = options.find((item) => String(item.value) === next);
          onChange(option ? option.value : next);
        }}
      >
        <SelectTrigger className="w-full" data-testid={`user-filter-value-${index}`}>
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
        value={value ? dayjs(1000 * Number(value)).format('YYYY-MM-DDTHH:mm') : ''}
        onChange={(event) =>
          onChange(event.target.value ? dayjs(event.target.value).format('X') : '')
        }
        data-testid={`user-filter-value-${index}`}
      />
    );
  }

  return (
    <Input
      placeholder="欲检索内容"
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
  onSubmit: (values: GenerateUserSubmit) => Promise<void>;
}) {
  const [submit, setSubmit] = useState<GenerateUserSubmit>({});

  useEffect(() => {
    if (!open) setSubmit({});
  }, [open]);

  const close = () => {
    setSubmit({});
    onClose();
  };

  const setField = <K extends keyof GenerateUserSubmit>(key: K, value: GenerateUserSubmit[K]) =>
    setSubmit((state) => ({ ...state, [key]: value }));

  const planItems = planSelectItems(plans, true);

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-generate-dialog">
        <DialogHeader>
          <DialogTitle>创建用户</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <FieldRow label="邮箱">
            <div className="flex items-center gap-2">
              {!submit.generate_count ? (
                <Input
                  placeholder="账号（批量生成请留空）"
                  value={submit.email_prefix ?? ''}
                  onChange={(event) => setField('email_prefix', event.target.value)}
                  data-testid="generate-email-prefix"
                />
              ) : null}
              <span className="text-muted-foreground">@</span>
              <Input
                placeholder="域"
                value={submit.email_suffix ?? ''}
                onChange={(event) => setField('email_suffix', event.target.value)}
                data-testid="generate-email-suffix"
              />
            </div>
          </FieldRow>
          <FieldRow label="密码">
            <Input
              placeholder="留空则密码与邮箱相同"
              value={submit.password ?? ''}
              onChange={(event) => setField('password', event.target.value)}
            />
          </FieldRow>
          <FieldRow label="到期时间">
            <Input
              type="date"
              placeholder="请选择用户到期日期，为空则不限制到期时间"
              value={submit.expired_at ? dayjs(1000 * Number(submit.expired_at)).format('YYYY-MM-DD') : ''}
              onChange={(event) =>
                setField('expired_at', event.target.value ? dayjs(event.target.value).format('X') : null)
              }
              data-testid="generate-expired"
            />
          </FieldRow>
          <FieldRow label="订阅计划">
            <Select
              value={submit.plan_id != null ? String(submit.plan_id) : PLAN_NONE}
              onValueChange={(value) => setField('plan_id', value === PLAN_NONE ? null : Number(value))}
            >
              <SelectTrigger className="w-full">
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
          </FieldRow>
          {!submit.email_prefix ? (
            <FieldRow label="生成数量">
              <Input
                placeholder="如果为批量生成请输入生成数量"
                value={submit.generate_count ?? ''}
                onChange={(event) => setField('generate_count', event.target.value)}
                data-testid="generate-count"
              />
            </FieldRow>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            取消
          </Button>
          <Button
            onClick={() => void onSubmit({ ...submit })}
            disabled={loading}
            loading={loading}
            data-testid="generate-submit"
          >
            生成
          </Button>
        </DialogFooter>
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
  onSubmit: (values: SendMailSubmit) => Promise<void>;
}) {
  const [submit, setSubmit] = useState<SendMailSubmit>({});

  useEffect(() => {
    if (!open) setSubmit({});
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent data-testid="user-send-mail-dialog">
        <DialogHeader>
          <DialogTitle>发送邮件</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <FieldRow label="收件人">
            <Input disabled value={filter.length ? '过滤用户' : '全部用户'} />
          </FieldRow>
          <FieldRow label="主题">
            <Input
              placeholder="请输入邮件主题"
              value={submit.subject ?? ''}
              onChange={(event) => setSubmit((state) => ({ ...state, subject: event.target.value }))}
              data-testid="send-mail-subject"
            />
          </FieldRow>
          <FieldRow label="发送内容">
            <Textarea
              rows={12}
              placeholder="请输入邮件内容"
              value={submit.content ?? ''}
              onChange={(event) => setSubmit((state) => ({ ...state, content: event.target.value }))}
              data-testid="send-mail-content"
            />
          </FieldRow>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button
            onClick={() => void onSubmit(submit)}
            disabled={loading}
            loading={loading}
            data-testid="send-mail-submit"
          >
            确定
          </Button>
        </DialogFooter>
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
  const queryClient = useQueryClient();
  const assign = useAssignOrderMutation();
  const [submit, setSubmit] = useState<AssignOrderSubmit>(() => assignOrderSubmit());

  useEffect(() => {
    if (user) setSubmit(assignOrderSubmit(user.email));
  }, [user]);

  const close = () => {
    setSubmit(assignOrderSubmit(user?.email));
    onClose();
  };

  const setField = <K extends keyof AssignOrderSubmit>(key: K, value: AssignOrderSubmit[K]) =>
    setSubmit((state) => ({ ...state, [key]: value }));

  const doAssign = () => {
    // total_amount stays the raw entered value; the api-client applies the ×100
    // cents conversion. Preserving the raw payload here is the contract.
    assign
      .mutateAsync(submit)
      .then(() => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }))
      .then(close)
      .catch(() => undefined);
  };

  return (
    <Dialog open={Boolean(user)} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-assign-dialog">
        <DialogHeader>
          <DialogTitle>订单分配</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          <FieldRow label="用户邮箱">
            <Input
              placeholder="请输入用户邮箱"
              value={submit.email ?? ''}
              onChange={(event) => setField('email', event.target.value)}
              data-testid="assign-email"
            />
          </FieldRow>
          <FieldRow label="请选择订阅">
            <Select
              value={submit.plan_id != null ? String(submit.plan_id) : undefined}
              onValueChange={(value) => setField('plan_id', Number(value))}
            >
              <SelectTrigger className="w-full">
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
          </FieldRow>
          <FieldRow label="请选择周期">
            <Select
              value={submit.period}
              onValueChange={(value) => setField('period', value as PlanPeriod)}
            >
              <SelectTrigger className="w-full">
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
          </FieldRow>
          <FieldRow label="支付金额">
            <AmountInput
              suffix="¥"
              placeholder="请输入需要支付的金额"
              value={submit.total_amount ?? ''}
              onChange={(event) => setField('total_amount', event.target.value)}
              data-testid="assign-amount"
            />
          </FieldRow>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            取消
          </Button>
          <Button
            onClick={doAssign}
            disabled={assign.isPending}
            loading={assign.isPending}
            data-testid="assign-submit"
          >
            确定
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

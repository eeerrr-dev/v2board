import { useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { ChevronDown, ListFilter, Plus, Search } from 'lucide-react';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminOrderRow, Plan, PlanPeriod } from '@v2board/types';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import {
  useAdminOrderDetail,
  useAdminOrders,
  useAdminPlans,
  useAdminUserInfo,
  useAssignOrderMutation,
  useCancelOrderMutation,
  useMarkOrderPaidMutation,
  useUpdateOrderMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
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
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { HeaderTooltip } from '@/components/ui/header-tooltip';
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
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';

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

const ORDER_TYPE_TEXT: Record<number, string> = {
  1: '新购',
  2: '续费',
  3: '变更',
  4: '流量包',
  9: '充值',
};

// Backend order-status codes -> display label + pill tone. The codes are the
// Tier-1 contract; the tones are Tier-2 presentation.
const ORDER_STATUS: Record<number, { label: string; tone: StatusTone }> = {
  0: { label: '待支付', tone: 'warning' },
  1: { label: '开通中', tone: 'info' },
  2: { label: '已取消', tone: 'default' },
  3: { label: '已完成', tone: 'success' },
  4: { label: '已折抵', tone: 'default' },
};

const COMMISSION_STATUS: Record<number, { label: string; tone: StatusTone }> = {
  0: { label: '待确认', tone: 'default' },
  1: { label: '发放中', tone: 'info' },
  2: { label: '已发放', tone: 'success' },
  3: { label: '已驳回', tone: 'destructive' },
};

const PAGINATION_LABELS = {
  itemsPerPage: '条/页',
  nextPage: '下一页',
  nextWindow: '向后 5 页',
  previousPage: '上一页',
  previousWindow: '向前 5 页',
};

interface QueryState {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
}

// Cross-page Tier-1 contract: the dashboard writes an AdminFilter[] to
// sessionStorage['v2board-admin-order-filter'] then navigates to /order. This
// page must read that key on mount, apply it as the initial filter, and clear
// it. The legacy object form ({ filter, total }) is still accepted defensively;
// the response total is authoritative, so the stored total is ignored.
function readStoredOrderFilter(): AdminFilter[] {
  if (typeof window === 'undefined') return [];
  const stored = window.sessionStorage.getItem('v2board-admin-order-filter');
  if (!stored) return [];
  window.sessionStorage.removeItem('v2board-admin-order-filter');
  try {
    const parsed = JSON.parse(stored) as AdminFilter[] | { filter?: AdminFilter[] };
    if (Array.isArray(parsed)) return parsed;
    return Array.isArray(parsed.filter) ? parsed.filter : [];
  } catch {
    return [];
  }
}

// Cents -> decimal string. Preserves the legacy amount interpretation exactly.
function cents(value?: number | null) {
  return ((value as number) / 100).toFixed(2);
}

function filterValue(filter: AdminFilter[], key: string) {
  const found = filter.find((item) => item.key === key)?.value;
  return found == null ? undefined : String(found);
}

interface AssignOrderSubmit {
  email?: string;
  plan_id?: number;
  period?: PlanPeriod;
  total_amount?: string;
}

function DetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-4 py-2 text-sm">
      <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
      <span className="min-w-0 flex-1 break-words text-foreground">{children}</span>
    </div>
  );
}

function OrderDetailSheet({
  id,
  open,
  onClose,
  plans,
  onUserFilter,
}: {
  id?: number;
  open: boolean;
  onClose: () => void;
  plans: Plan[];
  onUserFilter: (key: string, condition: string, value: string) => void;
}) {
  const order = useAdminOrderDetail(id);
  const user = useAdminUserInfo(order.data?.user_id);
  const inviteUser = useAdminUserInfo(order.data?.invite_user_id);
  const detail = order.data;
  const planName = plans.find((plan) => plan.id === detail?.plan_id)?.name;
  // Backend-field interpretation preserved: wait for the invited user's info too
  // before rendering, so the commission block never flashes a half-loaded row.
  const loaded = Boolean(detail && user.data?.email && (!detail.invite_user_id || inviteUser.data));

  return (
    <Sheet open={open} onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <SheetContent side="right" className="w-full gap-0 overflow-y-auto sm:max-w-md" data-testid="order-detail">
        <SheetHeader>
          <SheetTitle>订单信息</SheetTitle>
        </SheetHeader>

        {loaded && detail ? (
          <div className="divide-y divide-border px-4 pb-6">
            <DetailRow label="邮箱">
              <button
                type="button"
                className="text-primary underline-offset-4 hover:underline"
                onClick={() => user.data && onUserFilter('email', '模糊', user.data.email)}
                data-testid="order-detail-user"
              >
                {user.data?.email}
              </button>
            </DetailRow>
            <DetailRow label="订单号">
              <span className="font-mono">{detail.trade_no}</span>
            </DetailRow>
            <DetailRow label="订单周期">{PERIOD_TEXT[detail.period] ?? detail.period}</DetailRow>
            <DetailRow label="订单状态">{ORDER_STATUS[detail.status]?.label}</DetailRow>
            <DetailRow label="订阅计划">{planName}</DetailRow>
            <DetailRow label="回调单号">{detail.callback_no || '-'}</DetailRow>
            <DetailRow label="支付金额">{cents(detail.total_amount)}</DetailRow>
            <DetailRow label="余额支付">{cents(detail.balance_amount)}</DetailRow>
            <DetailRow label="优惠金额">{cents(detail.discount_amount)}</DetailRow>
            <DetailRow label="退回金额">{cents(detail.refund_amount)}</DetailRow>
            <DetailRow label="折抵金额">{cents(detail.surplus_amount)}</DetailRow>
            <DetailRow label="创建时间">{formatDateTime(detail.created_at)}</DetailRow>
            <DetailRow label="更新时间">{formatDateTime(detail.updated_at)}</DetailRow>
            {detail.invite_user_id && detail.status === 3 ? (
              <>
                <DetailRow label="邀请人">
                  <button
                    type="button"
                    className="text-primary underline-offset-4 hover:underline"
                    onClick={() =>
                      inviteUser.data && onUserFilter('invite_by_email', '模糊', inviteUser.data.email)
                    }
                    data-testid="order-detail-invite"
                  >
                    {inviteUser.data?.email}
                  </button>
                </DetailRow>
                <DetailRow label="佣金金额">{cents(detail.commission_balance)}</DetailRow>
                {detail.actual_commission_balance ? (
                  <DetailRow label="实际发放">{cents(detail.actual_commission_balance)}</DetailRow>
                ) : null}
                <DetailRow label="佣金状态">{COMMISSION_STATUS[detail.commission_status]?.label}</DetailRow>
              </>
            ) : null}
          </div>
        ) : (
          <div className="flex justify-center py-10" role="status">
            <Spinner className="size-5 text-muted-foreground" />
            <span className="sr-only">加载中</span>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}

function AssignOrderDialog({
  plans,
  onAssigned,
}: {
  plans: Plan[];
  onAssigned: () => void | Promise<unknown>;
}) {
  const assign = useAssignOrderMutation();
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<AssignOrderSubmit>({});

  const openDialog = () => {
    setSubmit({});
    setOpen(true);
  };

  const assignOrder = async () => {
    try {
      // total_amount stays the raw entered value; the api-client applies the
      // ×100 cents conversion. Preserving the raw payload here is the contract.
      await assign.mutateAsync(submit);
      await onAssigned();
      setOpen(false);
    } catch {
      // Errors surface through the global onError handler; keep the dialog open.
    }
  };

  return (
    <>
      <Button onClick={openDialog} data-testid="order-assign-open">
        <Plus className="size-4" />
        添加订单
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md" data-testid="order-assign-dialog">
          <DialogHeader>
            <DialogTitle>订单分配</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="assign-email">用户邮箱</Label>
              <Input
                id="assign-email"
                placeholder="请输入用户邮箱"
                value={submit.email ?? ''}
                onChange={(event) => setSubmit((state) => ({ ...state, email: event.target.value }))}
                data-testid="order-assign-email"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="assign-plan">请选择订阅</Label>
              <Select
                value={submit.plan_id != null ? String(submit.plan_id) : undefined}
                onValueChange={(value) => setSubmit((state) => ({ ...state, plan_id: Number(value) }))}
              >
                <SelectTrigger id="assign-plan" className="w-full">
                  <SelectValue placeholder="请选择订阅" />
                </SelectTrigger>
                <SelectContent>
                  {plans.map((plan) => (
                    <SelectItem key={plan.id} value={String(plan.id)}>
                      {plan.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="assign-period">请选择周期</Label>
              <Select
                value={submit.period}
                onValueChange={(value) =>
                  setSubmit((state) => ({ ...state, period: value as PlanPeriod }))
                }
              >
                <SelectTrigger id="assign-period" className="w-full">
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
            </div>
            <div className="space-y-2">
              <Label htmlFor="assign-amount">支付金额</Label>
              <div className="relative">
                <Input
                  id="assign-amount"
                  className="pr-8"
                  placeholder="请输入需要支付的金额"
                  value={submit.total_amount ?? ''}
                  onChange={(event) =>
                    setSubmit((state) => ({ ...state, total_amount: event.target.value }))
                  }
                  data-testid="order-assign-amount"
                />
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  ¥
                </span>
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => void assignOrder()}
              disabled={assign.isPending}
              loading={assign.isPending}
              data-testid="order-assign-submit"
            >
              确定
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default function OrdersPage() {
  const navigate = useNavigate();
  const [query, setQuery] = useState<QueryState>(() => ({
    current: 1,
    pageSize: 10,
    filter: readStoredOrderFilter(),
  }));
  const [search, setSearch] = useState(() => filterValue(query.filter, 'trade_no') ?? '');
  const [detailId, setDetailId] = useState<number>();
  const searchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const orders = useAdminOrders({
    current: query.current,
    pageSize: query.pageSize,
    filter: query.filter,
  });
  const plans = useAdminPlans();
  const paid = useMarkOrderPaidMutation();
  const cancel = useCancelOrderMutation();
  const updateOrder = useUpdateOrderMutation();

  const data = orders.data?.data ?? [];
  const total = orders.data?.total ?? 0;

  // Upsert/remove a single filter condition while preserving any other
  // conditions in the array (e.g. the dashboard's commission_balance jump).
  const setFilter = (key: string, condition: string, value: string) => {
    setQuery((state) => {
      const rest = state.filter.filter((item) => item.key !== key);
      const next = value ? [...rest, { key, condition, value }] : rest;
      return { ...state, current: 1, filter: next };
    });
  };

  const onSearchChange = (value: string) => {
    setSearch(value);
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => setFilter('trade_no', '模糊', value), 300);
  };

  const resetFilters = () => {
    setSearch('');
    setQuery((state) => ({ ...state, current: 1, filter: [] }));
  };

  const markPaid = (tradeNo: string) => {
    paid
      .mutateAsync(tradeNo)
      .then(() => void orders.refetch())
      .catch(() => undefined);
  };

  const cancelOrder = async (tradeNo: string) => {
    const confirmed = await confirmDialog({
      title: '取消订单',
      description: '确定要取消该订单吗？',
      confirmText: '确定',
    });
    if (!confirmed) return;
    cancel
      .mutateAsync(tradeNo)
      .then(() => void orders.refetch())
      .catch(() => undefined);
  };

  const updateCommission = (tradeNo: string, value: string) => {
    updateOrder
      .mutateAsync({ tradeNo, key: 'commission_status', value })
      .then(() => void orders.refetch())
      .catch(() => undefined);
  };

  // Cross-page Tier-1 contract: seed the user filter and navigate to /user,
  // where the user manager reads and applies it.
  const userFilter = (key: string, condition: string, value: string) => {
    window.sessionStorage.setItem(
      'v2board-admin-user-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    navigate('/user');
  };

  const statusValue = filterValue(query.filter, 'status') ?? 'all';

  const renderOrderStatus = (row: AdminOrderRow) => {
    const info = ORDER_STATUS[row.status] ?? ORDER_STATUS[0]!;
    if (row.status !== 0) {
      return (
        <StatusBadge tone={info.tone} showDot>
          {info.label}
        </StatusBadge>
      );
    }
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="inline-flex items-center gap-1 rounded-md outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
            data-testid={`order-status-trigger-${row.trade_no}`}
          >
            <StatusBadge tone={info.tone} showDot>
              {info.label}
            </StatusBadge>
            <ChevronDown className="size-3.5 text-muted-foreground" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          <DropdownMenuItem
            onClick={() => markPaid(row.trade_no)}
            data-testid={`order-mark-paid-${row.trade_no}`}
          >
            标记为已支付
          </DropdownMenuItem>
          <DropdownMenuItem
            variant="destructive"
            onClick={() => void cancelOrder(row.trade_no)}
            data-testid={`order-cancel-${row.trade_no}`}
          >
            取消订单
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  };

  const renderCommissionStatus = (row: AdminOrderRow) => {
    if (row.status === 0 || row.status === 2 || !row.commission_balance) return '-';
    const value = row.commission_status;
    const info = COMMISSION_STATUS[value] ?? COMMISSION_STATUS[0]!;
    if (value === 2) {
      return (
        <StatusBadge tone={info.tone} showDot>
          {info.label}
        </StatusBadge>
      );
    }
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="inline-flex items-center gap-1 rounded-md outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
            data-testid={`commission-status-trigger-${row.trade_no}`}
          >
            <StatusBadge tone={info.tone} showDot>
              {info.label}
            </StatusBadge>
            <ChevronDown className="size-3.5 text-muted-foreground" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          <DropdownMenuItem disabled={value === 0} onClick={() => updateCommission(row.trade_no, '0')}>
            待确认
          </DropdownMenuItem>
          <DropdownMenuItem disabled={value === 1} onClick={() => updateCommission(row.trade_no, '1')}>
            有效
          </DropdownMenuItem>
          <DropdownMenuItem disabled={value === 3} onClick={() => updateCommission(row.trade_no, '3')}>
            无效
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  };

  const columns: DataTableColumn<AdminOrderRow>[] = [
    {
      id: 'trade_no',
      header: () => <span># 订单号</span>,
      cell: ({ row }) => (
        <button
          type="button"
          className="font-mono text-primary underline-offset-4 hover:underline"
          onClick={() => setDetailId(row.original.id)}
          data-testid={`order-open-${row.original.id}`}
        >
          {row.original.trade_no}
        </button>
      ),
    },
    {
      id: 'type',
      header: () => <span>类型</span>,
      cell: ({ row }) => ORDER_TYPE_TEXT[row.original.type],
    },
    {
      id: 'plan_name',
      meta: { className: 'text-foreground' },
      header: () => <span>订阅计划</span>,
      cell: ({ row }) => row.original.plan_name,
    },
    {
      id: 'period',
      meta: { align: 'center' },
      header: () => <span>周期</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">{PERIOD_TEXT[row.original.period] ?? row.original.period}</Badge>
      ),
    },
    {
      id: 'total_amount',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>支付金额</span>,
      cell: ({ row }) => cents(row.original.total_amount),
    },
    {
      id: 'status',
      header: () => (
        <HeaderTooltip title="标记为[已支付]后将会由系统进行开通后并完成">订单状态</HeaderTooltip>
      ),
      cell: ({ row }) => renderOrderStatus(row.original),
    },
    {
      id: 'commission_balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>佣金金额</span>,
      cell: ({ row }) =>
        row.original.status === 0 || row.original.status === 2 || !row.original.commission_balance
          ? '-'
          : cents(row.original.commission_balance),
    },
    {
      id: 'commission_status',
      header: () => (
        <HeaderTooltip title="标记为[有效]后将会由系统处理后发放到用户并完成">佣金状态</HeaderTooltip>
      ),
      cell: ({ row }) => renderCommissionStatus(row.original),
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>创建时间</span>,
      cell: ({ row }) => formatDateMinuteSlash(row.original.created_at),
    },
  ];

  return (
    <PageShell data-testid="orders-page">
      <PageHeader
        title="订单管理"
        actions={<AssignOrderDialog plans={plans.data ?? []} onAssigned={() => orders.refetch()} />}
      />

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-center gap-2">
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" data-testid="order-status-filter">
                      <ListFilter className="size-4" />
                      {statusValue === 'all'
                        ? '订单状态'
                        : (ORDER_STATUS[Number(statusValue)]?.label ?? '订单状态')}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuLabel>订单状态</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    <DropdownMenuRadioGroup
                      value={statusValue}
                      onValueChange={(value) =>
                        setFilter('status', '=', value === 'all' ? '' : value)
                      }
                    >
                      <DropdownMenuRadioItem value="all">全部</DropdownMenuRadioItem>
                      {Object.keys(ORDER_STATUS).map((code) => (
                        <DropdownMenuRadioItem key={code} value={code}>
                          {ORDER_STATUS[Number(code)]!.label}
                        </DropdownMenuRadioItem>
                      ))}
                    </DropdownMenuRadioGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
                {query.filter.length > 0 ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={resetFilters}
                    data-testid="order-filter-reset"
                  >
                    清除筛选
                  </Button>
                ) : null}
              </div>
              <div className="relative w-full sm:w-64">
                <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  className="pl-9"
                  placeholder="搜索订单号"
                  value={search}
                  onChange={(event) => onSearchChange(event.target.value)}
                  data-testid="order-search"
                />
              </div>
            </div>

            <DataTable
              columns={columns}
              data={data}
              getRowKey={(row) => row.id}
              className="min-w-[1024px]"
              data-testid="orders-table"
              empty={data.length === 0 ? '暂无订单' : undefined}
              emptyTestId="orders-empty"
              virtualizer={{ enabled: data.length > VIRTUALIZE_MIN_ROWS }}
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
                testIds={{ page: 'order-page', pageSize: 'order-page-size' }}
              />
            ) : null}
          </CardContent>
        </Card>
      </TooltipProvider>

      <OrderDetailSheet
        id={detailId}
        open={detailId != null}
        onClose={() => setDetailId(undefined)}
        plans={plans.data ?? []}
        onUserFilter={userFilter}
      />

      {orders.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

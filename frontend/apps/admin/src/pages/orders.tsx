import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import { ChevronDown, ListFilter, Plus, Search } from 'lucide-react';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminOrderRow, Plan } from '@v2board/types';
import { formatBackendDateMinuteSlash, formatBackendDateTime } from '@v2board/config/format';
import { takeStoredAdminFilters } from '@/lib/stored-admin-filters';
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
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
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
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { EmptyState, PageHeader, PageShell } from '@/components/ui/page';
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
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { assignOrderSchema, type AssignOrderValues } from './users/form-schema';

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

// Cross-page Tier-1 contract: dashboard and user actions write an AdminFilter[] to
// sessionStorage['v2board-admin-order-filter'] then navigates to /order. This
// page must read that key on mount, apply it as the initial filter, and clear
// it. This key is application-internal, so malformed or obsolete shapes are
// discarded instead of carrying a second compatibility representation forever.
function readStoredOrderFilter(): AdminFilter[] {
  return takeStoredAdminFilters('v2board-admin-order-filter');
}

// Cents -> decimal string. Preserves the backend amount interpretation exactly.
function cents(value?: number | null) {
  return ((value as number) / 100).toFixed(2);
}

function filterValue(filter: AdminFilter[], key: string) {
  const found = filter.find((item) => item.key === key)?.value;
  return found == null ? undefined : String(found);
}

function DetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-4 py-2 text-sm">
      <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
      <span className="min-w-0 flex-1 break-words text-foreground">{children}</span>
    </div>
  );
}

function DetailLoading({ testId }: { testId: string }) {
  return (
    <div className="flex justify-center py-10" role="status" data-testid={testId}>
      <Spinner className="size-5 text-muted-foreground" />
      <span className="sr-only">加载中</span>
    </div>
  );
}

function OrderDetailSheet({
  tradeNo,
  open,
  onClose,
  plans,
  onUserFilter,
}: {
  tradeNo?: string;
  open: boolean;
  onClose: () => void;
  plans: Plan[];
  onUserFilter: (key: string, condition: string, value: string) => void;
}) {
  const order = useAdminOrderDetail(tradeNo);
  const user = useAdminUserInfo(order.data?.user_id);
  const inviteUser = useAdminUserInfo(order.data?.invite_user_id);
  const detail = order.data;
  const detailUser = user.data;
  const detailInviteUser = inviteUser.data;
  const planName = plans.find((plan) => plan.id === detail?.plan_id)?.name;
  const requiresInviteUser = detail?.invite_user_id != null;

  let content: ReactNode;
  if (order.isError) {
    content = (
      <div className="px-4 py-6">
        <ErrorState
          data-testid="order-detail-error"
          message="订单详情加载失败"
          onRetry={() => void order.refetch()}
        />
      </div>
    );
  } else if (order.isPending) {
    content = <DetailLoading testId="order-detail-loading" />;
  } else if (!detail) {
    content = (
      <EmptyState className="m-4 min-h-32" data-testid="order-detail-empty" title="暂无订单详情" />
    );
  } else if (user.isError) {
    content = (
      <div className="px-4 py-6">
        <ErrorState
          data-testid="order-detail-user-error"
          message="订单用户加载失败"
          onRetry={() => void user.refetch()}
        />
      </div>
    );
  } else if (user.isPending) {
    content = <DetailLoading testId="order-detail-user-loading" />;
  } else if (!detailUser) {
    content = (
      <EmptyState
        className="m-4 min-h-32"
        data-testid="order-detail-user-empty"
        title="未找到订单用户"
      />
    );
  } else if (requiresInviteUser && inviteUser.isError) {
    content = (
      <div className="px-4 py-6">
        <ErrorState
          data-testid="order-detail-invite-error"
          message="邀请人信息加载失败"
          onRetry={() => void inviteUser.refetch()}
        />
      </div>
    );
  } else if (requiresInviteUser && inviteUser.isPending) {
    content = <DetailLoading testId="order-detail-invite-loading" />;
  } else if (requiresInviteUser && !detailInviteUser) {
    content = (
      <EmptyState
        className="m-4 min-h-32"
        data-testid="order-detail-invite-empty"
        title="未找到邀请人"
      />
    );
  } else {
    content = (
      <div className="divide-y divide-border px-4 pb-6">
        <DetailRow label="邮箱">
          <button
            type="button"
            className="text-primary underline-offset-4 hover:underline"
            onClick={() => onUserFilter('email', '模糊', detailUser.email)}
            data-testid="order-detail-user"
          >
            {detailUser.email}
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
        <DetailRow label="创建时间">{formatBackendDateTime(detail.created_at)}</DetailRow>
        <DetailRow label="更新时间">{formatBackendDateTime(detail.updated_at)}</DetailRow>
        {detail.invite_user_id && detail.status === 3 ? (
          <>
            <DetailRow label="邀请人">
              <button
                type="button"
                className="text-primary underline-offset-4 hover:underline"
                onClick={() =>
                  detailInviteUser &&
                  onUserFilter('invite_by_email', '模糊', detailInviteUser.email)
                }
                data-testid="order-detail-invite"
              >
                {detailInviteUser?.email}
              </button>
            </DetailRow>
            <DetailRow label="佣金金额">{cents(detail.commission_balance)}</DetailRow>
            {detail.actual_commission_balance ? (
              <DetailRow label="实际发放">{cents(detail.actual_commission_balance)}</DetailRow>
            ) : null}
            <DetailRow label="佣金状态">
              {COMMISSION_STATUS[detail.commission_status]?.label}
            </DetailRow>
          </>
        ) : null}
      </div>
    );
  }

  return (
    <Sheet open={open} onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-md"
        data-testid="order-detail"
      >
        <SheetHeader>
          <SheetTitle>订单信息</SheetTitle>
          <SheetDescription>查看订单状态、金额、订阅周期与支付信息。</SheetDescription>
        </SheetHeader>

        {content}
      </SheetContent>
    </Sheet>
  );
}

function AssignOrderDialog({ plans }: { plans: Plan[] }) {
  const assign = useAssignOrderMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<AssignOrderValues>({
    resolver: zodResolver(assignOrderSchema),
    defaultValues: { email: '', plan_id: undefined, period: undefined, total_amount: '' },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  const openDialog = () => {
    form.reset();
    setOpen(true);
  };

  const assignOrder = form.handleSubmit(async (values) => {
    form.clearErrors('root.serverError');
    try {
      // total_amount stays the raw entered value; the api-client applies the
      // ×100 cents conversion. Preserving the raw payload here is the contract.
      await assign.mutateAsync(values);
      setOpen(false);
    } catch (error) {
      form.setError('root.serverError', {
        message: error instanceof Error && error.message ? error.message : '请求失败',
      });
    }
  });

  const setDialogOpen = (nextOpen: boolean) => {
    if (!nextOpen) form.reset();
    setOpen(nextOpen);
  };

  return (
    <>
      <Button onClick={openDialog} data-testid="order-assign-open">
        <Plus className="size-4" />
        添加订单
      </Button>
      <Dialog open={open} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-md" data-testid="order-assign-dialog">
          <DialogHeader>
            <DialogTitle>订单分配</DialogTitle>
            <DialogDescription>为指定用户创建订单并选择订阅计划与周期。</DialogDescription>
          </DialogHeader>

          <form className="space-y-4" onSubmit={assignOrder} noValidate>
            <FieldError errors={[formErrors.root?.serverError]} />
            <Field data-invalid={Boolean(formErrors.email)}>
              <FieldLabel htmlFor="order-assign-email">用户邮箱</FieldLabel>
              <Controller
                control={form.control}
                name="email"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    id="order-assign-email"
                    type="email"
                    placeholder="请输入用户邮箱"
                    data-testid="order-assign-email"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
              <FieldError errors={[formErrors.email]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.plan_id)}>
              <FieldLabel htmlFor="order-assign-plan">请选择订阅</FieldLabel>
              <Controller
                control={form.control}
                name="plan_id"
                render={({ field }) => (
                  <Select
                    value={field.value != null ? String(field.value) : ''}
                    onValueChange={(value) => field.onChange(Number(value))}
                  >
                    <SelectTrigger
                      id="order-assign-plan"
                      className="w-full"
                      aria-invalid={Boolean(formErrors.plan_id)}
                    >
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
                )}
              />
              <FieldError errors={[formErrors.plan_id]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.period)}>
              <FieldLabel htmlFor="order-assign-period">请选择周期</FieldLabel>
              <Controller
                control={form.control}
                name="period"
                render={({ field }) => (
                  <Select value={field.value ?? ''} onValueChange={field.onChange}>
                    <SelectTrigger
                      id="order-assign-period"
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
              <FieldLabel htmlFor="order-assign-amount">支付金额</FieldLabel>
              <div className="relative">
                <Controller
                  control={form.control}
                  name="total_amount"
                  render={({ field, fieldState }) => (
                    <Input
                      {...field}
                      id="order-assign-amount"
                      className="pr-8"
                      placeholder="请输入需要支付的金额"
                      data-testid="order-assign-amount"
                      aria-invalid={fieldState.invalid}
                    />
                  )}
                />
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  ¥
                </span>
              </div>
              <FieldError errors={[formErrors.total_amount]} />
            </Field>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setDialogOpen(false)}>
                取消
              </Button>
              <Button
                type="submit"
                disabled={assign.isPending || isSubmitting}
                loading={assign.isPending || isSubmitting}
                data-testid="order-assign-submit"
              >
                确定
              </Button>
            </DialogFooter>
          </form>
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
  const [detailTradeNo, setDetailTradeNo] = useState<string>();
  const searchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const orders = useAdminOrders({
    current: query.current,
    pageSize: query.pageSize,
    filter: query.filter,
  });
  const plans = useAdminPlans();
  const planData = plans.data;
  const plansReady = !plans.isError && planData !== undefined;
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

  useEffect(() => () => clearTimeout(searchTimer.current), []);

  const resetFilters = () => {
    clearTimeout(searchTimer.current);
    searchTimer.current = undefined;
    setSearch('');
    setQuery((state) => ({ ...state, current: 1, filter: [] }));
  };

  const markPaid = (tradeNo: string) => paid.mutate(tradeNo);

  const cancelOrder = async (tradeNo: string) => {
    const confirmed = await confirmDialog({
      title: '取消订单',
      description: '确定要取消该订单吗？',
      confirmText: '确定',
    });
    if (!confirmed) return;
    cancel.mutate(tradeNo);
  };

  const updateCommission = (tradeNo: string, value: string) => {
    updateOrder.mutate({ tradeNo, key: 'commission_status', value });
  };

  // Cross-page Tier-1 contract: seed the user filter and navigate to /user,
  // where the user manager reads and applies it.
  const userFilter = (key: string, condition: string, value: string) => {
    window.sessionStorage.setItem(
      'v2board-admin-user-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    void navigate('/user');
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
          <DropdownMenuItem
            disabled={value === 0}
            onClick={() => updateCommission(row.trade_no, '0')}
          >
            待确认
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={value === 1}
            onClick={() => updateCommission(row.trade_no, '1')}
          >
            有效
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={value === 3}
            onClick={() => updateCommission(row.trade_no, '3')}
          >
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
          onClick={() => setDetailTradeNo(row.original.trade_no)}
          data-testid={`order-open-${row.original.trade_no}`}
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
        <HeaderTooltip title="标记为[有效]后将会由系统处理后发放到用户并完成">
          佣金状态
        </HeaderTooltip>
      ),
      cell: ({ row }) => renderCommissionStatus(row.original),
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>创建时间</span>,
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
    },
  ];

  return (
    <PageShell data-testid="orders-page">
      {orders.isError ? (
        <ErrorState message="订单列表加载失败" onRetry={() => void orders.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      <PageHeader
        title="订单管理"
        actions={
          plansReady ? (
            <AssignOrderDialog plans={planData} />
          ) : (
            <Button disabled data-testid="order-assign-open">
              分配订单
            </Button>
          )
        }
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
              empty={
                !orders.isError && orders.data !== undefined && data.length === 0
                  ? '暂无订单'
                  : undefined
              }
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
        tradeNo={detailTradeNo}
        open={detailTradeNo != null}
        onClose={() => setDetailTradeNo(undefined)}
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

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
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
import { LoadingState, SkeletonLines, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { assignOrderSchema, type AssignOrderValues } from './users/form-schema';

// The record keys below are backend wire values (period identifiers, order
// type/status codes); only the labels are translated, resolved at render time.
function periodTextMap(t: TFunction): Record<string, string> {
  return {
    month_price: t(($) => $.admin.orders.period_month),
    quarter_price: t(($) => $.admin.orders.period_quarter),
    half_year_price: t(($) => $.admin.orders.period_half_year),
    year_price: t(($) => $.admin.orders.period_year),
    two_year_price: t(($) => $.admin.orders.period_two_year),
    three_year_price: t(($) => $.admin.orders.period_three_year),
    onetime_price: t(($) => $.admin.orders.period_onetime),
    reset_price: t(($) => $.admin.orders.period_reset),
  };
}

function orderTypeTextMap(t: TFunction): Record<number, string> {
  return {
    1: t(($) => $.admin.orders.type_new),
    2: t(($) => $.admin.orders.type_renew),
    3: t(($) => $.admin.orders.type_change),
    4: t(($) => $.admin.orders.type_traffic_package),
    9: t(($) => $.admin.orders.type_deposit),
  };
}

// Backend order-status codes -> display label + pill tone. The codes are the
// Tier-1 contract; the tones are Tier-2 presentation.
function orderStatusMap(t: TFunction): Record<number, { label: string; tone: StatusTone }> {
  return {
    0: { label: t(($) => $.common.unpaid), tone: 'warning' },
    1: { label: t(($) => $.admin.orders.status_activating), tone: 'info' },
    2: { label: t(($) => $.common.cancelled), tone: 'default' },
    3: { label: t(($) => $.common.completed), tone: 'success' },
    4: { label: t(($) => $.admin.orders.status_credited), tone: 'default' },
  };
}

function commissionStatusMap(t: TFunction): Record<number, { label: string; tone: StatusTone }> {
  return {
    0: { label: t(($) => $.admin.orders.commission_pending), tone: 'default' },
    1: { label: t(($) => $.admin.orders.commission_processing), tone: 'info' },
    2: { label: t(($) => $.admin.orders.commission_paid), tone: 'success' },
    3: { label: t(($) => $.admin.orders.commission_rejected), tone: 'destructive' },
  };
}

function paginationLabels(t: TFunction) {
  return {
    itemsPerPage: t(($) => $.common.items_per_page),
    nextPage: t(($) => $.common.next_page),
    nextWindow: t(($) => $.common.next_5),
    previousPage: t(($) => $.common.prev_page),
    previousWindow: t(($) => $.common.prev_5),
  };
}

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
    <LoadingState className="py-10" data-testid={testId}>
      <SkeletonLines lines={4} />
    </LoadingState>
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
  const { t } = useTranslation();
  const periodText = periodTextMap(t);
  const orderStatus = orderStatusMap(t);
  const commissionStatus = commissionStatusMap(t);
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
          message={t(($) => $.admin.orders.detail_load_failed)}
          onRetry={() => void order.refetch()}
        />
      </div>
    );
  } else if (order.isPending) {
    content = <DetailLoading testId="order-detail-loading" />;
  } else if (!detail) {
    content = (
      <EmptyState
        className="m-4 min-h-32"
        data-testid="order-detail-empty"
        title={t(($) => $.admin.orders.detail_empty)}
      />
    );
  } else if (user.isError) {
    content = (
      <div className="px-4 py-6">
        <ErrorState
          data-testid="order-detail-user-error"
          message={t(($) => $.admin.orders.detail_user_load_failed)}
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
        title={t(($) => $.admin.orders.detail_user_empty)}
      />
    );
  } else if (requiresInviteUser && inviteUser.isError) {
    content = (
      <div className="px-4 py-6">
        <ErrorState
          data-testid="order-detail-invite-error"
          message={t(($) => $.admin.orders.detail_invite_load_failed)}
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
        title={t(($) => $.admin.orders.detail_invite_empty)}
      />
    );
  } else {
    content = (
      <div className="divide-y divide-border px-4 pb-6">
        <DetailRow label={t(($) => $.admin.orders.email)}>
          <button
            type="button"
            className="text-primary underline-offset-4 hover:underline"
            onClick={() => onUserFilter('email', '模糊', detailUser.email)}
            data-testid="order-detail-user"
          >
            {detailUser.email}
          </button>
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.trade_no)}>
          <span className="font-mono">{detail.trade_no}</span>
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.order_period)}>
          {periodText[detail.period] ?? detail.period}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.order_status)}>
          {orderStatus[detail.status]?.label}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.plan)}>{planName}</DetailRow>
        <DetailRow label={t(($) => $.admin.orders.callback_no)}>
          {detail.callback_no || '-'}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.total_amount)}>
          {cents(detail.total_amount)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.balance_amount)}>
          {cents(detail.balance_amount)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.discount_amount)}>
          {cents(detail.discount_amount)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.refund_amount)}>
          {cents(detail.refund_amount)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.surplus_amount)}>
          {cents(detail.surplus_amount)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.created_at)}>
          {formatBackendDateTime(detail.created_at)}
        </DetailRow>
        <DetailRow label={t(($) => $.admin.orders.updated_at)}>
          {formatBackendDateTime(detail.updated_at)}
        </DetailRow>
        {detail.invite_user_id && detail.status === 3 ? (
          <>
            <DetailRow label={t(($) => $.admin.orders.invite_user)}>
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
            <DetailRow label={t(($) => $.admin.orders.commission_amount)}>
              {cents(detail.commission_balance)}
            </DetailRow>
            {detail.actual_commission_balance ? (
              <DetailRow label={t(($) => $.admin.orders.actual_commission)}>
                {cents(detail.actual_commission_balance)}
              </DetailRow>
            ) : null}
            <DetailRow label={t(($) => $.admin.orders.commission_status)}>
              {commissionStatus[detail.commission_status]?.label}
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
          <SheetTitle>{t(($) => $.admin.orders.detail_title)}</SheetTitle>
          <SheetDescription>{t(($) => $.admin.orders.detail_description)}</SheetDescription>
        </SheetHeader>

        {content}
      </SheetContent>
    </Sheet>
  );
}

function AssignOrderDialog({ plans }: { plans: Plan[] }) {
  const { t } = useTranslation();
  const periodText = periodTextMap(t);
  const periodOptions = Object.keys(periodText).map((period) => ({
    value: period,
    label: periodText[period] ?? period,
  }));
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
        message:
          error instanceof Error && error.message
            ? error.message
            : t(($) => $.admin.orders.request_failed),
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
        {t(($) => $.admin.orders.add_order)}
      </Button>
      <Dialog open={open} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-md" data-testid="order-assign-dialog">
          <DialogHeader>
            <DialogTitle>{t(($) => $.admin.orders.assign_title)}</DialogTitle>
            <DialogDescription>{t(($) => $.admin.orders.assign_description)}</DialogDescription>
          </DialogHeader>

          <form className="space-y-4" onSubmit={assignOrder} noValidate>
            <FieldError errors={[formErrors.root?.serverError]} />
            <Field data-invalid={Boolean(formErrors.email)}>
              <FieldLabel htmlFor="order-assign-email">
                {t(($) => $.admin.orders.assign_email_label)}
              </FieldLabel>
              <Controller
                control={form.control}
                name="email"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    id="order-assign-email"
                    type="email"
                    placeholder={t(($) => $.admin.orders.assign_email_placeholder)}
                    data-testid="order-assign-email"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
              <FieldError errors={[formErrors.email]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.plan_id)}>
              <FieldLabel htmlFor="order-assign-plan">
                {t(($) => $.admin.orders.assign_plan_label)}
              </FieldLabel>
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
                      <SelectValue placeholder={t(($) => $.admin.orders.assign_plan_label)} />
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
              <FieldLabel htmlFor="order-assign-period">
                {t(($) => $.admin.orders.assign_period_label)}
              </FieldLabel>
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
                      <SelectValue placeholder={t(($) => $.admin.orders.assign_period_label)} />
                    </SelectTrigger>
                    <SelectContent>
                      {periodOptions.map((option) => (
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
              <FieldLabel htmlFor="order-assign-amount">
                {t(($) => $.admin.orders.total_amount)}
              </FieldLabel>
              <div className="relative">
                <Controller
                  control={form.control}
                  name="total_amount"
                  render={({ field, fieldState }) => (
                    <Input
                      {...field}
                      id="order-assign-amount"
                      className="pr-8"
                      placeholder={t(($) => $.admin.orders.assign_amount_placeholder)}
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
                {t(($) => $.common.cancel)}
              </Button>
              <Button
                type="submit"
                disabled={assign.isPending || isSubmitting}
                loading={assign.isPending || isSubmitting}
                data-testid="order-assign-submit"
              >
                {t(($) => $.common.confirm)}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default function OrdersPage() {
  const { t } = useTranslation();
  const periodText = periodTextMap(t);
  const orderTypeText = orderTypeTextMap(t);
  const orderStatus = orderStatusMap(t);
  const commissionStatus = commissionStatusMap(t);
  const pagination = paginationLabels(t);
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
      title: t(($) => $.admin.orders.cancel_order),
      description: t(($) => $.admin.orders.cancel_order_confirm),
      confirmText: t(($) => $.common.confirm),
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
    const info = orderStatus[row.status] ?? orderStatus[0]!;
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
            {t(($) => $.admin.orders.mark_paid)}
          </DropdownMenuItem>
          <DropdownMenuItem
            variant="destructive"
            onClick={() => void cancelOrder(row.trade_no)}
            data-testid={`order-cancel-${row.trade_no}`}
          >
            {t(($) => $.admin.orders.cancel_order)}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  };

  const renderCommissionStatus = (row: AdminOrderRow) => {
    if (row.status === 0 || row.status === 2 || !row.commission_balance) return '-';
    const value = row.commission_status;
    const info = commissionStatus[value] ?? commissionStatus[0]!;
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
            {t(($) => $.admin.orders.commission_pending)}
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={value === 1}
            onClick={() => updateCommission(row.trade_no, '1')}
          >
            {t(($) => $.admin.orders.commission_valid)}
          </DropdownMenuItem>
          <DropdownMenuItem
            disabled={value === 3}
            onClick={() => updateCommission(row.trade_no, '3')}
          >
            {t(($) => $.admin.orders.commission_invalid)}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  };

  const columns: DataTableColumn<AdminOrderRow>[] = [
    {
      id: 'trade_no',
      header: () => <span>{t(($) => $.admin.orders.trade_no_col)}</span>,
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
      header: () => <span>{t(($) => $.admin.orders.type_col)}</span>,
      cell: ({ row }) => orderTypeText[row.original.type],
    },
    {
      id: 'plan_name',
      meta: { className: 'text-foreground' },
      header: () => <span>{t(($) => $.admin.orders.plan)}</span>,
      cell: ({ row }) => row.original.plan_name,
    },
    {
      id: 'period',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.orders.period_col)}</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">{periodText[row.original.period] ?? row.original.period}</Badge>
      ),
    },
    {
      id: 'total_amount',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.orders.total_amount)}</span>,
      cell: ({ row }) => cents(row.original.total_amount),
    },
    {
      id: 'status',
      header: () => (
        <HeaderTooltip title={t(($) => $.admin.orders.status_tooltip)}>
          {t(($) => $.admin.orders.order_status)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => renderOrderStatus(row.original),
    },
    {
      id: 'commission_balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.orders.commission_amount)}</span>,
      cell: ({ row }) =>
        row.original.status === 0 || row.original.status === 2 || !row.original.commission_balance
          ? '-'
          : cents(row.original.commission_balance),
    },
    {
      id: 'commission_status',
      header: () => (
        <HeaderTooltip title={t(($) => $.admin.orders.commission_status_tooltip)}>
          {t(($) => $.admin.orders.commission_status)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => renderCommissionStatus(row.original),
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.orders.created_at)}</span>,
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
    },
  ];

  return (
    <PageShell data-testid="orders-page">
      {orders.isError ? (
        <ErrorState
          message={t(($) => $.admin.orders.list_load_failed)}
          onRetry={() => void orders.refetch()}
        />
      ) : null}
      {plans.isError ? (
        <ErrorState
          message={t(($) => $.admin.orders.plans_load_failed)}
          onRetry={() => void plans.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.orders.title)}
        actions={
          plansReady ? (
            <AssignOrderDialog plans={planData} />
          ) : (
            <Button disabled data-testid="order-assign-open">
              {t(($) => $.admin.orders.assign_order)}
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
                        ? t(($) => $.admin.orders.order_status)
                        : (orderStatus[Number(statusValue)]?.label ??
                          t(($) => $.admin.orders.order_status))}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuLabel>{t(($) => $.admin.orders.order_status)}</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    <DropdownMenuRadioGroup
                      value={statusValue}
                      onValueChange={(value) =>
                        setFilter('status', '=', value === 'all' ? '' : value)
                      }
                    >
                      <DropdownMenuRadioItem value="all">
                        {t(($) => $.common.all)}
                      </DropdownMenuRadioItem>
                      {Object.keys(orderStatus).map((code) => (
                        <DropdownMenuRadioItem key={code} value={code}>
                          {orderStatus[Number(code)]!.label}
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
                    {t(($) => $.admin.orders.clear_filters)}
                  </Button>
                ) : null}
              </div>
              <div className="relative w-full sm:w-64">
                <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  className="pl-9"
                  placeholder={t(($) => $.admin.orders.search_placeholder)}
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
                  ? t(($) => $.admin.orders.empty)
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
                labels={pagination}
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
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}

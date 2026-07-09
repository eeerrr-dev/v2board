import { cloneElement, useState, type ReactElement } from 'react';
import dayjs from 'dayjs';
import { useLocation } from 'react-router';
import { Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import type { Coupon, CouponType, Giftcard, Plan } from '@v2board/types';
import {
  useAdminCoupons,
  useAdminGiftcards,
  useAdminPlans,
  useDropCouponMutation,
  useDropGiftcardMutation,
  useGenerateCouponMutation,
  useGenerateGiftcardMutation,
  useShowCouponMutation,
} from '@/lib/queries';
import { toast } from '@/lib/toast';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
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
import { Switch } from '@/components/ui/switch';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

type CouponSubmit = admin.GenerateCouponPayload;
type GiftcardSubmit = admin.GenerateGiftcardPayload;
type GenerateResponse = admin.GenerateCsvResponse;
type CouponRow = Coupon;
type GiftcardRow = Giftcard;

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

const PAGE_SIZE_OPTIONS = [10, 50, 100, 150];

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
}

function planOptions(plans: Plan[] | undefined) {
  return (plans ?? []).map((plan) => ({ value: `${plan.id}`, label: plan.name }));
}

// The validity window persists as unix SECONDS. Convert to ms for the local
// datetime-local field and back to a seconds string on change, mirroring the
// legacy dayjs(1000 * sec) / dayjs(value).format('X') round-trip exactly.
function toDateTimeLocal(seconds?: number | string | null) {
  return seconds ? dayjs(1000 * Number(seconds)).format('YYYY-MM-DDTHH:mm') : '';
}

function fromDateTimeLocal(value: string) {
  return value ? dayjs(value).format('X') : null;
}

function dateRange(startedAt?: number | string | null, endedAt?: number | string | null) {
  return `${dayjs(1000 * Number(startedAt)).format('YYYY/MM/DD HH:mm')} ~ ${dayjs(
    1000 * Number(endedAt),
  ).format('YYYY/MM/DD HH:mm')}`;
}

function giftcardValueUnit(type: GiftcardSubmit['type']) {
  switch (type) {
    case 1:
      return '¥';
    case 2:
      return '天';
    case 3:
      return 'GB';
    case 4:
      return '';
    case 5:
      return '天';
    default:
      return '';
  }
}

// Preserve the legacy CSV download for batch generation: the /generate endpoint
// returns an arraybuffer of codes only when generate_count is set.
function downloadGeneratedCsv(prefix: 'COUPON' | 'GIFTCARD', buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = `${prefix} ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

function useCopy() {
  return (text: string) => {
    void navigator.clipboard?.writeText(text);
    toast.success('复制成功');
  };
}

function CopyableCode({ value, onCopy }: { value: string; onCopy: (value: string) => void }) {
  return (
    <button type="button" onClick={() => onCopy(value)} className="inline-flex">
      <Badge variant="secondary" className="cursor-pointer font-mono">
        {value}
      </Badge>
    </button>
  );
}

interface CheckboxGroupProps {
  options: { value: string; label: string }[];
  value: string[];
  onChange: (value: string[]) => void;
  testId?: string;
}

function CheckboxGroup({ options, value, onChange, testId }: CheckboxGroupProps) {
  const toggle = (option: string, checked: boolean) => {
    onChange(checked ? [...value, option] : value.filter((item) => item !== option));
  };
  return (
    <div
      className="flex flex-wrap gap-x-4 gap-y-2 rounded-md border border-input p-3"
      data-testid={testId}
    >
      {options.map((option) => {
        const checked = value.includes(option.value);
        return (
          <label
            key={option.value}
            className="flex cursor-pointer items-center gap-2 text-sm text-foreground"
          >
            <Checkbox
              checked={checked}
              onCheckedChange={(next) => toggle(option.value, next === true)}
            />
            {option.label}
          </label>
        );
      })}
    </div>
  );
}

export default function CouponsPage() {
  const location = useLocation();
  if (location.pathname === '/giftcard') return <GiftcardsView />;
  return <CouponsView />;
}

function CouponEditor({
  record,
  plans,
  pending,
  onSave,
  onSaved,
  children,
}: {
  record?: CouponRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: CouponSubmit) => Promise<GenerateResponse>;
  onSaved: () => void | Promise<unknown>;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<CouponSubmit>(() => ({ type: 1, ...(record ?? {}) }));

  const openSheet = () => {
    setSubmit({ type: 1, ...(record ?? {}) });
    setOpen(true);
  };

  const patch = (next: Partial<CouponSubmit>) => setSubmit((current) => ({ ...current, ...next }));

  const save = async () => {
    const payload: CouponSubmit = { ...submit };
    // Amount coupons store cents; percent coupons store the raw percentage.
    if (payload.type === 1) payload.value = 100 * Number(payload.value);
    const response = await onSave(payload);
    if (payload.generate_count) downloadGeneratedCsv('COUPON', response.buffer);
    await onSaved();
    setOpen(false);
  };

  const selectedPlanIds = (submit.limit_plan_ids ?? []).map(String);
  const selectedPeriods = (submit.limit_period ?? []).map(String);

  return (
    <>
      {cloneElement(children, { onClick: openSheet })}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="right"
          className="w-full gap-0 overflow-y-auto sm:max-w-md"
          data-testid="coupon-editor"
        >
          <SheetHeader>
            <SheetTitle>{submit.id ? '编辑优惠券' : '新建优惠券'}</SheetTitle>
          </SheetHeader>

          <div className="space-y-4 px-4 pb-4">
            <div className="space-y-2">
              <Label htmlFor="coupon-name">名称</Label>
              <Input
                id="coupon-name"
                placeholder="请输入优惠券名称"
                value={(submit.name as string | undefined) ?? ''}
                onChange={(event) => patch({ name: event.target.value })}
                data-testid="coupon-name"
              />
            </div>

            {!submit.generate_count ? (
              <div className="space-y-2">
                <Label htmlFor="coupon-code">自定义优惠券码</Label>
                <Input
                  id="coupon-code"
                  placeholder="自定义优惠券码(留空随机生成)"
                  value={(submit.code as string | undefined) ?? ''}
                  onChange={(event) =>
                    patch({ code: event.target.value, generate_count: undefined })
                  }
                  data-testid="coupon-code"
                />
              </div>
            ) : null}

            <div className="space-y-2">
              <Label htmlFor="coupon-value">优惠信息</Label>
              <div className="flex gap-2">
                <Select
                  value={String(submit.type ?? 1)}
                  onValueChange={(value) => patch({ type: Number(value) as CouponType })}
                >
                  <SelectTrigger className="w-36 shrink-0" data-testid="coupon-type">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">按金额优惠</SelectItem>
                    <SelectItem value="2">按比例优惠</SelectItem>
                  </SelectContent>
                </Select>
                <div className="relative flex-1">
                  <Input
                    id="coupon-value"
                    type="number"
                    className="pr-8"
                    placeholder="请输入值"
                    value={submit.value ?? ''}
                    onChange={(event) => patch({ value: event.target.value })}
                    data-testid="coupon-value"
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                    {submit.type === 1 ? '¥' : '%'}
                  </span>
                </div>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label htmlFor="coupon-start">开始时间</Label>
                <Input
                  id="coupon-start"
                  type="datetime-local"
                  value={toDateTimeLocal(submit.started_at)}
                  onChange={(event) => patch({ started_at: fromDateTimeLocal(event.target.value) })}
                  data-testid="coupon-start"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="coupon-end">结束时间</Label>
                <Input
                  id="coupon-end"
                  type="datetime-local"
                  value={toDateTimeLocal(submit.ended_at)}
                  onChange={(event) => patch({ ended_at: fromDateTimeLocal(event.target.value) })}
                  data-testid="coupon-end"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="coupon-limit-use">最大使用次数</Label>
              <Input
                id="coupon-limit-use"
                placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
                value={(submit.limit_use as string | number | undefined) ?? ''}
                onChange={(event) => patch({ limit_use: event.target.value })}
                data-testid="coupon-limit-use"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="coupon-limit-use-user">每个用户可使用次数</Label>
              <Input
                id="coupon-limit-use-user"
                placeholder="限制每个用户可使用次数(为空则不限制)"
                value={(submit.limit_use_with_user as string | number | undefined) ?? ''}
                onChange={(event) => patch({ limit_use_with_user: event.target.value })}
                data-testid="coupon-limit-use-user"
              />
            </div>

            <div className="space-y-2">
              <Label>指定订阅</Label>
              {plans.length ? (
                <CheckboxGroup
                  options={planOptions(plans)}
                  value={selectedPlanIds}
                  onChange={(value) => patch({ limit_plan_ids: value.length ? value : null })}
                  testId="coupon-plan-ids"
                />
              ) : (
                <p className="text-sm text-muted-foreground">暂无可选订阅</p>
              )}
            </div>

            <div className="space-y-2">
              <Label>指定周期</Label>
              <CheckboxGroup
                options={PERIOD_OPTIONS}
                value={selectedPeriods}
                onChange={(value) =>
                  patch({ limit_period: (value.length ? value : null) as CouponSubmit['limit_period'] })
                }
                testId="coupon-periods"
              />
            </div>

            {!submit.code && !submit.id ? (
              <div className="space-y-2">
                <Label htmlFor="coupon-generate-count">生成数量</Label>
                <Input
                  id="coupon-generate-count"
                  placeholder="输入数量批量生成"
                  value={(submit.generate_count as string | number | undefined) ?? ''}
                  onChange={(event) =>
                    patch({ generate_count: event.target.value, code: undefined })
                  }
                  data-testid="coupon-generate-count"
                />
              </div>
            ) : null}
          </div>

          <SheetFooter>
            <Button onClick={() => void save()} disabled={pending} data-testid="coupon-submit">
              {pending ? <Loader2 className="size-4 animate-spin" /> : null}
              提交
            </Button>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </>
  );
}

function CouponsView() {
  const copy = useCopy();
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const coupons = useAdminCoupons(query);
  const plans = useAdminPlans();
  const generate = useGenerateCouponMutation();
  const drop = useDropCouponMutation();
  const show = useShowCouponMutation();

  const data = coupons.data?.data ?? [];
  const total = coupons.data?.total ?? 0;

  const removeCoupon = async (row: CouponRow) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    await drop.mutateAsync(row.id);
    void coupons.refetch();
  };

  const columns: DataTableColumn<CouponRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>启用</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(row.original.show)}
          onCheckedChange={() =>
            show.mutate(row.original.id, {
              onSuccess: () => {
                void coupons.refetch();
              },
            })
          }
          aria-label={`切换优惠券「${row.original.name}」启用`}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>券名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>类型</span>,
      cell: ({ row }) => (row.original.type === 1 ? '金额' : '比例'),
    },
    {
      id: 'code',
      header: () => <span>券码</span>,
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copy} />,
    },
    {
      id: 'limit_use',
      meta: { align: 'center' },
      header: () => <span>剩余次数</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">
          {row.original.limit_use !== null ? row.original.limit_use : '无限'}
        </Badge>
      ),
    },
    {
      id: 'validity',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>有效期</span>,
      cell: ({ row }) => dateRange(row.original.started_at, row.original.ended_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <CouponEditor
            record={row.original}
            plans={plans.data ?? []}
            pending={generate.isPending}
            onSave={(payload) => generate.mutateAsync(payload)}
            onSaved={() => coupons.refetch()}
          >
            <Button variant="ghost" size="sm" data-testid={`coupon-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </CouponEditor>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeCoupon(row.original)}
            data-testid={`coupon-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="coupons-page">
      <PageHeader
        title="优惠券管理"
        actions={
          <CouponEditor
            plans={plans.data ?? []}
            pending={generate.isPending}
            onSave={(payload) => generate.mutateAsync(payload)}
            onSaved={() => coupons.refetch()}
          >
            <Button data-testid="coupon-create">
              <Plus className="size-4" />
              添加优惠券
            </Button>
          </CouponEditor>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[900px]"
            data-testid="coupons-table"
            empty={data.length === 0 ? '暂无优惠券' : undefined}
            emptyTestId="coupons-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current}
              pageSize={query.pageSize}
              total={total}
              pageSizeOptions={PAGE_SIZE_OPTIONS}
              labels={PAGINATION_LABELS}
              onChange={(page, pageSize) => setQuery({ current: page, pageSize })}
              testIds={{ page: 'coupon-page', pageSize: 'coupon-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      {coupons.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

function GiftcardEditor({
  record,
  plans,
  pending,
  onSave,
  onSaved,
  children,
}: {
  record?: GiftcardRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: GiftcardSubmit) => Promise<GenerateResponse>;
  onSaved: () => void | Promise<unknown>;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<GiftcardSubmit>(() => ({ type: 1, ...(record ?? {}) }));

  const openSheet = () => {
    setSubmit({ type: 1, ...(record ?? {}) });
    setOpen(true);
  };

  const patch = (next: Partial<GiftcardSubmit>) =>
    setSubmit((current) => ({ ...current, ...next }));

  const save = async () => {
    const payload: GiftcardSubmit = { ...submit };
    if (payload.type === 1) payload.value = 100 * Number(payload.value);
    const response = await onSave(payload);
    if (payload.generate_count) downloadGeneratedCsv('GIFTCARD', response.buffer);
    await onSaved();
    setOpen(false);
  };

  return (
    <>
      {cloneElement(children, { onClick: openSheet })}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="right"
          className="w-full gap-0 overflow-y-auto sm:max-w-md"
          data-testid="giftcard-editor"
        >
          <SheetHeader>
            <SheetTitle>{submit.id ? '编辑礼品卡' : '新建礼品卡'}</SheetTitle>
          </SheetHeader>

          <div className="space-y-4 px-4 pb-4">
            <div className="space-y-2">
              <Label htmlFor="giftcard-name">名称</Label>
              <Input
                id="giftcard-name"
                placeholder="请输入礼品卡名称"
                value={(submit.name as string | undefined) ?? ''}
                onChange={(event) => patch({ name: event.target.value })}
                data-testid="giftcard-name"
              />
            </div>

            {!submit.generate_count ? (
              <div className="space-y-2">
                <Label htmlFor="giftcard-code">自定义礼品卡卡密</Label>
                <Input
                  id="giftcard-code"
                  placeholder="自定义礼品卡卡密(留空随机生成)"
                  value={(submit.code as string | undefined) ?? ''}
                  onChange={(event) =>
                    patch({ code: event.target.value, generate_count: undefined })
                  }
                  data-testid="giftcard-code"
                />
              </div>
            ) : null}

            <div className="space-y-2">
              <Label htmlFor="giftcard-value">礼品卡类型</Label>
              <div className="flex gap-2">
                <Select
                  value={String(submit.type ?? 1)}
                  onValueChange={(value) =>
                    patch({ type: Number(value) as GiftcardSubmit['type'] })
                  }
                >
                  <SelectTrigger className="w-40 shrink-0" data-testid="giftcard-type">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">增加账户余额</SelectItem>
                    <SelectItem value="2">增加订阅时长</SelectItem>
                    <SelectItem value="3">增加套餐流量</SelectItem>
                    <SelectItem value="4">重置套餐流量</SelectItem>
                    <SelectItem value="5">兑换订阅套餐</SelectItem>
                  </SelectContent>
                </Select>
                <div className="relative flex-1">
                  <Input
                    id="giftcard-value"
                    type="number"
                    className="pr-8"
                    disabled={submit.type === 4}
                    placeholder={submit.type === 5 ? '一次性套餐输入0' : '请输入值'}
                    value={submit.type === 4 ? 0 : (submit.value ?? '')}
                    onChange={(event) => patch({ value: event.target.value })}
                    data-testid="giftcard-value"
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                    {giftcardValueUnit(submit.type)}
                  </span>
                </div>
              </div>
            </div>

            {submit.type === 5 ? (
              <div className="space-y-2">
                <Label htmlFor="giftcard-plan">指定订阅</Label>
                <Select
                  value={submit.plan_id != null ? String(submit.plan_id) : undefined}
                  onValueChange={(value) => patch({ plan_id: value ? value : null })}
                >
                  <SelectTrigger id="giftcard-plan" className="w-full" data-testid="giftcard-plan">
                    <SelectValue placeholder="指定订阅" />
                  </SelectTrigger>
                  <SelectContent>
                    {planOptions(plans).map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            ) : null}

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label htmlFor="giftcard-start">开始时间</Label>
                <Input
                  id="giftcard-start"
                  type="datetime-local"
                  value={toDateTimeLocal(submit.started_at)}
                  onChange={(event) => patch({ started_at: fromDateTimeLocal(event.target.value) })}
                  data-testid="giftcard-start"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="giftcard-end">结束时间</Label>
                <Input
                  id="giftcard-end"
                  type="datetime-local"
                  value={toDateTimeLocal(submit.ended_at)}
                  onChange={(event) => patch({ ended_at: fromDateTimeLocal(event.target.value) })}
                  data-testid="giftcard-end"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="giftcard-limit-use">最大使用次数</Label>
              <Input
                id="giftcard-limit-use"
                placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
                value={(submit.limit_use as string | number | undefined) ?? ''}
                onChange={(event) => patch({ limit_use: event.target.value })}
                data-testid="giftcard-limit-use"
              />
            </div>

            {!submit.code && !submit.id ? (
              <div className="space-y-2">
                <Label htmlFor="giftcard-generate-count">生成数量</Label>
                <Input
                  id="giftcard-generate-count"
                  placeholder="输入数量批量生成"
                  value={(submit.generate_count as string | number | undefined) ?? ''}
                  onChange={(event) =>
                    patch({ generate_count: event.target.value, code: undefined })
                  }
                  data-testid="giftcard-generate-count"
                />
              </div>
            ) : null}
          </div>

          <SheetFooter>
            <Button onClick={() => void save()} disabled={pending} data-testid="giftcard-submit">
              {pending ? <Loader2 className="size-4 animate-spin" /> : null}
              提交
            </Button>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </>
  );
}

function GiftcardsView() {
  const copy = useCopy();
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const giftcards = useAdminGiftcards(query);
  const plans = useAdminPlans();
  const generate = useGenerateGiftcardMutation();
  const drop = useDropGiftcardMutation();

  const data = giftcards.data?.data ?? [];
  const total = giftcards.data?.total ?? 0;

  const planName = (id: number | string | null | undefined) =>
    (plans.data ?? []).find((plan) => plan.id === id)?.name ?? '-';

  const renderValue = (value: number, type: Giftcard['type']) => {
    switch (type) {
      case 1:
        return `${value.toFixed(2)} ¥`;
      case 2:
        return `${value} 天`;
      case 3:
        return `${value} GB`;
      case 4:
        return '-';
      case 5:
        return `${value} 天`;
      default:
        return value;
    }
  };

  const typeLabel = (type: Giftcard['type']) => {
    switch (type) {
      case 1:
        return '金额';
      case 2:
        return '时长';
      case 3:
        return '流量';
      case 4:
        return '重置';
      case 5:
        return '套餐';
      default:
        return '';
    }
  };

  const removeGiftcard = async (row: GiftcardRow) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    await drop.mutateAsync(row.id);
    void giftcards.refetch();
  };

  const columns: DataTableColumn<GiftcardRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>类型</span>,
      cell: ({ row }) => typeLabel(row.original.type),
    },
    {
      id: 'value',
      meta: { className: 'tabular-nums' },
      header: () => <span>数值</span>,
      cell: ({ row }) => renderValue(row.original.value, row.original.type),
    },
    {
      id: 'plan',
      header: () => <span>套餐</span>,
      cell: ({ row }) => planName(row.original.plan_id),
    },
    {
      id: 'code',
      header: () => <span>卡密</span>,
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copy} />,
    },
    {
      id: 'limit_use',
      meta: { align: 'center' },
      header: () => <span>剩余次数</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">
          {row.original.limit_use !== null ? row.original.limit_use : '无限'}
        </Badge>
      ),
    },
    {
      id: 'validity',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>有效期</span>,
      cell: ({ row }) => dateRange(row.original.started_at, row.original.ended_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <GiftcardEditor
            record={row.original}
            plans={plans.data ?? []}
            pending={generate.isPending}
            onSave={(payload) => generate.mutateAsync(payload)}
            onSaved={() => giftcards.refetch()}
          >
            <Button variant="ghost" size="sm" data-testid={`giftcard-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </GiftcardEditor>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeGiftcard(row.original)}
            data-testid={`giftcard-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="giftcards-page">
      <PageHeader
        title="礼品卡管理"
        actions={
          <GiftcardEditor
            plans={plans.data ?? []}
            pending={generate.isPending}
            onSave={(payload) => generate.mutateAsync(payload)}
            onSaved={() => giftcards.refetch()}
          >
            <Button data-testid="giftcard-create">
              <Plus className="size-4" />
              添加礼品卡
            </Button>
          </GiftcardEditor>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[960px]"
            data-testid="giftcards-table"
            empty={data.length === 0 ? '暂无礼品卡' : undefined}
            emptyTestId="giftcards-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current}
              pageSize={query.pageSize}
              total={total}
              pageSizeOptions={PAGE_SIZE_OPTIONS}
              labels={PAGINATION_LABELS}
              onChange={(page, pageSize) => setQuery({ current: page, pageSize })}
              testIds={{ page: 'giftcard-page', pageSize: 'giftcard-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      {giftcards.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

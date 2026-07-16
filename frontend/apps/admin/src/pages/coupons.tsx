import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import dayjs from 'dayjs';
import { useLocation } from 'react-router';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import type { admin } from '@v2board/api-client';
import type { Coupon, CouponType, Giftcard, Plan } from '@v2board/types';
import { copyText } from '@v2board/config/clipboard';
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
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
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
  SheetTrigger,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import {
  couponEditorSchema,
  giftcardEditorSchema,
  type CouponEditorValues,
  type GiftcardEditorValues,
} from './coupon-form-schema';

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

// The validity window persists as a decimal unix-seconds string. Use Day.js's
// core unix() API here: the `X` format token requires AdvancedFormat and would
// otherwise be emitted literally as "X".
function toDateTimeLocal(seconds?: number | string | null) {
  return seconds ? dayjs(1000 * Number(seconds)).format('YYYY-MM-DDTHH:mm') : '';
}

function fromDateTimeLocal(value: string) {
  return value ? String(dayjs(value).unix()) : null;
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

function normalizeGenerationPayload<T extends object>(value: T) {
  return Object.fromEntries(
    Object.entries(value).filter(
      ([key, fieldValue]) =>
        fieldValue !== undefined && !(key === 'generate_count' && fieldValue === ''),
    ),
  ) as T;
}

// Preserve the CSV download contract for batch generation: the /generate endpoint
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

async function copyWithToast(text: string) {
  if (await copyText(text)) toast.success('复制成功');
  else toast.error('复制失败');
}

function CopyableCode({
  value,
  onCopy,
}: {
  value: string;
  onCopy: (value: string) => Promise<void>;
}) {
  return (
    <button type="button" onClick={() => void onCopy(value)} className="inline-flex">
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
  children,
}: {
  record?: CouponRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: CouponSubmit, onSuccess: (response: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<CouponEditorValues>({
    resolver: zodResolver(couponEditorSchema),
    defaultValues: {
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      started_at: record?.started_at ?? null,
      ended_at: record?.ended_at ?? null,
      limit_use: record?.limit_use ?? null,
      limit_use_with_user: record?.limit_use_with_user ?? null,
      limit_plan_ids: record?.limit_plan_ids ?? null,
      limit_period: record?.limit_period ?? null,
      generate_count: undefined,
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const values = useWatch({ control: form.control });

  const openSheet = () => {
    form.reset({
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      started_at: record?.started_at ?? null,
      ended_at: record?.ended_at ?? null,
      limit_use: record?.limit_use ?? null,
      limit_use_with_user: record?.limit_use_with_user ?? null,
      limit_plan_ids: record?.limit_plan_ids ?? null,
      limit_period: record?.limit_period ?? null,
      generate_count: undefined,
    });
    setOpen(true);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(normalizeGenerationPayload(validValues) as CouponSubmit, (response) => {
      if (validValues.generate_count) downloadGeneratedCsv('COUPON', response.buffer);
      setOpen(false);
    });
  });

  const selectedPlanIds = (values.limit_plan_ids ?? []).map(String);
  const selectedPeriods = (values.limit_period ?? []).map(String);

  return (
    <Sheet
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) openSheet();
        else setOpen(false);
      }}
    >
      <SheetTrigger asChild>{children}</SheetTrigger>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-md"
        data-testid="coupon-editor"
      >
        <SheetHeader>
          <SheetTitle>{record?.id ? '编辑优惠券' : '新建优惠券'}</SheetTitle>
          <SheetDescription>设置优惠额度、使用限制、有效期和适用订阅。</SheetDescription>
        </SheetHeader>

        <form id="coupon-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="coupon-name">名称</FieldLabel>
            <Input
              id="coupon-name"
              placeholder="请输入优惠券名称"
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="coupon-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="coupon-code">自定义优惠券码</FieldLabel>
              <Input
                id="coupon-code"
                placeholder="自定义优惠券码(留空随机生成)"
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="coupon-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="coupon-value">优惠信息</FieldLabel>
            <div className="flex gap-2">
              <Select
                value={String(values.type ?? 1)}
                onValueChange={(value) => form.setValue('type', Number(value) as CouponType)}
              >
                <SelectTrigger
                  className="w-36 shrink-0"
                  data-testid="coupon-type"
                  aria-label="优惠类型"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">按金额优惠</SelectItem>
                  <SelectItem value="2">按比例优惠</SelectItem>
                </SelectContent>
              </Select>
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="coupon-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  placeholder="请输入值"
                  aria-invalid={Boolean(formErrors.value)}
                  {...form.register('value')}
                  data-testid="coupon-value"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{values.type === 1 ? '¥' : '%'}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
            <FieldError errors={[formErrors.value]} />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.started_at)}>
              <FieldLabel htmlFor="coupon-start">开始时间</FieldLabel>
              <Controller
                control={form.control}
                name="started_at"
                render={({ field }) => (
                  <Input
                    id="coupon-start"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('started_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.started_at)}
                    data-testid="coupon-start"
                  />
                )}
              />
              <FieldError errors={[formErrors.started_at]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.ended_at)}>
              <FieldLabel htmlFor="coupon-end">结束时间</FieldLabel>
              <Controller
                control={form.control}
                name="ended_at"
                render={({ field }) => (
                  <Input
                    id="coupon-end"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('ended_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.ended_at)}
                    data-testid="coupon-end"
                  />
                )}
              />
              <FieldError errors={[formErrors.ended_at]} />
            </Field>
          </div>

          <Field data-invalid={Boolean(formErrors.limit_use)}>
            <FieldLabel htmlFor="coupon-limit-use">最大使用次数</FieldLabel>
            <Input
              id="coupon-limit-use"
              type="number"
              step="1"
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="coupon-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          <Field data-invalid={Boolean(formErrors.limit_use_with_user)}>
            <FieldLabel htmlFor="coupon-limit-use-user">每个用户可使用次数</FieldLabel>
            <Input
              id="coupon-limit-use-user"
              type="number"
              step="1"
              placeholder="限制每个用户可使用次数(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use_with_user)}
              {...form.register('limit_use_with_user')}
              data-testid="coupon-limit-use-user"
            />
            <FieldError errors={[formErrors.limit_use_with_user]} />
          </Field>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">指定订阅</legend>
            {plans.length ? (
              <CheckboxGroup
                options={planOptions(plans)}
                value={selectedPlanIds}
                onChange={(value) => form.setValue('limit_plan_ids', value.length ? value : null)}
                testId="coupon-plan-ids"
              />
            ) : (
              <p className="text-sm text-muted-foreground">暂无可选订阅</p>
            )}
          </fieldset>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">指定周期</legend>
            <CheckboxGroup
              options={PERIOD_OPTIONS}
              value={selectedPeriods}
              onChange={(value) => form.setValue('limit_period', value.length ? value : null)}
              testId="coupon-periods"
            />
          </fieldset>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="coupon-generate-count">生成数量</FieldLabel>
              <Input
                id="coupon-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder="输入数量批量生成"
                aria-invalid={Boolean(formErrors.generate_count)}
                {...form.register('generate_count', {
                  onChange: () => form.setValue('code', undefined),
                })}
                data-testid="coupon-generate-count"
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
        </form>

        <SheetFooter>
          <Button
            type="submit"
            form="coupon-editor-form"
            disabled={pending}
            data-testid="coupon-submit"
          >
            {pending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            提交
          </Button>
          <Button variant="outline" onClick={() => setOpen(false)}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function CouponsView() {
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const coupons = useAdminCoupons(query);
  const plans = useAdminPlans();
  const generate = useGenerateCouponMutation();
  const drop = useDropCouponMutation();
  const show = useShowCouponMutation();
  const planOptions = plans.data;
  const plansReady = !plans.isError && planOptions !== undefined;

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
    drop.mutate(row.id);
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
          onCheckedChange={() => show.mutate(row.original.id)}
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
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copyWithToast} />,
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
          {plansReady ? (
            <CouponEditor
              record={row.original}
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button variant="ghost" size="sm" data-testid={`coupon-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </CouponEditor>
          ) : (
            <Button variant="ghost" size="sm" disabled>
              <Pencil className="size-4" />
              编辑
            </Button>
          )}
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
      {coupons.isError ? (
        <ErrorState message="优惠券列表加载失败" onRetry={() => void coupons.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      <PageHeader
        title="优惠券管理"
        actions={
          plansReady ? (
            <CouponEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="coupon-create">
                <Plus className="size-4" />
                添加优惠券
              </Button>
            </CouponEditor>
          ) : (
            <Button disabled data-testid="coupon-create">
              <Plus className="size-4" />
              添加优惠券
            </Button>
          )
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
            empty={
              !coupons.isError && coupons.data !== undefined && data.length === 0
                ? '暂无优惠券'
                : undefined
            }
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
  children,
}: {
  record?: GiftcardRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: GiftcardSubmit, onSuccess: (response: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<GiftcardEditorValues>({
    resolver: zodResolver(giftcardEditorSchema),
    defaultValues: {
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      plan_id: record?.plan_id ?? null,
      started_at: record?.started_at ?? null,
      ended_at: record?.ended_at ?? null,
      limit_use: record?.limit_use ?? null,
      generate_count: undefined,
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const values = useWatch({ control: form.control });

  const openSheet = () => {
    form.reset({
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      plan_id: record?.plan_id ?? null,
      started_at: record?.started_at ?? null,
      ended_at: record?.ended_at ?? null,
      limit_use: record?.limit_use ?? null,
      generate_count: undefined,
    });
    setOpen(true);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(normalizeGenerationPayload(validValues) as GiftcardSubmit, (response) => {
      if (validValues.generate_count) downloadGeneratedCsv('GIFTCARD', response.buffer);
      setOpen(false);
    });
  });

  return (
    <Sheet
      open={open}
      onOpenChange={(nextOpen) => {
        if (nextOpen) openSheet();
        else setOpen(false);
      }}
    >
      <SheetTrigger asChild>{children}</SheetTrigger>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-md"
        data-testid="giftcard-editor"
      >
        <SheetHeader>
          <SheetTitle>{record?.id ? '编辑礼品卡' : '新建礼品卡'}</SheetTitle>
          <SheetDescription>设置礼品卡额度、订阅计划、数量和有效期。</SheetDescription>
        </SheetHeader>

        <form id="giftcard-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="giftcard-name">名称</FieldLabel>
            <Input
              id="giftcard-name"
              placeholder="请输入礼品卡名称"
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="giftcard-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="giftcard-code">自定义礼品卡卡密</FieldLabel>
              <Input
                id="giftcard-code"
                placeholder="自定义礼品卡卡密(留空随机生成)"
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="giftcard-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="giftcard-value">礼品卡类型</FieldLabel>
            <div className="flex gap-2">
              <Select
                value={String(values.type ?? 1)}
                onValueChange={(value) =>
                  form.setValue('type', Number(value) as GiftcardEditorValues['type'])
                }
              >
                <SelectTrigger
                  className="w-40 shrink-0"
                  data-testid="giftcard-type"
                  aria-label="礼品卡类型"
                >
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
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="giftcard-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  disabled={values.type === 4}
                  placeholder={values.type === 5 ? '一次性套餐输入0' : '请输入值'}
                  value={values.type === 4 ? 0 : (values.value ?? '')}
                  onChange={(event) => form.setValue('value', event.target.value)}
                  aria-invalid={Boolean(formErrors.value)}
                  data-testid="giftcard-value"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{giftcardValueUnit(values.type)}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
            <FieldError errors={[formErrors.value]} />
          </Field>

          {values.type === 5 ? (
            <Field data-invalid={Boolean(formErrors.plan_id)}>
              <FieldLabel htmlFor="giftcard-plan">指定订阅</FieldLabel>
              <Select
                value={values.plan_id != null ? String(values.plan_id) : ''}
                onValueChange={(value) => form.setValue('plan_id', value || null)}
              >
                <SelectTrigger
                  id="giftcard-plan"
                  className="w-full"
                  aria-invalid={Boolean(formErrors.plan_id)}
                  data-testid="giftcard-plan"
                >
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
              <FieldError errors={[formErrors.plan_id]} />
            </Field>
          ) : null}

          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.started_at)}>
              <FieldLabel htmlFor="giftcard-start">开始时间</FieldLabel>
              <Controller
                control={form.control}
                name="started_at"
                render={({ field }) => (
                  <Input
                    id="giftcard-start"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('started_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.started_at)}
                    data-testid="giftcard-start"
                  />
                )}
              />
              <FieldError errors={[formErrors.started_at]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.ended_at)}>
              <FieldLabel htmlFor="giftcard-end">结束时间</FieldLabel>
              <Controller
                control={form.control}
                name="ended_at"
                render={({ field }) => (
                  <Input
                    id="giftcard-end"
                    name={field.name}
                    type="datetime-local"
                    value={toDateTimeLocal(field.value)}
                    onChange={(event) =>
                      form.setValue('ended_at', fromDateTimeLocal(event.target.value), {
                        shouldDirty: true,
                        shouldValidate: true,
                      })
                    }
                    onBlur={field.onBlur}
                    ref={field.ref}
                    aria-invalid={Boolean(formErrors.ended_at)}
                    data-testid="giftcard-end"
                  />
                )}
              />
              <FieldError errors={[formErrors.ended_at]} />
            </Field>
          </div>

          <Field data-invalid={Boolean(formErrors.limit_use)}>
            <FieldLabel htmlFor="giftcard-limit-use">最大使用次数</FieldLabel>
            <Input
              id="giftcard-limit-use"
              type="number"
              step="1"
              placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="giftcard-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="giftcard-generate-count">生成数量</FieldLabel>
              <Input
                id="giftcard-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder="输入数量批量生成"
                aria-invalid={Boolean(formErrors.generate_count)}
                {...form.register('generate_count', {
                  onChange: () => form.setValue('code', undefined),
                })}
                data-testid="giftcard-generate-count"
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
        </form>

        <SheetFooter>
          <Button
            type="submit"
            form="giftcard-editor-form"
            disabled={pending}
            data-testid="giftcard-submit"
          >
            {pending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            提交
          </Button>
          <Button variant="outline" onClick={() => setOpen(false)}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function GiftcardsView() {
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const giftcards = useAdminGiftcards(query);
  const plans = useAdminPlans();
  const generate = useGenerateGiftcardMutation();
  const drop = useDropGiftcardMutation();
  const planOptions = plans.data;
  const plansReady = !plans.isError && planOptions !== undefined;

  const data = giftcards.data?.data ?? [];
  const total = giftcards.data?.total ?? 0;

  const planName = (id: number | string | null | undefined) =>
    planOptions?.find((plan) => plan.id === id)?.name ?? '-';

  const renderValue = (value: Giftcard['value'], type: Giftcard['type']) => {
    if (value === null) return '-';
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
    drop.mutate(row.id);
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
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copyWithToast} />,
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
          {plansReady ? (
            <GiftcardEditor
              record={row.original}
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button variant="ghost" size="sm" data-testid={`giftcard-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </GiftcardEditor>
          ) : (
            <Button variant="ghost" size="sm" disabled>
              <Pencil className="size-4" />
              编辑
            </Button>
          )}
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
      {giftcards.isError ? (
        <ErrorState message="礼品卡列表加载失败" onRetry={() => void giftcards.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      <PageHeader
        title="礼品卡管理"
        actions={
          plansReady ? (
            <GiftcardEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="giftcard-create">
                <Plus className="size-4" />
                添加礼品卡
              </Button>
            </GiftcardEditor>
          ) : (
            <Button disabled data-testid="giftcard-create">
              <Plus className="size-4" />
              添加礼品卡
            </Button>
          )
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
            empty={
              !giftcards.isError && giftcards.data !== undefined && data.length === 0
                ? '暂无礼品卡'
                : undefined
            }
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

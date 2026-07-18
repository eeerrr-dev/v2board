import { useState, type ReactElement } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import type { admin } from '@v2board/api-client';
import type { Plan, PlanPeriod } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import {
  type FieldPath,
  type FieldPathValue,
  useForm,
  useFormState,
  useWatch,
} from 'react-hook-form';
import {
  useAdminPlans,
  useConfig,
  useDropPlanMutation,
  useSavePlanMutation,
  useServerGroups,
  useSortPlansMutation,
  useUpdatePlanMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { HeaderTooltip } from '@/components/ui/header-tooltip';
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
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { planEditorSchema, type PlanEditorValues } from './plan-form-schema';

// The plan (subscription) manager is a redesigned shadcn island. The Tier-1
// contract is the shared backend: the /plan/fetch shape, the /plan/save payload
// (the API client whitelists backend-consumed editor fields and converts prices
// to cents, so untouched/emptied prices stay `null` and never become NaN), the
// /plan/sort id list, /plan/drop id, and the dedicated /plan/update
// { id, [key]: value } show/renew toggles.
type SavePlanPayload = Parameters<typeof admin.savePlan>[1];
type EditablePlan = PlanEditorValues;

const PRICE_FIELDS: { key: PlanPeriod; label: string }[] = [
  { key: 'month_price', label: '月付' },
  { key: 'quarter_price', label: '季付' },
  { key: 'half_year_price', label: '半年' },
  { key: 'year_price', label: '年付' },
  { key: 'two_year_price', label: '两年付' },
  { key: 'three_year_price', label: '三年付' },
  { key: 'onetime_price', label: '一次性' },
  { key: 'reset_price', label: '重置包' },
];

// null → 跟随系统设置. Radix Select values are non-empty strings, so `null` is
// carried as the 'null' sentinel and converted back on change.
const RESET_TRAFFIC_OPTIONS: { value: string; label: string }[] = [
  { value: 'null', label: '跟随系统设置' },
  { value: '0', label: '每月1号' },
  { value: '1', label: '按月重置' },
  { value: '2', label: '不重置' },
  { value: '3', label: '每年1月1日' },
  { value: '4', label: '按年重置' },
];

function parseResetTrafficMethod(value: string): EditablePlan['reset_traffic_method'] {
  switch (value) {
    case 'null':
      return null;
    case '0':
      return 0;
    case '1':
      return 1;
    case '2':
      return 2;
    case '3':
      return 3;
    case '4':
      return 4;
    default:
      throw new Error(`Unsupported reset traffic method: ${value}`);
  }
}

// New plans start with every price explicitly null so the api-client leaves them
// null (not for sale) instead of scaling `undefined` into NaN.
function emptyPlan(): EditablePlan {
  return {
    name: null,
    transfer_enable: null,
    group_id: undefined,
    month_price: null,
    quarter_price: null,
    half_year_price: null,
    year_price: null,
    two_year_price: null,
    three_year_price: null,
    onetime_price: null,
    reset_price: null,
  };
}

function formatPrice(value: number | null) {
  return value !== null ? value.toFixed(2) : '-';
}

function inputValue(value: unknown) {
  return value === null || value === undefined ? '' : (value as string | number);
}

function PriceInput({
  id,
  label,
  value,
  currencySymbol,
  onChange,
  error,
}: {
  id: string;
  label: string;
  value: unknown;
  currencySymbol?: string;
  onChange: (value: string) => void;
  error?: { message?: string };
}) {
  return (
    <Field data-invalid={Boolean(error)}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <InputGroup>
        <InputGroupInput
          id={id}
          type="number"
          step="0.01"
          value={inputValue(value)}
          onChange={(event) => onChange(event.target.value)}
          aria-invalid={Boolean(error)}
          data-testid={id}
        />
        {currencySymbol ? (
          <InputGroupAddon align="inline-end">
            <InputGroupText>{currencySymbol}</InputGroupText>
          </InputGroupAddon>
        ) : null}
      </InputGroup>
      <FieldError errors={[error]} />
    </Field>
  );
}

function LimitInput({
  id,
  label,
  value,
  placeholder,
  suffix,
  onChange,
  error,
}: {
  id: string;
  label: string;
  value: unknown;
  placeholder?: string;
  suffix?: string;
  onChange: (value: string) => void;
  error?: { message?: string };
}) {
  return (
    <Field data-invalid={Boolean(error)}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <InputGroup>
        <InputGroupInput
          id={id}
          type="number"
          step="1"
          placeholder={placeholder}
          value={inputValue(value)}
          onChange={(event) => onChange(event.target.value)}
          aria-invalid={Boolean(error)}
          data-testid={id}
        />
        {suffix ? (
          <InputGroupAddon align="inline-end">
            <InputGroupText>{suffix}</InputGroupText>
          </InputGroupAddon>
        ) : null}
      </InputGroup>
      <FieldError errors={[error]} />
    </Field>
  );
}

function PlanEditor({
  record,
  groups,
  currencySymbol,
  pending,
  onSave,
  children,
}: {
  record?: Plan;
  groups: { id: number; name: string }[];
  currencySymbol?: string;
  pending: boolean;
  onSave: (payload: SavePlanPayload, onSuccess: () => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const form = useForm<EditablePlan>({
    resolver: zodResolver(planEditorSchema),
    defaultValues: { ...emptyPlan(), ...record },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const submit = useWatch({ control: form.control });

  const openSheet = () => {
    form.reset({ ...emptyPlan(), ...record });
    setOpen(true);
  };

  const change = <Key extends FieldPath<EditablePlan>>(
    key: Key,
    value: FieldPathValue<EditablePlan, Key>,
  ) => {
    form.setValue(key, value, { shouldDirty: true, shouldValidate: true });
  };

  // Empty price → null so it is not scaled/sold; any other value passes through
  // and is multiplied ×100 in the api-client.
  const priceChange = (key: PlanPeriod, value: string) => {
    change(key, value !== '' ? value : null);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(validValues, () => setOpen(false));
  });

  const resetValue =
    submit.reset_traffic_method === null
      ? 'null'
      : submit.reset_traffic_method === undefined
        ? undefined
        : String(submit.reset_traffic_method);

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
        className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
        data-testid="plan-editor"
      >
        <SheetHeader>
          <SheetTitle>{submit.id ? '编辑订阅' : '新建订阅'}</SheetTitle>
          <SheetDescription>配置订阅流量、权限组、价格周期和展示规则。</SheetDescription>
        </SheetHeader>

        <TooltipProvider delayDuration={100}>
          <form id="plan-editor-form" className="space-y-5 px-4 pb-4" onSubmit={save} noValidate>
            <Field data-invalid={Boolean(formErrors.name)}>
              <FieldLabel htmlFor="plan-name">套餐名称</FieldLabel>
              <Input
                id="plan-name"
                placeholder="请输入套餐名称"
                value={inputValue(submit.name)}
                onChange={(event) => change('name', event.target.value)}
                aria-invalid={Boolean(formErrors.name)}
                data-testid="plan-name"
              />
              <FieldError errors={[formErrors.name]} />
            </Field>

            <Field>
              <FieldLabel htmlFor="plan-content">套餐描述</FieldLabel>
              <Textarea
                id="plan-content"
                rows={6}
                className="font-mono text-xs"
                placeholder="请输入套餐描述，支持HTML"
                value={inputValue(submit.content)}
                onChange={(event) => change('content', event.target.value)}
                data-testid="plan-content"
              />
            </Field>

            <div className="space-y-3">
              <HeaderTooltip
                title="将金额留空则不会进行出售"
                className="text-sm font-medium text-foreground"
              >
                售价设置
              </HeaderTooltip>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                {PRICE_FIELDS.map((field) => (
                  <PriceInput
                    key={field.key}
                    id={`plan-price-${field.key}`}
                    label={field.label}
                    currencySymbol={currencySymbol}
                    value={submit[field.key]}
                    onChange={(value) => priceChange(field.key, value)}
                    error={formErrors[field.key]}
                  />
                ))}
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <LimitInput
                id="plan-transfer-enable"
                label="套餐流量"
                suffix="GB"
                placeholder="请输入套餐流量"
                value={submit.transfer_enable}
                onChange={(value) => change('transfer_enable', value)}
                error={formErrors.transfer_enable}
              />
              <LimitInput
                id="plan-device-limit"
                label="设备数限制"
                placeholder="留空则不限制"
                value={submit.device_limit}
                onChange={(value) => change('device_limit', value)}
                error={formErrors.device_limit}
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <Field data-invalid={Boolean(formErrors.group_id)}>
                <FieldLabel htmlFor="plan-group">权限组</FieldLabel>
                <Select
                  value={submit.group_id != null ? String(submit.group_id) : ''}
                  onValueChange={(value) => change('group_id', Number(value))}
                >
                  <SelectTrigger
                    id="plan-group"
                    className="w-full"
                    data-testid="plan-group"
                    aria-invalid={Boolean(formErrors.group_id)}
                  >
                    <SelectValue placeholder="请选择权限组" />
                  </SelectTrigger>
                  <SelectContent>
                    {groups.map((group) => (
                      <SelectItem key={group.id} value={String(group.id)}>
                        {group.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <FieldError errors={[formErrors.group_id]} />
              </Field>
              <Field>
                <FieldLabel htmlFor="plan-reset-method">流量重置方式</FieldLabel>
                <Select
                  value={resetValue ?? ''}
                  onValueChange={(value) =>
                    change('reset_traffic_method', parseResetTrafficMethod(value))
                  }
                >
                  <SelectTrigger
                    id="plan-reset-method"
                    className="w-full"
                    data-testid="plan-reset-method"
                  >
                    <SelectValue placeholder="请选择流量重置方式" />
                  </SelectTrigger>
                  <SelectContent>
                    {RESET_TRAFFIC_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <LimitInput
                id="plan-capacity-limit"
                label="最大容纳用户量"
                placeholder="留空则不限制"
                value={submit.capacity_limit}
                onChange={(value) => change('capacity_limit', value)}
                error={formErrors.capacity_limit}
              />
              <LimitInput
                id="plan-speed-limit"
                label="限速"
                suffix="Mbps"
                placeholder="留空则不限制"
                value={submit.speed_limit}
                onChange={(value) => change('speed_limit', value)}
                error={formErrors.speed_limit}
              />
            </div>

            <div className="space-y-1.5">
              <label className="flex cursor-pointer items-center gap-2 text-sm text-foreground">
                <Checkbox
                  checked={Boolean(submit.force_update)}
                  onCheckedChange={(value) => change('force_update', value === true)}
                  data-testid="plan-force-update"
                />
                强制更新到用户
              </label>
              <p className="text-xs text-muted-foreground">
                勾选后变更的流量、限速、权限组将应用到该套餐下的用户
              </p>
            </div>
          </form>
        </TooltipProvider>

        <SheetFooter>
          <Button
            type="submit"
            form="plan-editor-form"
            disabled={pending}
            data-testid="plan-submit"
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

export default function PlansPage() {
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const config = useConfig('site');
  const save = useSavePlanMutation();
  const drop = useDropPlanMutation();
  const update = useUpdatePlanMutation();
  const sort = useSortPlansMutation();
  const [orderOverride, setOrderOverride] = useState<Plan[] | null>(null);
  const order = orderOverride ?? plans.data ?? [];
  const groupData = groups.data;
  const groupsReady = !groups.isError && groupData !== undefined;

  const currencySymbol = config.data?.site?.currency_symbol;

  const savePlan = (payload: SavePlanPayload, onSuccess: () => void) => {
    save.mutate(payload, { onSuccess });
  };

  // Adjacent swap reorder. The persisted contract is unchanged: sort.mutate gets
  // the full id list in the new order, then the page refetches.
  const movePlan = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const list = order;
    if (target < 0 || target >= list.length) return;
    const next = [...list];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrderOverride(next);
    sort.mutate(
      next.map((plan) => plan.id),
      {
        onSettled: () => setOrderOverride(null),
      },
    );
  };

  const removePlan = async (record: Plan) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该订阅吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(record.id);
  };

  const updatePlan = (id: number, key: 'show' | 'renew', value: boolean) => {
    update.mutate({ id, key, value });
  };

  const groupName = (id: number) =>
    groupData?.find((group) => group.id === parseInt(String(id), 10))?.name;

  const columns: DataTableColumn<Plan>[] = [
    {
      id: 'sort',
      meta: { align: 'center' },
      header: () => <span>排序</span>,
      cell: ({ row }) => {
        const index = order.findIndex((item) => item.id === row.original.id);
        return (
          <div className="flex items-center justify-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index <= 0}
              onClick={() => movePlan(index, -1)}
              aria-label="上移"
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= order.length - 1}
              onClick={() => movePlan(index, 1)}
              aria-label="下移"
            >
              <ArrowDown className="size-4" />
            </Button>
          </div>
        );
      },
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>销售状态</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          onCheckedChange={() => updatePlan(row.original.id, 'show', !row.original.show)}
          aria-label={`切换「${row.original.name}」销售状态`}
        />
      ),
    },
    {
      id: 'renew',
      meta: { align: 'center' },
      header: () => (
        <HeaderTooltip title="在订阅停止销售时，已购用户是否可以续费" className="justify-center">
          续费
        </HeaderTooltip>
      ),
      cell: ({ row }) => (
        <Switch
          checked={row.original.renew}
          onCheckedChange={() => updatePlan(row.original.id, 'renew', !row.original.renew)}
          aria-label={`切换「${row.original.name}」续费`}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'count',
      meta: { align: 'center', className: 'tabular-nums' },
      header: () => <span>统计</span>,
      cell: ({ row }) => row.original.count,
    },
    {
      id: 'transfer_enable',
      meta: { className: 'tabular-nums' },
      header: () => <span>流量</span>,
      cell: ({ row }) => `${row.original.transfer_enable} GB`,
    },
    {
      id: 'device_limit',
      meta: { className: 'tabular-nums' },
      header: () => <span>设备数限制</span>,
      cell: ({ row }) => (row.original.device_limit !== null ? row.original.device_limit : '-'),
    },
    ...PRICE_FIELDS.map<DataTableColumn<Plan>>((field) => ({
      id: field.key,
      meta: { className: 'tabular-nums text-muted-foreground' },
      header: () => <span>{TABLE_PRICE_LABEL[field.key]}</span>,
      cell: ({ row }) => formatPrice(row.original[field.key]),
    })),
    {
      id: 'group_id',
      header: () => <span>权限组</span>,
      cell: ({ row }) => {
        const name = groupName(row.original.group_id);
        return name ? <Badge variant="secondary">{name}</Badge> : null;
      },
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          {groupsReady ? (
            <PlanEditor
              record={row.original}
              groups={groupData}
              currencySymbol={currencySymbol}
              pending={save.isPending}
              onSave={savePlan}
            >
              <Button variant="ghost" size="sm" data-testid={`plan-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </PlanEditor>
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
            onClick={() => void removePlan(row.original)}
            data-testid={`plan-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="plans-page">
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      {groups.isError ? (
        <ErrorState message="权限组加载失败" onRetry={() => void groups.refetch()} />
      ) : null}
      <PageHeader
        title="订阅管理"
        actions={
          groupsReady ? (
            <PlanEditor
              groups={groupData}
              currencySymbol={currencySymbol}
              pending={save.isPending}
              onSave={savePlan}
            >
              <Button data-testid="plan-create">
                <Plus className="size-4" />
                添加订阅
              </Button>
            </PlanEditor>
          ) : (
            <Button disabled data-testid="plan-create">
              <Plus className="size-4" />
              添加订阅
            </Button>
          )
        }
      />

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <DataTable
              columns={columns}
              data={order}
              getRowKey={(row) => row.id}
              className="min-w-[1180px]"
              data-testid="plans-table"
              empty={
                !plans.isError && plans.data !== undefined && order.length === 0
                  ? '暂无订阅'
                  : undefined
              }
              emptyTestId="plans-empty"
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {sort.isPending || plans.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

const TABLE_PRICE_LABEL: Record<PlanPeriod, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '重置包',
};

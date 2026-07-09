import { cloneElement, useEffect, useRef, useState, type ReactElement } from 'react';
import type { admin } from '@v2board/api-client';
import type { Plan, PlanPeriod } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageHeader, PageShell } from '@/components/ui/page';
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
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';

// The plan (subscription) manager is a redesigned shadcn island. The Tier-1
// contract is the shared backend: the /plan/fetch shape, the /plan/save payload
// (every field passed through verbatim — the per-period price cents ×100 lives
// in the api-client's serializePlanForSave, so this page must keep untouched /
// emptied prices as `null` and never send NaN), the /plan/sort id list, the
// /plan/drop id, and the /plan/update { id, [key]: value } show/renew toggles.
type SavePlanPayload = Parameters<typeof admin.savePlan>[1];
type EditablePlan = SavePlanPayload;

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

// New plans start with every price explicitly null so the api-client leaves them
// null (not for sale) instead of scaling `undefined` into NaN.
function emptyPlan(): EditablePlan {
  return {
    show: 0,
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
}: {
  id: string;
  label: string;
  value: unknown;
  currencySymbol?: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <div className="relative">
        <Input
          id={id}
          type="number"
          className={currencySymbol ? 'pr-8' : undefined}
          value={inputValue(value)}
          onChange={(event) => onChange(event.target.value)}
          data-testid={id}
        />
        {currencySymbol ? (
          <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
            {currencySymbol}
          </span>
        ) : null}
      </div>
    </div>
  );
}

function LimitInput({
  id,
  label,
  value,
  placeholder,
  suffix,
  onChange,
}: {
  id: string;
  label: string;
  value: unknown;
  placeholder?: string;
  suffix?: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <div className="relative">
        <Input
          id={id}
          className={suffix ? 'pr-12' : undefined}
          placeholder={placeholder}
          value={inputValue(value)}
          onChange={(event) => onChange(event.target.value)}
          data-testid={id}
        />
        {suffix ? (
          <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
            {suffix}
          </span>
        ) : null}
      </div>
    </div>
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
  onSave: (payload: SavePlanPayload) => Promise<unknown>;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<EditablePlan>(() => ({ ...(record ?? emptyPlan()) }));

  const openSheet = () => {
    setSubmit({ ...(record ?? emptyPlan()) });
    setOpen(true);
  };

  const change = (key: keyof EditablePlan, value: unknown) => {
    setSubmit((current) => ({ ...current, [key]: value }));
  };

  // Empty price → null so it is not scaled/sold; any other value passes through
  // and is multiplied ×100 in the api-client.
  const priceChange = (key: PlanPeriod, value: string) => {
    change(key, value !== '' ? value : null);
  };

  const save = async () => {
    await onSave({ ...submit });
    setOpen(false);
  };

  const resetValue =
    submit.reset_traffic_method === null
      ? 'null'
      : submit.reset_traffic_method === undefined
        ? undefined
        : String(submit.reset_traffic_method);

  return (
    <>
      {cloneElement(children, { onClick: openSheet })}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="right"
          className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
          data-testid="plan-editor"
        >
          <SheetHeader>
            <SheetTitle>{submit.id ? '编辑订阅' : '新建订阅'}</SheetTitle>
          </SheetHeader>

          <TooltipProvider delayDuration={100}>
            <div className="space-y-5 px-4 pb-4">
              <div className="space-y-2">
                <Label htmlFor="plan-name">套餐名称</Label>
                <Input
                  id="plan-name"
                  placeholder="请输入套餐名称"
                  value={inputValue(submit.name)}
                  onChange={(event) => change('name', event.target.value)}
                  data-testid="plan-name"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="plan-content">套餐描述</Label>
                <Textarea
                  id="plan-content"
                  rows={6}
                  className="font-mono text-xs"
                  placeholder="请输入套餐描述，支持HTML"
                  value={inputValue(submit.content)}
                  onChange={(event) => change('content', event.target.value)}
                  data-testid="plan-content"
                />
              </div>

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
                />
                <LimitInput
                  id="plan-device-limit"
                  label="设备数限制"
                  placeholder="留空则不限制"
                  value={submit.device_limit}
                  onChange={(value) => change('device_limit', value)}
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <Label htmlFor="plan-group">权限组</Label>
                  <Select
                    value={submit.group_id != null ? String(submit.group_id) : undefined}
                    onValueChange={(value) => change('group_id', Number(value))}
                  >
                    <SelectTrigger id="plan-group" className="w-full" data-testid="plan-group">
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
                </div>
                <div className="space-y-2">
                  <Label htmlFor="plan-reset-method">流量重置方式</Label>
                  <Select
                    value={resetValue}
                    onValueChange={(value) =>
                      change('reset_traffic_method', value === 'null' ? null : Number(value))
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
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <LimitInput
                  id="plan-capacity-limit"
                  label="最大容纳用户量"
                  placeholder="留空则不限制"
                  value={submit.capacity_limit}
                  onChange={(value) => change('capacity_limit', value)}
                />
                <LimitInput
                  id="plan-speed-limit"
                  label="限速"
                  suffix="Mbps"
                  placeholder="留空则不限制"
                  value={submit.speed_limit}
                  onChange={(value) => change('speed_limit', value)}
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
            </div>
          </TooltipProvider>

          <SheetFooter>
            <Button onClick={() => void save()} disabled={pending} data-testid="plan-submit">
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

export default function PlansPage() {
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const config = useConfig('site');
  const save = useSavePlanMutation();
  const drop = useDropPlanMutation();
  const update = useUpdatePlanMutation();
  const sort = useSortPlansMutation();
  const [order, setOrder] = useState<Plan[]>(() => plans.data ?? []);
  const [sortLoading, setSortLoading] = useState(false);
  const orderRef = useRef(order);
  orderRef.current = order;

  useEffect(() => {
    if (plans.data) setOrder(plans.data);
  }, [plans.data]);

  const currencySymbol = config.data?.site?.currency_symbol;

  const savePlan = async (payload: SavePlanPayload) => {
    await save.mutateAsync(payload);
    await plans.refetch();
  };

  // Adjacent swap reorder. The persisted contract is unchanged: sort.mutate gets
  // the full id list in the new order, then the page refetches.
  const movePlan = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const list = orderRef.current;
    if (target < 0 || target >= list.length) return;
    const next = [...list];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrder(next);
    setSortLoading(true);
    sort.mutate(
      next.map((plan) => plan.id),
      {
        onSuccess: () => {
          void plans.refetch().finally(() => {
            setSortLoading(false);
          });
        },
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
    drop.mutate(record.id, {
      onSuccess: () => {
        void plans.refetch();
      },
    });
  };

  const updatePlan = (id: number, key: 'show' | 'renew', value: 0 | 1) => {
    update.mutate(
      { id, key, value },
      {
        onSuccess: () => {
          void plans.refetch();
        },
      },
    );
  };

  const groupName = (id: number) =>
    (groups.data ?? []).find((group) => group.id === parseInt(String(id), 10))?.name;

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
          checked={Boolean(parseInt(String(row.original.show), 10))}
          onCheckedChange={() =>
            updatePlan(
              row.original.id,
              'show',
              parseInt(String(row.original.show), 10) ? 0 : 1,
            )
          }
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
          checked={Boolean(parseInt(String(row.original.renew), 10))}
          onCheckedChange={() =>
            updatePlan(
              row.original.id,
              'renew',
              parseInt(String(row.original.renew), 10) ? 0 : 1,
            )
          }
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
          <PlanEditor
            record={row.original}
            groups={groups.data ?? []}
            currencySymbol={currencySymbol}
            pending={save.isPending}
            onSave={savePlan}
          >
            <Button variant="ghost" size="sm" data-testid={`plan-edit-${row.original.id}`}>
              <Pencil className="size-4" />
              编辑
            </Button>
          </PlanEditor>
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
      <PageHeader
        title="订阅管理"
        actions={
          <PlanEditor
            groups={groups.data ?? []}
            currencySymbol={currencySymbol}
            pending={save.isPending}
            onSave={savePlan}
          >
            <Button data-testid="plan-create">
              <Plus className="size-4" />
              添加订阅
            </Button>
          </PlanEditor>
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
              empty={order.length === 0 ? '暂无订阅' : undefined}
              emptyTestId="plans-empty"
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {sortLoading || plans.isPending ? (
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

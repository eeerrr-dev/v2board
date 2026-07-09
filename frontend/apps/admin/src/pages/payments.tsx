import { cloneElement, useEffect, useRef, useState, type ReactElement } from 'react';
import { admin } from '@v2board/api-client';
import type { AdminPayment, PaymentFormDefinition } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { apiClient } from '@/lib/api';
import {
  useAdminPayments,
  useDropPaymentMutation,
  useSavePaymentMutation,
  useShowPaymentMutation,
  useSortPaymentMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
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
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';

type SavePaymentPayload = Parameters<typeof admin.savePayment>[1];

function PaymentEditor({
  record,
  fetchLoading,
  children,
  onSave,
  onSaved,
}: {
  record?: AdminPayment;
  fetchLoading: boolean;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SavePaymentPayload) => Promise<unknown>;
  onSaved: () => void;
}) {
  const [submit, setSubmit] = useState<Record<string, unknown>>(() => ({ ...(record ?? {}) }));
  const [open, setOpen] = useState(false);
  const [paymentMethods, setPaymentMethods] = useState<string[]>([]);
  const [selectPaymentMethod, setSelectPaymentMethod] = useState<string | undefined>(undefined);
  const [form, setForm] = useState<PaymentFormDefinition>({});
  const [config, setConfig] = useState<Record<string, unknown>>(() => ({
    ...(record?.config ?? {}),
  }));

  const submitOnChange = (key: string, value: unknown) => {
    setSubmit((current) => ({ ...current, [key]: value }));
  };

  const configOnChange = (key: string, value: unknown) => {
    setConfig((current) => ({ ...current, [key]: value }));
  };

  const onSelectPaymentMethod = async (payment: string | undefined) => {
    const nextForm = await admin.paymentForm(apiClient, payment, record?.id);
    setForm(nextForm);
    setSelectPaymentMethod(payment);
  };

  const show = async () => {
    const methods = await admin.paymentMethods(apiClient);
    const selected = record?.payment || methods[0];
    setPaymentMethods(methods);
    setSelectPaymentMethod(selected);
    setOpen(true);
    await onSelectPaymentMethod(selected);
  };

  const save = async () => {
    await onSave({
      ...submit,
      payment: selectPaymentMethod,
      config,
    } as SavePaymentPayload);
    setOpen(false);
    onSaved();
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="right"
          className="w-full gap-0 overflow-y-auto sm:max-w-md"
          data-testid="payment-editor"
        >
          <SheetHeader>
            <SheetTitle>{submit.id ? '编辑支付方式' : '添加支付方式'}</SheetTitle>
          </SheetHeader>

          <div className="space-y-4 px-4 pb-4">
            <div className="space-y-2">
              <Label htmlFor="payment-name">显示名称</Label>
              <Input
                id="payment-name"
                placeholder="用于前端显示使用"
                defaultValue={submit.name as string | undefined}
                onChange={(event) => submitOnChange('name', event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="payment-icon">图标URL(选填)</Label>
              <Input
                id="payment-icon"
                placeholder="用于前端显示使用(https://x.com/icon.svg)"
                defaultValue={submit.icon as string | undefined}
                onChange={(event) => submitOnChange('icon', event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="payment-notify">自定义通知域名(选填)</Label>
              <Input
                id="payment-notify"
                placeholder="网关的通知将会发送到该域名(https://x.com)"
                defaultValue={submit.notify_domain as string | undefined}
                onChange={(event) => submitOnChange('notify_domain', event.target.value)}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label htmlFor="payment-fee-percent">百分比手续费(选填)</Label>
                <div className="relative">
                  <Input
                    id="payment-fee-percent"
                    type="number"
                    className="pr-8"
                    placeholder="在订单金额基础上附加手续费"
                    defaultValue={submit.handling_fee_percent as string | number | undefined}
                    onChange={(event) => submitOnChange('handling_fee_percent', event.target.value)}
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                    %
                  </span>
                </div>
              </div>
              <div className="space-y-2">
                <Label htmlFor="payment-fee-fixed">固定手续费(选填)</Label>
                <Input
                  id="payment-fee-fixed"
                  type="number"
                  placeholder="在订单金额基础上附加手续费"
                  defaultValue={
                    submit.handling_fee_fixed != null
                      ? (submit.handling_fee_fixed as number) / 100
                      : undefined
                  }
                  onChange={(event) =>
                    submitOnChange('handling_fee_fixed', 100 * (event.target.value as unknown as number))
                  }
                />
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="payment-method">接口文件</Label>
              <Select
                value={selectPaymentMethod}
                onValueChange={(value) => {
                  void onSelectPaymentMethod(value);
                }}
              >
                <SelectTrigger id="payment-method" className="w-full">
                  <SelectValue placeholder="选择支付接口" />
                </SelectTrigger>
                <SelectContent>
                  {paymentMethods.map((method) => (
                    <SelectItem key={method} value={method}>
                      {method}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {Object.keys(form).map((key) => {
              const field = form[key] as PaymentFormDefinition[string];
              const inputType = field.type;
              const showInput =
                inputType === 'input' ||
                inputType === 'text' ||
                inputType === 'string' ||
                !inputType;

              return (
                <div className="space-y-2" key={key}>
                  <Label htmlFor={`payment-config-${key}`}>{field.label}</Label>
                  {showInput ? (
                    <Input
                      id={`payment-config-${key}`}
                      placeholder={field.description}
                      defaultValue={(config[key] || field.value) as string | undefined}
                      onChange={(event) => configOnChange(key, event.target.value)}
                    />
                  ) : null}
                </div>
              );
            })}

            {selectPaymentMethod === 'MGate' ? (
              <Alert className="border-warning/30 bg-warning/10 text-warning">
                <AlertDescription className="text-warning">MGate TG@nulledsan</AlertDescription>
              </Alert>
            ) : null}
          </div>

          <SheetFooter>
            <Button
              onClick={() => void save()}
              disabled={fetchLoading}
              data-testid="payment-save"
            >
              {fetchLoading ? <Loader2 className="size-4 animate-spin" /> : null}
              {submit.id ? '保存' : '添加'}
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

export default function PaymentsPage() {
  const payments = useAdminPayments();
  const save = useSavePaymentMutation();
  const show = useShowPaymentMutation();
  const drop = useDropPaymentMutation();
  const sort = useSortPaymentMutation();
  const [orderedPayments, setOrderedPayments] = useState<AdminPayment[]>(() => payments.data ?? []);
  const [sortLoading, setSortLoading] = useState(false);
  const orderRef = useRef(orderedPayments);

  useEffect(() => {
    if (payments.data) setOrderedPayments(payments.data);
  }, [payments.data]);

  orderRef.current = orderedPayments;

  // Adjacent swap reorder. The drag handle is retired for accessible move
  // buttons, but the persisted contract is unchanged: sort.mutate receives the
  // full id list in the new order, then the page refetches.
  const movePayment = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const list = orderRef.current;
    if (target < 0 || target >= list.length) return;
    const next = [...list];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrderedPayments(next);
    setSortLoading(true);
    sort.mutate(
      next.map((payment) => payment.id),
      {
        onSuccess: () => {
          void payments.refetch().finally(() => {
            setSortLoading(false);
          });
        },
      },
    );
  };

  const removePayment = async (row: AdminPayment) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
    });
    if (!confirmed) return;
    await drop.mutateAsync(row.id);
    void payments.refetch();
  };

  const columns: DataTableColumn<AdminPayment>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'enable',
      meta: { align: 'center' },
      header: () => <span>启用</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(parseInt(String(row.original.enable), 10))}
          onCheckedChange={() =>
            show.mutate(row.original.id, {
              onSuccess: () => {
                void payments.refetch();
              },
            })
          }
          aria-label={`切换「${row.original.name}」启用`}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>显示名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'payment',
      header: () => <span>支付接口</span>,
      cell: ({ row }) => row.original.payment,
    },
    {
      id: 'notify_url',
      meta: { className: 'max-w-[24rem] truncate text-muted-foreground' },
      header: () => (
        <HeaderTooltip title="支付网关将会把数据通知到本地址，请通过防火墙放行本地址。">
          通知地址
        </HeaderTooltip>
      ),
      cell: ({ row }) => row.original.notify_url,
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => {
        const index = orderedPayments.findIndex((item) => item.id === row.original.id);
        return (
          <div className="flex items-center justify-end gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index <= 0}
              onClick={() => movePayment(index, -1)}
              aria-label="上移"
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedPayments.length - 1}
              onClick={() => movePayment(index, 1)}
              aria-label="下移"
            >
              <ArrowDown className="size-4" />
            </Button>
            <PaymentEditor
              record={row.original}
              fetchLoading={payments.isFetching}
              onSave={(payload) => save.mutateAsync(payload)}
              onSaved={() => {
                void payments.refetch();
              }}
            >
              <Button variant="ghost" size="sm" data-testid={`payment-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </PaymentEditor>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => void removePayment(row.original)}
              data-testid={`payment-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              删除
            </Button>
          </div>
        );
      },
    },
  ];

  return (
    <PageShell data-testid="payments-page">
      <PageHeader
        title="支付配置"
        actions={
          <PaymentEditor
            fetchLoading={payments.isFetching}
            onSave={(payload) => save.mutateAsync(payload)}
            onSaved={() => {
              void payments.refetch();
            }}
          >
            <Button data-testid="payment-create">
              <Plus className="size-4" />
              添加支付方式
            </Button>
          </PaymentEditor>
        }
      />

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <DataTable
              columns={columns}
              data={orderedPayments}
              getRowKey={(row) => row.id}
              className="min-w-[900px]"
              data-testid="payments-table"
              empty={orderedPayments.length === 0 ? '暂无支付方式' : undefined}
              emptyTestId="payments-empty"
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {sortLoading || payments.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

import { useCallback, useEffect, useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import type { admin } from '@v2board/api-client';
import type { AdminPayment, PaymentFormDefinition } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import {
  useAdminPayments,
  useDropPaymentMutation,
  usePaymentForm,
  usePaymentMethods,
  useSavePaymentMutation,
  useShowPaymentMutation,
  useSortPaymentMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
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
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { TooltipProvider } from '@/components/ui/tooltip';
import { paymentFormSchema, type PaymentEditorValues } from './payment-form-schema';

type SavePaymentPayload = Parameters<typeof admin.savePayment>[1];

function paymentEditorValues(record?: AdminPayment): PaymentEditorValues {
  if (!record) {
    return {
      name: '',
      icon: '',
      notify_domain: '',
      handling_fee_percent: '',
      handling_fee_fixed: '',
      payment: '',
      config: {},
    };
  }
  return {
    id: record.id,
    name: record.name,
    icon: record.icon ?? '',
    notify_domain: record.notify_domain ?? '',
    // The backend models “no percentage fee” as nullable and rejects 0 on save
    // (`between:0.1,100`), while older rows may still expose a persisted zero.
    handling_fee_percent: record.handling_fee_percent || '',
    handling_fee_fixed:
      record.handling_fee_fixed == null ? '' : String(Number(record.handling_fee_fixed) / 100),
    payment: record.payment,
    config: record.config,
  };
}

function configForDefinition(
  definition: PaymentFormDefinition,
  existing: Record<string, string>,
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(definition).map(([key, field]) => [key, existing[key] ?? field.value ?? '']),
  );
}

function PaymentEditor({
  record,
  open,
  returnFocus,
  pending,
  onOpenChange,
  onSave,
}: {
  record?: AdminPayment;
  open: boolean;
  returnFocus: HTMLButtonElement;
  pending: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (payload: SavePaymentPayload, onSuccess: () => void) => void;
}) {
  const form = useForm<PaymentEditorValues>({
    resolver: zodResolver(paymentFormSchema),
    defaultValues: paymentEditorValues(record),
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const selectedPaymentMethod = useWatch({ control: form.control, name: 'payment' });
  const paymentMethodsQuery = usePaymentMethods(open);
  const definitionQuery = usePaymentForm(
    selectedPaymentMethod,
    record?.id,
    open && Boolean(selectedPaymentMethod),
  );
  const paymentMethods = paymentMethodsQuery.data ?? [];
  const definition = definitionQuery.data;
  const methodsLoading = open && (paymentMethodsQuery.isPending || paymentMethodsQuery.isFetching);
  const methodsError = open && paymentMethodsQuery.isError;
  const methodsEmpty =
    open &&
    !methodsLoading &&
    !methodsError &&
    paymentMethodsQuery.data !== undefined &&
    paymentMethods.length === 0;
  const definitionLoading =
    open &&
    Boolean(selectedPaymentMethod) &&
    (definitionQuery.isPending || definitionQuery.isFetching);
  const definitionError = open && Boolean(selectedPaymentMethod) && definitionQuery.isError;
  const definitionEmpty =
    open &&
    Boolean(selectedPaymentMethod) &&
    !definitionLoading &&
    !definitionError &&
    definition !== undefined &&
    Object.keys(definition).length === 0;
  const definitionReady =
    open &&
    Boolean(selectedPaymentMethod) &&
    !definitionLoading &&
    !definitionError &&
    definition !== undefined &&
    Object.keys(definition).length > 0;
  const editorReady = open && !methodsLoading && !methodsError && !methodsEmpty && definitionReady;

  const onSelectPaymentMethod = useCallback(
    (payment: string | undefined) => {
      if (!payment) return;
      const previousPayment = form.getValues('payment');
      const previousConfig = form.getValues('config');
      form.setValue('payment', payment, { shouldDirty: true, shouldValidate: true });
      // Clear every key owned by the previous driver immediately. The keyed
      // payment-form query will hydrate only the selected driver's definition;
      // a late response for another key is never observed by this editor.
      const existingConfig =
        previousPayment === payment
          ? previousConfig
          : record?.payment === payment
            ? record.config
            : {};
      form.setValue('config', existingConfig, {
        shouldDirty: previousPayment !== payment,
        shouldValidate: true,
      });
    },
    [form, record],
  );

  useEffect(() => {
    const firstMethod = paymentMethodsQuery.data?.[0];
    if (!open || methodsLoading || methodsError || selectedPaymentMethod || !firstMethod) return;
    onSelectPaymentMethod(firstMethod);
  }, [
    methodsError,
    methodsLoading,
    onSelectPaymentMethod,
    open,
    paymentMethodsQuery.data,
    selectedPaymentMethod,
  ]);

  const save = form.handleSubmit((values) => {
    if (!editorReady || !definition) return;
    onSave(
      {
        ...values,
        config: configForDefinition(definition, values.config),
      },
      () => onOpenChange(false),
    );
  });

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-md"
        data-testid="payment-editor"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          if (returnFocus.isConnected) returnFocus.focus();
        }}
      >
        <SheetHeader>
          <SheetTitle>{record?.id ? '编辑支付方式' : '添加支付方式'}</SheetTitle>
          <SheetDescription>配置支付驱动、显示信息、手续费和网关参数。</SheetDescription>
        </SheetHeader>

        <form id="payment-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="payment-name">显示名称</FieldLabel>
            <Input
              id="payment-name"
              placeholder="用于前端显示使用"
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
            />
            <FieldError errors={[formErrors.name]} />
          </Field>
          <Field>
            <FieldLabel htmlFor="payment-icon">图标URL(选填)</FieldLabel>
            <Input
              id="payment-icon"
              placeholder="用于前端显示使用(https://x.com/icon.svg)"
              {...form.register('icon')}
            />
          </Field>
          <Field data-invalid={Boolean(formErrors.notify_domain)}>
            <FieldLabel htmlFor="payment-notify">自定义通知域名(选填)</FieldLabel>
            <Input
              id="payment-notify"
              placeholder="网关的通知将会发送到该域名(https://x.com)"
              aria-invalid={Boolean(formErrors.notify_domain)}
              {...form.register('notify_domain')}
            />
            <FieldError errors={[formErrors.notify_domain]} />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.handling_fee_percent)}>
              <FieldLabel htmlFor="payment-fee-percent">百分比手续费(选填)</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="payment-fee-percent"
                  type="number"
                  min="0.1"
                  max="100"
                  step="0.1"
                  placeholder="在订单金额基础上附加手续费"
                  aria-invalid={Boolean(formErrors.handling_fee_percent)}
                  {...form.register('handling_fee_percent')}
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>%</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
              <FieldError errors={[formErrors.handling_fee_percent]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.handling_fee_fixed)}>
              <FieldLabel htmlFor="payment-fee-fixed">固定手续费(选填)</FieldLabel>
              <Input
                id="payment-fee-fixed"
                type="number"
                step="0.01"
                placeholder="在订单金额基础上附加手续费"
                aria-invalid={Boolean(formErrors.handling_fee_fixed)}
                {...form.register('handling_fee_fixed')}
              />
              <FieldError errors={[formErrors.handling_fee_fixed]} />
            </Field>
          </div>
          <Field data-invalid={Boolean(formErrors.payment)}>
            <FieldLabel htmlFor="payment-method">接口文件</FieldLabel>
            <Select
              value={selectedPaymentMethod ?? ''}
              disabled={methodsLoading || methodsError || methodsEmpty}
              onValueChange={onSelectPaymentMethod}
            >
              <SelectTrigger
                id="payment-method"
                className="w-full"
                aria-invalid={Boolean(formErrors.payment)}
              >
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
            <FieldError errors={[formErrors.payment]} />
          </Field>

          {methodsLoading ? (
            <div
              className="flex min-h-20 items-center justify-center gap-2 text-sm text-muted-foreground"
              role="status"
              data-testid="payment-methods-loading"
            >
              <Spinner className="size-4" />
              正在加载支付接口
            </div>
          ) : null}
          {methodsError ? (
            <ErrorState
              data-testid="payment-methods-error"
              message="支付接口列表加载失败"
              onRetry={() => void paymentMethodsQuery.refetch()}
            />
          ) : null}
          {methodsEmpty ? (
            <ErrorState
              data-testid="payment-methods-empty"
              message="暂无可用支付接口"
              onRetry={() => void paymentMethodsQuery.refetch()}
            />
          ) : null}

          {definitionLoading ? (
            <div
              className="flex min-h-20 items-center justify-center gap-2 text-sm text-muted-foreground"
              role="status"
              data-testid="payment-definition-loading"
            >
              <Spinner className="size-4" />
              正在加载接口配置
            </div>
          ) : null}
          {definitionError ? (
            <ErrorState
              data-testid="payment-definition-error"
              message="支付接口配置加载失败"
              onRetry={() => void definitionQuery.refetch()}
            />
          ) : null}
          {definitionEmpty ? (
            <ErrorState
              data-testid="payment-definition-empty"
              message="该支付接口未提供配置字段"
              onRetry={() => void definitionQuery.refetch()}
            />
          ) : null}

          {definitionReady && definition
            ? Object.entries(definition).map(([key, definitionField]) => {
                const inputType = definitionField.type;
                const showInput =
                  inputType === 'input' ||
                  inputType === 'text' ||
                  inputType === 'string' ||
                  !inputType;

                return (
                  <Field key={`${selectedPaymentMethod}:${key}`}>
                    <FieldLabel htmlFor={`payment-config-${key}`}>
                      {definitionField.label}
                    </FieldLabel>
                    {showInput ? (
                      <Controller
                        control={form.control}
                        name={`config.${key}`}
                        defaultValue={form.getValues('config')[key] ?? definitionField.value ?? ''}
                        render={({ field }) => (
                          <Input
                            id={`payment-config-${key}`}
                            placeholder={definitionField.description}
                            {...field}
                          />
                        )}
                      />
                    ) : null}
                  </Field>
                );
              })
            : null}
          <FieldError errors={[formErrors.config]} />

          {selectedPaymentMethod === 'MGate' ? (
            <Alert className="border-warning/30 bg-warning/10 text-warning">
              <AlertDescription className="text-warning">MGate TG@nulledsan</AlertDescription>
            </Alert>
          ) : null}
        </form>

        <SheetFooter>
          <Button
            type="submit"
            form="payment-editor-form"
            disabled={pending || !editorReady}
            data-testid="payment-save"
          >
            {pending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            {record?.id ? '保存' : '添加'}
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

export default function PaymentsPage() {
  const payments = useAdminPayments();
  const save = useSavePaymentMutation();
  const show = useShowPaymentMutation();
  const drop = useDropPaymentMutation();
  const sort = useSortPaymentMutation();
  const [orderOverride, setOrderOverride] = useState<AdminPayment[] | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editor, setEditor] = useState<{
    session: number;
    record?: AdminPayment;
    returnFocus: HTMLButtonElement;
  } | null>(null);
  const orderedPayments = orderOverride ?? payments.data ?? [];

  const openPaymentEditor = (record: AdminPayment | undefined, returnFocus: HTMLButtonElement) => {
    setEditor((current) => ({
      session: (current?.session ?? 0) + 1,
      record,
      returnFocus,
    }));
    setEditorOpen(true);
  };

  // Adjacent swap reorder. The drag handle is retired for accessible move
  // buttons, but the persisted contract is unchanged: sort.mutate receives the
  // full id list in the new order, then the page refetches.
  const movePayment = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const list = orderedPayments;
    if (target < 0 || target >= list.length) return;
    const next = [...list];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrderOverride(next);
    sort.mutate(
      next.map((payment) => payment.id),
      {
        onSettled: () => setOrderOverride(null),
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
    drop.mutate(row.id);
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
          onCheckedChange={() => show.mutate(row.original.id)}
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
            <Button
              variant="ghost"
              size="sm"
              data-testid={`payment-edit-${row.original.id}`}
              onClick={(event) => openPaymentEditor(row.original, event.currentTarget)}
              aria-haspopup="dialog"
              aria-expanded={editorOpen && editor?.record?.id === row.original.id}
            >
              <Pencil className="size-4" />
              编辑
            </Button>
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
      {payments.isError ? (
        <ErrorState message="支付配置加载失败" onRetry={() => void payments.refetch()} />
      ) : null}
      <PageHeader
        title="支付配置"
        actions={
          <Button
            data-testid="payment-create"
            onClick={(event) => openPaymentEditor(undefined, event.currentTarget)}
            aria-haspopup="dialog"
            aria-expanded={editorOpen && editor?.record === undefined}
          >
            <Plus className="size-4" />
            添加支付方式
          </Button>
        }
      />

      {editor ? (
        <PaymentEditor
          key={editor.session}
          record={editor.record}
          open={editorOpen}
          returnFocus={editor.returnFocus}
          pending={save.isPending}
          onOpenChange={setEditorOpen}
          onSave={(payload, onSuccess) => save.mutate(payload, { onSuccess })}
        />
      ) : null}

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <DataTable
              columns={columns}
              data={orderedPayments}
              getRowKey={(row) => row.id}
              className="min-w-[900px]"
              data-testid="payments-table"
              empty={
                !payments.isError && payments.data !== undefined && orderedPayments.length === 0
                  ? '暂无支付方式'
                  : undefined
              }
              emptyTestId="payments-empty"
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {sort.isPending || payments.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

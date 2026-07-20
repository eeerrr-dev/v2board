import { useCallback, useEffect, useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import type { admin } from '@v2board/api-client';
import type { AdminPayment, PaymentFormDefinition } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import {
  useAdminPayments,
  useDropPaymentMutation,
  usePaymentForm,
  usePaymentMethods,
  useSavePaymentMutation,
  useShowPaymentMutation,
  useSortPaymentMutation,
} from '@/lib/queries';
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import { HeaderTooltip } from '@v2board/ui/header-tooltip';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@v2board/ui/sheet';
import { LoadingState, SkeletonFields, SkeletonRows } from '@v2board/ui/loading-state';
import { Switch } from '@v2board/ui/switch';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import { TooltipProvider } from '@v2board/ui/tooltip';
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
  const { t } = useTranslation();
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

  // Deliberate useCallback: the payment-form hydration effect below keys on
  // this identity; a per-render function would replay the hydration.
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
          <SheetTitle>
            {record?.id
              ? t(($) => $.admin.payments.edit_title)
              : t(($) => $.admin.payments.add_title)}
          </SheetTitle>
          <SheetDescription>{t(($) => $.admin.payments.editor_description)}</SheetDescription>
        </SheetHeader>

        <form id="payment-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="payment-name">{t(($) => $.admin.payments.name_label)}</FieldLabel>
            <Input
              id="payment-name"
              placeholder={t(($) => $.admin.payments.name_placeholder)}
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
            />
            <FieldError errors={[formErrors.name]} />
          </Field>
          <Field>
            <FieldLabel htmlFor="payment-icon">{t(($) => $.admin.payments.icon_label)}</FieldLabel>
            <Input
              id="payment-icon"
              placeholder={t(($) => $.admin.payments.icon_placeholder)}
              {...form.register('icon')}
            />
          </Field>
          <Field data-invalid={Boolean(formErrors.notify_domain)}>
            <FieldLabel htmlFor="payment-notify">
              {t(($) => $.admin.payments.notify_domain_label)}
            </FieldLabel>
            <Input
              id="payment-notify"
              placeholder={t(($) => $.admin.payments.notify_domain_placeholder)}
              aria-invalid={Boolean(formErrors.notify_domain)}
              {...form.register('notify_domain')}
            />
            <FieldError errors={[formErrors.notify_domain]} />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={Boolean(formErrors.handling_fee_percent)}>
              <FieldLabel htmlFor="payment-fee-percent">
                {t(($) => $.admin.payments.fee_percent_label)}
              </FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="payment-fee-percent"
                  type="number"
                  min="0.1"
                  max="100"
                  step="0.1"
                  placeholder={t(($) => $.admin.payments.fee_placeholder)}
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
              <FieldLabel htmlFor="payment-fee-fixed">
                {t(($) => $.admin.payments.fee_fixed_label)}
              </FieldLabel>
              <Input
                id="payment-fee-fixed"
                type="number"
                step="0.01"
                placeholder={t(($) => $.admin.payments.fee_placeholder)}
                aria-invalid={Boolean(formErrors.handling_fee_fixed)}
                {...form.register('handling_fee_fixed')}
              />
              <FieldError errors={[formErrors.handling_fee_fixed]} />
            </Field>
          </div>
          <Field data-invalid={Boolean(formErrors.payment)}>
            <FieldLabel htmlFor="payment-method">
              {t(($) => $.admin.payments.method_label)}
            </FieldLabel>
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
                <SelectValue placeholder={t(($) => $.admin.payments.method_placeholder)} />
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
            <LoadingState
              className="min-h-20 py-2"
              label={t(($) => $.admin.payments.methods_loading)}
              data-testid="payment-methods-loading"
            >
              <SkeletonRows rows={2} />
            </LoadingState>
          ) : null}
          {methodsError ? (
            <ErrorState
              data-testid="payment-methods-error"
              message={t(($) => $.admin.payments.methods_load_failed)}
              onRetry={() => void paymentMethodsQuery.refetch()}
            />
          ) : null}
          {methodsEmpty ? (
            <ErrorState
              data-testid="payment-methods-empty"
              message={t(($) => $.admin.payments.methods_empty)}
              onRetry={() => void paymentMethodsQuery.refetch()}
            />
          ) : null}

          {definitionLoading ? (
            <LoadingState
              className="min-h-20 py-2"
              label={t(($) => $.admin.payments.definition_loading)}
              data-testid="payment-definition-loading"
            >
              <SkeletonFields fields={2} />
            </LoadingState>
          ) : null}
          {definitionError ? (
            <ErrorState
              data-testid="payment-definition-error"
              message={t(($) => $.admin.payments.definition_load_failed)}
              onRetry={() => void definitionQuery.refetch()}
            />
          ) : null}
          {definitionEmpty ? (
            <ErrorState
              data-testid="payment-definition-empty"
              message={t(($) => $.admin.payments.definition_empty)}
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
            {record?.id ? t(($) => $.common.save) : t(($) => $.admin.payments.add)}
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t(($) => $.common.cancel)}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

export default function PaymentsPage() {
  const { t } = useTranslation();
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
      title: t(($) => $.admin.payments.delete_confirm_title),
      description: t(($) => $.admin.payments.delete_confirm_description),
      confirmText: t(($) => $.common.confirm),
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
      header: () => <span>{t(($) => $.common.enable)}</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.enable}
          onCheckedChange={() => show.mutate({ id: row.original.id, enable: !row.original.enable })}
          aria-label={t(($) => $.admin.payments.toggle_enable, { name: row.original.name })}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.payments.name_label)}</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'payment',
      header: () => <span>{t(($) => $.admin.payments.payment_col)}</span>,
      cell: ({ row }) => row.original.payment,
    },
    {
      id: 'notify_url',
      meta: { className: 'max-w-[24rem] truncate text-muted-foreground' },
      header: () => (
        <HeaderTooltip title={t(($) => $.admin.payments.notify_url_tooltip)}>
          {t(($) => $.admin.payments.notify_url_col)}
        </HeaderTooltip>
      ),
      cell: ({ row }) => row.original.notify_url,
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
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
              aria-label={t(($) => $.admin.payments.move_up)}
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedPayments.length - 1}
              onClick={() => movePayment(index, 1)}
              aria-label={t(($) => $.admin.payments.move_down)}
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
              {t(($) => $.common.edit)}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => void removePayment(row.original)}
              data-testid={`payment-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              {t(($) => $.common.delete)}
            </Button>
          </div>
        );
      },
    },
  ];

  return (
    <PageShell data-testid="payments-page">
      {payments.isError ? (
        <ErrorState
          message={t(($) => $.admin.payments.list_load_failed)}
          onRetry={() => void payments.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.payments.title)}
        actions={
          <Button
            data-testid="payment-create"
            onClick={(event) => openPaymentEditor(undefined, event.currentTarget)}
            aria-haspopup="dialog"
            aria-expanded={editorOpen && editor?.record === undefined}
          >
            <Plus className="size-4" />
            {t(($) => $.admin.payments.add_title)}
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
                  ? t(($) => $.admin.payments.empty)
                  : undefined
              }
              emptyTestId="payments-empty"
            />
          </CardContent>
        </Card>
      </TooltipProvider>

      {sort.isPending || payments.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}

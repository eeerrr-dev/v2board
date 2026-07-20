import { useState, type ReactElement } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { Loader2 } from 'lucide-react';
import type { AdminPlanModel } from '@v2board/types';
import { Button } from '@v2board/ui/button';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@v2board/ui/sheet';
import { giftcardEditorSchema, type GiftcardEditorValues } from './form-schema';
import {
  downloadGeneratedCsv,
  fromDateTimeLocal,
  normalizeGenerationPayload,
  planOptions,
  rfc3339ToUnixInput,
  toDateTimeLocal,
  type GenerateResponse,
  type GiftcardRow,
  type GiftcardSubmit,
} from './shared';

function giftcardValueUnit(t: TFunction, type: GiftcardSubmit['type']) {
  switch (type) {
    case 1:
      return '¥';
    case 2:
      return t(($) => $.admin.coupons.giftcards.unit_days);
    case 3:
      return 'GB';
    case 4:
      return '';
    case 5:
      return t(($) => $.admin.coupons.giftcards.unit_days);
    default:
      return '';
  }
}

export function GiftcardEditor({
  record,
  plans,
  pending,
  onSave,
  children,
}: {
  record?: GiftcardRow;
  plans: AdminPlanModel[];
  pending: boolean;
  onSave: (payload: GiftcardSubmit, onSuccess: (response?: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const { t } = useTranslation();
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
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
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
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
      limit_use: record?.limit_use ?? null,
      generate_count: undefined,
    });
    setOpen(true);
  };

  const save = form.handleSubmit((validValues) => {
    onSave(normalizeGenerationPayload(validValues) as GiftcardSubmit, (response) => {
      if (validValues.generate_count && response?.buffer)
        downloadGeneratedCsv('GIFTCARD', response.buffer);
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
          <SheetTitle>
            {record?.id
              ? t(($) => $.admin.coupons.giftcards.edit_title)
              : t(($) => $.admin.coupons.giftcards.create_title)}
          </SheetTitle>
          <SheetDescription>
            {t(($) => $.admin.coupons.giftcards.editor_description)}
          </SheetDescription>
        </SheetHeader>

        <form id="giftcard-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="giftcard-name">{t(($) => $.admin.coupons.name)}</FieldLabel>
            <Input
              id="giftcard-name"
              placeholder={t(($) => $.admin.coupons.giftcards.name_placeholder)}
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="giftcard-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="giftcard-code">
                {t(($) => $.admin.coupons.giftcards.custom_code)}
              </FieldLabel>
              <Input
                id="giftcard-code"
                placeholder={t(($) => $.admin.coupons.giftcards.custom_code_placeholder)}
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="giftcard-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="giftcard-value">
              {t(($) => $.admin.coupons.giftcards.type_label)}
            </FieldLabel>
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
                  aria-label={t(($) => $.admin.coupons.giftcards.type_label)}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">
                    {t(($) => $.admin.coupons.giftcards.type_balance_option)}
                  </SelectItem>
                  <SelectItem value="2">
                    {t(($) => $.admin.coupons.giftcards.type_duration_option)}
                  </SelectItem>
                  <SelectItem value="3">
                    {t(($) => $.admin.coupons.giftcards.type_traffic_option)}
                  </SelectItem>
                  <SelectItem value="4">
                    {t(($) => $.admin.coupons.giftcards.type_reset_option)}
                  </SelectItem>
                  <SelectItem value="5">
                    {t(($) => $.admin.coupons.giftcards.type_plan_option)}
                  </SelectItem>
                </SelectContent>
              </Select>
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="giftcard-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  disabled={values.type === 4}
                  placeholder={
                    values.type === 5
                      ? t(($) => $.admin.coupons.giftcards.value_placeholder_onetime)
                      : t(($) => $.admin.coupons.value_placeholder)
                  }
                  value={values.type === 4 ? 0 : (values.value ?? '')}
                  onChange={(event) => form.setValue('value', event.target.value)}
                  aria-invalid={Boolean(formErrors.value)}
                  data-testid="giftcard-value"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{giftcardValueUnit(t, values.type)}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </div>
            <FieldError errors={[formErrors.value]} />
          </Field>

          {values.type === 5 ? (
            <Field data-invalid={Boolean(formErrors.plan_id)}>
              <FieldLabel htmlFor="giftcard-plan">
                {t(($) => $.admin.coupons.giftcards.plan_label)}
              </FieldLabel>
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
                  <SelectValue placeholder={t(($) => $.admin.coupons.giftcards.plan_label)} />
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
              <FieldLabel htmlFor="giftcard-start">
                {t(($) => $.admin.coupons.started_at)}
              </FieldLabel>
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
              <FieldLabel htmlFor="giftcard-end">{t(($) => $.admin.coupons.ended_at)}</FieldLabel>
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
            <FieldLabel htmlFor="giftcard-limit-use">
              {t(($) => $.admin.coupons.limit_use)}
            </FieldLabel>
            <Input
              id="giftcard-limit-use"
              type="number"
              step="1"
              placeholder={t(($) => $.admin.coupons.limit_use_placeholder)}
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="giftcard-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="giftcard-generate-count">
                {t(($) => $.admin.coupons.generate_count)}
              </FieldLabel>
              <Input
                id="giftcard-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder={t(($) => $.admin.coupons.generate_count_placeholder)}
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
            {t(($) => $.common.submit)}
          </Button>
          <Button variant="outline" onClick={() => setOpen(false)}>
            {t(($) => $.common.cancel)}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

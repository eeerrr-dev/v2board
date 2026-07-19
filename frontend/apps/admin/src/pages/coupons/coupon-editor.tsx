import { useState, type ReactElement } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { Loader2 } from 'lucide-react';
import type { CouponType, Plan } from '@v2board/types';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from '@/components/ui/input-group';
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
import { couponEditorSchema, type CouponEditorValues } from './form-schema';
import {
  downloadGeneratedCsv,
  fromDateTimeLocal,
  normalizeGenerationPayload,
  planOptions,
  rfc3339ToUnixInput,
  toDateTimeLocal,
  type CouponRow,
  type CouponSubmit,
  type GenerateResponse,
} from './shared';

// Wire period identifiers -> label selectors; labels resolve through t() at
// render so the period values stay pure data.
const PERIOD_LABEL_KEYS: Record<string, SelectorParam> = {
  month_price: ($) => $.admin.coupons.periods.month_price,
  quarter_price: ($) => $.admin.coupons.periods.quarter_price,
  half_year_price: ($) => $.admin.coupons.periods.half_year_price,
  year_price: ($) => $.admin.coupons.periods.year_price,
  two_year_price: ($) => $.admin.coupons.periods.two_year_price,
  three_year_price: ($) => $.admin.coupons.periods.three_year_price,
  onetime_price: ($) => $.admin.coupons.periods.onetime_price,
  reset_price: ($) => $.admin.coupons.periods.reset_price,
};

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

export function CouponEditor({
  record,
  plans,
  pending,
  onSave,
  children,
}: {
  record?: CouponRow;
  plans: Plan[];
  pending: boolean;
  onSave: (payload: CouponSubmit, onSuccess: (response?: GenerateResponse) => void) => void;
  children: ReactElement<{ onClick?: () => void }>;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const form = useForm<CouponEditorValues>({
    resolver: zodResolver(couponEditorSchema),
    defaultValues: {
      type: record?.type ?? 1,
      id: record?.id,
      name: record?.name,
      code: record?.code,
      value: record?.value ?? undefined,
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
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
      started_at: rfc3339ToUnixInput(record?.started_at),
      ended_at: rfc3339ToUnixInput(record?.ended_at),
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
      if (validValues.generate_count && response?.buffer)
        downloadGeneratedCsv('COUPON', response.buffer);
      setOpen(false);
    });
  });

  const selectedPlanIds = (values.limit_plan_ids ?? []).map(String);
  const selectedPeriods = (values.limit_period ?? []).map(String);
  const periodOptions = Object.entries(PERIOD_LABEL_KEYS).map(([period, labelKey]) => ({
    value: period,
    label: t(labelKey),
  }));

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
          <SheetTitle>
            {record?.id
              ? t(($) => $.admin.coupons.edit_title)
              : t(($) => $.admin.coupons.create_title)}
          </SheetTitle>
          <SheetDescription>{t(($) => $.admin.coupons.editor_description)}</SheetDescription>
        </SheetHeader>

        <form id="coupon-editor-form" className="space-y-4 px-4 pb-4" onSubmit={save} noValidate>
          <Field data-invalid={Boolean(formErrors.name)}>
            <FieldLabel htmlFor="coupon-name">{t(($) => $.admin.coupons.name)}</FieldLabel>
            <Input
              id="coupon-name"
              placeholder={t(($) => $.admin.coupons.name_placeholder)}
              aria-invalid={Boolean(formErrors.name)}
              {...form.register('name')}
              data-testid="coupon-name"
            />
            <FieldError errors={[formErrors.name]} />
          </Field>

          {!values.generate_count ? (
            <Field>
              <FieldLabel htmlFor="coupon-code">{t(($) => $.admin.coupons.custom_code)}</FieldLabel>
              <Input
                id="coupon-code"
                placeholder={t(($) => $.admin.coupons.custom_code_placeholder)}
                {...form.register('code', {
                  onChange: () => form.setValue('generate_count', undefined),
                })}
                data-testid="coupon-code"
              />
            </Field>
          ) : null}

          <Field data-invalid={Boolean(formErrors.value)}>
            <FieldLabel htmlFor="coupon-value">{t(($) => $.admin.coupons.value_label)}</FieldLabel>
            <div className="flex gap-2">
              <Select
                value={String(values.type ?? 1)}
                onValueChange={(value) => form.setValue('type', Number(value) as CouponType)}
              >
                <SelectTrigger
                  className="w-36 shrink-0"
                  data-testid="coupon-type"
                  aria-label={t(($) => $.admin.coupons.type_aria)}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">{t(($) => $.admin.coupons.type_amount_option)}</SelectItem>
                  <SelectItem value="2">{t(($) => $.admin.coupons.type_percent_option)}</SelectItem>
                </SelectContent>
              </Select>
              <InputGroup className="flex-1">
                <InputGroupInput
                  id="coupon-value"
                  type="number"
                  step={values.type === 1 ? '0.01' : '1'}
                  placeholder={t(($) => $.admin.coupons.value_placeholder)}
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
              <FieldLabel htmlFor="coupon-start">{t(($) => $.admin.coupons.started_at)}</FieldLabel>
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
              <FieldLabel htmlFor="coupon-end">{t(($) => $.admin.coupons.ended_at)}</FieldLabel>
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
            <FieldLabel htmlFor="coupon-limit-use">
              {t(($) => $.admin.coupons.limit_use)}
            </FieldLabel>
            <Input
              id="coupon-limit-use"
              type="number"
              step="1"
              placeholder={t(($) => $.admin.coupons.limit_use_placeholder)}
              aria-invalid={Boolean(formErrors.limit_use)}
              {...form.register('limit_use')}
              data-testid="coupon-limit-use"
            />
            <FieldError errors={[formErrors.limit_use]} />
          </Field>

          <Field data-invalid={Boolean(formErrors.limit_use_with_user)}>
            <FieldLabel htmlFor="coupon-limit-use-user">
              {t(($) => $.admin.coupons.limit_use_with_user)}
            </FieldLabel>
            <Input
              id="coupon-limit-use-user"
              type="number"
              step="1"
              placeholder={t(($) => $.admin.coupons.limit_use_with_user_placeholder)}
              aria-invalid={Boolean(formErrors.limit_use_with_user)}
              {...form.register('limit_use_with_user')}
              data-testid="coupon-limit-use-user"
            />
            <FieldError errors={[formErrors.limit_use_with_user]} />
          </Field>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">
              {t(($) => $.admin.coupons.limit_plans)}
            </legend>
            {plans.length ? (
              <CheckboxGroup
                options={planOptions(plans)}
                value={selectedPlanIds}
                onChange={(value) => form.setValue('limit_plan_ids', value.length ? value : null)}
                testId="coupon-plan-ids"
              />
            ) : (
              <p className="text-sm text-muted-foreground">{t(($) => $.admin.coupons.no_plans)}</p>
            )}
          </fieldset>

          <fieldset className="space-y-2">
            <legend className="text-sm font-medium text-foreground">
              {t(($) => $.admin.coupons.limit_periods)}
            </legend>
            <CheckboxGroup
              options={periodOptions}
              value={selectedPeriods}
              onChange={(value) => form.setValue('limit_period', value.length ? value : null)}
              testId="coupon-periods"
            />
          </fieldset>

          {!values.code && !values.id ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="coupon-generate-count">
                {t(($) => $.admin.coupons.generate_count)}
              </FieldLabel>
              <Input
                id="coupon-generate-count"
                type="number"
                min="1"
                max="500"
                step="1"
                placeholder={t(($) => $.admin.coupons.generate_count_placeholder)}
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

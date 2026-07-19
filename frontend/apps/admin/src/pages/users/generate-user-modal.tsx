import { useEffect } from 'react';
import dayjs from 'dayjs';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { admin } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Field, FieldError, FieldLabel, FieldLegend, FieldSet } from '@/components/ui/field';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { generateUserSchema, type GenerateUserValues } from './form-schema';
import { PLAN_NONE, requestErrorMessage, type PlanOption } from './shared';

type GenerateUserPayload = Parameters<typeof admin.generateUser>[1];

function planSelectItems(t: TFunction, plans: PlanOption[], includeEmpty = false) {
  return [
    ...(includeEmpty ? [{ value: PLAN_NONE, label: t(($) => $.admin.users.none) }] : []),
    ...plans.map((plan) => ({ value: String(plan.value), label: plan.label })),
  ];
}

export function GenerateUserModal({
  open,
  plans,
  loading,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: PlanOption[];
  loading: boolean;
  onClose: () => void;
  onSubmit: (values: GenerateUserPayload) => Promise<void>;
}) {
  const { t } = useTranslation();
  const form = useForm<GenerateUserValues>({
    resolver: zodResolver(generateUserSchema),
    defaultValues: {
      email_prefix: '',
      email_suffix: '',
      password: '',
      plan_id: null,
      expired_at: null,
      generate_count: '',
    },
  });
  // Read form state through the useFormState subscription instead of the
  // mutable form.formState proxy: the React Compiler caches proxy reads, which
  // drops react-hook-form's render-time access tracking and freezes error UI.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });
  const emailPrefix = useWatch({ control: form.control, name: 'email_prefix' });
  const generateCount = useWatch({ control: form.control, name: 'generate_count' });

  useEffect(() => {
    if (!open) form.reset();
  }, [form, open]);

  const close = () => {
    form.reset();
    onClose();
  };

  const planItems = planSelectItems(t, plans, true);
  const submit = form.handleSubmit(async (values) => {
    form.clearErrors('root.serverError');
    const emailPrefix = values.email_prefix.trim();
    const generateCount = values.generate_count.trim();
    const payload: GenerateUserPayload = {
      email_suffix: values.email_suffix.trim(),
      ...(emailPrefix ? { email_prefix: emailPrefix } : {}),
      ...(generateCount ? { generate_count: generateCount } : {}),
      ...(values.password ? { password: values.password } : {}),
      ...(values.plan_id != null ? { plan_id: values.plan_id } : {}),
      ...(values.expired_at ? { expired_at: values.expired_at } : {}),
    };
    try {
      await onSubmit(payload);
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-generate-dialog">
        <DialogHeader>
          <DialogTitle>{t(($) => $.admin.users.create_user)}</DialogTitle>
          <DialogDescription>{t(($) => $.admin.users.create_user_description)}</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={submit} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <FieldSet data-invalid={Boolean(formErrors.email_prefix || formErrors.email_suffix)}>
            <FieldLegend variant="label">{t(($) => $.admin.users.email)}</FieldLegend>
            <div className="flex items-center gap-2">
              {!generateCount ? (
                <Controller
                  control={form.control}
                  name="email_prefix"
                  render={({ field }) => (
                    <Input
                      {...field}
                      placeholder={t(($) => $.admin.users.email_prefix_placeholder)}
                      onChange={(event) => {
                        field.onChange(event);
                        if (event.target.value) {
                          form.setValue('generate_count', '', { shouldValidate: true });
                        }
                      }}
                      data-testid="generate-email-prefix"
                      aria-invalid={Boolean(formErrors.email_prefix)}
                    />
                  )}
                />
              ) : null}
              <span className="text-muted-foreground">@</span>
              <Controller
                control={form.control}
                name="email_suffix"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    placeholder={t(($) => $.admin.users.email_suffix_placeholder)}
                    data-testid="generate-email-suffix"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
            </div>
            <FieldError errors={[formErrors.email_prefix, formErrors.email_suffix]} />
          </FieldSet>
          <Field>
            <FieldLabel htmlFor="generate-password">{t(($) => $.admin.users.password)}</FieldLabel>
            <Controller
              control={form.control}
              name="password"
              render={({ field }) => (
                <Input
                  {...field}
                  id="generate-password"
                  placeholder={t(($) => $.admin.users.password_same_as_email)}
                />
              )}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="generate-expired">{t(($) => $.admin.users.expired_at)}</FieldLabel>
            <Controller
              control={form.control}
              name="expired_at"
              render={({ field }) => (
                <Input
                  id="generate-expired"
                  type="date"
                  placeholder={t(($) => $.admin.users.expired_at_placeholder)}
                  value={field.value ? dayjs(1000 * Number(field.value)).format('YYYY-MM-DD') : ''}
                  onChange={(event) =>
                    field.onChange(
                      event.target.value ? String(dayjs(event.target.value).unix()) : null,
                    )
                  }
                  data-testid="generate-expired"
                />
              )}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="generate-plan">
              {t(($) => $.admin.users.subscription_plan)}
            </FieldLabel>
            <Controller
              control={form.control}
              name="plan_id"
              render={({ field }) => (
                <Select
                  value={field.value != null ? String(field.value) : PLAN_NONE}
                  onValueChange={(value) =>
                    field.onChange(value === PLAN_NONE ? null : Number(value))
                  }
                >
                  <SelectTrigger id="generate-plan" className="w-full">
                    <SelectValue placeholder={t(($) => $.admin.users.plan_select_placeholder)} />
                  </SelectTrigger>
                  <SelectContent>
                    {planItems.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
          </Field>
          {!emailPrefix ? (
            <Field data-invalid={Boolean(formErrors.generate_count)}>
              <FieldLabel htmlFor="generate-count">
                {t(($) => $.admin.users.generate_count)}
              </FieldLabel>
              <Controller
                control={form.control}
                name="generate_count"
                render={({ field, fieldState }) => (
                  <Input
                    {...field}
                    id="generate-count"
                    placeholder={t(($) => $.admin.users.generate_count_placeholder)}
                    onChange={(event) => {
                      field.onChange(event);
                      if (event.target.value) {
                        form.setValue('email_prefix', '', { shouldValidate: true });
                      }
                    }}
                    data-testid="generate-count"
                    aria-invalid={fieldState.invalid}
                  />
                )}
              />
              <FieldError errors={[formErrors.generate_count]} />
            </Field>
          ) : null}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t(($) => $.common.cancel)}
            </Button>
            <Button
              type="submit"
              disabled={loading || isSubmitting}
              loading={loading || isSubmitting}
              data-testid="generate-submit"
            >
              {t(($) => $.admin.users.generate)}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

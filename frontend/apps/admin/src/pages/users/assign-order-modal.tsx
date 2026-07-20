import { useEffect, type ComponentProps } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { AdminUserRow } from '@v2board/types';
import { useAssignOrderMutation } from '@/lib/queries';
import { Button } from '@v2board/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Input } from '@v2board/ui/input';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import { assignOrderSchema, type AssignOrderValues } from './form-schema';
import { requestErrorMessage, type PlanOption } from './shared';

// The record keys below are backend wire values (period identifiers); only the
// labels are translated, resolved at render time.
function periodTextMap(t: TFunction): Record<string, string> {
  return {
    month_price: t(($) => $.admin.users.period_month),
    quarter_price: t(($) => $.admin.users.period_quarter),
    half_year_price: t(($) => $.admin.users.period_half_year),
    year_price: t(($) => $.admin.users.period_year),
    two_year_price: t(($) => $.admin.users.period_two_year),
    three_year_price: t(($) => $.admin.users.period_three_year),
    onetime_price: t(($) => $.admin.users.period_onetime),
    reset_price: t(($) => $.admin.users.period_reset),
  };
}

function periodOptions(t: TFunction) {
  const periodText = periodTextMap(t);
  return Object.keys(periodText).map((period) => ({
    value: period,
    label: periodText[period] ?? period,
  }));
}

function AmountInput({ suffix, ...props }: ComponentProps<typeof Input> & { suffix: string }) {
  return (
    <div className="relative">
      <Input className="pr-8" {...props} />
      <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
        {suffix}
      </span>
    </div>
  );
}

export function AssignOrderModal({
  user,
  plans,
  onClose,
}: {
  user: AdminUserRow | null;
  plans: PlanOption[];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const assign = useAssignOrderMutation();
  const form = useForm<AssignOrderValues>({
    resolver: zodResolver(assignOrderSchema),
    defaultValues: {
      email: user?.email ?? '',
      plan_id: undefined,
      period: undefined,
      total_amount: '',
    },
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors, isSubmitting } = useFormState({ control: form.control });

  useEffect(() => {
    form.reset({
      email: user?.email ?? '',
      plan_id: undefined,
      period: undefined,
      total_amount: '',
    });
  }, [form, user]);

  const close = () => {
    form.reset();
    onClose();
  };

  const doAssign = form.handleSubmit(async (values) => {
    // total_amount stays the raw entered value; the api-client applies the ×100
    // cents conversion. Preserving the raw payload here is the contract.
    form.clearErrors('root.serverError');
    try {
      await assign.mutateAsync(values);
      close();
    } catch (error) {
      form.setError('root.serverError', { message: requestErrorMessage(error) });
    }
  });

  return (
    <Dialog open={Boolean(user)} onOpenChange={(next) => (!next ? close() : undefined)}>
      <DialogContent data-testid="user-assign-dialog">
        <DialogHeader>
          <DialogTitle>{t(($) => $.admin.users.assign_title)}</DialogTitle>
          <DialogDescription>{t(($) => $.admin.users.assign_description)}</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={doAssign} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field data-invalid={Boolean(formErrors.email)}>
            <FieldLabel htmlFor="assign-email">{t(($) => $.admin.users.user_email)}</FieldLabel>
            <Controller
              control={form.control}
              name="email"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="assign-email"
                  type="email"
                  placeholder={t(($) => $.admin.users.user_email_placeholder)}
                  data-testid="assign-email"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.email]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.plan_id)}>
            <FieldLabel htmlFor="user-assign-plan">
              {t(($) => $.admin.users.plan_placeholder)}
            </FieldLabel>
            <Controller
              control={form.control}
              name="plan_id"
              render={({ field }) => (
                <Select
                  value={field.value != null ? String(field.value) : undefined}
                  onValueChange={(value) => field.onChange(Number(value))}
                >
                  <SelectTrigger
                    id="user-assign-plan"
                    className="w-full"
                    aria-invalid={Boolean(formErrors.plan_id)}
                  >
                    <SelectValue placeholder={t(($) => $.admin.users.plan_placeholder)} />
                  </SelectTrigger>
                  <SelectContent>
                    {plans.map((plan) => (
                      <SelectItem key={plan.value} value={String(plan.value)}>
                        {plan.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
            <FieldError errors={[formErrors.plan_id]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.period)}>
            <FieldLabel htmlFor="user-assign-period">
              {t(($) => $.admin.users.period_placeholder)}
            </FieldLabel>
            <Controller
              control={form.control}
              name="period"
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange}>
                  <SelectTrigger
                    id="user-assign-period"
                    className="w-full"
                    aria-invalid={Boolean(formErrors.period)}
                  >
                    <SelectValue placeholder={t(($) => $.admin.users.period_placeholder)} />
                  </SelectTrigger>
                  <SelectContent>
                    {periodOptions(t).map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            />
            <FieldError errors={[formErrors.period]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.total_amount)}>
            <FieldLabel htmlFor="assign-amount">{t(($) => $.admin.users.paid_amount)}</FieldLabel>
            <Controller
              control={form.control}
              name="total_amount"
              render={({ field, fieldState }) => (
                <AmountInput
                  {...field}
                  id="assign-amount"
                  suffix="¥"
                  placeholder={t(($) => $.admin.users.paid_amount_placeholder)}
                  data-testid="assign-amount"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.total_amount]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t(($) => $.common.cancel)}
            </Button>
            <Button
              type="submit"
              disabled={assign.isPending || isSubmitting}
              loading={assign.isPending || isSubmitting}
              data-testid="assign-submit"
            >
              {t(($) => $.common.confirm)}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

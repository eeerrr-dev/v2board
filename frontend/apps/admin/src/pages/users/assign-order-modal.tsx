import { useEffect, type ComponentProps } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import type { AdminUserRow } from '@v2board/types';
import { useAssignOrderMutation } from '@/lib/queries';
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
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { assignOrderSchema, type AssignOrderValues } from '../user-action-form-schema';
import { requestErrorMessage, type PlanOption } from './shared';

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
          <DialogTitle>订单分配</DialogTitle>
          <DialogDescription>为当前用户创建并分配订阅订单。</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={doAssign} noValidate>
          <FieldError errors={[formErrors.root?.serverError]} />
          <Field data-invalid={Boolean(formErrors.email)}>
            <FieldLabel htmlFor="assign-email">用户邮箱</FieldLabel>
            <Controller
              control={form.control}
              name="email"
              render={({ field, fieldState }) => (
                <Input
                  {...field}
                  id="assign-email"
                  type="email"
                  placeholder="请输入用户邮箱"
                  data-testid="assign-email"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.email]} />
          </Field>
          <Field data-invalid={Boolean(formErrors.plan_id)}>
            <FieldLabel htmlFor="user-assign-plan">请选择订阅</FieldLabel>
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
                    <SelectValue placeholder="请选择订阅" />
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
            <FieldLabel htmlFor="user-assign-period">请选择周期</FieldLabel>
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
                    <SelectValue placeholder="请选择周期" />
                  </SelectTrigger>
                  <SelectContent>
                    {PERIOD_OPTIONS.map((option) => (
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
            <FieldLabel htmlFor="assign-amount">支付金额</FieldLabel>
            <Controller
              control={form.control}
              name="total_amount"
              render={({ field, fieldState }) => (
                <AmountInput
                  {...field}
                  id="assign-amount"
                  suffix="¥"
                  placeholder="请输入需要支付的金额"
                  data-testid="assign-amount"
                  aria-invalid={fieldState.invalid}
                />
              )}
            />
            <FieldError errors={[formErrors.total_amount]} />
          </Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              取消
            </Button>
            <Button
              type="submit"
              disabled={assign.isPending || isSubmitting}
              loading={assign.isPending || isSubmitting}
              data-testid="assign-submit"
            >
              确定
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

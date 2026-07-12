import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { useWithdrawCommissionMutation } from '@/lib/queries';

interface WithdrawDialogProps {
  methods: string[];
  children?: ReactNode;
}

const withdrawSchema = z.object({
  method: z.string().min(1, 'invite.withdraw_method_placeholder'),
  account: z.string().trim().min(1, 'invite.withdraw_account_placeholder'),
});

type WithdrawFormValues = z.infer<typeof withdrawSchema>;

const WITHDRAW_METHOD_ID = 'invite-withdraw-method';
const WITHDRAW_ACCOUNT_ID = 'invite-withdraw-account';

export function WithdrawDialog({ methods, children }: WithdrawDialogProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const withdraw = useWithdrawCommissionMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<WithdrawFormValues>({
    resolver: zodResolver(withdrawSchema),
    defaultValues: { method: '', account: '' },
  });

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    form.reset({ method: '', account: '' });
  };

  const onSubmit = form.handleSubmit(({ account, method }) => {
    withdraw.mutate(
      { withdraw_method: method, withdraw_account: account },
      {
        onSuccess: () => {
          navigate('/ticket');
          onOpenChange(false);
        },
      },
    );
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? (
          <Button type="button" variant="outline">
            {t($ => $.invite.withdraw_button)}
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">{t($ => $.invite.withdraw)}</DialogTitle>
          <DialogDescription>{t($ => $.invite.withdraw_button)}</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={onSubmit} noValidate>
          <Controller
            control={form.control}
            name="method"
            render={({ field, fieldState }) => {
              const errorId = `${WITHDRAW_METHOD_ID}-error`;
              return (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={WITHDRAW_METHOD_ID}>
                    {t($ => $.invite.withdraw_method)}
                  </FieldLabel>
                  <Select
                    name={field.name}
                    value={field.value || undefined}
                    disabled={field.disabled}
                    onValueChange={field.onChange}
                  >
                    <SelectTrigger
                      ref={field.ref}
                      id={WITHDRAW_METHOD_ID}
                      onBlur={field.onBlur}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                      data-testid="invite-select-trigger"
                    >
                      <SelectValue placeholder={t($ => $.invite.withdraw_method_placeholder)} />
                    </SelectTrigger>
                    <SelectContent data-testid="invite-select-content">
                      {methods.map((item) => (
                        <SelectItem key={item} value={item}>
                          {item}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FieldError id={errorId} errors={[fieldState.error]} />
                </Field>
              );
            }}
          />
          <Controller
            control={form.control}
            name="account"
            render={({ field, fieldState }) => {
              const errorId = `${WITHDRAW_ACCOUNT_ID}-error`;
              return (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={WITHDRAW_ACCOUNT_ID}>
                    {t($ => $.invite.withdraw_account)}
                  </FieldLabel>
                  <Input
                    {...field}
                    id={WITHDRAW_ACCOUNT_ID}
                    placeholder={t($ => $.invite.withdraw_account_placeholder)}
                    aria-invalid={fieldState.invalid}
                    aria-describedby={fieldState.invalid ? errorId : undefined}
                  />
                  <FieldError id={errorId} errors={[fieldState.error]} />
                </Field>
              );
            }}
          />

          <DialogFooter data-testid="invite-dialog-footer">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                {t($ => $.common.cancel)}
              </Button>
            </DialogClose>
            <Button type="submit" loading={withdraw.isPending}>
              {t($ => $.profile.confirm)}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

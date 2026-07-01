import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { fieldError } from '@/lib/field-error';
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
} from '@/components/ui/shadcn-dialog';
import { FormField } from '@/components/ui/form-field';
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

  const onSubmit = form.handleSubmit(async ({ account, method }) => {
    try {
      await withdraw.mutateAsync({
        withdraw_method: method,
        withdraw_account: account,
      });
      navigate('/ticket');
      onOpenChange(false);
    } catch {}
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? (
          <Button type="button" variant="outline">
            {t('invite.withdraw_button')}
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">
            {t('invite.withdraw')}
          </DialogTitle>
          <DialogDescription>{t('invite.withdraw_button')}</DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={onSubmit} noValidate>
          <Controller
            control={form.control}
            name="method"
            render={({ field }) => (
              <Select value={field.value || undefined} onValueChange={field.onChange}>
                <FormField
                  id="invite-withdraw-method"
                  label={t('invite.withdraw_method')}
                  error={fieldError(form.formState.errors.method, t)}
                >
                  <SelectTrigger data-testid="invite-select-trigger">
                    <SelectValue placeholder={t('invite.withdraw_method_placeholder')} />
                  </SelectTrigger>
                </FormField>
                <SelectContent data-testid="invite-select-content">
                  {methods.map((item) => (
                    <SelectItem key={item} value={item}>
                      {item}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          />
          <FormField
            id="invite-withdraw-account"
            label={t('invite.withdraw_account')}
            error={fieldError(form.formState.errors.account, t)}
          >
            <Input
              placeholder={t('invite.withdraw_account_placeholder')}
              {...form.register('account')}
            />
          </FormField>

          <DialogFooter data-testid="invite-dialog-footer">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                {t('common.cancel')}
              </Button>
            </DialogClose>
            <Button type="submit" loading={withdraw.isPending}>
              {t('profile.confirm')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

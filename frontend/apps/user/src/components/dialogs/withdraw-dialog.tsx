import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
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
} from '@/components/ui/shadcn-dialog';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
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

        <Form {...form}>
          <form className="space-y-4" onSubmit={onSubmit} noValidate>
            <FormField
              control={form.control}
              name="method"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t('invite.withdraw_method')}</FormLabel>
                  <Select value={field.value || undefined} onValueChange={field.onChange}>
                    <FormControl>
                      <SelectTrigger data-testid="invite-select-trigger">
                        <SelectValue placeholder={t('invite.withdraw_method_placeholder')} />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent data-testid="invite-select-content">
                      {methods.map((item) => (
                        <SelectItem key={item} value={item}>
                          {item}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FormMessage />
                </FormItem>
              )}
            />
            <FormField
              control={form.control}
              name="account"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t('invite.withdraw_account')}</FormLabel>
                  <FormControl>
                    <Input
                      placeholder={t('invite.withdraw_account_placeholder')}
                      {...field}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

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
        </Form>
      </DialogContent>
    </Dialog>
  );
}

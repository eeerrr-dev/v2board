import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { AlertCircle } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
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
import { Label } from '@/components/ui/label';
import { userKeys, useTransferMutation } from '@/lib/queries';
import { getLegacySettings } from '@/lib/legacy-settings';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

const transferSchema = z.object({
  yuan: z.string().trim().min(1, 'invite.transfer_placeholder'),
});

type TransferFormValues = z.infer<typeof transferSchema>;

export function TransferDialog({ max, children }: TransferDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const transfer = useTransferMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<TransferFormValues>({
    resolver: zodResolver(transferSchema),
    defaultValues: { yuan: '' },
  });

  const onOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    form.reset({ yuan: '' });
  };

  const onSubmit = form.handleSubmit(async ({ yuan }) => {
    try {
      await transfer.mutateAsync(yuan);
      onOpenChange(false);
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    } catch {}
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? <Button type="button">{t('invite.transfer')}</Button>}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">
            {t('dashboard.transfer_to_balance')}
          </DialogTitle>
          <DialogDescription>
            {t('invite.current_commission_balance')}: {Number(max) / 100}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <Alert variant="destructive" className="bg-card">
            <AlertCircle className="size-4" />
            <AlertDescription>
              {t('invite.transfer_notice', { title: getLegacySettings().title })}
            </AlertDescription>
          </Alert>
          <div className="space-y-2">
            <Label htmlFor="invite-transfer-current">
              {t('invite.current_commission_balance')}
            </Label>
            <Input id="invite-transfer-current" disabled value={Number(max) / 100} readOnly />
          </div>
          <form className="space-y-4" onSubmit={onSubmit} noValidate>
            <FormField
              id="invite-transfer-amount"
              label={t('invite.transfer_amount')}
              error={
                form.formState.errors.yuan?.message
                  ? t(form.formState.errors.yuan.message)
                  : undefined
              }
            >
              <Input
                placeholder={t('invite.transfer_placeholder')}
                {...form.register('yuan')}
              />
            </FormField>
            <DialogFooter data-testid="invite-dialog-footer">
              <DialogClose asChild>
                <Button type="button" variant="outline">
                  {t('common.cancel')}
                </Button>
              </DialogClose>
              <Button type="submit" loading={transfer.isPending}>
                {t('profile.confirm')}
              </Button>
            </DialogFooter>
          </form>
        </div>
      </DialogContent>
    </Dialog>
  );
}

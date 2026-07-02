import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
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
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { formatCentsPlain } from '@v2board/config/format';
import { useTransferMutation } from '@/lib/queries';
import { getLegacySettings } from '@/lib/legacy-settings';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

const transferSchema = z.object({
  // The amount is sent as `Math.round(100 * Number(yuan))`, so a non-numeric or
  // non-positive value would post NaN / a negative integer. Gate it here.
  yuan: z
    .string()
    .trim()
    .min(1, 'invite.transfer_placeholder')
    .refine((value) => Number.isFinite(Number(value)) && Number(value) > 0, 'invite.transfer_invalid')
    // Balance is denominated in cents, so more than two decimals cannot be
    // represented — without this the extra digits were silently rounded by the
    // `Math.round(100 * …)` conversion (e.g. 10.999 → 1100 cents), transferring
    // a different amount than the user typed. Reject them explicitly instead.
    .refine((value) => {
      const decimals = value.split('.')[1];
      return decimals === undefined || decimals.length <= 2;
    }, 'invite.transfer_decimals'),
});

type TransferFormValues = z.infer<typeof transferSchema>;

export function TransferDialog({ max, children }: TransferDialogProps) {
  const { t } = useTranslation();
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

  const maxYuan = max !== undefined ? max / 100 : undefined;

  const onSubmit = form.handleSubmit(async ({ yuan }) => {
    // Surface the balance ceiling client-side; the backend still enforces it.
    if (maxYuan !== undefined && Number(yuan) > maxYuan) {
      form.setError('yuan', { message: 'invite.transfer_exceeds' });
      return;
    }
    try {
      // The transfer mutation invalidates the user record on success.
      await transfer.mutateAsync(yuan);
      onOpenChange(false);
    } catch {}
  });

  const maxText = max !== undefined ? formatCentsPlain(max) : '--.--';

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
            {t('invite.current_commission_balance')}: {maxText}
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
            <Input id="invite-transfer-current" disabled value={maxText} readOnly />
          </div>
          <Form {...form}>
            <form className="space-y-4" onSubmit={onSubmit} noValidate>
              <FormField
                control={form.control}
                name="yuan"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('invite.transfer_amount')}</FormLabel>
                    <FormControl>
                      <Input
                        placeholder={t('invite.transfer_placeholder')}
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
                <Button type="submit" loading={transfer.isPending}>
                  {t('profile.confirm')}
                </Button>
              </DialogFooter>
            </form>
          </Form>
        </div>
      </DialogContent>
    </Dialog>
  );
}

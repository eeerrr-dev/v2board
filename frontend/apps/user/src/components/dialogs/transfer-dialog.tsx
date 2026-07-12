import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
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
} from '@/components/ui/dialog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { formatCentsPlain } from '@v2board/config/format';
import { useTransferMutation } from '@/lib/queries';
import { getSiteTitle } from '@/lib/runtime-config';

interface TransferDialogProps {
  max?: number;
  children?: ReactNode;
}

const transferSchema = z.object({
  // The API client converts this major-unit string to exact integer cents, so
  // reject non-numeric and non-positive values before crossing that boundary.
  yuan: z
    .string()
    .trim()
    .min(1, 'invite.transfer_placeholder')
    .refine(
      (value) => Number.isFinite(Number(value)) && Number(value) > 0,
      'invite.transfer_invalid',
    )
    // Balance is denominated in cents, so more than two decimals cannot be
    // represented. Reject extra precision instead of asking the API boundary
    // to transfer a different amount than the user typed.
    .refine((value) => {
      const decimals = value.split('.')[1];
      return decimals === undefined || decimals.length <= 2;
    }, 'invite.transfer_decimals'),
});

type TransferFormValues = z.infer<typeof transferSchema>;

const TRANSFER_AMOUNT_ID = 'invite-transfer-amount';

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

  const onSubmit = form.handleSubmit(({ yuan }) => {
    // Surface the balance ceiling client-side; the backend still enforces it.
    if (maxYuan !== undefined && Number(yuan) > maxYuan) {
      form.setError('yuan', { message: 'invite.transfer_exceeds' });
      return;
    }
    // The transfer mutation invalidates the user record on success.
    transfer.mutate(yuan, { onSuccess: () => onOpenChange(false) });
  });

  const maxText = max !== undefined ? formatCentsPlain(max) : '--.--';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        {children ?? <Button type="button">{t($ => $.invite.transfer)}</Button>}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md" data-testid="invite-dialog">
        <DialogHeader>
          <DialogTitle data-testid="invite-dialog-title">
            {t($ => $.dashboard.transfer_to_balance)}
          </DialogTitle>
          <DialogDescription>
            {t($ => $.invite.current_commission_balance)}: {maxText}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <Alert variant="destructive" className="bg-card">
            <AlertCircle className="size-4" />
            <AlertDescription>
              {t($ => $.invite.transfer_notice, { title: getSiteTitle() })}
            </AlertDescription>
          </Alert>
          <div className="space-y-2">
            <Label htmlFor="invite-transfer-current">
              {t($ => $.invite.current_commission_balance)}
            </Label>
            <Input id="invite-transfer-current" disabled value={maxText} readOnly />
          </div>
          <form className="space-y-4" onSubmit={onSubmit} noValidate>
            <Controller
              control={form.control}
              name="yuan"
              render={({ field, fieldState }) => {
                const errorId = `${TRANSFER_AMOUNT_ID}-error`;
                return (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor={TRANSFER_AMOUNT_ID}>
                      {t($ => $.invite.transfer_amount)}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={TRANSFER_AMOUNT_ID}
                      placeholder={t($ => $.invite.transfer_placeholder)}
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
              <Button type="submit" loading={transfer.isPending}>
                {t($ => $.profile.confirm)}
              </Button>
            </DialogFooter>
          </form>
        </div>
      </DialogContent>
    </Dialog>
  );
}

import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { WalletCards } from 'lucide-react';
import { decimalToCents } from '@v2board/api-client';
import { formatCentsPlain } from '@v2board/config/format';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@v2board/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import { useCommConfig, useSaveOrderMutation, useUserInfo } from '@/lib/queries';
import { ProfileSwitch } from './profile-ui';
import { usePreferenceToggle } from './use-preference-toggle';

const depositSchema = z.object({
  // Keep the raw major-unit string through the form and let the API boundary
  // perform the cents conversion. Validate with that same exact converter so
  // an unsafe integer is reported inline instead of reaching the mutation.
  amount: z
    .string()
    .trim()
    .min(1, 'profile.deposit_invalid')
    .refine((value) => isSafePositiveAmount(value), 'profile.deposit_invalid')
    .refine((value) => {
      const decimals = value.split('.')[1];
      return decimals === undefined || decimals.length <= 2;
    }, 'profile.deposit_decimals'),
});

type DepositFormValues = z.infer<typeof depositSchema>;

const DEPOSIT_AMOUNT_ID = 'profile-deposit-amount';

function isSafePositiveAmount(value: string): boolean {
  try {
    return decimalToCents(value) > 0;
  } catch {
    return false;
  }
}

export function WalletCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const info = useUserInfo({ refetchOnMount: 'always' });
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const saveOrder = useSaveOrderMutation();
  const { toggle, pending } = usePreferenceToggle();
  const [depositOpen, setDepositOpen] = useState(false);
  const depositForm = useForm<DepositFormValues>({
    resolver: zodResolver(depositSchema),
    defaultValues: { amount: '' },
  });

  const data = info.data;
  const currency = comm?.currency;
  const depositPlaceholder = t(($) => $.profile.deposit_placeholder, { currency });

  const openDeposit = () => {
    depositForm.reset({ amount: '' });
    setDepositOpen(true);
  };

  const closeDeposit = () => {
    setDepositOpen(false);
    depositForm.reset({ amount: '' });
  };

  const onDeposit = depositForm.handleSubmit(({ amount }) => {
    // §5.5 (W4): the deposit arm of the create-order union replaced the
    // legacy plan_id:0 + period:"deposit" sentinel.
    saveOrder.mutate(
      { kind: 'deposit', deposit_amount: amount },
      { onSuccess: (tradeNo) => void navigate(`/order/${tradeNo}`) },
    );
    closeDeposit();
  });
  // No invalid handler: an empty / non-numeric / over-precise amount now keeps
  // the dialog open and surfaces the schema error inline (like the transfer
  // dialog) instead of silently closing.

  return (
    <Card className="overflow-hidden" data-testid="profile-wallet-card">
      <CardHeader className="gap-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-2">
            <CardDescription>{t(($) => $.profile.wallet)}</CardDescription>
            <CardTitle
              className="text-4xl font-semibold tracking-normal text-foreground sm:text-5xl"
              data-testid="profile-card-title"
            >
              {data?.balance !== undefined ? formatCentsPlain(data.balance) : '--.--'}
              <span className="ml-3 align-baseline text-base font-medium text-muted-foreground">
                {currency}
              </span>
            </CardTitle>
          </div>
          <div className="rounded-md border border-border bg-muted p-2.5 text-muted-foreground">
            <WalletCards className="size-5" />
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-5">
        <div className="flex flex-col gap-4 rounded-lg border border-border bg-muted/40 p-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="space-y-1">
            <div className="text-sm leading-5 font-medium">{t(($) => $.profile.auto_renewal)}</div>
          </div>
          <ProfileSwitch
            ariaLabel={t(($) => $.profile.auto_renewal)}
            checked={data?.auto_renewal}
            loading={pending.auto_renewal}
            onChange={(checked) => void toggle('auto_renewal', checked)}
          />
        </div>
        <Button
          className="w-full sm:w-fit"
          data-testid="profile-recharge"
          size="lg"
          onClick={openDeposit}
        >
          {t(($) => $.profile.recharge)}
        </Button>
      </CardContent>

      <Dialog
        open={depositOpen}
        onOpenChange={(nextOpen) => (nextOpen ? undefined : closeDeposit())}
      >
        <DialogContent
          className="sm:max-w-md"
          data-testid="profile-deposit-dialog"
          showCloseButton={false}
        >
          <DialogHeader>
            <DialogTitle>{t(($) => $.profile.recharge)}</DialogTitle>
            <DialogDescription>{depositPlaceholder}</DialogDescription>
          </DialogHeader>
          <form className="grid gap-4" onSubmit={onDeposit} noValidate>
            <Controller
              control={depositForm.control}
              name="amount"
              render={({ field, fieldState }) => {
                const errorId = `${DEPOSIT_AMOUNT_ID}-error`;

                return (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel className="sr-only" htmlFor={DEPOSIT_AMOUNT_ID}>
                      {depositPlaceholder}
                    </FieldLabel>
                    <Input
                      {...field}
                      id={DEPOSIT_AMOUNT_ID}
                      data-testid="profile-deposit-input"
                      autoComplete="one-time-code"
                      placeholder={depositPlaceholder}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.invalid ? errorId : undefined}
                    />
                    <FieldError
                      id={errorId}
                      data-testid="profile-deposit-error"
                      errors={[fieldState.error]}
                    />
                  </Field>
                );
              }}
            />
            <DialogFooter>
              <Button type="button" variant="outline" onClick={closeDeposit}>
                {t(($) => $.common.cancel)}
              </Button>
              <Button type="submit" data-testid="profile-deposit-confirm">
                {t(($) => $.profile.confirm)}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

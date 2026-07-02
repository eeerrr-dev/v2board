import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { WalletCards } from 'lucide-react';
import { formatCentsPlain } from '@v2board/config/format';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Form, FormControl, FormField, FormItem, FormMessage } from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { useCommConfig, useSaveOrderMutation, useUserInfo } from '@/lib/queries';
import { ProfileSwitch } from './profile-ui';
import { usePreferenceToggle } from './use-preference-toggle';

const depositSchema = z.object({
  // Validate the raw string so the decimal-place check sees the typed value
  // before coercion. deposit_amount is sent as Math.round(Number(amount) * 100),
  // so a value with more than two decimals cannot be represented in cents —
  // reject it inline instead of silently rounding (e.g. 19.999 → 2000 cents).
  // No .transform here (unlike a coercing schema): the canonical FormField wires
  // `control` with input === output, and the handler does the Number() itself.
  amount: z
    .string()
    .trim()
    .min(1, 'profile.deposit_invalid')
    .refine((value) => Number.isFinite(Number(value)) && Number(value) > 0, 'profile.deposit_invalid')
    .refine((value) => {
      const decimals = value.split('.')[1];
      return decimals === undefined || decimals.length <= 2;
    }, 'profile.deposit_decimals'),
});

type DepositFormValues = z.infer<typeof depositSchema>;

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
  const depositPlaceholder = t('profile.deposit_placeholder', { currency });

  const openDeposit = () => {
    depositForm.reset({ amount: '' });
    setDepositOpen(true);
  };

  const closeDeposit = () => {
    setDepositOpen(false);
    depositForm.reset({ amount: '' });
  };

  const onDeposit = depositForm.handleSubmit(({ amount }) => {
    void saveOrder
      .mutateAsync({
        plan_id: 0,
        period: 'deposit',
        // Cents must be an integer: the backend stores total_amount in an int
        // column that truncates, so a raw float (19.99 * 100 = 1998.9999…)
        // would under-credit the user by a cent. Round to the nearest cent.
        deposit_amount: Math.round(Number(amount) * 100),
      })
      .then((tradeNo) => navigate(`/order/${tradeNo}`))
      .catch(() => {});
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
            <CardDescription>{t('profile.wallet')}</CardDescription>
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
            <div className="text-sm font-medium leading-5">{t('profile.auto_renewal')}</div>
          </div>
          <ProfileSwitch
            ariaLabel={t('profile.auto_renewal')}
            checked={data?.auto_renewal}
            loading={pending.auto_renewal}
            onChange={(checked) => void toggle('auto_renewal', checked ? 1 : 0)}
          />
        </div>
        <Button
          className="w-full sm:w-fit"
          data-testid="profile-recharge"
          size="lg"
          onClick={openDeposit}
        >
          {t('profile.recharge')}
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
            <DialogTitle>{t('profile.recharge')}</DialogTitle>
            <DialogDescription>{depositPlaceholder}</DialogDescription>
          </DialogHeader>
          <Form {...depositForm}>
            <form className="grid gap-4" onSubmit={onDeposit} noValidate>
              <FormField
                control={depositForm.control}
                name="amount"
                render={({ field, fieldState }) => (
                  <FormItem>
                    <FormControl>
                      <Input
                        data-testid="profile-deposit-input"
                        autoComplete="one-time-code"
                        aria-label={depositPlaceholder}
                        placeholder={depositPlaceholder}
                        invalid={fieldState.error ? true : undefined}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage data-testid="profile-deposit-error" />
                  </FormItem>
                )}
              />
              <DialogFooter>
                <Button type="button" variant="outline" onClick={closeDeposit}>
                  {t('common.cancel')}
                </Button>
                <Button type="submit" data-testid="profile-deposit-confirm">
                  {t('profile.confirm')}
                </Button>
              </DialogFooter>
            </form>
          </Form>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

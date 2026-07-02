import { useState } from 'react';
import type { ParseKeys } from 'i18next';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { Gift } from 'lucide-react';
import { ApiError } from '@v2board/api-client';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Form, FormControl, FormField, FormItem, FormLabel } from '@/components/ui/form';
import { Input } from '@/components/ui/input';
import { useRedeemGiftCardMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import { SectionIcon } from './profile-ui';

const giftCardSchema = z.object({
  code: z.string().min(1),
});

type GiftCardFormValues = z.infer<typeof giftCardSchema>;

export function GiftCardCard() {
  const { t } = useTranslation();
  const redeem = useRedeemGiftCardMutation();
  const [redeemTimeoutStuck, setRedeemTimeoutStuck] = useState(false);
  const giftCardForm = useForm<GiftCardFormValues>({
    resolver: zodResolver(giftCardSchema),
    defaultValues: { code: '' },
  });

  const redeemLoading = redeem.isPending || redeemTimeoutStuck;

  const onRedeem = giftCardForm.handleSubmit(
    async ({ code }) => {
      setRedeemTimeoutStuck(false);
      try {
        const result = await redeem.mutateAsync(code);
        toast.success(
          t('profile.redeem_success', {
            detail: redeemGiftcardText(result.type, result.value, t),
          }),
        );
      } catch (error) {
        if (isTransportError(error)) setRedeemTimeoutStuck(true);
      }
    },
    () => toast.error(t('profile.redeem_placeholder')),
  );

  return (
    <Card data-testid="profile-gift-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <Gift className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t('profile.redeem_giftcard')}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <Form {...giftCardForm}>
          <form className="space-y-4" onSubmit={onRedeem} noValidate>
            <FormField
              control={giftCardForm.control}
              name="code"
              render={({ field, fieldState }) => (
                <FormItem className="gap-2.5">
                  <FormLabel>{t('profile.redeem_giftcard')}</FormLabel>
                  <FormControl>
                    <Input
                      data-testid="profile-giftcard-input"
                      placeholder={t('profile.redeem_placeholder')}
                      autoComplete="one-time-code"
                      invalid={fieldState.error ? true : undefined}
                      {...field}
                    />
                  </FormControl>
                </FormItem>
              )}
            />
            <Button
              type="submit"
              className="w-full sm:w-fit"
              data-testid="profile-redeem-button"
              loading={redeemLoading}
            >
              {t('profile.redeem_submit')}
            </Button>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

function redeemGiftcardText(
  type: number,
  value: number,
  // A minimal callable instead of the full TFunction: passing the heavy i18next
  // t type into this helper and calling it with interpolation overflows the TS
  // instantiation depth (TS2589). Keys are still checked against ParseKeys.
  t: (key: ParseKeys, options?: Record<string, string | number>) => string,
) {
  switch (type) {
    case 1:
      return t('profile.redeem_balance', { amount: (value / 100).toFixed(2) });
    case 2:
      return t('profile.redeem_days', { days: value });
    case 3:
      return t('profile.redeem_traffic', { traffic: value });
    case 4:
      return t('profile.redeem_reset');
    case 5:
      return t('profile.redeem_plan_days', { days: value });
    default:
      return t('profile.redeem_unknown');
  }
}

// A gift-card redeem that never gets a backend response (timeout / network
// drop) leaves the button in the legacy "stuck loading" state. The api-client
// already models every transport-level failure as ApiError.status === 0, so key
// off that structured signal instead of string-sniffing the message (the same
// anti-pattern api.test.ts forbids in the api layer).
function isTransportError(error: unknown) {
  return error instanceof ApiError && error.status === 0;
}

import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { Gift } from 'lucide-react';
import { formatCentsPlain } from '@v2board/config/format';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { useRedeemGiftCardMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import { SectionIcon } from './profile-ui';

const giftCardSchema = z.object({
  code: z.string().min(1, 'profile.redeem_placeholder'),
});

type GiftCardFormValues = z.infer<typeof giftCardSchema>;

const GIFT_CARD_CODE_ID = 'profile-gift-card-code';

export function GiftCardCard() {
  const { t } = useTranslation();
  const redeem = useRedeemGiftCardMutation();
  const giftCardForm = useForm<GiftCardFormValues>({
    resolver: zodResolver(giftCardSchema),
    defaultValues: { code: '' },
  });

  const onRedeem = giftCardForm.handleSubmit(
    ({ code }) => {
      redeem.mutate(code, {
        onSuccess: (result) => {
          toast.success(
            t(($) => $.profile.redeem_success, {
              detail: redeemGiftcardText(result.type, result.value, t),
            }),
          );
        },
      });
    },
    () => toast.error(t(($) => $.profile.redeem_placeholder)),
  );

  return (
    <Card data-testid="profile-gift-card">
      <CardHeader>
        <div className="flex items-center gap-3">
          <SectionIcon>
            <Gift className="size-4" />
          </SectionIcon>
          <CardTitle className="text-lg" data-testid="profile-card-title">
            {t(($) => $.profile.redeem_giftcard)}
          </CardTitle>
        </div>
      </CardHeader>
      <CardContent>
        <form className="space-y-4" onSubmit={onRedeem} noValidate>
          <Controller
            control={giftCardForm.control}
            name="code"
            render={({ field, fieldState }) => {
              const errorId = `${GIFT_CARD_CODE_ID}-error`;
              return (
                <Field className="gap-2.5" data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={GIFT_CARD_CODE_ID}>
                    {t(($) => $.profile.redeem_giftcard)}
                  </FieldLabel>
                  <Input
                    {...field}
                    id={GIFT_CARD_CODE_ID}
                    data-testid="profile-giftcard-input"
                    placeholder={t(($) => $.profile.redeem_placeholder)}
                    autoComplete="one-time-code"
                    aria-invalid={fieldState.invalid}
                    aria-describedby={fieldState.invalid ? errorId : undefined}
                  />
                  <FieldError id={errorId} errors={[fieldState.error]} />
                </Field>
              );
            }}
          />
          <Button
            type="submit"
            className="w-full sm:w-fit"
            data-testid="profile-redeem-button"
            loading={redeem.isPending}
          >
            {t(($) => $.profile.redeem_submit)}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

function redeemGiftcardText(type: number, value: number | null, t: TFunction) {
  if (value === null) {
    return type === 4 ? t(($) => $.profile.redeem_reset) : t(($) => $.profile.redeem_unknown);
  }
  switch (type) {
    case 1:
      return t(($) => $.profile.redeem_balance, { amount: formatCentsPlain(value) });
    case 2:
      return t(($) => $.profile.redeem_days, { days: value });
    case 3:
      return t(($) => $.profile.redeem_traffic, { traffic: value });
    case 4:
      return t(($) => $.profile.redeem_reset);
    case 5:
      return t(($) => $.profile.redeem_plan_days, { days: value });
    default:
      return t(($) => $.profile.redeem_unknown);
  }
}

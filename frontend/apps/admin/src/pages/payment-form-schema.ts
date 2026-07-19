import { z } from 'zod';
import { decimalToCents } from '@v2board/api-client';
import {
  isBlankInput,
  isEmptyInput,
  isHttpUrlInput,
  isMoneyInput,
  isNumericInput,
} from '@/lib/form-input-validation';

const optionalText = z.union([z.string(), z.null()]).optional();
const optionalNumberInput = z.union([z.string(), z.number(), z.null()]).optional();
const MAX_I32 = 2_147_483_647;

// Messages are i18n keys resolved at display time through FieldError ->
// translateRuntimeMessage (the values live in the admin.payments fragment).
export const paymentFormSchema = z.object({
  id: z.number().optional(),
  name: z.string().refine((value) => !isBlankInput(value), 'admin.payments.name_required'),
  icon: optionalText,
  notify_domain: optionalText.refine(
    (value) => isEmptyInput(value) || isHttpUrlInput(value),
    'admin.payments.notify_domain_invalid',
  ),
  handling_fee_percent: optionalNumberInput
    .refine(
      (value) => isEmptyInput(value) || isNumericInput(value),
      'admin.payments.fee_percent_numeric',
    )
    .refine(
      (value) => isEmptyInput(value) || (Number(value) >= 0.1 && Number(value) <= 100),
      'admin.payments.fee_percent_range',
    ),
  handling_fee_fixed: optionalNumberInput
    .refine(
      (value) => isEmptyInput(value) || isMoneyInput(value),
      'admin.payments.fee_fixed_invalid',
    )
    .refine(
      (value) =>
        isEmptyInput(value) ||
        !isMoneyInput(value) ||
        (Number(value) >= 0 && decimalToCents(value) <= MAX_I32),
      'admin.payments.fee_fixed_exceeds',
    ),
  payment: z.string().refine((value) => value.trim().length > 0, 'admin.payments.payment_required'),
  config: z
    .record(z.string(), z.string())
    .refine((value) => Object.keys(value).length > 0, 'admin.payments.config_required'),
});

export type PaymentEditorValues = z.input<typeof paymentFormSchema>;

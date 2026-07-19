import { z } from 'zod';
import { decimalToCents } from '@v2board/api-client';
import {
  isBlankInput,
  isEmptyInput,
  isIntegerInput,
  isMoneyInput,
} from '@/lib/form-input-validation';

const numberInput = z.union([z.string(), z.number()]);
const MAX_I32 = 2_147_483_647;
const nullableNumberInput = numberInput.nullable().optional();
// Messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage at display time.
const requiredIntegerInput = nullableNumberInput.refine(
  (value) => !isBlankInput(value) && isIntegerInput(value),
  'admin.coupons.integer_required',
);
const optionalIntegerInput = nullableNumberInput.refine(
  (value) => isEmptyInput(value) || isIntegerInput(value),
  'admin.coupons.integer_required',
);
const generateCountInput = numberInput
  .optional()
  .refine(
    (value) => isEmptyInput(value) || isIntegerInput(value),
    'admin.coupons.generate_count_integer',
  )
  .refine(
    (value) => isEmptyInput(value) || (Number(value) >= 1 && Number(value) <= 500),
    'admin.coupons.generate_count_range',
  );

const common = {
  id: z.number().optional(),
  name: z
    .string()
    .optional()
    .refine((value) => !isBlankInput(value), 'admin.coupons.name_required'),
  code: z.string().optional(),
  value: numberInput.optional(),
  started_at: requiredIntegerInput,
  ended_at: requiredIntegerInput,
  limit_use: optionalIntegerInput,
  generate_count: generateCountInput,
};

function validateWindow(
  values: { started_at?: string | number | null; ended_at?: string | number | null },
  context: z.RefinementCtx,
) {
  if (
    !isBlankInput(values.started_at) &&
    !isBlankInput(values.ended_at) &&
    isIntegerInput(values.started_at) &&
    isIntegerInput(values.ended_at) &&
    Number(values.ended_at) <= Number(values.started_at)
  ) {
    context.addIssue({
      code: 'custom',
      path: ['ended_at'],
      message: 'admin.coupons.ended_at_after_started_at',
    });
  }
}

export const couponEditorSchema = z
  .object({
    ...common,
    type: z.union([z.literal(1), z.literal(2)]),
    limit_use_with_user: optionalIntegerInput,
    limit_plan_ids: z
      .array(z.union([z.string(), z.number()]))
      .nullable()
      .optional(),
    limit_period: z.array(z.string()).nullable().optional(),
  })
  .superRefine((values, context) => {
    const validValue =
      values.type === 1 ? isMoneyInput(values.value) : isIntegerInput(values.value);
    const numericValue = Number(values.value);
    const storedValue =
      validValue && values.type === 1
        ? decimalToCents(values.value as string | number)
        : numericValue;
    if (
      isBlankInput(values.value) ||
      !validValue ||
      numericValue < 0 ||
      storedValue > MAX_I32 ||
      (values.type === 2 && numericValue > 100)
    ) {
      context.addIssue({
        code: 'custom',
        path: ['value'],
        message:
          values.type === 1 ? 'admin.coupons.amount_invalid' : 'admin.coupons.percent_invalid',
      });
    }
    validateWindow(values, context);
  });

export const giftcardEditorSchema = z
  .object({
    ...common,
    type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(5)]),
    plan_id: z.union([z.string(), z.number(), z.null()]).optional(),
  })
  .superRefine((values, context) => {
    if (values.type !== 4) {
      const validValue =
        values.type === 1 ? isMoneyInput(values.value) : isIntegerInput(values.value);
      const storedValue =
        validValue && values.type === 1
          ? decimalToCents(values.value as string | number)
          : Number(values.value);
      if (isBlankInput(values.value) || !validValue || storedValue < 0 || storedValue > MAX_I32) {
        context.addIssue({
          code: 'custom',
          path: ['value'],
          message:
            values.type === 1
              ? 'admin.coupons.giftcards.amount_invalid'
              : 'admin.coupons.giftcards.value_invalid',
        });
      }
    }
    if (values.type === 5 && !isIntegerInput(values.plan_id)) {
      context.addIssue({
        code: 'custom',
        path: ['plan_id'],
        message: 'admin.coupons.giftcards.plan_required',
      });
    }
    validateWindow(values, context);
  });

export type CouponEditorValues = z.input<typeof couponEditorSchema>;
export type GiftcardEditorValues = z.input<typeof giftcardEditorSchema>;

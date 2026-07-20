import { z } from 'zod';
import { decimalToCents, decimalToMoneyMinor, type admin } from '@v2board/api-client';
import {
  isBlankInput,
  isEmptyInput,
  isIntegerInput,
  isMoneyInput,
} from '@/lib/form-input-validation';

// Messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage at display time.
const numberInput = z.union([z.string(), z.number()]);
const nullableNumberInput = numberInput.nullable();
const MIN_I32 = -2_147_483_648;
const MAX_I32 = 2_147_483_647;
const requiredTransferInput = nullableNumberInput
  .refine((value) => !isBlankInput(value) && isIntegerInput(value), 'admin.plans.integer_invalid')
  .refine(
    (value) => Number(value) >= 0 && Number(value) <= MAX_I32,
    'admin.plans.integer_out_of_range',
  );
const optionalIntegerInput = nullableNumberInput
  .optional()
  .refine((value) => isEmptyInput(value) || isIntegerInput(value), 'admin.plans.integer_invalid')
  .refine(
    (value) => isEmptyInput(value) || (Number(value) >= 0 && Number(value) <= MAX_I32),
    'admin.plans.integer_out_of_range',
  );
const optionalPriceInput = nullableNumberInput
  .refine((value) => value === null || isMoneyInput(value), 'admin.plans.price_invalid')
  .refine(
    (value) =>
      value === null ||
      !isMoneyInput(value) ||
      (decimalToCents(value) >= MIN_I32 && decimalToCents(value) <= MAX_I32),
    'admin.plans.price_out_of_range',
  );

export const planEditorSchema = z
  .object({
    id: z.number().optional(),
    name: z
      .string()
      .nullable()
      .refine((value) => !isBlankInput(value), 'admin.plans.name_required'),
    content: z.string().nullable().optional(),
    group_id: numberInput
      .optional()
      .refine(
        (value) => !isBlankInput(value) && isIntegerInput(value),
        'admin.plans.group_required',
      ),
    reset_traffic_method: z
      .union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.null()])
      .optional(),
    transfer_enable: requiredTransferInput,
    device_limit: optionalIntegerInput,
    capacity_limit: optionalIntegerInput,
    speed_limit: optionalIntegerInput,
    month_price: optionalPriceInput,
    quarter_price: optionalPriceInput,
    half_year_price: optionalPriceInput,
    year_price: optionalPriceInput,
    two_year_price: optionalPriceInput,
    three_year_price: optionalPriceInput,
    onetime_price: optionalPriceInput,
    reset_price: optionalPriceInput,
    force_update: z.boolean().optional(),
  })
  .superRefine((values, ctx) => {
    if (values.id === undefined && values.force_update !== undefined) {
      ctx.addIssue({
        code: 'custom',
        path: ['force_update'],
        message: 'admin.plans.force_update_edit_only',
      });
    }
  });

/** UI-only plan draft. Money fields are decimal text/number inputs in major units. */
export type AdminPlanFormValues = z.input<typeof planEditorSchema>;

type ValidAdminPlanFormValues = z.output<typeof planEditorSchema>;

function optionalInteger(value: string | number | null | undefined): number | null {
  return value === null || value === undefined || value === '' ? null : Number(value);
}

/**
 * The sole admin plan form → wire boundary. It converts major-unit decimal
 * inputs to branded minor units and converts every numeric form input to a JSON
 * number before the API client sees it.
 */
export function adminPlanFormToSaveRequest(
  values: ValidAdminPlanFormValues,
): admin.AdminPlanSaveRequest {
  if (
    typeof values.name !== 'string' ||
    values.group_id == null ||
    values.transfer_enable == null
  ) {
    throw new TypeError('Validated admin plan form is missing required values');
  }

  const price = (value: string | number | null) =>
    value === null ? null : decimalToMoneyMinor(value);

  const writeFields = {
    name: values.name,
    content: values.content ?? null,
    group_id: Number(values.group_id),
    transfer_enable: Number(values.transfer_enable),
    device_limit: optionalInteger(values.device_limit),
    capacity_limit: optionalInteger(values.capacity_limit),
    speed_limit: optionalInteger(values.speed_limit),
    reset_traffic_method: values.reset_traffic_method ?? null,
    month_price: price(values.month_price),
    quarter_price: price(values.quarter_price),
    half_year_price: price(values.half_year_price),
    year_price: price(values.year_price),
    two_year_price: price(values.two_year_price),
    three_year_price: price(values.three_year_price),
    onetime_price: price(values.onetime_price),
    reset_price: price(values.reset_price),
  };
  return values.id === undefined
    ? writeFields
    : {
        ...writeFields,
        id: values.id,
        ...(values.force_update === undefined ? {} : { force_update: values.force_update }),
      };
}

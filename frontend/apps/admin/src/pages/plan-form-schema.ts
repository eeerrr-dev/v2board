import { z } from 'zod';
import { decimalToCents } from '@v2board/api-client';
import {
  isBlankInput,
  isEmptyInput,
  isIntegerInput,
  isMoneyInput,
} from '@/lib/form-input-validation';

const numberInput = z.union([z.string(), z.number()]);
const nullableNumberInput = numberInput.nullable();
const MAX_I32 = 2_147_483_647;
const requiredIntegerInput = nullableNumberInput.refine(
  (value) => !isBlankInput(value) && isIntegerInput(value),
  '请输入整数',
).refine((value) => Number(value) >= 0 && Number(value) <= MAX_I32, '数值超出可保存范围');
const optionalIntegerInput = nullableNumberInput
  .optional()
  .refine((value) => isEmptyInput(value) || isIntegerInput(value), '请输入整数')
  .refine(
    (value) => isEmptyInput(value) || (Number(value) >= 0 && Number(value) <= MAX_I32),
    '数值超出可保存范围',
  );
const optionalPriceInput = nullableNumberInput
  .refine((value) => value === null || isMoneyInput(value), '请输入有效金额')
  .refine(
    (value) =>
      value === null ||
      !isMoneyInput(value) ||
      (Number(value) >= 0 && decimalToCents(value) <= MAX_I32),
    '金额超出可保存范围',
  );

export const planEditorSchema = z.object({
  id: z.number().optional(),
  name: z
    .string()
    .nullable()
    .refine((value) => !isBlankInput(value), '套餐名称不能为空'),
  content: z.string().nullable().optional(),
  group_id: numberInput
    .optional()
    .refine((value) => !isBlankInput(value) && isIntegerInput(value), '权限组不能为空'),
  reset_traffic_method: z
    .union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.null()])
    .optional(),
  transfer_enable: requiredIntegerInput,
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
});

export type PlanEditorValues = z.input<typeof planEditorSchema>;

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
const requiredIntegerInput = nullableNumberInput.refine(
  (value) => !isBlankInput(value) && isIntegerInput(value),
  '请输入整数',
);
const optionalIntegerInput = nullableNumberInput.refine(
  (value) => isEmptyInput(value) || isIntegerInput(value),
  '请输入整数',
);
const generateCountInput = numberInput
  .optional()
  .refine((value) => isEmptyInput(value) || isIntegerInput(value), '生成数量必须为整数')
  .refine(
    (value) => isEmptyInput(value) || (Number(value) >= 1 && Number(value) <= 500),
    '生成数量须在 1 到 500 之间',
  );

const common = {
  id: z.number().optional(),
  name: z
    .string()
    .optional()
    .refine((value) => !isBlankInput(value), '名称不能为空'),
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
      message: '结束时间必须晚于开始时间',
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
        message: values.type === 1 ? '请输入非负优惠金额' : '优惠比例必须为 0 到 100 之间的整数',
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
          message: values.type === 1 ? '请输入非负礼品卡金额' : '礼品卡数值必须为非负整数',
        });
      }
    }
    if (values.type === 5 && !isIntegerInput(values.plan_id)) {
      context.addIssue({
        code: 'custom',
        path: ['plan_id'],
        message: '请选择订阅计划',
      });
    }
    validateWindow(values, context);
  });

export type CouponEditorValues = z.input<typeof couponEditorSchema>;
export type GiftcardEditorValues = z.input<typeof giftcardEditorSchema>;

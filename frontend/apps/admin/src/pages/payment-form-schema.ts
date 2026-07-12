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

export const paymentFormSchema = z.object({
  id: z.number().optional(),
  name: z.string().refine((value) => !isBlankInput(value), '显示名称不能为空'),
  icon: optionalText,
  notify_domain: optionalText.refine(
    (value) => isEmptyInput(value) || isHttpUrlInput(value),
    '请输入有效的 HTTP(S) 通知域名',
  ),
  handling_fee_percent: optionalNumberInput
    .refine((value) => isEmptyInput(value) || isNumericInput(value), '百分比手续费必须是数字')
    .refine(
      (value) => isEmptyInput(value) || (Number(value) >= 0.1 && Number(value) <= 100),
      '百分比手续费范围须在 0.1 到 100 之间',
    ),
  handling_fee_fixed: optionalNumberInput
    .refine((value) => isEmptyInput(value) || isMoneyInput(value), '固定手续费格式有误')
    .refine(
      (value) =>
        isEmptyInput(value) ||
        !isMoneyInput(value) ||
        (Number(value) >= 0 && decimalToCents(value) <= MAX_I32),
      '固定手续费超出可保存范围',
    ),
  payment: z.string().refine((value) => value.trim().length > 0, '请选择支付接口'),
  config: z
    .record(z.string(), z.string())
    .refine((value) => Object.keys(value).length > 0, '支付配置不能为空'),
});

export type PaymentEditorValues = z.input<typeof paymentFormSchema>;

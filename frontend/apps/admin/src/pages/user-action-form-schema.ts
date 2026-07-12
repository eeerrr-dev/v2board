import { z } from 'zod';
import { decimalToCents } from '@v2board/api-client';
import type { PlanPeriod } from '@v2board/types';
import { isIntegerInput, isMoneyInput } from '@/lib/form-input-validation';

export const PLAN_PERIOD_VALUES = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
] as const satisfies readonly PlanPeriod[];

const MAX_I32 = 2_147_483_647;

export const assignOrderSchema = z.object({
  email: z.string().trim().min(1, '邮箱不能为空').pipe(z.email('邮箱格式有误')),
  plan_id: z.number({ error: '订阅不能为空' }).int().positive('订阅不能为空'),
  period: z.enum(PLAN_PERIOD_VALUES, { error: '订阅周期不能为空' }),
  total_amount: z
    .string()
    .trim()
    .min(1, '支付金额不能为空')
    .refine(isMoneyInput, '支付金额格式有误')
    .refine(
      (value) => !isMoneyInput(value) || (Number(value) >= 0 && decimalToCents(value) <= MAX_I32),
      '支付金额超出可保存范围',
    ),
});

export type AssignOrderValues = z.input<typeof assignOrderSchema>;

const optionalString = z.string();

export const generateUserSchema = z
  .object({
    email_prefix: optionalString,
    email_suffix: z.string().trim().min(1, '邮箱域不能为空'),
    password: optionalString,
    plan_id: z.number().int().positive().nullable(),
    expired_at: z.string().nullable(),
    generate_count: z
      .string()
      .refine((value) => value === '' || isIntegerInput(value), '生成数量必须为数字')
      .refine(
        (value) => value === '' || (Number(value) >= 1 && Number(value) <= 500),
        '生成数量须在 1 到 500 之间',
      ),
  })
  .superRefine((values, context) => {
    const hasPrefix = values.email_prefix.trim().length > 0;
    const hasCount = values.generate_count.trim().length > 0;
    if (!hasPrefix && !hasCount) {
      context.addIssue({
        code: 'custom',
        path: ['email_prefix'],
        message: '请输入账号或生成数量',
      });
      context.addIssue({
        code: 'custom',
        path: ['generate_count'],
        message: '请输入账号或生成数量',
      });
    }
    if (hasPrefix && hasCount) {
      context.addIssue({
        code: 'custom',
        path: ['generate_count'],
        message: '单个账号与批量数量不能同时填写',
      });
    }
  });

export type GenerateUserValues = z.input<typeof generateUserSchema>;

export const sendMailSchema = z.object({
  subject: z.string().refine((value) => value.trim().length > 0, '主题不能为空'),
  content: z.string().refine((value) => value.trim().length > 0, '发送内容不能为空'),
});

export type SendMailValues = z.input<typeof sendMailSchema>;

const filterValueSchema = z
  .union([z.string(), z.number(), z.null()])
  .refine((value) => value !== '' && value != null, '请输入筛选值');

export const userFilterSchema = z.object({
  rows: z.array(
    z.object({
      key: z.string().min(1),
      condition: z.string().min(1),
      value: filterValueSchema,
    }),
  ),
});

export type UserFilterValues = z.input<typeof userFilterSchema>;

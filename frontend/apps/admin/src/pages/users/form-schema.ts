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

// Messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage at display time.
export const assignOrderSchema = z.object({
  email: z
    .string()
    .trim()
    .min(1, 'admin.users.email_required')
    .pipe(z.email('admin.users.email_invalid')),
  plan_id: z
    .number({ error: 'admin.users.plan_required' })
    .int()
    .positive('admin.users.plan_required'),
  period: z.enum(PLAN_PERIOD_VALUES, { error: 'admin.users.period_required' }),
  total_amount: z
    .string()
    .trim()
    .min(1, 'admin.users.amount_required')
    .refine(isMoneyInput, 'admin.users.amount_invalid')
    .refine(
      (value) => !isMoneyInput(value) || (Number(value) >= 0 && decimalToCents(value) <= MAX_I32),
      'admin.users.amount_out_of_range',
    ),
});

export type AssignOrderValues = z.input<typeof assignOrderSchema>;

const optionalString = z.string();

export const generateUserSchema = z
  .object({
    email_prefix: optionalString,
    email_suffix: z.string().trim().min(1, 'admin.users.email_suffix_required'),
    password: optionalString,
    plan_id: z.number().int().positive().nullable(),
    expired_at: z.string().nullable(),
    generate_count: z
      .string()
      .refine(
        (value) => value === '' || isIntegerInput(value),
        'admin.users.generate_count_numeric',
      )
      .refine(
        (value) => value === '' || (Number(value) >= 1 && Number(value) <= 500),
        'admin.users.generate_count_range',
      ),
  })
  .superRefine((values, context) => {
    const hasPrefix = values.email_prefix.trim().length > 0;
    const hasCount = values.generate_count.trim().length > 0;
    if (!hasPrefix && !hasCount) {
      context.addIssue({
        code: 'custom',
        path: ['email_prefix'],
        message: 'admin.users.prefix_or_count_required',
      });
      context.addIssue({
        code: 'custom',
        path: ['generate_count'],
        message: 'admin.users.prefix_or_count_required',
      });
    }
    if (hasPrefix && hasCount) {
      context.addIssue({
        code: 'custom',
        path: ['generate_count'],
        message: 'admin.users.prefix_count_conflict',
      });
    }
  });

export type GenerateUserValues = z.input<typeof generateUserSchema>;

export const sendMailSchema = z.object({
  subject: z
    .string()
    .refine((value) => value.trim().length > 0, 'admin.users.mail_subject_required'),
  content: z
    .string()
    .refine((value) => value.trim().length > 0, 'admin.users.mail_content_required'),
});

export type SendMailValues = z.input<typeof sendMailSchema>;

const filterValueSchema = z
  .union([z.string(), z.number(), z.null()])
  .refine((value) => value !== '' && value != null, 'admin.users.filter_value_required');

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

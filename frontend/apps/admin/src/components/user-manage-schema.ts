import { z } from 'zod';
import { decimalToCents, decimalToScaledInteger } from '@v2board/api-client';
import { BYTE_GB } from '@v2board/config/format';
import {
  isEmptyInput,
  isIntegerInput,
  isMoneyInput,
  isNumericInput,
} from '@/lib/form-input-validation';

const scalar = z.union([z.string(), z.number()]);
const nullableScalar = scalar.nullable();
// Messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage at display time.
const requiredNumeric = scalar.refine(isNumericInput, 'admin.users.numeric_invalid');
const requiredMoney = scalar
  .refine(isMoneyInput, 'admin.users.money_invalid')
  .refine(canConvertToCents, 'admin.users.money_out_of_range');
const requiredTraffic = requiredNumeric.refine(
  canConvertToBytes,
  'admin.users.traffic_out_of_range',
);
const optionalInteger = nullableScalar
  .optional()
  .refine((value) => isEmptyInput(value) || isIntegerInput(value), 'admin.users.integer_invalid');
const optionalPercentage = optionalInteger.refine(
  (value) => isEmptyInput(value) || (Number(value) >= 0 && Number(value) <= 100),
  'admin.users.percentage_range',
);

// Keep editable values in their input representation. Contract-specific unit
// conversion happens only when the valid form is submitted.
export const userManageSchema = z.object({
  email: z.email('admin.users.email_format'),
  invite_user_email: z
    .union([z.email('admin.users.email_format'), z.literal(''), z.null()])
    .optional(),
  password: z
    .string()
    .optional()
    .refine(
      (value) => value === undefined || value.trim() === '' || value.length >= 8,
      'admin.users.password_min',
    ),
  balance: requiredMoney,
  commission_balance: requiredMoney,
  transfer_enable: requiredTraffic,
  u: requiredTraffic,
  d: requiredTraffic,
  device_limit: optionalInteger,
  // The date controller owns string -> unix-seconds conversion, so the form
  // state and API payload stay honestly numeric instead of relying on a cast.
  expired_at: z.number().int().nullable().optional(),
  plan_id: z.number().int().nullable().optional(),
  banned: z.union([z.literal(0), z.literal(1)]),
  commission_type: scalar.refine(isIntegerInput, 'admin.users.commission_type_invalid'),
  commission_rate: optionalPercentage,
  discount: optionalPercentage,
  speed_limit: optionalInteger,
  is_admin: z.union([z.literal(0), z.literal(1)]),
  is_staff: z.union([z.literal(0), z.literal(1)]),
  remarks: z.string().nullable().optional(),
});

function canConvertToCents(value: unknown) {
  try {
    decimalToCents(value as string | number);
    return true;
  } catch {
    return false;
  }
}

function canConvertToBytes(value: unknown) {
  try {
    decimalToScaledInteger(value as string | number, BYTE_GB);
    return true;
  } catch {
    return false;
  }
}

export type UserManageFormValues = z.input<typeof userManageSchema>;

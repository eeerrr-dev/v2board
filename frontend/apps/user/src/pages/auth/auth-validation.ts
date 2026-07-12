import { z } from 'zod';
import { makeConfirmPasswordRefinement } from './refine-confirm-password';

export const AUTH_VALIDATION = {
  email: 'auth.email_invalid',
  password: 'auth.password_min',
  emailCode: 'auth.email_code_invalid',
  passwordMismatch: 'auth.password_mismatch',
  inviteCode: 'auth.invite_code_required',
} as const;

const emailInput = z.string().trim().min(1, AUTH_VALIDATION.email);
const emailAddress = z.email(AUTH_VALIDATION.email);
export const authEmailSchema = emailInput.pipe(emailAddress);
const characterCount = (value: string) => Array.from(value).length;
const password = z
  .string()
  .min(8, AUTH_VALIDATION.password)
  // JavaScript's string length counts UTF-16 code units; Laravel mb_strlen and
  // the Rust compatibility backend count Unicode characters.
  .refine((value) => value.length < 8 || characterCount(value) >= 8, AUTH_VALIDATION.password);
const confirmPassword = z.string().min(1, AUTH_VALIDATION.passwordMismatch);
const emailCode = z
  .string()
  .trim()
  .regex(/^\d{6}$/, AUTH_VALIDATION.emailCode);

export const loginSchema = z.object({
  email: authEmailSchema,
  // Do not trim passwords: the backend counts characters and spaces are valid.
  password,
});

export type LoginFormInput = z.input<typeof loginSchema>;
export type LoginFormValues = z.output<typeof loginSchema>;

export interface RegisterSchemaOptions {
  /** Selected server-provided suffix, or undefined when the full email is entered. */
  emailSuffix?: string;
  emailCodeRequired: boolean;
  inviteCodeRequired: boolean;
}

export function createRegisterSchema({
  emailSuffix,
  emailCodeRequired,
  inviteCodeRequired,
}: RegisterSchemaOptions) {
  const registerEmail = emailInput.refine((value) => {
    if (value.length === 0) return true;
    const address = emailSuffix === undefined ? value : `${value}@${emailSuffix}`;
    return emailAddress.safeParse(address).success;
  }, AUTH_VALIDATION.email);
  const registerEmailCode = z
    .string()
    .trim()
    .refine(
      (value) => !emailCodeRequired || emailCode.safeParse(value).success,
      AUTH_VALIDATION.emailCode,
    );
  const inviteCode = z
    .string()
    .trim()
    .optional()
    .refine((value) => !inviteCodeRequired || Boolean(value), AUTH_VALIDATION.inviteCode);

  return z
    .object({
      // Whitelist mode accepts a local-part; registerEmail checks the composed address.
      email: registerEmail,
      email_code: registerEmailCode,
      password,
      confirm_password: confirmPassword,
      invite_code: inviteCode,
    })
    .superRefine(
      makeConfirmPasswordRefinement({
        passwordKey: 'password',
        confirmKey: 'confirm_password',
        message: AUTH_VALIDATION.passwordMismatch,
      }),
    );
}

export type RegisterFormInput = z.input<ReturnType<typeof createRegisterSchema>>;
export type RegisterFormValues = z.output<ReturnType<typeof createRegisterSchema>>;

export const forgetSchema = z
  .object({
    email: emailInput.max(64, AUTH_VALIDATION.email).pipe(emailAddress),
    email_code: emailCode,
    password: password.refine((value) => characterCount(value) <= 64, AUTH_VALIDATION.password),
    confirm_password: confirmPassword.refine(
      (value) => characterCount(value) <= 64,
      AUTH_VALIDATION.password,
    ),
  })
  .superRefine(
    makeConfirmPasswordRefinement({
      passwordKey: 'password',
      confirmKey: 'confirm_password',
      message: AUTH_VALIDATION.passwordMismatch,
    }),
  );

export type ForgetFormInput = z.input<typeof forgetSchema>;
export type ForgetFormValues = z.output<typeof forgetSchema>;

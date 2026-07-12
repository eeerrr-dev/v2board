import type { z } from 'zod';

// Authored V2Board — shared confirm-password mismatch check. The register and
// forget schemas key it on `password`/`confirm_password`; the profile
// change-password schema keys it on `newPassword`/`confirmPassword`. The factory
// keeps the issue shape identical across password forms (custom code, configurable
// message id, path at the confirm field) instead of copy-pasting superRefine.
export function makeConfirmPasswordRefinement<
  PasswordKey extends string,
  ConfirmKey extends string,
>({
  passwordKey,
  confirmKey,
  message = 'password_mismatch',
}: {
  passwordKey: PasswordKey;
  confirmKey: ConfirmKey;
  message?: string;
}) {
  return (values: Record<PasswordKey | ConfirmKey, string>, context: z.RefinementCtx): void => {
    if (values[passwordKey] !== values[confirmKey]) {
      context.addIssue({
        code: 'custom',
        path: [confirmKey],
        message,
      });
    }
  };
}

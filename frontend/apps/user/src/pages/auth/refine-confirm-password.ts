import type { z } from 'zod';

// Authored V2Board — shared confirm-password check for the register and forget
// schemas. Both surfaces flag a `confirm_password` mismatch identically; keeping the
// refinement here means the message id (`password_mismatch`) and the issue shape stay
// in one place instead of being copy-pasted into each schema's superRefine.
export function refineConfirmPassword(
  values: { password: string; confirm_password: string },
  context: z.RefinementCtx,
): void {
  if (values.password !== values.confirm_password) {
    context.addIssue({
      code: 'custom',
      path: ['confirm_password'],
      message: 'password_mismatch',
    });
  }
}

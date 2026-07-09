import type { ParseKeys } from 'i18next';
import type { FieldError } from 'react-hook-form';

// Shared glue for the repeated "translate an RHF field error's message key, or
// nothing when the field is valid" expression. The resolver stashes an i18n key in
// FieldError.message; this resolves it to display text. Kept out of the generic
// FormField primitive on purpose — FormField takes already-rendered ReactNode error
// content and must not depend on i18n.
export function fieldError(
  error: FieldError | undefined,
  translate: (key: ParseKeys) => string,
): string | undefined {
  return error?.message ? translate(error.message as ParseKeys) : undefined;
}

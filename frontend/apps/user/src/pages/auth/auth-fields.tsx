import type { ComponentPropsWithRef, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button, type ButtonProps } from '@v2board/ui/button';
import { Input } from '@v2board/ui/input';
import { Label } from '@v2board/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import { LoadingState, SkeletonFields } from '@v2board/ui/loading-state';
import { Skeleton } from '@v2board/ui/skeleton';
import { cn } from '@v2board/ui/cn';
import { translateRuntimeMessage } from '@v2board/ui/translate-runtime-message';

type AuthInputProps = ComponentPropsWithRef<typeof Input>;

export function AuthFormStack({ children }: { children: ReactNode }) {
  return <div className="grid gap-6">{children}</div>;
}

interface AuthFieldProps {
  /** Stable id shared by the control (caller sets `id` on it) and the label's `htmlFor`. */
  id: string;
  label: ReactNode;
  /** Inline field error copy; rendered as a `role="alert"` message tied to the control. */
  error?: string;
  className?: string;
  /** The form control (e.g. <Input id={id} />). Props stay explicit — no cloneElement injection. */
  children: ReactNode;
}

// Explicit label/control/error field wrapper for the register-based auth island.
// The auth controllers expose only RHF `register` (no FormProvider/control), and
// the error copy is external controller state rather than a FieldError. Keep
// this register wiring explicit; Controller-driven forms use the shared Field
// primitive instead of a competing context or cloneElement abstraction.
export function AuthField({ id, label, error, className, children }: AuthFieldProps) {
  return (
    <div className={cn('grid gap-3', className)}>
      <Label htmlFor={id}>{label}</Label>
      {children}
      <AuthFieldError id={`${id}-error`} message={error} />
    </div>
  );
}

export function AuthFieldError({
  id,
  message,
  className,
}: {
  id: string;
  message?: string;
  className?: string;
}) {
  const { i18n } = useTranslation();
  if (!message) return null;
  return (
    <p id={id} role="alert" className={cn('text-sm text-destructive', className)}>
      {translateRuntimeMessage(i18n, message)}
    </p>
  );
}

export function AuthLoadingState() {
  return (
    <LoadingState className="min-h-64 space-y-5">
      <SkeletonFields fields={2} />
      <Skeleton className="h-9 w-full" aria-hidden />
    </LoadingState>
  );
}

export function AuthInlineError({ id, children }: { id: string; children: ReactNode }) {
  return (
    <Alert id={id} variant="destructive">
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

interface AuthEmailWithSuffixFieldProps {
  id: string;
  inputProps?: AuthInputProps;
  label: ReactNode;
  /** Always-present accessible name for the domain select (distinct from the email label). */
  selectLabel: string;
  suffixes: string[];
  value: string | undefined;
  onChange: (value: string) => void;
  error?: string;
}

export function AuthEmailWithSuffixField({
  id,
  inputProps,
  label,
  selectLabel,
  suffixes,
  value,
  onChange,
  error,
}: AuthEmailWithSuffixFieldProps) {
  return (
    <div className="grid gap-3">
      <Label htmlFor={id}>{label}</Label>
      <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-3">
        <Input
          id={id}
          type="text"
          autoComplete="username"
          placeholder="name"
          className="min-w-0"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? `${id}-error` : undefined}
          {...inputProps}
        />
        <Select value={value} onValueChange={onChange}>
          <SelectTrigger aria-label={selectLabel} className="max-w-40">
            <SelectValue>{value ? `@${value}` : undefined}</SelectValue>
          </SelectTrigger>
          <SelectContent>
            {suffixes.map((suffix) => (
              <SelectItem key={suffix} value={suffix}>
                @{suffix}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <AuthFieldError id={`${id}-error`} message={error} />
    </div>
  );
}

interface AuthEmailCodeFieldProps {
  id: string;
  inputProps?: AuthInputProps;
  label: ReactNode;
  buttonLabel: ReactNode;
  /** Stable accessible name while counting down, so the button is not announced as a bare number. */
  buttonAriaLabel?: string;
  disabled?: boolean;
  loading?: boolean;
  onSendCode: () => void;
  error?: string;
}

export function AuthEmailCodeField({
  id,
  inputProps,
  label,
  buttonLabel,
  buttonAriaLabel,
  disabled,
  loading,
  onSendCode,
  error,
}: AuthEmailCodeFieldProps) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-3">
      <div className="grid min-w-0 gap-3">
        <Label htmlFor={id}>{label}</Label>
        <Input
          id={id}
          type="text"
          inputMode="numeric"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? `${id}-error` : undefined}
          {...inputProps}
        />
      </div>
      <Button
        type="button"
        size="lg"
        aria-label={buttonAriaLabel}
        disabled={disabled}
        loading={loading}
        onClick={onSendCode}
        className="min-w-24 px-3"
      >
        {buttonLabel}
      </Button>
      <AuthFieldError id={`${id}-error`} message={error} className="col-span-2" />
    </div>
  );
}

interface AuthPasswordConfirmationFieldsProps {
  passwordId: string;
  passwordInputProps?: AuthInputProps;
  passwordLabel: ReactNode;
  confirmId: string;
  confirmInputProps?: AuthInputProps;
  confirmLabel: ReactNode;
  passwordError?: string;
  confirmError?: string;
}

export function AuthPasswordConfirmationFields({
  passwordId,
  passwordInputProps,
  passwordLabel,
  confirmId,
  confirmInputProps,
  confirmLabel,
  passwordError,
  confirmError,
}: AuthPasswordConfirmationFieldsProps) {
  return (
    <>
      <AuthField id={passwordId} label={passwordLabel} error={passwordError}>
        <Input
          id={passwordId}
          type="password"
          autoComplete="new-password"
          aria-invalid={passwordError ? true : undefined}
          aria-describedby={passwordError ? `${passwordId}-error` : undefined}
          {...passwordInputProps}
        />
      </AuthField>
      <AuthField id={confirmId} label={confirmLabel} error={confirmError}>
        <Input
          id={confirmId}
          type="password"
          autoComplete="new-password"
          aria-invalid={confirmError ? true : undefined}
          aria-describedby={confirmError ? `${confirmId}-error` : undefined}
          {...confirmInputProps}
        />
      </AuthField>
    </>
  );
}

export function AuthSubmitButton({ className, ...props }: ButtonProps) {
  return <Button type="submit" size="lg" block className={className} {...props} />;
}

import type { ComponentPropsWithRef, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button, type ButtonProps } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';

type AuthInputProps = ComponentPropsWithRef<typeof Input>;

export function AuthFormStack({ children }: { children: ReactNode }) {
  return <div className="grid gap-6">{children}</div>;
}

interface AuthFieldProps {
  /** Stable id shared by the control (caller sets `id` on it) and the label's `htmlFor`. */
  id: string;
  label: ReactNode;
  /** Inline field error copy; rendered as a `role="alert"` message tied to the control. */
  error?: ReactNode;
  className?: string;
  /** The form control (e.g. <Input id={id} />). Props stay explicit — no cloneElement injection. */
  children: ReactNode;
}

// Explicit label/control/error field wrapper for the register-based auth island.
// The auth controllers expose only RHF `register` (no FormProvider/control), and
// the error copy is external controller state rather than a FieldError, so the
// context-driven components/ui/form.tsx primitive cannot wire these fields; this
// keeps the wiring explicit instead of the retired cloneElement FormField.
export function AuthField({ id, label, error, className, children }: AuthFieldProps) {
  return (
    <div className={cn('grid gap-3', className)}>
      <Label htmlFor={id}>{label}</Label>
      {children}
      {error ? (
        <p id={`${id}-error`} role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </div>
  );
}

export function AuthLoadingState() {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-64 items-center justify-center" role="status">
      <Spinner className="size-5 text-muted-foreground" />
      <span className="sr-only">{t('common.loading')}</span>
    </div>
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
}

export function AuthEmailWithSuffixField({
  id,
  inputProps,
  label,
  selectLabel,
  suffixes,
  value,
  onChange,
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
          {...inputProps}
        />
        <Select
          value={value}
          onValueChange={onChange}
        >
          <SelectTrigger
            aria-label={selectLabel}
            className="max-w-40"
          >
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
}: AuthEmailCodeFieldProps) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-3">
      <AuthField id={id} label={label} className="min-w-0">
        <Input id={id} type="text" inputMode="numeric" {...inputProps} />
      </AuthField>
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
  confirmError?: ReactNode;
}

export function AuthPasswordConfirmationFields({
  passwordId,
  passwordInputProps,
  passwordLabel,
  confirmId,
  confirmInputProps,
  confirmLabel,
  confirmError,
}: AuthPasswordConfirmationFieldsProps) {
  return (
    <>
      <AuthField id={passwordId} label={passwordLabel}>
        <Input id={passwordId} type="password" autoComplete="new-password" {...passwordInputProps} />
      </AuthField>
      <AuthField id={confirmId} label={confirmLabel} error={confirmError}>
        <Input
          id={confirmId}
          type="password"
          autoComplete="new-password"
          invalid={confirmError ? true : undefined}
          aria-describedby={confirmError ? `${confirmId}-error` : undefined}
          {...confirmInputProps}
        />
      </AuthField>
    </>
  );
}

export function AuthSubmitButton({ className, ...props }: ButtonProps) {
  return (
    <Button
      type="submit"
      size="lg"
      block
      className={className}
      {...props}
    />
  );
}

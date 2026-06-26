import type { ReactNode } from 'react';
import { AlertCircle } from 'lucide-react';
import { Button, type ButtonProps } from '@/components/ui/button';
import { FormField } from '@/components/ui/form-field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select } from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { PasswordField } from './password-field';

export function AuthFormStack({ children }: { children: ReactNode }) {
  return <div className="tw:space-y-5">{children}</div>;
}

export function AuthLoadingState() {
  return (
    <div className="tw:flex tw:min-h-64 tw:items-center tw:justify-center" role="status">
      <Spinner className="tw:size-6 tw:text-primary" />
    </div>
  );
}

export function AuthInlineError({ id, children }: { id: string; children: ReactNode }) {
  return (
    <div
      id={id}
      role="alert"
      className="tw:flex tw:items-start tw:gap-2 tw:rounded-field tw:border tw:border-destructive/30 tw:bg-destructive-subtle tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-destructive"
    >
      <AlertCircle aria-hidden="true" className="tw:mt-0.5 tw:h-4 tw:w-4 tw:shrink-0" />
      <span>{children}</span>
    </div>
  );
}

interface AuthEmailWithSuffixFieldProps {
  id: string;
  label: ReactNode;
  suffixes: string[];
  value: string | undefined;
  onChange: (value: string) => void;
}

export function AuthEmailWithSuffixField({
  id,
  label,
  suffixes,
  value,
  onChange,
}: AuthEmailWithSuffixFieldProps) {
  return (
    <div className="tw:space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:gap-2">
        <Input id={id} type="text" name="email" autoComplete="username" className="tw:min-w-0" />
        <Select
          aria-label={typeof label === 'string' ? label : undefined}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="tw:max-w-40"
        >
          {suffixes.map((suffix) => (
            <option key={suffix} value={suffix}>
              @{suffix}
            </option>
          ))}
        </Select>
      </div>
    </div>
  );
}

interface AuthEmailCodeFieldProps {
  id: string;
  label: ReactNode;
  buttonLabel: ReactNode;
  disabled?: boolean;
  loading?: boolean;
  onSendCode: () => void;
}

export function AuthEmailCodeField({
  id,
  label,
  buttonLabel,
  disabled,
  loading,
  onSendCode,
}: AuthEmailCodeFieldProps) {
  return (
    <div className="tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:items-end tw:gap-2">
      <FormField id={id} label={label} className="tw:min-w-0">
        <Input type="text" name="email_code" inputMode="numeric" />
      </FormField>
      <Button
        type="button"
        disabled={disabled}
        loading={loading}
        onClick={onSendCode}
        className="tw:min-w-20 tw:px-3"
      >
        {buttonLabel}
      </Button>
    </div>
  );
}

interface AuthPasswordConfirmationFieldsProps {
  passwordId: string;
  passwordLabel: ReactNode;
  confirmId: string;
  confirmLabel: ReactNode;
}

export function AuthPasswordConfirmationFields({
  passwordId,
  passwordLabel,
  confirmId,
  confirmLabel,
}: AuthPasswordConfirmationFieldsProps) {
  return (
    <>
      <FormField id={passwordId} label={passwordLabel}>
        <PasswordField name="password" autoComplete="new-password" />
      </FormField>
      <FormField id={confirmId} label={confirmLabel}>
        <PasswordField name="confirm_password" autoComplete="new-password" />
      </FormField>
    </>
  );
}

export function AuthSubmitButton({ className, ...props }: ButtonProps) {
  return (
    <Button
      type="submit"
      size="lg"
      block
      className={cn('tw:ring-offset-surface', className)}
      {...props}
    />
  );
}

import type { ComponentPropsWithRef, ReactNode } from 'react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button, type ButtonProps } from '@/components/ui/button';
import { FormField } from '@/components/ui/form-field';
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
import { PasswordField } from './password-field';

type AuthInputProps = ComponentPropsWithRef<typeof Input>;

export function AuthFormStack({ children }: { children: ReactNode }) {
  return <div className="grid gap-6">{children}</div>;
}

export function AuthLoadingState() {
  return (
    <div className="flex min-h-64 items-center justify-center" role="status">
      <Spinner className="size-5 text-muted-foreground" />
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
  suffixes: string[];
  value: string | undefined;
  onChange: (value: string) => void;
}

export function AuthEmailWithSuffixField({
  id,
  inputProps,
  label,
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
            aria-label={typeof label === 'string' ? label : undefined}
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
  disabled?: boolean;
  loading?: boolean;
  onSendCode: () => void;
}

export function AuthEmailCodeField({
  id,
  inputProps,
  label,
  buttonLabel,
  disabled,
  loading,
  onSendCode,
}: AuthEmailCodeFieldProps) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-3">
      <FormField id={id} label={label} className="min-w-0">
        <Input type="text" inputMode="numeric" {...inputProps} />
      </FormField>
      <Button
        type="button"
        size="lg"
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
}

export function AuthPasswordConfirmationFields({
  passwordId,
  passwordInputProps,
  passwordLabel,
  confirmId,
  confirmInputProps,
  confirmLabel,
}: AuthPasswordConfirmationFieldsProps) {
  return (
    <>
      <FormField id={passwordId} label={passwordLabel}>
        <PasswordField autoComplete="new-password" {...passwordInputProps} />
      </FormField>
      <FormField id={confirmId} label={confirmLabel}>
        <PasswordField autoComplete="new-password" {...confirmInputProps} />
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
      className={className}
      {...props}
    />
  );
}

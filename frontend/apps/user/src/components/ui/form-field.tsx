import { cloneElement, type ReactElement, type ReactNode } from 'react';
import { cn } from '@/lib/cn';
import { Label } from './label';

type FieldControlProps = {
  id?: string;
  invalid?: boolean;
  'aria-describedby'?: string;
};

export interface FormFieldProps {
  /** Stable id wired onto the control; also seeds the description/error element ids. */
  id: string;
  label: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  /** The single form control (e.g. <Input/>) — id, invalid, and aria-describedby are injected. */
  children: ReactElement;
  className?: string;
}

// Authored V2Board — clean-modern reskin primitive. Pairs a Label with one control plus optional
// description/error text, wiring id + aria-describedby + the invalid state onto the control so
// callers never hand-thread accessibility ids. Reused across the auth surfaces.
export function FormField({ id, label, description, error, children, className }: FormFieldProps) {
  const childProps = children.props as FieldControlProps;
  const descriptionId = description ? `${id}-description` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy =
    [descriptionId, errorId, childProps['aria-describedby']].filter(Boolean).join(' ') || undefined;

  return (
    <div className={cn('tw:space-y-1.5', className)}>
      <Label htmlFor={id}>{label}</Label>
      {cloneElement(children, {
        // FormField owns the id so the label association always matches the control.
        id,
        invalid: error ? true : childProps.invalid,
        'aria-describedby': describedBy,
      } as FieldControlProps)}
      {description ? (
        <p id={descriptionId} className="tw:text-sm tw:text-foreground-muted">
          {description}
        </p>
      ) : null}
      {error ? (
        <p id={errorId} role="alert" className="tw:text-sm tw:text-destructive">
          {error}
        </p>
      ) : null}
    </div>
  );
}

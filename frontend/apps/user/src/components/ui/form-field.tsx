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
  children: ReactElement<FieldControlProps>;
  className?: string;
}

export function FormField({ id, label, description, error, children, className }: FormFieldProps) {
  const childProps = children.props;
  const descriptionId = description ? `${id}-description` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy =
    [descriptionId, errorId, childProps['aria-describedby']].filter(Boolean).join(' ') || undefined;

  return (
    <div className={cn('grid gap-3', className)}>
      <Label htmlFor={id}>{label}</Label>
      {cloneElement(children, {
        // FormField owns the id so the label association always matches the control.
        id,
        invalid: error ? true : childProps.invalid,
        'aria-describedby': describedBy,
      })}
      {description ? (
        <p id={descriptionId} className="text-sm text-muted-foreground">
          {description}
        </p>
      ) : null}
      {error ? (
        <p id={errorId} role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </div>
  );
}

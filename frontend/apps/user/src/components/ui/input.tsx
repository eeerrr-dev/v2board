import { forwardRef, type InputHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Renders the error treatment (destructive border/ring) and sets aria-invalid. */
  invalid?: boolean;
}

// Authored V2Board — clean-modern reskin primitive. Token-driven, `tw:`-prefixed.
export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, type = 'text', invalid, ...props }, ref) => (
    <input
      ref={ref}
      type={type}
      aria-invalid={invalid || undefined}
      className={cn(
        'tw:block tw:h-10 tw:w-full tw:rounded-field tw:border tw:bg-surface tw:px-3.5 tw:text-sm tw:text-foreground tw:shadow-sm tw:outline-none tw:transition',
        'tw:placeholder:text-foreground-muted',
        'tw:disabled:cursor-not-allowed tw:disabled:opacity-60',
        invalid
          ? 'tw:border-destructive tw:focus-visible:border-destructive tw:focus-visible:ring-2 tw:focus-visible:ring-destructive/25'
          : 'tw:border-input tw:focus-visible:border-primary tw:focus-visible:ring-2 tw:focus-visible:ring-ring/25',
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = 'Input';

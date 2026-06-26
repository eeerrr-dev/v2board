import { forwardRef, type SelectHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  /** Renders the error treatment (destructive border/ring) and sets aria-invalid. */
  invalid?: boolean;
}

// Authored V2Board — clean-modern reskin primitive. A native <select> wearing the same token-driven
// surface/border/shadow/focus treatment as <Input>, so composed rows (e.g. the register email +
// domain row) stay visually consistent instead of hand-copying the input class string.
export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, invalid, ...props }, ref) => (
    <select
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(
        'tw:h-10 tw:rounded-field tw:border tw:bg-surface tw:px-3 tw:text-sm tw:text-foreground tw:shadow-sm tw:outline-none tw:transition',
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
Select.displayName = 'Select';

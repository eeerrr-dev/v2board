import { forwardRef, type InputHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

// Authored V2Board — clean-modern reskin primitive. Token-driven, `tw:`-prefixed.
export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, type = 'text', ...props }, ref) => (
    <input
      ref={ref}
      type={type}
      className={cn(
        'tw:block tw:w-full tw:rounded-field tw:border tw:border-input tw:bg-surface tw:px-3.5 tw:py-2.5 tw:text-sm tw:text-foreground tw:shadow-sm tw:outline-none tw:transition',
        'tw:placeholder:text-muted-foreground tw:focus:border-primary tw:focus:ring-2 tw:focus:ring-ring/25',
        'tw:disabled:cursor-not-allowed tw:disabled:opacity-60',
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = 'Input';

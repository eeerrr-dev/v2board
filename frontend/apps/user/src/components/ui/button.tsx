import { cva, type VariantProps } from 'class-variance-authority';
import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cn } from '@/lib/cn';
import { Spinner } from './spinner';

// Authored V2Board — clean-modern reskin primitive. Token-driven (see @v2board/tokens);
// all Tailwind utilities carry the `tw:` prefix so they never collide with vendored legacy CSS.
const buttonVariants = cva(
  'tw:inline-flex tw:items-center tw:justify-center tw:gap-2 tw:rounded-field tw:font-semibold tw:transition tw:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40 tw:focus-visible:ring-offset-2 tw:motion-safe:active:scale-[0.99] tw:disabled:cursor-not-allowed tw:disabled:opacity-60 tw:disabled:active:scale-100',
  {
    variants: {
      variant: {
        primary: 'tw:bg-primary tw:text-primary-foreground tw:shadow-sm tw:hover:bg-primary-hover',
        secondary: 'tw:bg-muted tw:text-foreground tw:hover:bg-primary-subtle',
        outline: 'tw:border tw:border-input tw:bg-surface tw:text-foreground tw:hover:bg-muted',
        ghost: 'tw:text-foreground tw:hover:bg-muted',
      },
      size: {
        sm: 'tw:h-9 tw:px-3 tw:text-sm',
        md: 'tw:h-10 tw:px-4 tw:text-sm',
        lg: 'tw:h-11 tw:px-5 tw:text-base',
      },
      block: { true: 'tw:w-full' },
    },
    defaultVariants: { variant: 'primary', size: 'md' },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, block, loading, disabled, type, children, ...props }, ref) => (
    <button
      ref={ref}
      type={type ?? 'button'}
      className={cn(buttonVariants({ variant, size, block }), className)}
      disabled={disabled ?? loading}
      aria-busy={!!loading}
      {...props}
    >
      {loading ? <Spinner /> : null}
      {children}
    </button>
  ),
);
Button.displayName = 'Button';

export { buttonVariants };

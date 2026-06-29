import { cva, type VariantProps } from 'class-variance-authority';
import { Slot } from '@radix-ui/react-slot';
import { type ButtonHTMLAttributes, type Ref } from 'react';
import { cn } from '@/lib/cn';
import { Spinner } from './spinner';

const buttonVariants = cva(
  'inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium outline-none transition-all focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-[3px] aria-invalid:ring-destructive/20 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-4',
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground shadow-xs hover:bg-primary/90',
        destructive: 'bg-destructive text-white shadow-xs hover:bg-destructive/90',
        secondary: 'bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80',
        outline: 'border border-border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground',
        ghost: 'hover:bg-accent hover:text-accent-foreground',
        link: 'text-primary underline-offset-4 hover:underline',
      },
      size: {
        sm: 'h-8 gap-1.5 rounded-md px-3',
        md: 'h-9 px-4 py-2',
        lg: 'h-10 rounded-md px-6',
        icon: 'size-9',
      },
      block: { true: 'w-full' },
    },
    defaultVariants: { variant: 'default', size: 'md' },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  loading?: boolean;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  asChild,
  className,
  variant,
  size,
  block,
  loading,
  disabled,
  type,
  children,
  onClick,
  tabIndex,
  ref,
  ...props
}: ButtonProps) {
  const Comp = asChild ? Slot : 'button';
  const isDisabled = disabled || loading;
  const sharedProps = {
    ref,
    'data-slot': 'button',
    className: cn(buttonVariants({ variant, size, block }), className),
    'aria-busy': !!loading,
    ...props,
  };

  if (asChild) {
    return (
      <Comp
        {...sharedProps}
        aria-disabled={isDisabled || props['aria-disabled']}
        data-disabled={isDisabled ? '' : undefined}
        tabIndex={isDisabled ? -1 : tabIndex}
        onClick={(event) => {
          if (isDisabled) {
            event.preventDefault();
            event.stopPropagation();
            return;
          }
          onClick?.(event);
        }}
      >
        {children}
      </Comp>
    );
  }

  return (
    <Comp
      {...sharedProps}
      disabled={isDisabled}
      type={type ?? 'button'}
      onClick={onClick}
      tabIndex={tabIndex}
    >
      {loading ? <Spinner /> : null}
      {children}
    </Comp>
  );
}

export { buttonVariants };

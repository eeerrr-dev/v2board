import { forwardRef, type HTMLAttributes } from 'react';
import { cn } from '@/lib/cn';

// Authored V2Board — clean-modern reskin primitive. Token-driven surface/elevation/radius.
export const Card = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        'tw:overflow-hidden tw:rounded-card tw:bg-surface tw:shadow-card tw:ring-1 tw:ring-slate-900/5',
        className,
      )}
      {...props}
    />
  ),
);
Card.displayName = 'Card';

export const CardBody = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn('tw:px-6 tw:py-9 tw:sm:px-9', className)} {...props} />
  ),
);
CardBody.displayName = 'CardBody';

export const CardFooter = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        'tw:flex tw:items-center tw:gap-3 tw:border-t tw:border-border tw:bg-muted/60 tw:px-6 tw:py-4 tw:text-sm tw:sm:px-9',
        className,
      )}
      {...props}
    />
  ),
);
CardFooter.displayName = 'CardFooter';

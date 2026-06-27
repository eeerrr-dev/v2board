import * as ProgressPrimitive from '@radix-ui/react-progress';
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from 'react';
import { cn } from '@/lib/cn';

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined;
};

interface ProgressProps extends ComponentPropsWithoutRef<typeof ProgressPrimitive.Root> {
  indicatorClassName?: string;
  indicatorProps?: ComponentPropsWithoutRef<typeof ProgressPrimitive.Indicator> & DataAttributes;
}

const Progress = forwardRef<ElementRef<typeof ProgressPrimitive.Root>, ProgressProps>(
  ({ className, indicatorClassName, indicatorProps, value, ...props }, ref) => {
    const safeValue = typeof value === 'number' && Number.isFinite(value)
      ? Math.max(0, Math.min(100, value))
      : 0;

    return (
      <ProgressPrimitive.Root
        ref={ref}
        className={cn('relative h-2 w-full overflow-hidden rounded-full bg-muted', className)}
        value={safeValue}
        {...props}
      >
        <ProgressPrimitive.Indicator
          {...indicatorProps}
          className={cn('h-full w-full flex-1 rounded-full bg-primary transition-all', indicatorClassName)}
          style={{
            ...indicatorProps?.style,
            transform: `translateX(-${100 - safeValue}%)`,
          }}
        />
      </ProgressPrimitive.Root>
    );
  },
);
Progress.displayName = ProgressPrimitive.Root.displayName;

export { Progress };

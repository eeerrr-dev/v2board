import { Progress as ProgressPrimitive } from 'radix-ui';
import { type ComponentProps, type ComponentPropsWithoutRef } from 'react';
import { cn } from '@v2board/ui/cn';

type DataAttributes = {
  [key: `data-${string}`]: string | number | boolean | undefined;
};

interface ProgressProps extends ComponentProps<typeof ProgressPrimitive.Root> {
  indicatorClassName?: string;
  indicatorProps?: ComponentPropsWithoutRef<typeof ProgressPrimitive.Indicator> & DataAttributes;
}

function Progress({
  className,
  indicatorClassName,
  indicatorProps,
  value,
  ...props
}: ProgressProps) {
  const safeValue =
    typeof value === 'number' && Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 0;

  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      className={cn('relative h-2 w-full overflow-hidden rounded-full bg-muted', className)}
      value={safeValue}
      {...props}
    >
      <ProgressPrimitive.Indicator
        {...indicatorProps}
        className={cn(
          'h-full w-full rounded-full bg-primary transition-transform',
          indicatorClassName,
        )}
        style={{
          ...indicatorProps?.style,
          transform: `translateX(-${100 - safeValue}%)`,
        }}
      />
    </ProgressPrimitive.Root>
  );
}

export { Progress };

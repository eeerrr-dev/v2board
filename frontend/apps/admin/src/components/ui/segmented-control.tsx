import { RadioGroup as RadioGroupPrimitive } from 'radix-ui';
import type { ComponentPropsWithoutRef, ReactNode } from 'react';
import { cn } from '@/lib/cn';

interface SegmentedControlItem<T extends string> {
  label: ReactNode;
  value: T;
}

interface SegmentedControlProps<T extends string> extends Omit<
  ComponentPropsWithoutRef<typeof RadioGroupPrimitive.Root>,
  'defaultValue' | 'onChange' | 'onValueChange' | 'value' | 'children'
> {
  items: SegmentedControlItem<T>[];
  onValueChange: (value: T) => void;
  value: T;
}

export function SegmentedControl<T extends string>({
  className,
  items,
  onValueChange,
  value,
  ...props
}: SegmentedControlProps<T>) {
  return (
    <RadioGroupPrimitive.Root
      {...props}
      data-slot="segmented-control"
      className={cn(
        'inline-flex h-10 w-fit items-center justify-center gap-0 rounded-lg border border-border bg-muted p-1 text-muted-foreground shadow-xs',
        className,
      )}
      value={value}
      onValueChange={(nextValue) => onValueChange(nextValue as T)}
    >
      {items.map((item) => (
        <RadioGroupPrimitive.Item
          key={item.value}
          value={item.value}
          data-slot="segmented-control-item"
          className="inline-flex h-8 items-center justify-center rounded-md px-3 text-sm font-medium whitespace-nowrap transition-all hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50 data-[state=checked]:bg-background data-[state=checked]:text-foreground data-[state=checked]:shadow-xs dark:data-[state=checked]:bg-input/30"
        >
          <span>{item.label}</span>
        </RadioGroupPrimitive.Item>
      ))}
    </RadioGroupPrimitive.Root>
  );
}

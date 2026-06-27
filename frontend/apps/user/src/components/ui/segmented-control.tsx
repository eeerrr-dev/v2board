import type { ComponentPropsWithoutRef, ReactNode } from 'react';
import { cn } from '@/lib/cn';
import { Tabs, TabsList, TabsTrigger } from './tabs';

interface SegmentedControlItem<T extends string> {
  label: ReactNode;
  value: T;
}

interface SegmentedControlProps<T extends string>
  extends Omit<ComponentPropsWithoutRef<typeof Tabs>, 'defaultValue' | 'onChange' | 'onValueChange' | 'value'> {
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
    <Tabs
      {...props}
      className={cn('w-fit', className)}
      value={value}
      onValueChange={(nextValue) => onValueChange(nextValue as T)}
    >
      <TabsList>
        {items.map((item) => (
          <TabsTrigger
            key={item.value}
            value={item.value}
          >
            <span>{item.label}</span>
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}

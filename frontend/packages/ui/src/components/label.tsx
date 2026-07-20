import { Label as LabelPrimitive } from 'radix-ui';
import { type ComponentProps } from 'react';
import { cn } from '../lib/cn';

export function Label({ className, ...props }: ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn('text-sm leading-5 font-medium select-none', className)}
      {...props}
    />
  );
}

import { RadioGroup as RadioGroupPrimitive } from 'radix-ui';
import { Circle } from 'lucide-react';
import { type ComponentProps } from 'react';
import { cn } from '@v2board/ui/cn';

// Intentional divergence from the canonical shadcn radio-group (new-york-v4
// registry): the island only uses radios as full-width selectable card rows —
// the checkout period picker (pages/plans/checkout.tsx) and the order-detail
// payment options (pages/orders/detail.tsx) — so RadioGroupItem carries a
// card-row recipe instead of canonical's `aspect-square size-4` circle, and the
// indicator circle is a separately exported RadioGroupIndicator (canonical
// embeds it inside the item). Radix radio semantics are unchanged. Do not
// registry-diff this file; if a surface ever needs a plain circle radio,
// vendor the canonical recipe under another name rather than restyling these
// cards down.
function RadioGroup({ className, ...props }: ComponentProps<typeof RadioGroupPrimitive.Root>) {
  return (
    <RadioGroupPrimitive.Root
      data-slot="radio-group"
      className={cn('grid gap-3', className)}
      {...props}
    />
  );
}

function RadioGroupItem({
  className,
  children,
  ...props
}: ComponentProps<typeof RadioGroupPrimitive.Item>) {
  return (
    <RadioGroupPrimitive.Item
      data-slot="radio-group-item"
      className={cn(
        'group flex min-h-12 w-full items-center justify-between rounded-lg border border-border bg-background px-4 py-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none data-[state=checked]:border-primary data-[state=checked]:bg-accent data-[state=checked]:text-accent-foreground',
        className,
      )}
      {...props}
    >
      {children}
    </RadioGroupPrimitive.Item>
  );
}

function RadioGroupIndicator({
  className,
  ...props
}: ComponentProps<typeof RadioGroupPrimitive.Indicator>) {
  return (
    <span className="flex size-4 items-center justify-center rounded-full border border-input group-data-[state=checked]:border-primary">
      <RadioGroupPrimitive.Indicator
        data-slot="radio-group-indicator"
        className={cn('flex items-center justify-center', className)}
        {...props}
      >
        <Circle className="size-2 fill-primary text-primary" />
      </RadioGroupPrimitive.Indicator>
    </span>
  );
}

export { RadioGroup, RadioGroupIndicator, RadioGroupItem };

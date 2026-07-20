import { type ComponentProps } from 'react';
import { cn } from '../lib/cn';

// Radix-free port of the shadcn Separator (the island vendors static-behavior
// primitives without the Radix dependency — see avatar.tsx). Matches the
// canonical API: decorative separators are hidden from AT via role="none",
// semantic ones expose role="separator" with aria-orientation for vertical.
function Separator({
  className,
  orientation = 'horizontal',
  decorative = true,
  ...props
}: ComponentProps<'div'> & {
  orientation?: 'horizontal' | 'vertical';
  decorative?: boolean;
}) {
  return (
    <div
      data-slot="separator"
      data-orientation={orientation}
      role={decorative ? 'none' : 'separator'}
      aria-orientation={!decorative && orientation === 'vertical' ? 'vertical' : undefined}
      className={cn(
        'shrink-0 bg-border data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px',
        className,
      )}
      {...props}
    />
  );
}

export { Separator };

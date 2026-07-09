import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import { type ComponentProps } from 'react';
import { cn } from '@/lib/cn';

const TooltipProvider = TooltipPrimitive.Provider;
const Tooltip = TooltipPrimitive.Root;
const TooltipTrigger = TooltipPrimitive.Trigger;

function TooltipContent({
  className,
  sideOffset = 4,
  placement = 'top',
  ...props
}: Omit<ComponentProps<typeof TooltipPrimitive.Content>, 'align'> & {
  placement?: 'top' | 'topRight';
}) {
  // `placement` is the single positioning knob: 'top' centers, 'topRight'
  // end-aligns. Radix's raw `align` is intentionally not exposed, and the
  // scale-in origin comes from --radix-popper-transform-origin (see
  // user-shadcn-motion.css), so no data-placement attribute is needed.
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        side="top"
        align={placement === 'topRight' ? 'end' : 'center'}
        sideOffset={sideOffset}
        className={cn(
          'v2board-island v2board-tooltip-content v2board-radix-popover-content z-50 overflow-hidden rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground shadow-md',
          className,
        )}
        {...props}
      />
    </TooltipPrimitive.Portal>
  );
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };

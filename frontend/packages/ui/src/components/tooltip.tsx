import { Tooltip as TooltipPrimitive } from 'radix-ui';
import { type ComponentProps } from 'react';
import { cn } from '../lib/cn';

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
  // scale-in origin comes from Radix's public component variable, so no
  // data-placement attribute is needed.
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        side="top"
        align={placement === 'topRight' ? 'end' : 'center'}
        sideOffset={sideOffset}
        className={cn(
          'z-50 origin-(--radix-tooltip-content-transform-origin) animate-in overflow-hidden rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground shadow-md duration-150 fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-[state=closed]:animate-out data-[state=closed]:duration-100 data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 motion-reduce:animate-none!',
          className,
        )}
        {...props}
      />
    </TooltipPrimitive.Portal>
  );
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };

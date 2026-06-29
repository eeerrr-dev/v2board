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
  align,
  ...props
}: ComponentProps<typeof TooltipPrimitive.Content> & {
  placement?: 'top' | 'topRight';
}) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        side="top"
        align={align ?? (placement === 'topRight' ? 'end' : 'center')}
        sideOffset={sideOffset}
        data-placement={placement}
        className={cn(
          'v2board-tooltip-content v2board-radix-popover-content z-50 overflow-hidden rounded-md bg-primary px-3 py-1.5 text-xs text-primary-foreground shadow-md',
          className,
        )}
        {...props}
      />
    </TooltipPrimitive.Portal>
  );
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };

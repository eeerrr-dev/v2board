import type { ReactNode } from 'react';
import { CircleHelp } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/cn';

// Table-header label with a trailing help icon and tooltip, shared by the
// service (node/traffic) and invite tables. The `v2board-service-tooltip-trigger`
// class is selected by the interaction-parity harness — keep it. Alignment
// differs per table (node centers, traffic end-aligns, invite stays default),
// so callers pass `justify-*` via className.
function HeaderTooltip({
  children,
  className,
  placement = 'top',
  title,
}: {
  children: ReactNode;
  className?: string;
  placement?: 'top' | 'topRight';
  title: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={cn(
            'v2board-service-tooltip-trigger inline-flex cursor-help items-center gap-1',
            className,
          )}
        >
          {children}
          <CircleHelp className="size-3.5" />
        </span>
      </TooltipTrigger>
      <TooltipContent placement={placement}>{title}</TooltipContent>
    </Tooltip>
  );
}

export { HeaderTooltip };

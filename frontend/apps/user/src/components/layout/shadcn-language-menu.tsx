import { ChevronDown, Languages } from 'lucide-react';
import { cn } from '@/lib/cn';
import { LanguageMenu } from './language-menu';

interface ShadcnLanguageMenuProps {
  className?: string;
}

export function ShadcnLanguageMenu({ className }: ShadcnLanguageMenuProps) {
  return (
    <LanguageMenu
      align="center"
      side="bottom"
      contentClassName="v2board-app-shell-menu-content min-w-28"
      itemClassName="whitespace-nowrap"
      trigger={(currentLabel) => (
        <button
          type="button"
          aria-label={currentLabel ? `Language: ${currentLabel}` : 'Language'}
          className={cn(
            'inline-flex h-9 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:bg-accent data-[state=open]:text-accent-foreground',
            className,
          )}
          data-testid="app-language-trigger"
        >
          <Languages className="size-4" aria-hidden="true" />
          <span className="hidden sm:inline">{currentLabel}</span>
          <ChevronDown
            aria-hidden="true"
            className="size-3.5 opacity-70 transition-transform data-[state=open]:rotate-180"
          />
        </button>
      )}
    />
  );
}

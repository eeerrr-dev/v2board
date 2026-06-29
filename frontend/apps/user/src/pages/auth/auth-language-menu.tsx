import { ChevronDown } from 'lucide-react';
import { LanguageMenu } from '@/components/layout/language-menu';

export function AuthLanguageMenu() {
  return (
    <LanguageMenu
      align="end"
      side="bottom"
      contentClassName="v2board-auth-language-menu-content z-[1050] min-w-24"
      itemClassName="v2board-auth-language-menu-item whitespace-nowrap"
      trigger={(currentLabel) => (
        <button
          type="button"
          aria-label={currentLabel ? `Language: ${currentLabel}` : 'Language'}
          className="v2board-auth-language-trigger group inline-flex h-9 cursor-pointer items-center gap-1 rounded-md px-2 text-sm font-medium text-muted-foreground underline-offset-4 transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:text-foreground"
        >
          <span>{currentLabel}</span>
          <ChevronDown aria-hidden="true" className="size-3.5 opacity-70 transition-transform group-data-[state=open]:rotate-180" />
        </button>
      )}
    />
  );
}

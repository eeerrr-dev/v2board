import { useState } from 'react';
import { ChevronDown } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/cn';
import {
  getCurrentLocaleLabel,
  getEnabledLocales,
  selectLocale,
} from '@/lib/locale-menu';

interface ShadcnLanguageMenuProps {
  className?: string;
}

export function ShadcnLanguageMenu({ className }: ShadcnLanguageMenuProps) {
  const [open, setOpen] = useState(false);
  const locales = getEnabledLocales();
  const currentLabel = getCurrentLocaleLabel();

  return (
    <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={currentLabel ? `Language: ${currentLabel}` : 'Language'}
          className={cn(
            'v2board-app-language-trigger inline-flex h-9 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:bg-accent data-[state=open]:text-accent-foreground',
            className,
          )}
        >
          <i className="far fa fa-language" aria-hidden="true" />
          <span className="hidden sm:inline">{currentLabel}</span>
          <ChevronDown
            aria-hidden="true"
            className="size-3.5 opacity-70 transition-transform data-[state=open]:rotate-180"
          />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="center"
        side="bottom"
        sideOffset={4}
        className="v2board-app-shell-menu-content min-w-28"
      >
        {locales.map((locale) => (
          <DropdownMenuItem
            key={locale.code}
            className="whitespace-nowrap"
            onSelect={(event) => {
              event.preventDefault();
              selectLocale(locale.code);
            }}
          >
            {locale.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

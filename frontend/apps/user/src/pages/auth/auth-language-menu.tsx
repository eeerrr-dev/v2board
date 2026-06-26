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

interface AuthLanguageMenuProps {
  align?: 'center' | 'end';
  className?: string;
  placement?: 'topCenter' | 'bottomCenter';
}

export function AuthLanguageMenu({
  align = 'center',
  className,
  placement = 'topCenter',
}: AuthLanguageMenuProps) {
  const [open, setOpen] = useState(false);
  const side = placement === 'bottomCenter' ? 'bottom' : 'top';
  const locales = getEnabledLocales();
  const currentLabel = getCurrentLocaleLabel();

  return (
    <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={currentLabel ? `Language: ${currentLabel}` : 'Language'}
          className={cn(
            'v2board-auth-language-trigger',
            className,
            'group inline-flex h-9 cursor-pointer items-center gap-1 rounded-md px-2 text-sm font-medium text-muted-foreground underline-offset-4 transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:text-foreground',
          )}
        >
          <span>{currentLabel}</span>
          <ChevronDown aria-hidden="true" className="size-3.5 opacity-70 transition-transform group-data-[state=open]:rotate-180" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align={align}
        side={side}
        sideOffset={4}
        className="v2board-auth-language-menu-content z-[1050] min-w-24"
      >
        {locales.map((locale) => (
          <DropdownMenuItem
            key={locale.code}
            className="v2board-auth-language-menu-item whitespace-nowrap"
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

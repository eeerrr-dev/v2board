import { useState } from 'react';
import { Languages } from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
  getCurrentLocaleLabel,
  getEnabledLocales,
  selectLocale,
} from '@/lib/locale-menu';

interface AuthLanguageMenuProps {
  className?: string;
  placement?: 'topCenter' | 'bottomCenter';
}

export function AuthLanguageMenu({
  className = 'v2board-auth-language-trigger',
  placement = 'topCenter',
}: AuthLanguageMenuProps) {
  const [open, setOpen] = useState(false);
  const side = placement === 'bottomCenter' ? 'bottom' : 'top';
  const locales = getEnabledLocales();
  const currentLabel = getCurrentLocaleLabel();

  return (
    <DropdownMenu.Root open={open} onOpenChange={setOpen} modal={false}>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          className={`${className} tw:inline-flex tw:cursor-pointer tw:items-center tw:gap-1.5 tw:rounded-field tw:border-0 tw:bg-transparent tw:px-2 tw:py-1 tw:text-sm tw:text-foreground-muted tw:transition tw:hover:bg-muted tw:hover:text-foreground tw:focus-visible:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring/40`}
        >
          <Languages aria-hidden="true" className="tw:h-4 tw:w-4" />
          <span>{currentLabel}</span>
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="center"
          side={side}
          sideOffset={4}
          className="v2board-auth-language-menu-content tw:z-[1050] tw:min-w-36 tw:rounded-card tw:border tw:border-border tw:bg-surface tw:p-1 tw:text-sm tw:text-foreground tw:shadow-card tw:outline-none"
        >
          {locales.map((locale) => (
            <DropdownMenu.Item
              key={locale.code}
              className="v2board-auth-language-menu-item tw:cursor-pointer tw:rounded-field tw:px-3 tw:py-2 tw:outline-none tw:transition tw:hover:bg-muted tw:focus:bg-muted"
              onSelect={(event) => {
                event.preventDefault();
                selectLocale(locale.code);
              }}
            >
              {locale.label}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

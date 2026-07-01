import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  getCurrentLocaleLabel,
  getEnabledLocales,
  selectLocale,
} from '@/lib/locale-menu';

interface LanguageMenuProps {
  /** Renders the trigger button; receives the current locale label for chrome. */
  trigger: (currentLabel: string | undefined) => ReactNode;
  align: 'center' | 'end';
  side: 'top' | 'bottom';
  contentClassName?: string;
  itemClassName?: string;
}

// Authored V2Board — shared language switcher behavior. Both the auth surface and the logged-in
// app shell render the identical Radix DropdownMenu (modal={false}) over the enabled locales, with
// each item persisting the locale through selectLocale. Only the trigger chrome and the
// align/side/className wiring differ, so those stay caller-owned via props.
export function LanguageMenu({
  trigger,
  align,
  side,
  contentClassName,
  itemClassName,
}: LanguageMenuProps) {
  const { i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const locales = getEnabledLocales();
  const currentLabel = getCurrentLocaleLabel();

  return (
    <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
      <DropdownMenuTrigger asChild>{trigger(currentLabel)}</DropdownMenuTrigger>
      <DropdownMenuContent
        align={align}
        side={side}
        sideOffset={4}
        className={contentClassName}
      >
        {locales.map((locale) => (
          <DropdownMenuItem
            key={locale.code}
            className={itemClassName}
            onSelect={(event) => {
              event.preventDefault();
              selectLocale(locale.code);
              void i18n.changeLanguage(locale.code);
            }}
          >
            {locale.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

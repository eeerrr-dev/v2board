import { useState, type ReactNode } from 'react';
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
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
  /**
   * Mark the active locale with a check. The shell trigger is icon-only, so the
   * current language is no longer shown as trigger text and the menu carries it
   * instead. Opt-in so the auth trigger (which still renders its label) keeps its
   * plain item markup. Adds no item text, so exact-textContent assertions hold.
   */
  activeIndicator?: boolean;
}

// Authored V2Board — shared language switcher items. The auth surface renders them
// in its own dropdown (LanguageMenu below); the logged-in shell nests them in the
// account menu's Language submenu. Each item persists the locale through
// selectLocale, and preventDefault keeps the menu open so the check mark's move
// to the picked locale is visible.
export function LanguageMenuItems({
  itemClassName,
  activeIndicator,
}: Pick<LanguageMenuProps, 'itemClassName' | 'activeIndicator'>) {
  const { i18n } = useTranslation();
  const locales = getEnabledLocales();
  const currentLabel = getCurrentLocaleLabel();

  return (
    <>
      {locales.map((locale) => (
        <DropdownMenuItem
          key={locale.code}
          className={cn(activeIndicator && 'justify-between gap-4', itemClassName)}
          onSelect={(event) => {
            event.preventDefault();
            selectLocale(locale.code);
            void i18n.changeLanguage(locale.code);
          }}
        >
          {locale.label}
          {activeIndicator ? (
            <Check
              aria-hidden="true"
              className={cn('size-4', locale.label === currentLabel ? 'opacity-100' : 'opacity-0')}
            />
          ) : null}
        </DropdownMenuItem>
      ))}
    </>
  );
}

// Standalone dropdown chrome around the shared items, still used by the auth
// surface. Only the trigger chrome and the align/side/className wiring are
// caller-owned via props.
export function LanguageMenu({
  trigger,
  align,
  side,
  contentClassName,
  itemClassName,
  activeIndicator,
}: LanguageMenuProps) {
  const [open, setOpen] = useState(false);
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
        <LanguageMenuItems itemClassName={itemClassName} activeIndicator={activeIndicator} />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

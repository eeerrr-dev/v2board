import { ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { LanguageMenu } from '@/components/layout/language-menu';

export function AuthLanguageMenu() {
  const { t } = useTranslation();
  return (
    <LanguageMenu
      align="end"
      side="bottom"
      contentClassName="min-w-24"
      itemClassName="whitespace-nowrap"
      trigger={(currentLabel) => (
        <button
          type="button"
          data-testid="auth-language-trigger"
          aria-label={
            currentLabel ? `${t($ => $.common.language)}: ${currentLabel}` : t($ => $.common.language)
          }
          className="group inline-flex h-9 cursor-pointer items-center gap-1 rounded-md px-2 text-sm font-medium text-foreground underline-offset-4 transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:bg-accent"
        >
          <span>{currentLabel}</span>
          <ChevronDown
            aria-hidden="true"
            className="size-3.5 opacity-70 transition-transform group-data-[state=open]:rotate-180"
          />
        </button>
      )}
    />
  );
}

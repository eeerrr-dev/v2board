import { Languages } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { LanguageMenu } from './language-menu';

interface ShadcnLanguageMenuProps {
  className?: string;
}

export function ShadcnLanguageMenu({ className }: ShadcnLanguageMenuProps) {
  const { t } = useTranslation();
  return (
    <LanguageMenu
      align="end"
      side="bottom"
      contentClassName="v2board-island v2board-app-shell-menu-content min-w-40"
      itemClassName="whitespace-nowrap"
      activeIndicator
      trigger={(currentLabel) => (
        <button
          type="button"
          aria-label={currentLabel ? `${t('common.language')}: ${currentLabel}` : t('common.language')}
          className={cn(
            'inline-flex size-9 shrink-0 items-center justify-center rounded-md text-muted-foreground outline-none transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:bg-accent data-[state=open]:text-accent-foreground',
            className,
          )}
          data-testid="app-language-trigger"
        >
          <Languages className="size-4" aria-hidden="true" />
        </button>
      )}
    />
  );
}

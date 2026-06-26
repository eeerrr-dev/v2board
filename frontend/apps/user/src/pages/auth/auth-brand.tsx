import { getLegacyTitle } from '@/lib/legacy-settings';
import { cn } from '@/lib/cn';

export function AuthPanelBrand({ className }: { className?: string }) {
  const title = getLegacyTitle();

  return (
    <div
      className={cn(
        'v2board-auth-shell-brand text-lg font-semibold leading-none text-foreground',
        className,
      )}
    >
      {title}
    </div>
  );
}

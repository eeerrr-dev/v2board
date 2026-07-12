import { getLogoUrl, getSiteTitle } from '@/lib/runtime-config';
import { cn } from '@/lib/cn';

export function AuthPanelBrand({ className }: { className?: string }) {
  const title = getSiteTitle();
  const logo = getLogoUrl();

  return (
    <div
      className={cn(
        'flex min-w-0 items-center gap-2.5 text-lg font-semibold leading-none text-foreground',
        className,
      )}
    >
      {logo ? (
        <img
          src={logo}
          alt=""
          aria-hidden="true"
          decoding="async"
          className="h-8 max-w-36 shrink-0 object-contain"
        />
      ) : null}
      <span className="truncate">{title}</span>
    </div>
  );
}

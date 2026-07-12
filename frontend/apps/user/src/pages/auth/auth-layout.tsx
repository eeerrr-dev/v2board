import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { getBackgroundUrl } from '@/lib/runtime-config';
import { AuthPanelBrand } from './auth-brand';
import { AuthLanguageMenu } from './auth-language-menu';

export function AuthLayout() {
  const backgroundUrl = getBackgroundUrl();

  return (
    <div id="page-container">
      <main
        id="main-container"
        data-testid="auth-surface"
        className="relative min-h-svh overflow-hidden bg-muted text-foreground"
      >
        {backgroundUrl ? (
          <div className="pointer-events-none absolute inset-0" aria-hidden="true">
            <img
              src={backgroundUrl}
              alt=""
              decoding="async"
              fetchPriority="high"
              className="size-full object-cover"
            />
            <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px] dark:bg-background/90" />
          </div>
        ) : null}
        <header className="absolute inset-x-0 top-0 z-20 flex h-16 items-center justify-between px-5 sm:h-20 sm:px-8 lg:px-10">
          <AuthPanelBrand />
          <AuthLanguageMenu />
        </header>
        <div className="relative z-10 flex min-h-svh flex-col items-center justify-center px-5 py-24 sm:px-6 lg:px-10">
          <div data-slot="auth-route-frame" className="w-full">
            <RouteBoundaryOutlet />
          </div>
        </div>
      </main>
    </div>
  );
}

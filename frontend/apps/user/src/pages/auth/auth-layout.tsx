import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { AuthPanelBrand } from './auth-brand';
import { AuthLanguageMenu } from './auth-language-menu';

export function AuthLayout() {
  return (
    <div id="page-container">
      <main
        id="main-container"
        className="v2board-auth-surface relative min-h-svh overflow-hidden bg-muted text-foreground"
      >
        <header className="absolute inset-x-0 top-0 z-10 flex h-16 items-center justify-between px-5 sm:h-20 sm:px-8 lg:px-10">
          <AuthPanelBrand />
          <AuthLanguageMenu />
        </header>
        <div className="flex min-h-svh flex-col items-center justify-center px-5 py-24 sm:px-6 lg:px-10">
          <div className="v2board-auth-frame w-full">
            <RouteBoundaryOutlet />
          </div>
        </div>
      </main>
    </div>
  );
}

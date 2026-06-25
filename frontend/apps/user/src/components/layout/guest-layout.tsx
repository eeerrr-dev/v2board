import { useLocation } from 'react-router-dom';
import { getLegacySettings } from '@/lib/legacy-settings';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';

export function GuestLayout() {
  const { pathname } = useLocation();
  const backgroundUrl = getLegacySettings().background_url;
  const legacyBackgroundImage = backgroundUrl ? `url(${backgroundUrl})` : undefined;
  const isModernAuthSurface =
    pathname === '/login' || pathname === '/register' || pathname === '/forgetpassword';

  // The redesigned auth surfaces use the same route-isolated 2026 chrome. The single
  // `.v2board-auth-box` is kept so behavior gates can keep auth-box-scoped selectors stable; the
  // page component owns all controls, links, and headings inside it.
  if (isModernAuthSurface) {
    return (
      <div id="page-container">
        {/* `v2board-auth-surface` scopes the authored 2026 presentation and native dark theme to
            redesigned auth — see styles/user-auth-surface.css. */}
        <main id="main-container" className="v2board-auth-surface">
          <div className="tw:fixed tw:inset-0 tw:-z-10 tw:bg-gradient-to-br tw:from-background tw:to-primary-subtle" />
          <div className="v2board-auth-box tw:p-4 tw:sm:p-6">
            <div className="v2board-auth-frame tw:m-auto tw:w-full tw:max-w-md">
              <RouteBoundaryOutlet />
            </div>
          </div>
        </main>
      </div>
    );
  }

  return (
    <div id="page-container">
      <main id="main-container">
        <div
          className="v2board-background"
          style={{
            backgroundImage: legacyBackgroundImage,
          }}
        />
        <div className="no-gutters v2board-auth-box">
          <div
            style={{ maxWidth: 450, width: '100%', margin: 'auto' }}
          >
            <div className="mx-2 mx-sm-0">
              <RouteBoundaryOutlet />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

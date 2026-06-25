import { useLocation } from 'react-router-dom';
import { getLegacySettings } from '@/lib/legacy-settings';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';

export function GuestLayout() {
  const { pathname } = useLocation();
  const backgroundUrl = getLegacySettings().background_url;
  const legacyBackgroundImage = (backgroundUrl && `url(${backgroundUrl})`) as string;

  // /login is the redesigned 2026 surface (route-isolated reskin): a modern gradient backdrop
  // with the polished login card centered. Its single `.v2board-auth-box` is kept so the
  // behavior gate's auth-box-scoped selectors (controls/links/title/authBoxCount in
  // user-home-root-page-state) still resolve; the brand chrome adds no <button>/.btn and no
  // auth-box-internal links/headings, so that interaction stays byte-identical to the oracle.
  // Visual parity for this surface is retired (see user-login/user-home-root in visual-parity.mjs).
  if (pathname === '/login') {
    return (
      <div id="page-container">
        <main id="main-container">
          <div className="tw:fixed tw:inset-0 tw:-z-10 tw:overflow-hidden tw:bg-gradient-to-br tw:from-background tw:to-muted">
            <div className="tw:absolute tw:-left-32 tw:-top-32 tw:h-[26rem] tw:w-[26rem] tw:rounded-full tw:bg-primary/15 tw:blur-3xl" />
            <div className="tw:absolute tw:-bottom-40 tw:-right-24 tw:h-[30rem] tw:w-[30rem] tw:rounded-full tw:bg-primary/10 tw:blur-3xl" />
          </div>
          <div className="no-gutters v2board-auth-box tw:p-4 tw:sm:p-6">
            <div className="v2board-login-frame tw:m-auto tw:w-full tw:max-w-md">
              <RouteBoundaryOutlet />
            </div>
          </div>
        </main>
      </div>
    );
  }

  // register + forgetpassword keep the packaged-oracle chrome verbatim (still under the pixel gate).
  const hasEmptyContainerClass = pathname === '/register' || pathname === '/forgetpassword';
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
            className={hasEmptyContainerClass ? '' : undefined}
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

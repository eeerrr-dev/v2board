import { Outlet, useLocation } from 'react-router-dom';
import { getLegacySettings } from '@/lib/legacy-settings';

export function GuestLayout() {
  const { pathname } = useLocation();
  const backgroundUrl = getLegacySettings().background_url;
  const hasEmptyContainerClass = pathname === '/register' || pathname === '/forgetpassword';
  return (
    <div id="page-container">
      <main id="main-container">
        <div
          className="v2board-background"
          style={{
            backgroundImage: backgroundUrl ? `url(${backgroundUrl})` : undefined,
          }}
        />
        <div className="no-gutters v2board-auth-box">
          <div
            className={hasEmptyContainerClass ? '' : undefined}
            style={{ maxWidth: 450, width: '100%', margin: 'auto' }}
          >
            <div className="mx-2 mx-sm-0">
              <Outlet />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

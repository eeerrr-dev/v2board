import { Outlet } from 'react-router-dom';
import { getLegacySettings } from '@/lib/legacy-settings';

export function GuestLayout() {
  const backgroundUrl = getLegacySettings().background_url;
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
          <div style={{ maxWidth: 450, width: '100%', margin: 'auto' }}>
            <div className="mx-2 mx-sm-0">
              <Outlet />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

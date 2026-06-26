import { RouteBoundaryOutlet } from '@/components/route-error-boundary';

export function AuthLayout() {
  return (
    <div id="page-container">
      <main id="main-container" className="v2board-auth-surface">
        <div className="v2board-auth-backdrop" />
        <div className="v2board-auth-box">
          <div className="v2board-auth-frame">
            <RouteBoundaryOutlet />
          </div>
        </div>
      </main>
    </div>
  );
}

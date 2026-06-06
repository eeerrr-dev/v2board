import { createRoot } from 'react-dom/client';
import { useEffect, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n } from '@v2board/i18n';
import {
  getNormalizedLegacyHashPath,
  installLegacyDevModuleRecovery,
  installLegacyDevWhiteScreenFallback,
  installLegacyHashRouteNormalizer,
  installLegacyWhiteScreenRecovery,
  normalizeLegacyHashRoute,
} from '@v2board/config';
import { HashRouter, Navigate, useLocation } from 'react-router-dom';

import App, { USER_LEGACY_ROUTE_PATHS } from './App';
import { LegacyConfirmProvider } from './components/legacy-confirm';
import { RouteBoundaryElement } from './components/route-error-boundary';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyLegacySettings } from './lib/legacy-settings';
import './styles/globals.css';

const legacyHashRouteOptions = {
  authenticatedFallback: '/dashboard',
  canonicalPath: '/',
  guestFallback: '/login',
  nestedPrefixes: USER_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
} as const;
const legacyRecoveryVersion = 'white-screen-recovery-22';
const legacyWhiteScreenRecoveryConfig = {
  storageKey: `v2board:white-screen-recovery:${legacyRecoveryVersion}`,
} as const;
const legacyDevModuleRecoveryConfig = {
  storageKey: `v2board:dev-module-recovery:${legacyRecoveryVersion}`,
} as const;

normalizeLegacyHashRoute(legacyHashRouteOptions);
installLegacyHashRouteNormalizer(legacyHashRouteOptions);
if (import.meta.env.DEV) {
  installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);
  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {
    ...legacyWhiteScreenRecoveryConfig,
    delay: 3000,
  });
  installLegacyDevWhiteScreenFallback({ delay: 5000 });
} else {
  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, legacyWhiteScreenRecoveryConfig);
}
applyLegacySettings();
const i18n = createI18n();
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

applyInitialDarkMode();

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

function LegacyRouteGate({ children }: { children: ReactNode }) {
  const location = useLocation();
  const current = `${location.pathname}${location.search}`;
  const normalized = getNormalizedLegacyHashPath(current, legacyHashRouteOptions);

  useEffect(() => {
    normalizeLegacyHashRoute(legacyHashRouteOptions);
  }, [location.hash, location.pathname, location.search]);

  return normalized !== current ? <Navigate to={normalized} replace /> : <>{children}</>;
}

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <RouteBoundaryElement>
          <LegacyRouteGate>
            <App />
          </LegacyRouteGate>
          <LegacyConfirmProvider />
        </RouteBoundaryElement>
      </HashRouter>
    </QueryClientProvider>
  </I18nextProvider>,
);

import { createRoot } from 'react-dom/client';
import { useEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n } from '@v2board/i18n';
import {
  getNormalizedLegacyHashPath,
  installLegacyHashRouteNormalizer,
  normalizeLegacyHashRoute,
} from '@v2board/config';
import { HashRouter, useLocation, useNavigate } from 'react-router-dom';

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

normalizeLegacyHashRoute(legacyHashRouteOptions);
installLegacyHashRouteNormalizer(legacyHashRouteOptions);
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

function LegacyRouteGuard() {
  const location = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    const current = `${location.pathname}${location.search}`;
    const normalized = getNormalizedLegacyHashPath(current, legacyHashRouteOptions);
    if (normalized !== current) navigate(normalized, { replace: true });
  }, [location.pathname, location.search, navigate]);

  return null;
}

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <RouteBoundaryElement>
          <LegacyRouteGuard />
          <App />
          <LegacyConfirmProvider />
        </RouteBoundaryElement>
      </HashRouter>
    </QueryClientProvider>
  </I18nextProvider>,
);

import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n, installLocaleDocumentEnvironment } from '@v2board/i18n';
import {
  installLegacyDevModuleRecovery,
  installLegacyDevWhiteScreenFallback,
  installLegacyHashRouteNormalizer,
  installLegacyWhiteScreenRecovery,
  normalizeLegacyHashRoute,
} from '@v2board/config';
import { RouterProvider } from 'react-router';

import { createUserRouter, USER_LEGACY_ROUTE_PATHS } from './App';
import { ConfirmDialogProvider } from './components/ui/confirm-dialog';
import { Toaster } from './components/ui/toaster';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyLegacySettings } from './lib/legacy-settings';
import './styles/globals.css';
import './styles/user-legacy-replica.css';
import './styles/user-redesigned-surfaces.css';

const legacyHashRouteOptions = {
  authenticatedFallback: '/dashboard',
  canonicalPath: '/',
  guestFallback: '/login',
  nestedPrefixes: USER_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
} as const;
const legacyRecoveryVersion = 'white-screen-recovery-37';
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
installLocaleDocumentEnvironment(i18n);
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

applyInitialDarkMode();
const router = createUserRouter(queryClient);

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <ConfirmDialogProvider />
      <Toaster />
    </QueryClientProvider>
  </I18nextProvider>,
);

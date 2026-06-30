import { lazy, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { SubscribeInfo, UserInfo } from '@v2board/types';
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
import { reportSubscribeToChat, reportUserInfoToChat, userKeys } from './lib/queries';
import './styles/globals.css';
import './styles/user-legacy-replica.css';
import './styles/user-redesigned-surfaces.css';

const legacyHashRouteOptions = {
  authenticatedFallback: '/dashboard',
  authenticatedPublicFallbackRoutes: [],
  canonicalPath: '/',
  guestFallback: '/login',
  nestedPrefixes: USER_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
} as const;
const legacyRecoveryVersion = 'white-screen-recovery-38';
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
function queryKeyEquals(a: readonly unknown[], b: readonly unknown[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

// Mirror the legacy sagas: report the user to the Tawk/Crisp live-chat widgets
// after each successful user/info and user/subscribe fetch (refetches included).
// QueryCache onSuccess is React Query v5's canonical replacement for the removed
// useQuery onSuccess and fires once per successful fetch keyed by query, so the
// queryFns stay pure. The Crisp/Tawk payloads are external-integration contracts.
const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onSuccess: (data, query) => {
      if (queryKeyEquals(query.queryKey, userKeys.info)) {
        reportUserInfoToChat(data as UserInfo);
      } else if (queryKeyEquals(query.queryKey, userKeys.subscribe)) {
        reportSubscribeToChat(data as SubscribeInfo);
      }
    },
  }),
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

applyInitialDarkMode();
const router = createUserRouter(queryClient);

// Dev-only TanStack Query devtools. The import lives inside an import.meta.env.DEV
// branch so the production deploy build (where Vite statically resolves DEV to
// false) dead-code-eliminates the dynamic import and never ships it in umi.js.
const ReactQueryDevtools = import.meta.env.DEV
  ? lazy(() =>
      import('@tanstack/react-query-devtools').then((module) => ({
        default: module.ReactQueryDevtools,
      })),
    )
  : null;

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <ConfirmDialogProvider />
      <Toaster />
      {ReactQueryDevtools ? (
        <Suspense fallback={null}>
          <ReactQueryDevtools initialIsOpen={false} />
        </Suspense>
      ) : null}
    </QueryClientProvider>
  </I18nextProvider>,
);

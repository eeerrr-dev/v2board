import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { presentMutationError, shouldRetryQuery } from '@v2board/api-client';
import { I18nextProvider } from 'react-i18next';
import { createLazyI18n, installLocaleDocumentEnvironment } from '@v2board/i18n';
import { applyLegacyHashRedirect } from '@v2board/config';
import { RouterProvider } from 'react-router/dom';

import { createUserRouter } from './App';
import { AppShellBoundary } from '@v2board/app-shell/app-shell-boundary';
import { ConfirmDialogProvider } from '@v2board/ui/confirm-dialog';
import { Toaster } from '@v2board/ui/toaster';
import { registerSessionCacheClearer, setupAuthSync } from './lib/auth';
import { installChunkReloadRecovery } from '@v2board/app-shell/chunk-recovery';
import { applyInitialDarkMode, useDarkMode } from './lib/dark-mode';
import {
  applyRuntimeConfig,
  getLegacyHashRedirectEnabled,
  getSentryDsn,
} from './lib/runtime-config';
import { i18nGet } from './lib/errors';
import { registerRouterNavigation } from './lib/router-navigation';
import { toast } from '@v2board/app-shell/toast';
import './styles/globals.css';

applyRuntimeConfig();
// A stale tab whose lazy chunks were replaced by a newer release recovers with
// one guarded reload; installed before any dynamic import can fail.
installChunkReloadRecovery();
// Error reporting is opt-in via the injected runtime config; the SDK loads
// lazily so boot never blocks on it and the chunk is never fetched when off.
const sentryDsn = getSentryDsn();
if (sentryDsn) {
  void import('@v2board/app-shell/sentry').then(({ initSentry }) => initSentry(sentryDsn));
}
const i18n = await createLazyI18n();
installLocaleDocumentEnvironment(i18n);
const queryClient = new QueryClient({
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      presentMutationError(error, mutation.meta, (message) => toast.error(message), i18nGet);
    },
  }),
  defaultOptions: {
    // Retry only transient failures (network drop, 5xx) and never
    // deterministic outcomes; the /login probe opts back out via its own
    // per-query retry: false in userQueryOptions.checkLogin.
    queries: { retry: shouldRetryQuery, refetchOnWindowFocus: false },
  },
});

// Auth teardown (logout and the 401 session-expiry handler in lib/api.ts) must
// drop cached server state so the next session on this tab cannot read the
// previous account's data (e.g. the subscribe_url credential). Registered here
// because the QueryClient lives in the entry; lib/auth never imports main.
registerSessionCacheClearer(() => queryClient.clear());
setupAuthSync();

applyInitialDarkMode();
// docs/api-dialect.md §10.3: translate a legacy `/#/x?y` entry URL into its
// history URL before router creation, gated on the injected admin toggle.
applyLegacyHashRedirect({ enabled: getLegacyHashRedirectEnabled() });
const router = createUserRouter(queryClient);
registerRouterNavigation(router);

// Dev-only TanStack Query devtools. The import lives inside an import.meta.env.DEV
// branch so the production deploy build (where Vite statically resolves DEV to
// false) dead-code-eliminates the dynamic import from the production graph.
const ReactQueryDevtools = import.meta.env.DEV
  ? lazy(() =>
      import('@tanstack/react-query-devtools').then((module) => ({
        default: module.ReactQueryDevtools,
      })),
    )
  : null;

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

function AppToaster() {
  return <Toaster theme={useDarkMode() ? 'dark' : 'light'} />;
}

createRoot(root).render(
  <StrictMode>
    <AppShellBoundary getSentryDsn={getSentryDsn}>
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
          <ConfirmDialogProvider />
          <AppToaster />
          {ReactQueryDevtools ? (
            <Suspense fallback={null}>
              <ReactQueryDevtools initialIsOpen={false} />
            </Suspense>
          ) : null}
        </QueryClientProvider>
      </I18nextProvider>
    </AppShellBoundary>
  </StrictMode>,
);

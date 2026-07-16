import { lazy, StrictMode, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { presentMutationError, shouldRetryQuery } from '@v2board/api-client';
import type { SubscribeInfo, UserInfo } from '@v2board/types';
import { I18nextProvider } from 'react-i18next';
import { createLazyI18n, installLocaleDocumentEnvironment } from '@v2board/i18n';
import { RouterProvider } from 'react-router/dom';

import { createUserRouter } from './App';
import { ConfirmDialogProvider } from './components/ui/confirm-dialog';
import { Toaster } from './components/ui/toaster';
import { registerSessionCacheClearer, setupAuthSync } from './lib/auth';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyRuntimeConfig } from './lib/runtime-config';
import { i18nGet } from './lib/errors';
import { reportSubscribeToChat, reportUserInfoToChat, userKeys } from './lib/queries';
import { registerRouterNavigation } from './lib/router-navigation';
import { toast } from './lib/toast';
import './styles/globals.css';

applyRuntimeConfig();
const i18n = await createLazyI18n();
installLocaleDocumentEnvironment(i18n);
function queryKeyEquals(a: readonly unknown[], b: readonly unknown[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

// Preserve the Tawk/Crisp integration contract: report the user to the widgets
// after each successful user/info and user/subscribe fetch (refetches included).
// QueryCache onSuccess is React Query v5's canonical replacement for the removed
// useQuery onSuccess and fires once per successful fetch keyed by query, so the
// queryFns stay pure. The Crisp/Tawk payloads are external-integration contracts.
const queryClient = new QueryClient({
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      presentMutationError(error, mutation.meta, (message) => toast.error(message), i18nGet);
    },
  }),
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
    // Retry only transient failures (network drop, 5xx) and never
    // deterministic outcomes; the /login probe opts back out via its own
    // per-query retry: false in userQueryOptions.checkLogin.
    queries: { retry: shouldRetryQuery, refetchOnWindowFocus: false },
  },
});

// Auth teardown (logout and the 403 session-expiry handler in lib/api.ts) must
// drop cached server state so the next session on this tab cannot read the
// previous account's data (e.g. the subscribe_url credential). Registered here
// because the QueryClient lives in the entry; lib/auth never imports main.
registerSessionCacheClearer(() => queryClient.clear());
setupAuthSync();

applyInitialDarkMode();
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

createRoot(root).render(
  <StrictMode>
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
    </I18nextProvider>
  </StrictMode>,
);

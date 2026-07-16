import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { presentMutationError, shouldRetryQuery } from '@v2board/api-client';
import { I18nextProvider } from 'react-i18next';
import { createLazyI18n, installLocaleDocumentEnvironment } from '@v2board/i18n';
import { RouterProvider } from 'react-router/dom';
import { createAdminRouter } from './App';
import { StepUpDialogProvider } from './components/step-up-dialog';
import { ConfirmDialogProvider } from './components/ui/confirm-dialog';
import { Toaster } from './components/ui/toaster';
import { applyAdminRuntimeConfig } from './lib/runtime-config';
import { applyInitialDarkMode } from './lib/dark-mode';
import { registerSessionCacheClearer, setupAuthSync } from './lib/auth';
import { registerRouterNavigation } from './lib/router-navigation';
import { maybePromptStepUp } from './lib/step-up';
import { toast } from './lib/toast';
import './styles/globals.css';

applyAdminRuntimeConfig();
applyInitialDarkMode();

const i18n = await createLazyI18n();
installLocaleDocumentEnvironment(i18n);

const queryClient = new QueryClient({
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      // The step-up 403 opens the re-auth dialog instead of a raw error toast.
      if (maybePromptStepUp(error)) return;
      presentMutationError(error, mutation.meta, (message) => toast.error(message));
    },
  }),
  queryCache: new QueryCache({
    onError: (error) => {
      // Sensitive admin GETs share the step-up gate; the dialog's success path
      // refetches them.
      maybePromptStepUp(error);
    },
  }),
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: shouldRetryQuery,
    },
  },
});

registerSessionCacheClearer(() => queryClient.clear());
setupAuthSync();
const router = createAdminRouter(queryClient);
registerRouterNavigation(router);

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

createRoot(root).render(
  <StrictMode>
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        <ConfirmDialogProvider />
        <StepUpDialogProvider />
        <Toaster />
      </QueryClientProvider>
    </I18nextProvider>
  </StrictMode>,
);

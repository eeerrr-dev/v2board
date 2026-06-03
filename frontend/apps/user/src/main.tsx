import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n } from '@v2board/i18n';
import { normalizeLegacyHashRoute } from '@v2board/config';
import { HashRouter } from 'react-router-dom';

import App, { USER_LEGACY_ROUTE_PATHS } from './App';
import { LegacyConfirmProvider } from './components/legacy-confirm';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyLegacySettings } from './lib/legacy-settings';
import './styles/globals.css';

normalizeLegacyHashRoute({
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  publicRoutes: ['/', '/login', '/register', '/forgetpassword'],
  routes: USER_LEGACY_ROUTE_PATHS,
});
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

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <App />
        <LegacyConfirmProvider />
      </HashRouter>
    </QueryClientProvider>
  </I18nextProvider>,
);

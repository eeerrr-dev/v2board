import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n } from '@v2board/i18n';
import { HashRouter } from 'react-router-dom';

import App from './App';
import { LegacyConfirmProvider } from './components/legacy-confirm';
import { setupAuthSync } from './lib/auth';
import { applyDarkMode } from './lib/dark-mode';
import { applyLegacySettings } from './lib/legacy-settings';
import './styles/globals.css';

const i18n = createI18n();
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

setupAuthSync();
applyLegacySettings();
applyDarkMode();

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

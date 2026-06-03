import { createRoot } from 'react-dom/client';
import { ConfigProvider, App as AntdApp, theme as antdTheme } from 'antd';
import zhCN from 'antd/locale/zh_CN';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n } from '@v2board/i18n';
import {
  installLegacyHashRouteNormalizer,
  normalizeLegacyHashRoute,
} from '@v2board/config';
import { HashRouter } from 'react-router-dom';
import App, { ADMIN_LEGACY_ROUTE_PATHS } from './App';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyAdminLegacySettings } from './lib/legacy-settings';

const legacyHashRouteOptions = {
  authenticatedFallback: '/dashboard',
  guestFallback: '/login',
  publicRoutes: ['/login'],
  routes: ADMIN_LEGACY_ROUTE_PATHS,
} as const;

normalizeLegacyHashRoute(legacyHashRouteOptions);
installLegacyHashRouteNormalizer(legacyHashRouteOptions);
applyAdminLegacySettings();
applyInitialDarkMode();

const i18n = createI18n();
const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, retry: 1, refetchOnWindowFocus: false } },
});

const root = document.getElementById('root');
if (!root) throw new Error('root element missing');

function Boot() {
  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm: antdTheme.defaultAlgorithm,
        token: {
          colorPrimary: '#0665d0',
          colorLink: '#0665d0',
          colorLinkHover: '#2a84de',
          colorLinkActive: '#004aab',
          colorText: 'rgba(0, 0, 0, 0.65)',
          colorTextHeading: 'rgba(0, 0, 0, 0.85)',
          colorTextSecondary: 'rgba(0, 0, 0, 0.45)',
          colorBgLayout: '#f0f2f5',
          colorBorderSecondary: '#f0f0f0',
          borderRadius: 4,
          fontSize: 14,
          fontFamily:
            "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif",
        },
        components: {
          Layout: {
            headerBg: '#001529',
            siderBg: '#001529',
            bodyBg: '#f0f2f5',
            headerHeight: 64,
            headerPadding: '0 24px',
          },
          Menu: {
            itemHeight: 40,
            darkItemBg: '#001529',
            darkSubMenuItemBg: '#000c17',
            darkItemSelectedBg: '#0665d0',
          },
        },
      }}
    >
      <AntdApp>
        <QueryClientProvider client={queryClient}>
          <HashRouter>
            <App />
          </HashRouter>
        </QueryClientProvider>
      </AntdApp>
    </ConfigProvider>
  );
}

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <Boot />
  </I18nextProvider>,
);

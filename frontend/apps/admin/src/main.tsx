import { createRoot } from 'react-dom/client';
import { useEffect, type ReactNode } from 'react';
import { ConfigProvider, App as AntdApp, theme as antdTheme } from 'antd';
import zhCN from 'antd/locale/zh_CN';
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
import App, { ADMIN_LEGACY_ROUTE_PATHS } from './App';
import { RouteBoundaryElement } from './components/route-error-boundary';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyAdminLegacySettings } from './lib/legacy-settings';
import './styles/antd-v5-compat.css';

const legacyHashRouteOptions = {
  authenticatedFallback: '/dashboard',
  canonicalPath: '/',
  guestFallback: '/login',
  nestedPrefixes: ADMIN_LEGACY_ROUTE_PATHS,
  publicRoutes: ['/', '/login'],
  routes: ADMIN_LEGACY_ROUTE_PATHS,
} as const;
const legacyRecoveryVersion = 'white-screen-recovery-9';
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
    delay: 1000,
  });
  installLegacyDevWhiteScreenFallback({ delay: 5000 });
} else {
  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, legacyWhiteScreenRecoveryConfig);
}
applyAdminLegacySettings();
applyInitialDarkMode();

const i18n = createI18n();
const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 0, retry: false, refetchOnWindowFocus: false } },
});

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
            <RouteBoundaryElement>
              <LegacyRouteGate>
                <App />
              </LegacyRouteGate>
            </RouteBoundaryElement>
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

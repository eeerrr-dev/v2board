import { createRoot } from 'react-dom/client';
import { useEffect, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { I18nextProvider } from 'react-i18next';
import { createI18n, installLocaleDocumentEnvironment } from '@v2board/i18n';
import {
  getNormalizedLegacyHashPath,
  installLegacyDevModuleRecovery,
  installLegacyDevWhiteScreenFallback,
  installLegacyHashRouteNormalizer,
  installLegacyWhiteScreenRecovery,
  normalizeLegacyHashRoute,
} from '@v2board/config';
import { HashRouter, Navigate, useLocation } from 'react-router-dom';

import App, { USER_LEGACY_ROUTE_PATHS } from './App';
import { LegacyConfirmProvider } from './components/legacy-confirm';
import { RouteBoundaryElement } from './components/route-error-boundary';
import { applyInitialDarkMode } from './lib/dark-mode';
import { applyLegacySettings } from './lib/legacy-settings';
import './styles/globals.css';
import './styles/user-theme-colors.css';
import './styles/user-theme-legacy-tokens.css';
import './styles/user-theme-layout-tokens.css';
import './styles/user-document-root.css';
import './styles/user-heading-base.css';
import './styles/user-heading-scale.css';
import './styles/user-heading-native-color.css';
import './styles/user-prose-elements.css';
import './styles/user-link-elements.css';
import './styles/user-custom-html-base.css';
import './styles/user-custom-html-headings.css';
import './styles/user-custom-html-inline.css';
import './styles/user-custom-html-lists.css';
import './styles/user-custom-html-divider.css';
import './styles/user-custom-html-code-block.css';
import './styles/user-custom-html-inline-code.css';
import './styles/user-custom-html-blockquote.css';
import './styles/user-custom-html-media.css';
import './styles/user-custom-html-table-shell.css';
import './styles/user-custom-html-table-cell-wrap.css';
import './styles/user-custom-html-table-rows.css';
import './styles/user-custom-html-table-header-cells.css';
import './styles/user-custom-html-table-body-cells.css';
import './styles/user-browser-modes.css';
import './styles/user-oneui-display-utilities.css';
import './styles/user-oneui-spacing-utilities.css';
import './styles/user-oneui-size-utilities.css';
import './styles/user-oneui-text-color-utilities.css';
import './styles/user-oneui-text-layout-utilities.css';
import './styles/user-oneui-border-overflow-utilities.css';
import './styles/user-oneui-font-flex-utilities.css';
import './styles/user-oneui-mobile-type.css';
import './styles/user-oneui-hero.css';
import './styles/user-oneui-responsive-lg.css';
import './styles/user-oneui-responsive-md.css';
import './styles/user-oneui-responsive-sm.css';
import './styles/user-oneui-responsive-xl.css';
import './styles/user-bootstrap-button-base.css';
import './styles/user-bootstrap-button-primary.css';
import './styles/user-bootstrap-button-dark.css';
import './styles/user-bootstrap-button-danger.css';
import './styles/user-bootstrap-button-alt-primary.css';
import './styles/user-bootstrap-button-size-sm.css';
import './styles/user-bootstrap-button-size-lg.css';
import './styles/user-bootstrap-button-rounded.css';
import './styles/user-bootstrap-button-block-disabled.css';
import './styles/user-bootstrap-grid-rows-base.css';
import './styles/user-bootstrap-grid-rows-no-gutters.css';
import './styles/user-bootstrap-grid-rows-tiny-gutters.css';
import './styles/user-bootstrap-grid-rows-mobile-gutters.css';
import './styles/user-bootstrap-grid-deck-form.css';
import './styles/user-bootstrap-grid-columns-fixed.css';
import './styles/user-bootstrap-grid-columns-full.css';
import './styles/user-bootstrap-grid-columns-sm.css';
import './styles/user-bootstrap-grid-columns-md.css';
import './styles/user-bootstrap-grid-columns-xl.css';
import './styles/user-bootstrap-alert-shell.css';
import './styles/user-bootstrap-alert-variants.css';
import './styles/user-bootstrap-alert-links.css';
import './styles/user-bootstrap-form-control-base.css';
import './styles/user-bootstrap-form-control-focus.css';
import './styles/user-bootstrap-form-control-alt.css';
import './styles/user-bootstrap-form-control-placeholder.css';
import './styles/user-bootstrap-form-control-disabled.css';
import './styles/user-bootstrap-input-group-shell.css';
import './styles/user-bootstrap-input-group-control.css';
import './styles/user-bootstrap-input-group-prepend.css';
import './styles/user-bootstrap-custom-control-base.css';
import './styles/user-bootstrap-custom-control-indicators.css';
import './styles/user-bootstrap-custom-control-states.css';
import './styles/user-bootstrap-custom-control-primary.css';
import './styles/user-bootstrap-custom-control-active-alignment.css';
import './styles/user-bootstrap-progress-shell.css';
import './styles/user-bootstrap-progress-bar.css';
import './styles/user-bootstrap-progress-striped.css';
import './styles/user-bootstrap-progress-animated.css';
import './styles/user-bootstrap-background-utilities.css';
import './styles/user-bootstrap-badges.css';
import './styles/user-oneui-block-base.css';
import './styles/user-oneui-block-rounded.css';
import './styles/user-oneui-block-variants.css';
import './styles/user-oneui-block-header.css';
import './styles/user-oneui-block-links.css';
import './styles/user-oneui-block-content-base.css';
import './styles/user-oneui-block-content-pull.css';
import './styles/user-oneui-block-content-sizing.css';
import './styles/user-oneui-content-heading.css';
import './styles/user-bootstrap-color-utilities.css';
import './styles/user-bootstrap-spinner-grow.css';
import './styles/user-bootstrap-screen-reader.css';
import './styles/user-bootstrap-spinner-grow-keyframes.css';
import './styles/user-background-utilities.css';
import './styles/auth-shadcn.css';
import './styles/user-shadcn-motion.css';
import './styles/user-auth-surface.css';

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

createRoot(root).render(
  <I18nextProvider i18n={i18n}>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <RouteBoundaryElement>
          <LegacyRouteGate>
            <App />
          </LegacyRouteGate>
          <LegacyConfirmProvider />
        </RouteBoundaryElement>
      </HashRouter>
    </QueryClientProvider>
  </I18nextProvider>,
);

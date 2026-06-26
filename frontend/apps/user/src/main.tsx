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
import './styles/user-font-face-awesome.css';
import './styles/user-font-face-simple-line-icons.css';
import './styles/user-font-awesome-base.css';
import './styles/user-font-awesome-glyphs.css';
import './styles/user-font-awesome-sizing.css';
import './styles/user-simple-line-icons-base.css';
import './styles/user-simple-line-icons-glyphs.css';
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
import './styles/user-bootstrap-button-icon-offsets.css';
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
import './styles/user-oneui-block-loading.css';
import './styles/user-bootstrap-color-utilities.css';
import './styles/user-bootstrap-spinner-grow.css';
import './styles/user-bootstrap-screen-reader.css';
import './styles/user-bootstrap-spinner-grow-keyframes.css';
import './styles/user-font-awesome-spin-keyframes.css';
import './styles/user-antd-table-wrapper.css';
import './styles/user-antd-table-base.css';
import './styles/user-antd-table-body-shell.css';
import './styles/user-antd-table-content-scroll.css';
import './styles/user-antd-table-fixed-columns-mirror.css';
import './styles/user-antd-table-fixed-column-overlays.css';
import './styles/user-antd-table-fixed-column-tables.css';
import './styles/user-antd-table-fixed-column-edges.css';
import './styles/user-antd-table-geometry.css';
import './styles/user-antd-table-header-cells.css';
import './styles/user-antd-table-ellipsis-sort.css';
import './styles/user-antd-table-body-rows.css';
import './styles/user-antd-table-placeholder.css';
import './styles/user-antd-spin-root.css';
import './styles/user-antd-spin-nested.css';
import './styles/user-antd-spin-container.css';
import './styles/user-antd-spin-dot-shell.css';
import './styles/user-antd-spin-dot-item.css';
import './styles/user-antd-spin-dot-item-positions.css';
import './styles/user-antd-spin-dot-motion.css';
import './styles/user-antd-spin-keyframes.css';
import './styles/user-antd-motion-fade-in-keyframes.css';
import './styles/user-antd-motion-zoom-in-keyframes.css';
import './styles/user-antd-motion-slide-down-in-keyframes.css';
import './styles/user-antd-motion-fade-out-keyframes.css';
import './styles/user-antd-motion-zoom-out-keyframes.css';
import './styles/user-antd-modal-motion-fade.css';
import './styles/user-antd-modal-motion-zoom.css';
import './styles/user-antd-modal-motion-modal-zoom.css';
import './styles/user-antd-tag.css';
import './styles/user-antd-badge-base.css';
import './styles/user-antd-badge-status-layout.css';
import './styles/user-antd-badge-status-processing.css';
import './styles/user-antd-badge-status-colors.css';
import './styles/user-antd-badge-count.css';
import './styles/user-antd-badge-keyframes.css';
import './styles/user-antd-icon-base.css';
import './styles/user-antd-tooltip-base.css';
import './styles/user-antd-tooltip-arrow-base.css';
import './styles/user-antd-tooltip-arrow-top.css';
import './styles/user-antd-tooltip-arrow-top-offsets.css';
import './styles/user-antd-tooltip-motion-keyframes.css';
import './styles/user-antd-tooltip-motion-enter.css';
import './styles/user-antd-tooltip-motion-leave.css';
import './styles/user-antd-tooltip-motion-enter-active.css';
import './styles/user-antd-tooltip-motion-leave-active.css';
import './styles/user-antd-message-root.css';
import './styles/user-antd-message-notice.css';
import './styles/user-antd-message-content.css';
import './styles/user-antd-message-icon.css';
import './styles/user-antd-message-icon-status.css';
import './styles/user-antd-notification-root.css';
import './styles/user-antd-notification-notice.css';
import './styles/user-antd-notification-icon.css';
import './styles/user-antd-notification-close.css';
import './styles/user-antd-divider.css';
import './styles/user-antd-input-group-shell.css';
import './styles/user-antd-input-control.css';
import './styles/user-antd-input-states.css';
import './styles/user-antd-input-textarea-sizing.css';
import './styles/user-antd-input-group-edge-start.css';
import './styles/user-antd-input-group-cells.css';
import './styles/user-antd-input-group-addon-base.css';
import './styles/user-antd-input-group-edge-end.css';
import './styles/user-antd-input-search.css';
import './styles/user-antd-empty-shell.css';
import './styles/user-antd-empty-image.css';
import './styles/user-antd-empty-content.css';
import './styles/user-antd-empty-variants.css';
import './styles/user-antd-select-base.css';
import './styles/user-antd-select-selection.css';
import './styles/user-antd-select-sizing.css';
import './styles/user-antd-select-value.css';
import './styles/user-antd-select-arrow-base.css';
import './styles/user-antd-select-arrow-icon.css';
import './styles/user-antd-select-arrow-motion.css';
import './styles/user-antd-select-focus.css';
import './styles/user-antd-select-dropdown-root.css';
import './styles/user-antd-select-dropdown-menu.css';
import './styles/user-antd-select-dropdown-item.css';
import './styles/user-antd-select-dropdown-item-states.css';
import './styles/user-antd-select-dropdown-empty.css';
import './styles/user-antd-select-motion-keyframes.css';
import './styles/user-antd-select-motion-lifecycle.css';
import './styles/user-antd-select-motion-placement.css';
import './styles/user-antd-wave-token.css';
import './styles/user-antd-wave-trigger.css';
import './styles/user-antd-wave-surface.css';
import './styles/user-antd-wave-keyframes.css';
import './styles/user-antd-button-cjk.css';
import './styles/user-antd-button-core.css';
import './styles/user-antd-button-states.css';
import './styles/user-antd-button-content.css';
import './styles/user-antd-button-loading-overlay.css';
import './styles/user-antd-button-loading-spacing.css';
import './styles/user-antd-button-loading-small.css';
import './styles/user-antd-button-primary-base.css';
import './styles/user-antd-button-primary-interaction.css';
import './styles/user-antd-button-primary-disabled.css';
import './styles/user-antd-button-ghost-dashed.css';
import './styles/user-antd-button-danger-base.css';
import './styles/user-antd-button-danger-interaction.css';
import './styles/user-antd-button-danger-disabled.css';
import './styles/user-antd-button-link-base.css';
import './styles/user-antd-button-link-interaction.css';
import './styles/user-antd-button-link-disabled.css';
import './styles/user-antd-button-background-ghost-base.css';
import './styles/user-antd-button-background-ghost-primary.css';
import './styles/user-antd-button-background-ghost-danger.css';
import './styles/user-antd-button-background-ghost-link.css';
import './styles/user-antd-button-background-ghost-disabled-primary.css';
import './styles/user-antd-button-background-ghost-disabled-danger.css';
import './styles/user-antd-button-background-ghost-disabled-link.css';
import './styles/user-antd-button-group-primary.css';
import './styles/user-antd-button-disabled.css';
import './styles/user-antd-button-size.css';
import './styles/user-antd-button-round.css';
import './styles/user-antd-button-circle.css';
import './styles/user-antd-button-group-radius.css';
import './styles/user-antd-button-layout.css';
import './styles/user-list-group.css';
import './styles/user-antd-drawer-base.css';
import './styles/user-antd-drawer-horizontal-shell.css';
import './styles/user-antd-drawer-horizontal-placement.css';
import './styles/user-antd-drawer-horizontal-open.css';
import './styles/user-antd-drawer-horizontal-right-edge.css';
import './styles/user-antd-drawer-vertical-shared.css';
import './styles/user-antd-drawer-top.css';
import './styles/user-antd-drawer-bottom.css';
import './styles/user-antd-drawer-open-mask.css';
import './styles/user-antd-drawer-title.css';
import './styles/user-antd-drawer-content-shell.css';
import './styles/user-antd-drawer-close-button.css';
import './styles/user-antd-drawer-header.css';
import './styles/user-antd-drawer-body-wrapper.css';
import './styles/user-antd-drawer-mask.css';
import './styles/user-antd-modal-shell.css';
import './styles/user-antd-modal-wrap-mask.css';
import './styles/user-antd-modal-content-shell.css';
import './styles/user-antd-modal-close.css';
import './styles/user-antd-modal-centered-responsive.css';
import './styles/user-antd-modal-sections.css';
import './styles/user-antd-modal-confirm-shell.css';
import './styles/user-antd-modal-confirm-body.css';
import './styles/user-antd-modal-confirm-text.css';
import './styles/user-antd-modal-confirm-actions.css';
import './styles/user-antd-modal-confirm-status-icons.css';
import './styles/user-antd-switch-base.css';
import './styles/user-antd-switch-handle.css';
import './styles/user-antd-switch-active-width.css';
import './styles/user-antd-switch-loading-icon.css';
import './styles/user-antd-switch-checked.css';
import './styles/user-antd-switch-disabled-focus.css';
import './styles/user-form-groups.css';
import './styles/user-antd-pagination-shell.css';
import './styles/user-antd-pagination-item-base.css';
import './styles/user-antd-pagination-item-links.css';
import './styles/user-antd-pagination-item-interaction.css';
import './styles/user-antd-pagination-item-active.css';
import './styles/user-antd-pagination-jump-shell.css';
import './styles/user-antd-pagination-jump-ellipsis.css';
import './styles/user-antd-pagination-jump-icons.css';
import './styles/user-antd-pagination-jump-hover.css';
import './styles/user-antd-pagination-control-shell.css';
import './styles/user-antd-pagination-prev-next-links.css';
import './styles/user-antd-pagination-prev-next-active.css';
import './styles/user-antd-pagination-disabled.css';
import './styles/user-antd-pagination-options-shell.css';
import './styles/user-antd-pagination-quick-jumper.css';
import './styles/user-antd-pagination-quick-jumper-placeholder.css';
import './styles/user-antd-pagination-quick-jumper-states.css';
import './styles/user-antd-pagination-quick-jumper-sizing.css';
import './styles/user-antd-pagination-simple-controls.css';
import './styles/user-antd-pagination-simple-pager.css';
import './styles/user-antd-pagination-mini-items.css';
import './styles/user-antd-pagination-mini-options.css';
import './styles/user-antd-pagination-disabled-container.css';
import './styles/user-antd-pagination-disabled-jump.css';
import './styles/user-antd-pagination-responsive.css';
import './styles/user-ticket-chat-legacy.css';
import './styles/user-order-info.css';
import './styles/user-payment-select.css';
import './styles/user-border-utilities.css';
import './styles/user-antd-radio-button-group.css';
import './styles/user-antd-radio-button-wrapper.css';
import './styles/user-antd-radio-button-sizing.css';
import './styles/user-antd-radio-button-edges.css';
import './styles/user-antd-radio-button-focus.css';
import './styles/user-antd-radio-button-input.css';
import './styles/user-antd-radio-button-checked-base.css';
import './styles/user-antd-radio-button-checked-states.css';
import './styles/user-antd-radio-button-solid-checked.css';
import './styles/user-antd-radio-button-disabled.css';
import './styles/user-cashier-radio.css';
import './styles/user-antd-radio-native-wrapper.css';
import './styles/user-antd-radio-native-select-bridge.css';
import './styles/user-antd-radio-native-control.css';
import './styles/user-antd-radio-native-input.css';
import './styles/user-antd-radio-native-inner.css';
import './styles/user-antd-radio-native-focus.css';
import './styles/user-antd-radio-native-checked.css';
import './styles/user-antd-radio-native-disabled.css';
import './styles/user-antd-radio-native-spacing-motion.css';
import './styles/user-antd-result-shell.css';
import './styles/user-antd-result-icon.css';
import './styles/user-antd-result-icon-status.css';
import './styles/user-antd-result-text.css';
import './styles/user-antd-result-extra.css';
import './styles/user-payment-elements.css';
import './styles/user-plan-card-header.css';
import './styles/user-plan-antd-overrides.css';
import './styles/user-plan-tabs.css';
import './styles/user-plan-stock-tags.css';
import './styles/user-plan-content-features.css';
import './styles/user-plan-coupon-input.css';
import './styles/user-mobile-list-shell.css';
import './styles/user-mobile-list-item-shell.css';
import './styles/user-mobile-list-item-states.css';
import './styles/user-mobile-list-media.css';
import './styles/user-mobile-list-line-base.css';
import './styles/user-mobile-list-line-dividers.css';
import './styles/user-mobile-list-line-multiple.css';
import './styles/user-mobile-list-content.css';
import './styles/user-mobile-list-extra.css';
import './styles/user-mobile-list-brief.css';
import './styles/user-mobile-list-accessories.css';
import './styles/user-mobile-list-hairline-base.css';
import './styles/user-mobile-list-hairline-position.css';
import './styles/user-mobile-list-hairline-density.css';
import './styles/user-mobile-list-ripple-keyframes.css';
import './styles/user-mobile-block-layout-overrides.css';
import './styles/user-mobile-select-overrides.css';
import './styles/user-mobile-feedback-overrides.css';
import './styles/user-mobile-cashier-overrides.css';
import './styles/user-mobile-search-overrides.css';
import './styles/user-bg-image.css';
import './styles/user-auth-shell.css';
import './styles/user-auth-language.css';
import './styles/user-auth-alerts.css';
import './styles/user-antd-dropdown-root.css';
import './styles/user-antd-dropdown-menu.css';
import './styles/user-antd-dropdown-motion.css';
import './styles/user-antd-dropdown-items.css';
import './styles/user-email-whitelist-enable.css';
import './styles/user-dashboard-background-pixels.css';
import './styles/user-dashboard-shortcut-items.css';
import './styles/user-trade-number.css';
import './styles/user-antd-carousel-root.css';
import './styles/user-antd-carousel-slider-list.css';
import './styles/user-antd-carousel-slide-activation.css';
import './styles/user-antd-carousel-track.css';
import './styles/user-antd-carousel-slides.css';
import './styles/user-antd-carousel-arrows-base.css';
import './styles/user-antd-carousel-arrows-states.css';
import './styles/user-antd-carousel-arrows-prev.css';
import './styles/user-antd-carousel-arrows-next.css';
import './styles/user-antd-carousel-dots-root.css';
import './styles/user-antd-carousel-dots-placement.css';
import './styles/user-antd-carousel-dots-items.css';
import './styles/user-antd-carousel-dots-buttons.css';
import './styles/user-antd-carousel-dots-active.css';
import './styles/user-subscribe-list.css';
import './styles/user-bootstrap-dropdown-root.css';
import './styles/user-bootstrap-dropdown-menu.css';
import './styles/user-bootstrap-dropdown-placement.css';
import './styles/user-bootstrap-dropdown-item.css';
import './styles/user-bootstrap-dropdown-item-states.css';
import './styles/user-page-shell-containers.css';
import './styles/user-page-shell-content-base-shell.css';
import './styles/user-page-shell-content-base-pull.css';
import './styles/user-page-shell-content-base-full.css';
import './styles/user-page-shell-content-base-spacing.css';
import './styles/user-page-shell-content-desktop-shell.css';
import './styles/user-page-shell-content-desktop-pull.css';
import './styles/user-page-shell-content-desktop-full.css';
import './styles/user-page-shell-content-desktop-spacing.css';
import './styles/user-page-shell-content-mobile.css';
import './styles/user-page-shell-boxed.css';
import './styles/user-page-shell-header-shell.css';
import './styles/user-page-shell-header-overlay.css';
import './styles/user-page-shell-header-content.css';
import './styles/user-page-shell-sidebar-header.css';
import './styles/user-sidebar-mini-visibility-base.css';
import './styles/user-page-shell-sidebar-shell.css';
import './styles/user-page-shell-sidebar-transitions.css';
import './styles/user-page-shell-sidebar-open.css';
import './styles/user-page-shell-sidebar-dark.css';
import './styles/user-background-utilities.css';
import './styles/user-page-shell-content-side-base.css';
import './styles/user-page-shell-content-side-pull.css';
import './styles/user-page-shell-content-side-full.css';
import './styles/user-page-shell-content-side-spacing.css';
import './styles/user-page-shell-block-content.css';
import './styles/user-sidebar-nav-list.css';
import './styles/user-sidebar-nav-heading.css';
import './styles/user-sidebar-nav-link-base.css';
import './styles/user-sidebar-nav-link-content.css';
import './styles/user-sidebar-nav-link-active.css';
import './styles/user-sidebar-nav-submenu-shell.css';
import './styles/user-sidebar-nav-submenu-link.css';
import './styles/user-sidebar-nav-submenu-open.css';
import './styles/user-sidebar-nav-submenu-horizontal.css';
import './styles/user-sidebar-nav-dark-base.css';
import './styles/user-sidebar-nav-dark-active.css';
import './styles/user-sidebar-nav-dark-submenu.css';
import './styles/user-sidebar-nav-dark-open.css';
import './styles/user-sidebar-nav-dark-horizontal.css';
import './styles/user-sidebar-nav-footer-mask.css';
import './styles/user-shell-polish.css';
import './styles/user-sidebar-toggle.css';
import './styles/user-sidebar-desktop-open.css';
import './styles/user-sidebar-desktop-mini-shell.css';
import './styles/user-sidebar-desktop-mini-content.css';
import './styles/user-sidebar-desktop-mini-nav-motion.css';
import './styles/user-sidebar-desktop-mini-visibility.css';
import './styles/user-sidebar-desktop-mini-collapsed-nav.css';
import './styles/auth-shadcn.css';
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

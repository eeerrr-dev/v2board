import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const indexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.html'),
  'utf8',
);
const visualParitySource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../scripts/visual-parity.mjs'),
  'utf8',
);

describe('admin legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the admin router', () => {
    expect(mainSource).toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).toContain('installLegacyDevModuleRecovery');
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback');
    expect(mainSource).toContain('normalizeLegacyHashRoute');
    expect(mainSource).toContain('installLocaleDocumentEnvironment');
    expect(mainSource).toContain('getNormalizedLegacyHashPath');
    expect(mainSource).toContain('const legacyHashRouteOptions = {');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).not.toContain("canonicalPath: '/'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain("publicRoutes: ['/', '/login']");
    expect(mainSource).toContain('nestedPrefixes: ADMIN_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('routes: ADMIN_LEGACY_ROUTE_PATHS');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLegacyHashRouteNormalizer(legacyHashRouteOptions);');
    expect(mainSource).toContain('installLocaleDocumentEnvironment(i18n);');
    expect(mainSource).toContain('if (import.meta.env.DEV) {');
    expect(mainSource).toContain("const legacyRecoveryVersion = 'white-screen-recovery-37';");
    expect(mainSource).toContain(
      'storageKey: `v2board:white-screen-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain(
      'storageKey: `v2board:dev-module-recovery:${legacyRecoveryVersion}`',
    );
    expect(mainSource).toContain('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);');
    expect(mainSource).toContain(
      'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
    );
    expect(mainSource).toContain('installLegacyDevWhiteScreenFallback({ delay: 5000 });');
    expect(mainSource).toContain(
      '} else {\n  installLegacyWhiteScreenRecovery(legacyHashRouteOptions, legacyWhiteScreenRecoveryConfig);',
    );
    expect(mainSource.indexOf('if (import.meta.env.DEV) {')).toBeLessThan(
      mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);'),
    );
    expect(
      mainSource.indexOf('installLegacyDevModuleRecovery(legacyDevModuleRecoveryConfig);'),
    ).toBeLessThan(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
      ),
    );
    expect(
      mainSource.indexOf(
        'installLegacyWhiteScreenRecovery(legacyHashRouteOptions, {\n    ...legacyWhiteScreenRecoveryConfig,\n    delay: 3000,\n  });',
      ),
    ).toBeLessThan(mainSource.indexOf('installLegacyDevWhiteScreenFallback({ delay: 5000 });'));
    expect(mainSource).toContain("import { useEffect, type ReactNode } from 'react';");
    expect(mainSource).toContain('function LegacyRouteGate({ children }: { children: ReactNode })');
    expect(mainSource).toContain(
      'const normalized = getNormalizedLegacyHashPath(current, legacyHashRouteOptions);',
    );
    expect(mainSource).toContain('useEffect(() => {');
    expect(mainSource).toContain('normalizeLegacyHashRoute(legacyHashRouteOptions);');
    expect(mainSource).toContain('}, [location.hash, location.pathname, location.search]);');
    expect(mainSource).toContain(
      'return normalized !== current ? <Navigate to={normalized} replace /> : <>{children}</>;',
    );
  });

  it('initializes legacy settings and dark mode before rendering', () => {
    expect(mainSource).toContain('applyAdminLegacySettings();');
    expect(mainSource).toContain('applyInitialDarkMode();');
    const bootDarkModeIndex = mainSource.lastIndexOf('applyInitialDarkMode();');
    expect(mainSource.indexOf('applyAdminLegacySettings();')).toBeLessThan(
      bootDarkModeIndex,
    );
    expect(bootDarkModeIndex).toBeLessThan(mainSource.indexOf('const i18n = createI18n();'));
  });

  it('does not wrap the app in React StrictMode, matching the bundled admin entry', () => {
    expect(mainSource).not.toContain('StrictMode');
  });

  it('does not install timed query freshness or automatic retry absent from the bundled admin models', () => {
    expect(mainSource).toContain("import { redirectToLegacyLogin } from './lib/api';");
    expect(mainSource).toContain('queryCache: new QueryCache({');
    expect(mainSource).toContain('if (isUnauthorizedError(error)) redirectToLegacyLogin();');
    expect(mainSource).toContain('function isUnauthorizedError(error: unknown): boolean');
    expect(mainSource).toContain('const status = (error as { status?: unknown }).status;');
    expect(mainSource).toContain(
      "(error as { response?: { status?: unknown } }).response?.status",
    );
    expect(mainSource).toContain('return status === 403 || responseStatus === 403;');
    expect(mainSource).toContain(
      'defaultOptions: { queries: { staleTime: 0, retry: false, refetchOnWindowFocus: false } },',
    );
    expect(mainSource).not.toContain('staleTime: 30_000');
    expect(mainSource).not.toContain('retry: 1');
  });

  it('wraps the whole admin app with the white-screen guard inside HashRouter', () => {
    expect(mainSource).toContain('HashRouter');
    expect(mainSource).toContain('useLocation');
    expect(mainSource).toContain('Navigate');
    expect(mainSource).toContain(
      "import { RouteBoundaryElement } from './components/route-error-boundary';",
    );
    // The legacy antd confirm portal is gone; pages use the shadcn confirm
    // dialog + island toaster providers instead.
    expect(mainSource).not.toContain('LegacyConfirmProvider');
    expect(mainSource).toContain('<ConfirmDialogProvider />');
    expect(mainSource).toContain('<Toaster />');
    expect(mainSource).toContain('<HashRouter>');
    expect(mainSource).toContain('<LegacyRouteGate>');
    expect(mainSource).toContain('</LegacyRouteGate>');
    expect(mainSource).toContain('<RouteBoundaryElement>');
    expect(mainSource).toContain('<App />');
  });

  it('does not install a storage-event auth sync listener absent from the bundled admin entry', () => {
    expect(mainSource).not.toContain('setupAuthSync');
    expect(mainSource).not.toContain("from './lib/auth'");
  });

  it('no longer wraps the app in the antd ConfigProvider/App runtime', () => {
    // The admin surfaces are pure shadcn islands; the antd runtime provider and
    // its zh_CN locale were removed. API-error notifications now route through
    // the island Toaster instead of antd's static notification API.
    expect(mainSource).not.toContain("from 'antd'");
    expect(mainSource).not.toContain('antd/locale/zh_CN');
    expect(mainSource).not.toContain('ConfigProvider');
    expect(mainSource).not.toContain('AntdApp');
    expect(mainSource).toContain('<Toaster />');
  });

  it('installs dev entry recovery before the Vite module graph loads', () => {
    expect(indexSource).toContain("var recoveryVersion = 'white-screen-recovery-37';");
    expect(indexSource).toContain(
      "var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;",
    );
    expect(indexSource).toContain('function clearOldRecoveryState()');
    expect(indexSource).toContain("'v2board:white-screen-recovery:',");
    expect(indexSource).toContain("'v2board:dev-module-recovery:',");
    expect(indexSource).toContain("key.indexOf(':' + recoveryVersion + ':') !== -1");
    expect(indexSource).toContain('clearOldRecoveryState();');
    expect(indexSource).toContain('function clearBrowserCaches()');
    expect(indexSource).toContain("if (!('caches' in window)) return;");
    expect(indexSource).toContain('clearBrowserCaches();');
    expect(indexSource).toContain('var legacyRoutes = [');
    expect(indexSource).toContain("var legacyPublicRoutes = ['/', '/login'];");
    expect(indexSource).toContain('function normalizeBootUrl(url)');
    expect(indexSource).toContain("var nextHash = '#' + normalizedLegacyPath(routeSource);");
    expect(indexSource).toContain(
      "window.history.replaceState(window.history.state, '', bootUrl.toString());",
    );
    expect(indexSource).toContain('normalizeBootUrl(current);');
    expect(indexSource).toContain("text.indexOf('outdated optimize dep') !== -1");
    expect(indexSource).toContain("text.indexOf('/node_modules/.vite/') !== -1 &&");
    expect(indexSource).toContain("text.indexOf('module script') !== -1");
    expect(indexSource).not.toContain("text.indexOf('/node_modules/.vite/') !== -1\n          );");
    expect(indexSource).toContain('function routeMismatchWarning(value)');
    expect(indexSource).toContain("text.indexOf('no routes matched location') !== -1");
    expect(indexSource).toContain("text.indexOf('matched location \"/login/') !== -1");
    expect(indexSource).toContain('function patchConsoleRecovery(method)');
    expect(indexSource).toContain("patchConsoleRecovery('error');");
    expect(indexSource).toContain("patchConsoleRecovery('warn');");
    expect(indexSource).not.toContain('function legacyMainEmpty(root)');
    expect(indexSource).toContain('return elementEmpty(root);');
    expect(indexSource).not.toContain('legacyMainEmpty(root)');
    expect(indexSource).toContain("if (document.readyState === 'loading') {");
    expect(indexSource).toContain('if (appEmpty()) recover();');
    expect(indexSource).toContain("window.addEventListener('hashchange', schedule);");
    expect(indexSource).toContain("window.addEventListener('popstate', schedule);");
    expect(indexSource).toContain('new MutationObserver(schedule).observe(observerTarget');
    expect(indexSource).toContain("current.searchParams.set('__v2board_entry_recover'");
    expect(indexSource).toContain('data-v2board-white-screen-fallback="1"');
    expect(indexSource).not.toContain('/assets/admin/components.chunk.css');
    expect(indexSource).not.toContain('/assets/admin/umi.css');
    expect(indexSource).not.toContain('/assets/admin/vendors.async.js');
    expect(indexSource).not.toContain('/assets/admin/components.async.js');
    expect(
      indexSource.indexOf("var storageKey = 'v2board:dev-entry-recovery:' + recoveryVersion;"),
    ).toBeLessThan(
      indexSource.indexOf(
        '<script type="module" src="/src/main.tsx?v=20260607-white-screen-recovery-37"',
      ),
    );
  });

  it('chooses the visual parity browser lifecycle per scenario and keeps partial reports on disk', () => {
    expect(visualParitySource).toContain('for (const scenario of selectedScenarios) {');
    expect(visualParitySource).toContain('for (const viewport of selectedViewports) {');
    expect(visualParitySource).toContain("label: 'user-home-root'");
    expect(visualParitySource).toContain("path: '/#/'");
    expect(visualParitySource).toContain("readySelector: '.v2board-auth-box'");
    const requiredScreenshotScenarios = [
      'admin-ticket-detail',
      'admin-theme',
      'admin-root',
    ];
    for (const label of requiredScreenshotScenarios) {
      expect(visualParitySource).toContain(`label: '${label}'`);
    }
    expect(visualParitySource).toContain("path: `/${adminPath}#/ticket/7`");
    expect(visualParitySource).toContain("path: `/${adminPath}#/config/theme`");
    expect(visualParitySource).toContain("path: `/${adminPath}#/`");
    expect(visualParitySource).toContain("store.dispatch({ type: 'theme/setState', payload: themes });");
    expect(visualParitySource).toContain('seedLegacyAdminTicketDetailStore(page)');
    expect(visualParitySource).toContain('const adminTicketDetailFixture');
    expect(visualParitySource).toContain('ticket: adminTicketDetailFixture');
    expect(visualParitySource).toContain('ticket: ticketDetail');
    expect(visualParitySource).toContain("contentType: 'application/json'");
    expect(visualParitySource).toContain("'content-type': 'application/json'");
    expect(visualParitySource).toContain("readySelector: '.block-transparent.bg-image'");
    expect(visualParitySource).toContain("readySelector: '.js-chat-input'");
    expect(visualParitySource).toContain("const browserName = process.env.VISUAL_PARITY_BROWSER || 'chromium';");
    expect(visualParitySource).toContain(
      "const exactScenarioFilter = process.env.VISUAL_PARITY_EXACT_FILTER === '1';",
    );
    expect(visualParitySource).toContain('scenario.label === scenarioFilter');
    expect(visualParitySource).toContain(
      "const effectiveLocale = scenario.locale ?? (isAdminScenario ? '' : 'zh-CN');",
    );
    expect(visualParitySource).toContain('locale: effectiveLocale');
    expect(visualParitySource).toContain('const browserTypes = { chromium, firefox, webkit };');
    expect(visualParitySource).toContain('function launchBrowser()');
    expect(visualParitySource).toContain('return browserType.launch(launchOptions);');
    expect(visualParitySource).toContain('await browser.close();');
    expect(visualParitySource).toContain('async function captureScenarioWithFreshBrowser');
    expect(visualParitySource).toContain("const browserMode = process.env.VISUAL_PARITY_FRESH_BROWSER || 'auto';");
    expect(visualParitySource).toContain('function shouldUseFreshBrowser(scenario, viewport)');
    expect(visualParitySource).toContain(
      "return !(scenario.label === 'admin-dashboard' && viewport.label === 'desktop');",
    );
    expect(visualParitySource).toContain('if (!useFreshBrowser) {');
    expect(visualParitySource).toContain('async function writeReport()');
    expect(visualParitySource).toContain('await writeReport();');
    expect(visualParitySource.indexOf('const browser = await launchBrowser();')).toBeGreaterThan(
      visualParitySource.indexOf('for (const viewport of selectedViewports) {'),
    );
    const sharedBrowserStart = visualParitySource.indexOf('if (!useFreshBrowser) {');
    expect(visualParitySource.indexOf('await browser.close();', sharedBrowserStart)).toBeLessThan(
      visualParitySource.indexOf('} else {', sharedBrowserStart),
    );
    const freshBrowserStart = visualParitySource.indexOf(
      'async function captureScenarioWithFreshBrowser',
    );
    expect(visualParitySource.indexOf('await browser.close();', freshBrowserStart)).toBeLessThan(
      visualParitySource.indexOf('async function captureScenario(browser', freshBrowserStart),
    );
  });

  it('keeps interaction parity on the frozen oracle instead of packaged public runtime files', () => {
    expect(visualParitySource).toContain(
      "const parityMode = process.env.VISUAL_PARITY_MODE ?? 'screenshots';",
    );
    expect(visualParitySource).toContain("if (parityMode === 'interactions') {");
    expect(visualParitySource).toContain('await runInteractionParity(oracleServer.baseUrl);');
    expect(visualParitySource).toContain('const interactionScenarios = [');
    expect(visualParitySource).toContain('const darkModeStyleTargets = [');
    expect(visualParitySource).toContain('async function darkModeStyleSnapshot(page)');
    expect(visualParitySource).toContain('async function waitForStableDarkModeStyleSnapshot(page, diagnostics)');
    expect(visualParitySource).toContain(
      'styleSnapshot: await waitForStableDarkModeStyleSnapshot(page, diagnostics)',
    );
    // The tri-state dark-mode redesign moved the style-snapshot readiness gate
    // into hasUsefulDarkModeStyleSnapshot: it requires enough captured elements
    // plus a real header/sidebar/main background before treating dark mode as
    // applied, replacing the old inline capturedCount<8 / missing-pageHeader
    // retry checks.
    expect(visualParitySource).toContain('hasUsefulDarkModeStyleSnapshot(result.afterReload)');
    expect(visualParitySource).toContain('snapshot?.capturedCount >= 6 &&');
    expect(visualParitySource).toContain(
      'snapshot?.elements?.pageHeader?.backgroundColor ||',
    );
    const interactionLabels = [
      'user-login-form-language',
      'user-login-language-persistence',
      'user-auth-401-no-redirect',
      'user-dashboard-dark-mode-persistence',
      'user-dashboard-subscribe-drawer',
      'user-dashboard-notice-carousel',
      'user-dashboard-reset-package-confirm',
      'user-dashboard-alert-links',
      'user-profile-deposit-modal',
      'user-profile-reset-subscribe-confirm',
      'user-profile-telegram-bind-modal',
      'user-profile-telegram-unbind-confirm',
      'user-profile-preference-switches',
      'user-profile-redeem-giftcard',
      'user-profile-change-password-success',
      'user-plans-filter-tabs',
      'user-plan-checkout-coupon',
      'user-order-payment-method',
      'user-node-table-scroll',
      'user-traffic-table-scroll',
      'user-knowledge-drawer',
      'user-knowledge-extreme-content-matrix',
      'user-invite-generate',
      'user-invite-finance-submit-matrix',
      'user-ticket-reply-send',
      'user-ticket-error-matrix',
      'user-ticket-create-submit',
      'user-order-cancel-confirm',
      'admin-ticket-reply-send',
      'admin-tickets-reply-filter',
      'admin-auth-401-no-redirect',
      'admin-dashboard-dark-mode-persistence',
      'admin-dashboard-commission-shortcut',
      'admin-config-tabs',
      'admin-plan-create-drawer',
      'admin-plan-edit-drawer',
      'admin-mutation-failure-matrix',
      'admin-theme-settings-modal',
      'admin-config-save-failure-matrix',
      'admin-server-create-node-drawer',
      'admin-server-edit-node-drawer',
      'admin-server-route-create-modal',
      'admin-server-route-edit-modal',
      'admin-server-group-create-modal',
      'admin-server-group-edit-modal',
      'admin-payment-create-modal',
      'admin-payment-edit-modal',
      'admin-order-detail-modal',
      'admin-order-assign-modal',
      'admin-order-status-dropdown',
      'admin-order-commission-dropdown',
      'admin-orders-filter-pagination-matrix',
      'admin-coupon-create-modal',
      'admin-coupon-edit-modal',
      'admin-giftcard-create-modal',
      'admin-giftcard-edit-modal',
      'admin-notice-create-modal',
      'admin-notice-edit-modal',
      'admin-knowledge-create-drawer',
      'admin-knowledge-edit-drawer',
      'admin-users-filter-input',
      'admin-users-sort-matrix',
      'admin-user-bulk-ban-confirm',
      'admin-user-bulk-delete-confirm',
      'admin-user-destructive-failure-matrix',
      'admin-user-export-download-matrix',
      'admin-user-create-modal',
      'admin-user-send-mail-modal',
      'admin-user-send-mail-submit-matrix',
      'admin-user-reset-secret-confirm',
      'admin-user-delete-confirm',
      'admin-user-copy-action',
      'admin-user-edit-action',
      'admin-user-update-validation-failure',
      'admin-user-assign-action',
      'admin-user-orders-action',
      'admin-user-invite-action',
      'admin-user-traffic-action',
      'admin-users-extreme-viewport-matrix',
    ];
    for (const label of interactionLabels) {
      expect(visualParitySource).toContain(`label: '${label}'`);
    }
    expect(visualParitySource).toContain('async function runInteractionParity(oracleBaseUrl)');
    expect(visualParitySource).toContain(
      'async function runLoginLanguagePersistenceInteraction(page)',
    );
    expect(visualParitySource).toContain('async function runDarkModePersistenceInteraction(page)');
    expect(visualParitySource).toContain('async function runUnauthorizedHttp401NoRedirectInteraction(page)');
    expect(visualParitySource).toContain('async function runUserKnowledgeExtremeContentMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runInviteFinanceSubmitMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runUserTicketErrorMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminTicketsReplyFilterInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminThemeSettingsInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminConfigSaveFailureMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminPlanCreateDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminPlanEditDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminMutationFailureMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerCreateNodeDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerEditNodeDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerRouteCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerRouteEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerGroupCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminServerGroupEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminPaymentCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminPaymentEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminOrderDetailModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminOrderAssignModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminOrderStatusDropdownInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminOrderCommissionDropdownInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminOrdersFilterPaginationMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminCouponEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminGiftcardCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminGiftcardEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminNoticeCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminNoticeEditModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminKnowledgeCreateDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminKnowledgeEditDrawerInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserBulkBanConfirmInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserBulkDeleteConfirmInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserDestructiveFailureMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserExportDownloadMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserCreateModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserSendMailModalInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserSendMailSubmitMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserResetSecretConfirmInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserDeleteConfirmInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserCopyActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserEditActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserUpdateValidationFailureInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserAssignActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserOrdersActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserInviteActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUserTrafficActionInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUsersSortMatrixInteraction(page)');
    expect(visualParitySource).toContain('async function runAdminUsersExtremeViewportMatrixInteraction(page)');
    expect(visualParitySource).toContain('function readRequestData(request)');
    expect(visualParitySource).toContain("waitForVisibleElementsHidden(page, '.ant-select-dropdown')");
    expect(visualParitySource).toContain("case '/payment/getPaymentMethods':");
    expect(visualParitySource).toContain("case '/payment/getPaymentForm':");
    expect(visualParitySource).toContain("case '/payment/save':");
    expect(visualParitySource).toContain("case '/config/save':");
    expect(visualParitySource).toContain("case '/theme/saveThemeConfig':");
    expect(visualParitySource).toContain("case '/plan/save':");
    expect(visualParitySource).toContain("case '/plan/update':");
    expect(visualParitySource).toContain("case '/plan/drop':");
    expect(visualParitySource).toContain("case '/coupon/generate':");
    expect(visualParitySource).toContain("case '/giftcard/generate':");
    expect(visualParitySource).toContain("case '/knowledge/save':");
    expect(visualParitySource).toContain("case '/notice/save':");
    expect(visualParitySource).toContain("case '/notice/show':");
    expect(visualParitySource).toContain("case '/notice/drop':");
    expect(visualParitySource).toContain("case '/order/detail':");
    expect(visualParitySource).toContain("case '/order/assign':");
    expect(visualParitySource).toContain('Parity Created Group');
    expect(visualParitySource).toContain("case '/server/group/save':");
    expect(visualParitySource).toContain("case '/server/manage/sort':");
    expect(visualParitySource).toContain("__visualParityAdminServerGroupSaveRequests");
    expect(visualParitySource).toContain("delayAdminServerGroupSaveMs: 200");
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.name !== 'Parity Created Group'",
    );
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.name !== 'Parity Edited Group'",
    );
    expect(visualParitySource).toContain("case '/order/paid':");
    expect(visualParitySource).toContain("case '/order/update':");
    expect(visualParitySource).toContain("case '/user/update':");
    expect(visualParitySource).toContain("case '/user/delUser':");
    expect(visualParitySource).toContain("case '/user/ban':");
    expect(visualParitySource).toContain("case '/user/allDel':");
    expect(visualParitySource).toContain("case '/user/dumpCSV':");
    expect(visualParitySource).toContain("case '/user/sendMail':");
    expect(visualParitySource).toContain("case '/user/getUserInfoById':");
    expect(visualParitySource).toContain("case '/stat/getStatUser':");
    expect(visualParitySource).toContain("case '/api/v1/user/ticket/close':");
    expect(visualParitySource).toContain('adminPaymentFormFixtures');
    expect(visualParitySource).toContain("scenarioLabel: 'admin-ticket-detail'");
    expect(visualParitySource).toContain("scenarioLabel: 'admin-tickets'");
    expect(visualParitySource).toContain("scenarioLabel: 'admin-payments'");
    expect(visualParitySource).toContain("scenarioLabel: 'admin-orders'");
    expect(visualParitySource).toContain("scenarioLabel: 'admin-theme'");
    expect(visualParitySource).toContain('clickAdminTicketsReplyFilterOption(page,');
    expect(visualParitySource).toContain('Parity Pay');
    expect(visualParitySource).toContain('pk_parity_create');
    expect(visualParitySource).toContain('sk_parity_create');
    expect(visualParitySource).toContain('Parity Edited Node');
    expect(visualParitySource).toContain('Parity Created Route');
    expect(visualParitySource).toContain('Parity Edited Route');
    expect(visualParitySource).toContain('Parity Edited Group');
    expect(visualParitySource).toContain('Parity Plan');
    expect(visualParitySource).toContain('Parity Edited Plan');
    expect(visualParitySource).toContain("__visualParityAdminPlanSaveRequests");
    expect(visualParitySource).toContain("__visualParityAdminPlanUpdateRequests");
    expect(visualParitySource).toContain("__visualParityAdminPlanDropRequests");
    expect(visualParitySource).toContain("delayAdminPlanSaveMs: 200");
    expect(visualParitySource).toContain("delayAdminMutationMs: 200");
    expect(visualParitySource).toContain("result.saveRequests?.[0]?.name !== 'Parity Plan'");
    expect(visualParitySource).toContain("String(result.saveRequests?.[0]?.month_price) !== '1234'");
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.name !== 'Parity Edited Plan'",
    );
    expect(visualParitySource).toContain("String(result.saveRequests?.[0]?.month_price) !== '8888'");
    expect(visualParitySource).toContain('Parity Edited Pay');
    expect(visualParitySource).toContain('Parity Edited Coupon');
    expect(visualParitySource).toContain("__visualParityAdminCouponGenerateRequests");
    expect(visualParitySource).toContain("delayAdminCouponGenerateMs: 200");
    expect(visualParitySource).toContain("result.generateRequests?.[0]?.name !== 'Parity Coupon'");
    expect(visualParitySource).toContain("String(result.generateRequests?.[0]?.value) !== '2500'");
    expect(visualParitySource).toContain(
      "result.generateRequests?.[0]?.name !== 'Parity Edited Coupon'",
    );
    expect(visualParitySource).toContain("String(result.generateRequests?.[0]?.value) !== '1250'");
    expect(visualParitySource).toContain("__visualParityAdminGiftcardGenerateRequests");
    expect(visualParitySource).toContain("delayAdminGiftcardGenerateMs: 200");
    expect(visualParitySource).toContain("result.generateRequests?.[0]?.name !== 'Parity Giftcard'");
    expect(visualParitySource).toContain("String(result.generateRequests?.[0]?.value) !== '0'");
    expect(visualParitySource).toContain(
      "result.generateRequests?.[0]?.name !== 'Parity Edited Giftcard'",
    );
    expect(visualParitySource).toContain("String(result.generateRequests?.[0]?.value) !== '45'");
    expect(visualParitySource).toContain("__visualParityAdminNoticeSaveRequests");
    expect(visualParitySource).toContain("__visualParityAdminNoticeShowRequests");
    expect(visualParitySource).toContain("__visualParityAdminNoticeDropRequests");
    expect(visualParitySource).toContain("delayAdminNoticeSaveMs: 200");
    expect(visualParitySource).toContain("result.saveRequests?.[0]?.title !== 'Parity Notice'");
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.['tags[0]'] !== 'ops'",
    );
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.title !== 'Parity Edited Notice'",
    );
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.['tags[1]'] !== 'edited'",
    );
    expect(visualParitySource).toContain("__visualParityAdminKnowledgeSaveRequests");
    expect(visualParitySource).toContain("delayAdminKnowledgeSaveMs: 200");
    expect(visualParitySource).toContain("result.saveRequests?.[0]?.title !== 'Parity Knowledge'");
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.language !== 'en-US'",
    );
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.title !== 'Parity Edited Article'",
    );
    expect(visualParitySource).toContain("String(result.saveRequests?.[0]?.id) !== '1'");
    expect(visualParitySource).toContain('Parity Edited Giftcard');
    expect(visualParitySource).toContain('Parity Edited Notice');
    expect(visualParitySource).toContain('parity.created');
    expect(visualParitySource).toContain('Parity Mail Subject');
    expect(visualParitySource).toContain('Parity Mail Submit Success');
    expect(visualParitySource).toContain('Parity Mail Failure');
    expect(visualParitySource).toContain("__visualParityAdminUserSendMailRequests");
    expect(visualParitySource).toContain('重置UUID及订阅URL');
    expect(visualParitySource).toContain('assign-user@example.com');
    expect(visualParitySource).toContain("__visualParityLastAdminOrderPaid");
    expect(visualParitySource).toContain("__visualParityLastAdminOrderUpdate");
    expect(visualParitySource).toContain("__visualParityLastAdminOrderFetchQuery");
    expect(visualParitySource).toContain("__visualParityAdminPaymentSaveRequests");
    expect(visualParitySource).toContain("delayAdminPaymentSaveMs: 200");
    expect(visualParitySource).toContain("clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary')");
    expect(visualParitySource).toContain("result.saveRequests?.[0]?.payment !== 'StripeCheckout'");
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.['config[publishable_key]'] !== 'pk_parity_create'",
    );
    expect(visualParitySource).toContain(
      "result.saveRequests?.[0]?.['config[key]'] !== 'edited-secret'",
    );
    expect(visualParitySource).toContain("__visualParityLastAdminFilteredUserFetchQuery");
    expect(visualParitySource).toContain("__visualParityLastAdminUserTrafficQuery");
    expect(visualParitySource).toContain("__visualParityAdminConfigSaveRequests");
    expect(visualParitySource).toContain("__visualParityAdminThemeSaveRequests");
    expect(visualParitySource).toContain("__visualParityAdminUserDeleteRequests");
    expect(visualParitySource).toContain("__visualParityAdminUserBanRequests");
    expect(visualParitySource).toContain("__visualParityAdminUserAllDeleteRequests");
    expect(visualParitySource).toContain("__visualParityAdminUserDumpCsvRequests");
    expect(visualParitySource).toContain("__visualParityAdminUserUpdateRequests");
    expect(visualParitySource).toContain("__visualParityAdminServerSortRequests");
    expect(visualParitySource).toContain("__visualParityUserTicketCloseRequests");
    expect(visualParitySource).toContain('extreme-knowledge-token-2026');
    expect(visualParitySource).toContain('VISUAL2026110001');
    expect(visualParitySource).toContain('VISUAL2026110002');
    expect(visualParitySource).toContain('用户管理');
    expect(visualParitySource).toContain('订阅计划');
    expect(visualParitySource).toContain('分配订单');
    expect(visualParitySource).toContain('visual-user@example.com');
    expect(visualParitySource).toContain('TA的订单');
    expect(visualParitySource).toContain('TA的邀请');
    expect(visualParitySource).toContain('TA的流量记录');
    expect(visualParitySource).toContain('Parity Theme Title');
    expect(visualParitySource).toContain(
      "const interactionFilter = process.env.VISUAL_PARITY_INTERACTION_FILTER ?? scenarioFilter;",
    );
    expect(visualParitySource).toContain(
      'new URL(scenario.path, oracleBaseUrl).toString()',
    );
    expect(visualParitySource).toContain('assertUsefulInteraction(interaction.label, result);');
  });

});

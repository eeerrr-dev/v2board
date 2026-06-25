import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const indexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.html'),
  'utf8',
);
const antdCompatSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/antd-v5-compat.css'),
  'utf8',
);
const adminAntdV3Source = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-v3.css'),
  'utf8',
);
const adminAntdBaseSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-base.css'),
  'utf8',
);
const adminAntdFeedbackSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-feedback.css'),
  'utf8',
);
const adminAntdButtonsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-buttons.css'),
  'utf8',
);
const adminAntdTableSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-table.css'),
  'utf8',
);
const adminAntdRadioSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-radio.css'),
  'utf8',
);
const adminAntdCheckboxSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-checkbox.css'),
  'utf8',
);
const adminAntdDropdownSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-dropdown.css'),
  'utf8',
);
const adminAntdSpinPaginationSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-spin-pagination.css'),
  'utf8',
);
const adminAntdSelectSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-select.css'),
  'utf8',
);
const adminAntdDividerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-divider.css'),
  'utf8',
);
const adminAntdTooltipSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-tooltip.css'),
  'utf8',
);
const adminAntdSwitchSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-switch.css'),
  'utf8',
);
const adminAntdInputSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-input.css'),
  'utf8',
);
const adminAntdTabsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-tabs.css'),
  'utf8',
);
const adminAntdCalendarSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-calendar.css'),
  'utf8',
);
const adminAntdTimePickerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-time-picker.css'),
  'utf8',
);
const adminAntdTagSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-tag.css'),
  'utf8',
);
const adminAntdDrawerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-drawer.css'),
  'utf8',
);
const adminAntdBadgeSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-badge.css'),
  'utf8',
);
const adminAntdMenuSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-antd-menu.css'),
  'utf8',
);
const adminMarkdownEditorSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-markdown-editor.css'),
  'utf8',
);
const adminRuntimeBaseSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-runtime-base.css'),
  'utf8',
);
const adminBootstrapV4Source = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-v4.css'),
  'utf8',
);
const adminBootstrapRebootSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-reboot.css'),
  'utf8',
);
const adminBootstrapGridSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-grid.css'),
  'utf8',
);
const adminBootstrapTablesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-tables.css'),
  'utf8',
);
const adminBootstrapFormsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-forms.css'),
  'utf8',
);
const adminBootstrapButtonsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-buttons.css'),
  'utf8',
);
const adminBootstrapInteractionsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-interactions.css'),
  'utf8',
);
const adminBootstrapNavCardsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-nav-cards.css'),
  'utf8',
);
const adminBootstrapContentComponentsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-content-components.css'),
  'utf8',
);
const adminBootstrapOverlaysSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-overlays.css'),
  'utf8',
);
const adminBootstrapSpinnersSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-spinners.css'),
  'utf8',
);
const adminBootstrapUtilitiesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-bootstrap-utilities.css'),
  'utf8',
);
const adminBootstrapV4ComponentSource = [
  adminBootstrapRebootSource,
  adminBootstrapGridSource,
  adminBootstrapTablesSource,
  adminBootstrapFormsSource,
  adminBootstrapButtonsSource,
  adminBootstrapInteractionsSource,
  adminBootstrapNavCardsSource,
  adminBootstrapContentComponentsSource,
  adminBootstrapOverlaysSource,
  adminBootstrapSpinnersSource,
].join('\n');
const adminOneuiCoreSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-oneui-core.css'),
  'utf8',
);
const adminIconFontsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-icon-fonts.css'),
  'utf8',
);
const adminOneuiUtilitiesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-oneui-utilities.css'),
  'utf8',
);
const adminAnimationsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-animations.css'),
  'utf8',
);
const adminPluginWidgetsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-plugin-widgets.css'),
  'utf8',
);
const visualParitySource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../scripts/visual-parity.mjs'),
  'utf8',
);

function compactCss(source: string) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\s+/g, '')
    .replace(/;}/g, '}');
}

function expectCssContains(source: string, expected: string) {
  expect(compactCss(source)).toContain(compactCss(expected));
}

function expectCssNotContains(source: string, expected: string) {
  expect(compactCss(source)).not.toContain(compactCss(expected));
}
const adminAppOverridesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'styles/admin-app-overrides.css'),
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
    expect(mainSource).toContain(
      "import { LegacyConfirmProvider } from './components/legacy-confirm';",
    );
    expect(mainSource).toContain('<HashRouter>');
    expect(mainSource).toContain('<LegacyRouteGate>');
    expect(mainSource).toContain('</LegacyRouteGate>');
    expect(mainSource).toContain('<RouteBoundaryElement>');
    expect(mainSource).toContain('<App />');
    expect(mainSource).toContain('<LegacyConfirmProvider />');
  });

  it('does not install a storage-event auth sync listener absent from the bundled admin entry', () => {
    expect(mainSource).not.toContain('setupAuthSync');
    expect(mainSource).not.toContain("from './lib/auth'");
  });

  it('keeps the admin Ant Design locale fixed to zh_CN like the bundled admin app', () => {
    expect(mainSource).toContain("import zhCN from 'antd/locale/zh_CN';");
    expect(mainSource).toContain('locale={zhCN}');
    expect(mainSource).not.toContain('antd/locale/en_US');
  });

  it('keeps Ant Design 5 table spin wrappers visible under the legacy admin stylesheet', () => {
    expect(mainSource).toContain("import './styles/admin-antd-v3.css';");
    expect(mainSource).not.toContain("import './styles/legacy-components.css';");
    expect(mainSource).toContain("import './styles/admin-markdown-editor.css';");
    expect(mainSource).toContain("import './styles/admin-icon-fonts.css';");
    expect(mainSource).toContain("import './styles/admin-runtime-base.css';");
    expect(mainSource).toContain("import './styles/admin-bootstrap-v4.css';");
    expect(mainSource).not.toContain("import './styles/legacy-umi.css';");
    expect(mainSource).toContain("import './styles/admin-bootstrap-utilities.css';");
    expect(mainSource).toContain("import './styles/admin-oneui-core.css';");
    expect(mainSource).toContain("import './styles/admin-oneui-utilities.css';");
    expect(mainSource).toContain("import './styles/admin-animations.css';");
    expect(mainSource).toContain("import './styles/admin-plugin-widgets.css';");
    expect(mainSource).toContain("import './styles/admin-app-overrides.css';");
    expect(mainSource).toContain("import './styles/antd-v5-compat.css';");
    expect(mainSource.indexOf("import './styles/admin-antd-v3.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-markdown-editor.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-markdown-editor.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-icon-fonts.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-icon-fonts.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-runtime-base.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-runtime-base.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-bootstrap-v4.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-bootstrap-v4.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-bootstrap-utilities.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-bootstrap-utilities.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-oneui-core.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-oneui-core.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-oneui-utilities.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-oneui-utilities.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-animations.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-animations.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-plugin-widgets.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-plugin-widgets.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/admin-app-overrides.css';"),
    );
    expect(mainSource.indexOf("import './styles/admin-app-overrides.css';")).toBeLessThan(
      mainSource.indexOf("import './styles/antd-v5-compat.css';"),
    );
    expect(antdCompatSource).toContain('.ant-table-wrapper > .ant-spin');
    expect(antdCompatSource).toContain(".ant-message [class*='-leave']");
    expect(antdCompatSource).toContain('display: block;');
    expect(antdCompatSource).not.toMatch(/^\.ant-spin\s*\{/m);
  });

  it('keeps the restored Ant Design v3 component stylesheet order explicit', () => {
    const importOrder = [
      'admin-antd-base.css',
      'admin-antd-feedback.css',
      'admin-antd-buttons.css',
      'admin-antd-table.css',
      'admin-antd-radio.css',
      'admin-antd-checkbox.css',
      'admin-antd-dropdown.css',
      'admin-antd-spin-pagination.css',
      'admin-antd-select.css',
      'admin-antd-divider.css',
      'admin-antd-tooltip.css',
      'admin-antd-switch.css',
      'admin-antd-input.css',
      'admin-antd-tabs.css',
      'admin-antd-calendar.css',
      'admin-antd-time-picker.css',
      'admin-antd-tag.css',
      'admin-antd-drawer.css',
      'admin-antd-badge.css',
      'admin-antd-menu.css',
    ];
    let previousIndex = -1;

    for (const stylesheet of importOrder) {
      expect(adminAntdV3Source).toContain(`@import './${stylesheet}';`);
      const currentIndex = adminAntdV3Source.indexOf(`@import './${stylesheet}';`);
      expect(currentIndex).toBeGreaterThan(previousIndex);
      previousIndex = currentIndex;
    }

    expect(adminAntdV3Source).not.toContain('legacy-components.css');
  });

  it('keeps restored Ant Design v3 component styles in source-owned component files', () => {
    expectCssContains(adminAntdBaseSource, 'body,html{width:100%;height:100%}');
    expectCssContains(adminAntdBaseSource, '.anticon{display:inline-block');
    expect(adminAntdBaseSource).toContain('@keyframes loadingCircle');
    expectCssNotContains(adminAntdBaseSource, '.ant-notification{');

    expectCssContains(adminAntdFeedbackSource, '.ant-notification{box-sizing:border-box');
    expectCssContains(adminAntdFeedbackSource, '.ant-message{box-sizing:border-box');
    expectCssContains(adminAntdFeedbackSource, '.ant-modal{box-sizing:border-box');
    expectCssNotContains(adminAntdFeedbackSource, '.ant-btn{line-height:1.499');

    expectCssContains(adminAntdButtonsSource, '.ant-btn{line-height:1.499');
    expect(adminAntdButtonsSource).toContain('.ant-btn-primary');
    expect(adminAntdButtonsSource).toContain('.ant-btn-group');
    expectCssNotContains(adminAntdButtonsSource, '.ant-table-wrapper{');

    expectCssContains(adminAntdTableSource, '.ant-table-wrapper{zoom:1}');
    expectCssContains(adminAntdTableSource, '.ant-table{box-sizing:border-box');
    expectCssContains(adminAntdTableSource, '.ant-table-thead>tr>th');
    expect(adminAntdTableSource).toContain('.ant-table-filter-dropdown');
    expectCssContains(adminAntdTableSource, '.ant-empty{');
    expectCssContains(adminAntdTableSource, '.ant-pagination{float:right');
    expectCssNotContains(adminAntdTableSource, '.ant-radio-group{');

    expectCssContains(adminAntdRadioSource, '.ant-radio-group{');
    expectCssContains(adminAntdRadioSource, '.ant-radio-wrapper{');
    expect(adminAntdRadioSource).toContain('@keyframes antRadioEffect');
    expect(adminAntdRadioSource).not.toContain('@keyframes antCheckboxEffect');

    expect(adminAntdCheckboxSource).toContain('@keyframes antCheckboxEffect');
    expectCssContains(adminAntdCheckboxSource, '.ant-checkbox{');
    expectCssContains(adminAntdCheckboxSource, '.ant-checkbox-wrapper{');
    expectCssNotContains(adminAntdCheckboxSource, '.ant-dropdown{');

    expectCssContains(adminAntdDropdownSource, '.ant-dropdown{');
    expectCssContains(adminAntdDropdownSource, '.ant-dropdown-menu{');
    expectCssNotContains(adminAntdDropdownSource, '.ant-spin{');

    expectCssContains(adminAntdSpinPaginationSource, '.ant-spin{');
    expect(adminAntdSpinPaginationSource).toContain('@keyframes antSpinMove');
    expectCssContains(adminAntdSpinPaginationSource, '.ant-pagination{');
    expect(adminAntdSpinPaginationSource).toContain('.ant-pagination-options-size-changer.ant-select');
    expectCssNotContains(adminAntdSpinPaginationSource, '.ant-select{box-sizing:border-box');

    expectCssContains(adminAntdSelectSource, '.ant-select{box-sizing:border-box');
    expect(adminAntdSelectSource).toContain('.ant-select-selection');
    expect(adminAntdSelectSource).toContain('.ant-select-dropdown');
    expect(adminAntdSelectSource).not.toContain('.ant-pagination-options-size-changer');
    expectCssNotContains(adminAntdSelectSource, '.ant-divider{');

    expectCssContains(adminAntdDividerSource, '.ant-divider{');
    expect(adminAntdDividerSource).toContain('.ant-divider-horizontal');
    expectCssNotContains(adminAntdDividerSource, '.ant-tooltip{');

    expectCssContains(adminAntdTooltipSource, '.ant-tooltip{');
    expect(adminAntdTooltipSource).toContain('.ant-tooltip-inner');
    expectCssNotContains(adminAntdTooltipSource, '.ant-switch{');

    expectCssContains(adminAntdSwitchSource, '.ant-switch{');
    expect(adminAntdSwitchSource).toContain('.ant-switch-inner');
    expectCssNotContains(adminAntdSwitchSource, '.ant-input{');

    expectCssContains(adminAntdInputSource, '.ant-input{');
    expectCssContains(adminAntdInputSource, '.ant-input-group{');
    expectCssNotContains(adminAntdInputSource, '.ant-tabs{');

    expectCssContains(adminAntdTabsSource, '.ant-tabs{');
    expect(adminAntdTabsSource).toContain('.ant-tabs-tab');
    expect(adminAntdTabsSource).not.toContain('.ant-calendar-picker-container');

    expect(adminAntdCalendarSource).toContain('.ant-calendar-picker-container');
    expectCssContains(adminAntdCalendarSource, '.ant-calendar-picker{');
    expectCssContains(adminAntdCalendarSource, '.ant-calendar{');
    expectCssNotContains(adminAntdCalendarSource, '.ant-time-picker{');

    expectCssContains(adminAntdTimePickerSource, '.ant-time-picker{');
    expect(adminAntdTimePickerSource).toContain('.ant-time-picker-input');
    expectCssNotContains(adminAntdTimePickerSource, '.ant-tag{');

    expectCssContains(adminAntdTagSource, '.ant-tag{');
    expect(adminAntdTagSource).toContain('.ant-tag-checkable');
    expectCssNotContains(adminAntdTagSource, '.ant-drawer{');

    expectCssContains(adminAntdDrawerSource, '.ant-drawer{');
    expect(adminAntdDrawerSource).toContain('.ant-drawer-content-wrapper');
    expectCssNotContains(adminAntdDrawerSource, '.ant-badge{');

    expectCssContains(adminAntdBadgeSource, '.ant-badge{');
    expect(adminAntdBadgeSource).toContain('.ant-badge-count');
    expectCssNotContains(adminAntdBadgeSource, '.ant-menu{');

    expectCssContains(adminAntdMenuSource, '.ant-menu{');
    expect(adminAntdMenuSource).toContain('.ant-menu-item');
    expect(adminAntdMenuSource).toContain('.ant-menu-submenu');
  });

  it('keeps the restored Bootstrap v4 stylesheet order explicit', () => {
    const importOrder = [
      'admin-bootstrap-reboot.css',
      'admin-bootstrap-grid.css',
      'admin-bootstrap-tables.css',
      'admin-bootstrap-forms.css',
      'admin-bootstrap-buttons.css',
      'admin-bootstrap-interactions.css',
      'admin-bootstrap-nav-cards.css',
      'admin-bootstrap-content-components.css',
      'admin-bootstrap-overlays.css',
      'admin-bootstrap-spinners.css',
    ];
    let previousIndex = -1;

    for (const stylesheet of importOrder) {
      expect(adminBootstrapV4Source).toContain(`@import './${stylesheet}';`);
      const currentIndex = adminBootstrapV4Source.indexOf(`@import './${stylesheet}';`);
      expect(currentIndex).toBeGreaterThan(previousIndex);
      previousIndex = currentIndex;
    }

    expect(adminBootstrapV4Source).not.toContain('legacy-umi.css');
  });

  it('keeps restored Bootstrap v4 component styles in source-owned component files', () => {
    expectCssContains(adminBootstrapRebootSource, '*,:after,:before{box-sizing:border-box}');
    expectCssContains(adminBootstrapRebootSource, 'body{margin:0');
    expectCssNotContains(adminBootstrapRebootSource, '.container{');

    expectCssContains(adminBootstrapGridSource, '.container,.container-fluid');
    expectCssContains(adminBootstrapGridSource, '.row{display:flex');
    expectCssNotContains(adminBootstrapGridSource, '.table{');

    expectCssContains(adminBootstrapTablesSource, '.table{width:100%');
    expectCssContains(adminBootstrapTablesSource, '.table-responsive{display:block');
    expectCssNotContains(adminBootstrapTablesSource, '.form-control{');

    expectCssContains(adminBootstrapFormsSource, '.form-control{display:block');
    expectCssContains(adminBootstrapFormsSource, '.form-check{position:relative');
    expectCssNotContains(adminBootstrapFormsSource, '.btn{display:inline-block');

    expectCssContains(adminBootstrapButtonsSource, '.btn{display:inline-block');
    expectCssContains(adminBootstrapButtonsSource, '.btn-primary{color:#fff');
    expectCssNotContains(adminBootstrapButtonsSource, '.fade{');

    expectCssContains(adminBootstrapInteractionsSource, '.fade{transition:opacity');
    expectCssContains(adminBootstrapInteractionsSource, '.dropdown-menu{position:absolute');
    expectCssContains(adminBootstrapInteractionsSource, '.custom-select{display:inline-block');
    expectCssNotContains(adminBootstrapInteractionsSource, '.nav{display:flex');

    expectCssContains(adminBootstrapNavCardsSource, '.nav{display:flex');
    expectCssContains(adminBootstrapNavCardsSource, '.card{position:relative');
    expectCssNotContains(adminBootstrapNavCardsSource, '.breadcrumb{');

    expectCssContains(adminBootstrapContentComponentsSource, '.breadcrumb{display:flex');
    expectCssContains(adminBootstrapContentComponentsSource, '.pagination{display:flex');
    expectCssContains(adminBootstrapContentComponentsSource, '.list-group{display:flex');
    expectCssContains(adminBootstrapContentComponentsSource, '.close{float:right');
    expectCssNotContains(adminBootstrapContentComponentsSource, '.toast{');

    expectCssContains(adminBootstrapOverlaysSource, '.toast{flex-basis');
    expectCssContains(adminBootstrapOverlaysSource, '.modal-open{overflow:hidden');
    expectCssContains(adminBootstrapOverlaysSource, '.tooltip{position:absolute');
    expect(adminBootstrapOverlaysSource).not.toContain('@keyframes spinner-border');

    expect(adminBootstrapSpinnersSource).toContain('@keyframes spinner-border');
    expectCssContains(adminBootstrapSpinnersSource, '.spinner-border{display:inline-block');
    expect(adminBootstrapSpinnersSource).toContain('@keyframes spinner-grow');
  });

  it('keeps admin runtime base fixes out of the Bootstrap v4 component layer', () => {
    expectCssNotContains(adminBootstrapV4ComponentSource, '.content___DW5w1{position:absolute');
    expectCssNotContains(adminBootstrapV4ComponentSource, '#root,body,html{height:100%}');
    expectCssNotContains(adminBootstrapV4ComponentSource, '.ant-spin-blur{overflow:unset');
    expectCssNotContains(
      adminBootstrapV4ComponentSource,
      '.ant-drawer-header,.ant-modal-header{padding:15px}',
    );

    expectCssContains(adminRuntimeBaseSource, '.content___DW5w1{position:absolute');
    expectCssContains(adminRuntimeBaseSource, '#root,body,html{height:100%}');
    expectCssContains(adminRuntimeBaseSource, '.ant-spin-blur{overflow:unset');
    expectCssContains(adminRuntimeBaseSource, '.ant-drawer-header,.ant-modal-header{padding:15px}');
    expectCssNotContains(adminRuntimeBaseSource, '*,:after,:before');
    expectCssNotContains(adminRuntimeBaseSource, '.table{');
    expectCssNotContains(adminRuntimeBaseSource, '#page-container');
  });

  it('keeps Bootstrap and grid utility helpers out of the Bootstrap v4 component layer', () => {
    expectCssNotContains(
      adminBootstrapV4ComponentSource,
      '.align-baseline{vertical-align:baseline!important}',
    );
    expectCssNotContains(adminBootstrapV4ComponentSource, '.shadow-sm{box-shadow:0 .125rem .25rem');
    expectCssNotContains(adminBootstrapV4ComponentSource, '.visible{visibility:visible!important}');
    expectCssNotContains(adminBootstrapV4ComponentSource, '.row.gutters-tiny{margin-right:-0.125rem');

    expectCssContains(
      adminBootstrapUtilitiesSource,
      '.align-baseline{vertical-align:baseline!important}',
    );
    expectCssContains(
      adminBootstrapUtilitiesSource,
      '.shadow-sm{box-shadow:0 0.125rem 0.25rem',
    );
    expectCssContains(adminBootstrapUtilitiesSource, '.visible{visibility:visible!important}');
    expectCssContains(
      adminBootstrapUtilitiesSource,
      '.row.gutters-tiny{margin-right:-0.125rem',
    );
    expectCssNotContains(adminBootstrapUtilitiesSource, '#page-container{display:flex');
    expect(adminBootstrapUtilitiesSource).not.toContain('.form-control.form-control-alt');
  });

  it('keeps OneUI layout and component core out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('.row:not(.gutters-tiny):not(.no-gutters)');
    expect(adminBootstrapV4ComponentSource).not.toContain('.form-control.form-control-alt');
    expectCssNotContains(adminBootstrapV4ComponentSource, '#page-container{display:flex');
    expect(adminBootstrapV4ComponentSource).not.toContain('.nav-main');

    expect(adminOneuiCoreSource).toContain('.row:not(.gutters-tiny):not(.no-gutters)');
    expect(adminOneuiCoreSource).toContain('.form-control.form-control-alt');
    expectCssContains(adminOneuiCoreSource, '#page-container{display:flex');
    expect(adminOneuiCoreSource).toContain('.nav-main');
    expectCssNotContains(adminOneuiCoreSource, '.align-baseline{vertical-align:baseline!important}');
    expect(adminOneuiCoreSource).not.toContain('.animated{animation-duration');
    expect(adminOneuiCoreSource).not.toContain('[data-simplebar]');
  });

  it('keeps V2Board-specific admin overrides out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('#cashier .ant-radio-button-wrapper');
    expect(adminBootstrapV4ComponentSource).not.toContain('.v2board-drawer-action');
    expect(adminBootstrapV4ComponentSource).not.toContain('.v2board-filter-drawer');
    expect(adminAppOverridesSource).toContain('#cashier .ant-radio-button-wrapper');
    expect(adminAppOverridesSource).toContain('.v2board-drawer-action');
    expect(adminAppOverridesSource).toContain('.v2board-filter-drawer .ant-drawer-content-wrapper');
    expect(adminAppOverridesSource).toContain('.v2board-auth-box');
    expect(adminAppOverridesSource).toContain('.v2board-stats-bar');
  });

  it('keeps markdown editor styles out of the Ant Design v3 compatibility layer', () => {
    expect(adminAntdV3Source).not.toContain('@font-face{font-family:rmel-iconfont');
    expect(adminAntdV3Source).not.toContain('.rmel-iconfont');
    expect(adminAntdV3Source).not.toContain('.rmel-icon-fullscreen:before');
    expect(adminAntdV3Source).not.toContain('.rc-md-editor');
    expect(adminAntdV3Source).not.toContain('.custom-html-style');
    expect(adminAntdBadgeSource).toContain('.ant-badge');

    expect(adminMarkdownEditorSource).toContain('@font-face{font-family:rmel-iconfont');
    expect(adminMarkdownEditorSource).toContain('.rmel-iconfont');
    expect(adminMarkdownEditorSource).toContain('.rmel-icon-fullscreen:before');
    expect(adminMarkdownEditorSource).toContain('.rc-md-editor');
    expect(adminMarkdownEditorSource).toContain('.custom-html-style');
    expect(adminMarkdownEditorSource).toContain('.rc-md-editor .header-list .list-item:hover');
    expect(adminMarkdownEditorSource).not.toContain('.ant-badge');
  });

  it('keeps source-owned admin icon fonts out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('./static/fa-regular-400');
    expect(adminBootstrapV4ComponentSource).not.toContain('./static/fa-solid-900');
    expect(adminBootstrapV4ComponentSource).not.toContain('./static/fa-brands-400');
    expect(adminBootstrapV4ComponentSource).not.toContain('./static/Simple-Line-Icons');
    expect(adminBootstrapV4ComponentSource).not.toContain('@font-face');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fa,.fab,.fad,.fal,.far,.fas');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fa-500px:before');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fa-youtube:before');
    expect(adminBootstrapV4ComponentSource).not.toContain('.far{font-family:Font Awesome');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fab{font-family:Font Awesome');
    expect(adminBootstrapV4ComponentSource).not.toContain('.si{font-family:simple-line-icons');
    expect(adminBootstrapV4ComponentSource).not.toContain('.si-user:before');
    expect(adminBootstrapV4ComponentSource).not.toContain('.si-social-twitter:before');

    expect(adminIconFontsSource).toContain('@font-face');
    expect(adminIconFontsSource).toContain('font-family: "Font Awesome 5 Free";');
    expect(adminIconFontsSource).toContain('font-family: "Font Awesome 5 Brands";');
    expect(adminIconFontsSource).toContain('font-family: simple-line-icons;');
    expect(adminIconFontsSource).toContain('url("./static/fa-regular-400.ac21cac3.woff2")');
    expect(adminIconFontsSource).toContain('url("./static/fa-solid-900.d6d8d5da.woff2")');
    expect(adminIconFontsSource).toContain('url("./static/fa-brands-400.3e1b2a65.woff2")');
    expect(adminIconFontsSource).toContain('url("./static/Simple-Line-Icons.0cb0b9c5.woff2")');
    expect(adminIconFontsSource).toContain('.fa,.fab,.fad,.fal,.far,.fas');
    expect(adminIconFontsSource).toContain('.fa-500px:before');
    expect(adminIconFontsSource).toContain('.fa-youtube:before');
    expect(adminIconFontsSource).toContain('.far{font-family:Font Awesome');
    expect(adminIconFontsSource).toContain('.fab{font-family:Font Awesome');
    expect(adminIconFontsSource).toContain('.si{font-family:simple-line-icons');
    expect(adminIconFontsSource).toContain('.si-user:before');
    expect(adminIconFontsSource).toContain('.si-social-twitter:before');
    expect(adminIconFontsSource).not.toContain('[data-simplebar]');
    expect(adminIconFontsSource).not.toContain('/assets/admin/static/');
  });

  it('keeps admin animation utilities out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('.animated{animation-duration');
    expect(adminBootstrapV4ComponentSource).not.toContain('@keyframes bounce');
    expect(adminBootstrapV4ComponentSource).not.toContain('.bounce{animation-name:bounce');
    expect(adminBootstrapV4ComponentSource).not.toContain('@keyframes fadeIn');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fadeIn{animation-name:fadeIn');
    expect(adminBootstrapV4ComponentSource).not.toContain('@keyframes slideOutUp');
    expect(adminBootstrapV4ComponentSource).not.toContain('.slideOutUp{animation-name:slideOutUp');

    expect(adminAnimationsSource).toContain('.animated{animation-duration');
    expect(adminAnimationsSource).toContain('@keyframes bounce');
    expect(adminAnimationsSource).toContain('.bounce{animation-name:bounce');
    expect(adminAnimationsSource).toContain('@keyframes fadeIn');
    expect(adminAnimationsSource).toContain('.fadeIn{animation-name:fadeIn');
    expect(adminAnimationsSource).toContain('@keyframes slideOutUp');
    expect(adminAnimationsSource).toContain('.slideOutUp{animation-name:slideOutUp');
    expect(adminAnimationsSource).not.toContain('[data-simplebar]');
  });

  it('keeps OneUI utility color and typography classes out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('.font-w300{font-weight:300!important}');
    expect(adminBootstrapV4ComponentSource).not.toContain('.font-size-base{font-size:1rem!important}');
    expect(adminBootstrapV4ComponentSource).not.toContain('.tracking-widest{letter-spacing:.1em}');
    expect(adminBootstrapV4ComponentSource).not.toContain('.text-primary-dark{color:#054d9e!important}');
    expect(adminBootstrapV4ComponentSource).not.toContain('.text-xplay-lighter{color:#f3c2bc!important}');

    expect(adminOneuiUtilitiesSource).toContain('.font-w300{font-weight:300!important}');
    expect(adminOneuiUtilitiesSource).toContain('.font-size-base{font-size:1rem!important}');
    expect(adminOneuiUtilitiesSource).toContain('.tracking-widest{letter-spacing:.1em}');
    expect(adminOneuiUtilitiesSource).toContain('.text-primary-dark{color:#054d9e!important}');
    expect(adminOneuiUtilitiesSource).toContain('.text-xplay-lighter{color:#f3c2bc!important}');
    expect(adminOneuiUtilitiesSource).not.toContain('.animated{animation-duration');
    expect(adminOneuiUtilitiesSource).not.toContain('[data-simplebar]');
  });

  it('keeps admin plugin widget styles out of the Bootstrap v4 component layer', () => {
    expect(adminBootstrapV4ComponentSource).not.toContain('[data-simplebar]');
    expect(adminBootstrapV4ComponentSource).not.toContain('.simplebar-wrapper');
    expect(adminBootstrapV4ComponentSource).not.toContain('.datepicker');
    expect(adminBootstrapV4ComponentSource).not.toContain('.ck.ck-editor');
    expect(adminBootstrapV4ComponentSource).not.toContain('.dropzone');
    expect(adminBootstrapV4ComponentSource).not.toContain('table.dataTable');
    expect(adminBootstrapV4ComponentSource).not.toContain('.fc-theme-bootstrap');
    expect(adminBootstrapV4ComponentSource).not.toContain('.select2-container');
    expect(adminBootstrapV4ComponentSource).not.toContain('.slick-slider');
    expect(adminBootstrapV4ComponentSource).not.toContain('.flatpickr-weekdays');

    expect(adminPluginWidgetsSource).toContain('[data-simplebar]');
    expect(adminPluginWidgetsSource).toContain('.simplebar-wrapper');
    expect(adminPluginWidgetsSource).toContain('.datepicker');
    expect(adminPluginWidgetsSource).toContain('.ck.ck-editor');
    expect(adminPluginWidgetsSource).toContain('.dropzone');
    expect(adminPluginWidgetsSource).toContain('table.dataTable');
    expect(adminPluginWidgetsSource).toContain('.fc-theme-bootstrap');
    expect(adminPluginWidgetsSource).toContain('.select2-container');
    expect(adminPluginWidgetsSource).toContain('.slick-slider');
    expect(adminPluginWidgetsSource).toContain('.flatpickr-weekdays');
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
    expect(visualParitySource).toContain(
      'result.afterReload?.styleSnapshot?.capturedCount < 8',
    );
    expect(visualParitySource).toContain(
      '!result.afterReload?.styleSnapshot?.elements?.pageHeader?.backgroundColor',
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

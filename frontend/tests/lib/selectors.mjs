// Union selectors for the dual-world parity suite.
//
// One run(page) drives both the shadcn source build and the frozen antd oracle,
// so every selector must match both DOMs: shadcn slot/role/testid first, antd
// class fallback. Provenance: scratchpad admin-dom-map.

export const languageMenuItemSelector =
  '[data-slot="dropdown-menu-radio-item"], .ant-dropdown-menu-item';
export const userAuthSurfaceSelector = '[data-testid="auth-card"], .v2board-auth-box';
export const userAuthControlSelector = [
  '[data-testid="auth-card"] input',
  '[data-testid="auth-card"] select',
  '[data-testid="auth-card"] textarea',
  '.v2board-auth-box input',
  '.v2board-auth-box select',
  '.v2board-auth-box textarea',
].join(', ');
export const userAuthLinkSelector = [
  '[data-testid="auth-card"] a',
  '.v2board-auth-box a',
  '.bg-gray-lighter a',
].join(', ');
export const userAuthTitleTextSelector = [
  '[data-slot="auth-title"]',
  '.v2board-auth-box h1',
  '.v2board-auth-box h2',
  '.v2board-auth-box h3',
  '.v2board-auth-box .font-size-h1',
  '.v2board-auth-box p',
].join(', ');
export const adminAuthSurfaceSelector = '[data-testid="admin-login-surface"], .v2board-auth-box';
export const adminAuthControlSelector = [
  '[data-testid="admin-login-surface"] input',
  '[data-testid="admin-login-surface"] select',
  '[data-testid="admin-login-surface"] textarea',
  '.v2board-auth-box input',
  '.v2board-auth-box select',
  '.v2board-auth-box textarea',
].join(', ');
export const adminAuthIdentifierSelector = [
  '[data-testid="admin-login-surface"] input[type="email"]',
  '[data-testid="admin-login-surface"] input[type="text"]',
  '.v2board-auth-box input[type="email"]',
  '.v2board-auth-box input[type="text"]',
].join(', ');
export const adminAuthPasswordSelector = [
  '[data-testid="admin-login-surface"] input[type="password"]',
  '.v2board-auth-box input[type="password"]',
].join(', ');
export const adminAuthSubmitSelector =
  '[data-testid="admin-login-submit"], .v2board-auth-box button[type="submit"]';
export const adminAuthForgotSelector =
  '[data-testid="admin-forgot-password"], .v2board-auth-box .bg-gray-lighter a';
export const adminForgotDialogSelector =
  '[data-testid="admin-forgot-dialog"], .ant-modal-confirm, .ant-modal';
export const dashboardShortcutSelector =
  '[data-testid="dashboard-page"], [data-testid="dashboard-shortcut"], .content-heading, .block-title, .block-link-pop';
export const dashboardShortcutActionSelector =
  '[data-testid="dashboard-shortcut"], .block-link-pop';
export const orderDetailReadySelector =
  '[data-testid="order-info"], #cashier .block-content, #cashier .block-title';
export const dashboardSubscribeShortcutTexts = [
  '一键订阅',
  'One-click Subscription',
  'One-click subscription',
  '快速将节点导入对应客户端进行使用',
];
export const dashboardEmptyPlanReadySelector = '[data-testid="dashboard-empty-plan"], .fa-plus';
export const dashboardExpiredReadySelector =
  '[data-testid="dashboard-status-expired"], .text-danger';
export const dashboardProgressReadySelector =
  '[data-testid="dashboard-progress-bar"], .progress-bar';
export const dashboardTrafficUsedUpReadySelector =
  '[data-testid="dashboard-progress-bar"][data-status="danger"], .progress-bar.bg-danger';
export const userNodeRowsReadySelector = '[data-testid="node-table"] tbody tr, .ant-table-tbody tr';
// The API-500 scenario reaches the redesigned ErrorState rather than the empty
// subscription alert. Both are valid terminal surfaces; keep the legacy alert
// branch for the oracle without misclassifying source failures as empty data.
export const userNodeEmptyReadySelector =
  '[data-testid="node-empty"], [data-testid="node-error"], .alert.alert-dark';
export const userNodeLoadingReadySelector = '[data-testid="node-loading"], #page-container';
export const userTrafficRowsReadySelector =
  '[data-testid="traffic-table"] tbody tr, .ant-table-tbody tr';
export const userTrafficSurfaceReadySelector = '[data-testid="traffic-card"], .ant-table';

// Distribute a descendant combinator across both selector unions so
// `scope-a, scope-b` + `child-x, child-y` becomes the full cartesian product
// rather than mis-parsing as `scope-a` OR `scope-b child-x`.
export function scopedSelectorUnion(scopeSelector, childSelector) {
  const scopes = scopeSelector
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean);
  const children = childSelector
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean);
  return scopes.flatMap((scope) => children.map((child) => `${scope} ${child}`)).join(', ');
}

// Admin redesigned-vs-legacy union selectors.
export const adminDrawerOpenSelector = '.ant-drawer-open, [data-slot="sheet-content"]';
export const adminDialogOpenSelector = '.ant-modal, [data-slot="dialog-content"]';
export const adminOverlayOpenSelector = `${adminDrawerOpenSelector}, ${adminDialogOpenSelector}`;
export const adminFormInputSelector = '.ant-input, [data-slot="input"], [data-slot="textarea"]';
export const adminFormLabelSelector = 'label, [data-slot="label"], [data-slot="form-label"]';
export const adminFormFieldSelector =
  '.form-group, [data-slot="form-item"], .space-y-1\\.5, .space-y-2, .space-y-3';
export const adminSelectTriggerSelector =
  '.ant-select-selection, [data-slot="select-trigger"], [role="combobox"]';
export const adminSelectOptionSelector =
  '.ant-select-dropdown-menu-item, [data-slot="select-item"], [role="option"]';
export const adminSelectDropdownSelector = '.ant-select-dropdown, [data-slot="select-content"]';
export const adminTableRowSelector = '.ant-table-tbody tr, [data-slot="table-row"]';
export const adminMenuItemSelector =
  '.ant-dropdown-menu-item, [data-slot="dropdown-menu-item"], [role="menuitem"]';
export const adminSwitchSelector = '.ant-switch, [role="switch"], [data-slot="switch"]';
export const adminDrawerTitleSelector =
  '.ant-drawer-title, .ant-modal-title, [data-slot="sheet-title"], [data-slot="dialog-title"]';
export const adminDrawerInputSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  adminFormInputSelector,
);
export const adminDrawerInputGroupControlSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  '[data-slot="input-group-control"]',
);
export const adminDrawerSelectTriggerSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  adminSelectTriggerSelector,
);
export const adminDrawerLabelSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  'label, [data-slot="label"]',
);
export const adminDrawerLegendSelector = scopedSelectorUnion(adminOverlayOpenSelector, 'legend');
export const adminDrawerSelectedValueSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  '.ant-select-selection-selected-value, [data-slot="select-trigger"]:not([data-placeholder])',
);
export const adminDrawerFooterButtonSelector =
  '.ant-drawer-open .v2board-drawer-action .ant-btn, .ant-modal-footer .ant-btn, [data-slot="sheet-footer"] button, [data-slot="dialog-footer"] button';
export const adminConfirmDialogSelector = '[role="alertdialog"], .ant-modal-confirm';
export const adminConfirmPrimarySelector =
  '[role="alertdialog"] [data-slot="alert-dialog-action"], .ant-modal-confirm-btns .ant-btn-primary';
export const adminConfirmModalCountSelector =
  '[role="alertdialog"], [data-slot="dialog-content"], .ant-modal-confirm, .ant-modal';
export const adminConfirmButtonsSelector =
  '[role="alertdialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn';
export const adminConfirmContentSelector =
  '[data-slot="alert-dialog-description"], .ant-modal-confirm-content, .ant-modal-body';
export const adminConfirmTitleSelector =
  '[data-slot="alert-dialog-title"], .ant-modal-confirm-title, .ant-modal-title';
export const adminPlanCreateSelector = '.bg-white .ant-btn, [data-testid="plan-create"]';
export const adminPlanSubmitSelector =
  '.ant-drawer-open .v2board-drawer-action .ant-btn-primary, [data-testid="plan-submit"]';
export const adminPlanForceUpdateSelector =
  '.ant-drawer-open .ant-checkbox-wrapper, [data-testid="plan-force-update"]';
export const adminTableSwitchSelector =
  '.ant-table-tbody .ant-switch, [data-slot="table"] [data-slot="switch"], [data-slot="table"] [role="switch"]';
export const adminNodeAddTriggerSelector =
  '.v2board-table-action .ant-dropdown-trigger, [data-testid="node-add"]';
export const adminNodeSubmitSelector =
  '.v2board-drawer-action .ant-btn-primary, [data-testid="node-submit"]';
export const adminModalFooterButtonSelector =
  '.ant-modal-footer .ant-btn, [data-slot="dialog-footer"] button';
export const adminServerGroupSubmitSelector =
  '.ant-modal-footer .ant-btn-primary, [data-testid="server-group-submit"]';
export const adminServerRouteSubmitSelector =
  '.ant-modal-footer .ant-btn-primary, [data-testid="server-route-submit"]';
export const adminPaymentSaveSelector =
  '.ant-modal-footer .ant-btn-primary, [data-testid="payment-save"]';
export const adminConfigTabSelector = '.ant-tabs-tab, [data-testid^="config-tab-"]';
export const adminActiveConfigTabSelector =
  '.ant-tabs-tab-active, [data-testid^="config-tab-"][aria-current="page"]';
export const adminConfigFieldInputSelector =
  '.block.border-bottom input.form-control, .block.border-bottom textarea.form-control, input[data-testid^="config-"], textarea[data-testid^="config-"]';
export const adminTicketReplyInputSelector = '[data-testid="ticket-reply-input"], .js-chat-input';
export const adminTicketReplyFilterDropdownSelector =
  '.ant-table-filter-dropdown, [data-slot="dropdown-menu-content"]';
export const adminOrderMenuSelector = '.ant-dropdown, [data-slot="dropdown-menu-content"]';
export const adminOrderDetailRowSelector =
  '.ant-modal .ant-row, [data-testid="order-detail"] .divide-y > div';
export const adminOrderRowTriggerSelector =
  '.ant-table-tbody a, [data-testid^="order-status-trigger-"], [data-testid^="commission-status-trigger-"]';
export const adminOrderActivePageSelector =
  '.ant-pagination-item-active, [data-testid="order-page"][aria-current="page"]';
export const adminOrderPageItemSelector = '.ant-pagination-item, [data-testid="order-page"]';
export const adminDrawerTextareaSelector = scopedSelectorUnion(
  adminOverlayOpenSelector,
  'textarea.ant-input, [data-slot="textarea"]',
);
export const adminUserToolbarButtonSelector =
  '.v2board-table-action .ant-btn, [data-testid="user-filter-open"], [data-testid="user-bulk-actions"], [data-testid="user-create"], [data-testid="user-filter-reset"]';
export const adminUserRowActionTriggerSelector =
  '.ant-table-tbody a, [data-testid^="user-actions-"]';
export const adminUserPageItemSelector = '.ant-pagination-item, [data-testid="user-page"]';

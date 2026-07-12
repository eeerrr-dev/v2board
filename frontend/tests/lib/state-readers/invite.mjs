import {
  firstElementState,
  visibleCount,
  visibleInputValues,
  visibleTexts,
} from '../dom-helpers.mjs';

export async function inviteState(page) {
  return {
    generateButton: await firstElementState(page, '[data-testid="invite-generate"], .block-header .block-options .btn'),
    statBlocks: await visibleTexts(
      page,
      '[data-testid="invite-summary-card"], [data-testid="invite-stats-card"], .block-content.pb-3',
      4,
    ),
    tableRows: await visibleTexts(page, ':is([data-testid="invite-code-table"], [data-testid="invite-history-table"]) tbody tr, .ant-table-tbody tr', 10),
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
  };
}

export async function inviteFinanceDialogState(page) {
  return {
    buttons: await visibleTexts(
      page,
      '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn',
      4,
    ),
    dropdownItems: await visibleTexts(
      page,
      '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
      8,
    ),
    hash: await page.evaluate(() => window.location.hash),
    inputValues: await visibleInputValues(page, '[data-testid="invite-dialog"] input, .ant-modal input'),
    labels: await visibleTexts(page, '[data-testid="invite-dialog"] label, .ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '[data-testid="invite-dialog"], .ant-modal'),
    selectedValues: await visibleTexts(
      page,
      '[data-testid="invite-select-trigger"], .ant-modal .ant-select-selection-selected-value',
      4,
    ),
    tableRows: await visibleTexts(page, ':is([data-testid="invite-code-table"], [data-testid="invite-history-table"]) tbody tr, .ant-table-tbody tr', 4),
    titles: await visibleTexts(page, '[data-testid="invite-dialog-title"], .ant-modal-title', 2),
    toastTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
  };
}

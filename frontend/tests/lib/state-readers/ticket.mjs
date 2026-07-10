import {
  visibleTexts,
  visibleInputValues,
  visibleCount,
  firstInputValue,
  firstElementState,
} from '../dom-helpers.mjs';
import { adminTicketReplyInputSelector } from '../selectors.mjs';

export async function userTicketCreateModalState(page) {
  return {
    buttons: await visibleTexts(
      page,
      '[data-testid="ticket-dialog-footer"] button, .ant-modal-footer .ant-btn',
      4,
    ),
    inputValues: await visibleInputValues(
      page,
      '[data-testid="ticket-dialog"] input, [data-testid="ticket-dialog"] textarea, .ant-modal input, .ant-modal textarea',
    ),
    labels: await visibleTexts(page, '[data-testid="ticket-dialog"] label, .ant-modal .form-group label', 6),
    modalCount: await visibleCount(page, '[data-testid="ticket-dialog"], .ant-modal'),
    selectedValues: await visibleTexts(
      page,
      '[data-testid="ticket-select-trigger"], .ant-modal .ant-select-selection-selected-value',
      4,
    ),
    selectDropdownItems: await visibleTexts(
      page,
      '[data-testid="ticket-select-content"] [role="option"], .ant-select-dropdown-menu-item',
      6,
    ),
    tableRows: await visibleTexts(page, '[data-testid="ticket-table"] tbody tr, .ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '[data-testid="ticket-dialog-title"], .ant-modal-title', 2),
    toastTexts: await visibleTexts(page, '.v2board-toast-root, .ant-message-notice, .ant-notification-notice', 4),
  };
}

export async function ticketReplyState(page) {
  return {
    inputValue: await firstInputValue(page, adminTicketReplyInputSelector),
    messageTexts: await visibleTexts(
      page,
      '[data-testid="ticket-chat-messages"], .js-chat-messages',
      6,
    ),
    sendButton: await firstElementState(
      page,
      '[data-testid="ticket-reply-submit"], [data-testid="ticket-reply-send"], .js-chat-form button, .js-chat-form .ant-btn',
    ),
    toastTexts: await visibleTexts(page, '.v2board-toast-root, .ant-message-notice, .ant-notification-notice', 4),
  };
}

export async function userTicketListState(page) {
  return {
    actionLinks: await visibleTexts(page, '[data-testid="ticket-table"] button, .ant-table-tbody a', 8),
    closeCount: page.__visualParityUserTicketCloseCount ?? 0,
    hash: await page.evaluate(() => window.location.hash),
    tableRows: await visibleTexts(page, '[data-testid="ticket-table"] tbody tr, .ant-table-tbody tr', 6),
    toastTexts: await visibleTexts(page, '.v2board-toast-root, .ant-message-notice, .ant-notification-notice', 4),
  };
}

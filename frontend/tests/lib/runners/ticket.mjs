import {
  ticketReplyState,
  userTicketListState,
  userTicketCreateModalState,
} from '../state-readers/ticket.mjs';
import {
  fillFirstVisible,
  waitForPagePropertyAtLeast,
  clickFirstVisibleText,
  clickFirstVisible,
  fillVisibleAt,
  clickVisibleAt,
  waitForVisibleElementsHidden,
} from '../dom-helpers.mjs';
import { clonePageRequests } from '../json-util.mjs';

export async function runUserTicketReplySendInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  const replyInputSelector = '[data-testid="ticket-reply-input"], .js-chat-input';
  await fillFirstVisible(page, replyInputSelector, 'Parity reply send');
  await page.waitForTimeout(100);
  const filled = await ticketReplyState(page);

  await page.locator(replyInputSelector).first().press('Enter');
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const loading = await ticketReplyState(page);

  await waitForPagePropertyAtLeast(page, '__visualParityUserTicketReplyCount', 1);
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const sent = await ticketReplyState(page);

  return {
    filled,
    loading,
    replyRequests: (page.__visualParityUserTicketReplyRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    sent,
    ticketFetchDelta: (page.__visualParityUserTicketFetchCount ?? 0) - initialTicketFetchCount,
  };
}

export async function runUserTicketErrorMatrixInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  const initialReplyCount = page.__visualParityUserTicketReplyCount ?? 0;
  const initialCloseCount = page.__visualParityUserTicketCloseCount ?? 0;
  const replyInputSelector = '[data-testid="ticket-reply-input"], .js-chat-input';
  await fillFirstVisible(page, replyInputSelector, 'Parity failed reply');
  await page.waitForTimeout(100);
  const replyFilled = await ticketReplyState(page);
  await page.locator(replyInputSelector).first().press('Enter');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTicketReplyCount',
    initialReplyCount + 1,
  );
  await page.waitForTimeout(350);
  const replyFailed = await ticketReplyState(page);

  await page.evaluate(() => {
    window.__paritySpaNavigate('/ticket');
  });
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTicketFetchCount',
    initialTicketFetchCount + 1,
  );
  await page.waitForSelector('[data-testid="ticket-table"] tbody tr, .ant-table-tbody tr', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const listBeforeClose = await userTicketListState(page);
  const listFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  await clickFirstVisibleText(page, '[data-testid="ticket-close"], .ant-table-tbody a', [
    '关闭',
    'Close',
  ]);
  // The redesigned list guards close behind the shared confirm AlertDialog (closing a
  // ticket cannot be undone); the legacy oracle fires close directly on the link click.
  // Confirm the dialog when it appears so the close request fires on both. Do not wait
  // for it to hide -- this matrix rejects the close, and the shared dialog intentionally
  // stays open on a rejected onConfirm.
  const closeConfirmSelector =
    '[data-slot="alert-dialog-content"], .ant-modal-confirm, .ant-modal';
  const closeConfirmPrimarySelector =
    '[data-slot="alert-dialog-action"], .ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary';
  const closeConfirm = await page
    .waitForSelector(closeConfirmSelector, { state: 'visible', timeout: 1_500 })
    .catch(() => null);
  if (closeConfirm) {
    await clickFirstVisible(page, closeConfirmPrimarySelector);
  }
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTicketCloseCount',
    initialCloseCount + 1,
  );
  await page.waitForTimeout(350);
  const closeFailed = await userTicketListState(page);

  return {
    closeFailed,
    closeFetchDelta: (page.__visualParityUserTicketFetchCount ?? 0) - listFetchCount,
    closeRequests: clonePageRequests(page.__visualParityUserTicketCloseRequests),
    listBeforeClose,
    replyFailed,
    replyFilled,
    replyFetchDelta: listFetchCount - initialTicketFetchCount,
    replyRequests: clonePageRequests(page.__visualParityUserTicketReplyRequests),
  };
}

export async function runUserTicketCreateModalInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  const before = await userTicketCreateModalState(page);
  await clickFirstVisible(
    page,
    '[data-testid="ticket-new-trigger"], .block-header .block-options .btn, .block-header .block-options button',
  );
  await page.waitForSelector('[data-testid="ticket-dialog"], .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '[data-testid="ticket-dialog"] input, .ant-modal .ant-input', 0, 'Parity subject');
  await clickFirstVisible(page, '[data-testid="ticket-select-trigger"], .ant-modal .ant-select-selection');
  await page.waitForSelector('[data-testid="ticket-select-content"] [role="option"], .ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  const levelDropdown = await userTicketCreateModalState(page);
  await clickVisibleAt(page, '[data-testid="ticket-select-content"] [role="option"], .ant-select-dropdown-menu-item', 2);
  await waitForVisibleElementsHidden(page, '[data-testid="ticket-select-content"], .ant-select-dropdown');
  await fillVisibleAt(page, '[data-testid="ticket-dialog"] textarea, .ant-modal textarea.ant-input', 0, 'Parity ticket body');
  await page.waitForTimeout(100);
  const filled = await userTicketCreateModalState(page);
  await clickFirstVisible(page, '[data-testid="ticket-dialog-footer"] button:last-child, .ant-modal-footer .ant-btn-primary');
  await page.waitForTimeout(100);
  const saving = await userTicketCreateModalState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserTicketSaveCount', 1);
  await waitForVisibleElementsHidden(page, '[data-testid="ticket-dialog"], .ant-modal');
  await page.waitForTimeout(250);
  const saved = await userTicketCreateModalState(page);
  return {
    before,
    filled,
    levelDropdown,
    saveRequests: (page.__visualParityUserTicketSaveRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    saved,
    saving,
    ticketFetchDelta: (page.__visualParityUserTicketFetchCount ?? 0) - initialTicketFetchCount,
  };
}

export async function runUserTicketCreateValidationFailureInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  const initialTicketSaveCount = page.__visualParityUserTicketSaveCount ?? 0;
  const before = await userTicketCreateModalState(page);
  await clickFirstVisible(
    page,
    '[data-testid="ticket-new-trigger"], .block-header .block-options .btn, .block-header .block-options button',
  );
  await page.waitForSelector('[data-testid="ticket-dialog"], .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const opened = await userTicketCreateModalState(page);
  // The redesigned form validates client-side (zod min(1) on subject/message), so
  // an empty submit never fires a request. Fill valid fields so the save reaches
  // the server, where `ticketSaveError` rejects it -- exercising the server-error
  // path (save fires once, modal stays open, no list refetch) on both the source
  // and the legacy oracle, rather than the legacy-only "empty submit reaches the
  // server" path the redesigned surface intentionally blocks.
  await fillVisibleAt(page, '[data-testid="ticket-dialog"] input, .ant-modal .ant-input', 0, 'Parity subject');
  await clickFirstVisible(page, '[data-testid="ticket-select-trigger"], .ant-modal .ant-select-selection');
  await page.waitForSelector('[data-testid="ticket-select-content"] [role="option"], .ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  await clickVisibleAt(page, '[data-testid="ticket-select-content"] [role="option"], .ant-select-dropdown-menu-item', 2);
  await waitForVisibleElementsHidden(page, '[data-testid="ticket-select-content"], .ant-select-dropdown');
  await fillVisibleAt(page, '[data-testid="ticket-dialog"] textarea, .ant-modal textarea.ant-input', 0, 'Parity ticket body');
  await page.waitForTimeout(100);
  const filled = await userTicketCreateModalState(page);
  await clickFirstVisible(page, '[data-testid="ticket-dialog-footer"] button:last-child, .ant-modal-footer .ant-btn-primary');
  await page.waitForTimeout(100);
  const saving = await userTicketCreateModalState(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTicketSaveCount',
    initialTicketSaveCount + 1,
  );
  await page.waitForTimeout(250);
  const after = await userTicketCreateModalState(page);
  return {
    after,
    before,
    filled,
    opened,
    saveRequests: clonePageRequests(page.__visualParityUserTicketSaveRequests),
    saving,
    ticketFetchDelta: (page.__visualParityUserTicketFetchCount ?? 0) - initialTicketFetchCount,
  };
}

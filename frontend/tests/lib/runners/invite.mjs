import {
  clickFirstVisible,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  clickVisibleAt,
  fillVisibleAt,
  waitForPagePropertyAtLeast,
  waitForVisibleElementCountAtLeast,
  waitForVisibleElementsHidden,
  waitForVisibleText,
} from '../dom-helpers.mjs';
import { clonePageRequests } from '../json-util.mjs';
import { hoverAllTooltipTargetsInteraction } from '../tooltip-helpers.mjs';
import { inviteFinanceDialogState, inviteState } from '../state-readers/invite.mjs';

export async function runInviteGenerateInteraction(page) {
  const initialGenerateCount = page.__visualParityUserInviteGenerateCount ?? 0;
  const before = await inviteState(page);
  await clickFirstVisible(page, '[data-testid="invite-generate"], .block-header .block-options .btn');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserInviteGenerateCount',
    initialGenerateCount + 1,
  );
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const after = await inviteState(page);
  return {
    after,
    before,
    generateRequestDelta:
      (page.__visualParityUserInviteGenerateCount ?? 0) - initialGenerateCount,
  };
}

export async function runInviteTransferModalInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', ['划转', 'Transfer']);
  await page.waitForSelector('[data-testid="invite-dialog"], .ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const opened = await inviteFinanceDialogState(page);
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input:not([disabled])', 0, '12.34');
  await page.waitForTimeout(100);
  const filled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await page.waitForTimeout(100);
  const saving = await inviteFinanceDialogState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserTransferCount', 1);
  await waitForVisibleElementsHidden(page, '[data-testid="invite-dialog"], .ant-modal');
  await page.waitForTimeout(250);
  const closed = await inviteFinanceDialogState(page);
  return {
    before,
    closed,
    filled,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
    saving,
    transferRequests: clonePageRequests(page.__visualParityUserTransferRequests),
  };
}

export async function runInviteTransferFailureInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialTransferCount = page.__visualParityUserTransferCount ?? 0;
  const before = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', ['划转', 'Transfer']);
  await page.waitForSelector('[data-testid="invite-dialog"], .ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const opened = await inviteFinanceDialogState(page);
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input:not([disabled])', 0, '99999.99');
  await page.waitForTimeout(100);
  const filled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await page.waitForTimeout(100);
  const saving = await inviteFinanceDialogState(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTransferCount',
    initialTransferCount + 1,
  );
  await page.waitForTimeout(250);
  const after = await inviteFinanceDialogState(page);
  return {
    after,
    before,
    filled,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
    saving,
    transferRequests: clonePageRequests(page.__visualParityUserTransferRequests),
  };
}

export async function runInviteWithdrawModalInteraction(page) {
  const before = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', [
    '推广佣金提现',
    'Invitation Commission Withdrawal',
  ]);
  await page.waitForSelector('[data-testid="invite-dialog"], .ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const _opened = await inviteFinanceDialogState(page);
  await clickFirstVisible(page, '[data-testid="invite-select-trigger"], .ant-modal .ant-select-selection');
  await page.waitForSelector('[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const dropdown = await inviteFinanceDialogState(page);
  await clickFirstVisibleTextStable(
    page,
    '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
    ['Alipay'],
  );
  await waitForVisibleElementsHidden(page, '[data-testid="invite-select-content"], .ant-select-dropdown');
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input.ant-input', 0, 'parity-account@example.com');
  await page.waitForTimeout(100);
  const filled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await page.waitForTimeout(100);
  const saving = await inviteFinanceDialogState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserWithdrawCount', 1);
  await page.waitForFunction(() => window.__parityReadSpaRoute().includes('/ticket'), null, {
    timeout: 5_000,
  });
  await page.waitForTimeout(250);
  const navigated = await inviteFinanceDialogState(page);
  return {
    before,
    dropdown,
    filled,
    navigated,
    saving,
    withdrawRequests: clonePageRequests(page.__visualParityUserWithdrawRequests),
  };
}

export async function runInviteFinanceSubmitMatrixInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialTransferCount = page.__visualParityUserTransferCount ?? 0;
  const initialWithdrawCount = page.__visualParityUserWithdrawCount ?? 0;
  const before = await inviteFinanceDialogState(page);

  await clickFirstVisibleText(page, 'button, .ant-btn', ['划转', 'Transfer']);
  await waitForVisibleElementCountAtLeast(page, '[data-testid="invite-dialog"], .ant-modal', 1);
  const transferEmptyOpened = await inviteFinanceDialogState(page);
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input:not([disabled])', 0, '12.34');
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserTransferCount',
    initialTransferCount + 1,
  );
  await waitForVisibleElementsHidden(page, '[data-testid="invite-dialog"], .ant-modal');
  await page.waitForTimeout(250);
  const transferEmptyClosed = await inviteFinanceDialogState(page);

  await clickFirstVisibleText(page, 'button, .ant-btn', [
    '推广佣金提现',
    'Invitation Commission Withdrawal',
  ]);
  await waitForVisibleElementCountAtLeast(page, '[data-testid="invite-dialog"], .ant-modal', 1);
  const withdrawOpened = await inviteFinanceDialogState(page);
  await clickFirstVisible(page, '[data-testid="invite-select-trigger"], .ant-modal .ant-select-selection');
  await waitForVisibleText(
    page,
    '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
    'Alipay',
  );
  const withdrawDropdown = await inviteFinanceDialogState(page);
  await clickFirstVisibleTextStable(
    page,
    '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
    ['Alipay'],
  );
  await waitForVisibleElementsHidden(page, '[data-testid="invite-select-content"], .ant-select-dropdown');
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input.ant-input', 0, 'fail-account');
  const withdrawFailureFilled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserWithdrawCount',
    initialWithdrawCount + 1,
  );
  await page.waitForTimeout(350);
  const withdrawFailed = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '[data-testid="invite-dialog"], .ant-modal');

  await clickFirstVisibleText(page, 'button, .ant-btn', [
    '推广佣金提现',
    'Invitation Commission Withdrawal',
  ]);
  await waitForVisibleElementCountAtLeast(page, '[data-testid="invite-dialog"], .ant-modal', 1);
  await clickFirstVisible(page, '[data-testid="invite-select-trigger"], .ant-modal .ant-select-selection');
  await waitForVisibleText(
    page,
    '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
    'USDT',
  );
  await clickFirstVisibleTextStable(
    page,
    '[data-testid="invite-select-content"] [role="option"], .ant-select-dropdown-menu-item',
    ['USDT'],
  );
  await waitForVisibleElementsHidden(page, '[data-testid="invite-select-content"], .ant-select-dropdown');
  await fillVisibleAt(page, '[data-testid="invite-dialog"] input:not([disabled]), .ant-modal input.ant-input', 0, 'success-account');
  const withdrawSuccessFilled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '[data-testid="invite-dialog-footer"] button, .ant-modal-footer .ant-btn', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserWithdrawCount',
    initialWithdrawCount + 2,
  );
  await page.waitForFunction(() => window.__parityReadSpaRoute().includes('/ticket'), null, {
    timeout: 5_000,
  });
  await page.waitForTimeout(250);
  const withdrawSucceeded = await inviteFinanceDialogState(page);

  return {
    before,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    transferEmptyClosed,
    transferEmptyOpened,
    transferRequests: clonePageRequests(page.__visualParityUserTransferRequests),
    withdrawDropdown,
    withdrawFailed,
    withdrawFailureFilled,
    withdrawOpened,
    withdrawRequests: clonePageRequests(page.__visualParityUserWithdrawRequests),
    withdrawSuccessFilled,
    withdrawSucceeded,
  };
}

export async function runUserInviteTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, [
    '[data-testid="invite-surface"] [data-slot="header-tooltip-trigger"]',
    '.anticon-question-circle',
  ]);
}

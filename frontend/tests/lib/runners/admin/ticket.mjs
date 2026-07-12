import { fillFirstVisible, waitForPagePropertyAtLeast } from '../../dom-helpers.mjs';
import { clonePageRequests } from '../../json-util.mjs';
import {
  adminTicketReplyInputSelector,
  adminTicketReplyFilterDropdownSelector,
} from '../../selectors.mjs';
import { ticketReplyState } from '../../state-readers/ticket.mjs';
import {
  adminTicketsReplyFilterState,
  openAdminTicketsReplyFilter,
  confirmAdminTicketsReplyFilter,
  clickAdminTicketsReplyFilterOption,
} from '../../state-readers/admin.mjs';

export async function runAdminTicketReplySendInteraction(page) {
  const initialTicketFetchCount = page.__visualParityAdminTicketFetchCount ?? 0;
  await fillFirstVisible(page, adminTicketReplyInputSelector, 'Parity admin reply send');
  await page.waitForTimeout(100);
  const filled = await ticketReplyState(page);

  await page.locator(adminTicketReplyInputSelector).first().press('Enter');
  await page.waitForSelector('[data-sonner-toast], .ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const loading = await ticketReplyState(page);

  await waitForPagePropertyAtLeast(page, '__visualParityAdminTicketReplyCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminTicketFetchCount',
    initialTicketFetchCount + 1,
  );
  await page.waitForTimeout(150);
  const sent = await ticketReplyState(page);

  return {
    filled,
    loading,
    replyRequests: (page.__visualParityAdminTicketReplyRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    sent,
    ticketFetchDelta: (page.__visualParityAdminTicketFetchCount ?? 0) - initialTicketFetchCount,
  };
}

export async function runAdminTicketsReplyFilterInteraction(page) {
  const before = await adminTicketsReplyFilterState(page);
  await openAdminTicketsReplyFilter(page);
  await page.waitForSelector(adminTicketReplyFilterDropdownSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  const opened = await adminTicketsReplyFilterState(page);
  // Capture the fetch count before selecting the option: the redesigned checkbox
  // item refetches immediately on toggle while the antd filter refetches only on
  // the 确定 confirm, so counting/slicing from here captures the reply_status
  // fetch on both DOMs.
  const initialTicketFetchCount = page.__visualParityAdminTicketFetchCount ?? 0;
  await clickAdminTicketsReplyFilterOption(page, '待回复');
  await page.waitForTimeout(100);
  const selected = await adminTicketsReplyFilterState(page);
  await confirmAdminTicketsReplyFilter(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminTicketFetchCount',
    initialTicketFetchCount + 1,
  );
  await page.waitForTimeout(300);
  const confirmed = await adminTicketsReplyFilterState(page);
  return {
    before,
    confirmed,
    filterFetchRequests: clonePageRequests(page.__visualParityAdminTicketFetchRequests).slice(
      initialTicketFetchCount,
    ),
    opened,
    selected,
  };
}

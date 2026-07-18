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

  const staffMirror = await driveStaffTicketMirror(page);

  return {
    filled,
    loading,
    replyRequests: (page.__visualParityAdminTicketReplyRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    sent,
    staffMirror,
    ticketFetchDelta: (page.__visualParityAdminTicketFetchCount ?? 0) - initialTicketFetchCount,
  };
}

// W14 (§6.9): the staff namespace mirrors the admin ticket resources under
// its own /api/v1/staff prefix in both dialects. No SPA surface drives it, so
// the scenario exercises the mirror directly in each world's wire dialect
// (the modern resource rows on the source side, the legacy action spellings
// on the oracle side); the fixture layer canonicalizes the captures so the
// cross-world comparison proves both prefixes carry one Tier-1 contract.
async function driveStaffTicketMirror(page) {
  const modern = page.__parityWorld === 'source';
  const responses = await page.evaluate(
    async ({ modern }) => {
      const send = async (method, url, body, form) => {
        const headers = {};
        let payload;
        if (body !== undefined) {
          if (form) {
            headers['Content-Type'] = 'application/x-www-form-urlencoded';
            payload = new URLSearchParams(body).toString();
          } else {
            headers['Content-Type'] = 'application/json';
            payload = JSON.stringify(body);
          }
        }
        const response = await fetch(url, { body: payload, headers, method });
        const data = await response.json().catch(() => null);
        return { data, ok: response.ok };
      };
      if (modern) {
        const list = await send('GET', '/api/v1/staff/tickets?page=1&per_page=10');
        const detail = await send('GET', '/api/v1/staff/tickets/7');
        const reply = await send('POST', '/api/v1/staff/tickets/7/replies', {
          message: 'Parity staff reply',
        });
        const close = await send('POST', '/api/v1/staff/tickets/7/close');
        return {
          closeOk: close.ok,
          detailId: detail.data?.id ?? null,
          detailMessageCount: Array.isArray(detail.data?.message) ? detail.data.message.length : 0,
          listIds: (list.data?.items ?? []).map((ticket) => ticket.id),
          replyOk: reply.ok,
        };
      }
      const list = await send('GET', '/api/v1/staff/ticket/fetch?current=1&pageSize=10');
      const detail = await send('GET', '/api/v1/staff/ticket/fetch?id=7');
      const reply = await send(
        'POST',
        '/api/v1/staff/ticket/reply',
        { id: '7', message: 'Parity staff reply' },
        true,
      );
      const close = await send('POST', '/api/v1/staff/ticket/close', { id: '7' }, true);
      return {
        closeOk: close.ok,
        detailId: detail.data?.data?.id ?? null,
        detailMessageCount: Array.isArray(detail.data?.data?.message)
          ? detail.data.data.message.length
          : 0,
        listIds: (list.data?.data ?? []).map((ticket) => ticket.id),
        replyOk: reply.ok,
      };
    },
    { modern },
  );
  return {
    requests: clonePageRequests(page.__visualParityStaffTicketRequests ?? []),
    responses,
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

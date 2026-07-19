import { visibleCount, visibleTexts } from '../dom-helpers.mjs';
import { toPresenceTokens } from '../json-util.mjs';

export async function fetchFailureState(page) {
  const alertTexts = await visibleTexts(
    page,
    // Redesigned user surfaces render the shared ErrorState on fetch failure. It is
    // a Radix-style alert (role="alert", no literal `.alert` class), so capture its
    // per-surface testids: plans has no card fallback in `tables`, and this keeps the
    // failure state observable for the collapsed redesigned-fetch-failure normalizer.
    '.alert, .ant-alert, [data-testid="plan-error"], [data-testid="orders-error"], [data-testid="ticket-error"], [data-testid="node-error"], [data-testid="traffic-error"], [data-testid="knowledge-list-error"]',
    6,
  );
  const emptyTexts = await visibleTexts(
    page,
    '[data-testid="plan-empty"], [data-testid="orders-empty"], [data-testid="node-empty"], [data-testid="traffic-empty"], [data-testid="ticket-empty"], [data-testid="knowledge-empty"], .ant-empty, .ant-table-placeholder',
    6,
  );
  const listItemTexts = await visibleTexts(page, '.am-list-item', 6);
  const tablePlaceholderTexts = await visibleTexts(
    page,
    '[data-testid="orders-empty"], [data-testid="node-empty"], [data-testid="traffic-empty"], [data-testid="ticket-empty"], [data-testid="knowledge-empty"], .ant-table-placeholder',
    4,
  );
  const tableRows = await visibleTexts(
    page,
    '[data-testid="orders-table"] tbody tr, [data-testid="node-table"] tbody tr, [data-testid="traffic-table"] tbody tr, [data-testid="ticket-table"] tbody tr, .ant-table-tbody tr',
    6,
  );
  const legacyBlockLoadingCount = await visibleCount(page, '.block-mode-loading');
  const spinnerVisibleCount = await visibleCount(
    page,
    '[data-testid="plan-empty"] svg, [data-testid="orders-card"] svg, [data-testid="node-loading"] svg, [data-testid="traffic-card"] [role="status"] svg, .spinner-grow, .ant-spin-spinning, [role="status"] svg, [data-slot="skeleton"]',
  );

  return {
    alertTexts: toPresenceTokens(alertTexts, 'alert'),
    blockLoadingCount: 0,
    emptyTexts: toPresenceTokens(emptyTexts, 'empty'),
    hash: await page.evaluate(() => window.__parityReadSpaRoute()),
    listItemTexts: toPresenceTokens(listItemTexts, 'list-item'),
    requestSeen: {
      adminCouponFetch: (page.__visualParityAdminCouponFetchCount ?? 0) > 0,
      adminGiftcardFetch: (page.__visualParityAdminGiftcardFetchCount ?? 0) > 0,
      adminKnowledgeFetch: (page.__visualParityAdminKnowledgeFetchCount ?? 0) > 0,
      adminNoticeFetch: (page.__visualParityAdminNoticeFetchCount ?? 0) > 0,
      adminOrderFetch: (page.__visualParityAdminOrderFetchCount ?? 0) > 0,
      adminPaymentFetch: (page.__visualParityAdminPaymentFetchCount ?? 0) > 0,
      adminPlanFetch: (page.__visualParityAdminPlanFetchCount ?? 0) > 0,
      adminServerNodeFetch: (page.__visualParityAdminServerNodeFetchCount ?? 0) > 0,
      adminTicketFetch: (page.__visualParityAdminTicketFetchCount ?? 0) > 0,
      adminUserFetch: (page.__visualParityAdminUserFetchCount ?? 0) > 0,
      userKnowledgeFetch: (page.__visualParityUserKnowledgeFetchCount ?? 0) > 0,
      userOrderFetch: (page.__visualParityUserOrderFetchCount ?? 0) > 0,
      userPlanFetch: (page.__visualParityUserPlanFetchCount ?? 0) > 0,
      userServerFetch: (page.__visualParityUserServerFetchCount ?? 0) > 0,
      userTicketFetch: (page.__visualParityUserTicketFetchCount ?? 0) > 0,
      userTrafficFetch: (page.__visualParityUserTrafficFetchCount ?? 0) > 0,
    },
    spinnerCount: legacyBlockLoadingCount + spinnerVisibleCount > 0 ? 1 : 0,
    tablePlaceholderTexts: toPresenceTokens(tablePlaceholderTexts, 'table-placeholder'),
    tableRows: toPresenceTokens(tableRows, 'table-row'),
    tables: await page.evaluate(() => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(
        document.querySelectorAll(
          '[data-testid="orders-card"], [data-testid="node-card"], [data-testid="traffic-card"], [data-testid="ticket-surface"], [data-slot="table"], .ant-table',
        ),
      )
        .filter(isVisible)
        .slice(0, 4)
        .map(() => 'table');
    }),
  };
}

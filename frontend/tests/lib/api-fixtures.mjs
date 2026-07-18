import { adminPath } from './env.mjs';
import { emitFixtureResponse } from './dialect/fixture-emitters.mjs';
import { canonicalizeRequest } from './dialect/request-canonicalizer.mjs';
import {
  entryUrlForDialect,
  routingDialectFor,
} from './dialect/page-location-canonicalizer.mjs';
import {
  adminConfigFixture,
  adminCouponFixtures,
  adminCouponStoreFixtures,
  adminEmailTemplateFixtures,
  adminGiftcardFixtures,
  adminGiftcardStoreFixtures,
  adminKnowledgeFixtures,
  adminNoticeFixtures,
  adminOrderFixtures,
  adminOrderStatFixtures,
  adminPaymentFixtures,
  adminPaymentFormFixtures,
  adminPaymentMethodsFixture,
  adminPlanStoreFixtures,
  adminQueueStatsFixture,
  adminQueueWorkloadFixtures,
  adminServerGroupFixtures,
  adminServerNodeFixtures,
  adminServerRankFixtures,
  adminServerRouteFixtures,
  adminStatFixture,
  adminTicketDetailFixture,
  adminTicketFixtures,
  adminUserFixtures,
  adminUserRankFixtures,
  adminUserStoreFixtures,
  adminUserTrafficFixtures,
  bannedUserInfoFixture,
  couponCheckFixture,
  dashboardResetPackageOrderFixture,
  dashboardResetPackageTradeNo,
  deviceLimitExpiredSubscribeFixture,
  deviceLimitReachedSubscribeFixture,
  expiredSubscriptionFixture,
  expiredTrafficUsedUpSubscribeFixture,
  extremeKnowledgeFixtures,
  guestConfigFixture,
  inviteDetailFixtures,
  inviteFixture,
  knowledgeFixtures,
  longAdminOrderFixtures,
  longAdminServerNodeFixtures,
  longAdminUserFixtures,
  longOrderFixtures,
  longPlanFixtures,
  longTicketDetailFixture,
  longTicketFixtures,
  longUserServerFixtures,
  newPeriodSubscribeFixture,
  noSubscriptionFixture,
  noticeFixtures,
  orderFixtures,
  paymentMethodFixtures,
  planFixtures,
  profileDepositOrderFixture,
  profileDepositTradeNo,
  serverFixtures,
  subscribeFixture,
  ticketDetailFixture,
  ticketFixtures,
  toAdminPlanStoreFixtures,
  toAdminUserStoreFixtures,
  trafficFixtures,
  trafficUsedUpSubscribeFixture,
  userCommConfigFixture,
  userInfoFixture,
} from './fixture-data.mjs';

export async function installApiFixtures(page, scenario, target, interaction = {}) {
  const isAdminScenario = scenario.label.startsWith('admin-');
  const effectiveLocale = scenario.locale ?? (isAdminScenario ? '' : 'zh-CN');
  let seededAdminTicketDetailStore = false;
  let resolveAdminGroupsReady;
  const adminGroupsReady = new Promise((resolve) => {
    resolveAdminGroupsReady = resolve;
  });
  let adminGroupsResolved = false;

  await page.addInitScript(
    ({ authenticated, darkMode, locale, preserveRuntimeDarkMode, preserveRuntimeLocale, world }) => {
      const initializeDarkModeCookie = () => {
        document.cookie = darkMode
          ? 'dark_mode=1;path=/'
          : 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
      };
      // World-aware locale seed (docs/api-dialect.md §11): the source world
      // persists only the canonical v2board_locale key; the oracle keeps its
      // legacy umi_locale/g_lang/i18n-cookie trio.
      const initializeLocale = () => {
        if (!locale) return;
        if (world === 'source') {
          window.localStorage.setItem('v2board_locale', locale);
          return;
        }
        window.g_lang = locale;
        window.g_langSeparator = '-';
        window.localStorage.setItem('umi_locale', locale);
        document.cookie = `i18n=${encodeURIComponent(locale)};path=/`;
      };
      if (preserveRuntimeDarkMode) {
        const marker = 'v2board_visual_parity_dark_mode_initialized';
        if (!window.sessionStorage.getItem(marker)) {
          initializeDarkModeCookie();
          window.sessionStorage.setItem(marker, '1');
        }
      } else {
        initializeDarkModeCookie();
      }
      if (authenticated) {
        window.localStorage.setItem('authorization', 'VISUAL_PARITY_TOKEN');
      } else {
        window.localStorage.removeItem('authorization');
      }
      if (locale) {
        if (preserveRuntimeLocale) {
          const marker = 'v2board_visual_parity_locale_initialized';
          if (!window.sessionStorage.getItem(marker)) {
            initializeLocale();
            window.sessionStorage.setItem(marker, '1');
          }
        } else {
          initializeLocale();
        }
      }
    },
    {
      authenticated: Boolean(scenario.authenticated),
      darkMode: Boolean(scenario.darkMode),
      locale: effectiveLocale,
      preserveRuntimeDarkMode: Boolean(interaction.preserveRuntimeDarkMode),
      preserveRuntimeLocale: Boolean(interaction.preserveRuntimeLocale),
      world: target,
    },
  );

  await page.route('https://js.stripe.com/**', (route) => {
    route.fulfill({
      body: stripeFixtureScript({
        confirmError: interaction.stripeConfirmError,
        paymentElementComplete: interaction.stripePaymentElementComplete,
        // Oracle-only: the frozen bundle still calls createToken. The modern
        // source receives no token and must complete through confirmPayment.
        legacyToken: target === 'oracle' ? interaction.legacyOracleStripeToken : null,
      }),
      contentType: 'application/javascript',
      status: 200,
    });
  });

  await page.route('**/monitor/api/stats', (route) => {
    fulfillPlainJson(route, { status: 'running' });
  });

  await page.route('**/api/v1/**', async (route) => {
    const requestUrl = new URL(route.request().url());
    const pathname = requestUrl.pathname;
    const requestMethod = route.request().method();
    page.__visualParityDiagnostics?.push(`${requestMethod} ${pathname}`);
    const adminEndpoint = adminFixtureEndpoint(pathname);
    // W5 (§5.3/§5.4): the modern profile family splits reads and writes by
    // verb on shared paths. Match both worlds' spellings for the counters,
    // captures, and delay knobs below.
    const isUserProfileGet =
      pathname === '/api/v1/user/info' ||
      (pathname === '/api/v1/user/profile' && requestMethod === 'GET');
    const isUserProfileUpdate =
      pathname === '/api/v1/user/update' ||
      (pathname === '/api/v1/user/profile' && requestMethod === 'PATCH');
    const isUserPasswordUpdate =
      pathname === '/api/v1/user/changePassword' || pathname === '/api/v1/user/password';
    const isUserRedeemGiftcard =
      pathname === '/api/v1/user/redeemgiftcard' ||
      pathname === '/api/v1/user/gift-card-redemptions';
    const isUserUnbindTelegram =
      pathname === '/api/v1/user/unbindTelegram' || pathname === '/api/v1/user/telegram-binding';
    const isUserResetSecurity =
      pathname === '/api/v1/user/resetSecurity' ||
      pathname === '/api/v1/user/subscription/reset-token';
    const isUserSubscribeGet =
      pathname === '/api/v1/user/getSubscribe' || pathname === '/api/v1/user/subscription';
    const isUserNewPeriod =
      pathname === '/api/v1/user/newPeriod' ||
      pathname === '/api/v1/user/subscription/new-period';
    // W7 (§5.6): the modern invite family splits the legacy GET actions into
    // POST creates on new paths. Match both worlds' spellings for the
    // counters, captures, and delay knobs below.
    const isUserTransfer =
      pathname === '/api/v1/user/transfer' || pathname === '/api/v1/user/commission-transfers';
    const isUserInviteGenerate =
      pathname === '/api/v1/user/invite/save' || pathname === '/api/v1/user/invite-codes';
    // W8 (§5.7): the modern ticket family carries identity in the path.
    // Match both worlds' spellings for the counters, captures, timeout, and
    // delay knobs below; the fetch counter spans list + detail exactly like
    // the shared legacy /user/ticket/fetch path did.
    const isUserTicketFetch =
      pathname === '/api/v1/user/ticket/fetch' ||
      (requestMethod === 'GET' &&
        (pathname === '/api/v1/user/tickets' ||
          /^\/api\/v1\/user\/tickets\/[^/]+$/.test(pathname)));
    const isUserTicketSave =
      pathname === '/api/v1/user/ticket/save' ||
      (pathname === '/api/v1/user/tickets' && requestMethod === 'POST');
    const isUserTicketReply =
      pathname === '/api/v1/user/ticket/reply' ||
      /^\/api\/v1\/user\/tickets\/[^/]+\/replies$/.test(pathname);
    const isUserTicketClose =
      pathname === '/api/v1/user/ticket/close' ||
      /^\/api\/v1\/user\/tickets\/[^/]+\/close$/.test(pathname);
    const isUserWithdraw =
      pathname === '/api/v1/user/ticket/withdraw' ||
      pathname === '/api/v1/user/withdrawal-tickets';
    // W9 (§6.1): the modern admin config family splits GET vs PATCH on one
    // /{secure_path}/config row; match both worlds' spellings for the
    // counters, captures, and delay knobs below.
    const isAdminConfigFetch =
      adminEndpoint === '/config/fetch' ||
      (adminEndpoint === '/config' && requestMethod === 'GET');
    const isAdminConfigSave =
      adminEndpoint === '/config/save' ||
      (adminEndpoint === '/config' && requestMethod === 'PATCH');

    if (adminEndpoint) {
      page.__visualParityDiagnostics?.push(`fixture admin ${adminEndpoint}`);
    } else if (pathname === '/api/v1/user/checkLogin' || pathname === '/api/v1/auth/session') {
      page.__visualParityDiagnostics?.push(`fixture checkLogin admin=${isAdminScenario}`);
    } else if (isUserProfileGet) {
      page.__visualParityDiagnostics?.push('fixture user info');
    }

    if (adminEndpoint === '/server/manage/getNodes') {
      await waitForAdminGroups(adminGroupsReady);
    }
    const requestData = readRequestData(route.request());
    // W4 (§5.5) / W5 (§5.3): the modern families carry identity in the path
    // and booleans in JSON bodies. Match both worlds' spellings and capture
    // these requests in the canonical dialect shape so cross-world comparison
    // sees one contract.
    const modernOrderActionMatch =
      /^\/api\/v1\/user\/orders\/[^/]+\/(status|cancel|checkout|stripe-intent)$/.exec(pathname);
    const modernOrderAction = modernOrderActionMatch?.[1] ?? null;
    const canonicalRequestCapture = () => {
      const { params, body } = canonicalizeRequest(target, {
        method: requestMethod,
        url: route.request().url(),
        postData: route.request().postData(),
        securePath: adminPath,
      });
      const bodyFields = body && typeof body === 'object' && !Array.isArray(body) ? body : {};
      return { ...bodyFields, ...params };
    };
    const adminServerNodeSaveMatch = /^\/server\/([^/]+)\/save$/.exec(adminEndpoint ?? '');
    // W10 (§6.3): the modern admin content family carries identity in the
    // path — creates POST the collection, edits/toggles PATCH /{id}, deletes
    // DELETE /{id}; the show toggle is the PATCH whose body is exactly
    // {show}. Match both worlds' spellings for the counters, captures,
    // timeout, and delay knobs below.
    const isShowOnlyPatch =
      requestMethod === 'PATCH' &&
      requestData != null &&
      Object.keys(requestData).length === 1 &&
      'show' in requestData;
    const isAdminNoticeFetch =
      adminEndpoint === '/notice/fetch' ||
      (adminEndpoint === '/notices' && requestMethod === 'GET');
    const isAdminNoticeSave =
      adminEndpoint === '/notice/save' ||
      (adminEndpoint === '/notices' && requestMethod === 'POST') ||
      (/^\/notices\/\d+$/.test(adminEndpoint ?? '') &&
        requestMethod === 'PATCH' &&
        !isShowOnlyPatch);
    const isAdminNoticeShow =
      adminEndpoint === '/notice/show' ||
      (/^\/notices\/\d+$/.test(adminEndpoint ?? '') && isShowOnlyPatch);
    const isAdminNoticeDrop =
      adminEndpoint === '/notice/drop' ||
      (/^\/notices\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'DELETE');
    const isAdminKnowledgeListFetch =
      (adminEndpoint === '/knowledge/fetch' && !requestUrl.searchParams.has('id')) ||
      (adminEndpoint === '/knowledge' && requestMethod === 'GET');
    const isAdminKnowledgeSave =
      adminEndpoint === '/knowledge/save' ||
      (adminEndpoint === '/knowledge' && requestMethod === 'POST') ||
      (/^\/knowledge\/\d+$/.test(adminEndpoint ?? '') &&
        requestMethod === 'PATCH' &&
        !isShowOnlyPatch);
    const isAdminCouponFetch =
      adminEndpoint === '/coupon/fetch' ||
      (adminEndpoint === '/coupons' && requestMethod === 'GET');
    const isAdminCouponGenerate =
      adminEndpoint === '/coupon/generate' ||
      (adminEndpoint === '/coupons' && requestMethod === 'POST') ||
      (/^\/coupons\/\d+$/.test(adminEndpoint ?? '') &&
        requestMethod === 'PATCH' &&
        !isShowOnlyPatch);
    const isAdminGiftcardFetch =
      adminEndpoint === '/giftcard/fetch' ||
      (adminEndpoint === '/gift-cards' && requestMethod === 'GET');
    const isAdminGiftcardGenerate =
      adminEndpoint === '/giftcard/generate' ||
      (adminEndpoint === '/gift-cards' && requestMethod === 'POST') ||
      (/^\/gift-cards\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'PATCH');
    // W11 (§6.2/§6.4): the modern admin commerce family carries identity in the
    // path — plan/payment creates POST the collection, edits PATCH /{id}
    // (the show/renew/enable toggle is the single-flag PATCH), deletes DELETE
    // /{id}, sort POSTs /{collection}/sort; orders standardize on trade_no
    // path identity. Match both worlds' spellings for the counters, captures,
    // timeout, and delay knobs below.
    const isSingleFlagPatch = (flag) =>
      requestMethod === 'PATCH' &&
      requestData != null &&
      Object.keys(requestData).length === 1 &&
      flag in requestData;
    const isPlanTogglePatch = isSingleFlagPatch('show') || isSingleFlagPatch('renew');
    const isAdminPlanFetch =
      adminEndpoint === '/plan/fetch' ||
      (adminEndpoint === '/plans' && requestMethod === 'GET');
    const isAdminPlanSave =
      adminEndpoint === '/plan/save' ||
      (adminEndpoint === '/plans' && requestMethod === 'POST') ||
      (/^\/plans\/\d+$/.test(adminEndpoint ?? '') &&
        requestMethod === 'PATCH' &&
        !isPlanTogglePatch);
    const isAdminPlanUpdate =
      adminEndpoint === '/plan/update' ||
      (/^\/plans\/\d+$/.test(adminEndpoint ?? '') && isPlanTogglePatch);
    const isAdminPlanDrop =
      adminEndpoint === '/plan/drop' ||
      (/^\/plans\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'DELETE');
    const isAdminPaymentFetch =
      adminEndpoint === '/payment/fetch' ||
      (adminEndpoint === '/payments' && requestMethod === 'GET');
    const isAdminPaymentSave =
      adminEndpoint === '/payment/save' ||
      (adminEndpoint === '/payments' && requestMethod === 'POST') ||
      (/^\/payments\/\d+$/.test(adminEndpoint ?? '') &&
        requestMethod === 'PATCH' &&
        !isSingleFlagPatch('enable'));
    const isAdminOrderFetch =
      adminEndpoint === '/order/fetch' ||
      (adminEndpoint === '/orders' && requestMethod === 'GET');
    const isAdminOrderAssign =
      adminEndpoint === '/order/assign' ||
      (adminEndpoint === '/orders' && requestMethod === 'POST');
    const isAdminOrderPaid =
      adminEndpoint === '/order/paid' ||
      /^\/orders\/[^/]+\/mark-paid$/.test(adminEndpoint ?? '');
    const isAdminOrderUpdate =
      adminEndpoint === '/order/update' ||
      (/^\/orders\/[^/]+$/.test(adminEndpoint ?? '') && requestMethod === 'PATCH');
    // W12 (§6.6): the modern admin user family carries identity in the path —
    // the list GETs `/users` (§7/§8), the detail GETs `/users/{id}`, update
    // PATCHes `/users/{id}`, delete DELETEs `/users/{id}`, single/bulk create
    // POSTs `/users`, the bulk filter actions POST `/users/{export,mail,ban,
    // bulk-delete}`, and reset-secret/set-inviter POST `/users/{id}/…`. Match
    // both worlds' spellings for the counters, captures, timeout, and delay
    // knobs below.
    const isAdminUserFetch =
      adminEndpoint === '/user/fetch' ||
      (adminEndpoint === '/users' && requestMethod === 'GET');
    const isAdminUserUpdate =
      adminEndpoint === '/user/update' ||
      (/^\/users\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'PATCH');
    const isAdminUserGenerate =
      adminEndpoint === '/user/generate' ||
      (adminEndpoint === '/users' && requestMethod === 'POST');
    const isAdminUserDelete =
      adminEndpoint === '/user/delUser' ||
      (/^\/users\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'DELETE');
    const isAdminUserBan = adminEndpoint === '/user/ban' || adminEndpoint === '/users/ban';
    const isAdminUserAllDelete =
      adminEndpoint === '/user/allDel' || adminEndpoint === '/users/bulk-delete';
    const isAdminUserDumpCsv =
      adminEndpoint === '/user/dumpCSV' || adminEndpoint === '/users/export';
    const isAdminUserSendMail =
      adminEndpoint === '/user/sendMail' || adminEndpoint === '/users/mail';
    if (isUserProfileGet) {
      page.__visualParityUserInfoFetchCount = (page.__visualParityUserInfoFetchCount ?? 0) + 1;
    }
    if (isUserSubscribeGet) {
      page.__visualParityUserSubscribeFetchCount =
        (page.__visualParityUserSubscribeFetchCount ?? 0) + 1;
    }
    if (isUserUnbindTelegram) {
      page.__visualParityUserUnbindTelegramCount =
        (page.__visualParityUserUnbindTelegramCount ?? 0) + 1;
    }
    if (isUserResetSecurity) {
      page.__visualParityUserResetSecurityCount =
        (page.__visualParityUserResetSecurityCount ?? 0) + 1;
    }
    if (isUserProfileUpdate) {
      // Canonical capture: the legacy 0/1 form flags and the modern boolean
      // JSON flags compare as one §4.1 contract.
      const updateRequest = canonicalRequestCapture();
      page.__visualParityLastUserUpdate = updateRequest;
      page.__visualParityUserUpdateRequests = [
        ...(page.__visualParityUserUpdateRequests ?? []),
        updateRequest,
      ];
    }
    if (isUserRedeemGiftcard) {
      page.__visualParityLastUserRedeemGiftcard = requestData;
      page.__visualParityUserRedeemGiftcardCount =
        (page.__visualParityUserRedeemGiftcardCount ?? 0) + 1;
      page.__visualParityUserRedeemGiftcardRequests = [
        ...(page.__visualParityUserRedeemGiftcardRequests ?? []),
        requestData,
      ];
    }
    if (isUserPasswordUpdate) {
      page.__visualParityLastUserChangePassword = requestData;
      page.__visualParityUserChangePasswordRequests = [
        ...(page.__visualParityUserChangePasswordRequests ?? []),
        requestData,
      ];
    }
    if (isUserTransfer) {
      page.__visualParityLastUserTransfer = requestData;
      page.__visualParityUserTransferCount = (page.__visualParityUserTransferCount ?? 0) + 1;
      page.__visualParityUserTransferRequests = [
        ...(page.__visualParityUserTransferRequests ?? []),
        requestData,
      ];
    }
    if (isUserInviteGenerate) {
      page.__visualParityUserInviteGenerateCount =
        (page.__visualParityUserInviteGenerateCount ?? 0) + 1;
    }
    if (isUserNewPeriod) {
      page.__visualParityLastUserNewPeriod = requestData;
      page.__visualParityUserNewPeriodCount = (page.__visualParityUserNewPeriodCount ?? 0) + 1;
      page.__visualParityUserNewPeriodRequests = [
        ...(page.__visualParityUserNewPeriodRequests ?? []),
        requestData,
      ];
    }
    if (
      pathname === '/api/v1/user/order/save' ||
      (pathname === '/api/v1/user/orders' && requestMethod === 'POST')
    ) {
      const orderSaveRequest = canonicalRequestCapture();
      page.__visualParityLastUserOrderSave = orderSaveRequest;
      page.__visualParityUserOrderSaveCount = (page.__visualParityUserOrderSaveCount ?? 0) + 1;
      page.__visualParityUserOrderSaveRequests = [
        ...(page.__visualParityUserOrderSaveRequests ?? []),
        orderSaveRequest,
      ];
    }
    if (
      pathname === '/api/v1/user/order/fetch' ||
      (pathname === '/api/v1/user/orders' && requestMethod === 'GET')
    ) {
      page.__visualParityUserOrderFetchCount = (page.__visualParityUserOrderFetchCount ?? 0) + 1;
    }
    if (
      (pathname === '/api/v1/user/plan/fetch' && !requestUrl.searchParams.has('id')) ||
      pathname === '/api/v1/user/plans'
    ) {
      page.__visualParityUserPlanFetchCount = (page.__visualParityUserPlanFetchCount ?? 0) + 1;
    }
    if (
      pathname === '/api/v1/user/server/fetch' ||
      pathname === '/api/v1/user/servers'
    ) {
      page.__visualParityUserServerFetchCount = (page.__visualParityUserServerFetchCount ?? 0) + 1;
    }
    if (
      pathname === '/api/v1/user/stat/getTrafficLog' ||
      pathname === '/api/v1/user/traffic-logs'
    ) {
      page.__visualParityUserTrafficFetchCount =
        (page.__visualParityUserTrafficFetchCount ?? 0) + 1;
    }
    if (
      (pathname === '/api/v1/user/knowledge/fetch' && !requestUrl.searchParams.has('id')) ||
      pathname === '/api/v1/user/knowledge'
    ) {
      page.__visualParityUserKnowledgeFetchCount =
        (page.__visualParityUserKnowledgeFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/order/checkout' || modernOrderAction === 'checkout') {
      const checkoutRequest = canonicalRequestCapture();
      page.__visualParityLastUserOrderCheckout = checkoutRequest;
      page.__visualParityUserOrderCheckoutCount =
        (page.__visualParityUserOrderCheckoutCount ?? 0) + 1;
      page.__visualParityUserOrderCheckoutRequests = [
        ...(page.__visualParityUserOrderCheckoutRequests ?? []),
        checkoutRequest,
      ];
    }
    if (pathname === '/api/v1/user/coupon/check' || pathname === '/api/v1/user/coupons/check') {
      const couponCheckRequest = canonicalRequestCapture();
      page.__visualParityLastUserCouponCheck = couponCheckRequest;
      page.__visualParityUserCouponCheckCount = (page.__visualParityUserCouponCheckCount ?? 0) + 1;
      page.__visualParityUserCouponCheckRequests = [
        ...(page.__visualParityUserCouponCheckRequests ?? []),
        couponCheckRequest,
      ];
    }
    if (pathname === '/api/v1/user/comm/getStripePublicKey') {
      page.__visualParityUserStripePrepareCount =
        (page.__visualParityUserStripePrepareCount ?? 0) + 1;
      page.__visualParityUserStripePublicKeyCount =
        (page.__visualParityUserStripePublicKeyCount ?? 0) + 1;
      page.__visualParityUserStripePublicKeyRequests = [
        ...(page.__visualParityUserStripePublicKeyRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/order/stripe/intent' || modernOrderAction === 'stripe-intent') {
      page.__visualParityUserStripePrepareCount =
        (page.__visualParityUserStripePrepareCount ?? 0) + 1;
      page.__visualParityUserStripeIntentCount =
        (page.__visualParityUserStripeIntentCount ?? 0) + 1;
      page.__visualParityUserStripeIntentRequests = [
        ...(page.__visualParityUserStripeIntentRequests ?? []),
        canonicalRequestCapture(),
      ];
    }
    if (isUserTicketFetch) {
      page.__visualParityUserTicketFetchCount = (page.__visualParityUserTicketFetchCount ?? 0) + 1;
    }
    if (isUserTicketReply) {
      // Canonical capture: the legacy body-carried ticket id and the modern
      // path id compare as one §5.7 contract.
      const ticketReplyRequest = canonicalRequestCapture();
      page.__visualParityLastUserTicketReply = ticketReplyRequest;
      page.__visualParityUserTicketReplyCount = (page.__visualParityUserTicketReplyCount ?? 0) + 1;
      page.__visualParityUserTicketReplyRequests = [
        ...(page.__visualParityUserTicketReplyRequests ?? []),
        ticketReplyRequest,
      ];
    }
    if (isUserTicketClose) {
      const ticketCloseRequest = canonicalRequestCapture();
      page.__visualParityLastUserTicketClose = ticketCloseRequest;
      page.__visualParityUserTicketCloseCount = (page.__visualParityUserTicketCloseCount ?? 0) + 1;
      page.__visualParityUserTicketCloseRequests = [
        ...(page.__visualParityUserTicketCloseRequests ?? []),
        ticketCloseRequest,
      ];
    }
    if (isUserTicketSave) {
      const ticketSaveRequest = canonicalRequestCapture();
      page.__visualParityLastUserTicketSave = ticketSaveRequest;
      page.__visualParityUserTicketSaveCount = (page.__visualParityUserTicketSaveCount ?? 0) + 1;
      page.__visualParityUserTicketSaveRequests = [
        ...(page.__visualParityUserTicketSaveRequests ?? []),
        ticketSaveRequest,
      ];
    }
    if (isUserWithdraw) {
      const withdrawRequest = canonicalRequestCapture();
      page.__visualParityLastUserWithdraw = withdrawRequest;
      page.__visualParityUserWithdrawCount = (page.__visualParityUserWithdrawCount ?? 0) + 1;
      page.__visualParityUserWithdrawRequests = [
        ...(page.__visualParityUserWithdrawRequests ?? []),
        withdrawRequest,
      ];
    }
    if (pathname === '/api/v1/user/order/cancel' || modernOrderAction === 'cancel') {
      const orderCancelRequest = canonicalRequestCapture();
      page.__visualParityLastUserOrderCancel = orderCancelRequest;
      page.__visualParityUserOrderCancelCount = (page.__visualParityUserOrderCancelCount ?? 0) + 1;
      page.__visualParityUserOrderCancelRequests = [
        ...(page.__visualParityUserOrderCancelRequests ?? []),
        orderCancelRequest,
      ];
    }
    if (isAdminConfigFetch) {
      page.__visualParityAdminConfigFetchCount =
        (page.__visualParityAdminConfigFetchCount ?? 0) + 1;
    }
    if (isAdminConfigSave) {
      page.__visualParityLastAdminConfigSave = requestData;
      page.__visualParityAdminConfigSaveCount = (page.__visualParityAdminConfigSaveCount ?? 0) + 1;
      page.__visualParityAdminConfigSaveRequests = [
        ...(page.__visualParityAdminConfigSaveRequests ?? []),
        requestData,
      ];
    }
    if (isAdminOrderAssign) {
      // Canonical capture (W11 §6.4): the legacy form body and the modern JSON
      // body (total_amount cents in both) fold onto one contract.
      page.__visualParityLastAdminOrderAssign = canonicalRequestCapture();
    }
    if (isAdminOrderPaid) {
      // Canonical capture: legacy `{trade_no}` body and modern path identity
      // both surface `{trade_no}`.
      page.__visualParityLastAdminOrderPaid = canonicalRequestCapture();
    }
    if (isAdminOrderUpdate) {
      // Canonical capture: legacy `{trade_no, status|commission_status}` body
      // and the modern PATCH `{status|commission_status}` + path trade_no fold
      // onto one flat contract.
      page.__visualParityLastAdminOrderUpdate = canonicalRequestCapture();
    }
    if (isAdminOrderFetch) {
      page.__visualParityAdminOrderFetchCount = (page.__visualParityAdminOrderFetchCount ?? 0) + 1;
      // Canonical capture (§7/§8): the legacy `filter[i][…]`/`current`/`pageSize`
      // query and the modern `filter` JSON + `page`/`per_page` fold to one shape.
      page.__visualParityLastAdminOrderFetchQuery = canonicalRequestCapture();
    }
    if (isAdminPlanFetch) {
      page.__visualParityAdminPlanFetchCount = (page.__visualParityAdminPlanFetchCount ?? 0) + 1;
    }
    if (isAdminPlanSave) {
      const planSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminPlanSave = planSaveRequest;
      page.__visualParityAdminPlanSaveCount = (page.__visualParityAdminPlanSaveCount ?? 0) + 1;
      page.__visualParityAdminPlanSaveRequests = [
        ...(page.__visualParityAdminPlanSaveRequests ?? []),
        planSaveRequest,
      ];
    }
    if (isAdminPlanUpdate) {
      const planUpdateRequest = canonicalRequestCapture();
      page.__visualParityLastAdminPlanUpdate = planUpdateRequest;
      page.__visualParityAdminPlanUpdateCount = (page.__visualParityAdminPlanUpdateCount ?? 0) + 1;
      page.__visualParityAdminPlanUpdateRequests = [
        ...(page.__visualParityAdminPlanUpdateRequests ?? []),
        planUpdateRequest,
      ];
    }
    if (isAdminPlanDrop) {
      const planDropRequest = canonicalRequestCapture();
      page.__visualParityLastAdminPlanDrop = planDropRequest;
      page.__visualParityAdminPlanDropCount = (page.__visualParityAdminPlanDropCount ?? 0) + 1;
      page.__visualParityAdminPlanDropRequests = [
        ...(page.__visualParityAdminPlanDropRequests ?? []),
        planDropRequest,
      ];
    }
    if (adminEndpoint === '/server/group/fetch') {
      page.__visualParityAdminServerGroupFetchCount =
        (page.__visualParityAdminServerGroupFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/server/group/save') {
      page.__visualParityLastAdminServerGroupSave = requestData;
      page.__visualParityAdminServerGroupSaveCount =
        (page.__visualParityAdminServerGroupSaveCount ?? 0) + 1;
      page.__visualParityAdminServerGroupSaveRequests = [
        ...(page.__visualParityAdminServerGroupSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/server/route/fetch') {
      page.__visualParityAdminServerRouteFetchCount =
        (page.__visualParityAdminServerRouteFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/server/route/save') {
      page.__visualParityLastAdminServerRouteSave = requestData;
      page.__visualParityAdminServerRouteSaveCount =
        (page.__visualParityAdminServerRouteSaveCount ?? 0) + 1;
      page.__visualParityAdminServerRouteSaveRequests = [
        ...(page.__visualParityAdminServerRouteSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/server/manage/getNodes') {
      page.__visualParityAdminServerNodeFetchCount =
        (page.__visualParityAdminServerNodeFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/server/manage/sort') {
      page.__visualParityLastAdminServerSort = requestData;
      page.__visualParityAdminServerSortCount = (page.__visualParityAdminServerSortCount ?? 0) + 1;
      page.__visualParityAdminServerSortRequests = [
        ...(page.__visualParityAdminServerSortRequests ?? []),
        requestData,
      ];
    }
    if (adminServerNodeSaveMatch) {
      page.__visualParityLastAdminServerNodeSave = requestData;
      page.__visualParityAdminServerNodeSaveCount =
        (page.__visualParityAdminServerNodeSaveCount ?? 0) + 1;
      page.__visualParityAdminServerNodeSaveRequests = [
        ...(page.__visualParityAdminServerNodeSaveRequests ?? []),
        {
          ...requestData,
          __endpoint: adminEndpoint,
          __type: adminServerNodeSaveMatch[1],
        },
      ];
    }
    if (isAdminCouponFetch) {
      page.__visualParityAdminCouponFetchCount =
        (page.__visualParityAdminCouponFetchCount ?? 0) + 1;
    }
    if (isAdminCouponGenerate) {
      // Canonical capture (W10 §6.3): the legacy edit-by-generate body-id and
      // bracket arrays fold onto the modern path-identity JSON contract.
      const couponGenerateRequest = canonicalRequestCapture();
      page.__visualParityLastAdminCouponGenerate = couponGenerateRequest;
      page.__visualParityAdminCouponGenerateCount =
        (page.__visualParityAdminCouponGenerateCount ?? 0) + 1;
      page.__visualParityAdminCouponGenerateRequests = [
        ...(page.__visualParityAdminCouponGenerateRequests ?? []),
        couponGenerateRequest,
      ];
    }
    if (isAdminGiftcardFetch) {
      page.__visualParityAdminGiftcardFetchCount =
        (page.__visualParityAdminGiftcardFetchCount ?? 0) + 1;
    }
    if (isAdminGiftcardGenerate) {
      const giftcardGenerateRequest = canonicalRequestCapture();
      page.__visualParityLastAdminGiftcardGenerate = giftcardGenerateRequest;
      page.__visualParityAdminGiftcardGenerateCount =
        (page.__visualParityAdminGiftcardGenerateCount ?? 0) + 1;
      page.__visualParityAdminGiftcardGenerateRequests = [
        ...(page.__visualParityAdminGiftcardGenerateRequests ?? []),
        giftcardGenerateRequest,
      ];
    }
    if (isAdminKnowledgeListFetch) {
      page.__visualParityAdminKnowledgeFetchCount =
        (page.__visualParityAdminKnowledgeFetchCount ?? 0) + 1;
    }
    if (isAdminKnowledgeSave) {
      const knowledgeSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminKnowledgeSave = knowledgeSaveRequest;
      page.__visualParityAdminKnowledgeSaveCount =
        (page.__visualParityAdminKnowledgeSaveCount ?? 0) + 1;
      page.__visualParityAdminKnowledgeSaveRequests = [
        ...(page.__visualParityAdminKnowledgeSaveRequests ?? []),
        knowledgeSaveRequest,
      ];
    }
    if (isAdminNoticeFetch) {
      page.__visualParityAdminNoticeFetchCount =
        (page.__visualParityAdminNoticeFetchCount ?? 0) + 1;
    }
    if (isAdminNoticeSave) {
      const noticeSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminNoticeSave = noticeSaveRequest;
      page.__visualParityAdminNoticeSaveCount = (page.__visualParityAdminNoticeSaveCount ?? 0) + 1;
      page.__visualParityAdminNoticeSaveRequests = [
        ...(page.__visualParityAdminNoticeSaveRequests ?? []),
        noticeSaveRequest,
      ];
    }
    if (isAdminNoticeShow) {
      const noticeShowRequest = canonicalRequestCapture();
      page.__visualParityLastAdminNoticeShow = noticeShowRequest;
      page.__visualParityAdminNoticeShowCount = (page.__visualParityAdminNoticeShowCount ?? 0) + 1;
      page.__visualParityAdminNoticeShowRequests = [
        ...(page.__visualParityAdminNoticeShowRequests ?? []),
        noticeShowRequest,
      ];
    }
    if (isAdminNoticeDrop) {
      const noticeDropRequest = canonicalRequestCapture();
      page.__visualParityLastAdminNoticeDrop = noticeDropRequest;
      page.__visualParityAdminNoticeDropCount = (page.__visualParityAdminNoticeDropCount ?? 0) + 1;
      page.__visualParityAdminNoticeDropRequests = [
        ...(page.__visualParityAdminNoticeDropRequests ?? []),
        noticeDropRequest,
      ];
    }
    if (isAdminPaymentFetch) {
      page.__visualParityAdminPaymentFetchCount =
        (page.__visualParityAdminPaymentFetchCount ?? 0) + 1;
    }
    if (isAdminPaymentSave) {
      // Canonical capture (W11 §6.2): the legacy `config[key]` bracket form and
      // the modern nested `config` JSON object fold onto one contract.
      const paymentSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminPaymentSave = paymentSaveRequest;
      page.__visualParityAdminPaymentSaveCount =
        (page.__visualParityAdminPaymentSaveCount ?? 0) + 1;
      page.__visualParityAdminPaymentSaveRequests = [
        ...(page.__visualParityAdminPaymentSaveRequests ?? []),
        paymentSaveRequest,
      ];
    }
    if (adminEndpoint === '/ticket/fetch') {
      page.__visualParityAdminTicketFetchCount =
        (page.__visualParityAdminTicketFetchCount ?? 0) + 1;
      page.__visualParityAdminTicketFetchRequests = [
        ...(page.__visualParityAdminTicketFetchRequests ?? []),
        {
          data: requestData,
          searchParams: Array.from(requestUrl.searchParams.entries()),
        },
      ];
    }
    if (adminEndpoint === '/ticket/reply') {
      page.__visualParityLastAdminTicketReply = requestData;
      page.__visualParityAdminTicketReplyCount =
        (page.__visualParityAdminTicketReplyCount ?? 0) + 1;
      page.__visualParityAdminTicketReplyRequests = [
        ...(page.__visualParityAdminTicketReplyRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/stat/getStatUser') {
      page.__visualParityLastAdminUserTrafficQuery = Object.fromEntries(
        requestUrl.searchParams.entries(),
      );
    }
    if (isAdminUserFetch) {
      page.__visualParityAdminUserFetchCount = (page.__visualParityAdminUserFetchCount ?? 0) + 1;
      // Canonical capture (§7/§8): the legacy `filter[i][…]`/`current`/`pageSize`
      // /`sort` query and the modern `filter` JSON + `page`/`per_page`/`sort_by`
      // /`sort_dir` fold to one flat shape.
      const userFetchCapture = canonicalRequestCapture();
      page.__visualParityLastAdminUserFetchQuery = userFetchCapture;
      if (
        Array.isArray(userFetchCapture.filter) &&
        userFetchCapture.filter.some((clause) => clause?.field === 'invite_user_id')
      ) {
        page.__visualParityLastAdminFilteredUserFetchQuery = userFetchCapture;
      }
    }
    if (isAdminUserUpdate) {
      // Canonical capture (§6.6): the legacy `{id, …}` body and the modern PATCH
      // `{…}` + path id fold onto one flat contract.
      const userUpdateRequest = canonicalRequestCapture();
      page.__visualParityLastAdminUserUpdate = userUpdateRequest;
      page.__visualParityAdminUserUpdateCount = (page.__visualParityAdminUserUpdateCount ?? 0) + 1;
      page.__visualParityAdminUserUpdateRequests = [
        ...(page.__visualParityAdminUserUpdateRequests ?? []),
        userUpdateRequest,
      ];
    }
    if (isAdminUserGenerate) {
      page.__visualParityLastAdminUserGenerate = requestData;
      page.__visualParityAdminUserGenerateCount =
        (page.__visualParityAdminUserGenerateCount ?? 0) + 1;
      page.__visualParityAdminUserGenerateRequests = [
        ...(page.__visualParityAdminUserGenerateRequests ?? []),
        requestData,
      ];
    }
    if (isAdminUserDelete) {
      // Canonical capture: the legacy `{id}` body and the modern DELETE path id
      // both surface `{id}`.
      const userDeleteRequest = canonicalRequestCapture();
      page.__visualParityLastAdminUserDelete = userDeleteRequest;
      page.__visualParityAdminUserDeleteCount = (page.__visualParityAdminUserDeleteCount ?? 0) + 1;
      page.__visualParityAdminUserDeleteRequests = [
        ...(page.__visualParityAdminUserDeleteRequests ?? []),
        userDeleteRequest,
      ];
    }
    if (isAdminUserBan) {
      page.__visualParityLastAdminUserBan = requestData;
      page.__visualParityAdminUserBanCount = (page.__visualParityAdminUserBanCount ?? 0) + 1;
      page.__visualParityAdminUserBanRequests = [
        ...(page.__visualParityAdminUserBanRequests ?? []),
        requestData,
      ];
    }
    if (isAdminUserAllDelete) {
      page.__visualParityLastAdminUserAllDelete = requestData;
      page.__visualParityAdminUserAllDeleteCount =
        (page.__visualParityAdminUserAllDeleteCount ?? 0) + 1;
      page.__visualParityAdminUserAllDeleteRequests = [
        ...(page.__visualParityAdminUserAllDeleteRequests ?? []),
        requestData,
      ];
    }
    if (isAdminUserDumpCsv) {
      page.__visualParityLastAdminUserDumpCsv = requestData;
      page.__visualParityAdminUserDumpCsvCount =
        (page.__visualParityAdminUserDumpCsvCount ?? 0) + 1;
      page.__visualParityAdminUserDumpCsvRequests = [
        ...(page.__visualParityAdminUserDumpCsvRequests ?? []),
        requestData,
      ];
    }
    if (isAdminUserSendMail) {
      page.__visualParityLastAdminUserSendMail = requestData;
      page.__visualParityAdminUserSendMailCount =
        (page.__visualParityAdminUserSendMailCount ?? 0) + 1;
      page.__visualParityAdminUserSendMailRequests = [
        ...(page.__visualParityAdminUserSendMailRequests ?? []),
        requestData,
      ];
    }
    if (
      target === 'oracle' &&
      scenario.label === 'admin-ticket-detail' &&
      adminEndpoint === '/ticket/fetch' &&
      !seededAdminTicketDetailStore
    ) {
      seededAdminTicketDetailStore = true;
      await seedLegacyAdminTicketDetailStore(page).catch((error) => {
        page.__visualParityDiagnostics?.push(`ticket detail preseed failed: ${error.message}`);
      });
    }

    if (isUserRedeemGiftcard && interaction.redeemGiftcardTimeout) {
      await route.abort('timedout');
      return;
    }
    const shouldTimeout =
      (scenario.userPlansTimeout &&
        (pathname === '/api/v1/user/plan/fetch' || pathname === '/api/v1/user/plans')) ||
      (scenario.userOrdersTimeout &&
        (pathname === '/api/v1/user/order/fetch' ||
          (pathname === '/api/v1/user/orders' && requestMethod === 'GET'))) ||
      (scenario.userServersTimeout &&
        (pathname === '/api/v1/user/server/fetch' || pathname === '/api/v1/user/servers')) ||
      (scenario.userTrafficTimeout &&
        (pathname === '/api/v1/user/stat/getTrafficLog' ||
          pathname === '/api/v1/user/traffic-logs')) ||
      (scenario.userTicketsTimeout && isUserTicketFetch) ||
      (scenario.userKnowledgeTimeout &&
        (pathname === '/api/v1/user/knowledge/fetch' || pathname === '/api/v1/user/knowledge')) ||
      (scenario.adminPlansTimeout && isAdminPlanFetch) ||
      (scenario.adminOrdersTimeout && isAdminOrderFetch) ||
      (scenario.adminUsersTimeout && isAdminUserFetch) ||
      (scenario.adminTicketsTimeout && adminEndpoint === '/ticket/fetch') ||
      (scenario.adminServerManageTimeout && adminEndpoint === '/server/manage/getNodes') ||
      (scenario.adminPaymentsTimeout && isAdminPaymentFetch) ||
      (scenario.adminCouponsTimeout && isAdminCouponFetch) ||
      (scenario.adminGiftcardsTimeout && isAdminGiftcardFetch) ||
      (scenario.adminNoticesTimeout && isAdminNoticeFetch) ||
      (scenario.adminKnowledgeTimeout && isAdminKnowledgeListFetch);
    if (shouldTimeout) {
      await route.abort('timedout');
      return;
    }
    if (
      (pathname === '/api/v1/user/order/checkout' || modernOrderAction === 'checkout') &&
      interaction.orderCheckoutNetworkError
    ) {
      await route.abort('failed');
      return;
    }

    if (isUserProfileUpdate && interaction.delayUserUpdateMs) {
      await delay(interaction.delayUserUpdateMs);
    }
    if (isUserRedeemGiftcard && interaction.delayUserRedeemGiftcardMs) {
      await delay(interaction.delayUserRedeemGiftcardMs);
    }
    if (isUserPasswordUpdate && interaction.delayUserChangePasswordMs) {
      await delay(interaction.delayUserChangePasswordMs);
    }
    if (isUserTransfer && interaction.delayUserTransferMs) {
      await delay(interaction.delayUserTransferMs);
    }
    if (isUserNewPeriod && interaction.delayUserNewPeriodMs) {
      await delay(interaction.delayUserNewPeriodMs);
    }
    if (
      (pathname === '/api/v1/user/order/checkout' || modernOrderAction === 'checkout') &&
      interaction.delayUserOrderCheckoutMs
    ) {
      await delay(interaction.delayUserOrderCheckoutMs);
    }
    if (isUserTicketReply && interaction.delayUserTicketReplyMs) {
      await delay(interaction.delayUserTicketReplyMs);
    }
    if (isUserTicketClose && interaction.delayUserTicketCloseMs) {
      await delay(interaction.delayUserTicketCloseMs);
    }
    if (isUserTicketSave && interaction.delayUserTicketSaveMs) {
      await delay(interaction.delayUserTicketSaveMs);
    }
    if (isUserWithdraw && interaction.delayUserWithdrawMs) {
      await delay(interaction.delayUserWithdrawMs);
    }
    if (adminEndpoint === '/ticket/reply' && interaction.delayAdminTicketReplyMs) {
      await delay(interaction.delayAdminTicketReplyMs);
    }
    if (isAdminPaymentSave && interaction.delayAdminPaymentSaveMs) {
      await delay(interaction.delayAdminPaymentSaveMs);
    }
    if (isAdminCouponGenerate && interaction.delayAdminCouponGenerateMs) {
      await delay(interaction.delayAdminCouponGenerateMs);
    }
    if (isAdminGiftcardGenerate && interaction.delayAdminGiftcardGenerateMs) {
      await delay(interaction.delayAdminGiftcardGenerateMs);
    }
    if (isAdminKnowledgeSave && interaction.delayAdminKnowledgeSaveMs) {
      await delay(interaction.delayAdminKnowledgeSaveMs);
    }
    if (isAdminNoticeSave && interaction.delayAdminNoticeSaveMs) {
      await delay(interaction.delayAdminNoticeSaveMs);
    }
    if (isAdminPlanSave && interaction.delayAdminPlanSaveMs) {
      await delay(interaction.delayAdminPlanSaveMs);
    }
    if (
      (isAdminNoticeDrop ||
        isAdminNoticeShow ||
        isAdminPlanDrop ||
        isAdminPlanUpdate ||
        adminEndpoint === '/server/manage/sort') &&
      interaction.delayAdminMutationMs
    ) {
      await delay(interaction.delayAdminMutationMs);
    }
    if (adminEndpoint === '/server/group/save' && interaction.delayAdminServerGroupSaveMs) {
      await delay(interaction.delayAdminServerGroupSaveMs);
    }
    if (isAdminConfigSave && interaction.delayAdminConfigSaveMs) {
      await delay(interaction.delayAdminConfigSaveMs);
    }
    if (
      (isAdminUserUpdate || isAdminUserDelete || isAdminUserBan || isAdminUserAllDelete) &&
      interaction.delayAdminUserMutationMs
    ) {
      await delay(interaction.delayAdminUserMutationMs);
    }
    if (isAdminUserSendMail && interaction.delayAdminUserSendMailMs) {
      await delay(interaction.delayAdminUserSendMailMs);
    }
    if (isUserUnbindTelegram && interaction.delayUserUnbindTelegramMs) {
      await delay(interaction.delayUserUnbindTelegramMs);
    }
    await fulfillApiResponse(
      route,
      apiFixtureResponse(
        requestUrl,
        isAdminScenario,
        scenario,
        requestData,
        interaction,
        target,
        route.request().method(),
      ),
      target,
    );

    if (adminEndpoint === '/server/group/fetch' && !adminGroupsResolved) {
      adminGroupsResolved = true;
      resolveAdminGroupsReady();
    }
  });
}

export function adminFixtureEndpoint(pathname) {
  const prefix = `/api/v1/${adminPath}`;
  return pathname.startsWith(`${prefix}/`) ? pathname.slice(prefix.length) : null;
}

export function readRequestData(request) {
  const raw = request.postData();
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return Object.fromEntries(new URLSearchParams(raw));
  }
}

export function apiFixtureResponse(
  requestUrl,
  isAdminScenario,
  scenario = { label: '' },
  requestData = null,
  interaction = {},
  target = 'oracle',
  method = 'GET',
) {
  const pathname = requestUrl.pathname;
  const adminEndpoint = adminFixtureEndpoint(pathname);
  const body = (data, extra = {}) => ({ code: 200, data, ...extra });
  const error = (message, code = 400) => ({ code, data: null, message });
  const httpError = (message, status = 500) => ({
    code: status,
    data: null,
    httpStatus: status,
    message,
  });
  // Modern-dialect fixtures (docs/api-dialect.md §13.5, flipped for §5.2 auth
  // in W2): bare bodies with real HTTP statuses and problem+json errors. Only
  // the source world requests the migrated paths / receives these shapes.
  const v2Body = (data, httpStatus = 200) => ({ data, dialect: 'v2', httpStatus });
  const v2Empty = () => ({ data: null, dialect: 'v2', httpStatus: 204 });
  const v2Problem = (status, title, code, detail) => ({
    dialect: 'v2',
    httpStatus: status,
    problem: { code, detail, status, title, type: 'about:blank' },
  });
  // GLOBAL FLIP 2 (§3.2): the force-unauthorized knobs keep their legacy
  // canonical meaning — 403 was "session expired" (teardown + redirect), 401
  // was a non-session auth failure (keep token, contain the error). The
  // modern dialect swaps the wire for the same outcomes: teardown is 401
  // problem+json `session_expired`, keep-token is 403 `permission_denied`.
  const unauthorizedFixture = (legacyStatus) => {
    if (target !== 'source') return httpError('auth required', legacyStatus);
    return legacyStatus === 401
      ? v2Problem(403, 'Forbidden', 'permission_denied', 'Permission denied')
      : v2Problem(401, 'Unauthorized', 'session_expired', '未登录或登陆已过期');
  };

  if (
    (scenario.forceUserUnauthorized || interaction.forceUserUnauthorized) &&
    (pathname === '/api/v1/user/info' || pathname === '/api/v1/user/profile')
  ) {
    return unauthorizedFixture(
      interaction.forceUserUnauthorizedStatus ?? scenario.forceUserUnauthorizedStatus ?? 403,
    );
  }

  if ((scenario.forceAdminUnauthorized || interaction.forceAdminUnauthorized) && adminEndpoint) {
    return unauthorizedFixture(
      interaction.forceAdminUnauthorizedStatus ?? scenario.forceAdminUnauthorizedStatus ?? 403,
    );
  }

  if (adminEndpoint) {
    if (
      scenario.adminOrdersHttpError &&
      (adminEndpoint === '/order/fetch' || (adminEndpoint === '/orders' && method === 'GET'))
    ) {
      // W11 (§6.4): the source world speaks dialect v2, so a list failure is a
      // problem+json 500; the frozen oracle keeps the legacy HTTP-500 body.
      return target === 'source'
        ? v2Problem(500, 'Internal Server Error', 'internal_error', 'Server Error')
        : httpError('Server Error', 500);
    }
    if (
      scenario.adminUsersHttpError &&
      (adminEndpoint === '/user/fetch' || (adminEndpoint === '/users' && method === 'GET'))
    ) {
      // W12 (§6.6): the source world speaks dialect v2, so a list failure is a
      // problem+json 500; the frozen oracle keeps the legacy HTTP-500 body.
      return target === 'source'
        ? v2Problem(500, 'Internal Server Error', 'internal_error', 'Server Error')
        : httpError('Server Error', 500);
    }
    if (
      /^\/server\/(shadowsocks|vmess|trojan|vless|hysteria|tuic|anytls|v2node)\/save$/.test(
        adminEndpoint,
      )
    ) {
      if (interaction?.adminServerNodeSaveError) return error('节点保存失败');
      return body(true);
    }

    // §6.3 modern admin content family (W10): notices bare unpaginated array,
    // knowledge bare array + /{id} detail, coupons/gift-cards §8 pages,
    // creates 201 {id}, updates/toggles/deletes bodiless. Only the source
    // world requests these spellings (the shared /knowledge/sort path is
    // target-gated); the oracle keeps the legacy rows in the switch below.
    // The error-knob details mirror the legacy fixture toast text — the
    // Tier-1 comparison keys on the problem `code`, presentation drops.
    const contentValidationProblem = (detail) =>
      v2Problem(422, 'Unprocessable Entity', 'validation_failed', detail);
    const isShowOnlyBody =
      requestData != null && Object.keys(requestData).length === 1 && 'show' in requestData;
    if (adminEndpoint === '/notices') {
      if (method === 'POST') {
        if (interaction?.adminNoticeSaveError) return contentValidationProblem('公告保存失败');
        return v2Body({ id: adminNoticeFixtures.length + 1 }, 201);
      }
      return v2Body(adminNoticeFixtures.map(modernNoticeFixture));
    }
    if (/^\/notices\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') {
        if (interaction?.adminNoticeDropError) return contentValidationProblem('公告删除失败');
        return v2Empty();
      }
      if (isShowOnlyBody) {
        if (interaction?.adminNoticeShowError) {
          return contentValidationProblem('公告显示状态保存失败');
        }
        return v2Empty();
      }
      if (interaction?.adminNoticeSaveError) return contentValidationProblem('公告保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/knowledge-categories') {
      return v2Body(
        Array.from(new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category))),
      );
    }
    if (adminEndpoint === '/knowledge/sort' && target === 'source') {
      return v2Empty();
    }
    if (adminEndpoint === '/knowledge') {
      if (method === 'POST') {
        if (interaction?.adminKnowledgeSaveError) return contentValidationProblem('知识保存失败');
        return v2Body({ id: adminKnowledgeFixtures.length + 1 }, 201);
      }
      return v2Body(adminKnowledgeFixtures.map(modernKnowledgeSummaryFixture));
    }
    const modernAdminKnowledgeMatch = /^\/knowledge\/(\d+)$/.exec(adminEndpoint);
    if (modernAdminKnowledgeMatch) {
      if (method === 'GET') {
        return v2Body(
          modernKnowledgeDetailFixture(
            adminKnowledgeFixtures.find(
              (knowledge) => String(knowledge.id) === modernAdminKnowledgeMatch[1],
            ) ?? adminKnowledgeFixtures[0],
          ),
        );
      }
      if (method === 'DELETE') return v2Empty();
      if (!isShowOnlyBody && interaction?.adminKnowledgeSaveError) {
        return contentValidationProblem('知识保存失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/coupons') {
      if (method === 'POST') {
        if (interaction?.adminCouponGenerateError) {
          return contentValidationProblem('优惠券生成失败');
        }
        return v2Body({ id: adminCouponFixtures.length + 1 }, 201);
      }
      return v2Body({
        items: adminCouponFixtures.map(modernCouponFixture),
        total: adminCouponFixtures.length,
      });
    }
    if (/^\/coupons\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (!isShowOnlyBody && interaction?.adminCouponGenerateError) {
        return contentValidationProblem('优惠券生成失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/gift-cards') {
      if (method === 'POST') {
        if (interaction?.adminGiftcardGenerateError) {
          return contentValidationProblem('礼品卡生成失败');
        }
        return v2Body({ id: adminGiftcardFixtures.length + 1 }, 201);
      }
      return v2Body({
        items: adminGiftcardFixtures.map(modernGiftcardFixture),
        total: adminGiftcardFixtures.length,
      });
    }
    if (/^\/gift-cards\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (interaction?.adminGiftcardGenerateError) {
        return contentValidationProblem('礼品卡生成失败');
      }
      return v2Empty();
    }

    // §6.2/§6.4 modern admin commerce family (W11): plans/payments bare arrays
    // (prices/fees stay cents/number, booleans, RFC 3339), payment-providers
    // code array + provider form, orders §8 page + trade_no bare detail, 201
    // {id}/{trade_no} creates, bodiless updates/toggles/deletes, and a seeded
    // reconciliation page. Only the source world requests these spellings; the
    // oracle keeps the legacy rows in the switch below. Error-knob detail text
    // mirrors the legacy toast — the Tier-1 comparison keys on the problem
    // `code`, presentation drops.
    const isSingleFlagBody = (flag) =>
      requestData != null && Object.keys(requestData).length === 1 && flag in requestData;
    if (adminEndpoint === '/plans') {
      if (method === 'POST') {
        if (interaction?.adminPlanSaveError) return contentValidationProblem('订阅保存失败');
        return v2Body({ id: adminPlanFixturesFor(scenario).length + 1 }, 201);
      }
      return v2Body(adminPlanFixturesFor(scenario).map(modernAdminPlanFixture));
    }
    if (adminEndpoint === '/plans/sort') return v2Empty();
    if (/^\/plans\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') {
        if (interaction?.adminPlanDropError) return contentValidationProblem('订阅删除失败');
        return v2Empty();
      }
      if (isSingleFlagBody('show') || isSingleFlagBody('renew')) {
        if (interaction?.adminPlanUpdateError) return contentValidationProblem('订阅开关失败');
        return v2Empty();
      }
      if (interaction?.adminPlanSaveError) return contentValidationProblem('订阅保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/payments') {
      if (method === 'POST') {
        if (interaction?.adminPaymentSaveError) return contentValidationProblem('支付方式保存失败');
        return v2Body({ id: adminPaymentFixtures.length + 1 }, 201);
      }
      return v2Body(adminPaymentFixtures.map(modernAdminPaymentFixture));
    }
    if (adminEndpoint === '/payments/sort') return v2Empty();
    if (adminEndpoint === '/payment-providers') {
      return v2Body(adminPaymentMethodsFixture);
    }
    const modernProviderFormMatch = /^\/payment-providers\/([^/]+)\/form$/.exec(adminEndpoint);
    if (modernProviderFormMatch) {
      const requestedProvider = decodeURIComponent(modernProviderFormMatch[1]);
      return v2Body(
        adminPaymentFormFixtures[requestedProvider] ?? adminPaymentFormFixtures.AlipayF2F,
      );
    }
    if (/^\/payments\/\d+$/.test(adminEndpoint)) {
      if (method === 'DELETE') return v2Empty();
      if (isSingleFlagBody('enable')) return v2Empty();
      if (interaction?.adminPaymentSaveError) return contentValidationProblem('支付方式保存失败');
      return v2Empty();
    }
    if (adminEndpoint === '/orders') {
      if (method === 'POST') return v2Body({ trade_no: 'VISUAL2026110099' }, 201);
      return v2Body({
        items: adminOrderFixturesFor(scenario).map(modernAdminOrderFixture),
        total: adminOrderFixturesFor(scenario).length,
      });
    }
    const modernOrderScopedMatch = /^\/orders\/([^/]+)$/.exec(adminEndpoint);
    if (modernOrderScopedMatch) {
      if (method === 'GET') {
        const requestedTradeNo = decodeURIComponent(modernOrderScopedMatch[1]);
        return v2Body(
          modernAdminOrderFixture(
            adminOrderFixturesFor(scenario).find(
              (order) => order.trade_no === requestedTradeNo,
            ) ?? adminOrderFixtures[0],
          ),
        );
      }
      // PATCH status / commission_status
      return v2Empty();
    }
    if (/^\/orders\/[^/]+\/(mark-paid|cancel)$/.test(adminEndpoint)) {
      return v2Empty();
    }

    // §6.6 modern admin users family (W12): the list is an §8 `{items, total}`
    // page over the §7 DSL (RFC 3339 timestamps, `t`/password dropped), the
    // detail is a bare user with the conditional `invite_user` object, a single
    // create returns 201 `{id}` while a bulk run streams the byte-frozen
    // credential CSV, the bulk filter actions POST `/users/{export,mail,ban,
    // bulk-delete}`, and the update/toggle/delete/reset-secret/set-inviter carry
    // identity in the path with bodiless 204s. Only the source world requests
    // these spellings; the oracle keeps the legacy rows in the switch below.
    if (adminEndpoint === '/users') {
      if (method === 'POST') {
        if (requestData?.generate_count) {
          return {
            contentType: 'text/csv',
            httpStatus: 200,
            rawBody: 'email,password\nparity.created@example.com,secret123\n',
          };
        }
        return v2Body({ id: adminUserFixturesFor(scenario).length + 1 }, 201);
      }
      return v2Body({
        items: adminUserFixturesFor(scenario).map(modernAdminUserFixture),
        total: adminUserFixturesFor(scenario).length,
      });
    }
    if (adminEndpoint === '/users/export') {
      return {
        contentType: 'text/csv',
        httpStatus: 200,
        rawBody: 'id,email\n1,visual-user@example.com\n',
      };
    }
    if (adminEndpoint === '/users/mail') {
      if (requestData?.subject === interaction?.adminUserSendMailFailureSubject) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '邮件加入队列失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/users/ban') {
      if (interaction?.adminUserBanError) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '用户封禁失败');
      }
      return v2Empty();
    }
    if (adminEndpoint === '/users/bulk-delete') {
      if (interaction?.adminUserAllDeleteError) {
        return v2Problem(500, 'Internal Server Error', 'internal_error', '用户批量删除失败');
      }
      return v2Empty();
    }
    const modernUserDetailMatch = /^\/users\/(\d+)$/.exec(adminEndpoint);
    if (modernUserDetailMatch) {
      if (method === 'GET') {
        const requestedId = Number(modernUserDetailMatch[1]);
        const users = adminUserFixturesFor(scenario);
        return v2Body(
          modernAdminUserDetailFixture(
            users.find((user) => user.id === requestedId) ?? users[0],
            users,
          ),
        );
      }
      if (method === 'DELETE') {
        if (interaction?.adminUserDeleteError) {
          return v2Problem(500, 'Internal Server Error', 'internal_error', '用户删除失败');
        }
        return v2Empty();
      }
      // PATCH update
      if (interaction?.adminUserUpdateError) {
        return v2Problem(422, 'Unprocessable Entity', 'validation_failed', '邮箱格式错误');
      }
      return v2Empty();
    }
    if (/^\/users\/\d+\/(set-inviter|reset-secret)$/.test(adminEndpoint)) {
      return v2Empty();
    }

    switch (adminEndpoint) {
      case '/config/fetch':
        return body(adminConfigFixture);
      case '/config/save':
        if (interaction?.adminConfigSaveError) return error('配置保存失败');
        return body(true);
      case '/config/getEmailTemplate':
        return body(adminEmailTemplateFixtures);
      // §6.1 modern config & system family (W9). Only the source world
      // requests these spellings; the oracle keeps the legacy rows above.
      case '/config':
        if (method === 'PATCH') {
          if (interaction?.adminConfigSaveError) {
            return v2Problem(400, 'Bad Request', 'config_validation_failed', '配置保存失败');
          }
          // Full activation is a bodiless 204 (the 202 activation-pending
          // split is a single-process runtime concern, not a fixture path).
          return v2Empty();
        }
        return v2Body(modernAdminConfigFixture(adminConfigFixture));
      case '/email-templates':
        return v2Body(adminEmailTemplateFixtures);
      case '/telegram-webhook':
        return v2Empty();
      case '/test-mail':
        return v2Body({ log: null, sent: true });
      case '/system/queue-stats':
        return v2Body(modernQueueStatsFixture(adminQueueStatsFixture));
      case '/system/queue-workload':
        return v2Body(adminQueueWorkloadFixtures.map(modernQueueWorkloadFixture));
      case '/coupon/fetch':
        return body(adminCouponFixtures, { total: adminCouponFixtures.length });
      case '/coupon/generate':
        if (interaction?.adminCouponGenerateError) return error('优惠券生成失败');
        return body(true);
      case '/giftcard/fetch':
        return body(adminGiftcardFixtures, { total: adminGiftcardFixtures.length });
      case '/giftcard/generate':
        if (interaction?.adminGiftcardGenerateError) return error('礼品卡生成失败');
        return body(true);
      case '/knowledge/fetch':
        return body(
          requestUrl.searchParams.has('id')
            ? (adminKnowledgeFixtures.find(
                (knowledge) => String(knowledge.id) === requestUrl.searchParams.get('id'),
              ) ?? adminKnowledgeFixtures[0])
            : adminKnowledgeFixtures,
        );
      case '/knowledge/getCategory':
        return body(
          Array.from(new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category))),
        );
      case '/knowledge/save':
        if (interaction?.adminKnowledgeSaveError) return error('知识保存失败');
        return body(true);
      case '/notice/fetch':
        return body(adminNoticeFixtures, { total: adminNoticeFixtures.length });
      case '/notice/save':
        if (interaction?.adminNoticeSaveError) return error('公告保存失败');
        return body(true);
      case '/notice/show':
        if (interaction?.adminNoticeShowError) return error('公告显示状态保存失败');
        return body(true);
      case '/notice/drop':
        if (interaction?.adminNoticeDropError) return error('公告删除失败');
        return body(true);
      case '/stat/getOverride':
        return body(adminStatFixture);
      case '/stat/getOrder':
        return body(adminOrderStatFixtures);
      case '/stat/getServerLastRank':
      case '/stat/getServerTodayRank':
        return body(adminServerRankFixtures);
      case '/stat/getUserLastRank':
      case '/stat/getUserTodayRank':
        return body(adminUserRankFixtures);
      case '/plan/fetch':
        return body(adminPlanFixturesFor(scenario));
      case '/plan/save':
        if (interaction?.adminPlanSaveError) return error('订阅保存失败');
        return body(true);
      case '/plan/update':
        if (interaction?.adminPlanUpdateError) return error('订阅开关失败');
        return body(true);
      case '/plan/drop':
        if (interaction?.adminPlanDropError) return error('订阅删除失败');
        return body(true);
      case '/plan/sort':
        return body(true);
      case '/payment/fetch':
        return body(adminPaymentFixtures);
      case '/payment/save':
        if (interaction?.adminPaymentSaveError) return error('支付方式保存失败');
        return body(true);
      case '/payment/getPaymentMethods':
        return body(adminPaymentMethodsFixture);
      case '/payment/getPaymentForm': {
        const requestedPayment =
          requestData && typeof requestData.payment === 'string'
            ? requestData.payment
            : adminPaymentMethodsFixture[0];
        return body(
          adminPaymentFormFixtures[requestedPayment] ?? adminPaymentFormFixtures.AlipayF2F,
        );
      }
      case '/server/group/fetch':
        return body(adminServerGroupFixtures);
      case '/server/group/save':
        if (interaction?.adminServerGroupSaveError) return error('权限组保存失败');
        return body(true);
      case '/server/route/save':
        return body(true);
      case '/server/manage/getNodes':
        return body(adminServerNodeFixturesFor(scenario));
      case '/server/manage/sort':
        if (interaction?.adminServerSortError) return error('节点排序失败');
        return body(true);
      case '/server/route/fetch':
        return body(adminServerRouteFixtures);
      case '/system/getQueueStats':
        return body(adminQueueStatsFixture);
      case '/system/getQueueWorkload':
        return body(adminQueueWorkloadFixtures);
      case '/order/fetch':
        return body(adminOrderFixturesFor(scenario), {
          total: adminOrderFixturesFor(scenario).length,
        });
      case '/order/detail': {
        const requestedId = requestData?.id == null ? 1 : Number(requestData.id);
        return body(
          adminOrderFixturesFor(scenario).find((order) => order.id === requestedId) ??
            adminOrderFixtures[0],
        );
      }
      case '/order/assign':
        return body('VISUAL2026110099');
      case '/order/paid':
      case '/order/cancel':
      case '/order/update':
        return body(true);
      case '/user/fetch':
        return body(adminUserFixturesFor(scenario), {
          total: adminUserFixturesFor(scenario).length,
        });
      case '/user/update':
        if (interaction?.adminUserUpdateError) return error('邮箱格式错误');
        return body(true);
      case '/user/generate':
        return {
          contentType: 'text/csv',
          httpStatus: 200,
          rawBody: 'email,password\nparity.created@example.com,secret123\n',
        };
      case '/user/delUser':
        if (interaction?.adminUserDeleteError) return error('用户删除失败');
        return body(true);
      case '/user/ban':
        if (interaction?.adminUserBanError) return error('用户封禁失败');
        return body(true);
      case '/user/allDel':
        if (interaction?.adminUserAllDeleteError) return error('用户批量删除失败');
        return body(true);
      case '/user/dumpCSV':
        return {
          contentType: 'text/csv',
          httpStatus: 200,
          rawBody: 'id,email\n1,visual-user@example.com\n',
        };
      case '/user/sendMail':
        if (requestData?.subject === interaction?.adminUserSendMailFailureSubject) {
          return error('邮件加入队列失败');
        }
        return body(true);
      case '/user/getUserInfoById': {
        const requestedId = requestUrl.searchParams.has('id')
          ? Number(requestUrl.searchParams.get('id'))
          : 1;
        return body(
          adminUserFixturesFor(scenario).find((user) => user.id === requestedId) ??
            adminUserFixtures[0],
        );
      }
      case '/stat/getStatUser':
        return body(adminUserTrafficFixtures, { total: 25 });
      case '/ticket/fetch':
        if (requestUrl.searchParams.has('id')) {
          const requestedId = requestUrl.searchParams.get('id') ?? '7';
          const ticket =
            scenario.label === 'admin-ticket-detail'
              ? adminTicketDetailFixture
              : (adminTicketFixtures.find((item) => String(item.id) === requestedId) ??
                adminTicketFixtures[0]);
          return body(ticket);
        }
        return body(adminTicketFixtures, { total: adminTicketFixtures.length });
      case '/ticket/reply':
        return body(true);
      case '/ticket/close':
        return body(true);
      default:
        return body(null);
    }
  }

  // §5.8 modern knowledge detail (W3): /user/knowledge/{id} is source-world
  // only; the oracle keeps the legacy `?id=` branch on /user/knowledge/fetch.
  const knowledgeDetailMatch = /^\/api\/v1\/user\/knowledge\/(\d+)$/.exec(pathname);
  if (knowledgeDetailMatch) {
    return v2Body(
      modernKnowledgeDetailFixture(
        userKnowledgeFixtureById(knowledgeDetailMatch[1], interaction),
      ),
    );
  }

  // §5.5 modern commerce family (W4): path-identity routes are source-world
  // only; the oracle keeps the legacy /user/plan/* + /user/order/* cases below.
  const planDetailMatch = /^\/api\/v1\/user\/plans\/(\d+)$/.exec(pathname);
  if (planDetailMatch) {
    return v2Body(modernPlanFixture(userPlanFixtureById(planDetailMatch[1], scenario)));
  }
  const orderRouteMatch =
    /^\/api\/v1\/user\/orders\/([^/]+)(?:\/(status|cancel|checkout|stripe-intent))?$/.exec(
      pathname,
    );
  if (orderRouteMatch) {
    const [, tradeNo, orderAction] = orderRouteMatch;
    if (!orderAction) {
      return v2Body(modernOrderFixture(userOrderDetailFixtureFor(tradeNo, scenario)));
    }
    if (orderAction === 'status') {
      return v2Body({ status: 0 });
    }
    if (orderAction === 'cancel') {
      return v2Empty();
    }
    if (orderAction === 'stripe-intent') {
      return v2Body({
        public_key: 'pk_test_visual_parity',
        client_secret: 'pi_visual_secret_parity',
        amount: 1000,
        currency: 'cny',
      });
    }
    // checkout — the §9.3 discriminated union.
    if (interaction?.orderCheckoutError) {
      // A gateway pay failure surfaces from Rust as the 500 internal_error
      // problem (the legacy fixture's HTTP-200 `{code: 400}` "支付失败").
      return v2Problem(
        500,
        'Internal Server Error',
        'internal_error',
        '遇到了些问题，我们正在进行处理',
      );
    }
    const checkoutMethodId = Number(requestData?.method_id);
    if (checkoutMethodId === 2) {
      // Unreachable from the modern SPA (Stripe confirms via Payment Element,
      // never the checkout POST); mirror the legacy acknowledgment as settled.
      return v2Body({ kind: 'settled' });
    }
    if (checkoutMethodId === 3) {
      const redirectRoute =
        interaction.checkoutRedirectRoute ?? '/order/VISUAL2026110001?cashier=visual';
      return v2Body({
        kind: 'redirect',
        url: entryUrlForDialect(redirectRoute, routingDialectFor(target)),
      });
    }
    return v2Body({ kind: 'qr_code', payload: 'https://pay.example.test/qr/VISUAL2026110001' });
  }

  // §9.4 modern session revocation (W5): path-identified, 204, idempotent.
  if (/^\/api\/v1\/user\/sessions\/[^/]+$/.test(pathname) && method === 'DELETE') {
    return v2Empty();
  }

  // §5.7 modern user ticket family (W8): path-identity routes are
  // source-world only; the oracle keeps the legacy /user/ticket/* cases
  // below. The error details mirror the legacy fixture toast text — the
  // Tier-1 comparison keys on the problem `code`, presentation drops.
  const ticketRouteMatch = /^\/api\/v1\/user\/tickets(?:\/([^/]+)(?:\/(replies|close))?)?$/.exec(
    pathname,
  );
  if (ticketRouteMatch) {
    const [, ticketId, ticketAction] = ticketRouteMatch;
    if (!ticketId) {
      if (method === 'POST') {
        if (interaction?.ticketSaveError) {
          return v2Problem(422, 'Unprocessable Entity', 'validation_failed', '工单内容不能为空');
        }
        // §5.7: 201 with the created ticket id.
        return v2Body({ id: 10 }, 201);
      }
      return v2Body(userTicketFixturesFor(scenario).map(modernTicketFixture));
    }
    if (!ticketAction) {
      return v2Body(modernTicketDetailFixture(userTicketDetailFixtureFor(scenario)));
    }
    if (ticketAction === 'replies') {
      if (
        interaction?.ticketReplyError ||
        requestData?.message === interaction?.ticketReplyErrorMessage
      ) {
        return v2Problem(400, 'Bad Request', 'ticket_invalid_state', '工单回复失败');
      }
      return v2Empty();
    }
    // close
    if (interaction?.ticketCloseError) {
      return v2Problem(400, 'Bad Request', 'ticket_invalid_state', '工单关闭失败');
    }
    return v2Empty();
  }

  switch (pathname) {
    case '/api/v1/guest/comm/config':
      return body(guestConfigFixture);
    // §5.1 + §5.3 + §5.8 modern public/content family (W3). Only the source
    // world requests these paths; the oracle keeps the legacy cases below.
    case '/api/v1/public/config':
      return v2Body(modernPublicConfigFixture(guestConfigFixture));
    case '/api/v1/public/invite-views':
      return v2Empty();
    case '/api/v1/user/config':
      return v2Body(
        modernUserConfigFixture(
          interaction?.enableTelegramProfile
            ? {
                ...userCommConfigFixture,
                is_telegram: 1,
                telegram_discuss_link: 'https://t.me/visual_discuss',
              }
            : userCommConfigFixture,
        ),
      );
    case '/api/v1/user/notices':
      return v2Body({
        items: noticeFixtures.map(modernNoticeFixture),
        total: noticeFixtures.length,
      });
    case '/api/v1/user/knowledge':
      return v2Body(modernKnowledgeRecordFixture(userKnowledgeFixturesFor(interaction)));
    case '/api/v1/user/knowledge-categories':
      return v2Body(
        Object.keys(userKnowledgeFixturesFor(interaction)).map((category) => ({ category })),
      );
    case '/api/v1/user/telegram-bot':
      return v2Body({ username: 'legacy_bot' });
    // §5.5 modern commerce family (W4). Only the source world requests these
    // paths; the oracle keeps the legacy /user/plan/* + /user/order/* cases.
    case '/api/v1/user/plans':
      return v2Body(userPlanFixturesFor(scenario).map(modernPlanFixture));
    case '/api/v1/user/orders': {
      if (method === 'POST') {
        // §9.2: 201 with the created identity; the union arm picks the
        // scenario-specific trade_no exactly like the legacy sentinel did.
        if (requestData?.kind === 'deposit') {
          return v2Body({ trade_no: profileDepositTradeNo }, 201);
        }
        if (requestData?.period === 'reset_price') {
          return v2Body({ trade_no: dashboardResetPackageTradeNo }, 201);
        }
        return v2Body({ trade_no: 'VISUAL2026110099' }, 201);
      }
      if (scenario.userOrdersHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body(userOrderFixturesFor(scenario).map(modernOrderFixture));
    }
    case '/api/v1/user/payment-methods':
      return v2Body(paymentMethodFixtures.map(modernPaymentMethodFixture));
    case '/api/v1/user/coupons/check':
      if (interaction?.couponError) {
        return v2Problem(400, 'Bad Request', 'coupon_invalid', '优惠券无效');
      }
      return v2Body(modernCouponFixture(couponCheckFixture));
    // The Rust wire shape deliberately omits the permanent subscription
    // credential (`token`) from login/token2Login — clients read only
    // auth_data + is_admin and fetch the subscribe URL via /user/getSubscribe.
    // Cross-world safe: the reference bundles never read `data.token` either;
    // they persist only the `authorization` storage key.
    case '/api/v1/passport/auth/login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
      });
    case '/api/v1/passport/auth/token2Login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
      });
    case '/api/v1/user/checkLogin':
      return body({
        is_admin: isAdminScenario && !scenario.forceCheckLoginNotAdmin,
        is_login: !(scenario.forceUserUnauthorized || scenario.forceAdminUnauthorized),
      });
    // §5.2 modern auth family (W2). Only the source world requests these
    // paths; the oracle keeps the legacy passport/checkLogin cases above.
    case '/api/v1/auth/login':
    case '/api/v1/auth/token-login':
      return v2Body({ auth_data: 'VISUAL_PARITY_TOKEN', is_admin: isAdminScenario });
    case '/api/v1/auth/register':
      return v2Body({ auth_data: 'VISUAL_PARITY_TOKEN', is_admin: isAdminScenario }, 201);
    case '/api/v1/auth/password-reset':
    case '/api/v1/auth/email-codes':
      return v2Empty();
    case '/api/v1/auth/quick-login-url':
      return v2Body({
        url: 'https://visual.v2board.test/login?verify=VISUAL_VERIFY_TOKEN&redirect=dashboard',
      });
    case '/api/v1/auth/step-up':
      return v2Body({ expires_in: 900, step_up_token: 'VISUAL_STEP_UP_TOKEN' });
    case '/api/v1/auth/session': {
      if (method === 'DELETE') return v2Empty();
      const isLogin = !(scenario.forceUserUnauthorized || scenario.forceAdminUnauthorized);
      const isAdmin = isAdminScenario && !scenario.forceCheckLoginNotAdmin;
      // Mirror the Rust wire (golden auth.session*): `is_admin` appears only
      // on a logged-in admin session.
      return v2Body(isLogin && isAdmin ? { is_admin: true, is_login: true } : { is_login: isLogin });
    }
    case '/api/v1/user/info':
      return body(
        interaction?.telegramBoundProfile
          ? { ...userInfoFixture, telegram_id: 12345 }
          : scenario.bannedUser
            ? bannedUserInfoFixture
            : userInfoFixture,
      );
    // §5.3/§5.4 + §9.1/§9.4 modern profile family (W5). Only the source world
    // requests these paths; the oracle keeps the legacy /user/* cases.
    case '/api/v1/user/profile':
      if (method === 'PATCH') return v2Empty();
      return v2Body(
        modernUserProfileFixture(
          interaction?.telegramBoundProfile
            ? { ...userInfoFixture, telegram_id: 12345 }
            : scenario.bannedUser
              ? bannedUserInfoFixture
              : userInfoFixture,
        ),
      );
    case '/api/v1/user/password':
      return v2Empty();
    case '/api/v1/user/stats':
      return v2Body({ pending_order_count: 2, pending_ticket_count: 3, invited_user_count: 0 });
    case '/api/v1/user/sessions':
      return v2Body([
        {
          current: true,
          ip: '203.0.113.10',
          login_at: rfc3339FixtureTime(1_700_000_000),
          session_id: 'visual-parity-session',
          ua: 'Visual Parity Browser',
        },
      ]);
    case '/api/v1/user/gift-card-redemptions':
      if (interaction?.redeemGiftcardHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body({ type: 1, value: 1234 });
    case '/api/v1/user/telegram-binding':
      return v2Empty();
    case '/api/v1/user/subscription':
      return v2Body(modernSubscribeFixture(userSubscribeFixtureFor(scenario, interaction)));
    case '/api/v1/user/subscription/new-period':
      return v2Empty();
    case '/api/v1/user/subscription/reset-token':
      return v2Body({ subscribe_url: 'VISUAL-RESET-UUID' });
    // §5.4 modern service-usage family (W6). Only the source world requests
    // these paths; the oracle keeps the legacy /user/server/fetch +
    // /user/stat/getTrafficLog cases.
    case '/api/v1/user/servers':
      if (scenario.userServersHttpError) {
        return v2Problem(
          500,
          'Internal Server Error',
          'internal_error',
          '遇到了些问题，我们正在进行处理',
        );
      }
      return v2Body(userServerFixturesFor(scenario).map(modernServerFixture));
    case '/api/v1/user/traffic-logs':
      return v2Body(trafficFixtures.map(modernTrafficLogFixture));
    // §5.6 modern invite & commission family (W7). Only the source world
    // requests these paths; the oracle keeps the legacy /user/invite/* +
    // /user/transfer cases.
    case '/api/v1/user/invite':
      return v2Body(modernInviteFixture(inviteFixture));
    case '/api/v1/user/commissions':
      return v2Body({
        items: inviteDetailFixtures.map(modernCommissionFixture),
        total: inviteDetailFixtures.length,
      });
    case '/api/v1/user/invite-codes':
      // The one deliberate 204-no-body create (§1/§5.6).
      return v2Empty();
    // §5.7 modern withdrawal-ticket create (W8). Only the source world
    // requests this path; the oracle keeps the legacy /user/ticket/withdraw
    // case below.
    case '/api/v1/user/withdrawal-tickets':
      if (
        interaction?.withdrawError ||
        requestData?.withdraw_account === interaction?.withdrawErrorAccount
      ) {
        return v2Problem(400, 'Bad Request', 'withdraw_method_unsupported', '提现失败');
      }
      return v2Body({ id: 11 }, 201);
    case '/api/v1/user/commission-transfers':
      if (interaction?.transferError) {
        return v2Problem(
          400,
          'Bad Request',
          'insufficient_commission_balance',
          '推广佣金余额不足',
        );
      }
      return v2Empty();
    case '/api/v1/user/update':
      return body(true);
    case '/api/v1/user/redeemgiftcard':
      if (interaction?.redeemGiftcardHttpError) return httpError('Server Error', 500);
      return body(true, { type: 1, value: 1234 });
    case '/api/v1/user/changePassword':
      return body(true);
    case '/api/v1/user/transfer':
      if (interaction?.transferError) return error('余额不足');
      return body(true);
    case '/api/v1/user/resetSecurity':
      return body('VISUAL-RESET-UUID');
    case '/api/v1/user/unbindTelegram':
      return body(true);
    case '/api/v1/user/getSubscribe':
      return body(userSubscribeFixtureFor(scenario, interaction));
    case '/api/v1/user/getStat':
      return body([2, 3, 0]);
    case '/api/v1/user/plan/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userPlanFixtureById(requestUrl.searchParams.get('id'), scenario)
          : userPlanFixturesFor(scenario),
      );
    case '/api/v1/user/order/save':
      if (requestData?.period === 'deposit') return body(profileDepositTradeNo);
      if (requestData?.period === 'reset_price') return body(dashboardResetPackageTradeNo);
      return body('VISUAL2026110099');
    case '/api/v1/user/newPeriod':
      return body(true);
    case '/api/v1/user/order/fetch':
      if (scenario.userOrdersHttpError) return httpError('Server Error', 500);
      return body(userOrderFixturesFor(scenario));
    case '/api/v1/user/order/detail':
      return body(
        userOrderDetailFixtureFor(requestUrl.searchParams.get('trade_no'), scenario),
      );
    case '/api/v1/user/order/cancel':
      return body(true);
    case '/api/v1/user/order/getPaymentMethod':
      return body(paymentMethodFixtures);
    case '/api/v1/user/order/checkout': {
      if (interaction?.orderCheckoutError) return error('支付失败');
      const methodId = Number(requestData?.method);
      if (methodId === 2) {
        return body('stripe-accepted', { type: 1 });
      }
      if (methodId === 3) {
        // Backend-minted relative payment-return URL: the modern backend
        // mints path-style URLs while the legacy oracle mints `/#/…`
        // (docs/api-dialect.md Appendix A §W1), so emit per world.
        const redirectRoute =
          interaction.checkoutRedirectRoute ?? '/order/VISUAL2026110001?cashier=visual';
        return body(entryUrlForDialect(redirectRoute, routingDialectFor(target)), {
          type: 1,
        });
      }
      return body('https://pay.example.test/qr/VISUAL2026110001', { type: 0 });
    }
    case '/api/v1/user/order/check':
      return body(0);
    case '/api/v1/user/coupon/check':
      if (interaction?.couponError) return error('优惠券无效');
      return body(couponCheckFixture);
    case '/api/v1/user/server/fetch':
      if (scenario.userServersHttpError) return httpError('Server Error', 500);
      return body(userServerFixturesFor(scenario));
    case '/api/v1/user/stat/getTrafficLog':
      return body(trafficFixtures);
    case '/api/v1/user/invite/fetch':
      return body(inviteFixture);
    case '/api/v1/user/invite/details':
      return body(inviteDetailFixtures, { total: inviteDetailFixtures.length });
    case '/api/v1/user/invite/save':
      return body(true);
    case '/api/v1/user/ticket/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userTicketDetailFixtureFor(scenario)
          : scenario.emptyTickets
            ? []
            : userTicketFixturesFor(scenario),
      );
    case '/api/v1/user/ticket/save':
      if (interaction?.ticketSaveError) return error('工单内容不能为空');
      return body(true);
    case '/api/v1/user/ticket/reply':
      if (
        interaction?.ticketReplyError ||
        requestData?.message === interaction?.ticketReplyErrorMessage
      ) {
        return error('工单回复失败');
      }
      return body(true);
    case '/api/v1/user/ticket/close':
      if (interaction?.ticketCloseError) return error('工单关闭失败');
      return body(true);
    case '/api/v1/user/ticket/withdraw':
      if (
        interaction?.withdrawError ||
        requestData?.withdraw_account === interaction?.withdrawErrorAccount
      ) {
        return error('提现失败');
      }
      return body(true);
    case '/api/v1/user/knowledge/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userKnowledgeFixtureById(requestUrl.searchParams.get('id'), interaction)
          : userKnowledgeFixturesFor(interaction),
      );
    case '/api/v1/user/notice/fetch':
      return body(noticeFixtures);
    case '/api/v1/user/comm/config':
      return body(
        interaction?.enableTelegramProfile
          ? {
              ...userCommConfigFixture,
              is_telegram: 1,
              telegram_discuss_link: 'https://t.me/visual_discuss',
            }
          : userCommConfigFixture,
      );
    case '/api/v1/user/comm/getStripePublicKey':
      return body('pk_test_visual_parity');
    case '/api/v1/user/order/stripe/intent':
      return body({
        public_key: 'pk_test_visual_parity',
        client_secret: 'pi_visual_secret_parity',
        amount: 1000,
        currency: 'cny',
      });
    case '/api/v1/user/telegram/getBotInfo':
      return body({ username: 'legacy_bot' });
    default:
      return body(null);
  }
}

// ——— W3 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.1,
// §5.3, §5.8) ——— fixture-data.mjs stays the single legacy-shaped source;
// the source world serializes these derived modern shapes (booleans, real
// arrays, RFC 3339 timestamps, numeric rates) for the flipped family.
const rfc3339FixtureTime = (epochSeconds) =>
  new Date(epochSeconds * 1000).toISOString().replace(/\.\d{3}Z$/, 'Z');

const modernPublicConfigFixture = (fixture) => ({
  ...fixture,
  is_email_verify: fixture.is_email_verify !== 0,
  is_invite_force: fixture.is_invite_force !== 0,
  is_recaptcha: fixture.is_recaptcha !== 0,
  // §5.1: always an array — the legacy `0` disabled-sentinel died.
  email_whitelist_suffix: Array.isArray(fixture.email_whitelist_suffix)
    ? fixture.email_whitelist_suffix
    : [],
});

const modernUserConfigFixture = (fixture) => ({
  ...fixture,
  is_telegram: fixture.is_telegram !== 0,
  withdraw_close: fixture.withdraw_close !== 0,
  commission_distribution_enable: fixture.commission_distribution_enable !== 0,
  commission_distribution_l1: numericRate(fixture.commission_distribution_l1),
  commission_distribution_l2: numericRate(fixture.commission_distribution_l2),
  commission_distribution_l3: numericRate(fixture.commission_distribution_l3),
});

const numericRate = (value) => {
  if (value === null || value === undefined) return null;
  const rate = Number(value);
  return Number.isFinite(rate) ? rate : null;
};

const modernNoticeFixture = (notice) => ({
  id: notice.id,
  title: notice.title,
  content: notice.content,
  show: notice.show !== 0,
  img_url: notice.img_url ?? null,
  tags: notice.tags ?? null,
  created_at: rfc3339FixtureTime(notice.created_at),
  updated_at: rfc3339FixtureTime(notice.updated_at),
});

const modernKnowledgeSummaryFixture = (row) => ({
  id: row.id,
  category: row.category,
  title: row.title,
  sort: row.sort ?? null,
  show: row.show !== 0,
  updated_at: rfc3339FixtureTime(row.updated_at),
});

const modernKnowledgeDetailFixture = (row) => ({
  ...modernKnowledgeSummaryFixture(row),
  body: row.body,
  language: row.language,
  created_at: rfc3339FixtureTime(row.created_at),
});

const modernKnowledgeRecordFixture = (fixtures) =>
  Object.fromEntries(
    Object.entries(fixtures).map(([category, rows]) => [
      category,
      rows.map(modernKnowledgeSummaryFixture),
    ]),
  );

// ——— W4 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.5) ———
// booleans for show/renew, RFC 3339 timestamps, a numeric
// handling_fee_percent, and no legacy plan `count` (the modern plan body
// serializes remaining capacity in capacity_limit and drops the sold count).
const modernPlanFixture = (plan) => {
  const { count: _count, ...rest } = plan;
  return {
    ...rest,
    show: plan.show !== 0,
    renew: plan.renew !== 0,
    created_at: rfc3339FixtureTime(plan.created_at),
    updated_at: rfc3339FixtureTime(plan.updated_at),
  };
};

const modernOrderFixture = (order) => ({
  ...order,
  paid_at: order.paid_at == null ? null : rfc3339FixtureTime(order.paid_at),
  created_at: rfc3339FixtureTime(order.created_at),
  updated_at: rfc3339FixtureTime(order.updated_at),
  ...(order.plan
    ? {
        // Deposit orders carry the §5.5 `{id: 0, name: "deposit"}` synthetic
        // plan; real plans project like the plan routes.
        plan: order.plan.id === 0 ? { id: 0, name: 'deposit' } : modernPlanFixture(order.plan),
      }
    : {}),
});

const modernPaymentMethodFixture = (paymentMethod) => ({
  ...paymentMethod,
  handling_fee_percent:
    paymentMethod.handling_fee_percent == null
      ? null
      : Number(paymentMethod.handling_fee_percent),
});

const modernCouponFixture = (coupon) => ({
  ...coupon,
  show: coupon.show !== 0,
  started_at: rfc3339FixtureTime(coupon.started_at),
  ended_at: rfc3339FixtureTime(coupon.ended_at),
  created_at: rfc3339FixtureTime(coupon.created_at),
  updated_at: rfc3339FixtureTime(coupon.updated_at),
});

// ——— W10 modern-wire projection (docs/api-dialect.md §6.3) ——— RFC 3339
// windows and a real `used_user_ids` array (the legacy null sentinel died).
const modernGiftcardFixture = (giftcard) => ({
  ...giftcard,
  used_user_ids: giftcard.used_user_ids ?? [],
  started_at: rfc3339FixtureTime(giftcard.started_at),
  ended_at: rfc3339FixtureTime(giftcard.ended_at),
  created_at: rfc3339FixtureTime(giftcard.created_at),
  updated_at: rfc3339FixtureTime(giftcard.updated_at),
});

// ——— W11 modern-wire projections (docs/api-dialect.md §6.2, §6.4) ———
// boolean show/renew/enable flags, RFC 3339 timestamps, a numeric
// handling_fee_percent, and prices/fees that stay cents. The admin plan list
// keeps the sold `count` (unlike the user-side §5.5 plan body, which drops it).
const modernAdminPlanFixture = (plan) => ({
  ...plan,
  show: plan.show !== 0,
  renew: plan.renew !== 0,
  created_at: rfc3339FixtureTime(plan.created_at),
  updated_at: rfc3339FixtureTime(plan.updated_at),
});

const modernAdminPaymentFixture = (payment) => ({
  ...payment,
  enable: payment.enable !== 0,
  handling_fee_percent:
    payment.handling_fee_percent == null ? null : Number(payment.handling_fee_percent),
  created_at: rfc3339FixtureTime(payment.created_at),
  updated_at: rfc3339FixtureTime(payment.updated_at),
});

const modernAdminOrderFixture = (order) => ({
  ...order,
  paid_at: order.paid_at == null ? null : rfc3339FixtureTime(order.paid_at),
  created_at: rfc3339FixtureTime(order.created_at),
  updated_at: rfc3339FixtureTime(order.updated_at),
});

// ——— W12 modern-wire projections (docs/api-dialect.md §6.6) ——— the admin user
// list/detail keep 0/1 flag columns and integer bytes/cents, cross every epoch
// field as RFC 3339 UTC (nullable ones stay null), drop the `t` online marker
// and the stored password, and — on the detail only — attach the conditional
// `invite_user: {id, email}` object resolved from the inviter row.
const modernAdminUserFixture = (user) => ({
  ...user,
  password: '',
  expired_at: user.expired_at == null ? null : rfc3339FixtureTime(user.expired_at),
  last_login_at: user.last_login_at == null ? null : rfc3339FixtureTime(user.last_login_at),
  created_at: rfc3339FixtureTime(user.created_at),
  updated_at: rfc3339FixtureTime(user.updated_at),
});

const modernAdminUserDetailFixture = (user, users) => {
  const inviter =
    user.invite_user_id == null ? null : users.find((row) => row.id === user.invite_user_id);
  return {
    ...modernAdminUserFixture(user),
    subscribe_url: '',
    ...(inviter ? { invite_user: { id: inviter.id, email: inviter.email } } : {}),
  };
};

// ——— W5 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.3,
// §5.4) ——— boolean profile/subscription flags, RFC 3339 timestamps, and the
// subscription's explicit-null modern plan.
const modernUserProfileFixture = (fixture) => ({
  ...fixture,
  banned: fixture.banned !== 0,
  auto_renewal: fixture.auto_renewal !== 0,
  remind_expire: fixture.remind_expire !== 0,
  remind_traffic: fixture.remind_traffic !== 0,
  created_at: rfc3339FixtureTime(fixture.created_at),
  last_login_at:
    fixture.last_login_at == null ? null : rfc3339FixtureTime(fixture.last_login_at),
  expired_at: fixture.expired_at == null ? null : rfc3339FixtureTime(fixture.expired_at),
});

const modernSubscribeFixture = (fixture) => ({
  ...fixture,
  allow_new_period: fixture.allow_new_period !== 0,
  expired_at: fixture.expired_at == null ? null : rfc3339FixtureTime(fixture.expired_at),
  plan: fixture.plan ? modernPlanFixture(fixture.plan) : null,
});

// ——— W6 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.4) ———
// boolean is_online, numeric rate/port/server_rate, RFC 3339
// last_check_at/record_at.
const modernServerFixture = (server) => ({
  ...server,
  rate: Number(server.rate),
  port: Number(server.port),
  is_online: server.is_online !== 0,
  last_check_at: server.last_check_at == null ? null : rfc3339FixtureTime(server.last_check_at),
});

const modernTrafficLogFixture = (entry) => ({
  ...entry,
  record_at: rfc3339FixtureTime(entry.record_at),
  server_rate: Number(entry.server_rate),
});

// ——— W7 modern-wire projections (docs/api-dialect.md §4.5, §5.6, §8,
// §9.2) ——— the named invite stat object (was the legacy 5-tuple), RFC 3339
// timestamps, and the constant-status/caller-echo code columns dropped from
// the wire. Commission amounts stay integer cents.
const modernInviteFixture = (fixture) => ({
  codes: fixture.codes.map((code) => ({
    id: code.id,
    code: code.code,
    pv: code.pv,
    created_at: rfc3339FixtureTime(code.created_at),
    updated_at: rfc3339FixtureTime(code.updated_at),
  })),
  stat: {
    registered_count: fixture.stat[0],
    valid_commission: fixture.stat[1],
    pending_commission: fixture.stat[2],
    commission_rate: fixture.stat[3],
    available_commission: fixture.stat[4],
  },
});

const modernCommissionFixture = (entry) => ({
  id: entry.id,
  trade_no: entry.trade_no,
  order_amount: entry.order_amount,
  get_amount: entry.get_amount,
  created_at: rfc3339FixtureTime(entry.created_at),
});

// ——— W8 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.7) ———
// RFC 3339 timestamps, numeric level/status/reply_status enums, an
// always-present nullable last_reply_user_id, and no `message` stub on list
// rows (the thread ships only on the detail body).
const modernTicketFixture = (ticket) => ({
  id: ticket.id,
  user_id: ticket.user_id,
  subject: ticket.subject,
  level: ticket.level,
  status: ticket.status,
  reply_status: ticket.reply_status,
  last_reply_user_id: ticket.last_reply_user_id ?? null,
  created_at: rfc3339FixtureTime(ticket.created_at),
  updated_at: rfc3339FixtureTime(ticket.updated_at),
});

const modernTicketMessageFixture = (entry) => ({
  id: entry.id,
  user_id: entry.user_id,
  ticket_id: entry.ticket_id,
  message: entry.message,
  is_me: entry.is_me,
  created_at: rfc3339FixtureTime(entry.created_at),
  updated_at: rfc3339FixtureTime(entry.updated_at),
});

const modernTicketDetailFixture = (ticket) => ({
  ...modernTicketFixture(ticket),
  message: (ticket.message ?? []).map(modernTicketMessageFixture),
});

// ——— W9 modern-wire projections (docs/api-dialect.md §4.1, §6.1) ———
// the grouped config body flips every flag to a real boolean, keeps
// enums/counters as JSON numbers, converts email_port to a number, adds the
// §10.3 legacy_hash_redirect_enable site toggle, and pins
// commission_withdraw_limit to its decimal-string exception; queue
// stats/workload turn bare snake_case with a boolean status and RFC 3339
// last-run maps.
const modernConfigFlag = (value) => value !== 0 && value !== '0' && value !== false;

const modernAdminConfigFixture = (config) => ({
  ...config,
  invite: {
    ...config.invite,
    invite_force: modernConfigFlag(config.invite.invite_force),
    invite_never_expire: modernConfigFlag(config.invite.invite_never_expire),
    commission_first_time_enable: modernConfigFlag(config.invite.commission_first_time_enable),
    commission_auto_check_enable: modernConfigFlag(config.invite.commission_auto_check_enable),
    withdraw_close_enable: modernConfigFlag(config.invite.withdraw_close_enable),
    commission_distribution_enable: modernConfigFlag(
      config.invite.commission_distribution_enable,
    ),
    commission_withdraw_limit: String(config.invite.commission_withdraw_limit),
  },
  site: {
    ...config.site,
    force_https: modernConfigFlag(config.site.force_https),
    stop_register: modernConfigFlag(config.site.stop_register),
    legacy_hash_redirect_enable: false,
  },
  subscribe: {
    ...config.subscribe,
    plan_change_enable: modernConfigFlag(config.subscribe.plan_change_enable),
    surplus_enable: modernConfigFlag(config.subscribe.surplus_enable),
    allow_new_period: modernConfigFlag(config.subscribe.allow_new_period),
    new_order_event_id: modernConfigFlag(config.subscribe.new_order_event_id),
    renew_order_event_id: modernConfigFlag(config.subscribe.renew_order_event_id),
    change_order_event_id: modernConfigFlag(config.subscribe.change_order_event_id),
    show_info_to_server_enable: modernConfigFlag(config.subscribe.show_info_to_server_enable),
  },
  server: {
    ...config.server,
    device_limit_mode: modernConfigFlag(config.server.device_limit_mode),
  },
  email: {
    ...config.email,
    email_port: config.email.email_port == null ? null : Number(config.email.email_port),
  },
  telegram: {
    ...config.telegram,
    telegram_bot_enable: modernConfigFlag(config.telegram.telegram_bot_enable),
  },
  safe: {
    ...config.safe,
    email_verify: modernConfigFlag(config.safe.email_verify),
    safe_mode_enable: modernConfigFlag(config.safe.safe_mode_enable),
    email_whitelist_enable: modernConfigFlag(config.safe.email_whitelist_enable),
    email_gmail_limit_enable: modernConfigFlag(config.safe.email_gmail_limit_enable),
    recaptcha_enable: modernConfigFlag(config.safe.recaptcha_enable),
    register_limit_by_ip_enable: modernConfigFlag(config.safe.register_limit_by_ip_enable),
    password_limit_enable: modernConfigFlag(config.safe.password_limit_enable),
  },
});

const modernQueueLastRunMap = (stats) =>
  Object.fromEntries(Object.keys(stats.wait).map((name) => [name, rfc3339FixtureTime(1700000000)]));

const modernQueueStatsFixture = (stats) => ({
  failed_jobs: stats.failedJobs,
  jobs_per_minute: stats.jobsPerMinute,
  last_failure_at: {},
  last_run_at: modernQueueLastRunMap(stats),
  last_success_at: modernQueueLastRunMap(stats),
  paused_masters: stats.pausedMasters,
  periods: { failed_jobs: stats.periods.failedJobs, recent_jobs: stats.periods.recentJobs },
  processes: stats.processes,
  queue_with_max_runtime: stats.queueWithMaxRuntime,
  queue_with_max_throughput: stats.queueWithMaxThroughput,
  recent_jobs: stats.recentJobs,
  status: stats.status,
  wait: stats.wait,
});

const modernQueueWorkloadFixture = (row) => ({
  failed_jobs: 0,
  last_failure_at: null,
  last_run_at: rfc3339FixtureTime(1700000000),
  last_success_at: rfc3339FixtureTime(1700000000),
  length: row.length,
  name: row.name,
  processes: row.processes,
  recent_jobs: row.length,
  wait: row.wait,
});

export function userKnowledgeFixturesFor(interaction = {}) {
  return interaction.extremeKnowledgeContent ? extremeKnowledgeFixtures : knowledgeFixtures;
}

export function userKnowledgeFixtureById(id, interaction = {}) {
  const fixtures = userKnowledgeFixturesFor(interaction);
  const articles = Object.values(fixtures).flat();
  return articles.find((knowledge) => String(knowledge.id) === String(id)) ?? articles[0];
}

export function userPlanFixturesFor(scenario = {}) {
  if (scenario.emptyPlans) return [];
  if (scenario.longData) return longPlanFixtures;
  if (scenario.soldOutPlans) {
    return planFixtures.map((plan) => (plan.id === 2 ? { ...plan, capacity_limit: 0 } : plan));
  }
  return planFixtures;
}

export function userOrderFixturesFor(scenario = {}) {
  if (scenario.emptyOrders) return [];
  if (scenario.longData) return longOrderFixtures;
  return orderFixtures;
}

/** Resolve one order-detail fixture by trade_no (legacy query or W4 path). */
export function userOrderDetailFixtureFor(tradeNo, scenario = {}) {
  if (tradeNo === dashboardResetPackageTradeNo) return dashboardResetPackageOrderFixture;
  if (tradeNo === profileDepositTradeNo) return profileDepositOrderFixture;
  return (
    userOrderFixturesFor(scenario).find((order) => order.trade_no === tradeNo) ??
    orderFixtures.find((order) => order.trade_no === tradeNo) ??
    orderFixtures[0]
  );
}

export function userServerFixturesFor(scenario = {}) {
  if (scenario.emptyServers) return [];
  if (scenario.longData) return longUserServerFixtures;
  return serverFixtures;
}

export function userTicketFixturesFor(scenario = {}) {
  if (scenario.emptyTickets) return [];
  if (scenario.longData) return longTicketFixtures;
  return ticketFixtures;
}

export function userTicketDetailFixtureFor(scenario = {}) {
  if (scenario.longData) return longTicketDetailFixture;
  return ticketDetailFixture;
}

export function adminPlanFixturesFor(scenario = {}) {
  return scenario.longData ? longPlanFixtures : planFixtures;
}

export function adminServerNodeFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminServerNodeFixtures : adminServerNodeFixtures;
}

export function adminOrderFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminOrderFixtures : adminOrderFixtures;
}

export function adminUserFixturesFor(scenario = {}) {
  return scenario.longData ? longAdminUserFixtures : adminUserFixtures;
}

export function userSubscribeFixtureFor(scenario = {}, interaction = {}) {
  if (interaction?.newPeriodSubscribe) return newPeriodSubscribeFixture;
  if (scenario.noSubscription) return noSubscriptionFixture;
  if (scenario.expiredTrafficUsedUp) return expiredTrafficUsedUpSubscribeFixture;
  if (scenario.deviceLimitExpired) return deviceLimitExpiredSubscribeFixture;
  if (scenario.expiredSubscription) return expiredSubscriptionFixture;
  if (scenario.trafficUsedUp) return trafficUsedUpSubscribeFixture;
  if (scenario.deviceLimitReached) return deviceLimitReachedSubscribeFixture;
  return subscribeFixture;
}

export function userPlanFixtureById(id, scenario = {}) {
  const plan =
    userPlanFixturesFor(scenario).find((item) => String(item.id) === String(id)) ?? planFixtures[0];
  if (scenario.nonRenewablePlan) return { ...plan, renew: 0 };
  return plan;
}

export async function waitForAdminGroups(adminGroupsReady) {
  await Promise.race([adminGroupsReady, delay(1_000)]);
  await delay(300);
}

export function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

// World-aware serialization seam (docs/api-dialect.md §13.5): the fixture
// response object is canonical; the dialect emitter owns each world's wire
// shape (migrated families mark fixtures `dialect: 'v2'` for the source
// world; everything else emits legacy in both worlds).
export function fulfillApiResponse(route, body, world) {
  const wire = emitFixtureResponse(world, body);
  return route.fulfill({
    body: wire.body,
    contentType: wire.contentType,
    status: wire.status,
  });
}

export function fulfillPlainJson(route, data) {
  route.fulfill({
    body: JSON.stringify(data),
    contentType: 'application/json',
    status: 200,
  });
}

export function stripeFixtureScript({
  confirmError = false,
  legacyToken = null,
  paymentElementComplete = false,
} = {}) {
  const tokenPayload = legacyToken ? { id: legacyToken, object: 'token' } : null;
  return `
(() => {
  const legacyOracleToken = ${JSON.stringify(tokenPayload)};
  const visualPaymentElementComplete = ${JSON.stringify(Boolean(paymentElementComplete))};
  window.Stripe = function Stripe() {
    let lastElement = null;
    const createElement = () => {
      const handlers = new Map();
      const fire = (event, payload) => {
        const eventHandlers = handlers.get(event) || [];
        eventHandlers.forEach((handler) => handler(payload));
      };
      const fireElementComplete = () => {
        if (!visualPaymentElementComplete) return;
        [0, 50, 150, 300, 750].forEach((delay) => {
          setTimeout(() => fire('change', { complete: true, empty: false }), delay);
        });
      };
      return {
        blur() {},
        clear() {},
        destroy() {},
        focus() {},
        mount(target) {
          const element = document.createElement('div');
          element.className = 'StripeElement';
          target.appendChild(element);
          fire('ready', {});
          fireElementComplete();
        },
        off(event, handler) {
          const eventHandlers = handlers.get(event) || [];
          handlers.set(event, eventHandlers.filter((item) => item !== handler));
        },
        on(event, handler) {
          handlers.set(event, [...(handlers.get(event) || []), handler]);
          if (event === 'change') fireElementComplete();
        },
        unmount() {},
        update() {},
      };
    };
    return {
      _registerWrapper() {},
      registerAppInfo() {},
      elements() {
        return {
          getElement() {
            return lastElement;
          },
          create() {
            lastElement = createElement();
            return lastElement;
          },
        };
      },
      // React Stripe.js validates the complete Stripe instance shape, which
      // includes createToken even when the application only uses Payment
      // Element. Keep the SDK-shaped method in both worlds, but make any source
      // invocation fail loudly: only the frozen oracle may receive a token.
      createToken() {
        if (!legacyOracleToken) {
          window.__visualParityUnexpectedStripeCreateTokenCount =
            (window.__visualParityUnexpectedStripeCreateTokenCount ?? 0) + 1;
          return Promise.reject(new Error('Modern source must not call Stripe createToken'));
        }
        return Promise.resolve({ token: legacyOracleToken });
      },
      createPaymentMethod() {
        return Promise.resolve({});
      },
      confirmCardPayment() {
        return Promise.resolve({});
      },
      confirmPayment() {
        window.__visualParityUserStripeConfirmCount =
          (window.__visualParityUserStripeConfirmCount ?? 0) + 1;
        return Promise.resolve(
          ${JSON.stringify(Boolean(confirmError))}
            ? { error: { message: 'Stripe confirmation failed' } }
            : { paymentIntent: { status: 'succeeded' } },
        );
      },
    };
  };
  window.Stripe.version = 'dahlia';
})();
`;
}

export async function seedLegacyAdminTicketDetailStore(page) {
  await page.waitForFunction(() => window.g_app?._store, null, { timeout: 5_000 });
  await page.evaluate(
    ({ plans, ticket, userInfo, users }) => {
      const store = window.g_app?._store;
      if (!store) return;
      store.dispatch({ type: 'plan/setState', payload: { plans } });
      store.dispatch({
        type: 'user/setState',
        payload: {
          pagination: { current: 1, pageSize: 10, total: users.length },
          userInfo,
          users,
        },
      });
      store.dispatch({
        type: 'ticket/setState',
        payload: {
          filter: { status: 0 },
          pagination: { current: 1, pageSize: 10, total: 1 },
          ticket,
          tickets: [ticket],
        },
      });
    },
    {
      plans: adminPlanStoreFixtures,
      ticket: adminTicketDetailFixture,
      userInfo: userInfoFixture,
      users: adminUserStoreFixtures,
    },
  );
}

export async function seedLegacyAdminStore(page, scenario = {}) {
  await page
    .waitForFunction(() => window.g_app?._store, null, { timeout: 5_000 })
    .catch(() => undefined);
  for (let attempt = 0; attempt < 6; attempt += 1) {
    await page.evaluate(
      ({
        config,
        coupons,
        emailTemplates,
        giftcards,
        knowledgeCategories,
        knowledges,
        notices,
        orders,
        payments,
        plans,
        queueStats,
        queueWorkload,
        serverGroups,
        serverNodes,
        serverRoutes,
        skipCoupons,
        skipGiftcards,
        skipKnowledge,
        skipNotices,
        skipOrders,
        skipPayments,
        skipPlans,
        skipServerManage,
        skipTickets,
        skipUsers,
        stat,
        ticketDetail,
        tickets,
        userInfo,
        users,
      }) => {
        const store = window.g_app?._store;
        if (!store) return;
        if (!store.__visualParityStateGuard) {
          const originalGetState = store.getState.bind(store);
          store.getState = () => {
            const state = originalGetState();
            const ensureArrayState = (namespace, key, value) => {
              if (!state[namespace] || typeof state[namespace] !== 'object') {
                state[namespace] = {};
              }
              if (!Array.isArray(state[namespace][key])) {
                state[namespace][key] = value;
              }
            };

            ensureArrayState('serverGroup', 'groups', serverGroups);
            ensureArrayState('serverManage', 'servers', serverNodes);
            ensureArrayState('serverRoute', 'routes', serverRoutes);
            return state;
          };
          store.__visualParityStateGuard = true;
        }
        if (!skipUsers) {
          store.dispatch({
            type: 'user/setState',
            payload: {
              pagination: { current: 1, pageSize: 10, total: users.length },
              userInfo,
              users,
            },
          });
        }
        store.dispatch({ type: 'stat/save', payload: stat });
        store.dispatch({
          type: 'config/setState',
          payload: {
            ...config,
            emailTemplate: emailTemplates,
          },
        });
        if (!skipCoupons) {
          store.dispatch({
            type: 'coupon/setState',
            payload: {
              coupons,
              pagination: { current: 1, pageSize: 10, total: coupons.length },
            },
          });
        }
        if (!skipGiftcards) {
          store.dispatch({
            type: 'giftcard/setState',
            payload: {
              giftcards,
              pagination: { current: 1, pageSize: 10, total: giftcards.length },
            },
          });
        }
        if (!skipOrders) {
          store.dispatch({
            type: 'order/setState',
            payload: {
              orders,
              pagination: { current: 0, pageSize: 10, total: orders.length },
            },
          });
        }
        if (!skipPayments) {
          store.dispatch({ type: 'payment/setState', payload: { payments } });
        }
        if (!skipPlans) {
          store.dispatch({ type: 'plan/setState', payload: { plans } });
        }
        store.dispatch({
          type: 'system/save',
          payload: { queueStats, queueWorkload },
        });
        if (!skipKnowledge) {
          store.dispatch({
            type: 'knowledge/setState',
            payload: { categorys: knowledgeCategories, knowledges },
          });
        }
        if (!skipNotices) {
          store.dispatch({ type: 'notice/setState', payload: { notices } });
        }
        store.dispatch({ type: 'serverGroup/setState', payload: { groups: serverGroups } });
        if (!skipServerManage) {
          store.dispatch({
            type: 'serverManage/setState',
            payload: { fetchLoading: false, servers: serverNodes, sortMode: false },
          });
        }
        store.dispatch({
          type: 'serverRoute/setState',
          payload: { fetchLoading: false, routes: serverRoutes },
        });
        if (!skipTickets) {
          store.dispatch({
            type: 'ticket/setState',
            payload: {
              filter: { status: 0 },
              pagination: { current: 1, pageSize: 10, total: tickets.length },
              ticket: ticketDetail,
              tickets,
            },
          });
        }
      },
      {
        config: adminConfigFixture,
        coupons: adminCouponStoreFixtures,
        emailTemplates: adminEmailTemplateFixtures,
        giftcards: adminGiftcardStoreFixtures,
        knowledgeCategories: Array.from(
          new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category)),
        ),
        knowledges: adminKnowledgeFixtures,
        notices: adminNoticeFixtures,
        orders: adminOrderFixturesFor(scenario),
        payments: adminPaymentFixtures,
        plans: toAdminPlanStoreFixtures(adminPlanFixturesFor(scenario)),
        queueStats: adminQueueStatsFixture,
        queueWorkload: adminQueueWorkloadFixtures,
        serverGroups: adminServerGroupFixtures,
        serverNodes: scenario.adminServerManageTimeout ? [] : adminServerNodeFixturesFor(scenario),
        serverRoutes: adminServerRouteFixtures,
        skipCoupons: Boolean(scenario.adminCouponsTimeout),
        skipGiftcards: Boolean(scenario.adminGiftcardsTimeout),
        skipKnowledge: Boolean(scenario.adminKnowledgeTimeout),
        skipNotices: Boolean(scenario.adminNoticesTimeout),
        skipOrders: Boolean(scenario.adminOrdersHttpError || scenario.adminOrdersTimeout),
        skipPayments: Boolean(scenario.adminPaymentsTimeout),
        skipPlans: Boolean(scenario.adminPlansTimeout),
        skipServerManage: Boolean(scenario.adminServerManageTimeout),
        skipTickets: Boolean(scenario.adminTicketsTimeout),
        skipUsers: Boolean(scenario.adminUsersHttpError || scenario.adminUsersTimeout),
        stat: adminStatFixture,
        ticketDetail: adminTicketDetailFixture,
        tickets: adminTicketFixtures,
        userInfo: userInfoFixture,
        users: toAdminUserStoreFixtures(adminUserFixturesFor(scenario)),
      },
    );
    await page.waitForTimeout(50);
  }
  await page.waitForFunction(
    () => {
      const state = window.g_app?._store?.getState?.();
      return (
        Array.isArray(state?.serverGroup?.groups) &&
        Array.isArray(state?.serverManage?.servers) &&
        Array.isArray(state?.serverRoute?.routes)
      );
    },
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(100);
}

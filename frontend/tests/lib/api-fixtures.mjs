import { adminPath } from './env.mjs';
import { emitFixtureResponse } from './dialect/fixture-emitters.mjs';
import { canonicalizeRequest } from './dialect/request-canonicalizer.mjs';
import {
  adminFixtureEndpoint,
  apiFixtureResponse,
  readRequestData,
} from './api-fixture-response.mjs';
import { seedLegacyAdminTicketDetailStore, stripeFixtureScript } from './legacy-store-seed.mjs';

export async function installApiFixtures(page, scenario, target, interaction = {}) {
  const isAdminScenario = scenario.label.startsWith('admin-');
  // W14 (§6.9): runners that drive the staff mirror directly need to speak
  // the current world's wire dialect; stash it on the page handle.
  page.__parityWorld = target;
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
    // W14 (§6.5): the modern admin ticket family carries identity in the
    // path; the fetch counter spans list + detail exactly like the shared
    // legacy /ticket/fetch path did. Match both worlds' spellings for the
    // counters, captures, timeout, and delay knobs below.
    const isAdminTicketFetch =
      adminEndpoint === '/ticket/fetch' ||
      ((adminEndpoint === '/tickets' || /^\/tickets\/\d+$/.test(adminEndpoint ?? '')) &&
        requestMethod === 'GET');
    const isAdminTicketReply =
      adminEndpoint === '/ticket/reply' || /^\/tickets\/\d+\/replies$/.test(adminEndpoint ?? '');
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

    if (
      adminEndpoint === '/server/manage/getNodes' ||
      (adminEndpoint === '/nodes' && requestMethod === 'GET')
    ) {
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
    // W13 (§6.7): protocol saves are the legacy /server/{type}/save POST, the
    // modern POST /servers/{type} create, or the modern full-body (non-toggle)
    // PATCH /servers/{type}/{id} edit.
    const adminServerNodeSaveMatch =
      /^\/server\/([^/]+)\/save$/.exec(adminEndpoint ?? '') ??
      (requestMethod === 'POST'
        ? /^\/servers\/([^/]+)$/.exec(adminEndpoint ?? '')
        : requestMethod === 'PATCH' && !isShowOnlyPatch
          ? /^\/servers\/([^/]+)\/\d+$/.exec(adminEndpoint ?? '')
          : null);
    const isAdminServerGroupSave =
      adminEndpoint === '/server/group/save' ||
      (adminEndpoint === '/server-groups' && requestMethod === 'POST') ||
      (/^\/server-groups\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'PATCH');
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
    // W13 (§6.7): match both worlds' spellings for the server family counters
    // and captures; mutations capture canonically so the legacy body-id /
    // bracket-array form and the modern path-identity JSON compare as one
    // contract.
    if (
      adminEndpoint === '/server/group/fetch' ||
      (adminEndpoint === '/server-groups' && requestMethod === 'GET')
    ) {
      page.__visualParityAdminServerGroupFetchCount =
        (page.__visualParityAdminServerGroupFetchCount ?? 0) + 1;
    }
    if (isAdminServerGroupSave) {
      const groupSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminServerGroupSave = groupSaveRequest;
      page.__visualParityAdminServerGroupSaveCount =
        (page.__visualParityAdminServerGroupSaveCount ?? 0) + 1;
      page.__visualParityAdminServerGroupSaveRequests = [
        ...(page.__visualParityAdminServerGroupSaveRequests ?? []),
        groupSaveRequest,
      ];
    }
    if (
      adminEndpoint === '/server/route/fetch' ||
      (adminEndpoint === '/server-routes' && requestMethod === 'GET')
    ) {
      page.__visualParityAdminServerRouteFetchCount =
        (page.__visualParityAdminServerRouteFetchCount ?? 0) + 1;
    }
    if (
      adminEndpoint === '/server/route/save' ||
      (adminEndpoint === '/server-routes' && requestMethod === 'POST') ||
      (/^\/server-routes\/\d+$/.test(adminEndpoint ?? '') && requestMethod === 'PATCH')
    ) {
      const routeSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminServerRouteSave = routeSaveRequest;
      page.__visualParityAdminServerRouteSaveCount =
        (page.__visualParityAdminServerRouteSaveCount ?? 0) + 1;
      page.__visualParityAdminServerRouteSaveRequests = [
        ...(page.__visualParityAdminServerRouteSaveRequests ?? []),
        routeSaveRequest,
      ];
    }
    if (
      adminEndpoint === '/server/manage/getNodes' ||
      (adminEndpoint === '/nodes' && requestMethod === 'GET')
    ) {
      page.__visualParityAdminServerNodeFetchCount =
        (page.__visualParityAdminServerNodeFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/server/manage/sort' || adminEndpoint === '/nodes/sort') {
      // Both worlds POST the identical grouped `{type: {id: sort}}` JSON body.
      page.__visualParityLastAdminServerSort = requestData;
      page.__visualParityAdminServerSortCount = (page.__visualParityAdminServerSortCount ?? 0) + 1;
      page.__visualParityAdminServerSortRequests = [
        ...(page.__visualParityAdminServerSortRequests ?? []),
        requestData,
      ];
    }
    if (adminServerNodeSaveMatch) {
      const nodeSaveRequest = canonicalRequestCapture();
      page.__visualParityLastAdminServerNodeSave = nodeSaveRequest;
      page.__visualParityAdminServerNodeSaveCount =
        (page.__visualParityAdminServerNodeSaveCount ?? 0) + 1;
      page.__visualParityAdminServerNodeSaveRequests = [
        ...(page.__visualParityAdminServerNodeSaveRequests ?? []),
        nodeSaveRequest,
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
    if (isAdminTicketFetch) {
      page.__visualParityAdminTicketFetchCount =
        (page.__visualParityAdminTicketFetchCount ?? 0) + 1;
      // Canonical capture (§6.5/§8): the legacy bracket reply_status +
      // current/pageSize query and the modern repeated-key + page/per_page
      // query fold to one flat shape.
      page.__visualParityAdminTicketFetchRequests = [
        ...(page.__visualParityAdminTicketFetchRequests ?? []),
        canonicalRequestCapture(),
      ];
    }
    if (isAdminTicketReply) {
      // Canonical capture (§6.5): the legacy `{id, message}` body and the
      // modern `{message}` + path id fold onto one flat contract.
      const ticketReplyRequest = canonicalRequestCapture();
      page.__visualParityLastAdminTicketReply = ticketReplyRequest;
      page.__visualParityAdminTicketReplyCount =
        (page.__visualParityAdminTicketReplyCount ?? 0) + 1;
      page.__visualParityAdminTicketReplyRequests = [
        ...(page.__visualParityAdminTicketReplyRequests ?? []),
        ticketReplyRequest,
      ];
    }
    if (
      adminEndpoint === '/stat/getStatUser' ||
      (adminEndpoint === '/stats/user-traffic' && requestMethod === 'GET')
    ) {
      // Canonical capture (§6.8): the legacy page/pageSize query and the
      // modern page/per_page spelling fold to one flat shape.
      page.__visualParityLastAdminUserTrafficQuery = canonicalRequestCapture();
    }
    // W14 (§6.9): the staff mirror keeps its own /api/v1/staff prefix in both
    // worlds; capture the full canonicalized requests (routeId included) so
    // the runner can prove the mirror carries one Tier-1 contract.
    if (pathname.startsWith('/api/v1/staff/')) {
      page.__visualParityStaffTicketRequests = [
        ...(page.__visualParityStaffTicketRequests ?? []),
        canonicalizeRequest(target, {
          method: requestMethod,
          url: route.request().url(),
          postData: route.request().postData(),
          securePath: adminPath,
        }),
      ];
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
      (scenario.adminTicketsTimeout && isAdminTicketFetch) ||
      (scenario.adminServerManageTimeout &&
        (adminEndpoint === '/server/manage/getNodes' ||
          (adminEndpoint === '/nodes' && requestMethod === 'GET'))) ||
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
    if (isAdminTicketReply && interaction.delayAdminTicketReplyMs) {
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
        adminEndpoint === '/server/manage/sort' ||
        adminEndpoint === '/nodes/sort') &&
      interaction.delayAdminMutationMs
    ) {
      await delay(interaction.delayAdminMutationMs);
    }
    if (isAdminServerGroupSave && interaction.delayAdminServerGroupSaveMs) {
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

    if (
      (adminEndpoint === '/server/group/fetch' ||
        (adminEndpoint === '/server-groups' && requestMethod === 'GET')) &&
      !adminGroupsResolved
    ) {
      adminGroupsResolved = true;
      resolveAdminGroupsReady();
    }
  });
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

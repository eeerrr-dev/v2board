import { adminPath } from './env.mjs';
import { emitFixtureResponse } from './dialect/fixture-emitters.mjs';
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
    ({ authenticated, darkMode, locale, preserveRuntimeDarkMode, preserveRuntimeLocale }) => {
      const initializeDarkModeCookie = () => {
        document.cookie = darkMode
          ? 'dark_mode=1;path=/'
          : 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
      };
      const initializeLocale = () => {
        if (!locale) return;
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
    page.__visualParityDiagnostics?.push(`${route.request().method()} ${pathname}`);
    const adminEndpoint = adminFixtureEndpoint(pathname);

    if (adminEndpoint) {
      page.__visualParityDiagnostics?.push(`fixture admin ${adminEndpoint}`);
    } else if (pathname === '/api/v1/user/checkLogin') {
      page.__visualParityDiagnostics?.push(`fixture checkLogin admin=${isAdminScenario}`);
    } else if (pathname === '/api/v1/user/info') {
      page.__visualParityDiagnostics?.push('fixture user info');
    }

    if (adminEndpoint === '/server/manage/getNodes') {
      await waitForAdminGroups(adminGroupsReady);
    }
    const requestData = readRequestData(route.request());
    const adminServerNodeSaveMatch = /^\/server\/([^/]+)\/save$/.exec(adminEndpoint ?? '');
    if (pathname === '/api/v1/user/info') {
      page.__visualParityUserInfoFetchCount = (page.__visualParityUserInfoFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/getSubscribe') {
      page.__visualParityUserSubscribeFetchCount =
        (page.__visualParityUserSubscribeFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/unbindTelegram') {
      page.__visualParityUserUnbindTelegramCount =
        (page.__visualParityUserUnbindTelegramCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/resetSecurity') {
      page.__visualParityUserResetSecurityCount =
        (page.__visualParityUserResetSecurityCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/update') {
      page.__visualParityLastUserUpdate = requestData;
      page.__visualParityUserUpdateRequests = [
        ...(page.__visualParityUserUpdateRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/redeemgiftcard') {
      page.__visualParityLastUserRedeemGiftcard = requestData;
      page.__visualParityUserRedeemGiftcardCount =
        (page.__visualParityUserRedeemGiftcardCount ?? 0) + 1;
      page.__visualParityUserRedeemGiftcardRequests = [
        ...(page.__visualParityUserRedeemGiftcardRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/changePassword') {
      page.__visualParityLastUserChangePassword = requestData;
      page.__visualParityUserChangePasswordRequests = [
        ...(page.__visualParityUserChangePasswordRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/transfer') {
      page.__visualParityLastUserTransfer = requestData;
      page.__visualParityUserTransferCount = (page.__visualParityUserTransferCount ?? 0) + 1;
      page.__visualParityUserTransferRequests = [
        ...(page.__visualParityUserTransferRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/invite/save') {
      page.__visualParityUserInviteGenerateCount =
        (page.__visualParityUserInviteGenerateCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/newPeriod') {
      page.__visualParityLastUserNewPeriod = requestData;
      page.__visualParityUserNewPeriodCount = (page.__visualParityUserNewPeriodCount ?? 0) + 1;
      page.__visualParityUserNewPeriodRequests = [
        ...(page.__visualParityUserNewPeriodRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/order/save') {
      page.__visualParityLastUserOrderSave = requestData;
      page.__visualParityUserOrderSaveCount = (page.__visualParityUserOrderSaveCount ?? 0) + 1;
      page.__visualParityUserOrderSaveRequests = [
        ...(page.__visualParityUserOrderSaveRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/order/fetch') {
      page.__visualParityUserOrderFetchCount = (page.__visualParityUserOrderFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/plan/fetch' && !requestUrl.searchParams.has('id')) {
      page.__visualParityUserPlanFetchCount = (page.__visualParityUserPlanFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/server/fetch') {
      page.__visualParityUserServerFetchCount = (page.__visualParityUserServerFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/stat/getTrafficLog') {
      page.__visualParityUserTrafficFetchCount =
        (page.__visualParityUserTrafficFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/knowledge/fetch' && !requestUrl.searchParams.has('id')) {
      page.__visualParityUserKnowledgeFetchCount =
        (page.__visualParityUserKnowledgeFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/order/checkout') {
      page.__visualParityLastUserOrderCheckout = requestData;
      page.__visualParityUserOrderCheckoutCount =
        (page.__visualParityUserOrderCheckoutCount ?? 0) + 1;
      page.__visualParityUserOrderCheckoutRequests = [
        ...(page.__visualParityUserOrderCheckoutRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/coupon/check') {
      page.__visualParityLastUserCouponCheck = requestData;
      page.__visualParityUserCouponCheckCount = (page.__visualParityUserCouponCheckCount ?? 0) + 1;
      page.__visualParityUserCouponCheckRequests = [
        ...(page.__visualParityUserCouponCheckRequests ?? []),
        requestData,
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
    if (pathname === '/api/v1/user/order/stripe/intent') {
      page.__visualParityUserStripePrepareCount =
        (page.__visualParityUserStripePrepareCount ?? 0) + 1;
      page.__visualParityUserStripeIntentCount =
        (page.__visualParityUserStripeIntentCount ?? 0) + 1;
      page.__visualParityUserStripeIntentRequests = [
        ...(page.__visualParityUserStripeIntentRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/fetch') {
      page.__visualParityUserTicketFetchCount = (page.__visualParityUserTicketFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/ticket/reply') {
      page.__visualParityLastUserTicketReply = requestData;
      page.__visualParityUserTicketReplyCount = (page.__visualParityUserTicketReplyCount ?? 0) + 1;
      page.__visualParityUserTicketReplyRequests = [
        ...(page.__visualParityUserTicketReplyRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/close') {
      page.__visualParityLastUserTicketClose = requestData;
      page.__visualParityUserTicketCloseCount = (page.__visualParityUserTicketCloseCount ?? 0) + 1;
      page.__visualParityUserTicketCloseRequests = [
        ...(page.__visualParityUserTicketCloseRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/save') {
      page.__visualParityLastUserTicketSave = requestData;
      page.__visualParityUserTicketSaveCount = (page.__visualParityUserTicketSaveCount ?? 0) + 1;
      page.__visualParityUserTicketSaveRequests = [
        ...(page.__visualParityUserTicketSaveRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/withdraw') {
      page.__visualParityLastUserWithdraw = requestData;
      page.__visualParityUserWithdrawCount = (page.__visualParityUserWithdrawCount ?? 0) + 1;
      page.__visualParityUserWithdrawRequests = [
        ...(page.__visualParityUserWithdrawRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/order/cancel') {
      page.__visualParityLastUserOrderCancel = requestData;
      page.__visualParityUserOrderCancelCount = (page.__visualParityUserOrderCancelCount ?? 0) + 1;
      page.__visualParityUserOrderCancelRequests = [
        ...(page.__visualParityUserOrderCancelRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/config/fetch') {
      page.__visualParityAdminConfigFetchCount =
        (page.__visualParityAdminConfigFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/config/save') {
      page.__visualParityLastAdminConfigSave = requestData;
      page.__visualParityAdminConfigSaveCount = (page.__visualParityAdminConfigSaveCount ?? 0) + 1;
      page.__visualParityAdminConfigSaveRequests = [
        ...(page.__visualParityAdminConfigSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/order/assign') {
      page.__visualParityLastAdminOrderAssign = requestData;
    }
    if (adminEndpoint === '/order/paid') {
      page.__visualParityLastAdminOrderPaid = requestData;
    }
    if (adminEndpoint === '/order/update') {
      page.__visualParityLastAdminOrderUpdate = requestData;
    }
    if (adminEndpoint === '/order/fetch') {
      page.__visualParityAdminOrderFetchCount = (page.__visualParityAdminOrderFetchCount ?? 0) + 1;
      page.__visualParityLastAdminOrderFetchQuery = Object.fromEntries(
        requestUrl.searchParams.entries(),
      );
    }
    if (adminEndpoint === '/plan/fetch') {
      page.__visualParityAdminPlanFetchCount = (page.__visualParityAdminPlanFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/plan/save') {
      page.__visualParityLastAdminPlanSave = requestData;
      page.__visualParityAdminPlanSaveCount = (page.__visualParityAdminPlanSaveCount ?? 0) + 1;
      page.__visualParityAdminPlanSaveRequests = [
        ...(page.__visualParityAdminPlanSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/plan/update') {
      page.__visualParityLastAdminPlanUpdate = requestData;
      page.__visualParityAdminPlanUpdateCount = (page.__visualParityAdminPlanUpdateCount ?? 0) + 1;
      page.__visualParityAdminPlanUpdateRequests = [
        ...(page.__visualParityAdminPlanUpdateRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/plan/drop') {
      page.__visualParityLastAdminPlanDrop = requestData;
      page.__visualParityAdminPlanDropCount = (page.__visualParityAdminPlanDropCount ?? 0) + 1;
      page.__visualParityAdminPlanDropRequests = [
        ...(page.__visualParityAdminPlanDropRequests ?? []),
        requestData,
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
    if (adminEndpoint === '/coupon/fetch') {
      page.__visualParityAdminCouponFetchCount =
        (page.__visualParityAdminCouponFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/coupon/generate') {
      page.__visualParityLastAdminCouponGenerate = requestData;
      page.__visualParityAdminCouponGenerateCount =
        (page.__visualParityAdminCouponGenerateCount ?? 0) + 1;
      page.__visualParityAdminCouponGenerateRequests = [
        ...(page.__visualParityAdminCouponGenerateRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/giftcard/fetch') {
      page.__visualParityAdminGiftcardFetchCount =
        (page.__visualParityAdminGiftcardFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/giftcard/generate') {
      page.__visualParityLastAdminGiftcardGenerate = requestData;
      page.__visualParityAdminGiftcardGenerateCount =
        (page.__visualParityAdminGiftcardGenerateCount ?? 0) + 1;
      page.__visualParityAdminGiftcardGenerateRequests = [
        ...(page.__visualParityAdminGiftcardGenerateRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/knowledge/fetch' && !requestUrl.searchParams.has('id')) {
      page.__visualParityAdminKnowledgeFetchCount =
        (page.__visualParityAdminKnowledgeFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/knowledge/save') {
      page.__visualParityLastAdminKnowledgeSave = requestData;
      page.__visualParityAdminKnowledgeSaveCount =
        (page.__visualParityAdminKnowledgeSaveCount ?? 0) + 1;
      page.__visualParityAdminKnowledgeSaveRequests = [
        ...(page.__visualParityAdminKnowledgeSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/notice/fetch') {
      page.__visualParityAdminNoticeFetchCount =
        (page.__visualParityAdminNoticeFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/notice/save') {
      page.__visualParityLastAdminNoticeSave = requestData;
      page.__visualParityAdminNoticeSaveCount = (page.__visualParityAdminNoticeSaveCount ?? 0) + 1;
      page.__visualParityAdminNoticeSaveRequests = [
        ...(page.__visualParityAdminNoticeSaveRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/notice/show') {
      page.__visualParityLastAdminNoticeShow = requestData;
      page.__visualParityAdminNoticeShowCount = (page.__visualParityAdminNoticeShowCount ?? 0) + 1;
      page.__visualParityAdminNoticeShowRequests = [
        ...(page.__visualParityAdminNoticeShowRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/notice/drop') {
      page.__visualParityLastAdminNoticeDrop = requestData;
      page.__visualParityAdminNoticeDropCount = (page.__visualParityAdminNoticeDropCount ?? 0) + 1;
      page.__visualParityAdminNoticeDropRequests = [
        ...(page.__visualParityAdminNoticeDropRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/payment/fetch') {
      page.__visualParityAdminPaymentFetchCount =
        (page.__visualParityAdminPaymentFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/payment/save') {
      page.__visualParityLastAdminPaymentSave = requestData;
      page.__visualParityAdminPaymentSaveCount =
        (page.__visualParityAdminPaymentSaveCount ?? 0) + 1;
      page.__visualParityAdminPaymentSaveRequests = [
        ...(page.__visualParityAdminPaymentSaveRequests ?? []),
        requestData,
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
    if (adminEndpoint === '/user/fetch') {
      page.__visualParityAdminUserFetchCount = (page.__visualParityAdminUserFetchCount ?? 0) + 1;
      page.__visualParityLastAdminUserFetchQuery = Object.fromEntries(
        requestUrl.searchParams.entries(),
      );
    }
    if (adminEndpoint === '/user/update') {
      page.__visualParityLastAdminUserUpdate = requestData;
      page.__visualParityAdminUserUpdateCount = (page.__visualParityAdminUserUpdateCount ?? 0) + 1;
      page.__visualParityAdminUserUpdateRequests = [
        ...(page.__visualParityAdminUserUpdateRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/generate') {
      page.__visualParityLastAdminUserGenerate = requestData;
      page.__visualParityAdminUserGenerateCount =
        (page.__visualParityAdminUserGenerateCount ?? 0) + 1;
      page.__visualParityAdminUserGenerateRequests = [
        ...(page.__visualParityAdminUserGenerateRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/delUser') {
      page.__visualParityLastAdminUserDelete = requestData;
      page.__visualParityAdminUserDeleteCount = (page.__visualParityAdminUserDeleteCount ?? 0) + 1;
      page.__visualParityAdminUserDeleteRequests = [
        ...(page.__visualParityAdminUserDeleteRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/ban') {
      page.__visualParityLastAdminUserBan = requestData;
      page.__visualParityAdminUserBanCount = (page.__visualParityAdminUserBanCount ?? 0) + 1;
      page.__visualParityAdminUserBanRequests = [
        ...(page.__visualParityAdminUserBanRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/allDel') {
      page.__visualParityLastAdminUserAllDelete = requestData;
      page.__visualParityAdminUserAllDeleteCount =
        (page.__visualParityAdminUserAllDeleteCount ?? 0) + 1;
      page.__visualParityAdminUserAllDeleteRequests = [
        ...(page.__visualParityAdminUserAllDeleteRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/dumpCSV') {
      page.__visualParityLastAdminUserDumpCsv = requestData;
      page.__visualParityAdminUserDumpCsvCount =
        (page.__visualParityAdminUserDumpCsvCount ?? 0) + 1;
      page.__visualParityAdminUserDumpCsvRequests = [
        ...(page.__visualParityAdminUserDumpCsvRequests ?? []),
        requestData,
      ];
    }
    if (adminEndpoint === '/user/sendMail') {
      page.__visualParityLastAdminUserSendMail = requestData;
      page.__visualParityAdminUserSendMailCount =
        (page.__visualParityAdminUserSendMailCount ?? 0) + 1;
      page.__visualParityAdminUserSendMailRequests = [
        ...(page.__visualParityAdminUserSendMailRequests ?? []),
        requestData,
      ];
    }
    if (
      adminEndpoint === '/user/fetch' &&
      Array.from(requestUrl.searchParams.values()).includes('invite_user_id')
    ) {
      page.__visualParityLastAdminFilteredUserFetchQuery = Object.fromEntries(
        requestUrl.searchParams.entries(),
      );
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

    if (pathname === '/api/v1/user/redeemgiftcard' && interaction.redeemGiftcardTimeout) {
      await route.abort('timedout');
      return;
    }
    const shouldTimeout =
      (scenario.userPlansTimeout && pathname === '/api/v1/user/plan/fetch') ||
      (scenario.userOrdersTimeout && pathname === '/api/v1/user/order/fetch') ||
      (scenario.userServersTimeout && pathname === '/api/v1/user/server/fetch') ||
      (scenario.userTrafficTimeout && pathname === '/api/v1/user/stat/getTrafficLog') ||
      (scenario.userTicketsTimeout && pathname === '/api/v1/user/ticket/fetch') ||
      (scenario.userKnowledgeTimeout && pathname === '/api/v1/user/knowledge/fetch') ||
      (scenario.adminPlansTimeout && adminEndpoint === '/plan/fetch') ||
      (scenario.adminOrdersTimeout && adminEndpoint === '/order/fetch') ||
      (scenario.adminUsersTimeout && adminEndpoint === '/user/fetch') ||
      (scenario.adminTicketsTimeout && adminEndpoint === '/ticket/fetch') ||
      (scenario.adminServerManageTimeout && adminEndpoint === '/server/manage/getNodes') ||
      (scenario.adminPaymentsTimeout && adminEndpoint === '/payment/fetch') ||
      (scenario.adminCouponsTimeout && adminEndpoint === '/coupon/fetch') ||
      (scenario.adminGiftcardsTimeout && adminEndpoint === '/giftcard/fetch') ||
      (scenario.adminNoticesTimeout && adminEndpoint === '/notice/fetch') ||
      (scenario.adminKnowledgeTimeout && adminEndpoint === '/knowledge/fetch');
    if (shouldTimeout) {
      await route.abort('timedout');
      return;
    }
    if (pathname === '/api/v1/user/order/checkout' && interaction.orderCheckoutNetworkError) {
      await route.abort('failed');
      return;
    }

    if (pathname === '/api/v1/user/update' && interaction.delayUserUpdateMs) {
      await delay(interaction.delayUserUpdateMs);
    }
    if (pathname === '/api/v1/user/redeemgiftcard' && interaction.delayUserRedeemGiftcardMs) {
      await delay(interaction.delayUserRedeemGiftcardMs);
    }
    if (pathname === '/api/v1/user/changePassword' && interaction.delayUserChangePasswordMs) {
      await delay(interaction.delayUserChangePasswordMs);
    }
    if (pathname === '/api/v1/user/transfer' && interaction.delayUserTransferMs) {
      await delay(interaction.delayUserTransferMs);
    }
    if (pathname === '/api/v1/user/newPeriod' && interaction.delayUserNewPeriodMs) {
      await delay(interaction.delayUserNewPeriodMs);
    }
    if (pathname === '/api/v1/user/order/checkout' && interaction.delayUserOrderCheckoutMs) {
      await delay(interaction.delayUserOrderCheckoutMs);
    }
    if (pathname === '/api/v1/user/ticket/reply' && interaction.delayUserTicketReplyMs) {
      await delay(interaction.delayUserTicketReplyMs);
    }
    if (pathname === '/api/v1/user/ticket/close' && interaction.delayUserTicketCloseMs) {
      await delay(interaction.delayUserTicketCloseMs);
    }
    if (pathname === '/api/v1/user/ticket/save' && interaction.delayUserTicketSaveMs) {
      await delay(interaction.delayUserTicketSaveMs);
    }
    if (pathname === '/api/v1/user/ticket/withdraw' && interaction.delayUserWithdrawMs) {
      await delay(interaction.delayUserWithdrawMs);
    }
    if (adminEndpoint === '/ticket/reply' && interaction.delayAdminTicketReplyMs) {
      await delay(interaction.delayAdminTicketReplyMs);
    }
    if (adminEndpoint === '/payment/save' && interaction.delayAdminPaymentSaveMs) {
      await delay(interaction.delayAdminPaymentSaveMs);
    }
    if (adminEndpoint === '/coupon/generate' && interaction.delayAdminCouponGenerateMs) {
      await delay(interaction.delayAdminCouponGenerateMs);
    }
    if (adminEndpoint === '/giftcard/generate' && interaction.delayAdminGiftcardGenerateMs) {
      await delay(interaction.delayAdminGiftcardGenerateMs);
    }
    if (adminEndpoint === '/knowledge/save' && interaction.delayAdminKnowledgeSaveMs) {
      await delay(interaction.delayAdminKnowledgeSaveMs);
    }
    if (adminEndpoint === '/notice/save' && interaction.delayAdminNoticeSaveMs) {
      await delay(interaction.delayAdminNoticeSaveMs);
    }
    if (adminEndpoint === '/plan/save' && interaction.delayAdminPlanSaveMs) {
      await delay(interaction.delayAdminPlanSaveMs);
    }
    if (
      [
        '/notice/drop',
        '/notice/show',
        '/plan/drop',
        '/plan/update',
        '/server/manage/sort',
      ].includes(adminEndpoint ?? '') &&
      interaction.delayAdminMutationMs
    ) {
      await delay(interaction.delayAdminMutationMs);
    }
    if (adminEndpoint === '/server/group/save' && interaction.delayAdminServerGroupSaveMs) {
      await delay(interaction.delayAdminServerGroupSaveMs);
    }
    if (adminEndpoint === '/config/save' && interaction.delayAdminConfigSaveMs) {
      await delay(interaction.delayAdminConfigSaveMs);
    }
    if (
      ['/user/update', '/user/delUser', '/user/ban', '/user/allDel'].includes(
        adminEndpoint ?? '',
      ) &&
      interaction.delayAdminUserMutationMs
    ) {
      await delay(interaction.delayAdminUserMutationMs);
    }
    if (adminEndpoint === '/user/sendMail' && interaction.delayAdminUserSendMailMs) {
      await delay(interaction.delayAdminUserSendMailMs);
    }
    if (pathname === '/api/v1/user/unbindTelegram' && interaction.delayUserUnbindTelegramMs) {
      await delay(interaction.delayUserUnbindTelegramMs);
    }
    await fulfillApiResponse(
      route,
      apiFixtureResponse(requestUrl, isAdminScenario, scenario, requestData, interaction),
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

  if (
    (scenario.forceUserUnauthorized || interaction.forceUserUnauthorized) &&
    pathname === '/api/v1/user/info'
  ) {
    return httpError(
      'auth required',
      interaction.forceUserUnauthorizedStatus ?? scenario.forceUserUnauthorizedStatus ?? 403,
    );
  }

  if ((scenario.forceAdminUnauthorized || interaction.forceAdminUnauthorized) && adminEndpoint) {
    return httpError(
      'auth required',
      interaction.forceAdminUnauthorizedStatus ?? scenario.forceAdminUnauthorizedStatus ?? 403,
    );
  }

  if (adminEndpoint) {
    if (scenario.adminOrdersHttpError && adminEndpoint === '/order/fetch') {
      return httpError('Server Error', 500);
    }
    if (scenario.adminUsersHttpError && adminEndpoint === '/user/fetch') {
      return httpError('Server Error', 500);
    }
    if (
      /^\/server\/(shadowsocks|vmess|trojan|vless|hysteria|tuic|anytls|v2node)\/save$/.test(
        adminEndpoint,
      )
    ) {
      if (interaction?.adminServerNodeSaveError) return error('节点保存失败');
      return body(true);
    }

    switch (adminEndpoint) {
      case '/config/fetch':
        return body(adminConfigFixture);
      case '/config/save':
        if (interaction?.adminConfigSaveError) return error('配置保存失败');
        return body(true);
      case '/config/getEmailTemplate':
        return body(adminEmailTemplateFixtures);
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

  switch (pathname) {
    case '/api/v1/guest/comm/config':
      return body(guestConfigFixture);
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
    case '/api/v1/user/info':
      return body(
        interaction?.telegramBoundProfile
          ? { ...userInfoFixture, telegram_id: 12345 }
          : scenario.bannedUser
            ? bannedUserInfoFixture
            : userInfoFixture,
      );
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
    case '/api/v1/user/getActiveSession':
      return body({
        'visual-parity-session': {
          auth_data: '',
          current: true,
          ip: '203.0.113.10',
          login_at: 1_700_000_000,
          ua: 'Visual Parity Browser',
        },
      });
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
        requestUrl.searchParams.get('trade_no') === dashboardResetPackageTradeNo
          ? dashboardResetPackageOrderFixture
          : requestUrl.searchParams.get('trade_no') === profileDepositTradeNo
            ? profileDepositOrderFixture
            : (userOrderFixturesFor(scenario).find(
                (order) => order.trade_no === requestUrl.searchParams.get('trade_no'),
              ) ??
              orderFixtures.find(
                (order) => order.trade_no === requestUrl.searchParams.get('trade_no'),
              ) ??
              orderFixtures[0]),
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
        return body(interaction.checkoutRedirectUrl ?? '/#/order/VISUAL2026110001?cashier=visual', {
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
// shape (both worlds emit legacy until family waves flip the source world).
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

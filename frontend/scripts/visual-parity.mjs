import { createReadStream } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve, sep } from 'node:path';
import { deflateSync, inflateSync } from 'node:zlib';
import { chromium } from 'playwright';

// Oracle-only parity harness.
// This script reads packaged assets only from Docker /tmp so restored source can
// be screenshot-tested against the old bundle. Do not import this path from app
// code, Vite config, deploy scripts, or Laravel runtime.
const sourceBaseUrl = new URL(
  process.env.VISUAL_PARITY_SOURCE_BASE_URL ?? 'http://laravel:8000',
);
const adminPath = stripSlashes(process.env.VISUAL_PARITY_ADMIN_PATH ?? 'admin');
const oracleRoot = resolve(process.env.VISUAL_PARITY_ORACLE_ROOT ?? '/tmp/v2board-legacy-oracle');
const oraclePublicRoot = resolve(oracleRoot, 'public');
const artifactDir = resolve(process.env.VISUAL_PARITY_ARTIFACT_DIR ?? '/tmp/v2board-visual-parity');
const serveOnly = process.env.VISUAL_PARITY_SERVE_ONLY === '1';
const oracleHost = process.env.VISUAL_PARITY_ORACLE_HOST ?? '127.0.0.1';
const publicOracleHost = process.env.VISUAL_PARITY_PUBLIC_ORACLE_HOST ?? oracleHost;
const oraclePort = Number(process.env.VISUAL_PARITY_ORACLE_PORT ?? '0');
const maxDiffRatio = Number(process.env.VISUAL_PARITY_MAX_DIFF_RATIO ?? '0.01');
const maxAverageDelta = Number(process.env.VISUAL_PARITY_MAX_AVERAGE_DELTA ?? '2');
const channelThreshold = Number(process.env.VISUAL_PARITY_CHANNEL_THRESHOLD ?? '24');
const parityMode = process.env.VISUAL_PARITY_MODE ?? 'screenshots';
const scenarioFilter = process.env.VISUAL_PARITY_FILTER ?? '';
const viewportFilter = process.env.VISUAL_PARITY_VIEWPORT_FILTER ?? '';
const browserMode = process.env.VISUAL_PARITY_FRESH_BROWSER ?? 'auto';
const navigationAttempts = Number(process.env.VISUAL_PARITY_NAVIGATION_ATTEMPTS ?? '3');
const navigationTimeout = Number(process.env.VISUAL_PARITY_NAVIGATION_TIMEOUT ?? '45000');
const fontWaitTimeout = Number(process.env.VISUAL_PARITY_FONT_WAIT_TIMEOUT ?? '5000');
const LEGACY_GB_BYTES = 1_073_741_824;
const crc32Table = Array.from({ length: 256 }, (_, value) => {
  let current = value;
  for (let index = 0; index < 8; index += 1) {
    current = current & 1 ? 0xedb88320 ^ (current >>> 1) : current >>> 1;
  }
  return current >>> 0;
});

const scenarios = [
  { label: 'user-home-root', path: '/#/', readySelector: '.v2board-auth-box' },
  { label: 'user-login', path: '/#/login' },
  { label: 'user-register-rich', path: '/#/register?code=INVITE2026' },
  { label: 'user-forget', path: '/#/forgetpassword' },
  {
    authenticated: true,
    label: 'user-dashboard',
    path: '/#/dashboard',
    readySelector: '.v2board-shortcuts-item',
  },
  {
    authenticated: true,
    darkMode: true,
    label: 'user-dashboard-dark',
    path: '/#/dashboard',
    readySelector: '.v2board-shortcuts-item',
  },
  {
    authenticated: true,
    label: 'user-plans',
    path: '/#/plan',
    readySelector: '.block-link-pop',
  },
  {
    authenticated: true,
    label: 'user-plan-checkout',
    path: '/#/plan/1',
    readySelector: '#cashier',
  },
  {
    authenticated: true,
    label: 'user-orders',
    path: '/#/order',
    readySelector: '.ant-table-tbody tr',
  },
  {
    authenticated: true,
    label: 'user-order-detail',
    path: '/#/order/VISUAL2026110001',
    readySelector: '.v2board-order-info',
  },
  {
    authenticated: true,
    label: 'user-node',
    path: '/#/node',
    readySelector: '.ant-table-tbody tr',
  },
  {
    authenticated: true,
    label: 'user-traffic',
    path: '/#/traffic',
    readySelector: '.ant-table-tbody tr',
  },
  {
    authenticated: true,
    label: 'user-invite',
    path: '/#/invite',
    readySelector: '.ant-pagination',
  },
  {
    authenticated: true,
    label: 'user-tickets',
    path: '/#/ticket',
    readySelector: '.ant-table-fixed-right',
  },
  {
    authenticated: true,
    label: 'user-ticket-detail',
    path: '/#/ticket/7',
    readySelector: '.js-chat-input',
  },
  {
    authenticated: true,
    label: 'user-knowledge',
    path: '/#/knowledge',
    readySelector: '.list-group-item',
  },
  {
    authenticated: true,
    label: 'user-profile',
    path: '/#/profile',
    readySelector: '.ant-switch',
  },
  {
    authenticated: true,
    label: 'admin-dashboard',
    path: `/${adminPath}#/dashboard`,
    postReadyDelay: 800,
    readySelector: '.alert.alert-danger',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    darkMode: true,
    label: 'admin-dashboard-dark',
    path: `/${adminPath}#/dashboard`,
    postReadyDelay: 800,
    readySelector: '.alert.alert-danger',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-plans',
    path: `/${adminPath}#/plan`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-orders',
    path: `/${adminPath}#/order`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-users',
    path: `/${adminPath}#/user`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-tickets',
    path: `/${adminPath}#/ticket`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-ticket-detail',
    path: `/${adminPath}#/ticket/7`,
    readySelector: '.js-chat-input',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/ticket`,
  },
  {
    authenticated: true,
    label: 'admin-config',
    path: `/${adminPath}#/config/system`,
    readySelector: '.ant-tabs-tab',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-theme',
    path: `/${adminPath}#/config/theme`,
    readySelector: '.block-transparent.bg-image',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-system',
    path: `/${adminPath}#/queue`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-server-groups',
    path: `/${adminPath}#/server/group`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-server-manage',
    path: `/${adminPath}#/server/manage`,
    postReadyDelay: 300,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/server/group`,
  },
  {
    authenticated: true,
    label: 'admin-server-routes',
    path: `/${adminPath}#/server/route`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-payments',
    path: `/${adminPath}#/config/payment`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-coupons',
    path: `/${adminPath}#/coupon`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-giftcards',
    path: `/${adminPath}#/giftcard`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-notices',
    path: `/${adminPath}#/notice`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  {
    authenticated: true,
    label: 'admin-knowledge',
    path: `/${adminPath}#/knowledge`,
    readySelector: '.ant-table-tbody tr',
    seedLegacyAdminStore: true,
    warmupPath: `/${adminPath}#/login`,
  },
  { label: 'admin-root', path: `/${adminPath}#/`, readySelector: '.v2board-auth-box' },
  { label: 'admin-login', path: `/${adminPath}#/login` },
];
const interactionScenarios = [
  {
    label: 'user-login-form-language',
    run: runLoginFormLanguageInteraction,
    scenarioLabel: 'user-login',
  },
  {
    label: 'user-login-language-persistence',
    run: runLoginLanguagePersistenceInteraction,
    scenarioLabel: 'user-login',
  },
  {
    label: 'user-dashboard-header-language-dropdown',
    run: runDashboardHeaderLanguageDropdownInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-avatar-dropdown',
    run: runUserDashboardAvatarDropdownInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-dark-mode-persistence',
    preserveRuntimeDarkMode: true,
    run: runDarkModePersistenceInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-subscribe-drawer',
    run: runDashboardSubscribeDrawerInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-subscribe-import-links',
    run: runDashboardSubscribeImportLinksInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-notice-carousel',
    run: runDashboardNoticeCarouselInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-reset-package-confirm',
    run: runDashboardResetPackageConfirmInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    delayUserNewPeriodMs: 200,
    label: 'user-dashboard-new-period-confirm',
    newPeriodSubscribe: true,
    run: runDashboardNewPeriodConfirmInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-dashboard-alert-links',
    run: runDashboardAlertLinksInteraction,
    scenarioLabel: 'user-dashboard',
  },
  {
    label: 'user-profile-deposit-modal',
    run: runProfileDepositModalInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    label: 'user-profile-reset-subscribe-confirm',
    run: runProfileResetSubscribeConfirmInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    enableTelegramProfile: true,
    label: 'user-profile-telegram-bind-modal',
    run: runProfileTelegramBindModalInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    delayUserUnbindTelegramMs: 200,
    enableTelegramProfile: true,
    label: 'user-profile-telegram-unbind-confirm',
    run: runProfileTelegramUnbindConfirmInteraction,
    scenarioLabel: 'user-profile',
    telegramBoundProfile: true,
  },
  {
    delayUserUpdateMs: 200,
    label: 'user-profile-preference-switches',
    run: runProfilePreferenceSwitchesInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    delayUserRedeemGiftcardMs: 200,
    label: 'user-profile-redeem-giftcard',
    run: runProfileRedeemGiftcardInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    delayUserChangePasswordMs: 200,
    label: 'user-profile-change-password-success',
    run: runProfileChangePasswordSuccessInteraction,
    scenarioLabel: 'user-profile',
  },
  {
    label: 'user-plans-filter-tabs',
    run: runPlansFilterTabsInteraction,
    scenarioLabel: 'user-plans',
  },
  {
    label: 'user-plan-checkout-coupon',
    run: runPlanCheckoutCouponInteraction,
    scenarioLabel: 'user-plan-checkout',
  },
  {
    label: 'user-order-payment-method',
    run: runOrderPaymentMethodInteraction,
    scenarioLabel: 'user-order-detail',
  },
  {
    delayUserOrderCheckoutMs: 200,
    label: 'user-order-qr-checkout',
    run: runOrderQrCheckoutInteraction,
    scenarioLabel: 'user-order-detail',
  },
  {
    label: 'user-order-stripe-disabled-checkout',
    run: runOrderStripeDisabledCheckoutInteraction,
    scenarioLabel: 'user-order-detail',
  },
  {
    checkoutRedirectUrl: '/#/order/VISUAL2026110001?cashier=visual',
    delayUserOrderCheckoutMs: 200,
    label: 'user-order-redirect-checkout',
    run: runOrderRedirectCheckoutInteraction,
    scenarioLabel: 'user-order-detail',
  },
  {
    label: 'user-node-table-scroll',
    run: runNodeTableScrollInteraction,
    scenarioLabel: 'user-node',
  },
  {
    label: 'user-node-tooltips',
    run: runUserNodeTooltipsInteraction,
    scenarioLabel: 'user-node',
  },
  {
    label: 'user-traffic-table-scroll',
    run: runTrafficTableScrollInteraction,
    scenarioLabel: 'user-traffic',
  },
  {
    label: 'user-traffic-total-tooltip',
    run: runUserTrafficTotalTooltipInteraction,
    scenarioLabel: 'user-traffic',
  },
  {
    label: 'user-knowledge-drawer',
    run: runKnowledgeDrawerInteraction,
    scenarioLabel: 'user-knowledge',
  },
  {
    label: 'user-invite-generate',
    run: runInviteGenerateInteraction,
    scenarioLabel: 'user-invite',
  },
  {
    delayUserTransferMs: 200,
    label: 'user-invite-transfer-modal',
    run: runInviteTransferModalInteraction,
    scenarioLabel: 'user-invite',
  },
  {
    delayUserWithdrawMs: 200,
    label: 'user-invite-withdraw-modal',
    run: runInviteWithdrawModalInteraction,
    scenarioLabel: 'user-invite',
  },
  {
    label: 'user-invite-tooltips',
    run: runUserInviteTooltipsInteraction,
    scenarioLabel: 'user-invite',
  },
  {
    delayUserTicketReplyMs: 200,
    label: 'user-ticket-reply-send',
    run: runUserTicketReplySendInteraction,
    scenarioLabel: 'user-ticket-detail',
  },
  {
    delayUserTicketSaveMs: 200,
    label: 'user-ticket-create-submit',
    run: runUserTicketCreateModalInteraction,
    scenarioLabel: 'user-tickets',
  },
  {
    delayAdminTicketReplyMs: 200,
    label: 'admin-ticket-reply-send',
    run: runAdminTicketReplySendInteraction,
    scenarioLabel: 'admin-ticket-detail',
  },
  {
    label: 'admin-tickets-reply-filter',
    run: runAdminTicketsReplyFilterInteraction,
    scenarioLabel: 'admin-tickets',
  },
  {
    label: 'admin-dashboard-dark-mode-persistence',
    preserveRuntimeDarkMode: true,
    run: runDarkModePersistenceInteraction,
    scenarioLabel: 'admin-dashboard',
  },
  {
    label: 'admin-dashboard-avatar-dropdown',
    run: runAdminDashboardAvatarDropdownInteraction,
    scenarioLabel: 'admin-dashboard',
  },
  {
    label: 'admin-dashboard-commission-shortcut',
    run: runAdminDashboardCommissionShortcutInteraction,
    scenarioLabel: 'admin-dashboard',
  },
  {
    label: 'user-order-cancel-confirm',
    run: runOrderCancelConfirmInteraction,
    scenarioLabel: 'user-orders',
  },
  {
    delayAdminPlanSaveMs: 200,
    label: 'admin-plan-create-drawer',
    run: runAdminPlanCreateDrawerInteraction,
    scenarioLabel: 'admin-plans',
  },
  {
    label: 'admin-plan-create-group-select-dropdown',
    run: runAdminPlanCreateGroupSelectDropdownInteraction,
    scenarioLabel: 'admin-plans',
  },
  {
    delayAdminPlanSaveMs: 200,
    label: 'admin-plan-edit-drawer',
    run: runAdminPlanEditDrawerInteraction,
    scenarioLabel: 'admin-plans',
  },
  {
    label: 'admin-plan-renew-tooltip',
    run: runAdminPlanRenewTooltipInteraction,
    scenarioLabel: 'admin-plans',
  },
  {
    label: 'admin-config-tabs',
    run: runAdminConfigTabsInteraction,
    scenarioLabel: 'admin-config',
  },
  {
    label: 'admin-theme-settings-modal',
    run: runAdminThemeSettingsInteraction,
    scenarioLabel: 'admin-theme',
  },
  {
    label: 'admin-server-create-node-drawer',
    run: runAdminServerCreateNodeDrawerInteraction,
    scenarioLabel: 'admin-server-manage',
  },
  {
    label: 'admin-server-edit-node-drawer',
    run: runAdminServerEditNodeDrawerInteraction,
    scenarioLabel: 'admin-server-manage',
  },
  {
    label: 'admin-server-route-create-modal',
    run: runAdminServerRouteCreateModalInteraction,
    scenarioLabel: 'admin-server-routes',
  },
  {
    label: 'admin-server-route-edit-modal',
    run: runAdminServerRouteEditModalInteraction,
    scenarioLabel: 'admin-server-routes',
  },
  {
    delayAdminServerGroupSaveMs: 200,
    label: 'admin-server-group-create-modal',
    run: runAdminServerGroupCreateModalInteraction,
    scenarioLabel: 'admin-server-groups',
  },
  {
    delayAdminServerGroupSaveMs: 200,
    label: 'admin-server-group-edit-modal',
    run: runAdminServerGroupEditModalInteraction,
    scenarioLabel: 'admin-server-groups',
  },
  {
    delayAdminPaymentSaveMs: 200,
    label: 'admin-payment-create-modal',
    run: runAdminPaymentCreateModalInteraction,
    scenarioLabel: 'admin-payments',
  },
  {
    delayAdminPaymentSaveMs: 200,
    label: 'admin-payment-edit-modal',
    run: runAdminPaymentEditModalInteraction,
    scenarioLabel: 'admin-payments',
  },
  {
    label: 'admin-payment-notify-tooltip',
    run: runAdminPaymentNotifyTooltipInteraction,
    scenarioLabel: 'admin-payments',
  },
  {
    label: 'admin-order-detail-modal',
    run: runAdminOrderDetailModalInteraction,
    scenarioLabel: 'admin-orders',
  },
  {
    label: 'admin-order-status-tooltips',
    run: runAdminOrderStatusTooltipsInteraction,
    scenarioLabel: 'admin-orders',
  },
  {
    label: 'admin-order-assign-modal',
    run: runAdminOrderAssignModalInteraction,
    scenarioLabel: 'admin-orders',
  },
  {
    label: 'admin-order-status-dropdown',
    run: runAdminOrderStatusDropdownInteraction,
    scenarioLabel: 'admin-orders',
  },
  {
    label: 'admin-order-commission-dropdown',
    run: runAdminOrderCommissionDropdownInteraction,
    scenarioLabel: 'admin-orders',
  },
  {
    delayAdminCouponGenerateMs: 200,
    label: 'admin-coupon-create-modal',
    run: runAdminCouponCreateModalInteraction,
    scenarioLabel: 'admin-coupons',
  },
  {
    label: 'admin-coupon-range-picker',
    run: runAdminCouponRangePickerInteraction,
    scenarioLabel: 'admin-coupons',
  },
  {
    delayAdminCouponGenerateMs: 200,
    label: 'admin-coupon-edit-modal',
    run: runAdminCouponEditModalInteraction,
    scenarioLabel: 'admin-coupons',
  },
  {
    delayAdminGiftcardGenerateMs: 200,
    label: 'admin-giftcard-create-modal',
    run: runAdminGiftcardCreateModalInteraction,
    scenarioLabel: 'admin-giftcards',
  },
  {
    delayAdminGiftcardGenerateMs: 200,
    label: 'admin-giftcard-edit-modal',
    run: runAdminGiftcardEditModalInteraction,
    scenarioLabel: 'admin-giftcards',
  },
  {
    delayAdminNoticeSaveMs: 200,
    label: 'admin-notice-create-modal',
    run: runAdminNoticeCreateModalInteraction,
    scenarioLabel: 'admin-notices',
  },
  {
    delayAdminNoticeSaveMs: 200,
    label: 'admin-notice-edit-modal',
    run: runAdminNoticeEditModalInteraction,
    scenarioLabel: 'admin-notices',
  },
  {
    delayAdminKnowledgeSaveMs: 200,
    label: 'admin-knowledge-create-drawer',
    run: runAdminKnowledgeCreateDrawerInteraction,
    scenarioLabel: 'admin-knowledge',
  },
  {
    delayAdminKnowledgeSaveMs: 200,
    label: 'admin-knowledge-edit-drawer',
    run: runAdminKnowledgeEditDrawerInteraction,
    scenarioLabel: 'admin-knowledge',
  },
  {
    label: 'admin-users-filter-input',
    run: runAdminUsersFilterInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-users-filter-field-select-dropdown',
    run: runAdminUsersFilterFieldSelectDropdownInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-users-filter-expiry-picker',
    run: runAdminUsersFilterExpiryPickerInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-bulk-ban-confirm',
    run: runAdminUserBulkBanConfirmInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-bulk-delete-confirm',
    run: runAdminUserBulkDeleteConfirmInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-create-modal',
    run: runAdminUserCreateModalInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-create-plan-select-dropdown',
    run: runAdminUserCreatePlanSelectDropdownInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-create-expiry-picker',
    run: runAdminUserCreateExpiryPickerInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-send-mail-modal',
    run: runAdminUserSendMailModalInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-reset-secret-confirm',
    run: runAdminUserResetSecretConfirmInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-delete-confirm',
    run: runAdminUserDeleteConfirmInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-copy-action',
    run: runAdminUserCopyActionInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-edit-action',
    run: runAdminUserEditActionInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-assign-action',
    run: runAdminUserAssignActionInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-orders-action',
    run: runAdminUserOrdersActionInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-invite-action',
    run: runAdminUserInviteActionInteraction,
    scenarioLabel: 'admin-users',
  },
  {
    label: 'admin-user-traffic-action',
    run: runAdminUserTrafficActionInteraction,
    scenarioLabel: 'admin-users',
  },
];
const guestConfigFixture = {
  app_description: null,
  app_url: null,
  email_whitelist_suffix: ['example.com', 'v2board.test'],
  is_email_verify: 1,
  is_invite_force: 1,
  is_recaptcha: 0,
  logo: null,
  recaptcha_site_key: null,
  tos_url: 'https://example.com/tos',
};
const userInfoFixture = {
  auto_renewal: 0,
  avatar_url: '',
  balance: 12345,
  banned: 0,
  commission_balance: 0,
  commission_rate: null,
  created_at: 1_700_000_000,
  device_limit: 5,
  discount: null,
  email: 'visual@example.com',
  expired_at: 4_102_488_000,
  last_login_at: 1_700_000_000,
  plan_id: 1,
  remind_expire: 1,
  remind_traffic: 1,
  telegram_id: null,
  transfer_enable: 1000 * 1024 * 1024 * 1024,
  uuid: 'visual-parity-user',
};
const subscribeFixture = {
  alive_ip: 2,
  allow_new_period: 0,
  d: 200 * 1024 * 1024 * 1024,
  device_limit: 5,
  email: 'visual@example.com',
  expired_at: 4_102_488_000,
  plan: {
    content: '',
    created_at: 1_700_000_000,
    group_id: 1,
    id: 1,
    month_price: 990,
    name: 'Pro',
    onetime_price: null,
    quarter_price: 2_490,
    renew: 1,
    reset_price: 100,
    show: 1,
    sort: 1,
    transfer_enable: 1000,
    updated_at: 1_700_000_000,
    year_price: 9_900,
  },
  plan_id: 1,
  reset_day: 5,
  subscribe_url: 'https://example.test/sub?token=visual',
  token: 'visual-token',
  transfer_enable: 1000 * 1024 * 1024 * 1024,
  u: 650 * 1024 * 1024 * 1024,
  uuid: 'visual-parity-user',
};
const newPeriodSubscribeFixture = {
  ...subscribeFixture,
  allow_new_period: 1,
  d: 450 * 1024 * 1024 * 1024,
  reset_day: 0,
  u: 550 * 1024 * 1024 * 1024,
};
const planFixtures = [
  {
    capacity_limit: null,
    content: '<p>Fast nodes</p><p>Support ticket</p>',
    count: 12,
    created_at: 1_700_000_000,
    device_limit: 5,
    group_id: 1,
    half_year_price: null,
    id: 1,
    month_price: 990,
    name: 'Pro',
    onetime_price: null,
    quarter_price: 2_490,
    renew: 1,
    reset_price: 100,
    reset_traffic_method: 0,
    show: 1,
    sort: 1,
    speed_limit: null,
    three_year_price: null,
    transfer_enable: 1000,
    two_year_price: null,
    updated_at: 1_700_000_000,
    year_price: 9_900,
  },
  {
    capacity_limit: 3,
    content: '<p>Monthly traffic package</p>',
    count: 2,
    created_at: 1_700_000_000,
    device_limit: null,
    group_id: 1,
    half_year_price: null,
    id: 2,
    month_price: null,
    name: 'Traffic Pack',
    onetime_price: 1_990,
    quarter_price: null,
    renew: 0,
    reset_price: null,
    reset_traffic_method: null,
    show: 1,
    sort: 2,
    speed_limit: null,
    three_year_price: null,
    transfer_enable: 500,
    two_year_price: null,
    updated_at: 1_700_000_000,
    year_price: null,
  },
];
const orderFixtures = [
  {
    balance_amount: null,
    callback_no: null,
    commission_balance: 0,
    commission_status: 0,
    coupon_id: null,
    created_at: 1_700_000_000,
    discount_amount: null,
    handling_amount: null,
    invite_user_id: null,
    paid_at: null,
    payment_id: null,
    period: 'month_price',
    plan: planFixtures[0],
    plan_id: 1,
    refund_amount: null,
    status: 0,
    surplus_amount: null,
    surplus_order_ids: null,
    total_amount: 990,
    trade_no: 'VISUAL2026110001',
    type: 1,
    updated_at: 1_700_000_000,
  },
  {
    balance_amount: null,
    callback_no: null,
    commission_balance: 0,
    commission_status: 0,
    coupon_id: null,
    created_at: 1_700_086_400,
    discount_amount: null,
    handling_amount: null,
    invite_user_id: null,
    paid_at: 1_700_090_000,
    payment_id: 1,
    period: 'onetime_price',
    plan: planFixtures[1],
    plan_id: 2,
    refund_amount: null,
    status: 3,
    surplus_amount: null,
    surplus_order_ids: null,
    total_amount: 1_990,
    trade_no: 'VISUAL2026110002',
    type: 1,
    updated_at: 1_700_090_000,
  },
];
const profileDepositTradeNo = 'VISUAL2026110098';
const profileDepositOrderFixture = {
  balance_amount: null,
  bounus: 0,
  callback_no: null,
  commission_balance: 0,
  commission_status: 0,
  coupon_id: null,
  created_at: 1_700_172_800,
  discount_amount: null,
  get_amount: 1_234,
  handling_amount: null,
  invite_user_id: null,
  paid_at: null,
  payment_id: null,
  period: 'deposit',
  plan: { id: 0, name: 'deposit' },
  plan_id: 0,
  refund_amount: null,
  status: 0,
  surplus_amount: null,
  surplus_order_ids: null,
  total_amount: 1_234,
  trade_no: profileDepositTradeNo,
  type: 9,
  updated_at: 1_700_172_800,
};
const dashboardResetPackageTradeNo = 'VISUAL2026110097';
const dashboardResetPackageOrderFixture = {
  balance_amount: null,
  callback_no: null,
  commission_balance: 0,
  commission_status: 0,
  coupon_id: null,
  created_at: 1_700_259_200,
  discount_amount: null,
  handling_amount: null,
  invite_user_id: null,
  paid_at: null,
  payment_id: null,
  period: 'reset_price',
  plan: planFixtures[0],
  plan_id: 1,
  refund_amount: null,
  status: 0,
  surplus_amount: null,
  surplus_order_ids: null,
  total_amount: 100,
  trade_no: dashboardResetPackageTradeNo,
  type: 1,
  updated_at: 1_700_259_200,
};
const paymentMethodFixtures = [
  {
    handling_fee_fixed: 0,
    handling_fee_percent: 0,
    icon: null,
    id: 1,
    name: 'Alipay',
    payment: 'AlipayF2F',
  },
  {
    handling_fee_fixed: 100,
    handling_fee_percent: 2.5,
    icon: null,
    id: 2,
    name: 'Stripe',
    payment: 'StripeCredit',
  },
  {
    handling_fee_fixed: 100,
    handling_fee_percent: 0,
    icon: null,
    id: 3,
    name: 'Fee Pay',
    payment: 'ManualPay',
  },
];
const couponCheckFixture = {
  code: 'SAVE10',
  created_at: 1_700_000_000,
  ended_at: 4_102_488_000,
  id: 1,
  limit_period: null,
  limit_plan_ids: [1],
  limit_use: null,
  limit_use_with_user: null,
  name: 'Visual Coupon',
  show: 1,
  started_at: 1_600_000_000,
  type: 2,
  updated_at: 1_700_000_000,
  value: 10,
};
const serverFixtures = [
  {
    cache_key: 'server-1',
    group_id: [1],
    host: 'node-a.example.test',
    id: 1,
    is_online: 1,
    last_check_at: 1_700_000_000,
    name: 'Hong Kong 01',
    parent_id: null,
    port: 443,
    rate: '1',
    route_id: null,
    tags: ['IEPL', 'Netflix'],
    type: 'shadowsocks',
  },
  {
    cache_key: 'server-2',
    group_id: [1],
    host: 'node-b.example.test',
    id: 2,
    is_online: 0,
    last_check_at: 1_700_000_000,
    name: 'Tokyo 02',
    parent_id: null,
    port: 443,
    rate: '2.5',
    route_id: null,
    tags: ['Relay'],
    type: 'trojan',
  },
];
const trafficFixtures = [
  {
    d: 1024 * 1024 * 1024,
    record_at: 1_705_320_000,
    server_rate: '1.5',
    u: 512 * 1024 * 1024,
    user_id: 1,
  },
  {
    d: 256 * 1024 * 1024,
    record_at: 1_705_406_400,
    server_rate: '0.5',
    u: 128 * 1024 * 1024,
    user_id: 1,
  },
];
const inviteFixture = {
  codes: [
    {
      code: 'INVITE2026',
      created_at: 1_700_000_000,
      id: 1,
      status: 0,
      updated_at: 1_700_000_000,
      user_id: 1,
    },
    {
      code: 'WELCOME',
      created_at: 1_700_086_400,
      id: 2,
      status: 0,
      updated_at: 1_700_086_400,
      user_id: 1,
    },
  ],
  stat: [7, 23_450, 6_780, 12],
};
const inviteDetailFixtures = [
  {
    created_at: 1_700_100_000,
    get_amount: 1_234,
    id: 1,
    invite_user_id: 2,
    order_amount: 9_900,
    order_id: 100,
    trade_no: 'VISUAL-COMMISSION-1',
    updated_at: 1_700_100_000,
    user_id: 1,
  },
  {
    created_at: 1_700_186_400,
    get_amount: 2_345,
    id: 2,
    invite_user_id: 3,
    order_amount: 19_900,
    order_id: 101,
    trade_no: 'VISUAL-COMMISSION-2',
    updated_at: 1_700_186_400,
    user_id: 1,
  },
];
const ticketFixtures = [
  {
    created_at: 1_700_000_000,
    id: 7,
    level: 1,
    message: [],
    reply_status: 1,
    status: 0,
    subject: 'Need help',
    updated_at: 1_700_000_060,
  },
  {
    created_at: 1_700_086_400,
    id: 8,
    level: 0,
    message: [],
    reply_status: 0,
    status: 0,
    subject: 'Waiting reply',
    updated_at: 1_700_086_460,
  },
  {
    created_at: 1_700_172_800,
    id: 9,
    level: 2,
    message: [],
    reply_status: 0,
    status: 1,
    subject: 'Closed ticket',
    updated_at: 1_700_172_860,
  },
];
const ticketDetailFixture = {
  ...ticketFixtures[0],
  message: [
    {
      created_at: 1_700_000_120,
      is_me: 0,
      message: 'Hello, how can we help?',
    },
    {
      created_at: 1_700_000_240,
      is_me: 1,
      message: 'I need help with my subscription.',
    },
    {
      created_at: 1_700_000_360,
      is_me: 0,
      message: 'We checked it and the subscription is active now.',
    },
  ],
};
const knowledgeFixtures = {
  General: [
    {
      body: '<p>Copy article body</p>',
      category: 'General',
      created_at: 1_700_000_000,
      id: 1,
      language: 'en-US',
      show: 1,
      sort: 1,
      title: 'Copy Article',
      updated_at: 1_700_000_000,
    },
  ],
  Router: [
    {
      body: '<p>Router guide body</p>',
      category: 'Router',
      created_at: 1_700_086_400,
      id: 2,
      language: 'en-US',
      show: 1,
      sort: 2,
      title: 'Router Guide',
      updated_at: 1_700_086_400,
    },
  ],
};
const adminKnowledgeFixtures = [
  knowledgeFixtures.General[0],
  {
    ...knowledgeFixtures.Router[0],
    show: 0,
  },
];
const noticeFixtures = [
  {
    content: '<p>Visual parity notice</p>',
    created_at: 1_700_000_000,
    id: 1,
    img_url: null,
    show: 1,
    tags: [],
    title: 'Notice A',
    updated_at: 1_700_000_000,
  },
  {
    content: '<p>Second notice</p>',
    created_at: 1_700_086_400,
    id: 2,
    img_url: null,
    show: 1,
    tags: [],
    title: 'Notice B',
    updated_at: 1_700_086_400,
  },
];
const adminNoticeFixtures = [
  noticeFixtures[0],
  {
    ...noticeFixtures[1],
    show: 0,
    tags: ['ops'],
    title: 'Hidden Notice',
  },
];
const userCommConfigFixture = {
  commission_distribution_enable: 0,
  commission_distribution_l1: null,
  commission_distribution_l2: null,
  commission_distribution_l3: null,
  currency: 'CNY',
  currency_symbol: '¥',
  is_telegram: 0,
  stripe_pk: null,
  telegram_discuss_link: null,
  withdraw_close: 0,
  withdraw_methods: ['Alipay', 'USDT'],
};
const adminConfigFixture = {
  site: {
    currency: 'CNY',
    currency_symbol: '¥',
  },
};
const adminEmailTemplateFixtures = ['default', 'classic'];
const adminThemeTemplateFixtures = {
  default: {
    name: 'Default',
  },
};
const adminThemeFixtures = {
  active: 'default',
  themes: {
    default: {
      configs: [
        {
          field_name: 'homepage',
          field_type: 'input',
          label: '首页标题',
          placeholder: '请输入首页标题',
        },
      ],
      description: '默认主题描述',
      name: '默认主题',
    },
    classic: {
      configs: [],
      description: '经典主题描述',
      name: '经典主题',
    },
  },
};
const adminStatFixture = {
  commission_last_month_payout: null,
  commission_month_payout: 0,
  commission_pending_total: 1,
  day_income: 1,
  day_register_total: 0,
  last_month_income: 0,
  month_income: 1,
  month_register_total: 0,
  online_user: 1,
  ticket_pending_total: 1,
};
const adminQueueStatsFixture = {
  failedJobs: 2,
  jobsPerMinute: 12,
  pausedMasters: 0,
  periods: { failedJobs: 2, recentJobs: 34 },
  processes: 4,
  queueWithMaxRuntime: 'traffic_fetch',
  queueWithMaxThroughput: 'order_handle',
  recentJobs: 34,
  status: true,
  wait: { order_handle: 1, traffic_fetch: 6 },
};
const adminQueueWorkloadFixtures = [
  { length: 0, name: 'default', processes: 1, wait: 0 },
  { length: 5, name: 'order_handle', processes: 4, wait: 6 },
  { length: 8, name: 'traffic_fetch', processes: 7, wait: 9 },
];
const adminOrderStatFixtures = [];
const adminServerRankFixtures = [];
const adminUserRankFixtures = [];
const adminPlanStoreFixtures = planFixtures.map((plan) => {
  const next = { ...plan };
  for (const key of [
    'month_price',
    'quarter_price',
    'half_year_price',
    'year_price',
    'two_year_price',
    'three_year_price',
    'onetime_price',
    'reset_price',
  ]) {
    next[key] = next[key] !== null ? next[key] / 100 : null;
  }
  return next;
});
const adminServerGroupFixtures = [
  {
    created_at: 1_700_000_000,
    id: 1,
    name: 'Default',
    server_count: 6,
    updated_at: 1_700_000_000,
    user_count: 12,
  },
];
const adminServerRouteFixtures = [
  {
    action: 'block',
    action_value: null,
    created_at: 1_700_000_000,
    id: 1,
    match: ['geosite:category-ads-all', 'domain:example.com'],
    remarks: 'Block ads',
    updated_at: 1_700_000_000,
  },
  {
    action: 'default_out',
    action_value: JSON.stringify({ protocol: 'freedom' }),
    created_at: 1_700_100_000,
    id: 2,
    match: [],
    remarks: 'Default outbound',
    updated_at: 1_700_100_000,
  },
];
const adminServerNodeFixtures = [
  {
    available_status: 2,
    group_id: ['1'],
    host: 'jp.example.com',
    id: 1,
    is_online: 1,
    last_check_at: 1_700_000_000,
    name: 'Tokyo 01',
    online: 8,
    parent_id: null,
    port: 443,
    rate: '1.0',
    route_id: [1],
    server_port: 8388,
    show: 1,
    type: 'shadowsocks',
  },
  {
    available_status: 1,
    group_id: ['1'],
    host: 'relay.example.com',
    id: 2,
    is_online: 0,
    last_check_at: null,
    name: 'Tokyo Relay',
    online: 0,
    parent_id: 1,
    port: 8443,
    rate: '0.8',
    route_id: [2],
    server_port: null,
    show: 0,
    type: 'vmess',
  },
];
const adminOrderFixtures = [
  {
    balance_amount: 0,
    callback_no: null,
    commission_balance: 0,
    commission_status: 0,
    coupon_id: null,
    created_at: 1_700_000_000,
    discount_amount: 0,
    handling_amount: null,
    id: 1,
    invite_user_id: null,
    paid_at: null,
    payment_id: null,
    period: 'month_price',
    plan_id: 1,
    plan_name: 'Pro',
    refund_amount: 0,
    status: 0,
    surplus_amount: 0,
    surplus_order_ids: null,
    total_amount: 990,
    trade_no: 'VISUAL2026110001',
    type: 1,
    updated_at: 1_700_000_000,
    user_id: 1,
  },
  {
    balance_amount: 0,
    callback_no: 'cb-visual-2',
    commission_balance: 120,
    commission_status: 1,
    coupon_id: null,
    created_at: 1_700_086_400,
    discount_amount: 0,
    handling_amount: null,
    id: 2,
    invite_user_id: 3,
    paid_at: 1_700_090_000,
    payment_id: 1,
    period: 'onetime_price',
    plan_id: 2,
    plan_name: 'Traffic Pack',
    refund_amount: 0,
    status: 3,
    surplus_amount: 0,
    surplus_order_ids: null,
    total_amount: 1_990,
    trade_no: 'VISUAL2026110002',
    type: 4,
    updated_at: 1_700_090_000,
    user_id: 2,
  },
];
const adminUserFixtures = [
  {
    alive_ip: 2,
    balance: 12_340,
    banned: 0,
    commission_balance: 1_230,
    commission_rate: null,
    created_at: 1_700_000_000,
    d: 2_147_483_648,
    device_limit: 3,
    discount: null,
    email: 'visual-user@example.com',
    expired_at: 1_893_456_000,
    group_id: 1,
    id: 1,
    invite_user_id: null,
    ips: '127.0.0.1',
    is_admin: 0,
    is_staff: 0,
    last_login_at: 1_700_000_000,
    password: 'secret',
    plan_id: 1,
    plan_name: 'Pro',
    subscribe_url: 'https://example.com/api/v1/client/subscribe?token=visual-user',
    telegram_id: null,
    token: 'visual-user-token',
    total_used: 3_221_225_472,
    transfer_enable: 107_374_182_400,
    u: 1_073_741_824,
    updated_at: 1_700_000_000,
    uuid: 'visual-user-uuid',
  },
  {
    alive_ip: 0,
    balance: 0,
    banned: 1,
    commission_balance: 0,
    commission_rate: null,
    created_at: 1_700_086_400,
    d: 0,
    device_limit: null,
    discount: null,
    email: 'expired@example.com',
    expired_at: 1_650_000_000,
    group_id: null,
    id: 2,
    invite_user_id: 1,
    ips: '',
    is_admin: 0,
    is_staff: 0,
    last_login_at: null,
    password: 'secret',
    plan_id: null,
    plan_name: null,
    subscribe_url: 'https://example.com/api/v1/client/subscribe?token=expired',
    telegram_id: null,
    token: 'expired-token',
    total_used: 0,
    transfer_enable: 0,
    u: 0,
    updated_at: 1_700_086_400,
    uuid: 'expired-user-uuid',
  },
];
const adminUserStoreFixtures = adminUserFixtures.map((user) => ({
  ...user,
  balance: legacyScaledFixed(user.balance, 100),
  commission_balance: legacyScaledFixed(user.commission_balance, 100),
  d: legacyScaledFixed(user.d, LEGACY_GB_BYTES),
  password: '',
  total_used: legacyScaledFixed(user.total_used, LEGACY_GB_BYTES),
  transfer_enable: legacyScaledFixed(user.transfer_enable, LEGACY_GB_BYTES),
  u: legacyScaledFixed(user.u, LEGACY_GB_BYTES),
}));
const adminTicketFixtures = [
  {
    created_at: 1_700_000_000,
    id: 7,
    last_reply_user_id: null,
    level: 2,
    message: [
      {
        created_at: 1_700_000_000,
        id: 1,
        is_me: false,
        message: 'Cannot connect after renewal.',
        ticket_id: 7,
        updated_at: 1_700_000_000,
        user_id: 1,
      },
    ],
    reply_status: 0,
    status: 0,
    subject: 'Connection issue',
    updated_at: 1_700_003_600,
    user_id: 1,
  },
  {
    created_at: 1_700_086_400,
    id: 8,
    last_reply_user_id: 1,
    level: 1,
    message: [
      {
        created_at: 1_700_086_400,
        id: 2,
        is_me: false,
        message: 'Need help changing plan.',
        ticket_id: 8,
        updated_at: 1_700_086_400,
        user_id: 2,
      },
      {
        created_at: 1_700_090_000,
        id: 3,
        is_me: true,
        message: 'Please choose the new plan in orders.',
        ticket_id: 8,
        updated_at: 1_700_090_000,
        user_id: 1,
      },
    ],
    reply_status: 1,
    status: 0,
    subject: 'Plan change',
    updated_at: 1_700_090_000,
    user_id: 2,
  },
];
const adminTicketDetailFixture = { ...adminTicketFixtures[0], user_id: null };
const adminPaymentFixtures = [
  {
    config: {
      mch_id: 'visual-merchant',
      key: 'visual-secret',
    },
    created_at: 1_700_000_000,
    enable: 1,
    handling_fee_fixed: null,
    handling_fee_percent: null,
    icon: null,
    id: 1,
    name: 'Alipay',
    notify_domain: null,
    notify_url: 'https://example.com/api/v1/guest/payment/notify/visual-alipay',
    payment: 'AlipayF2F',
    sort: 1,
    updated_at: 1_700_000_000,
    uuid: 'visual-alipay',
  },
  {
    config: {
      app_id: 'visual-stripe',
      secret_key: 'sk_test_visual',
    },
    created_at: 1_700_086_400,
    enable: 0,
    handling_fee_fixed: 100,
    handling_fee_percent: 2,
    icon: null,
    id: 2,
    name: 'Stripe',
    notify_domain: null,
    notify_url: 'https://example.com/api/v1/guest/payment/notify/visual-stripe',
    payment: 'StripeCheckout',
    sort: 2,
    updated_at: 1_700_086_400,
    uuid: 'visual-stripe',
  },
];
const adminPaymentMethodsFixture = ['AlipayF2F', 'StripeCheckout', 'MGate'];
const adminPaymentFormFixtures = {
  AlipayF2F: {
    key: {
      description: '请输入支付宝当面付密钥',
      label: '密钥',
      type: 'input',
      value: 'visual-secret-default',
    },
    mch_id: {
      description: '请输入支付宝商户ID',
      label: '商户ID',
      type: 'input',
      value: 'visual-merchant-default',
    },
  },
  MGate: {
    token: {
      description: '请输入 MGate Token',
      label: 'Token',
      type: 'input',
      value: 'visual-mgate-token',
    },
  },
  StripeCheckout: {
    publishable_key: {
      description: '请输入 Stripe Publishable Key',
      label: 'Publishable Key',
      type: 'input',
      value: 'pk_test_visual',
    },
    secret_key: {
      description: '请输入 Stripe Secret Key',
      label: 'Secret Key',
      type: 'input',
      value: 'sk_test_visual_default',
    },
  },
};
const adminCouponFixtures = [
  {
    code: 'VISUAL100',
    created_at: 1_700_000_000,
    ended_at: 1_893_456_000,
    id: 1,
    limit_period: ['month_price', 'year_price'],
    limit_plan_ids: [1],
    limit_use: 50,
    limit_use_with_user: 1,
    name: 'Visual Amount',
    show: 1,
    started_at: 1_700_000_000,
    type: 1,
    updated_at: 1_700_000_000,
    value: 1_000,
  },
  {
    code: 'VISUAL20',
    created_at: 1_700_086_400,
    ended_at: 1_893_456_000,
    id: 2,
    limit_period: null,
    limit_plan_ids: null,
    limit_use: null,
    limit_use_with_user: null,
    name: 'Visual Percent',
    show: 0,
    started_at: 1_700_086_400,
    type: 2,
    updated_at: 1_700_086_400,
    value: 20,
  },
];
const adminCouponStoreFixtures = adminCouponFixtures.map((coupon) => ({
  ...coupon,
  value: coupon.type === 1 ? coupon.value / 100 : coupon.value,
}));
const adminGiftcardFixtures = [
  {
    code: 'GC-VISUAL-1000',
    created_at: 1_700_000_000,
    ended_at: 1_893_456_000,
    id: 1,
    limit_use: 3,
    name: 'Balance Gift',
    plan_id: null,
    started_at: 1_700_000_000,
    type: 1,
    updated_at: 1_700_000_000,
    used_user_ids: null,
    value: 1_000,
  },
  {
    code: 'GC-VISUAL-PLAN',
    created_at: 1_700_086_400,
    ended_at: 1_893_456_000,
    id: 2,
    limit_use: null,
    name: 'Plan Gift',
    plan_id: 1,
    started_at: 1_700_086_400,
    type: 5,
    updated_at: 1_700_086_400,
    used_user_ids: null,
    value: 30,
  },
];
const adminGiftcardStoreFixtures = adminGiftcardFixtures.map((giftcard) => ({
  ...giftcard,
  value: giftcard.type === 1 ? giftcard.value / 100 : giftcard.value,
}));
const viewports = [
  { height: 900, label: 'desktop', width: 1440 },
  { height: 844, label: 'mobile', width: 390 },
];
const darkModeStyleTargets = [
  { key: 'html', selector: 'html' },
  { key: 'body', selector: 'body' },
  { key: 'pageContainer', selector: '#page-container' },
  { key: 'pageHeader', selector: '#page-header' },
  { key: 'headerButton', selector: '#page-header button' },
  { key: 'sidebar', selector: '#sidebar' },
  { key: 'sidebarLink', selector: '#sidebar .nav-main-link, #sidebar a' },
  { key: 'mainContainer', selector: '#main-container' },
  { key: 'content', selector: '.content' },
  { key: 'block', selector: '.block' },
  { key: 'blockHeader', selector: '.block-header' },
  { key: 'blockContent', selector: '.block-content' },
  { key: 'primaryButton', selector: '.btn-primary, .ant-btn-primary' },
  { key: 'table', selector: '.ant-table, table' },
  { key: 'tableHeaderCell', selector: '.ant-table-thead th, table thead th' },
  { key: 'tableBodyCell', selector: '.ant-table-tbody td, table tbody td' },
  { key: 'input', selector: '.ant-input, input, textarea' },
  { key: 'alert', selector: '.alert' },
  { key: 'dashboardTile', selector: '.v2board-shortcuts-item, .block-link-pop' },
];
const selectedScenarios = scenarioFilter
  ? scenarios.filter((scenario) => scenario.label.includes(scenarioFilter))
  : scenarios;
const selectedViewports = viewportFilter
  ? viewports.filter((viewport) => viewport.label.includes(viewportFilter))
  : viewports;
const chromiumArgs = [
  '--disable-background-networking',
  '--disable-dev-shm-usage',
  '--disable-extensions',
  '--disable-gpu',
  '--mute-audio',
  '--no-sandbox',
];

function shouldUseFreshBrowser(scenario, viewport) {
  if (['0', 'false', 'shared'].includes(browserMode)) {
    return false;
  }
  if (['1', 'true', 'fresh'].includes(browserMode)) {
    return true;
  }
  if (browserMode === 'auto') {
    return !(scenario.label === 'admin-dashboard' && viewport.label === 'desktop');
  }
  throw new Error(`Unsupported VISUAL_PARITY_FRESH_BROWSER=${browserMode}`);
}

if (!selectedScenarios.length) {
  throw new Error(`No visual parity scenarios matched VISUAL_PARITY_FILTER=${scenarioFilter}`);
}

if (!selectedViewports.length) {
  throw new Error(
    `No visual parity viewports matched VISUAL_PARITY_VIEWPORT_FILTER=${viewportFilter}`,
  );
}
const metricSelectors = [
  'body',
  '#root',
  '#page-container',
  '#main-container',
  '.v2board-background',
  '.v2board-auth-box',
  '.v2board-auth-box > div',
  '.v2board-auth-box > div > div',
  '.block',
  '.block-content',
  '.block-content > .mb-3',
  '.block-content a',
  '.block-content p',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  '.font-size-h1',
  '.font-size-h1 > span',
  '.font-weight-normal',
  '.font-size-sm',
  '.form-group',
  '.form-row',
  '.col-9',
  '.col-3',
  '.form-control',
  'select.form-control',
  '.custom-control',
  '.custom-control-input',
  '.custom-control-label',
  '.btn',
  '#page-header .btn',
  '#page-header i',
  '.fa',
  '.far',
  '.fas',
  '.fa-fw',
  '.si',
  '.bg-gray-lighter',
  '.bg-gray-lighter > a',
  '.ant-divider',
  '.v2board-login-i18n-btn',
  '.v2board-login-i18n-btn span',
  '#sidebar',
  '#page-header',
  '#main-container > .content',
  '.content-header',
  '.nav-main-heading',
  '.nav-main-link',
  '.nav-main-link.active',
  '.v2board-copyright',
  '.v2board-container-title',
  '.alert',
  '.alert p',
  '.alert-link',
  '.alert strong',
  '.block-header',
  '.block-title',
  '.v2board-stats-bar',
  '.display-4',
  '.font-size-lg',
  '.font-w600',
  '.font-w700',
  '.text-white-75',
  '.badge',
  '.badge-danger',
  '.slick-slide .block-content',
  '.slick-slide .badge',
  '.slick-slide .font-size-lg',
  '.slick-slide .font-w600',
  '#orderChart',
  '#serverTodayRankChart',
  '#serverLastRankChart',
  '#userTodayRankChart',
  '#userLastRankChart',
  '.progress',
  '.progress-bar',
  '.ant-carousel',
  '.slick-slide',
  '.slick-dots',
  '.v2board-shortcuts-item',
  '.v2board-plan-tabs',
  '.block-link-pop',
  '.plan',
  '.v2board-sold-out-tag',
  '#cashier',
  '.v2board-select',
  '.v2board-select-radio',
  '.v2board-input-coupon',
  '.v2board-order-info',
  '.v2board-trade-no',
  '.ant-btn-primary',
  '.ant-result',
  '.ant-table-wrapper',
  '.ant-table',
  '.ant-table-thead',
  '.ant-table-tbody',
  '.ant-table-row',
  '.ant-table-scroll .ant-table-row',
  '.ant-table-scroll .ant-table-fixed-columns-in-body',
  '.ant-table-fixed-right',
  '.ant-table-fixed-right .ant-table-row',
  '.ant-table-fixed-right td',
  '.ant-table-fixed-right a',
  '.ant-table-fixed-right .ant-divider',
  '.ant-table-placeholder',
  '.ant-tag',
  '.ant-badge',
  '.ant-badge-status-dot',
  '.ant-pagination',
  '.ant-pagination-item',
  '.ant-pagination-options',
  '.am-list',
  '.am-list-item',
  '.list-group',
  '.list-group-item',
  '.ant-input',
  '.ant-input-search',
  '.ant-input-group-addon',
  '.ant-select',
  '.ant-select-selection',
  '.ant-switch',
  '.block-options',
  '.ant-spin',
  '.ant-spin-container',
  '.js-chat-messages',
  '.js-chat-form',
  '.js-chat-input',
  '.tag___12_9H',
  '.content___DW5w1',
  '.input___1j_ND',
  '.mw-100',
  '.bg-success-lighter',
];
const captureStabilityStyle = `
  *,
  *::before,
  *::after {
    animation-delay: 0s !important;
    animation-duration: 0s !important;
    animation-iteration-count: 1 !important;
    caret-color: transparent !important;
    transition-delay: 0s !important;
    transition-duration: 0s !important;
  }
`;

await mkdir(artifactDir, { recursive: true });

const sourceSettings = await readSourceSettings();
const oracleServer = await startOracleServer(serveOnly ? oraclePort : 0, oracleHost, publicOracleHost);

if (serveOnly) {
  console.log(`Legacy oracle user: ${new URL('/', oracleServer.baseUrl)}`);
  console.log(`Legacy oracle admin: ${new URL(`/${adminPath}#/login`, oracleServer.baseUrl)}`);
  console.log('Press Ctrl-C to stop.');
  await waitForShutdown();
  await oracleServer.close();
  process.exit(0);
}

if (parityMode === 'interactions') {
  try {
    await runInteractionParity(oracleServer.baseUrl);
  } finally {
    await oracleServer.close();
  }
  process.exit(0);
}

if (parityMode !== 'screenshots') {
  await oracleServer.close();
  throw new Error(`Unsupported VISUAL_PARITY_MODE=${parityMode}`);
}

const failures = [];
const report = [];
const reportPath = join(artifactDir, 'report.json');

async function writeReport() {
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
}

try {
  await writeReport();
  for (const scenario of selectedScenarios) {
    for (const viewport of selectedViewports) {
      const result = await compareScenario(oracleServer.baseUrl, scenario, viewport);
      report.push(result);
      await writeReport();
      if (result.diffRatio > maxDiffRatio || result.averageDelta > maxAverageDelta) {
        failures.push(
          `${result.label}/${result.viewport}: diff ${(result.diffRatio * 100).toFixed(2)}%, ` +
            `average delta ${result.averageDelta.toFixed(2)}`,
        );
      }
    }
  }
} finally {
  await oracleServer.close();
}

if (failures.length) {
  throw new Error(
    `Visual parity failed against the packaged oracle:\n` +
      failures.map((line) => `- ${line}`).join('\n') +
      `\nArtifacts: ${artifactDir}`,
  );
}

console.log('Visual parity OK: source screenshots match the packaged oracle threshold.');
for (const item of report) {
  console.log(
    `  ${item.label}/${item.viewport}: diff ${(item.diffRatio * 100).toFixed(3)}%, ` +
      `average delta ${item.averageDelta.toFixed(3)}`,
  );
}
console.log(`Artifacts: ${artifactDir}`);

async function compareScenario(oracleBaseUrl, scenario, viewport) {
  const name = `${scenario.label}-${viewport.label}`;
  const useFreshBrowser = shouldUseFreshBrowser(scenario, viewport);

  let sourceCapture;
  let oracleCapture;

  if (!useFreshBrowser) {
    const browser = await chromium.launch({ args: chromiumArgs, headless: true });
    try {
      try {
        sourceCapture = await captureScenario(
          browser,
          new URL(scenario.path, sourceBaseUrl).toString(),
          scenario,
          viewport,
          'source',
        );
      } catch (error) {
        throw new Error(`${name}/source: ${error.message}`);
      }

      try {
        oracleCapture = await captureScenario(
          browser,
          new URL(scenario.path, oracleBaseUrl).toString(),
          scenario,
          viewport,
          'oracle',
        );
      } catch (error) {
        throw new Error(`${name}/oracle: ${error.message}`);
      }
    } finally {
      await browser.close();
    }
  } else {
    try {
      sourceCapture = await captureScenarioWithFreshBrowser(
        new URL(scenario.path, sourceBaseUrl).toString(),
        scenario,
        viewport,
        'source',
      );
    } catch (error) {
      throw new Error(`${name}/source: ${error.message}`);
    }

    await delay(250);

    try {
      oracleCapture = await captureScenarioWithFreshBrowser(
        new URL(scenario.path, oracleBaseUrl).toString(),
        scenario,
        viewport,
        'oracle',
      );
    } catch (error) {
      throw new Error(`${name}/oracle: ${error.message}`);
    }
  }

  let diff = comparePngBuffers(
    sourceCapture.screenshot,
    oracleCapture.screenshot,
    channelThreshold,
  );
  let oracleRecaptures = [];

  if (shouldRecaptureUnstableFixedColumn(diff, sourceCapture, oracleCapture)) {
    for (let attempt = 1; attempt <= 2; attempt += 1) {
      await delay(250);
      const candidateOracleCapture = await captureScenarioWithFreshBrowser(
        new URL(scenario.path, oracleBaseUrl).toString(),
        scenario,
        viewport,
        'oracle',
      );
      const candidateDiff = comparePngBuffers(
        sourceCapture.screenshot,
        candidateOracleCapture.screenshot,
        channelThreshold,
      );
      oracleRecaptures.push({
        attempt,
        averageDelta: candidateDiff.averageDelta,
        diffPixels: candidateDiff.diffPixelCount,
        diffRatio: candidateDiff.diffRatio,
      });
      if (isBetterDiff(candidateDiff, diff)) {
        oracleCapture = candidateOracleCapture;
        diff = candidateDiff;
      }
      if (diff.diffPixelCount === 0) break;
    }
  }

  const sourcePath = join(artifactDir, `${name}-source.png`);
  const oraclePath = join(artifactDir, `${name}-oracle.png`);
  const diffPath = join(artifactDir, `${name}-diff.png`);

  await writeFile(sourcePath, sourceCapture.screenshot);
  await writeFile(oraclePath, oracleCapture.screenshot);
  await writeFile(diffPath, encodePng(diff.width, diff.height, diff.diffPixels));

  return {
    averageDelta: diff.averageDelta,
    diffPixels: diff.diffPixelCount,
    diffRatio: diff.diffRatio,
    height: diff.height,
    label: scenario.label,
    oracleDiagnostics: oracleCapture.diagnostics,
    oracleMetrics: oracleCapture.metrics,
    oraclePath,
    oracleRecaptures,
    sourceDiagnostics: sourceCapture.diagnostics,
    sourceMetrics: sourceCapture.metrics,
    sourcePath,
    totalPixels: diff.totalPixels,
    viewport: viewport.label,
    width: diff.width,
  };
}

function shouldRecaptureUnstableFixedColumn(diff, sourceCapture, oracleCapture) {
  return (
    diff.diffPixelCount > 0 &&
    (hasFixedColumnMetrics(sourceCapture.metrics) || hasFixedColumnMetrics(oracleCapture.metrics))
  );
}

function hasFixedColumnMetrics(metrics) {
  return (metrics?.elements ?? []).some((element) =>
    ['.ant-table-fixed-left', '.ant-table-fixed-right', '.ant-table-fixed-right .ant-table-row'].includes(
      element.selector,
    ),
  );
}

function isBetterDiff(candidate, current) {
  return (
    candidate.diffPixelCount < current.diffPixelCount ||
    (candidate.diffPixelCount === current.diffPixelCount &&
      candidate.averageDelta < current.averageDelta)
  );
}

async function runInteractionParity(oracleBaseUrl) {
  const interactionFilter = process.env.VISUAL_PARITY_INTERACTION_FILTER ?? scenarioFilter;
  const selectedInteractions = interactionScenarios.filter((interaction) => {
    if (!interactionFilter) return true;
    return (
      interaction.label.includes(interactionFilter) ||
      interaction.scenarioLabel.includes(interactionFilter)
    );
  });

  if (!selectedInteractions.length) {
    throw new Error(`No interaction parity scenarios matched ${interactionFilter}`);
  }

  await writeFile(join(artifactDir, 'report.json'), '[]\n');
  const report = [];
  const failures = [];

  for (const interaction of selectedInteractions) {
    const scenario = scenarioByLabel(interaction.scenarioLabel);
    for (const viewport of selectedViewports) {
      const name = `${interaction.label}-${viewport.label}`;
      let sourceResult;
      let oracleResult;

      sourceResult = await runInteractionTargetWithFreshBrowser(
        new URL(scenario.path, sourceBaseUrl).toString(),
        scenario,
        interaction,
        viewport,
        'source',
      );

      await delay(250);

      oracleResult = await runInteractionTargetWithFreshBrowser(
        new URL(scenario.path, oracleBaseUrl).toString(),
        scenario,
        interaction,
        viewport,
        'oracle',
      );

      const passed = stableJson(sourceResult) === stableJson(oracleResult);
      const item = {
        interaction: interaction.label,
        oracle: oracleResult,
        passed,
        source: sourceResult,
        viewport: viewport.label,
      };
      report.push(item);
      await writeFile(join(artifactDir, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
      if (!passed) {
        failures.push(
          `${name}\nsource: ${JSON.stringify(sourceResult)}\noracle: ${JSON.stringify(oracleResult)}`,
        );
      }
    }
  }

  if (failures.length) {
    throw new Error(
      `Interaction parity failed against the packaged oracle:\n${failures.join('\n\n')}\n` +
        `Artifacts: ${artifactDir}`,
    );
  }

  console.log('Interaction parity OK: source interactions match the packaged oracle.');
  for (const item of report) {
    console.log(`  ${item.interaction}/${item.viewport}: OK`);
  }
  console.log(`Artifacts: ${artifactDir}`);
}

async function runInteractionTargetWithFreshBrowser(url, scenario, interaction, viewport, target) {
  const browser = await chromium.launch({ args: chromiumArgs, headless: true });
  try {
    return await runInteractionTarget(browser, url, scenario, interaction, viewport, target);
  } finally {
    await browser.close();
  }
}

async function runInteractionTarget(browser, url, scenario, interaction, viewport, target) {
  const context = await browser.newContext({ viewport });
  const page = await context.newPage();
  try {
    await preparePageForInteraction(page, url, scenario, target, interaction);
    const result = await interaction.run(page);
    assertUsefulInteraction(interaction.label, result);
    return normalizeInteractionResult(interaction.label, result);
  } catch (error) {
    const snapshot = await readDebugSnapshot(page).catch(() => ({
      body: 'unavailable',
      title: 'unavailable',
      url: page.url(),
    }));
    throw new Error(
      `${interaction.label}/${viewport.label}/${target}: ${error.message}\n` +
        `URL: ${snapshot.url}\nTitle: ${snapshot.title}\nBody: ${snapshot.body}`,
    );
  } finally {
    await context.close();
  }
}

async function preparePageForInteraction(page, url, scenario, target, interaction = {}) {
  const diagnostics = [];
  page.__visualParityDiagnostics = diagnostics;
  page.on('console', (message) => {
    diagnostics.push(`${message.type()}: ${message.text()}`);
  });
  page.on('pageerror', (error) => {
    diagnostics.push(`pageerror: ${error.message}`);
  });
  page.on('requestfailed', (request) => {
    diagnostics.push(`requestfailed ${request.method()} ${request.url()}: ${request.failure()?.errorText}`);
  });
  page.on('response', (response) => {
    if (response.status() >= 400) {
      diagnostics.push(`response ${response.status()} ${response.url()}`);
    }
  });
  await installApiFixtures(page, scenario, target, interaction);
  if (scenario.warmupPath) {
    await gotoStable(page, new URL(scenario.warmupPath, url).toString());
    if (target === 'oracle' && scenario.seedLegacyAdminStore) {
      await seedLegacyAdminStore(page);
    }
    await navigateAfterWarmup(page, url);
  } else {
    await gotoStable(page, url);
  }
  if (target === 'oracle' && scenario.seedLegacyAdminStore) {
    await seedLegacyAdminStore(page);
  }
  if (scenario.readySelector) {
    await page.waitForSelector(scenario.readySelector, { state: 'visible', timeout: 10_000 });
  }
  if (scenario.postReadyDelay) {
    await page.waitForTimeout(scenario.postReadyDelay);
  }
  await waitForMountedContent(page, diagnostics);
  await waitForFontsBeforeCapture(page, diagnostics);
  await waitForFixedColumnLayout(page);
}

async function runLoginFormLanguageInteraction(page) {
  await fillFirstVisible(
    page,
    'input[type="text"], input:not([type]), input[type="email"]',
    'visual@example.com',
  );
  await fillFirstVisible(page, 'input[type="password"]', 'secret123');
  await clickFirstVisible(page, '.v2board-login-i18n-btn, .ant-dropdown-trigger');
  await page.waitForTimeout(150);
  return {
    email: await firstInputValue(
      page,
      'input[type="text"], input:not([type]), input[type="email"]',
    ),
    languageMenuItems: await visibleTexts(page, '.ant-dropdown-menu-item', 8),
    password: await firstInputValue(page, 'input[type="password"]'),
  };
}

async function runLoginLanguagePersistenceInteraction(page) {
  const before = await loginLanguagePersistenceState(page);
  await clickFirstVisible(page, '.v2board-login-i18n-btn, .ant-dropdown-trigger');
  await page.waitForTimeout(150);
  const menuItems = await visibleTexts(page, '.ant-dropdown-menu-item', 8);
  const navigation = page.waitForNavigation({ waitUntil: 'domcontentloaded', timeout: 3_000 }).catch(
    () => undefined,
  );
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item', ['English']);
  await navigation;
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await page.waitForTimeout(500);
  const afterSelect = await loginLanguagePersistenceState(page);
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 10_000 });
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await page.waitForTimeout(500);
  const afterReload = await loginLanguagePersistenceState(page);

  return {
    afterReload,
    afterSelect,
    before,
    menuItems,
  };
}

async function runDashboardHeaderLanguageDropdownInteraction(page) {
  const clicked = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const trigger = Array.from(
      document.querySelectorAll('#page-header button, #page-header .ant-dropdown-trigger'),
    ).find((element) => element.querySelector('.fa-language') && isVisible(element));
    if (!(trigger instanceof HTMLElement)) return false;
    trigger.click();
    return true;
  });
  if (!clicked) throw new Error('dashboard language trigger was not visible');
  await waitForVisibleText(page, '.ant-dropdown-menu-item', 'English');
  await page.waitForTimeout(150);
  return languageDropdownPlacementState(page);
}

async function runUserDashboardAvatarDropdownInteraction(page) {
  const before = await headerAvatarDropdownState(page);
  await clickHeaderAvatarTrigger(page);
  await waitForHeaderAvatarDropdown(page);
  await page.waitForTimeout(150);
  const opened = await headerAvatarDropdownState(page);
  return { before, opened };
}

async function runDarkModePersistenceInteraction(page) {
  const diagnostics = page.__visualParityDiagnostics ?? [];
  const before = await darkModePersistenceState(page);
  await clickDarkModeButton(page);
  await waitForDarkReader(page, diagnostics);
  const afterEnable = {
    ...(await darkModePersistenceState(page)),
    styleSnapshot: await waitForStableDarkModeStyleSnapshot(page, diagnostics),
  };
  await page.reload({ waitUntil: 'domcontentloaded', timeout: 10_000 });
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await waitForDarkReader(page, diagnostics);
  await waitForMountedContent(page, diagnostics);
  await waitForFontsBeforeCapture(page, diagnostics);
  await waitForFixedColumnLayout(page);
  const afterReload = {
    ...(await darkModePersistenceState(page)),
    styleSnapshot: await waitForStableDarkModeStyleSnapshot(page, diagnostics),
  };

  return {
    afterEnable,
    afterReload,
    before,
  };
}

async function runDashboardSubscribeDrawerInteraction(page) {
  await page.evaluate(() => {
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: (command) => command === 'copy',
    });
  });

  const before = await dashboardSubscribeState(page);
  await clickVisibleAt(page, '.v2board-shortcuts-item', 1);
  await page.waitForSelector('.oneClickSubscribe___2t9Xg', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(350);
  const opened = await dashboardSubscribeState(page);

  await clickFirstVisible(page, '.oneClickSubscribe___2t9Xg .subsrcibe-for-link');
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const copied = await dashboardSubscribeState(page);

  await clickFirstVisible(page, '.oneClickSubscribe___2t9Xg .subscribe-for-qrcode');
  await page.waitForSelector('.ant-modal canvas', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const qr = await dashboardSubscribeState(page);

  return { before, copied, opened, qr };
}

async function runDashboardSubscribeImportLinksInteraction(page) {
  const before = await dashboardSubscribeImportLinksState(page);
  await clickVisibleAt(page, '.v2board-shortcuts-item', 1);
  await page.waitForSelector('.oneClickSubscribe___2t9Xg', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(350);
  const opened = await dashboardSubscribeImportLinksState(page);

  return { before, opened };
}

async function runDashboardNoticeCarouselInteraction(page) {
  const before = await dashboardNoticeCarouselState(page);
  await clickVisibleAt(page, '.slick-dots li button', 1);
  await page.waitForTimeout(600);
  const afterDot = await dashboardNoticeCarouselState(page);

  await clickFirstVisible(page, '.slick-slide.slick-active a.block');
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(150);
  const opened = await dashboardNoticeCarouselState(page);

  await clickFirstVisible(page, '.ant-modal-close');
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await dashboardNoticeCarouselState(page);

  return { afterDot, before, closed, opened };
}

async function runDashboardResetPackageConfirmInteraction(page) {
  const initialOrderSaveCount = page.__visualParityUserOrderSaveCount ?? 0;
  const before = await dashboardResetPackageConfirmState(page);
  await clickFirstVisibleText(page, 'a, button', ['购买流量重置包']);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await dashboardResetPackageConfirmState(page);

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderSaveCount',
    initialOrderSaveCount + 1,
  );
  await page.waitForFunction(
    (tradeNo) => window.location.hash.includes(`/order/${tradeNo}`),
    dashboardResetPackageTradeNo,
    { timeout: 5_000 },
  );
  await page.waitForFunction(
    (tradeNo) => document.body.textContent?.includes(tradeNo),
    dashboardResetPackageTradeNo,
    { timeout: 10_000 },
  );
  await page.waitForTimeout(500);
  const confirmed = await dashboardResetPackageConfirmState(page);

  return {
    before,
    confirmed,
    hash: await page.evaluate(() => window.location.hash),
    opened,
    orderInfo: await visibleTexts(page, '.v2board-order-info', 6),
    orderSaveRequests: (page.__visualParityUserOrderSaveRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
  };
}

async function runDashboardNewPeriodConfirmInteraction(page) {
  const initialNewPeriodCount = page.__visualParityUserNewPeriodCount ?? 0;
  const initialSubscribeFetchCount = page.__visualParityUserSubscribeFetchCount ?? 0;
  const before = await dashboardNewPeriodConfirmState(page);

  await clickFirstVisibleText(page, 'a, button', ['提前开启流量周期']);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await dashboardNewPeriodConfirmState(page);

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserNewPeriodCount',
    initialNewPeriodCount + 1,
  );
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserSubscribeFetchCount',
    initialSubscribeFetchCount + 1,
  );
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const confirmed = await dashboardNewPeriodConfirmState(page);

  return {
    before,
    confirmed,
    hash: await page.evaluate(() => window.location.hash),
    newPeriodRequests: (page.__visualParityUserNewPeriodRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    opened,
    subscribeFetchDelta:
      (page.__visualParityUserSubscribeFetchCount ?? 0) - initialSubscribeFetchCount,
  };
}

async function runDashboardAlertLinksInteraction(page) {
  const before = await dashboardAlertLinksState(page);

  await clickVisibleAt(page, '.alert-danger .alert-link', 0);
  await page.waitForFunction(() => window.location.hash.includes('/order'), { timeout: 5_000 });
  await page.waitForSelector('.ant-table-thead, .am-list-body', { state: 'visible', timeout: 10_000 });
  await page.waitForTimeout(150);
  const order = await dashboardAlertLinksState(page);

  await page.evaluate(() => {
    window.location.hash = '#/dashboard';
  });
  await page.waitForSelector('.v2board-shortcuts-item', { state: 'visible', timeout: 10_000 });
  await page.waitForTimeout(300);
  const reset = await dashboardAlertLinksState(page);

  await clickVisibleAt(page, '.alert-warning .alert-link', 0);
  await page.waitForFunction(() => window.location.hash.includes('/ticket'), { timeout: 5_000 });
  await page.waitForSelector('.ant-table-thead, .am-list-body', { state: 'visible', timeout: 10_000 });
  await page.waitForTimeout(150);
  const ticket = await dashboardAlertLinksState(page);

  return { before, order, reset, ticket };
}

async function runProfileDepositModalInteraction(page) {
  await clickFirstVisible(page, '.ant-btn-primary');
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillFirstVisible(page, '.ant-modal-confirm input, .ant-modal input', '12.34');
  await page.waitForTimeout(100);
  const filled = {
    amount: await firstInputValue(page, '.ant-modal-confirm input, .ant-modal input'),
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
  };

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForPagePropertyAtLeast(page, '__visualParityUserOrderSaveCount', 1);
  await page.waitForFunction(
    (tradeNo) => window.location.hash.includes(`/order/${tradeNo}`),
    profileDepositTradeNo,
    { timeout: 5_000 },
  );
  await page.waitForFunction(
    (tradeNo) => document.body.textContent?.includes(tradeNo),
    profileDepositTradeNo,
    { timeout: 10_000 },
  );
  await page.waitForTimeout(500);

  return {
    filled,
    hash: await page.evaluate(() => window.location.hash),
    orderInfo: await visibleTexts(page, '.v2board-order-info', 6),
    orderSaveRequests: (page.__visualParityUserOrderSaveRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
  };
}

async function runProfileResetSubscribeConfirmInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialSubscribeFetchCount = page.__visualParityUserSubscribeFetchCount ?? 0;
  const before = await profileResetSubscribeState(page);
  await clickFirstVisibleText(page, 'a, button', ['重置', 'Reset']);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await profileResetSubscribeState(page);

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(page, '__visualParityUserResetSecurityCount', 1);
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const confirmed = await profileResetSubscribeState(page);

  return {
    before,
    confirmed,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
    subscribeFetchDelta:
      (page.__visualParityUserSubscribeFetchCount ?? 0) - initialSubscribeFetchCount,
  };
}

async function runProfileTelegramBindModalInteraction(page) {
  await page.evaluate(() => {
    window.__visualParityCopyCommandCount = 0;
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: (command) => {
        if (command === 'copy') window.__visualParityCopyCommandCount += 1;
        return command === 'copy';
      },
    });
  });

  const before = await profileTelegramBindState(page);
  await clickFirstVisibleText(page, '.bind_telegram a, .bind_telegram button', [
    '立即开始',
    'Start Now',
  ]);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForFunction(
    () => document.querySelector('.ant-modal')?.textContent?.includes('@legacy_bot'),
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
  const opened = await profileTelegramBindState(page);

  await clickFirstVisible(page, '.ant-modal code');
  await page.waitForFunction(() => (window.__visualParityCopyCommandCount ?? 0) > 0, {
    timeout: 5_000,
  });
  const copied = await profileTelegramBindState(page);

  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary, .ant-modal .ant-btn-primary');
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await profileTelegramBindState(page);

  return { before, closed, copied, opened };
}

async function runProfileTelegramUnbindConfirmInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const initialSubscribeFetchCount = page.__visualParityUserSubscribeFetchCount ?? 0;
  const before = await profileTelegramUnbindState(page);

  await clickFirstVisibleText(page, '.unbind_telegram button, .unbind_telegram .ant-btn', [
    '解除绑定',
  ]);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await profileTelegramUnbindState(page);

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(page, '__visualParityUserUnbindTelegramCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserInfoFetchCount',
    initialInfoFetchCount + 1,
  );
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserSubscribeFetchCount',
    initialSubscribeFetchCount + 1,
  );
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const confirmed = await profileTelegramUnbindState(page);

  return {
    before,
    confirmed,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    opened,
    subscribeFetchDelta:
      (page.__visualParityUserSubscribeFetchCount ?? 0) - initialSubscribeFetchCount,
  };
}

async function runProfilePreferenceSwitchesInteraction(page) {
  const preferenceKeys = ['auto_renewal', 'remind_expire', 'remind_traffic'];
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await profilePreferenceSwitchesState(page);
  const toggles = [];

  for (let index = 0; index < preferenceKeys.length; index += 1) {
    const infoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
    const updateResponse = page.waitForResponse(
      (response) => {
        const url = new URL(response.url());
        return (
          url.pathname === '/api/v1/user/update' && response.request().method() === 'POST'
        );
      },
      { timeout: 5_000 },
    );

    await clickVisibleAt(page, '.ant-switch', index);
    await waitForProfileSwitchLoading(page, index);
    const loading = await profilePreferenceSwitchesState(page);

    await updateResponse;
    await waitForPagePropertyAtLeast(
      page,
      '__visualParityUserInfoFetchCount',
      infoFetchCount + 1,
    );
    await page.waitForTimeout(100);

    const after = await profilePreferenceSwitchesState(page);
    toggles.push({
      afterSwitch: after.switches[index],
      field: preferenceKeys[index],
      loadingSwitch: loading.switches[index],
      updateRequestCount: after.updateRequests.length,
    });
  }

  const after = await profilePreferenceSwitchesState(page);
  return {
    after,
    before,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    toggles,
  };
}

async function runProfileRedeemGiftcardInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await profileRedeemGiftcardState(page);

  await page
    .locator('input[placeholder*="Gift Card"], input[placeholder*="礼品卡"]')
    .first()
    .fill('CARD-123');
  await page.waitForTimeout(100);
  const filled = await profileRedeemGiftcardState(page);

  await clickProfileRedeemGiftcardButton(page);
  await waitForProfileRedeemGiftcardLoading(page);
  const loading = await profileRedeemGiftcardState(page);

  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserInfoFetchCount',
    initialInfoFetchCount + 1,
  );
  await page.waitForTimeout(100);
  const after = await profileRedeemGiftcardState(page);

  return {
    after,
    before,
    filled,
    infoFetchDelta: (page.__visualParityUserInfoFetchCount ?? 0) - initialInfoFetchCount,
    loading,
  };
}

async function runProfileChangePasswordSuccessInteraction(page) {
  const before = await profileChangePasswordState(page);

  await fillProfileChangePasswordInputs(page, ['old-password', 'new-password', 'new-password']);
  await page.waitForTimeout(100);
  const filled = await profileChangePasswordState(page);

  await clickProfileChangePasswordButton(page);
  await waitForProfileChangePasswordLoading(page);
  const loading = await profileChangePasswordState(page);

  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () => window.location.hash.includes('/login') || window.location.hash.includes('/dashboard'),
    { timeout: 5_000 },
  );
  await page.waitForTimeout(300);
  const after = await profileChangePasswordState(page);

  return { after, before, filled, loading };
}

async function runPlansFilterTabsInteraction(page) {
  const before = await plansFilterState(page);
  await clickVisibleAt(page, '.v2board-plan-tabs span', 1);
  await page.waitForTimeout(150);
  const period = await plansFilterState(page);
  await clickVisibleAt(page, '.v2board-plan-tabs span', 2);
  await page.waitForTimeout(150);
  const traffic = await plansFilterState(page);
  return { before, period, traffic };
}

async function runPlanCheckoutCouponInteraction(page) {
  const selectCount = await visibleCount(page, '#cashier .v2board-select');
  if (selectCount > 1) {
    await clickVisibleAt(page, '#cashier .v2board-select', 1);
  }
  await fillFirstVisible(page, '.v2board-input-coupon', couponCheckFixture.code);
  await clickCouponVerifyButton(page);
  await page
    .waitForFunction((couponName) => document.body.textContent.includes(couponName), couponCheckFixture.name, {
      timeout: 5_000,
    })
    .catch(() => {});

  return {
    activePeriodIndex: await visibleElementDomIndex(page, '#cashier .v2board-select.active', 0),
    activePeriods: await visibleTexts(page, '#cashier .v2board-select.active', 2),
    couponInput: await firstInputValue(page, '.v2board-input-coupon'),
    selectCount,
    summaryBlocks: await visibleTexts(page, '#cashier .col-md-4 .block', 4),
    submitButton: await firstElementState(page, '#cashier .btn-block.btn-primary'),
  };
}

async function runOrderPaymentMethodInteraction(page) {
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('#cashier .v2board-select')).filter((element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      }).length >= 3,
    { timeout: 5_000 },
  );
  const before = await orderPaymentState(page);
  await clickVisibleAt(page, '#cashier .v2board-select', 2);
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll('#cashier .v2board-select'))
        .filter(isVisible)
        .findIndex((element) => element.className.includes('active')) === 2;
    },
    { timeout: 5_000 },
  );
  const after = await orderPaymentState(page);
  return { after, before };
}

async function runOrderQrCheckoutInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  const before = await orderCheckoutState(page);
  await clickFirstVisible(page, '#cashier .btn-block.btn-primary');
  await page.waitForTimeout(100);
  const loading = await orderCheckoutState(page);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCheckoutCount',
    initialCheckoutCount + 1,
  );
  await page.waitForSelector('.v2board-payment-qrcode svg, .v2board-payment-qrcode canvas', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await orderCheckoutState(page);
  return {
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    loading,
    opened,
  };
}

async function runOrderStripeDisabledCheckoutInteraction(page) {
  await waitForOrderPaymentMethodCount(page);
  const before = await orderCheckoutState(page);
  await clickVisibleAt(page, '#cashier .v2board-select', 1);
  await waitForPagePropertyAtLeast(page, '__visualParityUserStripePublicKeyCount', 1);
  await waitForCreditCardSection(page);
  await page.waitForTimeout(150);
  const selected = await orderCheckoutState(page);
  return {
    before,
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    selected,
  };
}

async function runOrderRedirectCheckoutInteraction(page) {
  const initialCheckoutCount = page.__visualParityUserOrderCheckoutCount ?? 0;
  await waitForOrderPaymentMethodCount(page);
  await clickVisibleAt(page, '#cashier .v2board-select', 2);
  await page.waitForTimeout(100);
  const selected = await orderCheckoutState(page);
  await clickFirstVisible(page, '#cashier .btn-block.btn-primary');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCheckoutCount',
    initialCheckoutCount + 1,
  );
  await page.waitForFunction(() => window.location.hash.includes('cashier=visual'), {
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const redirected = await orderCheckoutState(page);
  return {
    checkoutRequests: clonePageRequests(page.__visualParityUserOrderCheckoutRequests),
    redirected,
    selected,
  };
}

async function runNodeTableScrollInteraction(page) {
  const before = await legacyAntTableScrollState(page);
  await setLegacyAntTableScrollLeft(page, 'right');
  await page.waitForTimeout(150);
  const afterRight = await legacyAntTableScrollState(page);
  await setLegacyAntTableScrollLeft(page, 'middle');
  await page.waitForTimeout(150);
  const afterMiddle = await legacyAntTableScrollState(page);

  return { afterMiddle, afterRight, before };
}

async function runUserNodeTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, ['.ant-table-thead .anticon-question-circle']);
}

async function runTrafficTableScrollInteraction(page) {
  const before = await legacyAntTableScrollState(page);
  await setLegacyAntTableScrollLeft(page, 'right');
  await page.waitForTimeout(150);
  const afterRight = await legacyAntTableScrollState(page);
  await setLegacyAntTableScrollLeft(page, 'middle');
  await page.waitForTimeout(150);
  const afterMiddle = await legacyAntTableScrollState(page);

  return { afterMiddle, afterRight, before };
}

async function runUserTrafficTotalTooltipInteraction(page) {
  return hoverTooltipInteraction(page, [
    '.ant-table-fixed .anticon-question-circle',
    '.ant-table-thead .anticon-question-circle',
  ]);
}

async function runKnowledgeDrawerInteraction(page) {
  await fillFirstVisible(page, '.v2board-knowledge-search-bar input', 'router');
  await page.waitForTimeout(350);
  const before = await knowledgeState(page);
  await clickFirstVisible(page, '.list-group-item');
  await page.waitForSelector('.ant-drawer-open .ant-drawer-title', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () => document.querySelector('.ant-drawer-title')?.textContent?.includes('Copy Article'),
    { timeout: 5_000 },
  );
  const opened = await knowledgeState(page);
  await clickFirstVisible(page, '.ant-drawer-close');
  await page.waitForFunction(
    () => !document.querySelector('.ant-drawer-open'),
    { timeout: 5_000 },
  );
  const closed = await knowledgeState(page);
  return { before, closed, opened };
}

async function runInviteGenerateInteraction(page) {
  const before = await inviteState(page);
  await clickFirstVisible(page, '.block-header .block-options .btn');
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const after = await inviteState(page);
  return { after, before };
}

async function runInviteTransferModalInteraction(page) {
  const initialInfoFetchCount = page.__visualParityUserInfoFetchCount ?? 0;
  const before = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', ['划转', 'Transfer']);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const opened = await inviteFinanceDialogState(page);
  await fillVisibleAt(page, '.ant-modal input:not([disabled])', 0, '12.34');
  await page.waitForTimeout(100);
  const filled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  await page.waitForTimeout(100);
  const saving = await inviteFinanceDialogState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserTransferCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
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

async function runInviteWithdrawModalInteraction(page) {
  const before = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, 'button, .ant-btn', [
    '推广佣金提现',
    'Invitation Commission Withdrawal',
  ]);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await page.waitForTimeout(100);
  const opened = await inviteFinanceDialogState(page);
  await clickFirstVisible(page, '.ant-modal .ant-select-selection');
  await page.waitForSelector('.ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const dropdown = await inviteFinanceDialogState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Alipay']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal input.ant-input', 0, 'parity-account@example.com');
  await page.waitForTimeout(100);
  const filled = await inviteFinanceDialogState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  await page.waitForTimeout(100);
  const saving = await inviteFinanceDialogState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserWithdrawCount', 1);
  await page.waitForFunction(() => window.location.hash.includes('/ticket'), { timeout: 5_000 });
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

async function runUserInviteTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, ['.anticon-question-circle']);
}

async function runUserTicketReplySendInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  await fillFirstVisible(page, '.js-chat-input', 'Parity reply send');
  await page.waitForTimeout(100);
  const filled = await ticketReplyState(page);

  await page.locator('.js-chat-input').first().press('Enter');
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const loading = await ticketReplyState(page);

  await waitForPagePropertyAtLeast(page, '__visualParityUserTicketReplyCount', 1);
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
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

async function runAdminTicketReplySendInteraction(page) {
  const initialTicketFetchCount = page.__visualParityAdminTicketFetchCount ?? 0;
  await fillFirstVisible(page, '.js-chat-input', 'Parity admin reply send');
  await page.waitForTimeout(100);
  const filled = await ticketReplyState(page);

  await page.locator('.js-chat-input').first().press('Enter');
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
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

async function runUserTicketCreateModalInteraction(page) {
  const initialTicketFetchCount = page.__visualParityUserTicketFetchCount ?? 0;
  const before = await userTicketCreateModalState(page);
  await clickFirstVisible(page, '.block-header .block-options .btn, .block-header .block-options button');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity subject');
  await clickFirstVisible(page, '.ant-modal .ant-select-selection');
  await page.waitForSelector('.ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  const levelDropdown = await userTicketCreateModalState(page);
  await clickVisibleAt(page, '.ant-select-dropdown-menu-item', 2);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal textarea.ant-input', 0, 'Parity ticket body');
  await page.waitForTimeout(100);
  const filled = await userTicketCreateModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await page.waitForTimeout(100);
  const saving = await userTicketCreateModalState(page);
  await waitForPagePropertyAtLeast(page, '__visualParityUserTicketSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
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

async function runOrderCancelConfirmInteraction(page) {
  const cancelLinkTexts = ['Cancel', '取消'];
  const initialOrderCancelCount = page.__visualParityUserOrderCancelCount ?? 0;
  const initialOrderFetchCount = page.__visualParityUserOrderFetchCount ?? 0;
  const cancelLinks = await visibleTextCount(page, 'a', cancelLinkTexts);
  if (!cancelLinks) {
    return {
      cancelLinks,
      listItems: await visibleCount(page, '.am-list-item'),
      modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    };
  }

  await clickFirstVisibleText(page, 'a', cancelLinkTexts);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const opened = {
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 2),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    title: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 2),
  };

  await clickFirstVisible(
    page,
    '.ant-modal-confirm-btns .ant-btn-primary, .ant-modal .ant-btn-primary',
  );
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityUserOrderCancelCount',
    initialOrderCancelCount + 1,
  );
  await page.waitForTimeout(150);

  return {
    cancelLinks,
    confirmed: {
      modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    },
    opened,
    orderCancelRequests: (page.__visualParityUserOrderCancelRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    orderFetchDelta: (page.__visualParityUserOrderFetchCount ?? 0) - initialOrderFetchCount,
  };
}

async function runAdminDashboardCommissionShortcutInteraction(page) {
  const before = await adminDashboardShortcutState(page);
  await clickVisibleAt(page, '.alert-danger .alert-link', 1);
  await page.waitForFunction(() => window.location.hash.includes('/order'), { timeout: 5_000 });
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await page.waitForTimeout(150);
  const after = await adminDashboardShortcutState(page);

  return { after, before };
}

async function runAdminDashboardAvatarDropdownInteraction(page) {
  const before = await headerAvatarDropdownState(page);
  await clickHeaderAvatarTrigger(page);
  await waitForHeaderAvatarDropdown(page);
  await page.waitForTimeout(150);
  const opened = await headerAvatarDropdownState(page);
  return { before, opened };
}

async function runAdminConfigTabsInteraction(page) {
  const before = await activeTabState(page);
  await clickVisibleAt(page, '.ant-tabs-tab', 1);
  await page.waitForTimeout(250);
  const second = await activeTabState(page);
  await clickVisibleAt(page, '.ant-tabs-tab', 2);
  await page.waitForTimeout(250);
  const third = await activeTabState(page);
  return { before, second, third };
}

async function runAdminPlanCreateDrawerInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const before = await adminPlanDrawerState(page);
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '新建订阅');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Plan');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 1, '<p>Parity plan body</p>');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 2, '12.34');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 3, '23.45');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 8, '199.00');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 10, '250');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 11, '7');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 12, '99');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 13, '50');
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Default');
  const groupDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Default']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '按月重置');
  const resetDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['按月重置']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await clickFirstVisible(page, '.ant-drawer-open .ant-checkbox-wrapper');
  await page.waitForTimeout(100);
  const filled = await adminPlanDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-open .v2board-drawer-action .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  await waitForVisibleElementsHidden(page, '.ant-drawer-title');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPlanFetchCount',
    initialPlanFetchCount + 1,
  );
  const closed = await adminPlanDrawerState(page);
  return {
    before,
    closed,
    filled,
    groupDropdown,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    resetDropdown,
    saveRequests: (page.__visualParityAdminPlanSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminPlanCreateGroupSelectDropdownInteraction(page) {
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '新建订阅');
  const before = await legacySelectDropdownState(page, '.ant-drawer-open');
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Default');
  await page.waitForTimeout(700);
  const opened = await legacySelectDropdownState(page, '.ant-drawer-open');
  return { before, opened };
}

async function runAdminPlanEditDrawerInteraction(page) {
  const initialPlanFetchCount = page.__visualParityAdminPlanFetchCount ?? 0;
  const before = await adminPlanDrawerState(page);
  await clickAdminOrderRowAction(page, 'Pro', '操作');
  await waitForVisibleText(page, '.ant-dropdown-menu-item a', '编辑');
  const menuOpened = await adminPlanDrawerState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['编辑']);
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '编辑订阅');
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('.ant-drawer-open .ant-input')).some(
        (element) => 'value' in element && element.value === 'Pro',
      ),
    { timeout: 5_000 },
  );
  const opened = await adminPlanDrawerState(page);
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Edited Plan');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 1, '<p>Edited plan body</p>');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 2, '88.88');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 10, '300');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 11, '8');
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '不重置');
  const resetDropdown = await adminPlanDrawerState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['不重置']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await clickFirstVisible(page, '.ant-drawer-open .ant-checkbox-wrapper');
  await page.waitForTimeout(100);
  const edited = await adminPlanDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-open .v2board-drawer-action .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPlanSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  await waitForVisibleElementsHidden(page, '.ant-drawer-title');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPlanFetchCount',
    initialPlanFetchCount + 1,
  );
  const closed = await adminPlanDrawerState(page);
  return {
    before,
    closed,
    edited,
    menuOpened,
    opened,
    planFetchDelta: (page.__visualParityAdminPlanFetchCount ?? 0) - initialPlanFetchCount,
    resetDropdown,
    saveRequests: (page.__visualParityAdminPlanSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminPlanRenewTooltipInteraction(page) {
  return hoverTooltipInteraction(page, ['.ant-table-thead .anticon-question-circle']);
}

async function runAdminTicketsReplyFilterInteraction(page) {
  const before = await adminTicketsReplyFilterState(page);
  await clickFirstVisible(page, '.ant-table-column-has-filters .ant-dropdown-trigger');
  await page.waitForSelector('.ant-table-filter-dropdown', {
    state: 'visible',
    timeout: 5_000,
  });
  const opened = await adminTicketsReplyFilterState(page);
  await clickAdminTicketsReplyFilterOption(page, '待回复');
  await page.waitForTimeout(100);
  const selected = await adminTicketsReplyFilterState(page);
  await clickFirstVisibleText(page, '.ant-table-filter-dropdown-link.confirm', ['确定']);
  await page.waitForTimeout(300);
  const confirmed = await adminTicketsReplyFilterState(page);
  return { before, confirmed, opened, selected };
}

async function runAdminThemeSettingsInteraction(page) {
  await clickFirstVisibleText(page, 'button', ['主题设置']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Theme Title');
  await page.waitForTimeout(100);
  const opened = await adminThemeModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return !Array.from(document.querySelectorAll('.ant-modal')).some(isVisible);
    },
    { timeout: 5_000 },
  );
  const closed = await adminThemeModalState(page);
  return { closed, opened };
}

async function runAdminServerCreateNodeDrawerInteraction(page) {
  const before = await adminServerNodeDrawerState(page);
  await page.locator('.v2board-table-action .ant-dropdown-trigger').first().hover();
  await page.waitForTimeout(150);
  await clickFirstVisible(page, '.v2board-table-action .ant-dropdown-trigger');
  await waitForVisibleText(page, '.ant-dropdown-menu-item', 'Shadowsocks');
  const menuOpened = await adminServerNodeDrawerState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['Shadowsocks']);
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '新建节点');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Node');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 1, '1.5');
  await page.waitForTimeout(100);
  const drawerOpened = await adminServerNodeDrawerState(page);
  await page.mouse.move(1, 1);
  await page.waitForTimeout(150);
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Default');
  const groupDropdown = await adminServerNodeDrawerState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Default']);
  await page.keyboard.press('Escape').catch(() => undefined);
  await page.waitForTimeout(100);
  const groupSelected = await adminServerNodeDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-close');
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  const closed = await adminServerNodeDrawerState(page);
  return { before, closed, drawerOpened, groupDropdown, groupSelected, menuOpened };
}

async function runAdminServerEditNodeDrawerInteraction(page) {
  const before = await adminServerNodeDrawerState(page);
  await clickAdminTableRowDropdownAction(page, 'Tokyo 01', '编辑');
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '编辑节点');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-drawer-open .ant-input')).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Tokyo 01') && values.includes('jp.example.com') && values.includes('8388');
    },
    { timeout: 5_000 },
  );
  const opened = await adminServerNodeDrawerState(page);
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Edited Node');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 1, '2.25');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 2, 'edited-node.example.test');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 3, '9443');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 4, '18388');
  await page.waitForTimeout(100);
  const edited = await adminServerNodeDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-close');
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  const closed = await adminServerNodeDrawerState(page);
  return { before, closed, edited, opened };
}

async function runAdminServerRouteEditModalInteraction(page) {
  const before = await adminServerRouteModalState(page);
  await clickAdminOrderRowAction(page, 'Block ads', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑路由');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-modal .ant-input')).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Block ads') && values.some((value) => value.includes('domain:example.com'));
    },
    { timeout: 5_000 },
  );
  const opened = await adminServerRouteModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Route');
  await fillVisibleAt(
    page,
    '.ant-modal textarea.ant-input',
    0,
    'domain:edited.example.com\ngeosite:openai',
  );
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '指定DNS服务器进行解析');
  const actionDropdown = await adminServerRouteModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['指定DNS服务器进行解析']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal .ant-input', 2, '1.1.1.1');
  await page.waitForTimeout(100);
  const edited = await adminServerRouteModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminServerRouteModalState(page);
  return { actionDropdown, before, closed, edited, opened };
}

async function runAdminServerRouteCreateModalInteraction(page) {
  const before = await adminServerRouteModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加路由']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '创建路由');
  const opened = await adminServerRouteModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Created Route');
  await fillVisibleAt(
    page,
    '.ant-modal textarea.ant-input',
    0,
    'domain:created.example.com\ngeosite:created',
  );
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '指定DNS服务器进行解析');
  const actionDropdown = await adminServerRouteModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['指定DNS服务器进行解析']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal .ant-input', 2, '9.9.9.9');
  await page.waitForTimeout(100);
  const edited = await adminServerRouteModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminServerRouteModalState(page);
  return { actionDropdown, before, closed, edited, opened };
}

async function runAdminServerGroupCreateModalInteraction(page) {
  const initialGroupFetchCount = page.__visualParityAdminServerGroupFetchCount ?? 0;
  const before = await adminServerGroupModalState(page);
  await clickFirstVisibleText(page, 'button', ['添加权限组']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '创建组');
  const opened = await adminServerGroupModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Created Group');
  await page.waitForTimeout(100);
  const edited = await adminServerGroupModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerGroupSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerGroupFetchCount',
    initialGroupFetchCount + 1,
  );
  const closed = await adminServerGroupModalState(page);
  return {
    before,
    closed,
    edited,
    groupFetchDelta:
      (page.__visualParityAdminServerGroupFetchCount ?? 0) - initialGroupFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminServerGroupSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminServerGroupEditModalInteraction(page) {
  const initialGroupFetchCount = page.__visualParityAdminServerGroupFetchCount ?? 0;
  const before = await adminServerGroupModalState(page);
  await clickAdminOrderRowAction(page, 'Default', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑组');
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('.ant-modal .ant-input')).some(
        (element) => 'value' in element && element.value === 'Default',
      ),
    { timeout: 5_000 },
  );
  const opened = await adminServerGroupModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Group');
  await page.waitForTimeout(100);
  const edited = await adminServerGroupModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminServerGroupSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminServerGroupFetchCount',
    initialGroupFetchCount + 1,
  );
  const closed = await adminServerGroupModalState(page);
  return {
    before,
    closed,
    edited,
    groupFetchDelta:
      (page.__visualParityAdminServerGroupFetchCount ?? 0) - initialGroupFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminServerGroupSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminPaymentCreateModalInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  await clickFirstVisibleText(page, 'button', ['添加支付方式']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(() => document.body.textContent.includes('商户ID'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Pay');
  await page.waitForTimeout(100);
  const opened = await adminPaymentModalState(page);
  await clickFirstVisible(page, '.ant-modal .ant-select-selection');
  await page.waitForSelector('.ant-select-dropdown-menu-item', {
    state: 'visible',
    timeout: 5_000,
  });
  const dropdown = await adminPaymentModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['StripeCheckout']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await page.waitForFunction(() => document.body.textContent.includes('Secret Key'), {
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 5, 'pk_parity_create');
  await fillVisibleAt(page, '.ant-modal .ant-input', 6, 'sk_parity_create');
  await page.waitForTimeout(100);
  const switched = await adminPaymentModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return !Array.from(document.querySelectorAll('.ant-modal')).some(isVisible);
    },
    { timeout: 5_000 },
  );
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPaymentFetchCount',
    initialPaymentFetchCount + 1,
  );
  const closed = await adminPaymentModalState(page);
  return {
    closed,
    dropdown,
    opened,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: (page.__visualParityAdminPaymentSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    switched,
  };
}

async function runAdminPaymentEditModalInteraction(page) {
  const initialPaymentFetchCount = page.__visualParityAdminPaymentFetchCount ?? 0;
  const before = await adminPaymentModalState(page);
  await clickAdminOrderRowAction(page, 'Alipay', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑支付方式');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-modal .ant-input')).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Alipay') && values.includes('visual-merchant');
    },
    { timeout: 5_000 },
  );
  const opened = await adminPaymentModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Pay');
  await fillVisibleAt(page, '.ant-modal .ant-input', 5, 'edited-secret');
  await fillVisibleAt(page, '.ant-modal .ant-input', 6, 'edited-merchant');
  await page.waitForTimeout(100);
  const edited = await adminPaymentModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminPaymentSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminPaymentFetchCount',
    initialPaymentFetchCount + 1,
  );
  const closed = await adminPaymentModalState(page);
  return {
    before,
    closed,
    edited,
    opened,
    paymentFetchDelta:
      (page.__visualParityAdminPaymentFetchCount ?? 0) - initialPaymentFetchCount,
    saveRequests: (page.__visualParityAdminPaymentSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminPaymentNotifyTooltipInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, ['.ant-table-thead .anticon-question-circle']);
}

async function runAdminOrderDetailModalInteraction(page) {
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['VIS...001']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      document.body.textContent.includes('订单信息') &&
      document.body.textContent.includes('VISUAL2026110001'),
    { timeout: 5_000 },
  );
  const opened = await adminOrderDetailModalState(page);
  await clickFirstVisible(page, '.ant-modal-close');
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminOrderDetailModalState(page);
  return { closed, opened };
}

async function runAdminOrderStatusTooltipsInteraction(page) {
  return hoverAllTooltipTargetsInteraction(page, ['.ant-table-thead .anticon-question-circle']);
}

async function runAdminOrderAssignModalInteraction(page) {
  await clickFirstVisibleText(page, 'button', ['添加订单']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForFunction(
    () =>
      document.body.textContent.includes('订单分配') &&
      document.body.textContent.includes('用户邮箱'),
    { timeout: 5_000 },
  );
  const opened = await adminOrderAssignModalState(page);
  await fillVisibleAt(page, '.ant-modal input', 0, 'assign-user@example.com');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Pro');
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Pro']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '月付');
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['月付']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal input', 1, '12.34');
  await page.waitForTimeout(100);
  const filled = await adminOrderAssignModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminOrderAssignModalState(page);
  return {
    assignRequest: page.__visualParityLastAdminOrderAssign ?? null,
    closed,
    filled,
    opened,
  };
}

async function runAdminOrderStatusDropdownInteraction(page) {
  const before = await adminOrderStatusDropdownState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['标记为']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '已支付');
  const opened = await adminOrderStatusDropdownState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item', ['已支付']);
  await waitForVisibleElementsHidden(page, '.ant-dropdown');
  const closed = await adminOrderStatusDropdownState(page);
  return {
    before,
    closed,
    opened,
    paidRequest: page.__visualParityLastAdminOrderPaid ?? null,
  };
}

async function runAdminOrderCommissionDropdownInteraction(page) {
  const before = await adminOrderCommissionDropdownState(page);
  await clickAdminOrderRowAction(page, 'VIS...002', '标记为');
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '无效');
  const opened = await adminOrderCommissionDropdownState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item', ['无效']);
  await waitForVisibleElementsHidden(page, '.ant-dropdown');
  const closed = await adminOrderCommissionDropdownState(page);
  return {
    before,
    closed,
    opened,
    updateRequest: page.__visualParityLastAdminOrderUpdate ?? null,
  };
}

async function runAdminCouponCreateModalInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Coupon');
  await fillVisibleAt(page, '.ant-modal .ant-input', 1, 'PARITY2026');
  await fillVisibleAt(page, '.ant-modal input[type="number"], .ant-modal .ant-input', 2, '25');
  await page.waitForTimeout(100);
  const opened = await adminCouponModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return !Array.from(document.querySelectorAll('.ant-modal')).some(isVisible);
    },
    { timeout: 5_000 },
  );
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminCouponFetchCount',
    initialCouponFetchCount + 1,
  );
  const closed = await adminCouponModalState(page);
  return {
    closed,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    generateRequests: (page.__visualParityAdminCouponGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    opened,
  };
}

async function runAdminCouponRangePickerInteraction(page) {
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '新建优惠券');
  const before = await legacyRangePickerState(page);
  await clickFirstVisible(page, '.ant-modal .ant-calendar-range-picker-input');
  await page.waitForSelector('.ant-calendar-picker-container', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await legacyRangePickerState(page);
  return { before, opened };
}

async function runAdminCouponEditModalInteraction(page) {
  const initialCouponFetchCount = page.__visualParityAdminCouponFetchCount ?? 0;
  const before = await adminCouponModalState(page);
  await clickAdminOrderRowAction(page, 'Visual Amount', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑优惠券');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-modal input')).map((element) =>
        'value' in element ? element.value : '',
      );
      return values.includes('Visual Amount') && values.includes('VISUAL100') && values.includes('10');
    },
    { timeout: 5_000 },
  );
  const opened = await adminCouponModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Coupon');
  await fillVisibleAt(page, '.ant-modal .ant-input', 1, 'EDIT2026');
  await fillVisibleAt(page, '.ant-modal input[type="number"], .ant-modal .ant-input', 2, '12.5');
  await page.waitForTimeout(100);
  const edited = await adminCouponModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminCouponGenerateCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminCouponFetchCount',
    initialCouponFetchCount + 1,
  );
  const closed = await adminCouponModalState(page);
  return {
    before,
    closed,
    couponFetchDelta: (page.__visualParityAdminCouponFetchCount ?? 0) - initialCouponFetchCount,
    edited,
    generateRequests: (page.__visualParityAdminCouponGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    opened,
  };
}

async function runAdminGiftcardCreateModalInteraction(page) {
  const initialGiftcardFetchCount = page.__visualParityAdminGiftcardFetchCount ?? 0;
  const before = await adminGiftcardModalState(page);
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Giftcard');
  await fillVisibleAt(page, '.ant-modal .ant-input', 1, 'GIFT2026');
  const opened = await adminGiftcardModalState(page);
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '兑换订阅套餐');
  const typeDropdown = await adminGiftcardModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['兑换订阅套餐']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillFirstVisible(page, '.ant-modal input[placeholder="一次性套餐输入0"]', '0');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Pro');
  const planDropdown = await adminGiftcardModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Pro']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillFirstVisible(
    page,
    '.ant-modal input[placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"]',
    '9',
  );
  await page.waitForTimeout(100);
  const filled = await adminGiftcardModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminGiftcardGenerateCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminGiftcardFetchCount',
    initialGiftcardFetchCount + 1,
  );
  const closed = await adminGiftcardModalState(page);
  return {
    before,
    closed,
    filled,
    generateRequests: (page.__visualParityAdminGiftcardGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    giftcardFetchDelta:
      (page.__visualParityAdminGiftcardFetchCount ?? 0) - initialGiftcardFetchCount,
    opened,
    planDropdown,
    typeDropdown,
  };
}

async function runAdminGiftcardEditModalInteraction(page) {
  const initialGiftcardFetchCount = page.__visualParityAdminGiftcardFetchCount ?? 0;
  const before = await adminGiftcardModalState(page);
  await clickAdminOrderRowAction(page, 'Plan Gift', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑礼品卡');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-modal input')).map((element) =>
        'value' in element ? element.value : '',
      );
      return values.includes('Plan Gift') && values.includes('GC-VISUAL-PLAN') && values.includes('30');
    },
    { timeout: 5_000 },
  );
  const opened = await adminGiftcardModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Giftcard');
  await fillVisibleAt(page, '.ant-modal .ant-input', 1, 'EDIT-GIFT-2026');
  await fillFirstVisible(page, '.ant-modal input[placeholder="一次性套餐输入0"]', '45');
  await fillFirstVisible(
    page,
    '.ant-modal input[placeholder="限制最大使用次数，用完则无法使用(为空则不限制)"]',
    '4',
  );
  await page.waitForTimeout(100);
  const edited = await adminGiftcardModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminGiftcardGenerateCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminGiftcardFetchCount',
    initialGiftcardFetchCount + 1,
  );
  const closed = await adminGiftcardModalState(page);
  return {
    before,
    closed,
    edited,
    generateRequests: (page.__visualParityAdminGiftcardGenerateRequests ?? []).map((request) =>
      structuredClone(request),
    ),
    giftcardFetchDelta:
      (page.__visualParityAdminGiftcardFetchCount ?? 0) - initialGiftcardFetchCount,
    opened,
  };
}

async function runAdminNoticeCreateModalInteraction(page) {
  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  const before = await adminNoticeModalState(page);
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Notice');
  await fillVisibleAt(page, '.ant-modal textarea.ant-input', 0, 'Parity notice body');
  await fillFirstVisible(page, '.ant-modal .ant-select-search__field', 'ops');
  await page.keyboard.press('Enter');
  await fillVisibleAt(page, '.ant-modal .ant-input', 2, 'https://example.test/notice.png');
  await page.waitForTimeout(100);
  const filled = await adminNoticeModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminNoticeFetchCount',
    initialNoticeFetchCount + 1,
  );
  const closed = await adminNoticeModalState(page);
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  const reopened = await adminNoticeModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  return {
    before,
    closed,
    filled,
    noticeFetchDelta: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
    reopened,
    saveRequests: (page.__visualParityAdminNoticeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminNoticeEditModalInteraction(page) {
  const initialNoticeFetchCount = page.__visualParityAdminNoticeFetchCount ?? 0;
  const before = await adminNoticeModalState(page);
  await clickAdminOrderRowAction(page, 'Hidden Notice', '编辑');
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '编辑公告');
  await page.waitForFunction(
    () => {
      const values = Array.from(document.querySelectorAll('.ant-modal input, .ant-modal textarea')).map(
        (element) => ('value' in element ? element.value : ''),
      );
      return values.includes('Hidden Notice') && values.includes('<p>Second notice</p>');
    },
    { timeout: 5_000 },
  );
  const opened = await adminNoticeModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'Parity Edited Notice');
  await fillVisibleAt(page, '.ant-modal textarea.ant-input', 0, '<p>Parity edited notice body</p>');
  await fillFirstVisible(page, '.ant-modal .ant-select-search__field', 'edited');
  await page.keyboard.press('Enter');
  await fillVisibleAt(page, '.ant-modal .ant-input', 2, 'https://example.test/notice-edited.png');
  await page.waitForTimeout(100);
  const edited = await adminNoticeModalState(page);
  await clickFirstVisible(page, '.ant-modal-footer .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminNoticeSaveCount', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminNoticeFetchCount',
    initialNoticeFetchCount + 1,
  );
  const closed = await adminNoticeModalState(page);
  return {
    before,
    closed,
    edited,
    noticeFetchDelta: (page.__visualParityAdminNoticeFetchCount ?? 0) - initialNoticeFetchCount,
    opened,
    saveRequests: (page.__visualParityAdminNoticeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminKnowledgeCreateDrawerInteraction(page) {
  const initialKnowledgeFetchCount = page.__visualParityAdminKnowledgeFetchCount ?? 0;
  const before = await adminKnowledgeDrawerState(page);
  await clickFirstVisible(page, '.bg-white .ant-btn');
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '新增知识');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Knowledge');
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 1, 'Parity');
  await clickVisibleAt(page, '.ant-drawer-open .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'English');
  const languageDropdown = await adminKnowledgeDrawerState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['English']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillFirstVisible(
    page,
    '.ant-drawer-open textarea.section-container.input',
    '# Parity Knowledge\n\nParity body',
  );
  await page.waitForTimeout(100);
  const filled = await adminKnowledgeDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-open .v2board-drawer-action .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminKnowledgeSaveCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminKnowledgeFetchCount',
    initialKnowledgeFetchCount + 1,
  );
  const saved = await adminKnowledgeDrawerState(page);
  await clickVisibleAt(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  await waitForVisibleElementsHidden(page, '.ant-drawer-title');
  const closed = await adminKnowledgeDrawerState(page);
  return {
    before,
    closed,
    filled,
    knowledgeFetchDelta:
      (page.__visualParityAdminKnowledgeFetchCount ?? 0) - initialKnowledgeFetchCount,
    languageDropdown,
    saved,
    saveRequests: (page.__visualParityAdminKnowledgeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminKnowledgeEditDrawerInteraction(page) {
  const initialKnowledgeFetchCount = page.__visualParityAdminKnowledgeFetchCount ?? 0;
  const before = await adminKnowledgeDrawerState(page);
  await clickAdminOrderRowAction(page, 'Copy Article', '编辑');
  await page.waitForSelector('.ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-drawer-title', '编辑知识');
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('.ant-drawer-open .ant-input')).some(
        (element) => 'value' in element && element.value === 'Copy Article',
      ),
    { timeout: 5_000 },
  );
  const opened = await adminKnowledgeDrawerState(page);
  await fillVisibleAt(page, '.ant-drawer-open .ant-input', 0, 'Parity Edited Article');
  await fillFirstVisible(
    page,
    '.ant-drawer-open textarea.section-container.input',
    '## Parity Edited Article\n\nEdited body',
  );
  await page.waitForTimeout(100);
  const edited = await adminKnowledgeDrawerState(page);
  await clickFirstVisible(page, '.ant-drawer-open .v2board-drawer-action .ant-btn-primary');
  await waitForPagePropertyAtLeast(page, '__visualParityAdminKnowledgeSaveCount', 1);
  await waitForPagePropertyAtLeast(
    page,
    '__visualParityAdminKnowledgeFetchCount',
    initialKnowledgeFetchCount + 1,
  );
  const saved = await adminKnowledgeDrawerState(page);
  await clickVisibleAt(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  await waitForVisibleElementsHidden(page, '.ant-drawer-title');
  const closed = await adminKnowledgeDrawerState(page);
  return {
    before,
    closed,
    edited,
    knowledgeFetchDelta:
      (page.__visualParityAdminKnowledgeFetchCount ?? 0) - initialKnowledgeFetchCount,
    opened,
    saved,
    saveRequests: (page.__visualParityAdminKnowledgeSaveRequests ?? []).map((request) =>
      structuredClone(request),
    ),
  };
}

async function runAdminUsersFilterInteraction(page) {
  await clickFirstVisible(page, '.v2board-table-action .ant-btn, .ant-btn');
  await page.waitForSelector('.v2board-filter-drawer, .ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await clickFirstVisible(page, '.v2board-filter-drawer .ant-btn-primary');
  await fillFirstVisible(page, '.v2board-filter-drawer .ant-input', 'visual@example.com');
  await page.waitForTimeout(100);
  return {
    firstInput: await firstInputValue(page, '.v2board-filter-drawer .ant-input'),
    visibleButtons: await visibleTexts(page, '.v2board-filter-drawer .ant-btn', 6),
  };
}

async function runAdminUsersFilterFieldSelectDropdownInteraction(page) {
  await clickFirstVisible(page, '.v2board-table-action .ant-btn, .ant-btn');
  await page.waitForSelector('.v2board-filter-drawer, .ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await clickFirstVisible(page, '.v2board-filter-drawer .ant-btn-primary');
  const before = await legacySelectDropdownState(page, '.v2board-filter-drawer');
  await clickVisibleAt(page, '.v2board-filter-drawer .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '到期时间');
  await page.waitForTimeout(700);
  const opened = await legacySelectDropdownState(page, '.v2board-filter-drawer');
  return { before, opened };
}

async function runAdminUsersFilterExpiryPickerInteraction(page) {
  await clickFirstVisible(page, '.v2board-table-action .ant-btn, .ant-btn');
  await page.waitForSelector('.v2board-filter-drawer, .ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await clickFirstVisible(page, '.v2board-filter-drawer .ant-btn-primary');
  await clickVisibleAt(page, '.v2board-filter-drawer .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '到期时间');
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['到期时间']);
  await page.waitForSelector('.v2board-filter-drawer .ant-calendar-picker-input', {
    state: 'visible',
    timeout: 5_000,
  });
  const before = await legacyDatePickerState(page, '.v2board-filter-drawer');
  await clickFirstVisible(page, '.v2board-filter-drawer .ant-calendar-picker-input');
  await page.waitForSelector('.ant-calendar-picker-container', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await legacyDatePickerState(page, '.v2board-filter-drawer');
  return { before, opened };
}

async function runAdminUserBulkBanConfirmInteraction(page) {
  return runAdminUserBulkConfirmInteraction(page, '批量封禁', '确定要进行封禁吗？');
}

async function runAdminUserBulkDeleteConfirmInteraction(page) {
  return runAdminUserBulkConfirmInteraction(page, '批量删除', '确定要进行删除吗？');
}

async function runAdminUserBulkConfirmInteraction(page, actionText, contentText) {
  const before = await adminUserBulkActionState(page);
  page.__visualParityLastAdminUserFetchQuery = null;
  await clickFirstVisible(page, '.v2board-table-action .ant-btn, .ant-btn');
  await page.waitForSelector('.v2board-filter-drawer, .ant-drawer-open', {
    state: 'visible',
    timeout: 5_000,
  });
  await clickFirstVisible(page, '.v2board-filter-drawer .ant-btn-primary');
  await fillFirstVisible(page, '.v2board-filter-drawer .ant-input', 'visual@example.com');
  await clickFirstVisibleText(page, '.v2board-filter-drawer .v2board-drawer-action .ant-btn', [
    '检索',
    '检 索',
  ]);
  await waitForVisibleElementsHidden(page, '.ant-drawer-open');
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  const filtered = await adminUserBulkActionState(page);
  await page.hover('.v2board-table-action .ant-dropdown-trigger');
  await waitForVisibleText(page, '.ant-dropdown-menu-item', actionText);
  const dropdown = await adminUserBulkActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', [actionText]);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-confirm-title, .ant-modal-title', '提醒');
  await waitForVisibleText(page, '.ant-modal-confirm-content, .ant-modal-body', contentText);
  const opened = await adminUserBulkActionState(page);
  await clickVisibleAt(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  const closed = await adminUserBulkActionState(page);
  return { actionText, before, closed, contentText, dropdown, filtered, opened };
}

async function runAdminUserCreateModalInteraction(page) {
  const before = await adminUserCreateModalState(page);
  await clickVisibleAt(page, '.v2board-table-action .ant-btn', 2);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-modal-title', '创建用户');
  const opened = await adminUserCreateModalState(page);
  await fillVisibleAt(page, '.ant-modal .ant-input', 0, 'parity.created');
  await fillVisibleAt(page, '.ant-modal .ant-input', 2, 'example.com');
  await fillVisibleAt(page, '.ant-modal .ant-input', 3, 'secret123');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Pro');
  const planDropdown = await adminUserCreateModalState(page);
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Pro']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await page.waitForTimeout(100);
  const filled = await adminUserCreateModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminUserCreateModalState(page);
  return { before, closed, filled, opened, planDropdown };
}

async function runAdminUserCreatePlanSelectDropdownInteraction(page) {
  await clickVisibleAt(page, '.v2board-table-action .ant-btn', 2);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-modal-title', '创建用户');
  const before = await legacySelectDropdownState(page, '.ant-modal');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Pro');
  await page.waitForTimeout(700);
  const opened = await legacySelectDropdownState(page, '.ant-modal');
  return { before, opened };
}

async function runAdminUserCreateExpiryPickerInteraction(page) {
  await clickVisibleAt(page, '.v2board-table-action .ant-btn', 2);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-modal-title', '创建用户');
  const before = await legacyDatePickerState(page, '.ant-modal');
  await clickFirstVisible(page, '.ant-modal .ant-calendar-picker-input');
  await page.waitForSelector('.ant-calendar-picker-container', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(150);
  const opened = await legacyDatePickerState(page, '.ant-modal');
  return { before, opened };
}

async function runAdminUserSendMailModalInteraction(page) {
  const before = await adminUserSendMailModalState(page);
  await page.hover('.v2board-table-action .ant-dropdown-trigger');
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '发送邮件');
  const dropdown = await adminUserSendMailModalState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['发送邮件']);
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-modal-title', '发送邮件');
  const opened = await adminUserSendMailModalState(page);
  await fillVisibleAt(page, '.ant-modal input:not([disabled])', 0, 'Parity Mail Subject');
  await fillVisibleAt(page, '.ant-modal textarea.ant-input', 0, 'Parity mail body\nLine two');
  await page.waitForTimeout(100);
  const filled = await adminUserSendMailModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminUserSendMailModalState(page);
  return { before, closed, dropdown, filled, opened };
}

async function runAdminUserResetSecretConfirmInteraction(page) {
  const before = await adminUserConfirmState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '重置UUID及订阅URL');
  const dropdown = await adminUserConfirmState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['重置UUID及订阅URL']);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-confirm-title, .ant-modal-title', '重置安全信息');
  const opened = await adminUserConfirmState(page);
  await clickVisibleAt(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  const closed = await adminUserConfirmState(page);
  return { before, closed, dropdown, opened };
}

async function runAdminUserDeleteConfirmInteraction(page) {
  const before = await adminUserConfirmState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '删除用户');
  const dropdown = await adminUserConfirmState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['删除用户']);
  await page.waitForSelector('.ant-modal-confirm, .ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-confirm-title, .ant-modal-title', '删除用户');
  const opened = await adminUserConfirmState(page);
  await clickVisibleAt(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 0);
  await waitForVisibleElementsHidden(page, '.ant-modal-confirm, .ant-modal');
  const closed = await adminUserConfirmState(page);
  return { before, closed, dropdown, opened };
}

async function runAdminUserCopyActionInteraction(page) {
  const before = await adminUserCopyActionState(page);
  await page.evaluate(() => {
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value: (command) => command === 'copy',
    });
  });
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '复制订阅URL');
  const dropdown = await adminUserCopyActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['复制订阅URL']);
  await page.waitForSelector('.ant-message-notice, .ant-notification-notice', {
    state: 'visible',
    timeout: 5_000,
  });
  await page.waitForTimeout(100);
  const copied = await adminUserCopyActionState(page);
  return { before, copied, dropdown };
}

async function runAdminUserEditActionInteraction(page) {
  const before = await adminUserEditActionState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '编辑');
  const opened = await adminUserEditActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['编辑']);
  await page.waitForSelector('.ant-drawer-open', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-drawer-title', '用户管理');
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('.ant-drawer-open .ant-input')).some(
        (element) => element instanceof HTMLInputElement && element.value === 'visual-user@example.com',
      ),
    { timeout: 5_000 },
  );
  const drawer = await adminUserEditActionState(page);
  return { before, drawer, opened };
}

async function runAdminUserAssignActionInteraction(page) {
  const before = await adminUserAssignActionState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', '分配订单');
  const opened = await adminUserAssignActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['分配订单']);
  await page.waitForSelector('.ant-modal', {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, '.ant-modal-title', '订单分配');
  const modalOpened = await adminOrderAssignModalState(page);
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 0);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', 'Pro');
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['Pro']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await clickVisibleAt(page, '.ant-modal .ant-select-selection', 1);
  await waitForVisibleText(page, '.ant-select-dropdown-menu-item', '月付');
  await clickFirstVisibleText(page, '.ant-select-dropdown-menu-item', ['月付']);
  await waitForVisibleElementsHidden(page, '.ant-select-dropdown');
  await fillVisibleAt(page, '.ant-modal input', 1, '23.45');
  await page.waitForTimeout(100);
  const filled = await adminOrderAssignModalState(page);
  await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  await waitForVisibleElementsHidden(page, '.ant-modal');
  const closed = await adminOrderAssignModalState(page);
  return {
    assignRequest: page.__visualParityLastAdminOrderAssign ?? null,
    before,
    closed,
    filled,
    modalOpened,
    opened,
  };
}

async function runAdminUserOrdersActionInteraction(page) {
  const before = await adminUserOrdersActionState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', 'TA的订单');
  const opened = await adminUserOrdersActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item', ['TA的订单']);
  await page.waitForFunction(() => window.location.hash.includes('/order'), { timeout: 5_000 });
  await waitForPageProperty(page, '__visualParityLastAdminOrderFetchQuery');
  await page.waitForSelector('.ant-table-tbody tr', { state: 'visible', timeout: 5_000 });
  const navigated = await adminUserOrdersActionState(page);
  return { before, navigated, opened };
}

async function runAdminUserInviteActionInteraction(page) {
  const before = await adminUserInviteActionState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', 'TA的邀请');
  const opened = await adminUserInviteActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item', ['TA的邀请']);
  await waitForPageProperty(page, '__visualParityLastAdminFilteredUserFetchQuery');
  const filtered = await adminUserInviteActionState(page);
  return { before, filtered, opened };
}

async function runAdminUserTrafficActionInteraction(page) {
  const before = await adminUserTrafficActionState(page);
  await clickFirstVisibleText(page, '.ant-table-tbody a', ['操作']);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', 'TA的流量记录');
  const opened = await adminUserTrafficActionState(page);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', ['TA的流量记录']);
  await waitForPageProperty(page, '__visualParityLastAdminUserTrafficQuery');
  await page.waitForSelector('.ant-modal', { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, '.ant-modal-title', '流量记录');
  const modal = await adminUserTrafficActionState(page);
  return { before, modal, opened };
}

async function adminThemeModalState(page) {
  return {
    inputValues: await visibleInputValues(page, '.ant-modal .ant-input'),
    labels: await visibleTexts(page, '.ant-modal label', 10),
    modalCount: await visibleCount(page, '.ant-modal'),
    titles: await visibleTexts(page, '.ant-modal-title', 4),
  };
}

async function adminDashboardShortcutState(page) {
  const orderFilter = await page.evaluate(() => {
    const value = window.sessionStorage.getItem('v2board-admin-order-filter');
    if (!value) return null;
    try {
      return JSON.parse(value);
    } catch {
      return value;
    }
  });

  return {
    alertLinks: await visibleTexts(page, '.alert-danger .alert-link', 4),
    hash: await page.evaluate(() => window.location.hash),
    orderFetchQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminOrderFetchQuery),
    orderFilter,
    shortcutTexts: await visibleTexts(page, '.js-classic-nav .font-w600', 8),
  };
}

async function adminPaymentModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 6),
    inputValues: await visibleInputValues(page, '.ant-modal .ant-input'),
    labels: await visibleTexts(page, '.ant-modal label', 12),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedPayment: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 2),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 4),
  };
}

async function adminServerNodeDrawerState(page) {
  return {
    actionButtons: await visibleTexts(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 4),
    drawerCount: await visibleCount(page, '.ant-drawer-open'),
    dropdownCount: await visibleCount(page, '.ant-dropdown'),
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    inputValues: await visibleInputValues(page, '.ant-drawer-open .ant-input'),
    labels: await visibleTexts(page, '.ant-drawer-open .form-group label', 20),
    selectDropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 10),
    selectedValues: [
      ...(await visibleTexts(page, '.ant-drawer-open .ant-select-selection-selected-value', 8)),
      ...(await visibleTexts(page, '.ant-drawer-open .ant-select-selection__choice__content', 8)),
    ],
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 8),
    titles: await visibleTexts(page, '.ant-drawer-open .ant-drawer-title', 4),
  };
}

async function adminServerRouteModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 10),
    inputValues: await visibleInputValues(page, '.ant-modal .ant-input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '.ant-modal'),
    pageButtons: await visibleTexts(page, 'button', 8),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 4),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function adminServerGroupModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    inputValues: await visibleInputValues(page, '.ant-modal .ant-input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 4),
    modalCount: await visibleCount(page, '.ant-modal'),
    pageButtons: await visibleTexts(page, 'button', 8),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function adminOrderDetailModalState(page) {
  return {
    bodyRows: await visibleTexts(page, '.ant-modal .ant-row', 20),
    modalCount: await visibleCount(page, '.ant-modal'),
    titles: await visibleTexts(page, '.ant-modal-title', 4),
  };
}

async function adminOrderAssignModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    inputValues: await visibleInputValues(page, '.ant-modal input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 4),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function adminOrderStatusDropdownState(page) {
  return {
    dropdownCount: await visibleCount(page, '.ant-dropdown'),
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 6),
    orderRows: await visibleTexts(page, '.ant-table-tbody tr', 4),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 8),
  };
}

async function adminOrderCommissionDropdownState(page) {
  return {
    dropdownCount: await visibleCount(page, '.ant-dropdown'),
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 8),
    orderRows: await visibleTexts(page, '.ant-table-tbody tr', 4),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 8),
  };
}

async function adminTicketsReplyFilterState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizedText = (element) => (element.textContent ?? '').trim().replace(/\s+/g, ' ');
    const filterDropdowns = Array.from(
      document.querySelectorAll('.ant-table-filter-dropdown'),
    ).filter(isVisible);
    const filterItems = Array.from(
      document.querySelectorAll('.ant-table-filter-dropdown .ant-dropdown-menu-item'),
    )
      .filter(isVisible)
      .slice(0, 4)
      .map((element) => ({
        checked: Boolean(
          element.querySelector('.ant-checkbox-checked, .ant-checkbox-wrapper-checked, input:checked'),
        ),
        text: normalizedText(element),
      }));

    return {
      dropdownCount: filterDropdowns.length,
      filterItems,
      tableReplyStatusTexts: Array.from(document.querySelectorAll('.ant-table-tbody tr'))
        .filter(isVisible)
        .map((row) => Array.from(row.querySelectorAll('td')).filter(isVisible))
        .filter((cells) => cells.length >= 4)
        .slice(0, 4)
        .map((cells) => normalizedText(cells[3])),
    };
  });
}

async function adminUserOrdersActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    hash: await page.evaluate(() => window.location.hash),
    orderFetchQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminOrderFetchQuery),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function adminUserEditActionState(page) {
  return {
    actionButtons: await visibleTexts(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 4),
    drawerCount: await visibleCount(page, '.ant-drawer-open'),
    drawerInputValues: await visibleInputValues(page, '.ant-drawer-open .ant-input'),
    drawerLabels: await visibleTexts(page, '.ant-drawer-open .form-group label', 20),
    drawerTitle: await visibleTexts(page, '.ant-drawer-title', 2),
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    selectedValues: await visibleTexts(page, '.ant-drawer-open .ant-select-selection-selected-value', 8),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function adminUserCreateModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 8),
    inputValues: await visibleInputValues(page, '.ant-modal .ant-input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 4),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
    toolbarButtons: await visibleTexts(page, '.v2board-table-action .ant-btn', 6),
  };
}

async function adminUserSendMailModalState(page) {
  const modalCount = await visibleCount(page, '.ant-modal');
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: modalCount ? [] : await visibleTexts(page, '.ant-dropdown-menu-item', 8),
    inputValues: await visibleInputValues(page, '.ant-modal input, .ant-modal textarea'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 6),
    modalCount,
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
    toolbarButtons: await visibleTexts(page, '.v2board-table-action .ant-btn', 6),
  };
}

async function adminUserConfirmState(page) {
  const modalCount = await visibleCount(page, '.ant-modal-confirm, .ant-modal');
  return {
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 4),
    dropdownItems: modalCount ? [] : await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    modalCount,
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 2),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function adminUserCopyActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    messageTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function adminUserBulkActionState(page) {
  const modalCount = await visibleCount(page, '.ant-modal-confirm, .ant-modal');
  return {
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 4),
    drawerCount: await visibleCount(page, '.ant-drawer-open'),
    dropdownItems: modalCount ? [] : await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    filterQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserFetchQuery),
    inputValues: await visibleInputValues(page, '.v2board-filter-drawer .ant-input'),
    modalCount,
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 2),
    toolbarButtons: await visibleTexts(page, '.v2board-table-action .ant-btn', 8),
  };
}

async function adminUserAssignActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function adminUserInviteActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    hash: await page.evaluate(() => window.location.hash),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
    userFetchQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminFilteredUserFetchQuery),
  };
}

async function adminUserTrafficActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    modalCount: await visibleCount(page, '.ant-modal'),
    modalRows: await visibleTexts(page, '.ant-modal .ant-table-tbody tr', 6),
    modalTitle: await visibleTexts(page, '.ant-modal-title', 2),
    tableHeaders: await visibleTexts(page, '.ant-modal .ant-table-thead th', 8),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    trafficQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserTrafficQuery),
    triggerTexts: await visibleTexts(page, '.ant-table-tbody a', 10),
  };
}

async function userTicketCreateModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    inputValues: await visibleInputValues(page, '.ant-modal input, .ant-modal textarea'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 6),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 4),
    selectDropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 6),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function ticketReplyState(page) {
  return {
    inputValue: await firstInputValue(page, '.js-chat-input'),
    messageTexts: await visibleTexts(page, '.js-chat-messages', 6),
    sendButton: await firstElementState(page, '.js-chat-form button, .js-chat-form .ant-btn'),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

function normalizeAdminOrderFetchQuery(query) {
  if (!query) return null;
  const { total: _total, ...rest } = query;
  return rest;
}

function scenarioByLabel(label) {
  const scenario = scenarios.find((item) => item.label === label);
  if (!scenario) {
    throw new Error(`Unknown visual parity scenario ${label}`);
  }
  return scenario;
}

function stableJson(value) {
  return JSON.stringify(sortForStableJson(value));
}

function jsonIncludesAny(value, candidates) {
  const json = JSON.stringify(value);
  return candidates.some((candidate) => json.includes(candidate));
}

function clonePageRequests(requests = []) {
  return (requests ?? []).map((request) =>
    request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
  );
}

function sortForStableJson(value) {
  if (Array.isArray(value)) {
    return value.map(sortForStableJson);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, sortForStableJson(nested)]),
    );
  }
  return value;
}

function normalizeInteractionResult(label, result) {
  const normalized = sortForStableJson(result);
  if (
    label === 'admin-plan-create-group-select-dropdown' ||
    label === 'admin-users-filter-field-select-dropdown' ||
    label === 'admin-user-create-plan-select-dropdown'
  ) {
    return normalizeSelectDropdownInteractionResult(label, normalized);
  }
  if (label === 'admin-plan-edit-drawer') {
    const stripActionDropdownItems = (state) => {
      if (!state) return state;
      const { actionDropdownItems: _actionDropdownItems, ...rest } = state;
      return rest;
    };
    return {
      ...normalized,
      closed: stripActionDropdownItems(normalized.closed),
      edited: stripActionDropdownItems(normalized.edited),
      opened: stripActionDropdownItems(normalized.opened),
      resetDropdown: stripActionDropdownItems(normalized.resetDropdown),
    };
  }
  if (label === 'admin-server-edit-node-drawer') {
    const stripOuterDropdown = (state) => {
      if (!state) return state;
      const { dropdownCount: _dropdownCount, dropdownItems: _dropdownItems, ...rest } = state;
      return rest;
    };
    return {
      ...normalized,
      closed: stripOuterDropdown(normalized.closed),
      edited: stripOuterDropdown(normalized.edited),
      opened: stripOuterDropdown(normalized.opened),
    };
  }
  if (label === 'admin-user-invite-action' && normalized.filtered) {
    const { dropdownItems: _dropdownItems, ...filtered } = normalized.filtered;
    return { ...normalized, filtered };
  }
  if (label === 'admin-user-edit-action' && normalized.drawer) {
    const { dropdownItems: _dropdownItems, ...drawer } = normalized.drawer;
    return { ...normalized, drawer };
  }
  if (label === 'admin-user-traffic-action' && normalized.modal) {
    const { dropdownItems: _dropdownItems, ...modal } = normalized.modal;
    return { ...normalized, modal };
  }
  return normalized;
}

function normalizeSelectDropdownInteractionResult(label, result) {
  const stripTransientSelectMotionClass = (state) => {
    if (!state?.dropdownClass) return state;
    return {
      ...state,
      dropdownClass: state.dropdownClass
        .split(/\s+/)
        .filter(
          (className) =>
            !/^slide-up-(?:appear|enter|leave)(?:-active)?$/.test(className) &&
            !/^ant-select-dropdown-placement-[A-Za-z]+$/.test(className),
        )
        .join(' '),
    };
  };
  const stripUnstableModalGeometry = (state) => {
    if (label !== 'admin-user-create-plan-select-dropdown' || !state) return state;
    const { geometry: _geometry, ...rest } = state;
    return rest;
  };
  return {
    ...result,
    before: stripUnstableModalGeometry(stripTransientSelectMotionClass(result.before)),
    opened: stripUnstableModalGeometry(stripTransientSelectMotionClass(result.opened)),
  };
}

function assertUsefulInteraction(label, result) {
  if (
    label === 'user-login-form-language' &&
    (result.email !== 'visual@example.com' ||
      result.password !== 'secret123' ||
      !result.languageMenuItems.length)
  ) {
    throw new Error('login form or language menu did not produce observable state');
  }
  if (
    label === 'user-login-language-persistence' &&
    (!result.menuItems?.includes('English') ||
      result.afterSelect?.storedLocale !== 'en-US' ||
      result.afterSelect?.cookieI18n !== 'en-US' ||
      !result.afterSelect?.triggerText?.includes('English') ||
      result.afterReload?.storedLocale !== 'en-US' ||
      result.afterReload?.cookieI18n !== 'en-US' ||
      !result.afterReload?.triggerText?.includes('English'))
  ) {
    throw new Error(`login language persistence did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-header-language-dropdown' &&
    (result.dropdownCount !== 1 ||
      result.placement !== 'bottomCenter' ||
      !result.opensBelow ||
      result.gap !== 4 ||
      Math.abs(result.centerDelta ?? 99) > 1 ||
      !result.triggerOpen ||
      !result.items?.includes('English') ||
      !result.items?.includes('简体中文') ||
      !result.items?.includes('繁體中文'))
  ) {
    throw new Error(`dashboard language dropdown did not match legacy placement: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-avatar-dropdown' &&
    (result.before?.menuCount !== 0 ||
      result.opened?.menuCount !== 1 ||
      !result.opened?.menuClass?.includes('dropdown-menu-right') ||
      Math.abs(result.opened?.rightDelta ?? 99) > 1 ||
      result.opened?.items?.length < 2 ||
      !jsonIncludesAny(result.opened?.items, [
        'Profile',
        'User Center',
        'My Account',
        '个人中心',
        '我的账户',
        '您的帳戸',
        '您的帳戶',
      ]) ||
      !jsonIncludesAny(result.opened?.items, ['Logout', '登出']))
  ) {
    throw new Error(`user avatar dropdown did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-dashboard-avatar-dropdown' &&
    (result.before?.menuCount !== 0 ||
      result.opened?.menuCount !== 1 ||
      !result.opened?.menuClass?.includes('dropdown-menu-right') ||
      !result.opened?.menuClass?.includes('dropdown-menu-lg') ||
      Math.abs(result.opened?.rightDelta ?? 99) > 1 ||
      !jsonIncludesAny(result.opened?.items, ['Logout', '登出']))
  ) {
    throw new Error(`admin avatar dropdown did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label.endsWith('dark-mode-persistence') &&
    (result.before?.cookieDarkMode === '1' ||
      result.before?.darkReaderReady ||
      result.afterEnable?.cookieDarkMode !== '1' ||
      !result.afterEnable?.darkReaderReady ||
      !result.afterEnable?.iconClass?.includes('fa-moon') ||
      result.afterEnable?.styleSnapshot?.capturedCount < 8 ||
      !result.afterEnable?.styleSnapshot?.elements?.body?.color ||
      !result.afterEnable?.styleSnapshot?.elements?.pageHeader?.backgroundColor ||
      result.afterReload?.cookieDarkMode !== '1' ||
      !result.afterReload?.darkReaderReady ||
      !result.afterReload?.iconClass?.includes('fa-moon') ||
      result.afterReload?.styleSnapshot?.capturedCount < 8 ||
      !result.afterReload?.styleSnapshot?.elements?.body?.color ||
      !result.afterReload?.styleSnapshot?.elements?.pageHeader?.backgroundColor)
  ) {
    throw new Error(`dark mode persistence did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-subscribe-drawer' &&
    (result.before?.boxCount !== 0 ||
      result.opened?.boxCount < 1 ||
      (!result.opened?.drawerOpenCount && !result.opened?.modalCount) ||
      !jsonIncludesAny(result.opened?.itemTexts, ['复制订阅地址', 'Copy Subscription URL']) ||
      !jsonIncludesAny(result.opened?.itemTexts, ['扫描二维码订阅', 'Scan QR code to subscribe']) ||
      !JSON.stringify(result.opened?.itemTexts).includes('Hiddify') ||
      !JSON.stringify(result.opened?.itemTexts).includes('Sing-box') ||
      !jsonIncludesAny(result.copied?.messageTexts, ['复制成功', 'Copied successfully']) ||
      result.qr?.qrCanvasCount < 1 ||
      !jsonIncludesAny(result.qr?.qrTipTexts, [
        '使用支持扫码的客户端进行订阅',
        'Use a client app that supports scanning QR code to subscribe',
      ]))
  ) {
    throw new Error(`dashboard subscribe drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-subscribe-import-links' &&
    (result.before?.boxCount !== 0 ||
      result.opened?.boxCount < 1 ||
      (!result.opened?.drawerOpenCount && !result.opened?.modalCount) ||
      result.opened?.items?.length < 4 ||
      !jsonIncludesAny(result.opened?.itemTexts, ['复制订阅地址', 'Copy Subscription URL']) ||
      !jsonIncludesAny(result.opened?.itemTexts, ['扫描二维码订阅', 'Scan QR code to subscribe']) ||
      !JSON.stringify(result.opened?.itemTexts).includes('Hiddify') ||
      !JSON.stringify(result.opened?.itemTexts).includes('Sing-box') ||
      !JSON.stringify(result.opened?.itemClasses).includes('subsrcibe-for-link') ||
      !JSON.stringify(result.opened?.itemClasses).includes('subscribe-for-qrcode') ||
      !JSON.stringify(result.opened?.itemClasses).includes('hiddify') ||
      !JSON.stringify(result.opened?.itemClasses).includes('sing-box') ||
      !result.opened?.items?.some(
        (item) => item.className?.includes('hiddify') && item.imageCount >= 1,
      ) ||
      !result.opened?.items?.some(
        (item) => item.className?.includes('sing-box') && item.imageCount >= 1,
      ))
  ) {
    throw new Error(`dashboard subscribe import links did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-notice-carousel' &&
    (result.before?.dotCount < 2 ||
      result.afterDot?.activeDotIndex !== 1 ||
      result.opened?.modalCount < 1 ||
      !jsonIncludesAny(result.opened?.modalTitles, ['Notice A', 'Notice B']) ||
      !jsonIncludesAny(result.opened?.modalBodies, ['Visual parity notice', 'Second notice']) ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`dashboard notice carousel did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-reset-package-confirm' &&
    (result.before?.resetTriggerCount < 1 ||
      result.opened?.modalCount < 1 ||
      !result.opened?.title?.length ||
      !result.opened?.content?.length ||
      result.opened?.buttons?.length < 2 ||
      result.confirmed?.modalCount !== 0 ||
      result.orderSaveRequests?.length !== 1 ||
      Number(result.orderSaveRequests?.[0]?.plan_id) !== 1 ||
      result.orderSaveRequests?.[0]?.period !== 'reset_price' ||
      !result.hash?.includes(`/order/${dashboardResetPackageTradeNo}`) ||
      !jsonIncludesAny(result.orderInfo, [dashboardResetPackageTradeNo]))
  ) {
    throw new Error(
      `dashboard reset package confirm did not match legacy save-order behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-dashboard-new-period-confirm' &&
    (result.before?.newPeriodTriggerCount < 1 ||
      result.opened?.modalCount < 1 ||
      !result.opened?.title?.length ||
      !result.opened?.content?.length ||
      result.opened?.buttons?.length < 2 ||
      result.confirmed?.modalCount !== 0 ||
      result.newPeriodRequests?.length !== 1 ||
      result.subscribeFetchDelta < 1 ||
      !result.hash?.includes('/dashboard') ||
      !jsonIncludesAny(result.confirmed?.toastTexts, ['提前开启流量周期成功']))
  ) {
    throw new Error(`dashboard new-period confirm did not match legacy behavior: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-alert-links' &&
    (result.before?.alertLinks?.length < 2 ||
      !jsonIncludesAny(result.before?.alertLinks, ['立即支付', 'Pay Now']) ||
      !jsonIncludesAny(result.before?.alertLinks, ['立即查看', 'View Now']) ||
      !result.order?.hash?.includes('/order') ||
      !jsonIncludesAny(result.order?.containerTitles, ['我的订单', 'My Orders']) ||
      !jsonIncludesAny(result.order?.tableHeaders, ['# 订单号', 'Order Number #']) ||
      !result.reset?.hash?.includes('/dashboard') ||
      result.reset?.alertLinks?.length < 2 ||
      !result.ticket?.hash?.includes('/ticket') ||
      !jsonIncludesAny(result.ticket?.containerTitles, ['我的工单', 'My Tickets']) ||
      !jsonIncludesAny(result.ticket?.blockTitles, ['工单历史', 'Ticket History']) ||
      !jsonIncludesAny(result.ticket?.tableHeaders, ['工单状态', 'Ticket Status']))
  ) {
    throw new Error(`dashboard alert links did not route like legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-profile-deposit-modal' &&
    (result.filled?.amount !== '12.34' ||
      !result.filled?.modalCount ||
      result.filled?.buttons?.length < 2 ||
      result.orderSaveRequests?.length !== 1 ||
      Number(result.orderSaveRequests?.[0]?.plan_id) !== 0 ||
      result.orderSaveRequests?.[0]?.period !== 'deposit' ||
      Number(result.orderSaveRequests?.[0]?.deposit_amount) !== 1234 ||
      !result.hash?.includes(`/order/${profileDepositTradeNo}`) ||
      !jsonIncludesAny(result.orderInfo, [profileDepositTradeNo]))
  ) {
    throw new Error(
      `profile deposit modal did not match legacy save-order behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-reset-subscribe-confirm' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['重置订阅信息', 'Reset Subscription']) ||
      !jsonIncludesAny(result.before?.warningTexts, ['订阅', 'subscription']) ||
      !jsonIncludesAny(result.before?.resetButtons, ['重置', 'Reset']) ||
      result.opened?.modalCount < 1 ||
      !jsonIncludesAny(result.opened?.title, [
        '确定要重置订阅信息？',
        'Do you want to reset subscription?',
      ]) ||
      !jsonIncludesAny(result.opened?.content, ['UUID', 're-subscribe', '重新导入订阅']) ||
      result.opened?.buttons?.length < 2 ||
      result.confirmed?.modalCount !== 0 ||
      result.confirmed?.resetCount < 1 ||
      !jsonIncludesAny(result.confirmed?.toastTexts, ['重置成功', 'Reset successfully']) ||
      result.infoFetchDelta !== 0 ||
      result.subscribeFetchDelta !== 0)
  ) {
    throw new Error(
      `profile reset subscribe confirm did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-telegram-bind-modal' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['绑定 Telegram', 'Link to Telegram']) ||
      !jsonIncludesAny(result.before?.startButtons, ['立即开始', 'Start Now']) ||
      !jsonIncludesAny(result.before?.discussionLinks, ['https://t.me/visual_discuss']) ||
      result.opened?.modalCount < 1 ||
      !jsonIncludesAny(result.opened?.modalTitles, ['绑定 Telegram', 'Link to Telegram']) ||
      !jsonIncludesAny(result.opened?.modalBodies, ['@legacy_bot']) ||
      !jsonIncludesAny(result.opened?.modalBodies, ['First Step', '第一步']) ||
      !jsonIncludesAny(result.opened?.modalBodies, ['Second Step', '第二步']) ||
      !jsonIncludesAny(result.opened?.modalCode, ['/bind']) ||
      result.copied?.copyCommandCount < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `profile telegram bind modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-telegram-unbind-confirm' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['绑定 Telegram', 'Link to Telegram']) ||
      !jsonIncludesAny(result.before?.telegramIdTexts, ['Telegram ID: 12345']) ||
      !jsonIncludesAny(result.before?.unbindButtons, ['解除绑定']) ||
      result.opened?.modalCount < 1 ||
      !jsonIncludesAny(result.opened?.modalTitle, ['确定要解除绑定Telegram？']) ||
      !jsonIncludesAny(result.opened?.modalContent, [
        'Telegram ID',
        '重新进行绑定',
        're-bind',
      ]) ||
      result.opened?.buttons?.length < 2 ||
      result.confirmed?.modalCount !== 0 ||
      result.confirmed?.unbindCount < 1 ||
      result.infoFetchDelta < 1 ||
      result.subscribeFetchDelta < 1 ||
      !jsonIncludesAny(result.confirmed?.toastTexts, ['重置成功', 'Reset successfully']))
  ) {
    throw new Error(
      `profile telegram unbind confirm did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-preference-switches' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['通知', 'Notification']) ||
      !jsonIncludesAny(result.before?.labels, ['自动续费', 'Auto Renewal']) ||
      !jsonIncludesAny(result.before?.labels, [
        '到期邮件提醒',
        'Subscription expiration email reminder',
      ]) ||
      !jsonIncludesAny(result.before?.labels, [
        '流量邮件提醒',
        'Insufficient transfer data email alert',
      ]) ||
      result.before?.switchCount !== 3 ||
      result.before?.switches?.[0]?.checked !== false ||
      result.before?.switches?.[1]?.checked !== true ||
      result.before?.switches?.[2]?.checked !== true ||
      result.toggles?.length !== 3 ||
      result.toggles?.some((toggle) => !toggle.loadingSwitch?.loading) ||
      result.toggles?.some((toggle) => !toggle.loadingSwitch?.disabled) ||
      result.after?.updateRequests?.length !== 3 ||
      Number(result.after?.updateRequests?.[0]?.auto_renewal) !== 1 ||
      Number(result.after?.updateRequests?.[1]?.remind_expire) !== 0 ||
      Number(result.after?.updateRequests?.[2]?.remind_traffic) !== 0 ||
      result.infoFetchDelta < 3 ||
      result.after?.switches?.[0]?.checked !== false ||
      result.after?.switches?.[1]?.checked !== true ||
      result.after?.switches?.[2]?.checked !== true)
  ) {
    throw new Error(
      `profile preference switches did not match legacy update behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-redeem-giftcard' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['礼品卡', 'Gift Card']) ||
      !jsonIncludesAny(result.before?.redeemButton?.text, ['兑换', 'Redeem']) ||
      result.filled?.inputValue !== 'CARD-123' ||
      !result.loading?.redeemButton?.loading ||
      result.after?.redeemRequests?.length !== 1 ||
      result.after?.redeemRequests?.[0]?.giftcard !== 'CARD-123' ||
      result.infoFetchDelta < 1 ||
      !jsonIncludesAny(result.after?.toastTexts, ['兑换成功: 账户余额 12.34']))
  ) {
    throw new Error(
      `profile redeem giftcard did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-change-password-success' &&
    (!jsonIncludesAny(result.before?.blockTitles, ['修改密码', 'Change Password']) ||
      !jsonIncludesAny(result.before?.saveButton?.text, ['保存', 'Save']) ||
      result.filled?.passwordInputs?.[0]?.value !== 'old-password' ||
      result.filled?.passwordInputs?.[1]?.value !== 'new-password' ||
      result.filled?.passwordInputs?.[2]?.value !== 'new-password' ||
      !result.loading?.saveButton?.loading ||
      result.after?.changePasswordRequests?.length !== 1 ||
      result.after?.changePasswordRequests?.[0]?.old_password !== 'old-password' ||
      result.after?.changePasswordRequests?.[0]?.new_password !== 'new-password' ||
      !jsonIncludesAny(result.after?.toastTexts, ['修改成功，请重新登陆']) ||
      (!result.after?.hash?.includes('/login') && !result.after?.hash?.includes('/dashboard')) ||
      (result.after?.hash?.includes('/login') && result.after?.authBoxCount < 1) ||
      result.after?.localAuthPresent !== true)
  ) {
    throw new Error(
      `profile change password did not match legacy success behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-plans-filter-tabs' &&
    (result.before?.activeIndex !== 0 ||
      result.period?.activeIndex !== 1 ||
      result.traffic?.activeIndex !== 2 ||
      result.before?.cardCount < 2 ||
      result.period?.cardCount < 1 ||
      result.traffic?.cardCount < 1 ||
      stableJson(result.period?.cardTitles) === stableJson(result.traffic?.cardTitles))
  ) {
    throw new Error(`plan filter tabs did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-plan-checkout-coupon' &&
    (result.couponInput !== couponCheckFixture.code ||
      result.selectCount < 1 ||
      !result.activePeriods?.length ||
      !JSON.stringify(result.summaryBlocks).includes(couponCheckFixture.name))
  ) {
    throw new Error(`plan checkout coupon did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-order-payment-method' &&
    (result.before?.activeIndex !== 0 ||
      result.after?.activeIndex !== 2 ||
      result.before?.methodTexts?.length < 3 ||
      !JSON.stringify(result.after?.methodTexts).includes('Fee Pay') ||
      !JSON.stringify(result.after?.summaryBlocks).includes('1.00') ||
      !JSON.stringify(result.after?.summaryBlocks).includes('10.90'))
  ) {
    throw new Error(`order payment method did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-order-qr-checkout' &&
    (result.before?.activeIndex !== 0 ||
      result.loading?.submitButton?.disabled !== true ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method) !== 1 ||
      result.opened?.modalCount < 1 ||
      result.opened?.qrSvgCount + result.opened?.qrCanvasCount < 1 ||
      !jsonIncludesAny(result.opened?.modalTexts, ['等待支付中', 'Waiting for payment']))
  ) {
    throw new Error(`order QR checkout did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-order-stripe-disabled-checkout' &&
    (result.before?.activeIndex !== 0 ||
      result.selected?.activeIndex !== 1 ||
      result.selected?.stripePublicKeyCount < 1 ||
      !jsonIncludesAny(result.selected?.creditCardTexts, ['信用卡', 'credit card']) ||
      result.selected?.submitButton?.disabled !== true ||
      result.checkoutRequests?.length !== 0 ||
      !jsonIncludesAny(result.selected?.methodTexts, ['Stripe']))
  ) {
    throw new Error(
      `order Stripe disabled checkout did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-redirect-checkout' &&
    (result.selected?.activeIndex !== 2 ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method) !== 3 ||
      !result.redirected?.hash?.includes('cashier=visual'))
  ) {
    throw new Error(
      `order redirect checkout did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-node-table-scroll' &&
    (!JSON.stringify(result.before?.rows).includes('Hong Kong 01') ||
      !JSON.stringify(result.before?.rows).includes('Tokyo 02') ||
      !result.before?.className?.includes('ant-table-scroll-position-left') ||
      (result.before?.maxScroll > 0 &&
        (result.afterRight?.scrollLeft <= 0 ||
          !result.afterRight?.className?.includes('ant-table-scroll-position-right') ||
          result.afterMiddle?.scrollLeft <= 0 ||
          !result.afterMiddle?.className?.includes('ant-table-scroll-position-middle'))))
  ) {
    throw new Error(`node table scroll did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    [
      'user-node-tooltips',
      'user-invite-tooltips',
      'admin-payment-notify-tooltip',
      'admin-order-status-tooltips',
    ].includes(label)
  ) {
    const minimumTargets =
      result.viewportWidth >= 600
        ? label === 'admin-order-status-tooltips'
          ? 2
          : 1
        : 0;
    const invalidOpened = (result.opened ?? []).some(
      (item) =>
        item.tooltipCount !== 1 ||
        item.openTriggerCount < 1 ||
        item.placement !== 'top' ||
        !item.texts?.length,
    );
    if (
      result.before?.tooltipCount !== 0 ||
      result.targetCount < minimumTargets ||
      result.opened?.length !== result.targetCount ||
      invalidOpened
    ) {
      throw new Error(`tooltip sequence did not match legacy state: ${JSON.stringify(result)}`);
    }
  }
  if (
    label === 'user-traffic-table-scroll' &&
    (!JSON.stringify(result.before?.rows).includes('512.00 MB') ||
      !JSON.stringify(result.before?.rows).includes('1.50 x') ||
      !result.before?.className?.includes('ant-table-scroll-position-left') ||
      (result.before?.maxScroll > 0 &&
        (result.afterRight?.scrollLeft <= 0 ||
          !result.afterRight?.className?.includes('ant-table-scroll-position-right') ||
          result.afterMiddle?.scrollLeft <= 0 ||
          !result.afterMiddle?.className?.includes('ant-table-scroll-position-middle'))))
  ) {
    throw new Error(`traffic table scroll did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-traffic-total-tooltip' &&
    (result.before?.tooltipCount !== 0 ||
      result.opened?.tooltipCount !== 1 ||
      result.opened?.placement !== 'topRight' ||
      result.opened?.openTriggerCount < 1 ||
      !jsonIncludesAny(result.opened?.texts, ['Formula', 'formula', '公式', '上行']))
  ) {
    throw new Error(`user traffic tooltip did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-knowledge-drawer' &&
    (result.before?.searchValue !== 'router' ||
      result.before?.articleTitles?.length < 2 ||
      result.opened?.drawerOpenCount !== 1 ||
      !JSON.stringify(result.opened?.drawerTitles).includes('Copy Article') ||
      !JSON.stringify(result.opened?.drawerBodies).includes('Copy article body') ||
      result.closed?.drawerOpenCount !== 0)
  ) {
    throw new Error(`knowledge drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-invite-generate' &&
    (!JSON.stringify(result.before?.tableRows).includes('INVITE2026') ||
      !JSON.stringify(result.before?.tableRows).includes('WELCOME') ||
      !JSON.stringify(result.after?.toastTexts).includes('已生成') ||
      result.after?.generateButton?.disabled)
  ) {
    throw new Error(`invite generate did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-invite-transfer-modal' &&
    (result.before?.modalCount !== 0 ||
      result.opened?.modalCount !== 1 ||
      !jsonIncludesAny(result.opened?.titles, [
        '推广佣金划转至余额',
        'Transfer Invitation Commission to Account Balance',
      ]) ||
      !jsonIncludesAny(result.opened?.labels, ['划转金额', 'Transfer amount']) ||
      !JSON.stringify(result.filled?.inputValues).includes('12.34') ||
      result.transferRequests?.length !== 1 ||
      Number(result.transferRequests?.[0]?.transfer_amount) !== 1234 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `invite transfer modal did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (label === 'user-invite-withdraw-modal') {
    if (!result.withdrawRequests?.length) {
      throw new Error(
        `invite withdraw modal did not match legacy behavior: ${JSON.stringify(result)}`,
      );
    }
  }
  if (
    label === 'user-ticket-reply-send' &&
    (result.filled?.inputValue !== 'Parity reply send' ||
      !jsonIncludesAny(result.loading?.toastTexts, ['发送中']) ||
      result.replyRequests?.length !== 1 ||
      String(result.replyRequests?.[0]?.id) !== '7' ||
      result.replyRequests?.[0]?.message !== 'Parity reply send' ||
      result.sent?.inputValue !== '' ||
      !jsonIncludesAny(result.sent?.toastTexts, ['发送成功']) ||
      result.ticketFetchDelta !== 0)
  ) {
    throw new Error(
      `user ticket reply send did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-ticket-reply-send' &&
    (result.filled?.inputValue !== 'Parity admin reply send' ||
      !jsonIncludesAny(result.loading?.toastTexts, ['发送中']) ||
      result.replyRequests?.length !== 1 ||
      String(result.replyRequests?.[0]?.id) !== '7' ||
      result.replyRequests?.[0]?.message !== 'Parity admin reply send' ||
      result.sent?.inputValue !== '' ||
      result.ticketFetchDelta !== 1)
  ) {
    throw new Error(
      `admin ticket reply send did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-ticket-create-submit' &&
    (result.before?.modalCount !== 0 ||
      result.filled?.modalCount !== 1 ||
      result.filled?.titles?.length < 1 ||
      result.filled?.labels?.length < 3 ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity subject') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity ticket body') ||
      result.levelDropdown?.selectDropdownItems?.length < 3 ||
      result.filled?.selectedValues?.length < 1 ||
      result.filled?.buttons?.length < 2 ||
      result.saving?.modalCount !== 1 ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.subject !== 'Parity subject' ||
      Number(result.saveRequests?.[0]?.level) !== 2 ||
      result.saveRequests?.[0]?.message !== 'Parity ticket body' ||
      result.saved?.modalCount !== 0)
  ) {
    throw new Error(
      `user ticket create submit did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-tickets-reply-filter' &&
    (result.before?.dropdownCount !== 0 ||
      result.opened?.dropdownCount !== 1 ||
      !JSON.stringify(result.opened?.filterItems).includes('已回复') ||
      !JSON.stringify(result.opened?.filterItems).includes('待回复') ||
      !result.selected?.filterItems?.some((item) => item.text === '待回复' && item.checked) ||
      result.confirmed?.dropdownCount !== 0 ||
      !JSON.stringify(result.confirmed?.tableReplyStatusTexts).includes('待回复'))
  ) {
    throw new Error(`admin ticket reply filter did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-order-cancel-confirm' &&
    (result.cancelLinks
      ? (result.opened?.modalCount < 1 ||
          !jsonIncludesAny(result.opened?.title, ['注意', 'Attention']) ||
          !jsonIncludesAny(result.opened?.content, ['取消订单', 'cancel the order']) ||
          result.opened?.buttons?.length < 2 ||
          result.confirmed?.modalCount !== 0 ||
          result.orderCancelRequests?.length !== 1 ||
          result.orderCancelRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
          typeof result.orderFetchDelta !== 'number')
      : result.listItems < 1 || result.modalCount !== 0)
  ) {
    throw new Error(`order cancel confirm did not match legacy behavior: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-dashboard-commission-shortcut' &&
    (result.before?.alertLinks?.length < 2 ||
      !result.after?.hash?.includes('/order') ||
      !JSON.stringify(result.after?.orderFetchQuery).includes('filter[0][key]') ||
      !JSON.stringify(result.after?.orderFetchQuery).includes('status') ||
      !JSON.stringify(result.after?.orderFetchQuery).includes('commission_status') ||
      !JSON.stringify(result.after?.orderFetchQuery).includes('commission_balance'))
  ) {
    throw new Error(
      `admin dashboard commission shortcut did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-config-tabs' &&
    new Set([result.before?.text, result.second?.text, result.third?.text]).size < 2
  ) {
    throw new Error('admin config tabs did not change active tab');
  }
  if (
    label === 'admin-plan-renew-tooltip' &&
    (result.before?.tooltipCount !== 0 ||
      result.opened?.tooltipCount !== 1 ||
      result.opened?.placement !== 'top' ||
      result.opened?.openTriggerCount < 1 ||
      !jsonIncludesAny(result.opened?.texts, ['续费', 'renew']))
  ) {
    throw new Error(`admin plan renew tooltip did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-plan-create-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Pro') ||
      result.filled?.drawerCount !== 1 ||
      !JSON.stringify(result.filled?.titles).includes('新建订阅') ||
      !JSON.stringify(result.filled?.labels).includes('套餐名称') ||
      !JSON.stringify(result.filled?.labels).includes('套餐描述') ||
      !JSON.stringify(result.filled?.labels).includes('月付') ||
      !JSON.stringify(result.filled?.labels).includes('套餐流量') ||
      !JSON.stringify(result.filled?.labels).includes('权限组') ||
      !JSON.stringify(result.filled?.labels).includes('流量重置方式') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity Plan') ||
      !JSON.stringify(result.filled?.inputValues).includes('<p>Parity plan body</p>') ||
      !JSON.stringify(result.filled?.inputValues).includes('12.34') ||
      !JSON.stringify(result.filled?.inputValues).includes('23.45') ||
      !JSON.stringify(result.filled?.inputValues).includes('199.00') ||
      !JSON.stringify(result.filled?.inputValues).includes('250') ||
      !JSON.stringify(result.filled?.inputValues).includes('7') ||
      !JSON.stringify(result.filled?.inputValues).includes('99') ||
      !JSON.stringify(result.filled?.inputValues).includes('50') ||
      !JSON.stringify(result.groupDropdown?.dropdownItems).includes('Default') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('按月重置') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Default') ||
      !JSON.stringify(result.filled?.selectedValues).includes('按月重置') ||
      !JSON.stringify(result.filled?.addonTexts).includes('GB') ||
      !JSON.stringify(result.filled?.addonTexts).includes('Mbps') ||
      !JSON.stringify(result.filled?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.filled?.actionButtons).includes('提 交') ||
      !result.filled?.forceUpdate?.checked ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.name !== 'Parity Plan' ||
      result.saveRequests?.[0]?.content !== '<p>Parity plan body</p>' ||
      String(result.saveRequests?.[0]?.month_price) !== '1234' ||
      String(result.saveRequests?.[0]?.quarter_price) !== '2345' ||
      String(result.saveRequests?.[0]?.onetime_price) !== '19900' ||
      String(result.saveRequests?.[0]?.transfer_enable) !== '250' ||
      String(result.saveRequests?.[0]?.device_limit) !== '7' ||
      String(result.saveRequests?.[0]?.group_id) !== '1' ||
      String(result.saveRequests?.[0]?.reset_traffic_method) !== '1' ||
      String(result.saveRequests?.[0]?.capacity_limit) !== '99' ||
      String(result.saveRequests?.[0]?.speed_limit) !== '50' ||
      result.saveRequests?.[0]?.force_update !== 'true' ||
      result.planFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(`admin plan create drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-plan-create-group-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['Default'])
  ) {
    throw new Error(`admin plan create group select did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-plan-edit-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Pro') ||
      !JSON.stringify(result.menuOpened?.actionDropdownItems).includes('编辑') ||
      !JSON.stringify(result.menuOpened?.actionDropdownItems).includes('删除') ||
      result.opened?.drawerCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑订阅') ||
      !JSON.stringify(result.opened?.labels).includes('套餐名称') ||
      !JSON.stringify(result.opened?.labels).includes('套餐描述') ||
      !JSON.stringify(result.opened?.labels).includes('权限组') ||
      !JSON.stringify(result.opened?.labels).includes('流量重置方式') ||
      !JSON.stringify(result.opened?.inputValues).includes('Pro') ||
      !JSON.stringify(result.opened?.inputValues).includes('<p>Fast nodes</p><p>Support ticket</p>') ||
      !JSON.stringify(result.opened?.inputValues).includes('9.9') ||
      !JSON.stringify(result.opened?.inputValues).includes('24.9') ||
      !JSON.stringify(result.opened?.inputValues).includes('99') ||
      !JSON.stringify(result.opened?.inputValues).includes('1') ||
      !JSON.stringify(result.opened?.inputValues).includes('1000') ||
      !JSON.stringify(result.opened?.inputValues).includes('5') ||
      !JSON.stringify(result.opened?.selectedValues).includes('Default') ||
      !JSON.stringify(result.opened?.selectedValues).includes('每月1号') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('不重置') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Plan') ||
      !JSON.stringify(result.edited?.inputValues).includes('<p>Edited plan body</p>') ||
      !JSON.stringify(result.edited?.inputValues).includes('88.88') ||
      !JSON.stringify(result.edited?.inputValues).includes('300') ||
      !JSON.stringify(result.edited?.inputValues).includes('8') ||
      !JSON.stringify(result.edited?.selectedValues).includes('不重置') ||
      !JSON.stringify(result.edited?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.edited?.actionButtons).includes('提 交') ||
      !result.edited?.forceUpdate?.checked ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.name !== 'Parity Edited Plan' ||
      result.saveRequests?.[0]?.content !== '<p>Edited plan body</p>' ||
      String(result.saveRequests?.[0]?.month_price) !== '8888' ||
      String(result.saveRequests?.[0]?.quarter_price) !== '2490' ||
      String(result.saveRequests?.[0]?.year_price) !== '9900' ||
      String(result.saveRequests?.[0]?.reset_price) !== '100' ||
      String(result.saveRequests?.[0]?.transfer_enable) !== '300' ||
      String(result.saveRequests?.[0]?.device_limit) !== '8' ||
      String(result.saveRequests?.[0]?.group_id) !== '1' ||
      String(result.saveRequests?.[0]?.reset_traffic_method) !== '2' ||
      result.saveRequests?.[0]?.force_update !== 'true' ||
      result.planFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(`admin plan edit drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-theme-settings-modal' &&
    (result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('配置默认主题') ||
      !JSON.stringify(result.opened?.labels).includes('首页标题') ||
      !JSON.stringify(result.opened?.inputValues).includes('Parity Theme Title') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin theme modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-payment-create-modal' &&
    (result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('添加支付方式') ||
      !JSON.stringify(result.opened?.labels).includes('显示名称') ||
      !JSON.stringify(result.opened?.labels).includes('接口文件') ||
      !JSON.stringify(result.opened?.labels).includes('商户ID') ||
      !JSON.stringify(result.opened?.inputValues).includes('Parity Pay') ||
      !JSON.stringify(result.opened?.selectedPayment).includes('AlipayF2F') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('StripeCheckout') ||
      !JSON.stringify(result.switched?.selectedPayment).includes('StripeCheckout') ||
      !JSON.stringify(result.switched?.labels).includes('Secret Key') ||
      !JSON.stringify(result.switched?.inputValues).includes('pk_parity_create') ||
      !JSON.stringify(result.switched?.inputValues).includes('sk_parity_create') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.name !== 'Parity Pay' ||
      result.saveRequests?.[0]?.payment !== 'StripeCheckout' ||
      result.saveRequests?.[0]?.['config[publishable_key]'] !== 'pk_parity_create' ||
      result.saveRequests?.[0]?.['config[secret_key]'] !== 'sk_parity_create' ||
      result.paymentFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin payment modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-payment-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Alipay') ||
      !JSON.stringify(result.before?.tableRows).includes('Stripe') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑支付方式') ||
      !JSON.stringify(result.opened?.labels).includes('显示名称') ||
      !JSON.stringify(result.opened?.labels).includes('接口文件') ||
      !JSON.stringify(result.opened?.labels).includes('商户ID') ||
      !JSON.stringify(result.opened?.labels).includes('密钥') ||
      !JSON.stringify(result.opened?.inputValues).includes('Alipay') ||
      !JSON.stringify(result.opened?.inputValues).includes('visual-secret') ||
      !JSON.stringify(result.opened?.inputValues).includes('visual-merchant') ||
      !JSON.stringify(result.opened?.selectedPayment).includes('AlipayF2F') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('保 存') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Pay') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-secret') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-merchant') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.name !== 'Parity Edited Pay' ||
      result.saveRequests?.[0]?.payment !== 'AlipayF2F' ||
      result.saveRequests?.[0]?.['config[key]'] !== 'edited-secret' ||
      result.saveRequests?.[0]?.['config[mch_id]'] !== 'edited-merchant' ||
      result.paymentFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin payment edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-create-node-drawer' &&
    (result.before?.drawerCount !== 0 ||
      result.menuOpened?.dropdownCount !== 1 ||
      !JSON.stringify(result.menuOpened?.dropdownItems).includes('Shadowsocks') ||
      !JSON.stringify(result.menuOpened?.dropdownItems).includes('VMess') ||
      result.drawerOpened?.drawerCount !== 1 ||
      !JSON.stringify(result.drawerOpened?.titles).includes('新建节点') ||
      !JSON.stringify(result.drawerOpened?.labels).includes('节点名称') ||
      !JSON.stringify(result.drawerOpened?.labels).includes('倍率') ||
      !JSON.stringify(result.drawerOpened?.labels).includes('权限组') ||
      !JSON.stringify(result.drawerOpened?.labels).includes('节点地址') ||
      !JSON.stringify(result.drawerOpened?.labels).includes('连接端口') ||
      !JSON.stringify(result.drawerOpened?.inputValues).includes('Parity Node') ||
      !JSON.stringify(result.drawerOpened?.inputValues).includes('1.5') ||
      !JSON.stringify(result.groupDropdown?.selectDropdownItems).includes('Default') ||
      !JSON.stringify(result.groupSelected?.selectedValues).includes('Default') ||
      !JSON.stringify(result.groupSelected?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.groupSelected?.actionButtons).includes('提 交') ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(`admin server node drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-server-edit-node-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Tokyo 01') ||
      !JSON.stringify(result.before?.tableRows).includes('jp.example.com:443') ||
      result.opened?.drawerCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑节点') ||
      !JSON.stringify(result.opened?.labels).includes('节点名称') ||
      !JSON.stringify(result.opened?.labels).includes('倍率') ||
      !JSON.stringify(result.opened?.labels).includes('权限组') ||
      !JSON.stringify(result.opened?.labels).includes('节点地址') ||
      !JSON.stringify(result.opened?.labels).includes('连接端口') ||
      !JSON.stringify(result.opened?.inputValues).includes('Tokyo 01') ||
      !JSON.stringify(result.opened?.inputValues).includes('1.0') ||
      !JSON.stringify(result.opened?.inputValues).includes('jp.example.com') ||
      !JSON.stringify(result.opened?.inputValues).includes('443') ||
      !JSON.stringify(result.opened?.inputValues).includes('8388') ||
      !JSON.stringify(result.opened?.selectedValues).includes('Default') ||
      !JSON.stringify(result.opened?.selectedValues).includes('1') ||
      !JSON.stringify(result.opened?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.opened?.actionButtons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Node') ||
      !JSON.stringify(result.edited?.inputValues).includes('2.25') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-node.example.test') ||
      !JSON.stringify(result.edited?.inputValues).includes('9443') ||
      !JSON.stringify(result.edited?.inputValues).includes('18388') ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin server edit node drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-route-create-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.pageButtons).includes('添加路由') ||
      !JSON.stringify(result.before?.tableRows).includes('Block ads') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('创建路由') ||
      !JSON.stringify(result.opened?.labels).includes('备注') ||
      !JSON.stringify(result.opened?.labels).includes('匹配值') ||
      !JSON.stringify(result.opened?.labels).includes('动作') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.actionDropdown?.dropdownItems).includes('指定DNS服务器进行解析') ||
      !JSON.stringify(result.edited?.labels).includes('DNS服务器') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Created Route') ||
      !JSON.stringify(result.edited?.inputValues).includes('domain:created.example.com') ||
      !JSON.stringify(result.edited?.inputValues).includes('geosite:created') ||
      !JSON.stringify(result.edited?.inputValues).includes('9.9.9.9') ||
      !JSON.stringify(result.edited?.selectedValues).includes('指定DNS服务器进行解析') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin server route create modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-route-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Block ads') ||
      !JSON.stringify(result.before?.tableRows).includes('匹配 2 条规则') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑路由') ||
      !JSON.stringify(result.opened?.labels).includes('备注') ||
      !JSON.stringify(result.opened?.labels).includes('匹配值') ||
      !JSON.stringify(result.opened?.labels).includes('动作') ||
      !JSON.stringify(result.opened?.inputValues).includes('Block ads') ||
      !JSON.stringify(result.opened?.inputValues).includes('domain:example.com') ||
      !JSON.stringify(result.opened?.selectedValues).includes('禁止访问(域名目标)') ||
      !JSON.stringify(result.actionDropdown?.dropdownItems).includes('指定DNS服务器进行解析') ||
      !JSON.stringify(result.edited?.labels).includes('DNS服务器') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Route') ||
      !JSON.stringify(result.edited?.inputValues).includes('domain:edited.example.com') ||
      !JSON.stringify(result.edited?.inputValues).includes('geosite:openai') ||
      !JSON.stringify(result.edited?.inputValues).includes('1.1.1.1') ||
      !JSON.stringify(result.edited?.selectedValues).includes('指定DNS服务器进行解析') ||
      !JSON.stringify(result.edited?.buttons).includes('取 消') ||
      !JSON.stringify(result.edited?.buttons).includes('提 交') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin server route edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-group-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Default') ||
      !JSON.stringify(result.before?.tableRows).includes('编辑') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑组') ||
      !JSON.stringify(result.opened?.labels).includes('组名') ||
      !JSON.stringify(result.opened?.inputValues).includes('Default') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Group') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.name !== 'Parity Edited Group' ||
      result.groupFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin server group edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-group-create-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Default') ||
      !JSON.stringify(result.before?.pageButtons).includes('添加权限组') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('创建组') ||
      !JSON.stringify(result.opened?.labels).includes('组名') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Created Group') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.name !== 'Parity Created Group' ||
      result.groupFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin server group create modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-order-detail-modal' &&
    (result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('订单信息') ||
      !JSON.stringify(result.opened?.bodyRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.opened?.bodyRows).includes('VISUAL2026110001') ||
      !JSON.stringify(result.opened?.bodyRows).includes('订单周期月付') ||
      !JSON.stringify(result.opened?.bodyRows).includes('支付金额9.90') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin order detail modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-order-assign-modal' &&
    (result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('订单分配') ||
      !JSON.stringify(result.opened?.labels).includes('用户邮箱') ||
      !JSON.stringify(result.opened?.labels).includes('请选择订阅') ||
      !JSON.stringify(result.opened?.labels).includes('请选择周期') ||
      !JSON.stringify(result.filled?.inputValues).includes('assign-user@example.com') ||
      !JSON.stringify(result.filled?.inputValues).includes('12.34') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Pro') ||
      !JSON.stringify(result.filled?.selectedValues).includes('月付') ||
      result.assignRequest?.email !== 'assign-user@example.com' ||
      String(result.assignRequest?.plan_id) !== '1' ||
      result.assignRequest?.period !== 'month_price' ||
      String(result.assignRequest?.total_amount) !== '1234' ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin order assign modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-order-status-dropdown' &&
    (!JSON.stringify(result.before?.orderRows).includes('VIS...001') ||
      !JSON.stringify(result.before?.triggerTexts).includes('标记为') ||
      result.opened?.dropdownCount !== 1 ||
      !JSON.stringify(result.opened?.dropdownItems).includes('已支付') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('取消') ||
      result.paidRequest?.trade_no !== 'VISUAL2026110001' ||
      result.closed?.dropdownCount !== 0)
  ) {
    throw new Error(`admin order status dropdown did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-order-commission-dropdown' &&
    (!JSON.stringify(result.before?.orderRows).includes('VIS...002') ||
      !JSON.stringify(result.before?.orderRows).includes('发放中') ||
      !JSON.stringify(result.before?.triggerTexts).includes('标记为') ||
      result.opened?.dropdownCount !== 1 ||
      !JSON.stringify(result.opened?.dropdownItems).includes('待确认') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('有效') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('无效') ||
      result.updateRequest?.trade_no !== 'VISUAL2026110002' ||
      String(result.updateRequest?.commission_status) !== '3' ||
      result.closed?.dropdownCount !== 0)
  ) {
    throw new Error(`admin order commission dropdown did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-coupon-create-modal' &&
    (result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('新建优惠券') ||
      !JSON.stringify(result.opened?.labels).includes('优惠信息') ||
      !JSON.stringify(result.opened?.inputValues).includes('Parity Coupon') ||
      !JSON.stringify(result.opened?.inputValues).includes('PARITY2026') ||
      result.generateRequests?.length !== 1 ||
      result.generateRequests?.[0]?.name !== 'Parity Coupon' ||
      result.generateRequests?.[0]?.code !== 'PARITY2026' ||
      String(result.generateRequests?.[0]?.type) !== '1' ||
      String(result.generateRequests?.[0]?.value) !== '2500' ||
      result.couponFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin coupon modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-coupon-range-picker' &&
    (result.before?.modalCount !== 1 ||
      result.before?.popupCount !== 0 ||
      result.opened?.popupCount !== 1 ||
      !result.opened?.popupClass?.includes('ant-calendar-picker-container-placement-bottomLeft') ||
      !result.opened?.calendarClass?.includes('ant-calendar-range') ||
      !result.opened?.calendarClass?.includes('ant-calendar-time') ||
      !JSON.stringify(result.opened?.pickerInputPlaceholders).includes('Start Time') ||
      !JSON.stringify(result.opened?.pickerInputPlaceholders).includes('End Time') ||
      !JSON.stringify(result.opened?.popupInputPlaceholders).includes('Start Time') ||
      !JSON.stringify(result.opened?.popupInputPlaceholders).includes('End Time') ||
      !JSON.stringify(result.opened?.footerTexts).includes('选择时间') ||
      !JSON.stringify(result.opened?.footerTexts).includes('确 定'))
  ) {
    throw new Error(`admin coupon range picker did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-coupon-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Visual Amount') ||
      !JSON.stringify(result.before?.tableRows).includes('VISUAL100') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑优惠券') ||
      !JSON.stringify(result.opened?.labels).includes('名称') ||
      !JSON.stringify(result.opened?.labels).includes('自定义优惠券码') ||
      !JSON.stringify(result.opened?.labels).includes('优惠信息') ||
      !JSON.stringify(result.opened?.labels).includes('指定订阅') ||
      !JSON.stringify(result.opened?.labels).includes('指定周期') ||
      !JSON.stringify(result.opened?.inputValues).includes('Visual Amount') ||
      !JSON.stringify(result.opened?.inputValues).includes('VISUAL100') ||
      !JSON.stringify(result.opened?.inputValues).includes('10') ||
      !JSON.stringify(result.opened?.addonTexts).includes('¥') ||
      !JSON.stringify(result.opened?.selectedValues).includes('按金额优惠') ||
      !JSON.stringify(result.opened?.selectedValues).includes('月付') ||
      !JSON.stringify(result.opened?.selectedValues).includes('年付') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Coupon') ||
      !JSON.stringify(result.edited?.inputValues).includes('EDIT2026') ||
      !JSON.stringify(result.edited?.inputValues).includes('12.5') ||
      result.generateRequests?.length !== 1 ||
      String(result.generateRequests?.[0]?.id) !== '1' ||
      result.generateRequests?.[0]?.name !== 'Parity Edited Coupon' ||
      result.generateRequests?.[0]?.code !== 'EDIT2026' ||
      String(result.generateRequests?.[0]?.type) !== '1' ||
      String(result.generateRequests?.[0]?.value) !== '1250' ||
      result.couponFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin coupon edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-giftcard-create-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Balance Gift') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('新建礼品卡') ||
      !JSON.stringify(result.opened?.labels).includes('名称') ||
      !JSON.stringify(result.opened?.labels).includes('自定义礼品卡卡密') ||
      !JSON.stringify(result.opened?.labels).includes('礼品卡类型') ||
      !JSON.stringify(result.opened?.inputValues).includes('Parity Giftcard') ||
      !JSON.stringify(result.opened?.inputValues).includes('GIFT2026') ||
      !JSON.stringify(result.typeDropdown?.dropdownItems).includes('套餐') ||
      !JSON.stringify(result.planDropdown?.dropdownItems).includes('Pro') ||
      result.filled?.modalCount !== 1 ||
      !JSON.stringify(result.filled?.labels).includes('指定订阅') ||
      !JSON.stringify(result.filled?.labels).includes('最大使用次数') ||
      !JSON.stringify(result.filled?.selectedValues).includes('套餐') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Pro') ||
      !JSON.stringify(result.filled?.addonTexts).includes('天') ||
      !JSON.stringify(result.filled?.inputValues).includes('0') ||
      !JSON.stringify(result.filled?.inputValues).includes('9') ||
      !JSON.stringify(result.filled?.buttons).includes('取 消') ||
      !JSON.stringify(result.filled?.buttons).includes('提 交') ||
      result.generateRequests?.length !== 1 ||
      result.generateRequests?.[0]?.name !== 'Parity Giftcard' ||
      result.generateRequests?.[0]?.code !== 'GIFT2026' ||
      String(result.generateRequests?.[0]?.type) !== '5' ||
      String(result.generateRequests?.[0]?.value) !== '0' ||
      String(result.generateRequests?.[0]?.plan_id) !== '1' ||
      String(result.generateRequests?.[0]?.limit_use) !== '9' ||
      result.giftcardFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin giftcard modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-giftcard-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Plan Gift') ||
      !JSON.stringify(result.before?.tableRows).includes('GC-VISUAL-PLAN') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑礼品卡') ||
      !JSON.stringify(result.opened?.labels).includes('名称') ||
      !JSON.stringify(result.opened?.labels).includes('自定义礼品卡卡密') ||
      !JSON.stringify(result.opened?.labels).includes('礼品卡类型') ||
      !JSON.stringify(result.opened?.labels).includes('指定订阅') ||
      !JSON.stringify(result.opened?.labels).includes('最大使用次数') ||
      !JSON.stringify(result.opened?.inputValues).includes('Plan Gift') ||
      !JSON.stringify(result.opened?.inputValues).includes('GC-VISUAL-PLAN') ||
      !JSON.stringify(result.opened?.inputValues).includes('30') ||
      !JSON.stringify(result.opened?.selectedValues).includes('兑换订阅套餐') ||
      !JSON.stringify(result.opened?.addonTexts).includes('天') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Giftcard') ||
      !JSON.stringify(result.edited?.inputValues).includes('EDIT-GIFT-2026') ||
      !JSON.stringify(result.edited?.inputValues).includes('45') ||
      !JSON.stringify(result.edited?.inputValues).includes('4') ||
      result.generateRequests?.length !== 1 ||
      String(result.generateRequests?.[0]?.id) !== '2' ||
      result.generateRequests?.[0]?.name !== 'Parity Edited Giftcard' ||
      result.generateRequests?.[0]?.code !== 'EDIT-GIFT-2026' ||
      String(result.generateRequests?.[0]?.type) !== '5' ||
      String(result.generateRequests?.[0]?.value) !== '45' ||
      String(result.generateRequests?.[0]?.plan_id) !== '1' ||
      String(result.generateRequests?.[0]?.limit_use) !== '4' ||
      result.giftcardFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin giftcard edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-notice-create-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Notice A') ||
      result.filled?.modalCount !== 1 ||
      !JSON.stringify(result.filled?.titles).includes('新建公告') ||
      !JSON.stringify(result.filled?.labels).includes('标题') ||
      !JSON.stringify(result.filled?.labels).includes('公告内容') ||
      !JSON.stringify(result.filled?.labels).includes('公告标签') ||
      !JSON.stringify(result.filled?.labels).includes('图片URL') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity Notice') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity notice body') ||
      !JSON.stringify(result.filled?.inputValues).includes('https://example.test/notice.png') ||
      !JSON.stringify(result.filled?.choiceTexts).includes('ops') ||
      !JSON.stringify(result.filled?.buttons).includes('取 消') ||
      !JSON.stringify(result.filled?.buttons).includes('提 交') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.title !== 'Parity Notice' ||
      result.saveRequests?.[0]?.content !== 'Parity notice body' ||
      result.saveRequests?.[0]?.['tags[0]'] !== 'ops' ||
      result.saveRequests?.[0]?.img_url !== 'https://example.test/notice.png' ||
      result.noticeFetchDelta < 1 ||
      result.closed?.modalCount !== 0 ||
      result.reopened?.modalCount !== 1 ||
      JSON.stringify(result.reopened?.inputValues).includes('Parity Notice') ||
      JSON.stringify(result.reopened?.inputValues).includes('Parity notice body') ||
      JSON.stringify(result.reopened?.choiceTexts).includes('ops'))
  ) {
    throw new Error(`admin notice modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-notice-edit-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Hidden Notice') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑公告') ||
      !JSON.stringify(result.opened?.labels).includes('标题') ||
      !JSON.stringify(result.opened?.labels).includes('公告内容') ||
      !JSON.stringify(result.opened?.labels).includes('公告标签') ||
      !JSON.stringify(result.opened?.labels).includes('图片URL') ||
      !JSON.stringify(result.opened?.inputValues).includes('Hidden Notice') ||
      !JSON.stringify(result.opened?.inputValues).includes('<p>Second notice</p>') ||
      !JSON.stringify(result.opened?.choiceTexts).includes('ops') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Notice') ||
      !JSON.stringify(result.edited?.inputValues).includes('<p>Parity edited notice body</p>') ||
      !JSON.stringify(result.edited?.inputValues).includes('https://example.test/notice-edited.png') ||
      !JSON.stringify(result.edited?.choiceTexts).includes('ops') ||
      !JSON.stringify(result.edited?.choiceTexts).includes('edited') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '2' ||
      result.saveRequests?.[0]?.title !== 'Parity Edited Notice' ||
      result.saveRequests?.[0]?.content !== '<p>Parity edited notice body</p>' ||
      result.saveRequests?.[0]?.['tags[0]'] !== 'ops' ||
      result.saveRequests?.[0]?.['tags[1]'] !== 'edited' ||
      result.saveRequests?.[0]?.img_url !== 'https://example.test/notice-edited.png' ||
      result.noticeFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin notice edit modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-knowledge-create-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Copy Article') ||
      result.filled?.drawerCount !== 1 ||
      !JSON.stringify(result.filled?.titles).includes('新增知识') ||
      !JSON.stringify(result.filled?.labels).includes('标题') ||
      !JSON.stringify(result.filled?.labels).includes('分类') ||
      !JSON.stringify(result.filled?.labels).includes('语言') ||
      !JSON.stringify(result.filled?.labels).includes('内容') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity Knowledge') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity') ||
      !JSON.stringify(result.languageDropdown?.dropdownItems).includes('English') ||
      !JSON.stringify(result.filled?.selectedValues).includes('English') ||
      !String(result.filled?.markdownValue).includes('Parity body') ||
      !JSON.stringify(result.filled?.previewTexts).includes('Parity Knowledge') ||
      !JSON.stringify(result.filled?.previewTexts).includes('Parity body') ||
      !JSON.stringify(result.filled?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.filled?.actionButtons).includes('提 交') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.title !== 'Parity Knowledge' ||
      result.saveRequests?.[0]?.category !== 'Parity' ||
      result.saveRequests?.[0]?.language !== 'en-US' ||
      !String(result.saveRequests?.[0]?.body).includes('Parity body') ||
      result.knowledgeFetchDelta < 1 ||
      result.saved?.drawerCount !== 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(`admin knowledge drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-knowledge-edit-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Copy Article') ||
      result.opened?.drawerCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑知识') ||
      !JSON.stringify(result.opened?.labels).includes('标题') ||
      !JSON.stringify(result.opened?.labels).includes('分类') ||
      !JSON.stringify(result.opened?.labels).includes('语言') ||
      !JSON.stringify(result.opened?.labels).includes('内容') ||
      !JSON.stringify(result.opened?.inputValues).includes('Copy Article') ||
      !JSON.stringify(result.opened?.inputValues).includes('General') ||
      !JSON.stringify(result.opened?.selectedValues).includes('English') ||
      !String(result.opened?.markdownValue).includes('Copy article body') ||
      !JSON.stringify(result.opened?.previewTexts).includes('Copy article body') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Article') ||
      !String(result.edited?.markdownValue).includes('Edited body') ||
      !JSON.stringify(result.edited?.previewTexts).includes('Parity Edited Article') ||
      !JSON.stringify(result.edited?.previewTexts).includes('Edited body') ||
      !JSON.stringify(result.edited?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.edited?.actionButtons).includes('提 交') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.title !== 'Parity Edited Article' ||
      result.saveRequests?.[0]?.category !== 'General' ||
      result.saveRequests?.[0]?.language !== 'en-US' ||
      !String(result.saveRequests?.[0]?.body).includes('Edited body') ||
      result.knowledgeFetchDelta < 1 ||
      result.saved?.drawerCount !== 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(`admin knowledge edit drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (label === 'admin-users-filter-input' && result.firstInput !== 'visual@example.com') {
    throw new Error('admin users filter input did not preserve typed value');
  }
  if (
    label === 'admin-users-filter-field-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['邮箱', '到期时间'])
  ) {
    throw new Error(`admin users filter field select did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-users-filter-expiry-picker' &&
    (result.before?.popupCount !== 0 ||
      result.opened?.popupCount !== 1 ||
      !result.opened?.popupClass?.includes(
        result.opened?.viewportWidth >= 600
          ? 'ant-calendar-picker-container-placement-bottomRight'
          : 'ant-calendar-picker-container-placement-bottomLeft',
      ) ||
      !result.opened?.calendarClass?.includes('ant-calendar-time') ||
      !JSON.stringify(result.opened?.pickerInputPlaceholders).includes('请选择日期') ||
      !JSON.stringify(result.opened?.popupInputPlaceholders).includes('请选择日期') ||
      !JSON.stringify(result.opened?.footerTexts).includes('此刻') ||
      !JSON.stringify(result.opened?.footerTexts).includes('选择时间') ||
      !JSON.stringify(result.opened?.footerTexts).includes('确 定') ||
      result.opened?.headerTexts?.length < 2)
  ) {
    throw new Error(`admin users filter expiry picker did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    (label === 'admin-user-bulk-ban-confirm' || label === 'admin-user-bulk-delete-confirm') &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('过滤器') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('操作') ||
      result.filtered?.drawerCount !== 0 ||
      !JSON.stringify(result.filtered?.filterQuery).includes('filter[0][key]') ||
      !JSON.stringify(result.filtered?.filterQuery).includes('email') ||
      !JSON.stringify(result.filtered?.filterQuery).includes('visual@example.com') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('批量封禁') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('批量删除') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('提醒') ||
      !JSON.stringify(result.opened?.content).includes(
        label === 'admin-user-bulk-delete-confirm' ? '确定要进行删除吗？' : '确定要进行封禁吗？',
      ) ||
      !JSON.stringify(result.opened?.buttons).includes('Cancel') ||
      !JSON.stringify(result.opened?.buttons).includes('OK') ||
      result.closed?.modalCount !== 0 ||
      JSON.stringify(result.closed?.dropdownItems ?? []) !== '[]')
  ) {
    throw new Error(`admin user bulk confirm did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-create-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('过滤器') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('创建用户') ||
      !JSON.stringify(result.opened?.labels).includes('邮箱') ||
      !JSON.stringify(result.opened?.labels).includes('密码') ||
      !JSON.stringify(result.opened?.labels).includes('到期时间') ||
      !JSON.stringify(result.opened?.labels).includes('订阅计划') ||
      !JSON.stringify(result.opened?.labels).includes('生成数量') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('生 成') ||
      !JSON.stringify(result.planDropdown?.dropdownItems).includes('无') ||
      !JSON.stringify(result.planDropdown?.dropdownItems).includes('Pro') ||
      !JSON.stringify(result.filled?.inputValues).includes('parity.created') ||
      !JSON.stringify(result.filled?.inputValues).includes('example.com') ||
      !JSON.stringify(result.filled?.inputValues).includes('secret123') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Pro') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin user create modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-create-plan-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['无', 'Pro'])
  ) {
    throw new Error(`admin user create plan select did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-create-expiry-picker' &&
    (result.before?.modalCount !== 1 ||
      result.before?.popupCount !== 0 ||
      result.opened?.popupCount !== 1 ||
      !result.opened?.popupClass?.includes('ant-calendar-picker-container-placement-bottomLeft') ||
      !result.opened?.calendarClass?.includes('ant-calendar') ||
      !JSON.stringify(result.opened?.pickerInputPlaceholders).includes(
        '请选择用户到期日期，为空则不限制到期时间',
      ) ||
      !JSON.stringify(result.opened?.popupInputPlaceholders).includes(
        '请选择用户到期日期，为空则不限制到期时间',
      ) ||
      !JSON.stringify(result.opened?.footerTexts).includes('今天') ||
      result.opened?.headerTexts?.length < 2)
  ) {
    throw new Error(`admin user create expiry picker did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-send-mail-modal' &&
    (result.before?.modalCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('导出CSV') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('发送邮件') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('发送邮件') ||
      !JSON.stringify(result.opened?.labels).includes('收件人') ||
      !JSON.stringify(result.opened?.labels).includes('主题') ||
      !JSON.stringify(result.opened?.labels).includes('发送内容') ||
      !JSON.stringify(result.opened?.inputValues).includes('全部用户') ||
      !JSON.stringify(result.opened?.buttons).includes('取 消') ||
      !JSON.stringify(result.opened?.buttons).includes('确 定') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity Mail Subject') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity mail body') ||
      !JSON.stringify(result.filled?.inputValues).includes('Line two') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin user send mail modal did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-reset-secret-confirm' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('重置UUID及订阅URL') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('重置安全信息') ||
      !JSON.stringify(result.opened?.content).includes('确定要重置visual-user@example.com的安全信息吗？') ||
      (!JSON.stringify(result.opened?.buttons).includes('取消') &&
        !JSON.stringify(result.opened?.buttons).includes('取 消')) ||
      (!JSON.stringify(result.opened?.buttons).includes('确定') &&
        !JSON.stringify(result.opened?.buttons).includes('确 定')) ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin user reset-secret confirm did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-delete-confirm' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('删除用户') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('删除用户') ||
      !JSON.stringify(result.opened?.content).includes('确定要删除visual-user@example.com的用户信息吗？') ||
      (!JSON.stringify(result.opened?.buttons).includes('取消') &&
        !JSON.stringify(result.opened?.buttons).includes('取 消')) ||
      (!JSON.stringify(result.opened?.buttons).includes('确定') &&
        !JSON.stringify(result.opened?.buttons).includes('确 定')) ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin user delete confirm did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-copy-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('复制订阅URL') ||
      !JSON.stringify(result.copied?.messageTexts).includes('复制成功') ||
      !JSON.stringify(result.copied?.dropdownItems).includes('复制订阅URL') ||
      result.copied?.modalCount !== 0)
  ) {
    throw new Error(`admin user copy action did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-edit-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('编辑') ||
      result.drawer?.drawerCount !== 1 ||
      !JSON.stringify(result.drawer?.drawerTitle).includes('用户管理') ||
      !JSON.stringify(result.drawer?.drawerLabels).includes('邮箱') ||
      !JSON.stringify(result.drawer?.drawerLabels).includes('订阅计划') ||
      !JSON.stringify(result.drawer?.drawerLabels).includes('账户状态') ||
      !JSON.stringify(result.drawer?.drawerLabels).includes('备注') ||
      !JSON.stringify(result.drawer?.drawerInputValues).includes('visual-user@example.com') ||
      !JSON.stringify(result.drawer?.drawerInputValues).includes('123.40') ||
      !JSON.stringify(result.drawer?.drawerInputValues).includes('100.00') ||
      !JSON.stringify(result.drawer?.selectedValues).includes('Pro') ||
      !JSON.stringify(result.drawer?.actionButtons).includes('取 消') ||
      !JSON.stringify(result.drawer?.actionButtons).includes('提 交'))
  ) {
    throw new Error(`admin user edit action did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-assign-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('分配订单') ||
      result.modalOpened?.modalCount !== 1 ||
      !JSON.stringify(result.modalOpened?.titles).includes('订单分配') ||
      !JSON.stringify(result.modalOpened?.labels).includes('用户邮箱') ||
      !JSON.stringify(result.modalOpened?.labels).includes('请选择订阅') ||
      !JSON.stringify(result.modalOpened?.labels).includes('请选择周期') ||
      !JSON.stringify(result.modalOpened?.labels).includes('支付金额') ||
      !JSON.stringify(result.modalOpened?.inputValues).includes('visual-user@example.com') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Pro') ||
      !JSON.stringify(result.filled?.selectedValues).includes('月付') ||
      !JSON.stringify(result.filled?.inputValues).includes('23.45') ||
      result.assignRequest?.email !== 'visual-user@example.com' ||
      String(result.assignRequest?.plan_id) !== '1' ||
      result.assignRequest?.period !== 'month_price' ||
      String(result.assignRequest?.total_amount) !== '2345' ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(`admin user assign action did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-orders-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('TA的订单') ||
      !String(result.navigated?.hash).includes('/order') ||
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('user_id') ||
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('=') ||
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('1'))
  ) {
    throw new Error(`admin user orders action did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-invite-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('TA的邀请') ||
      !String(result.filtered?.hash).includes('/user') ||
      !JSON.stringify(result.filtered?.userFetchQuery).includes('invite_user_id') ||
      !JSON.stringify(result.filtered?.userFetchQuery).includes('=') ||
      !JSON.stringify(result.filtered?.userFetchQuery).includes('1'))
  ) {
    throw new Error(`admin user invite action did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-user-traffic-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('TA的流量记录') ||
      result.modal?.modalCount !== 1 ||
      !JSON.stringify(result.modal?.modalTitle).includes('流量记录') ||
      !JSON.stringify(result.modal?.tableHeaders).includes('日期') ||
      !JSON.stringify(result.modal?.tableHeaders).includes('上行') ||
      !JSON.stringify(result.modal?.tableHeaders).includes('下行') ||
      !JSON.stringify(result.modal?.tableHeaders).includes('倍率') ||
      !JSON.stringify(result.modal?.modalRows).includes('2024-01-15') ||
      !JSON.stringify(result.modal?.trafficQuery).includes('user_id') ||
      !JSON.stringify(result.modal?.trafficQuery).includes('1') ||
      !JSON.stringify(result.modal?.trafficQuery).includes('pageSize'))
  ) {
    throw new Error(`admin user traffic action did not produce observable state: ${JSON.stringify(result)}`);
  }
}

async function visibleTexts(page, selector, limit = 10) {
  return page.evaluate(
    ({ limit: maxItems, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector))
        .filter(isVisible)
        .slice(0, maxItems)
        .map((element) => (element.textContent ?? '').trim().replace(/\s+/g, ' '))
        .filter(Boolean);
    },
    { limit, selector },
  );
}

async function visibleLinkStates(page, selector, limit = 10) {
  return page.evaluate(
    ({ limit: maxItems, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector))
        .filter(isVisible)
        .slice(0, maxItems)
        .map((element) => ({
          href: element.getAttribute('href') ?? '',
          text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
        }));
    },
    { limit, selector },
  );
}

async function visibleCount(page, selector) {
  return page.evaluate(
    (targetSelector) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector)).filter(isVisible).length;
    },
    selector,
  );
}

async function hoverTooltipInteraction(page, selectors) {
  const before = await tooltipState(page);
  await hoverFirstVisibleFromSelectors(page, selectors);
  await waitForVisibleTooltip(page);
  await page.waitForTimeout(150);
  const opened = await tooltipState(page);
  return { before, opened };
}

async function hoverAllTooltipTargetsInteraction(page, selectors) {
  const before = await tooltipState(page);
  const viewportWidth = await page.evaluate(() => window.innerWidth);
  const targetCount = await visibleTooltipTargetCount(page, selectors);
  const opened = [];

  for (let index = 0; index < targetCount; index += 1) {
    await hoverVisibleTooltipTargetAt(page, selectors, index);
    try {
      await waitForVisibleTooltip(page, 800);
    } catch {
      await hoverVisibleTooltipTargetAncestorAt(page, selectors, index, 'span');
      await waitForVisibleTooltip(page);
    }
    await page.waitForTimeout(150);
    opened.push(await tooltipState(page));
    await page.mouse.move(1, 1);
    await waitForNoVisibleTooltip(page);
  }

  return { before, opened, targetCount, viewportWidth };
}

async function tooltipState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const tooltips = Array.from(document.querySelectorAll('.ant-tooltip'))
      .filter((element) => !element.className.includes('ant-tooltip-hidden'))
      .filter(isVisible);
    const tooltip = tooltips[0];

    return {
      className: tooltip ? normalize(tooltip.className) : '',
      openTriggerCount: Array.from(document.querySelectorAll('.ant-tooltip-open')).filter(
        isVisible,
      ).length,
      placement:
        tooltip?.className.match(/ant-tooltip-placement-([A-Za-z]+)/)?.[1] ?? '',
      texts: tooltip
        ? Array.from(tooltip.querySelectorAll('.ant-tooltip-inner'))
            .filter(isVisible)
            .map((element) => normalize(element.textContent))
            .filter(Boolean)
        : [],
      tooltipCount: tooltips.length,
    };
  });
}

async function waitForVisibleTooltip(page, timeout = 5_000) {
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      return Array.from(document.querySelectorAll('.ant-tooltip'))
        .filter((element) => !element.className.includes('ant-tooltip-hidden'))
        .some(isVisible);
    },
    { timeout },
  );
}

async function waitForNoVisibleTooltip(page) {
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      return !Array.from(document.querySelectorAll('.ant-tooltip'))
        .filter((element) => !element.className.includes('ant-tooltip-hidden'))
        .some(isVisible);
    },
    { timeout: 5_000 },
  );
}

async function hoverFirstVisibleFromSelectors(page, selectors) {
  const point = await page.evaluate((targetSelectors) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    for (const selector of targetSelectors) {
      const element = Array.from(document.querySelectorAll(selector)).find(isVisible);
      if (!element) continue;
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    }
    throw new Error(`No visible hover target for selectors: ${targetSelectors.join(', ')}`);
  }, selectors);
  await page.mouse.move(point.x, point.y);
}

async function visibleTooltipTargetCount(page, selectors) {
  return page.evaluate((targetSelectors) => {
    const isHoverable = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      const centerX = rect.left + rect.width / 2;
      const centerY = rect.top + rect.height / 2;
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        centerX >= 0 &&
        centerX <= window.innerWidth &&
        centerY >= 0 &&
        centerY <= window.innerHeight
      );
    };
    return Array.from(document.querySelectorAll(targetSelectors.join(', '))).filter(isHoverable)
      .length;
  }, selectors);
}

async function hoverVisibleTooltipTargetAt(page, selectors, index) {
  const point = await page.evaluate(
    ({ index: targetIndex, selectors: targetSelectors }) => {
      const isHoverable = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          centerX >= 0 &&
          centerX <= window.innerWidth &&
          centerY >= 0 &&
          centerY <= window.innerHeight
        );
      };
      const element = Array.from(document.querySelectorAll(targetSelectors.join(', '))).filter(
        isHoverable,
      )[targetIndex];
      if (!element) {
        throw new Error(
          `No visible hover target at ${targetIndex} for selectors: ${targetSelectors.join(', ')}`,
        );
      }
      const rect = element.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { index, selectors },
  );
  await page.mouse.move(point.x, point.y);
}

async function hoverVisibleTooltipTargetAncestorAt(page, selectors, index, ancestorSelector) {
  const point = await page.evaluate(
    ({ ancestorSelector: targetAncestorSelector, index: targetIndex, selectors: targetSelectors }) => {
      const isHoverable = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden' &&
          centerX >= 0 &&
          centerX <= window.innerWidth &&
          centerY >= 0 &&
          centerY <= window.innerHeight
        );
      };
      const element = Array.from(document.querySelectorAll(targetSelectors.join(', '))).filter(
        isHoverable,
      )[targetIndex];
      const ancestor = element?.closest(targetAncestorSelector);
      if (!ancestor) {
        throw new Error(
          `No visible hover target ancestor at ${targetIndex} for selectors: ${targetSelectors.join(', ')}`,
        );
      }
      const rect = ancestor.getBoundingClientRect();
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
      };
    },
    { ancestorSelector, index, selectors },
  );
  await page.mouse.move(point.x, point.y);
}

async function visibleTextCount(page, selector, texts) {
  return page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector)).filter((element) => {
        const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
        return isVisible(element) && targetTexts.includes(text);
      }).length;
    },
    { selector, texts },
  );
}

async function waitForVisibleText(page, selector, text) {
  await page.waitForFunction(
    ({ selector: targetSelector, text: targetText }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return Array.from(document.querySelectorAll(targetSelector)).some((element) => {
        const normalized = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
        return isVisible(element) && normalized === targetText;
      });
    },
    { selector, text },
    { timeout: 5_000 },
  );
}

async function waitForPageProperty(page, property, timeout = 5_000) {
  const deadline = Date.now() + timeout;
  while (!page[property]) {
    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for page property ${property}`);
    }
    await page.waitForTimeout(100);
  }
}

async function knowledgeState(page) {
  return {
    articleTitles: await visibleTexts(page, '.list-group-item h5', 8),
    categoryTitles: await visibleTexts(page, '.block-header .block-title', 8),
    drawerBodies: await visibleTexts(page, '.ant-drawer-body .custom-html-style', 4),
    drawerOpenCount: await visibleCount(page, '.ant-drawer-open'),
    drawerTitles: await visibleTexts(page, '.ant-drawer-title', 4),
    searchValue: await firstInputValue(page, '.v2board-knowledge-search-bar input'),
  };
}

async function loginLanguagePersistenceState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const readCookie = (name) =>
      document.cookie.split('; ').reduce((value, item) => {
        const [key, raw] = item.split('=');
        if (key !== name || raw === undefined) return value;
        try {
          return decodeURIComponent(raw);
        } catch {
          return value;
        }
      }, '');

    return {
      cookieI18n: readCookie('i18n'),
      gLang: window.g_lang ?? '',
      storedLocale: window.localStorage.getItem('umi_locale') ?? '',
      titleText: normalize(document.querySelector('.v2board-auth-box h1, .block-title')?.textContent),
      triggerText: normalize(document.querySelector('.v2board-login-i18n-btn')?.textContent),
    };
  });
}

async function languageDropdownPlacementState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const rectOf = (element) => {
      const rect = element.getBoundingClientRect();
      return {
        bottom: rect.bottom,
        height: rect.height,
        left: rect.left,
        right: rect.right,
        top: rect.top,
        width: rect.width,
      };
    };
    const trigger = Array.from(
      document.querySelectorAll('#page-header button, #page-header .ant-dropdown-trigger'),
    ).find((element) => element.querySelector('.fa-language') && isVisible(element));
    const dropdown = Array.from(document.querySelectorAll('.ant-dropdown')).find(isVisible);
    const triggerRect = trigger ? rectOf(trigger) : undefined;
    const dropdownRect = dropdown ? rectOf(dropdown) : undefined;
    const triggerCenter = triggerRect
      ? triggerRect.left + triggerRect.width / 2
      : undefined;
    const dropdownCenter = dropdownRect
      ? dropdownRect.left + dropdownRect.width / 2
      : undefined;

    return {
      centerDelta:
        triggerCenter === undefined || dropdownCenter === undefined
          ? undefined
          : Math.round(dropdownCenter - triggerCenter),
      dropdownCount: Array.from(document.querySelectorAll('.ant-dropdown')).filter(isVisible).length,
      gap:
        triggerRect && dropdownRect
          ? Math.round(dropdownRect.top - triggerRect.bottom)
          : undefined,
      items: Array.from(document.querySelectorAll('.ant-dropdown-menu-item'))
        .filter(isVisible)
        .map((element) => (element.textContent ?? '').trim().replace(/\s+/g, ' ')),
      opensBelow: Boolean(triggerRect && dropdownRect && dropdownRect.top >= triggerRect.bottom),
      placement: dropdown?.className.match(/ant-dropdown-placement-([A-Za-z]+)/)?.[1] ?? '',
      triggerOpen: Boolean(trigger?.className.includes('ant-dropdown-open')),
    };
  });
}

async function clickHeaderAvatarTrigger(page) {
  const clicked = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const trigger = Array.from(document.querySelectorAll('#page-header button')).find(
      (element) => element.querySelector('.fa-user-circle') && isVisible(element),
    );
    if (!(trigger instanceof HTMLElement)) return false;
    trigger.click();
    return true;
  });
  if (!clicked) throw new Error('header avatar trigger was not visible');
}

async function waitForHeaderAvatarDropdown(page) {
  await page.waitForFunction(
    () => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          style.display !== 'none' &&
          style.visibility !== 'hidden'
        );
      };
      return Array.from(document.querySelectorAll('#page-header .dropdown-menu.show')).some(
        isVisible,
      );
    },
    { timeout: 5_000 },
  );
}

async function headerAvatarDropdownState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const rectOf = (element) => {
      const rect = element.getBoundingClientRect();
      return {
        bottom: Math.round(rect.bottom),
        left: Math.round(rect.left),
        right: Math.round(rect.right),
        top: Math.round(rect.top),
        width: Math.round(rect.width),
      };
    };
    const trigger = Array.from(document.querySelectorAll('#page-header button')).find(
      (element) => element.querySelector('.fa-user-circle') && isVisible(element),
    );
    const visibleMenus = Array.from(
      document.querySelectorAll('#page-header .dropdown-menu.show'),
    ).filter(isVisible);
    const menu = visibleMenus[0];
    const triggerRect = trigger ? rectOf(trigger) : undefined;
    const menuRect = menu ? rectOf(menu) : undefined;

    return {
      items: menu
        ? Array.from(menu.querySelectorAll('.dropdown-item'))
            .filter(isVisible)
            .map((element) => normalize(element.textContent))
            .filter(Boolean)
        : [],
      menuClass: menu ? normalize(menu.className) : '',
      menuCount: visibleMenus.length,
      menuTopDelta:
        triggerRect && menuRect ? Math.round(menuRect.top - triggerRect.bottom) : undefined,
      menuWidth: menuRect?.width,
      rightDelta:
        triggerRect && menuRect ? Math.round(menuRect.right - triggerRect.right) : undefined,
    };
  });
}

async function clickDarkModeButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const icon = Array.from(
      document.querySelectorAll('#page-header button i.fa-sun, #page-header button i.fa-moon'),
    ).find(isVisible);
    const button = icon?.closest('button');
    if (!button) {
      throw new Error('No visible dark mode button');
    }
    button.click();
  });
}

async function darkModePersistenceState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const readCookie = (name) =>
      document.cookie.split('; ').reduce((value, item) => {
        const [key, raw] = item.split('=');
        if (key !== name || raw === undefined) return value;
        try {
          return decodeURIComponent(raw);
        } catch {
          return value;
        }
      }, '');
    const icon = Array.from(
      document.querySelectorAll('#page-header button i.fa-sun, #page-header button i.fa-moon'),
    ).find(isVisible);

    return {
      cookieDarkMode: readCookie('dark_mode'),
      darkReaderReady:
        document.documentElement.getAttribute('data-darkreader-mode') === 'dynamic' &&
        document.documentElement.getAttribute('data-darkreader-scheme') === 'dark' &&
        document.querySelectorAll('.darkreader').length > 0,
      iconClass: icon?.className ?? '',
    };
  });
}

async function waitForStableDarkModeStyleSnapshot(page, diagnostics) {
  let previousSnapshot;
  let currentSnapshot = await darkModeStyleSnapshot(page);

  for (let attempt = 0; attempt < 8; attempt += 1) {
    await page.waitForTimeout(250);
    currentSnapshot = await darkModeStyleSnapshot(page);
    if (previousSnapshot && stableJson(previousSnapshot) === stableJson(currentSnapshot)) {
      return currentSnapshot;
    }
    previousSnapshot = currentSnapshot;
  }

  diagnostics.push(`dark mode style snapshot did not stabilize ${stableJson(currentSnapshot)}`);
  return currentSnapshot;
}

async function darkModeStyleSnapshot(page) {
  return page.evaluate((targets) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const normalizeStyleValue = (value) => {
      const normalized = value.replace(/\s+/g, ' ').trim();
      if (/^rgba\(\d+, \d+, \d+, 0(?:\.0+)?\)$/.test(normalized)) {
        return 'rgba(0, 0, 0, 0)';
      }
      return normalized;
    };
    const visibleBorderColor = (style, side) => {
      const borderStyle = style[`border${side}Style`];
      const borderWidth = Number.parseFloat(style[`border${side}Width`]);
      if (!borderWidth || borderStyle === 'none' || borderStyle === 'hidden') {
        return '';
      }
      return normalizeStyleValue(style[`border${side}Color`]);
    };
    const snapshotElement = ({ key, selector }) => {
      const element = Array.from(document.querySelectorAll(selector)).find(isVisible);
      if (!element) return undefined;
      const style = window.getComputedStyle(element);
      return [
        key,
        {
          backgroundColor: normalizeStyleValue(style.backgroundColor),
          borderBottomColor: visibleBorderColor(style, 'Bottom'),
          borderLeftColor: visibleBorderColor(style, 'Left'),
          borderRightColor: visibleBorderColor(style, 'Right'),
          borderTopColor: visibleBorderColor(style, 'Top'),
          boxShadow: normalizeStyleValue(style.boxShadow),
          caretColor: normalizeStyleValue(style.caretColor),
          color: normalizeStyleValue(style.color),
          outlineColor: normalizeStyleValue(style.outlineColor),
          selector,
          textDecorationColor: normalizeStyleValue(style.textDecorationColor),
        },
      ];
    };
    const elements = Object.fromEntries(targets.map(snapshotElement).filter(Boolean));

    return {
      capturedCount: Object.keys(elements).length,
      darkReaderMode: document.documentElement.getAttribute('data-darkreader-mode') ?? '',
      darkReaderScheme: document.documentElement.getAttribute('data-darkreader-scheme') ?? '',
      elements,
    };
  }, darkModeStyleTargets);
}

async function dashboardSubscribeState(page) {
  return {
    bodyOverflow: await page.evaluate(() => document.body.style.overflow),
    boxCount: await visibleCount(page, '.oneClickSubscribe___2t9Xg'),
    drawerOpenCount: await visibleCount(page, '.ant-drawer-open'),
    itemTexts: await visibleTexts(page, '.oneClickSubscribe___2t9Xg .item___yrtOv', 12),
    messageTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
    modalCount: await visibleCount(page, '.ant-modal'),
    qrCanvasCount: await visibleCount(page, '.ant-modal canvas'),
    qrTipTexts: await visibleTexts(page, '.ant-modal .ant-modal-body', 4),
    shortcutTexts: await visibleTexts(page, '.v2board-shortcuts-item', 4),
    tutorialButtons: await visibleTexts(page, '.oneClickSubscribe___2t9Xg .ant-btn', 2),
  };
}

async function dashboardSubscribeImportLinksState(page) {
  const items = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('.oneClickSubscribe___2t9Xg .item___yrtOv'))
      .filter(isVisible)
      .map((item) => ({
        className: item.className,
        iconCount: item.querySelectorAll('i').length,
        imageCount: item.querySelectorAll('img').length,
        text: (item.textContent ?? '').trim().replace(/\s+/g, ' '),
      }));
  });

  return {
    bodyOverflow: await page.evaluate(() => document.body.style.overflow),
    boxCount: await visibleCount(page, '.oneClickSubscribe___2t9Xg'),
    drawerOpenCount: await visibleCount(page, '.ant-drawer-open'),
    itemClasses: items.map((item) => item.className),
    items,
    itemTexts: items.map((item) => item.text),
    modalCount: await visibleCount(page, '.ant-modal'),
    shortcutTexts: await visibleTexts(page, '.v2board-shortcuts-item', 4),
    tutorialButtons: await visibleTexts(page, '.oneClickSubscribe___2t9Xg .ant-btn', 2),
  };
}

async function dashboardNoticeCarouselState(page) {
  const dotState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const dots = Array.from(document.querySelectorAll('.slick-dots li')).filter(isVisible);
    return {
      activeDotIndex: dots.findIndex((dot) => dot.classList.contains('slick-active')),
      dotCount: dots.length,
    };
  });

  return {
    ...dotState,
    activeSlideTexts: await visibleTexts(page, '.slick-slide.slick-active', 4),
    modalBodies: await visibleTexts(page, '.ant-modal .ant-modal-body', 4),
    modalCount: await visibleCount(page, '.ant-modal'),
    modalTitles: await visibleTexts(page, '.ant-modal-title', 4),
  };
}

async function dashboardResetPackageConfirmState(page) {
  const resetTriggerCount = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('a, button')).filter((element) => {
      const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
      return isVisible(element) && text === '购买流量重置包';
    }).length;
  });

  return {
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 4),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    resetTriggerCount,
    title: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 4),
  };
}

async function dashboardNewPeriodConfirmState(page) {
  const newPeriodTriggerCount = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('a, button')).filter((element) => {
      const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
      return isVisible(element) && text === '提前开启流量周期';
    }).length;
  });

  return {
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 4),
    hash: await page.evaluate(() => window.location.hash),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    newPeriodCount: page.__visualParityUserNewPeriodCount ?? 0,
    newPeriodTriggerCount,
    title: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 4),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

async function dashboardAlertLinksState(page) {
  return {
    alertLinks: await visibleTexts(page, '.alert .alert-link', 4),
    blockTitles: await visibleTexts(page, '.block-title', 8),
    containerTitles: await visibleTexts(page, '.v2board-container-title', 4),
    hash: await page.evaluate(() => window.location.hash),
    tableHeaders: await visibleTexts(page, '.ant-table-column-title', 12),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr, .am-list-item', 8),
  };
}

async function profileResetSubscribeState(page) {
  return {
    blockTitles: await visibleTexts(page, '.block-title', 10),
    buttons: await visibleTexts(page, '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn', 4),
    content: await visibleTexts(page, '.ant-modal-confirm-content, .ant-modal-body', 4),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    resetButtons: await visibleTexts(page, '.ant-btn-danger', 4),
    resetCount: page.__visualParityUserResetSecurityCount ?? 0,
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
    title: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 4),
    warningTexts: await visibleTexts(page, '.alert-warning', 4),
  };
}

async function profileTelegramBindState(page) {
  return {
    blockTitles: await visibleTexts(page, '.block-title', 12),
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn, .ant-modal .ant-btn', 4),
    copyCommandCount: await page.evaluate(() => window.__visualParityCopyCommandCount ?? 0),
    discussionLinks: await visibleLinkStates(page, '.join_telegram_disscuss a'),
    modalBodies: await visibleTexts(page, '.ant-modal .ant-modal-body', 4),
    modalCode: await visibleTexts(page, '.ant-modal code', 4),
    modalCount: await visibleCount(page, '.ant-modal'),
    modalLinks: await visibleLinkStates(page, '.ant-modal a'),
    modalTitles: await visibleTexts(page, '.ant-modal-title', 4),
    startButtons: await visibleTexts(page, '.bind_telegram .btn, .bind_telegram button', 4),
  };
}

async function profileTelegramUnbindState(page) {
  return {
    blockTitles: await visibleTexts(page, '.block-title', 12),
    buttons: await visibleTexts(
      page,
      '.ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
      4,
    ),
    modalContent: await visibleTexts(
      page,
      '.ant-modal-confirm-content, .ant-modal-body',
      4,
    ),
    modalCount: await visibleCount(page, '.ant-modal-confirm, .ant-modal'),
    modalTitle: await visibleTexts(page, '.ant-modal-confirm-title, .ant-modal-title', 4),
    telegramIdTexts: await visibleTexts(page, '.unbind_telegram .block-options', 4),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
    unbindButtons: await visibleTexts(
      page,
      '.unbind_telegram .ant-btn, .unbind_telegram button',
      4,
    ),
    unbindCount: page.__visualParityUserUnbindTelegramCount ?? 0,
  };
}

async function profilePreferenceSwitchesState(page) {
  const switches = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('.ant-switch'))
      .filter(isVisible)
      .map((element) => ({
        ariaChecked: element.getAttribute('aria-checked'),
        checked: Boolean(element.matches('.ant-switch-checked, [aria-checked="true"]')),
        disabled: Boolean(element.matches(':disabled, .ant-switch-disabled')),
        loading: Boolean(
          element.matches('.ant-switch-loading') ||
            element.querySelector('.ant-switch-loading-icon'),
        ),
        role: element.getAttribute('role'),
      }));
  });
  const updateRequests = (page.__visualParityUserUpdateRequests ?? []).map((request) =>
    request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
  );
  return {
    blockTitles: await visibleTexts(page, '.block-title', 12),
    labels: await visibleTexts(page, '.text-muted, .form-group label', 16),
    switchCount: switches.length,
    switches,
    updateRequests,
  };
}

async function profileRedeemGiftcardState(page) {
  const domState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const input = Array.from(document.querySelectorAll('input')).find((element) => {
      const placeholder = element.getAttribute('placeholder') ?? '';
      return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
    });
    const block = input?.closest('.block') ?? null;
    const button = block
      ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
      : null;
    return {
      inputValue: input && 'value' in input ? input.value : '',
      redeemButton: button
        ? {
            className: normalizeClassName(button.className),
            disabled: Boolean(button.matches(':disabled, .ant-btn-disabled')),
            loading: Boolean(
              button.matches('.ant-btn-loading') ||
                button.querySelector('.anticon-loading, .fa-spin'),
            ),
            text: (button.textContent ?? '').trim().replace(/\s+/g, ' '),
          }
        : null,
    };
  });
  return {
    blockTitles: await visibleTexts(page, '.block-title', 12),
    ...domState,
    redeemRequests: (page.__visualParityUserRedeemGiftcardRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

async function profileChangePasswordState(page) {
  const domState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const block = Array.from(document.querySelectorAll('.block')).find((element) => {
      const title = element.querySelector('.block-title')?.textContent ?? '';
      return isVisible(element) && /Change Password|修改密码/.test(title);
    });
    const inputs = block
      ? Array.from(block.querySelectorAll('input')).filter(isVisible).map((element) => ({
          placeholder: element.getAttribute('placeholder') ?? '',
          type: element.getAttribute('type') ?? '',
          value: 'value' in element ? element.value : '',
        }))
      : [];
    const button = block
      ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
      : null;
    const loginPasswordInput = Array.from(document.querySelectorAll('input[type="password"]')).find(
      isVisible,
    );
    return {
      authBoxCount: Array.from(document.querySelectorAll('.v2board-auth-box')).filter(isVisible)
        .length,
      passwordInputs: inputs,
      saveButton: button
        ? {
            className: normalizeClassName(button.className),
            disabled: Boolean(button.matches(':disabled, .ant-btn-disabled')),
            loading: Boolean(
              button.matches('.ant-btn-loading') ||
                button.querySelector('.anticon-loading, .fa-spin'),
            ),
            text: (button.textContent ?? '').trim().replace(/\s+/g, ' '),
          }
        : null,
      visibleLoginPasswordPlaceholder: loginPasswordInput?.getAttribute('placeholder') ?? '',
    };
  });
  return {
    blockTitles: await visibleTexts(page, '.block-title', 12),
    changePasswordRequests: (page.__visualParityUserChangePasswordRequests ?? []).map((request) =>
      request && typeof request === 'object' && !Array.isArray(request) ? { ...request } : request,
    ),
    hash: await page.evaluate(() => window.location.hash),
    localAuthPresent: await page.evaluate(() => Boolean(window.localStorage.getItem('authorization'))),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
    ...domState,
  };
}

async function inviteState(page) {
  return {
    generateButton: await firstElementState(page, '.block-header .block-options .btn'),
    statBlocks: await visibleTexts(page, '.block-content.pb-3', 4),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 10),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

async function inviteFinanceDialogState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 8),
    hash: await page.evaluate(() => window.location.hash),
    inputValues: await visibleInputValues(page, '.ant-modal input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 4),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 4),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

async function adminCouponModalState(page) {
  return {
    addonTexts: await visibleTexts(page, '.ant-modal .ant-input-group-addon', 6),
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    inputValues: await visibleInputValues(page, '.ant-modal input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 12),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: [
      ...(await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 6)),
      ...(await visibleTexts(page, '.ant-modal .ant-select-selection__choice__content', 8)),
    ],
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function legacyDatePickerState(page, rootSelector) {
  return page.evaluate((selector) => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const visible = (selectorText) =>
      Array.from(document.querySelectorAll(selectorText)).filter(isVisible);
    const popup = visible('.ant-calendar-picker-container')[0];

    return {
      calendarClass: normalize(visible('.ant-calendar')[0]?.className),
      footerTexts: visible('.ant-calendar-footer a').map((element) =>
        normalize(element.textContent),
      ),
      headerTexts: visible('.ant-calendar-month-select, .ant-calendar-year-select').map(
        (element) => normalize(element.textContent),
      ),
      modalCount: visible('.ant-modal').length,
      pickerInputPlaceholders: visible(`${selector} .ant-calendar-picker-input`).map(
        (element) => element.getAttribute('placeholder') ?? '',
      ),
      popupClass: normalize(popup?.className),
      popupCount: visible('.ant-calendar-picker-container').length,
      popupInputPlaceholders: visible('.ant-calendar-picker-container .ant-calendar-input').map(
        (element) => element.getAttribute('placeholder') ?? '',
      ),
      viewportWidth: window.innerWidth,
    };
  }, rootSelector);
}

async function legacySelectDropdownState(page, rootSelector) {
  return page.evaluate((selector) => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const round = (value) => Math.round(value * 10) / 10;
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const visible = (selectorText) =>
      Array.from(document.querySelectorAll(selectorText)).filter(isVisible);
    const dropdown = visible('.ant-select-dropdown')[0];
    const trigger =
      visible(`${selector} .ant-select-open`)[0] ?? visible(`${selector} .ant-select`)[0];
    const dropdownRect = dropdown?.getBoundingClientRect();
    const triggerRect = trigger?.getBoundingClientRect();

    return {
      activeItems: visible('.ant-select-dropdown-menu-item-active').map((element) =>
        normalize(element.textContent),
      ),
      dropdownClass: normalize(dropdown?.className),
      dropdownCount: visible('.ant-select-dropdown').length,
      dropdownItems: visible('.ant-select-dropdown-menu-item').map((element) =>
        normalize(element.textContent),
      ),
      geometry:
        dropdownRect && triggerRect
          ? {
              opensAbove: dropdownRect.bottom <= triggerRect.top + 1,
              opensBelow: dropdownRect.top >= triggerRect.bottom - 1,
              widthDelta: round(dropdownRect.width - triggerRect.width),
            }
          : null,
      selectedItems: visible('.ant-select-dropdown-menu-item-selected').map((element) =>
        normalize(element.textContent),
      ),
      triggerClasses: visible(`${selector} .ant-select`).map((element) =>
        normalize(element.className),
      ),
      viewportWidth: window.innerWidth,
    };
  }, rootSelector);
}

function legacySelectDropdownHasOpened(result, expectedItems) {
  return (
    result.before?.dropdownCount === 0 &&
    result.opened?.dropdownCount === 1 &&
    result.opened?.dropdownClass?.includes('ant-select-dropdown') &&
    Boolean(result.opened?.geometry) &&
    expectedItems.every((item) => JSON.stringify(result.opened?.dropdownItems).includes(item))
  );
}

async function legacyRangePickerState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    const visible = (selector) => Array.from(document.querySelectorAll(selector)).filter(isVisible);
    const popup = visible('.ant-calendar-picker-container')[0];

    return {
      calendarClass: normalize(visible('.ant-calendar')[0]?.className),
      footerTexts: visible('.ant-calendar-footer a').map((element) =>
        normalize(element.textContent),
      ),
      headerTexts: visible('.ant-calendar-month-select, .ant-calendar-year-select').map(
        (element) => normalize(element.textContent),
      ),
      modalCount: visible('.ant-modal').length,
      pickerInputPlaceholders: visible('.ant-modal .ant-calendar-range-picker-input').map(
        (element) => element.getAttribute('placeholder') ?? '',
      ),
      popupClass: normalize(popup?.className),
      popupCount: visible('.ant-calendar-picker-container').length,
      popupInputPlaceholders: visible('.ant-calendar-picker-container .ant-calendar-input').map(
        (element) => element.getAttribute('placeholder') ?? '',
      ),
    };
  });
}

async function adminGiftcardModalState(page) {
  return {
    addonTexts: await visibleTexts(page, '.ant-modal .ant-input-group-addon', 6),
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 10),
    inputValues: await visibleInputValues(page, '.ant-modal input'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 12),
    modalCount: await visibleCount(page, '.ant-modal'),
    selectedValues: await visibleTexts(page, '.ant-modal .ant-select-selection-selected-value', 6),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function adminNoticeModalState(page) {
  return {
    buttons: await visibleTexts(page, '.ant-modal-footer .ant-btn', 4),
    choiceTexts: await visibleTexts(page, '.ant-modal .ant-select-selection__choice__content', 8),
    inputValues: await visibleInputValues(page, '.ant-modal input, .ant-modal textarea'),
    labels: await visibleTexts(page, '.ant-modal .form-group label', 8),
    modalCount: await visibleCount(page, '.ant-modal'),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-modal-title', 2),
  };
}

async function adminPlanDrawerState(page) {
  return {
    actionButtons: await visibleTexts(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 4),
    actionDropdownItems: await visibleTexts(page, '.ant-dropdown-menu-item', 10),
    addonTexts: await visibleTexts(page, '.ant-drawer-open .ant-input-group-addon', 8),
    drawerCount: await visibleCount(page, '.ant-drawer-open'),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 10),
    forceUpdate: await page.evaluate(() => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const wrapper = Array.from(
        document.querySelectorAll('.ant-drawer-open .ant-checkbox-wrapper'),
      ).find(isVisible);
      if (!wrapper) return null;
      return {
        checked: Boolean(
          wrapper.matches('.ant-checkbox-wrapper-checked') ||
            wrapper.querySelector('.ant-checkbox-checked, input:checked'),
        ),
        text: (wrapper.textContent ?? '').trim().replace(/\s+/g, ' '),
      };
    }),
    inputValues: await visibleInputValues(page, '.ant-drawer-open .ant-input'),
    labels: await visibleTexts(page, '.ant-drawer-open .form-group label', 24),
    selectedValues: await visibleTexts(page, '.ant-drawer-open .ant-select-selection-selected-value', 6),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-drawer-title', 2),
  };
}

async function adminKnowledgeDrawerState(page) {
  return {
    actionButtons: await visibleTexts(page, '.ant-drawer-open .v2board-drawer-action .ant-btn', 4),
    drawerCount: await visibleCount(page, '.ant-drawer-open'),
    dropdownItems: await visibleTexts(page, '.ant-select-dropdown-menu-item', 10),
    inputValues: await visibleInputValues(page, '.ant-drawer-open .ant-input'),
    labels: await visibleTexts(page, '.ant-drawer-open .form-group label', 8),
    markdownValue: await firstInputValue(page, '.ant-drawer-open textarea.section-container.input'),
    previewTexts: await visibleTexts(page, '.ant-drawer-open .custom-html-style', 4),
    selectedValues: await visibleTexts(page, '.ant-drawer-open .ant-select-selection-selected-value', 4),
    tableRows: await visibleTexts(page, '.ant-table-tbody tr', 6),
    titles: await visibleTexts(page, '.ant-drawer-title', 2),
  };
}

async function orderPaymentState(page) {
  return {
    activeIndex: await activeVisibleElementIndex(page, '#cashier .v2board-select'),
    methodTexts: await visibleTexts(page, '#cashier .v2board-select', 6),
    summaryBlocks: await visibleTexts(page, '#cashier .col-md-4 .block', 4),
    submitButton: await firstElementState(page, '#cashier .btn-block.btn-primary'),
  };
}

async function orderCheckoutState(page) {
  return {
    ...(await orderPaymentState(page)),
    creditCardTexts: await visibleTexts(page, '#cashier h3, #cashier .fa-user-shield, #cashier .mt-3.mb-5', 6),
    hash: await page.evaluate(() => window.location.hash),
    modalCount: await visibleCount(page, '.ant-modal'),
    modalTexts: await visibleTexts(page, '.ant-modal', 4),
    qrCanvasCount: await visibleCount(page, '.v2board-payment-qrcode canvas'),
    qrSvgCount: await visibleCount(page, '.v2board-payment-qrcode svg'),
    stripePublicKeyCount: page.__visualParityUserStripePublicKeyCount ?? 0,
    toastTexts: await visibleTexts(page, '.ant-message-notice, .ant-notification-notice', 4),
  };
}

async function waitForOrderPaymentMethodCount(page) {
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll('#cashier .v2board-select')).filter((element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      }).length >= 3,
    { timeout: 5_000 },
  );
}

async function waitForCreditCardSection(page) {
  await page.waitForFunction(
    () => {
      const text = document.querySelector('#cashier')?.textContent ?? '';
      return /信用卡|credit card/i.test(text);
    },
    null,
    { timeout: 5_000 },
  );
}

async function setLegacyAntTableScrollLeft(page, position) {
  await page.evaluate((targetPosition) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const body = Array.from(document.querySelectorAll('.ant-table-body')).find(isVisible);
    if (!body) return;
    const maxScroll = Math.max(0, body.scrollWidth - body.clientWidth);
    body.scrollLeft =
      targetPosition === 'middle' ? Math.floor(maxScroll / 2) : maxScroll;
    body.dispatchEvent(new Event('scroll', { bubbles: true }));
  }, position);
}

async function legacyAntTableScrollState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const table = Array.from(document.querySelectorAll('.ant-table.ant-table-default')).find(isVisible);
    const body = Array.from(document.querySelectorAll('.ant-table-body')).find(isVisible);
    const maxScroll = body ? Math.max(0, body.scrollWidth - body.clientWidth) : 0;

    return {
      className: normalizeClassName(table?.className ?? ''),
      clientWidth: Math.round(body?.clientWidth ?? 0),
      maxScroll: Math.round(maxScroll),
      rows: Array.from(document.querySelectorAll('.ant-table-tbody tr'))
        .filter(isVisible)
        .slice(0, 4)
        .map((row) => (row.textContent ?? '').trim().replace(/\s+/g, ' ')),
      scrollLeft: Math.round(body?.scrollLeft ?? 0),
      scrollWidth: Math.round(body?.scrollWidth ?? 0),
    };
  });
}

async function activeVisibleElementIndex(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll(targetSelector))
      .filter(isVisible)
      .findIndex((element) => element.className.includes('active'));
  }, selector);
}

async function plansFilterState(page) {
  return {
    activeIndex: await activePlanTabIndex(page),
    cardCount: await visibleCount(page, 'a.block-link-pop'),
    cardTitles: await visibleTexts(page, '.block-header.plan .block-title', 6),
    tabStates: await planTabStates(page),
  };
}

async function activePlanTabIndex(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('.v2board-plan-tabs span'))
      .filter(isVisible)
      .findIndex((element) => element.className.includes('active'));
  });
}

async function planTabStates(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    return Array.from(document.querySelectorAll('.v2board-plan-tabs span'))
      .filter(isVisible)
      .map((element) => ({
        className: normalizeClassName(element.className),
        text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
      }));
  });
}

async function activeTabState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const active =
      Array.from(document.querySelectorAll('.ant-tabs-tab-active')).find(isVisible) ??
      Array.from(document.querySelectorAll('.ant-tabs-tab')).find((element) =>
        element.className.includes('active'),
      );
    if (!active) return null;
    return {
      className: normalizeClassName(active.className),
      text: (active.textContent ?? '').trim().replace(/\s+/g, ' '),
    };
  });
}

async function firstElementState(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizeClassName = (value) =>
      String(value)
        .split(/\s+/)
        .filter(Boolean)
        .sort()
        .join(' ');
    const elementState = (element) => ({
      ariaChecked: element.getAttribute('aria-checked'),
      checked: Boolean(element.matches('.ant-switch-checked, [aria-checked="true"], :checked')),
      className: normalizeClassName(element.className),
      disabled: Boolean(element.matches(':disabled, .ant-switch-disabled, .ant-btn-disabled')),
      text: (element.textContent ?? '').trim().replace(/\s+/g, ' '),
      value: 'value' in element ? element.value : undefined,
    });
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    if (!element) return null;
    return elementState(element);
  }, selector);
}

async function firstInputValue(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const element = Array.from(document.querySelectorAll(targetSelector)).find(isVisible);
    return element && 'value' in element ? element.value : '';
  }, selector);
}

async function visibleInputValues(page, selector) {
  return page.evaluate((targetSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll(targetSelector))
      .filter(isVisible)
      .map((element) => ('value' in element ? element.value : ''));
  }, selector);
}

async function clickFirstVisible(page, selector) {
  await clickVisibleAt(page, selector, 0);
}

async function clickFirstVisibleText(page, selector, texts) {
  await page.evaluate(
    ({ selector: targetSelector, texts: targetTexts }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const element = Array.from(document.querySelectorAll(targetSelector)).find((candidate) => {
        const text = (candidate.textContent ?? '').trim().replace(/\s+/g, ' ');
        return isVisible(candidate) && targetTexts.includes(text);
      });
      if (!element) {
        throw new Error(`No visible element ${targetSelector} with text ${targetTexts.join(', ')}`);
      }
      element.click();
    },
    { selector, texts },
  );
}

async function clickCouponVerifyButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const input = Array.from(document.querySelectorAll('.v2board-input-coupon')).find(isVisible);
    const container = input?.closest('.block') ?? input?.parentElement;
    const button = container
      ? Array.from(container.querySelectorAll('button, .btn')).find(isVisible)
      : null;
    if (!button) {
      throw new Error('No visible coupon verify button');
    }
    button.click();
  });
}

async function clickAdminTicketsReplyFilterOption(page, text) {
  await page.evaluate((targetText) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const item = Array.from(
      document.querySelectorAll('.ant-table-filter-dropdown .ant-dropdown-menu-item'),
    ).find(
      (element) =>
        isVisible(element) &&
        (element.textContent ?? '').trim().replace(/\s+/g, ' ').includes(targetText),
    );
    if (!item) {
      throw new Error(`No visible admin ticket reply filter option ${targetText}`);
    }
    const checkbox = item.querySelector('input[type="checkbox"]');
    if (checkbox) {
      checkbox.click();
      return;
    }
    item.click();
  }, text);
}

async function clickAdminOrderRowAction(page, rowText, actionText) {
  await page.evaluate(
    ({ actionText: targetActionText, rowText: targetRowText }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const row = Array.from(document.querySelectorAll('.ant-table-tbody tr')).find(
        (element) =>
          isVisible(element) &&
          (element.textContent ?? '').trim().replace(/\s+/g, ' ').includes(targetRowText),
      );
      if (!row) {
        throw new Error(`No visible admin order row ${targetRowText}`);
      }
      const action = Array.from(row.querySelectorAll('a')).find((element) => {
        const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
        return isVisible(element) && text === targetActionText;
      });
      if (!action) {
        throw new Error(`No visible admin order row action ${targetActionText}`);
      }
      action.click();
    },
    { actionText, rowText },
  );
}

async function clickAdminTableRowDropdownAction(page, rowText, actionText) {
  await page.evaluate((targetRowText) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const row = Array.from(document.querySelectorAll('.ant-table-tbody tr')).find(
      (element) =>
        isVisible(element) &&
        (element.textContent ?? '').trim().replace(/\s+/g, ' ').includes(targetRowText),
    );
    if (!row) {
      throw new Error(`No visible admin table row ${targetRowText}`);
    }
    const trigger = Array.from(row.querySelectorAll('a')).find((element) => {
      const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
      return isVisible(element) && text.includes('操作');
    });
    if (!trigger) {
      throw new Error(`No visible admin table row operation trigger ${targetRowText}`);
    }
    trigger.click();
  }, rowText);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', actionText);
  await clickFirstVisibleText(page, '.ant-dropdown-menu-item a', [actionText]);
}

async function waitForVisibleElementsHidden(page, selector) {
  await page.waitForFunction(
    (targetSelector) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      return !Array.from(document.querySelectorAll(targetSelector)).some(isVisible);
    },
    selector,
    { timeout: 5_000 },
  );
}

async function waitForProfileSwitchLoading(page, index) {
  await page
    .waitForFunction(
      ({ index: switchIndex }) => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const element = Array.from(document.querySelectorAll('.ant-switch')).filter(isVisible)[
          switchIndex
        ];
        return Boolean(
          element &&
            (element.matches('.ant-switch-loading, .ant-switch-disabled, :disabled') ||
              element.querySelector('.ant-switch-loading-icon')),
        );
      },
      { index },
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}

async function clickProfileRedeemGiftcardButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const input = Array.from(document.querySelectorAll('input')).find((element) => {
      const placeholder = element.getAttribute('placeholder') ?? '';
      return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
    });
    const block = input?.closest('.block') ?? null;
    const button = block
      ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
      : null;
    if (!button) {
      throw new Error('No visible profile giftcard redeem button');
    }
    button.click();
  });
}

async function waitForProfileRedeemGiftcardLoading(page) {
  await page
    .waitForFunction(
      () => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const input = Array.from(document.querySelectorAll('input')).find((element) => {
          const placeholder = element.getAttribute('placeholder') ?? '';
          return isVisible(element) && /Gift Card|礼品卡/.test(placeholder);
        });
        const block = input?.closest('.block') ?? null;
        const button = block
          ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
          : null;
        return Boolean(
          button &&
            (button.matches('.ant-btn-loading, :disabled, .ant-btn-disabled') ||
              button.querySelector('.anticon-loading, .fa-spin')),
        );
      },
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}

async function clickProfileChangePasswordButton(page) {
  await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const block = Array.from(document.querySelectorAll('.block')).find((element) => {
      const title = element.querySelector('.block-title')?.textContent ?? '';
      return isVisible(element) && /Change Password|修改密码/.test(title);
    });
    const button = block
      ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
      : null;
    if (!button) {
      throw new Error('No visible profile change password button');
    }
    button.click();
  });
}

async function fillProfileChangePasswordInputs(page, values) {
  const inputIndexes = await page.evaluate((expectedCount) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const allInputs = Array.from(document.querySelectorAll('input'));
    const block = Array.from(document.querySelectorAll('.block')).find((element) => {
      const title = element.querySelector('.block-title')?.textContent ?? '';
      return isVisible(element) && /Change Password|修改密码/.test(title);
    });
    const inputs = block ? Array.from(block.querySelectorAll('input')).filter(isVisible) : [];
    if (inputs.length < expectedCount) {
      throw new Error(`Expected ${expectedCount} profile password inputs, got ${inputs.length}`);
    }
    return inputs.slice(0, expectedCount).map((input) => allInputs.indexOf(input));
  }, values.length);

  for (let index = 0; index < values.length; index += 1) {
    await page.locator('input').nth(inputIndexes[index]).fill(values[index]);
  }
}

async function waitForProfileChangePasswordLoading(page) {
  await page
    .waitForFunction(
      () => {
        const isVisible = (element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        };
        const block = Array.from(document.querySelectorAll('.block')).find((element) => {
          const title = element.querySelector('.block-title')?.textContent ?? '';
          return isVisible(element) && /Change Password|修改密码/.test(title);
        });
        const button = block
          ? Array.from(block.querySelectorAll('button')).find(isVisible) ?? null
          : null;
        return Boolean(
          button &&
            (button.matches('.ant-btn-loading, :disabled, .ant-btn-disabled') ||
              button.querySelector('.anticon-loading, .fa-spin')),
        );
      },
      { timeout: 5_000 },
    )
    .catch(() => undefined);
}

async function waitForPagePropertyAtLeast(page, property, minimum, timeout = 5_000) {
  const startedAt = Date.now();
  while ((page[property] ?? 0) < minimum) {
    if (Date.now() - startedAt > timeout) {
      throw new Error(`${property} did not reach ${minimum}`);
    }
    await delay(50);
  }
}

async function clickVisibleAt(page, selector, index) {
  await page.evaluate(
    ({ index: targetIndex, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const element = Array.from(document.querySelectorAll(targetSelector)).filter(isVisible)[
        targetIndex
      ];
      if (!element) {
        throw new Error(`No visible element ${targetSelector} at index ${targetIndex}`);
      }
      element.click();
    },
    { index, selector },
  );
}

async function visibleElementDomIndex(page, selector, index) {
  return page.evaluate(
    ({ index: targetIndex, selector: targetSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const elements = Array.from(document.querySelectorAll(targetSelector));
      const element = elements.filter(isVisible)[targetIndex];
      if (!element) {
        throw new Error(`No visible element ${targetSelector} at index ${targetIndex}`);
      }
      return elements.indexOf(element);
    },
    { index, selector },
  );
}

async function fillFirstVisible(page, selector, value) {
  await fillVisibleAt(page, selector, 0, value);
}

async function fillVisibleAt(page, selector, index, value) {
  const domIndex = await visibleElementDomIndex(page, selector, index);
  await page.locator(selector).nth(domIndex).fill(value);
}

async function captureScenarioWithFreshBrowser(url, scenario, viewport, target) {
  const browser = await chromium.launch({ args: chromiumArgs, headless: true });
  try {
    return await captureScenario(browser, url, scenario, viewport, target);
  } finally {
    await browser.close();
  }
}

async function captureScenario(browser, url, scenario, viewport, target) {
  const context = await browser.newContext({ viewport });
  const page = await context.newPage();
  try {
    return await capturePage(page, url, scenario, target);
  } finally {
    await context.close();
  }
}

async function capturePage(page, url, scenario, target) {
  const diagnostics = [];
  page.__visualParityDiagnostics = diagnostics;
  page.on('console', (message) => {
    const location = message.location();
    const where = location.url
      ? ` (${location.url}:${location.lineNumber}:${location.columnNumber})`
      : '';
    diagnostics.push(`${message.type()}: ${message.text()}${where}`);
  });
  page.on('pageerror', (error) => {
    diagnostics.push(`pageerror: ${error.message}`);
  });
  page.on('requestfailed', (request) => {
    diagnostics.push(`requestfailed ${request.method()} ${request.url()}: ${request.failure()?.errorText}`);
  });
  page.on('response', (response) => {
    if (response.status() >= 400) {
      diagnostics.push(`response ${response.status()} ${response.url()}`);
    }
  });
  await installApiFixtures(page, scenario);
  if (scenario.warmupPath) {
    await gotoStable(page, new URL(scenario.warmupPath, url).toString());
    if (target === 'oracle' && scenario.seedLegacyAdminStore) {
      await seedLegacyAdminStore(page);
    }
    await navigateAfterWarmup(page, url);
  } else {
    await gotoStable(page, url);
  }
  if (target === 'oracle' && scenario.seedLegacyAdminStore) {
    await seedLegacyAdminStore(page);
  }
  if (scenario.readySelector) {
    await page
      .waitForSelector(scenario.readySelector, { state: 'visible', timeout: 10_000 })
      .catch(async (error) => {
        const snapshot = await readDebugSnapshot(page);
        throw new Error(
          `${error.message}\n` +
            `URL: ${snapshot.url}\n` +
            `Title: ${snapshot.title}\n` +
            `Body: ${snapshot.body}\n` +
            `Diagnostics: ${diagnostics.slice(-40).join(' | ')}`,
        );
      });
  }
  if (scenario.postReadyDelay) {
    await page.waitForTimeout(scenario.postReadyDelay);
  }
  if (scenario.darkMode) {
    await waitForDarkReader(page, diagnostics);
  }
  const mountedState = await waitForMountedContent(page, diagnostics);
  diagnostics.push(`mounted content visible ${formatMountedContentState(mountedState)}`);
  await waitForFontsBeforeCapture(page, diagnostics);
  await waitForFixedColumnLayout(page);
  await assertMountedContentStillVisible(page, diagnostics);
  const metrics = await page.evaluate((selectors) => {
    const body = document.body;
    const root = document.querySelector('#root') ?? body;
    const rootRect = root.getBoundingClientRect();

    const round = (value) => Math.round(value * 1000) / 1000;
    const elements = selectors.flatMap((selector) =>
      Array.from(document.querySelectorAll(selector))
        .slice(0, 5)
        .map((element, index) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          const beforeStyle = window.getComputedStyle(element, '::before');
          return {
            backgroundColor: style.backgroundColor,
            beforeColor: beforeStyle.color,
            beforeContent: beforeStyle.content,
            beforeFontFamily: beforeStyle.fontFamily,
            beforeFontSize: beforeStyle.fontSize,
            beforeFontWeight: beforeStyle.fontWeight,
            borderRadius: style.borderRadius,
            boxSizing: style.boxSizing,
            color: style.color,
            display: style.display,
            fontFeatureSettings: style.fontFeatureSettings,
            fontFamily: style.fontFamily,
            fontKerning: style.fontKerning,
            fontSize: style.fontSize,
            fontWeight: style.fontWeight,
            height: round(rect.height),
            index,
            letterSpacing: style.letterSpacing,
            lineHeight: style.lineHeight,
            margin: [
              style.marginTop,
              style.marginRight,
              style.marginBottom,
              style.marginLeft,
            ].join(' '),
            padding: [
              style.paddingTop,
              style.paddingRight,
              style.paddingBottom,
              style.paddingLeft,
            ].join(' '),
            position: style.position,
            selector,
            text: (element.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 80),
            textRendering: style.textRendering,
            top: round(rect.top),
            webkitFontSmoothing: style.webkitFontSmoothing,
            width: round(rect.width),
            x: round(rect.x),
          };
        }),
    );

    return {
      bodyClass: body.className,
      darkReaderMode: document.documentElement.getAttribute('data-darkreader-mode'),
      darkReaderScheme: document.documentElement.getAttribute('data-darkreader-scheme'),
      darkReaderStyles: document.querySelectorAll('.darkreader').length,
      elements,
      rootHeight: rootRect.height,
      rootWidth: rootRect.width,
      text: (body.innerText ?? '').trim().slice(0, 200),
      title: document.title,
      visibleElements: Array.from(body.querySelectorAll('*')).filter((element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      }).length,
    };
  }, metricSelectors);
  const screenshot = await captureViewportPng(page);
  return { diagnostics: diagnostics.slice(-80), metrics, screenshot };
}

async function waitForMountedContent(page, diagnostics) {
  await page
    .waitForFunction(
      () => {
        const body = document.body;
        if (!body) return false;
        const root = document.querySelector('#root') ?? body;
        const rootRect = root.getBoundingClientRect();
        const hasVisibleElement = Array.from(body.querySelectorAll('*')).some((element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        });
        return (
          rootRect.width > 0 &&
          rootRect.height > 0 &&
          hasVisibleElement &&
          (body.innerText ?? '').trim().length > 0
        );
      },
      null,
      { timeout: 10_000 },
    )
    .catch(async (error) => {
      const snapshot = await readDebugSnapshot(page);
      throw new Error(
        `Mounted content did not become visible: ${error.message}\n` +
          `URL: ${snapshot.url}\n` +
          `Title: ${snapshot.title}\n` +
          `Body: ${snapshot.body}\n` +
          `Diagnostics: ${diagnostics.slice(-80).join(' | ')}`,
      );
    });
  return readMountedContentState(page);
}

async function assertMountedContentStillVisible(page, diagnostics) {
  const state = await readMountedContentState(page);
  if (isMountedContentStateVisible(state)) {
    return;
  }
  const snapshot = await readDebugSnapshot(page);
  throw new Error(
    `Mounted content disappeared before screenshot: ${formatMountedContentState(state)}\n` +
      `URL: ${snapshot.url}\n` +
      `Title: ${snapshot.title}\n` +
      `Body: ${snapshot.body}\n` +
      `Diagnostics: ${diagnostics.slice(-80).join(' | ')}`,
  );
}

async function readMountedContentState(page) {
  return page.evaluate(() => {
    const body = document.body;
    const root = document.querySelector('#root') ?? body;
    const rootRect = root?.getBoundingClientRect();
    const visibleElements = body
      ? Array.from(body.querySelectorAll('*')).filter((element) => {
          const rect = element.getBoundingClientRect();
          const style = window.getComputedStyle(element);
          return rect.width > 0 && rect.height > 0 && style.display !== 'none';
        }).length
      : 0;
    return {
      bodyTextLength: (body?.innerText ?? '').trim().length,
      rootChildCount: root?.children.length ?? 0,
      rootHeight: rootRect?.height ?? 0,
      rootHtmlLength: root?.innerHTML.length ?? 0,
      rootWidth: rootRect?.width ?? 0,
      scripts: Array.from(document.scripts)
        .slice(-8)
        .map((script) => script.src || script.textContent?.slice(0, 80) || ''),
      url: window.location.href,
      visibleElements,
    };
  });
}

function isMountedContentStateVisible(state) {
  return (
    state.rootWidth > 0 &&
    state.rootHeight > 0 &&
    state.visibleElements > 0 &&
    state.bodyTextLength > 0
  );
}

function formatMountedContentState(state) {
  return JSON.stringify({
    bodyTextLength: state.bodyTextLength,
    rootChildCount: state.rootChildCount,
    rootHeight: Math.round(state.rootHeight * 1000) / 1000,
    rootHtmlLength: state.rootHtmlLength,
    rootWidth: Math.round(state.rootWidth * 1000) / 1000,
    scripts: state.scripts,
    url: state.url,
    visibleElements: state.visibleElements,
  });
}

async function waitForFontsBeforeCapture(page, diagnostics) {
  const snapshot = await page
    .evaluate(async (timeout) => {
      if (!('fonts' in document)) return { status: 'unsupported', wait: 'unsupported' };
      const fontSet = document.fonts;
      const snapshot = (wait) => ({
        faces: Array.from(fontSet)
          .slice(0, 20)
          .map((font) => ({
            family: font.family,
            status: font.status,
            style: font.style,
            weight: font.weight,
          })),
        status: fontSet.status,
        wait,
      });
      if (fontSet.status === 'loaded') return snapshot('already-loaded');
      const wait = await Promise.race([
        fontSet.ready.then(() => 'ready'),
        new Promise((resolve) => {
          setTimeout(() => resolve('timeout'), timeout);
        }),
      ]);
      return snapshot(wait);
    }, fontWaitTimeout)
    .catch((error) => ({ error: error.message, status: 'error', wait: 'error' }));

  if (!['already-loaded', 'ready'].includes(snapshot.wait) || snapshot.status !== 'loaded') {
    diagnostics.push(`font wait ${JSON.stringify(snapshot)}`);
  }
}

async function waitForFixedColumnLayout(page) {
  await page.evaluate(async () => {
    const minimumObservationMs = 500;
    const timeoutMs = 1500;
    const fixedRows = () =>
      Array.from(
        document.querySelectorAll(
          '.ant-table-fixed-left .ant-table-row, .ant-table-fixed-right .ant-table-row',
        ),
      );
    const readSignature = () =>
      fixedRows()
        .map((row) => {
          const rect = row.getBoundingClientRect();
          return `${Math.round(rect.top * 1000) / 1000}:${Math.round(rect.height * 1000) / 1000}`;
        })
        .join('|');
    const nextFrame = () => new Promise((resolve) => requestAnimationFrame(resolve));

    if (!fixedRows().length) {
      await nextFrame();
      await nextFrame();
      return;
    }

    const startedAt = performance.now();
    let previous = '';
    let stableFrames = 0;
    while (performance.now() - startedAt < timeoutMs) {
      await nextFrame();
      const current = readSignature();
      stableFrames = current === previous ? stableFrames + 1 : 0;
      previous = current;
      if (performance.now() - startedAt >= minimumObservationMs && stableFrames >= 4) {
        return;
      }
    }
  });
}

async function captureViewportPng(page) {
  await page.addStyleTag({ content: captureStabilityStyle }).catch(() => undefined);
  const session = await page.context().newCDPSession(page);
  try {
    const { data } = await session.send('Page.captureScreenshot', {
      captureBeyondViewport: false,
      format: 'png',
      fromSurface: true,
    });
    return Buffer.from(data, 'base64');
  } finally {
    await session.detach().catch(() => undefined);
  }
}

async function waitForDarkReader(page, diagnostics) {
  await page
    .waitForFunction(
      () =>
        document.documentElement.getAttribute('data-darkreader-mode') === 'dynamic' &&
        document.documentElement.getAttribute('data-darkreader-scheme') === 'dark' &&
        document.querySelectorAll('.darkreader').length > 0,
      null,
      { timeout: 10_000 },
    )
    .catch(async (error) => {
      const snapshot = await readDebugSnapshot(page);
      const state = await page
        .evaluate(() => ({
          mode: document.documentElement.getAttribute('data-darkreader-mode'),
          scheme: document.documentElement.getAttribute('data-darkreader-scheme'),
          styles: document.querySelectorAll('.darkreader').length,
        }))
        .catch((stateError) => ({ error: stateError.message }));
      throw new Error(
        `DarkReader did not become ready: ${error.message}\n` +
          `URL: ${snapshot.url}\n` +
          `Title: ${snapshot.title}\n` +
          `Body: ${snapshot.body}\n` +
          `State: ${JSON.stringify(state)}\n` +
          `Diagnostics: ${diagnostics.slice(-40).join(' | ')}`,
      );
    });

  const state = await page.evaluate(() => ({
    mode: document.documentElement.getAttribute('data-darkreader-mode'),
    scheme: document.documentElement.getAttribute('data-darkreader-scheme'),
    styles: document.querySelectorAll('.darkreader').length,
  }));
  diagnostics.push(`darkreader ready ${JSON.stringify(state)}`);
  await page.waitForTimeout(500);
}

async function readDebugSnapshot(page) {
  const [title, body] = await Promise.all([
    page.title().catch((error) => `title error: ${error.message}`),
    page
      .locator('body')
      .innerText({ timeout: 1_000 })
      .catch((error) => `body error: ${error.message}`),
  ]);
  return {
    body: body.trim().replace(/\s+/g, ' ').slice(0, 500),
    title,
    url: page.url(),
  };
}

async function installApiFixtures(page, scenario, target, interaction = {}) {
  const isAdminScenario = scenario.label.startsWith('admin-');
  let seededAdminTicketDetailStore = false;
  let resolveAdminGroupsReady;
  const adminGroupsReady = new Promise((resolve) => {
    resolveAdminGroupsReady = resolve;
  });
  let adminGroupsResolved = false;

  await page.addInitScript(
    ({ authenticated, darkMode, preserveRuntimeDarkMode }) => {
      const initializeDarkModeCookie = () => {
        document.cookie = darkMode
          ? 'dark_mode=1;path=/'
          : 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
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
    },
    {
      authenticated: Boolean(scenario.authenticated),
      darkMode: Boolean(scenario.darkMode),
      preserveRuntimeDarkMode: Boolean(interaction.preserveRuntimeDarkMode),
    },
  );

  await page.route('https://js.stripe.com/v3**', (route) => {
    route.fulfill({
      body: stripeFixtureScript(),
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
      page.__visualParityUserTransferCount =
        (page.__visualParityUserTransferCount ?? 0) + 1;
      page.__visualParityUserTransferRequests = [
        ...(page.__visualParityUserTransferRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/newPeriod') {
      page.__visualParityLastUserNewPeriod = requestData;
      page.__visualParityUserNewPeriodCount =
        (page.__visualParityUserNewPeriodCount ?? 0) + 1;
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
      page.__visualParityUserOrderFetchCount =
        (page.__visualParityUserOrderFetchCount ?? 0) + 1;
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
    if (pathname === '/api/v1/user/comm/getStripePublicKey') {
      page.__visualParityUserStripePublicKeyCount =
        (page.__visualParityUserStripePublicKeyCount ?? 0) + 1;
      page.__visualParityUserStripePublicKeyRequests = [
        ...(page.__visualParityUserStripePublicKeyRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/fetch') {
      page.__visualParityUserTicketFetchCount =
        (page.__visualParityUserTicketFetchCount ?? 0) + 1;
    }
    if (pathname === '/api/v1/user/ticket/reply') {
      page.__visualParityLastUserTicketReply = requestData;
      page.__visualParityUserTicketReplyCount =
        (page.__visualParityUserTicketReplyCount ?? 0) + 1;
      page.__visualParityUserTicketReplyRequests = [
        ...(page.__visualParityUserTicketReplyRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/save') {
      page.__visualParityLastUserTicketSave = requestData;
      page.__visualParityUserTicketSaveCount =
        (page.__visualParityUserTicketSaveCount ?? 0) + 1;
      page.__visualParityUserTicketSaveRequests = [
        ...(page.__visualParityUserTicketSaveRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/ticket/withdraw') {
      page.__visualParityLastUserWithdraw = requestData;
      page.__visualParityUserWithdrawCount =
        (page.__visualParityUserWithdrawCount ?? 0) + 1;
      page.__visualParityUserWithdrawRequests = [
        ...(page.__visualParityUserWithdrawRequests ?? []),
        requestData,
      ];
    }
    if (pathname === '/api/v1/user/order/cancel') {
      page.__visualParityLastUserOrderCancel = requestData;
      page.__visualParityUserOrderCancelCount =
        (page.__visualParityUserOrderCancelCount ?? 0) + 1;
      page.__visualParityUserOrderCancelRequests = [
        ...(page.__visualParityUserOrderCancelRequests ?? []),
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
      page.__visualParityLastAdminOrderFetchQuery = Object.fromEntries(requestUrl.searchParams.entries());
    }
    if (adminEndpoint === '/plan/fetch') {
      page.__visualParityAdminPlanFetchCount =
        (page.__visualParityAdminPlanFetchCount ?? 0) + 1;
    }
    if (adminEndpoint === '/plan/save') {
      page.__visualParityLastAdminPlanSave = requestData;
      page.__visualParityAdminPlanSaveCount =
        (page.__visualParityAdminPlanSaveCount ?? 0) + 1;
      page.__visualParityAdminPlanSaveRequests = [
        ...(page.__visualParityAdminPlanSaveRequests ?? []),
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
      page.__visualParityAdminNoticeSaveCount =
        (page.__visualParityAdminNoticeSaveCount ?? 0) + 1;
      page.__visualParityAdminNoticeSaveRequests = [
        ...(page.__visualParityAdminNoticeSaveRequests ?? []),
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
      page.__visualParityLastAdminUserFetchQuery = Object.fromEntries(
        requestUrl.searchParams.entries(),
      );
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
    if (adminEndpoint === '/server/group/save' && interaction.delayAdminServerGroupSaveMs) {
      await delay(interaction.delayAdminServerGroupSaveMs);
    }
    if (pathname === '/api/v1/user/unbindTelegram' && interaction.delayUserUnbindTelegramMs) {
      await delay(interaction.delayUserUnbindTelegramMs);
    }
    await fulfillApiResponse(
      route,
      apiFixtureResponse(requestUrl, isAdminScenario, scenario, requestData, interaction),
    );

    if (adminEndpoint === '/server/group/fetch' && !adminGroupsResolved) {
      adminGroupsResolved = true;
      resolveAdminGroupsReady();
    }
  });
}

function adminFixtureEndpoint(pathname) {
  const prefix = `/api/v1/${adminPath}`;
  return pathname.startsWith(`${prefix}/`) ? pathname.slice(prefix.length) : null;
}

function readRequestData(request) {
  const raw = request.postData();
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return Object.fromEntries(new URLSearchParams(raw));
  }
}

function apiFixtureResponse(
  requestUrl,
  isAdminScenario,
  scenario = { label: '' },
  requestData = null,
  interaction = {},
) {
  const pathname = requestUrl.pathname;
  const adminEndpoint = adminFixtureEndpoint(pathname);
  const body = (data, extra = {}) => ({ code: 200, data, ...extra });

  if (adminEndpoint) {
    switch (adminEndpoint) {
      case '/config/fetch':
        return body(adminConfigFixture);
      case '/config/getEmailTemplate':
        return body(adminEmailTemplateFixtures);
      case '/config/getThemeTemplate':
        return body(adminThemeTemplateFixtures);
      case '/theme/getThemes':
        return body(adminThemeFixtures);
      case '/theme/getThemeConfig':
        return body({ homepage: 'V2Board' });
      case '/coupon/fetch':
        return body(adminCouponFixtures, { total: adminCouponFixtures.length });
      case '/coupon/generate':
        return body(true);
      case '/giftcard/fetch':
        return body(adminGiftcardFixtures, { total: adminGiftcardFixtures.length });
      case '/giftcard/generate':
        return body(true);
      case '/knowledge/fetch':
        return body(
          requestUrl.searchParams.has('id')
            ? adminKnowledgeFixtures.find(
                (knowledge) => String(knowledge.id) === requestUrl.searchParams.get('id'),
              ) ?? adminKnowledgeFixtures[0]
            : adminKnowledgeFixtures,
        );
      case '/knowledge/getCategory':
        return body(Array.from(new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category))));
      case '/knowledge/save':
        return body(true);
      case '/notice/fetch':
        return body(adminNoticeFixtures, { total: adminNoticeFixtures.length });
      case '/notice/save':
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
        return body(planFixtures);
      case '/plan/save':
        return body(true);
      case '/payment/fetch':
        return body(adminPaymentFixtures);
      case '/payment/save':
        return body(true);
      case '/payment/getPaymentMethods':
        return body(adminPaymentMethodsFixture);
      case '/payment/getPaymentForm': {
        const requestedPayment =
          requestData && typeof requestData.payment === 'string'
            ? requestData.payment
            : adminPaymentMethodsFixture[0];
        return body(adminPaymentFormFixtures[requestedPayment] ?? adminPaymentFormFixtures.AlipayF2F);
      }
      case '/server/group/fetch':
        return body(adminServerGroupFixtures);
      case '/server/group/save':
        return body(true);
      case '/server/manage/getNodes':
        return body(adminServerNodeFixtures);
      case '/server/route/fetch':
        return body(adminServerRouteFixtures);
      case '/system/getQueueStats':
        return body(adminQueueStatsFixture);
      case '/system/getQueueWorkload':
        return body(adminQueueWorkloadFixtures);
      case '/order/fetch':
        return body(adminOrderFixtures, { total: adminOrderFixtures.length });
      case '/order/detail': {
        const requestedId = requestData?.id == null ? 1 : Number(requestData.id);
        return body(
          adminOrderFixtures.find((order) => order.id === requestedId) ?? adminOrderFixtures[0],
        );
      }
      case '/order/assign':
        return body('VISUAL2026110099');
      case '/order/paid':
      case '/order/cancel':
      case '/order/update':
        return body(true);
      case '/user/fetch':
        return body(adminUserFixtures, { total: adminUserFixtures.length });
      case '/user/getUserInfoById': {
        const requestedId = requestUrl.searchParams.has('id')
          ? Number(requestUrl.searchParams.get('id'))
          : 1;
        return body(adminUserFixtures.find((user) => user.id === requestedId) ?? adminUserFixtures[0]);
      }
      case '/stat/getStatUser':
        return body(trafficFixtures, { total: 25 });
      case '/ticket/fetch':
        if (requestUrl.searchParams.has('id')) {
          const requestedId = requestUrl.searchParams.get('id') ?? '7';
          const ticket =
            scenario.label === 'admin-ticket-detail'
              ? adminTicketDetailFixture
              : adminTicketFixtures.find((item) => String(item.id) === requestedId) ??
                adminTicketFixtures[0];
          return body(ticket);
        }
        return body(adminTicketFixtures, { total: adminTicketFixtures.length });
      case '/ticket/reply':
        return body(true);
      default:
        return body(null);
    }
  }

  switch (pathname) {
    case '/api/v1/guest/comm/config':
      return body(guestConfigFixture);
    case '/api/v1/passport/auth/login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
        token: 'visual-parity-token',
      });
    case '/api/v1/passport/auth/token2Login':
      return body({
        auth_data: 'VISUAL_PARITY_TOKEN',
        is_admin: isAdminScenario,
        token: 'visual-parity-token',
      });
    case '/api/v1/user/checkLogin':
      return body({ is_admin: isAdminScenario, is_login: true });
    case '/api/v1/user/info':
      return body(
        interaction?.telegramBoundProfile
          ? { ...userInfoFixture, telegram_id: 12345 }
          : userInfoFixture,
      );
    case '/api/v1/user/update':
      return body(true);
    case '/api/v1/user/redeemgiftcard':
      return body(true, { type: 1, value: 1234 });
    case '/api/v1/user/changePassword':
      return body(true);
    case '/api/v1/user/transfer':
      return body(true);
    case '/api/v1/user/resetSecurity':
      return body('VISUAL-RESET-UUID');
    case '/api/v1/user/unbindTelegram':
      return body(true);
    case '/api/v1/user/getSubscribe':
      return body(interaction?.newPeriodSubscribe ? newPeriodSubscribeFixture : subscribeFixture);
    case '/api/v1/user/getStat':
      return body([2, 3, 0]);
    case '/api/v1/user/plan/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? planFixtures.find((plan) => String(plan.id) === requestUrl.searchParams.get('id')) ??
              planFixtures[0]
          : planFixtures,
      );
    case '/api/v1/user/order/save':
      if (requestData?.period === 'deposit') return body(profileDepositTradeNo);
      if (requestData?.period === 'reset_price') return body(dashboardResetPackageTradeNo);
      return body('VISUAL2026110099');
    case '/api/v1/user/newPeriod':
      return body(true);
    case '/api/v1/user/order/fetch':
      return body(orderFixtures);
    case '/api/v1/user/order/detail':
      return body(
        requestUrl.searchParams.get('trade_no') === dashboardResetPackageTradeNo
          ? dashboardResetPackageOrderFixture
          : requestUrl.searchParams.get('trade_no') === profileDepositTradeNo
          ? profileDepositOrderFixture
          : orderFixtures.find((order) => order.trade_no === requestUrl.searchParams.get('trade_no')) ??
              orderFixtures[0],
      );
    case '/api/v1/user/order/cancel':
      return body(true);
    case '/api/v1/user/order/getPaymentMethod':
      return body(paymentMethodFixtures);
    case '/api/v1/user/order/checkout': {
      const methodId = Number(requestData?.method);
      if (methodId === 2) {
        return body('stripe-accepted', { type: 1 });
      }
      if (methodId === 3) {
        return body(
          interaction.checkoutRedirectUrl ?? '/#/order/VISUAL2026110001?cashier=visual',
          { type: 1 },
        );
      }
      return body('https://pay.example.test/qr/VISUAL2026110001', { type: 0 });
    }
    case '/api/v1/user/order/check':
      return body(0);
    case '/api/v1/user/coupon/check':
      return body(couponCheckFixture);
    case '/api/v1/user/server/fetch':
      return body(serverFixtures);
    case '/api/v1/user/stat/getTrafficLog':
      return body(trafficFixtures);
    case '/api/v1/user/invite/fetch':
      return body(inviteFixture);
    case '/api/v1/user/invite/details':
      return body(inviteDetailFixtures, { total: inviteDetailFixtures.length });
    case '/api/v1/user/invite/save':
      return body(true);
    case '/api/v1/user/ticket/fetch':
      return body(requestUrl.searchParams.has('id') ? ticketDetailFixture : ticketFixtures);
    case '/api/v1/user/ticket/save':
      return body(true);
    case '/api/v1/user/ticket/reply':
      return body(true);
    case '/api/v1/user/ticket/withdraw':
      return body(true);
    case '/api/v1/user/knowledge/fetch':
      return body(
        requestUrl.searchParams.has('id')
          ? userKnowledgeFixtureById(requestUrl.searchParams.get('id'))
          : knowledgeFixtures,
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
    case '/api/v1/user/telegram/getBotInfo':
      return body({ username: 'legacy_bot' });
    default:
      return body(null);
  }
}

function userKnowledgeFixtureById(id) {
  return (
    Object.values(knowledgeFixtures)
      .flat()
      .find((knowledge) => String(knowledge.id) === String(id)) ?? knowledgeFixtures.General[0]
  );
}

function legacyScaledFixed(value, divisor) {
  return (Number(value) / divisor).toFixed(2);
}

async function waitForAdminGroups(adminGroupsReady) {
  await Promise.race([adminGroupsReady, delay(1_000)]);
  await delay(300);
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function fulfillApiResponse(route, body) {
  return route.fulfill({
    body: JSON.stringify(body),
    contentType: 'application/json',
    status: 200,
  });
}

function fulfillPlainJson(route, data) {
  route.fulfill({
    body: JSON.stringify(data),
    contentType: 'application/json',
    status: 200,
  });
}

function stripeFixtureScript() {
  return `
(() => {
  window.Stripe = function Stripe() {
    let lastElement = null;
    const createElement = () => {
      const handlers = new Map();
      const fire = (event, payload) => {
        const eventHandlers = handlers.get(event) || [];
        eventHandlers.forEach((handler) => handler(payload));
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
        },
        off(event, handler) {
          const eventHandlers = handlers.get(event) || [];
          handlers.set(event, eventHandlers.filter((item) => item !== handler));
        },
        on(event, handler) {
          handlers.set(event, [...(handlers.get(event) || []), handler]);
        },
        unmount() {},
        update() {},
      };
    };
    return {
      _registerWrapper() {},
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
      createToken() {
        return Promise.resolve({});
      },
    };
  };
})();
`;
}

async function gotoStable(page, url) {
  let lastError;

  for (let attempt = 1; attempt <= navigationAttempts; attempt += 1) {
    try {
      const response = await page.goto(url, {
        timeout: navigationTimeout,
        waitUntil: 'domcontentloaded',
      });
      if (!response?.ok()) {
        throw new Error(`${url} returned ${response?.status() ?? 'no response'}`);
      }
      await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
      await page.waitForTimeout(800);
      return;
    } catch (error) {
      lastError = error;
      page.__visualParityDiagnostics?.push(
        `navigation attempt ${attempt}/${navigationAttempts} failed: ${error.message}`,
      );
      if (attempt === navigationAttempts) break;
      await page.goto('about:blank', { timeout: 5_000, waitUntil: 'domcontentloaded' }).catch(
        () => undefined,
      );
      await page.waitForTimeout(500 * attempt);
    }
  }

  throw lastError ?? new Error(`${url} navigation failed`);
}

async function navigateAfterWarmup(page, url) {
  const targetUrl = new URL(url);
  const currentUrl = new URL(page.url());

  if (currentUrl.origin === targetUrl.origin && currentUrl.pathname === targetUrl.pathname) {
    await page.evaluate((hash) => {
      window.location.hash = hash;
    }, targetUrl.hash);
    await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => undefined);
    await page.waitForTimeout(800);
    return;
  }

  await gotoStable(page, url);
}

async function seedLegacyAdminTicketDetailStore(page) {
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

async function seedLegacyAdminStore(page) {
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
        stat,
        themes,
        themeTemplates,
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
        store.dispatch({
          type: 'user/setState',
          payload: {
            pagination: { current: 1, pageSize: 10, total: users.length },
            userInfo,
            users,
          },
        });
        store.dispatch({ type: 'stat/save', payload: stat });
        store.dispatch({
          type: 'config/setState',
          payload: {
            ...config,
            emailTemplate: emailTemplates,
            themeTemplate: themeTemplates,
          },
        });
        store.dispatch({
          type: 'coupon/setState',
          payload: {
            coupons,
            pagination: { current: 1, pageSize: 10, total: coupons.length },
          },
        });
        store.dispatch({
          type: 'giftcard/setState',
          payload: {
            giftcards,
            pagination: { current: 1, pageSize: 10, total: giftcards.length },
          },
        });
        store.dispatch({
          type: 'order/setState',
          payload: {
            orders,
            pagination: { current: 0, pageSize: 10, total: orders.length },
          },
        });
        store.dispatch({ type: 'payment/setState', payload: { payments } });
        store.dispatch({ type: 'plan/setState', payload: { plans } });
        store.dispatch({ type: 'theme/setState', payload: themes });
        store.dispatch({
          type: 'system/save',
          payload: { queueStats, queueWorkload },
        });
        store.dispatch({
          type: 'knowledge/setState',
          payload: { categorys: knowledgeCategories, knowledges },
        });
        store.dispatch({ type: 'notice/setState', payload: { notices } });
        store.dispatch({ type: 'serverGroup/setState', payload: { groups: serverGroups } });
        store.dispatch({
          type: 'serverManage/setState',
          payload: { fetchLoading: false, servers: serverNodes, sortMode: false },
        });
        store.dispatch({
          type: 'serverRoute/setState',
          payload: { fetchLoading: false, routes: serverRoutes },
        });
        store.dispatch({
          type: 'ticket/setState',
          payload: {
            filter: { status: 0 },
            pagination: { current: 1, pageSize: 10, total: tickets.length },
            ticket: ticketDetail,
            tickets,
          },
        });
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
        orders: adminOrderFixtures,
        payments: adminPaymentFixtures,
        plans: adminPlanStoreFixtures,
        queueStats: adminQueueStatsFixture,
        queueWorkload: adminQueueWorkloadFixtures,
        serverGroups: adminServerGroupFixtures,
        serverNodes: adminServerNodeFixtures,
        serverRoutes: adminServerRouteFixtures,
        stat: adminStatFixture,
        themes: adminThemeFixtures,
        themeTemplates: adminThemeTemplateFixtures,
        ticketDetail: adminTicketDetailFixture,
        tickets: adminTicketFixtures,
        userInfo: userInfoFixture,
        users: adminUserStoreFixtures,
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

async function startOracleServer(port = 0, host = '127.0.0.1', advertisedHost = host) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? '/', 'http://127.0.0.1');
    const pathname = decodeURIComponent(url.pathname);

    if (pathname === '/' || pathname === '/index.html') {
      sendHtml(response, legacyUserHtml());
      return;
    }

    if (pathname === `/${adminPath}` || pathname === '/admin') {
      sendHtml(response, legacyAdminHtml());
      return;
    }

    if (pathname === '/monitor/api/stats') {
      sendJson(response, { status: 'running' });
      return;
    }

    if (pathname.startsWith('/api/v1/')) {
      const referer = request.headers.referer ?? '';
      const isAdminScenario =
        Boolean(adminFixtureEndpoint(pathname)) || referer.includes(`/${adminPath}`);
      sendJson(response, apiFixtureResponse(url, isAdminScenario));
      return;
    }

    if (pathname.startsWith('/api/')) {
      sendJson(response, { code: 200, data: null });
      return;
    }

    await sendStaticFile(response, pathname);
  });

  await new Promise((resolveListen) => server.listen(port, host, resolveListen));
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('Oracle server did not bind a port');

  return {
    baseUrl: new URL(`http://${advertisedHost}:${address.port}`),
    close: () => new Promise((resolveClose) => server.close(resolveClose)),
  };
}

function waitForShutdown() {
  return new Promise((resolveShutdown) => {
    process.once('SIGINT', resolveShutdown);
    process.once('SIGTERM', resolveShutdown);
  });
}

function legacyUserHtml() {
  const settings = sourceSettings.user;
  const color = {
    black: '#343a40',
    darkblue: '#3b5998',
    default: '#0665d0',
    green: '#319795',
  }[settings.theme.color] ?? '#0665d0';

  return `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="/theme/default/assets/components.chunk.css?v=oracle">
  <link rel="stylesheet" href="/theme/default/assets/umi.css?v=oracle">
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
  <meta name="theme-color" content="${color}">
  <title>${escapeHtml(settings.title)}</title>
  <script>window.routerBase = "/";</script>
  <script>
    window.settings = {
      title: ${jsString(settings.title)},
      assets_path: '/theme/default/assets',
      theme: {
        sidebar: ${jsString(settings.theme.sidebar)},
        header: ${jsString(settings.theme.header)},
        color: ${jsString(settings.theme.color)}
      },
      version: ${jsString(settings.version)},
      background_url: ${jsString(settings.backgroundUrl)},
      description: ${jsString(settings.description)},
      i18n: ['zh-CN', 'en-US', 'ja-JP', 'vi-VN', 'ko-KR', 'zh-TW', 'fa-IR'],
      logo: ${jsString(settings.logo)}
    };
  </script>
  <script src="/theme/default/assets/i18n/zh-CN.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/zh-TW.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/en-US.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/ja-JP.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/vi-VN.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/ko-KR.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/fa-IR.js?v=oracle"></script>
</head>
<body>
  <div id="root"></div>
  <script src="/theme/default/assets/vendors.async.js?v=oracle"></script>
  <script src="/theme/default/assets/components.async.js?v=oracle"></script>
  <script src="/theme/default/assets/umi.js?v=oracle"></script>
</body>
</html>`;
}

function legacyAdminHtml() {
  const settings = sourceSettings.admin;
  return `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="/assets/admin/components.chunk.css?v=oracle">
  <link rel="stylesheet" href="/assets/admin/umi.css?v=oracle">
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
  <title>${escapeHtml(settings.title)}</title>
  <script>window.routerBase = "/";</script>
  <script>
    window.settings = {
      title: ${jsString(settings.title)},
      theme: {
        sidebar: ${jsString(settings.theme.sidebar)},
        header: ${jsString(settings.theme.header)},
        color: ${jsString(settings.theme.color)}
      },
      version: ${jsString(settings.version)},
      background_url: ${jsString(settings.backgroundUrl)},
      logo: ${jsString(settings.logo)},
      secure_path: ${jsString(settings.securePath)}
    };
  </script>
</head>
<body>
  <div id="root"></div>
  <script src="/assets/admin/vendors.async.js?v=oracle"></script>
  <script src="/assets/admin/components.async.js?v=oracle"></script>
  <script src="/assets/admin/umi.js?v=oracle"></script>
</body>
</html>`;
}

async function readSourceSettings() {
  const [userHtml, adminHtml] = await Promise.all([
    fetchSourceHtml('/'),
    fetchSourceHtml(`/${adminPath}`),
  ]);

  return {
    admin: {
      backgroundUrl: extractStringSetting(adminHtml, 'background_url', ''),
      logo: extractStringSetting(adminHtml, 'logo', ''),
      securePath: extractStringSetting(adminHtml, 'secure_path', adminPath),
      theme: extractTheme(adminHtml),
      title: extractStringSetting(adminHtml, 'title', 'V2Board'),
      version: extractStringSetting(adminHtml, 'version', 'oracle'),
    },
    user: {
      backgroundUrl: extractStringSetting(userHtml, 'background_url', ''),
      description: extractStringSetting(userHtml, 'description', ''),
      logo: extractStringSetting(userHtml, 'logo', ''),
      theme: extractTheme(userHtml),
      title: extractStringSetting(userHtml, 'title', 'V2Board'),
      version: extractStringSetting(userHtml, 'version', 'oracle'),
    },
  };
}

async function fetchSourceHtml(path) {
  let lastError;
  for (let attempt = 1; attempt <= navigationAttempts; attempt += 1) {
    try {
      const response = await fetch(new URL(path, sourceBaseUrl));
      if (!response.ok) {
        throw new Error(`Failed to read source settings from ${path}: ${response.status}`);
      }
      return response.text();
    } catch (error) {
      lastError = error;
      if (attempt < navigationAttempts) {
        await delay(500 * attempt);
      }
    }
  }
  throw lastError;
}

function extractTheme(html) {
  return {
    color: extractStringSetting(html, 'color', 'default'),
    header: extractStringSetting(html, 'header', 'dark'),
    sidebar: extractStringSetting(html, 'sidebar', 'light'),
  };
}

function extractStringSetting(html, key, fallback) {
  const match = new RegExp(`${key}:\\s*'([^']*)'`).exec(html);
  return match?.[1] ?? fallback;
}

function jsString(value) {
  return JSON.stringify(value);
}

function escapeHtml(value) {
  return value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return '&#39;';
    }
  });
}

async function sendStaticFile(response, pathname) {
  const filePath = safeResolve(oraclePublicRoot, pathname.slice(1));
  if (!filePath) {
    response.writeHead(403);
    response.end('Forbidden');
    return;
  }

  try {
    await readFile(filePath);
  } catch {
    response.writeHead(404);
    response.end('Not found');
    return;
  }

  if (pathname === '/assets/admin/umi.js') {
    const source = await readFile(filePath, 'utf8');
    response.writeHead(200, { 'content-type': contentType(filePath) });
    response.end(patchLegacyAdminOracle(source));
    return;
  }

  response.writeHead(200, { 'content-type': contentType(filePath) });
  createReadStream(filePath).pipe(response);
}

function patchLegacyAdminOracle(source) {
  // The packaged admin dashboard can render the connected header before the
  // user model slice is present in this oracle harness. Keep the old bundle's
  // intended follow-up /user/info flow, but make that first render null-safe.
  return source
    .replaceAll(
      'var e = this.props.user.userInfo;',
      'var e = (this.props.user && this.props.user.userInfo) || {};',
    )
    .replaceAll(
      'n.map(e=>{\n                    return m.a.createElement(_["a"].Option',
      '(n || []).map(e=>{\n                    return m.a.createElement(_["a"].Option',
    )
    .replaceAll(
      'return f.map(t=>{\n                            t.id === parseInt(e)',
      'return (f || []).map(t=>{\n                            t.id === parseInt(e)',
    )
    .replaceAll(
      '}, g.map(e=>{\n                    return f.a.createElement("option", {',
      '}, (g || []).map(e=>{\n                    return f.a.createElement("option", {',
    )
    .replaceAll('filters: R.map(e=>({', 'filters: (R || []).map(e=>({')
    .replaceAll(
      'var t = R.find(t=>t.id === parseInt(e));',
      'var t = (R || []).find(t=>t.id === parseInt(e));',
    )
    .replaceAll(
      'var t = M.find(t=>t.id === e);',
      'var t = (M || []).find(t=>t.id === e);',
    );
}

function sendHtml(response, html) {
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end(html);
}

function sendJson(response, body) {
  response.writeHead(200, { 'content-type': 'application/json' });
  response.end(JSON.stringify(body));
}

function safeResolve(root, path) {
  const resolved = resolve(root, normalize(path));
  return resolved === root || resolved.startsWith(`${root}${sep}`) ? resolved : null;
}

function contentType(filePath) {
  switch (extname(filePath).toLowerCase()) {
    case '.css':
      return 'text/css; charset=utf-8';
    case '.js':
      return 'application/javascript; charset=utf-8';
    case '.json':
      return 'application/json; charset=utf-8';
    case '.png':
      return 'image/png';
    case '.svg':
      return 'image/svg+xml';
    case '.woff':
      return 'font/woff';
    case '.woff2':
      return 'font/woff2';
    case '.ttf':
      return 'font/ttf';
    case '.eot':
      return 'application/vnd.ms-fontobject';
    default:
      return 'application/octet-stream';
  }
}

function comparePngBuffers(sourceBuffer, oracleBuffer, threshold) {
  const source = decodePng(sourceBuffer);
  const oracle = decodePng(oracleBuffer);
  if (source.width !== oracle.width || source.height !== oracle.height) {
    throw new Error(
      `Screenshot size mismatch: source ${source.width}x${source.height}, ` +
        `oracle ${oracle.width}x${oracle.height}`,
    );
  }

  const diffPixels = Buffer.alloc(source.pixels.length);
  let diffPixelCount = 0;
  let totalDelta = 0;
  const totalPixels = source.width * source.height;

  for (let index = 0; index < source.pixels.length; index += 4) {
    const dr = Math.abs(source.pixels[index] - oracle.pixels[index]);
    const dg = Math.abs(source.pixels[index + 1] - oracle.pixels[index + 1]);
    const db = Math.abs(source.pixels[index + 2] - oracle.pixels[index + 2]);
    const da = Math.abs(source.pixels[index + 3] - oracle.pixels[index + 3]);
    const delta = dr + dg + db + da;
    totalDelta += delta / 4;

    if (delta > threshold) {
      diffPixelCount += 1;
      diffPixels[index] = 255;
      diffPixels[index + 1] = 0;
      diffPixels[index + 2] = 80;
      diffPixels[index + 3] = 255;
    } else {
      diffPixels[index] = Math.round(source.pixels[index] * 0.7);
      diffPixels[index + 1] = Math.round(source.pixels[index + 1] * 0.7);
      diffPixels[index + 2] = Math.round(source.pixels[index + 2] * 0.7);
      diffPixels[index + 3] = 255;
    }
  }

  return {
    averageDelta: totalDelta / totalPixels,
    diffPixelCount,
    diffPixels,
    diffRatio: diffPixelCount / totalPixels,
    height: source.height,
    totalPixels,
    width: source.width,
  };
}

function decodePng(buffer) {
  const signature = buffer.subarray(0, 8).toString('hex');
  if (signature !== '89504e470d0a1a0a') throw new Error('Invalid PNG signature');

  let offset = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  let bitDepth = 0;
  const idat = [];

  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.subarray(offset + 4, offset + 8).toString('ascii');
    const data = buffer.subarray(offset + 8, offset + 8 + length);
    offset += 12 + length;

    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      break;
    }
  }

  if (bitDepth !== 8 || ![2, 6].includes(colorType)) {
    throw new Error(`Unsupported PNG format: bitDepth=${bitDepth}, colorType=${colorType}`);
  }

  const sourceChannels = colorType === 6 ? 4 : 3;
  const stride = width * sourceChannels;
  const raw = inflateSync(Buffer.concat(idat));
  const pixels = Buffer.alloc(width * height * 4);
  let rawOffset = 0;
  let previous = Buffer.alloc(stride);

  for (let y = 0; y < height; y += 1) {
    const filter = raw[rawOffset];
    rawOffset += 1;
    const scanline = Buffer.from(raw.subarray(rawOffset, rawOffset + stride));
    rawOffset += stride;
    unfilter(scanline, previous, sourceChannels, filter);

    for (let x = 0; x < width; x += 1) {
      const sourceIndex = x * sourceChannels;
      const targetIndex = (y * width + x) * 4;
      pixels[targetIndex] = scanline[sourceIndex];
      pixels[targetIndex + 1] = scanline[sourceIndex + 1];
      pixels[targetIndex + 2] = scanline[sourceIndex + 2];
      pixels[targetIndex + 3] = sourceChannels === 4 ? scanline[sourceIndex + 3] : 255;
    }

    previous = scanline;
  }

  return { height, pixels, width };
}

function unfilter(scanline, previous, channels, filter) {
  for (let index = 0; index < scanline.length; index += 1) {
    const left = index >= channels ? scanline[index - channels] : 0;
    const up = previous[index] ?? 0;
    const upLeft = index >= channels ? previous[index - channels] : 0;

    if (filter === 1) {
      scanline[index] = (scanline[index] + left) & 0xff;
    } else if (filter === 2) {
      scanline[index] = (scanline[index] + up) & 0xff;
    } else if (filter === 3) {
      scanline[index] = (scanline[index] + Math.floor((left + up) / 2)) & 0xff;
    } else if (filter === 4) {
      scanline[index] = (scanline[index] + paeth(left, up, upLeft)) & 0xff;
    } else if (filter !== 0) {
      throw new Error(`Unsupported PNG filter: ${filter}`);
    }
  }
}

function encodePng(width, height, pixels) {
  const scanlineLength = width * 4;
  const raw = Buffer.alloc((scanlineLength + 1) * height);
  for (let y = 0; y < height; y += 1) {
    const rawOffset = y * (scanlineLength + 1);
    raw[rawOffset] = 0;
    pixels.copy(raw, rawOffset + 1, y * scanlineLength, (y + 1) * scanlineLength);
  }

  return Buffer.concat([
    Buffer.from('89504e470d0a1a0a', 'hex'),
    pngChunk('IHDR', ihdr(width, height)),
    pngChunk('IDAT', deflateSync(raw)),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

function ihdr(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  data[10] = 0;
  data[11] = 0;
  data[12] = 0;
  return data;
}

function pngChunk(type, data) {
  const typeBuffer = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = crc32Table[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function paeth(left, up, upLeft) {
  const p = left + up - upLeft;
  const pa = Math.abs(p - left);
  const pb = Math.abs(p - up);
  const pc = Math.abs(p - upLeft);
  if (pa <= pb && pa <= pc) return left;
  if (pb <= pc) return up;
  return upLeft;
}

function stripSlashes(value) {
  return value.trim().replace(/^\/+|\/+$/g, '');
}

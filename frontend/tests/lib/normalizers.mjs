// Result normalizers + the label-keyed normalizeInteractionResult dispatcher.
// Extracted verbatim from the retired frontend/scripts/visual-parity.mjs driver;
// these modules are now the source of truth.
import {
  clonePageRequests,
  jsonIncludes,
  jsonIncludesAny,
  pickFetchQueryFields,
  sortForStableJson,
} from './json-util.mjs';
import { normalizeParityText } from './text.mjs';
import { visibleTexts } from './dom-helpers.mjs';
import {
  dashboardResetPackageTradeNo,
  profileDepositTradeNo,
  subscribeTargetTitles,
} from './fixture-data.mjs';
import { authPageState } from './state-readers/auth.mjs';
import { isDarkModeActiveControlState } from './assert-useful.mjs';

// fa-IR (Persian) was consciously dropped from the source locale registry (commit 97b8035b: the
// product ships 6 LTR locales). The frozen oracle still loads fa-IR.js, so its language menu lists
// فارسی (the sole fa-IR label). Drop that one retired locale before comparing menu items, so the
// gate still asserts i18n behavior (the menu renders, switching/persisting a locale works) without
// re-pinning a locale the product no longer ships. The label is inlined rather than held in a
// module const: this helper runs during the top-level interaction pass, before a const declared
// this far down the file would leave its temporal dead zone.
export function withoutDroppedLocale(menuItems) {
  return menuItems.filter((label) => label !== 'فارسی');
}

export function normalizeRedesignedAuthLinkText(text) {
  if (
    [
      'Back to Login',
      'Login',
      '登入',
      '戻る ログイン',
      'ログイン',
      'ログインに戻る',
      '返回登入',
      '登录',
      '返回登录',
    ].includes(text)
  ) {
    return 'login-link';
  }
  if (['Register', '注册', '新規登録', '登録'].includes(text)) {
    return 'register-link';
  }
  if (['Forgot password', 'Forgot your password?', '忘记密码', '忘记密码？'].includes(text)) {
    return 'forgot-password-link';
  }
  return text;
}

export function normalizeRedesignedAuthButtonText(text) {
  if (['Login', '登入', '登录', 'ログイン'].includes(text)) {
    return 'login-button';
  }
  return text;
}

export async function normalizeRedesignedAuthPageState(page) {
  const state = await authPageState(page);
  const languageTriggerTexts = await visibleTexts(
    page,
    '[data-testid="auth-language-trigger"], .v2board-login-i18n-btn',
    2,
  );
  const comboboxTriggerTexts = await visibleTexts(page, '[role="combobox"]', 8);
  return {
    ...state,
    // Released as redesigned accessibility: auth surfaces expose language as a native auxiliary
    // button. Keep comparing the actual submit/action buttons, but ignore the language button text
    // that had no button counterpart in the packaged oracle.
    // Radix Select triggers are buttons with role="combobox"; those remain covered by controls.
    buttons: state.buttons
      .filter(
        (text) => !languageTriggerTexts.includes(text) && !comboboxTriggerTexts.includes(text),
      )
      .map(normalizeRedesignedAuthButtonText),
    controls: state.controls.map((control) => {
      const behavioral = { ...control };
      // Released as redesigned presentation: placeholders became field labels, and identifier
      // inputs may be type="email". Collapse email -> text so the behavior contract stays focused
      // on field presence/order/value while password masking remains distinct.
      delete behavioral.placeholder;
      if (behavioral.type === 'email') behavioral.type = 'text';
      return behavioral;
    }),
    links: state.links
      .filter((text) => !state.titleTexts.includes(text))
      .map(normalizeRedesignedAuthLinkText)
      .sort(),
    titleTexts: [],
  };
}

// Read the redesigned admin auth surface through owned test ids and the oracle through its
// frozen fallback selectors. The cross-world shape keeps semantic control/action counts and
// values; text, placeholders and email-vs-text presentation are intentionally not contracts.
export function normalizeAdminAuthPageState(state) {
  return {
    authSurfaceCount: state?.authSurfaceCount,
    controls: (state?.controls ?? []).map((control) => {
      const behavioral = { ...control };
      delete behavioral.placeholder;
      if (behavioral.type === 'email') behavioral.type = 'text';
      return behavioral;
    }),
    forgotActionCount: state?.forgotActionCount,
    hash: state?.hash,
    submitActionCount: state?.submitActionCount,
  };
}

export function normalizeDownloadProbe(probe) {
  const normalizeDownloadName = (value) =>
    /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.csv$/.test(value) ? '<timestamp>.csv' : value;
  return {
    downloads: (probe.downloads ?? []).map((download) => ({
      ...download,
      download: normalizeDownloadName(download.download ?? ''),
    })),
    objectUrls: probe.objectUrls ?? [],
    revokedUrls: probe.revokedUrls ?? [],
  };
}

export function normalizeAdminOrderFetchQuery(query) {
  if (!query) return null;
  // Drop antd Table's own pagination/density chrome (`total` echo, `size`) — it
  // leaks into the antd query string but is not a backend order-fetch param, and
  // the redesigned table never sends it.
  const { total: _total, size: _size, ...rest } = query;
  return rest;
}

const ADMIN_PLAN_SAVE_KEYS = [
  'id',
  'name',
  'content',
  'group_id',
  'transfer_enable',
  'device_limit',
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
  'reset_traffic_method',
  'capacity_limit',
  'speed_limit',
  // W11 (§6.2): `force_update` is intentionally NOT compared cross-world — the
  // modern create body denies it (there are no subscribers to force yet) while
  // the frozen oracle always sends it, so the create capture would diverge. The
  // edit-only flag stays pinned per-world by the raw plan-edit assertion.
];

// W11 (§6.2/§4.4): a cleared numeric plan field (prices, capacity/speed/device
// limits, reset method, sort) means the same in both dialects — the modern body
// serializes it to `null`, the frozen oracle sends the legacy present-but-empty
// `''`. Fold both spellings to `null` so the cleared semantics compare equal
// cross-world; a real value stays compared. Only the free-text `name`/`content`
// fields keep an empty string as itself.
const ADMIN_PLAN_TEXT_KEYS = new Set(['name', 'content']);

function reduceAdminPlanSaveRequest(request) {
  if (!request || typeof request !== 'object') return request;
  return Object.fromEntries(
    ADMIN_PLAN_SAVE_KEYS.filter((key) => Object.hasOwn(request, key)).map((key) => [
      key,
      !ADMIN_PLAN_TEXT_KEYS.has(key) && request[key] === '' ? null : request[key],
    ]),
  );
}

// Reduce a payment modal/Sheet snapshot to its Tier-1 compare essence. Drops the
// Tier-2 background table rows (the antd fixed-right column duplicates action
// cells as extra rows the shadcn table has no equivalent for), sorts the footer
// button order (添加/保存 lead on the shadcn Sheet, 取消 leads on the antd modal),
// and unifies the optional numeric fee fields (rendered '0' on the antd modal, ''
// on the shadcn Sheet when unset/zero — display formatting). Submitted requests
// are reduced separately to PaymentController::save's accepted fields below.
// Applied to both targets, so it never masks a real mismatch.
export function reducePaymentSnapshot(state) {
  if (!state || typeof state !== 'object' || Array.isArray(state)) return state;
  const { tableRows: _tableRows, ...rest } = state;
  const next = { ...rest };
  if (Array.isArray(next.buttons)) {
    next.buttons = [...next.buttons].sort();
  }
  if (Array.isArray(next.inputValues)) {
    next.inputValues = next.inputValues.map((value) => (value === '0' ? '' : value));
  }
  return next;
}

const paymentSaveFields = new Set([
  'id',
  'name',
  'icon',
  'payment',
  'notify_domain',
  'handling_fee_fixed',
  'handling_fee_percent',
]);

const paymentConfigKeysByDriver = new Map([
  ['AlipayF2F', ['app_id', 'private_key', 'product_name', 'public_key']],
  ['MGate', ['mgate_app_id', 'mgate_app_secret', 'mgate_source_currency', 'mgate_url']],
  [
    'StripeCheckout',
    [
      'currency',
      'stripe_custom_field_name',
      'stripe_pk_live',
      'stripe_sk_live',
      'stripe_webhook_key',
    ],
  ],
]);

export function reducePaymentSaveRequest(request, metadataOnly = false) {
  if (!request || typeof request !== 'object' || Array.isArray(request)) return request;
  const base = Object.fromEntries(
    Object.entries(request)
      .filter(([key]) => paymentSaveFields.has(key) && (!metadataOnly || key !== 'payment'))
      .map(([key, value]) => [key, value == null ? '' : value]),
  );
  if (metadataOnly) return base;
  // W11 (§6.2): `config` is the canonical nested object — both worlds fold their
  // spellings onto it (the legacy `config[key]` bracket form and the modern
  // nested JSON). Keep only the selected driver's keys so the frozen oracle's
  // stale cross-driver config leakage (e.g. MGate config[token] carried into
  // StripeCheckout) drops out; deleting that invalid stale config is not a
  // relaxation of the selected driver's contract, whose complete keys and
  // values remain compared and are also required by the raw assertion.
  const rawConfig =
    request.config && typeof request.config === 'object' && !Array.isArray(request.config)
      ? request.config
      : {};
  const selectedKeys = paymentConfigKeysByDriver.get(request.payment);
  const config = {};
  for (const [key, value] of Object.entries(rawConfig)) {
    if (!selectedKeys || selectedKeys.includes(key)) config[key] = value == null ? '' : value;
  }
  return { ...base, config };
}

export function normalizeInteractionResult(label, result) {
  const normalized = sortForStableJson(result);
  if (label === 'user-dashboard-header-language-dropdown') {
    return {
      dropdownCount: normalized.dropdownCount,
      items: normalized.items,
      placement: normalized.placement,
    };
  }
  if (label === 'user-session-expired-redirect') {
    // The frozen oracle restored an expired credential, while the redesigned
    // clients correctly destroy it. Cross-world comparison therefore keeps the
    // externally visible redirect only; source unit/integration tests separately
    // require `authorization` to remain absent after the session-expiry error
    // (a legacy 403 in the oracle, a 401 `session_expired` problem in the
    // source world — §3.2).
    return {
      hash: normalized.hash,
      loginBoxCount: normalized.loginBoxCount,
    };
  }
  if (label === 'admin-session-expired-redirect') {
    return {
      hash: normalized.hash,
      loginSurfaceCount: normalized.loginSurfaceCount,
    };
  }
  if (label === 'user-node-table-scroll' || label === 'user-traffic-table-scroll') {
    return normalizeServiceTableScrollInteractionResult(normalized);
  }
  if (
    [
      'user-invite-generate',
      'user-invite-transfer-modal',
      'user-invite-transfer-insufficient-balance',
      'user-invite-withdraw-modal',
      'user-invite-finance-submit-matrix',
    ].includes(label)
  ) {
    return normalizeInviteInteractionResult(label, normalized);
  }
  if (
    [
      'user-ticket-reply-send',
      'user-ticket-error-matrix',
      'user-ticket-create-submit',
      'user-ticket-create-validation-failure',
    ].includes(label)
  ) {
    return normalizeTicketInteractionResult(label, normalized);
  }
  if (
    label === 'user-node-tooltips' ||
    label === 'user-invite-tooltips' ||
    label === 'admin-payment-notify-tooltip' ||
    label === 'admin-order-status-tooltips'
  ) {
    return normalizeTooltipSequenceInteractionResult(normalized);
  }
  if (label === 'user-traffic-total-tooltip' || label === 'admin-plan-renew-tooltip') {
    return {
      before: normalizeTooltipInteractionState(normalized.before),
      opened: normalizeTooltipInteractionState(normalized.opened),
    };
  }
  if (label === 'user-knowledge-drawer' || label === 'user-knowledge-extreme-content-matrix') {
    return normalizeKnowledgeInteractionResult(normalized);
  }
  if (
    [
      'admin-coupons-fetch-timeout',
      'admin-giftcards-fetch-timeout',
      'admin-knowledge-fetch-timeout',
      'admin-notices-fetch-timeout',
      'admin-orders-fetch-api-500',
      'admin-orders-fetch-timeout',
      'admin-payments-fetch-timeout',
      'admin-plans-fetch-timeout',
      'admin-server-manage-fetch-timeout',
      'admin-tickets-fetch-timeout',
      'admin-users-fetch-api-500',
      'admin-users-fetch-timeout',
      'user-knowledge-fetch-timeout',
      'user-node-fetch-api-500',
      'user-node-fetch-timeout',
      'user-orders-fetch-api-500',
      'user-orders-fetch-timeout',
      'user-plans-fetch-timeout',
      'user-tickets-fetch-timeout',
      'user-traffic-fetch-timeout',
    ].includes(label)
  ) {
    return normalizeRedesignedFetchFailureInteractionResult(label, normalized);
  }
  if (label === 'user-auth-401-no-redirect' || label === 'admin-auth-401-no-redirect') {
    const {
      dashboardTexts: _dashboardTexts,
      pageContainerCount,
      routeErrorCount,
      ...authState
    } = normalized;
    return {
      ...authState,
      authenticatedSurfaceCount: pageContainerCount > 0 || routeErrorCount > 0 ? 1 : 0,
    };
  }
  if (label === 'admin-system-queue-state') {
    // Redesigned monitoring surface: the overview stat-card layout is Tier-2
    // presentation (legacy block titles vs shadcn StatCards). The Tier-1 signal
    // is the workload table (headers + rows) and the /queue route.
    const { overview: _overview, ...rest } = normalized;
    return rest;
  }
  if (
    [
      'admin-plan-create-drawer',
      'admin-plan-save-failure',
      'admin-plan-reset-method-matrix',
      'admin-plan-drawer-keyboard-close',
      'admin-plan-edit-drawer',
    ].includes(label)
  ) {
    // Every capture is an adminPlanDrawerState. Its actionButtons order, footer/
    // label chrome (antd's inline 添加权限组), and rendered table rows (操作 dropdown
    // vs inline 编辑/删除 + antd fixed-column duplicates) are Tier-2 presentation.
    // Compare only the Tier-1 essence: drawer open/close, the field values, the
    // selected option labels, the option lists, the edit-only force-update
    // checkbox and the title. The create flow deliberately drops that control:
    // the modern contract forbids it while the frozen oracle still renders it.
    // Non-drawer captures (keyboard focus, fetch deltas, save payloads) pass
    // through untouched.
    const reduced = {};
    for (const [key, value] of Object.entries(normalized)) {
      reduced[key] =
        key === 'saveRequests' && Array.isArray(value)
          ? value.map(reduceAdminPlanSaveRequest)
          : value && typeof value === 'object' && !Array.isArray(value) && 'drawerCount' in value
            ? {
                drawerCount: value.drawerCount,
                dropdownItems: value.dropdownItems,
                ...(label === 'admin-plan-create-drawer'
                  ? {}
                  : { forceUpdate: value.forceUpdate }),
                inputValues: value.inputValues,
                selectedValues: value.selectedValues,
                titles: value.titles,
              }
            : value;
    }
    if (reduced.focused && typeof reduced.focused === 'object') {
      // Escape-close focus landed on the drawer container. The Tier-1 signal is
      // that it is a focusable div; its class/id/text are Tier-2 presentation
      // (antd `.ant-drawer` chrome vs the Radix sheet container).
      reduced.focused = { tag: reduced.focused.tag };
    }
    return reduced;
  }
  if (label === 'admin-mutation-failure-matrix') {
    // The row-toggle / row-delete affordances differ by design between the antd
    // oracle (row dropdowns, `.ant-switch`) and the shadcn source (inline buttons
    // + confirm dialog, Radix switches). The Tier-1 contract is the mutation
    // request payloads (kept at top level) plus each capture's route + running
    // request counts; the captured buttons/dropdown/switch/table/toast surfaces
    // are Tier-2 presentation. Reduce every sub-state to that essence. The
    // notice show/drop captures are canonical W10 shapes whose bodies
    // intentionally differ (§6.3: the legacy toggle flips server-side on {id},
    // the modern PATCH states {show}); the shared Tier-1 essence is the
    // targeted id.
    const reduceState = (state) =>
      state ? { hash: state.hash, requestCounts: state.requestCounts } : state;
    const pickId = (requests) =>
      (requests ?? []).map((request) => ({
        id: request?.id == null ? '' : String(request.id),
      }));
    return {
      ...normalized,
      // The redesigned source's optimistic show-toggles refetch their list on
      // settlement (including after failure); the frozen oracle only refetches
      // on success. Refetch cadence is presentation-tier — the server-sort
      // delta still rides through untouched.
      fetchDeltas: { ...normalized.fetchDeltas, notice: 0, plan: 0 },
      noticeDropRequests: pickId(normalized.noticeDropRequests),
      noticeShowRequests: pickId(normalized.noticeShowRequests),
      beforeNotice: reduceState(normalized.beforeNotice),
      beforePlan: reduceState(normalized.beforePlan),
      beforeServerSort: reduceState(normalized.beforeServerSort),
      noticeDeleteFailed: reduceState(normalized.noticeDeleteFailed),
      noticeSwitchFailed: reduceState(normalized.noticeSwitchFailed),
      planDeleteDropdown: reduceState(normalized.planDeleteDropdown),
      planDeleteFailed: reduceState(normalized.planDeleteFailed),
      planSwitchFailed: reduceState(normalized.planSwitchFailed),
      serverSortFailed: reduceState(normalized.serverSortFailed),
      serverSortMode: reduceState(normalized.serverSortMode),
    };
  }
  if (
    [
      'admin-coupon-create-modal',
      'admin-coupon-generate-failure',
      'admin-coupon-range-picker',
      'admin-coupon-type-matrix',
      'admin-coupon-edit-modal',
      'admin-giftcard-create-modal',
      'admin-giftcard-generate-failure',
      'admin-giftcard-edit-modal',
      'admin-notice-create-modal',
      'admin-notice-save-failure',
      'admin-notice-edit-modal',
      'admin-knowledge-create-drawer',
      'admin-knowledge-save-failure',
      'admin-knowledge-edit-drawer',
    ].includes(label)
  ) {
    return normalizeAdminCommerceEntityInteractionResult(label, normalized);
  }
  if (label === 'user-dashboard-subscribe-drawer') {
    return normalizeDashboardSubscribeDrawerInteractionResult(normalized);
  }
  if (
    label === 'user-dashboard-subscribe-import-links' ||
    (label.startsWith('user-dashboard-subscribe-import-') && label.endsWith('-ua'))
  ) {
    return normalizeDashboardSubscribeImportLinksInteractionResult(normalized);
  }
  if (label === 'user-dashboard-reset-package-confirm') {
    return normalizeDashboardResetPackageConfirmInteractionResult(normalized);
  }
  if (label === 'user-dashboard-new-period-confirm') {
    return normalizeDashboardNewPeriodConfirmInteractionResult(normalized);
  }
  if (label === 'user-dashboard-alert-links') {
    return normalizeDashboardAlertLinksInteractionResult(normalized);
  }
  if (label === 'user-plan-checkout-coupon') {
    return normalizePlanCheckoutCouponInteractionResult(normalized);
  }
  if (label === 'user-plan-checkout-coupon-error') {
    return normalizePlanCheckoutCouponErrorInteractionResult(normalized);
  }
  if (label === 'user-order-cancel-confirm') {
    return normalizeOrderCancelConfirmInteractionResult(normalized);
  }
  if (label === 'user-order-qr-checkout') {
    return normalizeOrderQrCheckoutInteractionResult(normalized);
  }
  if (
    label === 'user-order-qr-checkout-failure' ||
    label === 'user-order-checkout-network-failure'
  ) {
    return normalizeOrderCheckoutNetworkFailureInteractionResult(normalized);
  }
  if (
    label === 'user-order-stripe-disabled-checkout' ||
    label === 'user-order-stripe-payment-intent-checkout' ||
    label === 'user-order-stripe-confirmation-failure'
  ) {
    return normalizeOrderStripeInteractionResult(label, normalized);
  }
  if (label === 'user-order-redirect-checkout') {
    // §10.4/§W1: the backend mints a path-style relative return URL, so the
    // modern source follows it with a full document navigation (fresh order
    // page mount) while the frozen oracle hash-navigates in place and keeps
    // its widget state alive. Post-redirect presentation (selected-method
    // highlight, fee summary line, transit toast) is Tier-2; the Tier-1
    // contract is the checkout payload, the pre-checkout method selection,
    // and the redirect landing on the order route with the cashier marker.
    return {
      checkoutRequests: normalized.checkoutRequests,
      redirected: {
        hash: normalized.redirected?.hash,
        modalCount: normalized.redirected?.modalCount,
        qrCanvasCount: normalized.redirected?.qrCanvasCount,
        qrSvgCount: normalized.redirected?.qrSvgCount,
      },
      selected: {
        activeIndex: normalized.selected?.activeIndex,
        hash: normalized.selected?.hash,
        methodTexts: normalized.selected?.methodTexts,
      },
    };
  }
  if (label === 'user-profile-change-password-success') {
    return normalizeProfileChangePasswordInteractionResult(normalized);
  }
  if (label === 'user-profile-deposit-modal') {
    return normalizeProfileDepositModalInteractionResult(normalized);
  }
  if (label === 'user-profile-reset-subscribe-confirm') {
    // Cache refresh timing is presentation-tier on this redesigned surface. The
    // source now invalidates both credential projections immediately; the frozen
    // oracle leaves them stale. Keep comparing the reset request/dialog outcome.
    return { ...normalized, infoFetchDelta: 0, subscribeFetchDelta: 0 };
  }
  if (label === 'user-profile-telegram-unbind-confirm') {
    return { ...normalized, subscribeFetchDelta: 0 };
  }
  if (label === 'user-profile-preference-switches') {
    // The redesigned profile flips a preference switch optimistically (with
    // snapshot rollback on failure) while the frozen oracle holds the old
    // value until the server answers. The pending-window visual value is
    // presentation-tier; keep comparing that a disabled loading switch exists
    // plus the update payloads and the settled states.
    return {
      ...normalized,
      toggles: (normalized.toggles ?? []).map((toggle) => ({
        ...toggle,
        loadingSwitch: toggle.loadingSwitch
          ? { ...toggle.loadingSwitch, ariaChecked: null, checked: null }
          : toggle.loadingSwitch,
      })),
    };
  }
  if (
    label === 'user-profile-redeem-giftcard-api-500' ||
    label === 'user-profile-redeem-giftcard-timeout'
  ) {
    return normalizeProfileRedeemFailureInteractionResult(normalized);
  }
  if (
    label === 'user-dashboard-dark-mode-persistence' ||
    label === 'admin-dashboard-dark-mode-persistence'
  ) {
    return {
      afterEnable: normalizeUserDarkModePersistenceState(normalized.afterEnable),
      afterReload: normalizeUserDarkModePersistenceState(normalized.afterReload),
      before: normalizeUserDarkModePersistenceState(normalized.before),
    };
  }
  if (
    label === 'admin-plan-create-group-select-dropdown' ||
    label === 'admin-users-filter-field-select-dropdown' ||
    label === 'admin-user-create-plan-select-dropdown'
  ) {
    return normalizeSelectDropdownInteractionResult(label, normalized);
  }
  if (
    label === 'admin-user-bulk-ban-confirm' ||
    label === 'admin-user-bulk-delete-confirm' ||
    label === 'admin-user-reset-secret-confirm' ||
    label === 'admin-user-delete-confirm'
  ) {
    return normalizeAdminUserConfirmInteractionResult(normalized);
  }
  if (
    [
      'admin-user-copy-action',
      'admin-user-export-download-matrix',
      'admin-user-destructive-failure-matrix',
      'admin-user-create-modal',
      'admin-user-send-mail-modal',
      'admin-user-send-mail-submit-matrix',
      'admin-user-edit-action',
      'admin-user-update-validation-failure',
      'admin-user-assign-action',
      'admin-user-orders-action',
      'admin-user-invite-action',
      'admin-user-traffic-action',
    ].includes(label)
  ) {
    return normalizeAdminUserActionInteractionResult(label, normalized);
  }
  if (label === 'admin-users-filter-expiry-picker' || label === 'admin-user-create-expiry-picker') {
    // Both worlds reduce to whether a date field became reachable; the calendar
    // popup chrome (antd) vs native date input (redesign) is Tier-2 presentation.
    const reduce = (state) => ({ reachable: (state?.dateFieldCount ?? 0) >= 1 });
    return { before: reduce(normalized.before), opened: reduce(normalized.opened) };
  }
  if (label === 'admin-users-extreme-viewport-matrix') {
    // Keep only the structural essence shared by both worlds: the viewport width,
    // that a scrollable table body is present, and that the filter drawer opened.
    // Antd fixed-column chrome, horizontal-overflow observability, the shadcn-table
    // marker, toolbar/header/drawer button text, and body class are Tier-2
    // presentation pinned per-world by the raw assertion.
    const reduceLayout = (state) =>
      state
        ? {
            layout: {
              drawerOpen: state.layout?.drawerOpen,
              tableBodyPresent: state.layout?.tableBodyPresent,
              viewportWidth: state.layout?.viewportWidth,
            },
          }
        : state;
    return {
      before: reduceLayout(normalized.before),
      filterDrawer: reduceLayout(normalized.filterDrawer),
      narrowed: reduceLayout(normalized.narrowed),
    };
  }
  if (label === 'admin-payment-modal-keyboard-close') {
    // Reduce the modal snapshots via reducePaymentSnapshot (Tier-2 table rows,
    // button order, fee display), and reduce the focused element to its tag. The
    // Tier-1 contract is that Escape closes a modal whose container took focus (a
    // div) — the focused className (shadcn sheet classes vs `ant-modal`), radix id,
    // and full text (button order + the shadcn "Close dialog" a11y label) are
    // Tier-2 presentation. focused.tag === 'div' is still enforced per-target by
    // the raw assertion.
    return {
      before: reducePaymentSnapshot(normalized.before),
      closed: reducePaymentSnapshot(normalized.closed),
      opened: reducePaymentSnapshot(normalized.opened),
      focused: normalized.focused ? { tag: normalized.focused.tag } : normalized.focused,
    };
  }
  if (
    label === 'admin-payment-create-modal' ||
    label === 'admin-payment-edit-modal' ||
    label === 'admin-payment-plugin-field-matrix' ||
    label === 'admin-payment-save-failure'
  ) {
    // Reduce modal snapshots to their Tier-1 essence. Save requests keep every
    // base field plus every config key belonging to the selected payment driver.
    // The frozen oracle leaks hidden fields while switching drivers (for example,
    // MGate config[token] into StripeCheckout); deleting that invalid stale config
    // is not a relaxation of the selected driver's contract, whose complete keys
    // and values remain compared and are also required by the raw assertion.
    // Fetched model metadata that Laravel validation ignores is dropped as before.
    // All presentation signals remain verified per-target by the raw assertion.
    const reduced = {};
    const metadataOnly = label === 'admin-payment-edit-modal';
    for (const [key, value] of Object.entries(normalized)) {
      if (key === 'saveRequests' && Array.isArray(value)) {
        reduced[key] = value.map((request) => reducePaymentSaveRequest(request, metadataOnly));
        continue;
      }
      const snapshot = reducePaymentSnapshot(value);
      if (metadataOnly && snapshot && typeof snapshot === 'object' && !Array.isArray(snapshot)) {
        const { selectedPayment: _selectedPayment, ...metadataSnapshot } = snapshot;
        reduced[key] = {
          ...metadataSnapshot,
          ...(Array.isArray(metadataSnapshot.inputValues)
            ? { inputValues: metadataSnapshot.inputValues.slice(0, 5) }
            : {}),
          ...(Array.isArray(metadataSnapshot.labels)
            ? { labels: metadataSnapshot.labels.slice(0, 5) }
            : {}),
        };
      } else {
        reduced[key] = snapshot;
      }
    }
    return reduced;
  }
  if (label === 'admin-config-tabs') {
    // Compare only which tab is active by its text; the active-tab className
    // (antd `ant-tabs-tab-active` vs the redesigned nav button classes) is
    // Tier-2 presentation. The active-tab-changed contract is enforced by the
    // raw assertion.
    const reduceTab = (state) => (state ? { text: state.text } : state);
    return {
      before: reduceTab(normalized.before),
      second: reduceTab(normalized.second),
      third: reduceTab(normalized.third),
    };
  }
  if (label === 'admin-order-detail-modal') {
    // Compare only the overlay open/close transition and title. The detail rows
    // (antd `.ant-row` label+value cells vs the redesigned Sheet DetailRow divs,
    // with their own date/amount formatting and field ordering) are Tier-2
    // presentation; the critical rows (email, trade_no, period, amount) stay
    // pinned on the raw result by the assertion.
    const reduceSnapshot = (state) =>
      state ? { modalCount: state.modalCount, titles: state.titles } : state;
    return {
      ...normalized,
      opened: reduceSnapshot(normalized.opened),
      closed: reduceSnapshot(normalized.closed),
    };
  }
  if (label === 'admin-order-assign-modal') {
    // Keep the overlay open/close transition, title, filled field values, and
    // selected option text. The form labels and footer button chrome are Tier-2
    // presentation (pinned by the raw assertion's label/title includes), and the
    // assign payload stays pinned by assignRequest below.
    const reduceSnapshot = (state) =>
      state
        ? {
            inputValues: state.inputValues,
            modalCount: state.modalCount,
            selectedValues: state.selectedValues,
            titles: state.titles,
          }
        : state;
    return {
      ...normalized,
      opened: reduceSnapshot(normalized.opened),
      filled: reduceSnapshot(normalized.filled),
      closed: reduceSnapshot(normalized.closed),
    };
  }
  if (label === 'admin-order-status-dropdown' || label === 'admin-order-commission-dropdown') {
    // Compare only the menu open/close transition per snapshot. The dropdown
    // item labels (redesigned `标记为已支付`/`取消订单` vs antd `已支付`/`取消`),
    // the truncated-vs-full trade_no table rows, and the antd `标记为` trigger
    // links are Tier-2 presentation pinned by the raw assertion; the mark-paid /
    // commission-update payloads stay pinned by paidRequest / updateRequest.
    const reduceSnapshot = (state) => (state ? { dropdownCount: state.dropdownCount } : state);
    return {
      ...normalized,
      before: reduceSnapshot(normalized.before),
      opened: reduceSnapshot(normalized.opened),
      closed: reduceSnapshot(normalized.closed),
    };
  }
  if (label === 'admin-orders-filter-pagination-matrix') {
    // Keep the filter/pagination CONTRACT — the applied fetch filterQuery, active
    // page, page-item set, drawer/sorter counts — and drop the Tier-2 table
    // surface (row texts with full-vs-truncated trade_no, header labels, and the
    // antd `过滤器` toolbar the redesigned inline search replaces). The pre-
    // interaction `before.filterQuery` is dropped too: it is the initial page
    // load, where antd seeds current=0 and the redesign uses current=1 — an
    // index convention no external party consumes and the assertion never pins.
    const reduceSnapshot = (state, keepQuery) =>
      state
        ? {
            activePage: state.activePage,
            drawerCount: state.drawerCount,
            ...(keepQuery ? { filterQuery: state.filterQuery } : {}),
            pageItems: state.pageItems,
            sorterCount: state.sorterCount,
          }
        : state;
    return {
      ...normalized,
      before: reduceSnapshot(normalized.before, false),
      filtered: reduceSnapshot(normalized.filtered, true),
      page2: reduceSnapshot(normalized.page2, true),
    };
  }
  if (label === 'admin-ticket-reply-send') {
    // Compare only the reply input value per snapshot (staged 'Parity admin
    // reply send', cleared to '' after send). The send-button state (the
    // redesigned icon Button vs the legacy affordance the oracle reader doesn't
    // resolve), the rendered chat messageTexts, and the transient '发送中'
    // loading toast (dismissed at different times once the reply resolves) are
    // Tier-2 presentation; the '发送中' loading state, reply payload, and fetch
    // delta stay pinned by the raw assertion and the untouched replyRequests/
    // ticketFetchDelta below.
    const reduceSnapshot = (state) => (state ? { inputValue: state.inputValue } : state);
    return {
      ...normalized,
      filled: reduceSnapshot(normalized.filled),
      loading: reduceSnapshot(normalized.loading),
      sent: reduceSnapshot(normalized.sent),
    };
  }
  if (label === 'admin-tickets-reply-filter') {
    // Both worlds' fetches canonicalize onto one flat §6.5 shape (page/
    // per_page/status plus the toggled reply_status array — W14). The legacy
    // antd Table additionally leaks its own pagination chrome — total (the
    // echoed row count) and size (table density) — into the query string;
    // those are antd-internal presentation, not backend filter params, so
    // strip them from both sides before comparing. The reply_status
    // passthrough stays pinned by the raw assertion.
    const stripAntdTableParams = (request) =>
      request && typeof request === 'object' && !Array.isArray(request)
        ? Object.fromEntries(
            Object.entries(request).filter(([key]) => key !== 'total' && key !== 'size'),
          )
        : request;
    return {
      ...normalized,
      filterFetchRequests: Array.isArray(normalized.filterFetchRequests)
        ? normalized.filterFetchRequests.map(stripAntdTableParams)
        : normalized.filterFetchRequests,
    };
  }
  if (label === 'admin-server-create-node-drawer') {
    // The node drawer's field set diverges by design between the antd oracle
    // (multi-select 权限组/路由组, tag select) and the shadcn source (checkbox
    // groups, TagsInput), so labels/inputValues/selectedValues/dropdownItems are
    // Tier-2 presentation validated per-target by the raw assertion. The compare
    // keeps only the structural essence: menu opened, drawer opened+titled, group
    // selected, drawer closed.
    return {
      before: { drawerCount: normalized.before?.drawerCount },
      closed: normalized.closed,
      drawerOpened: {
        drawerCount: normalized.drawerOpened?.drawerCount,
        titles: normalized.drawerOpened?.titles,
      },
      groupDefaultSelected: normalized.groupDefaultSelected,
      groupSelected: { drawerCount: normalized.groupSelected?.drawerCount },
      menuOpened: { dropdownCount: normalized.menuOpened?.dropdownCount },
    };
  }
  if (label === 'admin-server-edit-node-drawer') {
    // Same divergence as create-node: the drawer's field set (checkbox groups vs
    // multi-selects, TagsInput vs tag-select) is Tier-2 presentation validated
    // per-target by the raw assertion (node identity, group pre-selection). The
    // compare keeps the structural essence only.
    return {
      before: { drawerCount: normalized.before?.drawerCount },
      closed: normalized.closed,
      edited: { drawerCount: normalized.edited?.drawerCount },
      opened: {
        drawerCount: normalized.opened?.drawerCount,
        titles: normalized.opened?.titles,
      },
      openedGroupSelected: normalized.openedGroupSelected,
    };
  }
  if (label === 'admin-server-node-save-failure') {
    // Node drawer field-set divergence is Tier-2 (raw assertion validates the
    // failed inputValues + save payload per-target). Keep the structural essence
    // and the Tier-1 save payload / no-refetch delta.
    return {
      after: { drawerCount: normalized.after?.drawerCount },
      before: { drawerCount: normalized.before?.drawerCount },
      filled: { drawerCount: normalized.filled?.drawerCount },
      menuOpened: { dropdownCount: normalized.menuOpened?.dropdownCount },
      nodeFetchDelta: normalized.nodeFetchDelta,
      saveRequests: normalized.saveRequests,
    };
  }
  if (
    [
      'admin-server-route-create-modal',
      'admin-server-route-edit-modal',
      'admin-server-group-create-modal',
      'admin-server-group-edit-modal',
      'admin-server-group-save-failure',
    ].includes(label)
  ) {
    // Modal chrome (labels, button order, pageButtons, table action columns,
    // dropdown options, prefilled inputValues) is Tier-2 presentation validated
    // per-target by the raw assertion. Reduce every modal capture to its
    // structural essence (modalCount + titles); the Tier-1 save payload and
    // refetch delta pass through untouched at top level.
    const reduceModalState = (state) => {
      if (!state || typeof state !== 'object' || !('modalCount' in state)) return state;
      return { modalCount: state.modalCount, titles: state.titles };
    };
    return Object.fromEntries(
      Object.entries(normalized).map(([key, value]) => {
        if (key === 'routeFetchDelta' && label.startsWith('admin-server-route-')) {
          // A successful invalidation/refetch is contractual; its exact request
          // count is target-specific cache timing and therefore Tier 2.
          return [key, Number(value) >= 1];
        }
        if (key === 'saveRequests' && label.startsWith('admin-server-route-')) {
          return [
            key,
            (value ?? []).map((request) => {
              if (!request || typeof request !== 'object') return request;
              return Object.fromEntries(
                Object.entries(request).filter(
                  ([field]) => field !== 'created_at' && field !== 'updated_at',
                ),
              );
            }),
          ];
        }
        if (label === 'admin-server-group-edit-modal' && key === 'saveRequests') {
          return [
            key,
            (value ?? []).map((request) => {
              if (!request || typeof request !== 'object') return request;
              return Object.fromEntries(
                ['id', 'name']
                  .filter((field) => Object.hasOwn(request, field))
                  .map((field) => [field, request[field]]),
              );
            }),
          ];
        }
        return [key, reduceModalState(value)];
      }),
    );
  }
  if (label === 'admin-users-filter-input') {
    // Only the typed filter value is contract; the redesigned Sheet's footer
    // buttons (重置/确定) differ from the antd drawer's (添加条件/检索) — Tier-2.
    return { firstInput: normalized.firstInput };
  }
  if (label === 'admin-users-filter-expiry-picker') {
    // The 到期时间 field opens an antd calendar popup on the oracle but a native
    // datetime-local input on the redesigned Sheet. Both are Tier-2 presentation;
    // reduce to whether a date filter became reachable (pinned per world by the
    // raw assertion).
    const reachable = (state) =>
      state
        ? { dateFilterReachable: (state.popupCount ?? 0) >= 1 || (state.dateFieldCount ?? 0) >= 1 }
        : state;
    return { before: reachable(normalized.before), opened: reachable(normalized.opened) };
  }
  if (label === 'admin-users-pagination-matrix') {
    // Keep the pagination CONTRACT — the applied fetch query (§8 page/per_page),
    // active page, and page-item set — and drop Tier-2 presentation: the antd
    // `.ant-pagination-next` classes, the page-size selection label formatting,
    // the full row texts (number/date formatting), and the size dropdown's option
    // list (the 50 条/页 presence stays pinned by the raw assertion).
    const reduceSnapshot = (state) =>
      state && typeof state === 'object' && 'activePage' in state
        ? {
            activePage: state.activePage,
            pageItems: state.pageItems,
            query: pickFetchQueryFields(state.query, ['page', 'per_page']),
          }
        : state;
    // Drop the size-changer path (sizeChangerCount, pageSize50, sizeDropdown) from
    // the cross-world compare: the antd ProTable hides the page-size changer on the
    // mobile viewport while the redesigned shadcn table keeps it visible (Tier-2
    // presentation), so the two worlds take different run branches there and their
    // shapes cannot match. The pageSize=50 CONTRACT stays fully pinned per-world by
    // the raw assertion, which asserts pageSize=50 when the changer is visible and
    // the skip markers when it is not. The cross-world compare keeps only the
    // shared page-navigation contract (active page + page-item set + current query).
    return {
      before: reduceSnapshot(normalized.before),
      page2: reduceSnapshot(normalized.page2),
    };
  }
  if (label === 'admin-users-sort-matrix') {
    // The sort CONTRACT is the fetch query (§7.2 sort_by=banned, sort_dir
    // asc→desc); the antd `ant-table-column-sorter-up/down` arrow classes, header
    // label set, and row texts are Tier-2 presentation the redesigned lucide sort
    // icons express differently. Compare only the applied asc/desc queries.
    const reduceSort = (state) =>
      state
        ? { query: pickFetchQueryFields(state.query, ['page', 'per_page', 'sort_by', 'sort_dir']) }
        : state;
    return { asc: reduceSort(normalized.asc), desc: reduceSort(normalized.desc) };
  }
  if (
    label === 'admin-server-protocol-field-matrix' ||
    label === 'admin-server-v2node-protocol-matrix' ||
    label === 'admin-server-vless-reality-matrix' ||
    label === 'admin-server-v2node-security-transport-matrix'
  ) {
    // Every snapshot is a node-drawer state whose Tier-2 chrome (node-list
    // tableRows, protocol dropdownCount/items, selectDropdownItems) renders
    // differently across the shadcn island and the antd drawer. Reduce to the
    // contract essence (field labels, chosen selectedValues, typed inputValues)
    // sorted; the vless matrix's Tier-1 saveRequests payload passes through the
    // array branch intact.
    return normalizeAdminServerProtocolMatrixResult(normalized);
  }
  if (label === 'admin-dashboard-avatar-dropdown') {
    // Portaled Radix menu vs Bootstrap dropdown: keep only that the menu toggles open and
    // exposes a logout action; drop menuClass and trigger-relative geometry (Tier-2).
    const reduceAvatar = (state) => ({
      hasLogout: jsonIncludesAny(state?.items, ['Logout', '登出']),
      menuCount: state?.menuCount,
    });
    return { before: reduceAvatar(normalized.before), opened: reduceAvatar(normalized.opened) };
  }
  if (label === 'admin-dashboard-commission-shortcut') {
    // Alert/shortcut copy is presentation. The contract is the commission
    // order-filter sessionStorage + fetch query and the /order hash.
    const reduceShortcutState = (state) => {
      if (!state) return state;
      const { alertLinks: _alertLinks, ...rest } = state;
      return rest;
    };
    return {
      after: reduceShortcutState(normalized.after),
      before: reduceShortcutState(normalized.before),
    };
  }
  if (label === 'admin-root-page-state') {
    const { hash: _hash, ...authState } = normalizeAdminAuthPageState(normalized);
    return {
      ...authState,
      loginDestination: ['/', '/login'].includes(normalized.hash),
    };
  }
  if (label === 'admin-login-form-state') {
    const forgot = normalized.forgotModal ?? {};
    return {
      // Tier-1: identifier+password fields retained their typed values, login+forgot actions
      // are present, and the native reset command is exposed. Dialog copy/chrome is Tier-2.
      filled: normalizeAdminAuthPageState(normalized.filled ?? {}),
      forgotModal: {
        hasResetCommand: jsonIncludesAny(forgot, [
          'v2board-api reset-admin-password',
          'reset:password',
        ]),
        modalCount: forgot.modalCount,
      },
    };
  }
  return normalized;
}

export function normalizeAdminUserConfirmInteractionResult(result) {
  // Reduce every confirm snapshot to its Tier-1 essence: overlay/drawer counts and
  // the applied filter query. Title/content/button text are dropped from the
  // compare — the readers capture different element sets across the two DOMs (the
  // antd `.ant-modal-body` folds the title into the body text; the Radix
  // AlertDialog exposes one description node), so those arrays diverge as pure
  // presentation. They stay pinned per-world by the raw assertion, along with the
  // rest of the Tier-2 surface (toolbarButtons 创建用户 vs 创建, tableRows,
  // dropdownItems, triggerTexts, filter inputValues). Non-snapshot values pass
  // through untouched.
  const reduceState = (state) => {
    if (!state || typeof state !== 'object' || Array.isArray(state)) return state;
    const reduced = {};
    if ('modalCount' in state) reduced.modalCount = state.modalCount;
    if ('drawerCount' in state) reduced.drawerCount = state.drawerCount;
    if ('filterQuery' in state) reduced.filterQuery = state.filterQuery;
    return reduced;
  };

  return Object.fromEntries(
    Object.entries(result ?? {}).map(([key, value]) => [key, reduceState(value)]),
  );
}

// Reduce a user-action snapshot to the structural + normalized-query essence the
// two worlds share. Presentation arrays the readers also capture (labels,
// buttons, dropdownItems, tableRows, triggerTexts, toolbarButtons, inputValues,
// selectedValues, toastTexts, hash) are Tier-2 and dropped from the compare —
// each is pinned per-world by the raw assertion.
export function reduceAdminUserActionSnapshot(state) {
  if (!state || typeof state !== 'object' || Array.isArray(state)) return state;
  const reduced = {};
  for (const key of ['modalCount', 'drawerCount', 'dropdownCount', 'requestCount']) {
    if (key in state) reduced[key] = state[key];
  }
  for (const key of ['filterQuery', 'orderFetchQuery', 'userFetchQuery', 'trafficQuery', 'probe']) {
    if (key in state) {
      // W14 (§6.8): the traffic modal fetch capture is canonical in both
      // worlds (`/stat/getStatUser` page/pageSize and the modern
      // `stats/user-traffic` page/per_page fold onto one shape); reduce to
      // the shared contract fields (user_id, page, per_page).
      reduced[key] =
        key === 'trafficQuery'
          ? pickFetchQueryFields(state[key], ['user_id', 'page', 'per_page'])
          : state[key];
    }
  }
  return reduced;
}

// Reduce the batch of user row/toolbar action scenarios. Snapshots collapse to
// their structural essence; the Tier-1 request payloads are picked to their
// contract fields and string-coerced (the antd oracle sends form-encoded string
// values, the redesigned api-client sends typed JSON, so raw types diverge). The
// filter-carrying destructive/export requests reduce to a world-agnostic marker
// (their exact form-vs-JSON filter shape is pinned per-world by the raw check).
export function normalizeAdminUserActionInteractionResult(label, result) {
  const pickStr = (obj, keys) =>
    Object.fromEntries(keys.map((key) => [key, obj?.[key] == null ? '' : String(obj[key])]));
  const carriesFilter = (requests) =>
    (requests ?? []).map((request) => ({ hasFilter: jsonIncludes(request, 'visual@example.com') }));
  const reduceSnapshots = (extra = {}) =>
    Object.fromEntries(
      Object.entries(result ?? {}).map(([key, value]) => [
        key,
        key in extra ? extra[key] : reduceAdminUserActionSnapshot(value),
      ]),
    );

  if (label === 'admin-user-copy-action') {
    const copied = result.copied ?? {};
    return {
      before: {},
      copied: {
        copied:
          (copied.clipboardWrites ?? []).some((text) =>
            jsonIncludes(text, 'subscribe?token=visual-user'),
          ) || jsonIncludes(copied.messageTexts, '复制成功'),
        modalCount: copied.modalCount,
      },
      dropdown: {},
    };
  }
  if (label === 'admin-user-export-download-matrix') {
    return reduceSnapshots({ dumpCsvRequests: carriesFilter(result.dumpCsvRequests) });
  }
  if (label === 'admin-user-destructive-failure-matrix') {
    return reduceSnapshots({
      allDeleteRequests: carriesFilter(result.allDeleteRequests),
      banRequests: carriesFilter(result.banRequests),
      deleteRequests: (result.deleteRequests ?? []).map((request) => pickStr(request, ['id'])),
    });
  }
  if (label === 'admin-user-create-modal') {
    return reduceSnapshots({
      generateRequests: (result.generateRequests ?? []).map((request) =>
        pickStr(request, ['email_prefix', 'email_suffix', 'password', 'plan_id']),
      ),
    });
  }
  if (label === 'admin-user-send-mail-submit-matrix') {
    return reduceSnapshots({
      sendMailRequests: (result.sendMailRequests ?? []).map((request) =>
        pickStr(request, ['subject', 'content']),
      ),
    });
  }
  if (label === 'admin-user-update-validation-failure') {
    // The modern form blocks the invalid payload client-side while the frozen
    // oracle exercises the equivalent backend rejection. The stable outcome is
    // the preserved editor with no list refetch, not where validation ran.
    return reduceSnapshots({ updateRequests: [] });
  }
  if (label === 'admin-user-assign-action') {
    return reduceSnapshots({
      assignRequest: result.assignRequest
        ? pickStr(result.assignRequest, ['email', 'plan_id', 'period', 'total_amount'])
        : result.assignRequest,
    });
  }
  // send-mail-modal, edit-action, orders-action, invite-action, traffic-action:
  // structural reduce only (no Tier-1 payload; the fetch query is kept by the
  // snapshot reducer).
  return reduceSnapshots();
}

// Reduce the coupon / giftcard / notice / knowledge editor scenarios. Every
// captured editor snapshot collapses to its structural overlay open/close count;
// the Tier-1 signal is the generate/save request payload (captured in the
// canonical W10 dialect shape — legacy bracket arrays fold to real arrays and
// the body id folds onto the path identity — then picked to its contract
// fields and string-coerced so arrays and numbers compare deterministically)
// plus that a refetch fired. The rich field text (labels, titles, input values,
// selected values, table rows, dropdown items, addon/preview chrome, tag chips)
// is Tier-2 presentation the redesign renders differently and is pinned per-world
// by the raw assertion. The coupon range picker reduces to date-field
// reachability, matching the users expiry picker.
export function normalizeAdminCommerceEntityInteractionResult(label, result) {
  const pickStr = (obj, keys) =>
    Object.fromEntries(keys.map((key) => [key, obj?.[key] == null ? '' : String(obj[key])]));
  const reduceOverlaySnapshot = (state) => {
    if (!state || typeof state !== 'object' || Array.isArray(state)) return state;
    const reduced = {};
    for (const key of ['modalCount', 'drawerCount']) {
      if (key in state) reduced[key] = state[key];
    }
    return reduced;
  };

  if (label === 'admin-coupon-range-picker') {
    const reduce = (state) => ({ reachable: (state?.dateFieldCount ?? 0) >= 1 });
    return { before: reduce(result.before), opened: reduce(result.opened) };
  }

  const couponKeys = ['id', 'name', 'code', 'type', 'value', 'limit_plan_ids', 'limit_period'];
  const giftcardKeys = ['id', 'name', 'code', 'type', 'value', 'plan_id', 'limit_use'];
  const noticeKeys = ['id', 'title', 'content', 'tags', 'img_url'];
  const knowledgeKeys = ['id', 'title', 'category', 'language', 'body'];
  const requestSpecByLabel = {
    'admin-coupon-create-modal': { field: 'generateRequests', keys: couponKeys },
    'admin-coupon-generate-failure': { field: 'generateRequests', keys: couponKeys },
    'admin-coupon-type-matrix': { field: 'generateRequests', keys: couponKeys },
    'admin-coupon-edit-modal': { field: 'generateRequests', keys: couponKeys },
    'admin-giftcard-create-modal': { field: 'generateRequests', keys: giftcardKeys },
    'admin-giftcard-generate-failure': { field: 'generateRequests', keys: giftcardKeys },
    'admin-giftcard-edit-modal': { field: 'generateRequests', keys: giftcardKeys },
    'admin-notice-create-modal': { field: 'saveRequests', keys: noticeKeys },
    'admin-notice-save-failure': { field: 'saveRequests', keys: noticeKeys },
    'admin-notice-edit-modal': { field: 'saveRequests', keys: noticeKeys },
    'admin-knowledge-create-drawer': { field: 'saveRequests', keys: knowledgeKeys },
    'admin-knowledge-save-failure': { field: 'saveRequests', keys: knowledgeKeys },
    'admin-knowledge-edit-drawer': { field: 'saveRequests', keys: knowledgeKeys },
  };
  const spec = requestSpecByLabel[label];
  const reduced = {};
  for (const [key, value] of Object.entries(result ?? {})) {
    if (spec && key === spec.field) {
      reduced[key] = (value ?? []).map((request) => pickStr(request, spec.keys));
    } else if (key.endsWith('FetchDelta')) {
      reduced[key] = (value ?? 0) >= 1 ? 1 : 0;
    } else {
      reduced[key] = reduceOverlaySnapshot(value);
    }
  }
  return reduced;
}

export function normalizePlanCheckoutCouponInteractionResult(result) {
  return {
    couponInput: result.couponInput,
    summaryBlocks: result.summaryBlocks,
    submitButton: result.submitButton,
  };
}

export function normalizePlanCheckoutCouponErrorInteractionResult(result) {
  return {
    after: {
      couponInput: result.after?.couponInput,
      summaryBlocks: result.after?.summaryBlocks,
      submitButton: result.after?.submitButton,
    },
    before: {
      summaryBlocks: result.before?.summaryBlocks,
    },
    couponRequests: clonePageRequests(result.couponRequests),
  };
}

export function normalizeOrderCancelConfirmInteractionResult(result) {
  if (!result?.opened || typeof result.opened !== 'object' || Array.isArray(result.opened)) {
    return result;
  }
  // Confirmation copy is Tier-2 on the redesigned commerce surface. The oracle's broad
  // `.ant-modal-body` selector captures both the complete modal text and the semantic body,
  // while the shadcn selector captures the body alone. `assertUsefulInteraction` already checks
  // each raw world for the cancel-order warning, so drop this presentation-only duplicate before
  // the Tier-1 cross-world comparison without weakening payload/refetch/dialog-outcome coverage.
  const { content: _content, ...opened } = result.opened;
  return { ...result, opened };
}

export function normalizeOrderQrCheckoutInteractionResult(result) {
  return {
    before: {
      activeIndex: result.before?.activeIndex,
      methodTexts: result.before?.methodTexts,
    },
    checkoutRequests: clonePageRequests(result.checkoutRequests),
    loading: {
      submitButton: result.loading?.submitButton,
    },
    opened: {
      hasPaymentDialog:
        (result.opened?.modalCount ?? 0) > 0 &&
        jsonIncludesAny(result.opened?.modalTexts, ['等待支付中', 'Waiting for payment']),
      modalTexts: normalizeOrderCheckoutModalTexts(result.opened?.modalTexts),
    },
  };
}

export function normalizeOrderCheckoutModalTexts(values = []) {
  if (jsonIncludesAny(values, ['等待支付中', 'Waiting for payment'])) {
    return ['waiting-for-payment'];
  }
  return values;
}

export function normalizeOrderCheckoutNetworkFailureInteractionResult(result) {
  return {
    after: {
      hash: result.after?.hash,
      modalCount: result.after?.modalCount,
      qrCanvasCount: result.after?.qrCanvasCount,
      qrSvgCount: result.after?.qrSvgCount,
    },
    before: {
      activeIndex: result.before?.activeIndex,
      methodTexts: result.before?.methodTexts,
    },
    checkoutRequests: clonePageRequests(result.checkoutRequests),
  };
}

export function normalizeOrderStripeInteractionResult(label, result) {
  const selected = result.selected ?? {};
  const terminal = result.checkedOut ?? result.after ?? {};
  const intentRequest = result.stripeIntentRequests?.[0];
  const legacyRequest = result.checkoutRequests?.[0];
  const request = intentRequest ?? legacyRequest;
  const base = {
    before: { activeIndex: result.before?.activeIndex },
    selected: {
      activeIndex: selected.activeIndex,
      prepared: (selected.stripeIntentCount ?? 0) + (selected.stripePublicKeyCount ?? 0) > 0,
      submitDisabled: selected.submitButton?.disabled,
    },
    // W4 canonical capture: {trade_no} path identity + {method_id} (§5.5).
    request: request ? { method_id: Number(request.method_id), trade_no: request.trade_no } : null,
  };
  if (label === 'user-order-stripe-disabled-checkout') {
    return { before: base.before, selected: base.selected };
  }
  return {
    ...base,
    attempted: (terminal.stripeConfirmCount ?? 0) > 0 || (result.checkoutRequests?.length ?? 0) > 0,
    after: {
      hash: terminal.hash,
      modalCount: terminal.modalCount,
      qrCanvasCount: terminal.qrCanvasCount,
      qrSvgCount: terminal.qrSvgCount,
      submitDisabled: terminal.submitButton?.disabled,
    },
  };
}

export function normalizeServiceTableScrollInteractionResult(result) {
  const normalizeScrollPosition = (state) => {
    if (state?.scrollPosition) return state.scrollPosition;
    const className = String(state?.className ?? '');
    if (className.includes('ant-table-scroll-position-middle')) return 'middle';
    const hasLeft = className.includes('ant-table-scroll-position-left');
    const hasRight = className.includes('ant-table-scroll-position-right');
    if (hasLeft && hasRight) return 'both';
    if (hasLeft) return 'left';
    if (hasRight) return 'right';
    return '';
  };
  const normalizeRows = (rows) => {
    const uniqueRows = Array.from(
      new Set((rows ?? []).map((row) => row.replace(/\b(online|offline)\b|在线|离线/gi, ''))),
    );
    return uniqueRows.filter(
      (row) => !uniqueRows.some((other) => other !== row && other.includes(row)),
    );
  };
  const normalizeState = (state) => {
    return {
      scrollPosition: normalizeScrollPosition(state),
      maxScroll: state?.maxScroll > 0 ? 1 : 0,
      rows: normalizeRows(state?.rows),
      scrollLeft: state?.scrollLeft > 0 ? 1 : 0,
    };
  };

  return {
    afterMiddle: normalizeState(result.afterMiddle),
    afterRight: normalizeState(result.afterRight),
    before: normalizeState(result.before),
  };
}

export function normalizeTooltipSequenceInteractionResult(result) {
  return {
    before: normalizeTooltipInteractionState(result.before),
    opened: (result.opened ?? []).map(normalizeTooltipInteractionState),
    targetCount: result.targetCount,
    viewportWidth: result.viewportWidth,
  };
}

export function normalizeTooltipInteractionState(state) {
  if (!state) return state;
  return {
    openTriggerCount: state.openTriggerCount > 0 ? 1 : 0,
    placement: state.placement,
    texts: (state.texts ?? []).map(normalizeTooltipInteractionText),
    tooltipCount: state.tooltipCount > 0 ? 1 : 0,
  };
}

export function normalizeTooltipInteractionText(value) {
  const text = normalizeParityText(value);
  if (text.length % 2 !== 0) return text;
  const middle = text.length / 2;
  const left = text.slice(0, middle);
  const right = text.slice(middle);
  return left && left === right ? left : text;
}

export function normalizeInviteInteractionResult(label, result) {
  // Invite dialog/table/toast/refetch details are Tier-2 on this redesigned
  // surface. Raw per-world assertions still prove those states are usable; the
  // cross-world reducer keeps only requests consumed by the backend.
  if (label === 'user-invite-generate') {
    return { generateRequestDelta: result.generateRequestDelta };
  }
  if (
    label === 'user-invite-transfer-modal' ||
    label === 'user-invite-transfer-insufficient-balance'
  ) {
    return { transferRequests: normalizeInviteTransferRequests(result.transferRequests) };
  }
  if (label === 'user-invite-withdraw-modal') {
    return { withdrawRequests: normalizeInviteWithdrawRequests(result.withdrawRequests) };
  }
  if (label === 'user-invite-finance-submit-matrix') {
    return {
      transferRequests: normalizeInviteTransferRequests(result.transferRequests),
      withdrawRequests: normalizeInviteWithdrawRequests(result.withdrawRequests),
    };
  }
  return result;
}

function normalizeInviteTransferRequests(requests) {
  return (requests ?? []).map((request) => ({
    transfer_amount: Number(request?.transfer_amount),
  }));
}

function normalizeInviteWithdrawRequests(requests) {
  return (requests ?? []).map((request) => ({
    withdraw_account: String(request?.withdraw_account ?? ''),
    withdraw_method: String(request?.withdraw_method ?? ''),
  }));
}

export function normalizeTicketInteractionResult(label, result) {
  // Toast/spinner/modal/poll/refetch timing and draft persistence are Tier-2.
  // `assertUsefulInteraction` checks them in each world; parity itself compares
  // the ticket id and payload fields that the shared backend consumes.
  if (label === 'user-ticket-reply-send') {
    return { replyRequests: normalizeTicketReplyRequests(result.replyRequests) };
  }
  if (label === 'user-ticket-error-matrix') {
    return {
      closeRequests: normalizeTicketCloseRequests(result.closeRequests),
      hash: result.closeFailed?.hash,
      replyRequests: normalizeTicketReplyRequests(result.replyRequests),
    };
  }
  if (label === 'user-ticket-create-submit' || label === 'user-ticket-create-validation-failure') {
    return { saveRequests: normalizeTicketSaveRequests(result.saveRequests) };
  }
  return result;
}

function normalizeTicketReplyRequests(requests) {
  return (requests ?? []).map((request) => ({
    id: String(request?.id ?? ''),
    message: String(request?.message ?? ''),
  }));
}

function normalizeTicketCloseRequests(requests) {
  return (requests ?? []).map((request) => ({ id: String(request?.id ?? '') }));
}

function normalizeTicketSaveRequests(requests) {
  return (requests ?? []).map((request) => ({
    level: Number(request?.level),
    message: String(request?.message ?? ''),
    subject: String(request?.subject ?? ''),
  }));
}

export function normalizeKnowledgeInteractionResult(result) {
  const normalizeState = (state) => {
    if (!state || typeof state !== 'object') return state;
    const normalized = { ...state };
    delete normalized.searchValue;
    if (normalized.drawerOpenCount === 0) {
      normalized.drawerBodies = [];
      normalized.drawerTitles = [];
    }
    return normalized;
  };

  return Object.fromEntries(
    Object.entries(result).map(([key, value]) => [key, normalizeState(value)]),
  );
}

export function normalizeRedesignedFetchFailureInteractionResult(label, result) {
  const expectedCountKey = {
    'admin-coupons-fetch-timeout': 'adminCouponFetch',
    'admin-giftcards-fetch-timeout': 'adminGiftcardFetch',
    'admin-knowledge-fetch-timeout': 'adminKnowledgeFetch',
    'admin-notices-fetch-timeout': 'adminNoticeFetch',
    'admin-orders-fetch-api-500': 'adminOrderFetch',
    'admin-orders-fetch-timeout': 'adminOrderFetch',
    'admin-payments-fetch-timeout': 'adminPaymentFetch',
    'admin-plans-fetch-timeout': 'adminPlanFetch',
    'admin-server-manage-fetch-timeout': 'adminServerNodeFetch',
    'admin-tickets-fetch-timeout': 'adminTicketFetch',
    'admin-users-fetch-api-500': 'adminUserFetch',
    'admin-users-fetch-timeout': 'adminUserFetch',
    'user-knowledge-fetch-timeout': 'userKnowledgeFetch',
    'user-node-fetch-api-500': 'userServerFetch',
    'user-node-fetch-timeout': 'userServerFetch',
    'user-orders-fetch-api-500': 'userOrderFetch',
    'user-orders-fetch-timeout': 'userOrderFetch',
    'user-plans-fetch-timeout': 'userPlanFetch',
    'user-tickets-fetch-timeout': 'userTicketFetch',
    'user-traffic-fetch-timeout': 'userTrafficFetch',
  }[label];
  const visibleFallbackCount =
    (result.alertTexts?.length ?? 0) +
    (result.emptyTexts?.length ?? 0) +
    (result.listItemTexts?.length ?? 0) +
    (result.spinnerCount ?? 0) +
    (result.tableRows?.length ?? 0) +
    (result.tables?.length ?? 0);

  return {
    hash: result.hash,
    requestSeen: { [expectedCountKey]: !!result.requestSeen?.[expectedCountKey] },
    visibleFallbackCount: visibleFallbackCount > 0 ? 1 : 0,
  };
}

export function normalizeDashboardSubscribeDrawerInteractionResult(result) {
  return {
    before: normalizeDashboardSubscribeDrawerState(result.before),
    copied: normalizeDashboardSubscribeDrawerState(result.copied),
    opened: normalizeDashboardSubscribeDrawerState(result.opened),
    qr: normalizeDashboardSubscribeDrawerState(result.qr),
  };
}

export function normalizeDashboardSubscribeDrawerState(state) {
  if (!state) return state;
  const itemTexts = (state.itemTexts ?? []).filter(
    (text) => !isDashboardSubscribeTutorialText(text),
  );
  return {
    copied: (state.messageTexts ?? []).length > 0,
    hasCopyAction: jsonIncludesAny(itemTexts, ['复制订阅地址', 'Copy Subscription URL']),
    hasQrAction: jsonIncludesAny(itemTexts, ['扫描二维码订阅', 'Scan QR code to subscribe']),
    menuOpen: (state.boxCount ?? 0) > 0,
    qrTipVisible: jsonIncludesAny(state.qrTipTexts, [
      '使用支持扫码的客户端进行订阅',
      'Use a client app that supports scanning QR code to subscribe',
    ]),
    qrVisible: (state.qrCount ?? 0) > 0,
    targets: subscribeTargetTitles.filter((target) =>
      itemTexts.some((text) => String(text).endsWith(target)),
    ),
  };
}

export function normalizeDashboardSubscribeImportLinksInteractionResult(result) {
  return {
    before: normalizeDashboardSubscribeImportLinksState(result.before),
    expectedTargets: result.expectedTargets,
    opened: normalizeDashboardSubscribeImportLinksState(result.opened),
  };
}

export function normalizeDashboardSubscribeImportLinksState(state) {
  if (!state) return state;
  const itemTexts = (state.itemTexts ?? []).filter(
    (text) => !isDashboardSubscribeTutorialText(text),
  );

  return {
    hasCopyAction: jsonIncludesAny(itemTexts, ['复制订阅地址', 'Copy Subscription URL']),
    hasQrAction: jsonIncludesAny(itemTexts, ['扫描二维码订阅', 'Scan QR code to subscribe']),
    menuOpen: (state.boxCount ?? 0) > 0,
    targets: subscribeTargetTitles.filter((target) =>
      itemTexts.some((text) => String(text).endsWith(target)),
    ),
  };
}

export function isDashboardSubscribeTutorialText(text) {
  return /教程|tutorial/i.test(String(text ?? ''));
}

export function normalizeDashboardResetPackageConfirmInteractionResult(result) {
  const request = result.orderSaveRequests?.[0] ?? {};
  return {
    before: {
      resetTriggerCount: result.before?.resetTriggerCount > 0 ? 1 : 0,
    },
    confirmed: {
      modalCount: result.confirmed?.modalCount ?? 0,
    },
    hashIncludesOrder: Boolean(result.hash?.includes(`/order/${dashboardResetPackageTradeNo}`)),
    opened: {
      buttonCount: (result.opened?.buttons?.length ?? 0) >= 2 ? 2 : 0,
      contentCount: (result.opened?.content?.length ?? 0) > 0 ? 1 : 0,
      modalCount: result.opened?.modalCount > 0 ? 1 : 0,
      titleCount: (result.opened?.title?.length ?? 0) > 0 ? 1 : 0,
    },
    orderSaveRequests:
      result.orderSaveRequests?.length === 1
        ? [
            {
              period: request.period,
              plan_id: String(Number(request.plan_id)),
            },
          ]
        : [],
  };
}

export function normalizeDashboardNewPeriodConfirmInteractionResult(result) {
  return {
    before: {
      triggerVisible: (result.before?.newPeriodTriggerCount ?? 0) > 0,
    },
    confirmed: {
      dialogClosed: (result.confirmed?.modalCount ?? 0) === 0,
    },
    hashRoute: result.hash?.includes('/dashboard') ? '/dashboard' : '',
    opened: {
      actionCount: (result.opened?.buttons?.length ?? 0) >= 2 ? 2 : 0,
      contentVisible: (result.opened?.content?.length ?? 0) > 0,
      dialogOpen: (result.opened?.modalCount ?? 0) > 0,
      titleVisible: (result.opened?.title?.length ?? 0) > 0,
    },
    requestCount: result.newPeriodRequests?.length ?? 0,
  };
}

export function normalizeDashboardAlertLinksInteractionResult(result) {
  return {
    before: {
      hasPayLink: jsonIncludesAny(result.before?.alertLinks, ['立即支付', 'Pay Now']),
      hasViewLink: jsonIncludesAny(result.before?.alertLinks, ['立即查看', 'View Now']),
    },
    order: {
      hashRoute: result.order?.hash?.includes('/order') ? '/order' : '',
      tableReady: (result.order?.tableCount ?? 0) > 0,
    },
    reset: {
      hashRoute: result.reset?.hash?.includes('/dashboard') ? '/dashboard' : '',
      linkCount: (result.reset?.alertLinks?.length ?? 0) >= 2 ? 2 : 0,
    },
    ticket: {
      hashRoute: result.ticket?.hash?.includes('/ticket') ? '/ticket' : '',
      tableReady: (result.ticket?.tableCount ?? 0) > 0,
    },
  };
}

export function normalizeProfileDepositModalInteractionResult(result) {
  const request = result.orderSaveRequests?.[0] ?? {};
  return {
    filled: {
      amount: result.filled?.amount,
      buttonCount: (result.filled?.buttons?.length ?? 0) >= 2 ? 2 : 0,
      modalCount: result.filled?.modalCount > 0 ? 1 : 0,
    },
    hashIncludesOrder: Boolean(result.hash?.includes(`/order/${profileDepositTradeNo}`)),
    // W4 canonical capture: the deposit arm of the §9.2 create-order union
    // (the legacy plan_id: 0 + period: "deposit" sentinel folds away).
    orderSaveRequests:
      result.orderSaveRequests?.length === 1
        ? [
            {
              deposit_amount: String(Number(request.deposit_amount)),
              kind: request.kind,
            },
          ]
        : [],
  };
}

export function normalizeProfileChangePasswordInteractionResult(result) {
  return {
    after: normalizeProfileChangePasswordState(result.after),
    before: normalizeProfileChangePasswordState(result.before),
    filled: normalizeProfileChangePasswordState(result.filled),
    loading: normalizeProfileChangePasswordState(result.loading),
  };
}

export function normalizeProfileChangePasswordState(state) {
  if (!state) return state;
  return {
    ...state,
    blockTitles: state.hash?.includes('/dashboard') ? ['我的订阅', '捷径'] : state.blockTitles,
  };
}

export function normalizeUserDarkModePersistenceState(state) {
  if (!state) return state;
  return {
    activeControl: isDarkModeActiveControlState(state),
    cookieDarkMode: state.cookieDarkMode,
    darkReady: Boolean(state.darkReaderReady || state.shadcnDarkReady),
    styleCaptured: (state.styleSnapshot?.capturedCount ?? 0) >= 6,
  };
}

export function normalizeAdminServerProtocolMatrixResult(value) {
  if (Array.isArray(value)) {
    return [...new Set(value.filter((item) => item !== ''))].sort((left, right) =>
      String(left).localeCompare(String(right)),
    );
  }
  if (!value || typeof value !== 'object') return value;
  const {
    dropdownCount: _dropdownCount,
    dropdownItems: _dropdownItems,
    selectDropdownItems: _selectDropdownItems,
    tableRows: _tableRows,
    // Field labels render structurally differently across the shadcn island and
    // the antd drawer (MultiCheckboxField "权限组"/"Default"/"1"/"2" vs the antd
    // "权限组添加权限组" multi-select, "父节点" vs "父节点更多解答"). Each DOM's
    // conditional fields are verified per-target by the raw assertion, so drop
    // labels from the source-vs-oracle compare as Tier-2 presentation.
    labels: _labels,
    ...rest
  } = value;
  return Object.fromEntries(
    Object.entries(rest)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, normalizeAdminServerProtocolMatrixResult(nested)]),
  );
}

export function normalizeSelectDropdownInteractionResult(label, result) {
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
  const stripUnstableSelectedItems = (state) => {
    if (
      ![
        'admin-users-filter-field-select-dropdown',
        'admin-user-create-plan-select-dropdown',
      ].includes(label) ||
      !state
    ) {
      return state;
    }
    const { selectedItems: _selectedItems, ...rest } = state;
    return rest;
  };
  return {
    ...result,
    before: stripUnstableSelectedItems(
      stripUnstableModalGeometry(stripTransientSelectMotionClass(result.before)),
    ),
    opened: stripUnstableSelectedItems(
      stripUnstableModalGeometry(stripTransientSelectMotionClass(result.opened)),
    ),
  };
}

// Redesigned shadcn dialogs append an sr-only close label (t('common.close_dialog'))
// as the modal's last child, which textContent/aria captures where the legacy oracle's
// antd close is an icon outside the compared region. Strip that trailing close label in
// every locale the interaction scenarios run in (plus the legacy English "Close").
export function withoutTrailingCloseLabel(text) {
  return text.replace(
    /(?:Close dialog|Close|关闭弹窗|關閉彈窗|ダイアログを閉じる|Đóng hộp thoại|대화 상자 닫기)$/u,
    '',
  );
}

export function normalizeDashboardDialogText(value) {
  return withoutTrailingCloseLabel(normalizeParityText(value))
    .replace(/^一键订阅(?=复制订阅地址|扫描二维码订阅|导入到)/u, '')
    .replace(/^扫描二维码订阅(?=使用支持扫码的客户端进行订阅)/u, '');
}

export function normalizeDashboardSubscribeItemClassName(value, attributes = {}) {
  const className = String(value ?? '');
  const tokens = ['item___yrtOv'];
  if (
    className.includes('subsrcibe-for-link') ||
    attributes.testId === 'dashboard-subscribe-copy'
  ) {
    tokens.push('subsrcibe-for-link');
  }
  if (
    className.includes('subscribe-for-qrcode') ||
    attributes.testId === 'dashboard-subscribe-qrcode'
  ) {
    tokens.push('subscribe-for-qrcode');
  }
  if (className.includes('hiddify') || attributes.subscribeTarget === 'hiddify') {
    tokens.push('hiddify');
  }
  if (className.includes('sing-box') || attributes.subscribeTarget === 'sing-box') {
    tokens.push('sing-box');
  }
  return tokens.join(' ');
}

export function normalizeDashboardNoticeModalBody(value, title) {
  // The redesigned dialog appends an sr-only close label inside
  // [data-testid="dashboard-dialog"] (t('common.close_dialog')), which textContent
  // captures; the legacy oracle's antd close is an icon outside .ant-modal-body.
  // Strip the localized close label (every locale the scenarios run in, plus the
  // legacy English "Close") so the comparison is on the notice body, not chrome.
  const closeLabels = [
    'Close',
    '关闭弹窗',
    '關閉彈窗',
    'Close dialog',
    'ダイアログを閉じる',
    'Đóng hộp thoại',
    '대화 상자 닫기',
  ];
  let text = normalizeParityText(value);
  for (const label of closeLabels) {
    if (text.endsWith(label)) {
      text = text.slice(0, -label.length).trimEnd();
      break;
    }
  }
  const normalizedTitle = normalizeParityText(title);
  if (normalizedTitle && text.startsWith(normalizedTitle)) {
    text = text.slice(normalizedTitle.length);
  }
  return text;
}

export function normalizeDashboardConfirmButtons(values) {
  return values
    .map((value) => withoutTrailingCloseLabel(normalizeParityText(value)))
    .filter(Boolean);
}

export function normalizeDashboardConfirmContent(values, titles) {
  const normalizedTitles = titles.map(normalizeParityText).filter(Boolean);
  return Array.from(
    new Set(
      values
        .map((value) => {
          let text = normalizeParityText(value).replace(/Close$/u, '');
          for (const title of normalizedTitles) {
            if (text.startsWith(title)) {
              text = text.slice(title.length);
            }
          }
          return text.replace(/取消(?:确定|确认)$/u, '').trim();
        })
        .filter(Boolean),
    ),
  );
}

export function normalizeProfileBlockTitles(values) {
  // The redesigned profile adds an account-info card (profile.account) and an
  // active-sessions card (profile.active_sessions) the legacy oracle has no
  // equivalent for — neither carries a backend contract (they are absent from the
  // AGENTS.md profile Tier-1 list). Drop those redesign-only titles in every locale
  // the scenarios run in so the card-inventory comparison stays on the legacy-common
  // set; the reset / telegram / preference / gift-card / password behavior each
  // scenario exercises is asserted separately.
  const redesignOnlyTitles = new Set(
    [
      '账户信息',
      '帳戶資訊',
      'Account',
      'アカウント情報',
      'Thông tin tài khoản',
      '계정 정보',
      '登录设备',
      '登入裝置',
      'Active Sessions',
      'ログイン中のデバイス',
      'Thiết bị đăng nhập',
      '로그인된 기기',
    ].map(normalizeParityText),
  );
  return values
    .map(normalizeParityText)
    .filter(Boolean)
    .filter((text) => !/^-?\d+(?:\.\d+)?[A-Z]{2,5}$/u.test(text))
    .filter((text) => !redesignOnlyTitles.has(text));
}

export function normalizeProfileTelegramBindBodies(values, titles) {
  const normalizedTitles = titles.map(normalizeParityText).filter(Boolean);
  return Array.from(
    new Set(
      values
        .map((value) => {
          let text = withoutTrailingCloseLabel(normalizeParityText(value));
          for (const title of normalizedTitles) {
            if (text.startsWith(title)) text = text.slice(title.length);
          }
          return text
            .replace(/我知道了$/u, '')
            .replace(/I understand$/u, '')
            .replace(/(@[A-Za-z0-9_]+)(?=(第二步|Second Step))/u, '$1 ')
            .trim();
        })
        .filter(Boolean),
    ),
  );
}

export function normalizeProfileTelegramIdTexts(values) {
  const texts = values.map(normalizeParityText).filter(Boolean);
  const actionTexts = texts.filter((text) =>
    /^(解除绑定|Unbind|Unlink|解除绑定 Telegram|Unbind Telegram)$/u.test(text),
  );
  const idTexts = texts.filter((text) => /Telegram ID:/u.test(text));
  return [...new Set([...actionTexts, ...idTexts])];
}

export function normalizeProfilePreferenceLabels(values) {
  return values
    .map(normalizeParityText)
    .filter((text) =>
      [
        '自动续费',
        'Auto Renewal',
        '到期邮件提醒',
        'Subscription expiration email reminder',
        '流量邮件提醒',
        'Insufficient transfer data email alert',
      ].includes(text),
    );
}

export function normalizeProfileActionButtonState(button) {
  if (!button) return null;
  return {
    ...button,
    className: button.loading
      ? 'ant-btn ant-btn-loading ant-btn-primary'
      : 'ant-btn ant-btn-primary',
    disabled: false,
    text: normalizeProfileActionButtonText(button.text),
  };
}

function normalizeProfileRedeemFailureInteractionResult(result) {
  const normalizeState = (state) => {
    if (!state?.redeemButton) return state;
    return {
      ...state,
      redeemButton: {
        ...state.redeemButton,
        className: 'profile-action-button',
        disabled: false,
        loading: false,
      },
      toastTexts: [],
    };
  };
  return {
    ...result,
    after: normalizeState(result.after),
    before: normalizeState(result.before),
    filled: normalizeState(result.filled),
    loading: normalizeState(result.loading),
  };
}

export function normalizeProfileActionButtonText(value) {
  const text = normalizeParityText(value).replace(/\s+/g, '');
  if (text === '兑换') return '兑 换';
  if (text === '保存') return '保 存';
  return normalizeParityText(value);
}

export function normalizeDashboardOrderInfo(values) {
  const normalized = values.map(normalizeParityText).filter(Boolean);
  const result = [];
  for (const text of normalized) {
    const traffic = text.match(/产品流量[:：]?\s*([^ ]+(?: [A-Za-z]+)?)/u)?.[1];
    if (traffic) {
      const value = `产品流量：${traffic}`;
      if (!result.includes(value)) result.push(value);
    }
    const tradeNo = text.match(/订单号[:：]?\s*([A-Z0-9-]+)/u)?.[1];
    if (tradeNo) {
      const createdAt = text.match(/创建时间[:：]?\s*([0-9/-]+ [0-9:]+)/u)?.[1];
      const value = `订单号：${tradeNo}${createdAt ? `创建时间：${createdAt}` : ''}`;
      if (!result.includes(value)) result.push(value);
    }
  }
  return result;
}

export function normalizeDashboardRouteAlertLinks(values) {
  return values
    .map(normalizeParityText)
    .filter((text) => ['立即支付', 'Pay Now', '立即查看', 'View Now'].includes(text));
}

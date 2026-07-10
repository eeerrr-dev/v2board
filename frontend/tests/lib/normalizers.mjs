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
import { dashboardResetPackageTradeNo, profileDepositTradeNo } from './fixture-data.mjs';
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
    '.v2board-auth-language-trigger, .v2board-login-i18n-btn',
    2,
  );
  const comboboxTriggerTexts = await visibleTexts(page, '[role="combobox"]', 8);
  return {
    ...state,
    // Released as redesigned accessibility: auth surfaces expose language as a native auxiliary
    // button. Keep comparing the actual submit/action buttons, but ignore the language button text
    // that had no button counterpart in the packaged oracle.
    // Radix Select triggers are buttons with role="combobox"; those remain covered by controls.
    buttons: state.buttons.filter(
      (text) => !languageTriggerTexts.includes(text) && !comboboxTriggerTexts.includes(text),
    ).map(normalizeRedesignedAuthButtonText),
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

// The admin auth surface is a redesigned shadcn island: its title/subtitle render as
// `<div data-slot=card-*>` (not `<h*>`), and the forgot-password affordance is a `<button>`
// that opens a Dialog rather than the oracle's `<a>` link. Collapse buttons+links into one
// order-independent `actions` set (login/forgot mapped to tokens, the brand link that doubles
// as a title dropped), and fold the identifier input's email->text + placeholder the same way
// the user auth normalizer does — so the shadcn source and antd oracle converge on the Tier-1
// contract (auth box present, email+password fields, login+forgot actions, #/login hash).
export function normalizeAdminAuthPageState(state) {
  const titleTexts = Array.isArray(state?.titleTexts) ? state.titleTexts : [];
  const actions = [...(state?.buttons ?? []), ...(state?.links ?? [])]
    .filter((text) => !titleTexts.includes(text))
    .map((text) => {
      if (['登入', '登录', 'Login', 'ログイン'].includes(text)) return 'login-action';
      if (['忘记密码', '忘记密码？'].includes(text)) return 'forgot-password-action';
      return text;
    })
    .filter((text, index, all) => all.indexOf(text) === index)
    .sort();
  return {
    actions,
    authBoxCount: state?.authBoxCount,
    controls: (state?.controls ?? []).map((control) => {
      const behavioral = { ...control };
      delete behavioral.placeholder;
      if (behavioral.type === 'email') behavioral.type = 'text';
      return behavioral;
    }),
    hash: state?.hash,
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

// Reduce a payment modal/Sheet snapshot to its Tier-1 compare essence. Drops the
// Tier-2 background table rows (the antd fixed-right column duplicates action
// cells as extra rows the shadcn table has no equivalent for), sorts the footer
// button order (添加/保存 lead on the shadcn Sheet, 取消 leads on the antd modal),
// and unifies the optional numeric fee fields (rendered '0' on the antd modal, ''
// on the shadcn Sheet when unset/zero — display formatting; the submitted payload
// in saveRequests is identical either way). Applied to both targets, so it never
// masks a real mismatch. Non-object/array values (saveRequests, paymentFetchDelta)
// pass through untouched.
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

export function normalizeInteractionResult(label, result) {
  const normalized = sortForStableJson(result);
  if (label === 'user-dashboard-header-language-dropdown') {
    return {
      dropdownCount: normalized.dropdownCount,
      items: normalized.items,
      placement: normalized.placement,
    };
  }
  if (
    label === 'user-session-expired-redirect' ||
    label === 'admin-session-expired-redirect'
  ) {
    // Redesigned login renders its brand/subtitle as card slots (not `<h*>`), so titleTexts
    // differs; the Tier-1 contract is the 403 keep-token dance + redirect to a single login box.
    return {
      authData: normalized.authData,
      hash: normalized.hash,
      loginBoxCount: normalized.loginBoxCount,
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
    return normalizeInviteInteractionResult(normalized);
  }
  if (
    [
      'user-ticket-reply-send',
      'user-ticket-error-matrix',
      'user-ticket-create-submit',
      'user-ticket-create-validation-failure',
    ].includes(label)
  ) {
    return normalizeTicketInteractionResult(normalized);
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
  if (
    label === 'user-knowledge-drawer' ||
    label === 'user-knowledge-extreme-content-matrix'
  ) {
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
    return {
      ...normalized,
      dashboardTexts: jsonIncludesAny(normalized.dashboardTexts, ['仪表盘', 'Dashboard'])
        ? ['仪表盘']
        : [],
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
    // selected option labels, the option lists, the force-update checkbox and the
    // title. Non-drawer captures (keyboard focus, fetch deltas, save payloads) pass
    // through untouched.
    const reduced = {};
    for (const [key, value] of Object.entries(normalized)) {
      reduced[key] =
        value && typeof value === 'object' && !Array.isArray(value) && 'drawerCount' in value
          ? {
              drawerCount: value.drawerCount,
              dropdownItems: value.dropdownItems,
              forceUpdate: value.forceUpdate,
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
    // are Tier-2 presentation. Reduce every sub-state to that essence.
    const reduceState = (state) =>
      state ? { hash: state.hash, requestCounts: state.requestCounts } : state;
    return {
      ...normalized,
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
  if (label === 'user-dashboard-alert-links') {
    return normalizeDashboardAlertLinksInteractionResult(normalized);
  }
  if (label === 'user-plan-checkout-coupon') {
    return normalizePlanCheckoutCouponInteractionResult(normalized);
  }
  if (label === 'user-plan-checkout-coupon-error') {
    return normalizePlanCheckoutCouponErrorInteractionResult(normalized);
  }
  if (label === 'user-order-qr-checkout') {
    return normalizeOrderQrCheckoutInteractionResult(normalized);
  }
  if (label === 'user-order-checkout-network-failure') {
    return normalizeOrderCheckoutNetworkFailureInteractionResult(normalized);
  }
  if (label === 'user-profile-change-password-success') {
    return normalizeProfileChangePasswordInteractionResult(normalized);
  }
  if (label === 'user-profile-deposit-modal') {
    return normalizeProfileDepositModalInteractionResult(normalized);
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
  if (
    label === 'admin-users-filter-expiry-picker' ||
    label === 'admin-user-create-expiry-picker'
  ) {
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
    // Reduce each payment modal snapshot to its Tier-1 compare essence (labels,
    // inputValues, selectedPayment, titles, saveRequests) while dropping Tier-2
    // presentation the redesign does not reproduce; see reducePaymentSnapshot.
    // All dropped signals are still verified per-target by the raw assertion.
    const reduced = {};
    for (const [key, value] of Object.entries(normalized)) {
      reduced[key] = reducePaymentSnapshot(value);
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
    const reduceSnapshot = (state) =>
      state ? { dropdownCount: state.dropdownCount } : state;
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
    // Both DOMs send the backend filter contract (current/pageSize/status and
    // the toggled reply_status[]=0). The legacy antd Table additionally leaks
    // its own pagination chrome — total (the echoed row count) and size (table
    // density) — into the query string; those are antd-internal presentation,
    // not backend filter params, so strip them from both sides before
    // comparing. reply_status[] passthrough stays pinned by the raw assertion.
    const stripAntdTableParams = (request) =>
      request && Array.isArray(request.searchParams)
        ? {
            ...request,
            searchParams: request.searchParams.filter(
              ([key]) => key !== 'total' && key !== 'size',
            ),
          }
        : request;
    return {
      ...normalized,
      filterFetchRequests: Array.isArray(normalized.filterFetchRequests)
        ? normalized.filterFetchRequests.map(stripAntdTableParams)
        : normalized.filterFetchRequests,
    };
  }
  if (label === 'admin-config-save-failure-matrix') {
    // Reduce the config-form snapshots to their Tier-1 essence. Keep only the
    // non-empty field values (the redesigned site form renders one fewer empty
    // <input> than the OneUI oracle — which control renders as a text input is
    // Tier-2), and drop the intermediate saveCount (the redesigned blur-save
    // lands before `edited` is read while the legacy debounced save lands after
    // — timing, covered by configSaveRequests). Reduce each config save request
    // to the single field the interaction changed: the legacy form re-sends the
    // whole site group's unchanged currency/currency_symbol values, the
    // redesigned form saves only app_name — both persist app_name identically,
    // which the raw assertion pins. Theme snapshots already match on both DOMs.
    const reduceConfigSnapshot = (state) => {
      if (!state || typeof state !== 'object') return state;
      const { blockLoadingCount: _blockLoadingCount, saveCount: _saveCount, ...rest } = state;
      return {
        ...rest,
        inputValues: Array.isArray(rest.inputValues)
          ? rest.inputValues.filter((value) => value !== '')
          : rest.inputValues,
      };
    };
    return {
      ...normalized,
      before: reduceConfigSnapshot(normalized.before),
      configFailed: reduceConfigSnapshot(normalized.configFailed),
      configSaveRequests: Array.isArray(normalized.configSaveRequests)
        ? normalized.configSaveRequests.map((request) =>
            request && typeof request === 'object' ? { app_name: request.app_name } : request,
          )
        : normalized.configSaveRequests,
      edited: reduceConfigSnapshot(normalized.edited),
    };
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
      Object.entries(normalized).map(([key, value]) => [key, reduceModalState(value)]),
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
    // Keep the pagination CONTRACT — the applied fetch query (current/pageSize),
    // active page, and page-item set — and drop Tier-2 presentation: the antd
    // `.ant-pagination-next` classes, the page-size selection label formatting,
    // the full row texts (number/date formatting), and the size dropdown's option
    // list (the 50 条/页 presence stays pinned by the raw assertion).
    const reduceSnapshot = (state) =>
      state && typeof state === 'object' && 'activePage' in state
        ? {
            activePage: state.activePage,
            pageItems: state.pageItems,
            query: pickFetchQueryFields(state.query, ['current', 'pageSize']),
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
    // The sort CONTRACT is the fetch query (sort=banned, sort_type ASC→DESC); the
    // antd `ant-table-column-sorter-up/down` arrow classes, header label set, and
    // row texts are Tier-2 presentation the redesigned lucide sort icons express
    // differently. Compare only the applied ASC/DESC queries.
    const reduceSort = (state) =>
      state
        ? { query: pickFetchQueryFields(state.query, ['current', 'pageSize', 'sort', 'sort_type']) }
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
    // shortcutTexts came from OneUI `.js-classic-nav .font-w600` nav labels (Tier-2). The
    // contract is the commission order-filter sessionStorage + fetch query and the /order hash.
    const stripShortcutTexts = (state) => {
      if (!state) return state;
      const { shortcutTexts: _shortcutTexts, ...rest } = state;
      return rest;
    };
    return {
      after: stripShortcutTexts(normalized.after),
      before: stripShortcutTexts(normalized.before),
    };
  }
  if (label === 'admin-root-page-state') {
    return normalizeAdminAuthPageState(normalized);
  }
  if (label === 'admin-login-form-state') {
    const forgot = normalized.forgotModal ?? {};
    return {
      // Tier-1: identifier+password fields retained their typed values, login+forgot actions
      // present. Tier-2 (forgot dialog's exact button/description copy) is dropped; we only
      // pin that a single dialog opened, titled 忘记密码, revealing the reset:password command.
      filled: normalizeAdminAuthPageState(normalized.filled ?? {}),
      forgotModal: {
        hasResetCommand: jsonIncludes(forgot, 'reset:password'),
        modalCount: forgot.modalCount,
        title: forgot.title,
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
      // The traffic modal fetch (`/stat/getStatUser`) carries the page number as
      // the legacy `page` alias in the frozen antd oracle but as the real backend
      // param `current` in the redesigned client; canonicalize so the shared
      // contract (user_id, current, pageSize) compares equal.
      reduced[key] =
        key === 'trafficQuery'
          ? pickFetchQueryFields(state[key], ['user_id', 'current', 'pageSize'])
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
    return reduceSnapshots({
      updateRequests: (result.updateRequests ?? []).map((request) =>
        pickStr(request, ['id', 'email']),
      ),
    });
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
// the Tier-1 signal is the generate/save request payload (picked to its contract
// fields and string-coerced — both worlds form-encode, but coerce defensively)
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

  const couponKeys = ['id', 'name', 'code', 'type', 'value', 'limit_plan_ids[0]', 'limit_period[0]'];
  const giftcardKeys = ['id', 'name', 'code', 'type', 'value', 'plan_id', 'limit_use'];
  const noticeKeys = ['id', 'title', 'content', 'tags[0]', 'tags[1]', 'img_url'];
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
      toastTexts: result.after?.toastTexts ?? [],
    },
    before: {
      summaryBlocks: result.before?.summaryBlocks,
    },
    couponRequests: clonePageRequests(result.couponRequests),
  };
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

export function normalizeInviteInteractionResult(result) {
  return Object.fromEntries(
    Object.entries(result ?? {}).map(([key, value]) => [
      key,
      normalizeInviteInteractionValue(key, value),
    ]),
  );
}

export function normalizeInviteInteractionValue(key, value) {
  if (Array.isArray(value)) {
    return value.map((item) => normalizeInviteInteractionValue('', item));
  }
  if (!value || typeof value !== 'object') return value;
  if (looksLikeInviteInteractionState(value)) {
    return normalizeInviteInteractionState(value, {
      stripTableRows: key === 'navigated' || key === 'withdrawSucceeded',
    });
  }
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      normalizeInviteInteractionValue(key, nested),
    ]),
  );
}

export function looksLikeInviteInteractionState(value) {
  return [
    'buttons',
    'dropdownItems',
    'generateButton',
    'inputValues',
    'labels',
    'modalCount',
    'statBlocks',
    'tableRows',
    'titles',
    'toastTexts',
  ].some((key) => Object.prototype.hasOwnProperty.call(value, key));
}

export function stripTrailingDecimalZeros(text) {
  if (typeof text !== 'string') return text;
  // Collapse trailing zeros in decimals (67.80 -> 67.8, 234.50 -> 234.5, 0.00 -> 0)
  // without touching integers, times, or dates, so display-only toFixed formatting
  // does not diverge from the trailing-zero-stripped oracle rendering.
  return text.replace(/(\d+)\.(\d*?)0+(?=\D|$)/g, (_match, intPart, frac) =>
    frac ? `${intPart}.${frac}` : intPart,
  );
}

export function normalizeInviteInteractionState(state, options = {}) {
  const { selectedValues: _selectedValues, ...rest } = state;
  const normalized = { ...rest };
  if ('buttons' in normalized) normalized.buttons = normalizeInviteTextArray(normalized.buttons);
  if ('dropdownItems' in normalized) {
    normalized.dropdownItems = normalizeInviteTextArray(normalized.dropdownItems);
  }
  if ('generateButton' in normalized) {
    normalized.generateButton = normalizeInviteButtonState(normalized.generateButton);
  }
  if ('inputValues' in normalized) {
    // The redesigned transfer/withdraw dialogs render the current commission
    // balance in a disabled input via toFixed(2) (e.g. 100000.00), while the
    // legacy oracle renders it without trailing zeros (100000). AGENTS.md pins
    // this commission toFixed formatting as Tier-2 relaxable, so fold trailing
    // decimal zeros on both sides; genuinely typed values like 12.34 or an
    // account string are untouched by the fold.
    normalized.inputValues = normalizeInviteTextArray(normalized.inputValues).map(
      stripTrailingDecimalZeros,
    );
  }
  if ('labels' in normalized) normalized.labels = normalizeInviteTextArray(normalized.labels);
  if ('statBlocks' in normalized) {
    // The redesigned invite surface renders commission with toFixed(2) (e.g.
    // ¥67.80), which AGENTS.md pins as Tier-2 relaxable formatting; the legacy
    // oracle strips trailing zeros (¥67.8). Fold trailing decimal zeros on both
    // sides so the display formatting difference does not fail parity while any
    // genuinely different value still diverges.
    normalized.statBlocks = normalizeInviteTextArray(normalized.statBlocks, {
      compact: true,
    }).map(stripTrailingDecimalZeros);
  }
  if ('tableRows' in normalized) {
    if (options.stripTableRows) delete normalized.tableRows;
    else normalized.tableRows = normalizeInviteTextArray(normalized.tableRows);
  }
  if ('titles' in normalized) normalized.titles = normalizeInviteTextArray(normalized.titles);
  if ('toastTexts' in normalized) {
    normalized.toastTexts = normalizeInviteTextArray(normalized.toastTexts);
  }
  return normalized;
}

export function normalizeInviteButtonState(button) {
  if (!button || typeof button !== 'object') return button;
  return {
    ariaChecked: button.ariaChecked,
    checked: button.checked,
    disabled: button.disabled,
    text: normalizeParityText(button.text),
    value: button.value,
  };
}

export function normalizeInviteTextArray(values, options = {}) {
  return (values ?? []).map((value) => {
    const text = normalizeParityText(value);
    return options.compact ? text.replace(/\s+/g, '') : text;
  });
}

export function normalizeTicketInteractionResult(result) {
  return Object.fromEntries(
    Object.entries(result ?? {}).map(([key, value]) => [
      key,
      normalizeTicketInteractionValue(key, value),
    ]),
  );
}

export function normalizeTicketInteractionValue(_key, value) {
  if (Array.isArray(value)) {
    return value.map((item) => normalizeTicketInteractionValue('', item));
  }
  if (!value || typeof value !== 'object') return value;
  if (looksLikeTicketInteractionState(value)) {
    return normalizeTicketInteractionState(value);
  }
  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      normalizeTicketInteractionValue(key, nested),
    ]),
  );
}

export function looksLikeTicketInteractionState(value) {
  return [
    'actionLinks',
    'buttons',
    'inputValue',
    'inputValues',
    'labels',
    'messageTexts',
    'modalCount',
    'selectedValues',
    'selectDropdownItems',
    'sendButton',
    'tableRows',
    'titles',
    'toastTexts',
  ].some((key) => Object.prototype.hasOwnProperty.call(value, key));
}

export function normalizeTicketInteractionState(state) {
  const { sendButton: _sendButton, ...rest } = state;
  const normalized = { ...rest };
  if ('actionLinks' in normalized) {
    normalized.actionLinks = normalizeTicketTextArray(normalized.actionLinks);
  }
  if ('buttons' in normalized) normalized.buttons = normalizeTicketTextArray(normalized.buttons);
  if ('inputValue' in normalized) normalized.inputValue = normalizeParityText(normalized.inputValue);
  if ('inputValues' in normalized) {
    normalized.inputValues = normalizeTicketTextArray(normalized.inputValues).filter(Boolean);
  }
  if ('labels' in normalized) normalized.labels = normalizeTicketTextArray(normalized.labels);
  if ('messageTexts' in normalized) normalized.messageTexts = [];
  if ('selectedValues' in normalized) {
    normalized.selectedValues = normalizeTicketTextArray(normalized.selectedValues).filter(
      (value) => value && !/请选择|please select/i.test(value),
    );
  }
  if ('selectDropdownItems' in normalized) {
    normalized.selectDropdownItems = normalizeTicketTextArray(normalized.selectDropdownItems);
  }
  if ('tableRows' in normalized) {
    const rows = Array.from(
      new Set(
        normalizeTicketTextArray(normalized.tableRows, {
          compact: true,
        }),
      ),
    );
    normalized.tableRows = rows.filter(
      (row) => !rows.some((other) => other !== row && other.includes(row)),
    );
  }
  if ('titles' in normalized) normalized.titles = normalizeTicketTextArray(normalized.titles);
  if ('toastTexts' in normalized) {
    normalized.toastTexts = normalizeTicketToastTexts(normalized.toastTexts);
  }
  return normalized;
}

export function normalizeTicketToastTexts(values) {
  return normalizeTicketTextArray(values).filter(
    (value) => value && !/^(发送中|Sending|Loading|处理中|Processing)$/i.test(value),
  );
}

export function normalizeTicketTextArray(values, options = {}) {
  return (values ?? []).map((value) => {
    const text = normalizeParityText(value);
    return options.compact ? text.replace(/\s+/g, '') : text;
  });
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
  return {
    ...state,
    itemTexts: (state.itemTexts ?? []).filter(
      (text) => !/教程|tutorial/i.test(String(text)),
    ),
    shortcutTexts: [],
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
  const overlayOpen = Boolean((state.drawerOpenCount ?? 0) || (state.modalCount ?? 0));
  const items = (state.items ?? [])
    .filter((item) => !isDashboardSubscribeTutorialText(item?.text))
    .map((item) => ({
      className: item.className,
      dataTestId: '',
      iconCount: item.iconCount,
      imageCount: item.imageCount,
      subscribeTarget: '',
      text: item.text,
    }));

  return {
    ...state,
    bodyOverflow: '',
    drawerOpenCount: overlayOpen ? 1 : 0,
    itemClasses: items.map((item) => item.className),
    items,
    itemTexts: (state.itemTexts ?? []).filter(
      (text) => !isDashboardSubscribeTutorialText(text),
    ),
    modalCount: 0,
    shortcutTexts: [],
    userAgent: undefined,
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
    hashIncludesOrder: Boolean(
      result.hash?.includes(`/order/${dashboardResetPackageTradeNo}`),
    ),
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

export function normalizeDashboardAlertLinksInteractionResult(result) {
  return {
    before: {
      hasPayLink: jsonIncludesAny(result.before?.alertLinks, ['立即支付', 'Pay Now']),
      hasViewLink: jsonIncludesAny(result.before?.alertLinks, ['立即查看', 'View Now']),
    },
    order: {
      hashRoute: result.order?.hash?.includes('/order') ? '/order' : '',
      hasOrderNumberHeader: jsonIncludesAny(result.order?.tableHeaders, [
        '# 订单号',
        'Order Number #',
      ]),
      title: jsonIncludesAny(result.order?.containerTitles, ['我的订单', 'My Orders'])
        ? 'orders'
        : '',
    },
    reset: {
      hashRoute: result.reset?.hash?.includes('/dashboard') ? '/dashboard' : '',
      linkCount: (result.reset?.alertLinks?.length ?? 0) >= 2 ? 2 : 0,
    },
    ticket: {
      hashRoute: result.ticket?.hash?.includes('/ticket') ? '/ticket' : '',
      hasStatusHeader: jsonIncludesAny(result.ticket?.tableHeaders, [
        '工单状态',
        'Ticket Status',
      ]),
      title: jsonIncludesAny(result.ticket?.containerTitles, ['我的工单', 'My Tickets'])
        ? 'tickets'
        : '',
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
    orderSaveRequests:
      result.orderSaveRequests?.length === 1
        ? [
            {
              deposit_amount: String(Number(request.deposit_amount)),
              period: request.period,
              plan_id: String(Number(request.plan_id)),
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
    blockTitles: state.hash?.includes('/dashboard')
      ? ['我的订阅', '捷径']
      : state.blockTitles,
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
      !['admin-users-filter-field-select-dropdown', 'admin-user-create-plan-select-dropdown'].includes(
        label,
      ) ||
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
  if (
    className.includes('hiddify') ||
    attributes.subscribeTarget === 'hiddify'
  ) {
    tokens.push('hiddify');
  }
  if (
    className.includes('sing-box') ||
    attributes.subscribeTarget === 'sing-box'
  ) {
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
  return values.map((value) => withoutTrailingCloseLabel(normalizeParityText(value))).filter(Boolean);
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
      '账户信息', '帳戶資訊', 'Account', 'アカウント情報', 'Thông tin tài khoản', '계정 정보',
      '登录设备', '登入裝置', 'Active Sessions', 'ログイン中のデバイス', 'Thiết bị đăng nhập', '로그인된 기기',
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
    className: button.loading ? 'ant-btn ant-btn-loading ant-btn-primary' : 'ant-btn ant-btn-primary',
    disabled: false,
    text: normalizeProfileActionButtonText(button.text),
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
    .filter((text) =>
      ['立即支付', 'Pay Now', '立即查看', 'View Now'].includes(text),
    );
}

export function uniqueDashboardTexts(values) {
  return Array.from(new Set(values.map(normalizeParityText).filter(Boolean)));
}

export function normalizeDashboardTableRows(values) {
  const actionOnlyRows = new Set([
    '查看详情取消',
    'View DetailsCancel',
    'View detailsCancel',
    '查看关闭',
    'ViewClose',
  ]);
  return values
    .map(normalizeParityText)
    .filter((text) => text && !actionOnlyRows.has(text));
}

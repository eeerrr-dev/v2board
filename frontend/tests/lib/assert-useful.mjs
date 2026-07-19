import {
  dashboardSubscribeTargetsMatch,
  jsonIncludes,
  jsonIncludesAny,
  requestIncludesParamValue,
  stableJson,
} from './json-util.mjs';
import {
  couponCheckFixture,
  couponErrorCode,
  dashboardResetPackageTradeNo,
  profileDepositTradeNo,
} from './fixture-data.mjs';
import { adminPath } from './env.mjs';

export function isDarkModeReadyState(state) {
  return Boolean(state?.darkReaderReady || state?.shadcnDarkReady);
}

export function isDarkModeActiveControlState(state) {
  // Legacy oracle: fa-moon icon. Shadcn shell: static "Toggle theme" trigger,
  // so active state is witnessed by shadcnDarkReady + a visible svg icon.
  return Boolean(
    state?.iconClass?.includes('fa-moon') || (state?.shadcnDarkReady && state?.visibleSvgIcon),
  );
}

function hasExpectedStripeCheckoutContract(result, target, legacyOracleToken) {
  const terminal = result.checkedOut ?? result.after ?? {};
  const intentRequest = result.stripeIntentRequests?.[0];
  const legacyRequest = result.checkoutRequests?.[0];

  if (target === 'source') {
    return Boolean(
      result.stripeIntentRequests?.length === 1 &&
      intentRequest?.trade_no === 'VISUAL2026110001' &&
      Number(intentRequest?.method_id) === 2 &&
      terminal.stripeConfirmCount === 1 &&
      terminal.stripeUnexpectedCreateTokenCount === 0 &&
      result.checkoutRequests?.length === 0,
    );
  }

  // The frozen packaged oracle predates PaymentIntent and can only witness the
  // equivalent order/method contract through its historical token checkout.
  // Keeping this branch target-specific prevents the modern source from ever
  // satisfying parity by regressing to CardElement/createToken.
  if (target === 'oracle') {
    return Boolean(
      result.stripeIntentRequests?.length === 0 &&
      result.checkoutRequests?.length === 1 &&
      legacyRequest?.trade_no === 'VISUAL2026110001' &&
      Number(legacyRequest?.method_id) === 2 &&
      legacyRequest?.token === legacyOracleToken,
    );
  }

  return false;
}

function hasExpectedStripePreparationContract(result, target) {
  const intentRequest = result.stripeIntentRequests?.[0];
  if (target === 'source') {
    return Boolean(
      result.selected?.stripeIntentCount === 1 &&
      result.selected?.stripePublicKeyCount === 0 &&
      result.stripeIntentRequests?.length === 1 &&
      intentRequest?.trade_no === 'VISUAL2026110001' &&
      Number(intentRequest?.method_id) === 2,
    );
  }
  if (target === 'oracle') {
    return Boolean(
      result.selected?.stripeIntentCount === 0 &&
      (result.selected?.stripePublicKeyCount ?? 0) >= 1 &&
      result.stripeIntentRequests?.length === 0,
    );
  }
  return false;
}

function hasExactObjectKeys(value, expectedKeys) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const actualKeys = Object.keys(value);
  return (
    actualKeys.length === expectedKeys.length &&
    expectedKeys.every((key) => Object.hasOwn(value, key))
  );
}

export function hasUsefulDarkModeStyleSnapshot(state) {
  const snapshot = state?.styleSnapshot;
  return Boolean(
    snapshot?.capturedCount >= 6 &&
    (snapshot?.elements?.body?.color || snapshot?.elements?.body?.backgroundColor) &&
    (snapshot?.elements?.pageHeader?.backgroundColor ||
      snapshot?.elements?.sidebar?.backgroundColor ||
      snapshot?.elements?.mainContainer?.backgroundColor),
  );
}

export function isUsefulDarkModePersistenceResult(result) {
  return Boolean(
    result.before?.cookieDarkMode !== '1' &&
    !isDarkModeReadyState(result.before) &&
    result.afterEnable?.cookieDarkMode === '1' &&
    isDarkModeReadyState(result.afterEnable) &&
    isDarkModeActiveControlState(result.afterEnable) &&
    hasUsefulDarkModeStyleSnapshot(result.afterEnable) &&
    result.afterReload?.cookieDarkMode === '1' &&
    isDarkModeReadyState(result.afterReload) &&
    isDarkModeActiveControlState(result.afterReload) &&
    hasUsefulDarkModeStyleSnapshot(result.afterReload),
  );
}

export function assertUsefulInteraction(label, result, target) {
  if (
    label.startsWith('a11y-') &&
    (result?.scanned !== true ||
      result?.blockingViolationCount !== 0 ||
      result?.scannedRuleCount < 1)
  ) {
    throw new Error(`accessibility smoke did not produce a valid scan: ${JSON.stringify(result)}`);
  }
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
      result.afterSelect?.persistedLocale !== 'en-US' ||
      !result.afterSelect?.triggerText?.includes('English') ||
      result.afterReload?.persistedLocale !== 'en-US' ||
      !result.afterReload?.triggerText?.includes('English'))
  ) {
    throw new Error(
      `login language persistence did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-home-root-page-state' &&
    (result.authBoxCount !== 1 || result.controls?.length < 2 || !result.buttons?.length)
  ) {
    throw new Error(`root auth page state did not match legacy shape: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-root-page-state' &&
    (result.authSurfaceCount !== 1 ||
      result.controls?.length !== 2 ||
      result.submitActionCount !== 1 ||
      result.forgotActionCount !== 1 ||
      (target === 'source' && result.hash !== '/login') ||
      (target === 'oracle' && !['/', '/login'].includes(result.hash)))
  ) {
    throw new Error(`admin root did not resolve to the login contract: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-register-form-state' &&
    (result.authBoxCount !== 1 ||
      result.controls?.length < 5 ||
      !JSON.stringify(result.controls).includes('INVITE2026') ||
      !result.buttons?.length)
  ) {
    throw new Error(`register form did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-register-legacy-hash-entry' &&
    (result.route !== '/register?code=INVITE2026' ||
      result.historyPath !== '/register?code=INVITE2026' ||
      result.locationHash !== '' ||
      result.authBoxCount !== 1 ||
      !JSON.stringify(result.controls).includes('INVITE2026'))
  ) {
    throw new Error(
      `legacy hash entry did not translate to the history route (§10.3): ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-forget-form-state' &&
    (result.authBoxCount !== 1 || result.controls?.length < 4 || !result.buttons?.length)
  ) {
    throw new Error(`forget form did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-login-form-state' &&
    (result.filled?.authSurfaceCount !== 1 ||
      result.filled?.controls?.length !== 2 ||
      result.filled?.controls?.[0]?.value !== 'admin@example.com' ||
      result.filled?.controls?.[1]?.value !== '12345678' ||
      result.filled?.submitActionCount !== 1 ||
      result.filled?.forgotActionCount !== 1 ||
      result.forgotModal?.modalCount !== 1 ||
      (target === 'source' &&
        !jsonIncludes(result.forgotModal, 'v2board-api reset-admin-password')) ||
      (target === 'oracle' && !jsonIncludes(result.forgotModal, 'reset:password')))
  ) {
    throw new Error(`admin login form did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-system-queue-state' &&
    (!String(result.hash ?? '').includes('/queue') ||
      result.overview?.length < 1 ||
      result.tableHeaders?.length < 4 ||
      result.rows?.length < 1)
  ) {
    throw new Error(`admin queue page did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-dashboard-header-language-dropdown' &&
    (result.dropdownCount !== 1 ||
      result.dropdownHit !== true ||
      result.placement !== 'bottomCenter' ||
      !result.items?.includes('English') ||
      !result.items?.includes('简体中文') ||
      !result.items?.includes('繁體中文'))
  ) {
    throw new Error(
      `dashboard language dropdown did not match legacy placement: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-session-expired-redirect' &&
    (!String(result.hash ?? '').includes('/login') || result.loginBoxCount !== 1)
  ) {
    throw new Error(`session expiry did not redirect like legacy: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-session-expired-redirect' &&
    (result.hash !== '/login' ||
      result.loginSurfaceCount !== 1 ||
      (target === 'source' && result.authData !== null))
  ) {
    throw new Error(`admin session expiry did not clear and redirect: ${JSON.stringify(result)}`);
  }
  if (
    (label === 'user-auth-401-no-redirect' || label === 'admin-auth-401-no-redirect') &&
    (String(result.hash ?? '').includes('/login') ||
      result.loginBoxCount !== 0 ||
      !result.authData ||
      (result.pageContainerCount < 1 && result.routeErrorCount < 1))
  ) {
    throw new Error(
      `HTTP 401 auth state did not match legacy no-redirect behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-dashboard-avatar-dropdown' &&
    // Redesigned as a portaled Radix menu: the oracle's Bootstrap `dropdown-menu-right/-lg`
    // classes and trigger-relative geometry are Tier-2 presentation. Pin only the behavioral
    // contract — the account menu opens (0 -> 1) and exposes a logout action.
    (result.before?.menuCount !== 0 ||
      result.opened?.menuCount !== 1 ||
      !jsonIncludesAny(result.opened?.items, ['Logout', '登出']))
  ) {
    throw new Error(`admin avatar dropdown did not match legacy state: ${JSON.stringify(result)}`);
  }
  if (label.endsWith('dark-mode-persistence') && !isUsefulDarkModePersistenceResult(result)) {
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
      result.qr?.qrCount < 1 ||
      !jsonIncludesAny(result.qr?.qrTipTexts, [
        '使用支持扫码的客户端进行订阅',
        'Use a client app that supports scanning QR code to subscribe',
      ]))
  ) {
    throw new Error(
      `dashboard subscribe drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !result.opened?.items?.some(
        (item) =>
          item.text?.includes('Hiddify') && (item.imageCount ?? 0) + (item.vectorCount ?? 0) >= 1,
      ) ||
      !result.opened?.items?.some(
        (item) =>
          item.text?.includes('Sing-box') && (item.imageCount ?? 0) + (item.vectorCount ?? 0) >= 1,
      ))
  ) {
    throw new Error(
      `dashboard subscribe import links did not expose the required targets: ${JSON.stringify(result)}`,
    );
  }
  if (
    label.startsWith('user-dashboard-subscribe-import-') &&
    label.endsWith('-ua') &&
    !dashboardSubscribeTargetsMatch(result)
  ) {
    throw new Error(
      `dashboard subscribe UA targets did not match legacy state: ${JSON.stringify(result)}`,
    );
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
    throw new Error(
      `dashboard notice carousel did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !result.hash?.includes(`/order/${dashboardResetPackageTradeNo}`))
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
      !result.hash?.includes('/dashboard'))
  ) {
    throw new Error(
      `dashboard new-period confirm did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-dashboard-alert-links' &&
    (result.before?.alertLinks?.length < 2 ||
      !jsonIncludesAny(result.before?.alertLinks, ['立即支付', 'Pay Now']) ||
      !jsonIncludesAny(result.before?.alertLinks, ['立即查看', 'View Now']) ||
      !result.order?.hash?.includes('/order') ||
      result.order?.tableCount < 1 ||
      !result.reset?.hash?.includes('/dashboard') ||
      result.reset?.alertLinks?.length < 2 ||
      !result.ticket?.hash?.includes('/ticket') ||
      result.ticket?.tableCount < 1)
  ) {
    throw new Error(
      `dashboard alert links did not route like legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-profile-deposit-modal' &&
    (result.filled?.amount !== '12.34' ||
      !result.filled?.modalCount ||
      result.filled?.buttons?.length < 2 ||
      result.orderSaveRequests?.length !== 1 ||
      // W4 canonical capture: the deposit arm of the §9.2 create-order union.
      result.orderSaveRequests?.[0]?.kind !== 'deposit' ||
      Number(result.orderSaveRequests?.[0]?.deposit_amount) !== 1234 ||
      !result.hash?.includes(`/order/${profileDepositTradeNo}`))
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
      !jsonIncludesAny(result.confirmed?.toastTexts, ['重置成功', 'Reset successfully']))
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
      !jsonIncludesAny(result.opened?.modalContent, ['Telegram ID', '重新进行绑定', 're-bind']) ||
      result.opened?.buttons?.length < 2 ||
      result.confirmed?.modalCount !== 0 ||
      result.confirmed?.unbindCount < 1 ||
      result.infoFetchDelta < 1 ||
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
    ['user-profile-redeem-giftcard-api-500', 'user-profile-redeem-giftcard-timeout'].includes(label)
  ) {
    if (
      !jsonIncludesAny(result.before?.blockTitles, ['礼品卡', 'Gift Card']) ||
      result.filled?.inputValue !== 'CARD-FAIL' ||
      result.after?.redeemRequests?.length !== 1 ||
      result.after?.redeemRequests?.[0]?.giftcard !== 'CARD-FAIL' ||
      result.after?.inputValue !== 'CARD-FAIL' ||
      result.infoFetchDelta !== 0
    ) {
      throw new Error(
        `profile redeem giftcard failure did not preserve legacy state: ${JSON.stringify(result)}`,
      );
    }
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
      !result.submitButton ||
      result.submitButton.disabled ||
      !JSON.stringify(result.summaryBlocks).includes(couponCheckFixture.name))
  ) {
    throw new Error(
      `plan checkout coupon did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-plan-checkout-coupon-error' &&
    (result.after?.couponInput !== couponErrorCode ||
      result.couponRequests?.length !== 1 ||
      result.couponRequests?.[0]?.code !== couponErrorCode ||
      String(result.couponRequests?.[0]?.plan_id) !== '1' ||
      JSON.stringify(result.after?.summaryBlocks).includes(couponCheckFixture.name) ||
      stableJson(result.before?.summaryBlocks) !== stableJson(result.after?.summaryBlocks))
  ) {
    throw new Error(
      `plan checkout coupon error did not preserve legacy state: ${JSON.stringify(result)}`,
    );
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
    throw new Error(
      `order payment method did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-qr-checkout' &&
    (result.before?.activeIndex !== 0 ||
      result.loading?.submitButton?.disabled !== true ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method_id) !== 1 ||
      result.opened?.modalCount < 1 ||
      !jsonIncludesAny(result.opened?.modalTexts, ['等待支付中', 'Waiting for payment']))
  ) {
    throw new Error(
      `order QR checkout did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-qr-checkout-failure' &&
    (result.before?.activeIndex !== 0 ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method_id) !== 1 ||
      result.after?.modalCount !== 0 ||
      result.after?.qrSvgCount + result.after?.qrCanvasCount !== 0 ||
      result.after?.submitButton?.disabled !== false ||
      !result.after?.hash?.includes('/order/VISUAL2026110001'))
  ) {
    throw new Error(
      `order QR checkout failure did not preserve legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-checkout-network-failure' &&
    (result.before?.activeIndex !== 0 ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method_id) !== 1 ||
      result.after?.modalCount !== 0 ||
      result.after?.qrSvgCount + result.after?.qrCanvasCount !== 0 ||
      !result.after?.hash?.includes('/order/VISUAL2026110001'))
  ) {
    throw new Error(
      `order network checkout failure did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    [
      'admin-coupons-fetch-timeout',
      'admin-giftcards-fetch-timeout',
      'admin-knowledge-fetch-timeout',
      'admin-notices-fetch-timeout',
      'user-orders-fetch-api-500',
      'user-node-fetch-api-500',
      'admin-orders-fetch-api-500',
      'admin-orders-fetch-timeout',
      'admin-payments-fetch-timeout',
      'admin-plans-fetch-timeout',
      'admin-server-manage-fetch-timeout',
      'admin-tickets-fetch-timeout',
      'admin-users-fetch-api-500',
      'admin-users-fetch-timeout',
      'user-knowledge-fetch-timeout',
      'user-node-fetch-timeout',
      'user-orders-fetch-timeout',
      'user-plans-fetch-timeout',
      'user-tickets-fetch-timeout',
      'user-traffic-fetch-timeout',
    ].includes(label)
  ) {
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
    if (!result.requestSeen?.[expectedCountKey] || visibleFallbackCount < 1) {
      throw new Error(`fetch API 500 state was not observable: ${JSON.stringify(result)}`);
    }
  }
  if (
    label === 'user-order-stripe-disabled-checkout' &&
    (result.before?.activeIndex !== 0 ||
      result.selected?.activeIndex !== 1 ||
      !hasExpectedStripePreparationContract(result, target) ||
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
    label === 'user-order-stripe-payment-intent-checkout' &&
    (result.before?.activeIndex !== 0 ||
      result.selected?.activeIndex !== 1 ||
      !hasExpectedStripePreparationContract(result, target) ||
      result.selected?.submitButton?.disabled !== false ||
      !hasExpectedStripeCheckoutContract(result, target, 'tok_visual_parity_success'))
  ) {
    throw new Error(
      `order Stripe PaymentIntent checkout did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-stripe-confirmation-failure' &&
    (result.before?.activeIndex !== 0 ||
      result.selected?.activeIndex !== 1 ||
      !hasExpectedStripePreparationContract(result, target) ||
      result.selected?.submitButton?.disabled !== false ||
      !hasExpectedStripeCheckoutContract(result, target, 'tok_visual_parity_failure') ||
      result.after?.modalCount !== 0 ||
      result.after?.qrSvgCount + result.after?.qrCanvasCount !== 0 ||
      result.after?.submitButton?.disabled !== false ||
      !result.after?.hash?.includes('/order/VISUAL2026110001'))
  ) {
    throw new Error(
      `order Stripe confirmation failure did not preserve checkout state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-redirect-checkout' &&
    (result.selected?.activeIndex !== 2 ||
      result.checkoutRequests?.length !== 1 ||
      result.checkoutRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
      Number(result.checkoutRequests?.[0]?.method_id) !== 3 ||
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
      !['both', 'left'].includes(result.before?.scrollPosition) ||
      (result.before?.maxScroll > 0 &&
        (result.afterRight?.scrollLeft <= 0 ||
          !['both', 'right'].includes(result.afterRight?.scrollPosition) ||
          result.afterMiddle?.scrollLeft <= 0 ||
          result.afterMiddle?.scrollPosition !== 'middle')))
  ) {
    throw new Error(
      `node table scroll did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      result.viewportWidth >= 600 ? (label === 'admin-order-status-tooltips' ? 2 : 1) : 0;
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
      !['both', 'left'].includes(result.before?.scrollPosition) ||
      (result.before?.maxScroll > 0 &&
        (result.afterRight?.scrollLeft <= 0 ||
          !['both', 'right'].includes(result.afterRight?.scrollPosition) ||
          result.afterMiddle?.scrollLeft <= 0 ||
          result.afterMiddle?.scrollPosition !== 'middle')))
  ) {
    throw new Error(
      `traffic table scroll did not produce observable state: ${JSON.stringify(result)}`,
    );
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
    (result.before?.articleTitles?.length < 2 ||
      result.opened?.drawerOpenCount !== 1 ||
      !JSON.stringify(result.opened?.drawerTitles).includes('Copy Article') ||
      !JSON.stringify(result.opened?.drawerBodies).includes('Copy article body') ||
      result.closed?.drawerOpenCount !== 0)
  ) {
    throw new Error(`knowledge drawer did not produce observable state: ${JSON.stringify(result)}`);
  }
  if (
    label === 'user-knowledge-extreme-content-matrix' &&
    (!jsonIncludes(result.filtered?.articleTitles, 'Extreme Legacy Knowledge Matrix') ||
      result.opened?.drawerOpenCount !== 1 ||
      !jsonIncludes(result.opened?.drawerTitles, 'Extreme Legacy Knowledge Matrix') ||
      !jsonIncludes(result.opened?.drawerBodies, 'extreme-knowledge-token-2026') ||
      !jsonIncludes(result.opened?.drawerBodies, 'very-long-hostname'))
  ) {
    throw new Error(
      `knowledge extreme content matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-invite-generate' &&
    (!JSON.stringify(result.before?.tableRows).includes('INVITE2026') ||
      !JSON.stringify(result.before?.tableRows).includes('WELCOME') ||
      !JSON.stringify(result.after?.toastTexts).includes('已生成') ||
      result.generateRequestDelta !== 1 ||
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
  if (
    label === 'user-invite-transfer-insufficient-balance' &&
    (result.before?.modalCount !== 0 ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.filled?.inputValues).includes('99999.99') ||
      result.transferRequests?.length !== 1 ||
      Number(result.transferRequests?.[0]?.transfer_amount) !== 9999999 ||
      result.after?.modalCount !== 1 ||
      !JSON.stringify(result.after?.inputValues).includes('99999.99') ||
      result.infoFetchDelta !== 0)
  ) {
    throw new Error(
      `invite transfer failure did not preserve legacy state: ${JSON.stringify(result)}`,
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
    label === 'user-invite-finance-submit-matrix' &&
    (result.before?.modalCount !== 0 ||
      result.transferEmptyOpened?.modalCount !== 1 ||
      result.transferRequests?.length !== 1 ||
      result.transferEmptyClosed?.modalCount !== 0 ||
      result.withdrawOpened?.modalCount !== 1 ||
      !jsonIncludes(result.withdrawDropdown?.dropdownItems, 'Alipay') ||
      !jsonIncludes(result.withdrawDropdown?.dropdownItems, 'USDT') ||
      !jsonIncludes(result.withdrawFailureFilled?.inputValues, 'fail-account') ||
      result.withdrawFailed?.modalCount !== 1 ||
      !jsonIncludes(result.withdrawFailed?.inputValues, 'fail-account') ||
      result.withdrawRequests?.length !== 2 ||
      result.withdrawRequests?.[0]?.withdraw_account !== 'fail-account' ||
      result.withdrawRequests?.[1]?.withdraw_account !== 'success-account' ||
      result.withdrawRequests?.[1]?.withdraw_method !== 'USDT' ||
      !String(result.withdrawSucceeded?.hash ?? '').includes('/ticket'))
  ) {
    throw new Error(
      `invite finance submit matrix did not match legacy behavior: ${JSON.stringify(result)}`,
    );
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
      // Tier-2 conscious change on the redesigned ticket surface: the reply
      // mutation invalidates the ticket detail query, so the source refetches
      // the thread once immediately; the frozen legacy oracle still waits on
      // its 5s detail poll and must not refetch inside the runner's window.
      result.ticketFetchDelta !== (target === 'source' ? 1 : 0))
  ) {
    throw new Error(
      `user ticket reply send did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-ticket-error-matrix' &&
    (result.replyFilled?.inputValue !== 'Parity failed reply' ||
      result.replyRequests?.length !== 1 ||
      result.replyRequests?.[0]?.message !== 'Parity failed reply' ||
      result.replyFailed?.inputValue !== 'Parity failed reply' ||
      result.closeRequests?.length !== 1 ||
      String(result.closeRequests?.[0]?.id) !== '7' ||
      !String(result.closeFailed?.hash ?? '').includes('/ticket') ||
      !jsonIncludes(result.closeFailed?.tableRows, 'Need help') ||
      result.closeFetchDelta !== 0)
  ) {
    throw new Error(
      `user ticket error matrix did not preserve legacy state: ${JSON.stringify(result)}`,
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
      result.ticketFetchDelta !== 1 ||
      // W14 (§6.9): the staff mirror must resolve the canonical staff rows in
      // both worlds and carry the same ticket contract as the admin prefix.
      result.staffMirror?.requests?.map((request) => request?.routeId).join(',') !==
        'staff.tickets.list,staff.tickets.get,staff.tickets.replies.create,staff.tickets.close' ||
      String(result.staffMirror?.requests?.[2]?.params?.id) !== '7' ||
      result.staffMirror?.requests?.[2]?.body?.message !== 'Parity staff reply' ||
      result.staffMirror?.responses?.listIds?.join(',') !== '7,8' ||
      result.staffMirror?.responses?.detailId !== 7 ||
      result.staffMirror?.responses?.detailMessageCount !== 1 ||
      result.staffMirror?.responses?.replyOk !== true ||
      result.staffMirror?.responses?.closeOk !== true)
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
    label === 'user-ticket-create-validation-failure' &&
    (result.before?.modalCount !== 0 ||
      result.opened?.modalCount !== 1 ||
      !result.filled?.inputValues?.length ||
      result.filled?.inputValues?.some((value) => value === '') ||
      result.saveRequests?.length !== 1 ||
      result.after?.modalCount !== 1 ||
      result.ticketFetchDelta !== 0)
  ) {
    throw new Error(
      `ticket create server error did not keep the modal open without refetching: ${JSON.stringify(result)}`,
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
      !requestIncludesParamValue(result.filterFetchRequests, 'reply_status', 0) ||
      !JSON.stringify(result.confirmed?.tableReplyStatusTexts).includes('待回复'))
  ) {
    throw new Error(
      `admin ticket reply filter did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'user-order-cancel-confirm' &&
    (result.cancelLinks
      ? result.opened?.modalCount < 1 ||
        !jsonIncludesAny(result.opened?.title, ['注意', 'Attention']) ||
        !jsonIncludesAny(result.opened?.content, ['取消订单', 'cancel the order']) ||
        result.opened?.buttons?.length < 2 ||
        result.confirmed?.modalCount !== 0 ||
        result.orderCancelRequests?.length !== 1 ||
        result.orderCancelRequests?.[0]?.trade_no !== 'VISUAL2026110001' ||
        typeof result.orderFetchDelta !== 'number'
      : result.listItems < 1 || result.modalCount !== 0)
  ) {
    throw new Error(
      `order cancel confirm did not match legacy behavior: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-dashboard-commission-shortcut' &&
    (result.before?.alertLinks?.length < 2 ||
      !result.after?.hash?.includes('/order') ||
      // W11 (§6.4/§7): the fetch query folds to the canonical DSL clause array
      // in both worlds — the commission jump seeds the status/commission_status
      // /commission_balance conditions.
      !JSON.stringify(result.after?.orderFetchQuery).includes('"field"') ||
      !JSON.stringify(result.after?.orderFetchQuery).includes('"op":"gt"') ||
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
  if (label === 'admin-config-unchanged-blur' && result.configSaveDelta !== 0) {
    throw new Error(`unchanged admin config blur unexpectedly saved: ${JSON.stringify(result)}`);
  }
  if (
    label === 'admin-config-save-failure-matrix' &&
    (!jsonIncludes(result.before?.activeTabs, '站点') ||
      !jsonIncludes(result.edited?.inputValues, 'Parity Config Failure') ||
      result.configSaveRequests?.length !== 1 ||
      result.configSaveRequests?.[0]?.app_name !== 'Parity Config Failure' ||
      result.configFetchDelta !== 0 ||
      !jsonIncludes(result.configFailed?.inputValues, 'Parity Config Failure'))
  ) {
    throw new Error(
      `admin config save failure matrix did not preserve the rejected draft: ${JSON.stringify(result)}`,
    );
  }
  if (label === 'admin-audit-filters') {
    // §6.11/§7: each control must mint its canonical clause on the captured
    // GET system/audit-logs query — surface eq, then +method eq, then +the
    // actor_email like clause (raw string per §7.1) — with the page reset to 1.
    const clauses = (query) => JSON.stringify(query?.filter ?? []);
    if (
      (result.initial?.rowCount ?? 0) < 1 ||
      result.initial?.query?.page !== 1 ||
      result.initial?.query?.per_page !== 20 ||
      result.initial?.query?.filter !== undefined ||
      !clauses(result.surfaceFiltered?.query).includes(
        '{"field":"surface","op":"eq","value":"staff"}',
      ) ||
      !clauses(result.methodFiltered?.query).includes(
        '{"field":"method","op":"eq","value":"POST"}',
      ) ||
      result.emailFiltered?.query?.filter?.length !== 3 ||
      !clauses(result.emailFiltered?.query).includes(
        '{"field":"actor_email","op":"like","value":"staff@example.com"}',
      ) ||
      result.emailFiltered?.query?.page !== 1
    ) {
      throw new Error(
        `admin audit filters did not mint the §7 clauses: ${JSON.stringify(result)}`,
      );
    }
  }
  if (
    label === 'admin-plan-legacy-hash-entry' &&
    (result.route !== '/plan' ||
      result.historyPath !== `/${adminPath}/plan` ||
      result.locationHash !== '' ||
      result.planRowCount < 1)
  ) {
    throw new Error(
      `admin legacy hash entry did not translate under the admin basename (§10.3): ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-plan-renew-tooltip' &&
    (result.before?.tooltipCount !== 0 ||
      result.opened?.tooltipCount !== 1 ||
      result.opened?.placement !== 'top' ||
      result.opened?.openTriggerCount < 1 ||
      !jsonIncludesAny(result.opened?.texts, ['续费', 'renew']))
  ) {
    throw new Error(
      `admin plan renew tooltip did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-mutation-failure-matrix' &&
    (!jsonIncludes(result.beforePlan?.tableRows, 'Pro') ||
      result.planUpdateRequests?.length !== 1 ||
      String(result.planUpdateRequests?.[0]?.id) !== '1' ||
      // W11 (§6.2): the show toggle rides as a native boolean (canonical false
      // from the legacy `0` / modern `false` spellings).
      result.planUpdateRequests?.[0]?.show !== false ||
      result.planDropRequests?.length !== 1 ||
      String(result.planDropRequests?.[0]?.id) !== '1' ||
      !jsonIncludes(result.beforeNotice?.tableRows, 'Notice A') ||
      result.noticeShowRequests?.length !== 1 ||
      String(result.noticeShowRequests?.[0]?.id) !== '1' ||
      result.noticeDropRequests?.length !== 1 ||
      String(result.noticeDropRequests?.[0]?.id) !== '1' ||
      !jsonIncludes(result.beforeServerSort?.tableRows, 'Tokyo 01') ||
      !result.serverSortMode?.sortModeActive ||
      result.serverSortRequests?.length !== 1 ||
      result.serverSortFailed?.requestCounts?.serverSort !== 1 ||
      // The oracle never refetches the plan list after a failed mutation; the
      // redesigned source's optimistic toggle invalidates on settlement, so it
      // refetches exactly once. Anything beyond that single settle refetch
      // would mean the failure path re-entered the mutation flow.
      ![0, 1].includes(result.fetchDeltas?.plan))
  ) {
    throw new Error(
      `admin mutation failure matrix did not preserve legacy state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludesAny(result.filled?.actionButtons, ['取 消', '取消']) ||
      !jsonIncludesAny(result.filled?.actionButtons, ['提 交', '提交']) ||
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
      // W11 (§6.2): the modern create body denies `force_update` (no subscribers
      // to force on a brand-new plan); the checkbox state is still verified by
      // `result.filled?.forceUpdate?.checked` above.
      result.planFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin plan create drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  const adminMutationFailureExpectations = {
    'admin-coupon-generate-failure': {
      fetchDeltaKey: 'couponFetchDelta',
      inputText: 'Parity Failed Coupon',
      openKey: 'modalCount',
      requestKey: 'generateRequests',
      requestMatches: (request) =>
        request?.name === 'Parity Failed Coupon' && request?.code === 'FAIL2026',
    },
    'admin-giftcard-generate-failure': {
      fetchDeltaKey: 'giftcardFetchDelta',
      inputText: 'Parity Failed Giftcard',
      openKey: 'modalCount',
      requestKey: 'generateRequests',
      requestMatches: (request) =>
        request?.name === 'Parity Failed Giftcard' && request?.code === 'FAIL-GIFT-2026',
    },
    'admin-knowledge-save-failure': {
      fetchDeltaKey: 'knowledgeFetchDelta',
      inputText: 'Parity Failed Knowledge',
      openKey: 'drawerCount',
      requestKey: 'saveRequests',
      requestMatches: (request) =>
        request?.title === 'Parity Failed Knowledge' &&
        request?.category === 'Parity' &&
        request?.language === 'en-US',
    },
    'admin-notice-save-failure': {
      fetchDeltaKey: 'noticeFetchDelta',
      inputText: 'Parity Failed Notice',
      openKey: 'modalCount',
      requestKey: 'saveRequests',
      requestMatches: (request) => request?.title === 'Parity Failed Notice',
    },
    'admin-payment-save-failure': {
      fetchDeltaKey: 'paymentFetchDelta',
      inputText: 'Parity Failed Pay',
      openKey: 'modalCount',
      requestKey: 'saveRequests',
      requestMatches: (request) =>
        request?.name === 'Parity Failed Pay' &&
        request?.payment === 'AlipayF2F' &&
        request?.config?.key === 'failed-secret' &&
        request?.config?.mch_id === 'failed-merchant',
    },
    'admin-plan-save-failure': {
      fetchDeltaKey: 'planFetchDelta',
      inputText: 'Parity Failed Plan',
      openKey: 'drawerCount',
      requestKey: 'saveRequests',
      requestMatches: (request) =>
        request?.name === 'Parity Failed Plan' &&
        request?.content === '<p>Plan failure body</p>' &&
        String(request?.month_price) === '1234',
    },
    'admin-server-group-save-failure': {
      fetchDeltaKey: 'groupFetchDelta',
      inputText: 'Parity Failed Group',
      openKey: 'modalCount',
      requestKey: 'saveRequests',
      requestMatches: (request) => request?.name === 'Parity Failed Group',
    },
    'admin-server-node-save-failure': {
      fetchDeltaKey: 'nodeFetchDelta',
      inputText: 'Parity Failed VLess',
      openKey: 'drawerCount',
      requestKey: 'saveRequests',
      // W13 (§6.7): the canonical capture carries the protocol as the `type`
      // path param in both worlds (legacy /server/vless/save, modern
      // POST /servers/vless).
      requestMatches: (request) =>
        request?.type === 'vless' &&
        request?.name === 'Parity Failed VLess' &&
        request?.host === 'failed-vless.example.test',
    },
  };
  const mutationFailure = adminMutationFailureExpectations[label];
  if (mutationFailure) {
    const after = result.after ?? {};
    const requests = result[mutationFailure.requestKey] ?? [];
    if (
      after[mutationFailure.openKey] !== 1 ||
      result[mutationFailure.fetchDeltaKey] !== 0 ||
      requests.length !== 1 ||
      !mutationFailure.requestMatches(requests[0]) ||
      !jsonIncludes(after.inputValues, mutationFailure.inputText)
    ) {
      throw new Error(
        `admin mutation failure did not preserve legacy state: ${JSON.stringify(result)}`,
      );
    }
  }
  if (
    label === 'admin-plan-create-group-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['Default'])
  ) {
    throw new Error(
      `admin plan create group select did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-plan-reset-method-matrix' &&
    (!JSON.stringify(result.resetDropdown?.dropdownItems).includes('跟随系统设置') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('每月1号') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('按月重置') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('不重置') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('每年1月1日') ||
      !JSON.stringify(result.resetDropdown?.dropdownItems).includes('按年重置') ||
      !JSON.stringify(result.monthlyFirst?.selectedValues).includes('每月1号') ||
      !JSON.stringify(result.neverReset?.selectedValues).includes('不重置') ||
      !JSON.stringify(result.final?.selectedValues).includes('每月1号') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.name !== 'Parity Reset Matrix' ||
      result.saveRequests?.[0]?.content !== '<p>Reset method matrix</p>' ||
      String(result.saveRequests?.[0]?.month_price) !== '1000' ||
      String(result.saveRequests?.[0]?.reset_price) !== '200' ||
      String(result.saveRequests?.[0]?.transfer_enable) !== '128' ||
      String(result.saveRequests?.[0]?.group_id) !== '1' ||
      String(result.saveRequests?.[0]?.reset_traffic_method) !== '0' ||
      result.planFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin plan reset matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-plan-drawer-keyboard-close' &&
    (result.before?.drawerCount !== 0 ||
      result.opened?.drawerCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('新建订阅') ||
      result.focused?.tag !== 'div' ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin plan drawer keyboard close did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-plan-edit-drawer' &&
    (result.before?.drawerCount !== 0 ||
      !JSON.stringify(result.before?.tableRows).includes('Pro') ||
      result.opened?.drawerCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('编辑订阅') ||
      !JSON.stringify(result.opened?.labels).includes('套餐名称') ||
      !JSON.stringify(result.opened?.labels).includes('套餐描述') ||
      !JSON.stringify(result.opened?.labels).includes('权限组') ||
      !JSON.stringify(result.opened?.labels).includes('流量重置方式') ||
      !JSON.stringify(result.opened?.inputValues).includes('Pro') ||
      !JSON.stringify(result.opened?.inputValues).includes(
        '<p>Fast nodes</p><p>Support ticket</p>',
      ) ||
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
      !jsonIncludesAny(result.edited?.actionButtons, ['取 消', '取消']) ||
      !jsonIncludesAny(result.edited?.actionButtons, ['提 交', '提交']) ||
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
      // W11 (§6.2): the modern edit body keeps `force_update` as a native
      // boolean (the legacy form spelled it `true`).
      result.saveRequests?.[0]?.force_update !== true ||
      result.planFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin plan edit drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      result.saveRequests?.[0]?.config?.publishable_key !== 'pk_parity_create' ||
      result.saveRequests?.[0]?.config?.secret_key !== 'sk_parity_create' ||
      result.paymentFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin payment modal did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      // Footer button labels ('保存'/'取消') are Tier-2 chrome; the antd oracle
      // renders them with a two-CJK-char space ('保 存'/'取 消') and the shadcn
      // Sheet without it, so the raw form can't share a literal. The edit outcome
      // is covered by title/labels/inputValues/saveRequests below.
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Pay') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-secret') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-merchant') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.name !== 'Parity Edited Pay' ||
      result.saveRequests?.[0]?.payment !== 'AlipayF2F' ||
      result.saveRequests?.[0]?.config?.key !== 'edited-secret' ||
      result.saveRequests?.[0]?.config?.mch_id !== 'edited-merchant' ||
      result.paymentFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin payment edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-payment-plugin-field-matrix' &&
    (result.alipay?.modalCount !== 1 ||
      !JSON.stringify(result.alipay?.selectedPayment).includes('AlipayF2F') ||
      !JSON.stringify(result.alipay?.labels).includes('密钥') ||
      !JSON.stringify(result.alipay?.labels).includes('商户ID') ||
      !JSON.stringify(result.mgate?.selectedPayment).includes('MGate') ||
      !JSON.stringify(result.mgate?.labels).includes('Token') ||
      !JSON.stringify(result.mgate?.inputValues).includes('mgate_matrix_token') ||
      !JSON.stringify(result.stripe?.selectedPayment).includes('StripeCheckout') ||
      !JSON.stringify(result.stripe?.labels).includes('Publishable Key') ||
      !JSON.stringify(result.stripe?.labels).includes('Secret Key') ||
      !JSON.stringify(result.stripe?.inputValues).includes('pk_matrix_plugin') ||
      !JSON.stringify(result.stripe?.inputValues).includes('sk_matrix_plugin') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.name !== 'Parity Plugin Matrix' ||
      result.saveRequests?.[0]?.payment !== 'StripeCheckout' ||
      result.saveRequests?.[0]?.config?.publishable_key !== 'pk_matrix_plugin' ||
      result.saveRequests?.[0]?.config?.secret_key !== 'sk_matrix_plugin' ||
      result.paymentFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin payment plugin matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-payment-modal-keyboard-close' &&
    (result.before?.modalCount !== 0 ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('添加支付方式') ||
      result.focused?.tag !== 'div' ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin payment modal keyboard close did not match legacy state: ${JSON.stringify(result)}`,
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
      result.groupDefaultSelected !== true ||
      !jsonIncludesAny(result.groupSelected?.actionButtons, ['取 消', '取消']) ||
      !jsonIncludesAny(result.groupSelected?.actionButtons, ['提 交', '提交']) ||
      result.closed?.openDrawerCount !== 0)
  ) {
    throw new Error(
      `admin server node drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-vless-reality-matrix' &&
    (result.before?.drawerCount !== 0 ||
      result.menuOpened?.dropdownCount !== 1 ||
      !jsonIncludes(result.menuOpened?.dropdownItems, 'VLess') ||
      result.opened?.drawerCount !== 1 ||
      !jsonIncludes(result.opened?.labels, '节点地址') ||
      !jsonIncludes(result.opened?.labels, '安全性') ||
      !jsonIncludes(result.opened?.labels, '传输协议') ||
      !jsonIncludes(result.opened?.labels, 'XTLS流控算法') ||
      !jsonIncludes(result.opened?.selectedValues, 'Default') ||
      !jsonIncludes(result.realityTcp?.selectedValues, 'Reality') ||
      !jsonIncludes(result.realityTcp?.selectedValues, 'TCP') ||
      !jsonIncludes(result.realityTcp?.selectedValues, 'xtls-rprx-vision') ||
      !jsonIncludes(result.realityTcp?.inputValues, 'Parity VLess Reality') ||
      !jsonIncludes(result.realityTcp?.inputValues, 'vless.example.test') ||
      result.saveRequests?.length !== 1 ||
      // W13 (§6.7): the canonical capture carries the protocol as the `type`
      // path param and folds the legacy bracket arrays / numeric strings onto
      // the modern typed JSON body.
      result.saveRequests?.[0]?.type !== 'vless' ||
      result.saveRequests?.[0]?.name !== 'Parity VLess Reality' ||
      String(result.saveRequests?.[0]?.rate) !== '3.5' ||
      result.saveRequests?.[0]?.host !== 'vless.example.test' ||
      String(result.saveRequests?.[0]?.port) !== '443' ||
      String(result.saveRequests?.[0]?.server_port) !== '10443' ||
      String(result.saveRequests?.[0]?.tls) !== '2' ||
      result.saveRequests?.[0]?.network !== 'tcp' ||
      result.saveRequests?.[0]?.flow !== 'xtls-rprx-vision' ||
      String(result.saveRequests?.[0]?.group_id?.[0]) !== '1' ||
      result.nodeFetchDelta < 1)
  ) {
    throw new Error(
      `admin server vless reality matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-protocol-field-matrix' &&
    (!jsonIncludes(result.shadowsocks?.menuOpened?.dropdownItems, 'Shadowsocks') ||
      !jsonIncludes(result.shadowsocks?.opened?.labels, '加密算法') ||
      !jsonIncludes(result.shadowsocks?.httpObfs?.selectedValues, 'HTTP') ||
      !jsonIncludes(result.shadowsocks?.httpObfs?.labels, '混淆') ||
      !jsonIncludes(result.vmess?.opened?.labels, 'TLS') ||
      !jsonIncludes(result.vmess?.grpcTls?.selectedValues, '支持') ||
      !jsonIncludes(result.vmess?.grpcTls?.selectedValues, 'gRPC') ||
      !jsonIncludes(result.trojan?.opened?.labels, '允许不安全') ||
      !jsonIncludes(result.trojan?.webSocket?.selectedValues, '是') ||
      !jsonIncludes(result.trojan?.webSocket?.selectedValues, 'WebSocket') ||
      !jsonIncludes(result.hysteria?.opened?.labels, 'HYSTERIA版本') ||
      !jsonIncludes(result.hysteria?.hysteria2?.selectedValues, 'v2') ||
      !jsonIncludes(result.hysteria?.hysteria2?.selectedValues, 'salamander') ||
      !jsonIncludes(result.tuic?.opened?.labels, '数据包中继模式') ||
      !jsonIncludes(result.tuic?.quic?.selectedValues, 'quic') ||
      !jsonIncludes(result.tuic?.quic?.selectedValues, 'bbr') ||
      // AnyTLS's unique padding editor ('编辑填充方案') renders as a standalone
      // ChildFieldLink button on the redesigned surface (not inside a <Label>), so
      // it is not captured by the label reader; the SNI inputValues check below
      // already proves the AnyTLS drawer opened and its conditional field fills.
      !jsonIncludes(result.anytls?.filled?.inputValues, 'anytls-sni.example.test'))
  ) {
    throw new Error(
      `admin server protocol field matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-v2node-protocol-matrix' &&
    (!jsonIncludes(result.menuOpened?.dropdownItems, 'V2node') ||
      !jsonIncludes(result.opened?.labels, '节点协议') ||
      !jsonIncludes(result.shadowsocks?.selectedValues, 'Shadowsocks') ||
      !jsonIncludes(result.shadowsocks?.selectedValues, 'HTTP伪装') ||
      !jsonIncludes(result.vless?.selectedValues, 'VLess') ||
      !jsonIncludes(result.vless?.selectedValues, 'Reality') ||
      !jsonIncludes(result.vless?.selectedValues, 'WebSocket') ||
      !jsonIncludes(result.vless?.selectedValues, 'MLKEM768X25519PLUS') ||
      !jsonIncludes(result.trojan?.selectedValues, 'Trojan') ||
      !jsonIncludes(result.trojan?.selectedValues, 'TLS') ||
      !jsonIncludes(result.trojan?.selectedValues, 'gRPC') ||
      !jsonIncludes(result.hysteria2?.selectedValues, 'Hysteria2') ||
      !jsonIncludes(result.hysteria2?.selectedValues, 'salamander') ||
      !jsonIncludes(result.tuic?.selectedValues, 'Tuic') ||
      !jsonIncludes(result.tuic?.selectedValues, 'quic') ||
      !jsonIncludes(result.anytls?.selectedValues, 'AnyTLS'))
  ) {
    throw new Error(
      `admin server v2node protocol matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-server-v2node-security-transport-matrix' &&
    (!jsonIncludes(result.menuOpened?.dropdownItems, 'V2node') ||
      !jsonIncludes(result.opened?.labels, '节点协议') ||
      !jsonIncludes(result.vmessNoneXhttp?.selectedValues, 'VMess') ||
      !jsonIncludes(result.vmessNoneXhttp?.selectedValues, '无') ||
      !jsonIncludes(result.vmessNoneXhttp?.selectedValues, 'XHTTP') ||
      !jsonIncludes(result.vmessTlsGrpc?.selectedValues, 'VMess') ||
      !jsonIncludes(result.vmessTlsGrpc?.selectedValues, 'TLS') ||
      !jsonIncludes(result.vmessTlsGrpc?.selectedValues, 'gRPC') ||
      !jsonIncludes(result.vlessTlsHttpUpgrade?.selectedValues, 'VLess') ||
      !jsonIncludes(result.vlessTlsHttpUpgrade?.selectedValues, 'TLS') ||
      !jsonIncludes(result.vlessTlsHttpUpgrade?.selectedValues, 'HTTPUpgrade') ||
      !jsonIncludes(result.vlessTlsHttpUpgrade?.selectedValues, 'MLKEM768X25519PLUS') ||
      !jsonIncludes(result.vlessRealityWebSocket?.selectedValues, 'VLess') ||
      !jsonIncludes(result.vlessRealityWebSocket?.selectedValues, 'Reality') ||
      !jsonIncludes(result.vlessRealityWebSocket?.selectedValues, 'WebSocket') ||
      !jsonIncludes(result.trojanTlsTcp?.selectedValues, 'Trojan') ||
      !jsonIncludes(result.trojanTlsTcp?.selectedValues, 'TLS') ||
      !jsonIncludes(result.trojanTlsTcp?.selectedValues, 'TCP') ||
      !jsonIncludes(result.trojanTlsGrpc?.selectedValues, 'Trojan') ||
      !jsonIncludes(result.trojanTlsGrpc?.selectedValues, 'TLS') ||
      !jsonIncludes(result.trojanTlsGrpc?.selectedValues, 'gRPC'))
  ) {
    throw new Error(
      `admin server v2node security transport matrix did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      // W13 (§6.7): the modern wire carries rate as a JSON number (1), the
      // legacy oracle as the '1.0' decimal string — Tier-2 input formatting.
      !JSON.stringify(result.opened?.inputValues).includes(target === 'oracle' ? '1.0' : '"1"') ||
      !JSON.stringify(result.opened?.inputValues).includes('jp.example.com') ||
      !JSON.stringify(result.opened?.inputValues).includes('443') ||
      !JSON.stringify(result.opened?.inputValues).includes('8388') ||
      result.openedGroupSelected !== true ||
      !jsonIncludes(result.opened?.actionButtons, '取 消') ||
      !jsonIncludes(result.opened?.actionButtons, '提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Node') ||
      !JSON.stringify(result.edited?.inputValues).includes('2.25') ||
      !JSON.stringify(result.edited?.inputValues).includes('edited-node.example.test') ||
      !JSON.stringify(result.edited?.inputValues).includes('9443') ||
      !JSON.stringify(result.edited?.inputValues).includes('18388') ||
      result.closed?.openDrawerCount !== 0)
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
      !JSON.stringify(result.actionDropdown?.dropdownItems).includes('指定DNS服务器进行解析') ||
      !JSON.stringify(result.edited?.labels).includes('DNS服务器') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Created Route') ||
      !JSON.stringify(result.edited?.inputValues).includes('domain:created.example.com') ||
      !JSON.stringify(result.edited?.inputValues).includes('geosite:created') ||
      !JSON.stringify(result.edited?.inputValues).includes('9.9.9.9') ||
      !JSON.stringify(result.edited?.selectedValues).includes('指定DNS服务器进行解析') ||
      result.saveRequests?.length !== 1 ||
      // W13 (§6.7): the canonical capture folds the legacy `match[i]` bracket
      // spelling onto the modern real `match` array.
      !hasExactObjectKeys(result.saveRequests?.[0], [
        'remarks',
        'match',
        'action',
        'action_value',
      ]) ||
      result.saveRequests?.[0]?.remarks !== 'Parity Created Route' ||
      result.saveRequests?.[0]?.match?.[0] !== 'domain:created.example.com' ||
      result.saveRequests?.[0]?.match?.[1] !== 'geosite:created' ||
      result.saveRequests?.[0]?.action !== 'dns' ||
      result.saveRequests?.[0]?.action_value !== '9.9.9.9' ||
      result.routeFetchDelta < 1 ||
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
      !jsonIncludes(result.edited?.buttons, '取 消') ||
      !jsonIncludes(result.edited?.buttons, '提 交') ||
      result.saveRequests?.length !== 1 ||
      // W13 (§6.7): the canonical capture folds the legacy body id onto the
      // modern path identity and the `match[i]` brackets onto the real array;
      // the oracle's legacy edit body still echoes the row timestamps.
      !hasExactObjectKeys(result.saveRequests?.[0], [
        'id',
        'remarks',
        'match',
        'action',
        'action_value',
        ...(target === 'oracle' ? ['created_at', 'updated_at'] : []),
      ]) ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.remarks !== 'Parity Edited Route' ||
      result.saveRequests?.[0]?.match?.[0] !== 'domain:edited.example.com' ||
      result.saveRequests?.[0]?.match?.[1] !== 'geosite:openai' ||
      result.saveRequests?.[0]?.action !== 'dns' ||
      result.saveRequests?.[0]?.action_value !== '1.1.1.1' ||
      result.routeFetchDelta < 1 ||
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
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
    throw new Error(
      `admin order detail modal did not produce observable state: ${JSON.stringify(result)}`,
    );
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
    throw new Error(
      `admin order assign modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    // The redesigned status trigger is a `待支付` badge DropdownMenu, not an antd
    // `标记为` `<a>`, and the redesigned table shows the full trade_no where the
    // antd oracle truncates to `VIS...001` — both Tier-2 presentation. The mark-
    // paid contract stays pinned by paidRequest.trade_no below.
    label === 'admin-order-status-dropdown' &&
    (result.before?.dropdownCount !== 0 ||
      result.opened?.dropdownCount !== 1 ||
      !JSON.stringify(result.opened?.dropdownItems).includes('已支付') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('取消') ||
      result.paidRequest?.trade_no !== 'VISUAL2026110001' ||
      result.closed?.dropdownCount !== 0)
  ) {
    throw new Error(
      `admin order status dropdown did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    // Same Tier-2 relaxation as the status dropdown: drop the antd `标记为` label
    // and truncated `VIS...002` cell checks (the `发放中` status label renders in
    // both DOMs and stays pinned); the commission contract stays pinned by
    // updateRequest.trade_no + commission_status below.
    label === 'admin-order-commission-dropdown' &&
    (result.before?.dropdownCount !== 0 ||
      !JSON.stringify(result.before?.orderRows).includes('发放中') ||
      result.opened?.dropdownCount !== 1 ||
      !JSON.stringify(result.opened?.dropdownItems).includes('待确认') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('有效') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('无效') ||
      result.updateRequest?.trade_no !== 'VISUAL2026110002' ||
      String(result.updateRequest?.commission_status) !== '3' ||
      result.closed?.dropdownCount !== 0)
  ) {
    throw new Error(
      `admin order commission dropdown did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    // The redesigned list filters through an inline `order-search` box + `order-
    // page` pagination instead of the antd `过滤器` drawer, and shows the full
    // trade_no where the antd oracle truncates to `ADM...001` — both Tier-2. The
    // filter/pagination CONTRACT stays pinned by filterQuery below; W11 (§6.4/
    // §7/§8) folds both worlds to the canonical DSL clause array plus
    // `page`/`per_page` pagination.
    label === 'admin-orders-filter-pagination-matrix' &&
    (!(result.before?.rowTexts?.length > 0) ||
      result.before?.sorterCount !== 0 ||
      result.filtered?.drawerCount !== 0 ||
      !jsonIncludes(result.filtered?.filterQuery, '"field"') ||
      !jsonIncludes(result.filtered?.filterQuery, 'trade_no') ||
      !jsonIncludes(result.filtered?.filterQuery, 'VISUAL202611') ||
      !jsonIncludes(result.filtered?.activePage, '1') ||
      !jsonIncludes(result.filtered?.pageItems, '2') ||
      !jsonIncludes(result.page2?.activePage, '2') ||
      String(result.page2?.filterQuery?.page) !== '2' ||
      String(result.page2?.filterQuery?.per_page) !== '10' ||
      result.page2?.sorterCount !== 0)
  ) {
    throw new Error(
      `admin orders filter pagination matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
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
    throw new Error(
      `admin coupon modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-coupon-range-picker' &&
    // The redesigned editor exposes the validity window as native datetime-local
    // inputs; the antd oracle opens a range-picker calendar popup. The popup chrome
    // is Tier-2 presentation, so both reduce to whether a validity-window date
    // field is reachable in the editor.
    ((result.before?.dateFieldCount ?? 0) < 1 || (result.opened?.dateFieldCount ?? 0) < 1)
  ) {
    throw new Error(
      `admin coupon range picker did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-coupon-type-matrix' &&
    // The type control's rendered selection (antd `.ant-select-selection-selected-
    // value` vs the Radix trigger label) and the ¥/% value addon are Tier-2
    // presentation; the type/scope selections are proven by the generate payload
    // (type, limit_plan_ids, limit_period — canonical W10 arrays; the legacy
    // bracket params fold onto them).
    (result.amount?.modalCount !== 1 ||
      result.limited?.modalCount !== 1 ||
      result.generateRequests?.length !== 1 ||
      result.generateRequests?.[0]?.name !== 'Parity Ratio Coupon' ||
      result.generateRequests?.[0]?.code !== 'RATIO2026' ||
      String(result.generateRequests?.[0]?.type) !== '2' ||
      String(result.generateRequests?.[0]?.value) !== '15' ||
      String(result.generateRequests?.[0]?.limit_plan_ids) !== '1' ||
      String(result.generateRequests?.[0]?.limit_period) !== 'month_price' ||
      result.couponFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin coupon type matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
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
      result.filled?.modalCount !== 1 ||
      !JSON.stringify(result.filled?.labels).includes('指定订阅') ||
      !JSON.stringify(result.filled?.labels).includes('最大使用次数') ||
      !JSON.stringify(result.filled?.inputValues).includes('0') ||
      !JSON.stringify(result.filled?.inputValues).includes('9') ||
      !jsonIncludes(result.filled?.buttons, '取 消') ||
      !jsonIncludes(result.filled?.buttons, '提 交') ||
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
    throw new Error(
      `admin giftcard modal did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
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
      !jsonIncludes(result.filled?.buttons, '取 消') ||
      !jsonIncludes(result.filled?.buttons, '提 交') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.title !== 'Parity Notice' ||
      result.saveRequests?.[0]?.content !== 'Parity notice body' ||
      String(result.saveRequests?.[0]?.tags) !== 'ops' ||
      result.saveRequests?.[0]?.img_url !== 'https://example.test/notice.png' ||
      result.noticeFetchDelta < 1 ||
      result.closed?.modalCount !== 0 ||
      result.reopened?.modalCount !== 1 ||
      JSON.stringify(result.reopened?.inputValues).includes('Parity Notice') ||
      JSON.stringify(result.reopened?.inputValues).includes('Parity notice body'))
  ) {
    throw new Error(
      `admin notice modal did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '提 交') ||
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Notice') ||
      !JSON.stringify(result.edited?.inputValues).includes('<p>Parity edited notice body</p>') ||
      !JSON.stringify(result.edited?.inputValues).includes(
        'https://example.test/notice-edited.png',
      ) ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '2' ||
      result.saveRequests?.[0]?.title !== 'Parity Edited Notice' ||
      result.saveRequests?.[0]?.content !== '<p>Parity edited notice body</p>' ||
      String(result.saveRequests?.[0]?.tags) !== 'ops,edited' ||
      result.saveRequests?.[0]?.img_url !== 'https://example.test/notice-edited.png' ||
      result.noticeFetchDelta < 1 ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin notice edit modal did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.filled?.actionButtons, '取 消') ||
      !jsonIncludes(result.filled?.actionButtons, '提 交') ||
      result.saveRequests?.length !== 1 ||
      result.saveRequests?.[0]?.title !== 'Parity Knowledge' ||
      result.saveRequests?.[0]?.category !== 'Parity' ||
      result.saveRequests?.[0]?.language !== 'en-US' ||
      !String(result.saveRequests?.[0]?.body).includes('Parity body') ||
      result.knowledgeFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin knowledge drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      !JSON.stringify(result.edited?.inputValues).includes('Parity Edited Article') ||
      !jsonIncludes(result.edited?.actionButtons, '取 消') ||
      !jsonIncludes(result.edited?.actionButtons, '提 交') ||
      result.saveRequests?.length !== 1 ||
      String(result.saveRequests?.[0]?.id) !== '1' ||
      result.saveRequests?.[0]?.title !== 'Parity Edited Article' ||
      result.saveRequests?.[0]?.category !== 'General' ||
      result.saveRequests?.[0]?.language !== 'en-US' ||
      !String(result.saveRequests?.[0]?.body).includes('Edited body') ||
      result.knowledgeFetchDelta < 1 ||
      result.closed?.drawerCount !== 0)
  ) {
    throw new Error(
      `admin knowledge edit drawer did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (label === 'admin-users-filter-input' && result.firstInput !== 'visual@example.com') {
    throw new Error('admin users filter input did not preserve typed value');
  }
  if (
    label === 'admin-users-filter-field-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['邮箱', '到期时间'])
  ) {
    throw new Error(
      `admin users filter field select did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-users-filter-expiry-picker' &&
    (result.before?.dateFieldCount !== 0 || (result.opened?.dateFieldCount ?? 0) < 1)
  ) {
    // Selecting 到期时间 must yield a reachable date affordance in both worlds (the
    // redesigned native datetime-local input, or the antd calendar picker input).
    // The antd calendar POPUP chrome is Tier-2 presentation and no longer pinned.
    throw new Error(
      `admin users filter expiry picker did not match legacy state: ${JSON.stringify(result)}`,
    );
  }

  if (label === 'admin-users-pagination-matrix') {
    const sizeChangerVisible = result.before?.sizeChangerCount === 1;
    const sizeChangerMismatch = sizeChangerVisible
      ? !jsonIncludes(result.before?.pageSizeSelection, '10') ||
        !jsonIncludes(result.sizeDropdown?.dropdownItems, '50 条/页') ||
        !jsonIncludes(result.pageSize50?.activePage, '1') ||
        String(result.pageSize50?.query?.page) !== '1' ||
        String(result.pageSize50?.query?.per_page) !== '50' ||
        !jsonIncludes(result.pageSize50?.pageSizeSelection, '50')
      : result.before?.sizeChangerCount !== 0 ||
        result.page2?.sizeChangerCount !== 0 ||
        result.sizeDropdown?.skipped !== 'not-visible' ||
        result.pageSize50 !== null;
    if (
      !jsonIncludes(result.before?.rowTexts, 'very.long.user.identity.1') ||
      !jsonIncludes(result.before?.pageItems, '2') ||
      !jsonIncludes(result.page2?.activePage, '2') ||
      // W12 (§8): the applied fetch page/per_page fold to the canonical names
      // in both worlds (legacy `current`/`pageSize` → `page`/`per_page`).
      String(result.page2?.query?.page) !== '2' ||
      String(result.page2?.query?.per_page) !== '10' ||
      sizeChangerMismatch
    ) {
      throw new Error(
        `admin users pagination matrix did not match legacy state: ${JSON.stringify(result)}`,
      );
    }
  }
  if (
    label === 'admin-users-sort-matrix' &&
    (!jsonIncludes(result.before?.rowTexts, 'very.long.user.identity.1') ||
      !jsonIncludes(result.before?.tableHeaders, '状态') ||
      // W12 (§7.2): the sort clause folds to the canonical `sort_by`/`sort_dir`
      // (lowercased) in both worlds (legacy `sort`/`sort_type` `ASC`/`DESC`).
      String(result.asc?.query?.sort_by) !== 'banned' ||
      String(result.asc?.query?.sort_dir) !== 'asc' ||
      String(result.asc?.query?.page) !== '1' ||
      String(result.desc?.query?.sort_by) !== 'banned' ||
      String(result.desc?.query?.sort_dir) !== 'desc' ||
      String(result.desc?.query?.page) !== '1')
  ) {
    // The ascending/descending arrow indicator (antd `ant-table-column-sorter-up/
    // down` vs the redesigned lucide ArrowUp/ArrowDown) is Tier-2 presentation; the
    // sort_dir asc→desc query above is the external contract.
    throw new Error(
      `admin users sort matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    (label === 'admin-user-bulk-ban-confirm' || label === 'admin-user-bulk-delete-confirm') &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('过滤器') ||
      !JSON.stringify(result.before?.toolbarButtons).includes('操作') ||
      result.filtered?.drawerCount !== 0 ||
      // W12 (§7): the applied list filter folds to the canonical DSL clause.
      !JSON.stringify(result.filtered?.filterQuery).includes('"field"') ||
      !JSON.stringify(result.filtered?.filterQuery).includes('email') ||
      !JSON.stringify(result.filtered?.filterQuery).includes('visual@example.com') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('批量封禁') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('批量删除') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('提醒') ||
      !JSON.stringify(result.opened?.content).includes(
        label === 'admin-user-bulk-delete-confirm' ? '确定要进行删除吗？' : '确定要进行封禁吗？',
      ) ||
      !jsonIncludesAny(result.opened?.buttons, ['Cancel', '取消']) ||
      !jsonIncludesAny(result.opened?.buttons, ['OK', '确定']) ||
      result.closed?.modalCount !== 0 ||
      JSON.stringify(result.closed?.dropdownItems ?? []) !== '[]')
  ) {
    throw new Error(
      `admin user bulk confirm did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-destructive-failure-matrix' &&
    (!jsonIncludes(result.before?.tableRows, 'visual-user@example.com') ||
      !jsonIncludes(result.deleteDropdown?.dropdownItems, '删除用户') ||
      result.deleteOpened?.modalCount !== 1 ||
      !jsonIncludes(result.deleteOpened?.titles, '删除用户') ||
      result.deleteRequests?.length !== 1 ||
      String(result.deleteRequests?.[0]?.id) !== '1' ||
      result.deleteFailed?.modalCount !== 0 ||
      !jsonIncludes(result.filtered?.filterQuery, 'visual@example.com') ||
      !jsonIncludes(result.banDropdown?.dropdownItems, '批量封禁') ||
      result.banOpened?.modalCount !== 1 ||
      !jsonIncludes(result.banOpened?.content, '确定要进行封禁吗？') ||
      result.banRequests?.length !== 1 ||
      !jsonIncludes(result.banRequests?.[0], 'visual@example.com') ||
      result.banFailed?.modalCount !== 0 ||
      !jsonIncludes(result.allDeleteDropdown?.dropdownItems, '批量删除') ||
      result.allDeleteOpened?.modalCount !== 1 ||
      !jsonIncludes(result.allDeleteOpened?.content, '确定要进行删除吗？') ||
      result.allDeleteRequests?.length !== 1 ||
      !jsonIncludes(result.allDeleteRequests?.[0], 'visual@example.com') ||
      result.allDeleteFailed?.modalCount !== 0 ||
      result.initialFetchDelta < 1 ||
      result.mutationFetchDelta !== 0)
  ) {
    throw new Error(
      `admin user destructive failure matrix did not preserve legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-export-download-matrix' &&
    (!jsonIncludes(result.before?.toolbarButtons, '操作') ||
      !jsonIncludes(result.filtered?.filterQuery, 'visual@example.com') ||
      !jsonIncludes(result.dropdown?.dropdownItems, '导出CSV') ||
      result.dumpCsvRequests?.length !== 1 ||
      !jsonIncludes(result.dumpCsvRequests?.[0], 'visual@example.com') ||
      result.downloaded?.requestCount !== 1 ||
      result.downloaded?.probe?.downloads?.length !== 1 ||
      !String(result.downloaded?.probe?.downloads?.[0]?.download ?? '').endsWith('.csv') ||
      result.downloaded?.probe?.objectUrls?.length !== 1 ||
      result.downloaded?.probe?.revokedUrls?.length !== 1)
  ) {
    throw new Error(
      `admin user export download matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '生成') ||
      !JSON.stringify(result.planDropdown?.dropdownItems).includes('无') ||
      !JSON.stringify(result.planDropdown?.dropdownItems).includes('Pro') ||
      !JSON.stringify(result.filled?.inputValues).includes('parity.created') ||
      !JSON.stringify(result.filled?.inputValues).includes('example.com') ||
      !JSON.stringify(result.filled?.inputValues).includes('secret123') ||
      !JSON.stringify(result.filled?.selectedValues).includes('Pro') ||
      result.generateRequests?.length !== 1 ||
      result.generateRequests?.[0]?.email_prefix !== 'parity.created' ||
      result.generateRequests?.[0]?.email_suffix !== 'example.com' ||
      result.generateRequests?.[0]?.password !== 'secret123' ||
      String(result.generateRequests?.[0]?.plan_id ?? '') !== '1' ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin user create modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-create-plan-select-dropdown' &&
    !legacySelectDropdownHasOpened(result, ['无', 'Pro'])
  ) {
    throw new Error(
      `admin user create plan select did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-create-expiry-picker' &&
    // The 到期时间 field is a native date input on the redesigned dialog and an antd
    // calendar-picker input on the oracle; the calendar popup chrome (placement
    // class, footer 今天, month/year headers) is Tier-2 presentation. The migrated
    // contract is simply that a date field is reachable in the create dialog.
    ((result.before?.dateFieldCount ?? 0) < 1 || (result.opened?.dateFieldCount ?? 0) < 1)
  ) {
    throw new Error(
      `admin user create expiry picker did not match legacy state: ${JSON.stringify(result)}`,
    );
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
      !jsonIncludes(result.opened?.buttons, '取 消') ||
      !jsonIncludes(result.opened?.buttons, '确 定') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity Mail Subject') ||
      !JSON.stringify(result.filled?.inputValues).includes('Parity mail body') ||
      !JSON.stringify(result.filled?.inputValues).includes('Line two') ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin user send mail modal did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-send-mail-submit-matrix' &&
    (!jsonIncludes(result.before?.tableRows, 'visual-user@example.com') ||
      !jsonIncludes(result.successDropdown?.dropdownItems, '发送邮件') ||
      !jsonIncludes(result.successFilled?.inputValues, 'Parity Mail Submit Success') ||
      !jsonIncludes(result.successFilled?.inputValues, 'Queued success body') ||
      result.successClosed?.modalCount !== 0 ||
      !jsonIncludes(result.failureDropdown?.dropdownItems, '发送邮件') ||
      !jsonIncludes(result.failureFilled?.inputValues, 'Parity Mail Failure') ||
      !jsonIncludes(result.failureFilled?.inputValues, 'Queued failure body') ||
      result.failureKept?.modalCount !== 1 ||
      !jsonIncludes(result.failureKept?.inputValues, 'Parity Mail Failure') ||
      result.sendMailRequests?.length !== 2 ||
      result.sendMailRequests?.[0]?.subject !== 'Parity Mail Submit Success' ||
      result.sendMailRequests?.[0]?.content !== 'Queued success body' ||
      result.sendMailRequests?.[1]?.subject !== 'Parity Mail Failure' ||
      result.sendMailRequests?.[1]?.content !== 'Queued failure body')
  ) {
    throw new Error(
      `admin user send mail submit matrix did not preserve legacy state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-reset-secret-confirm' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('重置UUID及订阅URL') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('重置安全信息') ||
      !JSON.stringify(result.opened?.content).includes(
        '确定要重置visual-user@example.com的安全信息吗？',
      ) ||
      (!JSON.stringify(result.opened?.buttons).includes('取消') &&
        !jsonIncludes(result.opened?.buttons, '取 消')) ||
      (!JSON.stringify(result.opened?.buttons).includes('确定') &&
        !jsonIncludes(result.opened?.buttons, '确 定')) ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin user reset-secret confirm did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-delete-confirm' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('删除用户') ||
      result.opened?.modalCount !== 1 ||
      !JSON.stringify(result.opened?.titles).includes('删除用户') ||
      !JSON.stringify(result.opened?.content).includes(
        '确定要删除visual-user@example.com的用户信息吗？',
      ) ||
      (!JSON.stringify(result.opened?.buttons).includes('取消') &&
        !jsonIncludes(result.opened?.buttons, '取 消')) ||
      (!JSON.stringify(result.opened?.buttons).includes('确定') &&
        !jsonIncludes(result.opened?.buttons, '确 定')) ||
      result.closed?.modalCount !== 0)
  ) {
    throw new Error(
      `admin user delete confirm did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-copy-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.dropdown?.dropdownItems).includes('复制订阅URL') ||
      // The redesigned surface copies the subscribe URL silently through
      // `navigator.clipboard`; the antd oracle copies via `execCommand` and shows
      // a `复制成功` toast. Accept either observable — the copied URL captured by
      // the clipboard probe, or the success toast.
      !(
        (result.copied?.clipboardWrites ?? []).some((text) =>
          String(text).includes('subscribe?token=visual-user'),
        ) || JSON.stringify(result.copied?.messageTexts).includes('复制成功')
      ) ||
      result.copied?.modalCount !== 0)
  ) {
    throw new Error(
      `admin user copy action did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      // Identity check: the drawer loaded the clicked row's user. The balance/
      // commission/traffic display formatting (redesigned `type=number` inputs
      // drop trailing zeros — 123.4 vs the oracle text input's 123.40) and the
      // plan Select's rendered label are Tier-2 presentation.
      !JSON.stringify(result.drawer?.drawerInputValues).includes('visual-user@example.com') ||
      !jsonIncludes(result.drawer?.actionButtons, '取 消') ||
      !jsonIncludes(result.drawer?.actionButtons, '提 交'))
  ) {
    throw new Error(
      `admin user edit action did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-update-validation-failure' &&
    (!jsonIncludes(result.before?.tableRows, 'visual-user@example.com') ||
      !jsonIncludes(result.dropdown?.dropdownItems, '编辑') ||
      result.edited?.drawerCount !== 1 ||
      !jsonIncludes(result.edited?.drawerInputValues, 'invalid-email') ||
      (result.updateRequests?.length !== 0 &&
        (result.updateRequests?.length !== 1 ||
          String(result.updateRequests?.[0]?.id) !== '1' ||
          result.updateRequests?.[0]?.email !== 'invalid-email')) ||
      result.failed?.drawerCount !== 1 ||
      !jsonIncludes(result.failed?.drawerInputValues, 'invalid-email') ||
      result.userFetchDelta !== 0)
  ) {
    throw new Error(
      `admin user update validation failure did not preserve legacy state: ${JSON.stringify(result)}`,
    );
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
    throw new Error(
      `admin user assign action did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-orders-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('TA的订单') ||
      !String(result.navigated?.hash).includes('/order') ||
      // W11 (§6.4/§7): the seeded user filter folds to the canonical DSL clause.
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('user_id') ||
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('"op":"eq"') ||
      !JSON.stringify(result.navigated?.orderFetchQuery).includes('1'))
  ) {
    throw new Error(
      `admin user orders action did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-user-invite-action' &&
    (!JSON.stringify(result.before?.tableRows).includes('visual-user@example.com') ||
      !JSON.stringify(result.before?.triggerTexts).includes('操作') ||
      !JSON.stringify(result.opened?.dropdownItems).includes('TA的邀请') ||
      !String(result.filtered?.hash).includes('/user') ||
      // W12 (§7): the seeded inviter filter folds to the canonical DSL clause.
      !JSON.stringify(result.filtered?.userFetchQuery).includes('invite_user_id') ||
      !JSON.stringify(result.filtered?.userFetchQuery).includes('"op":"eq"') ||
      !JSON.stringify(result.filtered?.userFetchQuery).includes('1'))
  ) {
    throw new Error(
      `admin user invite action did not produce observable state: ${JSON.stringify(result)}`,
    );
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
      // W14 (§6.8): the canonical capture folds both worlds onto per_page.
      !JSON.stringify(result.modal?.trafficQuery).includes('per_page'))
  ) {
    throw new Error(
      `admin user traffic action did not produce observable state: ${JSON.stringify(result)}`,
    );
  }
  if (
    label === 'admin-users-extreme-viewport-matrix' &&
    // Antd fixed-column duplicates, horizontal-overflow observability, and the
    // `检索` drawer button are Tier-2 presentation the redesign expresses
    // differently (no fixed columns; a `确定`/`重置` filter Sheet). Keep the
    // cross-world essence: a narrow viewport still renders a scrollable table
    // with the toolbar + 邮箱 header, and the filter drawer opens titled 过滤器.
    (result.before?.layout?.viewportWidth < 600 ||
      result.narrowed?.layout?.viewportWidth !== 320 ||
      !result.narrowed?.layout?.tableBodyPresent ||
      !jsonIncludes(result.narrowed?.toolbarButtons, '过滤器') ||
      !jsonIncludes(result.narrowed?.toolbarButtons, '操作') ||
      !jsonIncludes(result.narrowed?.tableHeaders, '邮箱') ||
      result.filterDrawer?.layout?.drawerOpen !== true ||
      !jsonIncludes(result.filterDrawer?.drawerTitles, '过滤器'))
  ) {
    throw new Error(
      `admin users extreme viewport matrix did not match legacy state: ${JSON.stringify(result)}`,
    );
  }
}

function legacySelectDropdownHasOpened(result, expectedItems) {
  return (
    result.before?.dropdownCount === 0 &&
    result.opened?.dropdownCount === 1 &&
    expectedItems.every((item) => JSON.stringify(result.opened?.dropdownItems).includes(item))
  );
}

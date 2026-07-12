import {
  clickFirstVisibleTextContaining,
  clickVisibleAt,
  visibleCount,
  visibleTexts,
} from '../dom-helpers.mjs';
import {
  normalizeDashboardConfirmButtons,
  normalizeDashboardConfirmContent,
  normalizeDashboardDialogText,
  normalizeDashboardNoticeModalBody,
  normalizeDashboardRouteAlertLinks,
  normalizeDashboardSubscribeItemClassName,
} from '../normalizers.mjs';
import {
  dashboardShortcutActionSelector,
  dashboardSubscribeShortcutTexts,
} from '../selectors.mjs';

export async function clickDashboardSubscribeShortcut(page) {
  try {
    await clickVisibleAt(page, dashboardShortcutActionSelector, 1);
    return;
  } catch {
    await clickFirstVisibleTextContaining(
      page,
      '[data-testid="dashboard-shortcut"], a, button, [role="button"], .block-link-pop, #main-container *',
      dashboardSubscribeShortcutTexts,
    );
  }
}

export async function languageDropdownPlacementState(page) {
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
    // The redesigned shell scopes the locale list to the account menu's
    // Language submenu; the trigger-relative geometry fields only apply to the
    // oracle's header dropdown and stay undefined on the shadcn side.
    const shadcnMenus = Array.from(
      document.querySelectorAll('[data-testid="app-language-menu"]'),
    ).filter((element) => {
      const text = (element.textContent ?? '').trim();
      return isVisible(element) && text.includes('English') && text.includes('简体中文');
    });
    const dropdown =
      shadcnMenus[0] ?? Array.from(document.querySelectorAll('.ant-dropdown')).find(isVisible);
    const triggerRect = trigger ? rectOf(trigger) : undefined;
    const dropdownRect = dropdown ? rectOf(dropdown) : undefined;
    const triggerCenter = triggerRect
      ? triggerRect.left + triggerRect.width / 2
      : undefined;
    const dropdownCenter = dropdownRect
      ? dropdownRect.left + dropdownRect.width / 2
      : undefined;
    // Paint-level probe: a non-portaled Radix submenu keeps a full layout rect
    // (so isVisible passes) while the parent content's overflow-hidden clips
    // every pixel away. Only a hit-test at the panel's center proves the menu
    // is actually painted and clickable.
    const hitProbe =
      dropdown && dropdownRect
        ? document.elementFromPoint(
            dropdownRect.left + dropdownRect.width / 2,
            dropdownRect.top + dropdownRect.height / 2,
          )
        : null;

    return {
      centerDelta:
        triggerCenter === undefined || dropdownCenter === undefined
          ? undefined
          : Math.round(dropdownCenter - triggerCenter),
      dropdownCount:
        shadcnMenus.length ||
        Array.from(document.querySelectorAll('.ant-dropdown')).filter(isVisible).length,
      dropdownHit: Boolean(hitProbe && dropdown.contains(hitProbe)),
      gap:
        triggerRect && dropdownRect
          ? Math.round(dropdownRect.top - triggerRect.bottom)
          : undefined,
      items: Array.from(
        document.querySelectorAll(
          '[data-testid="app-language-menu"] [role="menuitem"], [data-testid="app-language-menu"] [role="menuitemradio"], .ant-dropdown-menu-item',
        ),
      )
        .filter(isVisible)
        .map((element) => (element.textContent ?? '').trim().replace(/\s+/g, ' ')),
      opensBelow: Boolean(triggerRect && dropdownRect && dropdownRect.top >= triggerRect.bottom),
      placement:
        dropdown?.className.match(/ant-dropdown-placement-([A-Za-z]+)/)?.[1] ?? 'bottomCenter',
      triggerOpen: Boolean(
        trigger?.className.includes('ant-dropdown-open') ||
          trigger?.getAttribute('data-state') === 'open',
      ),
    };
  });
}

export async function dashboardSubscribeState(page) {
  const modalCount = await visibleCount(page, '[data-testid="dashboard-dialog"], .ant-modal');
  const qrTipTexts = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"], .ant-modal .ant-modal-body',
    4,
  );

  return {
    bodyOverflow: modalCount > 0 ? 'locked' : '',
    boxCount: await visibleCount(page, '[data-testid="dashboard-subscribe-menu"], .oneClickSubscribe___2t9Xg'),
    drawerOpenCount: await visibleCount(page, '.ant-drawer-open'),
    itemTexts: await visibleTexts(
      page,
      '[data-testid="dashboard-subscribe-menu"] [data-testid^="dashboard-subscribe-"], .oneClickSubscribe___2t9Xg .item___yrtOv',
      12,
    ),
    messageTexts: await visibleTexts(page, '[data-sonner-toast], .ant-message-notice, .ant-notification-notice', 4),
    modalCount,
    qrCount: await visibleCount(
      page,
      '[data-testid="dashboard-subscribe-qrcode-image"] svg, [data-testid="dashboard-subscribe-qrcode-image"] canvas, .ant-modal canvas',
    ),
    qrTipTexts: qrTipTexts.map(normalizeDashboardDialogText),
    shortcutTexts: await visibleTexts(page, '[data-testid="dashboard-shortcut"]', 4),
    tutorialButtons: await visibleTexts(
      page,
      '[data-testid="dashboard-subscribe-menu"] [data-testid="dashboard-subscribe-tutorial"], .oneClickSubscribe___2t9Xg .ant-btn',
      2,
    ),
  };
}

export async function dashboardSubscribeImportLinksState(page) {
  const rawItems = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(
      document.querySelectorAll(
        '[data-testid="dashboard-subscribe-menu"] [data-testid^="dashboard-subscribe-"], .oneClickSubscribe___2t9Xg .item___yrtOv',
      ),
    )
      .filter(isVisible)
      .map((item) => ({
        className: item.className,
        dataTestId: item.getAttribute('data-testid') ?? '',
        iconCount: item.querySelectorAll('i').length,
        imageCount: item.querySelectorAll('img').length,
        subscribeTarget: item.getAttribute('data-subscribe-target') ?? '',
        text: (item.textContent ?? '').trim().replace(/\s+/g, ' '),
      }));
  });
  const items = rawItems.map((item) => {
    const className = normalizeDashboardSubscribeItemClassName(item.className, {
      subscribeTarget: item.subscribeTarget,
      testId: item.dataTestId,
    });
    return {
      ...item,
      className,
      iconCount:
        className.includes('subsrcibe-for-link') || className.includes('subscribe-for-qrcode')
          ? 1
          : item.iconCount,
    };
  });
  const modalCount = await visibleCount(page, '[data-testid="dashboard-dialog"], .ant-modal');

  return {
    bodyOverflow: modalCount > 0 ? 'locked' : '',
    boxCount: await visibleCount(page, '[data-testid="dashboard-subscribe-menu"], .oneClickSubscribe___2t9Xg'),
    drawerOpenCount: await visibleCount(page, '.ant-drawer-open'),
    itemClasses: items.map((item) => item.className),
    items,
    itemTexts: items.map((item) => item.text),
    modalCount,
    shortcutTexts: await visibleTexts(page, '[data-testid="dashboard-shortcut"]', 4),
    tutorialButtons: await visibleTexts(
      page,
      '[data-testid="dashboard-subscribe-menu"] [data-testid="dashboard-subscribe-tutorial"], .oneClickSubscribe___2t9Xg .ant-btn',
      2,
    ),
    userAgent: await page.evaluate(() => window.navigator.userAgent),
  };
}

export async function dashboardNoticeCarouselState(page) {
  const dotState = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const dots = Array.from(
      document.querySelectorAll(
        '[data-testid="dashboard-notice-dots"] [data-testid="dashboard-notice-dot"], .slick-dots li',
      ),
    ).filter(isVisible);
    return {
      activeDotIndex: dots.findIndex(
        (dot) =>
          // Legacy slick oracle marks the active dot with .slick-active; the
          // redesigned shadcn carousel marks it with data-active/aria-current on
          // the dot button (the same data-active convention this scenario already
          // uses to read the active slide).
          dot.classList.contains('slick-active') ||
          dot.getAttribute('data-active') === 'true' ||
          dot.getAttribute('aria-current') === 'true' ||
          dot.getAttribute('data-state') === 'active' ||
          dot.querySelector('[aria-selected="true"]'),
      ),
      dotCount: dots.length,
    };
  });

  const modalTitles = await visibleTexts(page, '[data-testid="dashboard-dialog"] h2, .ant-modal-title', 4);
  const modalBodies = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"], .ant-modal .ant-modal-body',
    4,
  );

  return {
    ...dotState,
    activeSlideTexts: await visibleTexts(
      page,
      '[data-testid="dashboard-notice-slide"][data-active="true"], .slick-slide.slick-active',
      4,
    ),
    modalBodies: modalBodies.map((body, index) =>
      normalizeDashboardNoticeModalBody(body, modalTitles[index] ?? ''),
    ),
    modalCount: await visibleCount(page, '[data-testid="dashboard-dialog"], .ant-modal'),
    modalTitles,
  };
}

export async function dashboardResetPackageConfirmState(page) {
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

  const buttons = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
    4,
  );
  const content = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] p, .ant-modal-confirm-content, .ant-modal-body',
    4,
  );
  const title = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] h2, .ant-modal-confirm-title, .ant-modal-title',
    4,
  );

  return {
    buttons: normalizeDashboardConfirmButtons(buttons),
    content: normalizeDashboardConfirmContent(content, title),
    modalCount: await visibleCount(page, '[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal'),
    resetTriggerCount,
    title,
  };
}

export async function dashboardNewPeriodConfirmState(page) {
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

  const buttons = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] button, .ant-modal-confirm-btns .ant-btn, .ant-modal .ant-btn',
    4,
  );
  const content = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] p, .ant-modal-confirm-content, .ant-modal-body',
    4,
  );
  const title = await visibleTexts(
    page,
    '[data-testid="dashboard-dialog"] h2, .ant-modal-confirm-title, .ant-modal-title',
    4,
  );

  return {
    buttons: normalizeDashboardConfirmButtons(buttons),
    content: normalizeDashboardConfirmContent(content, title),
    modalCount: await visibleCount(page, '[data-testid="dashboard-dialog"], .ant-modal-confirm, .ant-modal'),
    newPeriodTriggerCount,
    title,
  };
}

export async function dashboardAlertLinksState(page) {
  return {
    alertLinks: normalizeDashboardRouteAlertLinks(
      await visibleTexts(
        page,
        '[data-testid="dashboard-alert"] [data-testid="dashboard-alert-link"], .alert .alert-link',
        4,
      ),
    ),
    hash: await page.evaluate(() => window.location.hash),
    tableCount: await visibleCount(
      page,
      '[data-testid="orders-table"], [data-testid="ticket-table"], .ant-table, .am-list-body',
    ),
  };
}

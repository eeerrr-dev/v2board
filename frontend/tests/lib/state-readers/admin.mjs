import {
  clickFirstVisible,
  clickFirstVisibleText,
  clickFirstVisibleTextStable,
  clickFirstVisibleTextInViewport,
  clickVisibleAt,
  dispatchFirstVisibleTextClick,
  fillFirstVisible,
  fillVisibleAt,
  fillVisibleInputByLabel,
  firstInputValue,
  openLegacySelectByLabel,
  selectLegacyFormOption,
  visibleClassNames,
  visibleCount,
  visibleInputValues,
  visibleTexts,
  waitForPageProperty,
  waitForVisibleElementsHidden,
  waitForVisibleInputByLabel,
  waitForVisibleText,
} from '../dom-helpers.mjs';
import { normalizeAdminOrderFetchQuery, normalizeDownloadProbe } from '../normalizers.mjs';
import {
  adminActiveConfigTabSelector,
  adminConfigFieldInputSelector,
  adminConfirmButtonsSelector,
  adminConfirmContentSelector,
  adminConfirmDialogSelector,
  adminConfirmModalCountSelector,
  adminConfirmPrimarySelector,
  adminConfirmTitleSelector,
  adminDialogOpenSelector,
  adminDrawerFooterButtonSelector,
  adminDrawerInputGroupControlSelector,
  adminDrawerInputSelector,
  adminDrawerLabelSelector,
  adminDrawerLegendSelector,
  adminDrawerOpenSelector,
  adminDrawerSelectTriggerSelector,
  adminDrawerSelectedValueSelector,
  adminDrawerTitleSelector,
  adminMenuItemSelector,
  adminModalFooterButtonSelector,
  adminNodeAddTriggerSelector,
  adminOrderActivePageSelector,
  adminOrderDetailRowSelector,
  adminOrderMenuSelector,
  adminOrderPageItemSelector,
  adminOrderRowTriggerSelector,
  adminOverlayOpenSelector,
  adminSelectDropdownSelector,
  adminSelectOptionSelector,
  adminSelectTriggerSelector,
  adminSwitchSelector,
  adminTableRowSelector,
  adminUserPageItemSelector,
  adminUserRowActionTriggerSelector,
  adminUserToolbarButtonSelector,
} from '../selectors.mjs';
import { normalizeParityText } from '../text.mjs';

export async function openAdminPlanRowEditor(page, rowText) {
  const usedInline = await page.evaluate((targetRowText) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const rows = Array.from(
      document.querySelectorAll('.ant-table-tbody tr, [data-slot="table-row"]'),
    );
    const row = rows.find(
      (element) =>
        isVisible(element) &&
        (element.textContent ?? '').replace(/\s+/g, ' ').includes(targetRowText),
    );
    if (!row) {
      throw new Error(`No visible admin plan row ${targetRowText}`);
    }
    const inline = Array.from(row.querySelectorAll('[data-testid^="plan-edit-"]')).find(isVisible);
    if (inline) {
      inline.click();
      return true;
    }
    return false;
  }, rowText);
  if (!usedInline) {
    await clickAdminOrderRowAction(page, rowText, '操作');
    await waitForVisibleText(page, '.ant-dropdown-menu-item a', '编辑');
    await clickFirstVisibleTextStable(page, '.ant-dropdown-menu-item a', ['编辑']);
  }
}

// Open a redesigned surface's inline `«prefix»«id»` row editor button (which
// opens its dialog directly), falling back to the antd oracle affordance the
// caller supplies. Mirrors openAdminPlanRowEditor for the server group/route
// modals where the shadcn row exposes an inline edit button, not a dropdown.

export async function openAdminInlineRowEditor(page, rowText, inlinePrefix, antdFallback) {
  const usedInline = await page.evaluate(
    ({ prefix, targetRowText }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const rows = Array.from(
        document.querySelectorAll('.ant-table-tbody tr, [data-slot="table-row"]'),
      );
      const row = rows.find(
        (element) =>
          isVisible(element) &&
          (element.textContent ?? '').replace(/\s+/g, ' ').includes(targetRowText),
      );
      if (!row) {
        throw new Error(`No visible admin row ${targetRowText}`);
      }
      const inline = Array.from(row.querySelectorAll(`[data-testid^="${prefix}"]`)).find(isVisible);
      if (inline) {
        inline.click();
        return true;
      }
      return false;
    },
    { prefix: inlinePrefix, targetRowText: rowText },
  );
  if (!usedInline) {
    await antdFallback();
  }
}

export async function deleteAdminRowWithConfirm(page, rowText, inlinePrefix, antdFallback) {
  const usedInline = await page.evaluate(
    ({ prefix, targetRowText }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const rows = Array.from(
        document.querySelectorAll('.ant-table-tbody tr, [data-slot="table-row"]'),
      );
      const row = rows.find(
        (element) =>
          isVisible(element) &&
          (element.textContent ?? '').replace(/\s+/g, ' ').includes(targetRowText),
      );
      if (!row) {
        throw new Error(`No visible admin row ${targetRowText}`);
      }
      const inline = Array.from(row.querySelectorAll(`[data-testid^="${prefix}"]`)).find(isVisible);
      if (inline) {
        inline.click();
        return true;
      }
      return false;
    },
    { prefix: inlinePrefix, targetRowText: rowText },
  );
  if (usedInline) {
    await page.waitForSelector(adminConfirmDialogSelector, { state: 'visible', timeout: 5_000 });
    await clickFirstVisible(page, adminConfirmPrimarySelector);
  } else {
    await antdFallback();
  }
}

export async function openAdminNodeAddMenu(page) {
  if ((await visibleCount(page, '[data-testid="node-add"]')) > 0) {
    await page.click('[data-testid="node-add"]');
  } else {
    await page.locator('.v2board-table-action .ant-dropdown-trigger').first().hover();
    await page.waitForTimeout(150);
    await clickFirstVisible(page, '.v2board-table-action .ant-dropdown-trigger');
  }
}

// Node-type menus animate in the frozen Ant UI. Clicking a precomputed point
// can hit a neighbouring item after the menu shifts; a Locator follows the
// chosen element and waits for it to become actionable in both Ant and Radix.
export async function clickVisibleAdminNodeType(page, typeLabel) {
  const expected = normalizeParityText(typeLabel);
  const items = page.locator(adminMenuItemSelector);
  const count = await items.count();
  for (let index = 0; index < count; index += 1) {
    const item = items.nth(index);
    if (!(await item.isVisible())) continue;
    if (normalizeParityText(await item.textContent()) !== expected) continue;
    await item.click();
    return;
  }
  throw new Error(`No visible node type menu item ${typeLabel}`);
}

// Open a node row's editor across both worlds. The redesigned row exposes a
// `node-actions-«id»` Radix DropdownMenu trigger (needs a real pointer event)
// whose 编辑 item opens the drawer; the antd oracle uses its fixed-column row
// dropdown.

export async function openAdminNodeRowEditor(page, rowText) {
  const actionsTestId = await page.evaluate((targetRowText) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const rows = Array.from(document.querySelectorAll('[data-slot="table-row"]'));
    const row = rows.find(
      (element) =>
        isVisible(element) &&
        (element.textContent ?? '').replace(/\s+/g, ' ').includes(targetRowText),
    );
    if (!row) return null;
    const trigger = Array.from(row.querySelectorAll('[data-testid^="node-actions-"]')).find(
      isVisible,
    );
    return trigger ? trigger.getAttribute('data-testid') : null;
  }, rowText);
  if (actionsTestId) {
    await page.click(`[data-testid="${actionsTestId}"]`);
    await waitForVisibleText(page, adminMenuItemSelector, '编辑');
    await clickFirstVisibleTextStable(page, adminMenuItemSelector, ['编辑']);
  } else {
    await clickAdminTableRowDropdownAction(page, rowText, '编辑');
  }
}

// Select the Default permission group in the node drawer across both worlds. The
// redesigned drawer renders 权限组 as a checkbox group (node-group-ids); the antd
// oracle renders it as a multi-select dropdown.

export async function selectAdminNodeGroupDefault(page) {
  const usedCheckbox = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const container = document.querySelector('[data-testid="node-group-ids"]');
    if (!container) return false;
    const label = Array.from(container.querySelectorAll('label')).find(
      (element) =>
        isVisible(element) && (element.textContent ?? '').replace(/\s+/g, '').includes('Default'),
    );
    if (!label) return false;
    const box =
      label.querySelector('[role="checkbox"], [data-slot="checkbox"], input[type="checkbox"]') ??
      label;
    box.click();
    return true;
  });
  if (!usedCheckbox) {
    await openLegacySelectByLabel(page, '.ant-drawer-open', '权限组');
    await waitForVisibleText(page, adminSelectOptionSelector, 'Default');
    await clickFirstVisibleTextStable(page, adminSelectOptionSelector, ['Default']);
    await waitForVisibleElementsHidden(page, adminSelectDropdownSelector).catch(() => undefined);
  }
}

// Whether the Default permission group currently reads as selected, across both
// the shadcn checkbox group and the antd select.

export async function adminNodeGroupDefaultSelected(page) {
  return page.evaluate(() => {
    const container = document.querySelector('[data-testid="node-group-ids"]');
    if (container) {
      const label = Array.from(container.querySelectorAll('label')).find((element) =>
        (element.textContent ?? '').replace(/\s+/g, '').includes('Default'),
      );
      const box = label?.querySelector(
        '[role="checkbox"], [data-slot="checkbox"], input[type="checkbox"]',
      );
      if (box) {
        return (
          box.getAttribute('aria-checked') === 'true' ||
          box.getAttribute('data-state') === 'checked' ||
          box.checked === true
        );
      }
    }
    return Array.from(
      document.querySelectorAll(
        '.ant-select-selection__choice__content, .ant-select-selection-selected-value, .ant-select-selection-item',
      ),
    ).some((element) => (element.textContent ?? '').includes('Default'));
  });
}

export async function openAdminServerNodeDrawerForType(page, typeLabel) {
  await openAdminNodeAddMenu(page);
  await waitForVisibleText(page, adminMenuItemSelector, typeLabel);
  const menuOpened = await adminServerNodeDrawerState(page);
  await clickVisibleAdminNodeType(page, typeLabel);
  await page.waitForSelector(adminDrawerOpenSelector, {
    state: 'visible',
    timeout: 5_000,
  });
  await waitForVisibleText(page, adminDrawerTitleSelector, '新建节点');
  await page.mouse.move(1, 1);
  await page.waitForTimeout(150);
  return { menuOpened, opened: await adminServerNodeDrawerState(page) };
}

export async function closeAdminServerNodeDrawer(page) {
  await closeVisibleAdminServerDrawers(page);
  return adminServerNodeDrawerState(page);
}

export async function reloadAdminServerManagePage(page) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(
    (triggerSelector) => {
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
      return (
        (document.body?.innerText ?? '').includes('Tokyo 01') &&
        Array.from(document.querySelectorAll(triggerSelector)).some(isVisible)
      );
    },
    adminNodeAddTriggerSelector,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(150);
}

export async function closeVisibleAdminServerDrawers(page) {
  for (let attempt = 0; attempt < 6; attempt += 1) {
    if ((await visibleCount(page, adminDrawerOpenSelector)) === 0) {
      await page.waitForTimeout(100);
      return;
    }
    const clicked = await page.evaluate((closeSelector) => {
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
      const buttons = Array.from(document.querySelectorAll(closeSelector))
        .filter(isVisible)
        .sort((left, right) => left.getBoundingClientRect().x - right.getBoundingClientRect().x);
      const button = buttons.at(-1);
      if (!(button instanceof HTMLElement)) return false;
      button.click();
      return true;
    }, '.ant-drawer-open .ant-drawer-close, [data-slot="sheet-content"] [data-slot="sheet-close"]');
    if (!clicked) {
      // The redesigned sheet closes on Escape when no explicit close button is
      // exposed.
      await page.keyboard.press('Escape').catch(() => undefined);
      await page.waitForTimeout(250);
      if ((await visibleCount(page, adminDrawerOpenSelector)) === 0) return;
      break;
    }
    await page.waitForTimeout(250);
  }
  const remaining = await visibleCount(page, adminDrawerOpenSelector);
  if (remaining > 0) {
    throw new Error(`Timed out closing admin server drawers; ${remaining} remained visible`);
  }
}

export async function selectAdminOverlayOption(page, triggerIndex, optionText) {
  await page.locator(adminDrawerSelectTriggerSelector).nth(triggerIndex).click();
  // Wait for the option itself to be visible, not the dropdown container: antd
  // keeps every select's dropdown mounted (the just-used one lingers as
  // `.ant-select-dropdown-hidden`), so a container-visibility wait can lock onto
  // a hidden sibling dropdown.
  await waitForVisibleText(page, adminSelectOptionSelector, optionText);
  await page.locator(adminSelectOptionSelector, { hasText: optionText }).first().click();
  await waitForVisibleElementsHidden(page, adminSelectDropdownSelector);
}

export async function openAdminOrderRowTrigger(page, shadcnTestId, legacyFallback) {
  const trigger = page.locator(`[data-testid="${shadcnTestId}"]`).first();
  if ((await trigger.count()) > 0) {
    await trigger.click();
    return;
  }
  await legacyFallback();
}

export async function openAdminCreateOverlay(page, createTestId) {
  const shadcn = page.locator(`[data-testid="${createTestId}"]`).first();
  if ((await shadcn.count()) > 0) {
    await shadcn.click();
  } else {
    await clickFirstVisible(page, '.bg-white .ant-btn');
  }
  await page.waitForSelector(adminOverlayOpenSelector, { state: 'visible', timeout: 5_000 });
}

// Click a redesigned overlay submit button (`${entity}-submit`) or the antd modal/
// drawer footer primary, in either world.

export async function clickAdminEntitySubmit(page, submitTestId) {
  const shadcn = page.locator(`[data-testid="${submitTestId}"]`).first();
  if ((await shadcn.count()) > 0) {
    await shadcn.click();
  } else {
    await clickFirstVisible(
      page,
      '.ant-modal-footer .ant-btn-primary, .ant-drawer-open .v2board-drawer-action .ant-btn-primary',
    );
  }
}

// Click a row's 编辑 control in either world: the redesigned inline
// `${entity}-edit-«id»` Button (collapsed text 编辑) or the antd `操作`-column
// `<a>编辑</a>` link. Synthetic click fires the React/antd handler in both.

export async function clickAdminRowEditControl(page, rowText) {
  await page.evaluate(
    ({ rowSelector, targetRowText }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const norm = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
      const row = Array.from(document.querySelectorAll(rowSelector)).find(
        (element) => isVisible(element) && norm(element.textContent).includes(targetRowText),
      );
      if (!row) throw new Error(`No visible admin row ${targetRowText}`);
      const edit = Array.from(row.querySelectorAll('a, button')).find((element) => {
        const testId = element.getAttribute('data-testid') ?? '';
        return (
          isVisible(element) && (norm(element.textContent) === '编辑' || testId.includes('-edit-'))
        );
      });
      if (!edit) throw new Error(`No visible edit control in row ${targetRowText}`);
      edit.click();
    },
    { rowSelector: adminTableRowSelector, targetRowText: rowText },
  );
}

// Select a coupon scope item (指定订阅 plan / 指定周期 period) in either world: click
// a redesigned CheckboxGroup label (scoped by `${groupTestId}`) or open the antd
// multi-select by its adjacent form label and pick the option.

export async function toggleAdminCouponScopeItem(page, groupTestId, itemText, antdLabel) {
  const group = page.locator(`[data-testid="${groupTestId}"]`).first();
  if ((await group.count()) > 0) {
    await group.getByText(itemText, { exact: true }).first().click();
    await page.waitForTimeout(80);
    return;
  }
  await selectLegacyFormOption(page, '.ant-modal', antdLabel, [itemText], { waitForHidden: false });
  await page
    .locator('.ant-modal-title')
    .click()
    .catch(() => undefined);
}

// Fill a notice-editor field in either world: the redesigned Dialog exposes each
// field by a stable id (`#notice-title`/`#notice-content`/`#notice-img`), while
// the antd oracle's `mode="tags"` select shifts the plain `.ant-input` order, so
// the oracle is still targeted by its exact index.

export async function fillAdminNoticeField(page, shadcnSelector, antdSelector, antdIndex, value) {
  const shadcn = page.locator(shadcnSelector).first();
  if ((await shadcn.count()) > 0) {
    await shadcn.fill(value);
  } else {
    await fillVisibleAt(page, antdSelector, antdIndex, value);
  }
}

// Add a notice tag in either world: type into the redesigned TagInput
// (`#notice-tags`) or the antd tag-select search field, then commit with Enter.

export async function addAdminNoticeTag(page, tag) {
  const shadcn = page.locator('#notice-tags').first();
  if ((await shadcn.count()) > 0) {
    await shadcn.fill(tag);
    await shadcn.press('Enter');
  } else {
    await fillFirstVisible(page, '.ant-modal .ant-select-search__field', tag);
    await page.keyboard.press('Enter');
  }
}

export async function addAdminUserFilterCondition(page) {
  const shadcn = await page.$('[data-testid="user-filter-add"]');
  if (shadcn) {
    await page.click('[data-testid="user-filter-add"]');
    await page.waitForSelector('[data-testid="user-filter-field-0"]', {
      state: 'visible',
      timeout: 5_000,
    });
  } else {
    await clickFirstVisible(page, '.v2board-filter-drawer .ant-btn-primary');
  }
}

// Open the first filter condition's field select for read-only inspection. The
// redesigned Radix trigger needs a real pointer (`page.click`); the antd oracle
// Select opens for read-only inspection with a synthetic click (`clickVisibleAt`),
// which is how the plan-group select-dropdown scenario also drives it.

export async function openAdminUserFilterFieldSelect(page) {
  const shadcn = await page.$('[data-testid="user-filter-field-0"]');
  if (shadcn) {
    await page.click('[data-testid="user-filter-field-0"]');
  } else {
    await clickVisibleAt(page, '.v2board-filter-drawer .ant-select-selection', 0);
  }
}

export async function adminUserFilterDateFieldState(page, testIdPrefix = 'user-filter-value-') {
  return page.evaluate(
    ({ prefix }) => {
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
      // Count a reachable date affordance in either world: the redesigned native
      // date/datetime-local input (matched by testid prefix), or the antd calendar
      // picker input.
      const dateFieldCount = Array.from(
        document.querySelectorAll(`[data-testid^="${prefix}"], .ant-calendar-picker-input`),
      ).filter(
        (element) =>
          isVisible(element) &&
          (element.classList.contains('ant-calendar-picker-input') ||
            element.getAttribute('type') === 'date' ||
            element.getAttribute('type') === 'datetime-local'),
      ).length;
      return { dateFieldCount };
    },
    { prefix: testIdPrefix },
  );
}

export async function clickAdminUserPage(page, pageNumber) {
  const shadcn = await page.$(`[data-testid="user-page"][data-page="${pageNumber}"]`);
  if (shadcn) {
    await page.click(`[data-testid="user-page"][data-page="${pageNumber}"]`);
  } else {
    await clickFirstVisible(page, `.ant-pagination-item-${pageNumber}`);
  }
}

// Open the page-size changer Select in either world.

export async function openAdminUserPageSizeChanger(page) {
  const shadcn = await page.$('[data-testid="user-page-size"]');
  if (shadcn) {
    await page.click('[data-testid="user-page-size"]');
  } else {
    await clickFirstVisible(page, '.ant-pagination-options-size-changer .ant-select-selection');
  }
}

export async function applyAdminUserEmailFilter(page, value = 'visual@example.com') {
  page.__visualParityLastAdminUserFetchQuery = null;
  await openAdminUserFilterSheet(page);
  const shadcnSheet = await page.$('[data-testid="user-filter-sheet"]');
  if (shadcnSheet) {
    // Redesigned filter Sheet: `添加条件` defaults to the 邮箱/模糊 field, so type the
    // email into its value input and apply. Empty rows are dropped on apply, so the
    // resulting filter is filter[0]={key:email, condition:模糊, value}.
    await page.click('[data-testid="user-filter-add"]');
    await page.waitForSelector('[data-testid="user-filter-value-0"]', {
      state: 'visible',
      timeout: 5_000,
    });
    await page.locator('[data-testid="user-filter-value-0"]').fill(value);
    await page.click('[data-testid="user-filter-apply"]');
  } else {
    await dispatchFirstVisibleTextClick(page, '.v2board-filter-drawer .ant-btn', ['添加条件']);
    await waitForVisibleInputByLabel(page, '.v2board-filter-drawer', '欲检索内容');
    await fillVisibleInputByLabel(page, '.v2board-filter-drawer', '欲检索内容', value);
    await page.waitForFunction(
      (targetValue) => {
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
        const group = Array.from(
          document.querySelectorAll('.v2board-filter-drawer .form-group'),
        ).find(
          (element) =>
            isVisible(element) &&
            Array.from(element.querySelectorAll('label')).some((label) =>
              (label.textContent ?? '').includes('欲检索内容'),
            ),
        );
        const input = group
          ? Array.from(group.querySelectorAll('input, textarea')).find(
              (element) =>
                isVisible(element) && !element.className.includes('ant-select-search__field'),
            )
          : null;
        return input && 'value' in input && input.value === targetValue;
      },
      value,
      { timeout: 5_000 },
    );
    await dispatchFirstVisibleTextClick(
      page,
      '.v2board-filter-drawer .v2board-drawer-action .ant-btn',
      ['检索', '检 索'],
    );
  }
  await waitForPageProperty(page, '__visualParityLastAdminUserFetchQuery');
  await waitForVisibleElementsHidden(page, adminDrawerOpenSelector);
}

export async function openAdminUserToolbarDropdown(page, itemText) {
  await page.mouse.move(0, 0);
  await page.waitForTimeout(150);
  const shadcnTrigger = await page.$('[data-testid="user-bulk-actions"]');
  if (shadcnTrigger) {
    // Redesigned bulk-action DropdownMenu opens on a real pointer click.
    await page.click('[data-testid="user-bulk-actions"]');
  } else {
    await page.hover('.v2board-table-action .ant-dropdown-trigger');
  }
  await waitForVisibleText(page, adminMenuItemSelector, itemText);
}

// Open the redesigned filter Sheet (`user-filter-open`) or the antd `过滤器`
// toolbar drawer, then wait for whichever surface mounts.

export async function openAdminUserFilterSheet(page) {
  const shadcnTrigger = await page.$('[data-testid="user-filter-open"]');
  if (shadcnTrigger) {
    await page.click('[data-testid="user-filter-open"]');
  } else {
    await clickFirstVisibleTextInViewport(page, '.v2board-table-action .ant-btn, .ant-btn', [
      '过滤器',
    ]);
  }
  await page.waitForSelector(
    '[data-testid="user-filter-sheet"], .v2board-filter-drawer, .ant-drawer-open',
    { state: 'visible', timeout: 5_000 },
  );
}

// Open the first row's action menu (redesigned `user-actions-«id»` DropdownMenu or
// the antd `操作` table `<a>`) and wait for the requested item.

export async function openAdminUserRowActionMenu(page, itemText) {
  await clickFirstVisibleText(page, adminUserRowActionTriggerSelector, ['操作']);
  await waitForVisibleText(page, adminMenuItemSelector, itemText);
}

// Open the create-user dialog (redesigned `user-create` button or the antd
// toolbar `创建用户` button) and wait for the dialog title in either world.

export async function openAdminUserCreateDialog(page) {
  const shadcn = await page.$('[data-testid="user-create"]');
  if (shadcn) {
    await page.click('[data-testid="user-create"]');
  } else {
    await clickVisibleAt(page, '.v2board-table-action .ant-btn', 2);
  }
  await page.waitForSelector(adminDialogOpenSelector, { state: 'visible', timeout: 5_000 });
  await waitForVisibleText(page, adminDrawerTitleSelector, '创建用户');
}

// Fill an overlay input by redesigned testid, else by antd overlay-input index.

export async function fillAdminOverlayInput(page, testId, legacyIndex, value) {
  const shadcn = await page.$(`[data-testid="${testId}"]`);
  if (shadcn) {
    await page.locator(`[data-testid="${testId}"]`).fill(value);
  } else {
    await fillVisibleAt(page, adminDrawerInputSelector, legacyIndex, value);
  }
}

// Fill an overlay input by redesigned testid, else by an explicit antd selector
// (first visible). Preserves the frozen oracle's exact field targeting where a
// generic overlay-input index would diverge — e.g. giftcard value/limit-use,
// which the oracle targets by placeholder because the redesigned editor inserts a
// plan Select and native datetime inputs that shift the antd `.ant-input` order.

export async function fillAdminOverlayInputBySelector(page, testId, antdSelector, value) {
  const shadcn = page.locator(`[data-testid="${testId}"]`).first();
  if ((await shadcn.count()) > 0) {
    await shadcn.fill(value);
  } else {
    await fillFirstVisible(page, antdSelector, value);
  }
}

// Open an overlay select trigger by redesigned testid (Radix) else by antd
// overlay-select index — real pointer click, which opens both worlds' selects.

export async function openAdminOverlaySelectTrigger(page, triggerTestId, legacyIndex) {
  const shadcn = page.locator(`[data-testid="${triggerTestId}"]`).first();
  if ((await shadcn.count()) > 0) {
    await shadcn.click();
  } else {
    await page.locator(adminDrawerSelectTriggerSelector).nth(legacyIndex).click();
  }
}

// Capture clipboard writes in either world: the redesigned surface calls
// `navigator.clipboard.writeText`; the antd oracle copies through a hidden
// textarea + `document.execCommand('copy')`. Record both so the copied
// subscribe URL is observable regardless of mechanism.

export async function installClipboardProbe(page) {
  await page.evaluate(() => {
    window.__visualParityClipboardWrites = [];
    try {
      if (!navigator.clipboard) {
        Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {} });
      }
      Object.defineProperty(navigator.clipboard, 'writeText', {
        configurable: true,
        value(text) {
          window.__visualParityClipboardWrites.push(String(text ?? ''));
          return Promise.resolve();
        },
      });
    } catch {
      /* clipboard may be read-only in some engines; the execCommand path still applies */
    }
    Object.defineProperty(document, 'execCommand', {
      configurable: true,
      value(command) {
        if (command === 'copy') {
          const active = document.activeElement;
          const selected =
            active && 'value' in active ? active.value : String(window.getSelection() ?? '');
          if (selected) window.__visualParityClipboardWrites.push(String(selected));
        }
        return command === 'copy';
      },
    });
  });
}

export async function fillAdminUserCreatePassword(page, value) {
  const shadcnDialog = await page.$('[data-testid="user-generate-dialog"]');
  if (shadcnDialog) {
    await page
      .locator('[data-testid="user-generate-dialog"] input[placeholder="留空则密码与邮箱相同"]')
      .fill(value);
  } else {
    await fillVisibleAt(page, adminDrawerInputSelector, 3, value);
  }
}

// Submit the create dialog (redesigned `generate-submit` or antd primary footer).

export async function clickAdminUserCreateSubmit(page) {
  const shadcn = await page.$('[data-testid="generate-submit"]');
  if (shadcn) {
    await page.click('[data-testid="generate-submit"]');
  } else {
    await clickVisibleAt(page, '.ant-modal-footer .ant-btn-primary', 0);
  }
}

export async function fillAdminUserSendMailSubject(page, value) {
  const shadcn = await page.$('[data-testid="send-mail-subject"]');
  if (shadcn) {
    await page.locator('[data-testid="send-mail-subject"]').fill(value);
  } else {
    await fillVisibleAt(page, '.ant-modal input:not([disabled])', 0, value);
  }
}

export async function fillAdminUserSendMailContent(page, value) {
  const shadcn = await page.$('[data-testid="send-mail-content"]');
  if (shadcn) {
    await page.locator('[data-testid="send-mail-content"]').fill(value);
  } else {
    await fillVisibleAt(page, '.ant-modal textarea.ant-input', 0, value);
  }
}

export async function clickAdminUserSendMailCancel(page) {
  const shadcn = await page.$('[data-testid="user-send-mail-dialog"]');
  if (shadcn) {
    await clickFirstVisibleText(page, '[data-testid="user-send-mail-dialog"] button', ['取消']);
  } else {
    await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 0);
  }
}

export async function clickAdminUserSendMailSubmit(page) {
  const shadcn = await page.$('[data-testid="send-mail-submit"]');
  if (shadcn) {
    await page.click('[data-testid="send-mail-submit"]');
  } else {
    await clickVisibleAt(page, '.ant-modal-footer .ant-btn', 1);
  }
}

export async function waitForOverlayInputValue(page, value) {
  await page.waitForFunction(
    ({ expected, selector }) =>
      Array.from(document.querySelectorAll(selector)).some(
        (element) => 'value' in element && element.value === expected,
      ),
    { expected: value, selector: adminDrawerInputSelector },
    { timeout: 5_000 },
  );
}

// Submit the user-manage drawer (redesigned `user-manage-submit` or antd primary).

export async function clickAdminUserManageSubmit(page) {
  const shadcn = await page.$('[data-testid="user-manage-submit"]');
  if (shadcn) {
    await page.click('[data-testid="user-manage-submit"]');
  } else {
    await clickFirstVisible(page, '.ant-drawer-open .v2board-drawer-action .ant-btn-primary');
  }
}

export async function adminConfigSaveFailureState(page) {
  return {
    activeTabs: await visibleTexts(page, adminActiveConfigTabSelector, 4),
    blockLoadingCount: await visibleCount(page, '.block-mode-loading'),
    inputValues: await visibleInputValues(page, adminConfigFieldInputSelector),
    saveCount: page.__visualParityAdminConfigSaveCount ?? 0,
    tableRows: await visibleTexts(page, adminTableRowSelector, 4),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      6,
    ),
  };
}

export async function adminDashboardShortcutState(page) {
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
    alertLinks: await visibleTexts(
      page,
      '[role="alert"] a, [role="alert"] button, .alert-danger .alert-link',
      4,
    ),
    hash: await page.evaluate(() => window.location.hash),
    orderFetchQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminOrderFetchQuery),
    orderFilter,
  };
}

export async function adminTablePaginationState(page, queryNamespace) {
  const query =
    queryNamespace === 'user'
      ? page.__visualParityLastAdminUserFetchQuery
      : page.__visualParityLastAdminOrderFetchQuery;
  const pageTestId = queryNamespace === 'user' ? 'user-page' : 'order-page';
  const pageSizeTestId = queryNamespace === 'user' ? 'user-page-size' : 'order-page-size';
  return {
    activePage: await visibleTexts(
      page,
      `.ant-pagination-item-active, [data-testid="${pageTestId}"][aria-current="page"]`,
      2,
    ),
    nextClasses: await visibleClassNames(page, '.ant-pagination-next', 1),
    pageItems: await visibleTexts(page, `.ant-pagination-item, [data-testid="${pageTestId}"]`, 8),
    pageSizeSelection: await visibleTexts(
      page,
      `.ant-pagination-options-size-changer .ant-select-selection-selected-value, [data-testid="${pageSizeTestId}"]`,
      2,
    ),
    query: normalizeAdminOrderFetchQuery(query),
    rowTexts: await visibleTexts(page, adminTableRowSelector, 6),
    sizeChangerCount: await visibleCount(
      page,
      `.ant-pagination-options-size-changer, [data-testid="${pageSizeTestId}"]`,
    ),
  };
}

export async function adminPaymentModalState(page) {
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 6),
    // Payment percentage fees use InputGroupInput in the redesign. The local
    // selector union retains DOM order and avoids changing every admin reader.
    inputValues: await visibleInputValues(
      page,
      `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`,
    ),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 12),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    selectedPayment: await visibleTexts(page, adminDrawerSelectedValueSelector, 2),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 4),
  };
}

export async function adminServerNodeDrawerState(page) {
  const openDrawerCount = await visibleCount(page, adminDrawerOpenSelector);
  const fallbackDrawerCount =
    openDrawerCount > 0
      ? openDrawerCount
      : (await visibleCount(
            page,
            '.ant-drawer .v2board-drawer-action, [data-slot="sheet-footer"]',
          )) > 0
        ? 1
        : 0;
  const rootedSelectedValues = await visibleTexts(page, adminDrawerSelectedValueSelector, 12);
  const selectedValues =
    rootedSelectedValues.length > 0 || fallbackDrawerCount === 0
      ? rootedSelectedValues
      : await visibleTexts(page, adminSelectTriggerSelector, 12);
  return {
    actionButtons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    drawerCount: fallbackDrawerCount,
    dropdownCount: await visibleCount(page, '.ant-dropdown, [data-slot="dropdown-menu-content"]'),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    // Redesigned permission/route groups are semantic fieldsets; the antd
    // oracle renders the same names as labels. Read both without changing the
    // shared product DOM.
    labels: await visibleTexts(
      page,
      `${adminDrawerLabelSelector}, ${adminDrawerLegendSelector}`,
      28,
    ),
    selectDropdownItems: await visibleTexts(page, adminSelectOptionSelector, 10),
    selectedValues,
    tableRows: await visibleTexts(page, adminTableRowSelector, 8),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 4),
  };
}

export async function adminServerRouteModalState(page) {
  return {
    buttons: await visibleTexts(page, adminModalFooterButtonSelector, 4),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 10),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 8),
    modalCount: await visibleCount(page, adminDialogOpenSelector),
    pageButtons: await visibleTexts(page, 'button:not([data-slot="sidebar"] button)', 12),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 4),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminServerGroupModalState(page) {
  return {
    buttons: await visibleTexts(page, adminModalFooterButtonSelector, 4),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 4),
    modalCount: await visibleCount(page, adminDialogOpenSelector),
    pageButtons: await visibleTexts(page, 'button:not([data-slot="sidebar"] button)', 12),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminOrderDetailModalState(page) {
  return {
    bodyRows: await visibleTexts(page, adminOrderDetailRowSelector, 20),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 4),
  };
}

export async function adminOrderAssignModalState(page) {
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(
      page,
      `${adminDrawerLabelSelector}, ${adminDrawerLegendSelector}`,
      8,
    ),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 4),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminOrderStatusDropdownState(page) {
  return {
    dropdownCount: await visibleCount(page, adminOrderMenuSelector),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 6),
    orderRows: await visibleTexts(page, adminTableRowSelector, 4),
    triggerTexts: await visibleTexts(page, adminOrderRowTriggerSelector, 8),
  };
}

export async function adminOrderCommissionDropdownState(page) {
  return {
    dropdownCount: await visibleCount(page, adminOrderMenuSelector),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 8),
    orderRows: await visibleTexts(page, adminTableRowSelector, 4),
    triggerTexts: await visibleTexts(page, adminOrderRowTriggerSelector, 8),
  };
}

export async function adminOrderFilterPaginationState(page) {
  return {
    activePage: await visibleTexts(page, adminOrderActivePageSelector, 2),
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    filterQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminOrderFetchQuery),
    pageItems: await visibleTexts(page, adminOrderPageItemSelector, 8),
    rowTexts: await visibleTexts(page, adminTableRowSelector, 6),
    sorterCount: await visibleCount(page, '.ant-table-column-has-sorters'),
    tableHeaders: await visibleTexts(page, '.ant-table-thead th, [data-slot="table-head"]', 12),
    toolbarButtons: await visibleTexts(
      page,
      '.bg-white .ant-btn, [data-testid="order-status-filter"]',
      6,
    ),
  };
}

export async function filterDrawerDebugState(page) {
  return {
    buttons: await visibleTexts(page, '.v2board-filter-drawer .ant-btn', 8),
    inputs: await visibleInputValues(
      page,
      '.v2board-filter-drawer input, .v2board-filter-drawer textarea',
    ),
    labels: await visibleTexts(page, '.v2board-filter-drawer label', 8),
    notifications: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-notification-notice, .ant-message-notice',
      4,
    ),
  };
}

export async function adminTicketsReplyFilterState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const normalizedText = (element) => (element.textContent ?? '').trim().replace(/\s+/g, ' ');
    const filterDropdowns = Array.from(
      document.querySelectorAll('.ant-table-filter-dropdown, [data-slot="dropdown-menu-content"]'),
    ).filter(isVisible);
    const filterItems = Array.from(
      document.querySelectorAll(
        '.ant-table-filter-dropdown .ant-dropdown-menu-item, [data-slot="dropdown-menu-checkbox-item"]',
      ),
    )
      .filter(isVisible)
      .slice(0, 4)
      .map((element) => ({
        checked:
          Boolean(
            element.querySelector(
              '.ant-checkbox-checked, .ant-checkbox-wrapper-checked, input:checked',
            ),
          ) ||
          // Radix DropdownMenuCheckboxItem marks its checked state on the item.
          element.getAttribute('aria-checked') === 'true' ||
          element.getAttribute('data-state') === 'checked',
        text: normalizedText(element),
      }));

    return {
      dropdownCount: filterDropdowns.length,
      filterItems,
      tableReplyStatusTexts: Array.from(
        document.querySelectorAll('.ant-table-tbody tr, [data-slot="table-row"]'),
      )
        .filter(isVisible)
        .map((row) =>
          Array.from(row.querySelectorAll('td, [data-slot="table-cell"]')).filter(isVisible),
        )
        .filter((cells) => cells.length >= 4)
        .slice(0, 4)
        .map((cells) => normalizedText(cells[3])),
    };
  });
}

export async function adminUserOrdersActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    hash: await page.evaluate(() => window.location.hash),
    orderFetchQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminOrderFetchQuery),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserEditActionState(page) {
  return {
    actionButtons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    drawerInputValues: await visibleInputValues(page, adminDrawerInputSelector),
    drawerLabels: await visibleTexts(page, adminDrawerLabelSelector, 20),
    drawerTitle: await visibleTexts(page, adminDrawerTitleSelector, 2),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 8),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserCreateModalState(page) {
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 8),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(
      page,
      `${adminDrawerLabelSelector}, ${adminDrawerLegendSelector}`,
      8,
    ),
    modalCount: await visibleCount(page, adminDialogOpenSelector),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 4),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 6),
  };
}

export async function adminUserSortState(page) {
  return {
    query: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserFetchQuery),
    rowTexts: await visibleTexts(page, adminTableRowSelector, 6),
    sorterClasses: await visibleClassNames(
      page,
      '.ant-table-column-sorter-up, .ant-table-column-sorter-down',
      8,
    ),
    tableHeaders: await visibleTexts(page, '.ant-table-thead th, [data-slot="table-head"]', 14),
  };
}

export async function adminUserSendMailModalState(page) {
  const modalCount = await visibleCount(page, adminDialogOpenSelector);
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    dropdownItems: modalCount ? [] : await visibleTexts(page, adminMenuItemSelector, 8),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(
      page,
      `${adminDrawerLabelSelector}, ${adminDrawerLegendSelector}`,
      6,
    ),
    modalCount,
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      4,
    ),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 6),
  };
}

export async function adminUserConfirmState(page) {
  const modalCount = await visibleCount(page, adminConfirmModalCountSelector);
  return {
    buttons: await visibleTexts(page, adminConfirmButtonsSelector, 4),
    content: await visibleTexts(page, adminConfirmContentSelector, 4),
    dropdownItems: modalCount ? [] : await visibleTexts(page, adminMenuItemSelector, 10),
    modalCount,
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminConfirmTitleSelector, 2),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserCopyActionState(page) {
  return {
    clipboardWrites: await page.evaluate(() => window.__visualParityClipboardWrites ?? []),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    messageTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      4,
    ),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserBulkActionState(page) {
  const modalCount = await visibleCount(page, adminConfirmModalCountSelector);
  return {
    buttons: await visibleTexts(page, adminConfirmButtonsSelector, 4),
    content: await visibleTexts(page, adminConfirmContentSelector, 4),
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    dropdownItems: modalCount ? [] : await visibleTexts(page, adminMenuItemSelector, 10),
    filterQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserFetchQuery),
    inputValues: await visibleInputValues(page, '.v2board-filter-drawer .ant-input'),
    modalCount,
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminConfirmTitleSelector, 2),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 8),
  };
}

export async function adminUserDestructiveFailureState(page) {
  const modalCount = await visibleCount(page, adminConfirmModalCountSelector);
  return {
    buttons: await visibleTexts(page, adminConfirmButtonsSelector, 4),
    content: await visibleTexts(page, adminConfirmContentSelector, 4),
    deleteCount: page.__visualParityAdminUserDeleteCount ?? 0,
    allDeleteCount: page.__visualParityAdminUserAllDeleteCount ?? 0,
    banCount: page.__visualParityAdminUserBanCount ?? 0,
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    dropdownItems: modalCount ? [] : await visibleTexts(page, adminMenuItemSelector, 10),
    filterQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserFetchQuery),
    modalCount,
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminConfirmTitleSelector, 2),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      6,
    ),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 8),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserExportDownloadState(page) {
  const probe = await page.evaluate(() => ({
    downloads: window.__visualParityDownloads ?? [],
    objectUrls: window.__visualParityObjectUrls ?? [],
    revokedUrls: window.__visualParityRevokedUrls ?? [],
  }));
  return {
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    filterQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserFetchQuery),
    probe: normalizeDownloadProbe(probe),
    requestCount: page.__visualParityAdminUserDumpCsvCount ?? 0,
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      6,
    ),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 8),
  };
}

export async function installDownloadProbe(page) {
  await page.evaluate(() => {
    window.__visualParityDownloads = [];
    window.__visualParityObjectUrls = [];
    window.__visualParityRevokedUrls = [];
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value(blob) {
        const url = `blob:visual-parity-${window.__visualParityObjectUrls.length + 1}`;
        window.__visualParityObjectUrls.push({
          size: typeof blob?.size === 'number' ? blob.size : null,
          type: blob?.type ?? '',
          url,
        });
        return url;
      },
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value(url) {
        window.__visualParityRevokedUrls.push(url);
      },
    });
    const originalAnchorClick = window.HTMLAnchorElement.prototype.click;
    Object.defineProperty(window.HTMLAnchorElement.prototype, 'click', {
      configurable: true,
      value() {
        const download = this.getAttribute('download') || this.download || '';
        const href = this.href || this.getAttribute('href') || '';
        if (!download && !href.startsWith('blob:visual-parity-')) {
          return originalAnchorClick.call(this);
        }
        window.__visualParityDownloads.push({
          download,
          href,
        });
        return undefined;
      },
    });
  });
}

export async function adminUserAssignActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUserInviteActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    hash: await page.evaluate(() => window.location.hash),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
    userFetchQuery: normalizeAdminOrderFetchQuery(
      page.__visualParityLastAdminFilteredUserFetchQuery,
    ),
  };
}

export async function adminUserTrafficActionState(page) {
  return {
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    modalCount: await visibleCount(page, adminDialogOpenSelector),
    modalRows: await visibleTexts(
      page,
      '.ant-modal .ant-table-tbody tr, [data-testid="user-traffic-modal"] [data-slot="table-row"]',
      6,
    ),
    modalTitle: await visibleTexts(page, adminDrawerTitleSelector, 2),
    tableHeaders: await visibleTexts(
      page,
      '.ant-modal .ant-table-thead th, [data-testid="user-traffic-modal"] [data-slot="table-head"]',
      8,
    ),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    trafficQuery: normalizeAdminOrderFetchQuery(page.__visualParityLastAdminUserTrafficQuery),
    triggerTexts: await visibleTexts(page, adminUserRowActionTriggerSelector, 10),
  };
}

export async function adminUsersExtremeViewportState(page) {
  const layout = await page.evaluate(
    ({ drawerSelector }) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const tableBody = Array.from(
        document.querySelectorAll('.ant-table-body, [data-slot="table-scroll"]'),
      ).find(isVisible);
      const drawer = Array.from(document.querySelectorAll(drawerSelector)).find(isVisible);
      return {
        bodyClass: document.body.className,
        drawerOpen: Boolean(drawer),
        fixedRightCount: Array.from(document.querySelectorAll('.ant-table-fixed-right')).filter(
          isVisible,
        ).length,
        hasHorizontalOverflow: tableBody ? tableBody.scrollWidth > tableBody.clientWidth : false,
        shadcnTable: Boolean(document.querySelector('[data-testid="users-table"]')),
        tableBodyPresent: Boolean(tableBody),
        viewportHeight: window.innerHeight,
        viewportWidth: window.innerWidth,
      };
    },
    { drawerSelector: adminDrawerOpenSelector },
  );
  return {
    drawerButtons: await visibleTexts(
      page,
      '.v2board-filter-drawer .v2board-drawer-action .ant-btn, [data-testid="user-filter-sheet"] button',
      4,
    ),
    drawerTitles: await visibleTexts(page, adminDrawerTitleSelector, 2),
    layout,
    pageItems: await visibleTexts(page, adminUserPageItemSelector, 6),
    tableHeaders: await visibleTexts(page, '.ant-table-thead th, [data-slot="table-head"]', 14),
    tableRows: await visibleTexts(page, adminTableRowSelector, 4),
    toolbarButtons: await visibleTexts(page, adminUserToolbarButtonSelector, 8),
  };
}

export async function clickHeaderAvatarTrigger(page) {
  // The redesigned account menu is a Radix DropdownMenu in the sidebar footer:
  // Radix opens on real pointer events, so a synthetic element.click() is a
  // no-op. Use Playwright's page.click for the shadcn trigger; the legacy antd
  // header dropdown still opens fine via a synthetic click.
  const shadcn = await page.evaluate(() => {
    const isVisible = (element) => {
      if (!element) return false;
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden'
      );
    };
    return {
      triggerVisible: isVisible(document.querySelector('[data-testid="admin-avatar-trigger"]')),
      // The redesigned shell always renders a header SidebarTrigger; antd does
      // not. On mobile the sidebar-footer account chip lives inside the collapsed
      // nav sheet, which Radix only mounts when open — so the avatar trigger is
      // absent from the DOM (not merely hidden) until the sheet opens.
      shadcnShell: Boolean(document.querySelector('#page-header [data-sidebar="trigger"]')),
    };
  });
  if (shadcn.triggerVisible) {
    await page.click('[data-testid="admin-avatar-trigger"]');
    return;
  }
  if (shadcn.shadcnShell) {
    // Open the collapsed mobile nav sheet via the header SidebarTrigger (same as
    // the dashboard language menu), then the footer avatar trigger mounts.
    await page.click('#page-header [data-sidebar="trigger"]');
    await page.waitForSelector('[data-testid="admin-avatar-trigger"]', {
      state: 'visible',
      timeout: 5_000,
    });
    await page.click('[data-testid="admin-avatar-trigger"]');
    return;
  }
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

export async function waitForHeaderAvatarDropdown(page) {
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
      return Array.from(
        document.querySelectorAll(
          '[data-testid="admin-avatar-menu"], #page-header .dropdown-menu.show',
        ),
      ).some(isVisible);
    },
    null,
    { timeout: 5_000 },
  );
}

export async function headerAvatarDropdownState(page) {
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
    const trigger =
      Array.from(document.querySelectorAll('[data-testid="admin-avatar-trigger"]')).find(
        isVisible,
      ) ??
      Array.from(document.querySelectorAll('#page-header button')).find(
        (element) => element.querySelector('.fa-user-circle') && isVisible(element),
      );
    const visibleMenus = Array.from(
      document.querySelectorAll(
        '[data-testid="admin-avatar-menu"], #page-header .dropdown-menu.show',
      ),
    ).filter(isVisible);
    const menu = visibleMenus[0];
    const triggerRect = trigger ? rectOf(trigger) : undefined;
    const menuRect = menu ? rectOf(menu) : undefined;

    return {
      items: menu
        ? Array.from(menu.querySelectorAll('[role="menuitem"], .dropdown-item'))
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

export async function adminCouponModalState(page) {
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    inputValues: await visibleInputValues(
      page,
      `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`,
    ),
    // Checkbox groups are correctly named by fieldset/legend in the shadcn
    // editor; the frozen antd oracle expresses the same names as form labels.
    labels: await visibleTexts(
      page,
      `${adminDrawerLabelSelector}, ${adminDrawerLegendSelector}`,
      16,
    ),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    // The redesigned coupon editor renders its type as a Radix Select whose
    // trigger text is the chosen option (`adminDrawerSelectedValueSelector`); the
    // antd oracle folds both single-select values and multi-select choice chips
    // into `selectedValues`. The value's currency/percent addon (antd
    // `.ant-input-group-addon` vs the shadcn inline suffix span) and the
    // plan/period multi-select choices (redesigned as CheckboxGroups) are Tier-2
    // presentation dropped from the compare and relaxed in the raw assertion.
    selectedValues: [
      ...(await visibleTexts(page, adminDrawerSelectedValueSelector, 6)),
      ...(await visibleTexts(page, '.ant-modal .ant-select-selection__choice__content', 8)),
    ],
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminGiftcardModalState(page) {
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 10),
    // Gift-card amount/period values use InputGroupInput in the redesign while
    // the antd oracle exposes ordinary inputs. Keep the union reader-local so
    // both worlds retain the same document-order field sequence.
    inputValues: await visibleInputValues(
      page,
      `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`,
    ),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 14),
    modalCount: await visibleCount(page, adminOverlayOpenSelector),
    // Type + plan are Radix Selects whose trigger text is the chosen option; the
    // value unit addon (¥/天/GB) is a shadcn inline suffix span rather than an
    // antd `.ant-input-group-addon`, so it is dropped and relaxed in the raw check.
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 6),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminNoticeModalState(page) {
  // Union reader across the antd modal oracle and the redesigned notice Dialog.
  // The committed tag chips (antd `.ant-select-selection__choice__content` vs the
  // shadcn TagInput badges) are Tier-2 presentation dropped from the compare; the
  // tag contract is proven by the `tags[i]` save payload.
  return {
    buttons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 8),
    modalCount: await visibleCount(page, adminDialogOpenSelector),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminPlanDrawerState(page) {
  return {
    actionButtons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    actionDropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 10),
    forceUpdate: await page.evaluate((overlaySelector) => {
      const isVisible = (element) => {
        const rect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return rect.width > 0 && rect.height > 0 && style.display !== 'none';
      };
      const roots = Array.from(document.querySelectorAll(overlaySelector));
      for (const root of roots) {
        const wrapper = Array.from(root.querySelectorAll('.ant-checkbox-wrapper')).find(isVisible);
        if (wrapper) {
          return {
            checked: Boolean(
              wrapper.matches('.ant-checkbox-wrapper-checked') ||
              wrapper.querySelector('.ant-checkbox-checked, input:checked'),
            ),
          };
        }
        const box = Array.from(
          root.querySelectorAll('[data-slot="checkbox"], [role="checkbox"]'),
        ).find(isVisible);
        if (box) {
          return {
            checked:
              box.getAttribute('data-state') === 'checked' ||
              box.getAttribute('aria-checked') === 'true',
          };
        }
      }
      return null;
    }, adminOverlayOpenSelector),
    inputValues: await visibleInputValues(
      page,
      `${adminDrawerInputSelector}, ${adminDrawerInputGroupControlSelector}`,
    ),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 24),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 6),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function adminMutationFailureState(page) {
  const switches = await page.evaluate((switchSelector) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll(switchSelector))
      .filter(isVisible)
      .map((element) => ({
        checked: Boolean(
          element.matches('.ant-switch-checked, [aria-checked="true"], [data-state="checked"]'),
        ),
        disabled: Boolean(element.matches(':disabled, .ant-switch-disabled')),
        loading: Boolean(
          element.matches('.ant-switch-loading') ||
          element.querySelector('.ant-switch-loading-icon'),
        ),
      }));
  }, adminSwitchSelector);
  // Whether the node sort toggle currently reads 保存排序 (sort mode on). Captured
  // as a boolean because the redesigned sidebar renders its nav as <button>s that
  // crowd the toolbar toggle past a positional `buttons` cutoff, while the antd
  // oracle nav is <a> links; a direct text scan is stable across both DOMs.
  const sortModeActive = await page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    return Array.from(document.querySelectorAll('button, .ant-btn')).some(
      (element) =>
        isVisible(element) && (element.textContent ?? '').replace(/\s+/g, '').includes('保存排序'),
    );
  });
  return {
    buttons: await visibleTexts(page, 'button, .ant-btn', 12),
    dropdownItems: await visibleTexts(page, adminMenuItemSelector, 10),
    hash: await page.evaluate(() => window.location.hash),
    sortModeActive,
    requestCounts: {
      noticeDrop: page.__visualParityAdminNoticeDropCount ?? 0,
      noticeShow: page.__visualParityAdminNoticeShowCount ?? 0,
      planDrop: page.__visualParityAdminPlanDropCount ?? 0,
      planUpdate: page.__visualParityAdminPlanUpdateCount ?? 0,
      serverSort: page.__visualParityAdminServerSortCount ?? 0,
    },
    switches,
    tableRows: await visibleTexts(page, adminTableRowSelector, 8),
    toastTexts: await visibleTexts(
      page,
      '[data-sonner-toast], .ant-message-notice, .ant-notification-notice',
      6,
    ),
  };
}

export async function adminKnowledgeDrawerState(page) {
  // Union reader across the antd knowledge drawer oracle and the redesigned
  // knowledge Sheet. The live markdown preview (antd `.custom-html-style`) has no
  // redesigned counterpart, so `previewTexts` is dropped; the markdown body is the
  // `knowledge-body` textarea (antd `textarea.section-container.input` fallback).
  return {
    actionButtons: await visibleTexts(page, adminDrawerFooterButtonSelector, 4),
    drawerCount: await visibleCount(page, adminDrawerOpenSelector),
    dropdownItems: await visibleTexts(page, adminSelectOptionSelector, 10),
    inputValues: await visibleInputValues(page, adminDrawerInputSelector),
    labels: await visibleTexts(page, adminDrawerLabelSelector, 8),
    markdownValue: await firstInputValue(
      page,
      '[data-testid="knowledge-body"], .ant-drawer-open textarea.section-container.input',
    ),
    selectedValues: await visibleTexts(page, adminDrawerSelectedValueSelector, 4),
    tableRows: await visibleTexts(page, adminTableRowSelector, 6),
    titles: await visibleTexts(page, adminDrawerTitleSelector, 2),
  };
}

export async function openAdminTicketsReplyFilter(page) {
  if ((await visibleCount(page, '[data-testid="ticket-reply-filter"]')) > 0) {
    await page.click('[data-testid="ticket-reply-filter"]');
  } else {
    await clickFirstVisible(page, '.ant-table-column-has-filters .ant-dropdown-trigger');
  }
}

// Commit/close the reply-status filter across both worlds. The antd filter
// applies and closes on its 确定 confirm link; the redesigned checkbox already
// refetched on toggle, so only close the still-open DropdownMenu with Escape.

export async function confirmAdminTicketsReplyFilter(page) {
  if ((await visibleCount(page, '.ant-table-filter-dropdown-link.confirm')) > 0) {
    await dispatchFirstVisibleTextClick(page, '.ant-table-filter-dropdown-link.confirm', ['确定']);
  } else {
    await page.keyboard.press('Escape');
  }
}

export async function clickAdminTicketsReplyFilterOption(page, text) {
  // Radix DropdownMenuCheckboxItem selects on a real pointer event; the antd
  // filter option toggles its inner checkbox on a synthetic click.
  const shadcnItem = page
    .locator('[data-slot="dropdown-menu-checkbox-item"]', { hasText: text })
    .first();
  if ((await shadcnItem.count()) > 0) {
    await shadcnItem.click();
    return;
  }
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

export async function clickAdminOrderRowAction(page, rowText, actionText) {
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

export async function clickAdminTableRowDropdownAction(page, rowText, actionText) {
  await page.evaluate((targetRowText) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const isInViewport = (element) => {
      if (!isVisible(element)) return false;
      const rect = element.getBoundingClientRect();
      return (
        rect.bottom > 0 &&
        rect.right > 0 &&
        rect.top < window.innerHeight &&
        rect.left < window.innerWidth
      );
    };
    const allRows = Array.from(document.querySelectorAll('.ant-table-tbody tr'));
    const row = allRows.find(
      (element) =>
        isVisible(element) &&
        (element.textContent ?? '').trim().replace(/\s+/g, ' ').includes(targetRowText),
    );
    if (!row) {
      throw new Error(`No visible admin table row ${targetRowText}`);
    }
    const triggerCandidates = [];
    const rowKey = row.getAttribute('data-row-key');
    if (rowKey !== null) {
      for (const fixedRow of document.querySelectorAll(
        '.ant-table-fixed-right .ant-table-tbody tr',
      )) {
        if (fixedRow.getAttribute('data-row-key') === rowKey) {
          triggerCandidates.push(
            ...fixedRow.querySelectorAll('.v2board-table-action .ant-dropdown-trigger'),
          );
          triggerCandidates.push(...fixedRow.querySelectorAll('a'));
        }
      }
    }
    const siblingRows = row.parentElement
      ? Array.from(row.parentElement.children).filter((element) => element.matches('tr'))
      : [];
    const rowIndex = siblingRows.indexOf(row);
    if (rowIndex >= 0) {
      const fixedRow = Array.from(
        document.querySelectorAll('.ant-table-fixed-right .ant-table-tbody tr'),
      )[rowIndex];
      if (fixedRow) {
        triggerCandidates.push(
          ...fixedRow.querySelectorAll('.v2board-table-action .ant-dropdown-trigger'),
        );
        triggerCandidates.push(...fixedRow.querySelectorAll('a'));
      }
    }
    triggerCandidates.push(...row.querySelectorAll('.v2board-table-action .ant-dropdown-trigger'));
    triggerCandidates.push(...row.querySelectorAll('a'));
    const trigger =
      triggerCandidates.find(isInViewport) ??
      triggerCandidates.find((element) => {
        const text = (element.textContent ?? '').trim().replace(/\s+/g, ' ');
        return isVisible(element) && text.includes('操作');
      });
    if (!trigger) {
      throw new Error(`No visible admin table row operation trigger ${targetRowText}`);
    }
    trigger.click();
  }, rowText);
  await waitForVisibleText(page, '.ant-dropdown-menu-item', actionText);
  await clickFirstVisibleTextStable(page, '.ant-dropdown-menu-item a', [actionText]);
}

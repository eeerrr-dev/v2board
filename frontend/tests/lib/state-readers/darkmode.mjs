import { stableJson } from '../json-util.mjs';
import { waitForDarkReader, waitForShadcnDarkMode } from '../page-prep.mjs';

const darkModeStyleTargets = [
  { key: 'html', selector: 'html' },
  { key: 'body', selector: 'body' },
  { key: 'pageContainer', selector: '#page-container' },
  { key: 'pageHeader', selector: '#page-header' },
  { key: 'headerButton', selector: '#page-header button' },
  { key: 'sidebar', selector: '#sidebar' },
  { key: 'sidebarLink', selector: '#sidebar .nav-main-link, #sidebar a, #sidebar button' },
  { key: 'mainContainer', selector: '#main-container' },
  { key: 'content', selector: '.content, [data-testid="dashboard-page"]' },
  { key: 'block', selector: '.block, [data-testid="dashboard-card"]' },
  { key: 'blockHeader', selector: '.block-header, [data-testid="dashboard-card"] [class*="border-b"]' },
  { key: 'blockContent', selector: '.block-content, [data-testid="dashboard-card"] [class*="pt-6"]' },
  {
    key: 'primaryButton',
    selector: '.btn-primary, .ant-btn-primary, [data-testid="dashboard-confirm-primary"]',
  },
  { key: 'table', selector: '.ant-table, table' },
  { key: 'tableHeaderCell', selector: '.ant-table-thead th, table thead th' },
  { key: 'tableBodyCell', selector: '.ant-table-tbody td, table tbody td' },
  { key: 'input', selector: '.ant-input, input, textarea' },
  { key: 'alert', selector: '.alert, [data-testid="dashboard-alert"]' },
  { key: 'dashboardTile', selector: '[data-testid="dashboard-shortcut"], .block-link-pop' },
];

export async function clickDarkModeButton(page) {
  const shadcnTriggerSelector = '#page-header button[data-dark-mode-trigger]';
  const shadcnTriggerVisible = await page.evaluate((selector) => {
    const element = document.querySelector(selector);
    if (!element) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return rect.width > 0 && rect.height > 0 && style.display !== 'none';
  }, shadcnTriggerSelector);

  if (shadcnTriggerVisible) {
    // The redesigned user header exposes a System/Light/Dark menu, so the trigger
    // opens the menu rather than toggling directly — open it and pick Dark to
    // enable dark mode for this interaction. The radio items are portaled to the
    // document body, so they are not scoped under #page-header.
    await page.click(shadcnTriggerSelector);
    await page.click('[data-theme-option="dark"]');
    return;
  }

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

export async function darkModePersistenceState(page) {
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
    const shadcnButton = document.querySelector('#page-header button[data-dark-mode-trigger]');

    return {
      cookieDarkMode: readCookie('dark_mode'),
      darkReaderReady:
        document.documentElement.getAttribute('data-darkreader-mode') === 'dynamic' &&
        document.documentElement.getAttribute('data-darkreader-scheme') === 'dark' &&
        document.querySelectorAll('.darkreader').length > 0,
      iconClass: icon?.className ?? '',
      shadcnDarkReady:
        document.documentElement.classList.contains('dark') &&
        document.documentElement.style.colorScheme === 'dark',
      triggerLabel: shadcnButton?.getAttribute('aria-label') ?? '',
      visibleSvgIcon: shadcnButton
        ? Boolean(Array.from(shadcnButton.querySelectorAll('svg')).find(isVisible))
        : false,
    };
  });
}

export async function waitForStableDarkModeStyleSnapshot(page, diagnostics) {
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

export async function darkModeStyleSnapshot(page) {
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

export async function currentDarkModeRuntime(page) {
  return page.evaluate(() =>
    document.querySelector('#page-header button[data-dark-mode-trigger]') ? 'shadcn' : 'darkreader',
  );
}

export async function waitForCurrentDarkModeRuntime(page, diagnostics) {
  await page.waitForFunction(
    () =>
      Boolean(
        document.querySelector(
          '#page-header button[data-dark-mode-trigger], #page-header button i.fa-sun, #page-header button i.fa-moon',
        ),
      ),
    null,
    { timeout: 10_000 },
  );
  const runtime = await currentDarkModeRuntime(page);
  if (runtime === 'shadcn') {
    await waitForShadcnDarkMode(page, diagnostics);
  } else {
    await waitForDarkReader(page, diagnostics);
  }
}

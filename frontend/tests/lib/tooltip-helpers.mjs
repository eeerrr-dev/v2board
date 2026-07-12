export async function hoverTooltipInteraction(page, selectors) {
  const before = await tooltipState(page);
  await hoverFirstVisibleFromSelectors(page, selectors);
  await waitForVisibleTooltip(page);
  await page.waitForTimeout(150);
  const opened = await tooltipState(page);
  return { before, opened };
}

export async function hoverAllTooltipTargetsInteraction(page, selectors) {
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
    await page.keyboard.press('Escape').catch(() => undefined);
    await waitForNoVisibleTooltip(page, 1_000).catch(() => undefined);
  }

  return { before, opened, targetCount, viewportWidth };
}

export async function tooltipState(page) {
  return page.evaluate(() => {
    const normalize = (value) => (value ?? '').trim().replace(/\s+/g, ' ');
    // The redesigned Radix tooltip renders its title twice inside the content
    // element: once visibly and once in a 1px visually-hidden aria copy for the
    // screen-reader announcement (the legacy antd tooltip has no such copy). Read
    // only the visible portion so `texts` reflects the shown help copy, not the
    // doubled DOM. Applied to both DOMs, so it never masks a real text mismatch.
    const readVisibleText = (element) => {
      let out = '';
      element.childNodes.forEach((node) => {
        if (node.nodeType === Node.TEXT_NODE) {
          out += node.textContent ?? '';
          return;
        }
        if (node.nodeType === Node.ELEMENT_NODE) {
          const rect = node.getBoundingClientRect();
          if (rect.width <= 1 && rect.height <= 1) return;
          out += node.textContent ?? '';
        }
      });
      return out;
    };
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
    const isOpenTooltip = (element) =>
      element.getAttribute('data-state') !== 'closed' &&
      !String(element.className).includes('ant-tooltip-hidden');
    const tooltips = Array.from(
      document.querySelectorAll('[data-slot="tooltip-content"], .ant-tooltip'),
    )
      .filter(isOpenTooltip)
      .filter(isVisible);
    const tooltip = tooltips[0];
    const textElements = tooltip
      ? tooltip.matches('[data-slot="tooltip-content"]')
        ? [tooltip]
        : Array.from(tooltip.querySelectorAll('.ant-tooltip-inner'))
      : [];

    return {
      className: tooltip ? normalize(tooltip.className) : '',
      openTriggerCount: Array.from(
        document.querySelectorAll(
          [
            '[data-slot="header-tooltip-trigger"][data-state="delayed-open"]',
            '[data-slot="header-tooltip-trigger"][data-state="instant-open"]',
            '.ant-tooltip-open',
          ].join(', '),
        ),
      ).filter(isVisible).length,
      placement: (() => {
        const antPlacement =
          tooltip?.getAttribute('data-placement') ??
          tooltip?.className.match(/ant-tooltip-placement-([A-Za-z]+)/)?.[1];
        if (antPlacement) return antPlacement;
        // The redesigned Radix tooltip encodes its position as data-side +
        // data-align instead of a legacy data-placement attribute or
        // ant-tooltip-placement-* class: side 'top' with align 'end' is the
        // legacy 'topRight', any other top alignment is plain 'top'. Reading
        // it back keeps the placement assertion honest instead of dropping it.
        const side = tooltip?.getAttribute('data-side');
        if (!side) return '';
        const align = tooltip?.getAttribute('data-align');
        return side === 'top' && align === 'end' ? 'topRight' : side;
      })(),
      texts: tooltip
        ? textElements
            .filter(isVisible)
            .map((element) => normalize(readVisibleText(element)))
            .filter(Boolean)
        : [],
      tooltipCount: tooltips.length,
    };
  });
}

export async function waitForVisibleTooltip(page, timeout = 5_000) {
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
      const isOpenTooltip = (element) =>
        element.getAttribute('data-state') !== 'closed' &&
        !String(element.className).includes('ant-tooltip-hidden');
      return Array.from(
        document.querySelectorAll('[data-slot="tooltip-content"], .ant-tooltip'),
      )
        .filter(isOpenTooltip)
        .some(isVisible);
    },
    null,
    { timeout },
  );
}

export async function waitForNoVisibleTooltip(page, timeout = 5_000) {
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
      const isOpenTooltip = (element) =>
        element.getAttribute('data-state') !== 'closed' &&
        !String(element.className).includes('ant-tooltip-hidden');
      return !Array.from(
        document.querySelectorAll('[data-slot="tooltip-content"], .ant-tooltip'),
      )
        .filter(isOpenTooltip)
        .some(isVisible);
    },
    null,
    { timeout },
  );
}

export async function hoverFirstVisibleFromSelectors(page, selectors) {
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
  // Playwright can reuse the browser pointer position across pages/worlds. If
  // two table-header triggers land at the same coordinates, a direct move may
  // emit no new mouseenter. Leave the target first, then enter it explicitly.
  await page.mouse.move(1, 1);
  await page.mouse.move(point.x, point.y);
}

export async function visibleTooltipTargetCount(page, selectors) {
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

export async function hoverVisibleTooltipTargetAt(page, selectors, index) {
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

export async function hoverVisibleTooltipTargetAncestorAt(page, selectors, index, ancestorSelector) {
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

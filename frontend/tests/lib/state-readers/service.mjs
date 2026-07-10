export async function setServiceTableScrollLeft(page, position) {
  await page.evaluate((targetPosition) => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const body = Array.from(
      document.querySelectorAll('[data-testid="service-table-scroll"], .ant-table-body'),
    ).find(isVisible);
    if (!body) return;
    const maxScroll = Math.max(0, body.scrollWidth - body.clientWidth);
    body.scrollLeft =
      targetPosition === 'middle' ? Math.floor(maxScroll / 2) : maxScroll;
    body.dispatchEvent(new Event('scroll', { bubbles: true }));
  }, position);
}

export async function serviceTableScrollState(page) {
  return page.evaluate(() => {
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none';
    };
    const table = Array.from(
      document.querySelectorAll('[data-testid="service-table-scroll"], .ant-table.ant-table-default'),
    ).find(isVisible);
    const body = Array.from(
      document.querySelectorAll('[data-testid="service-table-scroll"], .ant-table-body'),
    ).find(isVisible);
    const maxScroll = body ? Math.max(0, body.scrollWidth - body.clientWidth) : 0;
    const className = String(table?.className ?? '');
    const hasLegacyMiddle = className.includes('ant-table-scroll-position-middle');
    const hasLegacyLeft = className.includes('ant-table-scroll-position-left');
    const hasLegacyRight = className.includes('ant-table-scroll-position-right');
    let legacyScrollPosition = '';
    if (hasLegacyMiddle) {
      legacyScrollPosition = 'middle';
    } else if (hasLegacyLeft && hasLegacyRight) {
      legacyScrollPosition = 'both';
    } else if (hasLegacyLeft) {
      legacyScrollPosition = 'left';
    } else if (hasLegacyRight) {
      legacyScrollPosition = 'right';
    }

    return {
      className,
      clientWidth: Math.round(body?.clientWidth ?? 0),
      scrollPosition: table?.getAttribute('data-scroll-position') ?? legacyScrollPosition,
      maxScroll: Math.round(maxScroll),
      rows: Array.from(
        document.querySelectorAll(
          '[data-table-kind="service"] tbody tr, .ant-table-tbody tr',
        ),
      )
        .filter(isVisible)
        .slice(0, 4)
        .map((row) => (row.textContent ?? '').trim().replace(/\s+/g, ' ')),
      scrollLeft: Math.round(body?.scrollLeft ?? 0),
      scrollWidth: Math.round(body?.scrollWidth ?? 0),
    };
  });
}

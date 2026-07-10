import { fontWaitTimeout, navigationAttempts, navigationTimeout } from './env.mjs';
import { readDebugSnapshot } from './dom-helpers.mjs';

export async function waitForMountedContent(page, diagnostics) {
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

export async function readMountedContentState(page) {
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

export async function waitForFontsBeforeCapture(page, diagnostics) {
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

export async function waitForFixedColumnLayout(page) {
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

export async function waitForDarkReader(page, diagnostics) {
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

export async function waitForShadcnDarkMode(page, diagnostics) {
  await page
    .waitForFunction(
      () =>
        document.documentElement.classList.contains('dark') &&
        document.documentElement.style.colorScheme === 'dark',
      null,
      { timeout: 10_000 },
    )
    .catch(async (error) => {
      const snapshot = await readDebugSnapshot(page);
      const state = await page
        .evaluate(() => ({
          className: document.documentElement.className,
          colorScheme: document.documentElement.style.colorScheme,
          cookie: document.cookie,
        }))
        .catch((stateError) => ({ error: stateError.message }));
      throw new Error(
        `shadcn dark mode did not become ready: ${error.message}\n` +
          `URL: ${snapshot.url}\n` +
          `Title: ${snapshot.title}\n` +
          `Body: ${snapshot.body}\n` +
          `State: ${JSON.stringify(state)}\n` +
          `Diagnostics: ${diagnostics.slice(-40).join(' | ')}`,
      );
    });

  const state = await page.evaluate(() => ({
    className: document.documentElement.className,
    colorScheme: document.documentElement.style.colorScheme,
  }));
  diagnostics.push(`shadcn dark ready ${JSON.stringify(state)}`);
  await page.waitForTimeout(100);
}

export async function gotoStable(page, url) {
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

export async function navigateAfterWarmup(page, url) {
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

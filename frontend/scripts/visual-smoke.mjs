import { chromium } from 'playwright';

const baseUrl = new URL(process.env.VISUAL_SMOKE_BASE_URL ?? 'http://laravel:8000');
const adminPath = stripSlashes(process.env.VISUAL_SMOKE_ADMIN_PATH ?? 'admin');

const oldChunkNames = [
  `components${'.chunk.css'}`,
  `vendors${'.async.js'}`,
  `components${'.async.js'}`,
];
const forbiddenFragments = [
  ...oldChunkNames,
  `/theme/default/assets/${'i18n'}/`,
  `/theme/default/assets/${'theme'}/`,
  `/assets/admin/${'theme'}/`,
];

const pages = [
  {
    cssPath: userAssetPath('umi.css'),
    jsPath: userAssetPath('umi.js'),
    label: 'user',
    path: '/',
  },
  {
    cssPath: '/assets/admin/umi.css',
    jsPath: '/assets/admin/umi.js',
    label: 'admin',
    path: `/${adminPath}`,
  },
];

const viewports = [
  { height: 900, label: 'desktop', width: 1440 },
  { height: 844, label: 'mobile', width: 390 },
];

const browser = await chromium.launch({ headless: true });
const failures = [];
const summaries = [];

try {
  for (const pageTarget of pages) {
    for (const viewport of viewports) {
      const page = await browser.newPage({ viewport });
      await smokePage(page, pageTarget, viewport);
      await page.close();
    }
  }
} finally {
  await browser.close();
}

if (failures.length) {
  throw new Error(`Visual smoke failed:\n${failures.map((line) => `- ${line}`).join('\n')}`);
}

console.log('Visual smoke OK: source-built user/admin pages render in Chromium.');
for (const summary of summaries) {
  console.log(
    `  ${summary.label}/${summary.viewport}: ${summary.visibleElements} visible elements, ` +
      `${summary.textLength} chars, overflow ${summary.horizontalOverflow}px`,
  );
}

async function smokePage(page, target, viewport) {
  const label = `${target.label}/${viewport.label}`;
  const badResponses = [];
  const consoleErrors = [];
  const runtimeErrors = [];

  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(`${label} console error: ${message.text()}`);
    }
  });

  page.on('pageerror', (error) => {
    runtimeErrors.push(`${label} runtime error: ${error.message}`);
  });

  page.on('response', (response) => {
    const url = response.url();
    const pathname = safePathname(url);
    if (forbiddenFragments.some((fragment) => pathname.includes(fragment))) {
      badResponses.push(`${label} requested forbidden legacy asset ${pathname}`);
      return;
    }

    const resourceType = response.request().resourceType();
    if (['stylesheet', 'script', 'font', 'image'].includes(resourceType) && !response.ok()) {
      badResponses.push(
        `${label} ${resourceType} ${pathname} returned ${response.status()}`,
      );
    }
  });

  const response = await page.goto(urlFor(target.path), {
    timeout: 30_000,
    waitUntil: 'domcontentloaded',
  });
  if (!response?.ok()) {
    failures.push(`${label} page returned ${response?.status() ?? 'no response'}`);
  }

  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await page.waitForTimeout(500);

  const metrics = await page.evaluate(({ cssPath, jsPath }) => {
    const root = document.querySelector('#root') ?? document.body;
    const rootRect = root.getBoundingClientRect();
    const html = document.documentElement;
    const body = document.body;
    const visibleElements = Array.from(body.querySelectorAll('*')).filter((element) => {
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.visibility !== 'hidden' &&
        style.display !== 'none' &&
        Number(style.opacity) !== 0
      );
    }).length;
    const stylesheetHrefs = Array.from(document.querySelectorAll('link[rel="stylesheet"]')).map(
      (link) => link.href,
    );
    const scriptSrcs = Array.from(document.querySelectorAll('script[src]')).map(
      (script) => script.src,
    );
    const sameOriginRuleCount = Array.from(document.styleSheets).reduce((total, sheet) => {
      try {
        return total + sheet.cssRules.length;
      } catch {
        return total;
      }
    }, 0);
    const horizontalOverflow = Math.max(html.scrollWidth, body.scrollWidth) - window.innerWidth;
    const textLength = (body.innerText ?? '').trim().length;

    return {
      bodyBackground: window.getComputedStyle(body).backgroundColor,
      cssLoaded: stylesheetHrefs.some((href) => href.includes(cssPath)),
      horizontalOverflow,
      jsLoaded: scriptSrcs.some((src) => src.includes(jsPath)),
      rootHeight: rootRect.height,
      rootWidth: rootRect.width,
      sameOriginRuleCount,
      textLength,
      visibleElements,
    };
  }, target);

  if (!metrics.cssLoaded) failures.push(`${label} did not load ${target.cssPath}`);
  if (!metrics.jsLoaded) failures.push(`${label} did not load ${target.jsPath}`);
  if (metrics.sameOriginRuleCount < 100) {
    failures.push(`${label} loaded too few CSS rules: ${metrics.sameOriginRuleCount}`);
  }
  if (metrics.rootWidth < Math.min(320, viewport.width - 20) || metrics.rootHeight < 240) {
    failures.push(`${label} root is too small: ${metrics.rootWidth}x${metrics.rootHeight}`);
  }
  if (metrics.visibleElements < 12) {
    failures.push(`${label} appears visually empty: ${metrics.visibleElements} visible elements`);
  }
  if (metrics.textLength < 8) {
    failures.push(`${label} rendered too little text: ${metrics.textLength} chars`);
  }
  if (metrics.horizontalOverflow > 4) {
    failures.push(`${label} has horizontal overflow: ${metrics.horizontalOverflow}px`);
  }

  failures.push(...badResponses, ...runtimeErrors);
  failures.push(...consoleErrors.filter((line) => !line.includes('favicon.ico')));
  summaries.push({ label: target.label, viewport: viewport.label, ...metrics });
}

function stripSlashes(value) {
  return value.replace(/^\/+|\/+$/g, '');
}

function userAssetPath(fileName) {
  return `/theme/default/${'assets'}/${fileName}`;
}

function urlFor(path) {
  const url = new URL(path, baseUrl);
  return url.toString();
}

function safePathname(rawUrl) {
  try {
    return new URL(rawUrl).pathname;
  } catch {
    return rawUrl;
  }
}

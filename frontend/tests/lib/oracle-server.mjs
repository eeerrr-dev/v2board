import { createReadStream } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, normalize, resolve, sep } from 'node:path';
import { adminPath, navigationAttempts, oraclePublicRoot, sourceBaseUrl } from './env.mjs';
import { delay } from './api-fixtures.mjs';
import { adminFixtureEndpoint, apiFixtureResponse } from './api-fixture-response.mjs';

export async function startOracleServer(port = 0, host = '127.0.0.1', advertisedHost = host, sourceSettings) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? '/', 'http://127.0.0.1');
    const pathname = decodeURIComponent(url.pathname);

    if (pathname === '/' || pathname === '/index.html') {
      sendHtml(response, legacyUserHtml(sourceSettings));
      return;
    }

    if (pathname === `/${adminPath}` || pathname === '/admin') {
      sendHtml(response, legacyAdminHtml(sourceSettings));
      return;
    }

    if (pathname === '/monitor/api/stats') {
      sendJson(response, { status: 'running' });
      return;
    }

    if (pathname.startsWith('/api/v1/')) {
      const referer = request.headers.referer ?? '';
      const isAdminScenario =
        Boolean(adminFixtureEndpoint(pathname)) || referer.includes(`/${adminPath}`);
      sendJson(response, apiFixtureResponse(url, isAdminScenario));
      return;
    }

    if (pathname.startsWith('/api/')) {
      sendJson(response, { code: 200, data: null });
      return;
    }

    await sendStaticFile(response, pathname);
  });

  await new Promise((resolveListen) => server.listen(port, host, resolveListen));
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('Oracle server did not bind a port');

  return {
    baseUrl: new URL(`http://${advertisedHost}:${address.port}`),
    close: () => new Promise((resolveClose) => server.close(resolveClose)),
  };
}

export function waitForShutdown() {
  return new Promise((resolveShutdown) => {
    process.once('SIGINT', resolveShutdown);
    process.once('SIGTERM', resolveShutdown);
  });
}

function legacyUserHtml(sourceSettings) {
  const settings = sourceSettings.user;
  const color = {
    black: '#343a40',
    darkblue: '#3b5998',
    default: '#0665d0',
    green: '#319795',
  }[settings.theme.color] ?? '#0665d0';

  return `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="/theme/default/assets/components.chunk.css?v=oracle">
  <link rel="stylesheet" href="/theme/default/assets/umi.css?v=oracle">
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
  <meta name="theme-color" content="${color}">
  <title>${escapeHtml(settings.title)}</title>
  <script>window.routerBase = "/";</script>
  <script>
    window.settings = {
      title: ${jsString(settings.title)},
      assets_path: '/theme/default/assets',
      theme: {
        sidebar: ${jsString(settings.theme.sidebar)},
        header: ${jsString(settings.theme.header)},
        color: ${jsString(settings.theme.color)}
      },
      version: ${jsString(settings.version)},
      background_url: ${jsString(settings.backgroundUrl)},
      description: ${jsString(settings.description)},
      i18n: ['zh-CN', 'en-US', 'ja-JP', 'vi-VN', 'ko-KR', 'zh-TW', 'fa-IR'],
      logo: ${jsString(settings.logo)}
    };
  </script>
  <script src="/theme/default/assets/i18n/zh-CN.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/zh-TW.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/en-US.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/ja-JP.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/vi-VN.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/ko-KR.js?v=oracle"></script>
  <script src="/theme/default/assets/i18n/fa-IR.js?v=oracle"></script>
</head>
<body>
  <div id="root"></div>
  <script src="/theme/default/assets/vendors.async.js?v=oracle"></script>
  <script src="/theme/default/assets/components.async.js?v=oracle"></script>
  <script src="/theme/default/assets/umi.js?v=oracle"></script>
</body>
</html>`;
}

function legacyAdminHtml(sourceSettings) {
  const settings = sourceSettings.admin;
  return `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="/assets/admin/components.chunk.css?v=oracle">
  <link rel="stylesheet" href="/assets/admin/umi.css?v=oracle">
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
  <title>${escapeHtml(settings.title)}</title>
  <script>window.routerBase = "/";</script>
  <script>
    window.settings = {
      title: ${jsString(settings.title)},
      theme: {
        sidebar: ${jsString(settings.theme.sidebar)},
        header: ${jsString(settings.theme.header)},
        color: ${jsString(settings.theme.color)}
      },
      version: ${jsString(settings.version)},
      background_url: ${jsString(settings.backgroundUrl)},
      logo: ${jsString(settings.logo)},
      secure_path: ${jsString(settings.securePath)}
    };
  </script>
</head>
<body>
  <div id="root"></div>
  <script src="/assets/admin/vendors.async.js?v=oracle"></script>
  <script src="/assets/admin/components.async.js?v=oracle"></script>
  <script src="/assets/admin/umi.js?v=oracle"></script>
</body>
</html>`;
}

export async function readSourceSettings() {
  const [userHtml, adminHtml] = await Promise.all([
    fetchSourceHtml('/'),
    fetchSourceHtml(`/${adminPath}`),
  ]);

  const userRuntime = requireRuntimeConfig(userHtml, 'user');
  const adminRuntime = requireRuntimeConfig(adminHtml, 'admin');

  return {
    admin: {
      backgroundUrl: stringSetting(adminRuntime, 'background_url') ?? '',
      logo: stringSetting(adminRuntime, 'logo') ?? '',
      securePath: requireStringSetting(adminRuntime, 'secure_path', 'admin'),
      theme: requireRuntimeTheme(adminRuntime, 'admin'),
      title: requireStringSetting(adminRuntime, 'title', 'admin'),
      version: 'reference',
    },
    user: {
      backgroundUrl: stringSetting(userRuntime, 'background_url') ?? '',
      description: stringSetting(userRuntime, 'description') ?? '',
      logo: stringSetting(userRuntime, 'logo') ?? '',
      theme: requireRuntimeTheme(userRuntime, 'user'),
      title: requireStringSetting(userRuntime, 'title', 'user'),
      version: 'reference',
    },
  };
}

function requireRuntimeConfig(html, label) {
  const config = extractRuntimeConfig(html);
  if (!config) {
    throw new Error(`${label} source HTML is missing a valid runtime-config bootstrap`);
  }
  return config;
}

function extractRuntimeConfig(html) {
  const match = /<script\b(?=[^>]*\bid=["']v2board-runtime-config["'])[^>]*>([\s\S]*?)<\/script>/i.exec(
    html,
  );
  if (!match) return null;
  try {
    const parsed = JSON.parse(match[1]);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function stringSetting(settings, key) {
  const value = settings?.[key];
  return typeof value === 'string' ? value : null;
}

function requireStringSetting(settings, key, label) {
  const value = stringSetting(settings, key);
  if (value === null) throw new Error(`${label} runtime config is missing string field ${key}`);
  return value;
}

function requireRuntimeTheme(settings, label) {
  const color = settings?.theme?.color;
  if (typeof color !== 'string') {
    throw new Error(`${label} runtime config is missing theme.color`);
  }
  return { color, header: 'dark', sidebar: 'light' };
}

async function fetchSourceHtml(path) {
  let lastError;
  for (let attempt = 1; attempt <= navigationAttempts; attempt += 1) {
    try {
      const response = await fetch(new URL(path, sourceBaseUrl));
      if (!response.ok) {
        throw new Error(`Failed to read source settings from ${path}: ${response.status}`);
      }
      return response.text();
    } catch (error) {
      lastError = error;
      if (attempt < navigationAttempts) {
        await delay(500 * attempt);
      }
    }
  }
  throw lastError;
}

function jsString(value) {
  return JSON.stringify(value);
}

function escapeHtml(value) {
  return value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return '&#39;';
    }
  });
}

async function sendStaticFile(response, pathname) {
  const filePath = safeResolve(oraclePublicRoot, pathname.slice(1));
  if (!filePath) {
    response.writeHead(403);
    response.end('Forbidden');
    return;
  }

  try {
    await readFile(filePath);
  } catch {
    response.writeHead(404);
    response.end('Not found');
    return;
  }

  if (pathname === '/assets/admin/umi.js') {
    const source = await readFile(filePath, 'utf8');
    response.writeHead(200, { 'content-type': contentType(filePath) });
    response.end(patchLegacyAdminOracle(source));
    return;
  }

  response.writeHead(200, { 'content-type': contentType(filePath) });
  createReadStream(filePath).pipe(response);
}

function patchLegacyAdminOracle(source) {
  // The packaged admin dashboard can render the connected header before the
  // user model slice is present in this oracle harness. Keep the old bundle's
  // intended follow-up /user/info flow, but make that first render null-safe.
  return source
    .replaceAll(
      'var e = this.props.user.userInfo;',
      'var e = (this.props.user && this.props.user.userInfo) || {};',
    )
    .replaceAll(
      'n.map(e=>{\n                    return m.a.createElement(_["a"].Option',
      '(n || []).map(e=>{\n                    return m.a.createElement(_["a"].Option',
    )
    .replaceAll(
      'return f.map(t=>{\n                            t.id === parseInt(e)',
      'return (f || []).map(t=>{\n                            t.id === parseInt(e)',
    )
    .replaceAll(
      '}, g.map(e=>{\n                    return f.a.createElement("option", {',
      '}, (g || []).map(e=>{\n                    return f.a.createElement("option", {',
    )
    .replaceAll('filters: R.map(e=>({', 'filters: (R || []).map(e=>({')
    .replaceAll(
      'var t = R.find(t=>t.id === parseInt(e));',
      'var t = (R || []).find(t=>t.id === parseInt(e));',
    )
    .replaceAll(
      'var t = M.find(t=>t.id === e);',
      'var t = (M || []).find(t=>t.id === e);',
    );
}

function sendHtml(response, html) {
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end(html);
}

function sendJson(response, body) {
  response.writeHead(200, { 'content-type': 'application/json' });
  response.end(JSON.stringify(body));
}

function safeResolve(root, path) {
  const resolved = resolve(root, normalize(path));
  return resolved === root || resolved.startsWith(`${root}${sep}`) ? resolved : null;
}

function contentType(filePath) {
  switch (extname(filePath).toLowerCase()) {
    case '.css':
      return 'text/css; charset=utf-8';
    case '.js':
      return 'application/javascript; charset=utf-8';
    case '.json':
      return 'application/json; charset=utf-8';
    case '.png':
      return 'image/png';
    case '.svg':
      return 'image/svg+xml';
    case '.woff':
      return 'font/woff';
    case '.woff2':
      return 'font/woff2';
    case '.ttf':
      return 'font/ttf';
    case '.eot':
      return 'application/vnd.ms-fontobject';
    default:
      return 'application/octet-stream';
  }
}

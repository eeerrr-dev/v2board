import type { Plugin, UserConfig } from 'vite';
import path from 'node:path';
import fs from 'node:fs';

export interface AppViteOptions {
  port: number;
  base?: string;
  apiTarget?: string;
}

const LEGACY_MIME: Record<string, string> = {
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.eot': 'application/vnd.ms-fontobject',
  '.otf': 'font/otf',
};

function legacyAssetPlugin(name: string, mount: string, publicPath: string): Plugin {
  return {
    name,
    configureServer(server) {
      const assetRoot = path.resolve(server.config.root, publicPath);
      server.middlewares.use(mount, (req, res) => {
        const url = (req.url ?? '').split('?')[0] ?? '';
        const filePath = path.join(assetRoot, decodeURIComponent(url));
        if (filePath !== assetRoot && !filePath.startsWith(assetRoot + path.sep)) {
          res.statusCode = 403;
          res.end('Forbidden');
          return;
        }
        fs.stat(filePath, (err, stat) => {
          // `/theme` is exclusively the legacy-asset namespace, so a miss is a
          // real 404 — never fall through to Vite's SPA fallback (which would
          // answer a missing font/CSS with index.html under a 200).
          if (err || !stat.isFile()) {
            res.statusCode = 404;
            res.end('Not found');
            return;
          }
          const type = LEGACY_MIME[path.extname(filePath).toLowerCase()];
          if (type) res.setHeader('Content-Type', type);
          fs.createReadStream(filePath).pipe(res);
        });
      });
    },
  };
}

// Serve the original packaged theme assets (repo `public/theme/**`) from the
// Vite dev server so `pnpm dev` boots with the SAME legacy CSS, i18n maps and
// per-color theme files the production `dashboard.blade.php` loads. Without it
// the dev server only loads the rewrite's own globals.css and renders the app
// unstyled/untranslated (the ":5173 white screen"). The app's Vite `root` is
// `<repo>/frontend/apps/<name>`, so `public/theme` is three levels up.
export function legacyThemePlugin(): Plugin {
  return legacyAssetPlugin('serve-legacy-theme', '/theme', '../../../public/theme');
}

// Same idea for the admin app: its legacy blade loads `/assets/admin/*.css`
// before `umi.js`, and the OneUI login/layout classes rely on those files.
export function legacyAdminAssetsPlugin(): Plugin {
  return legacyAssetPlugin(
    'serve-legacy-admin-assets',
    '/assets/admin',
    '../../../public/assets/admin',
  );
}

// The legacy admin dashboard polls Horizon directly from the current origin.
// In Vite dev, that path would otherwise fall through to index.html and trip
// the React error overlay when the response is parsed as JSON.
export function localHorizonStatsPlugin(): Plugin {
  return {
    name: 'serve-local-horizon-stats',
    configureServer(server) {
      server.middlewares.use('/monitor/api/stats', (_req, res) => {
        res.statusCode = 200;
        res.setHeader('Content-Type', 'application/json; charset=utf-8');
        res.end(JSON.stringify({ status: 'running' }));
      });
    },
  };
}

export function legacyNavigationRedirectPlugin(): Plugin {
  return {
    name: 'legacy-navigation-redirect',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.method !== 'GET' && req.method !== 'HEAD') {
          next();
          return;
        }

        let url: URL;
        try {
          url = new URL(req.url ?? '/', 'http://127.0.0.1');
        } catch {
          next();
          return;
        }

        const pathname = url.pathname;
        const isAssetOrApi =
          pathname === '/' ||
          pathname === '/api' ||
          pathname.includes('.') ||
          pathname.startsWith('/@') ||
          pathname.startsWith('/api/') ||
          pathname.startsWith('/assets/') ||
          pathname.startsWith('/monitor/') ||
          pathname.startsWith('/node_modules/') ||
          pathname.startsWith('/src/') ||
          pathname.startsWith('/theme/');

        if (isAssetOrApi) {
          next();
          return;
        }

        req.resume();
        res.writeHead(302, {
          location: `/#${pathname}${url.search}`,
          'cache-control': 'no-store',
          'content-length': '0',
          connection: 'close',
        });
        res.end();
      });
    },
  };
}

export function stripViteClientPlugin(): Plugin {
  return {
    name: 'strip-vite-client',
    apply: 'serve',
    transformIndexHtml: {
      order: 'post',
      handler(html) {
        return html.replace(
          /\n?\s*<script\s+type="module"\s+src="[^"]*\/@vite\/client"><\/script>\n?/g,
          '\n',
        );
      },
    },
  };
}

const VITE_CLIENT_RUNTIME_STUB = `
const styles = new Map();

export function updateStyle(id, content) {
  let style = styles.get(id);
  if (!style) {
    style = document.createElement('style');
    style.setAttribute('data-vite-dev-id', id);
    document.head.appendChild(style);
    styles.set(id, style);
  }
  style.textContent = content;
}

export function removeStyle(id) {
  const style = styles.get(id);
  if (!style) return;
  style.remove();
  styles.delete(id);
}

export function createHotContext() {
  const hot = {
    accept() {},
    decline() {},
    dispose() {},
    invalidate() {},
    off() {},
    on() {},
    prune() {},
    send() {},
    data: {},
  };
  return hot;
}

export function injectQuery(url, queryToInject) {
  if (url[0] !== '.' && url[0] !== '/') return url;
  const pathname = url.replace(/[?#].*$/, '');
  const parsed = new URL(url, 'http://vite.dev');
  return pathname + '?' + queryToInject + (parsed.search ? '&' + parsed.search.slice(1) : '') + (parsed.hash || '');
}

export class ErrorOverlay extends HTMLElement {}

export default {};
`;

export function legacyViteClientStubPlugin(): Plugin {
  return {
    name: 'legacy-vite-client-stub',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const pathname = (req.url ?? '').split('?')[0];
        if (pathname !== '/@vite/client') {
          next();
          return;
        }

        res.statusCode = 200;
        res.setHeader('Content-Type', 'text/javascript; charset=utf-8');
        res.end(VITE_CLIENT_RUNTIME_STUB);
      });
    },
  };
}

export function buildAppViteConfig(options: AppViteOptions): UserConfig {
  const apiTarget = options.apiTarget ?? process.env.VITE_API_BASE ?? 'http://127.0.0.1:8000';
  return {
    base: options.base ?? '/',
    server: {
      port: options.port,
      host: '0.0.0.0',
      // The packaged theme never had Vite's HMR/React Refresh runtime. Disable
      // it in local dev so an open legacy page is not half-updated while code is
      // changing or the container recompiles; manual refreshes still load the
      // latest source, and runtime errors stay visible in the console/boundary.
      hmr: false,
      headers: {
        'Cache-Control': 'no-store, max-age=0',
      },
      proxy: {
        '/api': { target: apiTarget, changeOrigin: true, secure: false },
      },
    },
    preview: { port: options.port },
    build: {
      target: 'es2023',
      sourcemap: false,
      cssCodeSplit: true,
      reportCompressedSize: false,
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes('node_modules')) {
              if (id.includes('antd') || id.includes('@ant-design')) return 'antd';
              if (id.includes('react-dom')) return 'react-dom';
              if (id.includes('react')) return 'react';
              if (id.includes('framer-motion')) return 'motion';
            }
            return undefined;
          },
        },
      },
    },
  };
}

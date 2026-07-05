import type { Plugin, UserConfig } from 'vite';

export interface AppViteOptions {
  port: number;
  base?: string;
  apiTarget?: string;
}

export function rejectPackagedUserAssetsPlugin(): Plugin {
  return {
    name: 'reject-packaged-user-assets',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const pathname = (req.url ?? '').split('?')[0] ?? '';
        if (!pathname.startsWith('/theme/default/assets/')) {
          next();
          return;
        }

        req.resume();
        res.statusCode = 404;
        res.setHeader('Content-Type', 'text/plain; charset=utf-8');
        res.end('Not found');
      });
    },
  };
}

export function rejectPackagedAdminAssetsPlugin(): Plugin {
  return {
    name: 'reject-packaged-admin-assets',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const pathname = (req.url ?? '').split('?')[0] ?? '';
        if (!pathname.startsWith('/assets/admin/')) {
          next();
          return;
        }

        req.resume();
        res.statusCode = 404;
        res.setHeader('Content-Type', 'text/plain; charset=utf-8');
        res.end('Not found');
      });
    },
  };
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

export interface LegacyNavigationRedirectOptions {
  passthroughPaths?: string[];
}

export function legacyNavigationRedirectPlugin(options: LegacyNavigationRedirectOptions = {}): Plugin {
  const passthroughPaths = new Set(options.passthroughPaths ?? []);

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
          passthroughPaths.has(pathname) ||
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
      allowedHosts: ['frontend', 'host.docker.internal', 'localhost', '127.0.0.1'],
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
          // Vendor split for the admin replica, which still ships antd. The user app
          // is a pure shadcn island with no antd, so the `antd` branch is inert in its
          // plain `vite build` output — and that output is never deployed anyway: the
          // Tier-1 Laravel drop-in is built by vite.config.deploy.ts (a standalone
          // single-IIFE umi.js/umi.css config that does NOT extend this base).
          manualChunks(id) {
            if (id.includes('node_modules')) {
              if (id.includes('antd') || id.includes('@ant-design')) return 'antd';
              if (id.includes('react-dom')) return 'react-dom';
              if (id.includes('react')) return 'react';
            }
            return undefined;
          },
        },
      },
    },
  };
}

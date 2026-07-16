import type { Plugin, UserConfig } from 'vite';

export interface AppViteOptions {
  port: number;
  base?: string;
  apiTarget?: string;
}

export interface HashNavigationRedirectOptions {
  passthroughPaths?: string[];
}

export function hashNavigationRedirectPlugin(
  options: HashNavigationRedirectOptions = {},
): Plugin {
  const passthroughPaths = new Set(options.passthroughPaths ?? []);

  return {
    name: 'hash-navigation-redirect',
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
          pathname.startsWith('/node_modules/') ||
          pathname.startsWith('/src/');

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

export function buildAppViteConfig(options: AppViteOptions): UserConfig {
  const apiTarget = options.apiTarget ?? process.env.VITE_API_BASE ?? 'http://127.0.0.1:8000';
  return {
    base: options.base ?? '/',
    server: {
      port: options.port,
      host: '0.0.0.0',
      allowedHosts: ['frontend', 'host.docker.internal', 'localhost', '127.0.0.1'],
      hmr: true,
      // Pre-transform the entry graph while the deterministic dep optimizer
      // prepares, so the first browser request after startup is warm.
      warmup: { clientFiles: ['./src/main.tsx'] },
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
    },
  };
}

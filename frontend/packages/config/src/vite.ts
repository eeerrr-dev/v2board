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
  return legacyAssetPlugin('serve-legacy-admin-assets', '/assets/admin', '../../../public/assets/admin');
}

export function buildAppViteConfig(options: AppViteOptions): UserConfig {
  const apiTarget = options.apiTarget ?? process.env.VITE_API_BASE ?? 'http://127.0.0.1:8000';
  return {
    base: options.base ?? '/',
    server: {
      port: options.port,
      host: '0.0.0.0',
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

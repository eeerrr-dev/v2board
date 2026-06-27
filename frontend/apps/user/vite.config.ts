import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import {
  buildAppViteConfig,
  legacyNavigationRedirectPlugin,
  legacyViteClientStubPlugin,
  rejectPackagedUserAssetsPlugin,
  stripViteClientPlugin,
} from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5173 }),
  cacheDir: '../../node_modules/.vite/user-white-screen-recovery-37',
  plugins: [
    legacyNavigationRedirectPlugin(),
    legacyViteClientStubPlugin(),
    rejectPackagedUserAssetsPlugin(),
    react(),
    stripViteClientPlugin(),
  ],
  optimizeDeps: {
    include: [
      '@tanstack/react-query',
      '@v2board/api-client > axios',
      'clsx',
      'i18next',
      'markdown-it',
      'qrcode.react',
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-dev-runtime',
      'react/jsx-runtime',
      'react-i18next',
      'react-router',
      'tailwind-merge',
    ],
    holdUntilCrawlEnd: false,
    noDiscovery: true,
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
});

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
  cacheDir: '../../node_modules/.vite/user-white-screen-recovery-38',
  plugins: [
    legacyNavigationRedirectPlugin(),
    legacyViteClientStubPlugin(),
    rejectPackagedUserAssetsPlugin(),
    react(),
    stripViteClientPlugin(),
  ],
  optimizeDeps: {
    // noDiscovery is intentional (see vite-config.test.ts): it stops Vite from
    // re-optimizing mid-session, which would full-reload the HMR-disabled dev
    // server. That makes this list the *complete* pre-bundle set, so it must
    // declare every direct third-party runtime dependency the source imports.
    include: [
      '@hookform/resolvers/zod',
      '@radix-ui/react-alert-dialog',
      '@radix-ui/react-checkbox',
      '@radix-ui/react-dialog',
      '@radix-ui/react-dropdown-menu',
      '@radix-ui/react-label',
      '@radix-ui/react-progress',
      '@radix-ui/react-radio-group',
      '@radix-ui/react-select',
      '@radix-ui/react-slot',
      '@radix-ui/react-switch',
      '@radix-ui/react-tooltip',
      '@stripe/react-stripe-js',
      '@stripe/stripe-js',
      '@stripe/stripe-js/pure',
      '@tanstack/react-query',
      '@tanstack/react-table',
      '@tanstack/react-virtual',
      '@v2board/api-client > axios',
      'class-variance-authority',
      'clsx',
      'dompurify',
      'i18next',
      'lucide-react',
      'markdown-it',
      'qrcode.react',
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-dev-runtime',
      'react/jsx-runtime',
      'react-hook-form',
      'react-i18next',
      'react-router',
      'sonner',
      'tailwind-merge',
      'zod',
    ],
    holdUntilCrawlEnd: false,
    noDiscovery: true,
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
});

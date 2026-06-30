import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';
import {
  buildAppViteConfig,
  legacyNavigationRedirectPlugin,
  rejectPackagedUserAssetsPlugin,
} from '@v2board/config/vite';

const baseConfig = buildAppViteConfig({ port: 5173 });

export default defineConfig({
  ...baseConfig,
  server: {
    ...baseConfig.server,
    // The user app is a redesigned React island (admin still serves the legacy
    // replica with the shared hmr:false default + @vite/client stub). Here we
    // run Vite HMR + React Fast Refresh: noDiscovery below pins the dep graph,
    // so hot updates patch modules in place instead of the full re-optimizing
    // reloads the white-screen recovery net was built to guard against. The real
    // /@vite/client is allowed to load, so the stub/strip plugins are dropped.
    hmr: true,
  },
  cacheDir: '../../node_modules/.vite/user-white-screen-recovery-38',
  plugins: [
    legacyNavigationRedirectPlugin(),
    rejectPackagedUserAssetsPlugin(),
    tailwindcss(),
    react(),
    // React Compiler (1.0) auto-memoizes components/hooks so manual useMemo/
    // useCallback referential-stability ceremony is no longer load-bearing.
    // @vitejs/plugin-react@6 drives its transforms through oxc, so the compiler
    // runs as a separate @rolldown/plugin-babel pass via the plugin's own preset.
    babel({ presets: [reactCompilerPreset()] }),
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
      '@tanstack/react-query-devtools',
      '@tanstack/react-table',
      '@tanstack/react-virtual',
      '@v2board/api-client > axios',
      'class-variance-authority',
      'clsx',
      'dayjs',
      'dompurify',
      'i18next',
      'lucide-react',
      'markdown-it',
      'qrcode.react',
      'react',
      'react-dom',
      'react-dom/client',
      'react/compiler-runtime',
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

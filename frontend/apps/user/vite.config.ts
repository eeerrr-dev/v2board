import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';
import { buildAppViteConfig, hashNavigationRedirectPlugin } from '@v2board/config/vite';

const baseConfig = buildAppViteConfig({ port: 5173 });

export default defineConfig({
  ...baseConfig,
  cacheDir: '../../node_modules/.vite/user',
  plugins: [
    hashNavigationRedirectPlugin(),
    tailwindcss(),
    react(),
    // React Compiler (1.0) auto-memoizes components/hooks so manual useMemo/
    // useCallback referential-stability ceremony is no longer load-bearing.
    // @vitejs/plugin-react@6 drives its transforms through oxc, so the compiler
    // runs as a separate @rolldown/plugin-babel pass via the plugin's own preset.
    babel({ presets: [reactCompilerPreset()] }),
  ],
  optimizeDeps: {
    // noDiscovery makes dependency optimization deterministic; this list is
    // therefore the complete set of third-party runtime imports.
    include: [
      '@hookform/resolvers/zod',
      'radix-ui',
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
      'embla-carousel-react',
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
      'react-router/dom',
      'sonner',
      'tailwind-merge',
      'zod',
    ],
    holdUntilCrawlEnd: false,
    noDiscovery: true,
  },
  resolve: {
    alias: { '@': path.resolve(import.meta.dirname, 'src') },
  },
});

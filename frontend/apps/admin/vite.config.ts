import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';
import { buildAppViteConfig } from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5174 }),
  cacheDir: '../../node_modules/.vite/admin',
  plugins: [
    // History routing (docs/api-dialect.md §10.1): Vite's default SPA fallback
    // serves index.html for /{VITE_DEV_ADMIN_PATH}/* deep links in dev; the
    // router mounts under that base via createAdminRouter's basename.
    tailwindcss(),
    react(),
    // React Compiler (1.0) auto-memoizes the redesigned shadcn islands, matching
    // the user app. @vitejs/plugin-react@6 drives its transforms through oxc, so
    // the compiler runs as a separate @rolldown/plugin-babel pass via the preset.
    babel({ presets: [reactCompilerPreset()] }),
  ],
  optimizeDeps: {
    include: [
      '@hookform/resolvers/zod',
      'radix-ui',
      '@tanstack/react-query',
      '@tanstack/react-table',
      '@tanstack/react-virtual',
      '@v2board/api-client > axios',
      'class-variance-authority',
      'clsx',
      'dayjs',
      'i18next',
      'lucide-react',
      'react',
      'react-dom',
      'react-dom/client',
      'react/compiler-runtime',
      'react/jsx-dev-runtime',
      'react/jsx-runtime',
      'react-hook-form',
      'react-i18next',
      'react-is',
      'react-router',
      'react-router/dom',
      'recharts',
      'sonner',
      'tailwind-merge',
      'zod',
    ],
    // Every runtime dependency is declared above, so dependency optimization
    // can run in parallel with the browser instead of holding the first load.
    holdUntilCrawlEnd: false,
    noDiscovery: true,
  },
  resolve: { alias: { '@': path.resolve(import.meta.dirname, 'src') } },
});

import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';
import {
  buildAppViteConfig,
  legacyNavigationRedirectPlugin,
  legacyViteClientStubPlugin,
  localHorizonStatsPlugin,
  rejectPackagedAdminAssetsPlugin,
  stripViteClientPlugin,
} from '@v2board/config/vite';

const adminDevPath = (process.env.VITE_DEV_ADMIN_PATH ?? 'admin').replace(/^\/+|\/+$/g, '');
const adminPassthroughPaths = adminDevPath ? [`/${adminDevPath}`] : [];

export default defineConfig({
  ...buildAppViteConfig({ port: 5174 }),
  cacheDir: '../../node_modules/.vite/admin-white-screen-recovery-37',
  plugins: [
    legacyNavigationRedirectPlugin({ passthroughPaths: adminPassthroughPaths }),
    legacyViteClientStubPlugin(),
    rejectPackagedAdminAssetsPlugin(),
    tailwindcss(),
    react(),
    // React Compiler (1.0) auto-memoizes the redesigned shadcn islands, matching
    // the user app. @vitejs/plugin-react@6 drives its transforms through oxc, so
    // the compiler runs as a separate @rolldown/plugin-babel pass via the preset.
    babel({ presets: [reactCompilerPreset()] }),
    stripViteClientPlugin(),
    localHorizonStatsPlugin(),
  ],
  optimizeDeps: {
    include: [
      '@ant-design/icons',
      '@hookform/resolvers/zod',
      '@radix-ui/react-alert-dialog',
      '@radix-ui/react-avatar',
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
      '@tanstack/react-query',
      '@tanstack/react-query-devtools',
      '@tanstack/react-table',
      '@tanstack/react-virtual',
      'antd',
      'antd/locale/zh_CN',
      'axios',
      'class-variance-authority',
      'clsx',
      'dayjs',
      'echarts',
      'echarts/theme/vintage',
      'i18next',
      'lucide-react',
      'markdown-it',
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
    holdUntilCrawlEnd: true,
    noDiscovery: true,
  },
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
});

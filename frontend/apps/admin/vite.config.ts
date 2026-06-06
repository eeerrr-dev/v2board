import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import {
  buildAppViteConfig,
  legacyAdminAssetsPlugin,
  legacyViteClientStubPlugin,
  localHorizonStatsPlugin,
  stripViteClientPlugin,
} from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5174 }),
  cacheDir: '../../node_modules/.vite/admin-white-screen-recovery-19',
  plugins: [
    legacyViteClientStubPlugin(),
    react(),
    stripViteClientPlugin(),
    legacyAdminAssetsPlugin(),
    localHorizonStatsPlugin(),
  ],
  optimizeDeps: {
    include: [
      '@ant-design/icons',
      '@tanstack/react-query',
      'antd',
      'antd/locale/zh_CN',
      'axios',
      'dayjs',
      'echarts',
      'echarts/theme/vintage',
      'i18next',
      'markdown-it',
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-dev-runtime',
      'react/jsx-runtime',
      'react-i18next',
      'react-router-dom',
    ],
    holdUntilCrawlEnd: true,
    noDiscovery: true,
  },
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
});

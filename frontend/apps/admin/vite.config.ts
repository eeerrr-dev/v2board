import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import {
  buildAppViteConfig,
  legacyAdminAssetsPlugin,
  localHorizonStatsPlugin,
} from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5174 }),
  plugins: [react(), legacyAdminAssetsPlugin(), localHorizonStatsPlugin()],
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
      'react-dom/client',
      'react-i18next',
      'react-router-dom',
    ],
  },
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
});

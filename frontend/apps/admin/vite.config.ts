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
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
});

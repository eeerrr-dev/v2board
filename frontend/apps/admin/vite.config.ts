import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { buildAppViteConfig, legacyAdminAssetsPlugin } from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5174 }),
  plugins: [react(), legacyAdminAssetsPlugin()],
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
});

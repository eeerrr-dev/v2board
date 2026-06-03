import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { buildAppViteConfig, legacyThemePlugin } from '@v2board/config/vite';

export default defineConfig({
  ...buildAppViteConfig({ port: 5173 }),
  plugins: [react(), legacyThemePlugin()],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
});

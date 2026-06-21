import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

const deployOutDir =
  process.env.V2BOARD_DEPLOY_OUT_DIR ?? path.resolve(__dirname, '../../dist-deploy/theme/default/assets');

export default defineConfig({
  base: '/theme/default/assets/',
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  build: {
    target: 'es2023',
    sourcemap: false,
    cssCodeSplit: false,
    assetsInlineLimit: 0,
    reportCompressedSize: false,
    outDir: deployOutDir,
    emptyOutDir: false,
    modulePreload: false,
    rollupOptions: {
      input: path.resolve(__dirname, 'src/main.tsx'),
      output: {
        format: 'iife',
        entryFileNames: 'umi.js',
        chunkFileNames: 'chunks/[name].[hash].js',
        assetFileNames: (info) => {
          const name = info.name ?? '';
          if (name.endsWith('.css')) return 'umi.css';
          return 'static/[name].[hash][extname]';
        },
        manualChunks: undefined,
        inlineDynamicImports: true,
      },
    },
  },
});

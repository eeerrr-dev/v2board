import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

const deployOutDir = process.env.V2BOARD_DEPLOY_OUT_DIR;
if (!deployOutDir) {
  throw new Error('Deploy Vite config is internal; run the workspace pnpm build:deploy command');
}

export default defineConfig({
  base: '/assets/admin/',
  plugins: [tailwindcss(), react(), babel({ presets: [reactCompilerPreset()] })],
  resolve: {
    alias: { '@': path.resolve(import.meta.dirname, 'src') },
  },
  build: {
    target: 'es2023',
    sourcemap: false,
    cssCodeSplit: true,
    assetsInlineLimit: 0,
    reportCompressedSize: false,
    outDir: deployOutDir,
    emptyOutDir: true,
    manifest: 'manifest.json',
    modulePreload: { polyfill: false },
    rolldownOptions: {
      input: path.resolve(import.meta.dirname, 'index.html'),
      output: {
        entryFileNames: '[name]-[hash].js',
        chunkFileNames: '[name]-[hash].js',
        assetFileNames: 'asset-[hash][extname]',
      },
    },
  },
});

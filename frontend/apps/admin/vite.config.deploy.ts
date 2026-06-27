import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

const deployOutDir =
  process.env.V2BOARD_DEPLOY_OUT_DIR ?? path.resolve(__dirname, '../../dist-deploy/assets/admin');

export default defineConfig({
  base: '/assets/admin/',
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
    // The Laravel drop-in deploy is intentionally a single classic script.
    // Keep the limit near the current bundle so real growth still warns.
    chunkSizeWarningLimit: 3200,
    outDir: deployOutDir,
    emptyOutDir: false,
    modulePreload: false,
    rolldownOptions: {
      transform: {
        define: {
          // Deploy bundles are classic scripts, so make Rolldown's IIFE import.meta
          // replacement explicit instead of relying on its warning-time fallback.
          'import.meta': '{}',
        },
      },
      input: path.resolve(__dirname, 'src/main.tsx'),
      output: {
        format: 'iife',
        codeSplitting: false,
        entryFileNames: 'umi.js',
        chunkFileNames: 'chunks/[name].[hash].js',
        assetFileNames: (info) => {
          const name = info.name ?? '';
          if (name.endsWith('.css')) return 'umi.css';
          return 'static/[name].[hash][extname]';
        },
        manualChunks: undefined,
      },
    },
  },
});

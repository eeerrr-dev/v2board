import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

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
    reportCompressedSize: false,
    outDir: path.resolve(__dirname, '../../dist-deploy/assets/admin'),
    emptyOutDir: true,
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

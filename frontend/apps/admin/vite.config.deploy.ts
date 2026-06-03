import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { cpSync, existsSync, readFileSync, writeFileSync } from 'node:fs';

function copyLegacyAdminAssets() {
  return {
    name: 'copy-legacy-admin-assets',
    closeBundle() {
      const sourceRoot = path.resolve(__dirname, '../../../public/assets/admin');
      const outRoot = path.resolve(__dirname, '../../dist-deploy/assets/admin');

      for (const name of ['static', 'theme']) {
        const source = path.join(sourceRoot, name);
        if (existsSync(source)) cpSync(source, path.join(outRoot, name), { recursive: true });
      }

      for (const name of ['components.chunk.css', 'env.example.js', 'custom.css', 'custom.js']) {
        const source = path.join(sourceRoot, name);
        if (existsSync(source)) cpSync(source, path.join(outRoot, name));
      }

      const legacyCss = path.join(sourceRoot, 'umi.css');
      const generatedCss = path.join(outRoot, 'umi.css');
      if (!existsSync(legacyCss)) return;
      if (!existsSync(generatedCss)) {
        cpSync(legacyCss, generatedCss);
        return;
      }
      writeFileSync(
        generatedCss,
        `${readFileSync(legacyCss, 'utf8')}\n${readFileSync(generatedCss, 'utf8')}`,
      );
    },
  };
}

export default defineConfig({
  base: '/assets/admin/',
  plugins: [react(), copyLegacyAdminAssets()],
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

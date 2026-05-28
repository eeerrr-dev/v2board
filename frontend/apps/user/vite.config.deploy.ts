import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';
import { cpSync, existsSync, readFileSync, writeFileSync } from 'node:fs';

function copyLegacyAssets() {
  return {
    name: 'copy-legacy-user-assets',
    closeBundle() {
      const sourceRoot = path.resolve(__dirname, '../../../public/theme/default/assets');
      const outRoot = path.resolve(__dirname, '../../dist-deploy/theme/default/assets');
      for (const name of ['images', 'i18n']) {
        const source = path.join(sourceRoot, name);
        if (!existsSync(source)) continue;
        cpSync(source, path.join(outRoot, name), { recursive: true });
      }

      const vendorsJs = path.join(sourceRoot, 'vendors.async.js');
      if (existsSync(vendorsJs)) {
        cpSync(vendorsJs, path.join(outRoot, 'vendors.async.js'));
      }

      const componentsCss = path.join(sourceRoot, 'components.chunk.css');
      if (existsSync(componentsCss)) {
        cpSync(componentsCss, path.join(outRoot, 'components.chunk.css'));
      }

      const legacyCss = path.join(sourceRoot, 'umi.css');
      const generatedCss = path.join(outRoot, 'umi.css');
      if (existsSync(legacyCss) && existsSync(generatedCss)) {
        writeFileSync(
          generatedCss,
          `${readFileSync(legacyCss, 'utf8')}\n${readFileSync(generatedCss, 'utf8')}`,
        );
      }
    },
  };
}

export default defineConfig({
  base: '/theme/default/assets/',
  plugins: [react(), copyLegacyAssets()],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  build: {
    target: 'es2023',
    sourcemap: false,
    cssCodeSplit: false,
    reportCompressedSize: false,
    outDir: path.resolve(__dirname, '../../dist-deploy/theme/default/assets'),
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

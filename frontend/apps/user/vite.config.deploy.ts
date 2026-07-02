import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

const deployOutDir =
  process.env.V2BOARD_DEPLOY_OUT_DIR ?? path.resolve(__dirname, '../../dist-deploy/theme/default/assets');

export default defineConfig({
  base: '/theme/default/assets/',
  // @tailwindcss/vite now owns the `@import 'tailwindcss'` compile for the deploy
  // bundle's umi.css (postcss.config.cjs is retired); React Compiler runs via the
  // @rolldown/plugin-babel preset, same as the dev config.
  plugins: [tailwindcss(), react(), babel({ presets: [reactCompilerPreset()] })],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  build: {
    target: 'es2023',
    sourcemap: false,
    cssCodeSplit: false,
    assetsInlineLimit: 0,
    reportCompressedSize: false,
    // Keep the warning limit near the current single bundle so real growth still
    // warns (the deploy stays one classic script — see the output config below).
    chunkSizeWarningLimit: 1400,
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
        // Single classic-script deploy is a deliberate tradeoff, not an oversight.
        // The app source is already route-split (App.tsx USER_ROUTE_MODULES lazy
        // imports); this flattens it on purpose. Flipping to format:'es' +
        // codeSplitting:true works technically (a native ESM entry carries its own
        // import graph, so the hand-maintained Laravel blade needs no chunk manifest),
        // but is not worth it: `type="module"` would defer umi.js past the in-body
        // classic custom.js hook — running an operator's custom.js before the app
        // boots, a silent and untested break — and without modulePreload each deferred
        // route pays a fetch waterfall, for only a modest first-paint gain on a
        // login-gated SPA. Revisit together with a custom.js-ordering decision and
        // manifest-driven modulepreload injection if this bundle ever becomes a
        // measured LCP bottleneck.
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

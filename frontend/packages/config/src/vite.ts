import type { UserConfig } from 'vite';

export interface AppViteOptions {
  port: number;
  base?: string;
  apiTarget?: string;
}

export function buildAppViteConfig(options: AppViteOptions): UserConfig {
  const apiTarget = options.apiTarget ?? process.env.VITE_API_BASE ?? 'http://127.0.0.1:8000';
  return {
    base: options.base ?? '/',
    server: {
      port: options.port,
      host: '0.0.0.0',
      allowedHosts: ['frontend', 'host.docker.internal', 'localhost', '127.0.0.1'],
      hmr: true,
      // Pre-transform the entry graph while the deterministic dep optimizer
      // prepares, so the first browser request after startup is warm.
      warmup: { clientFiles: ['./src/main.tsx'] },
      headers: {
        'Cache-Control': 'no-store, max-age=0',
      },
      proxy: {
        '/api': { target: apiTarget, changeOrigin: true, secure: false },
      },
    },
    preview: { port: options.port },
    build: {
      target: 'es2023',
      sourcemap: false,
      cssCodeSplit: true,
      reportCompressedSize: false,
    },
  };
}

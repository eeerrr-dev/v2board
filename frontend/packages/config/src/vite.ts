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
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes('node_modules')) {
              if (id.includes('antd') || id.includes('@ant-design')) return 'antd';
              if (id.includes('react-dom')) return 'react-dom';
              if (id.includes('react')) return 'react';
              if (id.includes('framer-motion')) return 'motion';
            }
            return undefined;
          },
        },
      },
    },
  };
}

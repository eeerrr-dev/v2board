import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const viteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../vite.config.ts'),
  'utf8',
);

describe('user Vite dev optimizer', () => {
  it('keeps user optimized deps isolated and fully declared for stable page clicks', () => {
    expect(viteConfigSource).toContain("cacheDir: '../../node_modules/.vite/user'");
    expect(viteConfigSource).toContain('optimizeDeps: {');
    expect(viteConfigSource).toContain("'react-dom'");
    expect(viteConfigSource).toContain("'react/jsx-dev-runtime'");
    expect(viteConfigSource).toContain("'react/jsx-runtime'");
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: false');
    expect(viteConfigSource).toContain('noDiscovery: true');
  });
});

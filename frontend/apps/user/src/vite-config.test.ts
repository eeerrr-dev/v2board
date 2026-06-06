import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const viteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../vite.config.ts'),
  'utf8',
);
const sharedViteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../packages/config/src/vite.ts'),
  'utf8',
);

describe('user Vite dev optimizer', () => {
  it('keeps user optimized deps isolated and fully declared for stable page clicks', () => {
    expect(viteConfigSource).toContain(
      "cacheDir: '../../node_modules/.vite/user-white-screen-recovery-24'",
    );
    expect(viteConfigSource).toContain('optimizeDeps: {');
    expect(viteConfigSource).toContain('legacyViteClientStubPlugin()');
    expect(viteConfigSource).toContain('stripViteClientPlugin()');
    expect(viteConfigSource).toContain("'react-dom'");
    expect(viteConfigSource).toContain("'react/jsx-dev-runtime'");
    expect(viteConfigSource).toContain("'react/jsx-runtime'");
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: true');
    expect(viteConfigSource).toContain('noDiscovery: true');
  });

  it('disables Vite HMR so open legacy pages are not half-refreshed while clicking', () => {
    expect(sharedViteConfigSource).toContain('hmr: false');
    expect(sharedViteConfigSource).not.toContain('overlay: false');
    expect(sharedViteConfigSource).toContain('export function stripViteClientPlugin()');
    expect(sharedViteConfigSource).toContain('export function legacyViteClientStubPlugin()');
    expect(sharedViteConfigSource).toContain('export function updateStyle');
    expect(sharedViteConfigSource).toContain('export function createHotContext');
    expect(sharedViteConfigSource).toContain('export function injectQuery');
    expect(sharedViteConfigSource).toContain('export class ErrorOverlay');
    expect(sharedViteConfigSource).toContain('/@vite\\/client');
  });
});

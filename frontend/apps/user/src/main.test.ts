import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const mainSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'main.tsx'), 'utf8');
const configIndexSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../packages/config/src/index.ts'),
  'utf8',
);

describe('user legacy entrypoint', () => {
  it('normalizes broken hash routes before rendering the user router', () => {
    expect(mainSource).toContain("import { normalizeLegacyHashRoute } from '@v2board/config';");
    expect(mainSource).toContain('normalizeLegacyHashRoute({');
    expect(mainSource).toContain("authenticatedFallback: '/dashboard'");
    expect(mainSource).toContain("guestFallback: '/login'");
    expect(mainSource).toContain("publicRoutes: ['/', '/login', '/register', '/forgetpassword']");
    expect(mainSource).toContain('routes: USER_LEGACY_ROUTE_PATHS');
  });

  it('keeps the app on HashRouter like the bundled theme', () => {
    expect(mainSource).toContain("import { HashRouter } from 'react-router-dom';");
    expect(mainSource).toContain('<HashRouter>');
  });

  it('keeps the browser-facing config barrel free of Vite-only helpers', () => {
    expect(configIndexSource).toContain("export * from './format';");
    expect(configIndexSource).toContain("export * from './legacy-hash-route';");
    expect(configIndexSource).not.toContain("export * from './vite';");
  });
});

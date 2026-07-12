import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const mainSource = readFileSync(join(here, 'main.tsx'), 'utf8');
const indexSource = readFileSync(join(here, '../index.html'), 'utf8');

describe('user entrypoint', () => {
  it('uses the data router, StrictMode, session synchronization, and cache isolation', () => {
    expect(mainSource).toContain('createUserRouter(queryClient)');
    expect(mainSource).toContain('<RouterProvider router={router} />');
    expect(mainSource).toContain('<StrictMode>');
    expect(mainSource).toContain('registerSessionCacheClearer(() => queryClient.clear())');
    expect(mainSource).toContain('setupAuthSync()');
    expect(mainSource).toContain('registerRouterNavigation(router)');
    expect(mainSource).toContain('mutationCache: new MutationCache');
    expect(mainSource).toContain('presentMutationError(error, mutation.meta');
    expect(mainSource.indexOf('createUserRouter(queryClient)')).toBeLessThan(
      mainSource.indexOf('registerRouterNavigation(router)'),
    );
    expect(mainSource.indexOf('registerRouterNavigation(router)')).toBeLessThan(
      mainSource.indexOf('<RouterProvider router={router} />'),
    );
    expect(mainSource).not.toContain('HashRouter');
    expect(mainSource).not.toContain('queryCache: new QueryCache({\n    onError');
  });

  it('leaves URL normalization to data-router loaders without a watchdog', () => {
    expect(mainSource).not.toContain('normalizeLegacyHashRoute');
    expect(mainSource).not.toContain('installLegacyHashRouteNormalizer');
    expect(mainSource).not.toContain('installLegacyWhiteScreenRecovery');
    expect(mainSource).not.toContain('installLegacyDevModuleRecovery');
    expect(mainSource).not.toContain('installLegacyDevWhiteScreenFallback');
    expect(indexSource).not.toContain('white-screen-recovery');
    expect(indexSource).not.toContain('MutationObserver');
    expect(indexSource).not.toContain('window.location.replace');
  });

  it('prepaints dark mode before the standard Vite entry', () => {
    expect(indexSource).toContain("window.matchMedia('(prefers-color-scheme: dark)').matches");
    expect(indexSource).toContain("document.documentElement.classList.add('dark')");
    expect(indexSource).toContain('<script type="module" src="/src/main.tsx"></script>');
    expect(indexSource.indexOf("classList.add('dark')")).toBeLessThan(
      indexSource.indexOf('<script type="module" src="/src/main.tsx">'),
    );
  });
});

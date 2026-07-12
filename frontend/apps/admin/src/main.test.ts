import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const mainSource = readFileSync(join(here, 'main.tsx'), 'utf8');
const indexSource = readFileSync(join(here, '../index.html'), 'utf8');

describe('admin entrypoint', () => {
  it('uses the data router inside StrictMode', () => {
    expect(mainSource).toContain('createAdminRouter(queryClient)');
    expect(mainSource).toContain('<RouterProvider router={router} />');
    expect(mainSource).toContain('<StrictMode>');
    expect(mainSource).toContain('registerRouterNavigation(router)');
    expect(mainSource.indexOf('createAdminRouter(queryClient)')).toBeLessThan(
      mainSource.indexOf('registerRouterNavigation(router)'),
    );
    expect(mainSource.indexOf('registerRouterNavigation(router)')).toBeLessThan(
      mainSource.indexOf('<RouterProvider router={router} />'),
    );
    expect(mainSource).not.toContain('HashRouter');
    expect(mainSource).not.toContain('LegacyRouteGate');
  });

  it('synchronizes sessions and clears server state across identities', () => {
    expect(mainSource).toContain('registerSessionCacheClearer(() => queryClient.clear())');
    expect(mainSource).toContain('setupAuthSync()');
    expect(mainSource).toContain('mutationCache: new MutationCache');
    expect(mainSource).toContain('presentMutationError(error, mutation.meta');
    expect(mainSource).not.toContain('queryCache: new QueryCache');
  });

  it('boots the shared locale and document-language environment', () => {
    expect(mainSource).toContain('const i18n = await createLazyI18n()');
    expect(mainSource).toContain('installLocaleDocumentEnvironment(i18n)');
    expect(mainSource).toContain('<I18nextProvider i18n={i18n}>');
  });

  it('has no legacy DOM or white-screen recovery path', () => {
    for (const legacy of [
      'installLegacyWhiteScreenRecovery',
      'installLegacyDevModuleRecovery',
      'installLegacyDevWhiteScreenFallback',
      'normalizeLegacyHashRoute',
      'installLegacyHashRouteNormalizer',
    ]) {
      expect(mainSource).not.toContain(legacy);
    }
    expect(indexSource).not.toContain('white-screen-recovery');
    expect(indexSource).not.toContain('MutationObserver');
    expect(indexSource).not.toContain('window.location.replace');
  });

  it('prepaints dark mode before the standard Vite entry', () => {
    expect(indexSource).toContain("window.matchMedia('(prefers-color-scheme: dark)').matches");
    expect(indexSource).toContain("document.documentElement.classList.add('dark')");
    expect(indexSource).toContain('<script type="module" src="/src/main.tsx"></script>');
  });
});

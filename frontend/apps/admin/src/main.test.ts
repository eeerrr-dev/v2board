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
  });

  it('short-circuits the step-up 403 ahead of mutation error presentation', () => {
    // Privileged admin writes are mutations, so the MutationCache hook is the
    // step-up gate's primary trigger path — and no parity lane can cover it
    // (the Rust gate is production-only). The dialog must win before the raw
    // error toast.
    const mutationCacheBlock = mainSource.slice(
      mainSource.indexOf('mutationCache: new MutationCache'),
      mainSource.indexOf('queryCache: new QueryCache'),
    );
    expect(mutationCacheBlock).toContain('if (maybePromptStepUp(error)) return;');
    expect(mutationCacheBlock.indexOf('maybePromptStepUp(error)')).toBeLessThan(
      mutationCacheBlock.indexOf('presentMutationError'),
    );
  });

  it('scopes the global QueryCache hook to the step-up re-auth prompt', () => {
    // Query errors still belong to their route/query owners; the QueryCache
    // hook exists only so a step-up 403 on a sensitive admin GET opens the
    // re-auth dialog. It must never grow error presentation of its own.
    const queryCacheBlock = mainSource.slice(
      mainSource.indexOf('queryCache: new QueryCache'),
      mainSource.indexOf('defaultOptions'),
    );
    expect(queryCacheBlock).toContain('maybePromptStepUp(error)');
    expect(queryCacheBlock).not.toContain('toast');
    expect(queryCacheBlock).not.toContain('presentMutationError');
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

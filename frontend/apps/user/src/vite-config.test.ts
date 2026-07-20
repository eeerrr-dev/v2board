import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const viteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../vite.config.ts'),
  'utf8',
);
const deployViteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../vite.config.deploy.ts'),
  'utf8',
);
const sharedViteConfigSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../packages/config/src/vite.ts'),
  'utf8',
);
const userDevTemplateSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../index.html'),
  'utf8',
);
const buildDeploySource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../scripts/build-deploy.mjs'),
  'utf8',
);

const srcDir = dirname(fileURLToPath(import.meta.url));
const sharedUiSrcDir = join(srcDir, '../../../packages/ui/src');

// Collect the first capture group of every match, dropping the impossible
// undefined that noUncheckedIndexedAccess widens group access to.
function captures(source: string, pattern: RegExp): string[] {
  const out: string[] = [];
  for (const match of source.matchAll(pattern)) {
    if (match[1] !== undefined) out.push(match[1]);
  }
  return out;
}

// Every entry in the user app's optimizeDeps.include list, parsed from the
// config source so the exhaustiveness guard stays in lockstep with what ships.
const includeBlock = viteConfigSource.match(/include:\s*\[([\s\S]*?)\]/)?.[1] ?? '';
const includeSet = new Set<string>(captures(includeBlock, /['"]([^'"]+)['"]/g));

// A specifier needs pre-bundling only if it is an external package. Relative
// paths, the '@/' src alias, '@v2board/*' workspace packages, and node: builtins
// all resolve without entering the optimizeDeps set.
function isThirdPartyRuntime(specifier: string): boolean {
  return (
    !specifier.startsWith('.') &&
    !specifier.startsWith('@/') &&
    !specifier.startsWith('@v2board/') &&
    !specifier.startsWith('node:')
  );
}

// Collect every third-party specifier imported by non-test source under src/.
function collectSourceSpecifiers(dir: string, found: Set<string>): Set<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      // src/test holds test-only helpers, not shipped runtime.
      if (entry.name !== 'test') collectSourceSpecifiers(join(dir, entry.name), found);
      continue;
    }
    // Skip *.test.ts(x): they import react-dom/server, vitest, and other dev-only
    // specifiers that must never enter the noDiscovery pre-bundle set.
    if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) continue;
    const source = readFileSync(join(dir, entry.name), 'utf8');
    for (const pattern of [
      /\bfrom\s*['"]([^'"]+)['"]/g, // static import + re-export
      /\bimport\s+['"]([^'"]+)['"]/g, // side-effect import
      /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g, // dynamic import
    ]) {
      for (const specifier of captures(source, pattern)) {
        if (isThirdPartyRuntime(specifier)) found.add(specifier);
      }
    }
  }
  return found;
}

const appImportedThirdParty = collectSourceSpecifiers(srcDir, new Set<string>());
const sharedUiThirdParty = collectSourceSpecifiers(sharedUiSrcDir, new Set<string>());
const importedThirdParty = new Set(appImportedThirdParty);
for (const specifier of sharedUiThirdParty) {
  // Vite's nested-dependency syntax resolves packages owned by the linked UI
  // workspace without making each application redeclare UI internals.
  importedThirdParty.add(
    appImportedThirdParty.has(specifier) ? specifier : `@v2board/ui > ${specifier}`,
  );
}

// Include entries that never appear as a direct source import: the JSX and React
// Compiler runtimes the transform injects, the react-dom base entry (source
// imports the /client subpath, plus /server in tests), and axios, which reaches
// the bundle only transitively through @v2board/api-client.
const INJECTED_OR_TRANSITIVE = new Set([
  'react/jsx-runtime',
  'react/jsx-dev-runtime',
  'react/compiler-runtime',
  'react-dom',
  '@v2board/api-client > axios',
]);

describe('user Vite dev optimizer', () => {
  it('keeps user optimized deps isolated and fully declared for stable page clicks', () => {
    expect(viteConfigSource).toContain("cacheDir: '../../node_modules/.vite/user'");
    expect(viteConfigSource).toContain('optimizeDeps: {');
    // History routing (docs/api-dialect.md §10.1): the dev server relies on
    // Vite's default SPA fallback; the hash-redirect middleware is retired.
    expect(viteConfigSource).not.toContain('hashNavigationRedirectPlugin');
    expect(viteConfigSource).not.toContain('themeRuntimeAssetsPlugin()');
    expect(viteConfigSource).not.toContain('legacyThemePlugin()');
    expect(viteConfigSource).toContain('react()');
    expect(sharedViteConfigSource).toContain('hmr: true');
    expect(viteConfigSource).not.toContain('legacyViteClientStubPlugin');
    expect(viteConfigSource).not.toContain('stripViteClientPlugin');
    expect(viteConfigSource).toContain("'@v2board/api-client > axios'");
    expect(viteConfigSource).not.toContain("'axios'");
    expect(viteConfigSource).toContain("'react-dom'");
    expect(viteConfigSource).toContain("'react/jsx-dev-runtime'");
    expect(viteConfigSource).toContain("'react/jsx-runtime'");
    // The full include list is validated against the source imports by the
    // exhaustiveness guard below rather than a hand-maintained subset here.
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: false');
    expect(viteConfigSource).toContain('noDiscovery: true');
  });

  it('declares every third-party runtime import in optimizeDeps.include', () => {
    // noDiscovery makes this an explicit contract: source imports and the
    // pre-bundle list must agree in both directions.
    const undeclared = [...importedThirdParty].filter((s) => !includeSet.has(s)).sort();
    expect(undeclared).toEqual([]);

    const dead = [...includeSet]
      .filter((entry) => !importedThirdParty.has(entry) && !INJECTED_OR_TRANSITIVE.has(entry))
      .sort();
    expect(dead).toEqual([]);
  });

  it('uses the real Vite client and React Fast Refresh', () => {
    expect(sharedViteConfigSource).toContain('hmr: true');
    expect(sharedViteConfigSource).not.toContain('overlay: false');
    // No dev-server hash redirect survives history routing: deep path URLs
    // must reach the SPA fallback untouched (docs/api-dialect.md §10.1).
    expect(sharedViteConfigSource).not.toContain('hashNavigationRedirectPlugin');
    expect(sharedViteConfigSource).not.toContain('location: `/#');
    expect(sharedViteConfigSource).not.toContain("pathname.startsWith('/theme/')");
    expect(sharedViteConfigSource).not.toContain("pathname.startsWith('/monitor/')");
    expect(sharedViteConfigSource).not.toContain('export function stripViteClientPlugin()');
    expect(sharedViteConfigSource).not.toContain('export function legacyViteClientStubPlugin()');
    expect(sharedViteConfigSource).not.toContain(
      'export function rejectPackagedUserAssetsPlugin()',
    );
    expect(sharedViteConfigSource).not.toContain('export function themeRuntimeAssetsPlugin()');
    expect(sharedViteConfigSource).not.toContain('USER_THEME_PACKAGED_BUNDLE_ASSET.test(url)');
    expect(sharedViteConfigSource).not.toContain('components\\.chunk\\.css');
    expect(sharedViteConfigSource).not.toContain('(?:images|theme)');
    expect(sharedViteConfigSource).not.toContain('(?:i18n|images|static|theme)');
  });

  it('does not copy old packaged JS chunks into the user deploy output', () => {
    expect(deployViteConfigSource).not.toContain("'vendors.async.js'");
    expect(deployViteConfigSource).not.toContain("'components.async.js'");
    expect(deployViteConfigSource).not.toContain('componentsCss');
    expect(deployViteConfigSource).not.toContain('legacyCss');
    expect(deployViteConfigSource).not.toContain('readFileSync');
    expect(deployViteConfigSource).not.toContain('writeFileSync');
    expect(deployViteConfigSource).toContain('assetsInlineLimit: 0');
    expect(deployViteConfigSource).toContain('emptyOutDir: true');
    expect(deployViteConfigSource).toContain('cssCodeSplit: true');
    expect(deployViteConfigSource).toContain("manifest: 'manifest.json'");
    expect(deployViteConfigSource).toContain('modulePreload: { polyfill: false }');
    expect(deployViteConfigSource).toContain('rolldownOptions: {');
    expect(deployViteConfigSource).toContain("base: '/assets/user/'");
    expect(deployViteConfigSource).toContain(
      "input: path.resolve(import.meta.dirname, 'index.html')",
    );
    expect(deployViteConfigSource).toContain("entryFileNames: '[name]-[hash].js'");
    expect(deployViteConfigSource).toContain("chunkFileNames: '[name]-[hash].js'");
    expect(deployViteConfigSource).toContain("assetFileNames: 'asset-[hash][extname]'");
    expect(deployViteConfigSource).not.toContain('codeSplitting: false');
    expect(deployViteConfigSource).not.toContain("format: 'iife'");
    expect(deployViteConfigSource).not.toContain('rollupOptions: {');
    expect(deployViteConfigSource).not.toContain('inlineDynamicImports');
    expect(deployViteConfigSource).toContain('process.env.V2BOARD_DEPLOY_OUT_DIR');
    expect(deployViteConfigSource).toContain(
      'Deploy Vite config is internal; run the workspace pnpm build:deploy command',
    );
    expect(deployViteConfigSource).not.toContain("'../../dist-deploy");
    expect(deployViteConfigSource).not.toContain("'i18n'");
    expect(deployViteConfigSource).not.toContain("'static'");
    expect(deployViteConfigSource).not.toContain("'images'");
    expect(deployViteConfigSource).not.toContain("'theme'");
    expect(deployViteConfigSource).not.toContain("'env.example.js'");
    expect(deployViteConfigSource).not.toContain("'custom.css'");
    expect(deployViteConfigSource).not.toContain("'custom.js'");
    expect(deployViteConfigSource).not.toContain('public/theme/default/assets');
  });

  it('publishes a backend-neutral HTML entry with explicit runtime insertion points', () => {
    expect(userDevTemplateSource).toContain(
      '<script id="v2board-runtime-config" type="application/json">__V2BOARD_RUNTIME_CONFIG__</script>',
    );
    // docs/api-dialect.md §10.5: operator custom_html is removed — the only
    // injection point is the runtime-config data element above.
    expect(userDevTemplateSource).not.toContain('V2BOARD_CUSTOM_HTML');
    expect(userDevTemplateSource).not.toContain('window.settings');
    expect(userDevTemplateSource).not.toContain('custom.css');
    expect(userDevTemplateSource).not.toContain('custom.js');
    expect(buildDeploySource).toContain("const userStageOut = join(stageRoot, 'user')");
    expect(buildDeploySource).toContain('process.env.V2BOARD_DEPLOY_ROOT');
    expect(buildDeploySource).toContain("publicBase: '/assets/user/'");
    expect(buildDeploySource).toContain('const releaseName = `releases/${releaseId}`');
    expect(buildDeploySource).toContain('await publishReleaseLinks(target, releaseName)');
    expect(buildDeploySource).not.toContain('dashboard.blade.php');
    expect(buildDeploySource).not.toContain('backend/laravel');
  });

  it('applies the resolved theme before first paint to avoid a theme flash', () => {
    expect(userDevTemplateSource).not.toContain('catch (error) {}');
    expect(userDevTemplateSource).toContain(
      "if (separator === -1 || item.slice(0, separator) !== 'dark_mode') return value;",
    );
    // A 'system' / absent preference follows the OS before app CSS loads, so a
    // dark-OS visitor never flashes light.
    expect(userDevTemplateSource).toContain(
      "window.matchMedia('(prefers-color-scheme: dark)').matches",
    );
    expect(userDevTemplateSource.indexOf('if (dark) {')).toBeLessThan(
      userDevTemplateSource.indexOf('<script type="module"'),
    );
  });
});

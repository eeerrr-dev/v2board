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
const userDeployTemplateSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../deploy/dashboard.blade.php'),
  'utf8',
);

const srcDir = dirname(fileURLToPath(import.meta.url));

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

const importedThirdParty = collectSourceSpecifiers(srcDir, new Set<string>());

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
    expect(viteConfigSource).toContain(
      "cacheDir: '../../node_modules/.vite/user-white-screen-recovery-38'",
    );
    expect(viteConfigSource).toContain('optimizeDeps: {');
    expect(viteConfigSource).toContain('legacyNavigationRedirectPlugin()');
    expect(viteConfigSource).toContain('rejectPackagedUserAssetsPlugin()');
    expect(viteConfigSource).not.toContain('themeRuntimeAssetsPlugin()');
    expect(viteConfigSource).not.toContain('legacyThemePlugin()');
    // The redesigned user island runs real Vite HMR + React Fast Refresh, so the
    // @vite/client stub and strip plugins are dropped (admin still uses them via
    // the shared hmr:false default). noDiscovery keeps the dep graph stable.
    expect(viteConfigSource).toContain('react()');
    expect(viteConfigSource).toContain('hmr: true');
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
    // noDiscovery stops Vite from re-optimizing mid-session, so a third-party
    // import added without a matching include entry would trip a full re-optimize
    // and white-screen the HMR-disabled dev server. Assert the include set and
    // the source imports agree in both directions.
    const undeclared = [...importedThirdParty].filter((s) => !includeSet.has(s)).sort();
    expect(undeclared).toEqual([]);

    const dead = [...includeSet]
      .filter((entry) => !importedThirdParty.has(entry) && !INJECTED_OR_TRANSITIVE.has(entry))
      .sort();
    expect(dead).toEqual([]);
  });

  it('disables Vite HMR so open legacy pages are not half-refreshed while clicking', () => {
    expect(sharedViteConfigSource).toContain('hmr: false');
    expect(sharedViteConfigSource).not.toContain('overlay: false');
    expect(sharedViteConfigSource).toContain('export function legacyNavigationRedirectPlugin()');
    expect(sharedViteConfigSource).toContain('location: `/#${pathname}${url.search}`');
    expect(sharedViteConfigSource).toContain("'content-length': '0'");
    expect(sharedViteConfigSource).toContain('export function stripViteClientPlugin()');
    expect(sharedViteConfigSource).toContain('export function legacyViteClientStubPlugin()');
    expect(sharedViteConfigSource).toContain('export function rejectPackagedUserAssetsPlugin()');
    expect(sharedViteConfigSource).toContain("pathname.startsWith('/theme/default/assets/')");
    expect(sharedViteConfigSource).toContain("res.end('Not found')");
    expect(sharedViteConfigSource).not.toContain('export function themeRuntimeAssetsPlugin()');
    expect(sharedViteConfigSource).not.toContain('USER_THEME_PACKAGED_BUNDLE_ASSET.test(url)');
    expect(sharedViteConfigSource).not.toContain('components\\.chunk\\.css');
    expect(sharedViteConfigSource).not.toContain('(?:images|theme)');
    expect(sharedViteConfigSource).not.toContain('(?:i18n|images|static|theme)');
    expect(sharedViteConfigSource).toContain('export function updateStyle');
    expect(sharedViteConfigSource).toContain('export function createHotContext');
    expect(sharedViteConfigSource).toContain('export function injectQuery');
    expect(sharedViteConfigSource).toContain('export class ErrorOverlay');
    expect(sharedViteConfigSource).toContain('/@vite\\/client');
  });

  it('does not copy old packaged JS chunks into the user deploy output', () => {
    expect(deployViteConfigSource).not.toContain("'vendors.async.js'");
    expect(deployViteConfigSource).not.toContain("'components.async.js'");
    expect(deployViteConfigSource).not.toContain('componentsCss');
    expect(deployViteConfigSource).not.toContain('legacyCss');
    expect(deployViteConfigSource).not.toContain('readFileSync');
    expect(deployViteConfigSource).not.toContain('writeFileSync');
    expect(deployViteConfigSource).toContain('assetsInlineLimit: 0');
    expect(deployViteConfigSource).toContain('emptyOutDir: false');
    expect(deployViteConfigSource).toContain('chunkSizeWarningLimit: 1400');
    expect(deployViteConfigSource).toContain('rolldownOptions: {');
    expect(deployViteConfigSource).toContain('transform: {');
    expect(deployViteConfigSource).toContain("'import.meta': '{}'");
    expect(deployViteConfigSource).toContain('codeSplitting: false');
    expect(deployViteConfigSource).not.toContain('rollupOptions: {');
    expect(deployViteConfigSource).not.toContain('inlineDynamicImports');
    expect(deployViteConfigSource).toContain('process.env.V2BOARD_DEPLOY_OUT_DIR');
    expect(deployViteConfigSource).not.toContain("'i18n'");
    expect(deployViteConfigSource).not.toContain("'static'");
    expect(deployViteConfigSource).not.toContain("'images'");
    expect(deployViteConfigSource).not.toContain("'theme'");
    expect(deployViteConfigSource).not.toContain("'env.example.js'");
    expect(deployViteConfigSource).not.toContain("'custom.css'");
    expect(deployViteConfigSource).not.toContain("'custom.js'");
    expect(deployViteConfigSource).not.toContain('public/theme/default/assets');
  });

  it('cache-busts user deploy entry assets from file mtimes', () => {
    expect(userDeployTemplateSource).toContain('$assetVersion = function ($path) use ($version)');
    expect(userDeployTemplateSource).toContain('filemtime($assetPath)');
    expect(userDeployTemplateSource).toContain(
      '/assets/umi.css?v={{$assetVersion("theme/{$theme}/assets/umi.css")}}',
    );
    expect(userDeployTemplateSource).toContain(
      '/assets/umi.js?v={{$assetVersion("theme/{$theme}/assets/umi.js")}}',
    );
    expect(userDeployTemplateSource).not.toContain('/assets/umi.css?v={{$version}}');
    expect(userDeployTemplateSource).not.toContain('/assets/umi.js?v={{$version}}');
  });

  it('applies the resolved theme before first paint to avoid a theme flash', () => {
    expect(userDeployTemplateSource).toContain(
      "if (parts[0] !== 'dark_mode' || parts[1] === undefined) return value;",
    );
    expect(userDeployTemplateSource).toContain("document.documentElement.classList.add('dark');");
    expect(userDeployTemplateSource).toContain(
      "document.documentElement.style.colorScheme = 'dark';",
    );
    // A 'system' / absent preference follows the OS before umi.css loads, so a
    // dark-OS visitor never flashes light.
    expect(userDeployTemplateSource).toContain(
      "window.matchMedia('(prefers-color-scheme: dark)').matches",
    );
    expect(userDeployTemplateSource.indexOf('if (dark) {')).toBeLessThan(
      userDeployTemplateSource.indexOf('/assets/umi.css?v='),
    );
  });
});

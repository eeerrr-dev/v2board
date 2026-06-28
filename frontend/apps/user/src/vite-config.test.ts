import { readFileSync } from 'node:fs';
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

describe('user Vite dev optimizer', () => {
  it('keeps user optimized deps isolated and fully declared for stable page clicks', () => {
    expect(viteConfigSource).toContain(
      "cacheDir: '../../node_modules/.vite/user-white-screen-recovery-38'",
    );
    expect(viteConfigSource).toContain('optimizeDeps: {');
    expect(viteConfigSource).toContain('legacyNavigationRedirectPlugin()');
    expect(viteConfigSource).toContain('legacyViteClientStubPlugin()');
    expect(viteConfigSource).toContain('rejectPackagedUserAssetsPlugin()');
    expect(viteConfigSource).not.toContain('themeRuntimeAssetsPlugin()');
    expect(viteConfigSource).not.toContain('legacyThemePlugin()');
    expect(viteConfigSource).toContain('stripViteClientPlugin()');
    expect(viteConfigSource).toContain("'@v2board/api-client > axios'");
    expect(viteConfigSource).not.toContain("'axios'");
    expect(viteConfigSource).toContain("'react-dom'");
    expect(viteConfigSource).toContain("'react/jsx-dev-runtime'");
    expect(viteConfigSource).toContain("'react/jsx-runtime'");
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: false');
    expect(viteConfigSource).toContain('noDiscovery: true');
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

  it('applies the dark mode cookie before first paint to avoid a theme flash', () => {
    expect(userDeployTemplateSource).toContain(
      "if (parts[0] !== 'dark_mode' || parts[1] === undefined) return value;",
    );
    expect(userDeployTemplateSource).toContain("document.documentElement.classList.add('dark');");
    expect(userDeployTemplateSource).toContain(
      "document.documentElement.style.colorScheme = 'dark';",
    );
    expect(userDeployTemplateSource.indexOf("if (mode === '1') {")).toBeLessThan(
      userDeployTemplateSource.indexOf('/assets/umi.css?v='),
    );
  });
});

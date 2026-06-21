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
const buildDeploySource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../../../scripts/build-deploy.mjs'),
  'utf8',
);

describe('admin Vite dev optimizer', () => {
  it('keeps admin optimized deps isolated and fully declared for stable page clicks', () => {
    expect(viteConfigSource).toContain(
      "cacheDir: '../../node_modules/.vite/admin-white-screen-recovery-37'",
    );
    expect(viteConfigSource).toContain('optimizeDeps: {');
    expect(viteConfigSource).toContain('legacyNavigationRedirectPlugin()');
    expect(viteConfigSource).toContain('legacyViteClientStubPlugin()');
    expect(viteConfigSource).toContain('rejectPackagedAdminAssetsPlugin()');
    expect(viteConfigSource).toContain('stripViteClientPlugin()');
    expect(viteConfigSource).not.toContain('legacyAdminAssetsPlugin()');
    expect(viteConfigSource).toContain("'axios'");
    expect(viteConfigSource).toContain("'echarts/theme/vintage'");
    expect(viteConfigSource).toContain("'react-dom'");
    expect(viteConfigSource).toContain("'react/jsx-dev-runtime'");
    expect(viteConfigSource).toContain("'react/jsx-runtime'");
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: true');
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
    expect(sharedViteConfigSource).toContain('export function rejectPackagedAdminAssetsPlugin()');
    expect(sharedViteConfigSource).toContain("pathname.startsWith('/assets/admin/')");
    expect(sharedViteConfigSource).not.toContain('export function legacyAdminAssetsPlugin()');
    expect(sharedViteConfigSource).not.toContain('../../../public/assets/admin');
    expect(sharedViteConfigSource).toContain('export function updateStyle');
    expect(sharedViteConfigSource).toContain('export function createHotContext');
    expect(sharedViteConfigSource).toContain('export function injectQuery');
    expect(sharedViteConfigSource).toContain('export class ErrorOverlay');
    expect(sharedViteConfigSource).toContain('/@vite\\/client');
  });

  it('does not copy or concatenate old packaged admin assets into deploy output', () => {
    expect(deployViteConfigSource).toContain('assetsInlineLimit: 0');
    expect(deployViteConfigSource).toContain('emptyOutDir: false');
    expect(deployViteConfigSource).not.toContain('copyLegacyAdminAssets');
    expect(deployViteConfigSource).not.toContain('cpSync');
    expect(deployViteConfigSource).not.toContain('readFileSync');
    expect(deployViteConfigSource).not.toContain('writeFileSync');
    expect(deployViteConfigSource).not.toContain('../../../public/assets/admin');
    expect(deployViteConfigSource).not.toContain('components.chunk.css');
    expect(deployViteConfigSource).not.toContain('vendors.async.js');
    expect(deployViteConfigSource).not.toContain('components.async.js');
    expect(deployViteConfigSource).toContain('process.env.V2BOARD_DEPLOY_OUT_DIR');
  });

  it('deploys admin theme css from source-owned files without colliding with the entry css name', () => {
    expect(buildDeploySource).toContain("apps/admin/src/styles/themes");
    expect(buildDeploySource).toContain("mkdtemp(join(tmpdir(), 'v2board-deploy-'))");
    expect(buildDeploySource).toContain('V2BOARD_DEPLOY_OUT_DIR: userStageOut');
    expect(buildDeploySource).toContain('V2BOARD_DEPLOY_OUT_DIR: adminStageOut');
    expect(buildDeploySource).toContain('await rm(deployRoot, { recursive: true, force: true });');
    expect(buildDeploySource).toContain('await cp(userStageOut, userOut, { recursive: true });');
    expect(buildDeploySource).toContain('await cp(adminStageOut, adminOut, { recursive: true });');
    expect(buildDeploySource).toContain("resolve(adminOut, 'themes')");
    expect(buildDeploySource).toContain("['black.css', 'darkblue.css', 'default.css', 'green.css']");
    expect(buildDeploySource).toContain('const legacyRootFiles = [');
    expect(buildDeploySource).toContain("'components.chunk.css'");
    expect(buildDeploySource).toContain("'vendors.async.js'");
    expect(buildDeploySource).toContain("'components.async.js'");
    expect(buildDeploySource).toContain("'env.example.js'");
    expect(buildDeploySource).toContain("'custom.css'");
    expect(buildDeploySource).toContain("'custom.js'");
    expect(buildDeploySource).toContain("Unexpected legacy deploy artifact");
    expect(buildDeploySource).toContain('async function assertSingleRootEntryFile');
    expect(buildDeploySource).toContain(
      "await assertSingleRootEntryFile('User CSS', userOut, /^umi\\d*\\.css$/, 'umi.css');",
    );
    expect(buildDeploySource).toContain(
      "await assertSingleRootEntryFile('User JS', userOut, /^umi\\d*\\.js$/, 'umi.js');",
    );
    expect(buildDeploySource).toContain(
      "await assertSingleRootEntryFile('Admin CSS', adminOut, /^umi\\d*\\.css$/, 'umi.css');",
    );
    expect(buildDeploySource).toContain(
      "await assertSingleRootEntryFile('Admin JS', adminOut, /^umi\\d*\\.js$/, 'umi.js');",
    );
    expect(buildDeploySource).toContain('async function assertCssUrlsExist(label, cssFile)');
    expect(buildDeploySource).toContain('references missing deploy assets');
    expect(buildDeploySource).toContain("await assertCssUrlsExist('User entry CSS'");
    expect(buildDeploySource).toContain("await assertCssUrlsExist('Admin entry CSS'");
    expect(buildDeploySource).toContain("await assertCssUrlsExist(`Admin theme CSS ${name}`");
    expect(buildDeploySource).not.toContain('public/assets/admin/theme');
    expect(deployViteConfigSource).toContain("if (name.endsWith('.css')) return 'umi.css';");
  });

});

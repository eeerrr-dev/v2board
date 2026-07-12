import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const viteConfigSource = readFileSync(join(here, '../vite.config.ts'), 'utf8');
const deployViteConfigSource = readFileSync(join(here, '../vite.config.deploy.ts'), 'utf8');
const sharedViteConfigSource = readFileSync(
  join(here, '../../../packages/config/src/vite.ts'),
  'utf8',
);
const buildDeploySource = readFileSync(join(here, '../../../scripts/build-deploy.mjs'), 'utf8');
const adminDevTemplateSource = readFileSync(join(here, '../index.html'), 'utf8');
const packageJson = JSON.parse(readFileSync(join(here, '../package.json'), 'utf8')) as {
  dependencies: Record<string, string>;
  scripts: Record<string, string>;
};
const userPackageJson = JSON.parse(readFileSync(join(here, '../../user/package.json'), 'utf8')) as {
  scripts: Record<string, string>;
};
const workspacePackageJson = JSON.parse(
  readFileSync(join(here, '../../../package.json'), 'utf8'),
) as {
  scripts: Record<string, string>;
};

describe('admin Vite configuration', () => {
  it('uses the real Vite HMR client with an explicit dependency graph', () => {
    expect(viteConfigSource).toContain("cacheDir: '../../node_modules/.vite/admin'");
    expect(viteConfigSource).toContain('hashNavigationRedirectPlugin(');
    expect(viteConfigSource).not.toContain('legacyViteClientStubPlugin');
    expect(viteConfigSource).not.toContain('stripViteClientPlugin');
    expect(sharedViteConfigSource).toContain('hmr: true');
    expect(sharedViteConfigSource).not.toContain('export function legacyViteClientStubPlugin');
    expect(sharedViteConfigSource).not.toContain('export function stripViteClientPlugin');
    expect(sharedViteConfigSource).not.toContain('export function rejectPackagedAdminAssetsPlugin');
    expect(viteConfigSource).toContain("'@v2board/api-client > axios'");
    expect(viteConfigSource).toContain("'radix-ui'");
    expect(viteConfigSource).toContain("'react-is'");
    expect(viteConfigSource).toContain("'recharts'");
    expect(viteConfigSource).not.toContain("'echarts'");
    expect(viteConfigSource).not.toContain("'antd'");
    expect(viteConfigSource).toContain('holdUntilCrawlEnd: false');
    expect(viteConfigSource).toContain('noDiscovery: true');
    expect(sharedViteConfigSource).not.toContain('manualChunks');
    expect(sharedViteConfigSource).not.toContain('rollupOptions: {');
  });

  it('uses the shadcn Recharts v3 stack without retaining ECharts', () => {
    expect(packageJson.dependencies.recharts).toBe('^3.9.2');
    expect(packageJson.dependencies['react-is']).toBe('catalog:');
    expect(packageJson.dependencies).not.toHaveProperty('echarts');
  });

  it('emits manifest-driven hashed ESM chunks and split CSS', () => {
    expect(deployViteConfigSource).toContain("base: '/assets/admin/'");
    expect(deployViteConfigSource).toContain('cssCodeSplit: true');
    expect(deployViteConfigSource).toContain("manifest: 'manifest.json'");
    expect(deployViteConfigSource).toContain('modulePreload: { polyfill: false }');
    expect(deployViteConfigSource).toContain("input: path.resolve(import.meta.dirname, 'index.html')");
    expect(deployViteConfigSource).toContain("entryFileNames: '[name]-[hash].js'");
    expect(deployViteConfigSource).toContain("chunkFileNames: '[name]-[hash].js'");
    expect(deployViteConfigSource).toContain("assetFileNames: 'asset-[hash][extname]'");
    expect(deployViteConfigSource).not.toContain("format: 'iife'");
    expect(deployViteConfigSource).not.toContain('codeSplitting: false');
    expect(deployViteConfigSource).not.toContain('umi.js');
    expect(deployViteConfigSource).not.toContain('umi.css');
    expect(deployViteConfigSource).toContain(
      'Deploy Vite config is internal; run the workspace pnpm build:deploy command',
    );
    expect(deployViteConfigSource).not.toContain("'../../dist-deploy");
  });

  it('applies the dark-mode preference before loading development styles', () => {
    expect(adminDevTemplateSource).not.toContain('catch (error) {}');
    expect(adminDevTemplateSource).toContain(
      "if (separator === -1 || item.slice(0, separator) !== 'dark_mode') return value;",
    );
    expect(adminDevTemplateSource).toContain("document.documentElement.classList.add('dark');");
    expect(adminDevTemplateSource).toContain(
      "document.documentElement.style.colorScheme = 'dark';",
    );
    expect(adminDevTemplateSource.indexOf('if (dark) {')).toBeLessThan(
      adminDevTemplateSource.indexOf('<script type="module"'),
    );
    expect(adminDevTemplateSource).toContain(
      '<script id="v2board-runtime-config" type="application/json">__V2BOARD_RUNTIME_CONFIG__</script>',
    );
    expect(adminDevTemplateSource).not.toContain('window.settings');
  });

  it('validates every manifest asset and rejects old deploy artifacts', () => {
    expect(buildDeploySource).toContain(
      'pnpm --filter @v2board/user --filter @v2board/admin --parallel run typecheck',
    );
    expect(buildDeploySource).toContain('async function validateViteBuild');
    expect(buildDeploySource).toContain('manifest must contain exactly one entry');
    expect(buildDeploySource).toContain('manifest entry must reference JavaScript and CSS');
    expect(buildDeploySource).toContain('references missing deploy assets');
    expect(buildDeploySource).not.toContain('V2BOARD_DEPLOY_MODE');
    expect(buildDeploySource).not.toContain('finalizeOnly');
    expect(buildDeploySource).toContain("'umi.css'");
    expect(buildDeploySource).toContain("'umi.js'");
    expect(buildDeploySource).toContain("new Set(['i18n', 'images', 'theme', 'themes'])");
    expect(buildDeploySource).not.toContain('apps/admin/src/styles/themes');
    expect(buildDeploySource).not.toContain('adminThemeFiles');

    const validation = buildDeploySource.indexOf('const userBuild = await validateViteBuild({');
    const publish = buildDeploySource.indexOf(
      'await publishBuild(stageRoot, deployRoot, releaseId)',
    );
    expect(validation).toBeGreaterThan(-1);
    expect(publish).toBeGreaterThan(validation);
    expect(buildDeploySource).toContain('async function publishBuild(source, target, releaseId)');
    expect(buildDeploySource).toContain('await rename(pendingPath, releasePath)');
    expect(buildDeploySource).toContain("await replaceDeployLink(target, 'current', releaseName)");
    expect(buildDeploySource).toContain("await replaceDeployLink(target, 'previous', current)");
    expect(buildDeploySource).not.toContain(
      'await rm(deployRoot, { recursive: true, force: true })',
    );
  });

  it('exposes one canonical deploy command instead of app-level partial builds', () => {
    expect(workspacePackageJson.scripts['build:deploy']).toBe('node scripts/build-deploy.mjs');
    expect(packageJson.scripts).not.toHaveProperty('build:deploy');
    expect(userPackageJson.scripts).not.toHaveProperty('build:deploy');
  });
});

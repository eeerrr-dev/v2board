import { execSync } from 'node:child_process';
import { copyFile, cp, mkdir, mkdtemp, readFile, rm, readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';
import { tmpdir } from 'node:os';

const __dir = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dir, '..');
const deployRoot = resolve(root, 'dist-deploy');
const userThemeOut = resolve(root, 'dist-deploy/theme/default');
const userOut = resolve(root, 'dist-deploy/theme/default/assets');
const adminOut = resolve(root, 'dist-deploy/assets/admin');
const adminThemeSource = resolve(root, 'apps/admin/src/styles/themes');
const adminThemeOut = resolve(adminOut, 'themes');
const userDeployRoot = resolve(root, 'apps/user/deploy');
const adminThemeFiles = ['black.css', 'darkblue.css', 'default.css', 'green.css'];
const legacyRootFiles = [
  'components.chunk.css',
  'vendors.async.js',
  'components.async.js',
  'env.example.js',
  'custom.css',
  'custom.js',
];
const deployMode = process.env.V2BOARD_DEPLOY_MODE ?? 'all';
const finalizeOnly = deployMode === 'finalize';

if (!['all', 'finalize'].includes(deployMode)) {
  throw new Error(`Unsupported V2BOARD_DEPLOY_MODE: ${deployMode}`);
}

const stageRoot = finalizeOnly ? null : await mkdtemp(join(tmpdir(), 'v2board-deploy-'));
const userStageOut = stageRoot ? join(stageRoot, 'theme/default/assets') : userOut;
const adminStageOut = stageRoot ? join(stageRoot, 'assets/admin') : adminOut;

try {
  if (!finalizeOnly) {
    await rm(deployRoot, { recursive: true, force: true });

    execSync('pnpm -F @v2board/user exec vite build --config vite.config.deploy.ts', {
      cwd: root,
      env: { ...process.env, V2BOARD_DEPLOY_OUT_DIR: userStageOut },
      stdio: 'inherit',
    });
    execSync('pnpm -F @v2board/admin exec vite build --config vite.config.deploy.ts', {
      cwd: root,
      env: { ...process.env, V2BOARD_DEPLOY_OUT_DIR: adminStageOut },
      stdio: 'inherit',
    });

    await cp(userStageOut, userOut, { recursive: true });
    await cp(adminStageOut, adminOut, { recursive: true });
  }

  await mkdir(userThemeOut, { recursive: true });
  for (const name of ['dashboard.blade.php', 'config.json']) {
    await copyFile(join(userDeployRoot, name), join(userThemeOut, name));
  }

  await stat(join(userOut, 'umi.css'));
  await stat(join(userOut, 'umi.js'));

  await stat(join(adminOut, 'umi.css'));
  await stat(join(adminOut, 'umi.js'));

  await mkdir(adminThemeOut, { recursive: true });
  for (const name of adminThemeFiles) {
    await copyFile(join(adminThemeSource, name), join(adminThemeOut, name));
    await stat(join(adminThemeOut, name));
  }

  async function pathExists(path) {
    try {
      await stat(path);
      return true;
    } catch (error) {
      if (error?.code === 'ENOENT') return false;
      throw error;
    }
  }

  async function assertAbsent(path) {
    if (await pathExists(path)) {
      throw new Error(`Unexpected legacy deploy artifact: ${path}`);
    }
  }

  function normalizeCssUrl(rawUrl) {
    const url = rawUrl.trim().replace(/^['"]|['"]$/g, '');
    if (!url || url.startsWith('#')) return null;
    if (/^(?:data|blob|https?):/i.test(url) || url.startsWith('//')) return null;
    const path = url.split(/[?#]/, 1)[0];
    if (!path) return null;
    try {
      return decodeURIComponent(path);
    } catch {
      return path;
    }
  }

  async function assertCssUrlsExist(label, cssFile) {
    const css = await readFile(cssFile, 'utf8');
    const pattern = /url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*))\s*\)/g;
    const missing = [];

    for (const match of css.matchAll(pattern)) {
      const url = normalizeCssUrl(match[1] ?? match[2] ?? match[3] ?? '');
      if (!url) continue;

      const assetPath = url.startsWith('/')
        ? join(deployRoot, url.slice(1))
        : resolve(dirname(cssFile), url);
      if (!(await pathExists(assetPath))) {
        missing.push(url);
      }
    }

    if (missing.length) {
      throw new Error(`${label} references missing deploy assets: ${missing.join(', ')}`);
    }
  }

  for (const name of legacyRootFiles) {
    await assertAbsent(join(userOut, name));
    await assertAbsent(join(adminOut, name));
  }

  for (const name of ['i18n', 'images', 'theme']) {
    await assertAbsent(join(userOut, name));
  }
  await assertAbsent(join(adminOut, 'theme'));

  async function assertSingleRootEntryFile(label, dir, pattern, expectedName) {
    const matches = (await readdir(dir)).filter((name) => pattern.test(name)).sort();
    if (matches.length !== 1 || matches[0] !== expectedName) {
      throw new Error(
        `${label} deploy must expose exactly one root entry file named ${expectedName}: ${matches.join(', ')}`,
      );
    }
  }

  await assertSingleRootEntryFile('User CSS', userOut, /^umi\d*\.css$/, 'umi.css');
  await assertSingleRootEntryFile('User JS', userOut, /^umi\d*\.js$/, 'umi.js');
  await assertSingleRootEntryFile('Admin CSS', adminOut, /^umi\d*\.css$/, 'umi.css');
  await assertSingleRootEntryFile('Admin JS', adminOut, /^umi\d*\.js$/, 'umi.js');

  await assertCssUrlsExist('User entry CSS', join(userOut, 'umi.css'));
  await assertCssUrlsExist('Admin entry CSS', join(adminOut, 'umi.css'));
  for (const name of adminThemeFiles) {
    await assertCssUrlsExist(`Admin theme CSS ${name}`, join(adminThemeOut, name));
  }

  async function du(dir) {
    let total = 0;
    const entries = await readdir(dir, { withFileTypes: true });
    for (const e of entries) {
      const p = join(dir, e.name);
      if (e.isDirectory()) total += await du(p);
      else total += (await stat(p)).size;
    }
    return total;
  }

  const userSize = await du(userOut);
  const adminSize = await du(adminOut);

  console.log('\n=== Drop-in deployment build complete ===');
  console.log(`User theme:   ${(userSize / 1024).toFixed(1)} KB  →  public/theme/default/`);
  console.log(`Admin bundle: ${(adminSize / 1024).toFixed(1)} KB  →  public/assets/admin/`);
  console.log('\nDeploy:');
  console.log(`  rsync -a --delete ${userThemeOut}/ /path/to/v2board/public/theme/default/`);
  console.log(`  rsync -a --delete ${adminOut}/ /path/to/v2board/public/assets/admin/`);
} finally {
  if (stageRoot) {
    await rm(stageRoot, { recursive: true, force: true });
  }
}

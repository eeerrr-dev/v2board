import { execSync } from 'node:child_process';
import { copyFile, writeFile, mkdir, rm, readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, join } from 'node:path';

const __dir = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dir, '..');
const userThemeOut = resolve(root, 'dist-deploy/theme/default');
const userOut = resolve(root, 'dist-deploy/theme/default/assets');
const adminOut = resolve(root, 'dist-deploy/assets/admin');
const legacyThemeRoot = resolve(root, '../public/theme/default');

await rm(resolve(root, 'dist-deploy'), { recursive: true, force: true });

execSync('pnpm -F @v2board/user run build:deploy', { cwd: root, stdio: 'inherit' });
execSync('pnpm -F @v2board/admin run build:deploy', { cwd: root, stdio: 'inherit' });

await mkdir(userThemeOut, { recursive: true });
for (const name of ['dashboard.blade.php', 'config.json']) {
  await copyFile(join(legacyThemeRoot, name), join(userThemeOut, name));
}

await stat(join(userOut, 'components.async.js')).catch(async () => {
  await writeFile(join(userOut, 'components.async.js'), '');
});
await stat(join(userOut, 'vendors.async.js')).catch(async () => {
  await writeFile(join(userOut, 'vendors.async.js'), '');
});
await writeFile(join(adminOut, 'components.async.js'), '');
await writeFile(join(adminOut, 'vendors.async.js'), '');

await stat(join(userOut, 'components.chunk.css')).catch(async () => {
  await writeFile(join(userOut, 'components.chunk.css'), '');
});
await writeFile(join(adminOut, 'components.chunk.css'), '');

await stat(join(adminOut, 'umi.css')).catch(async () => {
  await writeFile(join(adminOut, 'umi.css'), '');
});

await stat(join(userOut, 'env.example.js')).catch(async () => {
  await writeFile(join(userOut, 'env.example.js'), '');
});
const i18nDir = join(userOut, 'i18n');
await mkdir(i18nDir, { recursive: true });
for (const l of ['zh-CN', 'en-US', 'zh-TW', 'ja-JP', 'vi-VN', 'ko-KR', 'fa-IR']) {
  await stat(join(i18nDir, `${l}.js`)).catch(async () => {
    await writeFile(join(i18nDir, `${l}.js`), '');
  });
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
console.log(`  cp -R ${userThemeOut}/* /path/to/v2board/public/theme/default/`);
console.log(`  cp -R ${adminOut}/* /path/to/v2board/public/assets/admin/`);

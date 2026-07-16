import { execSync, spawn } from 'node:child_process';
import {
  chmod,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat,
} from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { brotliCompressSync, gzipSync } from 'node:zlib';
import { releaseIdFromBuilds } from './release-content-id.mjs';
import { publishReleaseLinks } from './release-publication.mjs';

const __dir = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dir, '..');
const deployRoot = process.env.V2BOARD_DEPLOY_ROOT
  ? resolve(process.env.V2BOARD_DEPLOY_ROOT)
  : resolve(root, 'dist-deploy');
const releasesRoot = join(deployRoot, 'releases');
const forbiddenLegacyNames = new Set([
  'components.chunk.css',
  'vendors.async.js',
  'components.async.js',
  'custom.css',
  'custom.js',
  'env.example.js',
  'umi.css',
  'umi.js',
]);
const forbiddenLegacyDirectories = new Set(['i18n', 'images', 'theme', 'themes']);
const runtimeConfigToken = '__V2BOARD_RUNTIME_CONFIG__';

await mkdir(releasesRoot, { recursive: true });
const stageRoot = await mkdtemp(join(releasesRoot, '.build-'));
const userStageOut = join(stageRoot, 'user');
const adminStageOut = join(stageRoot, 'admin');

try {
  execSync('pnpm --filter @v2board/user --filter @v2board/admin --parallel run typecheck', {
    cwd: root,
    stdio: 'inherit',
  });

  // The two app builds share no state and write to disjoint stage dirs, so
  // they run concurrently; output is buffered per build to stay attributable.
  await Promise.all([
    runViteBuild('@v2board/user', userStageOut),
    runViteBuild('@v2board/admin', adminStageOut),
  ]);

  const userBuild = await validateViteBuild({
    label: 'User',
    outDir: userStageOut,
    publicBase: '/assets/user/',
    requiresCustomHtmlMarker: true,
  });
  const adminBuild = await validateViteBuild({
    label: 'Admin',
    outDir: adminStageOut,
    publicBase: '/assets/admin/',
    requiresCustomHtmlMarker: false,
  });

  for (const dir of [userStageOut, adminStageOut]) await rejectLegacyArtifacts(dir);

  const userSize = await du(userStageOut);
  const adminSize = await du(adminStageOut);
  const releaseId = await releaseIdFromBuilds([
    { name: 'user', outDir: userStageOut, files: userBuild.validatedFiles },
    { name: 'admin', outDir: adminStageOut, files: adminBuild.validatedFiles },
  ]);
  await normalizeReleasePermissions(stageRoot);
  const publication = await publishBuild(stageRoot, deployRoot, releaseId);

  console.log('\n=== Immutable frontend release complete ===');
  console.log(
    `User:  ${(userSize / 1024).toFixed(1)} KB, ${userBuild.files} verified files, ` +
      `${(userBuild.initialGzip / 1024).toFixed(1)} KB initial gzip → current/user/`,
  );
  console.log(
    `Admin: ${(adminSize / 1024).toFixed(1)} KB, ${adminBuild.files} verified files, ` +
      `${(adminBuild.initialGzip / 1024).toFixed(1)} KB initial gzip → current/admin/`,
  );
  console.log(`Release: ${publication.release}`);
  if (publication.previous) console.log(`Previous: ${publication.previous}`);
} finally {
  await rm(stageRoot, { recursive: true, force: true });
}

function runViteBuild(filter, outDir) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(
      'pnpm',
      ['-F', filter, 'exec', 'vite', 'build', '--config', 'vite.config.deploy.ts'],
      {
        cwd: root,
        env: { ...process.env, V2BOARD_DEPLOY_OUT_DIR: outDir },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    );
    const output = [];
    child.stdout.on('data', (chunk) => output.push(chunk));
    child.stderr.on('data', (chunk) => output.push(chunk));
    child.on('error', rejectPromise);
    child.on('close', (code) => {
      process.stdout.write(`\n=== vite build ${filter} ===\n`);
      process.stdout.write(Buffer.concat(output));
      if (code === 0) resolvePromise();
      else rejectPromise(new Error(`vite build for ${filter} exited with code ${code}`));
    });
  });
}

async function validateViteBuild({ label, outDir, publicBase, requiresCustomHtmlMarker }) {
  const manifestPath = join(outDir, 'manifest.json');
  const manifestSource = await readFile(manifestPath, 'utf8');
  const manifest = JSON.parse(manifestSource);
  if (!isRecord(manifest)) throw new Error(`${label} manifest must be a JSON object`);

  const chunks = Object.entries(manifest);
  const entries = chunks.filter(([, chunk]) => isRecord(chunk) && chunk.isEntry === true);
  if (entries.length !== 1) {
    throw new Error(`${label} manifest must contain exactly one entry, found ${entries.length}`);
  }

  const referencedFiles = new Set(['index.html', 'manifest.json']);
  const visitedChunks = new Set();

  async function visitChunk(key, ancestry = []) {
    if (visitedChunks.has(key)) return;
    const chunk = manifest[key];
    if (!isRecord(chunk)) {
      throw new Error(`${label} manifest import does not resolve to a chunk: ${key}`);
    }
    if (ancestry.includes(key)) {
      throw new Error(
        `${label} manifest contains an import cycle: ${[...ancestry, key].join(' -> ')}`,
      );
    }

    visitedChunks.add(key);
    await validateManifestAsset(`${label} chunk ${key}`, chunk.file, outDir, referencedFiles);
    for (const field of ['css', 'assets']) {
      const values = chunk[field] ?? [];
      if (!Array.isArray(values))
        throw new Error(`${label} manifest ${key}.${field} must be an array`);
      for (const asset of values) {
        await validateManifestAsset(
          `${label} chunk ${key}.${field}`,
          asset,
          outDir,
          referencedFiles,
        );
      }
    }
    for (const field of ['imports', 'dynamicImports']) {
      const imports = chunk[field] ?? [];
      if (!Array.isArray(imports)) {
        throw new Error(`${label} manifest ${key}.${field} must be an array`);
      }
      for (const importedKey of imports) {
        if (typeof importedKey !== 'string' || !Object.hasOwn(manifest, importedKey)) {
          throw new Error(`${label} manifest ${key}.${field} has an unsafe reference`);
        }
        await visitChunk(importedKey, [...ancestry, key]);
      }
    }
  }

  await visitChunk(entries[0][0]);
  // Vite also records source-imported images and fonts as standalone manifest
  // entries. They are not JavaScript imports, but they still need the same
  // path/hash/existence validation as the entry's recursive chunk graph.
  for (const [key] of chunks) {
    if (!visitedChunks.has(key)) await visitChunk(key);
  }

  const entry = entries[0][1];
  if (typeof entry.file !== 'string' || !entry.file.endsWith('.js') || !(entry.css?.length > 0)) {
    throw new Error(`${label} manifest entry must reference JavaScript and CSS`);
  }

  const indexPath = join(outDir, 'index.html');
  const indexSource = await readFile(indexPath, 'utf8');
  assertExactlyOnce(
    indexSource,
    `<script id="v2board-runtime-config" type="application/json">${runtimeConfigToken}</script>`,
    `${label} runtime config bootstrap`,
  );
  if (requiresCustomHtmlMarker) {
    assertExactlyOnce(indexSource, '<!-- V2BOARD_CUSTOM_HTML -->', `${label} custom HTML marker`);
    const rootPosition = indexSource.indexOf('<div id="root"></div>');
    const markerPosition = indexSource.indexOf('<!-- V2BOARD_CUSTOM_HTML -->');
    if (rootPosition === -1 || markerPosition < rootPosition) {
      throw new Error(`${label} custom HTML marker must follow #root`);
    }
  }
  if (indexSource.includes('window.settings')) {
    throw new Error(`${label} index.html retains the retired window.settings bootstrap`);
  }

  const htmlAssets = await validateHtmlAssets(label, indexSource, outDir, publicBase);
  for (const asset of [entry.file, ...(entry.css ?? [])]) {
    const expectedUrl = `${publicBase}${asset}`;
    if (!htmlAssets.has(expectedUrl)) {
      throw new Error(`${label} index.html does not load manifest entry asset: ${expectedUrl}`);
    }
  }

  for (const asset of referencedFiles) {
    if (!asset.endsWith('.css')) continue;
    await assertCssUrlsExist(`${label} CSS ${asset}`, join(outDir, asset), outDir, publicBase);
  }

  const initialBundle = await measureInitialJavaScript(label, manifest, entries[0][0], outDir);
  const defaultBudget = 300 * 1024;
  const budgetVariable = `V2BOARD_${label.toUpperCase()}_INITIAL_JS_GZIP_BUDGET`;
  const initialGzipBudget = Number(process.env[budgetVariable] ?? defaultBudget);
  if (!Number.isFinite(initialGzipBudget) || initialGzipBudget <= 0) {
    throw new Error(`${budgetVariable} must be a positive byte count`);
  }
  if (initialBundle.gzip > initialGzipBudget) {
    throw new Error(
      `${label} initial JavaScript is ${(initialBundle.gzip / 1024).toFixed(1)} KB gzip, ` +
        `above the ${(initialGzipBudget / 1024).toFixed(1)} KB budget`,
    );
  }

  const validatedFiles = [...referencedFiles].sort();
  const outputFiles = await collectBuildFiles(outDir);
  if (
    outputFiles.length !== validatedFiles.length ||
    outputFiles.some((path, index) => path !== validatedFiles[index])
  ) {
    const expected = new Set(validatedFiles);
    const actual = new Set(outputFiles);
    const unverified = outputFiles.filter((path) => !expected.has(path));
    const missing = validatedFiles.filter((path) => !actual.has(path));
    throw new Error(
      `${label} output does not exactly match its verified manifest graph` +
        ` (unverified: ${unverified.join(', ') || 'none'}; missing: ${missing.join(', ') || 'none'})`,
    );
  }

  return {
    files: validatedFiles.length,
    initialGzip: initialBundle.gzip,
    initialBrotli: initialBundle.brotli,
    validatedFiles,
  };
}

async function measureInitialJavaScript(label, manifest, entryKey, outDir) {
  const visited = new Set();
  const files = new Set();

  function visit(key) {
    if (visited.has(key)) return;
    visited.add(key);
    const chunk = manifest[key];
    if (!isRecord(chunk)) throw new Error(`${label} initial import is missing: ${key}`);
    if (typeof chunk.file === 'string' && chunk.file.endsWith('.js')) files.add(chunk.file);
    const imports = chunk.imports ?? [];
    if (!Array.isArray(imports))
      throw new Error(`${label} manifest ${key}.imports must be an array`);
    for (const importedKey of imports) visit(importedKey);
  }

  visit(entryKey);
  let gzip = 0;
  let brotli = 0;
  for (const file of files) {
    const source = await readFile(join(outDir, safeAssetPath(`${label} initial chunk`, file)));
    gzip += gzipSync(source).byteLength;
    brotli += brotliCompressSync(source).byteLength;
  }
  return { gzip, brotli };
}

async function validateManifestAsset(label, value, outDir, referencedFiles) {
  const asset = safeAssetPath(label, value);
  await assertRegularFile(join(outDir, asset), `${label} references a missing file: ${asset}`);
  referencedFiles.add(asset);
}

function safeAssetPath(label, value) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must reference a non-empty asset path`);
  }
  if (value.includes('\\') || value.includes('?') || value.includes('#')) {
    throw new Error(`${label} contains an unsafe asset path: ${value}`);
  }

  let decoded;
  try {
    decoded = decodeURIComponent(value);
  } catch {
    throw new Error(`${label} contains invalid URL encoding: ${value}`);
  }
  const segments = decoded.split('/');
  if (
    decoded !== value ||
    decoded.startsWith('/') ||
    segments.some((segment) => !segment || segment === '.' || segment === '..') ||
    segments.length !== 1 ||
    !/^[A-Za-z0-9._-]+-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9.]+$/.test(decoded)
  ) {
    throw new Error(`${label} contains a non-hashed or unsafe asset path: ${value}`);
  }
  if (forbiddenLegacyNames.has(basename(decoded))) {
    throw new Error(`${label} references forbidden legacy asset: ${value}`);
  }
  return decoded;
}

async function validateHtmlAssets(label, html, outDir, publicBase) {
  const urls = new Set();
  const pattern = /\b(?:href|src)=(?:"([^"]+)"|'([^']+)')/g;
  for (const match of html.matchAll(pattern)) {
    const value = match[1] ?? match[2];
    if (!value || value.startsWith('#') || /^(?:data|https?):/i.test(value)) continue;
    if (!value.startsWith(publicBase)) {
      throw new Error(`${label} index.html contains an asset outside ${publicBase}: ${value}`);
    }
    const asset = safeAssetPath(`${label} index.html`, value.slice(publicBase.length));
    await assertRegularFile(
      join(outDir, asset),
      `${label} index.html references a missing file: ${value}`,
    );
    urls.add(value);
  }
  return urls;
}

async function publishBuild(source, target, releaseId) {
  const releaseName = `releases/${releaseId}`;
  const releasePath = join(target, releaseName);
  const pendingPath = `${releasePath}.tmp`;

  if (!(await pathExists(releasePath))) {
    await rm(pendingPath, { recursive: true, force: true });
    await rename(source, pendingPath);
    await rename(pendingPath, releasePath);
  }
  await normalizeReleasePermissions(releasePath);

  const previous = await publishReleaseLinks(target, releaseName);
  return { release: releaseName, previous };
}

async function normalizeReleasePermissions(directory) {
  await chmod(directory, 0o755);
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      await normalizeReleasePermissions(path);
    } else if (entry.isFile()) {
      await chmod(path, 0o644);
    } else {
      throw new Error(`Deploy release contains an unsupported filesystem entry: ${path}`);
    }
  }
}

async function rejectLegacyArtifacts(directory, rootDirectory = directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    const relativePath = relative(rootDirectory, path).split(sep);
    if (forbiddenLegacyNames.has(entry.name)) {
      throw new Error(`Unexpected legacy deploy artifact: ${path}`);
    }
    if (entry.isDirectory() && relativePath.some((part) => forbiddenLegacyDirectories.has(part))) {
      throw new Error(`Unexpected legacy deploy directory: ${path}`);
    }
    if (entry.isDirectory()) await rejectLegacyArtifacts(path, rootDirectory);
  }
}

async function collectBuildFiles(directory, rootDirectory = directory, files = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      await collectBuildFiles(path, rootDirectory, files);
    } else if (entry.isFile()) {
      files.push(relative(rootDirectory, path).split(sep).join('/'));
    } else {
      throw new Error(`Deploy build contains an unsupported filesystem entry: ${path}`);
    }
  }
  return files.sort();
}

function assertExactlyOnce(source, value, label) {
  const first = source.indexOf(value);
  if (first === -1 || source.indexOf(value, first + value.length) !== -1) {
    throw new Error(`${label} must appear exactly once`);
  }
}

function normalizeCssUrl(rawUrl) {
  const url = rawUrl.trim().replace(/^['"]|['"]$/g, '');
  if (!url || url.startsWith('#')) return null;
  if (/^(?:data|blob|https?):/i.test(url) || url.startsWith('//')) return null;
  return url.split(/[?#]/, 1)[0] || null;
}

async function assertCssUrlsExist(label, cssFile, outDir, publicBase) {
  const css = await readFile(cssFile, 'utf8');
  const pattern = /url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*))\s*\)/g;
  const missing = [];

  for (const match of css.matchAll(pattern)) {
    const url = normalizeCssUrl(match[1] ?? match[2] ?? match[3] ?? '');
    if (!url) continue;

    let assetPath;
    if (url.startsWith('/')) {
      if (!url.startsWith(publicBase)) {
        throw new Error(`${label} references an asset outside ${publicBase}: ${url}`);
      }
      const asset = safeAssetPath(label, url.slice(publicBase.length));
      assetPath = join(outDir, asset);
    } else {
      let decoded;
      try {
        decoded = decodeURIComponent(url);
      } catch {
        throw new Error(`${label} contains invalid URL encoding: ${url}`);
      }
      assetPath = resolve(dirname(cssFile), decoded);
      const relativePath = relative(outDir, assetPath);
      if (relativePath.startsWith('..') || relativePath.includes(`..${sep}`)) {
        throw new Error(`${label} contains an unsafe relative URL: ${url}`);
      }
    }
    if (!(await isRegularFile(assetPath))) missing.push(url);
  }

  if (missing.length)
    throw new Error(`${label} references missing deploy assets: ${missing.join(', ')}`);
}

async function pathExists(path) {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
}

async function isRegularFile(path) {
  try {
    return (await stat(path)).isFile();
  } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
}

async function assertRegularFile(path, message) {
  if (!(await isRegularFile(path))) throw new Error(message);
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

async function du(dir) {
  let total = 0;
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    total += entry.isDirectory() ? await du(path) : (await stat(path)).size;
  }
  return total;
}

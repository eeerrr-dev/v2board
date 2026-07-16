import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { extname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { forbiddenLegacyNames } from './deploy-contract.mjs';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const repositoryRoot = resolve(frontendRoot, '..');
const roots = [
  'apps/user/src',
  'apps/admin/src',
  'packages/api-client/src',
  'packages/config/src',
  'packages/i18n/src',
  'packages/types/src',
];
const referenceAssetRoots = [
  'references/wyx2685-v2board/public/theme/default/assets',
  'references/wyx2685-v2board/public/assets/admin',
];
const standaloneFiles = [
  'apps/user/index.html',
  'apps/user/vite.config.ts',
  'apps/user/vite.config.deploy.ts',
  'apps/admin/index.html',
  'apps/admin/vite.config.ts',
  'apps/admin/vite.config.deploy.ts',
];
const sourceExtensions = new Set(['.css', '.html', '.ts', '.tsx']);

function isRuntimeFile(path) {
  return (
    !path.endsWith('.d.ts') &&
    !/\.(?:behavior\.)?test\.[cm]?[jt]sx?$/.test(path) &&
    !path.split('/').includes('test')
  );
}

function isRuntimeSource(path) {
  return isRuntimeFile(path) && sourceExtensions.has(extname(path));
}

async function collectFiles(directory, files = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await collectFiles(absolutePath, files);
    } else {
      const path = relative(frontendRoot, absolutePath).split(sep).join('/');
      if (isRuntimeFile(path)) files.push(path);
    }
  }
  return files;
}

async function collectAbsoluteFiles(directory, files = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await collectAbsoluteFiles(absolutePath, files);
    } else {
      files.push(absolutePath);
    }
  }
  return files;
}

async function sha256(path) {
  return createHash('sha256')
    .update(await readFile(path))
    .digest('hex');
}

const rules = [
  {
    label: 'retired UI package import',
    pattern:
      /(?:from\s*|import\s*\(|import\s+)['"](?:antd(?:\/|['"])|@ant-design\/|antd-mobile(?:\/|['"])|bootstrap(?:\/|['"])|react-bootstrap(?:\/|['"])|@fortawesome\/|font-awesome(?:\/|['"])|rc-switch(?:\/|['"])|slick-carousel(?:\/|['"]))/,
  },
  {
    label: 'retired Ant Design or antd-mobile class',
    pattern: /\b(?:ant-[a-z]|anticon\b|am-list\b)/,
  },
  {
    label: 'retired Bootstrap or OneUI class',
    pattern:
      /\b(?:block-(?:content|rounded|title|mode-loading)|form-control|btn-(?:block|primary)|list-group(?:-item)?|bg-gray-lighter|nav-main-link)\b/,
  },
  {
    label: 'retired icon class',
    pattern: /\b(?:fa fa-|si si-)/,
  },
  {
    label: 'retired packaged frontend asset',
    pattern: new RegExp(
      `(?:/theme/default/assets|\\b(?:${forbiddenLegacyNames
        .map((name) => name.replaceAll('.', '\\.'))
        .join('|')})\\b)`,
    ),
  },
  {
    label: 'retired runtime settings injection',
    pattern: /\bwindow\.settings\b/,
  },
  {
    label: 'retired Laravel runtime path',
    pattern: /\bbackend\/laravel\b/,
  },
  {
    label: 'retired Stripe CardElement/token flow',
    pattern: /\b(?:CardElement|createToken)\b/,
  },
  {
    label: 'retired theme-package or Stripe-token API',
    pattern:
      /\/(?:config\/getThemeTemplate|theme\/(?:getThemes|getThemeConfig|saveThemeConfig)|user\/comm\/getStripePublicKey)\b/,
  },
  {
    label: 'non-canonical shadcn dialog module name',
    pattern: /\bshadcn-dialog\b/,
  },
  {
    label: 'retired admin UI habit persistence',
    pattern: /\b(?:LEGACY_HABIT_KEY|server_manage_page_size)\b/,
  },
];

const runtimeFiles = [
  ...(await Promise.all(roots.map((root) => collectFiles(resolve(frontendRoot, root))))).flat(),
  ...standaloneFiles,
].sort();
const files = runtimeFiles.filter(isRuntimeSource);
const violations = [];

let referenceAssetFiles = [];
try {
  referenceAssetFiles = (
    await Promise.all(
      referenceAssetRoots.map((root) => collectAbsoluteFiles(resolve(repositoryRoot, root))),
    )
  ).flat();
} catch (error) {
  if (error?.code !== 'ENOENT') throw error;
  violations.push(
    'references/wyx2685-v2board: pinned reference assets are unavailable; run `git submodule update --init --recursive` before the source audit',
  );
}

const referencePathsByDigest = new Map();
for (const path of referenceAssetFiles) {
  const digest = await sha256(path);
  const paths = referencePathsByDigest.get(digest) ?? [];
  paths.push(relative(repositoryRoot, path).split(sep).join('/'));
  referencePathsByDigest.set(digest, paths);
}

for (const path of runtimeFiles) {
  const digest = await sha256(resolve(frontendRoot, path));
  const matchingReferencePaths = referencePathsByDigest.get(digest);
  if (matchingReferencePaths === undefined) continue;
  violations.push(
    `${path}: copied pinned-reference asset (sha256 ${digest}) also exists at ${matchingReferencePaths.join(', ')}`,
  );
}

for (const path of files) {
  const contents = await readFile(resolve(frontendRoot, path), 'utf8');
  for (const [index, line] of contents.split('\n').entries()) {
    for (const rule of rules) {
      if (rule.pattern.test(line)) {
        violations.push(`${path}:${index + 1}: ${rule.label}: ${line.trim()}`);
      }
    }
  }
}

if (violations.length > 0) {
  console.error('Frontend source isolation audit failed:\n');
  console.error(violations.join('\n'));
  process.exitCode = 1;
} else {
  console.log(
    `Frontend source isolation audit passed (${files.length} text sources and ${runtimeFiles.length - files.length} binary assets).`,
  );
}

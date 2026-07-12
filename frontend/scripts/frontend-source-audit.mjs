import { readdir, readFile } from 'node:fs/promises';
import { extname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const frontendRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const roots = [
  'apps/user/src',
  'apps/admin/src',
  'packages/api-client/src',
  'packages/config/src',
  'packages/i18n/src',
  'packages/types/src',
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

function isRuntimeSource(path) {
  return (
    sourceExtensions.has(extname(path)) &&
    !path.endsWith('.d.ts') &&
    !/\.(?:behavior\.)?test\.[cm]?[jt]sx?$/.test(path) &&
    !path.split('/').includes('test')
  );
}

async function collectFiles(directory, files = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolutePath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await collectFiles(absolutePath, files);
    } else {
      const path = relative(frontendRoot, absolutePath).split(sep).join('/');
      if (isRuntimeSource(path)) files.push(path);
    }
  }
  return files;
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
    pattern:
      /(?:\/theme\/default\/assets|\bumi\.(?:js|css)\b|components\.chunk\.css|vendors\.async\.js|components\.async\.js)/,
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

const files = [
  ...(await Promise.all(roots.map((root) => collectFiles(resolve(frontendRoot, root))))).flat(),
  ...standaloneFiles,
].sort();
const violations = [];

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
  console.log(`Frontend source isolation audit passed (${files.length} runtime source files).`);
}

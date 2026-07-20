#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP_SPECIFIC_UI = {
  admin: ['chart.tsx', 'input-group.tsx', 'tags-input.tsx'],
  user: ['carousel.tsx', 'progress.tsx', 'radio-group.tsx'],
};

const APP_SPECIFIC_UI_TESTS = {
  admin: ['tags-input.test.tsx'],
  user: ['carousel.test.tsx'],
};

const SHARED_UI = [
  'alert-dialog.tsx',
  'alert.tsx',
  'avatar.tsx',
  'badge.tsx',
  'button.tsx',
  'card.tsx',
  'checkbox.tsx',
  'confirm-dialog.tsx',
  'dialog-surface.ts',
  'dialog.tsx',
  'dropdown-menu.tsx',
  'error-state.tsx',
  'field.tsx',
  'header-tooltip.tsx',
  'input.tsx',
  'label.tsx',
  'loading-state.tsx',
  'page.tsx',
  'pagination.tsx',
  'segmented-control.tsx',
  'select.tsx',
  'separator.tsx',
  'sheet.tsx',
  'sidebar.tsx',
  'skeleton.tsx',
  'spinner.tsx',
  'status-badge.tsx',
  'switch.tsx',
  'table.tsx',
  'textarea.tsx',
  'toaster.tsx',
  'tooltip.tsx',
].sort();

const SHARED_STYLES = ['shadcn.css', 'theme.css'];

const SHARED_UI_TESTS = [
  'button.test.tsx',
  'card.test.tsx',
  'checkbox.test.tsx',
  'confirm-dialog.behavior.test.tsx',
  'confirm-dialog.test.ts',
  'field.test.tsx',
  'header-tooltip.test.tsx',
  'input.test.tsx',
  'loading-state.test.tsx',
  'select.test.tsx',
  'switch.test.tsx',
  'table.test.tsx',
  'table.virtualizer.test.tsx',
  'textarea.test.tsx',
  'toaster.test.ts',
].sort();

const SHARED_HOOK_TESTS = ['use-mobile.test.tsx'];
const SHARED_LIB_TESTS = ['dark-mode.test.ts'];
const SHARED_HOOK_TEST_SUBJECTS = ['use-mobile'];
const APP_FORBIDDEN_LIB_TEST_SUBJECTS = ['dark-mode', 'toast'];

export async function auditUiSync(projectRoot = getDefaultProjectRoot()) {
  const uiRoot = resolve(projectRoot, 'frontend/packages/ui/src');
  const userRoot = resolve(projectRoot, 'frontend/apps/user');
  const adminRoot = resolve(projectRoot, 'frontend/apps/admin');
  const [
    sharedFiles,
    sharedStyles,
    sharedUiTests,
    sharedHookTests,
    sharedLibTests,
    userFiles,
    adminFiles,
    userUiTests,
    adminUiTests,
    userHookTests,
    adminHookTests,
    userLibTests,
    adminLibTests,
    userGlobals,
    adminGlobals,
  ] = await Promise.all([
    productionUiFiles(resolve(uiRoot, 'components')),
    cssFiles(resolve(uiRoot, 'styles')),
    testFiles(resolve(uiRoot, 'components')),
    testFiles(resolve(uiRoot, 'hooks')),
    testFiles(resolve(uiRoot, 'lib')),
    productionUiFiles(resolve(userRoot, 'src/components/ui')),
    productionUiFiles(resolve(adminRoot, 'src/components/ui')),
    testFiles(resolve(userRoot, 'src/components/ui')),
    testFiles(resolve(adminRoot, 'src/components/ui')),
    testFiles(resolve(userRoot, 'src/hooks')),
    testFiles(resolve(adminRoot, 'src/hooks')),
    testFiles(resolve(userRoot, 'src/lib')),
    testFiles(resolve(adminRoot, 'src/lib')),
    readFile(resolve(userRoot, 'src/styles/globals.css'), 'utf8'),
    readFile(resolve(adminRoot, 'src/styles/globals.css'), 'utf8'),
  ]);

  const failures = [
    ...assertExactSet('canonical @v2board/ui primitives', sharedFiles, SHARED_UI),
    ...assertExactSet('canonical @v2board/ui stylesheets', sharedStyles, SHARED_STYLES),
    ...assertRequiredSet('canonical @v2board/ui component tests', sharedUiTests, SHARED_UI_TESTS),
    ...assertRequiredSet('canonical @v2board/ui hook tests', sharedHookTests, SHARED_HOOK_TESTS),
    ...assertRequiredSet('canonical @v2board/ui library tests', sharedLibTests, SHARED_LIB_TESTS),
    ...assertExactSet('user app-specific UI primitives', userFiles, APP_SPECIFIC_UI.user),
    ...assertExactSet('admin app-specific UI primitives', adminFiles, APP_SPECIFIC_UI.admin),
    ...assertExactSet('user app-specific UI tests', userUiTests, APP_SPECIFIC_UI_TESTS.user),
    ...assertExactSet('admin app-specific UI tests', adminUiTests, APP_SPECIFIC_UI_TESTS.admin),
    ...assertNoSharedTestSubjects('user hooks', userHookTests, SHARED_HOOK_TEST_SUBJECTS),
    ...assertNoSharedTestSubjects('admin hooks', adminHookTests, SHARED_HOOK_TEST_SUBJECTS),
    ...assertNoSharedTestSubjects(
      'user libraries',
      userLibTests,
      APP_FORBIDDEN_LIB_TEST_SUBJECTS,
    ),
    ...assertNoSharedTestSubjects(
      'admin libraries',
      adminLibTests,
      APP_FORBIDDEN_LIB_TEST_SUBJECTS,
    ),
    ...assertSharedStyleImports('user', userGlobals),
    ...assertSharedStyleImports('admin', adminGlobals),
  ];

  return {
    appSpecificCount: APP_SPECIFIC_UI.user.length + APP_SPECIFIC_UI.admin.length,
    failures,
    sharedPrimitiveCount: sharedFiles.length,
    sharedStylesheetCount: sharedStyles.length,
    sharedTestCount: sharedUiTests.length + sharedHookTests.length + sharedLibTests.length,
  };
}

export function formatUiSyncSuccess(result) {
  const appSpecificLabel =
    result.appSpecificCount === 1 ? 'app-specific primitive remains' : 'app-specific primitives remain';
  return (
    `UI ownership audit OK: ${result.sharedPrimitiveCount} canonical @v2board/ui primitives and ` +
    `${result.sharedStylesheetCount} canonical stylesheets are shared; ` +
    `${result.sharedTestCount} shared tests have single package ownership; ` +
    `${result.appSpecificCount} ${appSpecificLabel} local.`
  );
}

export function assertExactSet(name, actual, expected) {
  const actualSet = new Set(actual);
  const expectedSet = new Set(expected);
  const unexpected = actual.filter((file) => !expectedSet.has(file));
  const missing = expected.filter((file) => !actualSet.has(file));
  const failures = [];

  if (unexpected.length > 0) failures.push(`${name} contains unexpected files: ${unexpected.join(', ')}`);
  if (missing.length > 0) failures.push(`${name} is missing files: ${missing.join(', ')}`);
  return failures;
}

export function assertRequiredSet(name, actual, expected) {
  const actualSet = new Set(actual);
  const missing = expected.filter((file) => !actualSet.has(file));
  return missing.length > 0 ? [`${name} is missing files: ${missing.join(', ')}`] : [];
}

export function assertNoSharedTestSubjects(name, actual, sharedSubjects) {
  const sharedSet = new Set(sharedSubjects);
  const duplicates = actual.filter((file) => {
    const subject = file.replace(/\.(?:test|spec)\.(?:ts|tsx)$/, '');
    return sharedSet.has(subject);
  });
  return duplicates.length > 0
    ? [`${name} duplicates package-owned tests: ${duplicates.join(', ')}`]
    : [];
}

function assertSharedStyleImports(app, globals) {
  const failures = [];
  for (const stylesheet of SHARED_STYLES) {
    const specifier = `@import '@v2board/ui/styles/${stylesheet}';`;
    if (globals.split(specifier).length !== 2) {
      failures.push(`${app} globals must import ${specifier} exactly once`);
    }
  }
  if (!globals.includes("@source '../../../../packages/ui/src/**/*.{ts,tsx}';")) {
    failures.push(`${app} globals must scan production @v2board/ui TypeScript sources`);
  }
  return failures;
}

async function productionUiFiles(root) {
  return (await readdir(root))
    .filter((file) => /\.(?:ts|tsx)$/.test(file) && !/\.(?:test|spec)\./.test(file))
    .sort();
}

export async function testFiles(root) {
  let entries;
  try {
    entries = await readdir(root);
  } catch (error) {
    // Git does not preserve empty directories. Once the last app-owned hook
    // moved into @v2board/ui, a clean checkout legitimately had no src/hooks
    // directory at all; that means "no duplicate tests", not an audit crash.
    if (error && typeof error === 'object' && error.code === 'ENOENT') return [];
    throw error;
  }
  return entries
    .filter((file) => /\.(?:test|spec)\.(?:ts|tsx)$/.test(file))
    .sort();
}

async function cssFiles(root) {
  return (await readdir(root)).filter((file) => file.endsWith('.css')).sort();
}

function getDefaultProjectRoot() {
  return resolve(dirname(fileURLToPath(import.meta.url)), '../..');
}

function isMainModule() {
  return Boolean(process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]));
}

if (isMainModule()) {
  const result = await auditUiSync();
  if (result.failures.length > 0) {
    console.error(`UI ownership audit failed:\n${result.failures.join('\n\n')}`);
    process.exit(1);
  }
  console.log(formatUiSyncSuccess(result));
}

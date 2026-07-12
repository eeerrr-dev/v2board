#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// A primitive that exists in only one app must be a deliberate product-level
// exception. Everything present in both directories is one shared shadcn
// primitive and therefore must stay byte-for-byte identical.
const APP_SPECIFIC_UI = {
  // Admin alone renders operational charts, grouped numeric inputs, and edits
  // notice/server metadata.
  admin: ['chart.tsx', 'input-group.tsx', 'tags-input.tsx'],
  // User alone renders the dashboard's notice/traffic surfaces and checkout
  // option cards.
  user: ['carousel.tsx', 'progress.tsx', 'radio-group.tsx'],
};

// App filenames remain local so shadcn can continue to own generated source in
// each app. These pairs are nevertheless one design system and must not drift.
const SHARED_STYLE_PAIRS = [
  ['user-shadcn.css', 'admin-shadcn.css'],
  ['user-theme.css', 'admin-theme.css'],
];

export async function auditUiSync(projectRoot = getDefaultProjectRoot()) {
  const userUiRoot = resolve(projectRoot, 'frontend/apps/user/src/components/ui');
  const adminUiRoot = resolve(projectRoot, 'frontend/apps/admin/src/components/ui');
  const userStyleRoot = resolve(projectRoot, 'frontend/apps/user/src/styles');
  const adminStyleRoot = resolve(projectRoot, 'frontend/apps/admin/src/styles');

  const [userFiles, adminFiles] = await Promise.all([
    productionUiFiles(userUiRoot),
    productionUiFiles(adminUiRoot),
  ]);
  const userSet = new Set(userFiles);
  const adminSet = new Set(adminFiles);
  const sharedFiles = userFiles.filter((file) => adminSet.has(file));
  const failures = [
    ...assertExactSet(
      'user-only UI primitives',
      userFiles.filter((file) => !adminSet.has(file)),
      APP_SPECIFIC_UI.user,
    ),
    ...assertExactSet(
      'admin-only UI primitives',
      adminFiles.filter((file) => !userSet.has(file)),
      APP_SPECIFIC_UI.admin,
    ),
  ];

  for (const file of sharedFiles) {
    failures.push(
      ...(await assertSameFile(
        `shared UI primitive ${file}`,
        resolve(userUiRoot, file),
        resolve(adminUiRoot, file),
      )),
    );
  }

  for (const [userFile, adminFile] of SHARED_STYLE_PAIRS) {
    failures.push(
      ...(await assertSameFile(
        `shared shadcn stylesheet ${userFile} / ${adminFile}`,
        resolve(userStyleRoot, userFile),
        resolve(adminStyleRoot, adminFile),
      )),
    );
  }

  return {
    appSpecificCount: APP_SPECIFIC_UI.user.length + APP_SPECIFIC_UI.admin.length,
    failures,
    sharedPrimitiveCount: sharedFiles.length,
    sharedStylesheetCount: SHARED_STYLE_PAIRS.length,
  };
}

export function formatUiSyncSuccess(result) {
  const appSpecificLabel =
    result.appSpecificCount === 1 ? 'app-specific primitive is' : 'app-specific primitives are';
  return (
    `UI sync audit OK: ${result.sharedPrimitiveCount} shared shadcn primitives and ` +
    `${result.sharedStylesheetCount} shared stylesheets are identical; ` +
    `${result.appSpecificCount} ${appSpecificLabel} explicitly declared.`
  );
}

export function assertExactSet(name, actual, expected) {
  const actualSet = new Set(actual);
  const expectedSet = new Set(expected);
  const undeclared = actual.filter((file) => !expectedSet.has(file));
  const stale = expected.filter((file) => !actualSet.has(file));
  const failures = [];

  if (undeclared.length > 0) {
    failures.push(`${name} contains undeclared files: ${undeclared.join(', ')}`);
  }
  if (stale.length > 0) {
    failures.push(
      `${name} allowlist contains files that are no longer app-specific: ${stale.join(', ')}`,
    );
  }
  return failures;
}

async function assertSameFile(name, firstPath, secondPath) {
  const [first, second] = await Promise.all([
    readFile(firstPath, 'utf8'),
    readFile(secondPath, 'utf8'),
  ]);
  return first === second
    ? []
    : [
        `${name} drifted. Keep the two app-owned copies identical, or document a real app-specific primitive instead.`,
      ];
}

async function productionUiFiles(root) {
  return (await readdir(root))
    .filter((file) => /\.(?:ts|tsx)$/.test(file) && !/\.(?:test|spec)\./.test(file))
    .sort();
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
    console.error(`UI sync audit failed:\n${result.failures.join('\n\n')}`);
    process.exit(1);
  }
  console.log(formatUiSyncSuccess(result));
}

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  assertExactSet,
  assertNoLocalAppShellFiles,
  assertNoSharedTestSubjects,
  assertRequiredSet,
  directoryEntries,
  testFiles,
} from './ui-sync-audit.mjs';

test('a clean checkout may omit an app hooks directory with no remaining tracked files', async () => {
  const missing = new URL('./__missing-hooks-directory__', import.meta.url);
  assert.deepEqual(await testFiles(missing), []);
});

test('a clean checkout may omit an app directory entirely', async () => {
  const missing = new URL('./__missing-directory__', import.meta.url);
  assert.deepEqual(await directoryEntries(missing), []);
});

test('directoryEntries lists every entry in sorted order, unfiltered by extension', async () => {
  const entries = await directoryEntries(new URL('.', import.meta.url));
  assert.ok(entries.includes('ui-sync-audit.mjs'));
  assert.deepEqual(entries, [...entries].sort());
});

test('app UI ownership rejects tests outside the product-specific allowlist', () => {
  assert.deepEqual(
    assertExactSet(
      'user app-specific UI tests',
      ['button.test.tsx', 'carousel.test.tsx'],
      ['carousel.test.tsx'],
    ),
    ['user app-specific UI tests contains unexpected files: button.test.tsx'],
  );
});

test('package ownership requires canonical shared tests while allowing future package tests', () => {
  assert.deepEqual(
    assertRequiredSet(
      'canonical @v2board/ui component tests',
      ['button.test.tsx', 'new-primitive.test.tsx'],
      ['button.test.tsx'],
    ),
    [],
  );
  assert.deepEqual(
    assertRequiredSet('canonical @v2board/ui hook tests', [], ['use-mobile.test.tsx']),
    ['canonical @v2board/ui hook tests is missing files: use-mobile.test.tsx'],
  );
});

test('shared hook and platform subjects cannot return under test or spec filenames', () => {
  assert.deepEqual(
    assertNoSharedTestSubjects(
      'user hooks',
      ['business-hook.test.ts', 'use-mobile.spec.tsx'],
      ['use-mobile'],
    ),
    ['user hooks duplicates package-owned tests: use-mobile.spec.tsx'],
  );
  assert.deepEqual(
    assertNoSharedTestSubjects(
      'admin libraries',
      ['runtime-config.test.ts', 'dark-mode.test.ts', 'toast.spec.ts'],
      ['dark-mode', 'toast'],
    ),
    ['admin libraries duplicates package-owned tests: dark-mode.test.ts, toast.spec.ts'],
  );
});

test('@v2board/app-shell modules cannot silently re-drift into a per-app lib or components copy', () => {
  assert.deepEqual(
    assertNoLocalAppShellFiles(
      'user',
      ['runtime-config.ts', 'toast.ts', 'chunk-recovery.test.ts'],
      ['route-error-boundary.tsx'],
    ),
    ['user app duplicates package-owned @v2board/app-shell files: toast.ts, chunk-recovery.test.ts'],
  );
  assert.deepEqual(
    assertNoLocalAppShellFiles(
      'admin',
      ['runtime-config.ts'],
      ['app-shell-boundary.tsx', 'step-up-dialog.tsx'],
    ),
    ['admin app duplicates package-owned @v2board/app-shell files: app-shell-boundary.tsx'],
  );
  assert.deepEqual(assertNoLocalAppShellFiles('user', ['runtime-config.ts'], ['auth.tsx']), []);
});

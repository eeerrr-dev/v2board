import assert from 'node:assert/strict';
import test from 'node:test';
import {
  assertExactSet,
  assertNoSharedTestSubjects,
  assertRequiredSet,
  testFiles,
} from './ui-sync-audit.mjs';

test('a clean checkout may omit an app hooks directory with no remaining tracked files', async () => {
  const missing = new URL('./__missing-hooks-directory__', import.meta.url);
  assert.deepEqual(await testFiles(missing), []);
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

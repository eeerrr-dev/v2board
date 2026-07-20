import assert from 'node:assert/strict';
import test from 'node:test';

import {
  assertSourceOnlyAllowlist,
  assertUsefulInteractionCoverage,
} from './parity-config-audit.mjs';

test('source-only exceptions fail closed against an independent allowlist', () => {
  const interactions = [
    { label: 'covered', sourceOnly: true },
    { label: 'ordinary' },
  ];
  assert.deepEqual(assertSourceOnlyAllowlist(interactions, ['covered']), []);
  assert.match(
    assertSourceOnlyAllowlist(interactions, [])[0],
    /has labels not defined|missing labels/,
  );
});

test('every interaction needs an exact or intentional prefix assertion', () => {
  const interactions = [{ label: 'exact' }, { label: 'family-one' }];
  const source = `label === 'exact'; label.startsWith('family-');`;
  assert.deepEqual(assertUsefulInteractionCoverage(interactions, source), []);
  assert.match(
    assertUsefulInteractionCoverage([...interactions, { label: 'silent' }], source)[0],
    /silent/,
  );
  assert.match(
    assertUsefulInteractionCoverage([{ label: 'exact' }], source)[0],
    /family-\*/,
  );
});

import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  forbiddenLegacyNames,
  hashedAssetNamePattern,
  runtimeConfigToken,
} from './deploy-contract.mjs';

// This corpus is mirrored byte-for-byte by the Rust runtime gate's test
// (backend/rust/crates/api/src/routes.rs, public_asset_gate_accepts_only_
// flat_content_hashed_files). Build-time acceptance must stay a strict subset
// of runtime acceptance: every name the build certifies, Rust must serve.
const servedByBothSides = [
  'index-Dp3_abcdef.js',
  'asset-a1b2c3d4.woff2',
  'logo.dark-a1b2c3d4.png',
  'roboto-v30-latin-regular-a1b2c3d4.woff2',
];

const rejectedAtBuildTime = [
  // Dotted extension chains parse differently on the two sides ('.js' would
  // land inside Rust's hash segment), so the build refuses to certify them.
  'chunk-abcdefgh.js.map',
  'asset-a1b2c3d4.js.LICENSE.txt',
  'index.html',
  'manifest.json',
  'umi.js',
  'nested/index-a1b2c3d4.js',
  'index-abc.js',
  'index-abcdefgh.',
  '-abcdefgh.js',
];

test('hashed asset grammar accepts the shared served corpus', () => {
  for (const name of servedByBothSides) {
    assert.ok(hashedAssetNamePattern.test(name), `expected acceptance: ${name}`);
  }
});

test('hashed asset grammar rejects names the runtime gate cannot serve', () => {
  for (const name of rejectedAtBuildTime) {
    assert.ok(!hashedAssetNamePattern.test(name), `expected rejection: ${name}`);
  }
});

test('extension segment stays single and dot-free (Rust last-dot parse compatibility)', () => {
  // The grammar's extension class must never re-admit '.', or a
  // build-certified name could 404 at runtime (see deploy-contract.mjs).
  assert.equal(String(hashedAssetNamePattern).includes('\\.[A-Za-z0-9]+$'), true);
});

test('contract constants keep their pinned shapes', () => {
  assert.equal(runtimeConfigToken, '__V2BOARD_RUNTIME_CONFIG__');
  assert.equal(forbiddenLegacyNames.length, 8);
  assert.ok(Object.isFrozen(forbiddenLegacyNames));
});

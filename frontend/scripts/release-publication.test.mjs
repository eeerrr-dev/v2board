import assert from 'node:assert/strict';
import { access, mkdir, mkdtemp, readlink, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { publishReleaseLinks } from './release-publication.mjs';

const CONTENT_A = 'aaaaaaaaaaaaaaaaaaaa';
const CONTENT_B = 'bbbbbbbbbbbbbbbbbbbb';
const CONTENT_C = 'cccccccccccccccccccc';

test('publication bootstraps no-history fallback and then rotates exactly two generations', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'v2board-release-publication-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const releases = join(root, 'releases');
  await mkdir(releases);

  await mkdir(join(releases, CONTENT_A));
  assert.equal(await publishReleaseLinks(root, `releases/${CONTENT_A}`), `releases/${CONTENT_A}`);
  assert.equal(await readlink(join(root, 'current')), `releases/${CONTENT_A}`);
  assert.equal(await readlink(join(root, 'previous')), `releases/${CONTENT_A}`);

  await rm(join(root, 'previous'));
  assert.equal(await publishReleaseLinks(root, `releases/${CONTENT_A}`), `releases/${CONTENT_A}`);
  assert.equal(await readlink(join(root, 'previous')), `releases/${CONTENT_A}`);

  await mkdir(join(releases, CONTENT_B));
  assert.equal(await publishReleaseLinks(root, `releases/${CONTENT_B}`), `releases/${CONTENT_A}`);
  assert.equal(await readlink(join(root, 'current')), `releases/${CONTENT_B}`);
  assert.equal(await readlink(join(root, 'previous')), `releases/${CONTENT_A}`);

  // An idempotent rebuild must not erase the actual previous generation.
  assert.equal(await publishReleaseLinks(root, `releases/${CONTENT_B}`), `releases/${CONTENT_A}`);

  await mkdir(join(releases, CONTENT_C));
  assert.equal(await publishReleaseLinks(root, `releases/${CONTENT_C}`), `releases/${CONTENT_B}`);
  assert.equal(await readlink(join(root, 'current')), `releases/${CONTENT_C}`);
  assert.equal(await readlink(join(root, 'previous')), `releases/${CONTENT_B}`);
  await assert.rejects(access(join(releases, CONTENT_A)));
});

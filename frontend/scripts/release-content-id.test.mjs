import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { releaseIdFromBuilds } from './release-content-id.mjs';

test('release content ID binds canonical paths and actual bytes', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'v2board-release-content-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const user = join(root, 'user');
  const admin = join(root, 'admin');
  await mkdir(user);
  await mkdir(admin);
  await writeFile(join(user, 'index.html'), '<html>user</html>');
  await writeFile(join(user, 'index-aaaaaaaa.js'), 'first bytes');
  await writeFile(join(admin, 'index.html'), '<html>admin</html>');

  const builds = [
    { name: 'user', outDir: user, files: ['index.html', 'index-aaaaaaaa.js'] },
    { name: 'admin', outDir: admin, files: ['index.html'] },
  ];
  const initial = await releaseIdFromBuilds(builds);
  const reordered = await releaseIdFromBuilds([
    { ...builds[0], files: [...builds[0].files].reverse() },
    builds[1],
  ]);
  assert.equal(reordered, initial, 'caller enumeration order must not affect the content ID');

  await writeFile(join(user, 'renamed-aaaaaaaa.js'), 'first bytes');
  const repathed = await releaseIdFromBuilds([
    {
      ...builds[0],
      files: ['index.html', 'renamed-aaaaaaaa.js'],
    },
    builds[1],
  ]);
  assert.notEqual(repathed, initial, 'canonical file paths are part of the content ID');

  // Keep the Vite-looking filename unchanged: the builder must bind the bytes
  // itself rather than trusting an upstream filename hash convention.
  await writeFile(join(user, 'index-aaaaaaaa.js'), 'mutated bytes');
  const mutated = await releaseIdFromBuilds(builds);
  assert.notEqual(mutated, initial);
});

test('release content ID rejects paths that can escape or alias the release tree', async () => {
  await assert.rejects(
    releaseIdFromBuilds([{ name: 'user', outDir: '/tmp', files: ['../escape'] }]),
    /not canonical and relative/,
  );
});

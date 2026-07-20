import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const generator = path.resolve('scripts/generate-internal-api-contract.mjs');

async function runGeneratorWithResponses(responses) {
  const root = await mkdtemp(path.join(tmpdir(), 'v2board-api-contract-'));
  try {
    const specDirectory = path.join(root, 'packages/api-client/openapi');
    await mkdir(specDirectory, { recursive: true });
    await writeFile(
      path.join(specDirectory, 'internal-api.openapi.json'),
      JSON.stringify({
        openapi: '3.1.0',
        components: { schemas: {} },
        paths: {
          '/api/v1/example': {
            get: { operationId: 'exampleGet', responses },
          },
        },
      }),
    );
    return spawnSync(process.execPath, [generator, `--root=${root}`], {
      cwd: process.cwd(),
      encoding: 'utf8',
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test('checked-in internal API bindings match the Rust-exported OpenAPI document', () => {
  const result = spawnSync(
    process.execPath,
    ['scripts/generate-internal-api-contract.mjs', '--check'],
    { cwd: process.cwd(), encoding: 'utf8' },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
});

test('generated Zod integer schemas do not use the deprecated safe modifier', async () => {
  const generated = await readFile('packages/api-client/src/generated/internal-api.ts', 'utf8');

  assert.doesNotMatch(generated, /\.safe\(\)/);
});

test('generator rejects multiple exact 2xx responses instead of selecting one', async () => {
  const result = await runGeneratorWithResponses({
    200: { description: 'OK' },
    204: { description: 'No content' },
  });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr || result.stdout, /exactly one supported 2xx response/);
});

test('generator rejects an ambiguous OpenAPI 2XX response key', async () => {
  const result = await runGeneratorWithResponses({
    '2XX': { description: 'Any success' },
  });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr || result.stdout, /must pin an exact success status/);
});
